#!/usr/bin/env bash
# ADR-0017 / ADR-0025: publish the two crates the engine is made of —
# `delvewright-dsl`, then `delvec` — to crates.io from CI, the only path there is. No human ever runs `cargo publish` for this
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
# half-succeed: the dsl uploaded, `delvec` rejected.
# Naively retried, the second run dies on "crate version already uploaded" and
# those versions are burned. This script instead asks the registry what it
# already holds:
#
#   * version absent                  -> publish it
#   * present, same crate             -> SKIP; the previous run got that far
#   * present, a DIFFERENT crate      -> HARD FAIL, by name. Something changed
#                                        under a version already served.
#                                        crates.io will never accept the new
#                                        bytes and pretending otherwise would
#                                        ship a `delvec` bound to a sibling crate
#                                        nobody can reproduce.
#
# "Same crate" is decided below, under PROVENANCE IS NOT CONTENT.
#
# PROVENANCE IS NOT CONTENT, AND THE SHA256 CANNOT TELL THEM APART
#
# The registry index publishes the sha256 of the uploaded `.crate`, and two
# packagings of one tree produce the same tarball (MEASURED on cargo 1.97.1: two
# runs at one commit, byte-identical). But `cargo package` writes
# `.cargo_vcs_info.json` INTO the tarball, carrying the sha1 of the commit it
# packaged from — so the tarball's own sha256 is a function of the COMMIT, not
# of the crate. MEASURED, varying only that: `delvewright-dsl` 0.19.0 packaged
# at two commits whose `crates/dsl`, root `Cargo.toml` and `Cargo.lock` are
# byte-identical (`git diff` empty over all three) produced sha256
# e51b1459…4df at f25dc13a and 7640e366…8ee at 46de7d60, and `diff -r` over the
# two extracted trees reports EXACTLY ONE differing file: `.cargo_vcs_info.json`.
#
# On a release tag that never mattered — a re-run packages the same commit. It
# matters completely for `delvewright-dsl`, which publishes off `main`: the next
# push after a publish packages a different commit, so a byte comparison would
# report "the same version with DIFFERENT bytes" and refuse, on every push,
# forever, for a crate nobody had touched.
#
# So the question this script decides is the one that is actually being asked:
# does the registry's tarball hold the SAME CRATE this tree packages?
#
#   * sha256 equal                          -> identical, decided and cheap
#   * sha256 differs                        -> fetch the registry's own `.crate`
#     (checking it against the index sha256 first, so a bad download can never
#     read as "same") and compare the two archives FILE BY FILE, by content
#   * every file matches but the ignored set -> the same crate; skip
#   * any other file differs, is added or is removed -> HARD FAIL by name
#
# THE IGNORED SET IS TWO NAMES, CLOSED, AND THIS IS A LOOSENING — say so plainly:
# a version whose `.cargo_vcs_info.json` or `Cargo.lock` differs is accepted here
# where a byte comparison would refuse it.
#
#   `.cargo_vcs_info.json` — the commit the upload was cut from. It is a fact
#     about a checkout, not about the crate; keeping it in the comparison is the
#     defect above.
#   `Cargo.lock` — the resolution of this crate's own dependency graph. A
#     dependent NEVER reads a library's packaged lockfile, and the DSL crate's
#     number is the `dsl_version`, which moves for a format change or a Rust-API
#     change and for nothing else. Comparing it would demand a `dsl_version` bump
#     — and a re-declaration in every campaign document — for a `cargo update`.
#
# Nothing else is ignored: `Cargo.toml` (the manifest crates.io serves, workspace
# inheritance already resolved into it), `Cargo.toml.orig`, `README.md` and every
# source, schema and data file are compared by content, and the comparison prints
# how many files it examined on each side.
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
# WHICH CRATES ONE RUN DECIDES ABOUT
#
# The full set is the RELEASE set: `versions.toml [engine]` in dependency order,
# and `tools/check-publishable.sh` must have packaged it first — the release path
# publishes only what that gate has already proven builds standing alone.
#
# `--only <crate>` narrows the run to ONE crate the manifest names, and then this
# script packages that crate itself (`cargo package -p <crate>` into the same
# `package-verify` the full gate writes to, so both paths hash the same
# artifact). That is what the DSL crate's two automatic sites need: the crate's
# version IS the `dsl_version`, so it moves on `main` rather than on a release
# tag, and neither of those sites has a whole-shelf packaging run to piggyback on.
#
# THE FULL SET TRUSTS THE GATE'S OWN PROOF, NOT JUST ITS BYTES
#
# For the full (non-`--only`) set, `local_cksum` refuses a tarball whose
# CURRENT sha256 does not match the sha256 `tools/check-publishable.sh` wrote
# beside it the moment every one of its checks passed — a hash written by one
# script and re-read by the other is what makes "the gate packaged it, the plan
# reads it" one artifact instead of two scripts that happen to agree on a path.
# `--only` skips that check: it packages the one crate it is about to read in
# the very same process, so there is no gate boundary between the two bytes to
# assert.
#
# Every mode prints one machine-readable line, `crates-io-publish: TO_PUBLISH=…`,
# holding the names it decided to upload (empty when the registry already has
# them). `.github/workflows/dsl-crate-publish.yml` reads it to decide whether to
# ask for the environment approval at all.
#
# Usage:
#   tools/crates-io-publish.sh --plan      # what WOULD happen; touches nothing
#   tools/crates-io-publish.sh --publish   # do it (needs $CARGO_REGISTRY_TOKEN)
#   tools/crates-io-publish.sh --plan --only delvewright-dsl
#   tools/crates-io-publish.sh --publish --only delvewright-dsl [--allow-dirty]
#
# `--allow-dirty` is local-only and reaches nothing but the `--only` packaging:
# CI works from a clean checkout, so the VCS-dirty refusal stays armed there.
#
# The token is read by cargo straight out of the environment. This script never
# runs `cargo login` and never writes a credential to disk.
#
# Exit 0 = success, 1 = a finding, 2 = usage/IO error.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="$ROOT/versions.toml"
[ -f "$MANIFEST" ] || { echo "FATAL: $MANIFEST not found" >&2; exit 2; }

