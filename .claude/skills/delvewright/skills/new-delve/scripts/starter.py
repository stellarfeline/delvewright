#!/usr/bin/env python3
"""The starter generator for ONE place: an empty-but-correct program, read off its allocation.

`scripts/handout.py` copies this file beside every handout, with the place's
allocation saved next to it, so a place's author starts from a program that
already passes every judgement `delvec detail --check` makes of the place
itself (the commit's battery, which the check also runs, may still name what
the whole's story asks of the place) and builds the place into it — never
from research. Run it as the handout prints it:

    "$DELVEWRIGHT_PYTHON" starter.py --allocation allocation.json --out <campaign>/programs/<stem>.json

Run bare, it runs no binary and names no path: it reads the allocation
`delvec allocation <campaign> <place>` printed and writes the program. What it
emits, for any allocation the engine hands:

- the ground: a ground place's ground drawn down to the bottom of its claim
  in the blocks the whole hands each column (`ground.columns`, the terrain's
  own, every property written), the ring's run from its nearest column; an
  aloft place's one floor course under its walk plane and nothing beneath;
- a wall on every cell of the one-cell ring, from the walk plane up — a cell a
  neighbour or the whole owns is a void the verb replaces, so painting it is
  harmless and nothing depends on it — except on a side the place declares
  open, whose ring run (its two corners excluded) is left open from the walk
  plane up with the floor course running across it, claimed as the region a
  `walk` edge to `exterior` goes `via`;
- a lid over a roofed place (at the plan's `roof/lid-y` where it declares a
  roof, at the frame's top course otherwise), and the declared roof zone above
  it claimed as a region no body walks;
- every seam answered: its handed cells claimed as the region its contract
  edge goes `via`, or — where this place owns the bar — filled with the bar's
  block inside a `barred` edge over exactly those cells;
- every owed name answered by a `mark` of its stem on walkable floor: `spawn`
  with the `entry` role, `unlock-<edge>` beside its bar, the rest along the
  room's middle;
- on a plot, every handed part laid as a region of exactly its declared
  cells, named for the part with `part`: the columns beside every opening,
  ladder, landing and gate, and the middle of every open side, stay the room; a walked part is a space
  (a walk edge joins every two walked spaces that touch); a part seen and
  never entered is an out-of-walk region over the floor that no edge reaches,
  laid only where the walked floor stays one piece — what keeps a body off it
  (a hedge, a crop, a fence) is yours to build. Where each part stands is this starter's
  choice; its count is the plan's;
- a sconce every four cells of wall a course above the floor, and on a roofed
  place a lamp every four cells of the lid, so the light probe has something
  to measure.

Every handed value it uses is declared as a `handed/…` parameter with the
allocation's value as its default, and the body reads it only through that
parameter (`references/detail.md`, *The program's contract*). A number that is
not handed — where a mark stands, how far apart the lamps are — is this
starter's choice and yours to change.

The building is yours, and it is drawn, not edited in: a generator at
`<campaign>/design/generator/<stem>.py` imports `Model` and `run` from this
file (`$DELVEWRIGHT_SKILL/scripts`, never a copy), draws the place as cells and
names each part's columns; `run(build)` binds the handing as above and claims
every cell from what the engine says of the drawing (`delvec grammar survey`).
The section *A building drawn as cells* below is the template. Re-run it and
check after every edit:
`delvec --prefabs "$DELVEWRIGHT_PREFABS" detail --check <campaign> <place>`.

Standard library only.
"""

from __future__ import annotations

import argparse
import json
import sys

LAMP_PERIOD = 4  # one sconce (and one ceiling lamp) in four cells; yours to change

# ── BUILD HERE: the palette. Every role below is a name the rules paint; bind
# each to the material the design names (references/new-pieces.md step 2 —
# measure a block's colour, never guess it). A role the whole's palette names
# (`role/<stem>` in detail-plan.json) is rebound to it by `delvec detail`.
PALETTE = {
    "floor": "minecraft:stone_bricks",
    "wall": "minecraft:stone_bricks",
    "lid": "minecraft:spruce_planks",
    "lamp": "minecraft:sea_lantern",
}


# ---- the grammar's expression and operation vocabulary (grammar.md §2) ----


def num(v: int) -> dict:
    return {"expr": "int", "value": v}


def par(name: str) -> dict:
    return {"expr": "param", "name": name}


def arith(lhs: dict, op: str, rhs: dict) -> dict:
    return {"expr": "arith", "lhs": lhs, "op": op, "rhs": rhs}


def fill(role: str) -> dict:
    return {"material": {"role": role}, "op": "fill"}


VOID = {"op": "void"}


def claim(region: str, body: dict) -> dict:
    return {"body": body, "op": "claim", "region": region}


# ---- a value the program reads: a handed parameter, or this starter's literal ----


class Bound:
    """A coordinate on one axis: its value now, and how the program spells it."""

    def __init__(self, value: int, expr: dict, handed: bool) -> None:
        self.value, self.expr, self.handed = value, expr, handed

    def plus(self, n: int) -> "Bound":
        if n == 0:
            return self
        if self.expr.get("expr") == "int":
            return lit(self.value + n)
        return Bound(self.value + n, arith(self.expr, "add" if n > 0 else "sub", num(abs(n))), self.handed)


def lit(v: int) -> Bound:
    return Bound(v, num(v), False)


class Handed:
    """The handed values this program declares — each read only through its parameter."""

    def __init__(self) -> None:
        self.params: dict[str, int] = {}

    def __call__(self, name: str, value: int) -> Bound:
        key = f"handed/{name}"
        self.params[key] = value
        return Bound(value, par(key), True)


# ---- the place, read off the allocation ----


FACES = {"west": (0, -1), "east": (0, 1), "down": (1, -1), "up": (1, 1), "north": (2, -1), "south": (2, 1)}


