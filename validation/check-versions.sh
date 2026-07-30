#!/usr/bin/env bash
# spec-0005 tier-1 consistency gate: versions.toml is the single source of truth.
# This script FAILS (exit 1) if any Dockerfile / compose / workflow hardcodes a
# version, digest, or checksum that disagrees with the repo-root versions.toml.
#
# Design: values are read from versions.toml via Python's stdlib tomllib (no third-
# party deps; present on ubuntu-latest and any Python >= 3.11). The consumer files
# are then asserted against those values with plain grep — every check names the
# consumer it guards. Wired into CI tier 1 (.github/workflows/ci.yml).
#
# Run from anywhere:  bash validation/check-versions.sh
set -euo pipefail

# Resolve repo root from this script's location so it works from any CWD / in CI.
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="$ROOT/versions.toml"
DELVE_DF="$ROOT/validation/Dockerfile.delve"
TOOL_DF="$ROOT/validation/Dockerfile.toolserver"
COMPOSE="$ROOT/validation/compose.yaml"
HARNESS_PKG="$ROOT/harness/package.json"

[ -f "$MANIFEST" ] || { echo "FATAL: $MANIFEST not found"; exit 2; }

# --- pull every value we assert on out of the manifest in one shot --------------
eval "$(python3 - "$MANIFEST" <<'PY'
import sys, tomllib
d = tomllib.load(open(sys.argv[1], "rb"))
def emit(k, v): print(f'{k}={v!r}'.replace("'", '"'))
emit("MC_VERSION",        d["minecraft"]["version"])
emit("SERVER_SHA256",     d["minecraft"]["server_jar_sha256"])
emit("BASE_DIGEST",       d["images"]["base"]["digest"])          # sha256:...
emit("TOOL_DIGEST",       d["images"]["toolserver"]["digest"])
emit("TOOL_TAG",          d["images"]["toolserver"]["tag"])
emit("FABRIC_LOADER",     d["fabric"]["loader_version"])
emit("FABRIC_API_SHA1",   d["fabric"]["api_sha1"])
emit("PACKTEST_VERSION",  d["packtest"]["version"])
emit("PACKTEST_SHA1",     d["packtest"]["sha1"])
emit("MINEFLAYER",        d["harness"]["mineflayer"])
PY
)"

fails=0
pass() { printf '  ok   %s\n' "$1"; }
fail() { printf '  FAIL %s\n' "$1"; fails=$((fails+1)); }

# Assert the manifest value is present in a file.
want_in() { # <label> <value> <file>
  if grep -qF -- "$2" "$3"; then pass "$1 ($2 in ${3##*/})"
  else fail "$1: expected '$2' in ${3##*/} but it is absent"; fi
}

# Assert NO conflicting variant of a token appears. <label> <regex> <good> <file...>
no_conflict() { # <label> <extended-regex> <expected> <file>...
  local label="$1" re="$2" good="$3"; shift 3
  local bad
  bad="$(grep -hoE "$re" "$@" 2>/dev/null | grep -vxF "$good" | sort -u || true)"
  if [ -n "$bad" ]; then
    fail "$label: found value(s) disagreeing with manifest '$good': $(echo "$bad" | tr '\n' ' ')"
  else pass "$label (no conflicting values; manifest=$good)"; fi
}

echo "== Minecraft version =="
want_in "MC version -> Dockerfile.delve"      "$MC_VERSION" "$DELVE_DF"
want_in "MC version -> Dockerfile.toolserver" "$MC_VERSION" "$TOOL_DF"
want_in "MC version -> compose"               "$MC_VERSION" "$COMPOSE"
# Any 1.21.x literal in a consumer must be exactly the pinned version.
no_conflict "MC version (no drift)" '1\.21\.[0-9]+' "$MC_VERSION" "$DELVE_DF" "$TOOL_DF" "$COMPOSE"

echo "== Image digests =="
# Every sha256:<64hex> literal in the image consumers must be a manifest digest
# (base or toolserver) — anything else is drift.
stray="$(grep -hoE 'sha256:[0-9a-f]{64}' "$DELVE_DF" "$TOOL_DF" "$COMPOSE" 2>/dev/null | grep -vxF "$BASE_DIGEST" | grep -vxF "$TOOL_DIGEST" | sort -u || true)"
if [ -n "$stray" ]; then fail "image digests (no stray digest): unknown digest(s): $(echo "$stray" | tr '\n' ' ')"
else pass "image digests (no stray digest)"; fi
want_in "base digest -> Dockerfile.delve"      "$BASE_DIGEST" "$DELVE_DF"
want_in "base digest -> Dockerfile.toolserver" "$BASE_DIGEST" "$TOOL_DF"

echo "== Fabric / PackTest (toolserver pre-bake) =="
want_in "fabric loader -> Dockerfile.toolserver" "$FABRIC_LOADER"   "$TOOL_DF"
want_in "fabric API sha1 -> Dockerfile.toolserver" "$FABRIC_API_SHA1" "$TOOL_DF"
want_in "packtest sha1 -> Dockerfile.toolserver"   "$PACKTEST_SHA1"   "$TOOL_DF"
# compose pins the toolserver by its manifest digest (spec-0005).
want_in "toolserver digest -> compose" "$TOOL_DIGEST" "$COMPOSE"

echo "== Harness =="
want_in "mineflayer -> harness/package.json" "$MINEFLAYER" "$HARNESS_PKG"

# The server-jar sha256 must key the CI actions/cache (guards the EULA jar cache).
echo "== CI server-jar cache key =="
if grep -rqF -- "$SERVER_SHA256" "$ROOT/.github/workflows/"; then
  pass "server jar sha256 keys a CI cache ($SERVER_SHA256)"
else
  fail "server jar sha256 '$SERVER_SHA256' not referenced by any workflow (cache key drift)"
fi

echo
if [ "$fails" -ne 0 ]; then
  echo "check-versions: $fails inconsistency(ies) vs versions.toml"; exit 1
fi
echo "check-versions: all consumers agree with versions.toml"
