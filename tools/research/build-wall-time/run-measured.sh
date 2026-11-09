#!/bin/bash
# Research instrumentation (build wall time): one measured `delvec build` of a
# campaign, with the phase-timing notes on (`DELVEC_PHASE_TIMING=1`), a
# once-a-second process sample (sample-proc.sh), `/usr/bin/time -v`, and a
# flat `perf` CPU profile of the process at 97 Hz, user space only.
#
#   run-measured.sh <delvec> <content-dir> <campaign> <out-dir> <log-prefix> [perf]
#
# Writes <log-prefix>.meta, .stderr, .stdout, .samples, .time and, with `perf`,
# .perf.data. The build's exit status is the script's.
set -u
delvec=$1; content=$2; campaign=$3; out=$4; pfx=$5; perf=${6:-}
here=$(cd "$(dirname "$0")" && pwd)
{
  echo "start $(date -u +%FT%TZ)"
  echo "nproc $(nproc)"
  echo "loadavg-at-start $(cat /proc/loadavg)"
  echo "delvec $delvec $(sha256sum "$delvec" | cut -d' ' -f1)"
  echo "content $(git -C "$content" rev-parse HEAD)"
  echo "campaign $campaign"
  echo "threads ${DELVEC_THREADS:-unset}"
} > "$pfx.meta"
DELVEC_PHASE_TIMING=1 /usr/bin/time -v -o "$pfx.time" \
  "$delvec" --prefabs "$content/prefabs" build "$content/campaigns/$campaign" -o "$out" --json \
  > "$pfx.stdout" 2> "$pfx.stderr" &
tpid=$!
sleep 2
dpid=$(pgrep -P "$tpid" -x delvec | head -1)
if [ -z "$dpid" ]; then dpid=$tpid; fi
echo "pid $dpid" >> "$pfx.meta"
"$here/sample-proc.sh" "$dpid" > "$pfx.samples" &
spid=$!
ppid=
if [ "$perf" = perf ]; then
  "${PERF:-perf}" record -F 97 -e cpu-clock:u -p "$dpid" -o "$pfx.perf.data" > "$pfx.perf.log" 2>&1 &
  ppid=$!
fi
wait "$tpid"; status=$?
wait "$spid" 2>/dev/null
[ -n "$ppid" ] && wait "$ppid" 2>/dev/null
echo "end $(date -u +%FT%TZ) status $status loadavg-at-end $(cat /proc/loadavg)" >> "$pfx.meta"
exit $status
