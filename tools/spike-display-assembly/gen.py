#!/usr/bin/env python3
"""SPIKE TOOLING — generates `spikepack/` (vanilla 1.21.11) for the display-assembly
measurements spec-0082 rests on.

NOT part of the shipped pipeline, not wired into CI, never consumed by the
compiler. `run.sh` installs the pack on a throwaway pinned server and
`measure.mjs` drives a bot through it. Everything is deterministic: no clock,
no unseeded RNG. Re-run `python3 gen.py` after any edit — it rewrites the pack
wholesale and writes `site.json`, the one statement of every coordinate the
measurement reads (the bot never retypes a number this file computed).

The rig is the research spike's 34-segment tentacle (`research/eldritch-visuals`
at 2bbb1f28, `tools/spike-eldritch-visuals/gen.py`), re-cut as ONE assembly in
the shape the spec proposes:

  * a ROOT `minecraft:item_display` with no item (renders nothing) at the mark;
  * 34 `minecraft:block_display` PARTS riding the root as passengers;
  * one `minecraft:interaction` HITBOX riding the root;
  * five CLIPS (idle, rise, retract, windup, strike) as keyframe functions, one
    function per frame, applied by one macro dispatcher;
  * a hit counter polled off the hitbox's `attack` record, playing `retract` at
    five hits;
  * a strike pattern (windup clip, hold, strike clip, damage at the landing
    frame) that runs while a player stands in a trigger box.

Up to ten identical assemblies can be spawned (`dwa:spawn/<k>`) for the tick-cost
rows; only assembly 0 has the hit counter and the strike pattern.
"""

from __future__ import annotations

import json
import math
import shutil
from pathlib import Path

HERE = Path(__file__).resolve().parent
PACK = HERE / "spikepack"
NS = "dwa"

# ------------------------------------------------------------------ site plan
# Flat world, same layers as the eldritch spike and the owner's test world:
# water surface at y=62, floor laid at y=63, walking level y=64.
FY = 63
WY = 64
AX, AZ = 4096, 4116            # assembly 0's base cell
ASM_DX = 14                    # assembly k stands at AX + k*ASM_DX
N_ASM = 10
FX0, FX1 = AX - 24, AX + 230   # floor x span (the tracking lane runs east)
FZ0, FZ1 = AZ - 44, AZ + 24
LANE = [AX + d for d in (40, 56, 64, 72, 80, 96, 112, 128, 144, 160, 176, 192)]

HITBOX_W, HITBOX_H = 3.0, 8.0

# trigger box (cells), north of the base: x AX-6..AX+6, z AZ-26..AZ-4, y 63..67
TRIG = {"x": AX - 6, "y": FY, "z": AZ - 26, "dx": 12, "dy": 4, "dz": 22}

# ------------------------------------------------------------------ tentacle
N_SEG = 34
SEG_L = 0.8
W_BASE, W_TIP = 2.4, 0.32
D_HIDDEN = N_SEG * SEG_L + 1.5
D_UP = 0.6                     # base this far under the walking level
BODY = "minecraft:sculk"
BAND = "minecraft:crying_obsidian"
RISE, IDLE, WINDUP, STRIKE = 36, 60, 8, 10
WINDUP_BEND = 0.9              # rad, leaning south (+z)
STRIKE_BEND = -3.1             # rad, arcing over to the north (-z), spread evenly
HOLD_TICKS = 20
STRIKE_DAMAGE = 6              # half-hearts
RETRACT_AT_HITS = 5
INTERP = 5                     # interpolation_duration at summon


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


