"""`check-written-world.py` classifies a cane or a cactus the server aged as
`random-tick` (`DW0955`): the same block, differing only in `age`. Measured on
the pinned server: a written `sugar_cane[age=0]` read `age=1` in the save."""

from __future__ import annotations

import importlib.util
from pathlib import Path

import pytest

SCRIPT = Path(__file__).resolve().parents[1] / "ci" / "check-written-world.py"


@pytest.fixture(scope="module")
def ww():
    spec = importlib.util.spec_from_file_location("check_written_world", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def cls(ww, a, b):
    return ww.classify(a, b, (0, 0, 0), [])


def test_observed_cane_is_random_tick(ww):
    assert cls(ww, "minecraft:sugar_cane[age=0]", "minecraft:sugar_cane[age=1]") == "random-tick"
    assert cls(ww, "minecraft:cactus[age=3]", "minecraft:cactus[age=9]") == "random-tick"


def test_another_block_or_property_stays_model(ww):
    assert cls(ww, "minecraft:wheat[age=0]", "minecraft:wheat[age=1]") == "model"
    assert cls(ww, "minecraft:sugar_cane[age=0]", "minecraft:cactus[age=0]") == "model"
    assert cls(ww, "minecraft:sugar_cane[age=0]", "minecraft:sugar_cane") == "model"


def test_a_path_under_hay_is_the_models_to_fix(ww):
    # The server's dirt under a solid block is the model's defect, never a class.
    assert cls(ww, "minecraft:dirt_path", "minecraft:dirt") == "model"