class Place:
    def __init__(self, a: dict) -> None:
        self.a = a
        self.h = Handed()
        self.n = list(a["extent"])
        self.ex, self.ey, self.ez = self.n
        self.space = a["space"]
        self.datum = self.h("datum-y", a["datum_y"])
        self.aloft = isinstance(a["ground"].get("base"), dict)
        # The ground the whole hands each column under the plot (`ground.
        # columns`): its blocks bottom first up to the terrain's top or the
        # floor course, whichever is lower. The ground course is painted in
        # them, each a palette role of its own (`ground-<n>`, in block order);
        # an author's drawing over it is the author's.
        d = a["datum_y"]
        self.ground_cols: dict[tuple[int, int], tuple[int, list[str]]] = {}
        for c in a["ground"].get("columns", []) or []:
            blocks = [b for b in c["blocks"]]
            if not blocks:
                continue
            top = min(c["top"], d - 1)
            self.ground_cols[(c["x"], c["z"])] = (top - len(blocks) + 1, blocks)
        kinds = sorted({b for _, bs in self.ground_cols.values() for b in bs if b != "minecraft:air"})
        self.ground_roles: dict[str, str] = {b: f"ground-{i}" for i, b in enumerate(kinds)}
        # The way this place is, as the plan declares it (`way`: axis, clear
        # width, clear columns piece-local), or None: what a street's helpers
        # draw the run and the verge from, never retyped.
        self.way = a.get("way")
        roof = a.get("roof")
        self.roofed = roof is not None or a.get("brief", {}).get("kind") == "roofed"
        if roof is not None:
            self.lid = self.h("roof/lid-y", roof["lid_y"])
        elif self.roofed:
            self.lid = lit(self.ey - 1)
        else:
            self.lid = None
        # The walls stand on the cells just outside the play space, from the
        # walk plane to the lid — or, open to the sky, to the top of the play
        # space. Outside them (the eaves the plan reserves) is air.
        self.wall_top = self.lid if self.lid is not None else lit(self.space[1][1] + 1)
        self.seams = []
        for s in a["seams"]:
            stem = s["edge"].removeprefix("edge/")
            (x0, y0, z0), (x1, y1, z1) = s["cells"]
            lo = [self.h(f"seam/{stem}/{k}", v) for k, v in (("x0", x0), ("y0", y0), ("z0", z0))]
            hi = [self.h(f"seam/{stem}/{k}", v).plus(1) for k, v in (("x1", x1), ("y1", y1), ("z1", z1))]
            axis, sign = FACES[s["face"]]
            bar = None
            if s.get("bar") is not None:
                (bx0, by0, bz0), (bx1, by1, bz1) = s["bar"]
                bar = ([bx0, by0, bz0], [bx1 + 1, by1 + 1, bz1 + 1])
            seam = {"stem": stem, "lo": lo, "hi": hi, "seam": s, "bar": bar, "axis": axis, "approach": None}
            seam["approach"] = self.approach(seam, sign)
            self.seams.append(seam)
        # Every side the place declares open (spec-0123 §5.3): its ring run at
        # the walk plane, corners excluded, piece-local and inclusive.
        # A side whose every ring cell at the walk plane, inside the frame, is
        # a seam's opening is the seams' already: it is not declared again
        # (it would be a way out with no cell of its own), and over the
        # openings its ring is the place's open air, never a wall — a wall
        # there stands in the open air over a stair or a front (`DW0885`).
        self.sides = []
        self.covered: list[dict] = []
        d0 = a["datum_y"]
        for s in a.get("sides", []) or []:
            (sx0, _, sz0), (sx1, _, sz1) = s["cells"]
            side = {"face": s["face"], "lo": (sx0, sz0), "hi": (sx1, sz1)}
            own = [(x, z) for x in range(sx0, sx1 + 1) for z in range(sz0, sz1 + 1)
                   if 0 <= x < self.ex and 0 <= z < self.ez
                   and not any(all(se["lo"][i].value <= c[i] < se["hi"][i].value for i in range(3))
                               for se in self.seams for c in [(x, d0, z)])]
            if any(se["seam"]["face"] == s["face"] and se["lo"][1].value > d0 for se in self.seams):
                # the side a raised opening stands on is the neighbour's above
                # the walk plane (its void) and the pillar's below it: nothing
                # of it is a way out, so it is not declared
                continue
            (self.sides if own else self.covered).append(side)
        self.scenery = a.get("brief", {}).get("reached") is False
        # A station of kind `gate` is a volume that seals and clears, so it is
        # answered by a region, never a mark: a grille of bars in front of a
        # one-cell alcove against the play space's north wall, joined to the
        # room by a `barred` edge whose bar is that region.
        self.gates: list[dict] = []
        for st in a.get("brief", {}).get("stations", []) or []:
            name, _, kind = st.partition(" (")
            if kind.rstrip(")") != "gate":
                continue
            self.gates.append(self.place_gate(name.removeprefix("anchor/")))
        # A plot's parts (spec-0125 §5.1): the allocation's `parts`, each laid
        # as a region of its declared cells, named for the part.
        self.parts: list[dict] = []
        self.col_part: dict[tuple[int, int], dict] = {}
        self.lay_parts(a.get("parts", []) or [])

    def approach(self, seam: dict, sign: int) -> dict | None:
        """How a body reaches an opening above the walk plane, or None.

        A sideways opening whose sill stands above the floor, or a hole in the
        lid, is reached from a pillar of masonry under it: one course high it is
        a step, higher it carries a landing (its own space) and a ladder up its
        side, joined to the room by a `climb` edge. A hole in the floor needs
        nothing: a body drops through it.
        """
        s, lo, hi, axis = seam["seam"], seam["lo"], seam["hi"], seam["axis"]
        d = self.datum.value
        if axis == 1:
            if sign < 0:
                return None
            sill = lo[1].plus(-1)
            plo, phi = [lo[0], None, lo[2]], [hi[0], None, hi[2]]
            top = lo[1]
            # the ladder stands on the pillar's side toward the room's middle
            cx = (self.space[0][0] + self.space[1][0]) // 2
            lax, ldir = 0, (1 if cx >= lo[0].value else -1)
        else:
            if lo[1].value <= d:
                return None
            sill = lo[1]
            inward = -sign
            plo, phi = list(lo), list(hi)
            if inward > 0:
                plo[axis] = lo[axis].plus(1) if s["owns_plane"] else lo[axis]
                phi[axis] = plo[axis].plus(2)
            else:
                phi[axis] = hi[axis].plus(-1) if s["owns_plane"] else hi[axis]
                plo[axis] = phi[axis].plus(-2)
            top = hi[1]
            lax, ldir = axis, inward
        rise = sill.value - d
        if rise <= 0:
            return None
        out = {"plo": plo, "phi": phi, "sill": sill, "top": top, "rise": rise, "ladder": None}
        if rise > 1:
            other = 2 if lax == 0 else 0
            mid = (plo[other].value + phi[other].value - 1) // 2
            at = (phi[lax].value if ldir > 0 else plo[lax].value - 1)
            facing = {(0, 1): "east", (0, -1): "west", (2, 1): "south", (2, -1): "north"}[(lax, ldir)]
            cell_lo, cell_hi = [0, 0, 0], [0, 0, 0]
            cell_lo[lax], cell_hi[lax] = at, at + 1
            cell_lo[other], cell_hi[other] = mid, mid + 1
            out["ladder"] = {"lo": cell_lo, "hi": cell_hi, "facing": facing}
        return out

    def place_gate(self, stem: str) -> dict:
        lo, hi = self.space
        d = self.datum.value
        z = lo[2]
        for x in range(hi[0] - 1, lo[0], -1):
            cells = [(x2, y, zz) for x2 in (x - 1, x, x + 1) for y in (d, d + 1) for zz in (z, z + 1, z + 2)]
            if all(self.cell(*c) == ("claim", "room", None) for c in cells):
                g = {"stem": stem, "lo": [x, d, z], "hi": [x + 1, d + 2, z + 2], "x": x, "z": z}
                return g
        sys.exit(f"starter.py: no room against the north wall for the gate station `{stem}`; place it by hand")

    def lay_parts(self, parts: list[dict]) -> None:
        """Lay each part as a region of exactly its declared cells.

        The columns beside every opening, ladder, landing and gate, and the
        middle of every open side, stay the `room`, so every edge the room declares keeps its cells; the
        rest of the play space is walked in rows, a part a body walks taking
        the next run of its `cells` columns and a part seen and never entered
        taking columns from the far end, an out-of-walk region over the floor
        no edge reaches, only where the walked floor stays one piece. Where
        the parts stand is this starter's choice and yours to change; the
        counts are the plan's.
        """
        if not parts:
            return
        lo, hi = self.space
        d = self.datum.value
        top = self.lid.value if self.lid is not None else self.wall_top.value
        plain = {("claim", "room", None), ("claim", "roof", None), ("fill", "floor"), ("fill", "wall"), ("fill", "lid"), ("fill", "lamp"), ("void",)}
        plain |= {("fill", r) for r in getattr(self, "ground_roles", {}).values()}
        special = set()
        for x in range(lo[0] - 1, hi[0] + 2):
            for z in range(lo[2] - 1, hi[2] + 2):
                kinds = {self.cell(x, y, z) for y in range(d - 1, top + 1)} - plain
                # An open side's run is a way out along a whole side: the room
                # keeps the column inside its middle cell, and the parts may
                # stand along the rest of it.
                if kinds and not all(k[0] == "claim" and k[1].startswith("side-") for k in kinds):
                    special.add((x, z))
        room = ("claim", "room", None)
        floor = {
            (x, z)
            for x in range(lo[0], hi[0] + 1)
            for z in range(lo[2], hi[2] + 1)
            if self.cell(x, d, z) == room and self.cell(x, d + 1, z) == room
        }
        # A pillar, a ladder, an alcove: a column of the play space that is
        # not open floor is as special as an opening.
        special |= {(x, z) for x in range(lo[0], hi[0] + 1) for z in range(lo[2], hi[2] + 1)} - floor
        keep = set()
        for sd in self.sides:
            mx, mz = (sd["lo"][0] + sd["hi"][0]) // 2, (sd["lo"][1] + sd["hi"][1]) // 2
            keep.add((min(max(mx, lo[0]), hi[0]), min(max(mz, lo[2]), hi[2])))
        free = []
        for x in range(lo[0], hi[0] + 1):
            zs = range(lo[2], hi[2] + 1) if (x - lo[0]) % 2 == 0 else range(hi[2], lo[2] - 1, -1)
            for z in zs:
                if (x, z) not in floor:
                    continue
                if (x, z) in keep or any(n in special for n in ((x + 1, z), (x - 1, z), (x, z + 1), (x, z - 1))):
                    keep.add((x, z))
                else:
                    free.append((x, z))
        if not keep and free:
            keep.add(free.pop(len(free) // 2))
        need = sum(p["cells"] for p in parts)
        if need > len(free):
            targets = [max(-(-3 * p["cells"] // 4), p["cells"] * len(free) // need) for p in parts]
        else:
            targets = [p["cells"] for p in parts]
        if sum(targets) > len(free):
            sys.exit(
                f"starter.py: the play space beside the openings holds {len(free)} column(s) and the parts "
                f"need at least {sum(targets)}; lay them by hand"
            )
        walked = (keep & floor) | set(free)
        beside_sides = {
            (min(max(x, lo[0]), hi[0]), min(max(z, lo[2]), hi[2]))
            for sd in self.sides
            for x in range(sd["lo"][0], sd["hi"][0] + 1)
            for z in range(sd["lo"][1], sd["hi"][1] + 1)
        }

        def whole(cells: set) -> bool:
            if not cells:
                return True
            start = next(iter(cells))
            seen, todo = {start}, [start]
            while todo:
                x, z = todo.pop()
                for n in ((x + 1, z), (x - 1, z), (x, z + 1), (x, z - 1)):
                    if n in cells and n not in seen:
                        seen.add(n)
                        todo.append(n)
            return len(seen) == len(cells)

        for p, n in zip(parts, targets):
            stem = p["id"].removeprefix("part/")
            entry = {"part": p, "region": f"part-{stem}", "walked": p.get("reached", True), "cols": []}
            self.parts.append(entry)
            if entry["walked"]:
                continue
            for c in reversed(free):
                if len(entry["cols"]) == n:
                    break
                if c in beside_sides:
                    continue
                if whole(walked - {c}):
                    walked.discard(c)
                    entry["cols"].append(c)
            if len(entry["cols"]) < n:
                sys.exit(f"starter.py: no room for the seen part `{p['id']}` that leaves the walked floor one piece; lay it by hand")
            free = [c for c in free if c not in set(entry["cols"])]
        for e, n in zip(self.parts, targets):
            if e["walked"]:
                e["cols"], free = free[:n], free[n:]
        for e in self.parts:
            for c in e["cols"]:
                self.col_part[c] = e

    def in_space_xz(self, x: int, z: int) -> bool:
        lo, hi = self.space
        return lo[0] <= x <= hi[0] and lo[2] <= z <= hi[2]

    def open_side(self, x: int, z: int) -> str | None:
        """The face of the open side whose ring run holds column `(x, z)`."""
        for s in self.sides:
            if s["lo"][0] <= x <= s["hi"][0] and s["lo"][1] <= z <= s["hi"][1]:
                return s["face"]
        return None

    def ground_role(self, x: int, y: int, z: int) -> str | None:
        """The handed ground's role at a cell under the walk plane: the
        column's own block, a ring column's from the nearest column inside;
        the surface block at the floor course where the terrain lies below it
        and the one under the surface between; the bottom block under the
        column's bottom. None where the whole hands no column."""
        if not self.ground_cols:
            return None
        lo, hi = self.space
        key = (min(max(x, lo[0]), hi[0]), min(max(z, lo[2]), hi[2]))
        col = self.ground_cols.get(key)
        if col is None:
            return None
        bottom, blocks = col
        k = y - bottom
        if 0 <= k < len(blocks):
            b = blocks[k]
        elif k < 0:
            b = blocks[0]
        elif y == self.datum.value - 1 or len(blocks) == 1:
            b = blocks[-1]
        else:
            b = blocks[-2]
        return self.ground_roles.get(b)

    def wall_ring(self, x: int, z: int) -> bool:
        lo, hi = self.space
        return (lo[0] - 1 <= x <= hi[0] + 1 and lo[2] - 1 <= z <= hi[2] + 1) and not self.in_space_xz(x, z)

    # -- what one cell is, by the rules above --
    def cell(self, x: int, y: int, z: int) -> tuple:
        kind = self.base_cell(x, y, z)
        if kind[0] == "claim" and kind[1].startswith("side-") and getattr(self, "col_part", None):
            # An open side's cell is the way out of the space inside it.
            space = self.side_space(x, z)
            return kind if space == "room" else ("claim", f"{kind[1]}-{space}", None)
        part = getattr(self, "col_part", {}).get((x, z))
        if part is None or kind != ("claim", "room", None):
            return kind
        return ("claim", part["region"], None)

    def side_space(self, x: int, z: int) -> str:
        """The walked space whose column stands inside an open side's cell."""
        lo, hi = self.space
        inner = (min(max(x, lo[0]), hi[0]), min(max(z, lo[2]), hi[2]))
        e = self.col_part.get(inner)
        return e["region"] if e is not None and e["walked"] else "room"

    def walkable(self, kind: tuple) -> bool:
        """A cell of a space a body walks: the room, or a walked part."""
        return kind == ("claim", "room", None) or any(kind == ("claim", e["region"], None) for e in self.parts if e["walked"])

    def base_cell(self, x: int, y: int, z: int) -> tuple:
        c = (x, y, z)
        for s in self.seams:
            if s["bar"] is not None and all(s["bar"][0][i] <= c[i] < s["bar"][1][i] for i in range(3)):
                return ("claim", f"seam-{s['stem']}", f"bar-{s['stem']}")
        for s in self.seams:
            if all(s["lo"][i].value <= c[i] < s["hi"][i].value for i in range(3)):
                return ("claim", f"{s['stem']}-way", None)
        d = self.datum.value
        for g in getattr(self, "gates", []):
            # the alcove and its grille, two courses high, walled at both
            # sides and capped with wall to the lid, so the bar is the only way
            # between the alcove and the room
            if d <= y < self.wall_top.value and ((x == g["x"] and z in (g["z"], g["z"] + 1)) or (abs(x - g["x"]) == 1 and z == g["z"])):
                if x != g["x"] or y >= d + 2:
                    return ("fill", "wall")
                if z == g["z"]:
                    return ("claim", f"{g['stem']}-alcove", None)
                return ("claim", g["stem"], f"bar-{g['stem']}")
        for s in self.seams:
            ap = s["approach"]
            if ap is None:
                continue
            lad = ap["ladder"]
            if lad is not None and lad["lo"][0] <= x < lad["hi"][0] and lad["lo"][2] <= z < lad["hi"][2] and d <= y <= ap["sill"].value:
                # the ladder up to the sill, and the cell beside the landing
                # open, so the climb abuts both the room and the landing
                return ("claim", f"{s['stem']}-ladder", f"ladder-{lad['facing']}" if y < ap["sill"].value else None)
            xz = all(ap["plo"][i].value <= c[i] < ap["phi"][i].value for i in (0, 2))
            if xz and d <= y < ap["sill"].value:
                return ("fill", "floor")
            if xz and lad is not None and ap["sill"].value <= y < ap["top"].value:
                return ("claim", f"{s['stem']}-landing", None)
        d = self.datum.value
        if y < d:
            if self.aloft and y < d - 1:
                return ("void",)
            g = None if self.aloft else self.ground_role(x, y, z)
            return ("fill", g or "floor")
        if self.lid is not None and y == self.lid.value:
            if self.in_space_xz(x, z) and (x % LAMP_PERIOD, z % LAMP_PERIOD) == (2, 2):
                return ("fill", "lamp")
            return ("fill", "lid")
        if self.lid is not None and y > self.lid.value:
            return ("claim", "roof", None)
        if any(sd["lo"][0] <= x <= sd["hi"][0] and sd["lo"][1] <= z <= sd["hi"][1] for sd in self.covered) \
                and d <= y < self.wall_top.value:
            return ("claim", "room", None)  # over a covered side's openings: the place's air
        side = self.open_side(x, z)
        if side is not None and d <= y < self.wall_top.value:
            # An open side stands no wall: its ring cells are open from the
            # walk plane up, and the floor course below runs across them.
            return ("claim", f"side-{side}", None)
        if self.wall_ring(x, z) and y < self.wall_top.value:
            lo, hi = self.space
            along, a0, a1 = (z, lo[2], hi[2]) if x in (lo[0] - 1, hi[0] + 1) else (x, lo[0], hi[0])
            if y == d + 1 and along % LAMP_PERIOD == 2 and a0 <= along <= a1:
                return ("fill", "lamp")
            return ("fill", "wall")
        if self.in_space_xz(x, z) and y < self.wall_top.value:
            return ("claim", "room", None)
        return ("void",)

    def cuts(self, axis: int) -> list[Bound]:
        """Every coordinate on `axis` where what a cell is can change, one
        bound per value (a handed one where any is), in emitted order."""
        n = self.n[axis]
        best: dict[int, Bound] = {}
        for b in self.cut_candidates(axis):
            if 0 <= b.value <= n and (b.value not in best or (b.handed and not best[b.value].handed)):
                best[b.value] = b
        best[0] = lit(0)
        return [best[v] for v in sorted(best) if v < n]

    def cut_candidates(self, axis: int) -> list[Bound]:
        """Every bound a cut on `axis` stands for, coincident ones included."""
        n = self.n[axis]
        lo, hi = self.space
        out = [lit(0), lit(n)]
        for s in self.seams:
            out += [s["lo"][axis], s["hi"][axis]]
            if s["bar"] is not None:
                out += [lit(s["bar"][0][axis]), lit(s["bar"][1][axis])]
            ap = s["approach"]
            if ap is not None:
                if axis == 1:
                    out += [ap["sill"], ap["top"]]
                else:
                    out += [ap["plo"][axis], ap["phi"][axis]]
                if ap["ladder"] is not None and axis != 1:
                    out += [lit(ap["ladder"]["lo"][axis]), lit(ap["ladder"]["hi"][axis])]
                if ap["ladder"] is not None and axis == 1:
                    out += [ap["sill"].plus(1)]
        for s in self.sides:
            if axis != 1:
                i = 0 if axis == 0 else 1
                out += [lit(s["lo"][i]), lit(s["hi"][i] + 1)]
        for g in self.gates:
            out += [lit(g["lo"][axis]), lit(g["hi"][axis])]
            if axis == 0:
                out += [lit(g["x"] - 1), lit(g["x"] + 2)]
        if axis == 1:
            out += [self.datum.plus(-1), self.datum, self.datum.plus(1), self.datum.plus(2), self.wall_top]
            if self.lid is not None:
                out += [self.lid, self.lid.plus(1)]
        else:
            out += [lit(lo[axis] - 1), lit(lo[axis]), lit(hi[axis] + 1), lit(hi[axis] + 2)]
            if self.parts:
                out += [lit(v) for v in range(lo[axis], hi[axis] + 2)]
            out += [lit(v) for v in range(0, n, LAMP_PERIOD)] + [lit(v) for v in range(2, n, LAMP_PERIOD)] + [lit(v + 1) for v in range(2, n, LAMP_PERIOD)]
        return out


def size_of(lo: Bound, hi: Bound | None) -> dict:
    if hi is None:
        return {"size": "relative", "weight": num(1)}
    if lo.expr.get("expr") == "int" and hi.expr.get("expr") == "int":
        return {"blocks": num(hi.value - lo.value), "size": "absolute"}
    if lo.value == 0 and lo.expr.get("expr") == "int":
        return {"blocks": hi.expr, "size": "absolute"}
    return {"blocks": arith(hi.expr, "sub", lo.expr), "size": "absolute"}


class Rules:
    """Rules, each body stated once and called by name wherever it recurs."""

    def __init__(self) -> None:
        self.rules: dict[str, list] = {}
        self.names: dict[str, str] = {}

    def name(self, body: dict, hint: str) -> dict:
        key = json.dumps(body, sort_keys=True)
        if key not in self.names:
            name = f"{hint}_{len(self.names)}"
            self.names[key] = name
            self.rules[name] = [{"body": body, "weight": 1}]
        return {"op": "call", "symbol": self.names[key]}


AXES = ("x", "y", "z")


def order_guard(p: "Place") -> dict | None:
    """The order the program's cuts were emitted in, stated as the guard of
    its start rule.

    A split's sizes are differences of neighbouring cuts, and the cuts mix
    handed bounds with this starter's literals, sorted at their values now.
    The program binds when the plan moves only while that order holds: a
    handed bound that passes a literal (a seam's top `y1 + 1` dropping below a
    course `8`) would size a box negative, and the cells either side of the
    cut were read at the old order. So the start rule holds only while, in
    every split the program emits (`SPLITS`), each handed cut stays strictly
    between its neighbours, and each handed bound stays level with any
    handed bound it coincided with; past that the expansion refuses, naming
    the parameter and the bound it crossed (`no alternative of rule "place"
    applies … required … >`), and the remedy is to re-run the generator
    against the new allocation. Inside it no split sizes a box negative.
    """
    leaves: list[dict] = []
    seen: set[str] = set()

    def need(lhs: dict, op: str, rhs: dict) -> None:
        leaf = {"cond": "cmp", "lhs": lhs, "op": op, "rhs": rhs}
        key = json.dumps(leaf, sort_keys=True)
        if key not in seen:
            seen.add(key)
            leaves.append(leaf)

    for axis, cuts in SPLITS:
        end = Bound(p.n[AXES.index(axis)], {"expr": "dim", "dim": axis}, False)
        seq = cuts + [end]
        for a, b in zip(seq, seq[1:]):
            if a.handed or b.handed:
                need(b.expr, "gt", a.expr)
    for axis in range(3):
        rep = {c.value: c for c in p.cuts(axis)}
        for b in p.cut_candidates(axis):
            r = rep.get(b.value)
            if b.handed and r is not None and r.expr != b.expr:
                need(b.expr, "eq", r.expr)
    if not leaves:
        return None
    return leaves[0] if len(leaves) == 1 else {"cond": "all", "of": leaves}


def start_rule(p: "Place", start: dict) -> list:
    """The start rule's one alternative, guarded by the order it was emitted in."""
    alt = {"body": start, "weight": 1}
    guard = order_guard(p)
    if guard is not None:
        alt["when"] = guard
    return [alt]


def leaf(kind: tuple) -> dict:
    if kind[0] == "fill":
        return fill(kind[1])
    if kind[0] == "void":
        return VOID
    _, region, role = kind
    return claim(region, fill(role) if role else VOID)


SPLITS: list[tuple[str, list]] = []  # every split's cuts, as emitted: what `order_guard` reads


def split(axis: str, cuts: list[Bound], children: list[dict]) -> dict:
    if len(children) == 1:
        return children[0]
    SPLITS.append((axis, list(cuts)))
    sizes = [size_of(cuts[i], cuts[i + 1] if i + 1 < len(cuts) else None) for i in range(len(cuts))]
    return {"axis": axis, "children": children, "op": "split", "sizes": sizes}


def body(p: Place, rules: Rules) -> dict:
    """The place, cut into boxes on x, then y, then z, each box one kind of cell."""
    xs, ys, zs = p.cuts(0), p.cuts(1), p.cuts(2)
    columns = []
    for x in xs:
        courses = []
        for y in ys:
            row = [leaf(p.cell(x.value, y.value, z.value)) for z in zs]
            courses.append(rules.name(split("z", zs, row), "row"))
        columns.append(rules.name(split("y", ys, courses), "column"))
    return split("x", xs, columns)


def marks(p: Place, inner: dict) -> dict:
    """Every owed name, answered by a `mark` of its stem on walkable floor."""
    lo, hi = p.a["space"]
    cx, cz = (lo[0] + hi[0]) // 2, (lo[2] + hi[2]) // 2
    taken: set[tuple[int, int]] = set()

    def free(x: int, z: int) -> tuple[int, int]:
        for dz in range(0, max(p.ex, p.ez)):
            for x2, z2 in ((x, z + dz), (x, z - dz), (x + dz, z), (x - dz, z)):
                if lo[0] <= x2 <= hi[0] and lo[2] <= z2 <= hi[2] and (x2, z2) not in taken:
                    if p.walkable(p.cell(x2, p.datum.value, z2)) and p.walkable(p.cell(x2, p.datum.value + 1, z2)) and p.cell(x2, p.datum.value - 1, z2)[0] == "fill":
                        taken.add((x2, z2))
                        return x2, z2
        sys.exit("starter.py: no free floor cell for a mark; the play space is full")

    out = inner
    drawn = getattr(getattr(p, "model", None), "marks", {})
    for x, z, _, y in drawn.values():
        if y is None:
            taken.add((x, z))
    for name in reversed(p.a["owed_anchors"]):
        stem = name.removeprefix("anchor/")
        if stem.startswith("seam-") or any(g["stem"] == stem for g in p.gates):
            continue  # answered by a barred edge's region, never a mark
        if stem in drawn:
            x, z, facing, y = drawn[stem]
            if y is not None:
                # A block's own cell (a sign's, a container's): the name says
                # where the thing stands, never where a body does.
                mark = {"anchor": stem, "at": "offset", "facing": facing, "y": num(y), "x": num(x), "z": num(z)}
                out = {"body": out, "mark": mark, "op": "mark"}
                continue
            # Where the generator's model stands it: on floor a body reaches.
            if not p.walkable(p.cell(x, p.datum.value, z)):
                sys.exit(f"starter.py: the mark `{stem}` the model stands at ({x}, {z}) is not on floor a body reaches: {p.cell(x, p.datum.value, z)}")
            mark = {"anchor": stem, "at": "offset", "facing": facing, "y": p.datum.expr, "x": num(x), "z": num(z)}
            out = {"body": out, "mark": mark, "op": "mark"}
            continue
        mark = {"anchor": stem, "at": "offset", "facing": "south", "y": p.datum.expr}
        if stem == "spawn":
            mark["role"] = "entry"
        target = (cx, cz)
        if stem.startswith("unlock-"):
            s = next((s for s in p.seams if f"unlock-{s['stem']}" == stem), None)
            if s is not None:
                (x0, _, z0), (x1, _, z1) = s["seam"]["cells"]
                target = (min(max((x0 + x1) // 2, lo[0]), hi[0]), min(max((z0 + z1) // 2, lo[2]), hi[2]))
        x, z = free(*target)
        mark["x"], mark["z"] = num(x), num(z)
        out = {"body": out, "mark": mark, "op": "mark"}
    return out


def contract(p: Place) -> dict:
    """The spatial contract: the room, a landing under each raised opening, an
    edge per seam, the roof zone no body walks."""
    spaces = {"room": {"envelope": "enclosed" if p.lid is not None else "open"}}
    edges = []
    for s in p.seams:
        seam = s["seam"]
        space = "room"
        ap = s["approach"]
        if ap is not None and ap["ladder"] is not None:
            space = f"{s['stem']}-landing"
            spaces[space] = {"envelope": "open"}
            edges.append({"a": "room", "b": space, "class": "climb", "rise": ap["rise"], "via": f"{s['stem']}-ladder"})
        if s["bar"] is not None:
            edge = {"a": space, "b": "exterior", "bar": {"block": f"bar-{s['stem']}", "region": f"seam-{s['stem']}"}, "class": "barred"}
            if s["bar"] != ([v.value for v in s["lo"]], [v.value for v in s["hi"]]):
                edge["via"] = f"{s['stem']}-way"  # the opening the bar closes
            edges.append(edge)
            continue
        answers = [c for c in seam["answer_with"] if c != "barred"]
        cls = "walk" if "walk" in answers else answers[0]
        edge = {"a": space, "b": "exterior", "class": cls, "via": f"{s['stem']}-way"}
        if cls in ("stair", "drop", "climb"):
            edge["rise"] = 0  # the level beyond is the neighbour's to measure
        edges.append(edge)
    for s in p.sides:
        # A frontage a body crosses: the open side's cells are the way out
        # (spec-0123 §5.4). A counter a body does not cross stands at the play
        # space's edge, inside, and the side stays this edge's. On a plot the
        # run is one way out per walked space standing inside it.
        spaces_in = []
        for x in range(s["lo"][0], s["hi"][0] + 1):
            for z in range(s["lo"][1], s["hi"][1] + 1):
                sp = p.side_space(x, z) if p.col_part else "room"
                if sp not in spaces_in:
                    spaces_in.append(sp)
        for sp in sorted(spaces_in):
            via = f"side-{s['face']}" if sp == "room" else f"side-{s['face']}-{sp}"
            edges.append({"a": sp, "b": "exterior", "class": "walk", "via": via})
    no_body = {}
    pairs = set()
    for e in p.parts:
        if not e["walked"]:
            no_body[e["region"]] = {"part": e["part"]["id"], "reason": f"{e['part']['use']}: seen, never entered"}
            continue
        spaces[e["region"]] = {"envelope": spaces["room"]["envelope"], "part": e["part"]["id"]}
    # A walk edge between every two walked spaces whose columns touch.
    for (x, z), e in p.col_part.items():
        if not e["walked"]:
            continue
        for n in ((x + 1, z), (x, z + 1), (x - 1, z), (x, z - 1)):
            if not p.in_space_xz(*n):
                continue
            o = p.col_part.get(n)
            other = "room" if o is None else (o["region"] if o["walked"] else None)
            if other is not None and other != e["region"]:
                pairs.add(tuple(sorted((e["region"], other))))
    for a_, b_ in sorted(pairs):
        edges.append({"a": a_, "b": b_, "class": "walk"})
    for g in p.gates:
        spaces[f"{g['stem']}-alcove"] = {"envelope": "open"}
        edges.append({"a": "room", "b": f"{g['stem']}-alcove", "bar": {"block": f"bar-{g['stem']}", "region": g["stem"]}, "class": "barred"})
    out = {
        "edges": edges,
        "entry": "room",
        # ── BUILD HERE: the spaces. A room a body walks around inside walls and
        # under a lid is `enclosed`; a street, a deck or a court open to the sky
        # is `open`. A second floor is a second space and a `climb` or `stair`
        # edge between them — a landing below is reached that way
        # (grammar.md §2d; references/detail.md, the program's contract).
        "spaces": spaces,
    }
    if p.lid is not None and p.lid.value + 1 < p.ey:
        no_body["roof"] = {"reason": "the roof zone over the lid, which no one is meant to walk"}
        out["no_body_majority_ack"] = "the roof zone the plan reserves over the lid is left for the roof the author builds; nobody walks it"
    if no_body:
        out["no_body"] = no_body
    return out


# A handed block named with no state, written out as the gates require
# (`DW0735`, `DW0737`): every property, the block's own default where the
# design says nothing.
BARE_STATES = {
    "minecraft:iron_bars": "minecraft:iron_bars[east=false,north=false,south=false,waterlogged=false,west=false]",
}


def full_state(block: str) -> str:
    return BARE_STATES.get(block, block)


def program(a: dict) -> dict:
    p = Place(a)
    rules = Rules()
    SPLITS.clear()
    # ── BUILD HERE: the building. The body below is the empty place — floor,
    # ring wall, lid, openings, sconces. Replace the `room` boxes with the
    # rooms the design draws, starting from the idioms (grammar.md §2c: 1
    # repetition for a colonnade or a run of bays, 3 shape for an arch or a
    # gable, 7 symmetry for a front, 9 light for where the lamps stand) and the
    # procedure (references/new-pieces.md step 3, the program; step 4, the
    # gates). Keep every seam's region and every mark; they are what binds.
    start = marks(p, body(p, rules))
    rules.rules["place"] = start_rule(p, start)
    palette = dict(PALETTE)
    palette.update({r: b for b, r in p.ground_roles.items()})
    for s in p.seams:
        if s["bar"] is not None:
            palette[f"bar-{s['stem']}"] = full_state(s["seam"]["bar_block"])
        ap = s["approach"]
        if ap is not None and ap["ladder"] is not None:
            f = ap["ladder"]["facing"]
            palette[f"ladder-{f}"] = f"minecraft:ladder[facing={f},waterlogged=false]"
    for g in p.gates:
        palette[f"bar-{g['stem']}"] = full_state("minecraft:iron_bars")
    climbs = any(e.get("class") == "climb" for e in contract(p)["edges"])
    return {
        "contract": contract(p),
        "name": f"{a['place'].removeprefix('node/')}-starter",
        "palette": palette,
        "params": dict(sorted(p.h.params.items())),
        "rules": rules.rules,
        # ── BUILD HERE: the faces the piece shows the world (grammar.md §2f).
        # Derived from what this skeleton draws: a declared side must hold a
        # block (`DW0885`), and a side over open air must be declared.
        "shown_faces": shown_faces(p),
        "start": "place",
        "version": "1.14.0" if p.parts else ("1.10.0" if climbs else "1.9.0"),
    }


def shown_faces(p) -> list[str]:
    """The sides this skeleton finishes: the four walls; the top course where
    the lid stands at it, or the walls reach it under an open sky (a lid under a
    declared roof zone leaves the top course to the roof the author builds, so
    `up` is the author's to add with the roof); and the underside of an aloft
    place, which stands over air the party can be in."""
    faces = ["east", "north", "south", "west"]
    if (p.lid is not None and p.lid.value == p.ey - 1) or (p.lid is None and p.wall_top.value >= p.ey):
        faces.append("up")
    if p.aloft:
        faces.append("down")
    return sorted(faces)


# ════════════════════════════════════════════════════════════════════════
# A building drawn as cells
#
# A place generator does not edit the body above. It draws its buildings,
# yards and life into a `Model` — cells in piece-local coordinates (+x east,
# +z south, the walk plane at `place.datum.value`) — and hands the drawing to
# `run`:
#
#     import os, sys
#     sys.path.insert(0, os.path.join(os.environ["DELVEWRIGHT_SKILL"], "scripts"))
#     from starter import Model, run
#
#     def build(place):
#         m = Model(place)
#         m.palette["boards"] = "minecraft:spruce_planks"
#         m.box(3, 8, 1, 4, 3, 7, "boards")          # x0, x1, y0, y1, z0, z1, inclusive
#         m.clear(4, 7, 1, 3, 4, 6)                    # the room inside
#         m.part(3, 8, 3, 7, "house")                  # the columns of part/house
#         return m
#
#     raise SystemExit(run(build))
#
# `run` asks the engine (`delvec grammar survey`) what the drawing does — where
# a body stands, what it reaches from the place's ways in, which air is sealed,
# which spaces a walk joins, which blocks the server drops, which the palette
# refuses, which written states the pinned game does not have — and refuses
# before writing on any of the last three, or claims every cell from those
# answers (a drawn model therefore needs a `delvec` binary, `--delvec`): a cell a body reaches
# goes into the part whose column it stands in (the room where no part is
# drawn); floor no body reaches, and the sealed air over it, is claimed
# `unwalked`; a walk edge joins two spaces only where the engine proves a body
# crosses. Nothing here decides where a body goes.
# ════════════════════════════════════════════════════════════════════════

import os
import shutil
import subprocess
import tempfile

AIR = "~air"  # a cell the model clears: air, whatever the empty place had there

DIRS = {"north": (0, -1), "south": (0, 1), "east": (1, 0), "west": (-1, 0)}
OPP = {"north": "south", "south": "north", "east": "west", "west": "east"}
CCW = {"north": "west", "west": "south", "south": "east", "east": "north"}


# ---- block states, every property written ----
#
# Each constructor returns one full state: every property its block has at the
# pinned version, written, so a generator never spells a state by hand. A
# material is the block-name prefix the family is named by (`"spruce"`,
# `"stone_brick"`, `"deepslate_tile"`, namespaced or bare). `run` asks the
# engine whether every state the model writes is a state of the pinned game
# and refuses before it writes anything when one is not, so a misspelt
# material (`tuff_bricks_stairs`) or property is named at the generator, not
# at `detail --check`. Connections (stair shapes, the arms of fences, walls,
# panes and bars) are written straight here and derived from the neighbours by
# `Model.finish`.


def mc(name: str) -> str:
    """`spruce` -> `minecraft:spruce`; a namespaced name is kept."""
    return name if ":" in name else f"minecraft:{name}"


def _b(v: bool) -> str:
    return "true" if v else "false"


def stairs(mat: str, facing: str, half: str = "bottom", shape: str = "straight") -> str:
    return f"{mc(mat)}_stairs[facing={facing},half={half},shape={shape},waterlogged=false]"


def slab(mat: str, kind: str = "bottom") -> str:
    """`kind` is `bottom`, `top` or `double`."""
    return f"{mc(mat)}_slab[type={kind},waterlogged=false]"


def pillar(block: str, axis: str = "y") -> str:
    """A block with an axis: a log, wood, a stripped log, a basalt or quartz
    pillar, a hay bale. `block` is the whole block name (`spruce_log`)."""
    return f"{mc(block)}[axis={axis}]"


def door(mat: str, facing: str, half: str, hinge: str = "left", open: bool = False) -> str:
    """One half of a door (`lower` or `upper`); `Model.door` stands both."""
    return f"{mc(mat)}_door[facing={facing},half={half},hinge={hinge},open={_b(open)},powered=false]"


def trapdoor(mat: str, facing: str, half: str = "bottom", open: bool = False) -> str:
    return f"{mc(mat)}_trapdoor[facing={facing},half={half},open={_b(open)},powered=false,waterlogged=false]"


def fence(mat: str) -> str:
    return f"{mc(mat)}_fence[east=false,north=false,south=false,waterlogged=false,west=false]"


def fence_gate(mat: str, facing: str, open: bool = False, in_wall: bool = False) -> str:
    return f"{mc(mat)}_fence_gate[facing={facing},in_wall={_b(in_wall)},open={_b(open)},powered=false]"


def wall(mat: str) -> str:
    """A wall block (`stone_brick` -> `stone_brick_wall`)."""
    return f"{mc(mat)}_wall[east=none,north=none,south=none,up=true,waterlogged=false,west=none]"


def pane(block: str = "glass_pane") -> str:
    """A pane or bars by its whole name: `glass_pane`, `white_stained_glass_pane`, `iron_bars`."""
    return f"{mc(block)}[east=false,north=false,south=false,waterlogged=false,west=false]"


def lantern(hanging: bool = False, soul: bool = False) -> str:
    return f"minecraft:{'soul_' if soul else ''}lantern[hanging={_b(hanging)},waterlogged=false]"


def leaves(mat: str, distance: int = 1) -> str:
    """Persistent leaves (`azalea`, `oak`), which no decay takes."""
    return f"{mc(mat)}_leaves[distance={distance},persistent=true,waterlogged=false]"


def bed(colour: str, facing: str, part: str) -> str:
    """One half of a bed (`foot` or `head`); `Model.bed` lays both."""
    return f"minecraft:{colour}_bed[facing={facing},occupied=false,part={part}]"


def candle(n: int = 1, colour: str | None = None, lit: bool = False) -> str:
    name = f"{colour}_candle" if colour else "candle"
    return f"minecraft:{name}[candles={n},lit={_b(lit)},waterlogged=false]"


def torch() -> str:
    return "minecraft:torch"


def wall_torch(facing: str) -> str:
    return f"minecraft:wall_torch[facing={facing}]"


def campfire(facing: str = "north", lit: bool = True, soul: bool = False) -> str:
    return f"minecraft:{'soul_' if soul else ''}campfire[facing={facing},lit={_b(lit)},signal_fire=false,waterlogged=false]"


def ladder(facing: str) -> str:
    return f"minecraft:ladder[facing={facing},waterlogged=false]"


def chain(axis: str = "y") -> str:
    return f"minecraft:iron_chain[axis={axis},waterlogged=false]"


def barrel(facing: str = "up", open: bool = False) -> str:
    return f"minecraft:barrel[facing={facing},open={_b(open)}]"


def button(mat: str, facing: str, face: str = "wall") -> str:
    """`face` is `floor`, `wall` or `ceiling`."""
    return f"{mc(mat)}_button[face={face},facing={facing},powered=false]"


def grass_block(snowy: bool = False) -> str:
    return f"minecraft:grass_block[snowy={_b(snowy)}]"


def tall_plant(block: str, half: str) -> str:
    """One half of a two-high plant (`rose_bush`, `lilac`, `tall_grass`); `Model.tall` stands both."""
    return f"{mc(block)}[half={half}]"


def is_full(s: str) -> bool:
    """Whether a state is a full block by its name: anything that is not thin,
    partial or passable. What a connecting block joins as it joins a full
    block (`Model.finish`), and what may stand beside a way in without being
    the way's room (`Plot.base_cell`)."""
    b = s.split("[")[0]
    thin = ("_slab", "_stairs", "_trapdoor", "_door", "_carpet", "_pressure_plate", "_button", "_sign",
            "_banner", "_torch", "torch", "lantern", "_candle", "candle", "flower_pot", "_leaves", "_fence",
            "_wall", "_pane", "_bars", "_fence_gate", "ladder", "_chain", "air", "short_grass", "fern")
    return not b.startswith("minecraft:potted_") and not any(b.endswith(t) for t in thin)


class Model:
    """The place as cells a generator draws into.

    A cell holds a palette role (a key of `palette`, which may be a weighted
    list) or a literal block state (`minecraft:…`, every property written).
    The cells of every seam's opening stay the connection's: a write into one
    is skipped and counted. `part` names a column's part; `mark` stands an
    owed name.
    """

    def __init__(self, place: "Place") -> None:
        self.place = place
        self.ex, self.ey, self.ez = place.n
        self.palette: dict = dict(PALETTE)
        # the handed ground's blocks, each its own role (`Place.ground_role`)
        self.palette.update({r: b for b, r in place.ground_roles.items()})
        self.v: dict[tuple[int, int, int], str] = {}
        self.region: dict[tuple[int, int], str] = {}
        self.marks: dict[str, tuple[int, int, str, int | None]] = {}
        self.skipped = 0
        self.shown: set[str] = set()
        self.no_touch: set[tuple[int, int, int]] = set()
        self.climbed: set[str] = set()  # `climbs_to`: raised openings the drawing climbs to itself
        self.dx = self.dz = 0  # `shift`: where the drawing stands, moved
        for s in place.seams:
            # the ladder the starter stands up to a raised opening, bound
            ap = s.get("approach")
            if ap is not None and ap["ladder"] is not None:
                f = ap["ladder"]["facing"]
                self.palette[f"ladder-{f}"] = ladder(f)
            lo, hi = [b.value for b in s["lo"]], [b.value for b in s["hi"]]
            for x in range(lo[0], hi[0]):
                for y in range(lo[1], hi[1]):
                    for z in range(lo[2], hi[2]):
                        self.no_touch.add((x, y, z))

    def climbs_to(self, edge: str) -> None:
        """The drawing carries a body up to this seam's raised opening itself
        — a flight of stairs, a ramp — so the starter stands no pillar,
        landing or ladder under it (`Place.approach`), and the seam's edge is
        the plain way out the engine proves the drawing walks. `edge` is the
        seam's edge, `edge/<stem>` or `<stem>`. Without it, a drawing that
        writes where the starter stands that ladder is refused, never
        overwritten."""
        stem = edge.removeprefix("edge/")
        s = next((s for s in self.place.seams if s["stem"] == stem), None)
        if s is None:
            sys.exit(f"starter.py: climbs_to(`{edge}`): the place has no seam on that edge; it has {sorted(s['stem'] for s in self.place.seams)}")
        if s.get("approach") is None or s["approach"]["ladder"] is None:
            sys.exit(f"starter.py: climbs_to(`{edge}`): that seam's opening stands at most one course over the walk plane, so the starter stands no ladder under it to replace")
        self.climbed.add(stem)

    def shift(self, dx: int, dz: int = 0) -> None:
        """Draw moved `(dx, dz)` from here on: every write and read through
        `set`, `get`, `part` and `mark` (and so `box`, `door`, `roof`…) lands
        that far over, so a building drawn at its own coordinates is stood,
        whole, where the lot needs it (on its street edge, say). `shift(0)`
        ends it; `run` reads the cells as they stand whatever was left set.
        A seam's opening is absolute: a shifted write into it is still skipped."""
        self.dx, self.dz = dx, dz

    def set(self, x: int, y: int, z: int, role: str) -> None:
        x, z = x + self.dx, z + self.dz
        if not (0 <= x < self.ex and 0 <= y < self.ey and 0 <= z < self.ez):
            return
        if (x, y, z) in self.no_touch:
            self.skipped += 1
            return
        self.v[(x, y, z)] = role

    def get(self, x: int, y: int, z: int) -> str | None:
        return self.v.get((x + self.dx, y, z + self.dz))

    def box(self, x0, x1, y0, y1, z0, z1, role: str) -> None:
        """Fill a box, inclusive on every axis."""
        for x in range(min(x0, x1), max(x0, x1) + 1):
            for y in range(min(y0, y1), max(y0, y1) + 1):
                for z in range(min(z0, z1), max(z0, z1) + 1):
                    self.set(x, y, z, role)

    def clear(self, x0, x1, y0, y1, z0, z1) -> None:
        self.box(x0, x1, y0, y1, z0, z1, AIR)

    def hollow(self, x0, x1, y0, y1, z0, z1, role: str) -> None:
        """Four walls of a box, inclusive; the inside untouched."""
        for y in range(min(y0, y1), max(y0, y1) + 1):
            for x in range(min(x0, x1), max(x0, x1) + 1):
                self.set(x, y, z0, role)
                self.set(x, y, z1, role)
            for z in range(min(z0, z1), max(z0, z1) + 1):
                self.set(x0, y, z, role)
                self.set(x1, y, z, role)

    def part(self, x0, x1, z0, z1, stem: str) -> None:
        """The columns `[x0..x1] x [z0..z1]` are `part/<stem>`'s."""
        for x in range(min(x0, x1), max(x0, x1) + 1):
            for z in range(min(z0, z1), max(z0, z1) + 1):
                self.region[(x + self.dx, z + self.dz)] = stem

    def mark(self, stem: str, x: int, z: int, facing: str = "south", y: int | None = None) -> None:
        """Stand the owed name `stem` on the floor at `(x, z)`; with `y`, name
        the block at `(x, y, z)` instead — a sign a `signs` entry writes, a
        container a `loot` entry fills — which is not a place a body stands."""
        self.marks[stem] = (x + self.dx, z + self.dz, facing, y)

    def show(self, *faces: str) -> None:
        """Declare sides of the piece finished exterior, meant to be seen
        (`shown_faces`): `"up"` where the drawing stands a roof, a chimney or a
        lamp on the frame's top course in the open air. The four walls are
        always shown, the underside of an aloft place too. A solid cell on an
        undeclared side in air the party can be in is `DW0885`, which
        `detail --check` names with its cell."""
        for f in faces:
            if f not in ("east", "west", "north", "south", "up", "down"):
                sys.exit(f"starter.py: `{f}` is not a side; a side is east, west, north, south, up or down")
            self.shown.add(f)

    # -- shapes a builder reaches for --

    def door(self, x: int, y: int, z: int, mat: str, facing: str, hinge: str = "left", open: bool = False) -> None:
        """A door's two halves, its lower half at `(x, y, z)`."""
        self.set(x, y, z, door(mat, facing, "lower", hinge, open))
        self.set(x, y + 1, z, door(mat, facing, "upper", hinge, open))

    def bed(self, x: int, y: int, z: int, colour: str, facing: str) -> None:
        """A bed's foot at `(x, y, z)`, its head the next cell toward `facing`."""
        dx, dz = DIRS[facing]
        self.set(x, y, z, bed(colour, facing, "foot"))
        self.set(x + dx, y, z + dz, bed(colour, facing, "head"))

    def tall(self, x: int, y: int, z: int, block: str) -> None:
        """A two-high plant's halves, its lower half at `(x, y, z)`."""
        self.set(x, y, z, tall_plant(block, "lower"))
        self.set(x, y + 1, z, tall_plant(block, "upper"))

    def roof(self, x0: int, x1: int, z0: int, z1: int, head: int, along: str, mat: str,
             fill: str | None = None, overhang: int = 1, ridge: str | None = None, hip: bool = False) -> int:
        """A pitched roof of `mat` stairs over walls `[x0..x1] x [z0..z1]`
        (walls included) whose heads stand at course `head`. Its eave course
        sits level with the wall heads, `overhang` cells out, and each course
        rises one and steps one in, so the slope rests on the wall heads and
        the loft under it is closed. A gable roof (the default) runs its
        ridge along `along` (`"x"` or `"z"`) and fills its two gable ends
        under the slope with `fill`; a hipped roof (`hip=True`) slopes on all
        four sides and `along` is ignored. An odd span is capped with a
        course of `ridge` (a `mat` slab by default). Returns the top course;
        `Model.finish` turns the corner stairs."""
        cap = ridge or slab(mat)
        y0 = head - (overhang - 1) if overhang >= 1 else head + 1
        ax0, ax1, az0, az1 = x0 - overhang, x1 + overhang, z0 - overhang, z1 + overhang
        y = y0
        if hip:
            while ax0 <= ax1 and az0 <= az1:
                if ax0 == ax1 or az0 == az1:
                    self.box(ax0, ax1, y, y, az0, az1, cap)
                    return y
                for x in range(ax0, ax1 + 1):
                    self.set(x, y, az0, stairs(mat, "south"))
                    self.set(x, y, az1, stairs(mat, "north"))
                for z in range(az0 + 1, az1):
                    self.set(ax0, y, z, stairs(mat, "east"))
                    self.set(ax1, y, z, stairs(mat, "west"))
                ax0, ax1, az0, az1, y = ax0 + 1, ax1 - 1, az0 + 1, az1 - 1, y + 1
            return y - 1
        top = y0
        if along == "x":
            lo, hi = az0, az1
            while lo <= hi:
                top = y
                if lo == hi:
                    self.box(ax0, ax1, y, y, lo, lo, cap)
                    break
                self.box(ax0, ax1, y, y, lo, lo, stairs(mat, "south"))
                self.box(ax0, ax1, y, y, hi, hi, stairs(mat, "north"))
                if fill is not None and max(lo + 1, z0) <= min(hi - 1, z1):
                    for x in (x0, x1):
                        self.box(x, x, y, y, max(lo + 1, z0), min(hi - 1, z1), fill)
                lo, hi, y = lo + 1, hi - 1, y + 1
            return top
        lo, hi = ax0, ax1
        while lo <= hi:
            top = y
            if lo == hi:
                self.box(lo, lo, y, y, az0, az1, cap)
                break
            self.box(lo, lo, y, y, az0, az1, stairs(mat, "east"))
            self.box(hi, hi, y, y, az0, az1, stairs(mat, "west"))
            if fill is not None and max(lo + 1, x0) <= min(hi - 1, x1):
                for z in (z0, z1):
                    self.box(max(lo + 1, x0), min(hi - 1, x1), y, y, z, z, fill)
            lo, hi, y = lo + 1, hi - 1, y + 1
        return top

    def states(self, r: str) -> list[str]:
        """Every block state a cell's value may write: one, or each variant."""
        if r == AIR:
            return ["minecraft:air"]
        if r.startswith("minecraft:"):
            return [r]
        b = self.palette.get(r)
        if b is None:
            sys.exit(f"starter.py: the model paints the role `{r}`, which its palette does not bind")
        return [v["block"] for v in b] if isinstance(b, list) else [b]

    def state(self, r: str) -> str:
        return self.states(r)[0]

    def finish(self) -> None:
        """Write every connecting block as the game derives it from its
        neighbours: stair shapes, and the arms of panes, bars, fences and
        walls. A building helper: the arms are read off the neighbours' names,
        never asked of the engine."""
        self._stairs()
        self._arms()

    def _props(self, r):
        if r is None or r == AIR:
            return None
        s = self.state(r) if not r.startswith("minecraft:") else r
        if "_stairs[" not in s:
            return None
        kv = dict(p.split("=") for p in s[s.index("[") + 1:-1].split(","))
        return kv["facing"], kv["half"], s

    def _stairs(self) -> None:
        new = {}
        for (x, y, z), r in list(self.v.items()):
            st = self._props(r)
            if st is None:
                continue
            f, h, s = st

            def at(dx, dz):
                return self._props(self.v.get((x + dx, y, z + dz)))

            def take(face):
                n = at(*DIRS[face])
                return n is None or n[0] != f or n[1] != h

            shape = "straight"
            dx, dz = DIRS[f]
            front = at(dx, dz)
            if front and front[1] == h and (front[0] in ("north", "south")) != (f in ("north", "south")) and take(OPP[front[0]]):
                shape = "outer_left" if front[0] == CCW[f] else "outer_right"
            if shape == "straight":
                back = at(-dx, -dz)
                if back and back[1] == h and (back[0] in ("north", "south")) != (f in ("north", "south")) and take(back[0]):
                    shape = "inner_left" if back[0] == CCW[f] else "inner_right"
            if shape != "straight":
                new[(x, y, z)] = s.replace("shape=straight", f"shape={shape}")
        self.v.update(new)

    @staticmethod
    def _family(s: str) -> str | None:
        b = s.split("[")[0]
        if b.endswith("glass_pane") or b.endswith("_bars"):
            return "pane"
        if b.endswith("_fence_gate"):
            return "gate"
        if b.endswith("_fence"):
            return "fence"
        if b.endswith("_wall"):
            return "wall"
        return None

    def _arms(self) -> None:
        solid = is_full

        new = {}
        for (x, y, z), r in list(self.v.items()):
            if r == AIR:
                continue
            s = r if r.startswith("minecraft:") else self.state(r)
            if isinstance(self.palette.get(r), list):
                continue
            fam = self._family(s)
            if fam not in ("pane", "fence", "wall"):
                continue
            arms = {}
            for face, (dx, dz) in DIRS.items():
                nr = self.v.get((x + dx, y, z + dz))
                ns = "" if nr in (None, AIR) else (nr if nr.startswith("minecraft:") else self.state(nr))
                nf = self._family(ns) if ns else None
                arms[face] = bool(ns) and (solid(ns) or nf == fam or (fam == "fence" and nf == "gate") or (fam == "wall" and nf in ("pane", "gate")))
            base = s.split("[")[0]
            if fam == "wall":
                straight = (arms["north"] and arms["south"] and not arms["east"] and not arms["west"]) or (arms["east"] and arms["west"] and not arms["north"] and not arms["south"])
                up = not straight or self.v.get((x, y + 1, z)) not in (None, AIR)
                st = ",".join(f"{f}={'low' if arms[f] else 'none'}" for f in ("east", "north", "south"))
                new[(x, y, z)] = f"{base}[{st},up={str(up).lower()},waterlogged=false,west={'low' if arms['west'] else 'none'}]"
            else:
                st = ",".join(f"{f}={str(arms[f]).lower()}" for f in ("east", "north", "south"))
                new[(x, y, z)] = f"{base}[{st},waterlogged=false,west={str(arms['west']).lower()}]"
        self.v.update(new)


class Plot(Place):
    """The starter's place, with a generator's model in place of the empty room.

    What the model draws stands; every rule above still binds the handing
    (datum, seams, open sides, owed names). Inside the play space, above the
    floor, the model is everything: the empty place's room, lid and sconces
    give way to it. The ring wall stands where the place has no open side,
    and no corner post stands between two open sides."""

    BUILD = None  # (place) -> Model, set by `run`
    DELVEC = "delvec"

    def lay_parts(self, parts: list[dict]) -> None:
        self.model = type(self).BUILD(self)
        if not isinstance(self.model, Model):
            sys.exit("starter.py: the generator's build(place) returns no Model")
        self.model.shift(0)  # every read from here on is of the cells as they stand
        self.yield_approaches()
        by_stem = {p["id"].removeprefix("part/"): p for p in parts}
        unknown = sorted(set(self.model.region.values()) - set(by_stem))
        if unknown:
            sys.exit(f"starter.py: the model names part(s) the place does not hold: {unknown}; it holds {sorted(by_stem)}")
        missing = [f"part/{s} ({p['cells']} cell(s), {p['use']})" for s, p in by_stem.items() if s not in set(self.model.region.values())]
        if missing:
            sys.exit("starter.py: the model lays no column for " + ", ".join(missing) + "; name each part's columns with `Model.part`")
        self.parts, self.col_part = [], {}
        for stem, p in by_stem.items():
            e = {"part": p, "region": f"part-{stem}", "walked": p.get("reached", True), "cols": []}
            self.parts.append(e)
            for c, st in sorted(self.model.region.items()):
                if st == stem:
                    e["cols"].append(c)
                    self.col_part[c] = e
        self.reached: set = set()
        self.unwalked: set = set()
        self.standable: set = set()
        self.survey()
        if self.seen_now:
            self.survey()

    # -- what one cell is: the model's block over the empty place's claim --

    def base_cell(self, x: int, y: int, z: int) -> tuple:
        kind = Place.base_cell(self, x, y, z)
        if kind[0] == "claim" and kind[1].endswith("-way"):
            return kind  # a seam's opening, kept clear
        if kind[0] == "claim" and kind[1].endswith("-ladder"):
            return kind  # the ladder up to a raised opening, the starter's
        if self.in_pillar(x, y, z):
            # the pillar a ladder hangs on: a full block, whatever the drawing
            # put there, or the ladder holds nothing
            if "pillar" not in self.model.palette:
                floor = self.block_of(("fill", "floor"))
                self.model.palette["pillar"] = floor if is_full(floor) else "minecraft:stone_bricks"
            return ("fill", "pillar")
        r = self.model.get(x, y, z)
        c = (x, y, z)
        if r is not None and r != AIR:
            # A drawn block: claimed with the block in it where a body
            # stands in it (a flower, a carpet), out of walk where the floor
            # is one no body reaches, and only written everywhere else.
            if c in self.unwalked:
                return ("claim", "unwalked", r)
            e = self.col_part.get((x, z))
            if c in self.reached:
                region = kind[1] if kind[0] == "claim" and not self.in_space_xz(x, z) else None
                return ("claim", region or (e["region"] if e is not None else "room"), r)
            if e is not None and not e["walked"] and c in self.standable:
                return ("claim", e["region"], r)  # the floor of a part seen and never entered
            if self.in_space_xz(x, z) and not is_full(self.block_of(("fill", r))) and any(
                    (x + dx, y, z + dz) in self.model.no_touch for dx, dz in DIRS.values()):
                # A thin block beside a way in (a lantern, a post, a flower
                # pot) stands in the space the way opens into, so the way's
                # cells touch it (`contract-well-formed`).
                return ("claim", e["region"] if e is not None and e["walked"] else "room", r)
            return ("fill", r)
        if not self.in_space_xz(x, z):
            if kind in (("fill", "wall"), ("fill", "lamp")) and self.beside_side(x, z) and not self.before_opening(x, y, z):
                return ("claim", "room", None)  # no corner post marks an open place's edge
            if kind[0] == "claim" and kind[1].startswith("side-"):
                # An open side's ring cell is a way out only where the cell
                # inside it is air a body walks; beside a wall, a hedge, an
                # eave or a bush it is the place's own air, the room's.
                lo, hi = self.space
                inner = (min(max(x, lo[0]), hi[0]), y, min(max(z, lo[2]), hi[2]))
                ik = self.cell(*inner)
                if not (self.walkable(ik) or (ik[0] == "claim" and ik[1].endswith("-way"))):
                    return ("claim", "room", None)
            return kind
        if r is None and y < self.datum.value:
            return kind  # the ground the empty place lays
        if c in self.unwalked:
            return ("claim", "unwalked", None)
        return kind if kind[0] == "claim" else ("claim", "room", None)

    def yield_approaches(self) -> None:
        """Hand each raised opening the drawing climbs to itself
        (`Model.climbs_to`) to the drawing, and refuse a drawing that writes
        where the starter stands the pillar, landing or ladder of any other:
        the starter's ladder never overwrites a drawn stair."""
        m = self.model
        clashes = []
        for s in self.seams:
            ap = s["approach"]
            if ap is None or ap["ladder"] is None:
                continue
            if s["stem"] in m.climbed:
                s["approach"] = None
                continue
            lad = ap["ladder"]
            cells = []
            for (x, y, z), r in sorted(m.v.items()):
                if r == AIR or not self.datum.value <= y < ap["top"].value:
                    continue
                pillar = all(ap["plo"][i].value <= c < ap["phi"][i].value for i, c in ((0, x), (2, z)))
                ladder = lad["lo"][0] <= x < lad["hi"][0] and lad["lo"][2] <= z < lad["hi"][2] and y <= ap["sill"].value
                if pillar or ladder:
                    cells.append((x, y, z))
            if cells:
                shown = ", ".join(str(c) for c in cells[:6]) + (f" and {len(cells) - 6} more" if len(cells) > 6 else "")
                clashes.append(f"  edge/{s['stem']}: the opening stands {ap['rise']} course(s) over the walk plane, and the drawing writes {len(cells)} cell(s) where the starter stands its pillar, landing and ladder: {shown}")
        if clashes:
            sys.exit(
                "starter.py: the drawing builds where the starter stands the ladder up to a raised opening:\n"
                + "\n".join(clashes) + "\n"
                "If the drawing climbs to the opening itself (a flight, a ramp), say so with "
                "`Model.climbs_to(edge)` and the starter stands no ladder; otherwise leave those cells "
                "to the starter."
            )

    def in_pillar(self, x: int, y: int, z: int) -> bool:
        """A cell of the masonry pillar under a raised opening that a ladder
        climbs (`Place.approach`)."""
        d = self.datum.value
        for s in self.seams:
            ap = s["approach"]
            if ap is None or ap["ladder"] is None:
                continue
            if all(ap["plo"][i].value <= c < ap["phi"][i].value for i, c in ((0, x), (2, z))) and d <= y < ap["sill"].value:
                return True
        return False

    def before_opening(self, x: int, y: int, z: int) -> bool:
        """A cell straight out from a seam's opening, beyond its plane: the
        neighbour's side of the way, never this place's room — claimed room,
        it would stand the room on both sides of the opening and leave the
        way facing nowhere (`exterior_faces`)."""
        for s in self.seams:
            if s["axis"] == 1:
                continue
            axis, sign = FACES[s["seam"]["face"]]
            c = [x, y, z]
            c[axis] -= sign
            lo, hi = [b.value for b in s["lo"]], [b.value for b in s["hi"]]
            inside = lambda q: all(lo[i] <= q[i] < hi[i] for i in range(3))  # noqa: E731
            if inside(c) and not inside([x, y, z]):
                return True
        return False

    def beside_side(self, x: int, z: int) -> bool:
        """A ring column beside an open side's run: the corner between two
        open sides, where the empty place stands a post."""
        return any(self.open_side(x + dx, z + dz) is not None for dx in (-1, 0, 1) for dz in (-1, 0, 1))

    def block_of(self, kind: tuple) -> str:
        """The block a cell's kind writes (a weighted role's first variant)."""
        if kind[0] == "void":
            return "minecraft:air"
        role = kind[1] if kind[0] == "fill" else kind[2]
        if role is None:
            return "minecraft:air"
        if role.startswith("minecraft:"):
            return role
        return self.model.state(role)

    # -- the engine's answers --

    def survey(self) -> None:
        """Ask `delvec grammar survey` what the drawing does, and keep its answers."""
        m = self.model
        lo, hi = self.space
        d = self.datum.value
        voids = set()
        for v in self.a.get("voids", []) or []:
            (vx0, vy0, vz0), (vx1, vy1, vz1) = v["cells"]
            voids |= {(vx, vy, vz) for vx in range(vx0, vx1 + 1) for vy in range(vy0, vy1 + 1) for vz in range(vz0, vz1 + 1)}
        blocks = []
        for x in range(self.ex):
            for y in range(self.ey):
                for z in range(self.ez):
                    if (x, y, z) in voids:
                        blocks.append([x, y, z, "minecraft:structure_void"])
                        continue
                    b = self.block_of(self.cell(x, y, z))
                    if b != "minecraft:air":
                        blocks.append([x, y, z, b])
        # Where a body comes in: every seam's opening and every open side.
        start = []
        for s in self.seams:
            sl, sh = [b.value for b in s["lo"]], [b.value for b in s["hi"]]
            start += [[x, y, z] for x in range(sl[0], sh[0]) for y in range(sl[1], sh[1]) for z in range(sl[2], sh[2])]
        for sd in self.sides:
            start += [[x, yy, z] for x in range(sd["lo"][0], sd["hi"][0] + 1) for z in range(sd["lo"][1], sd["hi"][1] + 1) for yy in (d, d + 1)]
        spaces: dict[str, list] = {"room": []}
        for e in self.parts:
            if e["walked"]:
                spaces[e["region"]] = []
        for x in range(lo[0] - 1, hi[0] + 2):
            for z in range(lo[2] - 1, hi[2] + 2):
                if not self.in_space_xz(x, z) and self.open_side(x, z) is not None:
                    continue  # an open side's ring is the way out, no space's
                e = self.col_part.get((x, z))
                name = "room" if e is None else (e["region"] if e["walked"] else None)
                if name is None:
                    continue
                spaces[name] += [[x, y, z] for y in range(d, self.ey) if (x, y, z) not in m.no_touch]
        palette = sorted({b for r in set(m.v.values()) for b in m.states(r)})
        request = {
            "size": [self.ex, self.ey, self.ez],
            "blocks": blocks,
            "keep": [[lo[0] - 1, 0, lo[2] - 1], [hi[0] + 1, self.ey - 1, hi[2] + 1]],
            "from": start,
            "spaces": spaces,
            "palette": palette,
        }
        answer = ask_engine(type(self).DELVEC, request)
        problems = []
        for b in answer["malformed"]:
            problems.append(f"  {b['block']}: {b['why']}")
        for u in answer["unsupported"]:
            problems.append(f"  {u['block']} at {tuple(u['cell'])}: the server drops it; it needs {u['needs']}")
        for rf in answer["refused"]:
            route = f" — {rf['route']}" if rf.get("route") else ""
            problems.append(f"  {rf['block']}: the palette refuses it (DW0730){route}")
        if problems:
            sys.exit("starter.py: the model writes blocks the engine refuses:\n" + "\n".join(problems))
        if m.skipped:
            print(f"starter.py: {m.skipped} write(s) into a seam's opening skipped; the opening stays the connection's", file=sys.stderr)
        if answer["containers"]:
            print("starter.py: containers the holder census counts (DW1018, DW1024): " + ", ".join(answer["containers"]), file=sys.stderr)
        standable = self.standable = {tuple(c) for c in answer["standable"]}
        self.reached = {tuple(c) for c in answer["reached"]}
        pinched = self.pinches()
        if pinched:
            sys.exit(
                "starter.py: floor a body cannot reach reads as running on from floor it does reach, through "
                "the corner between two blocks that touch at their edges — a player sees a gap to squeeze "
                "through, and a body is wider than that corner:\n" + "\n".join(pinched) + "\n"
                "Open the corner (move one of the two blocks a cell, or widen the walk so it passes a body "
                "wide), or make that floor read as closed (stand something on it); the starter never "
                "declares such floor out of walk on its own."
            )
        self.joined = [tuple(p) for p in answer["joined"]]
        unwalked = set()
        for c in standable | {tuple(c) for c in answer["sealed"]}:
            x, y, z = c
            if not self.in_space_xz(x, z) or c in self.reached or y < 1:
                continue
            e = self.col_part.get((x, z))
            if e is None or e["walked"]:
                unwalked.add(c)
        self.unwalked = unwalked
        self.seen_now = []
        for e in self.parts:
            cols = set(e["cols"])
            if e["walked"] and not any((x, z) in cols for (x, _, z) in self.reached):
                # No column of it holds floor a body walks onto from the ways
                # in: a kiln, a stack, a shed drawn shut. The program declares
                # it seen and never entered, and the survey is asked again.
                e["walked"] = False
                self.seen_now.append(e["part"]["id"])
                print(f"starter.py: {e['part']['id']} has no walkable column — no body reaches any of its floor from the ways in — so the program declares it seen and never entered. If it is meant to be entered, open a way into it; if not, declare it `reached: false` in the layout graph's holds[], or `detail --check` refuses it (DW1074)", file=sys.stderr)

    def pinches(self) -> list[str]:
        """Every unreached standable cell of the play space that sits
        diagonally beside a reached one at the same course, the two cells
        between them each holding a block a body cannot stand in: floor that
        reads as joined through a corner no body passes. One line each."""
        out = []
        for (x, y, z) in sorted(self.standable - self.reached):
            if not self.in_space_xz(x, z) or y < 1:
                continue
            for dx in (-1, 1):
                for dz in (-1, 1):
                    v = (x + dx, y, z + dz)
                    if v not in self.reached:
                        continue
                    between = [(x + dx, y, z), (x, y, z + dz)]
                    blocks = [self.block_of(self.cell(*c)) for c in between]
                    if all(c not in self.standable and b not in ("minecraft:air", "minecraft:structure_void")
                           for c, b in zip(between, blocks)):
                        out.append(f"  {(x, y, z)} beside reached {v}, between {blocks[0]} at {between[0]} and {blocks[1]} at {between[1]}")
        return out

    def cuts(self, axis: int) -> list[Bound]:
        out = super().cuts(axis)
        best = {b.value: b for b in out}
        for v in range(self.n[axis]):
            best.setdefault(v, lit(v))
        return [best[v] for v in sorted(best)]


def ask_engine(delvec: str, request: dict) -> dict:
    """`delvec grammar survey` over `request`, answered as JSON."""
    exe = shutil.which(delvec) or delvec
    with tempfile.TemporaryDirectory(prefix="starter-survey-", dir=os.environ.get("DELVEWRIGHT_SCRATCH")) as tmp:
        path = os.path.join(tmp, "request.json")
        with open(path, "w", encoding="utf-8") as f:
            json.dump(request, f)
        try:
            r = subprocess.run([exe, "grammar", "survey", "--file", path], capture_output=True, text=True, check=False)
        except FileNotFoundError:
            sys.exit(f"starter.py: no `{delvec}` to ask; pass --delvec, or source .out/env.sh")
    if r.returncode != 0:
        sys.exit(f"starter.py: `{delvec} grammar survey` exited {r.returncode}: {r.stderr.strip()}")
    return json.loads(r.stdout)


def plot_body(p: Plot, rules: Rules) -> dict:
    """The place cut on x, then y, then z, one cell at a time, equal
    neighbours merged into one span."""
    xs, ys, zs = p.cuts(0), p.cuts(1), p.cuts(2)

    def runs(cuts, items):
        out_c, out_i = [], []
        for c, it in zip(cuts, items):
            if out_i and json.dumps(out_i[-1], sort_keys=True) == json.dumps(it, sort_keys=True):
                continue
            out_c.append(c)
            out_i.append(it)
        return out_c, out_i

    columns = []
    for x in xs:
        courses = []
        for y in ys:
            row = [leaf(p.cell(x.value, y.value, z.value)) for z in zs]
            cz, rz = runs(zs, row)
            courses.append(rules.name(split("z", cz, rz), "row"))
        cy, ry = runs(ys, courses)
        columns.append(rules.name(split("y", cy, ry), "column"))
    cx, rx = runs(xs, columns)
    return split("x", cx, rx)


def detached_openings(p: Plot) -> list[str]:
    """Every cell of a seam's opening that touches no cell of the room its
    edge enters, as the drawing claims it: one line per opening, naming what
    stands behind each such cell.

    The contract's well-formedness asks it of every opening to `exterior`
    (`contract-well-formed`, `docs/reference/grammar.md` §2d): each cell of
    the opening touches the space the edge enters, or a cell of another of
    its openings that does. A drawn plot breaks it where the cells behind the
    opening are a part's columns, a full block, or floor no body reaches —
    a yard named on the front row, a cauldron in the doorway's lee. The
    starter asks it of the claims it is about to write, and refuses before
    writing, naming each cell and what is behind it."""
    out = []
    openings = []
    for s in p.seams:
        if s["bar"] is not None or s["approach"] is not None or s["axis"] == 1:
            continue  # barred, raised or through a floor: answered otherwise
        lo, hi = [b.value for b in s["lo"]], [b.value for b in s["hi"]]
        cells = {(x, y, z) for x in range(lo[0], hi[0]) for y in range(lo[1], hi[1]) for z in range(lo[2], hi[2])}
        openings.append((s, cells))

    def room(c) -> bool:
        k = p.cell(*c)
        return k[0] == "claim" and k[1] == "room"

    for s, cells in openings:
        others = set().union(*[c for t, c in openings if t is not s]) if len(openings) > 1 else set()
        # sides answered by `side-*` edges of the room count as other openings
        for sd in p.sides:
            for x in range(sd["lo"][0], sd["hi"][0] + 1):
                for z in range(sd["lo"][1], sd["hi"][1] + 1):
                    for y in range(p.datum.value, p.wall_top.value):
                        if p.cell(x, y, z) == ("claim", f"side-{sd['face']}", None):
                            others.add((x, y, z))
        hops = {c for c in others if any(room(n) for n in _around(c) if n not in others)}
        bad = []
        for c in sorted(cells):
            near = [n for n in _around(c) if n not in cells]
            if any(room(n) for n in near) or any(n in hops for n in near):
                continue
            bad.append(c)
        if not bad:
            continue
        axis, sign = FACES[s["seam"]["face"]]
        behind = []
        for c in bad[:8]:
            n = list(c)
            n[axis] -= sign
            k = p.cell(*n)
            e = p.col_part.get((n[0], n[2]))
            drawn = p.model.get(*n)
            if e is not None and k[0] == "claim" and k[1] == e["region"]:
                what = f"the columns of {e['part']['id']}"
            elif k[0] == "claim" and k[1] == "unwalked":
                what = "floor no body reaches, claimed `unwalked`"
            elif drawn is not None and drawn != AIR:
                what = f"`{p.block_of(('fill', drawn))}`, a block a body is not in"
            else:
                what = f"`{k[1] if len(k) > 1 else k[0]}`"
            behind.append(f"{c} (behind it, {tuple(n)}: {what})")
        out.append(
            f"  `{s['seam']['edge']}`: {len(bad)} cell(s) of its opening touch no cell of the room — "
            + "; ".join(behind)
            + (f"; … ({len(bad)} in all)" if len(bad) > 8 else "")
        )
    return out


def _around(c):
    x, y, z = c
    return [(x + 1, y, z), (x - 1, y, z), (x, y + 1, z), (x, y - 1, z), (x, y, z + 1), (x, y, z - 1)]


def plot_program(a: dict, reason: str) -> dict:
    """The place's program from the generator's model."""
    p = Plot(a)
    m = p.model
    detached = detached_openings(p)
    if detached:
        sys.exit(
            "starter.py: a seam's opening must touch the room a body enters by, cell for cell "
            "(`contract-well-formed`: every cell of an opening to the outside touches the space "
            "its edge enters, or a cell of another of its openings that does), and the drawing "
            "stands something else behind it:\n" + "\n".join(detached) + "\n"
            "Keep the row behind each opening the plot's own ground from the walk plane to the "
            "opening's head: no part's columns (`Model.part`), no full block; start a yard or a "
            "bench one row in."
        )
    rules = Rules()
    SPLITS.clear()
    # Every literal state the model holds becomes a palette role, named in a
    # fixed order so the program is byte-stable.
    literal = sorted({r for r in m.v.values() if r != AIR and r.startswith("minecraft:")})
    name_of = {s: f"b{i}" for i, s in enumerate(literal)}

    def role(r: str) -> str:
        return name_of.get(r, r)

    def leaf_of(kind: tuple) -> dict:
        if kind[0] == "fill":
            return fill(role(kind[1]))
        if kind[0] == "void":
            return VOID
        _, region, r = kind
        return claim(region, fill(role(r)) if r else VOID)

    global leaf
    starter_leaf = leaf
    leaf = leaf_of
    try:
        start = marks(p, plot_body(p, rules))
    finally:
        leaf = starter_leaf
    rules.rules["place"] = start_rule(p, start)
    palette = dict(m.palette)
    for s, n in name_of.items():
        palette[n] = s
    for s in p.seams:
        if s["bar"] is not None:
            palette[f"bar-{s['stem']}"] = full_state(s["seam"]["bar_block"])
    used: set = set()

    def scan(o) -> None:
        if isinstance(o, dict):
            if o.get("op") == "fill":
                used.add(o["material"]["role"])
            for v in o.values():
                scan(v)
        elif isinstance(o, list):
            for v in o:
                scan(v)

    scan(rules.rules)
    palette = {k: v for k, v in sorted(palette.items()) if k in used}
    c = contract(p)
    # Two walked spaces are joined by a walk where the engine proves a body
    # crosses from one to the other, never where their columns merely touch.
    walked = {e["region"] for e in p.parts if e["walked"]} | {"room"}
    c["edges"] = [e for e in c["edges"] if not (e["class"] == "walk" and e["a"] in walked and e["b"] in walked and "via" not in e)]
    # A walk between two spaces whose floors differ carries the rise the
    # check measures: the lowest course each space's claimed cells reach
    # (`min_y(b) - min_y(a)` over the resolved boxes).
    low: dict[str, int] = {}
    cols: dict[str, set] = {}
    for x in range(p.ex):
        for y in range(p.ey):
            for z in range(p.ez):
                k = p.cell(x, y, z)
                if k[0] == "claim":
                    cols.setdefault(k[1], set()).add((x, z))
                    if k[1] not in low or y < low[k[1]]:
                        low[k[1]] = y
    # Each part's claimed columns against the band `DW1074` allows, as the
    # allocation hands it (`bands`): the projection of the region naming it.
    bands = {b["part"]: b for b in a.get("bands", []) or []}
    sizes = []
    for e in p.parts:
        b = bands.get(e["part"]["id"])
        if b is None:
            continue
        n = len(cols.get(e["region"], ()))
        out = "" if b["min"] <= n <= b["max"] else " — OUT OF BAND (DW1074)"
        sizes.append(f"{e['part']['id']} {n} of {b['min']}..{b['max']}{out}")
    if sizes:
        print("starter.py: part sizes, claimed columns of the band: " + "; ".join(sizes), file=sys.stderr)
    for x, y in sorted(p.joined):
        e = {"a": x, "b": y, "class": "walk"}
        if x in low and y in low and low[y] != low[x]:
            e["rise"] = low[y] - low[x]
        c["edges"].append(e)
    if p.unwalked:
        c.setdefault("no_body", {})["unwalked"] = {"reason": reason}
        floor = sum(1 for cell in p.reached)
        if len(p.unwalked) >= floor:
            c["no_body_majority_ack"] = reason
    faces = sorted({"east", "north", "south", "west"} | ({"down"} if p.aloft else set()) | m.shown)
    return {
        "contract": c,
        "name": a["place"].removeprefix("node/"),
        "palette": palette,
        "params": dict(sorted(p.h.params.items())),
        "rules": rules.rules,
        "shown_faces": sorted(faces),
        "start": "place",
        "version": "1.14.0",
    }


def run(build=None, argv: list[str] | None = None, reason: str = "roof slopes, wall heads and sills: seen, never stood on") -> int:
    """Write the place's program: the empty place with no `build`, the
    generator's model with one. `reason` is the out-of-walk reason the
    unreached floor is claimed with."""
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--allocation", required=True, help="`delvec allocation <campaign> <place>`, saved")
    ap.add_argument("--out", required=True, help="where the program is written")
    ap.add_argument("--delvec", default="delvec", help="the `delvec` a drawn model is surveyed with (default: the one on PATH)")
    args = ap.parse_args(argv)
    with open(args.allocation, encoding="utf-8") as f:
        allocation = json.load(f)
    if build is None:
        doc = program(allocation)
    else:
        Plot.BUILD = staticmethod(build)
        Plot.DELVEC = args.delvec
        doc = plot_program(allocation, reason)
    text = json.dumps(doc, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    with open(args.out, "w", encoding="utf-8") as f:
        f.write(text)
    return 0


def main(argv: list[str] | None = None) -> int:
    return run(None, argv)


def __getattr__(name: str):
    """The settlement helpers (`town.py`, beside this file) are the starter's
    too: `from starter import stretch, street_front` reaches them, loaded on
    first use so a bare run needs nothing but this file."""
    if name.startswith("__"):
        raise AttributeError(name)
    town = sys.modules.get("town")
    if town is None:
        # the file beside this one, by path: a campaign's own `town.py` on
        # the generator's path is never taken for it
        import importlib.util

        spec = importlib.util.spec_from_file_location("town", os.path.join(os.path.dirname(os.path.abspath(__file__)), "town.py"))
        town = importlib.util.module_from_spec(spec)
        sys.modules["town"] = town
        spec.loader.exec_module(town)
    try:
        return getattr(town, name)
    except AttributeError:
        raise AttributeError(f"module 'starter' has no attribute {name!r}") from None


if __name__ == "__main__":
    raise SystemExit(main())
