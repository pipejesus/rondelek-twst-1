# The grown-ups page

Everything a grown-up might want to adjust lives on one page, called
**For grown-ups**. It's a single scrolling page, with no pop-up windows and no
hidden menus. Open it with the ⚙ key at the top right of the start screen or of
a child's screen, or press `F12` anywhere. The back key takes you back to where
you were.

Changes are saved straight away. Each card also tells you whether it affects
**only this profile** (the child you came from) or **everyone on this
computer**.

## Profile

*Only shown when you opened the page from a child's screen.*

- **Change name or picture**, just like when you made the card.
- **Voice calibration**: whether it's done and when. If a different microphone
  is in use now than at the last calibration, you'll see a note here, and
  **Calibrate the voice** starts a fresh one. See
  [Voice calibration](voice-calibration.md).
- **Delete this profile…** removes the child's card, pictures and recordings
  from the app. To make sure it's on purpose, you type the child's name first.
  Nothing is erased for good, though: the folder moves into a `.trash` folder
  inside the library, so it can still be rescued by hand.

## Sessions

*Also only for the current child.*

Each set of twelve sound-board recordings is a **session**. Here you'll find
all of this child's sessions, newest first. Each one shows its date ("Today,
12:54", "25 September, 13:54") and how many keys hold a sound.

- The session the **Sounds** tile opens is marked in green, with a
  **Continue** key.
- **Open** any other session to carry on there. From then on, Sounds opens
  that one.
- **New session with empty pads** starts fresh (the others stay safe).
- The newest five are shown at first; **Show all** lists the rest.

## Sound

- **Speaker** and **Microphone**: *Auto* follows your computer's own choice,
  or you can pin a specific device. If a pinned device goes missing, Rondelek
  falls back to the default and tells you.
- **Volume**, with **Play a test sound** for a quick check.
- **Microphone level**: speak normally, and the bar should move but not turn red.

## Voice recognition

How sure the app must be before it shows a vowel or moves something in a game.

- **Relaxed** reacts more easily, which is good for quiet or shy voices.
- **Normal** is the everyday choice.
- **Strict** waits for a clearer sound and mixes up fewer vowels.

*Show advanced settings* reveals the individual knobs, for the curious. If you
change them, the card simply says you're on custom values.

## Sampler screen

What the little screen above the sound-board keys shows: **Sound bars**,
**Vowels** or **Off**. (The square key next to REC on the sound board switches
it too.) The advanced settings fine-tune the sound bars.

## Games

How quickly the games react to the voice, from **Steadier** (waits a moment,
fewer mistakes) to **Snappier** (reacts at once). It's the same setting as the
turtle-and-rabbit slider on a game's setup screen.

## Look & language

- **Language**: tap a flag. There are seven: English, Polski, Deutsch,
  Français, Español, Italiano and Українська.
- **Sampler skin**: the look of the sound board. **Arcade** is the default,
  **Classic** is the softer original, and any skins you add appear here too.
  **Open skins folder** shows where to put them. (Want to design one? The
  [skins guide](https://github.com/pipejesus/rondelek-twst-1/blob/develop/docs/SKINS.md)
  explains how.)

## About & data

The version number, and **Open data folder**, which opens the folder holding
all the children, recordings and settings. They're ordinary files, and
nothing is ever sent anywhere.

In case you'd like to know where that folder is:

| System | Children and recordings | Settings |
|--------|-------------------------|----------|
| Windows | `%APPDATA%\rondelek\profiles` | `%APPDATA%\rondelek\settings.json` |
| macOS | `~/Library/Application Support/rondelek/profiles` | `~/Library/Application Support/rondelek/settings.json` |
| Linux | `~/.local/share/rondelek/profiles` | `~/.config/rondelek/settings.json` |

Backing up is as simple as copying those `rondelek` folders somewhere safe.
