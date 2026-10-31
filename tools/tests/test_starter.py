"""`scripts/starter.py` answers an open side as it answers a seam (spec-0123 §5.3).

The starter is judged whole by `crates/delvec/tests/starter_program.rs`
(every gallery place, `detail --check`, unedited); this file pins the one
shape that test cannot name by itself: a side the place declares open is left
open in the ring from the walk plane up and is the `via` of a `walk` edge to
`exterior`, and a place with no open side has neither.
"""

from __future__ import annotations

import json
import pathlib
import subprocess
import sys

REPO = pathlib.Path(__file__).resolve().parents[2]
STARTER = REPO / ".claude" / "skills" / "delvewright" / "skills" / "new-delve" / "scripts" / "starter.py"
ALLOCATIONS = REPO / "tools" / "tests" / "fixtures" / "handout" / "allocations.json"


def program(tmp: pathlib.Path, place: str) -> dict:
    a = next(x for x in json.loads(ALLOCATIONS.read_text(encoding="utf-8")) if x["place"] == place)
    src, out = tmp / "allocation.json", tmp / "program.json"
    src.write_text(json.dumps(a), encoding="utf-8")
    r = subprocess.run([sys.executable, "-I", str(STARTER), "--allocation", str(src), "--out", str(out)], capture_output=True, text=True)
    assert r.returncode == 0, r.stderr
    return json.loads(out.read_text(encoding="utf-8"))


def regions(node) -> set[str]:
    found: set[str] = set()
    if isinstance(node, dict):
        if node.get("op") == "claim":
            found.add(node["region"])
        for v in node.values():
            found |= regions(v)
    elif isinstance(node, list):
        for v in node:
            found |= regions(v)
    return found


def test_an_open_side_is_left_open_and_is_a_walk_edge_to_exterior(tmp_path: pathlib.Path):
    p = program(tmp_path, "node/yard")
    assert {"a": "room", "b": "exterior", "class": "walk", "via": "side-south"} in p["contract"]["edges"]
    assert "side-south" in regions(p["rules"])


def test_a_place_with_no_open_side_has_no_side_region(tmp_path: pathlib.Path):
    p = program(tmp_path, "node/hall")
    assert not [e for e in p["contract"]["edges"] if str(e.get("via", "")).startswith("side-")]
    assert not [r for r in regions(p["rules"]) if r.startswith("side-")]


def program_of(tmp: pathlib.Path, a: dict) -> dict:
    src, out = tmp / "allocation.json", tmp / "program.json"
    src.write_text(json.dumps(a), encoding="utf-8")
    r = subprocess.run([sys.executable, "-I", str(STARTER), "--allocation", str(src), "--out", str(out)], capture_output=True, text=True)
    assert r.returncode == 0, r.stderr
    return json.loads(out.read_text(encoding="utf-8"))


def test_a_side_with_nothing_of_its_own_to_claim_is_not_declared(tmp_path: pathlib.Path):
    """A side whose run lies outside the frame (or is every cell a seam's
    opening) has no ring cell of its own: the starter does not declare it,
    so no `side-*` edge names a region nothing claims."""
    a = next(x for x in json.loads(ALLOCATIONS.read_text(encoding="utf-8")) if x["place"] == "node/yard")
    ex = a["extent"][0]
    a = dict(a)
    a["sides"] = list(a["sides"]) + [{"face": "north", "cells": [[1, a["datum_y"], -1], [ex - 2, a["datum_y"], -1]], "contacts": [], "walk_y": a["datum_y"]}]
    p = program_of(tmp_path, a)
    assert "side-north" not in regions(p["rules"])
    assert not [e for e in p["contract"]["edges"] if e.get("via") == "side-north"]
    # the side it does own is still declared
    assert {"a": "room", "b": "exterior", "class": "walk", "via": "side-south"} in p["contract"]["edges"]


def _starter_module():
    import importlib.util

    spec = importlib.util.spec_from_file_location("starter_under_test", STARTER)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


class _Datum:
    value = 1
    expr = {"expr": "int", "value": 1}


class _Place:
    """The fields `marks` reads, over a place whose floor no body reaches."""

    a = {"space": [[0, 0, 0], [4, 3, 4]], "owed_anchors": ["anchor/sign-bakery"]}
    ex = ez = 5
    datum = _Datum()
    gates: list = []
    seams: list = []

    def __init__(self, drawn: dict):
        self.model = type("M", (), {"marks": drawn})()

    def walkable(self, cell) -> bool:
        return False

    def cell(self, x, y, z):
        return ("fill", None, None)


def test_a_mark_given_a_course_names_the_block_at_that_cell():
    """`Model.mark(stem, x, z, y=…)` names a block's own cell — a sign a
    `signs` entry writes — so no floor is asked of it (spec-0128 §8); the
    same name with no course is refused off walkable floor."""
    st = _starter_module()
    out = st.marks(_Place({"sign-bakery": (2, 3, "south", 3)}), {"op": "skip"})
    assert out["mark"] == {
        "anchor": "sign-bakery",
        "at": "offset",
        "facing": "south",
        "y": {"expr": "int", "value": 3},
        "x": {"expr": "int", "value": 2},
        "z": {"expr": "int", "value": 3},
    }
    import pytest

    with pytest.raises(SystemExit) as e:
        st.marks(_Place({"sign-bakery": (2, 3, "south", None)}), {"op": "skip"})
    assert "not on floor a body reaches" in str(e.value)


# ---------------------------------------------------------------- the handed ground


