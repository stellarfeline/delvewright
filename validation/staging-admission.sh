#!/usr/bin/env bash
# The staging-admission verifier: refuse to serve a build tree the staging gate
# has not admitted.
#
#   validation/staging-admission.sh <build-tree>
#
# Exit 0 = this exact tree carries a valid admission token. Exit 1 = it does
# not, and nothing owner-facing may boot on it.
#
# ## Why this exists as a separate, tiny thing
#
# `tools/staging-gate.py` answers the coverage question. It was, on arrival,
# invoked by nothing but its own tests — the UNRUN shape: a correct gate that
# failed in the right direction and that nothing called, so the obligation to
# run it lived in a doc line. This project has shipped that shape five times.
#
# A doc line is not an invocation. The two owner-facing paths therefore do not
# *ask* for the gate, they REQUIRE its output:
#
#   * `tools/playtest-server.sh` runs the gate itself, between the build and the
#     container, and dies on a refusal.
#   * `validation/owner-play.yaml` — the ONE compose file that publishes host
#     25565, so reaching the owner's client means naming it — runs THIS script
#     as a `staging-admission` service that `server` and `playtest` both
#     `depends_on: service_completed_successfully`. Compose will not start them
#     if it exits non-zero.
#
# ## Why the token binds a fingerprint
#
# A token that merely said "the gate ran" would be defeated by the obvious
# move: run the gate green on one tree, serve another. So the gate stamps the
# sha256 of the tree's `manifest.json` — the compiler's reproducibility index
# over the WHOLE output tree — and this script recomputes it. A rebuilt, edited
# or substituted tree cannot present an older token. The gate also DELETES any
# existing token before it adjudicates, so a tree that was green once and has
# since gone red carries nothing.
#
# ## Parsing
#
# The token is written by `write_admission()` with `json.dumps(indent=2,
# sort_keys=True)`: one key per line, stable order, no embedded newlines in the
# fields read here. That is a format this repo controls end to end, and
# `tools/tests/test_staging_gate.py` asserts this script against real gate
# output so a format change reds rather than silently admitting.
#
# Deliberately dependency-free (bash + coreutils): it runs inside the delve
# image, which carries no Python and must never gain tooling (ADR-0003).
set -euo pipefail

die() { echo "staging-admission: $*" >&2; exit 1; }

BUILD="${1:-}"
[ -n "$BUILD" ] || die "usage: staging-admission.sh <build-tree>"
[ -d "$BUILD" ] || die "no such build tree: $BUILD"

TOKEN="$BUILD/staging-admission.json"
MANIFEST="$BUILD/manifest.json"

if [ ! -f "$TOKEN" ]; then
  die "no admission token at $TOKEN

This build has not been through the staging gate, or the gate REFUSED it (a
refusal deletes the token). Nothing owner-facing serves an unadmitted build.

  python3 tools/staging-gate.py --campaign <campaign-dir> --build $BUILD

If it reds, that red list is the set of defect classes the owner has already
reported once and that nothing on this build would catch a second time. Fix
them, or override deliberately — the gate prints the exact incantation."
fi

[ -f "$MANIFEST" ] || die "build tree has no manifest.json — no identity to verify"

# Capture, then test. Never `cmd | grep -q` under `set -o pipefail`: grep exits
# at the first match, the producer dies of SIGPIPE (141), and pipefail promotes
# that to the pipeline — the guard fails BECAUSE it matched. CI keeps this
# idiom out of the tree (tools/check-shell-pipe-shortcircuit.py).
SUMS="$(sha256sum "$MANIFEST")"
ACTUAL="${SUMS%% *}"

TOKEN_TEXT="$(cat "$TOKEN")"
RECORDED="$(printf '%s\n' "$TOKEN_TEXT" | sed -n 's/.*"build_fingerprint": "\([0-9a-f]*\)".*/\1/p')"
[ -n "$RECORDED" ] || die "token at $TOKEN records no build_fingerprint (corrupt or hand-written)"

if [ "$RECORDED" != "$ACTUAL" ]; then
  die "admission token is for a DIFFERENT build tree.

  token says:  $RECORDED
  this tree:   $ACTUAL

The build changed after it was admitted (a rebuild, an edited datapack, a
different campaign). Re-run the staging gate against the tree you intend to
serve — an admission is a statement about one exact build, never a receipt."
fi

# `sed -E` throughout: BASIC sed has no `\|` alternation, so a `true\|false`
# pattern silently matches nothing and an OVERRIDDEN build reports as a clean
# one — which is the whole banner failing open. Caught by driving a real
# overridden token through this script rather than by reading it.
CAMPAIGN="$(printf '%s\n' "$TOKEN_TEXT" | sed -nE 's/.*"campaign": "([^"]*)".*/\1/p')"
OVERRIDDEN="$(printf '%s\n' "$TOKEN_TEXT" | sed -nE 's/.*"overridden": (true|false).*/\1/p')"
[ -n "$OVERRIDDEN" ] || die "token at $TOKEN records no \`overridden\` flag (corrupt or hand-written)"

if [ "$OVERRIDDEN" = "true" ]; then
  # Trailing commas are part of the format; never anchor on a closing quote.
  REASON="$(printf '%s\n' "$TOKEN_TEXT" | sed -nE 's/.*"reason": "(.*)",?$/\1/p' | sed -E 's/",$//; s/"$//')"
  ACK="$(printf '%s\n' "$TOKEN_TEXT" | sed -nE 's/.*"red_count": ([0-9]+).*/\1/p')"
  REDS="$(printf '%s\n' "$TOKEN_TEXT" | sed -nE 's/.*"verdict": "([A-Z-]+)".*/\1/p' | sort -u | tr '\n' ' ')"
  echo "========================================================================" >&2
  echo "SERVING A BUILD ADMITTED UNDER OVERRIDE — campaign: $CAMPAIGN" >&2
  echo "  uncovered finding classes: ${ACK:-?}" >&2
  echo "  verdicts overridden:       ${REDS:-?}" >&2
  echo "  reason recorded:           ${REASON:-<none>}" >&2
  echo "" >&2
  echo "Anything the owner hits from those classes in this session is this" >&2
  echo "override, not a new finding. Say so in the round summary." >&2
  echo "========================================================================" >&2
  exit 0
fi

echo "staging-admission: $CAMPAIGN admitted (build $ACTUAL)" >&2
exit 0
