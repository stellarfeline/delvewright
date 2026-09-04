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
CI_WF="$ROOT/.github/workflows/ci.yml"
RELEASE_WF="$ROOT/.github/workflows/release.yml"
SKIN_REQ="$ROOT/tools/skin/requirements.txt"
SKIN_PYPROJECT="$ROOT/tools/skin/pyproject.toml"
SKIN_CATALOG="$ROOT/tools/skin/delve_skin/catalog.py"
RENDER_CARGO="$ROOT/crates/render/Cargo.toml"
BOOTSTRAP_SH="$ROOT/validation/server-bootstrap-cache.sh"

[ -f "$MANIFEST" ] || { echo "FATAL: $MANIFEST not found"; exit 2; }

# --- pull every value we assert on out of the manifest in one shot --------------
eval "$(python3 - "$MANIFEST" <<'PY'
import sys, tomllib
sys.stdout.reconfigure(newline="\n")  # CRLF-proof: tools/check-python-shell-newlines.py
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
emit("DEEPSLATE_VERSION", d["render"]["deepslate_version"])
emit("GL_MATRIX_VERSION", d["render"]["gl_matrix_version"])
emit("ESBUILD_VERSION",   d["render"]["esbuild_version"])
emit("DEEPSLATE_BUNDLE",  d["render"]["deepslate_bundle"])
emit("DEEPSLATE_SHA256",  d["render"]["deepslate_bundle_sha256"])
emit("NODE_VERSION",         d["ci"]["node_version"])
emit("PYTHON_TOOLS_VERSION", d["ci"]["python_tools_version"])
emit("PYTHON_MECHA_VERSION", d["ci"]["python_mecha_version"])
emit("MECHA_VERSION",        d["ci"]["mecha_version"])
emit("MECHA_REQUIRES_PYTHON", d["ci"]["mecha_requires_python"])
emit("MECHA_REQUIRES_BEET",   d["ci"]["mecha_requires_beet"])
emit("BEET_VERSION",         d["ci"]["beet_version"])
emit("PYTEST_VERSION",       d["ci"]["pytest_version"])
emit("CARGO_AUDIT_VERSION",  d["ci"]["cargo_audit_version"])
emit("SKINPY_EXTENDED",      d["skin"]["skinpy_extended"])
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
# ...and the Fabric LAUNCHER by name. Left at itzg's `LATEST` default this
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
# The pin's ZONE INVENTORY is a consumer of this value like any other.
# `.github/content-zone-corpus.json` names the campaigns the pin carries and how
# many zone programs each declares; every number in it is checked against the
# content checkout by crates/grammar/tests/campaign_zones.rs. That check is only
# about the right corpus while the record and the pin agree, so a re-pin that
# leaves the inventory behind is caught here, in tier 1, with no content checkout
# needed — rather than measuring the new tree against the old pin's expectations.
ZONE_CORPUS="$ROOT/.github/content-zone-corpus.json"
if [ -f "$ZONE_CORPUS" ]; then
  # `sys.stdout.reconfigure(newline="\n")` before the print, or a Windows runner
  # would append a `\r` that survives command substitution and makes the SHA
  # compare unequal to itself (tools/check-python-shell-newlines.py).
  corpus_sha="$(python3 -c 'import json,sys; sys.stdout.reconfigure(newline="\n"); print(json.load(open(sys.argv[1]))["content_sha"])' "$ZONE_CORPUS")"
  if [ "$corpus_sha" = "$CONTENT_SHA" ]; then
    pass "content.sha -> .github/content-zone-corpus.json ($CONTENT_SHA)"
  else
    fail ".github/content-zone-corpus.json enumerates content $corpus_sha but versions.toml pins $CONTENT_SHA — restate the zone inventory at the new pin: every campaign it carries, with the number of zone programs each declares"
  fi
else
  fail ".github/content-zone-corpus.json missing — the campaign zone corpus is judged against that enumeration, and without it a pin carrying no zone program cannot be told from one that lost them"
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
# deepslate is BUNDLED, unlike every other renderer here, so the pin has to bind
# to the bytes rather than to a version string: the page carries the renderer
# inline, and a hand-edited or silently-rebuilt bundle would ship a different
# renderer under the same declared version. Every consumer of the three npm pins
# is the build script, and the fourth check is the digest of what it produced.
DEEPSLATE_BUILDER="$ROOT/tools/build-deepslate-bundle.sh"
if [ -f "$DEEPSLATE_BUILDER" ]; then
  want_in "deepslate version -> tools/build-deepslate-bundle.sh" "\"$DEEPSLATE_VERSION\"" "$DEEPSLATE_BUILDER"
  want_in "gl-matrix version -> tools/build-deepslate-bundle.sh" "\"$GL_MATRIX_VERSION\"" "$DEEPSLATE_BUILDER"
  want_in "esbuild version -> tools/build-deepslate-bundle.sh"   "\"$ESBUILD_VERSION\"" "$DEEPSLATE_BUILDER"