. "$ROOT/tools/lib/checksum.sh"
. "$ROOT/tools/lib/package-verify.sh"

USAGE="usage: ${BASH_SOURCE[0]} (--plan|--publish) [--only <crate>] [--allow-dirty]"
MODE=""
ONLY=""
DIRTY_FLAG=()
while [ "$#" -gt 0 ]; do
  case "$1" in
    --plan|--publish)
      [ -z "$MODE" ] || { echo "crates-io-publish: --plan and --publish are exclusive" >&2; echo "$USAGE" >&2; exit 2; }
      MODE="$1" ;;
    --only)
      shift
      ONLY="${1:-}"
      [ -n "$ONLY" ] || { echo "crates-io-publish: --only needs a crate name" >&2; echo "$USAGE" >&2; exit 2; } ;;
    --allow-dirty) DIRTY_FLAG=(--allow-dirty) ;;
    *) echo "crates-io-publish: unknown argument '$1'" >&2; echo "$USAGE" >&2; exit 2 ;;
  esac
  shift
done
[ -n "$MODE" ] || { echo "$USAGE" >&2; exit 2; }

eval "$(python3 - "$MANIFEST" <<'PY'
import sys, tomllib
sys.stdout.reconfigure(newline="\n")  # CRLF-proof: tools/check-python-shell-newlines.py
e = tomllib.load(open(sys.argv[1], "rb"))["engine"]
for k in ("version", "crate", "dsl_crate", "dsl_crate_version"):
    print(f'{k.upper()}={e[k]!r}'.replace("'", '"'))
print('PUBLISH_CRATES=' + repr(" ".join(e["crates"])).replace("'", '"'))
PY
)"

# `DW_CRATES_INDEX` overrides the sparse-index base for a test or a local
# dry-run against a fixture index rather than the real crates.io — nothing
# under `.github/` ever sets it, and it never touches `--publish`'s upload
# path (`cargo publish` still resolves crates.io on its own). Named, and
# printed the moment it fires by the library below, so an override can never
# survive silently into a real plan.
POLL_TIMEOUT=180   # seconds
POLL_INTERVAL=5    # seconds

# THE INDEX LOOKUP IS NOT WRITTEN HERE. `tools/lib/crates_index.py` owns the
# sparse-index path scheme, the fetch, the JSON-lines scan and the bind test,
# because a second caller now asks the same question of the same registry
# (`tools/check-dsl-version-published.py`: is the number this change moves away
# from on crates.io?) and a private copy of a format reader is the shape
# `tools/lib/versions.py` and `tools/lib/checksum.sh` were each extracted after.
# Shell reaches it the way it reaches a pin — one subcommand, one line of output.
index_cksum() { # <crate-name> <version>
  python3 "$ROOT/tools/lib/crates_index.py" cksum "$1" "$2"
}

