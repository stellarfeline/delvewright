"""`check-written-world.py` classifies server-weathered copper as `random-tick`
(`DW0955`), the turf rule unchanged, and nothing else along with it."""

from __future__ import annotations

import importlib.util
from pathlib import Path

import pytest

SCRIPT = Path(__file__).resolve().parents[1] / "ci" / "check-written-world.py"

ROD = "minecraft:{}lightning_rod[facing=up,powered=false,waterlogged=false]"


@pytest.fixture(scope="module")
def ww():
    spec = importlib.util.spec_from_file_location("check_written_world", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def cls(ww, a, b):
    return ww.classify(a, b, (0, 0, 0), [])


def test_observed_pair_is_random_tick(ww):
    assert cls(ww, ROD.format(""), ROD.format("exposed_")) == "random-tick"


def test_several_stages_and_other_families(ww):
    assert cls(ww, ROD.format(""), ROD.format("oxidized_")) == "random-tick"
    assert cls(ww, "minecraft:copper_block", "minecraft:weathered_copper") == "random-tick"
    assert cls(ww, "minecraft:exposed_cut_copper", "minecraft:oxidized_cut_copper") == "random-tick"
    assert (
        cls(ww, "minecraft:cut_copper_stairs[facing=north,half=bottom,shape=straight,waterlogged=false]",
            "minecraft:exposed_cut_copper_stairs[facing=north,half=bottom,shape=straight,waterlogged=false]")
        == "random-tick"
    )


def test_waxed_stays_model(ww):
    assert cls(ww, ROD.format("waxed_"), ROD.format("waxed_exposed_")) == "model"
    assert cls(ww, ROD.format("waxed_"), ROD.format("exposed_")) == "model"
    assert cls(ww, "minecraft:waxed_copper_block", "minecraft:waxed_weathered_copper") == "model"


def test_backwards_and_same_stage_stay_model(ww):
    assert cls(ww, ROD.format("exposed_"), ROD.format("")) == "model"
    assert cls(ww, ROD.format("oxidized_"), ROD.format("weathered_")) == "model"


def test_other_family_or_property_change_stays_model(ww):
    assert cls(ww, "minecraft:copper_block", "minecraft:exposed_cut_copper") == "model"
    assert cls(ww, ROD.format(""), ROD.format("exposed_").replace("powered=false", "powered=true")) == "model"
    assert cls(ww, "minecraft:stone", "minecraft:exposed_copper") == "model"


def test_ladders_come_from_the_registry(ww):
    w = ww.weathering()
    assert w["copper_block"] == (0, "copper") and w["oxidized_copper"] == (3, "copper")
    assert "waxed_copper_block" not in w and "waxed_exposed_copper" not in w
    assert "copper_ore" not in w and "copper_torch" not in w
    assert len({stem for _, stem in w.values()}) >= 10


def test_turf_rule_unchanged(ww):
    assert cls(ww, "minecraft:grass_block[snowy=false]", "minecraft:dirt") == "random-tick"
    assert cls(ww, "minecraft:dirt", "minecraft:mycelium[snowy=false]") == "random-tick"
    assert cls(ww, "minecraft:stone", "minecraft:dirt") == "model"
