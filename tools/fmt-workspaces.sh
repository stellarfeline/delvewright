#!/usr/bin/env bash
#
# Format every cargo workspace this repository holds — the one command whose
# reach is CI's reach.
#
# ## What this exists to remove
#
# `cargo fmt --all` at the repository root reaches the ROOT workspace and nothing
# else. Every `prefabs/*-generator` is a separate workspace on purpose, so the
# obvious local command reports clean while a generator is unformatted — and CI's per-workspace fmt
# steps then redden the pull request. That is not a hypothetical: a stage's new
# generator code went in unformatted and reddened a pull request after a merge.
#
# The defect is not the missing minute. It is that **the cost is paid by whoever
# pushes rather than by whoever wrote it**, which is the same shape as every
# other check this repository moves earlier. Widening the root workspace would
# close it and is refused: those crates are separate for reasons measured in
# /Cargo.toml, and a generator entering `crates/` would enter the shipped binary's
# resolution.
#
# ## The population is DERIVED, never listed
#
# CI listed its seven generators by hand. A hand-written list is a claim about a
# repository that nothing re-checks, and this one was already wrong: the tree
# holds TEN cargo workspaces and that list, plus the root and render steps, named
# NINE. The tenth — `docs/experiments/m2-jigsaw-seed-stability/generator` — was
# reached by no fmt step anywhere and is unformatted today. Nothing was red,
# because nothing looked.
#
# So the population here is `git ls-files -- '*Cargo.toml'`, and each manifest's
# workspace is resolved by asking CARGO — `cargo locate-project --workspace` —
# rather than by grepping for a `[workspace]` table. Cargo is the authority on
# which workspace a manifest belongs to; a grep is a second implementation of that
# question, and it would answer wrongly for a lone package that declares no
# `[workspace]` table of its own and is a member of nothing.
#
# Everything else follows from taking the population from git: `target/`,
# `node_modules/` and every vendored manifest are absent because they are
# untracked, and `campaigns/` contributes nothing because it is a symlink to
# another repository. None of that is a rule that can go stale.
#
# ## The one exclusion, and why it is a property rather than a name
#
# `docs/experiments/` preserves an experiment exactly as it was run. Rewriting it
# would falsify the record, which is the reason
# `tools/check-shell-pipe-shortcircuit.py` and `tools/check-shell-redirect-dirs.py`
# already exclude the same prefix, in those words. Nothing in CI or the authoring
# loop builds or executes it.
#
# The exclusion cannot go quietly dark: it is COUNTED on every run, and an
# exclusion that matches zero tracked manifests is a refusal rather than a pass —
# a stale exemption is a finding (CLAUDE.md: a green gate that binds to nothing is
# vacuous).
#
# ## What it does NOT do
#
# It does not run `clippy`. `prefabs/generator` carries two pre-existing style
# lints that are somebody's own pull request, and bundling them in here would be a
# check nobody can go green under today.
#
# It does not format the harness (TypeScript, `harness/`). That is prettier's
# subject and a different command; this one is about `cargo fmt`'s reach.
#
set -euo pipefail

usage() {
  cat <<'USAGE'
usage: fmt-workspaces.sh [--check]

  (no flag)   format every cargo workspace in the tree, in place
  --check     format nothing; exit 1 naming every workspace that is not clean.
              This is what CI runs, so a clean run here is what CI will say.
USAGE
}

MODE="write"
while [ $# -gt 0 ]; do
  case "$1" in
    --check) MODE="check"; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
done

# The shell this script runs in never moves; every git call names its tree. A
# `cd` in the first clause of a compound command persists through the rest of it,
# which is how this project has made `git` answer about the wrong repository.
ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"

# Frozen record, not live tooling — see the header. Kept as a prefix so it is
# checked against the tree on every run rather than trusted.
EXCLUDED_PREFIX="docs/experiments/"

# Not `mapfile`: macOS ships bash 3.2, which has no such builtin, and the whole
# point of this script is that it runs on the creator's own machine. The same
# note stands in `tools/build-release-binaries.sh` and `validation/packtest-all.sh`
# — the second of which shipped the bug and covered ZERO projects because of it.
MANIFESTS=()
while IFS= read -r -d '' m; do
  MANIFESTS+=("$m")
done < <(git -C "$ROOT" ls-files -z -- '*Cargo.toml')
TOTAL=${#MANIFESTS[@]}
if [ "$TOTAL" = 0 ]; then
  echo "REFUSED — 'git ls-files -- *Cargo.toml' matched no file." >&2
  echo "          A population of zero is a finding, not a clean sweep." >&2
  exit 1
fi

KEPT=()
EXCLUDED=0
for m in "${MANIFESTS[@]}"; do
  case "$m" in
    "$EXCLUDED_PREFIX"*) EXCLUDED=$((EXCLUDED + 1)) ;;
    *) KEPT+=("$m") ;;
  esac
done
if [ "$EXCLUDED" = 0 ]; then
  echo "REFUSED — the '$EXCLUDED_PREFIX' exclusion matched no tracked manifest." >&2
  echo "          An exemption that binds to nothing is a finding: either the" >&2
  echo "          experiment record moved, or the exclusion outlived it. Remove" >&2
  echo "          it here rather than leaving a rule nobody can see is dead." >&2
  exit 1
fi

# Ask cargo which workspace each manifest belongs to. Cargo is the authority; a
# `[workspace]` grep is a second implementation of the same question.
ROOTS=()
for m in "${KEPT[@]}"; do
  ws=""
  if ! ws="$(cargo locate-project --workspace --message-format plain \
               --manifest-path "$ROOT/$m" 2>&1)"; then
    echo "REFUSED — cargo could not resolve the workspace of $m:" >&2
    echo "          $ws" >&2
    echo "          A manifest whose workspace is unknown is not skipped: it is" >&2
    echo "          the one file a sweep must not silently stop covering." >&2
    exit 1
  fi
  ROOTS+=("$ws")
done
UNIQUE=()
while IFS= read -r ws; do
  UNIQUE+=("$ws")
done < <(printf '%s\n' "${ROOTS[@]}" | sort -u)
ROOTS=("${UNIQUE[@]}")

echo "== fmt-workspaces ($MODE)"
echo "   population : $TOTAL tracked Cargo.toml, $EXCLUDED excluded as a frozen"
echo "                experiment record, ${#KEPT[@]} resolved -> ${#ROOTS[@]} workspace roots"

FAILED=()
for ws in "${ROOTS[@]}"; do
  rel="${ws#"$ROOT"/}"
  rc=0
  if [ "$MODE" = check ]; then
    cargo fmt --manifest-path "$ws" --all -- --check || rc=$?
  else
    cargo fmt --manifest-path "$ws" --all || rc=$?
  fi
  if [ "$rc" = 0 ]; then
    echo "   ok         : $rel"
  else
    echo "   NOT CLEAN  : $rel"
    FAILED+=("$rel")
  fi
done

echo "   binding    : ${#ROOTS[@]} workspaces swept, ${#FAILED[@]} not clean"
if [ "${#FAILED[@]}" != 0 ]; then
  if [ "$MODE" = check ]; then
    echo
    echo "Run 'bash tools/fmt-workspaces.sh' to fix all of these at once."
    echo "'cargo fmt --all' at the root reaches 1 of the ${#ROOTS[@]}, which is why"
    echo "a clean root sweep is not what CI will say."
  fi
  exit 1
fi
