"""`validation/mutex.sh`'s lock directory is a real path on every platform.

`DW_MUTEX_DIR` defaults to `/private/tmp/delvewright-validation.lock.d`, a
macOS-only path: on Linux `mkdir` against it fails because the PARENT is not
there — nobody holds anything — and an acquire loop that cannot tell that
apart from "someone else holds the lock" reports it as held, refusing to
start `playtest-server.sh up` on a host where the lock was never anyone's to
begin with.

Three things are bound here: the default is chosen by platform (a function
of the `uname -s` string, not a hardcoded literal, so a test can drive both
branches without a second host); a failed acquisition says WHY — "cannot
create" is a materially different fact from "someone is holding the lock",
tested against the PARENT directory rather than the lock directory itself, so
only a parent that is missing or unwritable ever reads as "cannot create";
and a lock directory that vanishes between a failed `mkdir` and either check
that follows it — a real holder releasing in that instant — retries instead
of reporting an empty holder as though something is held.
"""

import os
import pathlib
import subprocess

import pytest

ROOT = pathlib.Path(__file__).resolve().parents[2]
MUTEX = ROOT / "validation" / "mutex.sh"


def run_bash(code: str, env: dict) -> subprocess.CompletedProcess:
    full_env = {"PATH": "/usr/bin:/bin:/usr/sbin:/sbin", **env}
    return subprocess.run(
        ["bash", "-c", code], cwd=ROOT, env=full_env, capture_output=True, text=True
    )


# ---------------------------------------------------------------------------
# The default is a function of the platform, not a macOS-only literal
# ---------------------------------------------------------------------------


def test_darwin_keeps_the_private_tmp_default(tmp_path):
    result = run_bash(
        f'source "{MUTEX}"; dw_mutex_default_dir Darwin', env={"HOME": str(tmp_path)}
    )
    assert result.returncode == 0, result.stderr
    assert result.stdout.strip() == "/private/tmp/delvewright-validation.lock.d"


@pytest.mark.parametrize("uname_s", ["Linux", "FreeBSD", "SunOS"])
def test_a_non_macos_uname_gets_a_path_that_exists_there(tmp_path, uname_s):
    """`/private/tmp` is a macOS-only alias; everywhere else `/tmp` is the one
    directory guaranteed to exist."""
    result = run_bash(
        f'source "{MUTEX}"; dw_mutex_default_dir {uname_s}', env={"HOME": str(tmp_path)}
    )
    assert result.returncode == 0, result.stderr
    assert result.stdout.strip() == "/tmp/delvewright-validation.lock.d"


def test_an_explicit_dw_mutex_dir_still_overrides_the_default(tmp_path):
    """The platform default must never shadow an explicit override — every
    other test in this suite (and CI) relies on pointing this away from the
    real, sacred lock."""
    custom = tmp_path / "custom-lock.d"
    result = run_bash(
        f'source "{MUTEX}"; echo "$DW_MUTEX_DIR"',
        env={"HOME": str(tmp_path), "DW_MUTEX_DIR": str(custom)},
    )
    assert result.returncode == 0, result.stderr
    assert result.stdout.strip() == str(custom)


# ---------------------------------------------------------------------------
# A failed acquisition says WHY: "cannot create" vs "someone holds it"
# ---------------------------------------------------------------------------


def acquire(tmp_path, lock_dir, held_by=None):
    if held_by is not None:
        lock_dir.mkdir(parents=True)
        (lock_dir / "HOLDER").write_text(f"{held_by} 1234\n", encoding="utf-8")
    return run_bash(
        f'source "{MUTEX}"; dw_mutex_acquire test-holder 0',
        env={"HOME": str(tmp_path), "DW_MUTEX_DIR": str(lock_dir)},
    )


