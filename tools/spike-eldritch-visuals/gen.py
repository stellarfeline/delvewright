#!/usr/bin/env python3
"""SPIKE TOOLING — generates the `dw-eldritch-spike/` datapack (vanilla 1.21.11).

NOT part of the shipped pipeline, not wired into CI, never consumed by the
compiler. A research lab the owner walks herself (see README.md). The pack is
committed beside this script so it can be copied straight into a world; re-run
this script after any edit (`python3 gen.py`) — it rewrites the pack wholesale.

Everything is deterministic: no clock, no unseeded RNG.
"""

from __future__ import annotations

import json
import math
import random
import shutil
from pathlib import Path

HERE = Path(__file__).resolve().parent
PACK = HERE / "dw-eldritch-spike"
NS = "dwe"

# ------------------------------------------------------------------ site plan
# The lab sits far from the origin (the owner's test world holds a campaign
# near 0,0). Flat world: water surface at y=62, so the lab floor is y=63 and the
# walking level y=64. The walk runs north -> south (+z).
CX = 4096
FY = 63
WY = 64
FX0, FX1 = 4084, 4108          # floor x span
FZ0, FZ1 = 4084, 4300          # floor z span
LAB_BIOME = (4080, 56, 4080, 4111, 95, 4303)   # x0 y0 z0 x1 y1 z1
CHUNK_X = (4080, 4127)         # forceload span (twin tentacle stands at x=4116)
CHUNK_Z = (4080, 4303)

START = (4096.5, WY, 4087.5)

# station 1 — the tentacle pit
PIT_C = (4096, 4116)           # pit centre (x, z)
PIT_IN = 3                     # inner half-width: opening is 7x7
SHAFT_BOTTOM = 30
TENT_BASE = (4096.5, float(WY), 4116.5)
TWIN_BASE = (4116.5, float(WY), 4150.5)

# station 2 — the wrong place
ZONE = (4080, 56, 4128, 4111, 95, 4167)
ZONE_TRIGGER_Z = 4129

# station 3 — perception plate
PLATE = (4096, 4176)

# station 4 — the endless hall
COR_X0, COR_X1 = 4094, 4098    # walls at x0/x1, interior x0+1..x1-1
COR_Z0, COR_Z1 = 4188, 4278    # corridor walls z span
LAMP_Z0, LAMP_Z1, PERIOD = 4190, 4272, 6
LOOP_Z, LOOP_JUMP, LOOP_MAX = 4218, 12, 6

# station 5 — the living room
ROOM_X0, ROOM_X1 = 4086, 4106
ROOM_Z0, ROOM_Z1 = 4278, 4300
ROOM_Y1 = 70                   # roof at ROOM_Y1+1
POOL = (4099, 4103, 4284, 4290)
WATCH_POS = [(4090.5, 4296.5), (4102.5, 4281.5), (4089.5, 4283.5)]

# ------------------------------------------------------------------ tentacle
N_SEG = 34
SEG_L = 0.8
W_BASE, W_TIP = 2.4, 0.32
FRAME_TICKS = 5
RISE = 36                      # frames 0..RISE-1 rise; frame 0 is the hidden summon pose
SWAY = 60                      # loop frames RISE..RISE+SWAY-1
D_HIDDEN = N_SEG * SEG_L + 1.5
D_UP = 4.0                     # final depth of the base below the walking level
BODY = "minecraft:sculk"
BAND = "minecraft:crying_obsidian"


def qmul(a, b):
    ax, ay, az, aw = a
    bx, by, bz, bw = b
    return (
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    )


def qaxis(axis, ang):
    s = math.sin(ang / 2)
    return (axis[0] * s, axis[1] * s, axis[2] * s, math.cos(ang / 2))


def qrot(q, v):
    x, y, z, w = q
    p = (v[0], v[1], v[2], 0.0)
    r = qmul(qmul(q, p), (-x, -y, -z, w))
    return (r[0], r[1], r[2])


