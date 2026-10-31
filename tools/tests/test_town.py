"""`scripts/town.py`, the starter's settlement helpers, each held to what it draws.

Every helper is a mechanism the caller parameterises (seed, clear width,
axis, edge, roles); each test below pins one property a broken helper would
lose — a way stated along x draws the transpose of the same way along z, a
doorway stays open, a seed moves the wear and nothing else — on a synthetic
allocation read through the starter's own `Place` and `Model`, with no
binary. The starter's order guard (`order_guard`) is pinned here on its
inputs, and on a real expansion by `crates/delvec/tests/starter_program.rs`.
"""

from __future__ import annotations

import importlib
import pathlib
import sys

import pytest

SCRIPTS = pathlib.Path(__file__).resolve().parents[2] / ".claude" / "skills" / "delvewright" / "skills" / "new-delve" / "scripts"
sys.path.insert(0, str(SCRIPTS))
starter = importlib.import_module("starter")
town = importlib.import_module("town")

AIR = starter.AIR


def seam(stem: str, face: str, cells, other: str, rise: int = 0) -> dict:
    return {"edge": f"edge/{stem}", "face": face, "cells": cells, "bar": None, "owns_plane": False,
            "other": other, "answer_with": ["walk"], "rise": rise}


def allocation(ex: int, ey: int, ez: int, seams=(), voids=()) -> dict:
    """An open place `ex` x `ey` x `ez`, its play space inside the one-cell ring, walk plane 1."""
    return {"place": "node/test", "extent": [ex, ey, ez], "space": [[1, 1, 1], [ex - 2, ey - 1, ez - 2]],
            "datum_y": 1, "ground": {"base": "ground"}, "seams": list(seams), "sides": [], "voids": list(voids),
            "brief": {"kind": "open"}, "owed_anchors": [], "parts": []}


def transpose_alloc(a: dict) -> dict:
    """The same place with x and z swapped (faces and cells with them)."""
    sw = {"west": "north", "east": "south", "north": "west", "south": "east", "up": "up", "down": "down"}
    t = dict(a)
    t["extent"] = [a["extent"][2], a["extent"][1], a["extent"][0]]
    t["space"] = [[c[2], c[1], c[0]] for c in a["space"]]
    t["seams"] = [dict(s, face=sw[s["face"]], cells=[[c[2], c[1], c[0]] for c in s["cells"]]) for s in a["seams"]]
    t["voids"] = [dict(v, cells=[[c[2], c[1], c[0]] for c in v["cells"]]) for v in a["voids"]]
    return t


def model(a: dict):
    return starter.Model(starter.Place(a))


def swap_state(s: str) -> str:
    """A state as it reads in the transposed frame (facings, axes and arms swapped)."""
    pairs = {"facing=west": "facing=north", "facing=north": "facing=west", "facing=east": "facing=south",
             "facing=south": "facing=east", "axis=x": "axis=z", "axis=z": "axis=x"}
    out = []
    for part in s.replace("[", ",[,").replace("]", ",],").split(","):
        out.append(pairs.get(part, part))
    return ",".join(out).replace(",[,", "[").replace(",],", "]")


def transposed(cells: dict) -> dict:
    return {(z, y, x): (swap_state(r) if r.startswith("minecraft:") else r) for (x, y, z), r in cells.items()}


# ---------------------------------------------------------------- readings


def test_the_hash_is_fixed_and_in_the_unit_interval():
    assert town.hsh(1, 2, 3) == town.hsh(1, 2, 3)
    assert town.hsh(1, 2, 3) != town.hsh(1, 2, 4)
    assert town.hsh(4, 0, 7) == pytest.approx(0.32846367359161377)
    assert all(0.0 <= town.hsh(i, j) < 1.0 for i in range(20) for j in range(20))


def test_seam_columns_reach_the_opening_and_its_padding():
    p = starter.Place(allocation(9, 6, 20, [seam("a-to-b", "west", [[0, 1, 5], [0, 3, 7]], "node/b")]))
    cols = town.seam_cols(p)
    assert (0, 5) in cols and (1, 8) in cols and (-1, 4) in cols
    assert (2, 5) not in cols and (0, 9) not in cols
    assert (1, 5) not in town.seam_cols(p, pad=0)