# Where tools/check-publishable.sh packages, and where `--only` packages: its
# verify target directory. One statement of the path, because two things read
# it — `tools/lib/package-verify.sh` says why it sits beside `target/`, not
# under it.
local_crate_path() { # <crate-name> <version>
  printf '%s\n' "$ROOT/package-verify/package/$1-$2.crate"
}

# The sha256 `tools/check-publishable.sh` wrote beside the tarball, right after
# proving it builds standing alone — the file that makes the pair one artifact.
local_crate_hash_path() { # <crate-name> <version>
  printf '%s\n' "$(local_crate_path "$1" "$2").sha256"
}

local_cksum() { # <crate-name> <version>
  local f
  f="$(local_crate_path "$1" "$2")"
  [ -f "$f" ] || { echo "crates-io-publish: no packaged tarball at $f — run tools/check-publishable.sh first" >&2; exit 2; }
  # `--only` packages the ONE selected crate itself, right above, in the same
  # process that is about to read it back — there is no gate boundary to cross
  # and nothing yet to check it against. The full set has no such run in front
  # of it EXCEPT `tools/check-publishable.sh`, so THAT boundary is real: the
  # bytes at `$f` must still be the ones it verified, not a stale leftover or a
  # tree tampered with between the gate and this plan. A mismatch here errors
  # rather than silently trusting whatever sits at the path now.
  if [ -z "$ONLY" ]; then
    local hash_file expected actual
    hash_file="$(local_crate_hash_path "$1" "$2")"
    [ -f "$hash_file" ] || {
      echo "crates-io-publish: no verified sha256 at $hash_file — run tools/check-publishable.sh first" >&2
      echo "  (it writes this file beside the tarball only once every check passes)" >&2
      exit 2
    }
    expected="$(cat "$hash_file")"
    actual="$(dw_sha256_file "$f")"
    if [ "$actual" != "$expected" ]; then
      echo "crates-io-publish: $f now hashes $actual, but tools/check-publishable.sh verified $expected." >&2
      echo "  That proof (the standalone build, the --help surface) was made about DIFFERENT bytes than what is on disk now. Refusing." >&2
      exit 2
    fi
    printf '%s\n' "$expected"
    return 0
  fi
  dw_sha256_file "$f"
}

# The registry's own `.crate`, fetched and CHECKED against the index sha256
# before a byte of it is believed. A download that is truncated, cached wrong or
# served from somewhere else must never be read as "the same crate".
fetch_registry_crate() { # <crate-name> <version> <expected-sha256> <dest-file>
  curl -fsSL "https://static.crates.io/crates/$1/$1-$2.crate" -o "$4"
  local got
  got="$(dw_sha256_file "$4")"
  if [ "$got" != "$3" ]; then
    echo "crates-io-publish: the .crate downloaded for $1 $2 hashes $got, but the index says $3." >&2
    echo "  Nothing below can be decided from bytes the registry does not vouch for. Refusing." >&2
    exit 1
  fi
}

# Same crate or not: every file in both archives compared by content, with the
# two provenance names ignored (see PROVENANCE IS NOT CONTENT above). Prints the
# verdict and the counts; returns 0 = same crate, 1 = a different crate.
compare_crate_contents() { # <ours.crate> <theirs.crate> <crate-name> <version>
  python3 - "$1" "$2" "$3" "$4" <<'PY'
import hashlib, posixpath, sys, tarfile
sys.stdout.reconfigure(newline="\n")  # CRLF-proof: tools/check-python-shell-newlines.py

ours, theirs, name, vers = sys.argv[1:5]
# Closed and named. Neither is content a dependent can observe; every other
# member of the archive is compared.
IGNORED = {".cargo_vcs_info.json", "Cargo.lock"}


def members(path):
    """{path-inside-the-crate: sha256} for every regular file, top dir stripped."""
    out = {}
    with tarfile.open(path, "r:gz") as tf:
        for m in tf:
            if not m.isfile():
                continue
            rel = m.name.split("/", 1)[1] if "/" in m.name else m.name
            rel = posixpath.normpath(rel)
            fh = tf.extractfile(m)
            out[rel] = hashlib.sha256(fh.read()).hexdigest() if fh else ""
    return out


a, b = members(ours), members(theirs)
print(f"    ours {len(a)} file(s), registry {len(b)} file(s); "
      f"{len(IGNORED)} name(s) ignored: {' '.join(sorted(IGNORED))}")
if not a or not b:
    print("    REFUSE  an archive with no files in it decides nothing")
    raise SystemExit(1)

diffs = []
for rel in sorted(set(a) | set(b)):
    if rel in IGNORED:
        continue
    if rel not in b:
        diffs.append(f"only in ours:     {rel}")
    elif rel not in a:
        diffs.append(f"only in registry: {rel}")
    elif a[rel] != b[rel]:
        diffs.append(f"differs:          {rel}")

compared = len(set(a) | set(b)) - len(IGNORED & (set(a) | set(b)))
if compared == 0:
    print("    REFUSE  0 files compared — the ignored set swallowed the archive")
    raise SystemExit(1)
if diffs:
    print(f"    DIFFERENT {name} {vers}: {len(diffs)} of {compared} compared file(s) do not match")
    for d in diffs[:20]:
        print(f"      {d}")
    if len(diffs) > 20:
        print(f"      ... and {len(diffs) - 20} more")
    raise SystemExit(1)
print(f"    SAME {compared} file(s) compared, all identical; only provenance differs")
PY
}

