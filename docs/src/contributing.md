# Contributing to these docs

This handbook is **part of the codebase**, not an afterthought. The golden rule:

> **Any change to behaviour, wiring, data layout, or dependencies updates the
> relevant page in the same commit.** Docs and code move together.

The same rule is recorded in [`AGENTS.md`](https://github.com/pipejesus/rondelek-twst-1/blob/develop/AGENTS.md)
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

The source is plain Markdown, so it also renders directly on GitHub when browsing
`docs/src/`. Diagrams are [Mermaid](https://mermaid.js.org/) fenced code blocks
(```` ```mermaid ````), rendered by `mdbook-mermaid`.

## Build the site locally

The tooling is Rust-native. Install once:

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

`docs/book/`, `docs/mermaid.min.js`, and `docs/mermaid-init.js` are generated and
git-ignored — never commit them.

## Publishing (GitHub Pages)

`.github/workflows/docs.yml` builds the book and deploys it to GitHub Pages on
pushes to `main` / `develop` that touch `docs/**` (and on manual dispatch). It
installs mdBook + `mdbook-mermaid`, runs `mdbook-mermaid install`, then
`mdbook build`, and uploads the result.

**One-time repo setting:** in *Settings → Pages*, set the source to **GitHub
Actions**. (The repo is private today; the workflow is ready for when it goes
public.)

## Writing guidelines

- **Diagram when it clarifies wiring** — data flow, state machines, threading.
  Prose for everything else.
- **Reference code by symbol name** (`Playback::drain_monitor`), not line number —
  names survive refactors; line numbers rot.
- Keep each page focused on one topic; add it to `SUMMARY.md`.
- Keep Mermaid labels free of parentheses and angle brackets (use `<br/>` for line
  breaks) so they render reliably.