def test_a_way_frame_runs_along_the_axis_it_is_given():
    p = starter.Place(allocation(9, 6, 20))
    rows, cols, cell = town.way_frame(p, "z")
    assert rows == list(range(1, 19)) and cols == list(range(1, 8)) and cell(5, 2) == (2, 5)
    rows, cols, cell = town.way_frame(p, "x")
    assert rows == list(range(1, 8)) and cell(5, 2) == (5, 2)
    with pytest.raises(ValueError):
        town.way_frame(p, "y")


# ---------------------------------------------------------------- furniture


def test_a_lamp_is_a_post_with_a_light_on_it_from_the_walk_plane():
    m = model(allocation(9, 8, 9))
    town.lamp(m, 3, 4)
    assert [m.get(3, y, 4).split("[")[0] for y in (1, 2, 3)] == ["minecraft:oak_fence"] * 3
    assert m.get(3, 4, 4).startswith("minecraft:lantern[hanging=false")
    assert m.get(3, 0, 4) is None


def test_a_lamp_whose_light_stands_on_the_frames_top_course_declares_the_top_shown():
    """A lantern on the top course stands in the open air (`DW0885` refuses an
    undeclared side there): the helper that puts it there declares `up`; one
    under the top course declares nothing."""
    m = model(allocation(9, 5, 9))
    town.lamp(m, 3, 4)
    assert m.get(3, 4, 4).startswith("minecraft:lantern") and m.shown == {"up"}
    m = model(allocation(9, 8, 9))
    town.lamp(m, 3, 4)
    assert m.shown == set()


def test_a_bracket_hangs_the_sign_on_the_streets_own_post_and_names_its_cell():
    s = seam("street-to-shop", "west", [[0, 1, 6], [0, 3, 8]], "node/shop")
    p = starter.Place(allocation(9, 6, 20, [s]))
    m = starter.Model(p)
    took = town.bracket(m, p, p.seams[0], 5, "minecraft:oak_hanging_sign", "sign-shop")
    assert took == {(1, 5), (1, 6)}
    assert all(m.get(1, y, 5).startswith("minecraft:oak_fence") for y in range(1, 5))
    assert m.get(1, 5, 5) == m.get(1, 5, 6) == "minecraft:stripped_spruce_log[axis=z]"
    assert m.get(1, 4, 6) == "minecraft:oak_hanging_sign[attached=false,rotation=12,waterlogged=false]"
    assert m.marks["sign-shop"] == (1, 6, "south", 4)
    # no room beside a doorway that fills the edge: no bracket
    full = seam("street-to-shop", "west", [[0, 1, 1], [0, 3, 18]], "node/shop")
    p2 = starter.Place(allocation(9, 6, 20, [full]))
    assert town.bracket(starter.Model(p2), p2, p2.seams[0], 5, "minecraft:oak_hanging_sign", None) is None


# ---------------------------------------------------------------- ways


def test_a_way_is_paved_flush_with_its_kennel_down_the_middle():
    m = model(allocation(9, 6, 9))
    town.pave_way(m, 1, 7, 1, 7, "z", {"verge": "V", "kennel": "K", "kerb": "S", "paving": "P"})
    assert [m.get(x, 0, 3) for x in range(1, 8)] == ["V", "S", "P", "K", "P", "S", "V"]
    m2 = model(allocation(9, 6, 9))
    town.pave_way(m2, 1, 4, 1, 7, "z", {"verge": "V", "kennel": "K", "kerb": "S", "paving": "P"})
    assert "S" not in {m2.get(x, 0, 3) for x in range(1, 5)}  # narrower than five: no kerb
    assert all(m.get(x, 1, z) is None for x in range(1, 8) for z in range(1, 8))


def street_alloc():
    return allocation(11, 6, 24, [seam("street-to-bakery", "west", [[0, 1, 6], [0, 3, 8]], "node/bakery"),
                                  seam("street-to-cottage", "east", [[10, 1, 3], [10, 3, 20]], "node/cottage")])


