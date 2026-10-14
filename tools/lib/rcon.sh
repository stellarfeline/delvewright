#!/usr/bin/env bash
# The repo's ONE definition of "the server refused that command", for shell tools
# that drive a live Minecraft server. The Node half is
# `tools/lib/rcon.mjs`; the two agree on the reply shapes, which are measured on
# the pinned 1.21.11 server, not guessed.
#
# A command whose response nobody reads cannot fail. `validation/warden-probe.sh`
# set up its rig with `gamerule doMobSpawning false` and `gamerule
# randomTickSpeed 0` — both rejected outright by 1.21.11, both discarded with
# `>/dev/null` — so every warden reading it took was taken in a world with mobs
# spawning and random ticks running, and nothing said so.
#
# Usage (source, then call):
#   . "$REPO_ROOT/tools/lib/rcon.sh"
#   dw_rcon "$CONTAINER" "fill 0 64 0 4 64 4 minecraft:stone"   # dies on refusal
#   reply="$(dw_rcon_probe "$CONTAINER" "gamerule fallDamage")" # asks, judges nothing
#
# `dw_rcon` prints the reply on stdout and returns non-zero (with the reason on
# stderr) when the server refused. Under `set -e` that ends the run, which is the
# point: a rig built out of blocks that were never placed measures nothing.

# Reply shapes that mean the server did not do what was asked. Two families: a
# PARSE failure, which every Brigadier error marks with a `<--[HERE]` cursor, and
# a REFUSAL, where the command parsed and did nothing.
#
# The list is the UNION of every private copy this rule had before it moved
# here, and no two copies agreed — each was silent on exactly the refusals its
# own run never provoked. It must stay identical to `REJECTION` in rcon.mjs.
dw_rcon_rejected() {
  case "$1" in
    *"<--[HERE]"*) return 0 ;;
    "Unknown or incomplete command"*|"Incorrect argument"*|"Expected "*|"Invalid "*|"Unknown "*) return 0 ;;
    "That position is not loaded"*|"Cannot place blocks outside of the world"*) return 0 ;;
    "No blocks were filled"*|"Could not set the block"*|"No entity was found"*) return 0 ;;
    "No targets matched"*|"Malformed "*|"Failed to "*) return 0 ;;
    # `/data merge|modify entity` naming a PLAYER (measured on the pinned server).
    "Unable to modify player data"*) return 0 ;;
    # `/damage` at a target that cannot take that damage type (measured on the
    # pinned server against an invulnerable bot).
    "Target is invulnerable to the given damage type"*) return 0 ;;
  esac
  return 1
}

# The other silent answer, and it is worse than a refusal because it looks like a
# measurement. An rcon reply arrives in ONE packet and `rcon-cli` reads one: a
# reply longer than the payload the protocol allows is CUT, with no error, no
# marker and no short line — and what comes back is a shorter, entirely
# well-formed answer. A selector that matched 17 entities answers with 11 of them
# and nothing anywhere says 11 is not the population. That is CLAUDE.md's
# "a count equal to its own fetch limit is not a measurement, it is the limit",
# arriving through the one channel every live measurement in this repository uses.
#
# The cap is MEASURED, not assumed. Asking the same multi-record reply at
# increasing `limit=N` against the pinned 1.21.11 server (`execute as
# @e[type=minecraft:interaction,limit=N] run data get entity @s`, ~360 bytes a
# record) the reply grows with N to 4077 bytes at N=11 and then does not move:
# N=12, 13, 14 and 16 all answer 4097 bytes and 11 records. So the ceiling is
# 4096 payload bytes, plus the one trailing newline `tr` turns into a space.
DW_RCON_REPLY_CAP=4097

# dw_rcon_truncated <reply> — true when the reply is sitting on the ceiling and
# is therefore not an answer about anything. Deliberately `-ge` rather than `-eq`:
# a caller with its own framing must not be able to slip past by one byte.
dw_rcon_truncated() {
  [ "${#1}" -ge "$DW_RCON_REPLY_CAP" ]
}

# Extra `rcon-cli` flags a caller needs (e.g. `--password`). Set as an array
# before calling; expanded with the bash-3.2-safe idiom so `set -u` is happy when
# it is empty (macOS ships bash 3.2, where `"${empty[@]}"` is an unbound error).
DW_RCON_ARGS=()

# `docker exec -i` forwards the caller's stdin to the command, and reads it to
# the end whether or not the command wants any. Both calls below read
# /dev/null instead: a caller looping `while read …; do dw_rcon …; done <file`
# otherwise lost every line after the first to the first rcon call.

