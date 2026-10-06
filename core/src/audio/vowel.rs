//! Vowel detection by MFCC template matching.
//!
//! We do **not** try to measure formant frequencies. Pulling clean formants out
//! of a child's high-pitched, short-vocal-tract voice is a known ill-posed
//! problem (LPC reports false formants at high f0), and it was the source of the
//! detector's past unreliability. Instead we compare the *shape of the spectral
//! envelope* — the standard, robust representation for closed-set,
//! speaker-dependent vowel recognition.
//!
//! Pipeline (per analysis window):
//!
//! ```text
//! window -> voicing gate (RMS) -> MFCC (mel filterbank + DCT, c0 dropped)
//!        -> cepstral-mean normalization (running channel estimate)
//!        -> nearest calibrated template (diagonal Mahalanobis) -> vowel
//! ```
//!
//! Each child calibrates their six vowels once; that builds six MFCC templates
//! (mean + per-dimension variance). Detection matches the child against *their
//! own* templates, recorded on the *same* microphone — so the mic's fixed
//! coloration cancels on both sides (cepstral-mean normalization removes what's
//! left). This is why no microphone frequency-sweep calibration is needed.
//!
//! MFCC features come from the `spectrograms` crate (pure-Rust FFT). If this ever
//! needs to grow — onset/pitch detection, or a second opinion on the feature
//! front-end — swap the feature source for `aubio` via `aubio-rs` (battle-tested
//! C library); the template/classifier code here stays the same.

use non_empty_slice::NonEmptySlice;
use serde::{Deserialize, Serialize};
use spectrograms::{MfccParams, StftParams, WindowType, mfcc};
use std::num::NonZeroUsize;

/// The Polish oral vowels we classify, in a fixed order (indices line up with
/// [`VowelResult::scores`] and a calibration's templates).
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
}

// ---- MFCC feature extraction -------------------------------------------------

/// Requested MFCC coefficients; c0 (overall energy, which tracks mic gain) is
/// then dropped, leaving [`N_MFCC`] usable dimensions.
const MFCC_REQUEST: usize = 13;
/// Usable MFCC dimensions after dropping c0.
pub const N_MFCC: usize = MFCC_REQUEST - 1;
/// Mel filterbank bands (must be >= `MFCC_REQUEST`).
const N_MELS: usize = 26;
/// Sinusoidal liftering (standard speech value).
const LIFTER: usize = 22;

fn nz(n: usize) -> NonZeroUsize {
    NonZeroUsize::new(n).expect("non-zero MFCC/STFT parameter")
}

/// FFT size for the analysis rate: ~25 ms, a power of two, capped so short
/// windows still yield a frame.
fn fft_size(fs: u32) -> usize {
    if fs >= 32_000 {
        1024
    } else if fs >= 16_000 {
        512
    } else {
        256
    }
}

/// One MFCC vector ([`N_MFCC`] dims) for a window, averaged over its STFT frames.
/// `None` when the window is too short or the transform fails.
pub fn mfcc_frame(window: &[f32], fs: u32) -> Option<Vec<f32>> {
    let fft = fft_size(fs);
    if window.len() < fft || fs == 0 {
        return None;
    }
    let samples = NonEmptySlice::new(window)?;
    let stft = StftParams::new(nz(fft), nz(fft / 2), WindowType::Hanning, true).ok()?;
    // c0 dropped (gain), liftering on: MFCC_REQUEST in -> N_MFCC out.
    let params = MfccParams::new(nz(MFCC_REQUEST))
        .with_c0(false)
        .with_lifter(LIFTER);
    let m = mfcc::<f32>(samples, &stft, fs as f64, nz(N_MELS), &params).ok()?;
    let arr = &*m; // Array2<f32> [coeffs, frames]
    let (ncoeff, nframes) = (arr.nrows(), arr.ncols());
    if ncoeff == 0 || nframes == 0 {
        return None;
    }
    let mut out = vec![0.0f32; ncoeff];
    for (c, o) in out.iter_mut().enumerate() {
        let mut s = 0.0f32;
        for f in 0..nframes {
            s += arr[[c, f]];
        }
        *o = s / nframes as f32;
    }
    Some(out)
}

/// RMS of a window (used by the voicing gate and level checks).
pub fn rms(window: &[f32]) -> f32 {
    if window.is_empty() {
        return 0.0;
    }
    (window.iter().map(|x| x * x).sum::<f32>() / window.len() as f32).sqrt()
}

