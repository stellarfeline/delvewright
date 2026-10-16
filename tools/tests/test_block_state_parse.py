"""`check-written-world.py`'s block-state `split` answers the committed
fixture the harness's parse rule (`harness/src/block-state.ts`) is judged on,
row for row: the two readers of the format agree."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "tools" / "ci" / "check-written-world.py"
FIXTURE = ROOT / "harness" / "test" / "fixtures" / "block-states.json"


@pytest.fixture(scope="module")
def ww():
    spec = importlib.util.spec_from_file_location("check_written_world", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


ROWS = json.loads(FIXTURE.read_text(encoding="utf-8"))["parse"]


@pytest.mark.parametrize("row", ROWS, ids=[r["text"] for r in ROWS])
def test_split_agrees_with_the_shared_fixture(ww, row):
    name, props = ww.split(row["text"])
    if ":" not in name:
        name = f"minecraft:{name}"
    assert (name, props) == (row["name"], row["properties"])
