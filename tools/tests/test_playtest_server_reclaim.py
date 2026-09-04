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
import re
import subprocess

import pytest

ROOT = pathlib.Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "tools" / "playtest-server.sh"
NAME = "dw-test-session"

# The size on a named line, e.g. `staged world reclaimed: /x/y (116.4 MiB)`.
# Anchored on the line, not merely on the parentheses: `down` prints two figures
# when a build directory is recorded, and an unanchored search silently reads
# whichever came first.
SIZE = r"\((?P<value>\d+(?:\.(?P<decimals>\d+))?) (?P<unit>KiB|MiB|GiB)\)"
UNIT_KIB = {"KiB": 1, "MiB": 1024, "GiB": 1024 * 1024}


def printed_size(stdout: str, prefix: str = "staged world reclaimed") -> str:
    """The figure the named line printed, e.g. `116.4 MiB`."""
    match = re.search(rf"^{prefix}: \S+ {SIZE}$", stdout, re.MULTILINE)
    assert match, f"no `{prefix}` size printed at all:\n{stdout}"
    return f"{match['value']} {match['unit']}"


def printed_kib_range(stdout: str, prefix: str = "staged world reclaimed"):
    """The KiB interval that printed figure could have been rounded from.

    Only the unit table is used, never the script's formatting rule: a private
    copy of `%.1f`-versus-`%d` here would be a second authority for the same
    thing, and the point of the assertion is the NUMBER. The tolerance comes out
    of the figure's own printed precision — a value shown to d decimals stands
    for anything within half of the last digit.
    """
    match = re.search(rf"^{prefix}: \S+ {SIZE}$", stdout, re.MULTILINE)
    assert match, f"no `{prefix}` size printed at all:\n{stdout}"
    factor = UNIT_KIB[match["unit"]]
    half = 0.5 * 10 ** -len(match["decimals"] or "")
    value = float(match["value"])
    return (value - half) * factor, (value + half) * factor


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

    def du_kib(path):
        """The script's own measurement — `du -sk`, first field, same environment.

        `du` counts ALLOCATED BLOCKS, so what a filesystem reports for a file of
        N KiB is not N: the same directory measures differently on apfs and on
        ext4. Asking `du` is the only way to know what the script will print.
        """
        out = subprocess.run(["du", "-sk", str(path)], env=env,
                             capture_output=True, text=True, check=True).stdout
        return int(out.split()[0])

    run.record, run.staged, run.tmpdir, run.du_kib = record, staged, tmpdir, du_kib
    return run


@pytest.mark.parametrize("kib", [512, 4096])
def test_down_reclaims_the_staged_world_and_says_how_much(session, kib):
    """The printed figure is this directory's own size, measured the script's way.

    Not a literal: `512` bytes-times-1024 written into a file is not `512` to
    `du`, which counts allocated blocks — so a figure computed here by any other
    method is a claim about the filesystem rather than about the script. This ran
    green on apfs and red on the ubuntu runner for exactly that reason. The
    expected number therefore comes from the SAME `du -sk` the script runs, taken
    before `down` removes the directory.
    """
    world = session.staged(kib)
    measured = session.du_kib(world)
    session.record(stage_dir=str(world))
    result = session("down")
    assert result.returncode == 0, result.stderr
    assert not world.exists(), "the staged world survived `down`"
    low, high = printed_kib_range(result.stdout)
    assert low <= measured <= high, (
        f"du -sk says {measured} KiB; the script printed a figure covering "
        f"[{low}, {high}]\n{result.stdout}"
    )


def test_the_figure_is_right_where_allocation_overhead_dominates(session):
    """The case that reds a byte-count expectation on any filesystem.

    A world is thousands of small files, and `du` charges each one a whole block:
    the allocated total runs far above the bytes written, by a factor that is a
    property of the filesystem and of nothing else. An expectation computed from
    the bytes is a claim about ext4 or about apfs, never about this script — which
    is how the first version of this test passed on one runner and failed on the
    other. Here the two are deliberately far apart, and the assertion still holds
    because both sides come from `du`.
    """
    world = session.tmpdir.parent / "many-small-files"
    (world / "world" / "region").mkdir(parents=True)
    for i in range(2000):
        (world / "world" / "region" / f"c.{i}.dat").write_bytes(b"\0" * 11)
    written_kib = (2000 * 11) / 1024
    measured = session.du_kib(world)
    assert measured > written_kib * 4, (
        f"this case is meant to be dominated by allocation overhead, but du says "
        f"{measured} KiB against {written_kib:.1f} KiB written — it proves nothing"
    )
    session.record(stage_dir=str(world))
    low, high = printed_kib_range(session("down").stdout)
    assert low <= measured <= high


def test_the_reported_size_moves_with_the_directory(session):
    """The anti-constant property, asserted without knowing the format at all.

    A hardcoded figure satisfies the test above on any single case. This one is
    the check that the script measured anything: two directories an order of
    magnitude apart must not print the same string.
    """
    figures = []
    for kib in (64, 8192):
        world = session.staged(kib)
        session.record(stage_dir=str(world))
        figures.append(printed_size(session("down").stdout))
    assert figures[0] != figures[1], f"both printed {figures[0]}"


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