// ---- Calibration model -------------------------------------------------------

/// Bumped whenever the on-disk calibration shape changes. Older files (including
/// the pre-MFCC formant format, which had no `version`) are treated as "not
/// calibrated" and the child is asked to recalibrate.
pub const CALIBRATION_VERSION: u32 = 2;

/// One vowel's MFCC template: per-dimension mean and variance over the
/// (channel-normalized) calibration frames.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VowelTemplate {
    pub mean: Vec<f32>,
    pub var: Vec<f32>,
}

/// A profile's calibrated vowel detector: six MFCC templates plus the channel
/// (mic) estimate they were built against.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VowelCalibration {
    pub version: u32,
    /// One template per vowel, in [`VOWELS`] order.
    pub templates: Vec<VowelTemplate>,
    /// Mean MFCC across all calibration frames — the microphone/channel estimate
    /// subtracted from every template, and used to seed the live detector's
    /// running channel mean.
    pub channel_mean: Vec<f32>,
    pub n_mfcc: usize,
    /// Sample rate the templates were built at (informational).
    pub sample_rate: u32,
    /// Input device used at calibration, for the "you changed mics" warning.
    #[serde(default)]
    pub input_device: Option<String>,
    /// The quietest vowel's median calibration RMS. Drives the adaptive
    /// voicing gate; 0.0 (older files) = gate uses the settings slider alone.
    #[serde(default)]
    pub min_vowel_rms: f32,
    pub created: u64,
}

impl VowelCalibration {
    /// A loaded calibration is usable only if it matches the current version and
    /// dimensionality and has all six templates.
    pub fn is_valid(&self) -> bool {
        self.version == CALIBRATION_VERSION
            && self.n_mfcc == N_MFCC
            && self.templates.len() == VOWELS.len()
            && self.channel_mean.len() == N_MFCC
            && self
                .templates
                .iter()
                .all(|t| t.mean.len() == N_MFCC && t.var.len() == N_MFCC)
    }
}

/// Minimum voiced MFCC frames before a vowel capture is accepted.
pub const MIN_CAPTURE_SAMPLES: usize = 10;

/// Accumulates per-frame MFCC vectors while a child holds a vowel. Unvoiced
/// frames are simply never pushed by the caller.
#[derive(Default)]
pub struct CalibrationCapture {
    frames: Vec<Vec<f32>>,
    /// RMS of each accepted window, parallel to `frames` — feeds the adaptive
    /// voicing gate (quiet vowels like /i/ carry intrinsically less energy).
    rms: Vec<f32>,
}

impl CalibrationCapture {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed one [`mfcc_frame`] result and its window's RMS; `None` (unvoiced /
    /// failed) is ignored.
    pub fn push(&mut self, frame: Option<Vec<f32>>, rms: f32) {
        if let Some(f) = frame {
            self.frames.push(f);
            self.rms.push(rms);
        }
    }

    pub fn count(&self) -> usize {
        self.frames.len()
    }

    pub fn clear(&mut self) {
        self.frames.clear();
        self.rms.clear();
    }

    /// The captured frames + per-frame RMS (consumed when a vowel is accepted).
    pub fn take(&mut self) -> (Vec<Vec<f32>>, Vec<f32>) {
        (
            std::mem::take(&mut self.frames),
            std::mem::take(&mut self.rms),
        )
    }
}

/// Per-dimension mean over a set of vectors.
fn mean_vec(frames: &[Vec<f32>], dims: usize) -> Vec<f32> {
    let mut mean = vec![0.0f32; dims];
    if frames.is_empty() {
        return mean;
    }
    for f in frames {
        for (m, &x) in mean.iter_mut().zip(f.iter()) {
            *m += x;
        }
    }
    for m in &mut mean {
        *m /= frames.len() as f32;
    }
    mean
}

