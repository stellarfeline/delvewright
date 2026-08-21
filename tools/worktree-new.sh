#!/usr/bin/env bash
#
# Create a worker's worktree that does not recompile what it can clone.
#
# ## What this exists to remove
#
# Every dispatched worker gets its own git worktree — that rule is not up for
# revision, because two workers in one checkout is one `git add -A` away from
# sweeping three authors into one commit. What IS avoidable is that each of
# those worktrees then cold-compiles the entire dependency graph from scratch.
#
# `git worktree add` copies nothing and `cargo build` compiles fresh, so nothing
# anywhere in the chain ever asks the filesystem to CLONE. APFS clone support is
# present and real on this platform; it simply had no caller. Twenty-four
# worktrees held 149.8 GiB of `target/` with no block sharing at all, because
# there was nothing in the chain to share them.
#
# So this script does the one thing that was missing: it clones a donor's
# `target/` into the new tree with `cp -c`, which costs no disk and no compile
# time, and leaves only the workspace's own crates to rebuild.
#
# ## What the clone actually buys, measured rather than assumed
#
# Cargo fingerprints for REGISTRY dependencies survive the move intact: their
# sources live in `~/.cargo/registry`, at the same absolute path for every
# worktree, so nothing about them changes. Only PATH dependencies — this
# workspace's own crates — are invalidated, because their source path is part of
# what they are fingerprinted against, and that path is exactly what a new
# worktree changes.
#
# Measured on a warm donor, `cargo build --workspace --all-targets`, two runs of
# each condition alternated so machine load could not be attributed to one of
# them: **6 packages of 140 rebuilt**, the other 134 reused. The clone itself
# moved 5.8 GiB in about 5 seconds at a `df` cost indistinguishable from zero.
#
# The saving in TIME is much smaller than that ratio suggests, and it is stated
# here so nobody expects otherwise: **19.0% of CPU time** (859.2 s -> 695.7 s,
# and the two cold runs agreed to 0.03%). The 134 reused packages are the cheap
# ones. What dominates this workspace's build is its own six crates and the
# forty-odd test binaries that link them, and those are exactly what a new
# worktree must rebuild. On `cargo build --workspace` without `--all-targets`
# the figure is 11.5%.
#
# The clone is still worth making — it costs five seconds and one `cp` — but it
# is a fifth off, not a dependency graph for free.
#
# `debug/incremental/` is deliberately NOT cloned. It is 3.06 GiB of the donor's
# 5.77 GiB, and incremental state is only ever used for crates cargo compiles
# from a path — this workspace's own, which are exactly the units the move
# invalidates. Measured: dropping it changed nothing that matters (6 packages
# either way, 691.3 s against 695.7 s of CPU) and left the tree 3 GiB smaller.
#
# ## The pin is resolved from the CURRENT DIRECTORY, and that is a trap
#
# `rustc`'s version is part of every fingerprint, and rustup finds
# `rust-toolchain.toml` by walking up from the CWD — not from `--manifest-path`.
# A build launched from outside the checkout therefore silently uses the user's
# default toolchain. This was not a hypothetical: the measurement run for this
# very script did it, built the workspace with rustc 1.100.0-nightly instead of
# the pinned 1.97.1, invalidated every cloned fingerprint, rebuilt all 140
# packages, and reported that cloning saves nothing. Nothing errored. The number
# was plausible and wrong.
#
# So this script REFUSES when the donor and the new tree do not resolve the same
# rustc, rather than cloning output the new tree cannot use.
#
# ## What it does NOT do
#
# It does not share `CARGO_TARGET_DIR`. Cargo locks a target directory for the
# duration of a build, so a shared one would serialise every concurrent worker —
# and two `cargo test --workspace` runs against one target tree are already a
# recorded cause of reds that read as content defects.
#
set -euo pipefail

