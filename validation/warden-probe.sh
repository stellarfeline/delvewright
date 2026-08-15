#!/usr/bin/env bash
# Warden behavior probe (spec-0014 souls staging research, 2026-08-02).
#
# Measures what a summoned vanilla warden ACTUALLY does on the pinned 1.21.11
# server, because the blind-giant staging depends on the answer and the wiki is
# not evidence. Owner playtest reported the island's unleashed warden playing the
# dig-down animation immediately on spawn rather than after the documented 60 s
# no-vibration window; this establishes ground truth under the exact variables the
# compiler controls (`PersistenceRequired`, seeded `anger`, world difficulty).
#
# Runs against a THROWAWAY pinned server, not the `validation/` compose stack —
# the same pattern `docs/notes/td-routing-spike.md` used, so a behavioural spike
# never contends for the shared delve container or the owner's play port:
#
#   docker run -d --name dw-spike-warden --cpus=2 --memory=2g \
#     -p 127.0.0.1:25566:25565 -e EULA=TRUE -e TYPE=VANILLA -e VERSION=1.21.11 \
#     -e DIFFICULTY=easy -e LEVEL_TYPE=minecraft:flat -e SPAWN_MONSTERS=false \
#     -e ENABLE_RCON=TRUE -e RCON_PASSWORD=dwspike \
#     ghcr.io/stellarfeline/delvewright-base@sha256:<versions.toml [images.base].digest>
#
# Output: a TSV row per poll — trial, elapsed seconds, alive, anger, brain
# memories. Findings are written up in docs/notes/warden-behavior.md; this script
# is the reproduction.
set -uo pipefail

# Even though this spike runs on its OWN throwaway container and port, it still
# shares the Docker host with the owner's play session. Refuse to run at all
# while a human is playing (validation/mutex.sh).
# shellcheck source=validation/mutex.sh
. "$(dirname "$0")/mutex.sh"
dw_mutex_assert_not_owner_session || exit 1

CONTAINER="${CONTAINER:-dw-spike-warden}"
# A stone pad far from anything else, so nothing interacts with the subject.
PAD_X=200
PAD_Y=64
PAD_Z=200
POLL_SECONDS="${POLL_SECONDS:-10}"
WATCH_SECONDS="${WATCH_SECONDS:-150}"

# Two channels, and which one a command uses is a statement about that command
# (tools/lib/rcon.sh). `rcon` is for anything that MUST take effect and
# dies when the server refuses; `rcon_raw` is for the readbacks and the idempotent
# cleanups, where "No entity was found" is the answer, not a failure.
#
# This probe is why the distinction is not academic: its pad setup asked for
# `gamerule doMobSpawning false` and `gamerule randomTickSpeed 0`, both rejected
# outright by 1.21.11 (the registry is snake_case since the pin, and several rules
# were reworded), both discarded with `>/dev/null`. Every warden reading it has
# ever taken was taken in a world that was still spawning mobs and still running
# random ticks — the exact confounds the pad exists to remove.
# shellcheck source=tools/lib/rcon.sh
. "$(dirname "$0")/../tools/lib/rcon.sh"
rcon() { dw_rcon "$CONTAINER" "$1"; }
rcon_raw() { dw_rcon_probe "$CONTAINER" "$1"; }

# `data get` is the only readback that reaches rcon stdout: it answers with
# "<Entity> has the following entity data: <value>" or "No entity was found".
# (`execute if entity … run say` prints to chat, NOT to the rcon reply — that is
# what made the first run of this probe report every trial dead at t=0.)
get() { rcon_raw "data get entity @e[type=minecraft:warden,limit=1] $1"; }
alive() { case "$(get Health)" in *"following entity data"*) echo 1 ;; *) echo 0 ;; esac; }

