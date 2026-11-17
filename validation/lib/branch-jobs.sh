# shellcheck shell=bash
# How many branch worlds one Docker host runs at once (validation/branch-runs.sh).
#
# A world is one delve server and one bot. Its worst case is the server at the
# Java heap ceiling the BUILD states (`dw_server_heap_max`, the compiler's cost
# statement) plus what the JVM and the bot hold beside the heap, and it keeps
# its own CPUs busy: the server's tick thread, its GC and chunk workers, and
# the bot's node process. The fit is the smaller of
#
#   * three quarters of the daemon's memory over that worst case — the rest is
#     the daemon, the images being built, and whatever else the host runs;
#   * the daemon's CPUs over the CPUs a world holds;
#
# and never below one. Both are read from the daemon (`docker info`), which is
# the machine the worlds actually run on — a VM's, on a laptop, not the
# laptop's.
#
# The two per-world constants bound what was measured: the town's 40 branch
# worlds on the remote build machine (16 CPUs, 30.5 GiB), eight at once,
# `docker stats` every 30 s — a server at most 2.73 GiB resident under a 4G
# heap ceiling, a bot about 0.3 GiB, a walking server under one CPU and a
# placing one about three. The fit charges the heap ceiling, not the
# measured residency, so it is a floor: that host fits 4 by it and ran 8 with
# no server falling behind once its bot walked. A world above the overhead is
# the measurement failing, and the constant moves with a new one.

# MiB a world holds beside the server's heap: JVM non-heap (metaspace, code
# cache, thread stacks, direct buffers) plus the bot container.
DW_BRANCH_WORLD_OVERHEAD_MIB=1024
# CPUs a world keeps busy.
DW_BRANCH_WORLD_CPUS=2

# `4G`, `512M`, `1048576K` → MiB. Prints nothing for a size it cannot read.
dw_size_mib() {
  local v="$1" n u
  n="${v%[KkMmGg]}"
  u="${v#"$n"}"
  case "$n" in ''|*[!0-9]*) return 0 ;; esac
  case "$u" in
    G|g) echo $(( n * 1024 )) ;;
    M|m) echo "$n" ;;
    K|k) echo $(( n / 1024 )) ;;
    '') echo $(( n / 1048576 )) ;;
  esac
}

# The daemon's memory (MiB) and CPUs, as `<mib> <cpus>`; empty when the daemon
# does not answer.
dw_docker_capacity() {
  local mem cpus
  mem="$(docker info --format '{{.MemTotal}}' 2>/dev/null || true)"
  cpus="$(docker info --format '{{.NCPU}}' 2>/dev/null || true)"
  case "$mem$cpus" in ''|*[!0-9]*) return 0 ;; esac
  echo "$(( mem / 1048576 )) $cpus"
}

# The worst-case fit for the build at $1, a positive integer.
dw_branch_jobs_fit() {
  local cap mem cpus heap per by_mem by_cpu n
  cap="$(dw_docker_capacity)"
  if [ -z "$cap" ]; then
    echo 1
    return 0
  fi
  mem="${cap% *}"
  cpus="${cap#* }"
  heap="$(dw_size_mib "$(dw_server_heap_max "$1")")"
  [ -n "$heap" ] || heap=4096
  per=$(( heap + DW_BRANCH_WORLD_OVERHEAD_MIB ))
  by_mem=$(( mem * 3 / 4 / per ))
  by_cpu=$(( cpus / DW_BRANCH_WORLD_CPUS ))
  n="$by_mem"
  [ "$by_cpu" -ge "$n" ] || n="$by_cpu"
  [ "$n" -ge 1 ] || n=1
  echo "$n"
}

# Why: the numbers the fit for the build at $1 was taken from, one phrase.
dw_branch_jobs_why() {
  local cap heap
  cap="$(dw_docker_capacity)"
  if [ -z "$cap" ]; then
    echo "the Docker daemon did not state its memory and CPUs, so one world at a time"
    return 0
  fi
  heap="$(dw_server_heap_max "$1")"
  echo "daemon ${cap% *} MiB and ${cap#* } CPUs; a world is the build's heap-max ${heap} plus ${DW_BRANCH_WORLD_OVERHEAD_MIB} MiB, and ${DW_BRANCH_WORLD_CPUS} CPUs"
}

