"""`validation/mutex.sh`'s lock directory is a macOS path on every platform.

The defect (ENGINE-LIMITS.md B2, a creator's report from a WSL2 Linux host):
`DW_MUTEX_DIR` defaulted to `/private/tmp/delvewright-validation.lock.d`, which
exists only on macOS. On Linux `mkdir` fails because the PARENT is not there —
nobody holds anything — and the old `dw_mutex_acquire` could not tell that
apart from "someone else holds the lock", so `playtest-server.sh up` printed
"25565 mutex held by 'unknown' after 0s" for a lock nobody was holding, and
refused to start at all.

Two things are bound here: the default is chosen by platform (a function of the
`uname -s` string, not a hardcoded literal, so a test can drive both branches
without a second host), and a failed acquisition says WHY — "cannot create" is
a materially different fact from "someone is holding the lock", and only one of
them is a reason to wait or to complain about a holder.
"""

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
    directory guaranteed to exist, which is the entire fix for B2."""
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
