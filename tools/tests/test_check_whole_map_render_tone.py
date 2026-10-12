"""The whole-map gate's tone rule: every scene's Hable curve, evaluated as the
pinned core evaluates it, shows linear 0.18 at the game's transfer of it.

The engine's curve is written here as the scene carries it; the core's bare
preset, a missing key and another post-process are the perturbations only this
rule catches.
"""

from __future__ import annotations

import functools
import importlib.util
from pathlib import Path

CHECKER = Path(__file__).resolve().parents[1] / "ci" / "check-whole-map-render.py"


@functools.cache
def gate():
    spec = importlib.util.spec_from_file_location("cwmr_tone", CHECKER)
    mod = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(mod)
    return mod


ANCHORED = {
    "shoulderStrength": 0.32138956,
    "linearStrength": 0.7318806,
    "linearAngle": 0.1,
    "toeStrength": 0.2,
    "toeNumerator": 0.02,
    "toeDenominator": 0.3,
    "linearWhitePointValue": 7.651521,
}
PRESET = {
    "shoulderStrength": 0.15,
    "linearStrength": 0.5,
    "linearAngle": 0.1,
    "toeStrength": 0.2,
    "toeNumerator": 0.02,
    "toeDenominator": 0.3,
    "linearWhitePointValue": 11.2,
}


def scene(settings, post="TONEMAP3"):
    doc = {"postprocess": post}
    if settings is not None:
        doc["postprocessSettings"] = dict(settings)
    return doc


def test_the_anchored_curve_passes():
    assert gate().tone_findings(scene(ANCHORED)) == []
    shown = gate().hable_shown(ANCHORED, 0.18)
    assert abs(shown - 0.18**gate().CORE_EXPONENT) < 1e-6


def test_the_core_preset_is_refused_with_its_mid_grey():
    bad = gate().tone_findings(scene(PRESET))
    assert len(bad) == 1 and "0.393285" in bad[0] and "0.458656" in bad[0], bad


def test_missing_settings_and_a_missing_key_are_refused():
    assert "no complete postprocessSettings" in gate().tone_findings(scene(None))[0]
    partial = dict(ANCHORED)
    del partial["toeNumerator"]
    assert "no complete postprocessSettings" in gate().tone_findings(scene(partial))[0]


def test_another_post_process_is_refused():
    assert "not the Hable curve" in gate().tone_findings(scene(ANCHORED, "GAMMA"))[0]


def test_a_curve_off_by_more_than_the_tolerance_is_refused():
    brighter = dict(ANCHORED, linearStrength=ANCHORED["linearStrength"] * 1.01)
    assert gate().tone_findings(scene(brighter)), "a one-percent linear-section change"