usage() {
  cat <<'USAGE'
usage: worktree-new.sh --path DIR --branch NAME [--base REV] [--donor DIR]
                       [--detach REV] [--no-clone] [--lease-holder WHO]

  --path DIR         where the worktree goes (required)
  --branch NAME      branch to create and check out
  --detach REV       instead of --branch: check out REV detached
  --base REV         what --branch is cut from (default: origin/main)
  --donor DIR        checkout whose target/ is cloned (default: the main checkout)
  --no-clone         create the worktree but do not clone build output
  --lease-holder WHO claim the tree for WHO so no sweep evaluates it (default: $USER)
  --no-lease         do not claim the tree
USAGE
}

PATH_ARG=""; BRANCH=""; DETACH=""; BASE="origin/main"; DONOR=""
DO_CLONE=1; DO_LEASE=1; HOLDER="${USER:-dispatch}"

while [ $# -gt 0 ]; do
  case "$1" in
    --path) PATH_ARG="$2"; shift 2 ;;
    --branch) BRANCH="$2"; shift 2 ;;
    --detach) DETACH="$2"; shift 2 ;;
    --base) BASE="$2"; shift 2 ;;
    --donor) DONOR="$2"; shift 2 ;;
    --no-clone) DO_CLONE=0; shift ;;
    --no-lease) DO_LEASE=0; shift ;;
    --lease-holder) HOLDER="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
done

[ -n "$PATH_ARG" ] || { echo "--path is required" >&2; exit 2; }
[ -n "$BRANCH" ] || [ -n "$DETACH" ] || { echo "one of --branch / --detach is required" >&2; exit 2; }

# No `cd` below ever ESCAPES its subshell. A `cd` in the first clause of a
# compound command persists through the rest of it, which is how this project
# has made `git` and `gh` answer confidently about the wrong repository. Where a
# directory genuinely has to be entered — `rustc --version` reads the toolchain
# pin from the CWD and has no flag for it — it is entered inside `$( ... )` and
# the shell this script runs in never moves. Every git call names its tree.
HERE="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
MAIN="$(git -C "$HERE" worktree list --porcelain | awk 'NR==1{print $2}')"
[ -n "$DONOR" ] || DONOR="$MAIN"

echo "== worktree-new"
echo "   repository : $MAIN"
echo "   donor      : $DONOR"

# --- create the tree -------------------------------------------------------
if [ -n "$DETACH" ]; then
  git -C "$HERE" worktree add --detach "$PATH_ARG" "$DETACH"
else
  git -C "$HERE" worktree add -b "$BRANCH" "$PATH_ARG" "$BASE"
fi
NEW="$(cd -- "$PATH_ARG" && pwd)"
echo "   created    : $NEW  ($(git -C "$NEW" rev-parse HEAD))"

# --- the content symlink, without which two `analyze` tests fail on a fresh tree
#
# The main checkout's link is RELATIVE (`../delvewright-campaigns`), so a bare
# `readlink` yields a path that only resolves from inside that checkout. Copying
# that string into a worktree several directories deep produces a link that
# dangles — and a dangling `campaigns` does not crash a worker, it makes it
# MEASURE ZERO, which is the oldest silent failure in this repository. So the
# target is resolved to an absolute path here and the absolute path is written.
CONTENT="$(python3 -c 'import os,sys; print(os.path.realpath(sys.argv[1]))' "$MAIN/campaigns" 2>/dev/null || true)"
if [ -n "$CONTENT" ] && [ -d "$CONTENT" ]; then
  ln -sfn "$CONTENT" "$NEW/campaigns"
  echo "   campaigns  : -> $CONTENT"
else
  echo "   campaigns  : NOT LINKED — the main checkout has no resolvable 'campaigns' link."
  echo "                Two 'analyze' tests fail on a tree without it. Point it by hand."
fi

