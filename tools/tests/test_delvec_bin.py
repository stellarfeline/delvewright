"""Guards for `tools/lib/delvec_bin.py` — the one resolver six gates share.

The defect these are written against is not "the resolver picks the wrong file".
It is that a tool whose whole verdict is a property of the compiler in THIS tree
answered from a binary nine days old, said nothing about which binary it had
found, and reported a false red about a schema field that had landed.

So the assertions here are about the two properties that would have caught it:
the instrument is NAMED on every run, and a binary older than the sources is a
refusal rather than a quiet answer. Each is demonstrated by construction on a
throwaway repository — a fake `delvec` whose mtime the test sets — so the
refusal is observed firing rather than read out of the source.
"""

from __future__ import annotations

import importlib.util
import os
import subprocess
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parent.parent.parent
LIB = REPO / "tools" / "lib" / "delvec_bin.py"


@pytest.fixture()
def delvec_bin():
    spec = importlib.util.spec_from_file_location("delvec_bin", LIB)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def fake_repo(tmp_path: Path, *, git: bool = True) -> Path:
    """A tree shaped like this repository's: tracked sources under `crates/`."""
    repo = tmp_path / "repo"
    (repo / "crates" / "compiler" / "src").mkdir(parents=True)
    (repo / "crates" / "compiler" / "src" / "main.rs").write_text("fn main() {}\n")
    (repo / "Cargo.toml").write_text("[workspace]\n")
    (repo / "Cargo.lock").write_text("version = 4\n")
    (repo / "target" / "debug").mkdir(parents=True)
    if git:
        env = {
            **os.environ,
            "GIT_AUTHOR_NAME": "t",
            "GIT_AUTHOR_EMAIL": "t@t",
            "GIT_COMMITTER_NAME": "t",
            "GIT_COMMITTER_EMAIL": "t@t",
        }
        subprocess.run(["git", "init", "-q", str(repo)], check=True, env=env)
        subprocess.run(["git", "-C", str(repo), "add", "-A"], check=True, env=env)
        subprocess.run(
            ["git", "-C", str(repo), "commit", "-qm", "t"], check=True, env=env
        )
    return repo


def fake_delvec(repo: Path, rel: str = "target/debug/delvec") -> Path:
    p = repo / rel
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text("#!/bin/sh\necho 'delvec 9.9.9, dsl 9.9.9, mc 1.21.11'\n")
    p.chmod(0o755)
    return p


def age(p: Path, seconds: int) -> None:
    """Move a file's mtime `seconds` into the past."""
    ts = p.stat().st_mtime - seconds
    os.utime(p, (ts, ts))


class Sink:
    def __init__(self) -> None:
        self.text = ""

    def write(self, s: str) -> int:
        self.text += s
        return len(s)

    def flush(self) -> None:
        pass


def test_a_binary_older_than_the_sources_is_refused_by_name(delvec_bin, tmp_path):
    repo = fake_repo(tmp_path)
    binary = fake_delvec(repo)
    age(binary, 9 * 24 * 3600)  # the nine-day-old release binary, reproduced

    sink = Sink()
    with pytest.raises(SystemExit) as e:
        delvec_bin.resolve(None, repo=repo, caller="t", stream=sink)
    assert e.value.code == 1
    assert "STALE INSTRUMENT" in sink.text
    assert "main.rs" in sink.text or "Cargo" in sink.text
    assert "cargo build -p delvec" in sink.text


def test_a_current_binary_resolves_and_names_itself(delvec_bin, tmp_path):
    repo = fake_repo(tmp_path)
    binary = fake_delvec(repo)

    sink = Sink()
    got = delvec_bin.resolve(None, repo=repo, caller="t", stream=sink)
    assert got == binary
    # The three facts the defect's output was missing.
    assert str(binary) in sink.text
    assert "built " in sink.text
    assert "delvec 9.9.9" in sink.text


