"""`check-whole-map-render.py`'s own reading of a frame manifest's camera:
its projection of the layout box agrees with a hand-placed camera, reds a
camera aimed away, and its record comparison names a moved field."""

from __future__ import annotations

import importlib.util
from pathlib import Path

import pytest

CHECKER = Path(__file__).resolve().parents[1] / "ci" / "check-whole-map-render.py"


@pytest.fixture(scope="module")
def wm():
    spec = importlib.util.spec_from_file_location("check_whole_map_render", CHECKER)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def manifest(pos, yaw, pitch, fov=40.0, w=160, h=90):
    return {"camera": {"pos": pos, "yaw": yaw, "pitch": pitch, "fov": fov}, "image": {"width": w, "height": h}}


def test_a_camera_south_of_a_box_looking_north_frames_it(wm):
    # A 10-cube centred on (5, 69, 5), seen from 30 blocks south, looking
    # north (yaw 180) and level.
    inside, w, h = wm.projection(manifest([5.0, 69.0, 35.0], 180.0, 0.0), [0, 64, 0], [9, 73, 9])
    assert inside
    assert 0.2 < w < 0.6 and 0.3 < h < 0.9


def test_a_camera_looking_away_does_not(wm):
    inside, w, h = wm.projection(manifest([5.0, 69.0, 35.0], 0.0, 0.0), [0, 64, 0], [9, 73, 9])
    assert not inside
    assert w * h == 0.0


def test_yaw_ninety_looks_west(wm):
    # Minecraft's yaw 90 looks toward -X: a box west of the eye is in frame.
    inside, _, _ = wm.projection(manifest([40.0, 69.0, 5.0], 90.0, 0.0), [0, 64, 0], [9, 73, 9])
    assert inside
    inside, _, _ = wm.projection(manifest([-30.0, 69.0, 5.0], 90.0, 0.0), [0, 64, 0], [9, 73, 9])
    assert not inside


def test_a_moved_camera_field_is_named(wm):
    man = manifest([1.0, 2.0, 3.0], 10.0, 20.0, 55.0)
    cam = {"pos": [1.0, 2.0, 3.0], "yaw": 10.0, "pitch": 20.0, "fov": 55.0}
    assert wm.camera_matches(man, cam) == []
    cam["yaw"] = 12.0
    assert wm.camera_matches(man, cam) == ["`yaw` is 10.0 in the frame and 12.0 in the record"]
