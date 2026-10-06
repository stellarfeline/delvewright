#!/usr/bin/env bash
# SPIKE TOOLING (display assembly) — NOT part of the shipped pipeline and NOT
# wired into CI. spec-0082 §8 row 10: what a root's change of yaw does to its
# riding display parts mid-keyframe, how `execute … facing entity` and `rotate`
# read, and the scoreboard's division and remainder of negatives — the server
# facts an aimed strike (spec-0082 §5.7) stands on.
#
# Boots a throwaway vanilla server from the exact pinned image digest
# (`versions.toml [images.base] mirror_of`, Minecraft Java 1.21.11) on a flat
# world, and asks over rcon; every reply is read (tools/lib/rcon.sh) and printed
# beside its command.
#
# Usage:  EULA=TRUE tools/spike-display-assembly/measure-aim.sh > tools/spike-display-assembly/aim-observations.txt
#
# EULA: acceptance is the owner's action (ADR-0010) — read from the environment.
# PORTS: an ephemeral loopback port, never 25565 (validation/README.md).
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
. "${REPO_ROOT}/tools/lib/rcon.sh"
. "${REPO_ROOT}/tools/lib/server-heap.sh"
HEAP_ENV="$(dw_server_heap_env)"
C=dw-aim-measure
IMAGE='itzg/minecraft-server@sha256:3e7db2562b492dbf442568a327d361547628c98c04a7cb68218c8dde6abdd1de'
FLAT='{"layers":[{"block":"minecraft:bedrock","height":1},{"block":"minecraft:stone","height":62}],"biome":"minecraft:plains"}'
: "${EULA:?EULA}"
cleanup() { docker rm -f "$C" >/dev/null 2>&1 || true; }
trap cleanup EXIT; cleanup
docker run -d --name "$C" -e EULA="$EULA" -e VERSION=1.21.11 -e TYPE=VANILLA -e ONLINE_MODE=FALSE \
  -e "$HEAP_ENV" -e LEVEL_TYPE=minecraft:flat -e "GENERATOR_SETTINGS=$FLAT" -e GENERATE_STRUCTURES=false \
  -p 127.0.0.1::25565 "$IMAGE" >/dev/null
for _ in $(seq 1 120); do dw_rcon_ready "$C" && break; sleep 5; done
dw_rcon_ready "$C"
r() { printf '> %s\n' "$1"; dw_rcon "$C" "$1"; printf '\n'; }
p() { printf '> %s\n' "$1"; dw_rcon_probe "$C" "$1"; printf '\n'; }
r "forceload add 0 0 15 15"
sleep 2
r "summon minecraft:item_display 8.5 63 8.5 {Tags:[\"root\"]}"
for i in 0 1 2; do
  r "summon minecraft:block_display 8.5 63 8.5 {Tags:[\"part\",\"p$i\"],block_state:{Name:\"minecraft:stone\"},transformation:{translation:[-0.5f,${i}.0f,0.5f],left_rotation:[0f,0f,0f,1f],scale:[1f,1f,1f],right_rotation:[0f,0f,0f,1f]}}"
  r "ride @e[tag=p$i,limit=1] mount @e[tag=root,limit=1]"
done
r "summon minecraft:armor_stand 8.5 63 20.5 {Tags:[\"target\"],NoGravity:1b}"
echo "## before"
p "data get entity @e[tag=root,limit=1] Rotation"
for i in 0 1 2; do p "data get entity @e[tag=p$i,limit=1] Rotation"; p "data get entity @e[tag=p$i,limit=1] Pos"; done
echo "## mid-clip: a keyframe starts interpolating, then the root turns 90 by tp in the same function tick"
r "execute as @e[tag=part] run data merge entity @s {start_interpolation:0,interpolation_duration:10,transformation:{translation:[-0.5f,0.0f,2.5f],left_rotation:[0f,0f,0f,1f],scale:[1f,1f,1f],right_rotation:[0f,0f,0f,1f]}}"
r "execute as @e[tag=root] at @s run tp @s ~ ~ ~ 90 0"
p "data get entity @e[tag=root,limit=1] Rotation"
for i in 0 1 2; do p "data get entity @e[tag=p$i,limit=1] Rotation"; p "data get entity @e[tag=p$i,limit=1] Pos"; p "data get entity @e[tag=p$i,limit=1] transformation.translation"; p "data get entity @e[tag=p$i,limit=1] interpolation_duration"; done
p "execute as @e[tag=root] on passengers run data get entity @s Tags"
sleep 3
echo "## 3 s later (60 ticks of positionRider)"
p "data get entity @e[tag=root,limit=1] Rotation"
for i in 0 1 2; do p "data get entity @e[tag=p$i,limit=1] Rotation"; p "data get entity @e[tag=p$i,limit=1] Pos"; done
p "execute as @e[tag=root] on passengers run data get entity @s Tags"
echo "## the bearing: root faces the target with execute facing entity"
r "execute as @e[tag=root] at @s facing entity @e[tag=target,limit=1] feet run tp @s ~ ~ ~ ~ 0"
p "data get entity @e[tag=root,limit=1] Rotation"
p "data get entity @e[tag=p0,limit=1] Rotation"
r "tp @e[tag=target] 20.5 63 8.5"
r "execute as @e[tag=root] at @s facing entity @e[tag=target,limit=1] feet run tp @s ~ ~ ~ ~ 0"
p "data get entity @e[tag=root,limit=1] Rotation"
p "data get entity @e[tag=p0,limit=1] Rotation"
p "execute as @e[tag=root] on passengers run data get entity @s Tags"
echo "## rotate command on the root"
p "rotate @e[tag=root,limit=1] -45 0"
p "data get entity @e[tag=root,limit=1] Rotation"
p "data get entity @e[tag=p0,limit=1] Rotation"
p "execute as @e[tag=root] on passengers run data get entity @s Tags"
echo "## scoreboard floor division and modulo of negatives"
r "scoreboard objectives add m dummy"
r "scoreboard players set #a m -7"
r "scoreboard players set #b m 2"
r "scoreboard players operation #a m /= #b m"
p "scoreboard players get #a m"
r "scoreboard players set #a m -7"
r "scoreboard players set #b m 8"
r "scoreboard players operation #a m %= #b m"
p "scoreboard players get #a m"
echo "## data get Rotation scaled, negative yaw"
r "tp @e[tag=root] 8.5 63 8.5 -100.7 0"
p "execute store result score #y m run data get entity @e[tag=root,limit=1] Rotation[0] 8"
p "scoreboard players get #y m"
