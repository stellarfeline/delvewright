#!/usr/bin/env bash
# The per-test coverage map (spec-0126 §3.1): which production regions each test
# executes, one process per test.
#
#   bash tools/ci/test-coverage-map.sh --out DIR [--jobs N] [--binary REGEX]
#
# Builds the workspace's test binaries instrumented (`cargo llvm-cov show-env`),
# lists them with `cargo nextest list --message-format json`, then runs every
# test that is not `#[ignore]` alone through `tools/lib/coverage_map.py`, which
# writes `DIR/tests/<binary>/<n>.json` per test and `DIR/summary.json`, and
# prints `coverage map: W tests written of L listed …`. A re-run over the same
# DIR resumes: a test already written is not run again.
#
# Refuses unless the installed cargo-llvm-cov and cargo-nextest are the versions
# versions.toml [ci] records (`llvm_cov_version`, `nextest_version`). sccache
# must not chain under the coverage wrapper, so RUSTC_WRAPPER is emptied.
# Heavy: a whole-suite map is hours of CPU; run it on a build machine.
set -euo pipefail

OUT=""
JOBS=""
BINARY=""
while [ $# -gt 0 ]; do
  case "$1" in
    --out) OUT="$2"; shift 2 ;;
    --jobs) JOBS="$2"; shift 2 ;;
    --binary) BINARY="$2"; shift 2 ;;
    -h|--help) sed -n '2,17p' "$0"; exit 0 ;;
    *) echo "test-coverage-map: unknown argument $1" >&2; exit 2 ;;
  esac
done
[ -n "$OUT" ] || { echo "test-coverage-map: --out DIR is required" >&2; exit 2; }

ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
pin() {
  python3 - "$ROOT/versions.toml" "$1" <<'PY'
import sys, tomllib
sys.stdout.reconfigure(newline="\n")
print(tomllib.load(open(sys.argv[1], "rb"))["ci"][sys.argv[2]])
PY
}
want_cov="$(pin llvm_cov_version)"
want_nextest="$(pin nextest_version)"
have_cov="$(cargo llvm-cov --version 2>/dev/null | awk '{print $2}' || true)"
have_nextest="$(cargo nextest --version 2>/dev/null | awk 'NR==1{print $2}' || true)"
if [ "$have_cov" != "$want_cov" ]; then
  echo "test-coverage-map: cargo-llvm-cov is '${have_cov:-absent}', versions.toml says $want_cov — cargo install --locked cargo-llvm-cov@$want_cov" >&2
  exit 1
fi
if [ "$have_nextest" != "$want_nextest" ]; then
  echo "test-coverage-map: cargo-nextest is '${have_nextest:-absent}', versions.toml says $want_nextest — cargo install --locked cargo-nextest@$want_nextest" >&2
  exit 1
fi

mkdir -p "$OUT"
OUT="$(cd "$OUT" && pwd)"
export RUSTC_WRAPPER=
eval "$(cargo llvm-cov show-env --sh)"
cargo nextest list --workspace --locked --message-format json > "$OUT/list.json"
args=(--list "$OUT/list.json" --out "$OUT" --root "$ROOT")
[ -z "$JOBS" ] || args+=(--jobs "$JOBS")
[ -z "$BINARY" ] || args+=(--binary "$BINARY")
python3 "$ROOT/tools/lib/coverage_map.py" "${args[@]}"
