#!/usr/bin/env bash
# ADR-0017 / ADR-0023 §6: publish every crate the engine is made of —
# `delvewright-dsl`, the engine library crates and `delvec` — to crates.io from
# CI, the only path there is. No human ever runs `cargo publish` for this
# project. The set and its order come from `versions.toml [engine]`.
#
# THE ONE-WAY DOOR
#
# A crates.io version can never be reused and a crate name can never be freed
# (`cargo yank` only stops NEW dependents selecting it; the bytes stay
# downloadable forever). So the only failure worth engineering against is a
# SUCCESSFUL WRONG publish. A failed publish costs nothing and is retried.
#
# WHY THIS IS IDEMPOTENT, AND WHY THAT IS NOT A SHORTCUT
#
# Every dependency must land before its dependent, so the sequence can
# half-succeed: the dsl and the library crates uploaded, `delvec` rejected.
# Naively retried, the second run dies on "crate version already uploaded" and
# those versions are burned. This script instead asks the registry what it
# already holds:
#
#   * version absent               -> publish it
#   * present, cksum == our bytes  -> SKIP; the previous run got that far
#   * present, cksum != our bytes  -> HARD FAIL, by name. Something changed under
#                                     a version already served. crates.io will
#                                     never accept the new bytes and pretending
#                                     otherwise would ship a `delvec` bound to a
#                                     sibling crate nobody can reproduce.
#
# The comparison is exact: the registry index publishes the sha256 of the
# uploaded `.crate`, and `cargo package` output is byte-identical run to run
# (MEASURED on cargo 1.97.1, 2026-08-06: two packagings of both crates produced
# the same two sha256s), so "same version, same bytes" is a decidable question
# rather than a guess.
#
# INDEX PROPAGATION
#
# Cargo 1.97 already waits for a just-published dependency to appear in the index
# before publishing its dependent — its own binary carries the message "due to a
# timeout while waiting for published dependencies to be available" — so nothing
# here sleeps. What this script adds is the POST-CONDITION: after cargo returns,
# every crate must be visible in the sparse index at the exact version with the
# exact checksum we uploaded. That is a poll on an observable condition with a
# stated timeout, not a sleep chosen by feel, and it turns "cargo's internal wait
# was not enough" from an unresolvable `delvec` on the registry into a red job.
#
# Usage:
#   tools/crates-io-publish.sh --plan      # what WOULD happen; touches nothing
#   tools/crates-io-publish.sh --publish   # do it (needs $CARGO_REGISTRY_TOKEN)
#
# The token is read by cargo straight out of the environment. This script never
# runs `cargo login` and never writes a credential to disk.
#
# Exit 0 = success, 1 = a finding, 2 = usage/IO error.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="$ROOT/versions.toml"
[ -f "$MANIFEST" ] || { echo "FATAL: $MANIFEST not found" >&2; exit 2; }

MODE="${1:-}"
case "$MODE" in --plan|--publish) ;; *) echo "usage: ${BASH_SOURCE[0]} (--plan|--publish)" >&2; exit 2 ;; esac

eval "$(python3 - "$MANIFEST" <<'PY'
import sys, tomllib
sys.stdout.reconfigure(newline="\n")  # CRLF-proof: tools/check-python-shell-newlines.py
e = tomllib.load(open(sys.argv[1], "rb"))["engine"]
for k in ("version", "crate", "dsl_crate", "dsl_crate_version"):
    print(f'{k.upper()}={e[k]!r}'.replace("'", '"'))
print('ENGINE_CRATES=' + repr(" ".join(e["crates"])).replace("'", '"'))
PY
)"

INDEX="https://index.crates.io"
POLL_TIMEOUT=180   # seconds
POLL_INTERVAL=5    # seconds

