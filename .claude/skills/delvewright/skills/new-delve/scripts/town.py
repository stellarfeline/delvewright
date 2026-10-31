"""The starter's settlement helpers: streets, fronts, yards, boats and quays, drawn into a `Model`.

A town's places are drawn on the starter (`starter.py`, beside this file) like
any other place; these are the mechanisms a settlement's generators reach for
again and again, each written once here and imported, never copied:

    from starter import Model, run, stretch, street_front   # the starter re-exports this module

Every helper is a mechanism, never a design: the caller states the seed, the
clear width, the axis, the edge, the roles and the blocks — nothing is read
from a place's name — and the design (which palette, which trade hangs which
sign, which things stand on a verge, this island's coast) stays in the
campaign. Coordinates are piece-local and frame-relative: `place.space` is the
play space, `place.datum.value` the walk plane, the course under it the
ground. A role is a key of the model's palette (a weighted list is one role);
a literal state is written with every property (the starter's constructors).

What each one draws, and the craft rule behind it, is in
`references/town.md` and `docs/reference/place-craft.md` *A town*.

Standard library only.
"""

from __future__ import annotations

import sys

from starter import AIR, OPP, fence, lantern, slab, stairs

# ════════════════════════════════════════════════════════════════════════
# Small shared readings


def hsh(*v: int) -> float:
    """A fixed hash of integers into [0, 1): wear, wander and choice that are
    the same on every run (FNV-1a over the 32-bit values)."""
    h = 2166136261
    for x in v:
        h = ((h ^ (x & 0xFFFFFFFF)) * 16777619) & 0xFFFFFFFF
    return (h >> 8) / float(1 << 24)


def walk_of(m) -> int:
    """The walk plane a model is drawn against (1 where the model carries no place)."""
    place = getattr(m, "place", None)
    return place.datum.value if place is not None else 1


def seam_cols(place, pad: int = 1) -> set[tuple[int, int]]:
    """Every column a neighbour's opening stands in, or within `pad` of it:
    the ground a way in needs, which furniture keeps off."""
    out = set()
    for s in place.seams:
        lo, hi = [b.value for b in s["lo"]], [b.value for b in s["hi"]]
        for x in range(lo[0] - pad, hi[0] + pad):
            for z in range(lo[2] - pad, hi[2] + pad):
                out.add((x, z))
    return out


ACROSS = {"x": ("north", "south"), "z": ("west", "east")}  # (toward the low edge, toward the high edge)


def way_frame(place, axis: str):
    """A way's frame along `axis` (`"x"` or `"z"`, the way's run as the plan
    states it): `(rows, cols, cell)` — `rows` the coordinates along the way,
    `cols` across it, low edge first, and `cell(a, b)` the column `(x, z)` of
    row `a`, col `b`."""
    (x0, _, z0), (x1, _, z1) = place.space
    if axis == "x":
        return list(range(x0, x1 + 1)), list(range(z0, z1 + 1)), lambda a, b: (a, b)
    if axis == "z":
        return list(range(z0, z1 + 1)), list(range(x0, x1 + 1)), lambda a, b: (b, a)
    raise ValueError(f"town.way_frame: `{axis}` is not an axis; a way runs along x or z")


def handed_way(place, axis: str | None, clear: int | None, who: str) -> tuple[str, int | None, tuple[int, int] | None]:
    """The way's axis, clear width and clear columns as the plan hands them
    (`place.way`), or as the caller states them where the plan declares no
    way. A stated value the plan also hands is refused when it differs: the
    plan's number is the one the clear-width check judges (`DW1057`), so a
    retyped one can only be wrong. Returns `(axis, clear, cols)`, `cols` the
    first and last clear column across the run where handed."""
    way = getattr(place, "way", None)
    if way is not None:
        for name, said, handed in (("axis", axis, way["axis"]), ("clear", clear, way["clear"])):
            if said is not None and said != handed:
                sys.exit(f"town.{who}: `{name}={said!r}` is stated, and the plan hands this way `{name}` {handed!r}; leave it out — the plan's is the one judged")
        return way["axis"], way["clear"], tuple(way["cols"])
    if axis is None:
        sys.exit(f"town.{who}: the plan declares no `way` for this place, so state its `axis` (\"x\" or \"z\")")
    return axis, clear, None


def up_seam(place):
    """The seam a way climbs to (its `rise` above zero), or None."""
    return next((s for s in place.seams if s["seam"].get("rise", 0) > 0), None)


def climbs_itself(m, seam) -> None:
    """The way being drawn is the way up to `seam`'s opening: where the
    starter would stand a ladder under it (an opening two or more courses up),
    say so (`Model.climbs_to`), so no ladder stands in the steps."""
    ap = seam.get("approach")
    if ap is not None and ap["ladder"] is not None:
        m.climbs_to(seam["stem"])


def handcart(m, x: int, z: int, nx: int, nz: int, w: int, wood: str, along: str, load: str | None = None) -> None:
    """A cart of `wood` on two cells along the way, `(x, z)` and `(nx, nz)`:
    its bed two upside-down stairs back to back, each standing on its own
    end, so the bed rests on the ground at both ends and reads as a cart
    and never as boards floating over the street; `load` (an axis block, a
    hay bale) on the first cell, laid `along` the way."""
    dx, dz = nx - x, nz - z
    away = {(1, 0): "east", (-1, 0): "west", (0, 1): "south", (0, -1): "north"}[(dx, dz)]
    m.set(x, w, z, stairs(wood, OPP[away], "top"))
    m.set(nx, w, nz, stairs(wood, away, "top"))
    if load is not None:
        m.set(x, w + 1, z, f"{load}[axis={along}]")


# ════════════════════════════════════════════════════════════════════════
# Street furniture


def lamp(m, x: int, z: int, y: int | None = None, post: str | None = None, light: str | None = None, height: int = 3) -> None:
    """A lamp post: `height` courses of `post` (an oak fence) from the walk
    plane, `light` (a standing lantern) on top."""
    y = walk_of(m) if y is None else y
    for dy in range(height):
        m.set(x, y + dy, z, post or fence("minecraft:oak"))
    m.set(x, y + height, z, light or lantern(False))
    if y + height >= getattr(m, "ey", y + height + 2) - 1 and hasattr(m, "show"):
        # The light stands on the frame's top course in the open air: the
        # piece's top is seen there, and is declared so (`DW0885`).
        m.show("up")


