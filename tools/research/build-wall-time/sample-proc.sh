#!/bin/bash
# Research instrumentation (build wall time): sample one process once a second
# until it exits. Columns: epoch seconds, process CPU seconds (user+system,
# from /proc/<pid>/stat at 100 Hz ticks), thread count, resident KiB, bytes
# read and written through the filesystem (/proc/<pid>/io, cumulative), and
# the host's 1-minute load average.
set -u
pid=$1
echo "epoch cpu_s threads rss_kib read_bytes write_bytes load1"
while kill -0 "$pid" 2>/dev/null; do
  stat=$(cat /proc/"$pid"/stat 2>/dev/null) || break
  rest=${stat##*) }
  set -- $rest
  utime=${12}; stime=${13}; threads=${18}; rss_pages=${22}
  cpu=$(awk -v u="$utime" -v s="$stime" 'BEGIN{printf "%.2f", (u+s)/100}')
  rss=$(( rss_pages * 4 ))
  rb=$(awk '/^read_bytes/{print $2}' /proc/"$pid"/io 2>/dev/null || echo 0)
  wb=$(awk '/^write_bytes/{print $2}' /proc/"$pid"/io 2>/dev/null || echo 0)
  load=$(cut -d' ' -f1 /proc/loadavg)
  echo "$(date +%s.%N) $cpu $threads $rss $rb $wb $load"
  sleep 1
done
