"""The render gate's build-camera agreement (`tools/ci/check-gallery-render.py`).

A frame judged by the gate is the frame the build planned: its manifest's
camera is held to the one `render-plan.json` states. These cases perturb one
field at a time past the 3-decimal rounding both sides are written at, and the
agreement must turn false for each; an unrounded re-derivation of the stated
angles is the shape it must not accept as a second authority.
"""

import importlib.util
import pathlib

SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "ci" / "check-gallery-render.py"


def _gate():
    spec = importlib.util.spec_from_file_location("check_gallery_render", SCRIPT)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


PLANNED = {"pos": [15.5, 68.962, 1.135], "look_at": [15.5, 68.0, 4.5], "yaw": -0.0, "pitch": 15.945, "fov": 70.0}
DRAWN = {"pos": [15.5, 68.962, 1.135], "yaw": 0.0, "pitch": 15.945, "fov": 70.0}


def test_the_stated_camera_agrees():
    assert _gate().same_camera(PLANNED, DRAWN)


def test_yaw_wraps_at_the_half_turn():
    g = _gate()
    assert g.same_camera(dict(PLANNED, yaw=-180.0), dict(DRAWN, yaw=180.0))


def test_each_moved_field_disagrees():
    g = _gate()
    for key, value in [
        ("pos", [15.5, 68.962, 1.14]),
        ("yaw", 0.01),
        ("pitch", 15.95),
        ("fov", 60.0),
    ]:
        assert not g.same_camera(PLANNED, dict(DRAWN, **{key: value})), key


def test_a_manifest_with_no_camera_disagrees():
    assert not _gate().same_camera(PLANNED, {})
