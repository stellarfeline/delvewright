"""The whole-map gate's clear-sky rule: a clear scene's sky is the game's own sky
at its tick, as a map — the gate's own drawing of the client's sky plus one
uniform veil — and the light it casts is the lightmap's.

The scene block below is the shape `delvec cameras` writes for a clear camera at
the setting sun (tick 12782). The map beside it is drawn here by the gate's own
functions, so these tests judge the gate's logic; the engine's maps are judged
by the gate's run over the gallery.
"""

from __future__ import annotations

import copy
import functools
import importlib.util
import math
import struct
from pathlib import Path

CHECKER = Path(__file__).resolve().parents[1] / "ci" / "check-whole-map-render.py"


@functools.cache
def gate():
    spec = importlib.util.spec_from_file_location("cwmr", CHECKER)
    mod = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(mod)
    return mod


TICK = 12782
SCENE = {
    "name": "probe",
    "sun": {
        "altitude": -3.7e-05,
        "azimuth": 3.141593,
        "intensity": 0.0,
        "color": {"red": 1.0, "green": 1.0, "blue": 1.0},
        "drawTexture": True,
    },
    "sky": {
        "mode": "SKYMAP_EQUIRECTANGULAR",
        "color": {"red": 0.357581, "green": 0.357581, "blue": 1.0},
        "skyLight": None,
        "apparentSkyLight": 1.0,
        "skymap": "probe.sky.pfm",
        "skyMirrored": False,
    },
    "fog": {
        "mode": "UNIFORM",
        "uniformDensity": 0.0,
        "color": {"red": 0.12753, "green": 0.163641, "blue": 0.25084},
        "skyFogDensity": 0.0,
    },
    "delvewrightSky": {"daytimeTicks": TICK},
    "camera": {"orientation": {"roll": 0.0, "pitch": -1.500983, "yaw": 0.0}},
}
DESIGN = {"content": {"references": [{"name": "concept/x", "time": "noon", "weather": "clear"}]}}
CAM = {"answers": "concept/x"}


@functools.cache
def drawn() -> tuple[tuple[float, ...], float]:
    """The game's sky at TICK for SCENE's camera plus the least veil, and the
    multiplier that makes its light the lightmap's — the engine's rule, built
    from the gate's own drawing."""
    g = gate()
    o = SCENE["camera"]["orientation"]
    view = (
        math.cos(o["yaw"]) * math.sin(o["pitch"]),
        -math.cos(o["pitch"]),
        -math.sin(o["yaw"]) * math.sin(o["pitch"]),
    )
    colours = g.game_sky_colours(TICK, view)
    px: list[float] = []
    acc, weight = [0.0, 0.0, 0.0], 0.0
    for j in range(g.MAP_H):
        for i in range(g.MAP_W):
            d = g.map_direction(i, j)
            c = g.game_sky_pixel(colours, d)
            px.extend(c)
            if d[1] > 0:
                w = d[1] * math.sqrt(1 - d[1] * d[1])
                acc = [a + w * v for a, v in zip(acc, c)]
                weight += w
    mean = [a / weight for a in acc]
    light = g.lightmap_light(TICK)
    s = min([50.0] + [light[c] / mean[c] for c in range(3) if mean[c] > 0])
    veil = [max(light[c] / s - mean[c], 0.0) for c in range(3)]
    return tuple(v + veil[k % 3] for k, v in enumerate(px)), s


def write(tmp: Path, pixels) -> None:
    head = f"PF\n{gate().MAP_W} {gate().MAP_H}\n-1.0\n".encode()
    (tmp / "probe.sky.pfm").write_bytes(head + struct.pack(f"<{len(pixels)}f", *pixels))


def scene(s: float) -> dict:
    doc = copy.deepcopy(SCENE)
    doc["sky"]["skyLight"] = s
    return doc


def judge(doc: dict, tmp: Path) -> list[str]:
    return gate().sky_matches(doc, CAM, DESIGN, {}, {}, {}, tmp)


def test_the_tick_is_read_off_the_sun_by_the_closed_form():
    g = gate()
    assert g.tick_of_sun(-1.570796, 0.0) == 18000
    assert g.tick_of_sun(1.570796, 0.0) == 6000
    assert abs(g.tick_of_sun(-3.7e-05, 3.141593) - 12782) <= g.TICK_SLACK


def test_the_game_sky_plus_a_veil_with_the_lightmaps_light_passes(tmp_path):
    px, s = drawn()
    write(tmp_path, px)
    assert judge(scene(s), tmp_path) == []


def test_the_fan_is_on_the_suns_side():
    g = gate()
    colours = g.game_sky_colours(TICK, (-1.0, 0.0, 0.0))
    assert g._fan_alpha(colours["fan"], (-0.999, 0.04, 0.0)) > 0.5
    assert g._fan_alpha(colours["fan"], (0.999, 0.04, 0.0)) == 0.0


def test_a_wrong_light_is_refused(tmp_path):
    px, s = drawn()
    write(tmp_path, px)
    found = judge(scene(s * 1.1), tmp_path)
    assert any("casts" in f for f in found), found


def test_a_map_that_is_not_the_games_sky_is_refused(tmp_path):
    px, s = drawn()
    tinted = list(px)
    for k in range(0, len(tinted) // 4, 3):
        tinted[k] += 0.05
    write(tmp_path, tinted)
    found = judge(scene(s), tmp_path)
    assert any("one uniform veil" in f for f in found), found


def test_a_missing_map_or_marker_is_refused(tmp_path):
    px, s = drawn()
    assert any("not written beside it" in f for f in judge(scene(s), tmp_path))
    write(tmp_path, px)
    doc = scene(s)
    doc.pop("delvewrightSky")
    assert any("delvewrightSky" in f for f in judge(doc, tmp_path))
    doc = scene(s)
    doc["delvewrightSky"]["daytimeTicks"] = 13047
    assert any("names tick 13047" in f for f in judge(doc, tmp_path))


def test_each_fixed_key_is_judged(tmp_path):
    px, s = drawn()
    write(tmp_path, px)
    for path, value, needle in [
        (("sky", "mode"), "SOLID_COLOR", "`sky.mode`"),
        (("sky", "skyMirrored"), True, "`sky.skyMirrored`"),
        (("sky", "apparentSkyLight"), 0.5, "`sky.apparentSkyLight`"),
        (("fog", "skyFogDensity"), 1.0, "`fog.skyFogDensity`"),
        (("sun", "intensity"), 1.25, "`sun.intensity`"),
        (("sun", "drawTexture"), False, "`sun.drawTexture`"),
    ]:
        doc = scene(s)
        doc[path[0]][path[1]] = value
        assert any(needle in f for f in judge(doc, tmp_path)), (path, judge(doc, tmp_path))


def test_clear_scenes_are_tallied_and_overcast_ones_judged_apart(tmp_path):
    px, s = drawn()
    write(tmp_path, px)
    tally: dict[str, int] = {}
    gate().sky_matches(scene(s), CAM, DESIGN, {}, {}, tally, tmp_path)
    assert tally == {"ramp": 1}
    rain = {"content": {"references": [{"name": "concept/x", "time": "dusk", "weather": "rain"}]}}
    doc = scene(s)
    assert any("overcast" in f for f in gate().sky_matches(doc, CAM, rain, {}, {}))
    doc["sky"]["mode"] = "SOLID_COLOR"
    assert gate().sky_matches(doc, CAM, rain, {}, {}) == []
