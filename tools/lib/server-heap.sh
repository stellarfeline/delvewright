#!/usr/bin/env bash
# The repo's ONE rule for a Minecraft server's heap, for shell tools that start
# or watch one.
#
#   . "$REPO_ROOT/tools/lib/server-heap.sh"
#   docker run ... -e "$(dw_server_heap_env "$override")" ... <bare server image>
#   dw_server_log_shows_oom "$log_text"       # true when the JVM ran out of heap
#   dw_server_log_file_shows_oom "$log_file"  # the same question of a file
#
# The ceiling is `versions.toml` `[server].heap_max`. A server booted from the
# delve image, or by the PackTest runner, gets it from
# `validation/world-settings-entrypoint.sh` (which cannot source this file: it is
# baked into the shipped image); `tools/tests/test_server_heap.py` holds that
# entrypoint's default equal to the pin and fails any server entry point that
# takes neither.

DW_SERVER_HEAP_REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

# dw_server_heap_max -> the pinned heap ceiling (e.g. `4G`).
dw_server_heap_max() {
  python3 "$DW_SERVER_HEAP_REPO_ROOT/tools/lib/versions.py" server.heap_max
}

# dw_server_heap_env [override] -> the one `-e` value for itzg's heap.
# With an override (an operator's `--memory`), `MEMORY=<override>`: initial and
# ceiling both, exactly as `-e MEMORY=...` on the delve image. Without one,
# `MAX_MEMORY=<pin>`: the ceiling, leaving the initial heap at itzg's own 1G —
# the same default the delve entrypoint applies.
dw_server_heap_env() {
  if [ -n "${1:-}" ]; then
    printf 'MEMORY=%s\n' "$1"
  else
    local max
    max="$(dw_server_heap_max)" || return 1
    printf 'MAX_MEMORY=%s\n' "$max"
  fi
}

# The literal the JVM prints when a heap allocation fails. Read from the log the
# failure happened in: an OOM loading structures leaves templates unplaced and
# NPCs unspawned, and every downstream symptom (a missing NPC, a bot that never
# spawns, a PackTest run that never finishes) is true but names the wrong defect.
DW_SERVER_OOM_MARKER='java.lang.OutOfMemoryError'

# dw_server_log_shows_oom <text> — true when <text> holds the JVM's OOM line.
dw_server_log_shows_oom() {
  case "${1-}" in
    *"$DW_SERVER_OOM_MARKER"*) return 0 ;;
    *) return 1 ;;
  esac
}

# dw_server_log_file_shows_oom <file> — the same question of a file, without
# reading a growing log into a shell variable on every poll. A missing file is
# not an OOM.
dw_server_log_file_shows_oom() {
  [ -f "${1-}" ] && grep -qF -- "$DW_SERVER_OOM_MARKER" "$1"
}

# dw_server_oom_advice — what to do about it, said once for every caller.
dw_server_oom_advice() {
  local max
  max="$(dw_server_heap_max 2>/dev/null || echo '?')"
  printf '%s\n' "the server ran out of Java heap ($DW_SERVER_OOM_MARKER). The default ceiling is versions.toml [server].heap_max = $max; a delve that needs more is run with -e MEMORY=<size> (tools/creator/playtest-server.sh: --memory <size>), and a default too small for a campaign the engine builds is raised in versions.toml. This is an infrastructure failure, not a verdict on the delve."
}

# dw_server_watch <pid> <log-file> <timeout-seconds> [poll-seconds]
# Wait for the process <pid> (a server run whose output lands in <log-file>) to
# end, and stop waiting early for the two failures that otherwise hang a ladder:
#   0  — <pid> ended by itself; its exit status is the caller's to `wait` for;
#   10 — the log showed the JVM's OOM line first (`dw_server_log_file_shows_oom`);
#   11 — <timeout-seconds> passed with <pid> still running.
# An OOM'd server can keep running with its structures unloaded and nothing left
# to say, so waiting for it to exit is waiting forever. The caller stops the run
# on 10 or 11; this function stops nothing.
dw_server_watch() {
  local pid="$1" log="$2" timeout="$3" poll="${4:-2}" start now
  start="$(date +%s)"
  while kill -0 "$pid" 2>/dev/null; do
    if dw_server_log_file_shows_oom "$log"; then return 10; fi
    now="$(date +%s)"
    if [ $((now - start)) -ge "$timeout" ]; then return 11; fi
    sleep "$poll"
  done
  return 0
}
