r"""Guard: a plugin release is refused unless it records exactly what `main` delivered (ADR-0028 §3).

`tools/check-plugin-release-identity.py` over scratch repositories shaped like
`main`'s first-parent history. The control is two commits carrying 1.4.3 with
one plugin-root tree (a change outside the plugin root between them); each
perturbation moves one thing toward a release that would record something other
than what a creator received.
"""

from __future__ import annotations

import json
import subprocess
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
CHECKER = REPO / "tools" / "check-plugin-release-identity.py"
ROOT = ".claude/skills/delvewright"
MANIFEST = f"{ROOT}/.claude-plugin/plugin.json"


class Repo:
    def __init__(self, path: Path) -> None:
        self.path = path
        path.mkdir(parents=True)
        self.git("init", "-q", "-b", "main")
        self.git("config", "user.email", "t@example.com")
        self.git("config", "user.name", "t")

    def git(self, *args: str) -> str:
        return subprocess.run(
            ["git", "-C", str(self.path), *args], check=True, capture_output=True, text=True
        ).stdout.strip()

    def commit(self, message: str, *, version: str | None = None, page: str | None = None, other: str | None = None) -> str:
        if version is not None:
            p = self.path / MANIFEST
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(json.dumps({"name": "delvewright", "version": version}) + "\n", encoding="utf-8")
        if page is not None:
            p = self.path / ROOT / "skills" / "new-delve" / "SKILL.md"
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(page, encoding="utf-8")
        if other is not None:
            (self.path / "README.md").write_text(other, encoding="utf-8")
        self.git("add", "-A")
        self.git("commit", "-qm", message, "--allow-empty")
        return self.git("rev-parse", "HEAD")

    def publish_main(self) -> None:
        self.git("update-ref", "refs/remotes/origin/main", "main")


def _run(repo: Repo, *args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["python3", str(CHECKER), "--repo", str(repo.path), *args], capture_output=True, text=True
    )


def _history(tmp_path: Path) -> tuple[Repo, dict[str, str]]:
    r = Repo(tmp_path / "repo")
    c = {}
    c["a"] = r.commit("1.4.2", version="1.4.2", page="one\n", other="a\n")
    c["b"] = r.commit("1.4.3", version="1.4.3", page="two\n")
    c["c"] = r.commit("outside the plugin root", other="c\n")
    c["d"] = r.commit("1.4.4", version="1.4.4", page="three\n")
    r.publish_main()
    return r, c


def test_green_on_either_commit_carrying_the_version(tmp_path):
    r, c = _history(tmp_path)
    for commit in (c["b"], c["c"]):
        result = _run(r, "--commit", commit, "--tag", "delvewright--v1.4.3")
        assert result.returncode == 0, result.stdout + result.stderr
        assert "2 of 4 first-parent commit(s) on origin/main carry 1.4.3, 1 distinct tree(s)" in result.stdout


def test_the_dispatch_arm_derives_the_tag(tmp_path):
    r, c = _history(tmp_path)
    out = tmp_path / "out"
    result = _run(r, "--commit", c["d"], "--github-output", str(out))
    assert result.returncode == 0, result.stdout + result.stderr
    lines = dict(line.split("=", 1) for line in out.read_text().splitlines())
    assert lines["tag"] == "delvewright--v1.4.4"
    assert lines["version"] == "1.4.4"
    assert lines["commit"] == c["d"]
    assert lines["tree"] == r.git("rev-parse", f"{c['d']}:{ROOT}")


def test_red_on_a_bare_v_tag(tmp_path):
    r, c = _history(tmp_path)
    result = _run(r, "--commit", c["b"], "--tag", "v1.4.3")
    assert result.returncode == 1, result.stdout + result.stderr
    assert "pre-grammar" in result.stderr


def test_red_on_a_tag_naming_another_version(tmp_path):
    r, c = _history(tmp_path)
    result = _run(r, "--commit", c["d"], "--tag", "delvewright--v1.4.3")
    assert result.returncode == 1, result.stdout + result.stderr
    assert "must be 'delvewright--v1.4.4'" in result.stderr


def test_red_when_one_version_carries_two_plugin_roots(tmp_path):
    """The page edited under an unchanged version — what the version-bump rule
    forbids, and what would make one archive stand for two deliveries."""
    r = Repo(tmp_path / "repo")
    r.commit("1.4.3", version="1.4.3", page="two\n")
    edited = r.commit("page edited, no bump", page="two, edited\n")
    r.publish_main()
    result = _run(r, "--commit", edited, "--tag", "delvewright--v1.4.3")
    assert result.returncode == 1, result.stdout + result.stderr
    assert "2 of 2 first-parent commit(s) on origin/main carry 1.4.3, 2 distinct tree(s)" in result.stderr


def test_red_off_main(tmp_path):
    r, c = _history(tmp_path)
    r.git("checkout", "-q", "-b", "side", c["d"])
    side = r.commit("1.4.5 on a branch", version="1.4.5", page="four\n")
    result = _run(r, "--commit", side, "--tag", "delvewright--v1.4.5")
    assert result.returncode == 1, result.stdout + result.stderr
    assert "is not an ancestor of origin/main" in result.stderr


def test_red_on_a_second_parent_commit(tmp_path):
    """Reachable from `main`, never served by it: the branch commit behind a merge."""
    r, c = _history(tmp_path)
    r.git("checkout", "-q", "-b", "side", c["d"])
    side = r.commit("1.4.5 on a branch", version="1.4.5", page="four\n")
    r.git("checkout", "-q", "main")
    r.commit("main moves outside the plugin root", other="main\n")
    r.git("merge", "-q", "--no-ff", "-m", "merge side", "side")
    r.publish_main()
    result = _run(r, "--commit", side, "--tag", "delvewright--v1.4.5")
    assert result.returncode == 1, result.stdout + result.stderr
    assert "through a merge's second parent" in result.stderr
    merge = r.git("rev-parse", "main")
    ok = _run(r, "--commit", merge, "--tag", "delvewright--v1.4.5")
    assert ok.returncode == 0, ok.stdout + ok.stderr