# ## A world whose server stood still because the host could not hold it
#
# A server's tick stands still when its host runs out of memory and pages it
# to swap: the town's branch worlds 30, 31, 32 and 34 were halted by the
# vanilla watchdog (`A single server tick took 60.00 seconds`) during their
# first join's placement, with 8406, 13059, 12127 and 8792 MiB of the host's
# swap in use (each crash report's own `Swap memory used`), while this ladder's
# two worlds held at most 16 GiB of its 30.5. The same host idled at 113 MiB
# of swap with two worlds walking. A stall like that says nothing about the
# path, and the ladder says so instead of reporting it as a failed world.

# MiB of the host's swap in use at which a stall is the host's: a gibibyte out
# of RAM is demand past what the host holds.
DW_HOST_SWAP_OVERLOAD_MIB=1024

# dw_host_memory_sample <container> -> `<epoch> <MemAvailable MiB> <swap used MiB>`,
# from /proc/meminfo as <container> sees it: the daemon host's, which is the
# machine the worlds share. Prints nothing when the container does not answer.
dw_host_memory_sample() {
  docker exec "$1" cat /proc/meminfo 2>/dev/null |
    awk -v now="$(date +%s)" '
      /^MemAvailable:/ { a = $2 } /^SwapTotal:/ { st = $2 } /^SwapFree:/ { sf = $2 }
      END { if (a != "") printf "%d %d %d\n", now, a / 1024, (st - sf) / 1024 }'
}

# dw_world_stall <server-log> <run-log> -> why the world's server stood still,
# one phrase, or nothing (exit 1) when it did not: the vanilla watchdog halted
# it, or the bot heard no keepalive for minecraft-protocol's 30 s.
dw_world_stall() {
  local line
  if dw_server_log_shows_watchdog "$1"; then
    line="${1#*"$DW_SERVER_WATCHDOG_MARKER"}"
    line="${line%%$'\n'*}"
    line="${line%% (*}"
    echo "the vanilla watchdog halted it ($DW_SERVER_WATCHDOG_MARKER$line)"
    return 0
  fi
  if grep -qF "connection ended: keepAliveError" "$2" 2>/dev/null; then
    echo "the bot heard no keepalive from it for 30 s (keepAliveError)"
    return 0
  fi
  return 1
}

# dw_swap_peak_mib <server-log> <samples-file> -> the most swap (MiB) the host
# was seen holding over the world's run: the sampler's readings and the crash
# report's `Swap memory used (MiB)`, whichever is larger; nothing when neither
# was read.
dw_swap_peak_mib() {
  {
    printf '%s\n' "$1" | sed -n 's/.*Swap memory used (MiB): \([0-9]*\).*/\1/p'
    [ ! -f "$2" ] || awk '{ print $3 }' "$2"
  } | sort -n | tail -n 1
}

# dw_host_overload <server-log> <run-log> <samples-file> <jobs> <build-dir>
# -> the host verdict for a world, and exit 0, when its server stood still
# (`dw_world_stall`) while the host held at least DW_HOST_SWAP_OVERLOAD_MIB of
# swap; exit 1 otherwise, printing nothing — then the stall, if any, is the
# build's or the path's, and the world's failure stands as it is.
dw_host_overload() {
  local stall peak cap mem heap per held beside
  stall="$(dw_world_stall "$1" "$2")" || return 1
  peak="$(dw_swap_peak_mib "$1" "$3")"
  [ -n "$peak" ] && [ "$peak" -ge "$DW_HOST_SWAP_OVERLOAD_MIB" ] || return 1
  cap="$(dw_docker_capacity)"
  mem="${cap% *}"
  heap="$(dw_size_mib "$(dw_server_heap_max "$5")")"
  [ -n "$heap" ] || heap=4096
  per=$(( heap + DW_BRANCH_WORLD_OVERHEAD_MIB ))
  held=$(( $4 * per ))
  if [ -n "$mem" ] && [ "$held" -lt "$mem" ]; then
    beside="this ladder's $4 world(s) hold at most $held MiB of the host's $mem MiB, so something beside this ladder held the rest"
  else
    beside="this ladder's $4 world(s) can hold $held MiB, the host's whole memory${mem:+ ($mem MiB)}: the fit admitted more than the host holds"
  fi
  echo "HOST OVERLOADED, not a verdict on the path: the server's tick stood still — $stall — while the host held $peak MiB of swap (at least $DW_HOST_SWAP_OVERLOAD_MIB is demand past its memory); $beside. The world is unproven: run it again on a host with room"
}
