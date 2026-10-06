#!/usr/bin/env python3
"""The far-field threshold's second method (spec-0090 §5.2).

Station 4's largest far-field shift, computed from the eldritch spike's own
constants (`tools/spike-eldritch-visuals/gen.py` on branch
`research/eldritch-visuals` at `2bbb1f28`) with this file's own block-light
flood and voxel walk. It shares no code with `delvec`: the engine's reading is
`station_4_calibrates_the_far_field_threshold` in
`crates/delvec/tests/endless_corridor.rs`, and the two are compared in
`tools/tests/test_far_field.py`.

The hall is the spike's: stone-brick walls `x 4094..4098`, a three-wide
passage floored in dark oak with red carpet down its centre, a polished
deepslate pillar pair and a hanging soul lantern every 6 blocks on the spike's
phase, the slab at `z 4218` and the landing a jump back, station 5's room past
the far end. Its approach is lengthened to 84 blocks behind the slab, as the
engine's fixture is (spec-0090 §5.1), so the reading is the forward view's.

What it computes: every cell of the hall past the slab that differs from its
image under the offset, in block, or in light with its lit area (the air cell
and every non-air neighbour); for each, the largest shift of its nine points
(centre and corners) between the eye before the jump and after it, over every
judged eye that sees it (the landing cells' standing eyes, centre and hitbox
corners, and the catch band 0.3 short of the landing's approach face). Prints
the largest, its cell, why it differs, and its eye.

    python3 tools/spike-seamless-loop/far_field.py [--ahead 60] [--jump 12] [--lit]
"""

import argparse
import math
from collections import deque

CX, FY, WY = 4096, 63, 64
COR_X0, COR_X1 = 4094, 4098
PERIOD, PHASE, LOOP_Z = 6, 4190, 4218
ROOM_X0, ROOM_X1, ROOM_Y1, ROOM_DEPTH = 4086, 4106, 70, 22
PORCH, PX0, PX1 = 8, 4090, 4102


