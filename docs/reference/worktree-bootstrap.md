# Engine worktree bootstrap

A fresh `git worktree` of the engine repo is NOT self-sufficient: two pieces
of local state live outside version control and do not follow worktrees.
Every worker that skips them hits the same two failures.

## 1. `campaigns` symlink (untracked)

`crates/compiler/tests/analyze.rs` (via `common::prefabs_dir()`) and every
campaign build resolve content through the `campaigns` symlink at the repo
root (target: a `delvewright-campaigns` checkout). A fresh worktree lacks it —
symptom: exactly two `analyze.rs` test failures that look like a broken
compiler.

**The symlink is the only mechanism.** This file used to offer
`$DELVEWRIGHT_CAMPAIGNS_DIR` as an override and the skill repeated the offer;
no code has ever read that variable. The path is constructed literally in five
places (`crates/compiler/tests/common/mod.rs`, `crates/compiler/src/main.rs`'s
`--prefabs` default, `crates/render/src/nbt.rs`, `crates/render/tests/gpu.rs`,
`.github/workflows/release.yml`), so a worker who exported the variable instead
of making the symlink got the two failures this file exists to prevent, from
the fix this file recommended. Making it real means all five sites or none.

Fix, from the new worktree root:

```sh
ln -s <path-to-delvewright-campaigns-checkout> campaigns
```

For reproducible test runs, pin the content checkout to the SHA in
`versions.toml` (the engine CI does); a moving content branch can turn
engine tests red for content reasons.

## 2. `delvewright.local.toml` (gitignored)

Local machine configuration is read from `delvewright.local.toml` at the repo
root and is gitignored. Copy it from the main checkout when present:

```sh
cp <main-checkout>/delvewright.local.toml .
```

**Exactly two tools read it**, both Python, and only for their own section:
`tools/i18n-translate.py` (`[i18n]`) and `tools/refimg.py` (`[refimg]`). No
shell script, no compose file and no Rust crate reads it — grep the tree before
believing otherwise.

This section used to say the file held "validation ports, container tooling
paths" and that "validation compose runs inherit wrong defaults silently". Both
were false in the same way `$DELVEWRIGHT_CAMPAIGNS_DIR` above was false: a
plausible mechanism nothing implements. Nothing in `validation/` has ever read
this file — validation gets its ports from `ephemeral-port.yaml` and its pins
from `versions.toml`. A worker who skipped the copy and then blamed a red
ladder run on it was chasing a cause that does not exist.

So absence is not fatal to any ladder run; it is fatal only to `i18n-translate`
and `refimg`, which exit non-zero saying what to add. Copy it anyway if you may
touch either.

## Status

Documented stopgap. The sanctioned ladder runner (task #150) is expected to
automate both checks with a hard preflight; until then, treat this file as
the checklist for any worker dispatched into a fresh worktree.
