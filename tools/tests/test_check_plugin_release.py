"""`check-plugin-release.py`, perturbed toward each shape it refuses (ADR-0038 §4).

A scratch repository with one released engine and one released plugin at the
same commit, then a `main` commit that prepares the next plugin release and no
engine release — the case the record exists for. Each test makes that tree
wrong in exactly one way the tool is the only thing standing against.
"""

from __future__ import annotations

import importlib.util
import json
import pathlib
import subprocess

import pytest

REPO = pathlib.Path(__file__).resolve().parents[2]
TOOL = REPO / "tools" / "ci" / "check-plugin-release.py"

MANIFEST = ".claude/skills/delvewright/.claude-plugin/plugin.json"
PIN = ".claude/skills/delvewright/skills/new-delve/versions.toml"
PAGE = ".claude/skills/delvewright/skills/new-delve/SKILL.md"
MARKET = ".claude-plugin/marketplace.json"


@pytest.fixture(scope="module")
def mod():
    spec = importlib.util.spec_from_file_location("check_plugin_release", TOOL)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _git(repo: pathlib.Path, *args: str) -> str:
    return subprocess.run(["git", "-C", str(repo), *args], check=True, capture_output=True, text=True).stdout.strip()


def _write(repo: pathlib.Path, rel: str, text: str) -> None:
    path = repo / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def _plugin(repo: pathlib.Path, version: str, entry_ref: str, engine: str) -> None:
    _write(repo, MANIFEST, json.dumps({"name": "delvewright", "version": version}) + "\n")
    _write(repo, PIN, f'[engine]\nrepo = "o/r"\nref = "{engine}"\n')
    _write(
        repo,
        MARKET,
        json.dumps(
            {
                "name": "delvewright",
                "owner": {"name": "o"},
                "plugins": [
                    {
                        "name": "delvewright",
                        "source": {"source": "git-subdir", "url": "o/r", "path": ".claude/skills/delvewright", "ref": entry_ref},
                    }
                ],
            }
        )
        + "\n",
    )


@pytest.fixture
def repo(tmp_path):
    root = tmp_path / "engine"
    root.mkdir()
    _git(root, "init", "-q", "-b", "main")
    _write(root, "versions.toml", '[engine]\nversion = "1.15.0"\n')
    _plugin(root, "1.15.0", "delvewright--v1.15.0", "delvec--v1.15.0")
    _write(root, PAGE, "---\nname: new-delve\n---\n# page\n")
    _git(root, "add", "-A")
    _git(root, "commit", "-q", "-m", "release: engine and plugin 1.15.0")
    _git(root, "tag", "-a", "delvec--v1.15.0", "-m", "delvec 1.15.0")
    _git(root, "tag", "-a", "delvewright--v1.15.0", "-m", "delvewright 1.15.0")
    _git(root, "tag", "-a", "delvewright--v1.11.0", "-m", "an older one, on the same commit for the fixture")
    # the pull request that prepares plugin 1.15.1 with no engine release
    _plugin(root, "1.15.1", "delvewright--v1.15.1", "delvec--v1.15.0")
    _write(root, PAGE, "---\nname: new-delve\n---\n# page\n\nA fix a creator needs.\n")
    _git(root, "add", "-A")
    _git(root, "commit", "-q", "-m", "fix(skill): a page fix, prepared as 1.15.1")
    return root


def run(mod, repo: pathlib.Path, *extra: str, output: pathlib.Path | None = None) -> tuple[int, str, str]:
    """The tool as the workflow runs it: its own process, exit code and streams."""
    argv = ["python3", str(TOOL), "--commit", "HEAD", "--repo", str(repo), "--main", "main", "--repository", "o/r", *extra]
    if output is not None:
        argv += ["--output", str(output)]
    result = subprocess.run(argv, capture_output=True, text=True)
    return result.returncode, result.stdout, result.stderr


def judge(mod, repo: pathlib.Path, online: bool) -> tuple[list[str], dict[str, str], list[str]]:
    """In-process, for the `--online` arms, whose remote is monkeypatched."""
    return mod.judge(repo, "HEAD", "main", "o/r", online)


# ------------------------------------------------------------- the tree as it stands --


def test_a_plugin_release_with_no_engine_release_passes(mod, repo, tmp_path):
    """The case the record exists for: the tree differs from the last plugin
    release in skill files only, and nothing asks for `delvec--v1.15.1`."""
    out_file = tmp_path / "out"
    code, out, err = run(mod, repo, output=out_file)
    assert code == 0, err
    assert "2 delvewright--v* tag(s) read" in out
    assert "1.15.1 is above the newest release, delvewright--v1.15.0" in out
    assert "delvec--v1.15.0 is a tag this checkout carries" in out
    assert "check-plugin-release: ok — delvewright 1.15.1 at" in out and "writes delvewright--v1.15.1" in out
    facts = dict(line.split("=", 1) for line in out_file.read_text().splitlines())
    assert facts["tag"] == "delvewright--v1.15.1"
    assert facts["version"] == "1.15.1"
    assert facts["engine"] == "delvec--v1.15.0"
    assert facts["sha"] == _git(repo, "rev-parse", "HEAD")


