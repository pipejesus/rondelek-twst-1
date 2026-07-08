//! Vowel detection by formant estimation.
//!
//! Vowels are characterised by their formant frequencies — the resonances of
//! the vocal tract. F1 and F2 alone place a vowel in the classic vowel space, so
//! we estimate them with **Linear Predictive Coding** (the standard method for
//! modelling the speech spectral envelope) and map the result to the nearest
//! Polish vowel prototype.
//!
//! Pipeline (all pure Rust, no external DSP dependency — see the developer
//! handbook for why a crate was evaluated but not adopted):
//!
//! ```text
//! window -> voicing gate -> decimate ~8 kHz -> pre-emphasis + Hamming
//!        -> autocorrelation -> Levinson-Durbin (LPC) -> polynomial roots
//!        -> formant poles (F1, F2) -> nearest Polish vowel
//! ```
//!
//! The detector is deliberately decoupled from any visualizer: it takes a slice
//! of samples and returns a [`VowelResult`], so different visualizers (a debug
//! meter today, a playful kid view later) can share it.

use rustfft::num_complex::Complex;
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

/// The Polish oral vowels we classify, in a fixed order (indices line up with
/// [`VowelResult::scores`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vowel {
    A,
    E,
    I,
    O,
    U,
    Y,
}

/// All vowels in scoring order.
pub const VOWELS: [Vowel; 6] = [Vowel::A, Vowel::E, Vowel::I, Vowel::O, Vowel::U, Vowel::Y];

impl Vowel {
    /// Single-letter Polish label.
    pub fn label(self) -> &'static str {
        match self {
            Vowel::A => "a",
            Vowel::E => "e",
            Vowel::I => "i",
            Vowel::O => "o",
            Vowel::U => "u",
            Vowel::Y => "y",
        }
    }

    /// Reference **adult** (F1, F2) in Hz. Child formants run higher, so the
    /// detector scales these by `speaker_scale` (see [`VowelConfig`]).
    fn prototype(self) -> (f32, f32) {
        match self {
            Vowel::A => (730.0, 1200.0),
            Vowel::E => (530.0, 1840.0),
            Vowel::I => (300.0, 2200.0),
            Vowel::O => (520.0, 920.0),
            Vowel::U => (330.0, 720.0),
            Vowel::Y => (420.0, 1600.0),
        }
    }
}

/// Per-vowel (F1, F2) target set in Hz, indexed like [`VOWELS`]. Either the
/// scaled reference set ([`default_prototypes`]) or a profile's calibrated set.
pub type Prototypes = [(f32, f32); 6];

/// The reference prototypes scaled for a speaker — the fallback used until a
/// profile is calibrated. A larger `speaker_scale` suits younger children
/// (shorter vocal tract → higher formants).
pub fn default_prototypes(speaker_scale: f32) -> Prototypes {
    let mut out = [(0.0, 0.0); 6];
    for (slot, v) in out.iter_mut().zip(VOWELS.iter()) {
        let (f1, f2) = v.prototype();
        *slot = (f1 * speaker_scale, f2 * speaker_scale);
    }
    out
}

/// The corner vowels a child produces during calibration. These anchor the map
/// from the reference vowel space into the child's (see [`normalize_from_corners`]).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Corners {
    pub a: (f32, f32),
    pub i: (f32, f32),
    pub u: (f32, f32),
}

/// A profile's calibrated vowel detector.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VowelCalibration {
    /// Personalised (F1, F2) target per vowel, indexed like [`VOWELS`].
    pub prototypes: Prototypes,
    /// Raw corner measurements, kept so targets can be re-derived if the
    /// reference set or mapping changes later.
    pub corners: Corners,
    pub created: u64,
}

