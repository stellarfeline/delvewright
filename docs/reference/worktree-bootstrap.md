# Engine worktree bootstrap

A fresh `git worktree` of the engine repo is NOT self-sufficient: two pieces
of local state live outside version control and do not follow worktrees.

## 1. `campaigns` symlink (untracked)

The `campaigns` symlink at the repo root (target: a `delvewright-campaigns`
clone) is how this repository finds the content repository. Two kinds of
reader use it, and they read different things:

- **Tests read the content at the pin, never the working tree.** Every test
  that judges the prefab library — the only part of the content the engine
  judges; a campaign is judged at its own release — resolves it through
  `crates/delvec/tests/common/pinned.rs`: it takes `versions.toml`
  `[content].sha`, reads that commit out of the clone's git object store
  (git-lfs objects out of its own LFS store), and materialises it once into
  `target/tmp/pinned-content/<sha>/`. The revision the clone's working tree
  sits on never reaches a test, so a test run is a property of this
  repository's revision alone. The clone must hold the pinned commit and its
  LFS objects (`git -C campaigns fetch origin && git -C campaigns lfs fetch
  origin <sha>`); a test that cannot read them fails with that command and
  never skips.
- **The CLI reads the working tree.** `delvec`'s `--prefabs` default is
  `campaigns/prefabs`, so a build, an audit or a render run by hand judges
  whatever the clone has checked out. That is the creator's own content, by
  design.

A worktree without the symlink fails every test that reaches the pinned
content. There is no environment-variable override; `$DELVEWRIGHT_CAMPAIGNS_DIR`
is read by no code.

`tools/planner/worktree-new.sh` creates the link, resolved to an absolute path.
For a worktree made any other way, from the new worktree root:

```sh
ln -s <path-to-delvewright-campaigns-checkout> campaigns
```

## 2. `delvewright.local.toml` (gitignored)

Local machine configuration is read from `delvewright.local.toml` at the repo
root and is gitignored. Copy it from the main checkout when present:

```sh
cp <main-checkout>/delvewright.local.toml .
```

**Exactly three tools read it**, all Python, and only for their own section:
`tools/creator/i18n-translate.py` (`[i18n]`), `tools/creator/refimg.py` (`[refimg]`)
and `tools/creator/refscore.py` (`[refscore]`). No
shell script, no compose file and no Rust crate reads it — grep the tree before
believing otherwise.

**Nothing in `validation/` reads this file.** It holds no validation ports and
no container tooling paths; validation gets its ports from `ephemeral-port.yaml`
and its pins from `versions.toml`. A red ladder run is never caused by a missing
`delvewright.local.toml` — same shape as `$DELVEWRIGHT_CAMPAIGNS_DIR` above, a
plausible mechanism nothing implements.

So absence is not fatal to any ladder run; it matters only to `i18n-translate`,
`refimg` and `refscore`, which say what to add. Copy it anyway if you may touch
any of them.

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

Measured: four workers ran concurrently and **all four**
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

`tools/planner/worktree-new.sh` makes §1's link. §2's copy and §3's scratch
directory are made by hand, and this file is the checklist for them.
