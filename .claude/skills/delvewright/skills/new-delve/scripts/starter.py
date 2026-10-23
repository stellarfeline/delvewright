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
  harmless and nothing depends on it;
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


class Place:
    def __init__(self, a: dict) -> None:
        self.a = a
        self.h = Handed()
        self.ex, self.ey, self.ez = a["extent"]
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
        # The walls stand from the walk plane to the lid, or — open to the sky —
        # to the top of the play space.
        self.wall_top = self.lid if self.lid is not None else lit(a["space"][1][1] + 1)
        self.seams = []
        for s in a["seams"]:
            stem = s["edge"].removeprefix("edge/")
            (x0, y0, z0), (x1, y1, z1) = s["cells"]
            lo = [self.h(f"seam/{stem}/{k}", v) for k, v in (("x0", x0), ("y0", y0), ("z0", z0))]
            hi = [self.h(f"seam/{stem}/{k}", v).plus(1) for k, v in (("x1", x1), ("y1", y1), ("z1", z1))]
            barred = s.get("bar") is not None
            if barred and s["bar"] != s["cells"]:
                sys.exit(f"starter.py: the bar of `{s['edge']}` is not its opening; answer it by hand")
            self.seams.append({"stem": stem, "lo": lo, "hi": hi, "seam": s, "barred": barred})
        self.scenery = a.get("brief", {}).get("reached") is False

    # -- what one cell is, by the rules above --
    def cell(self, x: int, y: int, z: int) -> tuple:
        for s in self.seams:
            if all(s["lo"][i].value <= c < s["hi"][i].value for i, c in enumerate((x, y, z))):
                if s["barred"]:
                    return ("claim", f"seam-{s['stem']}", "bar")
                return ("claim", f"{s['stem']}-way", None)
        d = self.datum.value
        ring = x in (0, self.ex - 1) or z in (0, self.ez - 1)
        if y < d:
            if self.aloft and y < d - 1:
                return ("void",)
            return ("fill", "floor")
        if self.lid is not None and y == self.lid.value:
            if not ring and (x % LAMP_PERIOD, z % LAMP_PERIOD) == (2, 2):
                return ("fill", "lamp")
            return ("fill", "lid")
        if self.lid is not None and y > self.lid.value:
            return ("claim", "roof", None)
        if ring and y < self.wall_top.value:
            along = z if x in (0, self.ex - 1) else x
            if y == d + 1 and along % LAMP_PERIOD == 2 and 0 < along < (self.ez if x in (0, self.ex - 1) else self.ex) - 1:
                return ("fill", "lamp")
            return ("fill", "wall")
        if ring:
            return ("void",)
        return ("claim", "room", None)

    def cuts(self, axis: int) -> list[Bound]:
        """Every coordinate on `axis` where what a cell is can change."""
        n = (self.ex, self.ey, self.ez)[axis]
        out = [lit(0), lit(1), lit(n - 1)]
        for s in self.seams:
            out += [s["lo"][axis], s["hi"][axis]]
        if axis == 1:
            out += [self.datum.plus(-1), self.datum, self.datum.plus(1), self.datum.plus(2), self.wall_top]
            if self.lid is not None:
                out += [self.lid, self.lid.plus(1)]
        else:
            out += [lit(v) for v in range(2, n - 1, LAMP_PERIOD)] + [lit(v + 1) for v in range(2, n - 1, LAMP_PERIOD)]
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
                    if p.cell(x2, p.datum.value, z2) == ("claim", "room", None) and p.cell(x2, p.datum.value - 1, z2) == ("fill", "floor"):
                        taken.add((x2, z2))
                        return x2, z2
        sys.exit("starter.py: no free floor cell for a mark; the play space is full")

    out = inner
    for name in reversed(p.a["owed_anchors"]):
        stem = name.removeprefix("anchor/")
        if stem.startswith("seam-"):
            continue  # answered by the barred edge's region, never a mark
        mark = {"anchor": stem, "at": "offset", "facing": "south", "y": p.datum.expr}
        if stem == "spawn":
            mark["role"] = "entry"
        target = (cx, cz)
        if stem.startswith("unlock-"):
            s = next((s for s in p.seams if f"unlock-{s['stem']}" == stem), None)
            if s is not None:
                (x0, _, z0), (x1, _, z1) = s["seam"]["cells"]
                target = ((x0 + x1) // 2, (z0 + z1) // 2)
        x, z = free(*target)
        mark["x"], mark["z"] = num(x), num(z)
        out = {"body": out, "mark": mark, "op": "mark"}
    return out


def contract(p: Place) -> dict:
    """The spatial contract: one space, an edge per seam, the roof zone no body walks."""
    edges = []
    for s in p.seams:
        seam = s["seam"]
        if s["barred"]:
            edges.append({"a": "room", "b": "exterior", "bar": {"block": "bar", "region": f"seam-{s['stem']}"}, "class": "barred"})
            continue
        edge = {"a": "exterior", "b": "room", "class": seam["answer_with"][0], "via": f"{s['stem']}-way"}
        if seam["answer_with"][0] in ("stair", "drop", "climb") and seam.get("rise"):
            edge["rise"] = abs(seam["rise"])
        edges.append(edge)
    out = {
        "edges": edges,
        "entry": "room",
        # ── BUILD HERE: the spaces. A room a body walks around inside walls and
        # under a lid is `enclosed`; a street, a deck or a court open to the sky
        # is `open`. A second floor is a second space and a `climb` edge between
        # them (grammar.md §2d; references/detail.md, the program's contract).
        "spaces": {"room": {"envelope": "enclosed" if p.lid is not None else "open"}},
    }
    if p.lid is not None and p.lid.value + 1 < p.ey:
        out["no_body"] = {"roof": {"reason": "the roof zone over the lid, which no one is meant to walk"}}
    return out


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
        if s["barred"]:
            palette["bar"] = s["seam"]["bar_block"]
    climbs = any(e.get("class") == "climb" for e in contract(p)["edges"])
    return {
        "contract": contract(p),
        "name": f"{a['place'].removeprefix('node/')}-starter",
        "palette": palette,
        "params": dict(sorted(p.h.params.items())),
        "rules": rules.rules,
        # ── BUILD HERE: the faces the piece shows the world (grammar.md §2f).
        "shown_faces": ["east", "north", "south", "up", "west"],
        "start": "place",
        "version": "1.10.0" if climbs else "1.9.0",
    }


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
