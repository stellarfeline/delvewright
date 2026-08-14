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
  esac
  return 1
}

# Extra `rcon-cli` flags a caller needs (e.g. `--password`). Set as an array
# before calling; expanded with the bash-3.2-safe idiom so `set -u` is happy when
# it is empty (macOS ships bash 3.2, where `"${empty[@]}"` is an unbound error).
DW_RCON_ARGS=()

# dw_rcon <container> <command> — run it, FAIL LOUDLY if the server refused.
dw_rcon() {
  local container="$1" cmd="$2" reply
  # `2>&1 | tr` and not `| grep`: tr consumes all of its input, so the producer
  # never takes a SIGPIPE (CLAUDE.md's readiness-probe lesson).
  reply="$(docker exec -i "$container" rcon-cli ${DW_RCON_ARGS[@]+"${DW_RCON_ARGS[@]}"} "$cmd" 2>&1 | tr '\n' ' ')"
  if dw_rcon_rejected "$reply"; then
    echo "rcon: server rejected \`$cmd\`: $reply" >&2
    return 1
  fi
  printf '%s' "$reply"
}

# dw_rcon_probe <container> <command> — the raw reply, judged by nobody. Only for
# a measurement whose subject IS the rejection (or a liveness poll that expects
# failure until the server is up).
dw_rcon_probe() {
  docker exec -i "$1" rcon-cli ${DW_RCON_ARGS[@]+"${DW_RCON_ARGS[@]}"} "$2" 2>&1 | tr '\n' ' '
}