def pose(depth: float, amp: float, tau: float, bend: float, bend_profile: str):
    """Per-segment (translation, left_rotation, scale) for one keyframe, plus the
    tip joint position, all relative to the root entity.

    `bend` is a total rotation about x spread over the joints: `bend_profile`
    `low` weights it toward the base (a lean), `high` toward the tip (a curl).
    """
    q = qaxis((1, 0, 0), -0.12 * amp * amp)
    j = (0.0, -depth, 0.0)
    out = []
    us = [i / (N_SEG - 1) for i in range(N_SEG)]
    if bend_profile == "low":
        w = [(1 - u) for u in us]
    elif bend_profile == "uniform":
        w = [1.0 for _ in us]
    else:
        w = [u for u in us]
    tot = sum(w) or 1.0
    w = [x / tot for x in w]
    for i in range(N_SEG):
        u = us[i]
        wd = W_BASE + (W_TIP - W_BASE) * u
        twist = 0.35 * i + 0.4 * math.sin(tau)
        L = qmul(q, qaxis((0, 1, 0), twist))
        off = qrot(L, (wd / 2, -0.06 * SEG_L, wd / 2))
        t = (j[0] - off[0], j[1] - off[1], j[2] - off[2])
        nxt = qrot(q, (0.0, SEG_L, 0.0))
        out.append((t, L, (wd, SEG_L * 1.12, wd)))
        j = (j[0] + nxt[0], j[1] + nxt[1], j[2] + nxt[2])
        curl = -0.11 * (0.5 + 0.5 * math.sin(tau - 0.8)) * u * u
        ax = amp * (0.075 * math.sin(tau - 2.2 * math.pi * u) * u + curl) + bend * w[i]
        az = amp * (0.06 * math.sin(tau + 1.3 - 1.8 * math.pi * u) * u)
        q = qmul(q, qmul(qaxis((1, 0, 0), ax), qaxis((0, 0, 1), az)))
    return out, j


def clip_frames():
    """The five clips as lists of per-part transforms, plus the tip per frame."""
    clips = {}
    rise = []
    for f in range(RISE):
        p = f / (RISE - 1)
        e = 1 - (1 - p) ** 3
        depth = D_HIDDEN + (D_UP - D_HIDDEN) * e
        rise.append(pose(depth, 0.2 + 0.8 * p, 0.0, 0.0, "low"))
    idle = [pose(D_UP, 1.0, 2 * math.pi * f / IDLE, 0.0, "low") for f in range(IDLE)]
    windup = [
        pose(D_UP, 0.3, 0.0, WINDUP_BEND * (f + 1) / WINDUP, "low") for f in range(WINDUP)
    ]
    strike = []
    for f in range(STRIKE):
        p = (f + 1) / STRIKE
        e = p * p  # accelerating
        b = WINDUP_BEND + (STRIKE_BEND - WINDUP_BEND) * e
        strike.append(pose(D_UP, 0.3, 0.0, b, "uniform"))
    clips["idle"] = (0, idle, True)
    clips["rise"] = (1, rise, False)
    clips["retract"] = (2, list(reversed(rise)), False)
    clips["windup"] = (3, windup, False)
    clips["strike"] = (4, strike, False)
    return clips


def f4(v):
    return f"{v:.4f}f"


def tf(t, L, s):
    return (
        "{translation:[%s],left_rotation:[%s],scale:[%s],right_rotation:[0f,0f,0f,1f]}"
        % (",".join(map(f4, t)), ",".join(map(f4, L)), ",".join(map(f4, s)))
    )