# --- clone the build output ------------------------------------------------
CLONED=0
if [ "$DO_CLONE" = 1 ]; then
  if [ ! -d "$DONOR/target" ]; then
    echo "   build cache: donor has no target/ — nothing to clone, the first build is cold"
  else
    # The instrument is named literally on both sides and compared. A cloned
    # target/ built by a different rustc is not a saving; it is 140 rebuilt
    # packages and a wasted copy.
    D_RUSTC="$(cd -- "$DONOR" && rustc --version 2>/dev/null || echo unknown)"
    N_RUSTC="$(cd -- "$NEW" && rustc --version 2>/dev/null || echo unknown)"
    D_LOCK="$(shasum -a 256 < "$DONOR/Cargo.lock" | awk '{print $1}')"
    N_LOCK="$(shasum -a 256 < "$NEW/Cargo.lock" | awk '{print $1}')"
    if [ "$D_RUSTC" != "$N_RUSTC" ]; then
      echo "   build cache: REFUSED — donor resolves '$D_RUSTC', new tree resolves '$N_RUSTC'."
      echo "                The rustc version is part of every fingerprint, so every cloned"
      echo "                unit would be invalid and the copy would buy nothing."
    else
      BEFORE="$(df -k /System/Volumes/Data | awk 'NR==2{print $4}')"
      T0=$(date +%s)
      if cp -c -R "$DONOR/target" "$NEW/target"; then
        # Incremental state is per-tree and cannot survive the move; see the
        # header for the measurement. Dropping it is 3 GiB the tree never holds.
        rm -rf "$NEW/target/debug/incremental" "$NEW/target/release/incremental"
        T1=$(date +%s); AFTER="$(df -k /System/Volumes/Data | awk 'NR==2{print $4}')"
        CLONED=1
        # This is the WHOLE VOLUME's free-space delta across the copy, not this
        # copy's cost: anything else running on the machine is in it, and on a
        # busy one it comes back negative. It is printed because a clone that
        # silently degraded to a full copy would show up here as several
        # gibibytes — but it is reported as what it is, and a single reading is
        # never evidence on its own.
        awk -v b="$BEFORE" -v a="$AFTER" -v s="$((T1 - T0))" 'BEGIN{
          printf "   build cache: cloned in %ds; volume free space moved %+.3f GiB across the copy\n", s, (a-b)/1048576
          printf "                (whole-volume delta, other processes included — a clone costs ~0)\n" }'
        echo "                toolchain on both sides: $D_RUSTC"
        if [ "$D_LOCK" != "$N_LOCK" ]; then
          echo "                NOTE: Cargo.lock differs from the donor's, so the dependency sets"
          echo "                differ. Units they still share are reused; the rest rebuild."
        fi
      else
        echo "   build cache: cp -c FAILED — the tree is usable, the first build is cold."
        rm -rf "$NEW/target"
      fi
    fi
  fi
fi

# --- claim it, so the sweep never has to decide about a live dispatch -------
# LEASED counts what HAPPENED, never what was asked for. A stated count that
# reports the intention is the failure this project keeps paying for: it reads
# as evidence and is a restatement of the flag.
LEASED=0
if [ "$DO_LEASE" = 1 ]; then
  if [ ! -x "$HERE/tools/worktree-reclaim.py" ]; then
    echo "   lease      : NOT TAKEN — $HERE/tools/worktree-reclaim.py is missing or not executable."
    echo "                An unclaimed tree is one the sweep has to judge on its own."
  elif python3 "$HERE/tools/worktree-reclaim.py" --lease "$NEW" --holder "$HOLDER" \
         --reason "dispatched worktree" >/dev/null; then
    LEASED=1
    echo "   lease      : held by $HOLDER (release it at the merge; the sweep honours it until then)"
  else
    echo "   lease      : NOT TAKEN — the lease command failed. The tree is usable and unclaimed."
  fi
fi

echo "   binding    : 1 worktree created, $CLONED target/ cloned, $LEASED lease taken"
echo
echo "Build INSIDE the tree — 'cargo build --manifest-path $NEW/Cargo.toml' run from"
echo "elsewhere resolves rust-toolchain.toml from your CWD, not from the manifest,"
echo "and silently uses a different compiler."
