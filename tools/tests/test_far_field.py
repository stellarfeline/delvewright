"""The far-field threshold against its second method (spec-0090 §5.2).

`compiler::loop::FAR_FIELD_SHIFT_DEGREES` is station 4's largest far-field
shift as the engine reads it, rounded up at the fourth decimal; the engine's
own reading is asserted by `station_4_calibrates_the_far_field_threshold`.
This file holds the constant against the reading of a method that shares no
code with the engine — `crates/delvec/tests/measured/far_field.py`, the hall
rebuilt from the spike's constants with its own light flood and voxel walk —
so a change to either side that moves the number reds here.
"""

import importlib.util
import math
import pathlib
import re

REPO = pathlib.Path(__file__).resolve().parents[2]
TOOL = REPO / "crates" / "delvec" / "tests" / "measured" / "far_field.py"
LOOP_RS = REPO / "crates" / "delvec" / "src" / "compiler" / "loop.rs"


def _tool():
    spec = importlib.util.spec_from_file_location("far_field", TOOL)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def _engine_threshold() -> float:
    found = re.findall(
        r"^pub const FAR_FIELD_SHIFT_DEGREES: f64 = ([0-9.]+);$",
        LOOP_RS.read_text(),
        flags=re.M,
    )
    assert len(found) == 1, f"one threshold constant in {LOOP_RS}, found {found}"
    return float(found[0])


def test_station_4_reads_the_engines_threshold():
    shift, cell, eye, why = _tool().measure(60, 12, False)
    assert round(shift, 6) == 1.285125
    assert list(cell) == [4095, 64, 4260]
    assert why == "light 5 vs 3"
    assert [round(v, 2) for v in eye] == [4097.8, 65.62, 4206.8]
    assert _engine_threshold() == math.ceil(shift * 10_000) / 10_000


def test_the_lit_exit_rows_agree_with_the_spec():
    tool = _tool()
    limit = _engine_threshold()
    assert round(tool.measure(60, 12, True)[0], 4) == 1.3418
    assert tool.measure(60, 12, True)[0] > limit
    assert round(tool.measure(66, 12, True)[0], 4) == 1.0491
    assert round(tool.measure(48, 6, True)[0], 4) == 1.0198
