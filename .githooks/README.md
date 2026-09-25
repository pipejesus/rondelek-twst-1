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
| `pre-push`   | active | `cargo clippy -D warnings` + `cargo test` (mirrors CI)  |

Git config is shared by every worktree of a clone, so enabling the hooks once
covers all worktrees. The first push from a fresh worktree compiles the whole
workspace, so expect it to take a few minutes.