# crates.io sparse-index layout: 1-char `1/<n>`, 2-char `2/<n>`, 3-char
# `3/<n[0]>/<n>`, else `<n[0:2]>/<n[2:4]>/<n>`, all lowercase.
index_path() { # <crate-name>
  python3 - "$1" <<'PY'
import sys
sys.stdout.reconfigure(newline="\n")  # CRLF-proof: tools/check-python-shell-newlines.py
n = sys.argv[1].lower()
print({1: f"1/{n}", 2: f"2/{n}", 3: f"3/{n[0]}/{n}"}.get(len(n), f"{n[0:2]}/{n[2:4]}/{n}"))
PY
}

# The sha256 the registry records for a given version, or the empty string if
# that version is not in the index. 404 (crate unknown) is a normal answer here,
# so curl's failure is distinguished from "published but different".
# The index body goes to a FILE, and python is given its path. It cannot be
# piped: `python3 - <<'PY'` already binds stdin to the heredoc that carries the
# program, so a pipe into the same command is silently discarded and every
# lookup comes back empty — which reads as "not published yet" for every crate,
# forever. That exact bug was written here first and caught by the bind test
# below (`serde 1.0.229` must resolve to a checksum), which is why the test
# exists at all.
index_cksum() { # <crate-name> <version>
  local body_file rc
  body_file="$(mktemp)"
  rc=0
  curl -fsSL "$INDEX/$(index_path "$1")" -o "$body_file" 2>/dev/null || rc=$?
  if [ "$rc" -ne 0 ]; then rm -f "$body_file"; printf ''; return 0; fi
  python3 - "$body_file" "$2" <<'PY'
import json, sys
sys.stdout.reconfigure(newline="\n")  # CRLF-proof: tools/check-python-shell-newlines.py
path, want = sys.argv[1], sys.argv[2]
with open(path, encoding="utf-8") as fh:
    for line in fh:
        if not line.strip():
            continue
        row = json.loads(line)
        if row.get("vers") == want:
            print(row.get("cksum", ""))
            break
PY
  rm -f "$body_file"
}

local_cksum() { # <crate-name> <version>
  # Where tools/check-publishable.sh packages: its verify target directory.
  local f="$ROOT/target/package-verify/package/$1-$2.crate"
  [ -f "$f" ] || { echo "crates-io-publish: no packaged tarball at $f — run tools/check-publishable.sh first" >&2; exit 2; }
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$f" | cut -d' ' -f1
  else shasum -a 256 "$f" | cut -d' ' -f1; fi
}

# BIND TEST — not a connectivity check.
#
# Everything below decides what to upload by asking the index whether a version
# is there. If that lookup were broken in ANY way — curl blocked, the sparse-index
# path scheme changed, the JSON shape changed, or (the bug that was actually
# written here first) the heredoc eating the piped body — every answer would come
# back empty, every crate would look absent, and the "already published, skip"
# branch would silently never fire. That is the unbound gate this project keeps
# being bitten by (CLAUDE.md; the island's combat floor gate examined zero
# enemies for nineteen rounds), and here it would turn a safe retry into a
# permanently burned version.
#
# So the lookup is exercised against a fact that cannot change: `serde 1.0.0` is
# on crates.io and index rows are never deleted. If this cannot find its
# checksum, nothing below is believed.
echo "== index lookup bind test =="
probe="$(index_cksum serde 1.0.0)"
if [ -z "$probe" ]; then
  echo "crates-io-publish: the index lookup returned nothing for serde 1.0.0, which certainly" >&2
  echo "  exists. 'This version is absent' would therefore be an unbound answer for our own" >&2
  echo "  crates too, and acting on it could burn a version. Refusing to plan." >&2
  exit 1
fi
echo "  ok   serde 1.0.0 resolves to sha256 $probe"
echo

