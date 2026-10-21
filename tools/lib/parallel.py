"""Independent engine runs, concurrently, read back in the order they were listed.

The gallery gates each run many `delvec` invocations that share nothing but
read-only inputs: one per probe, per build point, per language. Run one after
another on a four-core runner they leave three cores idle, and the coverage
gate's probe walk alone took 1707 s that way.

This module is the one place that decides how such runs are spread, so every
gate that needs it takes the same rule:

- **a bounded pool sized to the cores this process may use** (`jobs()`), never a
  number typed per call site;
- **results come back in input order**, each as the value or the exception its
  call raised, so a caller walks them exactly as its serial loop walked the
  items and its output, exit status and first refusal are the serial ones;
- **the caller owns isolation**: every task gets its own output and scratch
  paths from the caller, and a task never prints — it returns what the serial
  loop would have printed, and the in-order walk prints it;
- **memory is admitted, never over-committed** (`run()`): a task starts its
  child only when the budget — a share of the host's available memory, read
  when the process's first child starts — holds the reservations of every
  child already running plus this one's. A child's reservation is the largest
  peak resident set any finished child of its declared kind measured
  (`os.wait4`); a kind no child has finished yet reserves the whole budget, so
  it runs alone. Cores bound how many run; memory bounds which of them start.
  A child killed by pressure from outside the pool still returns its signal as
  a negative status, and the caller says so — it is never read as a verdict.

Threads, not processes: each task spends its time waiting on a child process,
which releases the GIL, and a thread pool keeps module-level functions
patchable by the tests that drive these gates.
"""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import threading
from collections import deque
from concurrent.futures import Future, ThreadPoolExecutor
from typing import Callable, Iterable, Iterator, TypeVar

T = TypeVar("T")
R = TypeVar("R")

# Read so a person can reproduce the serial walk (`DELVEWRIGHT_JOBS=1`) without
# editing a gate.
ENV = "DELVEWRIGHT_JOBS"


def jobs() -> int:
    """How many tasks run at once: `DELVEWRIGHT_JOBS`, else the cores this process may use."""
    raw = os.environ.get(ENV)
    if raw is not None:
        try:
            n = int(raw)
        except ValueError:
            raise SystemExit(f"error: {ENV}={raw!r} is not a whole number of jobs")
        if n < 1:
            raise SystemExit(f"error: {ENV}={raw!r}; at least one job must run")
        return n
    try:
        return len(os.sched_getaffinity(0)) or 1
    except AttributeError:  # macOS has no affinity call
        return os.cpu_count() or 1


class Outcome:
    """One task's result: `get()` returns its value or raises what the task raised."""

    __slots__ = ("_future",)

    def __init__(self, future: Future):
        self._future = future

    def get(self):
        return self._future.result()


def ordered(fn: Callable[[T], R], items: Iterable[T], n: int | None = None) -> Iterator[Outcome]:
    """Run `fn` over `items` on `n` workers; yield one `Outcome` per item, in item order.

    Every item is submitted up front. A caller that stops walking early (a
    refusal at item k) leaves the generator, and the pool cancels whatever has
    not started — the serial loop would never have started it either.
    """
    items = list(items)
    workers = max(1, min(n or jobs(), len(items) or 1))
    pool = ThreadPoolExecutor(max_workers=workers)
    try:
        futures = [pool.submit(fn, item) for item in items]
        for f in futures:
            yield Outcome(f)
    finally:
        pool.shutdown(wait=True, cancel_futures=True)


# ------------------------------------------------------------ memory admission

# The share of the available memory the pool's children may hold together: the
# rest is the host's, and the gate's own.
BUDGET_SHARE = (3, 4)


