#!/usr/bin/env bash
# playtest-server.sh — build a thing and serve it locally for the owner to walk
# (Multiplayer -> Direct Connect localhost:25565), or tear it down.
#
#   tools/playtest-server.sh up <path> [--lang LANG] [--prefabs DIR]
#                                [--delvec BIN] [--name NAME] [--out DIR]
#                                [--stage-anyway "REASON" --acknowledge-red N]
#   tools/playtest-server.sh down [--name NAME]
#   tools/playtest-server.sh status
#
# ## One command, one path, two kinds of thing
#
# This engine makes two kinds of artifact a person might want to stand inside: a
# CAMPAIGN, and a PREFAB — a building that exists before any campaign places it,
# and which for a zone past the 48-per-axis template cap ships as several `.nbt`
# tiles plus a manifest. There was a one-command path to the first and none at
# all to the second, so the answer to "can I walk it" depended on which kind the
# creator happened to be holding. It should not: they type one thing.
#
# So `up` takes whichever it is handed, and says which it read:
#
#   a directory holding `world.json`   -> a campaign. Built and gated exactly as
#                                         before, byte for byte.
#   a `.nbt`, a `.json` tile-set manifest, or a directory of `.nbt`
#                                      -> a prefab. Built into a browse world by
#                                         `delvec prefab gallery` and served.
#
# Neither shape is guessed at: a directory that is neither is refused with both
# shapes named, rather than being built as whichever branch happened to run.
#
# `up` on a CAMPAIGN builds, then runs the staging gate against that exact tree,
# and only then starts a container: no build reaches the owner while a past
# finding's general form is not a live, binding check on it
# (playtest-methodology.md rule 7). A refusal exits non-zero and prints the red
# list. `--stage-anyway "<reason>" --acknowledge-red <N>` overrides deliberately
# — it prints every class being overridden and stamps the reason into the
# build's admission token.
#
# ## What the prefab path does about the staging gate, and why that is honest
#
# It does not run it, and it says so in those words on every run rather than
# passing silently. The gate judges a CAMPAIGN against the findings ledger —
# `--campaign <dir>` is required, and every row it reads is a claim about a
# delve's content. A browse world has no campaign to judge: no quests, no NPCs,
# no player progression, and nothing that ships. Running the gate with an
# invented campaign argument, or with the rows it could not evaluate scored as
# passes, would be a green that binds to nothing — which is the exact shape
# CLAUDE.md forbids, and worse than no gate because it reads as one.
#
# What the prefab path is admitted by instead is stated, not implied, and it is a
# real check rather than a courtesy: `delvec prefab gallery` validates every
# `.mcfunction` line it emits against the pinned 1.21.11 command tree BEFORE it
# writes anything, refuses a lone tile of a tiled zone (`DW0739`), and refuses a
# manifest that does not tile its own zone. Then this script reads what the
# server says back — the pack's objectives, and one label entity per exhibit —
# so a datapack the server dropped at load cannot come up looking ready.
#
# The gate is never weakened, skipped or made optional on the campaign path to
# make this convenient. A campaign still gets the gate; a prefab was never in its
# scope.
#
# `--delvec BIN` names an engine outright. WITHOUT it, this script uses the
# `delvec` already on `PATH` when that binary IS this engine — its `--version`
# equal to `versions.toml` `[engine].version`, which is what a creator's `Init`
# puts there (ADR-0023) — and otherwise builds one from source. Either way it
# prints, in one line, which binary it chose and why, before it builds anything.
# `tools/lib/delvec-bin.sh` holds the rule and is shared with the other creator
# step that reaches for `delvec`, `validation/render-shots.sh`.
#
# Owner-facing contract: `up` ends by printing the connect address and, if the
# build ships a resource pack, the pack file name to enable. `down` removes the
# container, reclaims the staged world directory and prints what it reclaimed
# (server lifecycle rule: tear down as soon as feedback arrives). `status` lists
# the container and what the session is holding on disk.
#
# Machine-local paths (resource-pack install dir) come from the environment,
# never from this script: set DELVEWRIGHT_RESOURCEPACKS_DIR to the Minecraft
# instance's resourcepacks directory; unset, the pack is left in the build
# output and its path printed instead.
#
# The container is a throwaway itzg/minecraft-server pinned to the project MC
# version. It binds host 25565 — with `validation/owner-play.yaml`, one of the two
# sanctioned 25565 bindings; nothing else in the repo may publish that port
# (`tools/check-compose-isolation.py`, and validation ladders are project-scoped
# with no host port at all). Because 25565 is the one genuinely shared resource
# left on this host, `up` TAKES the 25565 mutex as `owner-play-session` and `down`
# releases it: while it is held, no automation may bind the port, and the release
# is refused while any container still publishes it (validation/mutex.sh).
set -euo pipefail