else
  fail "tools/build-deepslate-bundle.sh missing (cannot verify the bundled renderer pins)"
fi
if [ -f "$ROOT/$DEEPSLATE_BUNDLE" ]; then
  got="$(shasum -a 256 < "$ROOT/$DEEPSLATE_BUNDLE" | cut -d' ' -f1)"
  if [ "$got" = "$DEEPSLATE_SHA256" ]; then
    pass "deepslate bundle digest matches the manifest ($DEEPSLATE_SHA256)"
  else
    fail "deepslate bundle digest is $got but the manifest pins $DEEPSLATE_SHA256 — rebuild with tools/build-deepslate-bundle.sh and update versions.toml in the same commit"
  fi
  # The local patch, asserted on the shipped bytes. Unpatched, every banner and
  # shield in every page renders as the missing-texture checker and says nothing.
  if grep -qF 'entity/banner/banner_base' "$ROOT/$DEEPSLATE_BUNDLE"; then
    fail "the vendored deepslate bundle still asks for entity/banner/banner_base, a path no Minecraft version ships"
  else
    pass "the vendored deepslate bundle carries the banner/shield texture-id patch"
  fi
else
  fail "$DEEPSLATE_BUNDLE missing (the review page has no renderer to embed)"
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
# (it cannot be checked from a working tree), and the skill's window is bound in
# the campaigns repository, by `tools/check-skill-version.py` there — the page
# lives with the creator who clones that repository (ADR-0014), and a gate over a
# page this repository does not carry would bind to nothing.
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
sys.stdout.reconfigure(newline="\n")  # CRLF-proof: tools/check-python-shell-newlines.py
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
#    publishable, and every other crate under `crates/` that must NOT be — a new
#    crate added without `publish = false` would otherwise be swept onto
#    crates.io by the first `--workspace` anything, irreversibly.
#
#    `exclude` counts as much as `members`: `crates/render` sits outside the
#    workspace (its git dependency is quarantined there, /Cargo.toml), and a
#    members-only walk would have quietly dropped it from this inventory — the
#    exemption a crate gets for free by leaving the workspace is exactly the kind
#    that nobody notices. The binding count below is what makes that visible.
ws = tomllib.load((root / "Cargo.toml").open("rb"))["workspace"]
crates = list(ws["members"]) + [x for x in ws.get("exclude", []) if x.startswith("crates/")]
publishable = {e["crate"], e["dsl_crate"]}
n_pub = n_priv = 0
for m in crates:
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
missing = publishable - {tomllib.load((root / m / "Cargo.toml").open("rb"))["package"]["name"] for m in ws["members"]}
if missing:
    bad(f"versions.toml names crate(s) that are not workspace members: {sorted(missing)}")
ok(f"publish inventory: {len(crates)} crate(s) = {n_pub} publishable + {n_priv} private "
   f"({len(ws['members'])} workspace member(s) + {len(crates) - len(ws['members'])} excluded)")

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

# Every occurrence of a keyed line in a file states the manifest value, and there
# are exactly as many as declared. PRESENCE IS NOT ENOUGH for this class: the
# drift it catches is one of five statements of a value moving while the rest sit
# still, and a `grep -q` reads that as agreement because it stops at the first
# one. The declared count is what makes a NEW site a decision rather than a
# detail — a sixth job selecting a toolchain reds here until somebody says so.
all_stated() { # <label> <extended-regex> <expected-literal> <expected-count> <file>
  local label="$1" re="$2" good="$3" want="$4" file="$5"
  local n bad
  n="$( { grep -oE "$re" "$file" || true; } | wc -l | tr -d ' ')"
  bad="$( { grep -oE "$re" "$file" || true; } | grep -vxF "$good" | sort -u || true)"
  if [ -n "$bad" ]; then
    fail "$label: ${file##*/} states $(echo "$bad" | tr '\n' ' ') where versions.toml says '$good'"
  elif [ "$n" != "$want" ]; then
    fail "$label: ${file##*/} carries $n occurrence(s) and $want are declared — a new site for a pinned toolchain is a decision, not a detail"
  else
    pass "$label ($n x '$good' in ${file##*/})"
  fi
}

echo "== CI toolchain ([ci], [skin]) =="
# Node runtime for the harness and the storybook jobs.
all_stated "node -> ci.yml"      'node-version: *"[^"]*"' "node-version: \"$NODE_VERSION\"" 2 "$CI_WF"
all_stated "node -> release.yml" 'node-version: *"[^"]*"' "node-version: \"$NODE_VERSION\"" 3 "$RELEASE_WF"