def available_memory() -> int:
    """The bytes the host can give new processes now, read from the kernel.

    Linux: `MemAvailable` in `/proc/meminfo`. macOS: the free, inactive,
    speculative and purgeable pages `vm_stat` reports, at its stated page size.
    Anything else, or a read that finds neither, refuses: a pool that cannot
    read memory cannot promise not to over-commit it.
    """
    try:
        with open("/proc/meminfo", encoding="ascii") as f:
            for line in f:
                if line.startswith("MemAvailable:"):
                    return int(line.split()[1]) * 1024
    except FileNotFoundError:
        pass
    if sys.platform == "darwin":
        r = subprocess.run(["vm_stat"], capture_output=True, text=True)
        if r.returncode == 0:
            lines = r.stdout.splitlines()
            page = int(lines[0].split("page size of")[1].split()[0])
            wanted = ("Pages free:", "Pages inactive:", "Pages speculative:", "Pages purgeable:")
            pages = sum(
                int(line.split(":")[1].strip().rstrip("."))
                for line in lines[1:]
                if line.startswith(wanted)
            )
            return pages * page
    raise SystemExit(
        "error: the available memory could not be read (no `MemAvailable` in /proc/meminfo, "
        "no `vm_stat`); the pool will not start children it cannot admit"
    )


class Admission:
    """Which children may start: their reservations fit the budget, first come first served."""

    def __init__(self, budget: int):
        self.budget = budget
        self.held = 0
        self.running = 0
        self.most_running = 0
        self.peaks: dict[str, int] = {}
        self._waiting: deque = deque()
        self._cond = threading.Condition()

    def reservation(self, kind: str) -> int:
        return self.peaks.get(kind, self.budget)

    def acquire(self, kind: str) -> int:
        me = object()
        with self._cond:
            self._waiting.append(me)
            while True:
                r = self.reservation(kind)
                if self._waiting[0] is me and (self.running == 0 or self.held + r <= self.budget):
                    self._waiting.popleft()
                    self.held += r
                    self.running += 1
                    self.most_running = max(self.most_running, self.running)
                    self._cond.notify_all()
                    return r
                self._cond.wait()

    def release(self, kind: str, reserved: int, peak: int | None) -> None:
        with self._cond:
            self.held -= reserved
            self.running -= 1
            if peak is not None:
                self.peaks[kind] = max(self.peaks.get(kind, 0), peak)
            self._cond.notify_all()

    def line(self) -> str:
        """What the admission held to, for a gate's binding lines."""
        mib = 1 << 20
        peaks = ", ".join(f"{k} {v // mib} MiB" for k, v in sorted(self.peaks.items())) or "none"
        return (
            f"memory admission: budget {self.budget // mib} MiB "
            f"({BUDGET_SHARE[0]}/{BUDGET_SHARE[1]} of the memory available at the first child); "
            f"peak resident set per kind: {peaks}; at most {self.most_running} child(ren) at once"
        )


_ADMISSION: Admission | None = None
_ADMISSION_LOCK = threading.Lock()


def admission() -> Admission:
    """The process's one admission, its budget read when the first child starts."""
    global _ADMISSION
    with _ADMISSION_LOCK:
        if _ADMISSION is None:
            _ADMISSION = Admission(available_memory() * BUDGET_SHARE[0] // BUDGET_SHARE[1])
        return _ADMISSION


def run(cmd: list[str], *, kind: str, **popen) -> subprocess.CompletedProcess:
    """`subprocess.run(cmd, capture_output=True, text=True)`, started only when memory admits it.

    `kind` is the caller's statement of what the child is (`"delvec build"`):
    a child of a kind is taken to need what the largest finished one of that
    kind measured. The status is the child's own, a signal as its negative
    number, exactly as `subprocess` reports it.
    """
    adm = admission()
    reserved = adm.acquire(kind)
    peak: int | None = None
    try:
        with tempfile.TemporaryFile() as out, tempfile.TemporaryFile() as err:
            child = subprocess.Popen(cmd, stdout=out, stderr=err, **popen)
            try:
                _, status, usage = os.wait4(child.pid, 0)
            except BaseException:
                child.kill()
                child.wait()
                raise
            child.returncode = os.waitstatus_to_exitcode(status)
            # Kilobytes on Linux, bytes on macOS (getrusage(2) on each).
            peak = usage.ru_maxrss * (1 if sys.platform == "darwin" else 1024)
            out.seek(0)
            err.seek(0)
            stdout = out.read().decode("utf-8", errors="replace")
            stderr = err.read().decode("utf-8", errors="replace")
    finally:
        adm.release(kind, reserved, peak)
    return subprocess.CompletedProcess(cmd, child.returncode, stdout, stderr)
