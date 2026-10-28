#!/usr/bin/env python3
"""The starter generator for ONE place: an empty-but-correct program, read off its allocation.

`scripts/handout.py` copies this file beside every handout, with the place's
allocation saved next to it, so a place's author starts from a program that
already passes `delvec detail --check` and builds the place into it — never
from research. Run it as the handout prints it:

    "$DELVEWRIGHT_PYTHON" starter.py --allocation allocation.json --out <campaign>/programs/<stem>.json

It runs no binary and names no path: it reads the allocation `delvec allocation
<campaign> <place>` printed and writes the program. What it emits, for any
allocation the engine hands:

- the ground: a ground place's floor drawn down to the bottom of its claim; an
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
- a sconce every four cells of wall a course above the floor, and on a roofed
  place a lamp every four cells of the lid, so the light probe has something
  to measure.

Every handed value it uses is declared as a `handed/…` parameter with the
allocation's value as its default, and the body reads it only through that
parameter (`references/detail.md`, *The program's contract*). A number that is
not handed — where a mark stands, how far apart the lamps are — is this
starter's choice and yours to change.

The building is yours. The places to put it are marked `# ── BUILD HERE`, each
naming the idiom of `docs/reference/grammar.md` §2c it reaches for and the step
of `references/new-pieces.md` that says how. Edit a copy under
`<campaign>/design/generator/<stem>.py`, re-run it, and check after every edit:
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
        self.sides = []
        for s in a.get("sides", []) or []:
            (sx0, _, sz0), (sx1, _, sz1) = s["cells"]
            self.sides.append({"face": s["face"], "lo": (sx0, sz0), "hi": (sx1, sz1)})
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

        The columns beside every opening, ladder, landing, gate and open side
        stay the `room`, so every edge the room declares keeps its cells; the
        rest of the play space is walked in rows, a part a body walks taking
        the next run of its `cells` columns and a part seen and never entered
        taking columns from the far end, each one a solid mass under a
        no-body region, only where the walked floor stays one piece. Where
        the parts stand is this starter's choice and yours to change; the
        counts are the plan's.
        """
        if not parts:
            return
        lo, hi = self.space
        d = self.datum.value
        top = self.lid.value if self.lid is not None else self.wall_top.value
        plain = {("claim", "room", None), ("claim", "roof", None), ("fill", "floor"), ("fill", "wall"), ("fill", "lid"), ("fill", "lamp"), ("void",)}
        special = set()
        for x in range(lo[0] - 1, hi[0] + 2):
            for z in range(lo[2] - 1, hi[2] + 2):
                if any(self.cell(x, y, z) not in plain for y in range(d - 1, top + 1)):
                    special.add((x, z))
        free = []
        keep = set()
        for x in range(lo[0], hi[0] + 1):
            zs = range(lo[2], hi[2] + 1) if (x - lo[0]) % 2 == 0 else range(hi[2], lo[2] - 1, -1)
            for z in zs:
                if any((x + i, z + k) in special for i in (-1, 0, 1) for k in (-1, 0, 1)):
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
        walked = set(keep) | set(free)

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

    def wall_ring(self, x: int, z: int) -> bool:
        lo, hi = self.space
        return (lo[0] - 1 <= x <= hi[0] + 1 and lo[2] - 1 <= z <= hi[2] + 1) and not self.in_space_xz(x, z)

    # -- what one cell is, by the rules above --
    def cell(self, x: int, y: int, z: int) -> tuple:
        kind = self.base_cell(x, y, z)
        part = getattr(self, "col_part", {}).get((x, z))
        if part is None or kind != ("claim", "room", None):
            return kind
        return ("claim", part["region"], None if part["walked"] else "wall")

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
            return ("fill", "floor")
        if self.lid is not None and y == self.lid.value:
            if self.in_space_xz(x, z) and (x % LAMP_PERIOD, z % LAMP_PERIOD) == (2, 2):
                return ("fill", "lamp")
            return ("fill", "lid")
        if self.lid is not None and y > self.lid.value:
            return ("claim", "roof", None)
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
        """Every coordinate on `axis` where what a cell is can change."""
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
        best: dict[int, Bound] = {}
        for b in out:
            if 0 <= b.value <= n and (b.value not in best or (b.handed and not best[b.value].handed)):
                best[b.value] = b
        best[0] = lit(0)
        return [best[v] for v in sorted(best) if v < n]


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


def leaf(kind: tuple) -> dict:
    if kind[0] == "fill":
        return fill(kind[1])
    if kind[0] == "void":
        return VOID
    _, region, role = kind
    return claim(region, fill(role) if role else VOID)


def split(axis: str, cuts: list[Bound], children: list[dict]) -> dict:
    if len(children) == 1:
        return children[0]
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
                    if p.walkable(p.cell(x2, p.datum.value, z2)) and p.walkable(p.cell(x2, p.datum.value + 1, z2)) and p.cell(x2, p.datum.value - 1, z2) == ("fill", "floor"):
                        taken.add((x2, z2))
                        return x2, z2
        sys.exit("starter.py: no free floor cell for a mark; the play space is full")

    out = inner
    for name in reversed(p.a["owed_anchors"]):
        stem = name.removeprefix("anchor/")
        if stem.startswith("seam-") or any(g["stem"] == stem for g in p.gates):
            continue  # answered by a barred edge's region, never a mark
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
        # space's edge, inside, and the side stays this edge's.
        edges.append({"a": "room", "b": "exterior", "class": "walk", "via": f"side-{s['face']}"})
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
    # ── BUILD HERE: the building. The body below is the empty place — floor,
    # ring wall, lid, openings, sconces. Replace the `room` boxes with the
    # rooms the design draws, starting from the idioms (grammar.md §2c: 1
    # repetition for a colonnade or a run of bays, 3 shape for an arch or a
    # gable, 7 symmetry for a front, 9 light for where the lamps stand) and the
    # procedure (references/new-pieces.md step 3, the program; step 4, the
    # gates). Keep every seam's region and every mark; they are what binds.
    start = marks(p, body(p, rules))
    rules.rules["place"] = [{"body": start, "weight": 1}]
    palette = dict(PALETTE)
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


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--allocation", required=True, help="`delvec allocation <campaign> <place>`, saved")
    ap.add_argument("--out", required=True, help="where the program is written")
    args = ap.parse_args(argv)
    with open(args.allocation, encoding="utf-8") as f:
        allocation = json.load(f)
    text = json.dumps(program(allocation), indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    with open(args.out, "w", encoding="utf-8") as f:
        f.write(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
