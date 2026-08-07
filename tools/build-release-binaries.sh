#!/usr/bin/env bash
# ADR-0017: build ONE release-shelf artifact for `delvec`, or cross-CHECK the
# whole shelf.
#
# WHY THIS IS A SCRIPT AND NOT WORKFLOW YAML
#
# Two callers need the same target list and the same archive layout: the CI job
# that proves the shelf still cross-builds on every PR, and the release workflow
# that actually fills it on a `v*` tag. If each carried its own copy they would
# drift, and the drift would surface at release time — after the tag is pushed.
# Both call this, and this holds NO copy of any pinned value: the version and the
# target list are read out of `versions.toml`, the way
# `validation/server-bootstrap-cache.sh` reads the server-jar pin. A target
# triple hardcoded here is a `check-versions.sh` failure by construction.
#
# ARCHIVE SHAPE
#
#   dist/delvec-v<version>-<target>.tar.gz        binary + LICENSE
#   dist/delvec-v<version>-<target>.tar.gz.sha256 the checksum line for it
#
# `.tar.gz` for EVERY target including Windows, on purpose: one archive format is
# one extraction path for ADR-0014's future bootstrap, and a per-OS format branch
# is somewhere for the shelf to end up half-built. Windows 10 1803+ ships bsdtar;
# Windows 11 Explorer opens `.tar.gz` natively.
#
# LICENSE travels inside every archive: the binary is a GPL-3.0-only work and
# §4 requires the license text to accompany it (the tag body and the crates.io
# page are not "accompanying" a downloaded tarball).
#
# The archives are NOT claimed byte-reproducible — rustc output is not identical
# across runner images, and ADR-0006's determinism invariant is about the
# COMPILER'S OUTPUT (datapack + world), not about our own build artifacts. What
# binds a download to a release is the published sha256, which is emitted here
# and asserted by the release workflow.
#
# Usage:
#   tools/build-release-binaries.sh --list-targets
#   tools/build-release-binaries.sh --check-only          # every target, no link
#   tools/build-release-binaries.sh --target <triple>     # build + archive one
#
# Exit 0 = success, 1 = a finding, 2 = usage/IO error.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="$ROOT/versions.toml"
[ -f "$MANIFEST" ] || { echo "FATAL: $MANIFEST not found" >&2; exit 2; }

# Every value this script pins comes back through here, so the `\n` it prints has
# to BE a `\n`. On Windows a text-mode `print` writes `\r\n`, and the trailing
# `\r` survives command substitution — which is how the very first release run
# rejected `x86_64-pc-windows-msvc` as "not in versions.toml [engine].targets" on
# the msvc runner alone while the other four targets went green (v1.0.0,
# 2026-08-06). `reconfigure(newline="\n")` makes the interpreter's platform
# irrelevant; `tools/check-python-shell-newlines.py` requires it of every python
# in this repo whose stdout a shell reads.
read_manifest() { # <python expression over `e` == the [engine] table>
  python3 - "$MANIFEST" "$1" <<'PY'
import sys, tomllib
sys.stdout.reconfigure(newline="\n")  # CRLF-proof: tools/check-python-shell-newlines.py
e = tomllib.load(open(sys.argv[1], "rb"))["engine"]
print(eval(sys.argv[2]))
PY
}

VERSION="$(read_manifest 'e["version"]')"
CRATE="$(read_manifest 'e["crate"]')"
# Not `mapfile`: macOS ships bash 3.2 and the owner's workstation is a dev
# environment this must run on (CLAUDE.md Environments).
TARGETS=()
while IFS= read -r _line; do TARGETS+=("$_line"); done < <(read_manifest '"\n".join(e["targets"])')

