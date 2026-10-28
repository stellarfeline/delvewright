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


def _child(mib: int, hold: float = 0.0) -> list[str]:
    """A child that holds `mib` MiB resident for `hold` seconds, then exits."""
    code = (
        f"import time; b = bytearray({mib} << 20); "
        f"[b.__setitem__(i, 1) for i in range(0, len(b), 4096)]; time.sleep({hold})"
    )
    return [sys.executable, "-c", code]


def test_a_child_s_status_output_and_peak_come_back(monkeypatch):
    monkeypatch.setattr(parallel, "_ADMISSION", parallel.Admission(1 << 40))
    r = parallel.run([sys.executable, "-c", "print('out'); import sys; sys.exit(3)"], kind="t")
    assert (r.returncode, r.stdout) == (3, "out\n")
    parallel.run(_child(64), kind="big")
    assert parallel.admission().peaks["big"] >= 64 << 20


def test_a_killed_child_reports_its_signal_not_a_status(monkeypatch):
    monkeypatch.setattr(parallel, "_ADMISSION", parallel.Admission(1 << 40))
    r = parallel.run([sys.executable, "-c", "import os, signal; os.kill(os.getpid(), signal.SIGKILL)"], kind="t")
    assert r.returncode == -9


def test_a_kind_never_measured_runs_alone_and_then_by_its_peak(monkeypatch):
    # Perturbed toward the over-committing shape: without the admission the
    # four would overlap, as `test_the_runs_actually_overlap` shows the pool does.
    # The budget holds two measured peaks and not three.
    adm = parallel.Admission(0)
    monkeypatch.setattr(parallel, "_ADMISSION", adm)
    parallel.run(_child(40), kind="probe")
    peak = adm.peaks["probe"]
    adm.budget = 2 * peak + peak // 2
    adm.most_running = 0
    for o in parallel.ordered(lambda _: parallel.run(_child(40, 0.3), kind="probe"), range(6), n=6):
        assert o.get().returncode == 0
    assert adm.most_running == 2, adm.line()
    # A child too large for the budget still runs, alone.
    adm.budget = 0
    adm.most_running = 0
    for o in parallel.ordered(lambda _: parallel.run(_child(40, 0.2), kind="probe"), range(3), n=3):
        o.get()
    assert adm.most_running == 1, adm.line()


def test_a_kind_no_child_has_finished_runs_alone(monkeypatch):
    monkeypatch.setattr(parallel, "_ADMISSION", parallel.Admission(1 << 40))
    span = "import time; s = time.time(); time.sleep(0.3); print(s, time.time())"
    runs = parallel.ordered(lambda _: parallel.run([sys.executable, "-c", span], kind="new"), range(3), n=3)
    spans = sorted(tuple(map(float, o.get().stdout.split())) for o in runs)
    first = spans[0]
    assert all(s >= first[1] for s, _ in spans[1:]), spans
    # ...and once one has, the rest share the host by its peak.
    later = [s for s, _ in spans[1:]]
    assert abs(later[0] - later[1]) < 0.25, spans


def test_the_available_memory_is_read():
    assert parallel.available_memory() > 0
