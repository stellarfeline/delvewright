"""`tools/worktree-new.sh` refuses a `--path` under a session-scoped tmp root.

A worktree is where a dispatched worker keeps everything it has not pushed
yet, sometimes for hours. A `--path` under `/tmp`, its macOS alias
`/private/tmp` (which is where the Claude Code harness's own per-session
scratch root, `/private/tmp/claude-501/<project>/<session>/…`, actually lives),
or `$TMPDIR` is deleted by the OS or the harness the moment the owning process
exits — and a live round's worktree, with everything unpushed in it, was lost
this way once. Before the fix below, `worktree-new.sh` happily created a
worktree there and exited 0.

The refusal fires before this script ever runs `git`, so a test whose path IS
refused needs no repository fixture. A test proving a legitimate path is NOT
refused is a different matter: the script proceeds past the check to a real
`git worktree add`, and `$HERE` is derived from `${BASH_SOURCE[0]}`, i.e. from
wherever the script FILE lives — which, run in place, is this checkout. The
first version of this test ran the real script in place and that "not
refused" case did exactly what it says: it created a real worktree and a real
branch in the repository these tests live in, which pytest's `tmp_path`
cleanup does not undo (git worktree metadata lives in `.git/`, not in the
worktree directory itself) and which was only found by a stray `git worktree
list` afterwards. So every invocation here runs a COPY of the script placed
under a directory that is not a git repository at all: the refusal path is
identical either way, and a path that gets past it fails on "not a git
repository" instead of touching anything real.

## Binding count

4 tests: three tmp-root shapes each proven refused (exit 1, no directory
created), one proving a directory the project owns is unaffected (the
refusal's own sentence is absent from stderr; the run then fails for the
unrelated reason that the copied script's directory is not a git repository,
proving it reached past the check rather than being silently short-circuited
by some other refusal). Run against the version that preceded this fix, all
three refusal tests go red: the script printed a normal "== worktree-new"
banner and exited 0.
"""

from __future__ import annotations

import os
import pathlib
import subprocess

import pytest

TOOL = pathlib.Path(__file__).resolve().parents[1] / "worktree-new.sh"


@pytest.fixture
def tool_copy(tmp_path):
    """A copy of the script under a directory that is NOT a git repository.

    `$HERE` in the script resolves to the copy's own parent-of-parent, never to
    this checkout, so nothing a test does here can register a worktree or a
    branch against the repository these tests live in.
    """
    root = tmp_path / "toolcopy"
    (root / "tools").mkdir(parents=True)
    copy = root / "tools" / "worktree-new.sh"
    copy.write_bytes(TOOL.read_bytes())
    copy.chmod(0o755)
    return copy


def run_refusal_probe(
    tool_path: pathlib.Path, path_str: str, *, env_overrides: dict[str, str] | None = None
):
    env = dict(os.environ)
    if env_overrides:
        env.update(env_overrides)
    return subprocess.run(
        ["bash", str(tool_path), "--path", path_str, "--branch", "throwaway/does-not-matter"],
        capture_output=True,
        text=True,
        env=env,
    )


@pytest.mark.parametrize(
    "path_str",
    [
        "/tmp/some-session/wt",
        "/private/tmp/claude-501/some-project/some-session/wt",
    ],
)
def test_refuses_under_tmp_roots(path_str, tool_copy):
    p = run_refusal_probe(tool_copy, path_str)
    assert p.returncode == 1, p.stdout + p.stderr
    assert "refused" in p.stderr
    assert "session-scoped" in p.stderr
    assert not pathlib.Path(path_str).exists()


def test_refuses_under_tmpdir_env(tool_copy, tmp_path):
    # A stand-in TMPDIR the test owns and can safely assert against, in case the
    # ambient TMPDIR is not writable by this process or does not exist.
    fake_tmpdir = tmp_path / "session-scratch"
    fake_tmpdir.mkdir()
    candidate = fake_tmpdir / "wt"
    p = run_refusal_probe(tool_copy, str(candidate), env_overrides={"TMPDIR": str(fake_tmpdir) + "/"})
    assert p.returncode == 1, p.stdout + p.stderr
    assert "refused" in p.stderr
    assert not candidate.exists()


def test_a_project_owned_directory_is_not_refused(tool_copy, tmp_path):
    # pytest's own `tmp_path` fixture is rooted at `tempfile.gettempdir()`,
    # which on this machine (and in CI) IS `$TMPDIR` — so without overriding
    # the environment, "a directory pytest made" and "a directory under
    # $TMPDIR" are the same claim, and this test would prove nothing. Point
    # $TMPDIR somewhere else so the only ambient roots left (/tmp,
    # /private/tmp) are the ones the fixture directory is genuinely outside of.
    candidate = tmp_path / "owned" / "wt"
    p = run_refusal_probe(
        tool_copy,
        str(candidate),
        env_overrides={"TMPDIR": str(tmp_path / "unrelated-tmpdir") + "/"},
    )
    assert "refused" not in p.stderr
    assert "session-scoped" not in p.stderr
    # It got past the refusal and reached a real `git` call, which fails on
    # this copy's non-repository parent rather than on anything this test set
    # up — proof the path was accepted rather than rejected by some other
    # short-circuit that would also print neither of the two strings above.
    assert "not a git repository" in p.stderr
    assert not candidate.exists()