usage() {
  cat >&2 <<EOF
usage: ${BASH_SOURCE[0]} (--list-targets | --check-only | --target <triple>)
  shelf: ${#TARGETS[@]} target(s) from versions.toml [engine].targets
EOF
  exit 2
}

# `shasum -a 256` on macOS, `sha256sum` on Linux and on the Windows runners'
# git-bash. Emits the two-field `<hex>  <name>` line both tools speak, from
# inside the file's directory so the recorded name has no path in it.
sha256_line() { # <file>
  local dir base
  dir="$(cd "$(dirname "$1")" && pwd)"; base="$(basename "$1")"
  if command -v sha256sum >/dev/null 2>&1; then
    (cd "$dir" && sha256sum "$base")
  else
    (cd "$dir" && shasum -a 256 "$base")
  fi
}

# RUSTFLAGS for a target. Everything here is a property of the SHELF, not of the
# workspace, so it lives at the call site rather than in `[profile.release]`
# (which every developer's `cargo build --release` also uses).
#
#   -C strip=symbols  a downloaded binary carries no debug symbols. 11.7 MB ->
#                     ~4 MB per target; the debug build in `target/` is
#                     untouched, and a backtrace from a release binary was never
#                     going to be useful without the matching source anyway.
#
#   -C linker=rust-lld (musl only)  `*-linux-musl` normally wants `musl-gcc`,
#                     which means an apt step on the runner and NOTHING on a
#                     macOS workstation — the shelf would be un-buildable on the
#                     owner's own dev machine, which is the "works on my machine"
#                     shape CLAUDE.md's Environments section forbids. rustup
#                     already ships `rust-lld` and a self-contained musl libc,
#                     and `delvec`'s dependency set is pure Rust (flate2 on the
#                     miniz_oxide backend), so lld links it directly. MEASURED
#                     2026-08-06: cross-linked from macOS/aarch64 to
#                     x86_64-unknown-linux-musl, `static-pie linked`, no apt, no
#                     cross-gcc, no container.
target_rustflags() { # <triple>
  local flags="-C strip=symbols"
  case "$1" in
    *-linux-musl)
      local sysroot host
      sysroot="$(rustc --print sysroot)"
      host="$(rustc -vV | sed -n 's/^host: //p')"
      flags="$flags -C linker=$sysroot/lib/rustlib/$host/bin/rust-lld -C linker-flavor=ld.lld"
      ;;
  esac
  printf '%s' "$flags"
}

# A musl artifact that quietly acquired a dynamic interpreter is no longer the
# "runs on any Linux" binary the shelf promises, and the promise would fail on a
# stranger's machine rather than here. The exact invariant is "this ELF has no
# PT_INTERP program header", so it is read out of the ELF header directly rather
# than pattern-matched on `file`'s prose (which is not a stable interface, and
# `file` is not on every runner).
assert_no_dynamic_interpreter() { # <binary>
  python3 - "$1" <<'PY'
import sys, struct
sys.stdout.reconfigure(newline="\n")  # CRLF-proof: tools/check-python-shell-newlines.py
path = sys.argv[1]
with open(path, "rb") as fh:
    data = fh.read()
if data[:4] != b"\x7fELF":
    sys.exit(f"{path}: not an ELF file — a musl shelf artifact must be one")
is64, little = data[4] == 2, data[5] == 1
end = "<" if little else ">"
if not is64:
    sys.exit(f"{path}: 32-bit ELF; every shelf target is 64-bit")
e_phoff, = struct.unpack_from(end + "Q", data, 0x20)
e_phentsize, e_phnum = struct.unpack_from(end + "HH", data, 0x36)
PT_INTERP = 3
for i in range(e_phnum):
    p_type, = struct.unpack_from(end + "I", data, e_phoff + i * e_phentsize)
    if p_type == PT_INTERP:
        sys.exit(f"{path}: has a PT_INTERP dynamic interpreter — not statically linked")
print(f"  ok   no PT_INTERP in {e_phnum} program headers (statically linked)")
PY
}

