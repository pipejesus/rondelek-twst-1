# Vowel Detector Calibration — Design & Plan

**Date:** 2026-07-05
**Status:** Plan — ready to execute next session
**Builds on:** the vowel detector (`src/audio/vowel.rs`) + `VowelVisualizer`.

## Why

The detector classifies against **fixed adult prototypes scaled by one global
`speaker_scale`**. That can't reliably separate the closest, most
speaker-dependent pair — Polish **i** vs **y** (distinguished almost only by F2)
— and every child's vocal tract differs. Real use (the author testing on their
own voice) confirmed it: personalisation is **essential**, not a nice-to-have.

## The pedagogical subtlety (drives the whole design)

This is a **practice/therapy** tool, so calibration must **not** simply learn the
child's *current* production. If we recorded the child's (possibly conflated)
`/y/` and called that "correct y", we'd reward the error and defeat the practice.

**Therefore: calibrate the child's vocal-tract *scale*, but keep the *targets*
phonetically correct.** Concretely:

1. Capture the **corner vowels** — `/a/`, `/i/`, `/u/` — which are the most
   distinct and easiest to produce, and which define the extremes of the vowel
   space (a = max F1; i = max F2, low F1; u = min F2, low F1).
2. Fit a simple per-axis linear map (independent F1 and F2 scale + offset) from
   the **reference** vowel space into **this child's** space, anchored on those
   corners.
3. Apply that map to **all six** reference prototypes → the child's personalised
   *but still correct* targets. So practising `/y/` means reaching the correct
   central-high position scaled to their anatomy — not their current `/i/`-like one.

Rejected alternative: calibrate all six vowels directly to the child's
production. Simpler, but rewards incorrect vowels — wrong for a therapy tool.

## Data model

New `VowelCalibration` stored **per profile** (each child calibrates once):

```rust
struct VowelCalibration {
    /// Personalised (F1, F2) target per vowel, indexed like `vowel::VOWELS`.
    prototypes: [(f32, f32); 6],
    /// Raw corner measurements kept so targets can be re-derived if the
    /// reference set or mapping changes later.
    corners: { a: (f32,f32), i: (f32,f32), u: (f32,f32) },
    created: u64,
}
```

Store as **`calibration.json` inside the profile folder** (sibling of
`profile.json`) — keeps `profile.json` lean and calibration optional. Load lazily
when a profile is selected.

## Detector changes (`src/audio/vowel.rs`)

- `classify(f1, f2, prototypes: &[(f32,f32); 6]) -> [f32;6]` — take the prototype
  set as a parameter instead of reading fixed values.
- `default_prototypes(speaker_scale) -> [(f32,f32);6]` — current adult×scale
  behaviour, used when a profile is uncalibrated.
- `analyze(...)` gains the prototype set (via `VowelConfig` or a new arg).
- New `normalize_from_corners(a, i, u) -> [(f32,f32);6]` — the per-axis linear
  map described above. Pure, fully unit-testable.

## Calibration capture

- `CalibrationCapture` accumulates per-frame formant readings (reusing
  `vowel::analyze`) while the child holds a vowel, and aggregates to a robust
  `(F1, F2)` via the **median** over N stable, voiced frames (median rejects
  onset/offset outliers).
- **Stability gate:** only count frames that are voiced *and* whose formants sit
  within a tolerance of the running median — so we capture the steady middle of
  the vowel, not the glide in/out. Fail gracefully (allow retry) if it never
  stabilises.

## UI flow

A guided calibration screen (kid-friendly, large type):

- **Entry point:** a "Calibrate voice" button on the **Sessions** screen (per
  profile), plus a subtle "recalibrate" affordance if already calibrated.
- **Steps:** intro → for each corner vowel `[a, i, u]`: show the letter big +
  "Say and hold: **A**", a progress ring that fills as stable frames are
  collected, live F1/F2 readout, then ✓ and Next. Retry per step.
- **Finish:** compute the map, derive the six prototypes, write
  `calibration.json`, return to Sessions. Show calibrated state.

Reuse the existing screenshot harness pattern (`RONDELEK_SCREEN=calibrate`) so the
flow is capturable without a live mic.

## Integration

- When a profile is selected / a session opens, load its `calibration.json` (if
  any) and hand the detector that profile's prototypes; uncalibrated profiles
  fall back to `default_prototypes(speaker_scale)`.
- `speaker_scale` remains as the uncalibrated fallback and a dev-panel escape
  hatch.

## i18n

New keys across all 7 locales (`en` source of truth): `sessions.calibrate`,
`calibrate.title`, `calibrate.intro`, `calibrate.say` (formats the letter),
`calibrate.hold`, `calibrate.captured`, `calibrate.retry`, `calibrate.done`.

## Tests (headless, synthetic vowels)

1. `normalize_from_corners`: a child whose corners are ~1.3× the reference yields
   prototypes ~1.3× (and correct relative positions); a wider/narrower F2 span
   stretches F2 targets accordingly.
2. Capture aggregation: feed synthetic vowel windows (with noisy on/offset) →
   median recovers the held formants; unstable input fails.
3. Classify with calibrated prototypes: a synthetic vowel at a child's scaled
   formants classifies correctly with calibrated prototypes where the default set
   would miss.
4. Storage round-trip: save → load `calibration.json`.

## Build order

1. Detector: parameterise `classify` by prototypes + `default_prototypes`. (tests)
2. `normalize_from_corners` + `VowelCalibration` type. (tests)
3. Per-profile storage: save/load `calibration.json`. (tests)
4. `CalibrationCapture` robust aggregation. (tests)
5. Calibration UI flow + entry point + i18n + harness hook.
6. Integration: load a profile's calibration into the active detector.
7. Docs: `data-model.md` (calibration storage) + `visualizer.md` (calibration),
   per the AGENTS docs-currency rule.

## Open decisions for next session

- **Corner-vowel normalisation vs full six-vowel** capture — recommend corners.
- **Map shape:** per-axis linear (recommended, simple, robust) vs full 2-D affine.
- **UI:** dedicated screen (recommended) vs modal over Sessions.
- **Future:** a "distance-to-target" cue in the practice view to guide the child
  toward each vowel — and a progress log over sessions (the therapy payoff).
