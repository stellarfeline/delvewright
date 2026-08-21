#!/usr/bin/env bash
# Run EVERY PackTest project the tier-2 gate runs, on this tree.
#
#   EULA=TRUE validation/packtest-all.sh [--prefix <id>] [--only <project>]
#
# ## Why this exists
#
# A change to PackTest emission is verified on a live server, and the surface it
# must be verified on is "every project the gate runs" — the parsed matrix. Until
# this existed the only way to know that was to read the tier-2 job and remember
# what you found, and `CLAUDE.md` says in as many words that a doc line is not an
# invocation. A round read the job, found the gallery, ran that suite three times
# end to end at 97/97, and shipped a red on `dw-ci-helloworld`: a campaign with no
# cast ledger, a shape the gallery does not have. Every project it ran was green.
# Nothing anywhere said how many there were.
#
# That is truncation faking coverage, and it fakes it in the direction that reads
# as a clean pass. So the fix is not a longer checklist: it is one command that
# takes the choice away, and a matrix DERIVED from `ci.yml` rather than restated
# beside it — one authority, nothing to drift, and a project added to the job is
# covered by the next local run without anyone remembering.
#
# ## Binding
#
# The run states the project count before it starts and again at the end, and a
# partial sweep is a FAILURE line, never a summary. `--only` exists for iterating
# on one red; it prints what it is skipping and exits non-zero, so it can never be
# mistaken for a full pass.
#
# Each project goes through `packtest-run.sh`, which owns its own compose project
# and tears it down; nothing here is shared between passes.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/.." && pwd)"

prefix="dw-local"
only=""
while [ $# -gt 0 ]; do
  case "$1" in
    --prefix) [ $# -ge 2 ] || { echo "usage: --prefix <id>" >&2; exit 2; }; prefix="$2"; shift 2 ;;
    --only)   [ $# -ge 2 ] || { echo "usage: --only <project>" >&2; exit 2; }; only="$2"; shift 2 ;;
    -h|--help) sed -n '1,32p' "$0" >&2; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

delvec="$repo/target/debug/delvec"
[ -x "$delvec" ] || { echo "error: $delvec is not built — run \`cargo build --bin delvec\` first" >&2; exit 2; }

# The matrix is read out of ci.yml. A zero or short parse refuses in there.
#
# Written for bash 3.2, which is what macOS ships and therefore what a creator
# runs: `mapfile` is a bash 4 builtin, and using it made this script — whose whole
# purpose is local verification — work in CI and fail on the only machine that
# needed it. It failed by printing `mapfile: command not found`, covering ZERO
# projects, and exiting 0. A coverage runner that reports success having run
# nothing is the defect in this file's own header, one layer in.
#
# Via a temp file rather than a process substitution: a refusal from the parser is
# then an ordinary non-zero exit that `set -e` catches, instead of a SIGPIPE on
# the writer (`CLAUDE.md`'s recorded `grep -q` trap, same shape).
matrix_file="$(mktemp "${TMPDIR:-/tmp}/dw-packtest-matrix.XXXXXX")"
trap 'rm -f "$matrix_file"' EXIT
python3 "$here/packtest-matrix.py" --list > "$matrix_file"

rows=""
total=0
while IFS= read -r line; do
  [ -n "$line" ] || continue
  rows="${rows}${line}"$'\n'
  total=$((total + 1))
done < "$matrix_file"
[ "$total" -gt 0 ] || { echo "error: empty matrix — refusing rather than passing" >&2; exit 1; }

echo "==> packtest-all: ${total} project(s) from ci.yml tier 2 (prefix '${prefix}')"
ran=0
skipped=0
failed=""
failures=0
# The row stream is on FD 3, and every command in the body has its stdin closed.
# Read from plain stdin, the FIRST project's server run consumes the rest of the
# heredoc and the loop ends after one row — the sweep then reports one green
# project and stops. It did: 1 of 12, with nothing wrong-looking in the output.
# That is the same shape as `CLAUDE.md`'s recorded `grep -q` trap (an inner
# consumer eating its producer's stream) and it produced the same kind of answer:
# plausible, silent, and short. The accounting identity below is what caught it.
while IFS= read -r row <&3; do
  [ -n "$row" ] || continue
  IFS=$'\t' read -r project tree campaign prefabs generator skins <<< "$row"
  if [ -n "$only" ] && [ "$project" != "$only" ]; then
    skipped=$((skipped + 1))
    continue
  fi
  echo "---- ${project}: build ${campaign} -> validation/${tree}"
  if [ "$generator" != "-" ]; then
    mkdir -p "$repo/${prefabs}"
    ( cd "$repo" && cargo run --quiet --manifest-path "$generator" -- "$prefabs" --skins "$skins" >/dev/null </dev/null )
  fi
  rm -rf "${repo:?}/validation/${tree}"
  if ! ( cd "$repo" && "$delvec" build "$campaign" -o "validation/${tree}" --prefabs "$prefabs" >/dev/null 2>&1 </dev/null ); then
    echo "::error::${project}: delvec build failed for ${campaign}"
    failed="${failed} ${project}(build)"
    failures=$((failures + 1))
    continue
  fi
  if EULA=TRUE bash "$here/packtest-run.sh" --project "${prefix}-${project#dw-ci-}" --output "./${tree}" </dev/null; then
    ran=$((ran + 1))
  else
    failed="${failed} ${project}"
    failures=$((failures + 1))
  fi
done 3<<EOF
${rows}
EOF

echo "==== packtest-all ==================================================="
# The accounting identity. Every row is ran, failed or deliberately skipped, and
# nothing else — so a row lost to a `continue` somebody adds later cannot be
# absorbed into a green summary. This is the guard the first version of this file
# did not have, which is how it reported success over zero projects.
accounted=$((ran + failures + skipped))
if [ "$accounted" -ne "$total" ]; then
  echo "FAILED: accounted for ${accounted} of ${total} project(s) — ${ran} ran, ${failures} failed, ${skipped} skipped."
  echo "  A row that is none of those was dropped silently; that is a coverage hole, not a pass."
  exit 1
fi
if [ -n "$only" ]; then
  echo "PARTIAL: --only ${only}; ${skipped} of ${total} project(s) NOT run.${failed:+ failed:${failed}}"
  echo "  This is not a pass. Re-run without --only before reporting green."
  exit 1
fi
if [ "$failures" -ne 0 ]; then
  echo "FAILED: ${failures} of ${total} project(s):${failed}"
  exit 1
fi
if [ "$ran" -ne "$total" ]; then
  echo "FAILED: ran ${ran} of ${total} project(s) — a short sweep is not a pass."
  exit 1
fi
echo "OK: all ${total} PackTest project(s) green."
