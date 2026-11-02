#!/usr/bin/env bash
# rss-sample.sh <out-prefix> -- <command...>
#
# The /proc sampler ADR-0037's measurement tables name: runs <command> and,
# once a second while it lives, reads the process's own resident set (VmRSS,
# /proc/<pid>/status) into <out-prefix>.rss as "<epoch> <rss_kib>" and its I/O
# counters (/proc/<pid>/io: rchar wchar read_bytes write_bytes) into
# <out-prefix>.io as "<epoch> rchar wchar read_bytes write_bytes" (the last
# sample stands for the run); the command's stderr goes to <out-prefix>.err
# with each line prefixed by the epoch second it arrived, so a stage line can
# be set beside the resident set at that second. Exits with the command's
# status, written as the last line of .err.
#
# It is one of two instruments: GNU `time -v` wrapping it gives the kernel's
# own maximum resident set (wait4) and the CPU time, so the two peaks are
# compared and the one that disagrees is the instrument failing. Linux only
# (/proc); the sampling shares no configuration with the process it reads.
#
#   /usr/bin/time -v -o run.time tools/maintenance/rss-sample.sh run -- \
#       delvec --prefabs <prefabs> build <campaign> -o <out>
#   peak_kib=$(sort -k2 -n run.rss | tail -1 | cut -d' ' -f2)
set -u
OUT=$1; shift; [ "$1" = "--" ] && shift
"$@" 2> >(while IFS= read -r line; do printf '%s %s\n' "$(date +%s)" "$line"; done > "$OUT.err") &
PID=$!
: > "$OUT.rss"; : > "$OUT.io"
while kill -0 "$PID" 2>/dev/null; do
  now=$(date +%s)
  rss=$(awk '/VmRSS/{print $2}' /proc/$PID/status 2>/dev/null)
  [ -n "${rss:-}" ] && printf '%s %s\n' "$now" "$rss" >> "$OUT.rss"
  io=$(awk '/^rchar|^wchar|^read_bytes|^write_bytes/{printf "%s ", $2}' /proc/$PID/io 2>/dev/null)
  [ -n "${io:-}" ] && printf '%s %s\n' "$now" "$io" >> "$OUT.io"
  sleep 1
done
wait "$PID"; st=$?
echo "exit=$st" >> "$OUT.err"
exit $st