def pose(frame: int):
    """Per-segment (translation, left_rotation, scale, mid_y) for one keyframe."""
    if frame < RISE:
        p = frame / (RISE - 1)
        e = 1 - (1 - p) ** 3
        depth = D_HIDDEN + (D_UP - D_HIDDEN) * e
        amp = 0.2 + 0.8 * p
        tau = 2 * math.pi * (frame - RISE) / SWAY
    else:
        depth = D_UP
        amp = 1.0
        tau = 2 * math.pi * (frame - RISE) / SWAY
    # lean slightly toward the viewer (north, -z): rotating +y toward -z is a
    # negative rotation about +x.
    q = qaxis((1, 0, 0), -0.12 * amp * amp)
    j = (0.0, -depth, 0.0)
    out = []
    for i in range(N_SEG):
        u = i / (N_SEG - 1)
        w = W_BASE + (W_TIP - W_BASE) * u
        twist = 0.35 * i + 0.4 * math.sin(tau)
        L = qmul(q, qaxis((0, 1, 0), twist))
        off = qrot(L, (w / 2, -0.06 * SEG_L, w / 2))
        t = (j[0] - off[0], j[1] - off[1], j[2] - off[2])
        nxt = qrot(q, (0.0, SEG_L, 0.0))
        mid_y = j[1] + nxt[1] / 2
        out.append((t, L, (w, SEG_L * 1.12, w), mid_y, (j[0], j[2])))
        j = (j[0] + nxt[0], j[1] + nxt[1], j[2] + nxt[2])
        # joint bends: a travelling wave in two planes, growing toward the tip,
        # plus a periodic curl of the tip toward the viewer ("beckoning").
        curl = -0.11 * (0.5 + 0.5 * math.sin(tau - 0.8)) * u * u
        ax = amp * (0.075 * math.sin(tau - 2.2 * math.pi * u) * u + curl)
        az = amp * (0.06 * math.sin(tau + 1.3 - 1.8 * math.pi * u) * u)
        q = qmul(q, qmul(qaxis((1, 0, 0), ax), qaxis((0, 0, 1), az)))
    return out


def f4(v):
    return f"{v:.4f}f"


def tf(t, L, s):
    return (
        "{translation:[%s],left_rotation:[%s],scale:[%s],right_rotation:[0f,0f,0f,1f]}"
        % (",".join(map(f4, t)), ",".join(map(f4, L)), ",".join(map(f4, s)))
    )


def is_dark(mid_y):
    return mid_y < -0.1   # local y relative to the walking level


# ------------------------------------------------------------------ helpers
def sign(x, z, lines, rot=8, block="minecraft:dark_oak_sign"):
    msgs = ",".join(json.dumps(s) for s in (lines + ["", "", "", ""])[:4])
    return (
        f"setblock {x} {WY} {z} {block}[rotation={rot}]"
        f"{{front_text:{{has_glowing_text:1b,color:\"white\",messages:[{msgs}]}}}}"
    )


