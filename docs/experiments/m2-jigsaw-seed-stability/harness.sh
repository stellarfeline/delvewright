#!/usr/bin/env bash
# Seed-stability experiment harness. Each run = fresh container = fresh world.
set -u
SP=/private/tmp/claude-501/-Users-steve235lab-Documents-projects-Delvewright/68f4f0ec-20bc-47f8-a70e-808d8fbe50cc/scratchpad
OUT=$SP/exp/results
mkdir -p $OUT

run_one(){ # name seed pos depth
  local name=$1 seed=$2 pos="$3" depth=$4
  docker rm -f "$name" >/dev/null 2>&1
  docker run -d --name "$name" -e EULA=TRUE -e SEED="$seed" dwexp:local >/dev/null 2>&1
  local tries=0
  until [ "$(docker inspect -f '{{.State.Health.Status}}' "$name" 2>/dev/null)" = "healthy" ]; do
    sleep 2; tries=$((tries+1)); [ $tries -gt 90 ] && { echo "$name NEVER HEALTHY"; return 1; }
  done
  local R="docker exec $name rcon-cli"
  $R "gamerule maxCommandChainLength 2147483647" >/dev/null 2>&1
  $R "forceload add -56 -56 56 56" >/dev/null 2>&1
  sleep 1
  local q=$($R "forceload query" 2>&1 | grep -oE '^[0-9]+ force loaded|[0-9]+ force loaded chunks' | head -1)
  local placed=$($R "place jigsaw dwexp:main dwexp:entrance $depth $pos" 2>&1 | tail -1)
  $R "function dwexp:scan" >/dev/null 2>&1
  sleep 2
  docker logs "$name" 2>&1 | grep -oE 'M [A-Z] -?[0-9]+ -?[0-9]+ -?[0-9]+' | sort -u > "$OUT/fp_$name.txt"
  local n=$(wc -l < "$OUT/fp_$name.txt" | tr -d ' ')
  local h=$(shasum -a256 "$OUT/fp_$name.txt" | cut -d' ' -f1)
  echo "$name seed=$seed pos='$pos' depth=$depth | forceload='$q' | place='$placed' | markers=$n | sha256=$h"
  docker rm -f "$name" >/dev/null 2>&1
}

echo "=== A: same seed (20260729), same pos, fresh worlds x6 ==="
for i in 1 2 3 4 5 6; do run_one "dwrunA$i" 20260729 "0 -59 0" 6; done
echo "=== B: DIFFERENT seed (99999999), same pos, fresh worlds x3 ==="
for i in 1 2 3; do run_one "dwrunB$i" 99999999 "0 -59 0" 6; done
echo "DONE"