setup_pad() {
  # 1.21.11 identifiers, measured on the pinned server (2026-08-11): the legacy
  # `doMobSpawning` / `randomTickSpeed` answer "Incorrect argument for command".
  rcon "gamerule spawn_mobs false" >/dev/null || exit 1
  rcon "gamerule random_tick_speed 0" >/dev/null || exit 1
  # Forceload BEFORE filling, and this order is load-bearing. With no player
  # online nothing around the pad is loaded, and `fill` into an unloaded chunk
  # answers "That position is not loaded" and places nothing — so every reading
  # this probe has taken was taken on a warden standing on whatever the world
  # generated, not on the stone pad the method describes. Found the first time the
  # checked channel was used here: the old `>/dev/null` made the
  # refusal invisible, which is the same defect as the rejected gamerules one line
  # above, one layer along.
  rcon "forceload add $((PAD_X - 16)) $((PAD_Z - 16)) $((PAD_X + 16)) $((PAD_Z + 16))" >/dev/null || exit 1
  rcon "fill $((PAD_X - 10)) $PAD_Y $((PAD_Z - 10)) $((PAD_X + 10)) $PAD_Y $((PAD_Z + 10)) minecraft:stone" >/dev/null || exit 1
}

# trial <name> <difficulty> <summon-nbt-or-empty>
trial() {
  local name="$1" difficulty="$2" nbt="${3:-}"
  # Idempotent cleanup: "No entity was found" is the expected answer on the first
  # trial, so this one is deliberately unjudged.
  rcon_raw "kill @e[type=minecraft:warden]" >/dev/null
  rcon "difficulty $difficulty" >/dev/null || exit 1
  local cmd="summon minecraft:warden $PAD_X $((PAD_Y + 1)) $PAD_Z"
  [ -n "$nbt" ] && cmd="$cmd $nbt"
  printf '#TRIAL\t%s\tdifficulty=%s\tnbt=%s\tsummon=%s\n' \
    "$name" "$difficulty" "${nbt:-<none>}" "$(rcon "$cmd")"

  local t=0 a
  while [ "$t" -le "$WATCH_SECONDS" ]; do
    a="$(alive)"
    if [ "$a" = 1 ]; then
      printf '%s\t%ds\talive=1\tanger=%s\tbrain=%s\n' "$name" "$t" \
        "$(get anger | sed 's/.*entity data: //')" \
        "$(get Brain | sed 's/.*entity data: //')"
    else
      printf '%s\t%ds\talive=0\tanger=-\tbrain=-\n' "$name" "$t"
      break
    fi
    sleep "$POLL_SECONDS"
    t=$((t + POLL_SECONDS))
  done
}

setup_pad
printf '== warden probe: %s ==\n' "$(rcon_raw 'version' | sed 's/Server version info://')"

# 1. Control: bare summon, easy, nobody online. Does it dig out immediately, at
#    ~60 s, or not at all?
trial bare-easy easy ""

# 2. Exactly what `unleash-actor` emits today. Does PersistenceRequired hold it?
trial persistent-easy easy '{PersistenceRequired:1b}'

# 3. Peaceful: a separate removal path from the dig-down, and a confound for any
#    campaign whose derived difficulty is peaceful.
trial bare-peaceful peaceful ""

# 4. Does a NoAI/Silent staging puppet standing 3 blocks away feed the warden's
#    vibration listener (i.e. can our own staging keep it awake)?
rcon_raw "kill @e[tag=probe_puppet]" >/dev/null
rcon "summon minecraft:zombie $((PAD_X + 3)) $((PAD_Y + 1)) $PAD_Z {NoAI:1b,Silent:1b,NoGravity:1b,PersistenceRequired:1b,Tags:[\"probe_puppet\"]}" >/dev/null || exit 1
trial puppet-neighbour easy '{PersistenceRequired:1b}'
rcon_raw "kill @e[tag=probe_puppet]" >/dev/null

rcon_raw "kill @e[type=minecraft:warden]" >/dev/null
rcon_raw "forceload remove all" >/dev/null
echo "== probe complete =="
