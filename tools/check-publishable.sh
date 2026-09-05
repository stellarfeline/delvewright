#!/usr/bin/env bash
# ADR-0017 / ADR-0023 §6: prove `cargo install delvec` will work — WITHOUT
# publishing anything.
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
# that binds to nothing). `delvec` depends on seven sibling crates in this
# workspace. If the dry run satisfied those dependencies by reaching for the
# siblings ON DISK, it would prove nothing about a tarball a stranger downloads.
# MEASURED on cargo 1.97.1 (2026-08-06) rather than assumed: a multi-package
# `cargo package` builds a temporary LOCAL REGISTRY under
# `<target>/package/tmp-registry/` holding the packaged siblings, and verifies each
# dependent against THAT. Good. But "good on the version we measured" is a fact
# about today, so this script does not rely on it: check 3 below rebuilds the
# packaged `delvec` tarball in a directory OUTSIDE the workspace entirely, with
# every sibling supplied from ITS packaged tarball.
#
# WHAT IS CHECKED
#
# 1. Every published crate packages at all (`cargo package`), which is where a
#    path-only dependency, a missing `description`/`license`, a `publish =
#    false`, or a file `include!`d from outside the package would fail — by
#    name. The set is `versions.toml [engine]`: the DSL crate, every crate in
#    `crates`, and `crate` itself.
# 2. The GENERATED manifest that crates.io will actually serve declares no
#    `path` under any `*dependencies*` table (dev-dependencies included — a
#    path-only dev-dependency is stripped, one carrying a version survives as a
#    registry dependency, and either way nothing named `path` may remain),
#    carries the four fields crates.io refuses an upload without, and names
#    every in-tree sibling by the `=` requirement versions.toml declares.
# 3. The packaged `delvec` tarball, extracted into a temp directory with NO
#    workspace above it and NO path dependency anywhere, builds its binary — with
#    every sibling supplied from the packaged TARBALLS, i.e. the exact bytes
#    crates.io will hold, never from `crates/*` on disk — and the binary it
#    builds offers the whole surface (`--version`, and `--help` on every
#    mounted group).
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
print('ENGINE_CRATES=' + repr(" ".join(e["crates"])).replace("'", '"'))
PY
)"
# Dependency order, as versions.toml states it: the DSL crate first, the engine
# library crates, the binary last. bash 3.2 (macOS) has no `mapfile`.
NAMES=("$DSL_CRATE")
VERS=("$DSL_CRATE_VERSION")
for n in $ENGINE_CRATES; do NAMES+=("$n"); VERS+=("$VERSION"); done
NAMES+=("$CRATE"); VERS+=("$VERSION")

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