/// Build one template from a vowel's frames after subtracting the shared channel
/// mean. Drops frames far from the centroid (glide-in/out, throat clears) before
/// computing mean + variance. A small variance floor keeps the distance stable.
fn build_template(frames: &[Vec<f32>], channel_mean: &[f32]) -> VowelTemplate {
    let dims = N_MFCC;
    // Channel-normalize.
    let cmn: Vec<Vec<f32>> = frames
        .iter()
        .map(|f| {
            (0..dims)
                .map(|i| f.get(i).copied().unwrap_or(0.0) - channel_mean[i])
                .collect()
        })
        .collect();

    // Trim frames beyond 1.5x the median distance from the centroid.
    let centroid = mean_vec(&cmn, dims);
    let mut dists: Vec<f32> = cmn
        .iter()
        .map(|f| {
            f.iter()
                .zip(&centroid)
                .map(|(a, b)| (a - b) * (a - b))
                .sum::<f32>()
                .sqrt()
        })
        .collect();
    let med = {
        let mut d = dists.clone();
        d.sort_by(|a, b| a.partial_cmp(b).unwrap());
        d.get(d.len() / 2).copied().unwrap_or(0.0)
    };
    let cutoff = (med * 1.5).max(1e-6);
    let kept: Vec<&Vec<f32>> = cmn
        .iter()
        .zip(dists.drain(..))
        .filter(|(_, d)| *d <= cutoff)
        .map(|(f, _)| f)
        .collect();
    let kept: Vec<Vec<f32>> = if kept.len() >= MIN_CAPTURE_SAMPLES {
        kept.into_iter().cloned().collect()
    } else {
        cmn.clone()
    };

    let mean = mean_vec(&kept, dims);
    let mut var = vec![0.0f32; dims];
    for f in &kept {
        for i in 0..dims {
            let e = f[i] - mean[i];
            var[i] += e * e;
        }
    }
    let n = kept.len().max(1) as f32;
    for v in &mut var {
        *v = (*v / n).max(VAR_FLOOR);
    }
    VowelTemplate { mean, var }
}

/// Variance floor: no MFCC dimension is treated as more certain than this, so a
/// nearly-constant coefficient can't dominate the distance.
const VAR_FLOOR: f32 = 0.25;

/// Build a full calibration from six vowels' captured MFCC frames (in [`VOWELS`]
/// order). Returns `None` if any vowel has too few frames.
pub fn build_calibration(
    per_vowel: &[Vec<Vec<f32>>],
    per_vowel_rms: &[Vec<f32>],
    sample_rate: u32,
    input_device: Option<String>,
    created: u64,
) -> Option<VowelCalibration> {
    if per_vowel.len() != VOWELS.len() {
        return None;
    }
    if per_vowel.iter().any(|f| f.len() < MIN_CAPTURE_SAMPLES) {
        return None;
    }
    // Channel estimate = mean MFCC across every calibration frame.
    let all: Vec<Vec<f32>> = per_vowel.iter().flatten().cloned().collect();
    let channel_mean = mean_vec(&all, N_MFCC);

    let templates: Vec<VowelTemplate> = per_vowel
        .iter()
        .map(|frames| build_template(frames, &channel_mean))
        .collect();

    // The quietest vowel's median RMS: close vowels (/i/, /u/) carry
    // intrinsically less energy, so the voicing gate must respect the child's
    // softest vowel rather than demand /a/-level loudness for everything.
    let min_vowel_rms = per_vowel_rms
        .iter()
        .filter(|r| !r.is_empty())
        .map(|r| {
            let mut v = r.clone();
            v.sort_by(|a, b| a.total_cmp(b));
            v[v.len() / 2]
        })
        .fold(f32::MAX, f32::min);
    let min_vowel_rms = if min_vowel_rms == f32::MAX {
        0.0
    } else {
        min_vowel_rms
    };

    Some(VowelCalibration {
        version: CALIBRATION_VERSION,
        templates,
        channel_mean,
        n_mfcc: N_MFCC,
        sample_rate,
        input_device,
        min_vowel_rms,
        created,
    })
}

// ---- Live detection ----------------------------------------------------------

/// Adaptive score sharpness: the softmax temperature scales with the gap
/// between the two closest templates, so peakiness — and therefore how fast a
/// smoothed score crosses the show threshold — does not depend on the
/// calibration's variance scale. Without this, adding takes (honest, wider
/// variances → smaller distances) made every score flatter and detection
/// visibly laggier. The floor keeps genuinely ambiguous frames ambiguous.
const SOFTMAX_BETA: f32 = 0.5;
const SOFTMAX_MIN_TEMP: f32 = 8.0;
/// Steady mode default: classify the average of this many recent voiced
/// frames instead of each ~46 ms frame alone. Sustained-vowel noise shrinks by
/// ~sqrt(n), which is what separates close pairs; costs ~0.1-0.25 s of onset
/// latency. Games override this via the pre-game Reaction slider.
const STEADY_FRAMES: usize = 8;

