#!/usr/bin/env bash
# ADR-0017 / ADR-0023: build ONE release-shelf artifact for `delvec`, or
# cross-CHECK the whole shelf.
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
# The binary is the WHOLE creator (ADR-0023 §3): compiler, prefab authoring and
# admission, schematic conversion, playtest harvesting, and both the CPU and the
# GPU render arms. Nothing is a second download and no cargo feature narrows it.
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
# LINUX IS GNU (ADR-0023 §4). The binary links glibc dynamically, so it carries a
# glibc floor: the newest `GLIBC_x.y` symbol version it references, which is a
# property of the runner that built it. The floor is MEASURED off the ELF and
# printed by `build_one`, never promised; a creator on an older distro builds
# from source, which is the guaranteed path (ADR-0023 §2).
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
#   -C strip=symbols  a downloaded binary carries no debug symbols; the debug
#                     build in `target/` is untouched, and a backtrace from a
#                     release binary was never going to be useful without the
#                     matching source anyway. NOTE this flag lives HERE and not
#                     in `[profile.release]`, so `cargo install delvec` hands
#                     out an UNstripped binary. That is a deliberate trade — a
#                     repo-wide `strip` would take symbols off every developer's
#                     `cargo build --release` — and it is recorded, not hidden
#                     (docs/reference/distribution-size.md).
#
# No target needs a linker flag: every Linux target is built natively on a
# runner of its own architecture (engine-release.yml), darwin links both slices
# on the macOS runner, and msvc links on the Windows runner.
target_rustflags() { # <triple>
  printf '%s' "-C strip=symbols"
}

# The glibc floor a Linux artifact actually carries, read off the ELF's
# versioned symbol references rather than promised from the runner's name. A
# floor that cannot be measured is a refusal, not a blank: a downloaded binary
# that fails on a stranger's machine is the exact moment this line exists for.
print_glibc_floor() { # <binary>
  command -v objdump >/dev/null 2>&1 || {
    echo "build-release-binaries: objdump is not on PATH, so the glibc floor of $1 cannot be measured" >&2
    exit 1
  }
  local floor
  floor="$( { objdump -T "$1" || true; } | grep -o 'GLIBC_[0-9][0-9.]*' | sort -t_ -k2,2V | uniq | tail -1)"
  if [ -z "$floor" ]; then
    echo "build-release-binaries: $1 references no GLIBC_* symbol version — not the gnu binary the shelf promises" >&2
    exit 1
  fi
  printf '  ok   glibc floor of %s: %s (newest versioned symbol the binary references)\n' "$(basename "$1")" "$floor"
}

# ------------------------------------------------- ADR-0021 §1 / ADR-0023 §3:
# the surface is unconditional code. A cargo feature would ship a
# same-name-different-capability binary — an artifact whose name promises a
# surface its bytes may not carry — and nothing else in this repo would notice,
# because every test builds with the same feature set. So the rule is asserted
# here, where the artifact is made, over EVERY crate whose clap types the binary
# mounts.
#
# This half needs no binary and therefore runs for EVERY target, cross or not.
assert_no_feature_gated_surface() {
  python3 - "$ROOT" <<'PY'
import pathlib, re, sys
sys.stdout.reconfigure(newline="\n")  # CRLF-proof: tools/check-python-shell-newlines.py
root = pathlib.Path(sys.argv[1])
# A clap item is a `#[derive(Parser)]`/`#[derive(Subcommand)]` type or anything
# inside one. A feature gate ANYWHERE in those files could remove a subcommand,
# a variant or a flag, so the rule is the file, not a line-by-line adjacency
# guess that a reformat would slip past.
CLAP = re.compile(r"#\[derive\((?:[^)]*\b(?:Parser|Subcommand|Args|ValueEnum)\b[^)]*)\)\]")
GATE = re.compile(r"#\[(?:cfg|cfg_attr)\(\s*(?:[^)]*\b)?feature\s*=")
examined, findings, crates = 0, [], set()
for f in sorted((root / "crates").glob("*/src/**/*.rs")):
    text = f.read_text(encoding="utf-8")
    if not CLAP.search(text):
        continue
    examined += 1
    crates.add(f.relative_to(root / "crates").parts[0])
    for i, line in enumerate(text.splitlines(), 1):
        if GATE.search(line):
            findings.append(f"{f.relative_to(root)}:{i}: {line.strip()}")
if examined == 0:
    print("  FAIL no file under crates/*/src declares a clap type — this "
          "check examined nothing, which is a vacuous pass, not a pass")
    raise SystemExit(1)
for hit in findings:
    print(f"  FAIL feature gate on a clap surface: {hit}")
if findings:
    raise SystemExit(1)
print(f"  ok   no cargo feature gates any subcommand ({examined} clap-bearing "
      f"file(s) examined across {len(crates)} crate(s))")
PY
}

