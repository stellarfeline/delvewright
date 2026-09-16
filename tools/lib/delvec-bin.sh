# Resolving the `delvec` a SHELL step runs — the creator's, or a stated substitute.
#
# ## The defect this exists to end
#
# A creator's `Init` establishes the toolchain: it downloads the release archive
# for the pinned engine version, verifies its checksum, and puts `delvec` on
# `PATH` (ADR-0023 — the release archive is the route, a source build is the
# floor). Later steps then reached for a `delvec` of their own:
#
# - `tools/playtest-server.sh` defaulted to `$REPO_ROOT/target/release/delvec`
#   and, finding nothing there, compiled the whole workspace — 1 min 33 s on the
#   third end-to-end drill — while the binary Init had already verified sat on
#   `PATH` unused.
# - `validation/render-shots.sh` did the opposite and took `PATH` FIRST, with no
#   check at all: a `delvec` of any version, from any release, silently produced
#   the scene set a delve is judged on.
#
# The two failures look opposite and are the same one: a step used an instrument
# nobody chose, and said nothing.
#
# ## The rule, and it is the whole rule
#
# **A step uses the instrument the creator established, or it says out loud that
# it is using a different one.** When no `--delvec` is given:
#
# 1. a `delvec` on `PATH` is used **when it is this engine** — its `--version`
#    equal to `versions.toml` `[engine].version`;
# 2. otherwise the tree's own `target/release/delvec` is used when IT answers the
#    pinned version, and built from source when it does not exist or does not;
# 3. and one line names which was chosen and why, every run, on every path.
#
# The version equality is the guard, not a nicety. 93 seconds of compiling is a
# cost; a `PATH` binary of a DIFFERENT engine building a delve is a wrong
# artifact that every downstream check then passes over, so the cheap outcome is
# only reachable through the check that makes it safe.
#
# The pin is read from `versions.toml` through `tools/lib/versions.py` — no
# script here restates a version number.
#
# ## What this deliberately does not reach
#
# Engine-side tools whose whole verdict is a property of THIS tree's compiler —
# `validation/packtest-all.sh`, the `cargo run` rehearsal flows, and the five
# Python gates behind `tools/lib/delvec_bin.py` — must NOT prefer a released
# binary on `PATH`: for them a `PATH` binary of the same version is still the
# wrong instrument, because it was built from a different tree. Their rule is the
# stricter one already written in `tools/lib/delvec_bin.py`, and it stays.
#
# Usage (both callers):
#   . "$REPO_ROOT/tools/lib/delvec-bin.sh"
#   DELVEC="$(dw_resolve_delvec "$EXPLICIT" "$REPO_ROOT" "<caller>")" || exit 1
#
# `$EXPLICIT` may be empty. The chosen path is printed on stdout; the one line
# naming it goes to stderr, so a caller may capture the path without capturing
# the explanation.

# The version number a `delvec` answers with, or the empty string when it cannot
# be run or does not answer in the documented shape. `delvec --version` prints
# `delvec <engine>, dsl <dsl>, mc <mc>` (crates/delvec/src/main.rs).
dw_delvec_version() { # <binary>
  local line
  line="$("$1" --version 2>/dev/null)" || return 0
  case "$line" in
    delvec\ *) ;;
    *) return 0;;
  esac
  printf '%s\n' "$line" | awk '{ sub(/,$/, "", $2); print $2 }'
}

# `versions.toml [engine].version`, read through the one reader. A registry this
# cannot read is fatal to the caller: choosing an engine without knowing which
# one is pinned is the state this file exists to remove.
dw_engine_pin() { # <repo-root>
  python3 "$1/tools/lib/versions.py" engine.version
}

dw_resolve_delvec() { # <explicit-or-empty> <repo-root> <caller>
  local explicit="$1" repo="$2" caller="$3"
  local pin path_bin path_ver built built_ver why

  if [ -n "$explicit" ]; then
    if [ ! -x "$explicit" ]; then
      echo "$caller: --delvec \`$explicit\` is not an executable file" >&2
      return 1
    fi
    echo "$caller: delvec — $explicit (named by --delvec, version $(dw_delvec_version "$explicit" || true))" >&2
    printf '%s\n' "$explicit"
    return 0
  fi

  if ! pin="$(dw_engine_pin "$repo")"; then
    echo "$caller: cannot read \`[engine].version\` from $repo/versions.toml — refusing to pick an engine without knowing which one this tree is" >&2
    return 1
  fi

  path_bin="$(command -v delvec 2>/dev/null || true)"
  if [ -n "$path_bin" ]; then
    path_ver="$(dw_delvec_version "$path_bin")"
    if [ "$path_ver" = "$pin" ]; then
      echo "$caller: delvec — $path_bin (on PATH, version $path_ver, which is the version this tree pins): the toolchain Init established, used as it stands" >&2
      printf '%s\n' "$path_bin"
      return 0
    fi
    why="the delvec on PATH ($path_bin) answers version ${path_ver:-<no version line>} and this tree pins $pin"
  else
    why="no delvec on PATH"
  fi

  built="$repo/target/release/delvec"
  if [ -x "$built" ]; then
    built_ver="$(dw_delvec_version "$built")"
    if [ "$built_ver" = "$pin" ]; then
      echo "$caller: delvec — $built (built from this tree, version $built_ver): $why" >&2
      printf '%s\n' "$built"
      return 0
    fi
    why="$why; $built answers ${built_ver:-<no version line>}"
  fi

  echo "$caller: delvec — building $built from source (cargo build --release -p delvec): $why" >&2
  if ! ( cd "$repo" && cargo build --release -p delvec --bin delvec >/dev/null ); then
    echo "$caller: the source build failed — there is no delvec to run" >&2
    return 1
  fi
  printf '%s\n' "$built"
}
