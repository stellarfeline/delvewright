#!/usr/bin/env bash
# The kill-set proof of a test removal (spec-0126 §3.3 rule 3).
#
#   bash tools/ci/test-kill-set.sh --out DIR --test BINARY_ID::TEST \
#        --file SRC [--file SRC ...] --binary BINARY_ID [--binary BINARY_ID ...]
#
# Runs cargo-mutants twice over the mutants in the --file sources (the files of
# the candidate's region set, from the coverage map), judged by the --binary
# test binaries (every binary the map says covers those files): once with the
# candidate present and once with it filtered out. Writes DIR/with/ and
# DIR/without/ (each cargo-mutants' own `mutants.out`), DIR/command.txt (both
# command lines, verbatim), and prints `kill set: C caught with, C' caught
# without, D only with`. Exits 1 when the caught sets differ: a mutant caught
# only with the candidate names an assertion only it holds.
#
# Refuses unless the installed cargo-mutants is the version versions.toml [ci]
# records (`mutants_version`). Heavy; run it on a build machine.
set -euo pipefail

OUT=""
TEST=""
FILES=()
BINARIES=()
while [ $# -gt 0 ]; do
  case "$1" in
    --out) OUT="$2"; shift 2 ;;
    --test) TEST="$2"; shift 2 ;;
    --file) FILES+=("$2"); shift 2 ;;
    --binary) BINARIES+=("$2"); shift 2 ;;
    -h|--help) sed -n '2,17p' "$0"; exit 0 ;;
    *) echo "test-kill-set: unknown argument $1" >&2; exit 2 ;;
  esac
done
if [ -z "$OUT" ] || [ -z "$TEST" ] || [ ${#FILES[@]} -eq 0 ] || [ ${#BINARIES[@]} -eq 0 ]; then
  echo "test-kill-set: --out, --test, at least one --file and one --binary are required" >&2
  exit 2
fi

ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
want="$(python3 - "$ROOT/versions.toml" <<'PY'
import sys, tomllib
sys.stdout.reconfigure(newline="\n")
print(tomllib.load(open(sys.argv[1], "rb"))["ci"]["mutants_version"])
PY
)"
have="$(cargo mutants --version 2>/dev/null | awk '{print $2}' || true)"
if [ "$have" != "$want" ]; then
  echo "test-kill-set: cargo-mutants is '${have:-absent}', versions.toml says $want — cargo install --locked cargo-mutants@$want" >&2
  exit 1
fi

mkdir -p "$OUT"
OUT="$(cd "$OUT" && pwd)"
bin_expr=""
for b in "${BINARIES[@]}"; do
  bin_expr="${bin_expr:+$bin_expr | }binary_id($b)"
done
cand_bin="${TEST%%::*}"
cand_test="${TEST#*::}"
with_expr="$bin_expr"
without_expr="($bin_expr) & not (binary_id($cand_bin) & test(=$cand_test))"
fargs=()
for f in "${FILES[@]}"; do fargs+=(-f "$f"); done
{
  echo "cargo mutants --test-tool nextest ${fargs[*]} --output $OUT/with -- -E '$with_expr'"
  echo "cargo mutants --test-tool nextest ${fargs[*]} --output $OUT/without -- -E '$without_expr'"
} > "$OUT/command.txt"
export RUSTC_WRAPPER=
# cargo-mutants exits non-zero when any mutant is missed; the comparison below
# is the verdict, so neither run's own status decides it.
cargo mutants --test-tool nextest "${fargs[@]}" --output "$OUT/with" -- -E "$with_expr" || true
cargo mutants --test-tool nextest "${fargs[@]}" --output "$OUT/without" -- -E "$without_expr" || true
python3 - "$OUT" <<'PY'
import json, pathlib, sys
sys.stdout.reconfigure(newline="\n")
out = pathlib.Path(sys.argv[1])
def caught(d):
    o = json.loads((out / d / "mutants.out" / "outcomes.json").read_text(encoding="utf-8"))
    return {json.dumps(x["scenario"], sort_keys=True) for x in o["outcomes"] if x.get("summary") == "CaughtMutant"}
w, wo = caught("with"), caught("without")
print(f"kill set: {len(w)} caught with, {len(wo)} caught without, {len(w - wo)} only with")
sys.exit(1 if w != wo else 0)
PY
