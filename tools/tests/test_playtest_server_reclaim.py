"""`tools/playtest-server.sh` reclaims every class a session holds.

The defect: `up` staged the world into `$TMPDIR/dw-playtest-data.XXXXXX` and the
build into `$TMPDIR/dw-playtest-out.XXXXXX`, and `down` removed the container and
released the 25565 mutex and nothing else. Both paths came out of `mktemp -d`, so
they were unguessable, and `down` — a different shell, told only `--name` — could
not have found them however much it wanted to. Every session left a whole
generated world behind, permanently.

A reclaimer must name every class its subject holds and account for each, rather
than the classes its author remembered. So these drive the REAL script's `down`
and `status` — `up` is not exercised here because it publishes host 25565 with a
hardcoded `-p 25565:25565` and no flag to move it, which is precisely why the
proof of the whole cycle is a one-off and this is the part that must keep working.

Accounted-for, not merely deleted: the staged world is reclaimed (with its size),
and the build output is KEPT and printed with its size and the command to remove
it, because that is where `staging-gate.md` and `resourcepack.zip` live. Silently
skipping it would be the same defect one class along.
"""

import pathlib
import subprocess

import pytest

ROOT = pathlib.Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "tools" / "playtest-server.sh"
NAME = "dw-test-session"


@pytest.fixture
def session(tmp_path):
    """An isolated `$TMPDIR` + mutex dir, and a helper that runs the script."""
    tmpdir = tmp_path / "tmp"
    tmpdir.mkdir()
    env = {
        "PATH": "/usr/bin:/bin:/usr/sbin:/sbin",
        "HOME": str(tmp_path),
        "TMPDIR": str(tmpdir),
        # Never the real lock: it guards the owner's live play session.
        "DW_MUTEX_DIR": str(tmp_path / "mutex.lock.d"),
    }

    def run(*args, name=NAME):
        return subprocess.run(
            ["bash", str(SCRIPT), *args, "--name", name],
            cwd=ROOT, env=env, capture_output=True, text=True,
        )

    def record(**paths):
        """Write the session file the way `up` writes it: `key<TAB>path`."""
        lines = "".join(f"{k.replace('_', '-')}\t{v}\n" for k, v in paths.items())
        (tmpdir / f"dw-playtest-session.{NAME}").write_text(lines, encoding="utf-8")

    def staged(kib):
        d = tmp_path / f"world-{kib}"
        (d / "world" / "region").mkdir(parents=True)
        (d / "world" / "region" / "r.0.0.mca").write_bytes(b"\0" * kib * 1024)
        return d

    run.record, run.staged, run.tmpdir = record, staged, tmpdir
    return run


@pytest.mark.parametrize("kib,expected", [(512, "512 KiB"), (4096, "4.0 MiB")])
def test_down_reclaims_the_staged_world_and_says_how_much(session, kib, expected):
    world = session.staged(kib)
    session.record(stage_dir=str(world))
    result = session("down")
    assert result.returncode == 0, result.stderr
    assert not world.exists(), "the staged world survived `down`"
    assert "staged world reclaimed" in result.stdout
    # The figure MOVES with the directory: a constant would read identically here
    # on one case and say nothing about whether anything was measured.
    assert f"({expected})" in result.stdout, result.stdout


def test_the_session_record_does_not_outlive_the_session(session):
    session.record(stage_dir=str(session.staged(1)))
    assert session("down").returncode == 0
    assert not (session.tmpdir / f"dw-playtest-session.{NAME}").exists()


def test_a_world_already_gone_refuses_nothing(session):
    session.record(stage_dir=str(session.tmpdir / "never-existed"))
    result = session("down")
    assert result.returncode == 0, result.stderr
    assert "already gone" in result.stdout


def test_no_record_at_all_refuses_nothing(session):
    result = session("down")
    assert result.returncode == 0, result.stderr
    assert "nothing to reclaim" in result.stdout


def test_the_build_output_is_kept_and_named_rather_than_skipped(session):
    build = session.staged(4)
    session.record(stage_dir=str(session.tmpdir / "gone"), build_dir=str(build))
    result = session("down")
    assert result.returncode == 0, result.stderr
    assert build.exists(), "the build output holds the gate report; it is kept"
    assert "build output KEPT" in result.stdout
    assert f"rm -rf {build}" in result.stdout


def test_status_shows_what_the_session_holds_on_disk(session):
    world = session.staged(2)
    session.record(stage_dir=str(world), build_dir=str(session.tmpdir / "gone"))
    result = session("status")
    assert result.returncode == 0, result.stderr
    assert f"stage-dir  {world}" in result.stdout
    assert "(already gone)" in result.stdout


def test_status_says_so_when_there_is_no_record(session):
    result = session("status")
    assert result.returncode == 0, result.stderr
    assert "session record: none" in result.stdout


def test_every_directory_up_mints_is_recorded():
    """The half of the cycle no test can execute, asserted statically.

    `up` binds host 25565 with a hardcoded `-p 25565:25565`, so nothing in this
    suite may run it — and `down` can only reclaim what `up` wrote down. A THIRD
    `mktemp -d` added later without a `session_record` beside it would leak
    exactly as the first two did, with every test here still green, so the count
    is the thing asserted: it moves the moment somebody mints a directory this
    script does not account for.
    """
    text = SCRIPT.read_text(encoding="utf-8")
    code = "\n".join(l for l in text.splitlines() if not l.lstrip().startswith("#"))
    minted = code.count("mktemp -d")
    recorded = code.count("session_record ")
    assert minted == 2, f"{minted} temporary directories minted; update this test"
    assert recorded == minted, (
        f"{minted} `mktemp -d` but {recorded} `session_record` — a directory this "
        f"script mints and does not record cannot be reclaimed by `down`, which is "
        f"given nothing but --name"
    )


@pytest.mark.parametrize("name", ["../escape", "a/b", "", "..", "a b"])
def test_a_name_that_is_not_a_plain_container_name_is_refused(session, name):
    # `--name` reaches a filesystem path, so it is validated rather than trusted;
    # the grammar is docker's own for a container name.
    result = session("down", name=name)
    assert result.returncode != 0
    assert "--name" in result.stderr
