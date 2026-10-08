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
  loop would have printed, and the in-order walk prints it.

Threads, not processes: each task spends its time waiting on a child process,
which releases the GIL, and a thread pool keeps module-level functions
patchable by the tests that drive these gates.
"""

from __future__ import annotations

import os
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
