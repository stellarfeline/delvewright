#!/usr/bin/env bash
# ADR-0017: prove `cargo install delvec` will work — WITHOUT publishing anything.
#
# THE FAILURE MODE THIS EXISTS TO RULE OUT
#
# `cargo publish` is irreversible: a version can never be reused and a crate name
# can never be freed (`cargo yank` only stops NEW dependents selecting it; the
# bytes stay downloadable forever). So the whole packaging contract has to be
# proven on every push, from a tree nobody has published from.
#
# The obvious instrument, `cargo publish --dry-run`, has a documented way of
# being VACUOUS — this repo's own named failure class (CLAUDE.md: a green gate
# that binds to nothing). `delvec` depends on `delvewright-dsl`, which is a
# sibling in this workspace. If the dry run satisfied that dependency by reaching
# for the sibling ON DISK, it would prove nothing about a tarball a stranger
# downloads. MEASURED on cargo 1.97.1 (2026-08-06) rather than assumed: a
# multi-package `cargo package -p delvewright-dsl -p delvec` builds a temporary
# LOCAL REGISTRY under `target/package/tmp-registry/` holding the packaged
# sibling, and verifies the dependent against THAT — the build log prints
# `Compiling delvewright-dsl v0.1.0` with no path, where a workspace resolution
# would have printed the path. Good. But "good on the version we measured" is a
# fact about today, so this script does not rely on it: check 3 below rebuilds
# the packaged tarball in a directory OUTSIDE the workspace entirely.
#
# WHAT IS CHECKED
#
# 1. Both crates package at all (`cargo package`), which is where a path-only
#    dependency, a missing `description`/`license`, a `publish = false`, or a
#    file `include!`d from outside the package would fail — by name.
# 2. The GENERATED manifest that crates.io will actually serve declares no
#    `path`, carries the `=` requirement on the dsl crate, and has dropped EVERY
#    path-only dev-dependency entirely. (That last is the reason an unpublished
#    sibling may be used by `delvec`'s tests: verified here rather than trusted.
#    The set is read out of `crates/compiler/Cargo.toml`, never named here — a
#    named one binds to the dev-dep somebody thought of, and the second such
#    dependency arrived and was examined by nothing.)
# 3. The packaged `delvec` tarball, extracted into a temp directory with NO
#    workspace above it and NO path dependency anywhere, builds its binary — with
#    `delvewright-dsl` supplied from the packaged DSL TARBALL, i.e. the exact
#    bytes crates.io will hold, never from `crates/dsl` on disk.
#
# WHAT THIS DOES NOT PROVE
#
# That crates.io will accept the upload (name availability, token, rate limits)
# and that the registry serves what it was given. Nothing pre-publication can;
# that is what the release workflow's post-publish index poll is for.
#
# Deterministic and offline except for the crates.io index / dep downloads that
# `cargo package` needs anyway. Run from anywhere:
#
#   bash tools/check-publishable.sh [--allow-dirty]
#
# Exit 0 = publishable, 1 = a finding, 2 = IO/usage error.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="$ROOT/versions.toml"
[ -f "$MANIFEST" ] || { echo "FATAL: $MANIFEST not found" >&2; exit 2; }

# `--allow-dirty` is for local runs only. CI works from a clean checkout, so the
# VCS-dirty check stays armed there — a packaged tarball built from uncommitted
# bytes is exactly the artifact nobody could reproduce.
DIRTY_FLAG=()
[ "${1:-}" = "--allow-dirty" ] && DIRTY_FLAG=(--allow-dirty)

eval "$(python3 - "$MANIFEST" <<'PY'
import sys, tomllib
sys.stdout.reconfigure(newline="\n")  # CRLF-proof: tools/check-python-shell-newlines.py
e = tomllib.load(open(sys.argv[1], "rb"))["engine"]
for k in ("version", "crate", "dsl_crate", "dsl_crate_version", "dsl_crate_req"):
    print(f'{k.upper()}={e[k]!r}'.replace("'", '"'))
PY
)"

fails=0
pass() { printf '  ok   %s\n' "$1"; }
fail() { printf '  FAIL %s\n' "$1"; fails=$((fails + 1)); }

