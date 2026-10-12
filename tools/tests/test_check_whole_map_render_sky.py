"""The whole-map gate's clear-sky rule: a clear scene carries the clear cell of
its tick, derived in the gate from the vendored day timeline.

The two blocks below are what `delvec cameras` emitted for the Treehouse Camp's
`from-the-glade` camera (`{"sun": "setting"}` + clear, tick 12782) and its
`lantern-night` camera (`{"moon": "high"}` + clear, tick 18000), copied from the
scene files — the engine's bytes, not the gate's derivation.
"""

from __future__ import annotations

import copy
import importlib.util
from pathlib import Path

CHECKER = Path(__file__).resolve().parents[1] / "ci" / "check-whole-map-render.py"


def gate():
    spec = importlib.util.spec_from_file_location("cwmr", CHECKER)
    mod = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(mod)
    return mod


SETTING = {
    "sun": {
        "altitude": -3.7e-05,
        "azimuth": 3.141593,
        "intensity": 0.0,
        "color": {"red": 1.0, "green": 1.0, "blue": 1.0},
        "drawTexture": True,
    },
    "sky": {
        "mode": "SOLID_COLOR",
        "color": {"red": 0.357581, "green": 0.357581, "blue": 1.0},
        "skyLight": 0.374446,
        "apparentSkyLight": 0.197251,
    },
    "fog": {
        "mode": "UNIFORM",
        "uniformDensity": 0.0001,
        "color": {"red": 0.12753, "green": 0.163641, "blue": 0.25084},
        "skyFogDensity": 1.0,
    },
}
NIGHT = {
    "sun": {
        "altitude": -1.570796,
        "azimuth": 0.0,
        "intensity": 0.0,
        "color": {"red": 1.0, "green": 1.0, "blue": 1.0},
        "drawTexture": False,
    },
    "sky": {
        "mode": "SOLID_COLOR",
        "color": {"red": 0.286661, "green": 0.286661, "blue": 1.0},
        "skyLight": 0.209457,
        "apparentSkyLight": 0.0,
    },
    "fog": {
        "mode": "UNIFORM",
        "uniformDensity": 0.0001,
        "color": {"red": 0.000992, "green": 0.001202, "blue": 0.00456},
        "skyFogDensity": 1.0,
    },
}
DESIGN = {"content": {"references": [{"name": "concept/x", "time": "noon", "weather": "clear"}]}}
CAM = {"answers": "concept/x"}


def judge(doc: dict) -> list[str]:
    return gate().sky_matches(doc, CAM, DESIGN, {}, {}, {})


def test_the_tick_is_read_off_the_sun_by_the_closed_form():
    g = gate()
    assert abs(g.closed_form_altitude(18000) + 90.0) < 1e-9
    assert abs(g.closed_form_altitude(6000) - 90.0) < 1e-9
    assert g.tick_of_sun(-1.570796, 0.0) == 18000
    assert g.tick_of_sun(1.570796, 0.0) == 6000
    # The setting sun's tick by the closed form, within the slack of the track's.
    assert abs(g.tick_of_sun(-3.7e-05, 3.141593) - 12782) <= g.TICK_SLACK


def test_the_engines_clear_cells_pass():
    assert judge(copy.deepcopy(SETTING)) == []
    assert judge(copy.deepcopy(NIGHT)) == []


def test_a_clear_scene_left_to_the_renderer_is_refused():
    bare = {"sun": {"altitude": -3.7e-05, "azimuth": 3.141593}}
    found = judge(bare)
    assert any("`sky.mode`" in f for f in found), found
    assert any("`sun.intensity`" in f for f in found), found


def test_each_value_of_the_cell_is_judged():
    for path, value, needle in [
        (("sky", "skyLight"), 0.5, "`sky.skyLight`"),
        (("sky", "apparentSkyLight"), 1.0, "`sky.apparentSkyLight`"),
        (("fog", "skyFogDensity"), 0.0, "`fog.skyFogDensity`"),
        (("sun", "intensity"), 1.25, "`sun.intensity`"),
        (("sun", "drawTexture"), False, "`sun.drawTexture`"),
    ]:
        doc = copy.deepcopy(SETTING)
        doc[path[0]][path[1]] = value
        assert any(needle in f for f in judge(doc)), (path, judge(doc))
    tint = copy.deepcopy(SETTING)
    tint["sky"]["color"]["red"] = 0.478431
    assert any("`sky.color.red`" in f for f in judge(tint))
    fog = copy.deepcopy(NIGHT)
    fog["fog"]["color"]["blue"] = 0.086275
    assert any("`fog.color.blue`" in f for f in judge(fog))


def test_a_cell_keyed_on_the_wrong_tick_is_refused():
    # The night block under the setting sun's hour: every number is a real
    # cell's, at the wrong tick.
    doc = copy.deepcopy(NIGHT)
    doc["sun"] = copy.deepcopy(SETTING["sun"])
    found = judge(doc)
    assert any("`sky.skyLight`" in f for f in found), found


def test_clear_scenes_are_tallied_by_where_their_tick_falls():
    tally: dict[str, int] = {}
    g = gate()
    g.sky_matches(copy.deepcopy(SETTING), CAM, DESIGN, {}, {}, tally)
    g.sky_matches(copy.deepcopy(NIGHT), CAM, DESIGN, {}, {}, tally)
    assert tally == {"ramp": 1, "night plateau": 1}


def test_an_overcast_scene_carries_the_overcast_block():
    design = {"content": {"references": [{"name": "concept/x", "time": "dusk", "weather": "rain"}]}}
    doc = copy.deepcopy(SETTING)
    assert gate().sky_matches(doc, CAM, design, {}, {}, {}) == []
    doc.pop("sky")
    assert any("overcast" in f for f in gate().sky_matches(doc, CAM, design, {}, {}, {}))
