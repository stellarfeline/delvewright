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

echo "== Engine release line ([engine], ADR-0016 / ADR-0017) =="
# ADR-0016 requires four numbers to be ONE number: the version compiled into the
# binary, the git tag, the crates.io version, and the window the `/new-delve`
# skill declares. Before this block they agreed only by intention, and this
# repo's history is a list of contracts that lived in comments. Here they are
# bound; `.github/workflows/engine-release.yml` binds the git tag at release time
# (it cannot be checked from a working tree), and `tools/check-skill-version.py`
# binds the skill's window.
#
# Cargo manifests are parsed with tomllib rather than grepped: `name = ` and
# `version = ` occur inside dependency tables and inside comments, and a gate
# that matches the wrong line is worse than no gate.
# NOT `$(python3 - <<'PY' ... PY)`: macOS ships bash 3.2, whose command-
# substitution parser scans the heredoc body for backticks and parentheses even
# when the heredoc is QUOTED — so a backtick in a Python comment inside `$( )`
# is a syntax error reported at a line ~130 further down, in code that never
# changed. Measured 2026-08-06. Redirecting to a file keeps the heredoc out of a
# command substitution entirely, which makes the trap stop existing rather than
# leaving the next editor to rediscover it.
eng_report_file="$(mktemp)"
python3 - "$MANIFEST" "$ROOT" > "$eng_report_file" <<'PY'
import sys, tomllib
from pathlib import Path

manifest, root = Path(sys.argv[1]), Path(sys.argv[2])
e = tomllib.load(manifest.open("rb"))["engine"]
out = []
def ok(msg):   out.append(("ok", msg))
def bad(msg):  out.append(("FAIL", msg))

compiler = tomllib.load((root / "crates/compiler/Cargo.toml").open("rb"))
dsl      = tomllib.load((root / "crates/dsl/Cargo.toml").open("rb"))

# 1. crates.io identity. `cargo install` resolves by CRATE name, never by binary
#    name, so the package must BE `delvec` or `cargo install delvec` installs
#    somebody else's crate (or nothing).
for label, mani, want_name, want_ver in (
    ("compiler", compiler, e["crate"], e["version"]),
    ("dsl", dsl, e["dsl_crate"], e["dsl_crate_version"]),
):
    got_name, got_ver = mani["package"]["name"], mani["package"]["version"]
    (ok if got_name == want_name else bad)(
        f"{label} package name {got_name!r} (manifest: {want_name!r})")
    (ok if got_ver == want_ver else bad)(
        f"{label} package version {got_ver!r} (manifest: {want_ver!r})")

# 2. The lib TARGET keeps the pre-rename name on purpose (366 in-tree
#    `use delvewright_compiler::` paths). If that ever moves silently, every one
#    of them breaks at once, so it is pinned here rather than left to luck.
lib_name = compiler.get("lib", {}).get("name")
(ok if lib_name == "delvewright_compiler" else bad)(
    f"compiler lib target name {lib_name!r} (must stay 'delvewright_compiler')")

# 3. Once `path` is stripped on publish, the `=` requirement is the ONLY thing
#    tying `delvec` to a specific `delvewright-dsl`.
req = compiler["dependencies"][e["dsl_crate"]]
req = req if isinstance(req, str) else req.get("version", "<absent>")
(ok if req == e["dsl_crate_req"] else bad)(
    f"compiler depends on {e['dsl_crate']} {req!r} (manifest: {e['dsl_crate_req']!r})")

# 4. Publishability inventory, in BOTH directions. The two crates that must be
#    publishable, and every other workspace member that must NOT be — a new
#    crate added without `publish = false` would otherwise be swept onto
#    crates.io by the first `--workspace` anything, irreversibly.
members = tomllib.load((root / "Cargo.toml").open("rb"))["workspace"]["members"]
publishable = {e["crate"], e["dsl_crate"]}
n_pub = n_priv = 0
for m in members:
    mani = tomllib.load((root / m / "Cargo.toml").open("rb"))
    name, flag = mani["package"]["name"], mani["package"].get("publish", True)
    if name in publishable:
        n_pub += 1
        (ok if flag is not False else bad)(f"{name} is publishable")
    else:
        n_priv += 1
        (ok if flag is False else bad)(
            f"{name} declares `publish = false` (it is not on the release line "
            f"and must never reach crates.io)")
missing = publishable - {tomllib.load((root / m / "Cargo.toml").open("rb"))["package"]["name"] for m in members}
if missing:
    bad(f"versions.toml names crate(s) that are not workspace members: {sorted(missing)}")
ok(f"publish inventory: {len(members)} member(s) = {n_pub} publishable + {n_priv} private")

# 5. `rust-version` in a published manifest is a promise to a stranger running
#    `cargo install`. It must be the toolchain this repo actually builds on, not
#    a looser number nobody has tested.
toolchain = tomllib.load((root / "rust-toolchain.toml").open("rb"))["toolchain"]["channel"]
(ok if toolchain == e["rust_toolchain"] else bad)(
    f"rust-toolchain.toml channel {toolchain!r} (manifest: {e['rust_toolchain']!r})")
for label, mani in (("compiler", compiler), ("dsl", dsl)):
    rv = mani["package"].get("rust-version")
    (ok if rv == e["rust_toolchain"] else bad)(
        f"{label} rust-version {rv!r} (manifest: {e['rust_toolchain']!r})")

# 6. The release matrix and the shelf must be the same set, both directions —
#    a target in versions.toml with no matrix row is a promised binary nobody
#    builds; a matrix row with no manifest line is a binary nobody declared.
wf = (root / ".github/workflows/engine-release.yml").read_text(encoding="utf-8")
declared = set(e["targets"])
in_matrix = {t for t in declared if f"target: {t}," in wf}
stray = set()
import re as _re
for m in _re.finditer(r"target:\s*([A-Za-z0-9_.-]+),", wf):
    stray.add(m.group(1))
if not declared:
    bad("[engine].targets is empty — the shelf would bind to nothing")
elif in_matrix == declared and stray == declared:
    ok(f"release matrix == [engine].targets ({len(declared)} target(s))")
else:
    bad(f"release matrix {sorted(stray)} != [engine].targets {sorted(declared)}")

# 7. The build script must hold no COPY of the shelf — the way a consumer cannot
#    drift from the manifest is to carry nothing (same rule as the server-jar
#    bootstrap below).
script = (root / "tools/build-release-binaries.sh").read_text(encoding="utf-8")
code = "\n".join(l for l in script.splitlines() if not l.lstrip().startswith("#"))
hard = sorted(t for t in declared if t in code)
(ok if not hard else bad)(
    "tools/build-release-binaries.sh hardcodes no target triple"
    if not hard else
    f"tools/build-release-binaries.sh hardcodes {hard} — read them from versions.toml")

for status, msg in out:
    print(f"{status}\t{msg}")
PY
while IFS=$'\t' read -r status msg; do
  [ -n "$status" ] || continue
  if [ "$status" = "ok" ]; then pass "$msg"; else fail "$msg"; fi
done < "$eng_report_file"
rm -f "$eng_report_file"

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