def measure(ahead: int = 60, jump: int = 12, lit: bool = False):
    """The largest far-field shift of station 4 with its far end `ahead`
    blocks past the slab: `(degrees, cell, eye, why)`."""
    REAR, END, JUMP = LOOP_Z - 84, LOOP_Z + ahead, jump
    blocks = {}
    def fill(a, b, blk):
        for x in range(a[0], b[0]+1):
            for y in range(a[1], b[1]+1):
                for z in range(a[2], b[2]+1):
                    if blk == "air": blocks.pop((x,y,z), None)
                    else: blocks[(x,y,z)] = blk
    z0, z1 = REAR - PORCH, END + ROOM_DEPTH
    fill((ROOM_X0, FY, z0), (ROOM_X1, FY, z1), "polished_deepslate")
    for x in range(PX0, PX1+1):
        for y in range(WY, WY+5):
            for z in range(z0, REAR):
                if x in (PX0, PX1) or y == WY+4 or z == z0: blocks[(x,y,z)] = "stone_bricks"
    fill((PX0, WY, REAR), (PX1, WY+4, REAR), "stone_bricks")
    for x in (PX0+2, PX1-2): blocks[(x, WY+3, z0+3)] = "lantern"
    fill((COR_X0, WY, REAR), (COR_X1, WY+3, END), "stone_bricks")
    fill((COR_X0+1, WY, REAR), (COR_X1-1, WY+2, END), "air")
    fill((COR_X0+1, FY, REAR), (COR_X1-1, FY, END), "dark_oak_planks")
    fill((CX, WY, REAR), (CX, WY, END), "red_carpet")
    lamps = [z for z in range(REAR+1, END-PERIOD+1) if (z-PHASE) % PERIOD == 0]
    for z in lamps:
        fill((COR_X0, WY, z), (COR_X0, WY+2, z), "polished_deepslate")
        fill((COR_X1, WY, z), (COR_X1, WY+2, z), "polished_deepslate")
        blocks[(CX, WY+2, z)] = "soul_lantern"
    rz0, rz1 = END, END + ROOM_DEPTH
    for x in range(ROOM_X0, ROOM_X1+1):
        for y in range(FY, ROOM_Y1+1):
            for z in range(rz0, rz1+1):
                if x in (ROOM_X0, ROOM_X1) or y in (FY, ROOM_Y1) or z in (rz0, rz1):
                    blocks[(x,y,z)] = "deepslate_bricks"
    fill((ROOM_X0, ROOM_Y1+1, rz0), (ROOM_X1, ROOM_Y1+1, rz1), "deepslate_tiles")
    fill((ROOM_X0+1, FY, rz0+1), (ROOM_X1-1, FY, rz1-1), "sculk")
    fill((COR_X0+1, WY, rz0), (COR_X1-1, WY+2, rz0), "air")
    for x, z in ((ROOM_X0+1, rz0+1), (ROOM_X1-1, rz0+1), (ROOM_X0+1, rz1-1), (ROOM_X1-1, rz1-1)):
        blocks[(x, WY, z)] = "sculk_catalyst"
    z = rz0 + 4
    while z < rz1 - 1:
        blocks[(ROOM_X0+2, WY, z)] = "sculk_sensor"; blocks[(ROOM_X1-2, WY, z)] = "sculk_sensor"; z += 4
    blocks[(CX, WY, rz1-2)] = "sculk_shrieker"
    for x, z in ((ROOM_X0+5, rz0+6), (ROOM_X1-5, rz0+6), (ROOM_X0+5, rz1-6), (ROOM_X1-5, rz1-6)):
        blocks[(x, ROOM_Y1-1, z)] = "lantern"
    if lit:
        fill((COR_X0+1, WY+3, rz0), (COR_X1-1, WY+3, rz0), "glowstone")

    EMIT = {"glowstone": 15, "lantern": 15, "soul_lantern": 10, "sculk_catalyst": 6, "sculk_sensor": 1}
    # Light passes these (vanilla: not full opaque cubes); everything else stops it.
    CLEAR = {"lantern", "soul_lantern", "red_carpet", "sculk_sensor", "sculk_shrieker"}
    OPAQUE_STOP = lambda c: c in blocks and blocks[c] not in CLEAR
    lo = (ROOM_X0, FY, z0); hi = (ROOM_X1, ROOM_Y1+1, z1)
    light = {}
    q = deque()
    for c, b in blocks.items():
        if b in EMIT:
            light[c] = EMIT[b]; q.append(c)
    while q:
        c = q.popleft(); l = light[c]
        for i in range(3):
            for s in (-1, 1):
                n = list(c); n[i] += s; n = tuple(n)
                if any(n[k] < lo[k] or n[k] > hi[k] for k in range(3)): continue
                if OPAQUE_STOP(n): continue
                if light.get(n, 0) < l - 1:
                    light[n] = l - 1; q.append(n)
    L = lambda c: 0 if OPAQUE_STOP(c) else light.get(c, 0)
    blk = lambda c: blocks.get(c, "air")

    # Eyes: the three standable landing cells, centre and hitbox corners, and the catch band.
    eyes = []
    for cx in (4095, 4096, 4097):
        feet = WY + (1/16 if cx == CX else 0)          # the carpet lifts the centre cell's feet
        ey = feet + 1.62
        for dx, dz in ((0,0),(-.3,-.3),(-.3,.3),(.3,-.3),(.3,.3)):
            eyes.append((cx+.5+dx, ey, LOOP_Z-JUMP+.5+dz))
        for side in (-.3, 0, .3):
            eyes.append((cx+.5+side, ey, LOOP_Z-JUMP-.3))
    d = (0, 0, -JUMP)
    def pts(c):
        m = (c[0]+.5, c[1]+.5, c[2]+.5); out = [m]; e = 1e-3
        for a in (e, 1-e):
            for b in (e, 1-e):
                for g in (e, 1-e): out.append((c[0]+a, c[1]+b, c[2]+g))
        return out
    def shift(post, p):
        pre = [post[i]-d[i] for i in range(3)]
        u = [p[i]-pre[i] for i in range(3)]; v = [p[i]-post[i] for i in range(3)]
        cos = sum(u[i]*v[i] for i in range(3))
        cr = (u[1]*v[2]-u[2]*v[1], u[2]*v[0]-u[0]*v[2], u[0]*v[1]-u[1]*v[0])
        return math.degrees(math.atan2(math.sqrt(sum(x*x for x in cr)), cos))
    CAM = lambda c: c in blocks and blocks[c] not in CLEAR   # what stops a sightline
    def walk(a, b, stop):
        """Voxel walk (Amanatides & Woo 1987) from a to b; the first cell `stop`
        holds for, or None."""
        cell = [math.floor(v) for v in a]; end = [math.floor(v) for v in b]
        dv = [b[i]-a[i] for i in range(3)]
        step = [0,0,0]; tmax = [math.inf]*3; tdel = [math.inf]*3
        for i in range(3):
            if dv[i] > 0: step[i] = 1; tmax[i] = (cell[i]+1-a[i])/dv[i]; tdel[i] = 1/dv[i]
            elif dv[i] < 0: step[i] = -1; tmax[i] = (cell[i]-a[i])/dv[i]; tdel[i] = -1/dv[i]
        if stop(tuple(cell)): return tuple(cell)
        for _ in range(sum(abs(end[i]-cell[i]) for i in range(3))):
            i = min(range(3), key=lambda k: tmax[k])
            cell[i] += step[i]; tmax[i] += tdel[i]
            if stop(tuple(cell)): return tuple(cell)
        return None
    def sees(eye, c):
        return any(walk(eye, t, lambda p: p != c and CAM(p)) is None for t in pts(c))
    # Candidate cells: the hall's interior and its shell, from the slab's far side to the end.
    best = (0, None, None, None)
    for z in range(LOOP_Z+1, END+1):
        for x in range(COR_X0, COR_X1+1):
            for y in range(FY, WY+4):
                c = (x, y, z); img = (x, y, z+JUMP)
                if blk(c) != blk(img):
                    area = [c]; why = "block"
                elif not CAM(c) and L(c) != L(img):
                    area = [c] + [n for n in ((x+1,y,z),(x-1,y,z),(x,y+1,z),(x,y-1,z),(x,y,z+1),(x,y,z-1)) if blk(n) != "air"]
                    why = "light %d vs %d" % (L(c), L(img))
                else:
                    continue
                for e in eyes:
                    if not any(sees(e, a) for a in area): continue
                    s = max(shift(e, p) for a in area for p in pts(a))
                    if s > best[0]: best = (s, c, e, why)
    return best


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("--ahead", type=int, default=60, help="far end, blocks past the slab")
    p.add_argument("--jump", type=int, default=12, help="the offset's length")
    p.add_argument("--lit", action="store_true", help="a glowstone lintel over the doorway")
    a = p.parse_args()
    s, cell, eye, why = measure(a.ahead, a.jump, a.lit)
    print(
        "ahead %d, jump %d%s: largest far shift %.6f deg at %s (%s) from eye %s"
        % (a.ahead, a.jump, ", lit" if a.lit else "", s, list(cell), why, [round(v, 2) for v in eye])
    )


if __name__ == "__main__":
    main()