# Python is the one value in this section that is NOT one value: two interpreter
# lines, for two disjoint dependency sets. So the binding is over the whole set —
# every `python-version:` in the workflow is one of the two the manifest
# declares, and each is used exactly as many times as declared. A third value
# appearing anywhere breaks the sum and reds, which is the property a per-value
# `want_in` cannot give.
py_total="$( { grep -oE 'python-version: *"[^"]*"' "$CI_WF" || true; } | wc -l | tr -d ' ')"
py_tools="$( { grep -oF "python-version: \"$PYTHON_TOOLS_VERSION\"" "$CI_WF" || true; } | wc -l | tr -d ' ')"
py_mecha="$( { grep -oF "python-version: \"$PYTHON_MECHA_VERSION\"" "$CI_WF" || true; } | wc -l | tr -d ' ')"
if [ "$py_tools" = "2" ] && [ "$py_mecha" = "1" ] && [ "$py_total" = "3" ]; then
  pass "python lines -> ci.yml (2 x $PYTHON_TOOLS_VERSION + 1 x $PYTHON_MECHA_VERSION = $py_total, none other)"
else
  fail "python lines -> ci.yml: $py_total selection(s), of which $py_tools at '$PYTHON_TOOLS_VERSION' and $py_mecha at '$PYTHON_MECHA_VERSION' — versions.toml declares 2 and 1 and nothing else"
fi

# The mecha cross-check's interpreter and its beet are ENTAILED, not chosen:
# mecha declares both floors itself. Recording them makes the reason machine-held
# instead of a sentence — lowering either line reds here rather than failing an
# install inside a required job, where the message would name pip and not the pin.
floor_report="$(python3 - "$PYTHON_MECHA_VERSION" "$MECHA_REQUIRES_PYTHON" "$BEET_VERSION" "$MECHA_REQUIRES_BEET" <<'FLOORS'
import sys
sys.stdout.reconfigure(newline="\n")  # CRLF-proof: tools/check-python-shell-newlines.py


def parts(v):
    return tuple(int(x) for x in v.split(".") if x.isdigit())


got_py, floor_py, got_beet, floor_beet = sys.argv[1:5]
for label, got, floor in (
    ("mecha cross-check interpreter", got_py, floor_py),
    ("beet", got_beet, floor_beet),
):
    ok = parts(got) >= parts(floor)
    print(f"{'ok' if ok else 'FAIL'}\t{label} {got} vs mecha's declared floor {floor}")
FLOORS
)"
while IFS=$'\t' read -r status msg; do
  [ -n "$status" ] || continue
  if [ "$status" = "ok" ]; then pass "$msg"; else fail "$msg"; fi
done <<< "$floor_report"

# The packages CI installs into those interpreters, each an instrument of a
# required status check and so each named literally rather than resolved.
all_stated "mecha -> ci.yml"  'mecha==[0-9A-Za-z.*+!-]*'  "mecha==$MECHA_VERSION"   1 "$CI_WF"
all_stated "beet -> ci.yml"   'beet==[0-9A-Za-z.*+!-]*'   "beet==$BEET_VERSION"     1 "$CI_WF"
all_stated "pytest -> ci.yml" 'pytest==[0-9A-Za-z.*+!-]*' "pytest==$PYTEST_VERSION" 2 "$CI_WF"
all_stated "cargo-audit -> ci.yml" 'cargo-audit --locked --version [0-9A-Za-z.*+!-]*' "cargo-audit --locked --version $CARGO_AUDIT_VERSION" 1 "$CI_WF"

echo "== Skin toolchain ([skin], spec-0009) =="
# Four statements of one library version, and the fourth is the one that matters
# most: catalog.py stamps it into every emitted provenance record, so a bump that
# missed it would ship metadata naming a library that did not draw the picture.
all_stated "skinpy -> tools/skin/requirements.txt" 'skinpy-extended==[0-9A-Za-z.*+!-]*' "skinpy-extended==$SKINPY_EXTENDED" 1 "$SKIN_REQ"
all_stated "skinpy -> tools/skin/pyproject.toml"   'skinpy-extended==[0-9A-Za-z.*+!-]*' "skinpy-extended==$SKINPY_EXTENDED" 1 "$SKIN_PYPROJECT"
all_stated "skinpy -> tools/skin/delve_skin/catalog.py" '"version": "[^"]*", "license"' "\"version\": \"$SKINPY_EXTENDED\", \"license\"" 1 "$SKIN_CATALOG"

echo "== Server-jar bootstrap =="
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
  # `grep -c` counts the whole file; it never stops reading early.
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