# shellcheck source=validation/mutex.sh
. "$(cd "$(dirname "$0")/.." && pwd)/validation/mutex.sh"
set -euo pipefail  # mutex.sh sets its own options when sourced; take ours back

MC_VERSION="1.21.11"
NAME="dw-playtest"
LANG_ARG=""
PREFABS_ARG=""
DELVEC=""
OUT_DIR=""
STAGE_ANYWAY=""
ACK_RED=""
RCON_PW="playtest"

die() { echo "playtest-server: $*" >&2; exit 1; }

cmd="${1:-}"; shift || true
case "$cmd" in up|down|status) ;; *) die "usage: up|down|status (see header)";; esac

CAMPAIGN=""
while [ $# -gt 0 ]; do
  case "$1" in
    --lang)    LANG_ARG="$2"; shift 2;;
    --prefabs) PREFABS_ARG="$2"; shift 2;;
    --delvec)  DELVEC="$2"; shift 2;;
    --name)    NAME="$2"; shift 2;;
    --out)     OUT_DIR="$2"; shift 2;;
    # The deliberate override (playtest-methodology.md rule 7). Never a bare
    # flag: it needs a real reason AND the exact current red count, which moves
    # as the ledger does, so it cannot become the way this script is run.
    --stage-anyway)    STAGE_ANYWAY="$2"; shift 2;;
    --acknowledge-red) ACK_RED="$2"; shift 2;;
    *) [ -z "$CAMPAIGN" ] && CAMPAIGN="$1" || die "unexpected arg: $1"; shift;;
  esac
done

# Checked by default (tools/lib/rcon.sh): a staging command the server
# refused must not pass for one that worked. `rcon_raw` is the unjudged form, for
# the one call whose failure is genuinely uninteresting.
# shellcheck source=tools/lib/rcon.sh
. "$(cd "$(dirname "$0")/.." && pwd)/tools/lib/rcon.sh"
DW_RCON_ARGS=(--password "$RCON_PW")
rcon() { dw_rcon "$NAME" "$1"; }
rcon_raw() { dw_rcon_probe "$NAME" "$1"; }