# ---------------------------------------------------------------- --check-only
# The standing gate (CI job `engine binaries (cross-build shelf)`). `cargo check`
# rather than a full link because ONE ubuntu runner can check all five targets —
# rustup ships std for every one of them regardless of host — while linking
# darwin and msvc needs their own runners, which is the release workflow's job.
#
# What this catches is the decay that would otherwise surface only at release
# time, with the tag already pushed: a new dependency that does not build for
# musl / msvc / darwin. Build scripts DO run under `cargo check`, so a dep that
# drags in a C toolchain fails here too.
check_only() {
  local failed=0
  echo "== cross-build shelf: cargo check for every target in versions.toml =="
  for t in "${TARGETS[@]}"; do
    printf '  -- %s\n' "$t"
    rustup target add "$t" >/dev/null
    if (cd "$ROOT" && cargo check --release --quiet -p "$CRATE" --bin delvec --target "$t"); then
      printf '  ok   %s\n' "$t"
    else
      printf '  FAIL %s does not compile\n' "$t"; failed=$((failed + 1))
    fi
  done
  # A gate that walked an empty set is vacuous, not green (CLAUDE.md).
  if [ "${#TARGETS[@]}" -eq 0 ]; then
    echo "build-release-binaries: FAIL — versions.toml [engine].targets is empty" >&2
    exit 1
  fi
  echo
  if [ "$failed" -ne 0 ]; then
    echo "build-release-binaries: $failed of ${#TARGETS[@]} shelf target(s) do not compile" >&2
    exit 1
  fi
  echo "build-release-binaries: OK — ${#TARGETS[@]} shelf target(s) compile (delvec v$VERSION)"
}

# -------------------------------------------------------------------- --target
build_one() { # <triple>
  local t="$1" known=0 exe="" stage archive
  for k in "${TARGETS[@]}"; do [ "$k" = "$t" ] && known=1; done
  if [ "$known" -ne 1 ]; then
    echo "build-release-binaries: '$t' is not in versions.toml [engine].targets" >&2
    exit 1
  fi
  case "$t" in *-windows-*) exe=".exe";; esac

  rustup target add "$t" >/dev/null
  (cd "$ROOT" && RUSTFLAGS="$(target_rustflags "$t")" \
      cargo build --release --locked -p "$CRATE" --bin delvec --target "$t")

  local bin="$ROOT/target/$t/release/delvec$exe"
  [ -f "$bin" ] || { echo "build-release-binaries: no binary at $bin" >&2; exit 1; }

  case "$t" in *-linux-musl) assert_no_dynamic_interpreter "$bin" ;; esac

  # The binary must be able to state its own identity, and it must be the
  # version this release claims. On a cross build we cannot run it, so the
  # check is by target: run it only when it is the host's own triple.
  local host; host="$(rustc -vV | sed -n 's/^host: //p')"
  if [ "$t" = "$host" ]; then
    local reported; reported="$("$bin" --version)"
    # `--version` prints `delvec <engine>, dsl <format>, mc <pinned>` (main.rs);
    # the engine identity is the leading field and the only one pinned here.
    if [ "${reported#delvec $VERSION}" = "$reported" ]; then
      echo "build-release-binaries: $bin reports '$reported', expected it to open with 'delvec $VERSION'" >&2
      exit 1
    fi
    printf '  ok   host-target binary reports: %s\n' "$reported"
  else
    printf '  --   %s is a cross build; --version not executable here\n' "$t"
  fi

  archive="delvec-v$VERSION-$t.tar.gz"
  stage="$ROOT/dist/.stage-$t"
  rm -rf "$stage"; mkdir -p "$stage" "$ROOT/dist"
  cp "$bin" "$stage/delvec$exe"
  cp "$ROOT/LICENSE" "$stage/LICENSE"
  tar -czf "$ROOT/dist/$archive" -C "$stage" "delvec$exe" LICENSE
  rm -rf "$stage"

  sha256_line "$ROOT/dist/$archive" > "$ROOT/dist/$archive.sha256"
  printf '  ok   %s\n' "$(cat "$ROOT/dist/$archive.sha256")"
  echo "build-release-binaries: OK — 1 archive + 1 checksum for $t (delvec v$VERSION)"
}

case "${1:-}" in
  --list-targets) printf '%s\n' "${TARGETS[@]}" ;;
  --check-only)   check_only ;;
  --target)       [ $# -eq 2 ] || usage; build_one "$2" ;;
  *)              usage ;;
esac
