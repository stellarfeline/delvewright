"""The whole-map gate's clear-sky rule (spec-0079, departure 1).

A clear scene whose sky is lit or ramping carries no `sky` or `fog` key; a
clear scene under the timeline's dark-sky plateau carries exactly the night
cell. The cell below is the block `delvec cameras` emitted for the Treehouse
Camp's `lantern-night` camera (`{"moon": "high"}` + clear), copied from the
scene file — the engine's bytes, not the gate's derivation.
"""

from __future__ import annotations

import copy
import importlib.util
import math
from pathlib import Path

CHECKER = Path(__file__).resolve().parents[1] / "ci" / "check-whole-map-render.py"


def gate():
    spec = importlib.util.spec_from_file_location("cwmr", CHECKER)
    mod = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(mod)
    return mod


NIGHT_SKY = {
    "mode": "SOLID_COLOR",
    "color": {"red": 0.478431, "green": 0.478431, "blue": 1.0},
    "skyLight": 0.24,
    "apparentSkyLight": 0.0,
}
NIGHT_FOG = {
    "mode": "UNIFORM",
    "uniformDensity": 0.0,
    "color": {"red": 0.058824, "green": 0.058824, "blue": 0.086275},
    "skyFogDensity": 0.0,
}
DESIGN = {"content": {"references": [{"name": "concept/x", "time": "noon", "weather": "clear"}]}}
CAM = {"answers": "concept/x"}


def scene(alt_deg: float, block: bool) -> dict:
    doc = {"sun": {"altitude": math.radians(alt_deg), "azimuth": 0.0}}
    if block:
        doc["sky"] = copy.deepcopy(NIGHT_SKY)
        doc["fog"] = copy.deepcopy(NIGHT_FOG)
        doc["sun"].update({"intensity": 0.0, "drawTexture": False})
    return doc


def judge(doc: dict) -> list[str]:
    return gate().sky_matches(doc, CAM, DESIGN, {}, {}, {})


def test_the_dark_edge_is_read_off_the_timeline_by_the_closed_form():
    g = gate()
    assert abs(g.closed_form_altitude(18000) + 90.0) < 1e-9
    assert abs(g.closed_form_altitude(6000) - 90.0) < 1e-9
    edge = g.dark_edge_altitude()
    assert -15.0 < edge < -14.0, edge


def test_a_sun_up_clear_scene_carries_no_block():
    assert judge(scene(30.0, False)) == []
    assert any("lit or ramping" in f for f in judge(scene(30.0, True)))


def test_a_ramping_clear_scene_keeps_the_renderers_sky():
    # A sun setting or rising sits on the horizon, in the dusk/dawn ramp.
    assert judge(scene(-0.002, False)) == []
    assert any("lit or ramping" in f for f in judge(scene(-3.5, True)))


def test_a_dark_clear_scene_carries_exactly_the_night_cell():
    assert judge(scene(-90.0, True)) == []
    missing = judge(scene(-90.0, False))
    assert missing and all("clear dark sky" in f for f in missing), missing
    altered = scene(-90.0, True)
    altered["sky"]["skyLight"] = 0.5
    assert any("`sky.skyLight` is 0.5" in f for f in judge(altered))
    lens = scene(-90.0, True)
    lens["sky"]["apparentSkyLight"] = 1.0
    assert any("apparentSkyLight" in f for f in judge(lens))
    tint = scene(-90.0, True)
    tint["sky"]["color"]["blue"] = 0.5
    assert any("`sky.color`" in f for f in judge(tint))
    sunny = scene(-90.0, True)
    sunny["sun"]["intensity"] = 1.25
    assert any("`sun.intensity`" in f for f in judge(sunny))


def test_a_dark_scene_is_tallied():
    tally: dict[str, int] = {}
    gate().sky_matches(scene(-90.0, True), CAM, DESIGN, {}, {}, tally)
    assert tally == {"night cell": 1}
