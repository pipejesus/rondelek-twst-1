# The visualizer

The visualizer is a **pluggable** subsystem: the app owns a
`Box<dyn Visualizer>`, and the current dot-matrix spectrum is just one
implementation (`SpectrumVisualizer`). New styles (waveform, VU meter, scope) can
be added without touching the app wiring. Everything lives in
`src/ui/visualizer.rs`.

## The interface

```rust
pub struct AudioFrame<'a> {
    pub input: &'a [f32],      // microphone tap
    pub playback: &'a [f32],   // playback monitor tap
    pub input_rate: u32,
    pub playback_rate: u32,
}

pub trait Visualizer {
    fn update(&mut self, frame: &AudioFrame, settings: &Settings);
    fn draw(&self, painter: &Painter, rect: Rect, theme: &Theme, settings: &Settings);
    fn demo_fill(&mut self, _num_bars: usize) {}   // for screenshots; default no-op
}
```

`App::drain_capture` builds an `AudioFrame` from both taps (see
[Audio routing](audio.md)) and calls `update` **only on frames that brought new
audio**, so the smoothing/decay cadence follows the sound rather than the render
frame rate. `draw` is called every frame from `draw_session`.

### Source selection

`SpectrumVisualizer::update` picks whichever tap is **louder** over the last FFT
window (`louder_window`). This automatically follows the sound with no coupling to
UI mode: the mic wins while recording, the (full-scale) playback monitor wins while
a clip plays. Ties favour the mic.

```mermaid
flowchart TD
    frame[AudioFrame: input + playback] --> e{louder window?}
    e -->|playback louder| p[process playback samples]
    e -->|else| m[process mic samples]
```

## The spectrum DSP pipeline

```mermaid
flowchart LR
    win[last 1024 samples] --> hann[Hann window]
    hann --> fft[FFT 1024]
    fft --> mag[linear magnitude per bin<br/>512 bins]
    mag --> bands[group into log-frequency bands<br/>bar_bin_range]
    bands --> db[dB response<br/>level_from_magnitude<br/>floor..ceiling]
    db --> smooth[attack / release smoothing]
    smooth --> draw[dot-matrix draw]
```

Three design decisions matter here, each learned from real behaviour:

1. **Logarithmic frequency bands** (`bar_bin_range`). FFT bins map *linearly* to
   frequency, but almost all speech energy sits below ~4 kHz (the bottom ~18% of
   bins). Linear grouping clumped all movement into the leftmost few bars. Log
   bands give each bar a geometrically wider frequency range, so the energy-dense
   low end spreads across many bars and the display fills at any bar count.

2. **Decibel response** (`level_from_magnitude`). Loudness is perceived
   logarithmically, so bar height is a dB mapping across a `[floor, ceiling]`
   window (ceiling fixed at −12 dB; floor user-tunable, default −60 dB) rather than
   a linear gain. Normal speech lands in the lively middle; loud input tops out
   gracefully instead of pinning.

3. **Attack / release smoothing**. Rises use `visualizer_smoothing` (fast attack),
   falls use `visualizer_decay` (slow release) — classic peak-meter behaviour.

All of this is **display-only** — it never touches recorded or played audio.

### Tunable knobs (dev panel)

Open the dev panel with `Ctrl+Shift+D`. It exposes:

| Control | Setting | Effect |
|---------|---------|--------|
| Smoothing | `visualizer_smoothing` | Attack speed (higher = smoother rise). |
| Decay | `visualizer_decay` | Release speed. |
| Bars | `visualizer_num_bars` | Number of columns. |
| Floor (dB) | `visualizer_floor_db` | Sensitivity — lower = more movement. |

These persist in the settings JSON (see [Data model](data-model.md)).

## Cycling between visualizers

The app owns a `Vec<Box<dyn Visualizer>>` plus an `active_visualizer` index. The
**square button just left of REC** (`layout.cycle`) calls `App::cycle_visualizer`,
which advances the index (wrapping around) and persists it to settings. Only the
active visualizer is fed audio (`update`) and drawn each frame. Two ship today:
the [spectrum](#the-spectrum-dsp-pipeline) and the [vowel meter](#vowel-detection).

## Vowel detection

`VowelVisualizer` (`ui/vowel_visualizer.rs`) is a developer/debug view that
validates the vowel pipeline: it shows the detected Polish vowel as a large
letter plus a match meter per vowel (**a e i o u y**), with the raw F1/F2 readout.
A kid-facing view will later reuse the same detector.

The detector lives in `audio/vowel.rs` and estimates **formants** — the vocal
tract resonances that define a vowel — via Linear Predictive Coding:

```mermaid
flowchart LR
    win[recent ~45 ms window] --> gate[voicing gate RMS]
    gate --> dec[decimate to ~8 kHz + anti-alias]
    dec --> pre[pre-emphasis + Hamming]
    pre --> ac[autocorrelation]
    ac --> lpc[Levinson-Durbin LPC]
    lpc --> roots[polynomial roots Durand-Kerner]
    roots --> form[formant poles F1 F2]
    form --> cls[nearest Polish vowel]
```

Key choices:

- **Root-finding, not envelope peaks.** Each conjugate pole pair is a resonance
  (frequency from its angle, bandwidth from its radius). This cleanly separates
  close formants such as `/a/`'s F1 and F2, which envelope peak-picking merges.
- **~8 kHz analysis rate** keeps the LPC order low (~10), which keeps the root
  finder well-conditioned; poles far from the unit circle are rejected.
- **Child-leaning prototypes.** Adult reference formants scaled by a
  `speaker_scale` (default 1.25); tune per voice in the dev panel.
- **Library note.** `loqa-voice-dsp` was evaluated (recent, pure Rust) but a
  compact in-tree estimator was chosen for zero extra dependencies and full
  unit-test coverage (synthetic vowels). It is the drop-in upgrade path.

Dev-panel controls (`Ctrl+Shift+D`): **Speaker scale**, **Voicing** threshold,
**Vowel smoothing**. The detector is unit-tested by synthesising source-filter
vowels at known formants and asserting the classification.

## Adding a new visualizer

1. Create a struct implementing `Visualizer` (`update` + `draw`, optionally
   `demo_fill`).
2. Consume whichever of `frame.input` / `frame.playback` you need — the rates are
   available for time/frequency-axis visualizers.
3. Add it to the `visualizers` vector in `App::new`; the cycle button picks it up
   automatically.

Because the app only knows the trait, nothing else has to change.
