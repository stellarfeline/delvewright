#!/usr/bin/env bash
#
# Assemble the engine's TEST library: a content prefab library with the
# engine's own pool declarations laid over it.
#
#   tools/ci/test-library.sh <content-prefabs-dir> <out-dir>
#
# The shipped library declares no pool. The engine's own `areas[]` fixtures
# (crates/dsl/fixtures/valid/keep-*, crates/delvec/tests/fixtures/*) draw
# pieces from pools, and those pools are engine test data:
# crates/delvec/tests/fixtures/test-pools/pools.json. This copies every file
# of <content-prefabs-dir> into <out-dir> (which must not exist) and writes
# that `pools.json` beside them.
#
# The one assembler: `crates/delvec/tests/common/pinned.rs` runs it over the
# pinned content tree for every engine test, and CI runs it over the
# `checkout-content` checkout before building a fixture that draws a pool.
#
# Refuses a source library that declares its own `pools.json`: two pool tables
# for one build is two authorities, and the engine's would silently win.

set -euo pipefail

if [ "$#" -ne 2 ]; then
  echo "usage: tools/ci/test-library.sh <content-prefabs-dir> <out-dir>" >&2
  exit 2
fi
src="$1"
out="$2"
here="$(cd "$(dirname "$0")/../.." && pwd)"
pools="$here/crates/delvec/tests/fixtures/test-pools/pools.json"

if [ ! -d "$src" ]; then
  echo "test-library: $src is not a directory (the content prefab library)" >&2
  exit 1
fi
if [ -e "$src/pools.json" ]; then
  echo "test-library: $src declares its own pools.json; the engine's test pools ($pools) would shadow it — two pool tables for one build" >&2
  exit 1
fi
if [ -e "$out" ]; then
  echo "test-library: $out exists; the test library is assembled into a fresh directory" >&2
  exit 1
fi
if [ ! -f "$pools" ]; then
  echo "test-library: $pools is missing" >&2
  exit 1
fi

mkdir -p "$out"
cp -R "$src/." "$out/"
cp "$pools" "$out/pools.json"

files="$(find "$out" -type f | wc -l | tr -d ' ')"
echo "test-library: $out — $files file(s): $src, plus the engine's test pools ($pools)"
