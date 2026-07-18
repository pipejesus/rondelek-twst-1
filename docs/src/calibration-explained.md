# How calibration works (and what it deliberately doesn't)

This page explains the *idea* behind voice calibration in plain language — no code.
It's meant to be readable by anyone, including a speech‑language therapist or
phonetician trying the app with a child. For the implementation, see
[The visualizer → Calibration](visualizer.md#calibration).

## What a vowel is, to the app

Every vowel has a characteristic **sound‑shape** — the pattern of which pitches are
loud and which are quiet when you hold the sound. `ee` and `oo` *feel* different
because that pattern is different. The app captures that whole pattern as a compact
"fingerprint" for each vowel.

So "detecting a vowel" simply means: *which stored fingerprint does the voice match
right now?*

> **For clinicians:** the fingerprint is a vector of **MFCCs** (mel‑frequency
> cepstral coefficients) — the standard, robust description of a speech spectral
> envelope. We deliberately do **not** track formants (F1/F2). Estimating formants
> from a child's high‑pitched, short‑vocal‑tract voice is a known ill‑posed problem
> (LPC reports false formants at high f0), and it was the source of the detector's
> earlier unreliability. Comparing envelope *shapes* sidesteps that entirely.

## The problem calibration solves

Everyone's voice is different, and so is every microphone and room. The app can't
know in advance what *this* child's `a` sounds like on *this* laptop. So instead of
guessing from a textbook, it **learns the child's six vowels directly**, once, and
then recognises the child against their own recordings.

There is exactly **one** job here: *recognise what the child said, based on their
calibration.* Making the child's vowels better over time is the **therapist's**
work, not the toy's — you simply re‑calibrate as the child's productions improve, and
the app's reference improves with them.

## What calibration actually does

The app asks the child to say **all six vowels** (`a e i o u y`), one at a time,
holding each briefly. For each vowel it collects several fingerprints and stores
their average as that vowel's **template**. Detection then compares the live voice to
those six templates and lights up the nearest one — but only when it's a clear
winner, so a half‑formed sound reads as "no vowel" rather than a flickering guess.

**The tailor analogy.** It's like a tailor taking your measurements so clothes fit
*you*, rather than assuming everyone is the same size. As the child grows and their
speech sharpens, you take the measurements again.

## Why the microphone "cancels out" (and why there's no sweep)

A cheap microphone colours the sound — it boosts some pitches and dips others. You
might expect that to ruin recognition. It doesn't, because the child is always
compared to **their own templates recorded on the same microphone**. The mic
colours the templates and the live voice *the same way*, so the colour cancels when
they're compared. (The app also removes a running estimate of the microphone's
colour from every frame — "cepstral‑mean normalization" — which cancels most of what
remains, even across a mic change.)

That's why the app has **no microphone frequency‑sweep calibration**: it would add a
lot of machinery to flatten something that already cancels.

What *does* matter is the **level**: audio that clips (too loud) or is buried in noise
(too quiet) has a distorted shape. So calibration and the settings panel show a live
**level meter** with "too quiet" / "clipping" warnings — watch it while you record.

> **For clinicians:** matching is a nearest‑template classifier over the MFCC
> vectors (diagonal‑covariance / Mahalanobis distance), with a confidence **and
> margin** gate so recognition is a firm decision. Templates are stored with the
> mic's channel mean removed; the live detector keeps its own running channel mean.

## Practical notes for the microphone

- Use a **consistent, decent** microphone at a **steady distance**. Consistency
  matters more than quality.
- If you **change microphones** (or move to a very different room), **re‑calibrate**.
  The app records which mic was used and warns you in Settings → Calibration when the
  current mic differs.
- Re‑calibrate whenever the child's vowels have clearly improved — that's how the
  app's standard tracks the child.

## What changed from earlier versions

Earlier builds tried to *measure formants* and offered two modes ("Practice" vs
"Play") with different targets. Both the formant approach and the split were removed:
formants are unreliable for children, and the toy now has a single, honest job —
recognise the child against their own calibration. This page and the code reflect
that single MFCC‑template path.
