"""Under `gallery/baseline/` git tracks only the expected-warnings ledger (ADR-0034).

A file of its own because it asks `git ls-files`, which the reach guard judges as
a read of the whole tracked tree; `conftest.WHOLE_TREE` lists it, so it runs on
every pull request — including the one that adds a file under `gallery/baseline/`.
"""

from __future__ import annotations

import pathlib
import subprocess

REPO = pathlib.Path(__file__).resolve().parents[2]


def test_only_the_warning_ledger_is_committed():
    tracked = subprocess.run(
        ["git", "-C", str(REPO), "ls-files", "gallery/baseline"], capture_output=True, text=True, check=True
    ).stdout.split()
    assert tracked == ["gallery/baseline/warnings.json"]
    ignored = (REPO / ".gitignore").read_text().splitlines()
    for name in ("header.json", "manifests.json", "delta.json"):
        assert f"gallery/baseline/{name}" in ignored
