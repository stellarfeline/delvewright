"""Guards for `tools/ci/check-demo-queue.py`: every `campaigns/<id>` a demo-queue
Status cell names is a directory the content pin carries.

The content repository is a tiny real git repo built here; the queue is a
string. The red this gate exists for is reproduced by naming a directory the
fixture's commit does not hold.
"""

from __future__ import annotations

import importlib.util
import subprocess
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[2]
CHECKER = REPO / "tools" / "ci" / "check-demo-queue.py"


@pytest.fixture
def gate():
    spec = importlib.util.spec_from_file_location("cdq", CHECKER)
    mod = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(mod)
    return mod


def queue(*status: str) -> str:
    rows = "\n".join(f"| Mechanic {i} (0001) | a demo | {s} |" for i, s in enumerate(status))
    return (
        "# Demo levels\n\n## Mechanic demos\n\n"
        "| Mechanic (spec) | Demo concept | Status |\n|---|---|---|\n" + rows + "\n"
    )


def content_repo(root: Path, campaigns: list[str]) -> str:
    root.mkdir(parents=True)
    run = lambda *a: subprocess.run(a, cwd=root, check=True, capture_output=True, text=True)  # noqa: E731
    run("git", "init", "-q", "-b", "main")
    run("git", "config", "user.email", "test@example.com")
    run("git", "config", "user.name", "Test")
    (root / "campaigns").mkdir()
    (root / "campaigns" / "README.md").write_text("campaigns\n")
    for c in campaigns:
        (root / "campaigns" / c).mkdir()
        (root / "campaigns" / c / "world.json").write_text("{}\n")
    run("git", "add", "-A")
    run("git", "commit", "-q", "-m", "fixture")
    return run("git", "rev-parse", "HEAD").stdout.strip()


def test_an_address_the_pin_carries_passes_with_its_count(gate, tmp_path):
    sha = content_repo(tmp_path / "content", ["the-ferry"])
    have = gate.carried(tmp_path / "content", sha)
    assert have == {"the-ferry"}
    findings, b = gate.judge(queue("built (`campaigns/the-ferry`)", "pending"), have)
    assert findings == []
    assert b == {"rows": 2, "cells": 1, "addresses": 1, "carried": 1}


def test_an_address_the_pin_lacks_is_refused_naming_the_line(gate, tmp_path):
    sha = content_repo(tmp_path / "content", ["the-ferry"])
    have = gate.carried(tmp_path / "content", sha)
    findings, b = gate.judge(
        queue("built (`campaigns/the-ferry`)", "built (`campaigns/the-ferry-bell`)"), have
    )
    assert len(findings) == 1, findings
    assert "campaigns/the-ferry-bell" in findings[0]
    assert "docs/demo-levels.md:8:" in findings[0], findings[0]
    assert b["addresses"] == 2


def test_a_queue_naming_no_address_passes_with_zero_stated(gate, tmp_path):
    sha = content_repo(tmp_path / "content", [])
    findings, b = gate.judge(queue("pending", "covered"), gate.carried(tmp_path / "content", sha))
    assert findings == [] and b["cells"] == 0 and b["carried"] == 0


def test_an_absent_content_repository_is_a_failure_never_a_skip(gate, tmp_path):
    with pytest.raises(SystemExit) as e:
        gate.carried(tmp_path / "nowhere", "0" * 40)
    assert "not a git repository" in str(e.value)
    sha = content_repo(tmp_path / "content", [])
    with pytest.raises(SystemExit) as e:
        gate.carried(tmp_path / "content", "1" * 40)
    assert "cannot list" in str(e.value)


def test_the_main_entry_reads_the_live_queue_and_prints_its_binding(gate, tmp_path, capsys):
    # Every address the live queue names, carried by a fixture commit: the run
    # reads `docs/demo-levels.md` itself and states its binding.
    text = (REPO / "docs" / "demo-levels.md").read_text(encoding="utf-8")
    named = sorted(set(gate.ADDRESS.findall(text)))
    sha = content_repo(tmp_path / "content", named)
    assert gate.main(["--content", str(tmp_path / "content"), "--sha", sha]) == 0
    out = capsys.readouterr().out
    assert "check-demo-queue binding:" in out and f"carries {len(named)} campaign(s)" in out
    # And one address the fixture lacks reds it.
    if named:
        sha = content_repo(tmp_path / "less", named[1:])
        assert gate.main(["--content", str(tmp_path / "less"), "--sha", sha]) == 1