def test_a_stretch_keeps_its_clear_way_and_its_fronts_doorways_open():
    a = street_alloc()
    m = model(a)
    f = town.stretch(m, starter.Place(a), "z", 3, clear=5, fronts={"node/bakery": "minecraft:spruce_hanging_sign"})
    assert (f["c0"], f["c1"]) == (3, 7)
    # nothing stands in the clear way at the walk plane or above it
    assert not [c for c in m.v if 3 <= c[0] <= 7 and c[1] >= 1]
    # the bakery's sign on the bracket; the verge before its doorway clear
    assert m.marks["sign-bakery"][3] == m.ey - 2
    assert all(m.get(1, y, z) is None for y in (1, 2) for z in (6, 7, 8) if (1, z) not in {(1, 5), (1, 6)})
    # the cottage's open frontage is no front: no sign, its verge furnished freely
    assert "sign-cottage" not in m.marks


def way_alloc(axis: str = "z", clear: int = 3, cols=(4, 6)) -> dict:
    """The street allocation with the way the plan hands it."""
    a = street_alloc()
    a["way"] = {"axis": axis, "clear": clear, "cols": list(cols)}
    return a


def test_a_stretch_takes_its_axis_and_clear_way_from_the_plan_and_refuses_a_retyped_one():
    """The plan's `way` is handed (`place.way`): the stretch draws its run and
    its clear columns from it with nothing stated but the seed, and a stated
    axis or width that differs from the plan's is refused by name."""
    a = way_alloc(cols=(5, 7))
    m = model(a)
    f = town.stretch(m, starter.Place(a), seed=3)
    assert (f["axis"], f["c0"], f["c1"]) == ("z", 5, 7), "the handed columns, not centred"
    assert not [c for c in m.v if 5 <= c[0] <= 7 and c[1] >= 1]
    same = model(a)
    town.stretch(same, starter.Place(a), "z", 3, clear=3)  # the plan's own numbers, restated
    assert same.v == m.v
    for bad in ({"axis": "x"}, {"clear": 5}):
        with pytest.raises(SystemExit) as e:
            town.stretch(model(a), starter.Place(a), seed=3, **bad)
        assert "the plan hands this way" in str(e.value)
    with pytest.raises(SystemExit) as e:
        town.stretch(model(street_alloc()), starter.Place(street_alloc()), seed=3)
    assert "declares no `way`" in str(e.value)


def test_no_default_verge_or_lane_thing_is_a_container():
    """A barrel is a holder the census counts (`DW1018`): no helper's default
    furniture is one, so a default never refuses a campaign that declares no
    scenery."""
    for things in (town.VERGE_THINGS, town.LANE_THINGS):
        assert "barrel" not in things
    a = allocation(11, 6, 40)
    for seed in range(8):
        m = model(a)
        town.stretch(m, starter.Place(a), "z", seed, clear=5)
        town.lane(m, starter.Place(a), "z")
        assert not [r for r in m.v.values() if "barrel" in r], seed


def test_verge_life_places_what_the_caller_states_where_it_states_it():
    """`at` places each `(kind, row, side)` first, the cycle fills round it,
    `things=()` places `at` alone, and a stated column that is not free verge
    is refused."""
    a = allocation(11, 6, 40)
    m = model(a)
    placed = []

    def verge(m, f):
        rows = f["rows"]
        placed.extend(town.verge_life(m, f, things=(), at=[("lamp", rows[2], "low"), ("lamp", rows[-3], "high")]))

    f = town.stretch(m, starter.Place(a), "z", 1, clear=5, verge=verge)
    rows = f["rows"]
    assert [(k, r, s) for k, r, s, _, _ in placed] == [("lamp", rows[2], "low"), ("lamp", rows[-3], "high")]
    lanterns = sorted((x, z) for (x, y, z), r in m.v.items() if r.startswith("minecraft:lantern"))
    assert lanterns == sorted((x, z) for _, _, _, x, z in placed)
    assert lanterns[0][0] == f["c0"] - 1 and lanterns[1][0] == f["c1"] + 1
    with pytest.raises(SystemExit) as e:
        town.stretch(model(a), starter.Place(a), "z", 1, clear=5,
                     verge=lambda m, f: town.verge_life(m, f, at=[("lamp", f["rows"][0], "low")]))
    assert "not free verge" in str(e.value)


