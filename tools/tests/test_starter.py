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