# A STALE SIBLING IS A SILENT WRONG ANSWER. `cargo package` verifies each
# dependent against the siblings in its temporary registry, and cargo extracts a
# registry crate into `$CARGO_HOME/registry/src/<registry>/<name>-<version>/`
# keyed by name and version alone — it never re-extracts when the bytes under
# the same version change. Re-packaging an UNPUBLISHED version after its source
# moved (every push of a branch does this) therefore verifies the dependents
# against the previous packaging's source, and the verdict is about a tree that
# no longer exists (measured: grammar's verify failed on a `GeneratedBy` shape
# the packaged dsl had already left behind), and the same holds one layer up:
# the `.crate` cargo copies into `registry/cache/<registry>/<name>-<version>.crate`
# is reused by name and version too (measured: the compiler's verify unpacked a
# dsl 0.2.0 tarball from the previous run and missed a function the packaged one
# exports). Both copies of OUR crates at OUR versions are purged from every
# registry cache except crates.io's own — whose copy, if one exists at this
# version, is the burned-version finding `crates-io-publish.sh` reports and
# nothing here may hide.
CARGO_REG="${CARGO_HOME:-$HOME/.cargo}/registry"
purged=0
i=0
while [ "$i" -lt "${#NAMES[@]}" ]; do
  for d in "$CARGO_REG"/src/*/"${NAMES[$i]}-${VERS[$i]}" "$CARGO_REG"/cache/*/"${NAMES[$i]}-${VERS[$i]}.crate"; do
    [ -e "$d" ] || continue
    case "$d" in "$CARGO_REG"/*/index.crates.io-*) continue ;; esac
    rm -rf "$d"; purged=$((purged + 1))
  done
  i=$((i + 1))
done
echo "== 0. stale extractions of our own crates purged from the temporary registry cache: $purged =="

echo "== 1. every published crate packages =="
echo "   (${#NAMES[@]} crates: ${NAMES[*]}; engine v$VERSION, $DSL_CRATE v$DSL_CRATE_VERSION)"
mkdir -p "$ROOT/target"
# The verify builds get a target directory of their own, emptied first. Cargo
# fingerprints a REGISTRY dependency as immutable — by name, version and
# registry, never by its bytes — so a `delvewright-dsl 0.2.0` rlib compiled by
# a previous run's verify from a previous packaging is reused as-is by the next
# run's, and a dependent is then judged against a sibling that no longer exists
# (measured: the compiler's verify missed a function the freshly packaged dsl
# exports, with `Unpacking` of the fresh tarball printed right above it). An
# unpublished version has no immutable bytes, so nothing built from one may
# outlive the run that built it.
VERIFY_TARGET="$ROOT/target/package-verify"
rm -rf "$VERIFY_TARGET"
PKG_LOG="$ROOT/target/package-log.txt"
rm -f "$PKG_LOG"
PKG_ARGS=()
for n in "${NAMES[@]}"; do PKG_ARGS+=(-p "$n"); done
rc=0
# bash 3.2 — which macOS still ships, and which CLAUDE.md names as Dev — treats
# "${arr[@]}" on an EMPTY array as an unbound variable under `set -u`. The
# `${arr[@]+...}` guard expands to nothing when the array is unset or empty and
# to the quoted elements otherwise, so it is correct on 3.2 and on bash 5 alike.
(cd "$ROOT" && CARGO_TARGET_DIR="$VERIFY_TARGET" cargo package "${PKG_ARGS[@]}" ${DIRTY_FLAG[@]+"${DIRTY_FLAG[@]}"} \
      >"$PKG_LOG" 2>&1) || rc=$?
if [ "$rc" -eq 0 ]; then
  pass "cargo package ${PKG_ARGS[*]}"
else
  fail "cargo package exited $rc:"; emit_log "$PKG_LOG" "cargo package"
  echo; echo "check-publishable: 1 finding" >&2; exit 1
fi

# `cargo package` writes beside the target directory it builds in.
PKG="$VERIFY_TARGET/package"
i=0
while [ "$i" -lt "${#NAMES[@]}" ]; do
  d="$PKG/${NAMES[$i]}-${VERS[$i]}"
  [ -d "$d" ] || fail "expected packaged tree $d"
  i=$((i + 1))
done
[ "$fails" -eq 0 ] || { echo; echo "check-publishable: $fails finding(s)" >&2; exit 1; }

echo
echo "== 2. the manifests crates.io will serve =="
# One pass over every packaged manifest, in python: `path` keys under any
# dependencies table, the four required fields, and the `=` requirement on every
# in-tree sibling. tomllib rather than grep: `path =` also names `[lib] path`,
# which is a target path and is fine in a published manifest.
report_file="$(mktemp)"
python3 - "$PKG" "$VERSION" "$DSL_CRATE" "$DSL_CRATE_VERSION" "$DSL_CRATE_REQ" "${NAMES[@]}" > "$report_file" <<'PY'
import sys, tomllib
from pathlib import Path
sys.stdout.reconfigure(newline="\n")  # CRLF-proof: tools/check-python-shell-newlines.py
pkg, version, dsl_crate, dsl_version, dsl_req = sys.argv[1:6]
names = sys.argv[6:]
out = []
ok = lambda m: out.append(("ok", m))
bad = lambda m: out.append(("FAIL", m))
n_req = 0
for n in names:
    gen = Path(pkg) / f"{n}-{dsl_version if n == dsl_crate else version}" / "Cargo.toml"
    m = tomllib.load(gen.open("rb"))
    paths = [
        f"{key}.{dep}" for key, tbl in m.items() if "dependencies" in key and isinstance(tbl, dict)
        for dep, spec in tbl.items() if isinstance(spec, dict) and "path" in spec
    ]
    if paths:
        bad(f"{n}: dependency `path` key(s) survive packaging: {paths} — crates.io cannot resolve those")
    else:
        ok(f"{n}: 0 dependency `path` keys survive packaging")
    for field in ("description", "license", "repository", "readme"):
        if field in m["package"]:
            ok(f"{n}: declares {field}")
        else:
            bad(f"{n}: packaged manifest has no `{field}` (crates.io rejects the upload)")
    # Once `path` is gone, the `=` requirement is the ONLY thing binding a crate
    # to its siblings, so every in-tree dependency is asserted by version.
    for key, tbl in m.items():
        if "dependencies" not in key or not isinstance(tbl, dict):
            continue
        for dep, spec in tbl.items():
            if dep not in names:
                continue
            req = spec if isinstance(spec, str) else spec.get("version", "<absent>")
            want = dsl_req if dep == dsl_crate else f"={version}"
            n_req += 1
            if req == want:
                ok(f"{n} depends on {dep} '{req}' (== versions.toml)")
            else:
                bad(f"{n} depends on {dep} '{req}' but versions.toml says '{want}'")
if n_req == 0:
    bad("binding count is zero: no packaged manifest names an in-tree sibling, so the `=` requirement check examined nothing")
else:
    ok(f"in-tree requirements examined: {n_req}")
for status, msg in out:
    print(f"{status}\t{msg}")
PY
while IFS=$'\t' read -r status msg; do
  [ -n "$status" ] || continue
  if [ "$status" = "ok" ]; then pass "$msg"; else fail "$msg"; fi
done < "$report_file"
rm -f "$report_file"

echo
echo "== 3. the packaged binary builds standing alone =="
# Outside the workspace, so no parent `[workspace]` and no sibling on disk can
# rescue it. Every sibling comes from ITS packaged TARBALL — the bytes crates.io
# will hold — via a config-level `[patch.crates-io]`. If an `=` requirement and
# a packaged version disagreed, cargo would go looking for a crate on crates.io
# that does not exist and fail here.
SCRATCH="$(mktemp -d)"
trap 'rm -rf "$SCRATCH"' EXIT
i=0
while [ "$i" -lt "${#NAMES[@]}" ]; do
  tar -xzf "$PKG/${NAMES[$i]}-${VERS[$i]}.crate" -C "$SCRATCH"
  i=$((i + 1))
done
STANDALONE="$SCRATCH/$CRATE-$VERSION"
mkdir -p "$STANDALONE/.cargo"
{
  echo "[patch.crates-io]"
  i=0
  while [ "$i" -lt "${#NAMES[@]}" ]; do
    if [ "${NAMES[$i]}" != "$CRATE" ]; then
      echo "${NAMES[$i]} = { path = \"../${NAMES[$i]}-${VERS[$i]}\" }"
    fi
    i=$((i + 1))
  done
} > "$STANDALONE/.cargo/config.toml"
# Same shape as check 1, and therefore the same treatment: `$SCRATCH` is a
# `mktemp -d` so the directory is guaranteed here, but the else-branch must still
# be honest about a log that does not exist or is empty (a failed `cd`, a full
# disk) rather than reporting a build failure that never happened.
BUILD_LOG="$SCRATCH/build-log.txt"
rc=0
(cd "$STANDALONE" && cargo build --release --bin delvec \
      >"$BUILD_LOG" 2>&1) || rc=$?
if [ "$rc" -eq 0 ]; then
  pass "extracted $CRATE-$VERSION.crate builds \`delvec\` with no workspace and no path dep ($((${#NAMES[@]} - 1)) sibling tarballs patched in)"
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
  # The crates.io bytes must build the SAME surface the archive carries
  # (ADR-0023 §3): every mounted group answers `--help`.
  n_groups=0
  for group in grammar prefab schem harvest render; do
    if "$STANDALONE/target/release/delvec" "$group" --help >/dev/null 2>&1; then
      n_groups=$((n_groups + 1))
    else
      fail "the standalone binary does not offer \`delvec $group\`"
    fi
  done
  pass "the standalone binary mounts $n_groups/5 surfaces"
fi

echo
# Binding counts, so a green here can never be a green that examined nothing.
echo "check-publishable: ${#NAMES[@]} crate(s) packaged, 1 standalone build, engine ${VERSION} / dsl ${DSL_CRATE_VERSION}"
if [ "$fails" -ne 0 ]; then
  echo "check-publishable: $fails finding(s)" >&2; exit 1
fi
echo "check-publishable: OK — \`cargo install $CRATE\` has everything it needs"