# AN ERROR PATH MUST NOT DEPEND ON AN ARTIFACT THE ERROR MAY HAVE PREVENTED FROM
# EXISTING (v1.0.0 release run, 2026-08-06).
#
# `cmd >"$LOG" 2>&1` is opened by the SHELL, before `cmd` is executed. On a
# runner with no build cache there was no `target/` directory at all, so the
# redirect failed, `cargo package` NEVER RAN, and the else-branch then `sed`ed a
# file whose absence was the actual finding. The report was not merely unhelpful,
# it was wrong about what happened: it said "cargo package failed" about a
# command that had not been executed.
#
# Two things are needed and neither substitutes for the other:
#   * the directory is guaranteed before any redirect into it
#     (`tools/check-shell-redirect-dirs.py` now requires that of every script);
#   * the report is honest when the log is missing or empty, so the next such
#     failure names itself instead of impersonating a different one.
emit_log() { # <log-path> <what-was-being-run>
  if [ ! -f "$1" ]; then
    printf '       (no log at %s — the redirect never opened it, so `%s` DID NOT RUN)\n' "$1" "$2" >&2
  elif [ ! -s "$1" ]; then
    printf '       (the log at %s is empty — `%s` ran and wrote nothing)\n' "$1" "$2" >&2
  else
    sed 's/^/       /' "$1" >&2
  fi
}

echo "== 1. both crates package =="
echo "   ($CRATE v$VERSION, $DSL_CRATE v$DSL_CRATE_VERSION)"
mkdir -p "$ROOT/target"
rm -rf "$ROOT/target/package"
PKG_LOG="$ROOT/target/package-log.txt"
rm -f "$PKG_LOG"
rc=0
# bash 3.2 — which macOS still ships, and which CLAUDE.md names as Dev — treats
# "${arr[@]}" on an EMPTY array as an unbound variable under `set -u`. The
# `${arr[@]+...}` guard expands to nothing when the array is unset or empty and
# to the quoted elements otherwise, so it is correct on 3.2 and on bash 5 alike.
(cd "$ROOT" && cargo package -p "$DSL_CRATE" -p "$CRATE" ${DIRTY_FLAG[@]+"${DIRTY_FLAG[@]}"} \
      >"$PKG_LOG" 2>&1) || rc=$?
if [ "$rc" -eq 0 ]; then
  pass "cargo package -p $DSL_CRATE -p $CRATE"
else
  fail "cargo package exited $rc:"; emit_log "$PKG_LOG" "cargo package"
  echo; echo "check-publishable: 1 finding" >&2; exit 1
fi

PKG="$ROOT/target/package"
CRATE_DIR="$PKG/$CRATE-$VERSION"
DSL_DIR="$PKG/$DSL_CRATE-$DSL_CRATE_VERSION"
for d in "$CRATE_DIR" "$DSL_DIR"; do
  [ -d "$d" ] || { fail "expected packaged tree $d"; }
done
[ "$fails" -eq 0 ] || { echo; echo "check-publishable: $fails finding(s)" >&2; exit 1; }

echo
echo "== 2. the manifest crates.io will serve =="
# `grep -c` counts the whole file and never exits early — `grep -q` on the right
# of a pipe is the SIGPIPE trap this repo has a gate for
# (tools/check-shell-pipe-shortcircuit.py).
for name in "$CRATE" "$DSL_CRATE"; do
  case "$name" in
    "$CRATE") gen="$CRATE_DIR/Cargo.toml" ;;
    *)        gen="$DSL_DIR/Cargo.toml" ;;
  esac
  n_path="$(grep -cE '^[[:space:]]*path[[:space:]]*=' "$gen" || true)"
  # `[lib] path` / `[[bin]] path` are TARGET paths, not dependency paths, and are
  # normal in a published manifest. Only a dependency `path` is disqualifying, so
  # count the ones that sit under a `[*dependencies*]` table.
  n_dep_path="$(python3 - "$gen" <<'PY'
import sys, tomllib
sys.stdout.reconfigure(newline="\n")  # CRLF-proof: tools/check-python-shell-newlines.py
m = tomllib.load(open(sys.argv[1], "rb"))
n = 0
for key, tbl in m.items():
    if "dependencies" not in key or not isinstance(tbl, dict):
        continue
    for _, spec in tbl.items():
        if isinstance(spec, dict) and "path" in spec:
            n += 1
print(n)
PY
)"
  if [ "$n_dep_path" -eq 0 ]; then
    pass "$name: 0 dependency \`path\` keys survive packaging ($n_path target path(s), which are fine)"
  else
    fail "$name: $n_dep_path dependency \`path\` key(s) in the packaged manifest — crates.io cannot resolve those"
  fi
  for field in description license repository readme; do
    if python3 -c "import sys,tomllib;sys.exit(0 if '$field' in tomllib.load(open('$gen','rb'))['package'] else 1)"; then
      pass "$name: declares $field"
    else
      fail "$name: packaged manifest has no \`$field\` (crates.io rejects the upload)"
    fi
  done
done

# The `=` requirement is the ONLY thing binding the two crates once `path` is
# gone, so it is asserted against versions.toml rather than eyeballed.
got_req="$(python3 - "$CRATE_DIR/Cargo.toml" "$DSL_CRATE" <<'PY'
import sys, tomllib
sys.stdout.reconfigure(newline="\n")  # CRLF-proof: tools/check-python-shell-newlines.py
m = tomllib.load(open(sys.argv[1], "rb"))
spec = m.get("dependencies", {}).get(sys.argv[2])
print(spec if isinstance(spec, str) else (spec or {}).get("version", "<absent>"))
PY
)"
if [ "$got_req" = "$DSL_CRATE_REQ" ]; then
  pass "$CRATE depends on $DSL_CRATE '$got_req' (== versions.toml dsl_crate_req)"
