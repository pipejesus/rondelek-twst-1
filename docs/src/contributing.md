# Contributing to these docs

Thanks for stopping by. This book is **part of the codebase**, not something
tacked on afterwards, and it only stays useful if it keeps up with the code.
So there's one golden rule:

> **Any change to behaviour, wiring, data layout, or dependencies updates the
> relevant page in the same commit.** Docs and code move together.

The same rule is written down in [`AGENTS.md`](https://github.com/pipejesus/rondelek-twst-1/blob/develop/AGENTS.md),
so automated contributors follow it too.

## Where things live

```
docs/
  book.toml           mdBook + mermaid configuration
  src/
    SUMMARY.md        the table of contents (add new pages here)
    *.md              one page per topic
    images/           screenshots and static assets
```

The pages are plain Markdown, so they also read fine on GitHub if you're just
browsing `docs/src/`. Diagrams are [Mermaid](https://mermaid.js.org/) fenced
code blocks (```` ```mermaid ````), rendered by `mdbook-mermaid`.

## Build the site locally

The tooling is all Rust, so there's nothing exotic to set up. Install it once:

```bash
cargo install mdbook mdbook-mermaid --locked
```

Then, from the repo root:

```bash
mdbook-mermaid install docs   # drops the mermaid JS assets into docs/ (git-ignored)
mdbook serve docs --open      # live-reloading preview at http://localhost:3000
# or a one-shot build into docs/book/
mdbook build docs
```

`mdbook serve` rebuilds as you type, so you can keep the preview open next to
your editor. `docs/book/`, `docs/mermaid.min.js` and `docs/mermaid-init.js` are
generated and git-ignored; please never commit them.

## Publishing (GitHub Pages)

`.github/workflows/docs.yml` builds the book and deploys it to GitHub Pages
whenever a push to `main` / `develop` touches `docs/**` (you can also start it
by hand). It installs mdBook + `mdbook-mermaid`, runs `mdbook-mermaid install`,
then `mdbook build`, and uploads the result.

**One-time repo setting:** in *Settings → Pages*,
the source is set to **GitHub Actions**. The book is live at
<https://pipejesus.github.io/rondelek-twst-1/>.

## Writing guidelines

- **Draw a diagram when it makes the wiring clearer**: data flow, state
  machines, threading. Prose is fine for everything else.
- **Reference code by symbol name** (`Playback::drain_monitor`), not by line
  number. Names survive refactors; line numbers go stale the moment someone
  adds a blank line.
- Keep each page about one topic, and add new pages to `SUMMARY.md`.
- Keep Mermaid labels free of parentheses and angle brackets (use `<br/>` for
  line breaks) so they render reliably.
- Write like you're showing a friend around: plain words, short sentences, and
  "you" is welcome.
- The **Playing with Rondelek** pages are for parents. Keep them free of code
  and jargon, and describe what they'll see on screen, using the app's own
  words for buttons (**Voice calibration**, **For grown-ups**…).
