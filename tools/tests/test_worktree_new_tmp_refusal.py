"""`tools/worktree-new.sh` refuses a `--path` under a session-scoped tmp root.

A worktree is where a dispatched worker keeps everything it has not pushed
yet, sometimes for hours. A `--path` under `/tmp`, its macOS alias
`/private/tmp` (which is where the Claude Code harness's own per-session
scratch root, `/private/tmp/claude-501/<project>/<session>/…`, actually lives),
or `$TMPDIR` is deleted by the OS or the harness the moment the owning process
exits — and a live round's worktree, with everything unpushed in it, was lost
this way once. Before the fix below, `worktree-new.sh` happily created a
worktree there and exited 0.

The refusal fires before this script ever runs `git`, so these tests exercise
it directly, with no repository fixture required — a bare `bash` and `python3`
(the script's own realpath helper) are the whole dependency.

## Binding count

4 tests: three tmp-root shapes each proven refused (exit 1, no directory
created), one proving a directory the project owns is unaffected (exit
continues past the refusal — checked by the ABSENCE of the refusal's own
sentence in stderr, since actually creating a worktree needs a real repo).
Run against the version that preceded this fix, all three refusal tests go
red: the script printed a normal "== worktree-new" banner and exited 0.
"""

from __future__ import annotations

import os
import pathlib
import subprocess

import pytest

TOOL = pathlib.Path(__file__).resolve().parents[1] / "worktree-new.sh"


def run_refusal_probe(path_str: str, *, env_overrides: dict[str, str] | None = None):
    env = dict(os.environ)
    if env_overrides:
        env.update(env_overrides)
    return subprocess.run(
        ["bash", str(TOOL), "--path", path_str, "--branch", "throwaway/does-not-matter"],
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
def test_refuses_under_tmp_roots(path_str, tmp_path):
    p = run_refusal_probe(path_str)
    assert p.returncode == 1, p.stdout + p.stderr
    assert "refused" in p.stderr
    assert "session-scoped" in p.stderr
    assert not pathlib.Path(path_str).exists()


def test_refuses_under_tmpdir_env(tmp_path):
    # A stand-in TMPDIR the test owns and can safely assert against, in case the
    # ambient TMPDIR is not writable by this process or does not exist.
    fake_tmpdir = tmp_path / "session-scratch"
    fake_tmpdir.mkdir()
    candidate = fake_tmpdir / "wt"
    p = run_refusal_probe(str(candidate), env_overrides={"TMPDIR": str(fake_tmpdir) + "/"})
    assert p.returncode == 1, p.stdout + p.stderr
    assert "refused" in p.stderr
    assert not candidate.exists()


def test_a_project_owned_directory_is_not_refused(tmp_path):
    # pytest's own `tmp_path` fixture is rooted at `tempfile.gettempdir()`,
    # which on this machine (and in CI) IS `$TMPDIR` — so without overriding
    # the environment, "a directory pytest made" and "a directory under
    # $TMPDIR" are the same claim, and this test would prove nothing. Point
    # $TMPDIR somewhere else so the only ambient roots left (/tmp,
    # /private/tmp) are the ones the fixture directory is genuinely outside of.
    candidate = tmp_path / "owned" / "wt"
    p = run_refusal_probe(
        str(candidate), env_overrides={"TMPDIR": str(tmp_path / "unrelated-tmpdir") + "/"}
    )
    assert "refused" not in p.stderr
    assert "session-scoped" not in p.stderr