SIGN_ROT = {"west": 12, "east": 4, "north": 0, "south": 8}  # a sign hung before a front on each face looks back at the street


def bracket(m, place, seam, top: int, sign: str, mark: str | None, post: str | None = None,
            beam: str = "minecraft:stripped_spruce_log", margin: int = 2) -> set | None:
    """A shop sign on the street's own bracket before a front's doorway.

    `seam` is the starter's seam entry for the front (its handed `cells` the
    doorway); a post of `post` stands in the street beside the doorway, a beam
    of `beam` (a log, its axis along the edge) in course `top` from the post
    over the doorway's first column, and `sign` (a hanging sign's block name,
    `minecraft:spruce_hanging_sign`) under it, turned to face back across the
    street. Post, beam and sign are the street's own blocks, so the sign hangs
    on nothing another piece places. The sign's cell is named `mark` (the
    campaign's `signs` entry letters it). The post keeps `margin` columns off
    the play space's ends. Returns the columns it takes, or None where the
    doorway has no room beside it."""
    s = seam["seam"]
    face = s["face"]
    (lx, _, lz), (hx, _, hz) = s["cells"]
    (x0, _, z0), (x1, _, z1) = place.space
    if face in ("west", "east"):
        u = lx + 1 if face == "west" else lx - 1
        lo, hi, amin, amax = lz, hz, z0 + margin, z1 - margin
        cell = lambda a: (u, a)  # noqa: E731
        axis = "z"
    else:
        u = lz + 1 if face == "north" else lz - 1
        lo, hi, amin, amax = lx, hx, x0 + margin, x1 - margin
        cell = lambda a: (a, u)  # noqa: E731
        axis = "x"
    if lo - 1 >= amin:
        p, hang = lo - 1, lo
    elif hi + 1 <= amax:
        p, hang = hi + 1, hi
    else:
        return None
    px, pz = cell(p)
    sx, sz = cell(hang)
    for y in range(walk_of(m), top):
        m.set(px, y, pz, post or fence("minecraft:oak"))
    for x, z in ((px, pz), (sx, sz)):
        m.set(x, top, z, f"{beam}[axis={axis}]")
    m.set(sx, top - 1, sz, f"{sign}[attached=false,rotation={SIGN_ROT[face]},waterlogged=false]")
    if top >= getattr(m, "ey", top + 2) - 1 and hasattr(m, "show"):
        m.show("up")  # the beam on the frame's top course, in the open air (`DW0885`)
    if mark:
        m.mark(mark, sx, sz, y=top - 1)
    return {(px, pz), (sx, sz)}


def doorway_verge(seam) -> set[tuple[int, int]]:
    """The street's columns just in front of a front's doorway: the way in,
    kept clear."""
    s = seam["seam"]
    (lx, _, lz), (hx, _, hz) = s["cells"]
    face = s["face"]
    out = set()
    if face in ("west", "east"):
        u = lx + 1 if face == "west" else lx - 1
        out |= {(u, a) for a in range(min(lz, hz), max(lz, hz) + 1)}
    else:
        u = lz + 1 if face == "north" else lz - 1
        out |= {(a, u) for a in range(min(lx, hx), max(lx, hx) + 1)}
    return out


# ════════════════════════════════════════════════════════════════════════
# Ways: streets, lanes, flights, ramps, verges

WAY_ROLES = {"verge": "verge", "kennel": "kennel", "kerb": "kerb", "paving": "paving"}


def pave_way(m, x0: int, x1: int, z0: int, z1: int, axis: str, roles: dict | None = None, y: int | None = None) -> None:
    """A street or a lane laid flush: `paving` across, a `kennel` (the gutter
    the rain runs down) along the middle, `kerb` setts beside the outer
    columns where the way is five or wider, the outer columns `verge` worn
    into the ground. `axis` is the way's run; `y` the ground course (the walk
    plane's under it by default). The kerb is in the paving, never a step at
    its edge."""
    r = dict(WAY_ROLES, **(roles or {}))
    y = walk_of(m) - 1 if y is None else y
    if axis == "z":
        a0, a1, b0, b1 = z0, z1, x0, x1
    else:
        a0, a1, b0, b1 = x0, x1, z0, z1
    mid = (b0 + b1) // 2
    for a in range(a0, a1 + 1):
        for b in range(b0, b1 + 1):
            if b in (b0, b1):
                role = r["verge"]
            elif b == mid:
                role = r["kennel"]
            elif b in (b0 + 1, b1 - 1) and b1 - b0 >= 4:
                role = r["kerb"]
            else:
                role = r["paving"]
            if axis == "z":
                m.set(b, y, a, role)
            else:
                m.set(a, y, b, role)


# No container among the defaults: a barrel is a holder the census counts
# (`DW1018`), so one stands on a verge only where the caller names it.
VERGE_THINGS = ("lamp", "bed", "bench", "cart", "bed", "lamp", "bench")
FLOWERS = ("minecraft:poppy", "minecraft:cornflower", "minecraft:oxeye_daisy", "minecraft:allium", "minecraft:dandelion")


