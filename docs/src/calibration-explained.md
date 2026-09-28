# How it works, in plain words

Ever wondered how the app tells an *a* from an *o*? This page explains the
idea behind voice calibration without any code, so anyone can follow along.
If you'd like the implementation, it's in
[The visualizer → Calibration](visualizer.md#calibration).

## What a vowel is, to the app

Every vowel has its own **sound shape**: the pattern of which pitches are loud
and which are quiet while you hold the sound. *"ee"* and *"oo"* feel different
in your mouth because that pattern is different, and a microphone can hear the
difference too. The app boils each pattern down to a compact **fingerprint**.

So "recognising a vowel" simply means asking: *which stored fingerprint does
the voice match right now?*

> **For the technically curious:** the fingerprint is a vector of **MFCCs**
> (mel-frequency cepstral coefficients), the standard, sturdy way to describe
> the shape of a speech sound. The app deliberately doesn't track formants
> (F1/F2). Estimating formants from a child's high, bright voice is famously
> unreliable (LPC reports false formants at high pitch), and it was the cause
> of an earlier version's wobbles. Comparing whole shapes sidesteps the
> problem.

## The problem calibration solves

Every voice is different, and so is every microphone and every room. The app
can't know in advance what *this* child's *a* sounds like on *this* laptop. So
instead of guessing, it **learns the child's six vowels directly**, and then
recognises the child against their own recordings.

The app has exactly one job here: *recognise what the child said, compared
with their own calibration.* It doesn't grade or correct anyone. As a child's
vowels grow clearer over time, you simply calibrate again, and the app's idea
of "their *a*" grows along with them.

## What calibration actually does

The app asks for **all six vowels** (*a e i o u y*), one at a time, each held
for a moment. For every vowel it collects a handful of fingerprints and keeps
their average as that vowel's **template**. From then on, it compares the live
voice with those six templates and lights up the closest one. It only does
that when there's a clear winner, so a half-formed sound shows as "no vowel"
instead of a flickering guess.

**Think of a tailor.** A tailor takes your measurements so the clothes fit
*you*, instead of assuming everyone's the same size. As the child grows, you
take the measurements again.

## Why the microphone cancels out (and why there's no sweep)

A cheap microphone colours the sound: it boosts some pitches and dips others.
You might expect that to spoil recognition, but it doesn't, because the child
is always compared with **their own templates, recorded on the same
microphone**. The microphone colours the templates and the live voice in the
same way, so the colouring cancels out when they're compared. (The app also
removes a running estimate of the microphone's colouring from every moment of
sound, a trick called *cepstral mean normalisation*, which takes care of most
of what's left, even after a microphone change.)

That's why there's **no microphone sweep test** to sit through: it would add
a lot of machinery to flatten something that already cancels out.

What *does* matter is the **level**. Sound that's too loud (clipping) or too
quiet (lost in noise) has a smudged shape. That's why calibration, and the
*Sound* card on the grown-ups page, show a live **level meter** that warns
about *too quiet* and *too loud*. Keep an eye on it while you record.

> **For the technically curious:** matching is a nearest-template classifier
> over the MFCC vectors (diagonal-covariance, Mahalanobis distance), with a
> confidence **and** a margin gate, so a recognition is a firm decision.
> Templates are stored with the microphone's channel mean removed, and the
> live detector keeps its own running channel mean.

## A few words about microphones

- A **steady, decent** microphone at a **steady distance** works best.
  Consistency matters more than quality.
- If you **change microphones** (or move to a very different room),
  **calibrate again**. The app remembers which microphone was used, and the
  Profile card on the *For grown-ups* page tells you when the current one is
  different.
- Calibrate again whenever the child's vowels have clearly changed. That's
  how the app keeps up with them.

## What changed from earlier versions

Earlier builds tried to *measure formants*, and offered two modes ("Practice"
and "Play") with different targets. Both are gone: formants are unreliable
for children's voices, and the toy now has one simple, honest job, which is
recognising the child against their own calibration. This page and the code
both follow that single MFCC-template path.
