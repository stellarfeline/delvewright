"""`check-written-world.py`'s placed-object comparison (spec-0112 §8.2,
`DW1040`): every block entity of the written load world by cell and compound,
and every entity by kind, cell and tag set, against the server save; a
difference in no named class reds `DW1040` naming the object, and the classes
are counted.

Each case builds two worlds through `tools/lib/anvil.py`'s own writer — the
written one and a copy standing for the server's save — and perturbs the copy
by one thing."""

from __future__ import annotations

import copy
import importlib.util
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "tools" / "ci" / "check-written-world.py"
sys.path.insert(0, str(ROOT / "tools" / "lib"))
import anvil  # noqa: E402
from anvil import T_BYTE, T_COMPOUND, T_DOUBLE, T_FLOAT, T_INT, T_INT_ARRAY, T_LIST, T_STRING, Tag  # noqa: E402


@pytest.fixture(scope="module")
def ww():
    spec = importlib.util.spec_from_file_location("check_written_world", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def keeper(tags=("dw_npc", "dw_npc_keeper"), uuid=(1, 2, 3, 4)) -> Tag:
    return Tag(
        T_COMPOUND,
        {
            "id": Tag(T_STRING, "minecraft:villager"),
            "Pos": Tag(T_LIST, [Tag(T_DOUBLE, p) for p in (5.5, 65.0, 4.5)], T_DOUBLE),
            "Rotation": Tag(T_LIST, [Tag(T_FLOAT, 180.0), Tag(T_FLOAT, 0.0)], T_FLOAT),
            "Tags": Tag(T_LIST, [Tag(T_STRING, t) for t in tags], T_STRING),
            "UUID": Tag(T_INT_ARRAY, list(uuid)),
            "NoAI": Tag(T_BYTE, 1),
        },
    )


def chest(items=1) -> Tag:
    return Tag(
        T_COMPOUND,
        {
            "id": Tag(T_STRING, "minecraft:chest"),
            "x": Tag(T_INT, 1),
            "y": Tag(T_INT, 65),
            "z": Tag(T_INT, 2),
            "Items": Tag(
                T_LIST,
                [
                    Tag(
                        T_COMPOUND,
                        {"Slot": Tag(T_BYTE, 0), "id": Tag(T_STRING, "minecraft:emerald"), "count": Tag(T_INT, items)},
                    )
                ],
                T_COMPOUND,
            ),
        },
    )


def world(tmp: Path, name: str, entities: list[Tag], block_entities: list[Tag]) -> Path:
    w = tmp / name
    (w / "entities").mkdir(parents=True)
    (w / "region").mkdir(parents=True)
    (w / "level.dat").write_bytes(b"")
    anvil.region_write(
        w / "entities" / "r.0.0.mca",
        {(0, 0): Tag(T_COMPOUND, {"Entities": Tag(T_LIST, entities, T_COMPOUND)})},
    )
    anvil.region_write(
        w / "region" / "r.0.0.mca",
        {
            (0, 0): Tag(
                T_COMPOUND,
                {
                    "sections": Tag(T_LIST, [], T_COMPOUND),
                    "block_entities": Tag(T_LIST, block_entities, T_COMPOUND),
                },
            )
        },
    )
    return w


def test_the_same_objects_pass_with_both_denominators(ww, tmp_path):
    a = world(tmp_path, "w", [keeper()], [chest()])
    b = world(tmp_path, "s", [keeper()], [chest()])
    code, lines, summary = ww.objects(a, b)
    assert code == 0, lines
    assert lines[0] == (
        "objects binding: 1 block entit(ies) written, 1 in the server save; "
        "1 entit(ies) written, 1 in the server save"
    )
    assert summary["verdict"] == "pass"


def test_a_tag_moved_on_one_body_reds_dw1040_naming_it(ww, tmp_path):
    a = world(tmp_path, "w", [keeper()], [])
    b = world(tmp_path, "s", [keeper(tags=("dw_npc", "dw_npc_warden"))], [])
    code, lines, summary = ww.objects(a, b)
    assert code == 1
    assert lines[-1].startswith("DW1040 the objects the engine writes are not the objects")
    assert "entity minecraft:villager at [5, 65, 4] tags ['dw_npc', 'dw_npc_keeper']: written, not in the server save" in lines[-1]
    assert summary["differing"] == 2  # the written one missing, the server's one unwritten


def test_a_uuid_the_server_assigned_lands_in_bookkeeping(ww, tmp_path):
    a = world(tmp_path, "w", [keeper(uuid=(1, 2, 3, 4))], [])
    b = world(tmp_path, "s", [keeper(uuid=(9, 9, 9, 9))], [])
    code, lines, summary = ww.objects(a, b)
    assert code == 0, lines
    assert summary["classes"]["bookkeeping"] == 1


def test_a_key_the_server_adds_is_bookkeeping_and_a_key_it_drops_is_not(ww, tmp_path):
    more = keeper()
    more.value["Motion"] = Tag(T_LIST, [Tag(T_DOUBLE, 0.0)] * 3, T_DOUBLE)
    more.value["CanPickUpLoot"] = Tag(T_BYTE, 0)
    a = world(tmp_path, "w", [keeper()], [])
    b = world(tmp_path, "s", [more], [])
    code, lines, summary = ww.objects(a, b)
    assert code == 0, lines
    assert summary["classes"]["bookkeeping"] == 1
    fewer = copy.deepcopy(keeper())
    del fewer.value["NoAI"]
    b2 = world(tmp_path, "s2", [fewer], [])
    code, lines, _ = ww.objects(a, b2)
    assert code == 1
    assert "entity minecraft:villager at [5, 65, 4]: NoAI differ" in lines[-1]


def test_a_chest_whose_items_differ_reds_and_a_rolled_loot_table_does_not(ww, tmp_path):
    a = world(tmp_path, "w", [], [chest(items=1)])
    b = world(tmp_path, "s", [], [chest(items=3)])
    code, lines, _ = ww.objects(a, b)
    assert code == 1
    assert "block entity minecraft:chest at [1, 65, 2]: Items differ" in lines[-1]
    rolled = chest(items=3)
    rolled.value["LootTable"] = Tag(T_STRING, "minecraft:chests/simple_dungeon")
    written = chest(items=1)
    written.value["LootTable"] = Tag(T_STRING, "minecraft:chests/simple_dungeon")
    code, lines, summary = ww.objects(world(tmp_path, "w2", [], [written]), world(tmp_path, "s3", [], [rolled]))
    assert code == 0, lines
    assert summary["classes"]["loot"] == 1


def test_a_wave_body_on_the_server_side_alone_is_played(ww, tmp_path):
    a = world(tmp_path, "w", [keeper()], [])
    wave = keeper(tags=("dw_wave_muster",))
    b = world(tmp_path, "s", [keeper(), wave], [])
    code, lines, summary = ww.objects(a, b)
    assert code == 0, lines
    assert summary["classes"]["played"] == 1