def stretch(m, place, axis: str | None = None, seed: int | None = None, clear: int | None = None, roles: dict | None = None,
            fronts: dict | None = None, bands: int = 2, verge=None, top: int | None = None) -> dict:
    """A stretch of a main street, after the medieval street (cobbles set in
    sand, a central kennel, the paving re-laid in patches).

    The run's `axis`, the clear way's width and its columns are the plan's
    (`place.way`, handed by the allocation): leave them out. Only a place the
    plan declares no way for states `axis` and `clear` (centred across the
    play space). `seed` is the stretch's own wear, and is always stated. The
    clear way's
    `kennel` wanders a column either side of the middle every three rows and
    is broken where the paving was re-laid, `bands` rows of `kerb` setts lie
    at each end (where the flights land), the centre worn in patches of
    `verge` and `kerb`; the columns either side of the clear way are the
    `verge`. All of it is drawn from `seed`, the same on every run.

    `fronts` maps a neighbour's node (`node/bakery`) to the hanging sign its
    front hangs (`minecraft:spruce_hanging_sign`; None for a front with no
    sign): the sign goes on the street's bracket (`bracket`, named
    `sign-<stem>`, under course `top`, the frame's top by default) and the
    street in front of the doorway is kept clear. Last, `verge(m, frame)` —
    `verge_life` with its defaults where none is given — furnishes the
    verge; `frame` is a dict of `free` (the verge columns off every opening,
    doorway and bracket), `rows`, `cols`, `cell`, `c0`, `c1` (the clear way's
    cols), `axis` and `seed`. Returns that dict."""
    if seed is None:
        sys.exit("town.stretch: state the stretch's `seed`, its own wear, the same on every run")
    axis, clear, handed = handed_way(place, axis, clear, "stretch")
    if handed is None and clear is None:
        sys.exit("town.stretch: the plan declares no `way` for this place, so state its `clear` width")
    r = dict(WAY_ROLES, **(roles or {}))
    rows, cols, cell = way_frame(place, axis)
    a0, a1 = rows[0], rows[-1]
    b0, b1 = cols[0], cols[-1]
    g = walk_of(m) - 1
    if handed is not None:
        c0, c1 = handed
    else:
        c0 = b0 + (len(cols) - clear) // 2
        c1 = c0 + clear - 1
    mid = (c0 + c1) // 2
    band = set(rows[:bands]) | set(rows[-bands:]) if bands else set()
    for a in rows:
        wander = (-1, 0, 0, 1)[int(hsh(seed, a // 3, 7) * 4)] if a not in band else 0
        broken = hsh(seed, a, 11) < 0.18
        for b in cols:
            if b < c0 or b > c1:
                role = r["verge"]
            elif a in band:
                role = r["kerb"]
            elif b == mid + wander and not broken:
                role = r["kennel"]
            elif hsh(seed, b, a, 3) < 0.22:
                role = r["verge"] if hsh(seed, b, a, 5) < 0.4 else r["kerb"]
            else:
                role = r["paving"]
            x, z = cell(a, b)
            m.set(x, g, z, role)
    keep = seam_cols(place)
    top = m.ey - 1 if top is None else top
    for s in place.seams:
        other = s["seam"].get("other")
        if fronts is None or other not in fronts:
            continue
        if fronts[other]:
            took = bracket(m, place, s, top, fronts[other], f"sign-{other.removeprefix('node/')}")
            if took:
                keep |= took
        keep |= doorway_verge(s)
    lanes = [b for b in cols if b < c0 or b > c1]
    free = {cell(a, b) for b in lanes for a in range(a0 + 2, a1 - 1)} - keep
    frame = {"free": free, "rows": rows, "cols": cols, "cell": cell, "c0": c0, "c1": c1, "axis": axis, "seed": seed}
    (verge or verge_life)(m, frame)
    return frame


def verge_life(m, frame: dict, things: tuple = VERGE_THINGS, every: int = 3, flowers: tuple = FLOWERS,
               bench: str = "minecraft:spruce", cart: str = "minecraft:spruce", cart_load: str = "minecraft:hay_block",
               at: list | tuple = ()) -> list[tuple[str, int, str, int, int]]:
    """A street's verge furnished: first each thing the caller places in
    `at` — `(kind, row, side)`, `row` a coordinate along the way (one of
    `frame["rows"]`), `side` `"low"` or `"high"` across it — then one thing
    every `every` rows on each side, taken round `things` from the seed,
    wherever the verge column is still free. A kind is `lamp`, `bed` (a
    flower and a tuft in turf), `bench` (two stairs of `bench`, their backs
    to the lot), `cart` (a cart of `cart` wood along the way, `cart_load` on
    it: `town.handcart`) or `barrel` — a container the holder census counts (`DW1018`), so
    it is never among the defaults and stands only where named. `things=()`
    places `at` alone. A placement in `at` on a column that is not free (an
    opening, a doorway, a bracket, the clear way) is refused. Returns every
    placement made, `(kind, row, side, x, z)`."""
    free, rows, cell, c0, c1, seed = frame["free"], frame["rows"], frame["cell"], frame["c0"], frame["c1"], frame["seed"]
    low, high = ACROSS[frame["axis"]]
    w = walk_of(m)
    grass = "minecraft:grass_block[snowy=false]"
    placed: list[tuple[str, int, str, int, int]] = []
    taken: set[tuple[int, int]] = set()

    def put(kind: str, a: int, side: str) -> bool:
        b = c0 - 1 if side == "low" else c1 + 1
        x, z = cell(a, b)
        nx, nz = cell(a + 1, b)
        if (x, z) not in free or (x, z) in taken:
            return False
        nxt = (nx, nz) in free and (nx, nz) not in taken
        taken.add((x, z))
        if kind in ("bed", "bench", "cart") and nxt:
            taken.add((nx, nz))
        placed.append((kind, a, side, x, z))
        draw(kind, b, x, z, nx, nz, nxt, a)
        return True

    def draw(kind, b, x, z, nx, nz, nxt, a) -> None:
        if kind == "lamp":
            lamp(m, x, z)
        elif kind == "bed":
            m.set(x, w - 1, z, grass)
            m.set(x, w, z, flowers[(seed + a) % len(flowers)])
            if nxt:
                m.set(nx, w - 1, nz, grass)
                m.set(nx, w, nz, "minecraft:short_grass")
        elif kind == "bench":
            face = low if b < c0 else high
            m.set(x, w, z, stairs(bench, face))
            if nxt:
                m.set(nx, w, nz, stairs(bench, face))
        elif kind == "barrel":
            m.set(x, w, z, "minecraft:barrel[facing=up,open=false]")
        elif kind == "cart" and nxt:
            handcart(m, x, z, nx, nz, w, cart, frame["axis"], cart_load)

    for kind, a, side in at:
        if side not in ("low", "high"):
            sys.exit(f"town.verge_life: side `{side}` is not one; a verge side is \"low\" or \"high\" across the way")
        if not put(kind, a, side):
            sys.exit(f"town.verge_life: `{kind}` at row {a} on the {side} verge stands on a column that is not free verge (an opening, a doorway, a bracket, the clear way, or taken); choose another row")
    if not things:
        return placed
    i = 0
    for a in range(rows[0] + 2, rows[-1] - 1, every):
        for side, b in enumerate((c0 - 1, c1 + 1)):
            x, z = cell(a, b)
            if (x, z) not in free or (x, z) in taken:
                continue
            kind = things[(i + seed + side * 3) % len(things)]
            i += 1
            put(kind, a, ("low", "high")[side])
    return placed


STALL_COLOURS = ("red", "yellow", "blue", "green")


def market(m, frame: dict, colours: tuple = STALL_COLOURS, wood: str = "minecraft:spruce",
           goods: tuple = (("minecraft:potted_red_tulip", "minecraft:potted_dandelion"), ("minecraft:hay_block", "minecraft:melon"))) -> None:
    """A market on a street's verges (a `verge` for `stretch`): stalls along
    both verges, a counter of `wood` slabs on the inner column under a
    striped awning three courses up in the stall's colour, fence posts at the
    stall's ends, goods on the counter, and lamps at the market's ends."""
    free, rows, cols, cell, c0, c1 = frame["free"], frame["rows"], frame["cols"], frame["cell"], frame["c0"], frame["c1"]
    w = walk_of(m)
    a0, a1 = rows[0], rows[-1]
    fb = {cell_b for cell_b in cols if any(cell(a, cell_b) in free for a in rows)}
    left = sorted(b for b in fb if b < c0)
    right = sorted(b for b in fb if b > c1)
    load = "x" if frame["axis"] == "x" else "z"
    n = 0
    for side in (left, right):
        if not side:
            continue
        inner = max(side) if side is left else min(side)
        for a in range(a0 + 2, a1 - 1, 4):
            span = [aa for aa in range(a, a + 3) if all(cell(aa, b) in free for b in side)]
            if len(span) < 2:
                continue
            col = colours[n % len(colours)]
            n += 1
            for aa in span:
                x, z = cell(aa, inner)
                m.set(x, w, z, slab(wood, "top"))
                for b in side:
                    x, z = cell(aa, b)
                    m.set(x, w + 3, z, f"minecraft:{col}_wool" if aa % 2 else "minecraft:white_wool")
                if w + 3 >= getattr(m, "ey", w + 5) - 1 and hasattr(m, "show"):
                    m.show("up")  # the awning on the frame's top course, in the open air (`DW0885`)
            for aa in (span[0], span[-1]):
                for y in (w, w + 1, w + 2):
                    for b in side:
                        if b != inner:
                            x, z = cell(aa, b)
                            m.set(x, y, z, fence(wood))
            pots, loads = goods
            x, z = cell(span[0], inner)
            m.set(x, w + 1, z, pots[0] if n % 2 else pots[1])
            x, z = cell(span[-1], inner)
            heavy = loads[0] if n % 3 else loads[1]
            m.set(x, w + 1, z, f"{heavy}[axis={load}]" if heavy.endswith("hay_block") else heavy)
    for b in left[:1] + right[-1:]:
        for a in (a0 + 2, a1 - 2):
            x, z = cell(a, b)
            if (x, z) in free and m.get(x, w, z) is None:
                lamp(m, x, z)


def flight(m, place, roles: dict | None = None, step: str = "minecraft:stone_brick", lamp_side: str = "low",
           flower: str = "minecraft:cornflower") -> None:
    """A flight carrying a way down two courses in half-steps: a landing of
    `kerb` setts at the opening above (the seam whose `rise` is above zero),
    two stairs of `step` climbing toward it, the foot paved as a way, and a
    lamp at the foot landing on the `lamp_side` edge (`"low"` or `"high"`),
    a bed with `flower` on the other. Every step below the landing is a stair
    block, never a full block in the walk plane's clear width."""
    r = dict(WAY_ROLES, **(roles or {}))
    up = up_seam(place)
    if up is not None:
        climbs_itself(m, up)
    face = up["seam"]["face"] if up else "north"
    axis = "z" if face in ("north", "south") else "x"
    rows, cols, cell = way_frame(place, axis)
    if face in ("south", "east"):
        rows = rows[::-1]
    w = walk_of(m)
    b0, b1 = cols[0], cols[-1]
    for b in cols:
        top, half_hi, mid, half_lo = rows[0], rows[1], rows[2], rows[3]
        for a, lo_, hi_ in ((top, r["kerb"], r["kerb"]), (half_hi, r["kerb"], stairs(step, face))):
            x, z = cell(a, b)
            m.set(x, w, z, lo_)
            m.set(x, w + 1, z, hi_)
        for a in (mid, half_lo):
            x, z = cell(a, b)
            m.set(x, w, z, stairs(step, face))
        for a in rows[4:]:
            x, z = cell(a, b)
            m.set(x, w - 1, z, r["verge"] if b in (b0, b1) else r["paving"])
        for a in rows[:4]:
            x, z = cell(a, b)
            m.set(x, w - 1, z, r["kerb"])
    keep = seam_cols(place)
    lb, bb = (b0, b1) if lamp_side == "low" else (b1, b0)
    lx, lz = cell(rows[4], lb)
    if (lx, lz) not in keep:
        lamp(m, lx, lz)
    fx, fz = cell(rows[4], bb)
    if (fx, fz) not in keep:
        m.set(fx, w - 1, fz, "minecraft:grass_block[snowy=false]")
        m.set(fx, w, fz, flower)


LANE_THINGS = ("cart", "bench")  # no container among the defaults (`VERGE_THINGS`)


def lane(m, place, axis: str | None = None, edge: str = "high", roles: dict | None = None, every: int = 5,
         things: tuple = LANE_THINGS, wood: str = "minecraft:spruce") -> None:
    """A side street: paved as a way (`pave_way`), a lamp and then one of
    `things` in turn (`cart` a cart on two cells along the edge, `town.handcart`,
    where both are clear; `bench` a stair its back to the edge,
    `barrel` — a container the census counts, so only where named) every
    `every` cells along its `edge` column (`"high"` or `"low"` across the
    way), clear of every opening. The clear width is the way's other
    columns. `axis` is the plan's (`place.way`) where it declares a way."""
    axis, _, _ = handed_way(place, axis, None, "lane")
    rows, cols, cell = way_frame(place, axis)
    (x0, _, z0), (x1, _, z1) = place.space
    pave_way(m, x0, x1, z0, z1, axis, roles)
    keep = seam_cols(place)
    w = walk_of(m)
    b = cols[-1] if edge == "high" else cols[0]
    face = ACROSS[axis][1 if edge == "high" else 0]
    n = 0
    for a in range(rows[0] + 2, rows[-1] - 1, every):
        x, z = cell(a, b)
        if (x, z) in keep or m.get(x, w, z) is not None:
            continue
        if n % 2 == 0:
            lamp(m, x, z)
        else:
            kind = things[(n // 2) % len(things)]
            if kind == "cart":
                nx, nz = cell(a + 1, b)
                if (nx, nz) not in keep and m.get(nx, w, nz) is None:
                    handcart(m, x, z, nx, nz, w, wood, axis)
            elif kind == "barrel":
                m.set(x, w, z, "minecraft:barrel[facing=up,open=false]")
            elif kind == "bench":
                m.set(x, w, z, stairs(wood, face))
        n += 1


def ramp(m, place, axis: str | None = None, roles: dict | None = None, step: str = "minecraft:stone_brick", fill: str = "road",
         landing: str | None = None, flat: int = 2) -> bool:
    """A way that climbs to the opening above (the seam whose `rise` is above
    zero) across its whole width: `flat` rows at grade, then a stair of
    `step`, then a landing of `landing` (`kerb` by default), and so on up the
    rise, the ground under each filled with `fill`. `axis` is the plan's
    (`place.way`) where it declares a way. Returns False (and draws nothing)
    when no seam rises."""
    r = dict(WAY_ROLES, **(roles or {}))
    up = up_seam(place)
    if up is None:
        return False
    climbs_itself(m, up)
    axis, _, _ = handed_way(place, axis, None, "ramp")
    rows, cols, cell = way_frame(place, axis)
    face = up["seam"]["face"]
    top_end = {"north": rows[0], "west": rows[0], "south": rows[-1], "east": rows[-1]}[face]
    order = rows if top_end == rows[-1] else rows[::-1]
    rise = up["seam"]["rise"]
    g = walk_of(m) - 1
    for i, a in enumerate(order):
        if i < flat:
            h, is_step = 0, False
        else:
            j = i - flat
            h, is_step = min(j // 2 + 1, rise), (j % 2 == 0 and j // 2 + 1 <= rise)
        for b in cols:
            x, z = cell(a, b)
            m.set(x, g, z, fill)
            for y in range(g + 1, g + h):
                m.set(x, y, z, fill)
            if h:
                m.set(x, g + h, z, stairs(step, face) if is_step else (landing or r["kerb"]))
    return True


def rail(m, place, axis: str, edge: str, seed: int, block, lamp_at=None, gap: float = 0.0, special=None) -> None:
    """A verge rail along a way's `edge` column (`"high"` or `"low"`): `block`
    (one state, or a list the seed chooses among) on every cell but the two
    ends and any opening's, broken where `hsh(seed, i, 13)` falls past
    `1 - gap`; a lamp wherever `lamp_at(i, n)` holds; and first, wherever
    `special(m, i, n, x, z)` draws something and returns True, that instead
    (a shrine, a bed of flowers: the design's)."""
    rows, cols, cell = way_frame(place, axis)
    b = cols[-1] if edge == "high" else cols[0]
    keep = seam_cols(place)
    w = walk_of(m)
    n = len(rows)
    for i, a in enumerate(rows):
        x, z = cell(a, b)
        if (x, z) in keep or i in (0, n - 1):
            continue
        if special is not None and special(m, i, n, x, z):
            continue
        if lamp_at is not None and lamp_at(i, n):
            lamp(m, x, z)
        elif hsh(seed, i, 13) < 1.0 - gap:
            pick = block if isinstance(block, str) else block[int(hsh(seed, i, 17) * len(block))]
            m.set(x, w, z, pick)


def square(m, place, roles: dict | None = None, path: str | None = None, centre: tuple[int, int] | None = None) -> tuple[int, int]:
    """A square paved in `paving` with a `verge` border, and a path of `path`
    (the `kennel` role by default) from every way in to its centre (the play
    space's middle unless stated). What stands at the centre and round it —
    a well, trees, a stall — is the design's. Returns the centre."""
    r = dict(WAY_ROLES, **(roles or {}))
    (x0, _, z0), (x1, _, z1) = place.space
    g = walk_of(m) - 1
    for x in range(x0, x1 + 1):
        for z in range(z0, z1 + 1):
            m.set(x, g, z, r["verge"] if x in (x0, x1) or z in (z0, z1) else r["paving"])
    cx, cz = centre if centre is not None else ((x0 + x1) // 2, (z0 + z1) // 2)
    p = path or r["kennel"]
    for s in place.seams:
        lo, hi = [b.value for b in s["lo"]], [b.value for b in s["hi"]]
        mx, mz = (lo[0] + hi[0] - 1) // 2, (lo[2] + hi[2] - 1) // 2
        mx, mz = min(max(mx, x0 + 1), x1 - 1), min(max(mz, z0 + 1), z1 - 1)
        for x in range(min(mx, cx), max(mx, cx) + 1):
            m.set(x, g, mz, p)
        for z in range(min(mz, cz), max(mz, cz) + 1):
            m.set(cx, g, z, p)
    return cx, cz


# ════════════════════════════════════════════════════════════════════════
# A lot's edges


def clip_beyond(m, x0: int, x1: int, z0: int, z1: int, above: int = 0) -> None:
    """Delete every cell above course `above` outside the columns x0..x1,
    z0..z1: an eave or a rake that overhangs the play space stops at its edge."""
    for c in [c for c in m.v if c[1] > above and not (x0 <= c[0] <= x1 and z0 <= c[2] <= z1)]:
        del m.v[c]


def clip_unwalled_edge(m, x0: int, x1: int, z0: int, z1: int, wall_roles, from_y: int) -> None:
    """On the edge columns of x0..x1, z0..z1 whose walk-plane cell is not one
    of `wall_roles`, delete every cell from course `from_y` up: an eave over a
    column with no wall under it would stand over air beside the neighbour's
    ground."""
    w = walk_of(m)
    for x in range(x0, x1 + 1):
        for z in range(z0, z1 + 1):
            if x not in (x0, x1) and z not in (z0, z1):
                continue
            if m.get(x, w, z) not in wall_roles:
                for c in [c for c in m.v if c[0] == x and c[2] == z and c[1] >= from_y]:
                    del m.v[c]


def close_edge_gaps(m, x0: int, x1: int, z0: int, z1: int, role) -> None:
    """On the edge columns of x0..x1, z0..z1, fill the air between the lowest
    and the highest solid cell above the ground with `role` (a role, or a
    function of `(x, z)` giving one), so no edge column stands over air."""
    for x in range(x0, x1 + 1):
        for z in range(z0, z1 + 1):
            if x not in (x0, x1) and z not in (z0, z1):
                continue
            ys = sorted(y for (cx, y, cz) in m.v if cx == x and cz == z and y > 0 and m.v[(cx, y, cz)] != AIR)
            if len(ys) < 2:
                continue
            for y in range(ys[0], ys[-1]):
                if (x, y, z) not in m.v or m.v[(x, y, z)] == AIR:
                    m.set(x, y, z, role(x, z) if callable(role) else role)


def street_seam(place, face: str):
    """A lot's street edge on `face`, read off its allocation: `(fronted,
    (edge, rows, door_top, air))`. `edge` is the edge's column (east or west)
    or row (north or south); `rows` the coordinates along it the seam leaves
    open (the doorway); `door_top` the opening's top course; `air` the course
    the street's air rises to (the street's void on the ring beside the edge;
    the seam's own top where none is handed). Fronted means the seam is cut
    to a doorway span narrower than the edge (a contact with an `extent`):
    the lot then stands its building's front on the edge. `(False, None)`
    where the lot has no seam on `face`."""
    a = place.a
    lo, hi = place.space
    for c in a["seams"]:
        if c.get("face") != face:
            continue
        (x0, y0, z0), (x1, y1, z1) = c["cells"]
        air = y1
        for v in a.get("voids", []):
            (vx0, vy0, vz0), (vx1, vy1, vz1) = v["cells"]
            if face in ("east", "west"):
                beside = vx0 == vx1 == x0 + (1 if face == "east" else -1)
            else:
                beside = vz0 == vz1 == z0 + (1 if face == "south" else -1)
            if beside:
                air = vy1
        if face in ("east", "west"):
            rows, span, edge = set(range(z0, z1 + 1)), hi[2] - lo[2] + 1, x0
        else:
            rows, span, edge = set(range(x0, x1 + 1)), hi[0] - lo[0] + 1, z0
        return len(rows) < span, (edge, rows, y1, air)
    return False, None


def street_front(m, edge: int, lo: int, hi: int, door_rows, air: int, roles: dict, door_h: int = 3,
                 solid=None, inward: int | None = None, axis: str = "x") -> None:
    """Close a lot's street edge as the street's air above it asks: up to
    course `air` nothing on the edge is open but the doorway, and nothing
    stands proud of it above.

    The edge is the column `x = edge` over rows z lo..hi (`axis="x"`, an east
    or west face) or the row `z = edge` over x lo..hi (`axis="z"`, a north or
    south face). Where the lot's building already stands on the edge (its
    walk-plane cell and the one over it filled: a flower or a tuft alone is
    not a building) the building's front carries the closure: any open cell
    of it up to `air` is filled with `roles["wall"]` (a window is a pane:
    solid), and the doorway's head up to `air` with `roles["lintel"]`. Where
    nothing stands, a yard wall goes up: `roles["wall"]` to course `air - 1`
    and `roles["coping"]` on top, every cell above it cleared, so no eave
    stands over the yard wall. The doorway (`door_rows`) is left open from the
    walk plane to course `door_h`. Last, no cell of the edge is left standing
    over air. `solid` tells a filled cell from an open one (anything but air
    by default).

    With `inward` (+1 or -1, toward the lot across the edge) the edge's part
    columns are handed back too: no part claims the edge (it is the front,
    closed but for the doorway), and the column just inside the doorway is the
    room the street's way opens into, never a part's."""
    is_solid = solid or (lambda r: r is not None and r != AIR)
    w = walk_of(m)
    if axis == "x":
        at = lambda u: (edge, u)  # noqa: E731
        inside = lambda u: (edge + inward, u)  # noqa: E731
    else:
        at = lambda u: (u, edge)  # noqa: E731
        inside = lambda u: (u, edge + inward)  # noqa: E731
    for u in range(lo, hi + 1):
        x, z = at(u)
        if u in door_rows:
            for y in range(w, door_h + 1):
                m.set(x, y, z, AIR)
            for y in range(door_h + 1, air + 1):
                if not is_solid(m.get(x, y, z)):
                    m.set(x, y, z, roles["lintel"])
            continue
        if is_solid(m.get(x, w, z)) and is_solid(m.get(x, w + 1, z)):
            for y in range(w + 1, air + 1):
                if not is_solid(m.get(x, y, z)):
                    m.set(x, y, z, roles["wall"])
            continue
        for y in range(w, air):
            m.set(x, y, z, roles["wall"])
        m.set(x, air, z, roles["coping"])
        for c in [c for c in m.v if c[0] == x and c[2] == z and c[1] > air]:
            del m.v[c]
    for u in range(lo, hi + 1):
        x, z = at(u)
        ys = [y for (cx, y, cz) in m.v if cx == x and cz == z and y > 0 and is_solid(m.v[(cx, y, cz)])]
        if not ys:
            continue
        start = door_h + 1 if u in door_rows else w
        for y in range(start, max(ys)):
            if not is_solid(m.get(x, y, z)):
                m.set(x, y, z, roles["lintel"] if u in door_rows else roles["wall"])
    if inward is not None and hasattr(m, "region"):
        for u in range(lo, hi + 1):
            m.region.pop(at(u), None)
            if u in door_rows:
                m.region.pop(inside(u), None)


# ════════════════════════════════════════════════════════════════════════
# Small buildings and furnishing


def ring(x0: int, x1: int, z0: int, z1: int) -> list[tuple[int, int]]:
    """The columns of a rectangle's outline, inclusive."""
    return [(x, z) for x in range(x0, x1 + 1) for z in (z0, z1)] + [(x, z) for z in range(z0 + 1, z1) for x in (x0, x1)]


def window(m, x: int, z: int, glass: str, y0: int | None = None, h: int = 2) -> None:
    """A window `h` courses of `glass` in a wall's column, from `y0` (the
    course over the walk plane by default)."""
    y0 = walk_of(m) + 1 if y0 is None else y0
    for y in range(y0, y0 + h):
        m.set(x, y, z, glass)


def flower_bed(m, cells, flowers, soil: str = "minecraft:grass_block[snowy=false]") -> None:
    """A bed of flowers: `soil` in the ground course, a bloom from `flowers`
    on each cell in turn."""
    w = walk_of(m)
    for i, (x, z) in enumerate(cells):
        m.set(x, w - 1, z, soil)
        m.set(x, w, z, flowers[i % len(flowers)])


def post_shed(m, box, plate: int, posts_x, roles: dict, back_height: int = 0, back: str = "north") -> None:
    """An open shed on posts. `box` = (x0, x1, z0, z1) its post lines
    inclusive; posts at every x of `posts_x` (and the four corners) on both
    long sides up to course `plate - 1`; a beam along each long side at
    `plate`; the `back` side boarded from the walk plane to `back_height`
    between its corner posts. `roles` maps `post`, `beam` and `back`. The
    roof is the caller's (`Model.roof`), since its pitch and cover are."""
    x0, x1, z0, z1 = box
    w = walk_of(m)
    for x in sorted(set(posts_x) | {x0, x1}):
        for z in (z0, z1):
            for y in range(w, plate):
                m.set(x, y, z, roles["post"])
    for z in (z0, z1):
        for x in range(x0, x1 + 1):
            m.set(x, plate, z, roles["beam"])
    if back_height:
        bz = z0 if back == "north" else z1
        for x in range(x0 + 1, x1):
            for y in range(w, back_height + 1):
                m.set(x, y, bz, roles["back"])


def loft(m, x0: int, x1: int, z0: int, z1: int, floor_y: int, ladder, roles: dict) -> None:
    """A loft floor over x0..x1, z0..z1 at course `floor_y`, and a ladder at
    column `ladder` = (x, z) from the walk plane up to it. `roles` maps
    `floor` and `ladder` (the ladder's role carries its facing)."""
    for x in range(x0, x1 + 1):
        for z in range(z0, z1 + 1):
            m.set(x, floor_y, z, roles["floor"])
    lx, lz = ladder
    for y in range(walk_of(m), floor_y + 1):
        m.set(lx, y, lz, roles["ladder"])


# ════════════════════════════════════════════════════════════════════════
# Boats

HULL_ROLES = ("hull", "keel", "deck", "gunwale", "gunwale_hi", "stem", "stem_post",
              "flare_n", "flare_s", "flare_e", "flare_w")


def hull(m, axis: str, s0: int, c0: int, widths, waterline: int, roles: dict | None = None, toward: int = 1,
         cols: set | None = None, shoulder: bool = True) -> None:
    """A boat's hull in half steps, laid along `axis` ("x" or "z") from its
    stern station `s0` toward its bow (`toward` = +1 or -1), centred on the
    cross line `c0`. `widths[i]` is the half-beam at station i (0 at the
    stem). In section: planks a cell in at `waterline`, the keel under them,
    upturned stairs flaring out a course above, a slab gunwale on top; the
    sheer rises toward both ends, a strake more at the bow's shoulder (left
    out with `shoulder=False`), the stem a post rising past the gunwale.
    `roles` renames any of `HULL_ROLES`; `cols` collects the hull's columns."""
    r = {k: k for k in HULL_ROLES}
    r.update(roles or {})
    deck_y = waterline + 1

    def at(i, c):
        s = s0 + toward * i
        return (s, c) if axis == "x" else (c, s)

    n = len(widths)
    for i, wd in enumerate(widths):
        for c in range(c0 - wd, c0 + wd + 1):
            x, z = at(i, c)
            if cols is not None:
                cols.add((x, z))
            off = c - c0
            side = abs(off) == wd and wd > 0
            if abs(off) <= max(0, wd - 1):
                m.set(x, waterline, z, r["hull"])
            if off == 0:
                m.set(x, waterline - 1, z, r["keel"])
            if wd == 0 or (side and i == n - 1):
                m.set(x, deck_y, z, r["hull"])
                m.set(x, deck_y + 1, z, r["stem_post"])
                m.set(x, deck_y + 2, z, r["stem"])
            elif side and i == n - 2 and shoulder:
                m.set(x, deck_y, z, r["hull"])
                m.set(x, deck_y + 1, z, r["hull"])
                m.set(x, deck_y + 2, z, r["gunwale"])
            elif side:
                if axis == "x":
                    face = "south" if off < 0 else "north"
                else:
                    face = "east" if off < 0 else "west"
                m.set(x, deck_y, z, r["flare_" + face[0]])
                m.set(x, deck_y + 1, z, r["gunwale_hi"] if i <= 1 or i >= n - 3 else r["gunwale"])
            elif i == 0:
                m.set(x, deck_y, z, r["hull"])
                m.set(x, deck_y + 1, z, r["gunwale_hi"])
            else:
                m.set(x, deck_y, z, r["deck"])


def mast(m, axis: str, s: int, c0: int, foot_y: int, top_y: int, roles: dict) -> None:
    """A mast at station `s` on the cross line `c0` from course `foot_y` to
    `top_y`, a yard two each side a course under the top, and the sail furled
    along it: a thin roll lying on the yard, never a block hung under it.
    `roles` maps `mast`, `yard_x`, `yard_z` (a spar along x or z) and `sail`."""
    x, z = (s, c0) if axis == "x" else (c0, s)
    for y in range(foot_y, top_y + 1):
        m.set(x, y, z, roles["mast"])
    for k in (-2, -1, 1, 2):
        xx, zz = (x + k, z) if axis == "z" else (x, z + k)
        m.set(xx, top_y - 1, zz, roles["yard_z" if axis == "x" else "yard_x"])
        m.set(xx, top_y, zz, roles["sail"])


def hull_on_stocks(m, axis: str, stern: int, bow: int, c0: int, keel_y: int, framed: int, stocks, roles: dict | None = None) -> None:
    """A boat building on its stocks: bow at station `bow`, the transom's foot
    at `stern` (stern < bow along `axis`), centred on `c0`, keel at course
    `keel_y` on stocks at the stations `stocks`; the stem a narrow post on a
    solid foot, the planking drawn in to it (thin plates on the inner faces
    at the station behind it), the turn of the bilge upturned stairs, lapped
    planks amidships, the `framed` stations by the transom in bare frames, the
    transom raking out. Side roles carry the side's letter (`bilge_n`,
    `lap_s`; `_w`/`_e` along z). `roles` renames any role: stock, keel, stem,
    stem_post, stem_tip, strake, gunwale, gunwale_hi, rib, transom,
    transom_rake, bilge_<side>, lap_<side>, bow_plate_<side>."""
    r = dict(roles or {})
    R = lambda k: r.get(k, k)  # noqa: E731
    P = (lambda s, c, y: (s, y, c)) if axis == "x" else (lambda s, c, y: (c, y, s))

    def put(s, y, c, role):
        x, yy, z = P(s, c, y)
        m.set(x, yy, z, R(role))

    lo, hi = c0 - 1, c0 + 1
    side = (lambda c: "n" if c < c0 else "s") if axis == "x" else (lambda c: "w" if c < c0 else "e")
    for s in stocks:
        put(s, keel_y - 1, c0, "stock")
    for s in range(stern, bow):
        put(s, keel_y, c0, "keel")
    put(bow, keel_y, c0, "stem")
    for y in range(keel_y + 1, keel_y + 4):
        put(bow, y, c0, "stem_post")
    put(bow, keel_y + 4, c0, "stem_tip")
    for c in (lo, hi):
        for y in (keel_y + 1, keel_y + 2, keel_y + 3):
            put(bow - 1, y, c, "bow_plate_" + side(c))
        put(bow - 2, keel_y, c, "bilge_" + side(c))
        put(bow - 2, keel_y + 1, c, "strake")
        put(bow - 2, keel_y + 2, c, "gunwale_hi")
    for s in range(stern + 1 + framed, bow - 2):
        for c in (lo, hi):
            put(s, keel_y, c, "bilge_" + side(c))
            put(s, keel_y + 1, c, "lap_" + side(c))
            put(s, keel_y + 2, c, "gunwale")
    for s in range(stern + 1, stern + 1 + framed):
        for c in (lo, hi):
            put(s, keel_y, c, "bilge_" + side(c))
            if (s - stern) % 2 == 1:
                put(s, keel_y + 1, c, "rib")
            put(s, keel_y + 2, c, "rib")
    put(stern, keel_y + 1, c0, "transom")
    put(stern, keel_y + 1, lo, "bilge_" + side(lo))
    put(stern, keel_y + 1, hi, "bilge_" + side(hi))
    for c in range(lo, hi + 1):
        put(stern - 1, keel_y + 2, c, "transom_rake")
    put(stern - 1, keel_y + 3, c0, "gunwale")


# ════════════════════════════════════════════════════════════════════════
# Quays and basins


def slipway(m, cols, edge: int, run_to: int, surface: int, bed: int, quay_y: int, roles: dict, toward: int = 1) -> None:
    """A slipway off a quay into the water along z over the columns `cols`:
    row `edge - toward` a low slab on the quay's ground course `quay_y`; from
    row `edge` to `run_to` the ramp drops half a course a row (slabs
    alternating bottom and top) from a course under the quay, `bed` filled
    under each slab and water over it up to `surface`. `roles` maps `low`,
    `wet_lo`, `wet_hi` (waterlogged slabs), `fill` and `water`."""
    for x in cols:
        m.set(x, quay_y, edge - toward, roles["low"])
        m.set(x, quay_y, edge, AIR)
        for k, z in enumerate(range(edge, run_to + toward, toward)):
            y = surface - (k + 1) // 2
            for yy in range(bed, y):
                m.set(x, yy, z, roles["fill"])
            m.set(x, y, z, roles["wet_hi"] if k % 2 == 1 else roles["wet_lo"])
            for yy in range(y + 1, surface + 1):
                m.set(x, yy, z, roles["water"])


def pier(m, cols, z_from: int, z_to: int, quay_y: int, bed: int, roles: dict, kerb_x: int | None = None) -> None:
    """A pier out into a basin along z: `fill` from `bed` up under the quay
    course, the quay course paved, kerbed on its head row and on the column
    `kerb_x`. `roles` maps `fill`, `pave` and `kerb`."""
    for x in cols:
        for z in range(z_from, z_to + 1):
            for y in range(bed, quay_y):
                m.set(x, y, z, roles["fill"])
            m.set(x, quay_y, z, roles["kerb"] if z == z_to or x == kerb_x else roles["pave"])


def water_step(m, x: int, z: int, surface: int, bed: int, roles: dict) -> None:
    """A stair at the water's surface at (x, z), solid under it to `bed`, so a
    swimmer climbs out onto the quay. `roles` maps `step` and `fill`."""
    m.set(x, surface, z, roles["step"])
    for y in range(bed, surface):
        m.set(x, y, z, roles["fill"])


# ════════════════════════════════════════════════════════════════════════
# The plan's side: a front's doorway span, a sign station, what is seen


def front_contact(at: int, meets: int, first_row: int, width: int, height: int) -> dict:
    """A lot fronting a street meets it only at its doorway: the site-plan
    seam's `at`/`meets` moved to the doorway's first row (counted from 1
    along the lot's street face) and its `contact` cut to the doorway's
    `(width, height)`. Every other cell of the edge is the lot's building
    front or yard wall (`street_front` closes it)."""
    return {"at": at + first_row - 1, "meets": meets + first_row - 1, "contact": {"extent": [width, height]}}


def sign_station(street: str, lot: str, text: str, note: str | None = None) -> tuple[dict, dict]:
    """A trade's hanging sign on the street's own bracket: the station the
    street's layout node carries (`anchor/sign-<lot stem>`, a point) and the
    stage-5 `signs` entry that letters it, front and back. The street's
    generator hangs the sign and marks its cell (`bracket`)."""
    stem = lot.removeprefix("node/")
    anchor = f"anchor/sign-{stem}"
    station = {"anchor": anchor, "kind": "point",
               "note": note or f"the {text.lower()}'s hanging sign, on the street's bracket before its doorway"}
    entry = {"anchor": anchor, "back": ["", text], "front": ["", text], "id": f"sign/{stem}"}
    return station, entry


def seen_reach(view_distance: int = 10) -> int:
    """How far past the walk region a player sees: `16 × (view_distance + 1)`
    blocks (the served chunks plus the chunk the player stands in;
    `world.view_distance`, the engine's floor 10 where undeclared)."""
    return 16 * (view_distance + 1)


def seen_region(walk: tuple[int, int, int, int], view_distance: int = 10) -> tuple[int, int, int, int]:
    """The ground the fill must lay so the world goes on past the walk region
    `(x0, x1, z0, z1)` (every box grown by the boundary's margin) as far as a
    player at its edge sees: the walk region grown by `seen_reach` on every
    side, inclusive."""
    r = seen_reach(view_distance)
    x0, x1, z0, z1 = walk
    return x0 - r, x1 + r, z0 - r, z1 + r