# dw_rcon <container> <command> — run it, FAIL LOUDLY if the server refused, and
# equally loudly if the answer was cut off at the packet ceiling.
dw_rcon() {
  local container="$1" cmd="$2" reply
  # `2>&1 | tr` and not `| grep`: tr consumes all of its input, so the producer
  # never takes a SIGPIPE (CLAUDE.md's readiness-probe lesson).
  reply="$(docker exec -i "$container" rcon-cli ${DW_RCON_ARGS[@]+"${DW_RCON_ARGS[@]}"} "$cmd" 2>&1 </dev/null | tr '\n' ' ')"
  if dw_rcon_rejected "$reply"; then
    echo "rcon: server rejected \`$cmd\`: $reply" >&2
    return 1
  fi
  if dw_rcon_truncated "$reply"; then
    echo "rcon: the reply to \`$cmd\` is ${#reply} bytes and the ceiling is" \
      "$DW_RCON_REPLY_CAP — it was CUT, silently, and whatever it appears to say" \
      "about how many of anything there are is the ceiling talking. Ask for less" \
      "per record, or split the query (disjoint distance bands, one type at a time)." >&2
    printf '%s' "$reply"
    return 1
  fi
  printf '%s' "$reply"
}

# dw_rcon_probe <container> <command> — the raw reply, judged by nobody. Only for
# a measurement whose subject IS the rejection (or a liveness poll that expects
# failure until the server is up).
#
# Truncation is NOT part of what this opts out of. "Judges nothing" is about the
# server's answer; a cut reply is the statement that there is no complete answer
# to judge, which no caller can want. It still prints what came back — a caller
# that genuinely wants the fragment can read stdout — and returns non-zero so the
# fragment cannot be mistaken for the population.
dw_rcon_probe() {
  local reply
  reply="$(docker exec -i "$1" rcon-cli ${DW_RCON_ARGS[@]+"${DW_RCON_ARGS[@]}"} "$2" 2>&1 </dev/null | tr '\n' ' ')"
  printf '%s' "$reply"
  if dw_rcon_truncated "$reply"; then
    echo "rcon: the reply to \`$2\` is ${#reply} bytes and the ceiling is" \
      "$DW_RCON_REPLY_CAP — it was CUT, silently. Ask for less per record, or" \
      "split the query." >&2
    return 1
  fi
}

# ---- "the world is placed": the datapack's own signal ------------------------
#
# A delve builds its world AFTER the server logs `Done (`: `setup` sets
# `#placed dw.sys` 0, the tick re-places every template, the world build latches
# it at 2 while its block writes run, and `setup_finish` sets it to 1 as its last
# line, after it has summoned the NPCs. Until then the world a probe would judge
# does not exist yet. These three values are the ones the harness reads
# (`PLACED_HOLDER`, `PLACED_OBJECTIVE`, `PLACED_DONE` in
# `harness/src/placement.ts`; `tools/tests/test_placed_reading.py` holds the two
# in lockstep), and the question is the one the harness puts: `execute if score
# … matches 1`, adjudicated by the server.
DW_PLACED_HOLDER="#placed"
DW_PLACED_OBJECTIVE="dw.sys"
DW_PLACED_DONE=1

# dw_placed_reading <container> — one reading, printed as exactly one of:
#   `placed`              the server answered that the world is placed;
#   `pending`             the server answered that it is not, yet;
#   `unanswered <reply>`  anything else: rcon not listening, the pack's `load`
#                         not yet run (`Unknown scoreboard objective`), a cut
#                         reply. Never a pass; the reply is carried so a caller
#                         whose budget runs out can say what it last heard.
dw_placed_reading() {
  local reply
  if ! reply="$(dw_rcon_probe "$1" "execute if score $DW_PLACED_HOLDER $DW_PLACED_OBJECTIVE matches $DW_PLACED_DONE" 2>/dev/null)"; then
    printf 'unanswered %s\n' "$reply"
    return 0
  fi
  # The WHOLE reply, never a prefix: vanilla answers every rcon client through
  # one shared output buffer, so a reply read while another client is talking
  # can carry that client's text (measured on the pinned server: a concurrent
  # `scoreboard players get` came back as `Test failed#placed has 2 [dw.sys]`).
  # A reply that is not exactly one of the two answers is not an answer.
  reply="${reply%"${reply##*[![:space:]]}"}"
  case "$reply" in
    "Test passed") echo "placed" ;;
    "Test failed") echo "pending" ;;
    *) printf 'unanswered %s\n' "$reply" ;;
  esac
}