def test_a_stretch_along_x_is_the_transpose_of_the_same_stretch_along_z():
    a = street_alloc()
    mz = model(a)
    town.stretch(mz, starter.Place(a), "z", 5, clear=5)
    t = transpose_alloc(a)
    mx = model(t)
    town.stretch(mx, starter.Place(t), "x", 5, clear=5)
    assert mx.v == transposed(mz.v)


def test_a_stretchs_seed_moves_its_wear_and_the_same_seed_repeats_it():
    a = street_alloc()
    one, again, other = model(a), model(a), model(a)
    for m, seed in ((one, 1), (again, 1), (other, 2)):
        town.stretch(m, starter.Place(a), "z", seed, clear=5)
    assert one.v == again.v
    assert one.v != other.v


def test_a_market_on_a_stretchs_verge_stands_stalls_only_on_free_columns():
    a = allocation(13, 6, 24)
    m = model(a)
    f = town.stretch(m, starter.Place(a), "z", 4, clear=5, verge=town.market)
    awnings = [c for c, r in m.v.items() if c[1] == 4 and r.endswith("_wool")]
    assert awnings
    assert all((x, z) in f["free"] for x, _, z in awnings)


def flight_alloc(face: str):
    up = [[1, 1, 0], [5, 4, 0]] if face == "north" else [[1, 1, 9], [5, 4, 9]]
    down = [[1, 1, 9], [5, 2, 9]] if face == "north" else [[1, 1, 0], [5, 2, 0]]
    return allocation(7, 6, 10, [seam("up", face, up, "node/up", rise=2),
                                 seam("down", "south" if face == "north" else "north", down, "node/down")])


def test_a_flight_climbs_toward_the_opening_that_rises():
    for face, top_row in (("north", 1), ("south", 8)):
        a = flight_alloc(face)
        m = model(a)
        town.flight(m, starter.Place(a), {"verge": "V", "kennel": "K", "kerb": "S", "paving": "P"})
        assert m.get(3, 1, top_row) == "S" and m.get(3, 2, top_row) == "S"
        step = 1 + 2 if face == "north" else 8 - 2
        assert m.get(3, 1, step) == starter.stairs("minecraft:stone_brick", face)


def test_a_lanes_furniture_stands_on_the_edge_it_is_given():
    a = allocation(30, 6, 7)
    for edge, col in (("high", 5), ("low", 1)):
        m = model(a)
        town.lane(m, starter.Place(a), "x", edge=edge)
        placed = {c[2] for c in m.v if c[1] == 1}
        assert placed == {col}


def test_a_ramp_climbs_its_rise_a_stair_then_a_landing():
    a = allocation(7, 8, 14, [seam("up", "north", [[1, 1, 0], [5, 6, 0]], "node/up", rise=3)])
    m = model(a)
    assert town.ramp(m, starter.Place(a), "z", {"kerb": "S"}, fill="F")
    heights = [max(y for (x, y, z) in m.v if x == 3 and z == row) for row in range(12, 0, -1)]
    assert heights == sorted(heights) and heights[-1] == 3
    assert m.get(3, 1, 10) == starter.stairs("minecraft:stone_brick", "north")
    assert not town.ramp(model(allocation(7, 8, 14)), starter.Place(allocation(7, 8, 14)), "z")


def test_a_rail_runs_the_edge_but_its_ends_and_takes_lamps_and_specials_first():
    a = allocation(7, 8, 22)
    m = model(a)
    town.rail(m, starter.Place(a), "z", "high", 0, "R", lamp_at=lambda i, n: i == 5,
              special=lambda m, i, n, x, z: i == 7 and (m.set(x, 1, z, "S") or True))
    col = {z: m.get(5, 1, z) for z in range(1, 21)}
    assert col[1] is None and col[20] is None
    assert col[6].startswith("minecraft:oak_fence") and col[8] == "S"
    assert all(col[z] == "R" for z in range(2, 20) if z not in (6, 8))
    gapped = model(a)
    town.rail(gapped, starter.Place(a), "z", "high", 9, "R", gap=0.5)
    assert 0 < sum(1 for z in range(2, 20) if gapped.get(5, 1, z) == "R") < 18