/// Outcome of analysing one window.
#[derive(Clone, Debug)]
pub struct VowelResult {
    /// Per-vowel match in `0.0..=1.0`, indexed like [`VOWELS`]. All zero when
    /// unvoiced or uncalibrated.
    pub scores: [f32; 6],
    /// Instantaneous best match (raw argmax), or `None` when there's nothing to
    /// match. The visualizer applies smoothing + show/margin thresholds on top of
    /// `scores`, so it reads these only in tests / other consumers.
    #[allow(dead_code)]
    pub best: Option<Vowel>,
    /// Top score.
    #[allow(dead_code)]
    pub confidence: f32,
    /// Top score minus the runner-up — the separation the toy is built around.
    #[allow(dead_code)]
    pub margin: f32,
}

impl VowelResult {
    fn silent() -> Self {
        Self {
            scores: [0.0; 6],
            best: None,
            confidence: 0.0,
            margin: 0.0,
        }
    }
}

/// Stateful vowel detector: holds the active profile's templates and the
/// calibration's channel mean for cepstral-mean normalization (fixed at
/// runtime — see the note in `analyze`). One lives in the vowel
/// visualizer; calibration builds the templates it consumes.
pub struct VowelDetector {
    templates: Option<Vec<VowelTemplate>>,
    channel_ema: Option<Vec<f32>>,
    /// Recent channel-normalized frames for steady mode.
    recent: std::collections::VecDeque<Vec<f32>>,
    steady: bool,
    steady_frames: usize,
    /// Quietest vowel's calibration RMS (0.0 = unknown, use the slider alone).
    min_rms: f32,
}

impl Default for VowelDetector {
    fn default() -> Self {
        Self {
            templates: None,
            channel_ema: None,
            recent: std::collections::VecDeque::new(),
            steady: true,
            steady_frames: STEADY_FRAMES,
            min_rms: 0.0,
        }
    }
}

impl VowelDetector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Toggle steady mode (feature averaging over recent voiced frames).
    pub fn set_steady(&mut self, on: bool) {
        if self.steady != on {
            self.recent.clear();
        }
        self.steady = on;
    }

    /// How many recent voiced frames steady mode averages (the reaction /
    /// stability trade-off; clamped to 1..=16).
    pub fn set_steady_frames(&mut self, n: usize) {
        self.steady_frames = n.clamp(1, 16);
        while self.recent.len() > self.steady_frames {
            self.recent.pop_front();
        }
    }

    /// Install (or clear) the active calibration. Seeds the running channel mean
    /// from the calibration so early frames match well before it adapts.
    pub fn set_calibration(&mut self, cal: Option<&VowelCalibration>) {
        match cal {
            Some(c) if c.is_valid() => {
                self.templates = Some(c.templates.clone());
                self.channel_ema = Some(c.channel_mean.clone());
                self.min_rms = c.min_vowel_rms;
            }
            _ => {
                self.templates = None;
                self.channel_ema = None;
                self.min_rms = 0.0;
            }
        }
    }

    pub fn is_calibrated(&self) -> bool {
        self.templates.is_some()
    }

    /// Analyse a window: voicing gate, MFCC, channel normalization, classify.
    ///
    /// The gate adapts to the calibration: quiet vowels (/i/, /u/) carry
    /// intrinsically less energy than /a/, so when the calibration knows the
    /// child's quietest vowel, the gate opens at 60% of that — the settings
    /// slider only acts as a ceiling.
    pub fn analyze(&mut self, window: &[f32], fs: u32, voicing_threshold: f32) -> VowelResult {
        let threshold = if self.min_rms > 0.0 {
            voicing_threshold.min(self.min_rms * 0.6)
        } else {
            voicing_threshold
        };
        if rms(window) < threshold {
            // A pause ends the utterance; don't smear it into the next one.
            self.recent.clear();
            return VowelResult::silent();
        }
        let Some(templates) = self.templates.as_ref() else {
            return VowelResult::silent();
        };
        let Some(raw) = mfcc_frame(window, fs) else {
            return VowelResult::silent();
        };

        // Normalize by the *calibration's* channel mean — and never adapt it
        // at runtime. A running EMA here can only ever observe voiced speech,
        // so it inevitably absorbs the held vowel itself and subtracts it
        // away (regression test: sustained_vowel_does_not_drift_away — with
        // adaptation, a 10 s "aaa" flips to "o"). Mic changes are what
        // recalibration is for.
        let ema = self.channel_ema.get_or_insert_with(|| raw.clone());
        if ema.len() != raw.len() {
            *ema = raw.clone();
        }
        let cmn: Vec<f32> = raw.iter().zip(ema.iter()).map(|(&x, &m)| x - m).collect();

        if self.steady {
            self.recent.push_back(cmn);
            if self.recent.len() > self.steady_frames {
                self.recent.pop_front();
            }
            let n = self.recent.len() as f32;
            let avg: Vec<f32> = (0..N_MFCC)
                .map(|k| self.recent.iter().map(|f| f[k]).sum::<f32>() / n)
                .collect();
            classify(&avg, templates)
        } else {
            classify(&cmn, templates)
        }
    }
}