# ------------------------------------------------------------------- the plan
# Dependency order, as versions.toml states it: the DSL crate, the engine
# library crates, the binary last. bash 3.2 (macOS) has no `mapfile`.
NAMES=("$DSL_CRATE")
VERS=("$DSL_CRATE_VERSION")
for n in $ENGINE_CRATES; do NAMES+=("$n"); VERS+=("$VERSION"); done
NAMES+=("$CRATE"); VERS+=("$VERSION")
TO_PUBLISH=()
echo "== what crates.io already holds =="
i=0
while [ "$i" -lt "${#NAMES[@]}" ]; do
  n="${NAMES[$i]}"; v="${VERS[$i]}"
  remote="$(index_cksum "$n" "$v")"
  mine="$(local_cksum "$n" "$v")"
  if [ -z "$remote" ]; then
    printf '  PUBLISH %s %s (absent from the index; our sha256 %s)\n' "$n" "$v" "$mine"
    TO_PUBLISH+=("$n")
  elif [ "$remote" = "$mine" ]; then
    printf '  skip    %s %s (already published, byte-identical: %s)\n' "$n" "$v" "$mine"
  else
    printf '  FAIL    %s %s is on crates.io with DIFFERENT bytes\n' "$n" "$v"
    printf '            registry sha256 %s\n' "$remote"
    printf '            ours     sha256 %s\n' "$mine"
    echo >&2
    echo "crates-io-publish: $n $v cannot be republished — a crates.io version is permanent." >&2
    echo "  Bump [engine] $( [ "$n" = "$DSL_CRATE" ] && echo 'dsl_crate_version + dsl_crate_req' || echo 'version (and the root Cargo.toml [workspace.package] + [workspace.dependencies] it binds)' ) in versions.toml and re-tag." >&2
    exit 1
  fi
  i=$((i + 1))
done

echo
echo "crates-io-publish: ${#TO_PUBLISH[@]} of ${#NAMES[@]} crate(s) would be uploaded"
if [ "$MODE" = "--plan" ]; then
  echo "crates-io-publish: --plan, nothing was uploaded"
  exit 0
fi

# ---------------------------------------------------------------- the upload
if [ "${#TO_PUBLISH[@]}" -eq 0 ]; then
  echo "== nothing to upload; the registry already holds every crate, byte-identical =="
else
  : "${CARGO_REGISTRY_TOKEN:?crates-io-publish: CARGO_REGISTRY_TOKEN is not set — this job must declare the crates-io environment}"
  args=()
  for n in "${TO_PUBLISH[@]}"; do args+=(-p "$n"); done
  echo "== cargo publish ${args[*]} =="
  # One invocation: cargo orders the packages by dependency and waits for a
  # just-published dependency to become available before uploading its
  # dependent. Splitting this into two calls would move that ordering into our
  # own shell, where nothing enforces it.
  (cd "$ROOT" && cargo publish "${args[@]}")
fi

# ------------------------------------------------- the post-condition, polled
echo
echo "== post-condition: every crate visible in the index with our checksums =="
deadline=$((SECONDS + POLL_TIMEOUT))
while :; do
  ok=0
  i=0
  while [ "$i" -lt "${#NAMES[@]}" ]; do
    n="${NAMES[$i]}"; v="${VERS[$i]}"
    if [ "$(index_cksum "$n" "$v")" = "$(local_cksum "$n" "$v")" ]; then ok=$((ok + 1)); fi
    i=$((i + 1))
  done
  if [ "$ok" -eq "${#NAMES[@]}" ]; then
    echo "  ok   ${#NAMES[@]}/${#NAMES[@]} visible with matching sha256"
    break
  fi
  if [ "$SECONDS" -ge "$deadline" ]; then
    echo "  FAIL only $ok/${#NAMES[@]} visible after ${POLL_TIMEOUT}s" >&2
    echo "crates-io-publish: the upload was accepted but the index has not served it." >&2
    echo "  Re-running this job is SAFE — it will skip whatever already landed byte-identically." >&2
    exit 1
  fi
  printf '  ..   %s/%s visible; re-checking in %ss\n' "$ok" "${#NAMES[@]}" "$POLL_INTERVAL"
  sleep "$POLL_INTERVAL"
done

echo
echo "crates-io-publish: OK — ${#NAMES[@]} crate(s) are on crates.io: $DSL_CRATE $DSL_CRATE_VERSION, and $CRATE $VERSION with its $((${#NAMES[@]} - 2)) library crates"
echo "crates-io-publish: \`cargo install $CRATE\` now resolves to $VERSION"
