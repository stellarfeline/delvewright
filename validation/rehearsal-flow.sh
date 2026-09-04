#!/usr/bin/env bash
# Live shot-calibration flow (spec-0019 acceptance): stand up the compose
# `playtest` server (shipped delve image + creator overlay), drive one
# calibration pass with the mineflayer rehearsal-bot (`dw.aim`, `dw.faster`,
# `dw.mark`, then a single `dw.done`), harvest the captured server log into
# `rehearsal-report.json`, and assert the report carries the ADJUSTED values —
# not the compiled defaults — and that `delvec calibrate` turns them back into
# an `anchor + offset` DSL patch.
#
#   EULA=TRUE validation/rehearsal-flow.sh
#
# Exit 0 = the whole loop is green. Runs locally against OrbStack/Docker; like
# playtest-note-flow.sh this is a live-server test — tier-3 / local class, not
# tier-2 — because it boots a full server + a bot. Every-push coverage of the
# mechanism already exists in tier 1 (overlay emission + determinism in the
# compiler tests, `[DelveShot]` parsing in the orchestrator tests, snapping and
# the diagnostic matrix in the `delvec calibrate` tests). This job proves the
# live wiring end to end: that the triggers fire for a plain (non-op) player,
# that the macro stamp reaches the server log, and that the numbers survive the
# whole round trip.
set -euo pipefail

cd "$(dirname "$0")/.."
: "${EULA:?set EULA=TRUE to accept the Mojang EULA (https://aka.ms/MinecraftEULA)}"
# CREATOR_NAME is intentionally left UNSET (so compose's OPS is empty): the
# calibration verbs need no op — every `dw.*` trigger is enabled for everyone each
# tick — and itzg's OPS resolves the name via PlayerDB, which fails for a fake
# offline bot name. Same reasoning as playtest-note-flow.sh.
unset CREATOR_NAME

# Isolation by construction: its own compose project, unique per
# invocation, and an EPHEMERAL host port instead of 25565 — so this flow can run
# beside another ladder, or beside the owner's play session, without a lock and
# without a name either of them could collide with. Override the project with
# DW_COMPOSE_PROJECT when you want a stable one to inspect afterwards.
PROJECT="${DW_COMPOSE_PROJECT:-dw-rehearsal-$$}"
COMPOSE="docker compose -p $PROJECT -f validation/compose.yaml -f validation/ephemeral-port.yaml --profile playtest"
# …and the image tag, the one Docker-global name the paragraph above did not
# cover. This flow builds cutscene-shots and `playtest-note-flow.sh` builds
# hello-world; with the tag left at the compose default both wrote
# `delvewright/delve:local`, so running them side by side — the very thing the
# unique project is for — boots one campaign's server on the other's image.
# shellcheck source=validation/lib/delve-image.sh
. validation/lib/delve-image.sh
dw_export_delve_image "$PROJECT"
# The cutscene fixture: hello-world's world/cast with a two-shot cutscene on its
# exit beat, so the overlay has a real proposal to calibrate. hello-world itself
# stays the minimal v0.2 baseline every other tier uses.
CAMPAIGN="crates/dsl/fixtures/valid/cutscene-shots"
OUT="validation/delve-output"
LOG="validation/rehearsal.log"
REPORT="validation/rehearsal-report.json"
PATCH="validation/shot-patch.json"
BOT_OUT="validation/rehearsal-bot.out"

# Tear down ONLY this project, and prove it (never a bare `docker compose down`).
cleanup() { validation/fresh-volumes.sh --project "$PROJECT" >/dev/null 2>&1 || true; }
trap cleanup EXIT

echo "==> building the delve output (datapack + creator overlay)"
cargo run -q -p delvec --bin delvec -- \
  build "$CAMPAIGN" -o "$OUT" --prefabs campaigns/prefabs

echo "==> the compiled proposal defaults the overlay will seed"
grep -F 'dw:rehearsal base set value' \
  "$OUT/creator-datapack/data/cutscene-shots/function/creator/rehearsal/defaults.mcfunction"

echo "==> starting the playtest server (delve image + mounted creator overlay)"
$COMPOSE up -d --build

echo "==> waiting for the server to finish starting"
# No pinned container name — ask compose for this project's id.
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

echo "==> driving one calibration pass with the rehearsal-bot"
# Ask compose which host port it got — never assume a number (ephemeral-port.yaml).
MC_PORT="$($COMPOSE port playtest 25565 | sed 's/.*://')"
[ -n "$MC_PORT" ] || { echo "::error:: no host port published for the playtest server"; exit 1; }
echo "    (server reachable at 127.0.0.1:$MC_PORT)"
DELVEWRIGHT_MC_HOST=127.0.0.1 \
DELVEWRIGHT_MC_PORT="$MC_PORT" \
DELVEWRIGHT_BOT_USERNAME=delve-creator \
  node harness/src/rehearsal-bot.ts | tee "$BOT_OUT"

