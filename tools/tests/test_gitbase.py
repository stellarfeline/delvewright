r"""Guards for `tools/lib/gitbase.py` — the one remedy two numbering gates print.

The defect: both gates printed CI's own fetch line to whoever ran them.

    git fetch --no-tags --depth=1 origin main:refs/remotes/origin/main

In CI that is right — the checkout is disposable and already shallow, and one
commit is cheaper than a branch's whole history. Run in a developer's full clone
it converts the working repository into a shallow one, and the boundary lives in
the object store, so the main checkout and every linked worktree go with it.

Then git stops erroring and starts ANSWERING WRONG. On a 405-commit history with
a feature branch 1 ahead of `origin/main` and 5 behind:

    git merge origin/main               -> refusing to merge unrelated histories
    git merge-base HEAD origin/main     -> (empty)
    rev-list --count origin/main..HEAD  -> 401   (truth: 1)
    rev-list --count HEAD..origin/main  -> 1     (truth: 5)

The first is loud. The rest are the hazard: nothing re-checks them, and "401
ahead" is a number someone resets or force-pushes on.

The last test here is the one that matters. Asserting the string does not contain
`--depth` only pins today's wording; it cannot notice a future remedy that is
differently worded and equally destructive. So the printed command is EXECUTED
against a throwaway clone and the repository is re-examined afterwards — the
property under test is "this instruction does not damage the thing it is helping
with", and it is checked by carrying the instruction out.
"""

from __future__ import annotations

import importlib.util
import shlex
import subprocess
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[2]
LIB = REPO / "tools" / "lib" / "gitbase.py"


@pytest.fixture
def gitbase():
    spec = importlib.util.spec_from_file_location("gitbase", LIB)
    mod = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(mod)
    return mod


def git(root: Path, *args: str, check: bool = True) -> str:
    result = subprocess.run(
        ["git", *args], cwd=root, capture_output=True, text=True,
        env={"GIT_CONFIG_GLOBAL": "/dev/null", "GIT_CONFIG_SYSTEM": "/dev/null",
             "GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@t",
             "GIT_COMMITTER_NAME": "t", "GIT_COMMITTER_EMAIL": "t@t",
             "PATH": "/usr/bin:/bin:/usr/local/bin:/opt/homebrew/bin", "HOME": str(root)},
    )
    if check:
        assert result.returncode == 0, f"git {' '.join(args)}: {result.stderr}"
    return result.stdout


def build_origin(tmp_path: Path, commits: int = 12) -> Path:
    """A bare remote carrying a real multi-commit history."""
    origin = tmp_path / "origin.git"
    seed = tmp_path / "seed"
    git(tmp_path, "init", "-q", "--bare", str(origin), "--initial-branch=main")
    seed.mkdir()
    git(seed, "init", "-q", ".", "--initial-branch=main")
    for i in range(commits):
        (seed / "f.txt").write_text(str(i), encoding="utf-8")
        git(seed, "add", "f.txt")
        git(seed, "commit", "-q", "-m", f"c{i}")
    git(seed, "remote", "add", "origin", str(origin))
    git(seed, "push", "-q", "origin", "main")
    return origin


def test_a_full_clone_is_not_told_to_shallow_itself(gitbase, tmp_path):
    origin = build_origin(tmp_path)
    clone = tmp_path / "full"
    git(tmp_path, "clone", "-q", f"file://{origin}", str(clone))

    assert gitbase.is_shallow(clone) is False
    remedy = gitbase.fetch_remedy(clone, "origin/main")
    assert "git fetch --no-tags origin main:refs/remotes/origin/main" in remedy
    assert "Do NOT add `--depth=1`" in remedy


def test_an_already_shallow_clone_is_told_to_fetch_shallowly(gitbase, tmp_path):
    origin = build_origin(tmp_path)
    clone = tmp_path / "shal"
    git(tmp_path, "clone", "-q", "--depth=1", "--branch", "main", f"file://{origin}", str(clone))

    assert gitbase.is_shallow(clone) is True
    remedy = gitbase.fetch_remedy(clone, "origin/main")
    assert "git fetch --no-tags --depth=1 origin main:refs/remotes/origin/main" in remedy


def test_a_base_that_is_not_remote_slash_branch_gets_prose_not_a_wrong_command(
    gitbase, tmp_path
):
    """A sha, a tag or a local branch cannot be turned into a fetch that is
    certain to install the ref asked for — so it gets no command at all. Printing
    a plausible wrong one is the same class of defect as printing a damaging one."""
    origin = build_origin(tmp_path)
    clone = tmp_path / "full"
    git(tmp_path, "clone", "-q", f"file://{origin}", str(clone))

    for base in ("deadbeef", "v1.0.0", "refs/heads/main", "origin/feature/x"):
        remedy = gitbase.fetch_remedy(clone, base)
        assert "git fetch --no-tags origin" not in remedy, base
        assert "WITHOUT `--depth`" in remedy, base


def test_resolve_base_returns_the_sha_when_the_ref_is_there(gitbase, tmp_path):
    origin = build_origin(tmp_path)
    clone = tmp_path / "full"
    git(tmp_path, "clone", "-q", f"file://{origin}", str(clone))
    expected = git(clone, "rev-parse", "origin/main").strip()
    assert gitbase.resolve_base(clone, "origin/main", "t") == expected


def test_resolve_base_raises_with_the_tool_name_and_the_remedy(gitbase, tmp_path):
    origin = build_origin(tmp_path)
    clone = tmp_path / "full"
    git(tmp_path, "clone", "-q", f"file://{origin}", str(clone))
    git(clone, "update-ref", "-d", "refs/remotes/origin/main")

    with pytest.raises(gitbase.BaseUnresolved) as raised:
        gitbase.resolve_base(clone, "origin/main", "check-thing")
    message = raised.value.message
    assert message.startswith("check-thing: FAIL")
    assert "does not resolve to a commit" in message
    assert "git fetch --no-tags origin main:refs/remotes/origin/main" in message


def test_running_the_printed_command_leaves_a_full_clone_full(gitbase, tmp_path):
    """The load-bearing test: carry the instruction out and re-examine the repo.

    A string assertion pins wording; this pins the property. It also proves the
    remedy WORKS — the ref is installed and ancestry is still answerable — so the
    fix cannot degenerate into printing something harmless and useless.
    """
    origin = build_origin(tmp_path)
    clone = tmp_path / "full"
    git(tmp_path, "clone", "-q", f"file://{origin}", str(clone))
    # The exact state the remedy is printed in.
    git(clone, "update-ref", "-d", "refs/remotes/origin/main")
    git(clone, "checkout", "-q", "-b", "feature")
    (clone / "g.txt").write_text("feature", encoding="utf-8")
    git(clone, "add", "g.txt")
    git(clone, "commit", "-q", "-m", "feature work")

    remedy = gitbase.fetch_remedy(clone, "origin/main")
    command = next(
        line.strip() for line in remedy.splitlines() if line.strip().startswith("git fetch")
    )
    git(clone, *shlex.split(command)[1:])

    assert gitbase.is_shallow(clone) is False, "the printed remedy shallowed a full clone"
    # It did the job it was printed for.
    assert gitbase.resolve_base(clone, "origin/main", "t")
    # And ancestry still answers truthfully: 1 ahead, 0 behind.
    assert git(clone, "rev-list", "--count", "origin/main..HEAD").strip() == "1"
    assert git(clone, "rev-list", "--count", "HEAD..origin/main").strip() == "0"
    assert git(clone, "merge-base", "HEAD", "origin/main").strip()
