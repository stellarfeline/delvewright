r"""Guard: main is fast-forwarded to a release commit only when every required check SUCCEEDED on it.

`tools/wait-required-checks.py` with the GitHub read injected. The required set
is the real `.github/required-status-checks.txt`, so a context added there is
waited for without an edit here.
"""

from __future__ import annotations

import importlib.util
import pathlib

REPO = pathlib.Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("wait_required_checks", REPO / "tools" / "wait-required-checks.py")
mod = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(mod)

NAMES = mod.required_names()


def _world(overrides: dict[str, tuple[str, str | None]] | None = None, missing: tuple[str, ...] = ()):
    runs = []
    for i, name in enumerate(NAMES):
        if name in missing:
            continue
        status, conclusion = (overrides or {}).get(name, ("completed", "success"))
        runs.append({"id": 100 + i, "name": name, "status": status, "conclusion": conclusion})

    def fetch(path: str):
        assert "app_id=15368" in path and "filter=latest" in path, path
        return {"check_runs": runs if path.endswith("page=1") else []}

    return fetch


def _run(fetch, timeout="0"):
    sleeps = []
    code = mod.main(["--sha", "a" * 40, "--repo", "o/r", "--timeout-minutes", timeout], fetch=fetch, sleep=sleeps.append, clock=lambda: 0.0)
    return code, sleeps


def test_the_required_set_is_the_lockstep_file():
    assert len(NAMES) >= 15 and "tier 2 (datapack load + PackTest)" in NAMES


def test_every_required_check_succeeded():
    assert _run(_world())[0] == 0


def test_a_skipped_required_check_is_refused():
    """The perturbation the dispatch trigger exists to prevent: a PR-only job skipped."""
    code, _ = _run(_world({"tier 2 (datapack load + PackTest)": ("completed", "skipped")}))
    assert code == 1


def test_a_failed_required_check_is_refused_without_waiting():
    code, sleeps = _run(_world({"rust (fmt, clippy, test)": ("completed", "failure")}), timeout="10")
    assert code == 1 and sleeps == []


def test_a_missing_check_is_waited_for_then_times_out():
    code, _ = _run(_world(missing=("docs (local link check)",)))
    assert code == 1


def test_a_running_check_is_waited_for():
    calls = {"n": 0}
    done = _world()
    running = _world({"gallery (coverage + build + baseline)": ("in_progress", None)})

    def fetch(path):
        calls["n"] += 1
        return (running if calls["n"] == 1 else done)(path)

    ticks = iter([0.0, 0.0, 1.0, 1.0])
    code = mod.main(["--sha", "a" * 40, "--repo", "o/r", "--timeout-minutes", "10"], fetch=fetch, sleep=lambda s: None, clock=lambda: next(ticks))
    assert code == 0 and calls["n"] == 2