def fill_split(x0, y0, z0, x1, y1, z1, what, limit=32768):
    """`fill` in z-slabs that respect max_block_modifications (32768)."""
    area = (x1 - x0 + 1) * (y1 - y0 + 1)
    step = max(1, limit // area)
    cmds = []
    z = z0
    while z <= z1:
        ze = min(z1, z + step - 1)
        cmds.append(f"fill {x0} {y0} {z} {x1} {y1} {ze} {what}")
        z = ze + 1
    return cmds


def fillbiome_split(box, biome):
    x0, y0, z0, x1, y1, z1 = box
    return [c.replace("fill ", "fillbiome ", 1) for c in fill_split(x0, y0, z0, x1, y1, z1, biome)]


def chunk_probe():
    conds = []
    for cx in range(CHUNK_X[0] // 16, CHUNK_X[1] // 16 + 1):
        for cz in range(CHUNK_Z[0] // 16, CHUNK_Z[1] // 16 + 1):
            conds.append(f"if loaded {cx * 16 + 8} {FY} {cz * 16 + 8}")
    return conds


# ------------------------------------------------------------------ the pack
def write(rel, text):
    p = PACK / rel
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(text if text.endswith("\n") else text + "\n")


def fn(name, lines):
    write(f"data/{NS}/function/{name}.mcfunction", "\n".join(lines))


def biome_json(attributes, effects, precip=False, temp=0.8):
    return json.dumps(
        {
            "attributes": attributes,
            "carvers": [],
            "downfall": 0.4,
            "effects": effects,
            "features": [],
            "has_precipitation": precip,
            "spawn_costs": {},
            "spawners": {
                k: []
                for k in [
                    "ambient", "axolotls", "creature", "misc", "monster",
                    "underground_water_creature", "water_ambient", "water_creature",
                ]
            },
            "temperature": temp,
        },
        indent=2,
        sort_keys=True,
    )


def main():
    if PACK.exists():
        shutil.rmtree(PACK)
    write(
        "pack.mcmeta",
        json.dumps(
            {
                "pack": {
                    "description": "Delvewright SPIKE - eldritch visuals lab (1.21.11). Research only; never shipped.",
                    "min_format": [94, 1],
                    "max_format": [94, 1],
                }
            },
            indent=2,
        ),
    )
    write("data/minecraft/tags/function/load.json", json.dumps({"values": [f"{NS}:load"]}, indent=2))
    write("data/minecraft/tags/function/tick.json", json.dumps({"values": [f"{NS}:tick"]}, indent=2))

    # The lab's own neutral biome: plains look, and NO spawners, so nothing
    # spawns in the dark rooms. Station 2 swaps a slab of it for the wrong place.
    write(
        f"data/{NS}/worldgen/biome/lab.json",
        biome_json({"minecraft:visual/sky_color": "#78a7ff"}, {"water_color": "#3f76e4"}),
    )
    write(
        f"data/{NS}/worldgen/biome/wrong_place.json",
        biome_json(
            {
                # Every key below is from the 1.21.11 jar's own attribute list.
                # Overworld timeline `day` MULTIPLIES sky/fog/cloud colour and
                # sky light (so these survive, darkened at night), takes the
                # MAXIMUM of star_brightness (so 0.9 here = stars at noon), and
                # OVERRIDES sun_angle / moon_angle / sunrise colour (so a biome
                # cannot move the sun in the overworld — not set here).
                "minecraft:visual/sky_color": "#3b4a1e",
                "minecraft:visual/fog_color": "#56602f",
                "minecraft:visual/fog_start_distance": 1.0,
                "minecraft:visual/fog_end_distance": 26.0,
                "minecraft:visual/sky_fog_end_distance": 30.0,
                "minecraft:visual/cloud_color": "#e04a1424",
                "minecraft:visual/cloud_fog_end_distance": 40.0,
                "minecraft:visual/sky_light_color": "#b6e07a",
                "minecraft:visual/sky_light_factor": 0.55,
                "minecraft:visual/star_brightness": 0.9,
                "minecraft:visual/water_fog_color": "#120a16",
                "minecraft:visual/water_fog_end_distance": 6.0,
                "minecraft:visual/ambient_particles": [
                    {"particle": {"type": "minecraft:ash"}, "probability": 0.02},
                    {"particle": {"type": "minecraft:crimson_spore"}, "probability": 0.004},
                ],
                "minecraft:audio/background_music": {},
                "minecraft:audio/music_volume": 0.0,
                "minecraft:audio/ambient_sounds": {
                    "loop": "minecraft:ambient.soul_sand_valley.loop",
                    "additions": {"sound": "minecraft:ambient.crimson_forest.additions", "tick_chance": 0.0111},
                    "mood": {
                        "block_search_extent": 8,
                        "offset": 2.0,
                        "sound": "minecraft:ambient.soul_sand_valley.mood",
                        "tick_delay": 1200,
                    },
                },
            },
            {
                "water_color": "#1a0f1f",
                "grass_color": "#6b6a2a",
                "foliage_color": "#5a4a2a",
                "dry_foliage_color": "#4a3a2a",
            },
        ),
    )

    # ---------------------------------------------------------------- load / tick
    fn("load", [
        "# SPIKE — eldritch visuals lab. Nothing runs until /function dwe:start.",
        "scoreboard objectives add dwe.s dummy",
    ])
    fn("tick", [
        "execute unless score #on dwe.s matches 1 run return 0",
        f"function {NS}:tentacle/tick",
        f"function {NS}:zone/tick",
        f"function {NS}:perception/tick",
        f"function {NS}:corridor/tick",
        f"function {NS}:living/tick",
    ])

    # ---------------------------------------------------------------- start / build
    fn("start", [
        "# Run by the owner in her world. Tags her as the viewer, loads the site,",
        "# then builds once every chunk of it is loaded (build_wait).",
        "scoreboard objectives add dwe.s dummy",
        "scoreboard players set #on dwe.s 0",
        "tag @a remove dwe_viewer",
        "tag @s add dwe_viewer",
        "kill @e[tag=dwe]",
        f"forceload add {CHUNK_X[0]} {CHUNK_Z[0]} {CHUNK_X[1]} {CHUNK_Z[1]}",
        "scoreboard players set #wait dwe.s 0",
        'tellraw @s {"text":"[eldritch lab] loading the site at x=4096 z=4096 ...","color":"gray"}',
        f"schedule function {NS}:build_wait 5t",
    ])
    fn("build_wait", [
        "scoreboard players add #wait dwe.s 1",
        f"execute {' '.join(chunk_probe())} run return run function {NS}:build",
        f"execute if score #wait dwe.s matches 120.. run return run tellraw @a {{\"text\":\"[eldritch lab] site never finished loading\",\"color\":\"red\"}}",
        f"schedule function {NS}:build_wait 10t",
    ])

    build = ["# Builds the whole lab (idempotent: re-running rebuilds it)."]
    build += fillbiome_split(LAB_BIOME, f"{NS}:lab")
    # floor + clear the air above it
    build += fill_split(FX0, FY, FZ0, FX1, FY, FZ1, "minecraft:polished_deepslate")
    build += fill_split(FX0, WY, FZ0, FX1, WY + 12, FZ1, "minecraft:air")
    # low edge so nobody walks off into the sea by accident
    build += fill_split(FX0, WY, FZ0, FX0, WY, FZ1, "minecraft:polished_blackstone_brick_wall")
    build += fill_split(FX1, WY, FZ0, FX1, WY, FZ1, "minecraft:polished_blackstone_brick_wall")
    build.append(f"fill {FX0} {WY} {FZ0} {FX1} {WY} {FZ0} minecraft:polished_blackstone_brick_wall")

    # station 1: shaft + guard ring
    px, pz = PIT_C
    r = PIT_IN
    build.append(f"fill {px-r-1} {SHAFT_BOTTOM} {pz-r-1} {px+r+1} {FY-1} {pz+r+1} minecraft:black_concrete")
    build.append(f"fill {px-r} {SHAFT_BOTTOM+1} {pz-r} {px+r} {FY} {pz+r} minecraft:air")
    g = r + 2
    for a, b, c, d in [
        (px - g, pz - g, px + g, pz - g), (px - g, pz + g, px + g, pz + g),
        (px - g, pz - g, px - g, pz + g), (px + g, pz - g, px + g, pz + g),
    ]:
        build.append(f"fill {a} {WY} {b} {c} {WY} {d} minecraft:deepslate_tile_wall")

    # station 2: blighted ground (deterministic scatter)
    rng = random.Random(4096)
    for _ in range(70):
        x = rng.randint(FX0 + 1, FX1 - 1)
        z = rng.randint(ZONE[2] + 3, ZONE[5] - 2)
        blk = rng.choice(["minecraft:coarse_dirt", "minecraft:mud", "minecraft:podzol", "minecraft:rooted_dirt"])
        build.append(f"setblock {x} {FY} {z} {blk}")
        if blk != "minecraft:mud" and rng.random() < 0.45:
            build.append(f"setblock {x} {WY} {z} minecraft:dead_bush")

    # station 3: the plate
    build.append(f"setblock {PLATE[0]} {FY} {PLATE[1]} minecraft:crying_obsidian")

    # station 4: the endless hall
    build.append(f"function {NS}:corridor/build")

    # station 5: the living room
    # hollow shells include their bottom face, so the shell starts at the floor layer
    build.append(f"fill {ROOM_X0} {FY} {ROOM_Z0} {ROOM_X1} {ROOM_Y1} {ROOM_Z1} minecraft:deepslate_bricks hollow")
    build.append(f"fill {ROOM_X0} {ROOM_Y1+1} {ROOM_Z0} {ROOM_X1} {ROOM_Y1+1} {ROOM_Z1} minecraft:deepslate_tiles")
    build.append(f"fill {ROOM_X0+1} {FY} {ROOM_Z0+1} {ROOM_X1-1} {FY} {ROOM_Z1-1} minecraft:sculk")
    # doorway from the hall
    build.append(f"fill {COR_X0+1} {WY} {ROOM_Z0} {COR_X1-1} {WY+2} {ROOM_Z0} minecraft:air")
    # pool (sealed against the open sea beneath the floor)
    x0, x1, z0, z1 = POOL
    build.append(f"fill {x0-1} 54 {z0-1} {x1+1} {FY} {z1+1} minecraft:deepslate_tiles")
    build.append(f"fill {x0} 55 {z0} {x1} {FY} {z1} minecraft:water")
    for x, z in [(ROOM_X0 + 1, ROOM_Z0 + 1), (ROOM_X1 - 1, ROOM_Z0 + 1), (ROOM_X0 + 1, ROOM_Z1 - 1), (ROOM_X1 - 1, ROOM_Z1 - 1)]:
        build.append(f"setblock {x} {WY} {z} minecraft:sculk_catalyst")
    for z in range(ROOM_Z0 + 4, ROOM_Z1 - 1, 4):
        build.append(f"setblock {ROOM_X0 + 2} {WY} {z} minecraft:sculk_sensor")
        if not (x0 - 1 <= ROOM_X1 - 2 <= x1 + 1 and z0 - 1 <= z <= z1 + 1):
            build.append(f"setblock {ROOM_X1 - 2} {WY} {z} minecraft:sculk_sensor")
    build.append(f"setblock {CX} {WY} {ROOM_Z1 - 2} minecraft:sculk_shrieker")
    for x in range(ROOM_X0 + 1, ROOM_X1):
        build.append(f"setblock {x} {ROOM_Y1} {ROOM_Z0 + 1} minecraft:sculk_vein[down=false,up=true]")

    # signs (standing, facing north toward the approaching viewer)
    build += [
        sign(CX - 3, FZ0 + 4, ["ELDRITCH LAB", "walk south", "5 stations", "spike, not a delve"]),
        sign(CX - 4, 4100, ["1  GIANT SHAPE", "block displays", "interpolated", "go to the pit"]),
        sign(CX - 4, ZONE_TRIGGER_Z - 2, ["2  WRONG PLACE", "biome attributes", "fog, sky, light", "set as you enter"]),
        sign(CX - 3, PLATE[1] - 4, ["3  PERCEPTION", "step on the", "purple block", "effects + curse"]),
        sign(CX - 3, COR_Z0 - 3, ["4  ENDLESS HALL", "keep walking", "forward", "relative tp loop"]),
        sign(CX - 3, ROOM_Z0 + 3, ["5  IT LIVES", "sculk, squid,", "watcher, sound", ""]),
        sign(CX + 3, ROOM_Z1 - 2, ["END", "/function", "dwe:stop", "removes entities"]),
    ]

    # entities
    build.append(f"function {NS}:tentacle/summon")
    build.append(
        f"summon minecraft:enderman {WATCH_POS[0][0]} {WY} {WATCH_POS[0][1]} "
        "{Tags:[\"dwe\",\"dwe_watcher\"],NoAI:1b,PersistenceRequired:1b,Invulnerable:1b}"
    )
    for i in range(3):
        build.append(
            f"summon minecraft:glow_squid {x0 + 1 + i} 59 {z0 + 2 + i} "
            "{Tags:[\"dwe\"],PersistenceRequired:1b}"
        )

    # state
    build += [
        "scoreboard players set #tframe dwe.s -1",
        "scoreboard players set #ttick dwe.s 0",
        "scoreboard players set #zone dwe.s 0",
        "scoreboard players set #p3 dwe.s 0",
        "scoreboard players set #loops dwe.s 0",
        "scoreboard players set #wpos dwe.s 0",
        "scoreboard players set #hb dwe.s 0",
        "scoreboard players set #on dwe.s 1",
        f"tp @a[tag=dwe_viewer] {START[0]} {START[1]} {START[2]} 0 0",
        'tellraw @a[tag=dwe_viewer] {"text":"[eldritch lab] ready. Walk south.","color":"gray"}',
    ]
    fn("build", build)

    fn("stop", [
        "scoreboard players set #on dwe.s 0",
        "kill @e[tag=dwe]",
        f"forceload remove {CHUNK_X[0]} {CHUNK_Z[0]} {CHUNK_X[1]} {CHUNK_Z[1]}",
        "tag @a remove dwe_viewer",
        "tag @a remove dwe_p3",
        'tellraw @s {"text":"[eldritch lab] stopped; blocks stay at x=4096 z=4096.","color":"gray"}',
    ])

    # ---------------------------------------------------------------- tentacle
    frames = [pose(f) for f in range(RISE + SWAY)]
    worst = 0.0
    for fr in frames:
        for (_t, _L, s, mid_y, (jx, jz)) in fr:
            if mid_y < 0:
                worst = max(worst, max(abs(jx), abs(jz)) + s[0] * 0.71)
    top = max(max(seg[3] for seg in fr) for fr in frames[RISE:])
    low = min(max(seg[3] for seg in fr) for fr in frames[RISE:])
    print(f"tentacle: {N_SEG} segments x2, {len(frames)} keyframes; tip height above floor "
          f"{low:.1f}..{top:.1f}; widest below-floor reach {worst:.2f} (pit inner half-width {PIT_IN + 0.5})")
    assert worst < PIT_IN + 0.5, "the tentacle would clip the shaft walls"

    summon = []
    for base, extra in [(TENT_BASE, "dwe_main"), (TWIN_BASE, "dwe_twin")]:
        for i, (t, L, s, mid_y, _j) in enumerate(frames[0]):
            blk = BAND if i % 4 == 2 else BODY
            summon.append(
                f"summon minecraft:block_display {base[0]} {base[1]} {base[2]} "
                f"{{Tags:[\"dwe\",\"{extra}\",\"dwe_seg_{i}\"],block_state:{{Name:\"{blk}\"}},"
                f"brightness:{{sky:0,block:0}},interpolation_duration:{FRAME_TICKS},"
                f"transformation:{tf(t, L, s)}}}"
            )
    fn("tentacle/summon", summon)

    for f in range(1, RISE + SWAY):
        prev = frames[f - 1] if f > 1 else frames[0]
        if f == RISE:
            prev = frames[RISE + SWAY - 1]  # the loop re-enters here; brightness is re-asserted
        lines = []
        for i, (t, L, s, mid_y, _j) in enumerate(frames[f]):
            lines.append(
                f"execute as @e[tag=dwe_seg_{i}] run data merge entity @s "
                f"{{start_interpolation:0,interpolation_duration:{FRAME_TICKS},transformation:{tf(t, L, s)}}}"
            )
            was_dark = is_dark(prev[i][3])
            dark = is_dark(mid_y) and was_dark
            if f in (1, RISE) or was_dark != dark:
                if dark:
                    lines.append(f"execute as @e[tag=dwe_seg_{i}] run data merge entity @s {{brightness:{{sky:0,block:0}}}}")
                else:
                    lines.append(f"execute as @e[tag=dwe_seg_{i}] run data remove entity @s brightness")
        fn(f"tentacle/f/{f}", lines)

    fn("tentacle/tick", [
        f"execute if score #tframe dwe.s matches -1 if entity @a[x={FX0},y={WY - 4},z={4097},dx={FX1 - FX0},dy=20,dz=30] run function {NS}:tentacle/wake",
        "execute if score #tframe dwe.s matches 1.. run scoreboard players add #ttick dwe.s 1",
        f"execute if score #ttick dwe.s matches {FRAME_TICKS}.. run function {NS}:tentacle/advance",
    ])
    fn("tentacle/wake", [
        "scoreboard players set #tframe dwe.s 1",
        f"scoreboard players set #ttick dwe.s {FRAME_TICKS}",
        f"playsound minecraft:entity.warden.emerge hostile @a {PIT_C[0]} {FY - 4} {PIT_C[1]} 4 0.5",
        f"playsound minecraft:entity.warden.emerge hostile @a {TWIN_BASE[0]} {FY - 4} {TWIN_BASE[2]} 2 0.4",
        f"particle minecraft:sculk_soul {PIT_C[0]} {WY} {PIT_C[1]} 2 0.3 2 0.02 60",
    ])
    fn("tentacle/advance", [
        "scoreboard players set #ttick dwe.s 0",
        "execute store result storage dwe:t f int 1 run scoreboard players get #tframe dwe.s",
        f"function {NS}:tentacle/play with storage dwe:t",
        "scoreboard players add #tframe dwe.s 1",
        f"execute if score #tframe dwe.s matches {RISE + SWAY}.. run scoreboard players set #tframe dwe.s {RISE}",
    ])
    fn("tentacle/play", [f"$function {NS}:tentacle/f/$(f)"])

    # ---------------------------------------------------------------- zone
    fn("zone/tick", [
        f"execute if score #zone dwe.s matches 0 if entity @a[x={FX0},y={WY - 4},z={ZONE_TRIGGER_Z},dx={FX1 - FX0},dy=20,dz=1] run function {NS}:zone/enter",
    ])
    enter = ["scoreboard players set #zone dwe.s 1"]
    enter += fillbiome_split(ZONE, f"{NS}:wrong_place")
    enter.append("execute store result score #zone_at dwe.s run time query gametime")
    fn("zone/enter", enter)

    # ---------------------------------------------------------------- perception
    px, pz = PLATE
    fn("perception/tick", [
        f"execute if score #p3 dwe.s matches 0 as @a[x={px},y={WY},z={pz},dx=0,dy=1,dz=0] at @s run function {NS}:perception/hit",
        f"execute if score #p3 dwe.s matches 1 unless entity @a[x={px}.5,y={WY},z={pz}.5,distance=..5] run scoreboard players set #p3 dwe.s 0",
    ])
    fn("perception/hit", [
        "scoreboard players set #p3 dwe.s 1",
        "tag @s add dwe_p3",
        "effect give @s minecraft:darkness 8 0 true",
        "effect give @s minecraft:nausea 10 0 true",
        "particle minecraft:elder_guardian ~ ~ ~ 0 0 0 0 1 force @s",
        "playsound minecraft:entity.elder_guardian.curse hostile @s ~ ~ ~ 1 0.7",
        "playsound minecraft:entity.warden.heartbeat hostile @s ~ ~ ~ 1 0.6",
        f"schedule function {NS}:perception/second 50t",
    ])
    fn("perception/second", [
        "execute as @a[tag=dwe_p3] at @s run effect give @s minecraft:blindness 2 0 true",
        "execute as @a[tag=dwe_p3] at @s run particle minecraft:elder_guardian ~ ~ ~ 0 0 0 0 1 force @s",
        "execute as @a[tag=dwe_p3] at @s run playsound minecraft:entity.elder_guardian.curse hostile @s ~ ~ ~ 1 0.5",
        f"schedule function {NS}:perception/third 60t",
    ])
    fn("perception/third", [
        "execute as @a[tag=dwe_p3] at @s rotated ~ 0 positioned ^ ^ ^-3 run playsound minecraft:entity.warden.nearby_closest hostile @a[tag=dwe_p3] ~ ~ ~ 1 0.5",
        "tag @a remove dwe_p3",
    ])

    # ---------------------------------------------------------------- corridor
    cb = [
        f"fill {COR_X0} {WY} {COR_Z0} {COR_X1} {WY + 3} {COR_Z1} minecraft:stone_bricks",
        f"fill {COR_X0 + 1} {WY} {COR_Z0} {COR_X1 - 1} {WY + 2} {COR_Z1} minecraft:air",
        f"fill {COR_X0 + 1} {FY} {COR_Z0} {COR_X1 - 1} {FY} {COR_Z1} minecraft:dark_oak_planks",
        f"fill {CX} {WY} {COR_Z0} {CX} {WY} {COR_Z1} minecraft:red_carpet",
    ]
    for z in range(LAMP_Z0, LAMP_Z1 + 1, PERIOD):
        cb.append(f"fill {COR_X0} {WY} {z} {COR_X0} {WY + 2} {z} minecraft:polished_deepslate")
        cb.append(f"fill {COR_X1} {WY} {z} {COR_X1} {WY + 2} {z} minecraft:polished_deepslate")
        cb.append(f"setblock {CX} {WY + 2} {z} minecraft:soul_lantern[hanging=true]")
    fn("corridor/build", cb)
    fn("corridor/tick", [
        f"execute if score #loops dwe.s matches ..{LOOP_MAX - 1} as @a[x={COR_X0 + 1},y={WY},z={LOOP_Z},dx={COR_X1 - COR_X0 - 2},dy=2,dz=0] at @s run function {NS}:corridor/loop",
    ])
    dim = []
    for z in range(LAMP_Z0 + PERIOD, LAMP_Z1 + 1, 2 * PERIOD):
        dim.append(f"setblock {CX} {WY + 2} {z} minecraft:air")
    fn("corridor/loop", [
        f"tp @s ~ ~ ~-{LOOP_JUMP}",
        "scoreboard players add #loops dwe.s 1",
        f"execute if score #loops dwe.s matches 2 run fill {COR_X0} {WY} {COR_Z0} {COR_X1} {WY + 3} {COR_Z1} minecraft:cracked_stone_bricks replace minecraft:stone_bricks",
        f"execute if score #loops dwe.s matches 4 run function {NS}:corridor/dim",
        f"execute if score #loops dwe.s matches {LOOP_MAX} at @s rotated ~ 0 positioned ^ ^ ^-8 run playsound minecraft:entity.enderman.stare hostile @s ~ ~ ~ 0.6 0.5",
        "execute store result score #loop_at dwe.s run time query gametime",
    ])
    fn("corridor/dim", dim)

    # ---------------------------------------------------------------- living
    room_sel = f"x={ROOM_X0 + 1},y={WY},z={ROOM_Z0 + 1},dx={ROOM_X1 - ROOM_X0 - 2},dy=6,dz={ROOM_Z1 - ROOM_Z0 - 2}"
    fn("living/tick", [
        "execute as @e[tag=dwe_watcher] at @s facing entity @p eyes run tp @s ~ ~ ~ ~ ~",
        f"execute as @e[tag=dwe_watcher] at @s if entity @a[distance=..4.5] run function {NS}:living/vanish",
        f"execute unless entity @a[{room_sel}] run return 0",
        "scoreboard players add #hb dwe.s 1",
        f"execute if score #hb dwe.s matches 24 run playsound minecraft:entity.warden.heartbeat hostile @a[{room_sel}] {CX} {WY + 1} {ROOM_Z1 - 3} 1.6 0.9",
        f"execute if score #hb dwe.s matches 24 run particle minecraft:sculk_charge_pop {CX} {WY + 0.2} {(ROOM_Z0 + ROOM_Z1) // 2} 7 0 9 0 25",
        "execute if score #hb dwe.s matches 24.. run scoreboard players set #hb dwe.s 0",
        "scoreboard players add #amb dwe.s 1",
        f"execute if score #amb dwe.s matches 300.. as @a[{room_sel}] at @s rotated ~ 0 positioned ^ ^ ^-5 run playsound minecraft:ambient.cave ambient @s ~ ~ ~ 1 0.7",
        "execute if score #amb dwe.s matches 300.. run scoreboard players set #amb dwe.s 0",
    ])
    vanish = [
        "particle minecraft:portal ~ ~1 ~ 0.3 1 0.3 0.6 90",
        "playsound minecraft:entity.enderman.teleport hostile @a ~ ~ ~ 1 0.6",
        "scoreboard players add #wpos dwe.s 1",
        f"execute if score #wpos dwe.s matches {len(WATCH_POS)}.. run scoreboard players set #wpos dwe.s 0",
    ]
    for i, (x, z) in enumerate(WATCH_POS):
        vanish.append(f"execute if score #wpos dwe.s matches {i} run tp @s {x} {WY} {z}")
    fn("living/vanish", vanish)



if __name__ == "__main__":
    main()
