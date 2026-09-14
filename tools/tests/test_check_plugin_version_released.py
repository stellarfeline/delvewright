r"""Guard: a plugin version moved past without its Release is a red (ADR-0028 §5).

`tools/check-plugin-version-released.py` holds that the `plugin.json` version a
pull request moves AWAY from has a published `delvewright--v<X>` Release. These
tests prove it binds: the red arrives on the shapes a skipped release takes, and
the green is not the checker failing to look.

OFFLINE. The trees are scratch git repositories holding only the plugin
manifest; GitHub is `tests/_fake_github.py` over `DW_GITHUB_API`.

THE PERTURBATION is `test_red_when_the_retired_version_has_no_release`: a
change moving 1.4.3 -> 1.4.4 while no `delvewright--v1.4.3` Release exists —
exactly the state `main` is in until the first manual plugin release runs. Its
control, `test_green_when_the_retired_version_is_released`, differs only in the
served Release.
"""

from __future__ import annotations

import json
import os
import subprocess
from pathlib import Path

import pytest

from _fake_github import World, serve

REPO = Path(__file__).resolve().parents[2]
CHECKER = REPO / "tools" / "check-plugin-version-released.py"
MANIFEST = ".claude/skills/delvewright/.claude-plugin/plugin.json"


def _write(repo: Path, version: str) -> None:
    path = repo / MANIFEST
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps({"name": "delvewright", "version": version}) + "\n", encoding="utf-8")


def _repo(root: Path, base: str, tree: str) -> tuple[Path, str]:
    repo = root / "tree"
    repo.mkdir(parents=True)

    def git(*args: str) -> str:
        return subprocess.run(["git", "-C", str(repo), *args], check=True, capture_output=True, text=True).stdout

    git("init", "-q", "-b", "main")
    git("config", "user.email", "t@example.com")
    git("config", "user.name", "t")
    _write(repo, base)
    git("add", ".")
    git("commit", "-qm", "base")
    base_sha = git("rev-parse", "HEAD").strip()
    git("update-ref", "refs/remotes/origin/main", "HEAD")
    if tree != base:
        _write(repo, tree)
        git("commit", "-qam", "bump")
    return repo, base_sha


def _released(world: World, version: str, commit: str, *, assets=None, draft=False, carries=None) -> None:
    tag = f"delvewright--v{version}"
    names = assets if assets is not None else [f"delvewright-plugin-{version}.zip", "SHA256SUMS"]
    world.releases[tag] = {"draft": draft, "assets": {n: b"x" for n in names}}
    world.tags[tag] = commit
    world.files[(MANIFEST, commit)] = json.dumps({"name": "delvewright", "version": carries or version}).encode()


@pytest.fixture
def github():
    stops = []

    def start(world: World) -> str:
        url, stop = serve(world)
        stops.append(stop)
        return url

    yield start
    for stop in stops:
        stop()


def _run(repo: Path, api: str) -> subprocess.CompletedProcess[str]:
    env = {**os.environ, "DW_GITHUB_API": api, "no_proxy": "*", "NO_PROXY": "*"}
    env.pop("GH_TOKEN", None)
    env.pop("GITHUB_TOKEN", None)
    env.pop("GITHUB_REPOSITORY", None)
    return subprocess.run(
        ["python3", str(CHECKER), "--repo", str(repo)], capture_output=True, text=True, env=env
    )


def test_red_when_the_retired_version_has_no_release(tmp_path, github):
    repo, _ = _repo(tmp_path, "1.4.3", "1.4.4")
    result = _run(repo, github(World()))
    assert result.returncode == 1, result.stdout + result.stderr
    assert "0 of 1 retired plugin version(s) released" in result.stderr
    assert "delvewright--v1.4.3" in result.stderr
    assert "(it is absent)" in result.stderr
    # The remedy is named: the workflow and both of its entry points.
    assert ".github/workflows/plugin-release.yml" in result.stderr
    assert "manual arm" in result.stderr


def test_green_when_the_retired_version_is_released(tmp_path, github):
    repo, base_sha = _repo(tmp_path, "1.4.3", "1.4.4")
    world = World()
    _released(world, "1.4.3", base_sha)
    result = _run(repo, github(world))
    assert result.returncode == 0, result.stdout + result.stderr
    assert "1 of 1 retired plugin version(s) released (delvewright--v1.4.3)" in result.stdout


def test_a_draft_is_not_a_release(tmp_path, github):
    repo, base_sha = _repo(tmp_path, "1.4.3", "1.4.4")
    world = World()
    _released(world, "1.4.3", base_sha, draft=True)
    result = _run(repo, github(world))
    assert result.returncode == 1, result.stdout + result.stderr
    assert "(it is draft)" in result.stderr


def test_a_release_missing_its_archive_is_not_one(tmp_path, github):
    repo, base_sha = _repo(tmp_path, "1.4.3", "1.4.4")
    world = World()
    _released(world, "1.4.3", base_sha, assets=["SHA256SUMS"])
    result = _run(repo, github(world))
    assert result.returncode == 1, result.stdout + result.stderr
    assert "lacks 1 of 2 asset(s): delvewright-plugin-1.4.3.zip" in result.stderr


def test_a_tag_on_a_commit_carrying_another_version_is_not_one(tmp_path, github):
    repo, base_sha = _repo(tmp_path, "1.4.3", "1.4.4")
    world = World()
    _released(world, "1.4.3", base_sha, carries="1.4.2")
    result = _run(repo, github(world))
    assert result.returncode == 1, result.stdout + result.stderr
    assert "states 1.4.2, not 1.4.3" in result.stderr


def test_a_change_that_does_not_move_the_version_retires_nothing(tmp_path, github):
    repo, _ = _repo(tmp_path, "1.4.3", "1.4.3")
    result = _run(repo, github(World()))
    assert result.returncode == 0, result.stdout + result.stderr
    assert "0 plugin version(s) retired by this change" in result.stdout
    assert "states delvewright 1.4.3" in result.stdout


def test_a_branch_behind_the_base_retires_nothing(tmp_path, github):
    repo, _ = _repo(tmp_path, "1.4.4", "1.4.4")
    _write(repo, "1.4.3")
    result = _run(repo, github(World()))
    assert result.returncode == 0, result.stdout + result.stderr
    assert "BEHIND" in result.stdout


def test_exit_2_when_the_lookup_itself_is_unbound(tmp_path, github):
    repo, base_sha = _repo(tmp_path, "1.4.3", "1.4.4")
    world = World(bind=False)
    _released(world, "1.4.3", base_sha)
    result = _run(repo, github(world))
    assert result.returncode == 2, result.stdout + result.stderr
    assert "v1.5.0" in result.stderr


def test_exit_2_when_the_base_does_not_resolve(tmp_path, github):
    repo, _ = _repo(tmp_path, "1.4.3", "1.4.4")
    env = {**os.environ, "DW_GITHUB_API": github(World()), "no_proxy": "*", "NO_PROXY": "*"}
    result = subprocess.run(
        ["python3", str(CHECKER), "--repo", str(repo), "--base", "origin/nonexistent"],
        capture_output=True, text=True, env=env,
    )
    assert result.returncode == 2, result.stdout + result.stderr