# Capture every match, then take the first with a parameter expansion. `| head -1`
# would SIGPIPE grep the moment the bot reported a second cell, and pipefail turns
# that into a non-zero `$(...)` — `set -e` then kills the run for succeeding.
MARKED_ALL="$(grep -oE 'MARKED_CELL=[-0-9,]+' "$BOT_OUT" || true)"
MARKED="${MARKED_ALL%%$'\n'*}"; MARKED="${MARKED#*=}"
[ -n "$MARKED" ] || { echo "::error:: the bot did not report the cell it marked"; exit 1; }
echo "==> the bot marked cell $MARKED"

# Give the server a moment to flush the last stamp to its log.
sleep 3

echo "==> capturing the server log"
docker logs "$CID" > "$LOG" 2>&1

echo "==> harvesting the log into $REPORT"
cargo run -q -p delvewright-orchestrator --bin delve-harvest -- \
  "$LOG" "$OUT/creator-datapack/layout.json" \
  -o validation/playtest-report.json --rehearsal-out "$REPORT"

echo "----- rehearsal-report.json -----"
cat "$REPORT"
echo "---------------------------------"

echo "==> asserting the report carries the ADJUSTED proposal"
assert() { grep -qF "$2" "$1" || { echo "::error:: expected $3 in $1"; exit 1; }; }

grep -qF '[DelveShot] ' "$LOG" || { echo "::error:: no [DelveShot] stamp in server log"; exit 1; }
assert "$REPORT" '"version": "0.1.0"' "report schema version"
assert "$REPORT" '"campaign_id": "cutscene-shots"' "campaign id"
assert "$REPORT" '"pointer": "/content/quests/0/on_complete/0"' "shot 1 DSL pointer"

# The whole point: shot 1 must read back as the creator LEFT it, not as compiled.
#   seconds 6 --dw.faster--> 6 - max(1, 20%) = 5
#   path 3,67,8;7,67,8 --dw.mark--> exactly one waypoint, the bot's own eye cell
python3 - "$REPORT" "$MARKED" <<'PY'
import json, sys
sys.stdout.reconfigure(newline="\n")  # CRLF-proof: tools/check-python-shell-newlines.py
report, marked = sys.argv[1], [int(v) for v in sys.argv[2].split(",")]
shots = {s["shot"]: s for s in json.load(open(report))["shots"]}
assert set(shots) == {1, 2}, f"expected both shots stamped, got {sorted(shots)}"
one = shots[1]
assert one["seconds"] == 5, f"dw.faster did not shorten shot 1: seconds={one['seconds']}"
assert one["path"] == [marked], f"dw.mark did not replace shot 1's path: {one['path']} != [{marked}]"
assert one["look_at"] is not None, "dw.aim left shot 1 without a look target"
assert one["look_at"] != [5, 67, 4], "dw.aim did not move shot 1's look target off the compiled one"
assert one["stamps"] == 1, f"expected exactly one dw.done stamp, got {one['stamps']}"
# Shot 2 was never touched, so it must still read its compiled values — proof the
# verbs address one shot and not the whole proposal.
two = shots[2]
assert two["seconds"] == 4 and two["path"] == [[5, 67, 5]] and two["look_at"] is None, \
    f"an untouched shot drifted: {two}"
print("report matches the adjusted values")
PY

echo "==> converting the harvest back into a DSL patch"
cargo run -q -p delvec --bin delvec -- \
  calibrate "$REPORT" --layout "$OUT/creator-datapack/layout.json" -o "$PATCH"

echo "----- shot-patch.json -----"
cat "$PATCH"
echo "---------------------------"

python3 - "$PATCH" "$OUT/creator-datapack/layout.json" "$MARKED" <<'PY'
import json, sys
sys.stdout.reconfigure(newline="\n")  # CRLF-proof: tools/check-python-shell-newlines.py
patch, layout, marked = sys.argv[1], sys.argv[2], [int(v) for v in sys.argv[3].split(",")]
p = json.load(open(patch))
assert not p["unsnappable"], f"every proposal must snap in a one-room fixture: {p['unsnappable']}"
anchors = {a["id"]: a["pos"] for a in json.load(open(layout))["anchors"]}
one = next(x for x in p["patches"] if x["shot"] == 1)
wp = one["patch"]["path"][0]
base = anchors[wp["anchor"]]
off = wp.get("offset", [0, 0, 0])
resolved = [base[i] + off[i] for i in range(3)]
assert resolved == marked, f"the patch does not resolve back to the marked cell: {resolved} != {marked}"
assert one["patch"]["seconds"] == 5, "seconds must survive the round trip"
print("patch resolves back to the marked cell, losslessly")
PY

echo "==> rehearsal-flow PASSED"