def test_a_squares_paths_run_from_every_way_in_to_its_centre():
    a = allocation(15, 6, 15, [seam("w", "west", [[0, 1, 6], [0, 3, 8]], "node/w")])
    m = model(a)
    assert town.square(m, starter.Place(a), {"verge": "V", "paving": "P"}, path="K") == (7, 7)
    assert all(m.get(x, 0, 7) == "K" for x in range(2, 8)) and m.get(8, 0, 7) == "P"
    assert m.get(1, 0, 1) == "V" and m.get(3, 0, 3) == "P"


# ---------------------------------------------------------------- a lot's edges


def test_cells_past_the_play_space_are_clipped_above_the_course_given():
    m = model(allocation(9, 8, 9))
    m.box(0, 8, 0, 5, 0, 8, "W")
    town.clip_beyond(m, 1, 7, 1, 7, above=2)
    assert m.get(0, 2, 0) == "W" and m.get(0, 3, 0) is None and m.get(4, 5, 4) == "W"


def test_an_eave_over_an_unwalled_edge_column_is_taken_off():
    m = model(allocation(9, 8, 9))
    m.box(1, 7, 4, 4, 1, 7, "roof")
    m.box(1, 1, 1, 3, 1, 7, "wall")
    town.clip_unwalled_edge(m, 1, 7, 1, 7, {"wall"}, 4)
    assert m.get(1, 4, 3) == "roof" and m.get(7, 4, 3) is None and m.get(4, 4, 4) == "roof"


def test_an_edge_column_never_stands_over_air():
    m = model(allocation(9, 8, 9))
    m.set(1, 1, 1, "wall")
    m.set(1, 5, 1, "roof")
    town.close_edge_gaps(m, 1, 7, 1, 7, lambda x, z: "fill")
    assert [m.get(1, y, 1) for y in range(1, 6)] == ["wall", "fill", "fill", "fill", "roof"]


def lot_alloc(span_rows=(5, 7), air=4):
    return allocation(20, 12, 12, [seam("street-to-lot", "east", [[19, 1, span_rows[0]], [19, 3, span_rows[1]]], "node/street")],
                      voids=[{"cells": [[20, 0, 0], [20, air, 11]]}])


def test_a_street_seam_says_whether_the_lot_fronts_and_how_high_the_streets_air_is():
    p = starter.Place(lot_alloc())
    fronted, (edge, rows, top, air) = town.street_seam(p, "east")
    assert fronted and edge == 19 and rows == {5, 6, 7} and top == 3 and air == 4
    p2 = starter.Place(lot_alloc(span_rows=(1, 10), air=8))
    fronted, (_, rows, _, air) = town.street_seam(p2, "east")
    assert not fronted and air == 8
    assert town.street_seam(p, "north") == (False, None)


def test_a_street_front_closes_the_edge_but_its_doorway():
    roles = {"wall": "W", "lintel": "L", "coping": "C"}
    m = model(lot_alloc())
    m.box(18, 18, 1, 6, 2, 4, "house")       # a building standing on the edge's rows 2..4
    m.box(17, 18, 7, 7, 1, 10, "eave")       # an eave crossing the yard
    m.part(15, 18, 1, 10, "yard")
    town.street_front(m, 18, 1, 10, {5, 6, 7}, 4, roles, door_h=3, inward=-1)
    assert [m.get(18, y, 6) for y in (1, 2, 3)] == [AIR] * 3 and m.get(18, 4, 6) == "L"
    assert [m.get(18, y, 9) for y in (1, 2, 3, 4)] == ["W", "W", "W", "C"] and m.get(18, 7, 9) is None
    assert m.get(18, 3, 3) == "house"
    assert (18, 9) not in m.region and (17, 6) not in m.region and (17, 9) in m.region


def test_a_street_front_on_a_north_or_south_face_is_the_same_closure_turned():
    roles = {"wall": "W", "lintel": "L", "coping": "C"}
    mx = model(allocation(20, 12, 12))
    mx.box(18, 18, 1, 6, 2, 4, "house")
    town.street_front(mx, 18, 1, 10, {5, 6, 7}, 4, roles)
    mz = model(allocation(12, 12, 20))
    mz.box(2, 4, 1, 6, 18, 18, "house")
    town.street_front(mz, 18, 1, 10, {5, 6, 7}, 4, roles, axis="z")
    assert mz.v == {(z, y, x): r for (x, y, z), r in mx.v.items()}