# BIND TEST — not a connectivity check.
#
# Everything below decides what to upload by asking the index whether a version
# is there. If that lookup were broken in ANY way — the host blocked, the
# sparse-index path scheme changed, the JSON shape changed, or (the bug that was
# actually written here first) a heredoc eating the fetched body — every answer
# would come back empty, every crate would look absent, and the "already
# published, skip" branch would silently never fire. That is the unbound gate
# this project keeps being bitten by (CLAUDE.md; the island's combat floor gate
# examined zero enemies for nineteen rounds), and here it would turn a safe retry
# into a permanently burned version.
#
# The test itself lives with the lookup, in `tools/lib/crates_index.py`, so the
# other caller of that lookup inherits it rather than deciding for itself whether
# to run one. Its subject cannot change: `serde 1.0.0` is on crates.io and index
# rows are never deleted. If it cannot find that checksum, nothing below is
# believed.
echo "== index lookup bind test =="
python3 "$ROOT/tools/lib/crates_index.py" bind-test || {
  echo "crates-io-publish: acting on an unbound lookup could burn a version. Refusing to plan." >&2
  exit 1
}
echo

# ------------------------------------------------------------------- the plan
# Publish order, as versions.toml states it: the format crate, then the engine
# (ADR-0025); each name's version is its own line's. bash 3.2 (macOS) has no
# `mapfile`.
NAMES=()
VERS=()
for n in $PUBLISH_CRATES; do
  NAMES+=("$n")
  if [ "$n" = "$DSL_CRATE" ]; then VERS+=("$DSL_CRATE_VERSION"); else VERS+=("$VERSION"); fi
done
DECLARED="${#NAMES[@]}"

# --only: one crate the manifest declares, and only one. A name it does not
# declare is refused HERE rather than surviving as an empty selection — a run
# that compared zero crates and exited 0 is the unbound gate the bind test above
# exists to prevent, arriving through a typo instead of through the network.
if [ -n "$ONLY" ]; then
  sel=-1
  i=0
  while [ "$i" -lt "${#NAMES[@]}" ]; do
    [ "${NAMES[$i]}" = "$ONLY" ] && sel="$i"
    i=$((i + 1))
  done
  if [ "$sel" -lt 0 ]; then
    echo "crates-io-publish: --only '$ONLY' names no crate in versions.toml [engine]." >&2
    echo "  It declares ${#NAMES[@]}: ${NAMES[*]}" >&2
    exit 2
  fi
  only_name="${NAMES[$sel]}"; only_vers="${VERS[$sel]}"
  NAMES=("$only_name"); VERS=("$only_vers")
  echo "== --only: 1 of $DECLARED declared crate(s) selected — $only_name $only_vers =="
  # The bytes this run compares against the registry. The full set arrives here
  # already packaged by `tools/check-publishable.sh`; a single selected crate has
  # no such run in front of it, so it is packaged here, into the SAME directory,
  # by the same command shape. Output is not redirected: a packaging failure is
  # the finding, and it belongs in the log of whatever invoked this.
  echo "== cargo package -p $only_name (into package-verify) =="
  (cd "$ROOT" && CARGO_TARGET_DIR="$ROOT/package-verify" \
      cargo package -p "$only_name" ${DIRTY_FLAG[@]+"${DIRTY_FLAG[@]}"})
  # Same lifetime rule as tools/check-publishable.sh's own packaging: keep the
  # tarball, drop the extracted source tree (its nested `tests/` fixtures were
  # the ENOENT class `tools/lib/package-verify.sh` documents) and everything
  # else `cargo package` leaves alongside it. This run has no sha256 to write —
  # `local_cksum` above skips the gate check for `--only`, because there is no
  # gate in front of it to check against.
  dw_prune_package_verify "$ROOT/package-verify"
  echo
