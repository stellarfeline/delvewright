#!/usr/bin/env bash
# Order/context-independence test: is a placement at position Q identical whether
# it is the 1st or the 2nd /place jigsaw in the world? If yes => layout is a pure
# function of (world seed, position), the strongest determinism guarantee.
set -u
SP=/private/tmp/claude-501/-Users-steve235lab-Documents-projects-Delvewright/68f4f0ec-20bc-47f8-a70e-808d8fbe50cc/scratchpad
OUT=$SP/exp/results; mkdir -p $OUT
SEED=20260729
P="0 -59 0"        # first cluster (origin)
Q="40 -59 -40"     # second cluster, within the +-48 scan box, non-overlapping

boot(){ local n=$1; docker rm -f "$n" >/dev/null 2>&1
  docker run -d --name "$n" -e EULA=TRUE -e SEED=$SEED dwexp:local >/dev/null 2>&1
  local t=0; until [ "$(docker inspect -f '{{.State.Health.Status}}' "$n" 2>/dev/null)" = healthy ]; do sleep 2; t=$((t+1)); [ $t -gt 90 ] && { echo "$n UNHEALTHY"; return 1; }; done
  docker exec "$n" rcon-cli "gamerule maxCommandChainLength 2147483647" >/dev/null
  docker exec "$n" rcon-cli "forceload add -56 -56 56 56" >/dev/null; sleep 1; }

scanQ(){ local n=$1 tag=$2 # capture only the Q cluster (x>=20)
  docker exec "$n" rcon-cli "function dwexp:scan" >/dev/null 2>&1; sleep 2
  docker logs "$n" 2>&1 | grep -oE 'M [A-Z] -?[0-9]+ -?[0-9]+ -?[0-9]+' \
    | awk '$3>=20' | sort -u > "$OUT/q_$tag.txt"
  echo "$tag Qmarkers=$(wc -l < $OUT/q_$tag.txt | tr -d ' ') sha=$(shasum -a256 $OUT/q_$tag.txt | cut -d' ' -f1)"; }

# dwX: place P then Q  -> Q is the 2nd placement
boot dwX || exit 1
docker exec dwX rcon-cli "place jigsaw dwexp:main dwexp:entrance 6 $P" >/dev/null
docker exec dwX rcon-cli "place jigsaw dwexp:main dwexp:entrance 6 $Q" >/dev/null
scanQ dwX Qsecond
docker rm -f dwX >/dev/null 2>&1

# dwY: place Q only -> Q is the 1st placement
boot dwY || exit 1
docker exec dwY rcon-cli "place jigsaw dwexp:main dwexp:entrance 6 $Q" >/dev/null
scanQ dwY Qfirst
docker rm -f dwY >/dev/null 2>&1
echo DONE