def fill_split(x0, y0, z0, x1, y1, z1, what, limit=32768):
    area = (x1 - x0 + 1) * (y1 - y0 + 1)
    step = max(1, limit // area)
    cmds = []
    z = z0
    while z <= z1:
        ze = min(z1, z + step - 1)
        cmds.append(f"fill {x0} {y0} {z} {x1} {y1} {ze} {what}")
        z = ze + 1
    return cmds


def chunk_probe():
    conds = []
    for cx in range(FX0 // 16, FX1 // 16 + 1):
        for cz in range(FZ0 // 16, FZ1 // 16 + 1):
            conds.append(f"if loaded {cx * 16 + 8} {FY} {cz * 16 + 8}")
    return conds


def write(rel, text):
    p = PACK / rel
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(text if text.endswith("\n") else text + "\n")


def fn(name, lines):
    write(f"data/{NS}/function/{name}.mcfunction", "\n".join(lines))


def sel_box(b):
    return f"x={b['x']},y={b['y']},z={b['z']},dx={b['dx']},dy={b['dy']},dz={b['dz']}"


def main():
    if PACK.exists():
        shutil.rmtree(PACK)
    write(
        "pack.mcmeta",
        json.dumps(
            {
                "pack": {
                    "description": "Delvewright SPIKE - display assembly (1.21.11). Research only; never shipped.",
                    "min_format": [94, 1],
                    "max_format": [94, 1],
                }
            },
            indent=2,
        ),
    )
    write("data/minecraft/tags/function/load.json", json.dumps({"values": [f"{NS}:load"]}, indent=2))
    write("data/minecraft/tags/function/tick.json", json.dumps({"values": [f"{NS}:tick"]}, indent=2))

    clips = clip_frames()
    by_index = {idx: (name, frames, loop) for name, (idx, frames, loop) in clips.items()}

    # the landing frame: the strike clip's last frame; its tip is where the blow lands
    strike_tip = clips["strike"][1][-1][1]
    land_cell = [AX + math.floor(0.5 + strike_tip[0]), FY, AZ + math.floor(0.5 + strike_tip[2])]
    strike_box = {"x": land_cell[0] - 2, "y": FY, "z": land_cell[2] - 2, "dx": 4, "dy": 3, "dz": 4}
    assert TRIG["z"] <= strike_box["z"] and strike_box["z"] + strike_box["dz"] <= TRIG["z"] + TRIG["dz"], (
        "the strike box must lie inside the trigger box; tune STRIKE_BEND",
        strike_tip,
    )
    assert TRIG["x"] <= strike_box["x"] and strike_box["x"] + strike_box["dx"] <= TRIG["x"] + TRIG["dx"]
    assert -1.5 <= strike_tip[1] <= 1.5, ("the tip must land at floor level", strike_tip)

    # ---------------------------------------------------------------- load / tick
    fn("load", [
        "# SPIKE — display assembly rig. Nothing runs until /function dwa:start.",
        "scoreboard objectives add dwa.s dummy",
    ])
    fn("tick", [
        "execute unless score #on dwa.s matches 1 run return 0",
        f"function {NS}:anim/tick",
        f"execute if score #poll dwa.s matches 1 run function {NS}:hit/tick",
        f"function {NS}:strike/tick",
    ])

    # ---------------------------------------------------------------- start / build
    fn("start", [
        "scoreboard objectives add dwa.s dummy",
        "scoreboard players set #on dwa.s 0",
        "kill @e[tag=dwa]",
        f"forceload add {FX0} {FZ0} {FX1} {FZ1}",
        "scoreboard players set #wait dwa.s 0",
        f"schedule function {NS}:build_wait 5t",
    ])
    fn("build_wait", [
        "scoreboard players add #wait dwa.s 1",
        f"execute {' '.join(chunk_probe())} run return run function {NS}:build",
        "execute if score #wait dwa.s matches 120.. run return run scoreboard players set #on dwa.s -1",
        f"schedule function {NS}:build_wait 10t",
    ])
    build = ["# Builds the lab (idempotent)."]
    build += fill_split(FX0, FY, FZ0, FX1, FY, FZ1, "minecraft:polished_deepslate")
    build += fill_split(FX0, WY, FZ0, FX1, WY + 40, FZ1, "minecraft:air")
    # mark the trigger box and the strike box on the floor so a human can see them
    build.append(
        f"fill {TRIG['x']} {FY} {TRIG['z']} {TRIG['x'] + TRIG['dx']} {FY} {TRIG['z'] + TRIG['dz']} minecraft:deepslate_tiles"
    )
    build.append(
        f"fill {strike_box['x']} {FY} {strike_box['z']} {strike_box['x'] + strike_box['dx']} {FY} {strike_box['z'] + strike_box['dz']} minecraft:red_concrete"
    )
    build += [
        f"function {NS}:spawn/0",
        f"scoreboard players set #ft dwa.s {INTERP}",
        "scoreboard players set #poll dwa.s 1",
        "scoreboard players set #hits dwa.s 0",
        "scoreboard players set #lands dwa.s 0",
        "scoreboard players set #sm dwa.s 0",
        "scoreboard players set #hold dwa.s 0",
        "scoreboard players set #on dwa.s 1",
    ]
    fn("build", build)
    fn("stop", [
        "scoreboard players set #on dwa.s 0",
        "kill @e[tag=dwa]",
        f"forceload remove {FX0} {FZ0} {FX1} {FZ1}",
    ])

    # ---------------------------------------------------------------- spawn
    idle0 = clips["idle"][1][0][0]
    for k in range(N_ASM):
        bx, bz = AX + k * ASM_DX + 0.5, AZ + 0.5
        lines = [
            f"kill @e[tag=dwa_all_{k}]",
            f"summon minecraft:item_display {bx} {WY} {bz} "
            f"{{Tags:[\"dwa\",\"dwa_all_{k}\",\"dwa_root\",\"dwa_root_{k}\"]}}",
        ]
        for i, (t, L, s) in enumerate(idle0):
            blk = BAND if i % 4 == 2 else BODY
            lines.append(
                f"summon minecraft:block_display {bx} {WY} {bz} "
                f"{{Tags:[\"dwa\",\"dwa_all_{k}\",\"dwa_part\",\"dwa_asm_{k}\",\"dwa_p_{i}\"],"
                f"block_state:{{Name:\"{blk}\"}},interpolation_duration:{INTERP},"
                f"transformation:{tf(t, L, s)}}}"
            )
            # Every part rides the root directly (a STAR). The first rig run read
            # the root's passenger count through a forked `execute store result`,
            # which stores one branch's 1, not the sum; measure.mjs now counts by
            # `scoreboard players add` per passenger and records the real number.
            lines.append(
                f"ride @e[tag=dwa_asm_{k},tag=dwa_p_{i},limit=1] mount @e[tag=dwa_root_{k},limit=1]"
            )
        # the hitbox stands at the mark on its own (step 2 replaces it with the mob)
        lines.append(
            f"summon minecraft:interaction {bx} {WY} {bz} "
            f"{{width:{HITBOX_W}f,height:{HITBOX_H}f,response:1b,"
            f"Tags:[\"dwa\",\"dwa_all_{k}\",\"dwa_hitbox\",\"dwa_hit_{k}\"]}}"
        )
        lines += [
            f"scoreboard players set #live_{k} dwa.s 1",
            f"scoreboard players set #clip_{k} dwa.s 0",
            f"scoreboard players set #f_{k} dwa.s 0",
            f"scoreboard players set #t_{k} dwa.s 0",
            f"scoreboard players set #done_{k} dwa.s 0",
        ]
        fn(f"spawn/{k}", lines)
        fn(f"despawn/{k}", [f"kill @e[tag=dwa_all_{k}]", f"scoreboard players set #live_{k} dwa.s 0"])

    # ---------------------------------------------------------------- animation
    fn("anim/tick", [
        f"execute if score #live_{k} dwa.s matches 1 run function {NS}:anim/step_{k}" for k in range(N_ASM)
    ])
    for k in range(N_ASM):
        fn(f"anim/step_{k}", [
            f"scoreboard players add #t_{k} dwa.s 1",
            f"execute if score #t_{k} dwa.s >= #ft dwa.s run function {NS}:anim/adv_{k}",
        ])
        adv = [
            f"scoreboard players set #t_{k} dwa.s 0",
            f"scoreboard players add #f_{k} dwa.s 1",
        ]
        for idx, (name, frames, loop) in sorted(by_index.items()):
            n = len(frames)
            if loop:
                adv.append(
                    f"execute if score #clip_{k} dwa.s matches {idx} if score #f_{k} dwa.s matches {n}.. run scoreboard players set #f_{k} dwa.s 0"
                )
            else:
                adv.append(
                    f"execute if score #clip_{k} dwa.s matches {idx} if score #f_{k} dwa.s matches {n}.. run scoreboard players set #f_{k} dwa.s {n - 1}"
                )
                adv.append(
                    f"execute if score #clip_{k} dwa.s matches {idx} if score #f_{k} dwa.s matches {n - 1} run scoreboard players set #done_{k} dwa.s 1"
                )
        adv += [
            f"tag @e[tag=dwa_asm_{k}] add dwa_cur",
            f"execute store result storage dwa:m f int 1 run scoreboard players get #f_{k} dwa.s",
            f"execute store result storage dwa:m c int 1 run scoreboard players get #clip_{k} dwa.s",
            f"function {NS}:anim/apply with storage dwa:m",
            f"tag @e[tag=dwa_asm_{k}] remove dwa_cur",
        ]
        fn(f"anim/adv_{k}", adv)
    fn("anim/apply", [f"$function {NS}:clip/$(c)/f$(f)"])
    for idx, (name, frames, loop) in by_index.items():
        for f, (parts, _tip) in enumerate(frames):
            fn(
                f"clip/{idx}/f{f}",
                [
                    f"execute as @e[tag=dwa_cur,tag=dwa_p_{i},limit=1] run data merge entity @s "
                    f"{{start_interpolation:0,transformation:{tf(t, L, s)}}}"
                    for i, (t, L, s) in enumerate(parts)
                ],
            )
    # play a clip on assembly 0 (the only one the hit counter and the strikes drive)
    for idx in by_index:
        fn(f"play/0_{idx}", [
            f"scoreboard players set #clip_0 dwa.s {idx}",
            "scoreboard players set #f_0 dwa.s -1",
            "scoreboard players operation #t_0 dwa.s = #ft dwa.s",
            "scoreboard players set #done_0 dwa.s 0",
            "execute store result score #play_at dwa.s run time query gametime",
        ])

    # ---------------------------------------------------------------- cadence (one reply, not one per part)
    for d in (1, 5):
        fn(f"interp/{d}", [
            f"scoreboard players set #ft dwa.s {d}",
            f"execute as @e[tag=dwa_part] run data merge entity @s {{interpolation_duration:{d}}}",
        ])

    # ---------------------------------------------------------------- counting
    # counts the passengers of assembly 0's root (a forked `store result` cannot)
    fn("count/passengers", [
        "scoreboard players set #n dwa.s 0",
        "execute as @e[tag=dwa_root_0,limit=1] on passengers run scoreboard players add #n dwa.s 1",
    ])

    # ---------------------------------------------------------------- hits
    fn("hit/tick", [f"execute as @e[tag=dwa_hit_0,nbt={{attack:{{}}}}] run function {NS}:hit/one"])
    fn("hit/one", [
        "scoreboard players add #hits dwa.s 1",
        "execute store result score #hit_at dwa.s run time query gametime",
        "data remove entity @s attack",
        f"execute if score #hits dwa.s matches {RETRACT_AT_HITS} run function {NS}:play/0_{clips['retract'][0]}",
    ])

    # ---------------------------------------------------------------- strikes
    fn("strike/tick", [
        f"execute if score #sm dwa.s matches 0 if entity @a[{sel_box(TRIG)}] run function {NS}:strike/begin",
        f"execute if score #sm dwa.s matches 1 if score #done_0 dwa.s matches 1 run function {NS}:strike/hold",
        "execute if score #sm dwa.s matches 2 run scoreboard players add #hold dwa.s 1",
        f"execute if score #sm dwa.s matches 2 if score #hold dwa.s matches {HOLD_TICKS}.. run function {NS}:strike/swing",
        f"execute if score #sm dwa.s matches 3 if score #done_0 dwa.s matches 1 run function {NS}:strike/land",
    ])
    fn("strike/begin", [
        "scoreboard players set #sm dwa.s 1",
        f"function {NS}:play/0_{clips['windup'][0]}",
        "execute store result score #wind_at dwa.s run time query gametime",
    ])
    fn("strike/hold", [
        "scoreboard players set #sm dwa.s 2",
        "scoreboard players set #hold dwa.s 0",
        "execute store result score #hold_at dwa.s run time query gametime",
    ])
    fn("strike/swing", [
        "scoreboard players set #sm dwa.s 3",
        f"function {NS}:play/0_{clips['strike'][0]}",
        "execute store result score #swing_at dwa.s run time query gametime",
    ])
    fn("strike/land", [
        f"execute as @a[{sel_box(strike_box)},tag=!dw_cutscene] run damage @s {STRIKE_DAMAGE} minecraft:generic",
        "scoreboard players add #lands dwa.s 1",
        "execute store result score #land_at dwa.s run time query gametime",
        f"function {NS}:play/0_{clips['idle'][0]}",
        "scoreboard players set #sm dwa.s 0",
    ])

    # ---------------------------------------------------------------- site.json
    site = {
        "generator": "tools/spike-display-assembly/gen.py",
        "ax": AX, "az": AZ, "wy": WY, "fy": FY,
        "asm_dx": ASM_DX, "n_asm": N_ASM, "n_seg": N_SEG,
        "lane": LANE,
        "hitbox": {"width": HITBOX_W, "height": HITBOX_H,
                   "aabb": [AX + 0.5 - HITBOX_W / 2, WY, AZ + 0.5 - HITBOX_W / 2,
                            AX + 0.5 + HITBOX_W / 2, WY + HITBOX_H, AZ + 0.5 + HITBOX_W / 2]},
        "trigger_box": TRIG,
        "strike_box": strike_box,
        "landing_tip_rel": [round(v, 3) for v in strike_tip],
        "landing_cell": land_cell,
        "hold_ticks": HOLD_TICKS,
        "strike_damage": STRIKE_DAMAGE,
        "retract_at_hits": RETRACT_AT_HITS,
        "interpolation_duration": INTERP,
        "clips": {name: {"index": idx, "frames": len(frames), "loop": loop} for name, (idx, frames, loop) in clips.items()},
        "positions": {
            "attack": [AX + 0.5, WY, AZ + 0.5 + HITBOX_W / 2 + 1.5],
            "bow": [AX + 0.5, WY, AZ + 6.5],
            "bow_aim": [AX + 0.5, WY + 3, AZ + 0.5],
            "arrow_summon": [AX + 0.5, WY + 3, AZ + 5.0],
            "in_strike": [land_cell[0] + 0.5, WY, land_cell[2] + 0.5],
            "in_trigger_only": [TRIG["x"] + 1.5, WY, TRIG["z"] + TRIG["dz"] - 0.5],
            "outside": [AX + 0.5, WY, AZ + 8.5],
        },
    }
    # the trigger-only stand must not meet the strike box with a 0.6-wide body
    p = site["positions"]["in_trigger_only"]
    sb = strike_box
    assert not (sb["x"] - 0.3 <= p[0] <= sb["x"] + sb["dx"] + 1.3 and sb["z"] - 0.3 <= p[2] <= sb["z"] + sb["dz"] + 1.3), p
    (HERE / "site.json").write_text(json.dumps(site, indent=2) + "\n")
    print(json.dumps({"landing_tip_rel": site["landing_tip_rel"], "landing_cell": land_cell,
                      "strike_box": strike_box, "functions": sum(1 for _ in (PACK / "data" / NS / "function").rglob("*.mcfunction"))}))


if __name__ == "__main__":
    main()
