# Git hooks

Version-controlled git hooks. Git doesn't run hooks from a tracked directory by
default, so each contributor enables them once per clone:

```sh
git config core.hooksPath .githooks
```

To disable again:

```sh
git config --unset core.hooksPath
```

## Hooks

| Hook         | Status | Does                                                    |
|--------------|--------|---------------------------------------------------------|
| `pre-commit` | active | `cargo fmt --all -- --check` (rejects unformatted code) |
| `pre-push`   | stub   | placeholder — clippy + tests are commented out for now  |

Both files have `TODO` markers showing the heavier checks (clippy, tests) to
switch on once we're ready.