# The other half: the BUILT binary lists exactly the subcommands the source
# declares — at the top level, and inside every mounted group (`delvec grammar
# --help` lists exactly the grammar verbs, and so on). Executable only on the
# host's own triple, like the `--version` check below; on a cross build it says
# so by name rather than passing quietly.
assert_help_matches_source() { # <binary>
  python3 - "$ROOT" "$1" <<'PY'
import importlib.util, pathlib, re, subprocess, sys
sys.stdout.reconfigure(newline="\n")  # CRLF-proof: tools/check-python-shell-newlines.py
root, binary = pathlib.Path(sys.argv[1]), sys.argv[2]

# ONE parser. `tools/lib/clap_surface.py` reads the clap surface out of the
# crates' sources for every gate that asks — here, and in the campaigns
# repository, which vendors that file byte-for-byte. A second copy would be a
# mirror that drifts, which is the defect this project names rather than a
# saving.
spec = importlib.util.spec_from_file_location(
    "clap_surface", root / "tools" / "lib" / "clap_surface.py")
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)
# The binary's own `main.rs` first — its `Cli` is the root — then every crate's
# sources, since the mounted surfaces are declared in their own crates.
main_rs = root / "crates" / "delvec" / "src" / "main.rs"
sources = [main_rs.read_text(encoding="utf-8")]
sources += [f.read_text(encoding="utf-8")
            for f in sorted((root / "crates").glob("*/src/**/*.rs")) if f != main_rs]
source = "\n".join(sources)
declared = set(gate.parse_cli(source)[0])
groups = gate.parse_groups(source)

def built_commands(args):
    out = subprocess.run([binary, *args, "--help"], capture_output=True, text=True).stdout
    block, built = False, set()
    for line in out.splitlines():
        if line.startswith("Commands:"):
            block = True
            continue
        if block:
            if re.match(r"^[A-Za-z].*:$", line):
                break
            m = re.match(r"^  ([a-z][a-z0-9-]*)\s", line)
            if m and m.group(1) != "help":
                built.add(m.group(1))
    return built

def compare(label, declared, built):
    if not built:
        print(f"  FAIL {label} lists no subcommands — this check examined "
              "nothing, which is a vacuous pass")
        return False
    missing, extra = sorted(declared - built), sorted(built - declared)
    if missing:
        print(f"  FAIL the source declares {missing} under {label} but the built "
              f"binary does not offer them — the artifact's name promises a "
              f"surface its bytes do not carry")
    if extra:
        print(f"  FAIL {label} offers {extra}, which the source parse does not "
              f"know about — the parser has fallen behind the CLI")
    return not (missing or extra)

ok = compare("the built binary", declared, built_commands([]))
n_verbs = 0
for group, verbs in sorted(groups.items()):
    ok = compare(f"`delvec {group}`", verbs, built_commands([group])) and ok
    n_verbs += len(verbs)
if not groups:
    print("  FAIL the source parse found no mounted group — the binary is "
          "expected to mount grammar, prefab, schem and render surfaces")
    ok = False
if not ok:
    raise SystemExit(1)
print(f"  ok   built binary offers exactly the {len(declared)} top-level "
      f"subcommand(s) the source declares, and its {len(groups)} mounted "
      f"group(s) exactly their {n_verbs} verb(s)")
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
# gnu / msvc / darwin. Build scripts DO run under `cargo check`, so a dependency
# whose build script needs a C toolchain for the target fails here too.
check_only() {
  local failed=0
  # ADR-0021 §1, and it is bound to BOTH entry points on purpose: a check that
  # only runs on the release path is one the standing gate never exercises.
  assert_no_feature_gated_surface || failed=$((failed + 1))
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

  case "$t" in *-linux-gnu) print_glibc_floor "$bin" ;; esac

  assert_no_feature_gated_surface

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
    assert_help_matches_source "$bin"
  else
    printf '  --   %s is a cross build; --version and the --help surface check are not executable here\n' "$t"
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