def test_the_ground_course_is_the_ground_the_whole_hands_each_column():
    """`ground.columns` hands each column's blocks (spec-0128): the starter
    paints the ground course in them — a ring column from its nearest
    column inside — and falls back to the `floor` role only where nothing is
    handed."""
    st = _starter_module()
    a = next(x for x in json.loads(ALLOCATIONS.read_text(encoding="utf-8")) if x["place"] == "node/yard")
    a = json.loads(json.dumps(a))
    lo, hi = a["space"]
    cols = a["ground"]["columns"]
    assert cols, "the binding: a handed column"
    for c in cols:
        c["blocks"] = ["minecraft:coarse_dirt" if (c["x"] + c["z"]) % 2 else "minecraft:grass_block[snowy=false]"]
    p = st.Place(a)
    d = a["datum_y"]
    assert p.ground_roles == {"minecraft:coarse_dirt": "ground-0", "minecraft:grass_block[snowy=false]": "ground-1"}
    by = {(c["x"], c["z"]): c["blocks"][0] for c in cols}
    for (x, z), b in by.items():
        assert p.cell(x, d - 1, z) == ("fill", p.ground_roles[b]), (x, z)
    # a ring column takes its nearest column inside
    x, z = lo[0] - 1, lo[2]
    assert p.cell(x, d - 1, z) == ("fill", p.ground_roles[by[(lo[0], lo[2])]])
    # nothing handed: the `floor` role, as before
    a["ground"]["columns"] = []
    assert st.Place(a).cell(lo[0], d - 1, lo[2]) == ("fill", "floor")


# ---------------------------------------------------------------- an opening touches the room


def _plot_of(st, a: dict, build, monkeypatch):
    """A drawn plot over `a`, the engine's survey answered without a binary:
    every air cell over a solid one in the play space is standable and
    reached, nothing sealed."""

    def survey(_delvec, request):
        solid = {(x, y, z) for x, y, z, _ in request["blocks"]}
        stand = sorted([x, y, z] for (x, y, z) in {(x, y + 1, z) for x, y, z in solid} if (x, y, z) not in solid)
        return {"standable": stand, "reached": stand, "sealed": [], "joined": [], "unsupported": [],
                "refused": [], "containers": [], "malformed": []}

    monkeypatch.setattr(st, "ask_engine", survey)
    st.Plot.BUILD = staticmethod(build)
    return st.plot_program(a, "seen")


def _front_alloc() -> dict:
    """An open 9x6x9 plot whose north seam opens its whole first row,
    owned by the street, with one part to lay."""
    return {"place": "node/lot", "extent": [9, 6, 9], "space": [[1, 1, 1], [7, 5, 7]], "datum_y": 1,
            "ground": {"base": "ground", "columns": []},
            "seams": [{"edge": "edge/street-to-lot", "face": "north", "cells": [[2, 1, 1], [6, 3, 1]], "bar": None,
                       "owns_plane": False, "other": "node/street", "answer_with": ["walk"], "rise": 0}],
            "sides": [], "voids": [], "brief": {"kind": "open"}, "owed_anchors": [],
            "parts": [{"id": "part/yard", "cells": 9, "kind": "ground", "use": "the yard"}]}


def test_a_part_or_a_full_block_behind_an_opening_is_refused_before_writing(monkeypatch):
    """Every cell of a seam's opening touches the room its edge enters
    (`contract-well-formed`): a part named on the row behind it, or a full
    block standing there, is refused by the starter with the cell and what is
    behind it; the same part one row in passes."""
    import pytest

    st = _starter_module()

    def yard_at(z0):
        def build(place):
            m = st.Model(place)
            m.part(2, 4, z0, z0 + 2, "yard")
            return m
        return build

    with pytest.raises(SystemExit) as e:
        _plot_of(st, _front_alloc(), yard_at(2), monkeypatch)
    msg = str(e.value)
    # the opening's end column touches the room beside it, and its top course
    # the room above it: what is refused is what the engine would refuse
    assert "`edge/street-to-lot`: 4 cell(s)" in msg and "(3, 1, 1) (behind it, (3, 1, 2): the columns of part/yard)" in msg, msg

    def cauldron(place):
        m = st.Model(place)
        m.part(2, 4, 4, 6, "yard")
        m.set(5, 1, 2, "minecraft:water_cauldron[level=3]")
        return m

    with pytest.raises(SystemExit) as e:
        _plot_of(st, _front_alloc(), cauldron, monkeypatch)
    msg = str(e.value)
    assert "(5, 1, 1)" in msg and "minecraft:water_cauldron" in msg, msg

    doc = _plot_of(st, _front_alloc(), yard_at(4), monkeypatch)
    assert {"a": "room", "b": "exterior", "class": "walk", "via": "street-to-lot-way"} in doc["contract"]["edges"]


def test_part_sizes_name_every_column_named_for_a_part_and_left_out(monkeypatch, capsys):
    """The generator's part sizes are the count `DW1074` judges, and beside
    it every column the drawing named for the part that the claim leaves
    out, with why (`Model.unpart`): an author reads a band against what was
    drawn before running the check. A part with nothing left out is the
    count alone."""
    st = _starter_module()
    a = _front_alloc()
    a["bands"] = [{"part": "part/yard", "min": 6, "max": 12}]

    def build(place):
        m = st.Model(place)
        m.part(2, 4, 4, 6, "yard")
        m.unpart(3, 5, why="handed back for the test")
        return m

    _plot_of(st, a, build, monkeypatch)
    err = capsys.readouterr().err
    assert "part/yard 8 of 6..12 (9 named, 1 left out: 1 handed back for the test — (3,5))" in err, err

    def whole(place):
        m = st.Model(place)
        m.part(2, 4, 4, 6, "yard")
        return m

    _plot_of(st, a, whole, monkeypatch)
    err = capsys.readouterr().err
    assert "part/yard 9 of 6..12;" in err or err.rstrip().endswith("part/yard 9 of 6..12"), err
