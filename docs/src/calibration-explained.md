# How calibration works (and what it deliberately doesn't)

This page explains the *idea* behind voice calibration in plain language — no code.
It's meant to be readable by anyone, including a speech‑language therapist or
phonetician trying the app with a child. For the implementation, see
[The visualizer → Calibration](visualizer.md#calibration).

## What a vowel is, to the app

When you say a vowel, your mouth acts as a small echo chamber whose shape produces
two characteristic resonances. The app measures those two numbers and uses them to
place every vowel as a **dot on a 2‑D map**:

```
        front  ←—  tongue  —→  back
close   i · · · · · · y · · · · · u     ← mouth nearly closed
 ↑      ·                         ·
 |             e            o
 ↓                  a                    ← mouth wide open
        (how open the mouth is, top→bottom)
```

- **Left–right** ≈ how far forward the tongue sits.
- **Up–down** ≈ how open the mouth is.

So "detecting a vowel" simply means: *which dot is the voice sitting closest to
right now?*

> **For clinicians:** the two numbers are the first two **formants**, F1 (≈ vowel
> height/openness) and F2 (≈ backness). The "map" is the familiar F1–F2 vowel
> space. Calibration is a form of **speaker normalization** anchored on the
> point (corner) vowels.

## The problem calibration solves

Everyone's map is a **different size**. A young child's vocal tract is smaller, so
their whole map is "zoomed" compared with an adult's — the *same* vowel lands on
*different* numbers. If the app judges a child against an adult‑sized map, even a
perfectly pronounced vowel can look "wrong." (This is exactly what made an early
version confuse one child's **y** with **i**.)

## What calibration deliberately does **not** do

Two tempting approaches — both rejected:

1. **Record the child's *current* vowels and treat them as the targets.**
   ❌ No. If we saved a child's current, mixed‑up **y** and labelled it "correct
   y," the app would happily say *"nice y!"* every time they said it wrong. That
   rewards the mistake and defeats the entire point of practice.

2. **Have a trained adult record the "true" vowels.**
   ❌ No. A trained adult's voice is a *different‑sized* map. A child could
   pronounce a vowel perfectly and still "miss" the adult's exact numbers simply
   because their mouth is smaller. Comparing a child to an adult recording was the
   original bug, not the fix.

## What calibration actually does

It measures **only the size and shape of the child's personal map**, then places
the **correct** vowel targets onto *that* map.

To do it, the app asks the child to say **all six vowels** (`a e i o u y`), one at
a time, holding each briefly. From those six it works out how big and stretched
*this child's* map is — its centre and spread. It then takes the **textbook‑correct
positions of the six vowels** and redraws them at the right spots for a map of that
size.

> **For clinicians:** the six‑vowel fit is a **Lobanov‑style** per‑formant
> z‑score/de‑normalization — more robust for the interior vowels (`e`, `o`) than a
> 3‑corner fit. Matching is done on the **Bark** scale.

**The tailor analogy.** It's like a tailor taking measurements and then cutting the
*correct* pattern to fit *your* body. We don't copy your slouch (the child's current
mispronunciation), and we don't force you into someone else's suit (the adult
reference). We tailor the *right* target to *you*.

## Two modes from one calibration

The same six recordings power two ways of using the app:

- **Practice mode (therapy)** — targets are the **correct** vowels, scaled to the
  child. The app only says "that's a good `e`" when the child actually reaches a
  correct `e`, so it can reveal — and help correct — a drift like the `y`/`i` mix.
- **Play mode (games)** — targets are the vowels the child **actually produces**.
  Detection is forgiving and responsive, so a child's voice can drive a game (e.g.
  steering a ball) without being judged for correctness. This intentionally
  *accepts* the child's current sounds — it's for fun and engagement, not therapy.

## Why Practice mode fixes the y/i confusion

In Practice mode the **`y` target sits at the correct central‑high spot — sized for
that child's voice.** So the app only recognises "y" when the child moves their
tongue to the correct place. It won't cheat by accepting an old, `i`‑like `y`, and
it won't unfairly reject a good `y` for being "too small" for an adult. **It holds
the correct standard, in the child's size.**

That's the whole idea in one sentence:

> **Measure the child's voice *size* from all six vowels, then — in Practice mode —
> judge each vowel against the *correct* target scaled to that size (Play mode
> instead recognises the child's own vowels, for games).**
