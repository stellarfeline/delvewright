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
import shutil
import subprocess
import tempfile

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


def tmp_roots(env: dict[str, str]) -> list[pathlib.Path]:
    """The same roots `worktree-new.sh` itself refuses, resolved the same way.

    Unconditional `/tmp` and `/private/tmp`, plus `$TMPDIR` from the given
    environment when it is set — mirrors `TMP_ROOTS` in the script so a test
    can check its own precondition against the actual refusal surface rather
    than against an assumption about the platform.
    """
    roots = [pathlib.Path("/tmp").resolve(), pathlib.Path("/private/tmp").resolve()]
    tmpdir = env.get("TMPDIR")
    if tmpdir:
        roots.append(pathlib.Path(tmpdir.rstrip("/")).resolve())
    return roots


def assert_outside_tmp_roots(candidate: pathlib.Path, env: dict[str, str]) -> None:
    resolved = candidate.resolve()
    for root in tmp_roots(env):
        if resolved == root or root in resolved.parents:
            pytest.fail(
                f"test precondition failed: candidate {resolved} is under "
                f"tmp root {root} — this test would prove nothing on this "
                "platform, not that the tool refused anything"
            )


def test_a_project_owned_directory_is_not_refused(tool_copy):
    # pytest's own `tmp_path` fixture is rooted at `tempfile.gettempdir()`,
    # which IS `$TMPDIR` on macOS and is `/tmp/...` on the Linux CI runner —
    # under a refused root either way, so it cannot stand in for "a directory
    # the project owns" here. `$HOME` is neither `/tmp`, `/private/tmp` nor
    # (barring a perverse environment) `$TMPDIR`, on macOS, on the Linux CI
    # runner and on the GitHub-hosted runner alike, so a directory made under
    # it with `tempfile.mkdtemp` — removed again in `finally`, since nothing
    # here cleans up a `$HOME` child the way pytest's `tmp_path` fixture would
    # — is genuinely outside every root the tool refuses. Assert that
    # precondition instead of trusting it: this test reds by that assertion,
    # naming the offending root, if some environment ever makes it false,
    # rather than passing or failing by accident of `tempfile.gettempdir()`.
    env = dict(os.environ)
    owned_root = tempfile.mkdtemp(dir=str(pathlib.Path.home()))
    try:
        candidate = pathlib.Path(owned_root) / "owned" / "wt"
        assert_outside_tmp_roots(candidate, env)
        p = run_refusal_probe(tool_copy, str(candidate))
        assert "refused" not in p.stderr
        assert "session-scoped" not in p.stderr
        # It got past the refusal and reached a real `git` call, which fails on
        # this copy's non-repository parent rather than on anything this test set
        # up — proof the path was accepted rather than rejected by some other
        # short-circuit that would also print neither of the two strings above.
        assert "not a git repository" in p.stderr
        assert not candidate.exists()
    finally:
        shutil.rmtree(owned_root, ignore_errors=True)
