#!/usr/bin/env bash
#
# Assemble the engine's TEST library: a content prefab library with the
# engine's own test data laid over it.
#
#   tools/ci/test-library.sh <content-prefabs-dir> <out-dir>
#
# Two kinds of engine test data, held with the tests:
#
#   - pools: the shipped library declares no pool. The engine's own `areas[]`
#     fixtures (crates/dsl/fixtures/valid/keep-*, crates/delvec/tests/fixtures/*)
#     draw pieces from pools, declared in
#     crates/delvec/tests/fixtures/test-pools/pools.json.
#   - pieces: the pieces only the engine's tests bind (`prefab/hello-room`), each
#     an `.nbt` and its `.json` at the top of
#     crates/delvec/tests/fixtures/test-pieces/. A subdirectory there (a piece's
#     generator, kept as its provenance record) is not a piece and is not copied.
#
# This copies every file of <content-prefabs-dir> into <out-dir> (which must not
# exist) and writes that `pools.json` and those pieces beside them.
#
# The one assembler: `crates/delvec/tests/common/pinned.rs` runs it over the
# pinned content tree for every engine test, and CI runs it over the
# `checkout-content` checkout before building a fixture that draws a pool or
# binds a test piece.
#
# Refuses a source library that declares its own `pools.json`, or that holds a
# file of the same name as a test piece: two pool tables, or two copies of one
# piece, for one build is two authorities, and the engine's would silently win.

set -euo pipefail

if [ "$#" -ne 2 ]; then
  echo "usage: tools/ci/test-library.sh <content-prefabs-dir> <out-dir>" >&2
  exit 2
fi
src="$1"
out="$2"
here="$(cd "$(dirname "$0")/../.." && pwd)"
pools="$here/crates/delvec/tests/fixtures/test-pools/pools.json"
pieces_dir="$here/crates/delvec/tests/fixtures/test-pieces"

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

# The test pieces, by what is on disk: every top-level `.nbt` and `.json`.
# Not `mapfile`: macOS ships bash 3.2.
pieces=()
for f in "$pieces_dir"/*.nbt "$pieces_dir"/*.json; do
  [ -f "$f" ] && pieces+=("$f")
done
if [ "${#pieces[@]}" = 0 ]; then
  echo "test-library: $pieces_dir holds no .nbt or .json; a test-piece overlay of nothing is a finding" >&2
  exit 1
fi
for f in "${pieces[@]}"; do
  name="$(basename "$f")"
  if [ -e "$src/$name" ]; then
    echo "test-library: $src holds $name, which is an engine test piece ($f) — two copies of one piece for one build" >&2
    exit 1
  fi
done

mkdir -p "$out"
cp -R "$src/." "$out/"
cp "$pools" "$out/pools.json"
for f in "${pieces[@]}"; do
  cp "$f" "$out/"
done

files="$(find "$out" -type f | wc -l | tr -d ' ')"
echo "test-library: $out — $files file(s): $src, plus the engine's test pools ($pools) and ${#pieces[@]} test-piece file(s) ($pieces_dir)"
