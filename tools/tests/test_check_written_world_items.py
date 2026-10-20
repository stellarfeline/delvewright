"""`check-written-world.py` counts the item entities in the server save, and an
item entity no load function summons reds as `DW0955` (no block cell moves
when a falling block a later write lands on turns into an item, so the cell
comparison alone passes over it)."""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "tools" / "ci" / "check-written-world.py"
sys.path.insert(0, str(ROOT / "tools" / "lib"))
import anvil  # noqa: E402
from anvil import T_COMPOUND, T_DOUBLE, T_INT, T_LIST, T_STRING, Tag  # noqa: E402


@pytest.fixture(scope="module")
def ww():
    spec = importlib.util.spec_from_file_location("check_written_world", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def entity(kind: str, pos, item: str | None = None, count: int = 1) -> Tag:
    v = {
        "id": Tag(T_STRING, kind),
        "Pos": Tag(T_LIST, [Tag(T_DOUBLE, p) for p in pos], T_DOUBLE),
    }
    if item is not None:
        v["Item"] = Tag(T_COMPOUND, {"id": Tag(T_STRING, item), "count": Tag(T_INT, count)})
    return Tag(T_COMPOUND, v)


def save(tmp: Path, entities: list[Tag]) -> Path:
    world = tmp / "world"
    (world / "entities").mkdir(parents=True)
    (world / "level.dat").write_bytes(b"")
    root = Tag(T_COMPOUND, {"Entities": Tag(T_LIST, entities, T_COMPOUND)})
    anvil.region_write(world / "entities" / "r.0.0.mca", {(0, 0): root})
    return world


def build(tmp: Path, functions: dict[str, str]) -> Path:
    b = tmp / "build"
    d = b / "datapack" / "data" / "t" / "function"
    d.mkdir(parents=True)
    for name, body in functions.items():
        (d / f"{name}.mcfunction").write_text(body, encoding="utf-8")
    return b


def test_an_item_on_the_ground_no_load_function_summons_is_dw0955(ww, tmp_path):
    world = save(
        tmp_path,
        [
            entity("minecraft:item", (118.84, 66.0, 99.7), "minecraft:gravel", 2),
            entity("minecraft:item", (119.26, 66.0, 95.21), "minecraft:gravel", 1),
            entity("minecraft:villager", (120.0, 66.0, 90.0)),
        ],
    )
    code, lines, summary = ww.item_findings(build(tmp_path, {"setup": "say hi\n"}), world)
    assert code == 1
    assert summary["entities"] == 2 and summary["items"] == 3 and summary["declared"] == 0
    assert summary["by_item"] == {"minecraft:gravel": 3}
    assert lines[-1].startswith("DW0955 the server save holds 2 item entit")
    assert "minecraft:gravel x2 at [118.84, 66.0, 99.7]" in lines[-1]


def test_an_item_a_load_function_summons_is_declared(ww, tmp_path):
    world = save(tmp_path, [entity("minecraft:item", (1.5, 65.0, 1.5), "minecraft:apple")])
    b = build(
        tmp_path,
        {
            "setup_finish": 'summon minecraft:item 1.5 65 1.5 {Item:{id:"minecraft:apple"}}\n',
            # A runtime function's summon is no declaration for a save at load.
            "on_death": 'summon minecraft:item 1.5 65 1.5 {Item:{id:"minecraft:apple"}}\n',
        },
    )
    code, lines, summary = ww.item_findings(b, world)
    assert (code, summary["declared"], summary["entities"]) == (0, 1, 1)
    assert ww.declared_items(build(tmp_path / "other", {"on_death": "summon minecraft:item 0 0 0\n"})) == 0


def test_a_save_with_no_item_is_clean(ww, tmp_path):
    world = save(tmp_path, [entity("minecraft:mannequin", (0.0, 64.0, 0.0))])
    code, lines, summary = ww.item_findings(build(tmp_path, {}), world)
    assert code == 0 and summary["entities"] == 0
    assert lines == [
        "item entities: 0 in the server save holding 0 item(s) (none); "
        "0 declared by a `summon minecraft:item` in the load's functions"
    ]
