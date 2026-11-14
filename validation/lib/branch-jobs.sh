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
# The two per-world constants are measurements, not choices: the town's 40
# branch worlds on the remote build machine (16 CPUs, 30 GiB), sampled by
# `docker stats` every 30 s with eight worlds running — the record is the
# commit that set them. A world above them is the measurement failing, and the
# constant moves with a new one.

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
