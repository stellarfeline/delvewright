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

**The symlink is the only mechanism.** There is no environment-variable
override — `$DELVEWRIGHT_CAMPAIGNS_DIR` is read by no code, so exporting it
produces exactly the two failures this file exists to prevent. The path is
constructed literally in five places (`crates/delvec/tests/common/mod.rs`,
`crates/delvec/src/main.rs`'s `--prefabs` default, `crates/compiler/src/view/nbt.rs`,
`crates/render/tests/gpu.rs`, `.github/workflows/release.yml`); making an
override real means all five sites or none.

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

**Nothing in `validation/` reads this file.** It holds no validation ports and
no container tooling paths; validation gets its ports from `ephemeral-port.yaml`
and its pins from `versions.toml`. A red ladder run is never caused by a missing
`delvewright.local.toml` — same shape as `$DELVEWRIGHT_CAMPAIGNS_DIR` above, a
plausible mechanism nothing implements.

So absence is not fatal to any ladder run; it is fatal only to `i18n-translate`
and `refimg`, which exit non-zero saying what to add. Copy it anyway if you may
touch either.

## 3. Scratch space is NOT isolated between workers

The agent harness hands every worker a scratchpad path it describes as
"session-specific, isolated from the user's project". The isolating token in
that path is the **planner session's** id, so every worker fanned out from one
dispatch is handed **the same string**. It is one flat shared directory with no
per-agent segment. The isolation the name promises does not exist across a
fan-out.

**Convention, mirroring the worktree rule.** Every dispatched worker gets its
own scratch directory, named in the dispatch prompt, using the **same token as
its worktree**:

```sh
mkdir -p "$SCRATCHPAD/<branch-token>"     # e.g. .../scratchpad/lethal-volume/
```

Nothing outside that subdirectory is yours, including anything you find already
there.

### Why this is not a tidiness rule

Measured, 2026-08-09: four workers ran concurrently and **all four**
independently built before/after trees in that one namespace — `base`/`after`,
`out-base`/`out-new`, `zh-base`/`zh-new`, `out/base-<campaign>`/`out/new-<campaign>`.
One worker's `base/` was replaced mid-run by another's repo checkout, and its
setup line was `rm -rf $SP/base && mkdir -p $SP/base` — so with the arrival
order reversed it would have deleted the other worker's tree instead. This is
not bad luck: a before/after byte comparison is the evidence this project
demands of everyone who touches emission, and `base` is the first word every
one of them reaches for. With four workers in one namespace, collision was the
expected outcome.

**The loud failure is the safe one.** That worker diffed a file, got `ENOENT`,
and noticed. The quiet failure is the danger: a byte-identity proof is
`find base/ | shasum` against the same over `after/`. Had the foreign tree
landed *before* the hash rather than after, the run would have hashed someone
else's repo. Worse — another worker was writing build outputs **of the same
campaign** under `out-base`/`out-new`; had those names collided, a worker could
have compared *that worker's* before-tree against *that worker's* after-tree and
reported **its own** change byte-identical. Hundreds of files, all matching, a
number nobody can re-derive from the PR.

That is a green gate that binds to nothing, and review cannot see it, because
the output is indistinguishable from a pass. Note the asymmetry against the
worktree-collision precedent this rule is modelled on: **a code leak fails CI; a
corrupted evidence tree fails nothing.** It emits a sentence in a PR
description.

### The stronger fix, which is not this convention

A convention still relies on everyone following it. The invariant that removes
the class: **a baseline hash manifest records the git SHA and `delvec --version`
it was produced from, and the comparison asserts them.** Then a swapped tree
fails loudly instead of silently comparing the wrong thing — and it covers every
piece of before/after evidence, not just the ones in a shared directory.

## Status

Documented stopgap. The sanctioned ladder runner is expected to
automate the first two checks with a hard preflight; until then, treat this file
as the checklist for any worker dispatched into a fresh worktree.