/// Derive personalised vowel targets from a child's corner vowels.
///
/// A per-axis linear map takes the **reference** vowel space into the **child's**:
/// F1 is anchored on `/a/` (high) vs the mean of `/i/,/u/` (low), F2 on `/i/`
/// (high) vs `/u/` (low). The map is then applied to *all six* reference
/// prototypes, so every target — including `/y/` — stays phonetically correct,
/// just scaled to the child's vocal tract (never learning a mispronunciation).
pub fn normalize_from_corners(c: Corners) -> Prototypes {
    let (ra1, _) = Vowel::A.prototype();
    let (ri1, ri2) = Vowel::I.prototype();
    let (ru1, ru2) = Vowel::U.prototype();

    // F1: /a/ is the high anchor, mean(/i/, /u/) the low anchor.
    let (m1, b1) = fit_line((ri1 + ru1) * 0.5, (c.i.0 + c.u.0) * 0.5, ra1, c.a.0);
    // F2: /i/ is the high anchor, /u/ the low anchor.
    let (m2, b2) = fit_line(ru2, c.u.1, ri2, c.i.1);

    let mut out = default_prototypes(1.0);
    for (slot, v) in out.iter_mut().zip(VOWELS.iter()) {
        let (f1, f2) = v.prototype();
        slot.0 = (m1 * f1 + b1).max(50.0);
        slot.1 = (m2 * f2 + b2).max(100.0);
    }
    out
}

/// Line through `(x0, y0)`–`(x1, y1)` as `(slope, intercept)`. Falls back to the
/// identity map on degenerate or implausible input (noisy corners), so a bad
/// calibration yields the reference set rather than nonsense.
fn fit_line(x0: f32, y0: f32, x1: f32, y1: f32) -> (f32, f32) {
    let dx = x1 - x0;
    if dx.abs() < 1e-3 {
        return (1.0, 0.0);
    }
    let m = (y1 - y0) / dx;
    if !m.is_finite() || !(0.4..=2.6).contains(&m) {
        return (1.0, 0.0);
    }
    (m, y0 - m * x0)
}

/// Minimum stable samples before a corner-vowel capture is accepted.
const MIN_CAPTURE_SAMPLES: usize = 10;

/// Accumulates per-frame formant readings while a child holds a vowel, then
/// reduces them to a robust `(F1, F2)` via an outlier-trimmed median — so the
/// steady middle of the vowel wins over the glide in and out. Unvoiced frames
/// are ignored.
#[derive(Default)]
pub struct CalibrationCapture {
    f1: Vec<f32>,
    f2: Vec<f32>,
}

impl CalibrationCapture {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed one [`analyze`] result's formants; `None` (unvoiced) is ignored.
    pub fn push(&mut self, formants: Option<(f32, f32)>) {
        if let Some((f1, f2)) = formants {
            self.f1.push(f1);
            self.f2.push(f2);
        }
    }

    pub fn count(&self) -> usize {
        self.f1.len()
    }

    pub fn clear(&mut self) {
        self.f1.clear();
        self.f2.clear();
    }

    /// The robust `(F1, F2)` once enough stable samples are gathered, else `None`.
    pub fn result(&self) -> Option<(f32, f32)> {
        if self.f1.len() < MIN_CAPTURE_SAMPLES {
            return None;
        }
        Some((trimmed_median(&self.f1), trimmed_median(&self.f2)))
    }
}

/// Median after discarding samples more than 20% from the raw median.
fn trimmed_median(xs: &[f32]) -> f32 {
    let m = median(xs);
    let kept: Vec<f32> = xs
        .iter()
        .copied()
        .filter(|x| (x - m).abs() <= 0.2 * m)
        .collect();
    if kept.is_empty() { m } else { median(&kept) }
}