# ---------------------------------------------------------------- small buildings


def test_a_ring_a_window_and_a_flower_bed():
    assert sorted(town.ring(0, 2, 0, 2)) == [(0, 0), (0, 1), (0, 2), (1, 0), (1, 2), (2, 0), (2, 1), (2, 2)]
    m = model(allocation(9, 8, 9))
    town.window(m, 2, 3, "G")
    assert [m.get(2, y, 3) for y in (1, 2, 3, 4)] == [None, "G", "G", None]
    town.flower_bed(m, [(4, 4), (5, 4)], ["a", "b"], soil="S")
    assert (m.get(4, 0, 4), m.get(4, 1, 4), m.get(5, 1, 4)) == ("S", "a", "b")


def test_a_post_shed_and_a_loft_stand_from_the_walk_plane():
    m = model(allocation(12, 8, 12))
    town.post_shed(m, (2, 8, 2, 6), 4, [5], {"post": "P", "beam": "B", "back": "K"}, back_height=2)
    assert [m.get(5, y, 6) for y in (1, 2, 3, 4)] == ["P", "P", "P", "B"]
    assert m.get(5, 1, 4) is None and m.get(4, 2, 2) == "K" and m.get(4, 3, 2) is None and m.get(4, 2, 6) is None
    town.loft(m, 2, 4, 7, 9, 4, (5, 8), {"floor": "F", "ladder": "L"})
    assert m.get(3, 4, 8) == "F" and [m.get(5, y, 8) for y in (1, 2, 3, 4)] == ["L"] * 4


# ---------------------------------------------------------------- boats and quays


def test_a_hull_along_z_is_the_transpose_of_the_same_hull_along_x():
    mx = model(allocation(30, 12, 30))
    mz = model(allocation(30, 12, 30))
    cols: set = set()
    town.hull(mx, "x", 5, 15, [1, 2, 2, 2, 1, 0], 4, cols=cols)
    town.hull(mz, "z", 5, 15, [1, 2, 2, 2, 1, 0], 4)
    flare = {"flare_s": "flare_e", "flare_n": "flare_w"}
    assert mz.v == {(z, y, x): flare.get(r, r) for (x, y, z), r in mx.v.items()}
    assert mx.get(10, 7, 15) == "stem" and mx.get(7, 3, 15) == "keel"
    assert len(cols) == sum(2 * w + 1 for w in [1, 2, 2, 2, 1, 0])


def test_a_mast_carries_a_yard_across_the_hull_and_a_furled_sail_on_it():
    m = model(allocation(30, 14, 30))
    town.mast(m, "x", 10, 15, 5, 11, {"mast": "M", "yard_x": "YX", "yard_z": "YZ", "sail": "S"})
    assert [m.get(10, y, 15) for y in range(5, 12)] == ["M"] * 7
    assert m.get(10, 10, 13) == "YZ" and m.get(10, 11, 17) == "S" and m.get(12, 10, 15) is None


def test_a_hull_on_its_stocks_has_its_keel_stem_and_transom():
    m = model(allocation(30, 14, 30))
    town.hull_on_stocks(m, "x", 5, 15, 10, 4, 2, [7, 12])
    assert m.get(7, 3, 10) == "stock" and m.get(10, 4, 10) == "keel" and m.get(15, 8, 10) == "stem_tip"
    assert m.get(14, 5, 9) == "bow_plate_n" and m.get(4, 6, 10) == "transom_rake"


