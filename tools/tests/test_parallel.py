"""`tools/lib/parallel.py`: concurrent runs, read back in the order they were listed."""

from __future__ import annotations

import sys
import threading
import time
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "lib"))
import parallel  # noqa: E402


def test_results_come_back_in_input_order_whatever_order_they_finish_in():
    # The first item finishes LAST, so an order taken from completion would be reversed.
    def slow_first(i: int) -> int:
        time.sleep(0.05 * (5 - i))
        return i * 10

    got = [o.get() for o in parallel.ordered(slow_first, range(5), n=5)]
    assert got == [0, 10, 20, 30, 40]


def test_the_runs_actually_overlap():
    # Perturbed toward the vacuous shape: a pool that ran one task at a time
    # would never see two inside at once.
    inside, peak, lock = [0], [0], threading.Lock()

    def task(_):
        with lock:
            inside[0] += 1
            peak[0] = max(peak[0], inside[0])
        time.sleep(0.05)
        with lock:
            inside[0] -= 1

    for o in parallel.ordered(task, range(8), n=4):
        o.get()
    assert peak[0] > 1


def test_an_exception_surfaces_at_its_own_position_and_not_before():
    def boom_at_two(i: int) -> int:
        if i == 2:
            raise ValueError("two")
        return i

    seen = []
    with pytest.raises(ValueError, match="two"):
        for o in parallel.ordered(boom_at_two, range(5), n=5):
            seen.append(o.get())
    assert seen == [0, 1]


def test_the_job_count_is_read_from_the_environment(monkeypatch):
    monkeypatch.setenv(parallel.ENV, "1")
    assert parallel.jobs() == 1
    monkeypatch.setenv(parallel.ENV, "0")
    with pytest.raises(SystemExit):
        parallel.jobs()
    monkeypatch.setenv(parallel.ENV, "many")
    with pytest.raises(SystemExit):
        parallel.jobs()
    monkeypatch.delenv(parallel.ENV)
    assert parallel.jobs() >= 1