fn median(xs: &[f32]) -> f32 {
    let mut v = xs.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

/// Tunable detector parameters.
#[derive(Clone, Copy, Debug)]
pub struct VowelConfig {
    /// RMS below this is treated as silence / unvoiced (no vowel).
    pub voicing_threshold: f32,
    /// The vowel targets to match against (scaled reference or calibrated).
    pub prototypes: Prototypes,
}

impl Default for VowelConfig {
    fn default() -> Self {
        Self {
            voicing_threshold: 0.012,
            prototypes: default_prototypes(1.25),
        }
    }
}

/// Outcome of analysing one window.
#[derive(Clone, Copy, Debug)]
pub struct VowelResult {
    /// Estimated `(F1, F2)` in Hz, or `None` when unvoiced.
    pub formants: Option<(f32, f32)>,
    /// Per-vowel match in `0.0..=1.0`, indexed like [`VOWELS`]. All zero when
    /// unvoiced.
    pub scores: [f32; 6],
    /// The instantaneous best match, or `None` when unvoiced. (The bundled
    /// visualizer derives its own best from *smoothed* scores; this raw value is
    /// exposed for other consumers and tests.)
    #[allow(dead_code)]
    pub best: Option<Vowel>,
}

impl VowelResult {
    fn silent() -> Self {
        Self {
            formants: None,
            scores: [0.0; 6],
            best: None,
        }
    }
}

const ANALYSIS_RATE: u32 = 8_000; // Nyquist ~4 kHz covers F1..F3; low order = robust roots
const FMIN: f32 = 150.0;
const FMAX: f32 = 3200.0;
const MAX_BANDWIDTH: f32 = 600.0; // wider poles are spectral shaping, not formants
/// Formant poles sit near the unit circle. This floor rejects wide/degenerate
/// roots (including any the root finder fails to converge).
const MIN_POLE_RADIUS: f64 = 0.5;
const SCORE_SIGMA: f32 = 0.35; // spread of the match scores, in log-frequency units

/// Analyse a window of mono samples and return the vowel match.
pub fn analyze(samples: &[f32], fs: u32, cfg: &VowelConfig) -> VowelResult {
    if samples.len() < 256 || fs == 0 {
        return VowelResult::silent();
    }

    // Voicing gate on the raw window.
    let rms = (samples.iter().map(|x| x * x).sum::<f32>() / samples.len() as f32).sqrt();
    if rms < cfg.voicing_threshold {
        return VowelResult::silent();
    }

    let (sig, fs_a) = decimate(samples, fs);
    if sig.len() < 64 {
        return VowelResult::silent();
    }
    let order = (2 + (fs_a / 1000) as usize).clamp(8, 14);

    let pre = preemphasis_and_window(&sig);
    let r = autocorrelation(&pre, order);
    if r[0] <= 0.0 {
        return VowelResult::silent();
    }
    let a = levinson(&r, order);

    let formants = formant_freqs(&a, fs_a as f32);
    let (f1, f2) = match (formants.first(), formants.get(1)) {
        (Some(&f1), Some(&f2)) => (f1, f2),
        _ => return VowelResult::silent(),
    };

    let (scores, best) = classify(f1, f2, &cfg.prototypes);
    VowelResult {
        formants: Some((f1, f2)),
        scores,
        best,
    }
}

/// Downsample to ~[`ANALYSIS_RATE`], keeping the LPC order sane and focusing on
/// the formant band. A biquad low-pass below the new Nyquist prevents high
/// frequencies from aliasing into the formants. Returns the signal and new rate.
fn decimate(samples: &[f32], fs: u32) -> (Vec<f32>, u32) {
    let factor = (fs / ANALYSIS_RATE).max(1) as usize;
    if factor == 1 {
        return (samples.to_vec(), fs);
    }
    let fs_a = fs / factor as u32;
    let filtered = lowpass(samples, fs as f32, 0.45 * fs_a as f32);
    let out: Vec<f32> = filtered.iter().step_by(factor).copied().collect();
    (out, fs_a)
}

/// RBJ biquad low-pass (Butterworth Q), applied forward as a simple anti-alias.
fn lowpass(x: &[f32], fs: f32, fc: f32) -> Vec<f32> {
    let w0 = 2.0 * PI * fc / fs;
    let (sin, cos) = (w0.sin(), w0.cos());
    let alpha = sin / (2.0 * std::f32::consts::FRAC_1_SQRT_2); // Q = 1/sqrt(2)
    let a0 = 1.0 + alpha;
    let (b0, b1, b2) = (
        (1.0 - cos) / 2.0 / a0,
        (1.0 - cos) / a0,
        (1.0 - cos) / 2.0 / a0,
    );
    let (a1, a2) = (-2.0 * cos / a0, (1.0 - alpha) / a0);
    let (mut x1, mut x2, mut y1, mut y2) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    x.iter()
        .map(|&xn| {
            let yn = b0 * xn + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
            x2 = x1;
            x1 = xn;
            y2 = y1;
            y1 = yn;
            yn
        })
        .collect()
}

/// Pre-emphasis (boost the higher formants) then a Hamming window.
fn preemphasis_and_window(x: &[f32]) -> Vec<f32> {
    let n = x.len();
    let mut out = Vec::with_capacity(n);
    let mut prev = x[0];
    for (i, &s) in x.iter().enumerate() {
        let emph = s - 0.63 * prev;
        prev = s;
        let w = 0.54 - 0.46 * (2.0 * PI * i as f32 / (n as f32 - 1.0)).cos();
        out.push(emph * w);
    }
    out
}

fn autocorrelation(x: &[f32], p: usize) -> Vec<f32> {
    (0..=p)
        .map(|k| {
            x[..x.len() - k]
                .iter()
                .zip(&x[k..])
                .map(|(a, b)| a * b)
                .sum()
        })
        .collect()
}

/// Levinson-Durbin recursion. Returns `a[0..=order]` with `a[0] = 1`, defining
/// `A(z) = sum a[k] z^-k`; the envelope is `1 / |A|`.
fn levinson(r: &[f32], order: usize) -> Vec<f32> {
    let mut a = vec![0.0f32; order + 1];
    a[0] = 1.0;
    let mut err = r[0];
    for i in 1..=order {
        let mut acc = r[i];
        for j in 1..i {
            acc += a[j] * r[i - j];
        }
        let k = -acc / err;
        // Update a[1..i] from the previous iteration's values (clone avoids the
        // in-place symmetric-aliasing pitfall).
        let prev = a.clone();
        for j in 1..i {
            a[j] = prev[j] + k * prev[i - j];
        }
        a[i] = k;
        err *= 1.0 - k * k;
        if err <= 0.0 {
            break;
        }
    }
    a
}

/// Formant frequencies (Hz, ascending) from the roots of the LPC polynomial.
/// Each conjugate pole pair is a resonance: its angle gives the frequency and its
/// radius the bandwidth. Narrow-band poles in the formant range are formants;
/// wide-band poles (spectral tilt) are rejected. Unlike envelope peak-picking,
/// this cleanly separates close formants such as `/a/`'s F1 and F2.
fn formant_freqs(a: &[f32], fs: f32) -> Vec<f32> {
    let coeffs: Vec<Complex<f64>> = a.iter().map(|&x| Complex::new(x as f64, 0.0)).collect();
    // Every stable conjugate pole in the formant band, as (frequency, bandwidth).
    let mut poles: Vec<(f32, f32)> = Vec::new();
    for z in durand_kerner(&coeffs) {
        if z.im <= 0.0 {
            continue; // one root per conjugate pair
        }
        let radius = z.norm();
        if !(MIN_POLE_RADIUS..1.0).contains(&radius) {
            continue; // must be a resonant pole near (but inside) the unit circle
        }
        let freq = z.arg() as f32 * fs / (2.0 * PI);
        let bandwidth = -(fs / PI) * radius.ln() as f32;
        if (FMIN..=FMAX).contains(&freq) {
            poles.push((freq, bandwidth));
        }
    }
    poles.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap());

    // Prefer the narrow (resonant) poles; if close formants have merged into
    // fewer than two, fall back to the lowest in-band poles so F1/F2 still exist.
    let narrow: Vec<f32> = poles
        .iter()
        .filter(|(_, bw)| *bw < MAX_BANDWIDTH)
        .map(|(f, _)| *f)
        .collect();
    if narrow.len() >= 2 {
        narrow
    } else {
        poles.into_iter().map(|(f, _)| f).collect()
    }
}