# ------------------------------------------------------------------- each refusal --


def test_a_commit_off_main_is_refused(mod, repo):
    _git(repo, "checkout", "-q", "-b", "side")
    _write(repo, PAGE, "---\nname: new-delve\n---\n# page\n\nnot reviewed\n")
    _git(repo, "add", "-A")
    _git(repo, "commit", "-q", "-m", "a side commit")
    code, _out, err = run(mod, repo)
    assert code == 1 and "is not on main" in err, err


def test_a_tag_already_at_the_commit_is_refused(mod, repo):
    _git(repo, "tag", "-a", "delvewright--v1.15.1", "-m", "already")
    code, _out, err = run(mod, repo)
    assert code == 1 and "delvewright--v1.15.1 already exists" in err, err


def test_the_released_version_again_is_refused(mod, repo):
    _plugin(repo, "1.15.0", "delvewright--v1.15.0", "delvec--v1.15.0")
    _git(repo, "add", "-A")
    _git(repo, "commit", "-q", "-m", "a page edit under the released version")
    code, _out, err = run(mod, repo)
    assert code == 1 and "delvewright--v1.15.0 already exists" in err, err
    assert "is not above the newest release" not in err  # one refusal per defect


def test_a_version_below_the_newest_release_is_refused(mod, repo):
    _plugin(repo, "1.14.9", "delvewright--v1.14.9", "delvec--v1.15.0")
    _git(repo, "add", "-A")
    _git(repo, "commit", "-q", "-m", "a version behind the line")
    code, _out, err = run(mod, repo)
    assert code == 1 and "1.14.9 is not above the newest release of the line, delvewright--v1.15.0" in err, err


def test_an_entry_naming_the_previous_release_is_refused(mod, repo):
    _plugin(repo, "1.15.1", "delvewright--v1.15.0", "delvec--v1.15.0")
    _git(repo, "add", "-A")
    _git(repo, "commit", "-q", "-m", "the entry was not moved")
    code, _out, err = run(mod, repo)
    assert code == 1 and "names 'delvewright--v1.15.0', and the tag this release writes is delvewright--v1.15.1" in err, err


def test_an_unborn_engine_pin_is_refused(mod, repo):
    _plugin(repo, "1.15.1", "delvewright--v1.15.1", "delvec--v1.16.0")
    _git(repo, "add", "-A")
    _git(repo, "commit", "-q", "-m", "pins an engine nobody released")
    code, _out, err = run(mod, repo)
    assert code == 1 and "pins delvec--v1.16.0, which this checkout has no tag for" in err, err


def test_a_version_outside_the_grammar_cannot_be_judged(mod, repo):
    _write(repo, MANIFEST, json.dumps({"name": "delvewright", "version": "1.15"}) + "\n")
    _git(repo, "add", "-A")
    _git(repo, "commit", "-q", "-m", "a version that names no tag")
    code, _out, err = run(mod, repo)
    assert code == 2 and "not a strict" in err, err


def test_a_checkout_with_no_plugin_tag_cannot_judge(mod, repo):
    for tag in ("delvewright--v1.15.0", "delvewright--v1.11.0"):
        _git(repo, "tag", "-d", tag)
    code, _out, err = run(mod, repo)
    assert code == 2 and "carries no `delvewright--v*` tag" in err, err


# ------------------------------------------------------------------------ --online --


def test_online_passes_when_the_tag_is_unborn_and_the_engine_is_published(mod, repo, monkeypatch):
    monkeypatch.setattr(mod.github_releases, "tag_commit", lambda _repo, _tag: None)
    monkeypatch.setattr(mod.github_releases, "state", lambda _repo, _tag: "published")
    refusals, facts, said = judge(mod, repo, online=True)
    assert refusals == [], refusals
    assert "o/r has no delvewright--v1.15.1" in said
    assert "delvec--v1.15.0 is published on o/r" in said
    assert facts["tag"] == "delvewright--v1.15.1"


def test_online_refuses_a_tag_the_remote_already_has(mod, repo, monkeypatch):
    monkeypatch.setattr(mod.github_releases, "tag_commit", lambda _repo, _tag: "a" * 40)
    monkeypatch.setattr(mod.github_releases, "state", lambda _repo, _tag: "published")
    refusals, _facts, _said = judge(mod, repo, online=True)
    assert any("o/r already carries delvewright--v1.15.1" in r for r in refusals), refusals


@pytest.mark.parametrize("state", ["draft", "absent"])
def test_online_refuses_an_engine_whose_release_is_not_published(mod, repo, monkeypatch, state):
    """The page pins an engine the remote tags but nobody can download: the
    plugin is refused before its tag, since Init would fall to the source
    build on every platform."""
    monkeypatch.setattr(mod.github_releases, "tag_commit", lambda _repo, _tag: None)
    monkeypatch.setattr(mod.github_releases, "state", lambda _repo, _tag: state)
    refusals, _facts, _said = judge(mod, repo, online=True)
    assert any(f"whose Release on o/r is {state}" in r for r in refusals), refusals
