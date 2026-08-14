#!/usr/bin/env bash
# Tier-1 gate: every server that boots a build output must derive its world from
# the compiler's `server/server.properties` — never from hardcoded settings.
#
# Twice now a server profile has been caught booting a DIFFERENT world than the
# delve ships (the shipped image booting an ocean campaign as a void; the PackTest
# runner hardcoding the void superflat while the delve shipped an ocean). The fix
# is one shared entrypoint, validation/world-settings-entrypoint.sh; this script
# fails CI (exit 1) if a consumer drifts from it or re-hardcodes world settings.
#
# Run from anywhere:  bash validation/check-world-settings.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$ROOT/validation/world-settings-entrypoint.sh"
DELVE_DF="$ROOT/validation/Dockerfile.delve"
COMPOSE="$ROOT/validation/compose.yaml"

fails=0
pass() { printf '  ok   %s\n' "$1"; }
fail() { printf '  FAIL %s\n' "$1"; fails=$((fails+1)); }

[ -f "$SCRIPT" ] || { echo "FATAL: $SCRIPT not found"; exit 2; }

echo "== shared world-settings entrypoint =="
# 1) The delve image bakes a byte-identical copy (its build context is the compiler
#    output tree, so it cannot COPY a repo file — it heredocs the same body).
tmp="$(mktemp)"; trap 'rm -f "$tmp"' EXIT
awk '/^RUN cat > \/delve\/entrypoint\.sh <<.EOS./{f=1;next} f&&/^EOS$/{f=0} f' \
  "$DELVE_DF" > "$tmp"
if [ ! -s "$tmp" ]; then
  fail "Dockerfile.delve: no 'RUN cat > /delve/entrypoint.sh <<EOS' heredoc found"
elif diff -u "$SCRIPT" "$tmp" > /dev/null; then
  pass "Dockerfile.delve heredoc is byte-identical to world-settings-entrypoint.sh"
else
  fail "Dockerfile.delve heredoc drifted from world-settings-entrypoint.sh:"
  diff -u "$SCRIPT" "$tmp" || true
fi

# 1b) ...and that script must actually DERIVE each campaign-varying setting from
#     the file. The drift check above only proves the two copies agree, so a line
#     deleted from both passes it silently — and the itzg base then falls back to
#     the image's own ENV, which is where `DIFFICULTY=peaceful` lives. A delve
#     that declares `world.difficulty: "hard"` (v0.6) and boots peaceful is not
#     merely mistuned: peaceful discards every hostile mob, so the whole cast of
#     threats disappears. One assertion per derived key.
#
#     The two chunk distances are here for the same reason one layer out: they
#     have no image ENV fallback at all, so a deleted line does not fall back to
#     a value we chose - it falls back to the itzg base's own
#     /image/server.properties template (view-distance) and the vanilla jar's
#     built-in default (simulation-distance), two files nobody in this repo owns.
#     What renders and what ticks would then be a property of the host.
for key in difficulty:DIFFICULTY level-seed:SEED level-type:LEVEL_TYPE \
           generator-settings:GENERATOR_SETTINGS view-distance:VIEW_DISTANCE \
           simulation-distance:SIMULATION_DISTANCE; do
  prop="${key%%:*}"; env_var="${key##*:}"
  if grep -q "prop $prop" "$SCRIPT" && grep -q "export $env_var=" "$SCRIPT"; then
    pass "entrypoint derives $env_var from the build's \`$prop\`"
  else
    fail "entrypoint must read \`$prop\` from server.properties and export $env_var - without it the image ENV default decides, and the server boots a world the campaign never declared"
  fi
done

# 2) The PackTest runner runs that same script as its entrypoint, over the build's
#    server.properties. The output tree is selectable (`DELVE_OUTPUT`, so CI can run
#    the profile a second time over a campaign whose generated templates hello-world
#    cannot carry), which is why the mount is matched in its parameterized form.
if grep -q 'entrypoint: \["/bin/bash", "/packs/world-settings-entrypoint.sh"\]' "$COMPOSE" \
   && grep -q './world-settings-entrypoint.sh:/packs/world-settings-entrypoint.sh:ro' "$COMPOSE" \
   && grep -q '${DELVE_OUTPUT:-./delve-output}/server:/packs/server:ro' "$COMPOSE" \
   && grep -q 'DELVE_SERVER_PROPERTIES: "/packs/server/server.properties"' "$COMPOSE"; then
  pass "compose packtest runner derives its world via the shared entrypoint"
else
  fail "compose packtest runner must mount \${DELVE_OUTPUT:-./delve-output}/server + the shared entrypoint script, set DELVE_SERVER_PROPERTIES, and use it as its entrypoint"
fi

# 2b) ...and every pack mount must come from the SAME tree. Selecting the output
#     directory per-run created a new way to reproduce exactly the defect this file
#     exists to catch: point `datapack` at one build and `server` at another, and the
#     runner tests a world the delve never shipped — with nothing else to notice.
#     Three mounts (datapack, packtest-datapack, server), one prefix.
prefixed=$(grep -c '\${DELVE_OUTPUT:-\./delve-output}/' "$COMPOSE" || true)
if [ "$prefixed" -eq 3 ]; then
  pass "compose packtest runner draws all three pack mounts from one build tree"
else
  fail "compose packtest runner must draw datapack, packtest-datapack and server from the SAME \${DELVE_OUTPUT:-./delve-output} tree (found $prefixed of 3) - a split tree tests a world the delve never ships"
fi

# 3) No consumer may hardcode a campaign-varying world setting: those come from the
#    build, or the server boots a world the campaign never declared. Dockerfile.delve
#    keeps ENV *fallbacks* for provenance; compose must not (it has no build to bake).
if grep -nE '^\s+(LEVEL_TYPE|GENERATOR_SETTINGS|SEED|DIFFICULTY):' "$COMPOSE"; then
  fail "compose hardcodes a campaign-varying world setting (see lines above) - derive it from the build instead"
else
  pass "compose hardcodes no campaign-varying world setting"
fi

echo
if [ "$fails" -ne 0 ]; then
  echo "check-world-settings: $fails problem(s)"; exit 1
fi
echo "check-world-settings: every server derives its world from the build"