/// Durand-Kerner (Weierstrass) simultaneous root finder for the polynomial
/// `a[0] z^p + a[1] z^(p-1) + ... + a[p]`.
fn durand_kerner(a: &[Complex<f64>]) -> Vec<Complex<f64>> {
    let p = a.len() - 1;
    if p == 0 {
        return Vec::new();
    }
    // Spread initial guesses around a circle, offset to break symmetry.
    let mut roots: Vec<Complex<f64>> = (0..p)
        .map(|k| Complex::from_polar(0.8, 2.0 * std::f64::consts::PI * k as f64 / p as f64 + 0.35))
        .collect();
    for _ in 0..200 {
        let mut delta_max = 0.0f64;
        for i in 0..p {
            let numer = horner(a, roots[i]);
            let mut denom = Complex::new(1.0, 0.0);
            for j in 0..p {
                if j != i {
                    denom *= roots[i] - roots[j];
                }
            }
            if denom.norm() < 1e-30 {
                continue;
            }
            let step = numer / denom;
            roots[i] -= step;
            delta_max = delta_max.max(step.norm());
        }
        if delta_max < 1e-10 {
            break;
        }
    }
    roots
}

/// Horner evaluation of `a[0] z^p + a[1] z^(p-1) + ... + a[p]`.
fn horner(a: &[Complex<f64>], z: Complex<f64>) -> Complex<f64> {
    let mut acc = a[0];
    for c in &a[1..] {
        acc = acc * z + c;
    }
    acc
}