def test_a_missing_parent_directory_is_reported_as_cannot_create_not_held(tmp_path):
    # The parent ("no-such-parent") is never created, so `mkdir` fails with
    # ENOENT — nobody holds this lock, mkdir itself could not run.
    lock_dir = tmp_path / "no-such-parent" / "lock.d"
    result = acquire(tmp_path, lock_dir)
    assert result.returncode != 0
    assert "cannot create" in result.stderr, result.stderr
    assert "held by" not in result.stderr, (
        "a missing parent directory was reported as though someone held the "
        f"lock:\n{result.stderr}"
    )


def test_a_real_holder_is_still_reported_as_held_not_as_cannot_create(tmp_path):
    lock_dir = tmp_path / "lock.d"
    result = acquire(tmp_path, lock_dir, held_by="someone-else")
    assert result.returncode != 0
    assert "held by 'someone-else'" in result.stderr, result.stderr
    assert "cannot create" not in result.stderr, result.stderr


def test_the_sacred_owner_session_is_still_never_stolen_or_waited_on(tmp_path):
    lock_dir = tmp_path / "lock.d"
    result = acquire(tmp_path, lock_dir, held_by="owner-play-session")
    assert result.returncode != 0
    assert "owner-play-session" in result.stderr
    assert "refusing to wait or steal" in result.stderr, result.stderr


# ---------------------------------------------------------------------------
# The race: a holder can release BETWEEN a failed `mkdir` and the check that
# decides what the failure meant, and a release cannot touch the PARENT
# directory. Testing the lock directory itself for that decision reads a
# holder's release as "nobody can ever hold this lock" instead of "someone
# just did."
# ---------------------------------------------------------------------------


def stub_mkdir_failing_n_times(tmp_path, n: int) -> pathlib.Path:
    """A `mkdir` that fails its first `n` calls (creating nothing) and then
    behaves normally — standing in for a real `mkdir` that failed because a
    holder was there for an instant and is gone before the next check."""
    bindir = tmp_path / "fakebin"
    bindir.mkdir(exist_ok=True)
    counter = tmp_path / "mkdir-calls.txt"
    counter.write_text("0", encoding="utf-8")
    stub = bindir / "mkdir"
    stub.write_text(
        "#!/usr/bin/env python3\n"
        "import os, sys, pathlib\n"
        f"counter = pathlib.Path({str(counter)!r})\n"
        "count = int(counter.read_text()) + 1\n"
        "counter.write_text(str(count))\n"
        "target = sys.argv[-1]\n"
        f"if count <= {n}:\n"
        "    sys.stderr.write(\"mkdir: cannot create directory '\" + target + \"': File exists\\n\")\n"
        "    sys.exit(1)\n"
        "os.mkdir(target)\n"
        "sys.exit(0)\n",
        encoding="utf-8",
    )
    stub.chmod(0o755)
    return bindir


def test_a_mkdir_that_fails_once_with_the_lock_dir_absent_retries_and_succeeds(tmp_path):
    """The exact race: `mkdir` reports failure, and by the time the next line
    checks, the lock directory is not there and its parent is fine — a real
    holder that let go in between, not a path nobody could ever create."""
    parent = tmp_path / "parent"
    parent.mkdir()
    lock_dir = parent / "lock.d"
    bindir = stub_mkdir_failing_n_times(tmp_path, n=1)
    env = {
        "PATH": f"{bindir}{os.pathsep}/usr/bin:/bin:/usr/sbin:/sbin",
        "HOME": str(tmp_path),
        "DW_MUTEX_DIR": str(lock_dir),
    }
    result = run_bash('source "%s"; dw_mutex_acquire test-holder 0' % MUTEX, env=env)
    assert result.returncode == 0, result.stderr
    assert "cannot create" not in result.stderr, (
        f"a transient mkdir failure with the lock dir absent was reported as "
        f"'cannot create' instead of retried:\n{result.stderr}"
    )
    assert "held by" not in result.stderr, result.stderr
    assert lock_dir.is_dir(), "the retry never actually acquired the lock"