else
  fail "$CRATE depends on $DSL_CRATE '$got_req' but versions.toml says '$DSL_CRATE_REQ'"
fi

# A path-only DEV-dependency is stripped on publish, which is what lets an
# unpublished sibling be used by `delvec`'s test suite. That is load-bearing, so
# it is verified, not assumed — and the SET is read out of the source manifest
# rather than named here. A named one is a check that binds to the dev-dep
# somebody thought of: the second such dependency arrived (`delvewright-schem`,
# for the one test that authors a prefab palette) and was examined by nothing.
dev_paths="$(python3 - "$ROOT/crates/compiler/Cargo.toml" <<'PY'
import sys, tomllib
sys.stdout.reconfigure(newline="\n")  # CRLF-proof: tools/check-python-shell-newlines.py
m = tomllib.load(open(sys.argv[1], "rb"))
for name, spec in (m.get("dev-dependencies") or {}).items():
    if isinstance(spec, dict) and "path" in spec and "version" not in spec:
        print(name)
PY
)"
n_dev=0
for dep in $dev_paths; do
  n_dev=$((n_dev + 1))
  n_left="$(grep -cF "$dep" "$CRATE_DIR/Cargo.toml" || true)"
  if [ "$n_left" -eq 0 ]; then
    pass "path-only dev-dependency $dep is stripped from the packaged manifest"
  else
    fail "$dep survives into the packaged manifest ($n_left line(s)) — it would have to be published too"
  fi
done
if [ "$n_dev" -eq 0 ]; then
  fail "binding count is zero: no path-only dev-dependency found in crates/compiler/Cargo.toml, so this check examined nothing"
else
  pass "path-only dev-dependencies examined: $n_dev"
fi

echo
echo "== 3. the packaged tarball builds standing alone =="
# Outside the workspace, so no parent `[workspace]` and no sibling on disk can
# rescue it. `delvewright-dsl` comes from the packaged DSL TARBALL — the bytes
# crates.io will hold — via a config-level `[patch.crates-io]`. If the `=`
# requirement and the packaged dsl version disagreed, cargo would go looking for
# a `delvewright-dsl` on crates.io that does not exist and fail here.
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT
tar -xzf "$PKG/$CRATE-$VERSION.crate" -C "$SCRATCH"
tar -xzf "$PKG/$DSL_CRATE-$DSL_CRATE_VERSION.crate" -C "$SCRATCH"
STANDALONE="$SCRATCH/$CRATE-$VERSION"
mkdir -p "$STANDALONE/.cargo"
cat > "$STANDALONE/.cargo/config.toml" <<EOF
[patch.crates-io]
$DSL_CRATE = { path = "../$DSL_CRATE-$DSL_CRATE_VERSION" }
EOF
# Same shape as check 1, and therefore the same treatment: `$SCRATCH` is a
# `mktemp -d` so the directory is guaranteed here, but the else-branch must still
# be honest about a log that does not exist or is empty (a failed `cd`, a full
# disk) rather than reporting a build failure that never happened.
BUILD_LOG="$SCRATCH/build-log.txt"
rc=0
(cd "$STANDALONE" && cargo build --release --bin delvec \
      >"$BUILD_LOG" 2>&1) || rc=$?
if [ "$rc" -eq 0 ]; then
  pass "extracted $CRATE-$VERSION.crate builds \`delvec\` with no workspace and no path dep"
else
  fail "the packaged tarball does not build standing alone (cargo build exited $rc):"
  emit_log "$BUILD_LOG" "cargo build"
fi
if [ -x "$STANDALONE/target/release/delvec" ]; then
  reported="$("$STANDALONE/target/release/delvec" --version)"
  # `--version` prints `delvec <engine>, dsl <format>, mc <pinned>` (main.rs);
  # only the leading engine identity is this gate's business.
  if [ "${reported#delvec $VERSION}" != "$reported" ]; then
    pass "the standalone binary reports '$reported'"
  else
    fail "the standalone binary reports '$reported', expected it to open with 'delvec $VERSION'"
  fi
fi

echo
# Binding counts, so a green here can never be a green that examined nothing.
echo "check-publishable: 2 crate(s) packaged, 1 standalone build, ${VERSION} / ${DSL_CRATE_VERSION}"
if [ "$fails" -ne 0 ]; then
  echo "check-publishable: $fails finding(s)" >&2; exit 1
fi
echo "check-publishable: OK — \`cargo install $CRATE\` has everything it needs"