/// Score `cmn` against each template by diagonal-Mahalanobis distance, softmax
/// the negative distances into `0..=1` scores, and report the best + margin.
fn classify(cmn: &[f32], templates: &[VowelTemplate]) -> VowelResult {
    let mut dists = [f32::MAX; 6];
    for (i, t) in templates.iter().enumerate().take(6) {
        let mut d = 0.0f32;
        for k in 0..N_MFCC {
            let e = cmn.get(k).copied().unwrap_or(0.0) - t.mean[k];
            d += e * e / t.var[k];
        }
        dists[i] = d;
    }
    // Softmax over -distance/temp, temp scaled by the winner/runner-up gap.
    let min_d = dists.iter().copied().fold(f32::MAX, f32::min);
    let second_d = dists
        .iter()
        .copied()
        .filter(|&d| d > min_d)
        .fold(f32::MAX, f32::min);
    let temp = if second_d == f32::MAX {
        SOFTMAX_MIN_TEMP
    } else {
        (SOFTMAX_BETA * (second_d - min_d)).max(SOFTMAX_MIN_TEMP)
    };
    let mut scores = [0.0f32; 6];
    let mut sum = 0.0f32;
    for (s, &d) in scores.iter_mut().zip(dists.iter()) {
        let v = (-(d - min_d) / temp).exp();
        *s = v;
        sum += v;
    }
    if sum > 0.0 {
        for s in &mut scores {
            *s /= sum;
        }
    }

    // Best and runner-up.
    let mut best = 0usize;
    for i in 1..6 {
        if scores[i] > scores[best] {
            best = i;
        }
    }
    let second = (0..6)
        .filter(|&i| i != best)
        .map(|i| scores[i])
        .fold(0.0f32, f32::max);
    VowelResult {
        scores,
        best: Some(VOWELS[best]),
        confidence: scores[best],
        margin: scores[best] - second,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FS: u32 = 44_100;

    /// A one-pole-pair resonator (formant), applied in place.
    fn resonate(x: &mut [f32], f: f32, fs: u32, r: f32) {
        let theta = 2.0 * std::f32::consts::PI * f / fs as f32;
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

    /// Synthesise a vowel: an `f0` glottal impulse train through F1/F2/F3
    /// resonators (a minimal source-filter model). `f0 = 300` mimics a child.
    fn synth(f0: f32, f1: f32, f2: f32, f3: f32, n: usize) -> Vec<f32> {
        let mut x = vec![0.0f32; n];
        let period = (FS as f32 / f0) as usize;
        let mut i = 0;
        while i < n {
            x[i] = 1.0;
            i += period.max(1);
        }
        resonate(&mut x, f1, FS, 0.99);
        resonate(&mut x, f2, FS, 0.99);
        resonate(&mut x, f3, FS, 0.985);
        let peak = x.iter().fold(0.0f32, |m, v| m.max(v.abs())).max(1e-6);
        for s in &mut x {
            *s /= peak;
        }
        x
    }

    /// Child-ish formants (Hz) — higher than adult (short vocal tract).
    fn formants(v: Vowel) -> (f32, f32, f32) {
        match v {
            Vowel::A => (1000.0, 1600.0, 3600.0),
            Vowel::E => (700.0, 2400.0, 3600.0),
            Vowel::I => (400.0, 3000.0, 3800.0),
            Vowel::O => (700.0, 1200.0, 3400.0),
            Vowel::U => (450.0, 1000.0, 3200.0),
            Vowel::Y => (550.0, 2100.0, 3600.0),
        }
    }

    /// A microphone-coloration filter applied to a signal in place.
    type Color<'a> = Option<&'a dyn Fn(&mut [f32])>;

    /// A vowel window (last 2048 samples), optionally passed through `color`.
    fn vowel_window(v: Vowel, jitter: f32, color: Color) -> Vec<f32> {
        let (f1, f2, f3) = formants(v);
        let mut sig = synth(300.0, f1 * jitter, f2 * jitter, f3, 4096);
        if let Some(c) = color {
            c(&mut sig);
        }
        sig[sig.len() - 2048..].to_vec()
    }

    /// Capture several jittered frames per vowel and build a calibration.
    fn synth_calibration(color: Color) -> VowelCalibration {
        let mut per_vowel: Vec<Vec<Vec<f32>>> = Vec::new();
        let mut per_rms: Vec<Vec<f32>> = Vec::new();
        for &v in &VOWELS {
            let mut frames = Vec::new();
            let mut rmss = Vec::new();
            for k in 0..14 {
                let jitter = 1.0 + (k as f32 - 7.0) * 0.006;
                let w = vowel_window(v, jitter, color);
                rmss.push(rms(&w));
                frames.push(mfcc_frame(&w, FS).expect("mfcc"));
            }
            per_vowel.push(frames);
            per_rms.push(rmss);
        }
        build_calibration(&per_vowel, &per_rms, FS, Some("synth".into()), 0).expect("calibration")
    }

    fn detector(cal: &VowelCalibration) -> VowelDetector {
        let mut d = VowelDetector::new();
        d.set_calibration(Some(cal));
        d
    }

    #[test]
    fn classifies_each_calibrated_vowel() {
        let cal = synth_calibration(None);
        let mut det = detector(&cal);
        for &v in &VOWELS {
            // A beat of silence between utterances, as in real speech (it
            // resets steady mode's rolling average).
            det.analyze(&vec![0.0; 2048], FS, 0.001);
            let w = vowel_window(v, 1.0, None);
            let res = det.analyze(&w, FS, 0.001);
            assert_eq!(res.best, Some(v), "misclassified {:?}", v.label());
            assert!(
                res.margin > 0.1,
                "weak margin for {:?}: {}",
                v.label(),
                res.margin
            );
        }
    }

    #[test]
    fn gain_invariant() {
        // Overall level (mic gain) must not change the vowel: c0 is dropped and
        // CMN removes channel offset.
        let cal = synth_calibration(None);
        let mut det = detector(&cal);
        for &v in &VOWELS {
            det.analyze(&vec![0.0; 2048], FS, 0.0001);
            let mut w = vowel_window(v, 1.0, None);
            for s in &mut w {
                *s *= 0.2;
            }
            assert_eq!(det.analyze(&w, FS, 0.0001).best, Some(v), "{:?}", v.label());
        }
    }

    #[test]
    fn coloration_cancels_when_same_on_both_sides() {
        // A fixed mic coloration applied to BOTH calibration and live input must
        // cancel — the reason a mic frequency-sweep is unnecessary.
        let tilt = |x: &mut [f32]| {
            let mut prev = 0.0f32;
            for s in x.iter_mut() {
                let y = *s - 0.5 * prev;
                prev = *s;
                *s = y;
            }
        };
        let cal = synth_calibration(Some(&tilt));
        let mut det = detector(&cal);
        for &v in &VOWELS {
            det.analyze(&vec![0.0; 2048], FS, 0.001);
            let w = vowel_window(v, 1.0, Some(&tilt));
            assert_eq!(det.analyze(&w, FS, 0.001).best, Some(v), "{:?}", v.label());
        }
    }

    #[test]
    fn silence_is_unvoiced() {
        let cal = synth_calibration(None);
        let mut det = detector(&cal);
        let res = det.analyze(&vec![0.0; 4096], FS, 0.01);
        assert!(res.best.is_none());
        assert_eq!(res.scores, [0.0; 6]);
    }

    #[test]
    fn uncalibrated_detects_nothing() {
        let mut det = VowelDetector::new();
        let w = vowel_window(Vowel::A, 1.0, None);
        assert!(det.analyze(&w, FS, 0.001).best.is_none());
    }

    #[test]
    fn capture_needs_minimum_frames() {
        let mut c = CalibrationCapture::new();
        for _ in 0..(MIN_CAPTURE_SAMPLES - 1) {
            c.push(Some(vec![0.0; N_MFCC]), 0.02);
        }
        assert!(c.count() < MIN_CAPTURE_SAMPLES);
        c.push(Some(vec![0.0; N_MFCC]), 0.02);
        assert_eq!(c.count(), MIN_CAPTURE_SAMPLES);
        c.push(None, 0.0); // unvoiced ignored
        assert_eq!(c.count(), MIN_CAPTURE_SAMPLES);
    }

    #[test]
    fn build_calibration_rejects_thin_vowels() {
        let mut per_vowel: Vec<Vec<Vec<f32>>> = vec![vec![vec![0.0; N_MFCC]; 20]; 6];
        per_vowel[2].truncate(3); // one vowel too thin
        let per_rms: Vec<Vec<f32>> = vec![vec![0.02; 20]; 6];
        assert!(build_calibration(&per_vowel, &per_rms, FS, None, 0).is_none());
    }

    #[test]
    fn calibration_validity_checks_shape() {
        let cal = synth_calibration(None);
        assert!(cal.is_valid());
        let mut bad = cal.clone();
        bad.version = 0;
        assert!(!bad.is_valid());
    }

    #[test]
    fn sustained_vowel_does_not_drift_away() {
        // The old CHANNEL_ALPHA=0.02 made the running channel mean absorb a
        // held vowel within seconds, dissolving close pairs. Holding "a" for
        // ~10 s (300 voiced frames) must keep classifying as "a" with a
        // healthy margin to the end.
        let cal = synth_calibration(None);
        let mut det = detector(&cal);
        let w = vowel_window(Vowel::A, 1.0, None);
        let mut last = VowelResult::silent();
        for _ in 0..300 {
            last = det.analyze(&w, FS, 0.001);
        }
        assert_eq!(last.best, Some(Vowel::A), "drifted off 'a' during a hold");
        assert!(
            last.margin > 0.1,
            "margin collapsed during hold: {}",
            last.margin
        );
    }

    #[test]
    fn quiet_i_passes_the_adaptive_gate() {
        // /i/ carries intrinsically less energy than /a/; the gate must open
        // at 60% of the calibration's quietest vowel even when the settings
        // slider is set far higher.
        let cal = synth_calibration(None);
        assert!(cal.min_vowel_rms > 0.0);
        let mut det = detector(&cal);

        let w = vowel_window(Vowel::I, 1.0, None);
        let scale = (0.7 * cal.min_vowel_rms) / rms(&w);
        let quiet: Vec<f32> = w.iter().map(|s| s * scale).collect();
        // Slider demands twice the quietest vowel's RMS — without adaptation
        // this window would read as silence.
        let slider = cal.min_vowel_rms * 2.0;
        assert!(rms(&quiet) < slider);
        let res = det.analyze(&quiet, FS, slider);
        assert_eq!(res.best, Some(Vowel::I), "quiet /i/ was gated out");

        // But truly-below-the-adaptive-gate stays silent.
        let too_quiet: Vec<f32> = quiet.iter().map(|s| s * 0.4).collect();
        assert_eq!(det.analyze(&too_quiet, FS, slider).best, None);
    }

    #[test]
    fn score_sharpness_survives_wider_variances() {
        // More calibration takes -> honestly wider variances -> smaller
        // Mahalanobis distances. The adaptive softmax temperature must keep
        // scores similarly peaky, or detection turns sluggish (the "11 takes
        // feels laggy" bug).
        let cal = synth_calibration(None);
        let mut wide = cal.clone();
        for t in &mut wide.templates {
            for v in &mut t.var {
                *v *= 4.0;
            }
        }
        let mut det_a = detector(&cal);
        let mut det_b = detector(&wide);
        let w = vowel_window(Vowel::A, 1.0, None);
        det_a.analyze(&vec![0.0; 2048], FS, 0.001);
        det_b.analyze(&vec![0.0; 2048], FS, 0.001);
        let normal = det_a.analyze(&w, FS, 0.001);
        let wide_res = det_b.analyze(&w, FS, 0.001);
        assert_eq!(wide_res.best, Some(Vowel::A));
        assert!(
            wide_res.margin > normal.margin * 0.6,
            "margin collapsed under wider variances: {} vs {}",
            wide_res.margin,
            normal.margin
        );
    }
}
