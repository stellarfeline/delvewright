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
# The owner sets CREATOR_NAME to her real (resolvable) MC name for hands-on playtests.
unset CREATOR_NAME

COMPOSE="docker compose -f validation/compose.yaml --profile playtest"
CAMPAIGN="crates/dsl/fixtures/valid/hello-world"
OUT="validation/delve-output"
LOG="validation/playtest.log"
REPORT="validation/playtest-report.json"
NOTE_TEXT="这个房间太暗了"   # multilingual (Chinese) fixture note

cleanup() { $COMPOSE down -v --remove-orphans >/dev/null 2>&1 || true; }
trap cleanup EXIT

echo "==> building the delve output (datapack + creator overlay)"
cargo run -q -p delvewright-compiler --bin delvec -- \
  build "$CAMPAIGN" -o "$OUT" --prefabs campaigns/prefabs

echo "==> starting the playtest server (delve image + mounted creator overlay)"
$COMPOSE up -d --build

echo "==> waiting for the server to finish starting"
# The itzg base fetches + unpacks the pinned server jar on a cold volume before the
# world loads, which can take a couple of minutes; budget generously (~7.5 min) and
# bail early if the container exits. Read the fixed container name directly (robust
# vs. compose-service log quirks), mirroring the ci.yml tier-2 boot check.
CID=delvewright-playtest
for _ in $(seq 1 90); do
  if docker logs "$CID" 2>&1 | grep -qE 'Done \([0-9]'; then break; fi
  if [ "$(docker inspect -f '{{.State.Status}}' "$CID" 2>/dev/null)" = "exited" ]; then break; fi
  sleep 5
done
if ! docker logs "$CID" 2>&1 | grep -qE 'Done \([0-9]'; then
  echo "::error:: server never finished starting"; docker logs "$CID" 2>&1 | tail -n 30; exit 1
fi

echo "==> installing harness deps (if needed)"
[ -d harness/node_modules ] || npm --prefix harness ci

echo "==> driving one note capture with the note-bot"
DELVEWRIGHT_MC_HOST=127.0.0.1 \
DELVEWRIGHT_MC_PORT=25565 \
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
