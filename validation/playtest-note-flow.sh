#!/usr/bin/env bash
# Live note-flow test (spec-0006 M2 acceptance): stand up the compose `playtest`
# server (shipped delve image + creator overlay), drive one note capture with the
# mineflayer note-bot (`/trigger dw.note` + a fixture chat line), harvest the
# captured server log into `playtest-report.json`, and assert the report contains
# the note with the correct area + quest state resolved.
#
#   EULA=TRUE validation/playtest-note-flow.sh
#
# Exit 0 = the whole flow is green. Runs locally against OrbStack/Docker; see the
# CI-placement note in validation/README.md (this is a live-server test — tier-3 /
# local class, not tier-2 — because it boots a full server + a bot).
set -euo pipefail

cd "$(dirname "$0")/.."
: "${EULA:?set EULA=TRUE to accept the Mojang EULA (https://aka.ms/MinecraftEULA)}"
# CREATOR_NAME is intentionally left UNSET here (so compose's OPS is empty): the
# note flow needs no op — `/trigger dw.note` is enabled for everyone each tick — and
# itzg's OPS resolves the name via PlayerDB, which fails for a fake offline bot name.
# The owner sets CREATOR_NAME to their real (resolvable) MC name for hands-on playtests.
unset CREATOR_NAME

# Isolation by construction (task #185): its own compose project, unique per
# invocation, and an EPHEMERAL host port instead of 25565 — so this flow can run
# beside another ladder, or beside the owner's play session, without a lock and
# without a name either of them could collide with. Override the project with
# DW_COMPOSE_PROJECT when you want a stable one to inspect afterwards.
PROJECT="${DW_COMPOSE_PROJECT:-dw-noteflow-$$}"
COMPOSE="docker compose -p $PROJECT -f validation/compose.yaml -f validation/ephemeral-port.yaml --profile playtest"
CAMPAIGN="crates/dsl/fixtures/valid/hello-world"
OUT="validation/delve-output"
LOG="validation/playtest.log"
REPORT="validation/playtest-report.json"
NOTE_TEXT="这个房间太暗了"   # multilingual (Chinese) fixture note

# Tear down ONLY this project, and prove it (never a bare `docker compose down`).
cleanup() { validation/fresh-volumes.sh --project "$PROJECT" >/dev/null 2>&1 || true; }
trap cleanup EXIT

echo "==> building the delve output (datapack + creator overlay)"
cargo run -q -p delvec --bin delvec -- \
  build "$CAMPAIGN" -o "$OUT" --prefabs campaigns/prefabs

echo "==> starting the playtest server (delve image + mounted creator overlay)"
$COMPOSE up -d --build

echo "==> waiting for the server to finish starting"
# The itzg base fetches + unpacks the pinned server jar on a cold volume before the
# world loads, which can take a couple of minutes; budget generously (~7.5 min) and
# bail early if the container exits. The container has no pinned NAME any more, so
# ask compose for its id (robust vs. compose-service log quirks, same as before).
CID="$($COMPOSE ps -q playtest)"
[ -n "$CID" ] || { echo "::error:: the playtest container did not start"; exit 1; }
STARTED=0
for _ in $(seq 1 90); do
  # Capture, then test. `docker logs | grep -q` exits at the first match and
  # SIGPIPEs `docker logs`; under pipefail that reads as NO MATCH, so a server
  # that started is reported as never started (tools/check-shell-pipe-shortcircuit.py).
  BOOT_LOG="$(docker logs "$CID" 2>&1 || true)"
  if [[ $BOOT_LOG == *"Done ("[0-9]* ]]; then STARTED=1; break; fi
  if [ "$(docker inspect -f '{{.State.Status}}' "$CID" 2>/dev/null)" = "exited" ]; then break; fi
  sleep 5
done
if [ "$STARTED" != 1 ]; then
  echo "::error:: server never finished starting"; docker logs "$CID" 2>&1 | tail -n 30; exit 1
fi

echo "==> installing harness deps (if needed)"
[ -d harness/node_modules ] || npm --prefix harness ci

echo "==> driving one note capture with the note-bot"
# Ask compose which host port it got — never assume a number (ephemeral-port.yaml).
MC_PORT="$($COMPOSE port playtest 25565 | sed 's/.*://')"
[ -n "$MC_PORT" ] || { echo "::error:: no host port published for the playtest server"; exit 1; }
echo "    (server reachable at 127.0.0.1:$MC_PORT)"
DELVEWRIGHT_MC_HOST=127.0.0.1 \
DELVEWRIGHT_MC_PORT="$MC_PORT" \
DELVEWRIGHT_BOT_USERNAME=delve-creator \
DELVEWRIGHT_NOTE_TEXT="$NOTE_TEXT" \
  node harness/src/note-bot.ts

# Give the server a moment to flush the last chat line to its log.
sleep 3

echo "==> capturing the server log"
docker logs "$CID" > "$LOG" 2>&1

echo "==> harvesting the log into $REPORT"
cargo run -q -p delvewright-orchestrator --bin delve-harvest -- \
  "$LOG" "$OUT/creator-datapack/layout.json" -o "$REPORT"

echo "----- playtest-report.json -----"
cat "$REPORT"
echo "--------------------------------"

echo "==> asserting the report contains the note"
# The report is canonical pretty JSON, so substring assertions are sufficient and
# robust. Each must appear for the flow to be green:
#   - the stamp reached the server log at all (proves say-based capture works);
#   - the harvested note carries the multilingual text verbatim, paired past the
#     offline `[Not Secure]` chat prefix;
#   - pos + area resolved in-game, and area→prefab + quest_state resolved by the
#     harvester. (The note-bot fires the mark on a fresh player, so quest_state is
#     legitimately `{}`; the non-empty grouping is exhaustively covered by the
#     orchestrator unit tests. Here we assert the field is present and correct.)
assert() { grep -qF "$2" "$1" || { echo "::error:: expected $3 in $1"; exit 1; }; }

grep -qF '[DelveNote] ' "$LOG" || { echo "::error:: no [DelveNote] stamp in server log"; exit 1; }
assert "$REPORT" "\"version\": \"0.1.0\"" "report schema version"
assert "$REPORT" "\"campaign_id\": \"hello-world\"" "campaign id"
assert "$REPORT" "$NOTE_TEXT" "the multilingual note text (pairing works)"
assert "$REPORT" "\"area\": \"area/keep\"" "resolved area"
assert "$REPORT" "\"prefab\": \"prefab/hello-room\"" "resolved prefab"
assert "$REPORT" "\"nearest_npc\": \"npc/keeper\"" "resolved nearest npc"
assert "$REPORT" "\"quest_state\"" "quest_state field present"

echo "==> note-flow PASSED"
