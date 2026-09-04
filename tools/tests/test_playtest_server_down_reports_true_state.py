"""`tools/playtest-server.sh down` says whether a container ever existed.

The defect: `docker rm -f "$NAME" >/dev/null 2>&1 && echo "$NAME removed" ||
echo "$NAME was not running"` reads the exit status of `docker rm -f` as the
existence signal. Modern `docker rm -f` on a name that never existed still
exits 0 — removing nothing is not an error to it — so the old line printed
"$NAME removed" for a container that was never running, on every single
`down` where nothing was up. The fix asks a different question first
(`docker container inspect "$NAME"`, which does distinguish present from
absent) and only then removes.

Driven with a fake `docker` on PATH rather than the real one: `container
inspect` answers from a fixed set of "known" names, and `rm -f` always exits 0
regardless of the name — reproducing exactly the modern-docker behaviour that
made the old one-liner lie. `down`'s exit status must stay 0 either way (it
still releases the 25565 mutex and tears down a session that may never have
had a container).
"""

import os
import pathlib
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "tools" / "playtest-server.sh"
NAME = "dw-test-session"


def fake_docker(tmp_path, known):
    """A `docker` whose `rm -f` always exits 0 (the modern, lying behaviour)
    and whose `container inspect` exits 0 only for a name in `known`."""
    bindir = tmp_path / "fakebin"
    bindir.mkdir(exist_ok=True)
    script = bindir / "docker"
    script.write_text(
        "#!/usr/bin/env python3\n"
        "import sys\n"
        f"KNOWN = {known!r}\n"
        "args = sys.argv[1:]\n"
        "if args[:2] == ['container', 'inspect']:\n"
        "    sys.exit(0 if args[2] in KNOWN else 1)\n"
        "if args[:1] == ['rm']:\n"
        "    sys.exit(0)\n"
        "sys.exit(0)\n",
        encoding="utf-8",
    )
    script.chmod(0o755)
    return bindir


def run_down(tmp_path, known):
    bindir = fake_docker(tmp_path, known)
    env = dict(os.environ)
    env["PATH"] = f"{bindir}{os.pathsep}/usr/bin:/bin:/usr/sbin:/sbin"
    env["HOME"] = str(tmp_path)
    # Never the real lock: it guards the owner's live play session.
    env["DW_MUTEX_DIR"] = str(tmp_path / "mutex.lock.d")
    return subprocess.run(
        ["bash", str(SCRIPT), "down", "--name", NAME],
        cwd=ROOT, env=env, capture_output=True, text=True,
    )


def test_a_container_that_never_existed_is_reported_absent(tmp_path):
    result = run_down(tmp_path, known=[])
    assert result.returncode == 0, result.stderr
    assert f"{NAME} was not running" in result.stdout, result.stdout
    assert "removed" not in result.stdout, (
        "a container the fake docker never knew about was reported removed — "
        "the exit status of a rm -f that always succeeds is not an existence "
        "check"
    )


def test_a_container_that_exists_is_reported_removed(tmp_path):
    result = run_down(tmp_path, known=[NAME])
    assert result.returncode == 0, result.stderr
    assert f"{NAME} removed" in result.stdout, result.stdout
    assert "was not running" not in result.stdout, result.stdout


def test_down_still_exits_zero_and_releases_the_mutex_either_way(tmp_path):
    # No mutex was ever taken by this test, so the release path is the
    # "already free" one — asserted here only for its exit status, which the
    # header contract (down always tears the session down) does not vary on
    # whether a container was there to remove.
    for known in ([], [NAME]):
        result = run_down(tmp_path, known)
        assert result.returncode == 0, result.stderr