# ---- what a session HOLDS, and where `down` finds it -------------------------
#
# `up` holds three classes of resource, not one: the container, host 25565 (the
# mutex), and two temporary DIRECTORIES — the staged `/data` tree, which becomes a
# whole generated world, and the build output. `down` reclaimed the first two and
# nothing at all knew about the third, so every session left a world behind
# permanently: the paths came out of `mktemp -d`, so they were unguessable and
# `down` — a different shell, told only `--name` — could not have found them.
#
# A reclaimer must name EVERY class its subject holds and account for each, rather
# than the classes its author remembered. So `up` records the paths the moment it
# mints them, keyed by the one thing `down` is given, and `down` accounts for both.
#
# Not the mutex directory: that is keyed to the PORT, is shared with
# `owner-play.yaml`'s binding, and is removed on release — three reasons it is the
# wrong home for a per-session record. This is a plain file beside the directories
# it names, written before the container exists, so a session that dies during
# boot is still reclaimable by the next `down`.
case "$NAME" in
  ""|*/*|.|..) die "--name must be a plain container name, not '$NAME'";;
esac
case "$NAME" in
  *[!A-Za-z0-9_.-]*) die "--name may hold only [A-Za-z0-9_.-]: '$NAME'";;
esac
SESSION_FILE="${TMPDIR:-/tmp}/dw-playtest-session.$NAME"

# `key<TAB>path` lines, read field-wise — never sourced, because a path is data
# and `.` would execute whatever a stray edit put in there. One record per key.
session_record() {
  mkdir -p "${TMPDIR:-/tmp}"
  printf '%s\t%s\n' "$1" "$2" >>"$SESSION_FILE"
}

session_path() {
  [ -f "$SESSION_FILE" ] || return 0
  local key value
  while IFS=$'\t' read -r key value; do
    if [ "$key" = "$1" ]; then printf '%s\n' "$value"; fi
  done <"$SESSION_FILE"
}

# Size of a directory in KiB, or 0 if it is not there. `du -sk` is POSIX and reads
# the same on macOS and Linux; `du -sh` does not.
dir_kib() {
  if [ ! -d "$1" ]; then echo 0; return 0; fi
  du -sk "$1" 2>/dev/null | awk 'NR==1 {print $1+0} END {if (NR==0) print 0}'
}

human_kib() {
  awk -v k="${1:-0}" 'BEGIN {
    if (k >= 1048576) printf "%.1f GiB\n", k/1048576;
    else if (k >= 1024) printf "%.1f MiB\n", k/1024;
    else printf "%d KiB\n", k;
  }'
}

if [ "$cmd" = "status" ]; then
  # The container half must not abort the disk half. An unguarded `docker ps`
  # exits 127 where docker is absent, and `set -e` then killed the script before
  # it could say what the session is still holding on disk — which is the half
  # that matters most on a machine that is not running the session.
  docker ps --filter "name=$NAME" --format '{{.Names}}\t{{.Status}}\t{{.Ports}}' \
    || echo "docker unavailable — container state unknown" >&2
  # What the session is holding on disk, so a leak is visible without a teardown.
  if [ -f "$SESSION_FILE" ]; then
    echo "session record: $SESSION_FILE"
    for key in stage-dir build-dir; do
      path="$(session_path "$key")"
      if [ -z "$path" ]; then
        continue
      elif [ -d "$path" ]; then
        echo "  $key  $path  ($(human_kib "$(dir_kib "$path")"))"
      else
        echo "  $key  $path  (already gone)"
      fi
    done
  else
    echo "session record: none at $SESSION_FILE"
  fi
  exit 0
fi

if [ "$cmd" = "down" ]; then
  # `docker rm -f` on a name that never existed still exits 0 on modern docker —
  # that is not "I removed something", so the old one-liner printed "removed" for
  # a container that was never running. Ask first: existence is a different
  # question from removal, and `docker container inspect` is the one that
  # answers it. Where docker itself is not on PATH — this script's own test
  # fixtures run with none, and so does a host that never installed it — the
  # inspect call fails not-found (127) exactly the way `rm -f` already had to
  # tolerate, and is read the same as "no such container": the disk half below
  # must still run either way.
  if docker container inspect "$NAME" >/dev/null 2>&1; then
    docker rm -f "$NAME" >/dev/null 2>&1 && echo "$NAME removed" || echo "$NAME existed but could not be removed"
  else
    echo "$NAME was not running"
  fi
  # The staged world. This is `up`'s own copy — a server.properties, the campaign
  # datapack and whatever the server then generated on top — so nothing outside
  # the session refers to it and `down` is the moment it stops being wanted.
  STAGE_PATH="$(session_path stage-dir)"
  if [ -z "$STAGE_PATH" ]; then
    echo "no staged world recorded at $SESSION_FILE — nothing to reclaim"
  elif [ -d "$STAGE_PATH" ]; then
    STAGE_KIB="$(dir_kib "$STAGE_PATH")"
    rm -rf "$STAGE_PATH"
    echo "staged world reclaimed: $STAGE_PATH ($(human_kib "$STAGE_KIB"))"
  else
    # Already gone is not a failure: a `down` after a `down`, or after a manual
    # sweep, must still release the port and finish clean.
    echo "staged world already gone: $STAGE_PATH (0 KiB reclaimed)"
  fi
  # The build output is KEPT, and said rather than silently skipped: it holds
  # the resource pack, and its sibling `.gate` directory holds the staging-gate
  # report — which is what the session's findings are written against. The
  # report is a sibling and not a child because this gate measures the build
  # tree, and a report inside it is a file the next run counts. Both paths and
  # both sizes are printed so keeping them is a decision rather than a leak.
  BUILD_PATH="$(session_path build-dir)"
  if [ -n "$BUILD_PATH" ] && [ -d "$BUILD_PATH" ]; then
    echo "build output KEPT: $BUILD_PATH ($(human_kib "$(dir_kib "$BUILD_PATH")"))"
    echo "  it holds any resourcepack.zip; remove with: rm -rf $BUILD_PATH"
    GATE_PATH="${BUILD_PATH%/}.gate"
    if [ -d "$GATE_PATH" ]; then
      echo "staging-gate report KEPT: $GATE_PATH ($(human_kib "$(dir_kib "$GATE_PATH")"))"
      echo "  it holds staging-gate.md; remove with: rm -rf $GATE_PATH"
    fi
  fi
  rm -f "$SESSION_FILE"
  # Give 25565 back. Cross-shell by construction (`up` ran in another shell), so
  # release BY NAME — which refuses if anything still publishes the port, and
  # refuses outright if the holder is somebody else.
  dw_mutex_release_named "owner-play-session" || true
  exit 0
fi

# ---- up ---------------------------------------------------------------------
[ -n "$CAMPAIGN" ] || die "up needs a path: a campaign directory, or a prefab (a .nbt, a .json tile-set manifest, or a directory of .nbt)"
[ -e "$CAMPAIGN" ] || die "no such path: $CAMPAIGN"

# ---- which kind of thing is this? --------------------------------------------
#
# Read from the path, never asked for as a flag: a creator holding a castle
# should not have to know that this engine calls it a prefab rather than a
# campaign, and a flag they can get wrong is a flag they will get wrong.
#
# The marker for a campaign is `world.json` — the first document
# `compiler::load::load_campaign_dir` reads, and the one whose absence it already
# reports as a mistyped path. It is asked FIRST, so a campaign directory that
# also happens to hold a stray `.nbt` is still a campaign, and today's behaviour
# on every campaign is untouched.
#
# There is no third branch that guesses. A directory holding neither marker is
# refused with both shapes named, because "build it as whichever branch ran" is
# how a tool answers confidently about the wrong artifact.
SUBJECT_KIND=""
case "$CAMPAIGN" in
  *.nbt|*.json) [ -f "$CAMPAIGN" ] || die "not a file: $CAMPAIGN"; SUBJECT_KIND="prefab";;
  *)
    [ -d "$CAMPAIGN" ] || die "not a directory, and not a .nbt or .json prefab: $CAMPAIGN"
    if [ -f "$CAMPAIGN/world.json" ]; then
      SUBJECT_KIND="campaign"
    else
      # `find -maxdepth 1` rather than a glob, so a directory of 10 000 tiles
      # cannot overflow the argument list, and an unmatched glob cannot read as
      # a filename.
      FIRST_NBT="$(find "$CAMPAIGN" -maxdepth 1 -name '*.nbt' -print -quit 2>/dev/null || true)"
      [ -n "$FIRST_NBT" ] || die "$CAMPAIGN is neither a campaign nor a prefab:
  a campaign directory holds world.json (delvec build reads it first)
  a prefab is a .nbt, a .json tile-set manifest, or a directory holding .nbt
  this directory holds neither, and this script will not guess which you meant"
      SUBJECT_KIND="prefab"
    fi
    ;;
esac

# Flags that belong to one branch are REFUSED on the other rather than ignored.
# A flag silently dropped is a creator who believes it took effect — and
# `--prefabs` in particular reads as though it would point this at a prefab,
# which is the mistake this whole change exists to make impossible to make
# quietly.
if [ "$SUBJECT_KIND" = "prefab" ]; then
  [ -z "$LANG_ARG" ] || die "--lang is a campaign's translation, and $CAMPAIGN is a prefab"
  [ -z "$PREFABS_ARG" ] || die "--prefabs names the library a CAMPAIGN builds from; $CAMPAIGN is already the prefab to show — pass it as the path"
  [ -z "$STAGE_ANYWAY" ] || die "--stage-anyway overrides the staging gate, which judges a campaign; $CAMPAIGN is a prefab and the gate does not run on it (see this script's header)"
  [ -z "$ACK_RED" ] || die "--acknowledge-red belongs to --stage-anyway, which does not apply to a prefab"
fi
echo "subject: $CAMPAIGN  ->  $SUBJECT_KIND"
# Capture, then test — never `cmd | grep -q`. Under `set -o pipefail` grep exits at
# the first match, `docker ps` dies of SIGPIPE (141), and pipefail promotes that to
# the pipeline: the guard silently fails to fire *because* it matched. CI keeps this
# out of the tree (tools/check-shell-pipe-shortcircuit.py).
RUNNING="$(docker ps --format '{{.Names}}' || true)"
if [[ $'\n'"$RUNNING"$'\n' == *$'\n'"$NAME"$'\n'* ]]; then die "$NAME already running — 'down' first"; fi
BOUND_PORTS="$(docker ps --format '{{.Ports}}' || true)"
if [[ $BOUND_PORTS == *":25565->"* ]]; then die "host 25565 already bound by another container"; fi
# Claim the port before building anything: if a human is already playing, this
# session must not start at all. Nothing is torn down before the lock is ours.
dw_mutex_acquire "owner-play-session" || die "another 25565 session holds the mutex"
# Hold it only while a session actually exists: a build or boot failure must give
# the port back, or the next `up` waits on a lock nothing is behind (the failure
# mode compose-project isolation removed everywhere else).
UP_OK=0
# A failed `up` keeps its directories on purpose — `$STAGE/logs/latest.log` is
# where a boot failure is diagnosed, and `die` already sends the reader to the
# logs. What it must not do is leave them unreachable, so it names the command
# that reclaims them. The record is written before the container exists, which is
# what makes that command work after any failure past this point.
up_failed() {
  [ "$UP_OK" = 1 ] && return 0
  dw_mutex_release
  if [ -f "$SESSION_FILE" ]; then
    echo "playtest-server: this session left directories behind (see $SESSION_FILE)." >&2
    echo "  reclaim: tools/playtest-server.sh down --name $NAME" >&2
  fi
  return 0
}
trap up_failed EXIT

# A previous session under this name that never reached `down` (a crash, or a
# boot that failed) left its record here. Reclaim it before minting new paths, or
# the record is overwritten and the old world becomes unreachable — which is the
# leak this file exists to close, one session further back.
if [ -f "$SESSION_FILE" ]; then
  echo "a previous '$NAME' session left a record — reclaiming it first"
  OLD_STAGE="$(session_path stage-dir)"
  if [ -n "$OLD_STAGE" ] && [ -d "$OLD_STAGE" ]; then
    echo "  staged world reclaimed: $OLD_STAGE ($(human_kib "$(dir_kib "$OLD_STAGE")"))"
    rm -rf "$OLD_STAGE"
  fi
  OLD_BUILD="$(session_path build-dir)"
  if [ -n "$OLD_BUILD" ] && [ -d "$OLD_BUILD" ]; then
    echo "  build output KEPT: $OLD_BUILD ($(human_kib "$(dir_kib "$OLD_BUILD")"))"
  fi
  rm -f "$SESSION_FILE"
fi

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# Which delvec, and why — one line, every run. `--delvec` still names one
# outright; without it, a `delvec` on PATH at the pinned engine version IS the
# toolchain the creator's Init established and is used as it stands, and
# anything else is built from source with the reason printed. The rule, and why
# the version equality is the whole guard, live in tools/lib/delvec-bin.sh.
# shellcheck source=tools/lib/delvec-bin.sh
. "$REPO_ROOT/tools/lib/delvec-bin.sh"
DELVEC="$(dw_resolve_delvec "$DELVEC" "$REPO_ROOT" "playtest-server")" || exit 1
if [ -z "$OUT_DIR" ]; then
  OUT_DIR="$(mktemp -d "${TMPDIR:-/tmp}/dw-playtest-out.XXXXXX")"
  # Recorded the instant it exists — an unguessable path nothing else knows about
  # is exactly as lost as an unrecorded one. A caller-supplied `--out` is NOT
  # recorded: it is the caller's directory, not this session's to account for.
  session_record build-dir "$OUT_DIR"
fi

if [ "$SUBJECT_KIND" = "campaign" ]; then
  BUILD_ARGS=(build "$CAMPAIGN" --out "$OUT_DIR")
  [ -n "$LANG_ARG" ]    && BUILD_ARGS+=(--lang "$LANG_ARG")
  [ -n "$PREFABS_ARG" ] && BUILD_ARGS+=(--prefabs "$PREFABS_ARG")
  echo "delvec ${BUILD_ARGS[*]}"
  "$DELVEC" "${BUILD_ARGS[@]}" || die "build failed — fix the campaign before serving it"

  # ---- the staging gate ------------------------------------------------------
  # A build compiling is not a build she should see. Every past finding's general
  # form must be a live, binding check ON THIS TREE (playtest-methodology.md rule
  # 7). This runs BETWEEN the build and the container, so a refusal costs a
  # container that never started rather than an hour of hers.
  #
  # It is an invocation, not a doc line, on purpose: the gate shipped with nothing
  # calling it, which is the UNRUN shape — a correct gate whose obligation to run
  # lived in prose. `validation/owner-play.yaml` requires the same admission for
  # the other 25565 binder.
  #
  # The report goes BESIDE the build tree, never into it. It names every ledger
  # row, so it prints the strings the ledger's own probes search that tree for —
  # and a run after it landed counted it, added a row, and moved the red count
  # between the run that prints N and the run handed `--acknowledge-red N`. The
  # gate refuses a `--report` inside `--build` outright now; this directory is
  # where the report the refusal asks for goes, and `down` prints its path.
  GATE_DIR="${OUT_DIR%/}.gate"
  mkdir -p "$GATE_DIR"
  GATE_REPORT="$GATE_DIR/staging-gate.md"
  GATE_ARGS=(--campaign "$CAMPAIGN" --build "$OUT_DIR" --report "$GATE_REPORT")
  if [ -n "$STAGE_ANYWAY" ]; then
    [ -n "$ACK_RED" ] || die "--stage-anyway needs --acknowledge-red <N> (the gate prints N)"
    GATE_ARGS+=(--stage-anyway "$STAGE_ANYWAY" --acknowledge-red "$ACK_RED")
  fi
  echo "staging gate: $CAMPAIGN"
  echo "staging gate report: $GATE_REPORT"
  python3 "$REPO_ROOT/tools/staging-gate.py" "${GATE_ARGS[@]}" || die \
    "staging gate REFUSED this build — not serving it (full table: $GATE_REPORT)"
else
  # ---- a prefab: the browse world --------------------------------------------
  # `prefab gallery` writes the same two things `build` writes — `datapack/` and
  # `server/server.properties` — so everything below this point is shared, and
  # there is no second staging path to keep in step with the first.
  GALLERY_ARGS=(prefab gallery "$CAMPAIGN" --out "$OUT_DIR")
  echo "delvec ${GALLERY_ARGS[*]}"
  "$DELVEC" "${GALLERY_ARGS[@]}" || die "could not build a browse world from $CAMPAIGN"

  # Said out loud on every run, because a gate that does not run and is not
  # mentioned is indistinguishable from one that ran green. The reasoning is in
  # this script's header; this is the one-line form the operator sees.
  echo "staging gate: NOT RUN — it judges a campaign against the findings ledger,"
  echo "  and a browse world has no campaign to judge. This build is admitted by"
  echo "  what gallery emission already refused: every emitted line validated"
  echo "  against the pinned $MC_VERSION command tree, a lone tile of a tiled zone"
  echo "  refused (DW0739), and a manifest that does not tile its own zone refused."
  echo "  It is a building to look at, not a delve, and it ships to nobody."
fi

STAGE="$(mktemp -d "${TMPDIR:-/tmp}/dw-playtest-data.XXXXXX")"
# Recorded before anything is copied into it, and before the container that will
# generate a whole world inside it exists — so `down`, which is a different shell
# given nothing but `--name`, can reclaim it however this session ends.
session_record stage-dir "$STAGE"
mkdir -p "$STAGE/world/datapacks"
cp "$OUT_DIR/server/server.properties" "$STAGE/server.properties"
printf 'enable-rcon=true\nrcon.password=%s\nrcon.port=25575\n' "$RCON_PW" >> "$STAGE/server.properties"
# The datapack's directory name. A campaign is a directory, so its basename is
# already a name; a prefab may arrive as `the-castle.json`, whose basename would
# put a `.json` in a datapack directory name. Strip the extension and nothing
# else, so a campaign's id is byte-identical to what it always was.
CAMP_ID="$(basename "$CAMPAIGN")"
if [ "$SUBJECT_KIND" = "prefab" ]; then CAMP_ID="${CAMP_ID%.*}"; fi
cp -R "$OUT_DIR/datapack" "$STAGE/world/datapacks/$CAMP_ID"

docker run -d --name "$NAME" -p 25565:25565 \
  -e EULA=TRUE -e TYPE=VANILLA -e VERSION="$MC_VERSION" \
  -e RCON_PASSWORD="$RCON_PW" -e OVERRIDE_SERVER_PROPERTIES=false \
  -v "$STAGE:/data" itzg/minecraft-server:latest >/dev/null

echo "waiting for world generation…"
READY=0
for _ in $(seq 1 60); do
  sleep 10
  BOOT_LOG="$(docker logs "$NAME" 2>&1 || true)"
  if [[ $BOOT_LOG == *"Done ("* ]]; then READY=1; break; fi
done
[ "$READY" = 1 ] || die "server did not come up — docker logs $NAME"

# rcon verification. Every command's response is READ (CLAUDE.md) — a world that
# booted is not a world that loaded its pack, and the difference is invisible
# from the outside: a single unparseable line drops a whole function and the
# server comes up empty and cheerful.
#
# What is asked differs by kind because what each kind CLAIMS differs. Both
# assertions are of the same strength: the pack's own objectives exist, and the
# thing the pack was supposed to put in the world is in it.
OBJECTIVES="$(rcon "scoreboard objectives list")"
if [ "$SUBJECT_KIND" = "campaign" ]; then
  # campaign objectives present, at least one campaign NPC, sidebar cleared.
  [[ $OBJECTIVES == *"dw."* ]] || die "no dw.* objectives — datapack not loaded"
  NPC_PROBE="$(rcon "execute if entity @e[tag=dw_npc]")"
  [[ $NPC_PROBE == *"Test passed"* ]] || die "no dw_npc entities found"
else
  # `admit.sys` is created by `admit:load`, so its absence is a pack the server
  # dropped rather than a world that is merely slow.
  [[ $OBJECTIVES == *"admit.sys"* ]] || die "no admit.sys objective — the gallery datapack did not load (docker logs $NAME)"
  # One label per exhibit, COUNTED against the layout the same build wrote.
  # `execute if entity` answers only pass/fail, and a pass over one label of nine
  # exhibits is the vacuity this project keeps paying for — so the count is
  # summed into a scoreboard and read back, and it must EQUAL the denominator.
  EXPECTED_EXHIBITS="$(python3 -c 'import json,sys; sys.stdout.reconfigure(newline="\n"); print(len(json.load(open(sys.argv[1]))["areas"]))' "$OUT_DIR/gallery-layout.json")"
  # `admit:finish` summons the labels at tick 6 and `admit:place` runs at tick 3,
  # so a label existing proves the placement function was reached. The forceloaded
  # chunks take a few ticks to arrive, so this waits rather than asking once.
  LABELS=0
  for _ in $(seq 1 30); do
    rcon "scoreboard players set #dwlabels admit.sys 0" >/dev/null
    rcon "execute as @e[tag=admit_label] run scoreboard players add #dwlabels admit.sys 1" >/dev/null
    LABEL_READ="$(rcon "scoreboard players get #dwlabels admit.sys")"
    LABELS="$(printf '%s' "$LABEL_READ" | awk '{for(i=1;i<=NF;i++) if ($i ~ /^[0-9]+$/) {print $i; exit}}')"
    LABELS="${LABELS:-0}"
    [ "$LABELS" -ge "$EXPECTED_EXHIBITS" ] && break
    sleep 2
  done
  [ "$LABELS" = "$EXPECTED_EXHIBITS" ] || die \
    "the browse world has $LABELS labelled exhibit(s) where the layout declares $EXPECTED_EXHIBITS — the datapack did not finish placing (docker logs $NAME)"
  echo "browse world binding: $LABELS of $EXPECTED_EXHIBITS exhibit(s) placed and labelled"
fi
rcon_raw "scoreboard objectives setdisplay sidebar" >/dev/null || true

PACK_NOTE="no resource pack in this build"
if [ -f "$OUT_DIR/resourcepack.zip" ]; then
  if [ -n "${DELVEWRIGHT_RESOURCEPACKS_DIR:-}" ] && [ -d "$DELVEWRIGHT_RESOURCEPACKS_DIR" ]; then
    cp "$OUT_DIR/resourcepack.zip" "$DELVEWRIGHT_RESOURCEPACKS_DIR/$CAMP_ID.zip"
    PACK_NOTE="enable resource pack: $CAMP_ID.zip"
  else
    PACK_NOTE="resource pack at $OUT_DIR/resourcepack.zip (set DELVEWRIGHT_RESOURCEPACKS_DIR to auto-install)"
  fi
fi

UP_OK=1   # the session exists; the 25565 mutex stays held until `down`
echo
echo "READY — Multiplayer -> Direct Connect: localhost:25565"
if [ "$SUBJECT_KIND" = "prefab" ]; then
  echo "creative, flight on; you spawn on a stone platform beside the building"
fi
echo "$PACK_NOTE"
echo "teardown: tools/playtest-server.sh down --name $NAME  (also frees the 25565 mutex)"
