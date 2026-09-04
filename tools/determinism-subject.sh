#!/usr/bin/env bash
#
# Build the cross-OS determinism subject and digest it.
#
# ## Why this is a script and not two workflow blocks
#
# The comparison this feeds is only meaningful if both hosts built THE SAME
# THING. Two `run:` blocks, one per operating system, are two statements of one
# subject: they can drift by a flag, and the day they do the comparison stops
# being about determinism and starts being about the workflow file, with nothing
# to say so. One script, invoked identically on both runners, removes that
# question — and it is what a creator runs on their own machine, which is the
# floor this project holds above every convenience.
#
# ## Why the metrics gym
#
# `delvec metrics --gym` generates a whole site-plan campaign FROM the compiler's
# own metrics table (spec-0049 §2.3). Three properties make it the right subject:
#
#   * it needs no prefab library, so neither runner checks out the content repo,
#     fetches git-lfs, or depends on the content pin;
#   * it is GENERATED rather than committed, so the subject cannot go stale
#     against the engine — the moment the table moves, so does the campaign;
#   * the table carries derived floating-point values, and float formatting is
#     precisely where a libm difference between platforms would surface. A
#     committed fixture would have compared two copies of the same bytes and
#     proved nothing about the arithmetic.
#
# Both the generated campaign and the built output tree are digested, so a
# divergence in the generator and a divergence in emission are each visible, and
# the report says which file moved.
#
# ## Usage
#
#   tools/determinism-subject.sh --delvec target/debug/delvec --out digest.txt
#                                [--work DIR]

set -euo pipefail

DELVEC=""
OUT=""
WORK=""

while [ $# -gt 0 ]; do
  case "$1" in
    --delvec) DELVEC="$2"; shift 2;;
    --out)    OUT="$2"; shift 2;;
    --work)   WORK="$2"; shift 2;;
    -h|--help)
      sed -n '2,40p' "$0"; exit 0;;
    *)
      echo "determinism-subject: unknown argument \`$1\`" >&2; exit 2;;
  esac
done

if [ -z "$DELVEC" ] || [ -z "$OUT" ]; then
  echo "determinism-subject: --delvec BIN and --out FILE are both required" >&2
  exit 2
fi
if [ ! -x "$DELVEC" ]; then
  echo "determinism-subject: \`$DELVEC\` is not an executable file. Build one:" >&2
  echo "    cargo build --locked -p delvec --bin delvec" >&2
  exit 1
fi

here="$(cd "$(dirname "$0")/.." && pwd)"

if [ -z "$WORK" ]; then
  WORK="$here/determinism-subject-work"
fi
rm -rf "$WORK"
mkdir -p "$WORK/campaign"

echo "determinism-subject: engine — $("$DELVEC" --version)"

# The campaign, generated from the compiler's own metrics table.
"$DELVEC" metrics --gym "$WORK/campaign" > "$WORK/metrics.log" 2>&1

# The build. No `--prefabs`: a site-plan campaign places its geometry from its
# own plan, so nothing outside this tree is read.
"$DELVEC" build "$WORK/campaign" -o "$WORK/out" > "$WORK/build.log" 2>&1

# `metrics.log` and `build.log` are deliberately OUTSIDE the digested root: they
# carry the tool's own diagnostics, not the artifact, and the artifact is what
# ADR-0006 makes a promise about.
mkdir -p "$WORK/subject"
mv "$WORK/campaign" "$WORK/subject/campaign"
mv "$WORK/out" "$WORK/subject/out"

python3 "$here/tools/tree-digest.py" --root "$WORK/subject" --out "$OUT"