fi

TO_PUBLISH=()
TO_PUBLISH_LINE=""
echo "== what crates.io already holds =="
i=0
while [ "$i" -lt "${#NAMES[@]}" ]; do
  n="${NAMES[$i]}"; v="${VERS[$i]}"
  remote="$(index_cksum "$n" "$v")"
  mine="$(local_cksum "$n" "$v")"
  if [ -z "$remote" ]; then
    printf '  PUBLISH %s %s (absent from the index; our sha256 %s)\n' "$n" "$v" "$mine"
    TO_PUBLISH+=("$n")
    TO_PUBLISH_LINE="${TO_PUBLISH_LINE:+$TO_PUBLISH_LINE }$n"
  elif [ "$remote" = "$mine" ]; then
    printf '  skip    %s %s (already published, byte-identical: %s)\n' "$n" "$v" "$mine"
  else
    # The bytes differ, which on its own says nothing: `cargo package` stamps the
    # commit into the tarball. Ask the registry for its own copy and compare the
    # two archives file by file.
    printf '  ?       %s %s is on crates.io with different BYTES; comparing CONTENTS\n' "$n" "$v"
    printf '            registry sha256 %s\n' "$remote"
    printf '            ours     sha256 %s\n' "$mine"
    theirs="$(mktemp)"
    fetch_registry_crate "$n" "$v" "$remote" "$theirs"
    same=0
    compare_crate_contents "$(local_crate_path "$n" "$v")" "$theirs" "$n" "$v" || same=$?
    rm -f "$theirs"
    if [ "$same" -eq 0 ]; then
      printf '  skip    %s %s (already published, same crate)\n' "$n" "$v"
    else
      printf '  FAIL    %s %s is on crates.io as a DIFFERENT crate\n' "$n" "$v"
      echo >&2
      echo "crates-io-publish: $n $v cannot be republished — a crates.io version is permanent." >&2
      if [ "$n" = "$DSL_CRATE" ]; then
        # The DSL crate's version IS the dsl_version, so moving it moves four
        # statements of one number that validation/check-versions.sh holds equal.
        # Nothing is re-tagged for it: .github/workflows/dsl-crate-publish.yml
        # uploads it off `main`.
        echo "  $DSL_CRATE $v must move. It is the dsl_version, so all four of these carry it:" >&2
        echo "    crates/dsl/Cargo.toml            [package] version" >&2
        echo "    crates/dsl/src/envelope.rs       SUPPORTED_DSL_VERSION" >&2
        echo "    versions.toml                    [engine] dsl_crate_version" >&2
        echo "    versions.toml                    [engine] dsl_crate_req (=<version>)" >&2
        echo "  A format change bumps the minor, a Rust-API-only change bumps the patch." >&2
        echo "  Push the bump to main and the publish hook uploads it; no tag is involved." >&2
      else
        echo "  Bump [engine] version (and the root Cargo.toml [workspace.package] + [workspace.dependencies] it binds) in versions.toml and re-tag." >&2
      fi
      exit 1
    fi
  fi
  i=$((i + 1))
done

echo
echo "crates-io-publish: ${#TO_PUBLISH[@]} of ${#NAMES[@]} crate(s) would be uploaded"
# The machine-readable verdict, one line, always printed. A caller that must
# decide whether an upload is even going to happen reads this instead of
# re-asking the index with a second copy of the lookup.
echo "crates-io-publish: TO_PUBLISH=$TO_PUBLISH_LINE"
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
if [ -n "$ONLY" ]; then
  echo "crates-io-publish: OK — $ONLY ${VERS[0]} is on crates.io ($DECLARED declared, 1 selected)"
else
  echo "crates-io-publish: OK — ${#NAMES[@]} crate(s) are on crates.io: $DSL_CRATE $DSL_CRATE_VERSION and $CRATE $VERSION"
  echo "crates-io-publish: \`cargo install $CRATE\` now resolves to $VERSION"
fi
