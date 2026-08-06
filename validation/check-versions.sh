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
RENDER_CARGO="$ROOT/crates/render/Cargo.toml"
BOOTSTRAP_SH="$ROOT/validation/server-bootstrap-cache.sh"

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
emit("FABRIC_LAUNCHER",   d["fabric"]["launcher_version"])
emit("FABRIC_API_SHA1",   d["fabric"]["api_sha1"])
emit("PACKTEST_VERSION",  d["packtest"]["version"])
emit("PACKTEST_SHA1",     d["packtest"]["sha1"])
emit("MINEFLAYER",        d["harness"]["mineflayer"])
emit("CONTENT_REPO",      d["content"]["repo"])   # spec-0007 pinned content repo
emit("CONTENT_SHA",       d["content"]["sha"])
emit("NUCLEATION_REV",    d["render"]["nucleation_rev"])   # spec-0007 render layer
emit("NUCLEATION_REPO",   d["render"]["nucleation_repo"])
emit("CHUNKY_CORE",       d["render"]["chunky_core"])
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
# ...and the Fabric LAUNCHER by name (task #41). Left at itzg's `LATEST` default this
# is a meta.fabricmc.net request on every boot of an already-provisioned image — six
# per `tier 2` run, each its own chance to red a required check.
want_in "fabric launcher -> compose" "$FABRIC_LAUNCHER" "$COMPOSE"

echo "== Harness =="
want_in "mineflayer -> harness/package.json" "$MINEFLAYER" "$HARNESS_PKG"

echo "== Content repo pin (spec-0007 [content]) =="
# The prefab library + campaign sources live in the pinned content repo; CI
# (.github/actions/checkout-content) checks it out at this SHA and the compiler
# stamps it into manifest.json. Guard the shape so a malformed pin fails tier 1
# rather than at checkout/build time.
# Shape tests use bash's own `=~`, never `printf | grep -q`: grep exits at the
# match and SIGPIPEs printf, which `pipefail` reads as NO MATCH — a well-formed
# pin would fail its own guard (tools/check-shell-pipe-shortcircuit.py).
if [[ $CONTENT_REPO =~ ^[A-Za-z0-9._-]+/[A-Za-z0-9._-]+$ ]]; then
  pass "content.repo is a valid owner/name ($CONTENT_REPO)"
else
  fail "content.repo '$CONTENT_REPO' is not a valid GitHub owner/name slug"
fi
# A pinned commit must be a full 40-hex SHA (determinism: ADR-0006). "unpinned" or
# a short/branch ref is a mistake — CI would build against a moving target.
if [[ $CONTENT_SHA =~ ^[0-9a-f]{40}$ ]]; then
  pass "content.sha is a full 40-hex commit ($CONTENT_SHA)"
else
  fail "content.sha '$CONTENT_SHA' is not a full 40-hex commit SHA"
fi

echo "== Render layer ([render], spec-0007) =="
# Nucleation is pinned by git REV; the compiler-independent render crate must pin
# exactly this rev, and a rev must be a full 40-hex commit (determinism/repro).
if [[ $NUCLEATION_REV =~ ^[0-9a-f]{40}$ ]]; then
  pass "render.nucleation_rev is a full 40-hex commit ($NUCLEATION_REV)"
else
  fail "render.nucleation_rev '$NUCLEATION_REV' is not a full 40-hex commit SHA"
fi
if [ -f "$RENDER_CARGO" ]; then
  want_in "nucleation rev -> crates/render/Cargo.toml"  "$NUCLEATION_REV"  "$RENDER_CARGO"
  want_in "nucleation repo -> crates/render/Cargo.toml" "$NUCLEATION_REPO" "$RENDER_CARGO"
else
  fail "crates/render/Cargo.toml missing (cannot verify the render dep pin)"
fi
# Chunky is a snapshot core (1.21.x needs it); assert the pin looks like one.
if [[ $CHUNKY_CORE =~ ^chunky-core-.*SNAPSHOT ]]; then
  pass "render.chunky_core is a snapshot build ($CHUNKY_CORE)"
else
  fail "render.chunky_core '$CHUNKY_CORE' is not a chunky-core snapshot build"
fi

echo "== Server-jar bootstrap (task #41) =="
# `server_jar_url` / `server_jar_sha256` stopped being provenance-only on 2026-08-05:
# validation/server-bootstrap-cache.sh performs the ONE live fetch of a `tier 2` run
# and verifies it against the pin. The way a consumer cannot drift from the manifest
# is to hold no COPY of the value, so what is asserted here is the absence of one:
# the script must read both keys out of versions.toml and must hardcode neither the
# checksum nor the download host. (This is NOT the 2026-07-30 jar cache returning —
# that was a read-only bind mount of the jar, which itzg's rename-into-place could
# not target; this is a copy, and its point is the fetch COUNT, not speed.)
if [ -f "$BOOTSTRAP_SH" ]; then
  if grep -q 'server_jar_url' "$BOOTSTRAP_SH" && grep -q 'server_jar_sha256' "$BOOTSTRAP_SH"; then
    pass "bootstrap script reads server_jar_url + server_jar_sha256 from versions.toml"
  else
    fail "validation/server-bootstrap-cache.sh must read server_jar_url and server_jar_sha256 from versions.toml"
  fi
  # `grep -c` counts the whole file; it never stops reading early (task #173).
  strayhash="$(grep -cE '[0-9a-f]{64}' "$BOOTSTRAP_SH" || true)"
  strayurl="$(grep -cF 'piston-data.mojang.com' "$BOOTSTRAP_SH" || true)"
  if [ "$strayhash" -eq 0 ] && [ "$strayurl" -eq 0 ]; then
    pass "bootstrap script hardcodes no checksum and no download host"
  else
    fail "validation/server-bootstrap-cache.sh hardcodes a checksum ($strayhash line(s)) or the Mojang host ($strayurl line(s)) — read them from versions.toml, or the pin and the fetch can disagree"
  fi
else
  fail "validation/server-bootstrap-cache.sh missing — it is the single-fetch bootstrap tier 2 depends on"
fi

echo
if [ "$fails" -ne 0 ]; then
  echo "check-versions: $fails inconsistency(ies) vs versions.toml"; exit 1
fi
echo "check-versions: all consumers agree with versions.toml"
