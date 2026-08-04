# Engine worktree bootstrap

A fresh `git worktree` of the engine repo is NOT self-sufficient: two pieces
of local state live outside version control and do not follow worktrees.
Every worker that skips them hits the same two failures.

## 1. `campaigns` symlink (untracked)

`crates/compiler/tests/analyze.rs` (via `common::prefabs_dir()`) and every
campaign build resolve content through the `campaigns` symlink at the repo
root (target: a `delvewright-campaigns` checkout; override with
`$DELVEWRIGHT_CAMPAIGNS_DIR`). A fresh worktree lacks it — symptom: exactly
two `analyze.rs` test failures that look like a broken compiler.

Fix, from the new worktree root:

```sh
ln -s <path-to-delvewright-campaigns-checkout> campaigns
```

For reproducible test runs, pin the content checkout to the SHA in
`versions.toml` (the engine CI does); a moving content branch can turn
engine tests red for content reasons.

## 2. `delvewright.local.toml` (gitignored)

Local machine configuration (validation ports, container tooling paths)
is read from `delvewright.local.toml` at the repo root and is gitignored.
Copy it from the main checkout when present:

```sh
cp <main-checkout>/delvewright.local.toml .
```

Absence is not always fatal (defaults exist) but validation compose runs
inherit wrong defaults silently — prefer copying.

## Status

Documented stopgap. The sanctioned ladder runner (task #150) is expected to
automate both checks with a hard preflight; until then, treat this file as
the checklist for any worker dispatched into a fresh worktree.
