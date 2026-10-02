#!/usr/bin/env python3
"""SPIKE TOOLING (seamless relative-teleport loop) — NOT part of the shipped
pipeline, NOT wired into CI, never read by the compiler.

Writes `dw-loop-spike/` wholesale: one corridor with a one-cell-thick detection
slab across it, a loop function that records the body's position, motion and
rotation on the server before and after a relative `tp`, a party-wide release
score that stands the loop down, and two catch slabs in open air above the sea
(one cell thick and three cells thick) that only TAG a falling body, so the rig
can ask whether a one-tick poll sees a body at terminal speed. Deterministic:
same source, same bytes.
"""

from __future__ import annotations

import json
import shutil
from pathlib import Path

HERE = Path(__file__).resolve().parent
PACK = HERE / "dw-loop-spike"
NS = "dwl"
OBJ = "dwl.s"

# The corridor: walls at X0/X1, interior X0+1..X1-1, floor at WY-1, interior WY..WY+2.
X0, X1 = 4094, 4098
Z0, Z1 = 4188, 4278
WY = 64
CX = 4096
LAMP_Z0, LAMP_Z1, PERIOD = 4190, 4272, 6
# The slab the loop watches, one cell thick in z, and the whole-block offset.
SLAB_Z = 4218
OFFSET = -12
# The two catch slabs in open air above the sea (water top is y 126 in the flat
# preset `run.sh` uses), well clear of the corridor.
CATCH_X0, CATCH_Z0, CATCH_W = 4094, 4300, 4   # 5x5 in x/z
CATCH_Y = 150
DROP_Y = 310
THICK = 3


def fn(name: str, lines: list[str]) -> None:
    p = PACK / "data" / NS / "function" / f"{name}.mcfunction"
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text("\n".join(lines) + "\n", encoding="utf-8")


def tag(name: str, values: list[str]) -> None:
    p = PACK / "data" / "minecraft" / "tags" / "function" / f"{name}.json"
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(json.dumps({"values": values}, indent=2) + "\n", encoding="utf-8")


def main() -> None:
    if PACK.exists():
        shutil.rmtree(PACK)
    PACK.mkdir(parents=True)
    (PACK / "pack.mcmeta").write_text(
        json.dumps({"pack": {"description": "Delvewright SPIKE - seamless loop rig (1.21.11). Research only; never shipped.", "min_format": [94, 1], "max_format": [94, 1]}}, indent=2) + "\n",
        encoding="utf-8",
    )
    tag("load", [f"{NS}:load"])
    tag("tick", [f"{NS}:tick"])

    fn("load", [
        "# SPIKE — seamless-loop rig. Nothing runs until /function dwl:start.",
        f"scoreboard objectives add {OBJ} dummy",
    ])

    build = [
        f"fill {X0} {WY} {Z0} {X1} {WY + 3} {Z1} minecraft:stone_bricks",
        f"fill {X0 + 1} {WY} {Z0} {X1 - 1} {WY + 2} {Z1} minecraft:air",
        f"fill {X0 + 1} {WY - 1} {Z0} {X1 - 1} {WY - 1} {Z1} minecraft:dark_oak_planks",
        f"fill {CX} {WY} {Z0} {CX} {WY} {Z1} minecraft:red_carpet",
    ]
    for z in range(LAMP_Z0, LAMP_Z1 + 1, PERIOD):
        build.append(f"fill {X0} {WY} {z} {X0} {WY + 2} {z} minecraft:polished_deepslate")
        build.append(f"fill {X1} {WY} {z} {X1} {WY + 2} {z} minecraft:polished_deepslate")
        build.append(f"setblock {CX} {WY + 2} {z} minecraft:soul_lantern[hanging=true]")
    fn("corridor/build", build)

    fn("start", [
        f"scoreboard objectives add {OBJ} dummy",
        f"scoreboard players set #on {OBJ} 0",
        f"forceload add 4080 4080 4127 4319",
        f"function {NS}:corridor/build",
        *[f"scoreboard players set #{k} {OBJ} 0" for k in ("released", "moves", "thick", "caught")],
        f"scoreboard players set #on {OBJ} 1",
    ])
    fn("stop", [
        f"scoreboard players set #on {OBJ} 0",
        "forceload remove 4080 4080 4127 4319",
        "tag @a remove dwl_caught",
    ])

    slab = f"x={X0 + 1},y={WY},z={SLAB_Z},dx={X1 - X0 - 2},dy=2,dz=0"
    thin = f"x={CATCH_X0},y={CATCH_Y},z={CATCH_Z0},dx={CATCH_W},dy=0,dz={CATCH_W}"
    thick = f"x={CATCH_X0},y={CATCH_Y},z={CATCH_Z0},dx={CATCH_W},dy={THICK - 1},dz={CATCH_W}"
    fn("tick", [
        f"execute unless score #on {OBJ} matches 1 run return 0",
        # The loop: every body in the slab, judged one by one, while the party has not released it.
        f"execute if score #released {OBJ} matches 0 as @a[{slab}] at @s run function {NS}:loop",
        # The catch slabs only tag; nothing moves a falling body.
        f"execute if score #thick {OBJ} matches 0 as @a[{thin}] run function {NS}:catch",
        f"execute if score #thick {OBJ} matches 1 as @a[{thick}] run function {NS}:catch",
    ])
    fn("catch", [
        "tag @s add dwl_caught",
        f"scoreboard players add #caught {OBJ} 1",
    ])

    def store(prefix: str, suffix: str) -> list[str]:
        out = []
        for i, axis in enumerate("xyz"):
            out.append(f"execute store result score #{prefix}{axis}{suffix} {OBJ} run data get entity @s Pos[{i}] 1000")
            out.append(f"execute store result score #{prefix}m{axis}{suffix} {OBJ} run data get entity @s Motion[{i}] 1000")
        out.append(f"execute store result score #{prefix}yaw{suffix} {OBJ} run data get entity @s Rotation[0] 100")
        out.append(f"execute store result score #{prefix}pitch{suffix} {OBJ} run data get entity @s Rotation[1] 100")
        return out

    fn("loop", [
        *store("", "0"),
        f"tp @s ~ ~ ~{OFFSET}",
        *store("", "1"),
        f"scoreboard players add #moves {OBJ} 1",
        f"execute store result score #move_at {OBJ} run time query gametime",
    ])

    print(f"wrote {PACK.relative_to(HERE)}: slab z={SLAB_Z}, offset {OFFSET}, catch slabs at y={CATCH_Y} (1 and {THICK} thick), drop from y={DROP_Y}")


if __name__ == "__main__":
    main()
