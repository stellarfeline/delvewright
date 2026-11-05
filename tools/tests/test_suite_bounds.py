"""`tools/lib/suite_bounds.py`: the one reader of the suite's time bounds (spec-0126 §5)."""

from __future__ import annotations

import pathlib
import sys

import pytest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "lib"))

import suite_bounds  # noqa: E402

PROFILE = """[profile.ci]
global-timeout = "78m"
slow-timeout = { period = "60s", terminate-after = 2 }

[[profile.ci.overrides]]
filter = 'test(=a::slow)'
slow-timeout = { period = "300s", terminate-after = 1 }

[[profile.ci.overrides]]
filter = 'test(=a::marked)'
slow-timeout = "90s"
"""


def tree(tmp: pathlib.Path, text: str) -> pathlib.Path:
    (tmp / ".config").mkdir()
    (tmp / ".config/nextest.toml").write_text(text, encoding="utf-8")
    return tmp


def test_the_bounds_are_read_from_the_profile(tmp_path):
    b = suite_bounds.bounds(tree(tmp_path, PROFILE))
    assert b.test_seconds == 120
    assert b.run_seconds == 78 * 60
    assert b.overrides == {"a::slow": 300, "a::marked": None}
    assert b.for_test("a::slow") == 300 and b.for_test("a::other") == 120


def test_an_unarmed_profile_has_no_bounds(tmp_path):
    b = suite_bounds.bounds(tree(tmp_path, '[profile.ci]\nslow-timeout = { period = "60s" }\n'))
    assert b.test_seconds is None and b.run_seconds is None


@pytest.mark.parametrize("text,want", [("500ms", 0.5), ("60s", 60), ("78m", 4680), ("1h", 3600)])
def test_durations(text, want):
    assert suite_bounds.seconds(text) == want


def test_an_unknown_duration_is_refused():
    with pytest.raises(ValueError):
        suite_bounds.seconds("1 fortnight")


def test_the_live_profile_reads():
    root = pathlib.Path(__file__).resolve().parents[2]
    b = suite_bounds.bounds(root)
    assert len(b.overrides) > 0