def test_a_slipway_drops_half_a_course_a_row_under_water():
    m = model(allocation(12, 12, 20))
    roles = {"low": "lo", "wet_lo": "wl", "wet_hi": "wh", "fill": "F", "water": "~"}
    town.slipway(m, [3], 5, 9, 6, 2, 7, roles)
    assert m.get(3, 7, 4) == "lo" and m.get(3, 7, 5) == AIR
    assert [m.get(3, 6 - (k + 1) // 2, 5 + k) for k in range(5)] == ["wl", "wh", "wl", "wh", "wl"]
    assert m.get(3, 6, 7) == "~" and m.get(3, 2, 9) == "F"


def test_a_pier_and_a_water_step():
    m = model(allocation(12, 12, 20))
    town.pier(m, [3, 4], 5, 8, 6, 2, {"fill": "F", "pave": "P", "kerb": "K"}, kerb_x=4)
    assert m.get(3, 6, 5) == "P" and m.get(3, 6, 8) == "K" and m.get(4, 6, 5) == "K" and m.get(3, 2, 6) == "F"
    town.water_step(m, 6, 6, 5, 2, {"step": "S", "fill": "F"})
    assert [m.get(6, y, 6) for y in (2, 3, 4, 5)] == ["F", "F", "F", "S"]


# ---------------------------------------------------------------- the plan's side


def test_a_front_contact_moves_the_seam_to_the_doorway_and_cuts_it_to_the_span():
    assert town.front_contact(1, 0, 6, 3, 3) == {"at": 6, "meets": 5, "contact": {"extent": [3, 3]}}


def test_a_sign_station_and_its_entry_name_the_same_anchor():
    station, entry = town.sign_station("node/street-1", "node/net-loft", "Net Loft")
    assert station["anchor"] == entry["anchor"] == "anchor/sign-net-loft" and station["kind"] == "point"
    assert entry["front"] == entry["back"] == ["", "Net Loft"] and entry["id"] == "sign/net-loft"


def test_the_seen_reach_is_the_served_chunks_and_the_players_own():
    assert town.seen_reach() == 176 and town.seen_reach(12) == 208
    assert town.seen_region((10, 100, 20, 300)) == (-166, 276, -156, 476)


# ---------------------------------------------------------------- the starter's own


def test_the_starter_reexports_the_town_helpers():
    assert starter.stretch is town.stretch and starter.street_front is town.street_front
    with pytest.raises(AttributeError):
        starter.no_such_helper  # noqa: B018


def test_a_shifted_drawing_lands_shifted_and_run_reads_it_as_it_stands():
    m = model(allocation(12, 8, 12))
    m.shift(3, 1)
    m.set(2, 1, 2, "W")
    m.part(2, 2, 2, 2, "p")
    m.mark("m", 2, 2)
    assert m.v == {(5, 1, 3): "W"} and m.get(2, 1, 2) == "W"
    assert m.region == {(5, 3): "p"} and m.marks["m"] == (5, 3, "south", None)
    m.shift(0)
    assert m.get(5, 1, 3) == "W"


def guard_leaves(p) -> list[dict]:
    g = starter.order_guard(p)
    return [] if g is None else (g["of"] if g["cond"] == "all" else [g])


def test_the_order_guard_holds_each_handed_cut_between_its_emitted_neighbours():
    p = starter.Place(lot_alloc())
    starter.SPLITS.clear()
    y1 = p.seams[0]["hi"][1]          # handed: seam y1 + 1
    starter.SPLITS.append(("y", [starter.lit(0), starter.lit(2), y1, starter.lit(6)]))
    leaves = guard_leaves(p)
    assert {"cond": "cmp", "lhs": y1.expr, "op": "gt", "rhs": {"expr": "int", "value": 2}} in leaves
    assert {"cond": "cmp", "lhs": {"expr": "int", "value": 6}, "op": "gt", "rhs": y1.expr} in leaves
    # two literals side by side owe nothing
    assert not [l for l in leaves if l["lhs"] == {"expr": "int", "value": 2}]


def test_the_order_guard_keeps_coincident_handed_bounds_level():
    p = starter.Place(lot_alloc())
    starter.SPLITS.clear()
    leaves = guard_leaves(p)
    # the seam's floor y0 and the datum coincide at 1: the program read them as one cut
    y0 = {"expr": "param", "name": "handed/seam/street-to-lot/y0"}
    assert any(l["op"] == "eq" and y0 in (l["lhs"], l["rhs"]) for l in leaves)


def test_every_program_the_starter_emits_carries_its_order_guard():
    a = lot_alloc()
    doc = starter.program(a)
    when = doc["rules"]["place"][0]["when"]
    assert when["cond"] == "all" and when["of"]
    names = {starter.json.dumps(l, sort_keys=True) for l in when["of"]}
    assert any("handed/seam/street-to-lot/z1" in n for n in names)


def digest(m) -> str:
    import hashlib
    import json

    return hashlib.sha256(json.dumps(sorted((list(c), r) for c, r in m.v.items())).encode()).hexdigest()[:16]


def test_a_seed_draws_the_way_it_drew():
    """The cells each way draws for a fixed allocation and seed, by digest: a
    campaign's committed programs regenerate byte-stable only while these
    hold, so a change to what a helper draws reds here first."""
    a = street_alloc()
    got = {}
    for name, draw in (
        ("stretch", lambda m, p: town.stretch(m, p, "z", 3, clear=5, fronts={"node/bakery": "minecraft:spruce_hanging_sign"})),
        ("market", lambda m, p: town.stretch(m, p, "z", 4, clear=5, verge=town.market)),
        ("lane", lambda m, p: town.lane(m, p, "z")),
        ("rail", lambda m, p: town.rail(m, p, "z", "high", 7, ["A", "B"], lamp_at=lambda i, n: i % 9 == 4, gap=0.28)),
    ):
        m = model(a)
        draw(m, starter.Place(a))
        got[name] = digest(m)
    f = flight_alloc("south")
    m = model(f)
    town.flight(m, starter.Place(f), lamp_side="high", flower="minecraft:poppy")
    got["flight"] = digest(m)
    assert got == EXPECTED_DIGESTS


def test_a_verge_bench_turns_its_back_to_the_lot():
    a = allocation(11, 6, 40)
    for seed in range(8):
        m = model(a)
        f = town.stretch(m, starter.Place(a), "z", seed, clear=5, verge=lambda m, f: town.verge_life(m, f, things=("bench",)))
        benches = {c: r for c, r in m.v.items() if "_stairs[" in r}
        assert benches
        for (x, _, _), r in benches.items():
            assert ("facing=west" in r) == (x < f["c0"]) and ("facing=east" in r) == (x > f["c1"])


# measured with the helpers that regenerated the town's 72 street programs
# equal; `stretch` re-measured when the default verge stopped standing a
# barrel (no default is a container the census counts)
EXPECTED_DIGESTS = {"stretch": "0738cf0fee822b4a", "market": "1a568c6a8f4968d1", "lane": "150a4d074863c337",
                    "rail": "4f53cda18c2baa0c", "flight": "e9fed371a5aa6448"}


def test_a_cart_rests_on_its_own_ends_and_no_top_slab_floats_in_a_verge():
    """A verge cart is two upside-down stairs back to back, each standing on
    its own end, its load on the first: a bed that rests on the ground, never
    a top slab with half a course of air under it (`DW1076`)."""
    a = allocation(11, 6, 40)
    seen = 0
    for seed in range(8):
        m = model(a)
        town.stretch(m, starter.Place(a), "z", seed, clear=5, verge=lambda m, f: town.verge_life(m, f, things=("cart",)))
        assert not [r for r in m.v.values() if "_slab[type=top" in r], seed
        loads = [c for c, r in m.v.items() if r.startswith("minecraft:hay_block")]
        for (x, y, z) in loads:
            under = m.get(x, y - 1, z)
            assert under is not None and "_stairs[" in under and "half=top" in under, (seed, under)
            seen += 1
    assert seen, "the binding: a cart drawn"


def test_a_drawn_way_up_says_it_climbs_where_the_starter_would_stand_a_ladder():
    """`town.flight` and `town.ramp` draw the way up to an opening two or more
    courses over the walk plane, so they tell the starter (`Model.climbs_to`)
    and no ladder stands in their steps; under an opening at the walk plane
    there is no ladder to replace and nothing is said."""
    raised = allocation(7, 8, 10, [seam("up", "north", [[1, 3, 0], [5, 6, 0]], "node/up", rise=2),
                                   seam("down", "south", [[1, 1, 9], [5, 4, 9]], "node/down")])
    for draw in (lambda m, p: town.flight(m, p), lambda m, p: town.ramp(m, p, "z")):
        m = model(raised)
        draw(m, m.place)
        assert m.climbed == {"up"}
    m = model(flight_alloc("north"))
    town.flight(m, m.place)
    assert m.climbed == set()
