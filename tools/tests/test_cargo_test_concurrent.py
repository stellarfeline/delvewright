"""`tools/ci/cargo-test-concurrent.py`: every executable cargo lists runs once.

The listing is read from cargo's own JSON messages, so it is proven against a
recorded stream of them; the run is proven against real executables (small
scripts) that pass and fail, with cargo's build and doc-test calls stubbed.
"""

from __future__ import annotations

import importlib.util
import json
import pathlib
import subprocess

TOOLS = pathlib.Path(__file__).resolve().parents[1]

_spec = importlib.util.spec_from_file_location(
    "cargo_test_concurrent", TOOLS / "ci" / "cargo-test-concurrent.py"
)
tool = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(tool)


def artifact(exe, kind, src, test=True, manifest="/w/crates/a/Cargo.toml"):
    return json.dumps(
        {
            "reason": "compiler-artifact",
            "manifest_path": manifest,
            "target": {"kind": [kind], "src_path": src},
            "profile": {"test": test},
            "executable": exe,
        }
    )


def test_the_listing_is_every_test_executable_cargo_reports_once_in_its_order(monkeypatch):
    stream = "\n".join(
        [
            json.dumps({"reason": "build-script-executed"}),
            artifact("/t/a-1", "lib", "/w/crates/a/src/lib.rs"),
            artifact(None, "lib", "/w/crates/a/src/lib.rs", test=False),
            artifact("/t/delvec", "bin", "/w/crates/a/src/main.rs", test=False),
            artifact("/t/x-2", "test", "/w/crates/a/tests/x.rs"),
            artifact("/t/x-2", "test", "/w/crates/a/tests/x.rs"),
            "not json",
            json.dumps({"reason": "build-finished", "success": True}),
        ]
    )
    seen = {}

    def fake_run(cmd, **kw):
        seen["cmd"] = cmd
        return subprocess.CompletedProcess(cmd, 0, stdout=stream)

    monkeypatch.setattr(tool.subprocess, "run", fake_run)
    got = tool.listed(["--locked"])
    assert [t["exe"] for t in got] == ["/t/a-1", "/t/x-2"]
    assert seen["cmd"][:4] == ["cargo", "test", "--workspace", "--no-run"]
    assert "--locked" in seen["cmd"]


def script(path: pathlib.Path, code: int) -> str:
    path.write_text(f'#!/bin/sh\necho "test result: from $CARGO_MANIFEST_DIR"\nexit {code}\n')
    path.chmod(0o755)
    return str(path)


def targets(tmp_path, codes):
    (tmp_path / "Cargo.toml").write_text("")
    return [
        {
            "exe": script(tmp_path / f"t{i}", c),
            "src": str(tmp_path / "tests" / f"t{i}.rs"),
            "kind": "test",
            "manifest": str(tmp_path / "Cargo.toml"),
        }
        for i, c in enumerate(codes)
    ]


def test_every_executable_runs_and_a_failure_anywhere_fails_the_run(monkeypatch, tmp_path, capsys):
    monkeypatch.setattr(tool, "listed", lambda a: targets(tmp_path, [0, 1, 0]))
    monkeypatch.setattr(tool, "doc_tests", lambda a: 0)
    assert tool.main(["--jobs", "2"]) == 1
    out = capsys.readouterr().out
    assert "test executables: 3 run of 3 listed, 1 failed" in out
    assert out.count(f"test result: from {tmp_path}") == 3


def test_all_passing_is_green_and_a_doc_test_failure_is_not(monkeypatch, tmp_path, capsys):
    monkeypatch.setattr(tool, "listed", lambda a: targets(tmp_path, [0, 0]))
    monkeypatch.setattr(tool, "doc_tests", lambda a: 0)
    assert tool.main(["--jobs", "2"]) == 0
    assert "2 run of 2 listed, 0 failed" in capsys.readouterr().out
    monkeypatch.setattr(tool, "doc_tests", lambda a: 101)
    assert tool.main(["--jobs", "1"]) == 1


def test_a_listing_of_nothing_binds_nothing_and_refuses(monkeypatch):
    monkeypatch.setattr(tool, "listed", lambda a: [])
    assert tool.main(["--jobs", "2"]) == 2