/// Score every vowel from the measured `(F1, F2)` against `prototypes`, and
/// return the best. Distance is in log-frequency space (scale-robust and
/// perceptually saner).
fn classify(f1: f32, f2: f32, prototypes: &Prototypes) -> ([f32; 6], Option<Vowel>) {
    let mut scores = [0.0f32; 6];
    let mut best = (0usize, f32::MIN);
    for (i, &(p1, p2)) in prototypes.iter().enumerate() {
        let d1 = (f1 / p1).ln();
        let d2 = (f2 / p2).ln();
        let d = (d1 * d1 + d2 * d2).sqrt();
        let s = (-(d * d) / (2.0 * SCORE_SIGMA * SCORE_SIGMA)).exp();
        scores[i] = s;
        if s > best.1 {
            best = (i, s);
        }
    }
    (scores, Some(VOWELS[best.0]))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-pole-pair resonator (formant) applied in place.
    fn resonate(x: &mut [f32], f: f32, fs: u32, r: f32) {
        let theta = 2.0 * PI * f / fs as f32;
        let a1 = 2.0 * r * theta.cos();
        let a2 = -r * r;
        let (mut y1, mut y2) = (0.0f32, 0.0f32);
        for s in x.iter_mut() {
            let y = *s + a1 * y1 + a2 * y2;
            y2 = y1;
            y1 = y;
            *s = y;
        }
    }

    /// Synthesise a vowel: a 120 Hz glottal impulse train through resonators at
    /// F1/F2/F3 (a minimal source-filter model).
    fn synth(f1: f32, f2: f32, f3: f32, fs: u32, n: usize) -> Vec<f32> {
        let mut x = vec![0.0f32; n];
        let period = (fs as f32 / 120.0) as usize;
        let mut i = 0;
        while i < n {
            x[i] = 1.0;
            i += period.max(1);
        }
        resonate(&mut x, f1, fs, 0.97);
        resonate(&mut x, f2, fs, 0.96);
        resonate(&mut x, f3, fs, 0.95);
        // Normalise to a sensible amplitude.
        let peak = x.iter().fold(0.0f32, |m, v| m.max(v.abs())).max(1e-6);
        for s in &mut x {
            *s /= peak;
        }
        x
    }

    fn cfg() -> VowelConfig {
        // scale 1.0: synth at the adult prototypes, so classification is exact.
        VowelConfig {
            voicing_threshold: 0.001,
            prototypes: default_prototypes(1.0),
        }
    }

    fn detect_synth(v: Vowel, fs: u32) -> VowelResult {
        let (f1, f2) = v.prototype();
        let sig = synth(f1, f2, 2800.0, fs, 4096);
        analyze(&sig, fs, &cfg())
    }

    #[test]
    fn recovers_formants_for_a() {
        let res = detect_synth(Vowel::A, 16_000);
        let (f1, f2) = res.formants.expect("voiced");
        let (p1, p2) = Vowel::A.prototype();
        assert!((f1 - p1).abs() / p1 < 0.18, "F1 {f1} vs {p1}");
        assert!((f2 - p2).abs() / p2 < 0.18, "F2 {f2} vs {p2}");
    }

    #[test]
    fn classifies_each_vowel_at_16k() {
        for v in VOWELS {
            let res = detect_synth(v, 16_000);
            assert_eq!(res.best, Some(v), "misclassified {:?}", v.label());
        }
    }

    #[test]
    fn classifies_through_decimation_at_44k() {
        // 44.1 kHz exercises the decimation + anti-alias path (the real hardware
        // case), which must classify every vowel correctly.
        for v in VOWELS {
            let res = detect_synth(v, 44_100);
            assert_eq!(res.best, Some(v), "misclassified {:?} at 44k", v.label());
        }
    }

    #[test]
    fn capture_needs_enough_samples_then_averages() {
        let mut c = CalibrationCapture::new();
        for _ in 0..5 {
            c.push(Some((500.0, 1500.0)));
        }
        assert!(c.result().is_none(), "too few samples");
        for _ in 0..10 {
            c.push(Some((500.0, 1500.0)));
        }
        let (f1, f2) = c.result().unwrap();
        assert!((f1 - 500.0).abs() < 1.0 && (f2 - 1500.0).abs() < 1.0);
    }

    #[test]
    fn capture_rejects_onset_offset_outliers() {
        let mut c = CalibrationCapture::new();
        for _ in 0..20 {
            c.push(Some((500.0, 1500.0)));
        }
        c.push(Some((60.0, 250.0))); // silence-ish onset
        c.push(Some((2900.0, 4800.0))); // garbage offset
        let (f1, f2) = c.result().unwrap();
        assert!((f1 - 500.0).abs() < 20.0, "f1={f1}");
        assert!((f2 - 1500.0).abs() < 20.0, "f2={f2}");
    }

    #[test]
    fn capture_ignores_unvoiced_frames() {
        let mut c = CalibrationCapture::new();
        for _ in 0..12 {
            c.push(Some((400.0, 1600.0)));
            c.push(None);
        }
        assert_eq!(c.count(), 12);
    }

    #[test]
    fn corners_identity_gives_reference() {
        let c = Corners {
            a: Vowel::A.prototype(),
            i: Vowel::I.prototype(),
            u: Vowel::U.prototype(),
        };
        let p = normalize_from_corners(c);
        for (got, want) in p.iter().zip(default_prototypes(1.0).iter()) {
            assert!((got.0 - want.0).abs() < 5.0, "F1 {} vs {}", got.0, want.0);
            assert!((got.1 - want.1).abs() < 5.0, "F2 {} vs {}", got.1, want.1);
        }
    }

    #[test]
    fn corners_scaled_map_scales_all_targets() {
        // A child whose corners are 1.3x the reference should get 1.3x targets —
        // including /y/, whose target stays the *correct* y, just scaled.
        let s = 1.3f32;
        let sc = |(f1, f2): (f32, f32)| (f1 * s, f2 * s);
        let c = Corners {
            a: sc(Vowel::A.prototype()),
            i: sc(Vowel::I.prototype()),
            u: sc(Vowel::U.prototype()),
        };
        let p = normalize_from_corners(c);
        for (got, base) in p.iter().zip(default_prototypes(1.0).iter()) {
            assert!((got.0 - base.0 * s).abs() < base.0 * 0.05);
            assert!((got.1 - base.1 * s).abs() < base.1 * 0.05);
        }
    }

    #[test]
    fn silence_is_unvoiced() {
        let res = analyze(&vec![0.0; 4096], 16_000, &VowelConfig::default());
        assert!(res.best.is_none());
        assert!(res.formants.is_none());
    }

    #[test]
    fn best_score_is_highest() {
        let res = detect_synth(Vowel::I, 16_000);
        let best_idx = VOWELS.iter().position(|v| *v == res.best.unwrap()).unwrap();
        let max = res.scores.iter().cloned().fold(f32::MIN, f32::max);
        assert_eq!(res.scores[best_idx], max);
    }
}