def test_the_refusal_also_covers_a_binary_the_caller_named(delvec_bin, tmp_path):
    """CI passes `--delvec` explicitly; an explicit path may be stale too."""
    repo = fake_repo(tmp_path)
    binary = fake_delvec(repo, "target/release/delvec")
    age(binary, 3600)

    sink = Sink()
    with pytest.raises(SystemExit):
        delvec_bin.resolve(binary, repo=repo, caller="t", stream=sink)
    assert "STALE INSTRUMENT" in sink.text


def test_release_wins_the_search_and_the_line_says_so(delvec_bin, tmp_path):
    repo = fake_repo(tmp_path)
    fake_delvec(repo, "target/debug/delvec")
    rel = fake_delvec(repo, "target/release/delvec")

    sink = Sink()
    got = delvec_bin.resolve(None, repo=repo, caller="t", stream=sink)
    assert got == rel
    assert "target/release/delvec" in sink.text


def test_nothing_found_is_a_refusal_naming_both_places(delvec_bin, tmp_path):
    repo = fake_repo(tmp_path)
    sink = Sink()
    with pytest.raises(SystemExit):
        delvec_bin.resolve(None, repo=repo, caller="t", stream=sink)
    assert "target/release/delvec" in sink.text
    assert "target/debug/delvec" in sink.text


def test_a_required_caller_is_never_handed_an_inferred_engine(delvec_bin, tmp_path):
    """A required caller asks WHICH engine did something; inferring one would
    answer a question nobody asked."""
    repo = fake_repo(tmp_path)
    fake_delvec(repo)  # present, and still not used
    sink = Sink()
    with pytest.raises(SystemExit):
        delvec_bin.resolve(None, repo=repo, caller="t", required=True, stream=sink)
    assert "never inferred" in sink.text


def test_a_missing_named_path_is_refused(delvec_bin, tmp_path):
    repo = fake_repo(tmp_path)
    sink = Sink()
    with pytest.raises(SystemExit):
        delvec_bin.resolve(repo / "nowhere/delvec", repo=repo, caller="t", stream=sink)
    assert "is not a file" in sink.text


def test_a_sibling_targets_fresh_artifacts_cannot_make_the_gate_refuse(
    delvec_bin, tmp_path
):
    """`crates/render` carries its own `target/`. Walking `crates/` naively makes
    every run refuse forever, which is the cry-wolf direction that gets a gate
    disabled. Tracked files are the population."""
    repo = fake_repo(tmp_path)
    binary = fake_delvec(repo)
    junk = repo / "crates" / "render" / "target" / "debug" / "build.rs"
    junk.parent.mkdir(parents=True)
    junk.write_text("fresh\n")  # newer than the binary, and untracked

    sink = Sink()
    got = delvec_bin.resolve(None, repo=repo, caller="t", stream=sink)
    assert got == binary
    assert "git ls-files" in sink.text


def test_without_git_the_walk_skips_target_and_says_which_method_decided(
    delvec_bin, tmp_path
):
    repo = fake_repo(tmp_path, git=False)
    binary = fake_delvec(repo)
    junk = repo / "crates" / "render" / "target" / "debug" / "build.rs"
    junk.parent.mkdir(parents=True)
    junk.write_text("fresh\n")

    sink = Sink()
    got = delvec_bin.resolve(None, repo=repo, caller="t", stream=sink)
    assert got == binary
    assert "directory walk" in sink.text


def test_every_tool_that_runs_an_engine_uses_the_one_resolver():
    """The extraction is only worth anything if nothing kept its private copy.

    Binding: the five `tools/*.py` that take a `--delvec` PATH and run it. The
    population is stated so a sixth tool added beside them is visibly absent
    from this list rather than silently uncovered.
    """
    users = [
        "check-gallery-coverage.py",
        "check-gallery-render.py",
        "check-whole-map-render.py",
        "gallery-baseline.py",
        "gallery-build.py",
    ]
    assert len(users) == 5
    for name in users:
        src = (REPO / "tools" / name).read_text(encoding="utf-8")
        assert "from delvec_bin import resolve" in src, name
        assert 'default=str(REPO / "target/release/delvec")' not in src, name
