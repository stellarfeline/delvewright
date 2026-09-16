"""`tools/playtest-server.sh up <path>` reads which kind of artifact it was handed.

This engine makes two kinds of thing a person might want to stand inside — a
campaign, and a prefab, which for a zone past the 48-per-axis template cap ships
as several `.nbt` tiles plus a manifest. There was a one-command path to the
first and none at all to the second, so whether "can I walk it" had an answer
depended on which kind the creator happened to be holding.

The classification is what this file binds, and it is the whole of what can be
bound without a container: `up` publishes host 25565 with a hardcoded
`-p 25565:25565` and no flag to move it, so the branches BELOW the classification
are proved by running the thing once, not by a test. What is provable here is
that the classification is total and refuses rather than guesses — which is the
half that decides whether the right branch runs at all.

Every case is driven through the REAL script with `PATH` holding no `docker`, so
a case that reached the container would fail loudly rather than quietly starting
one. The classification, and every flag refusal, sits above the first `docker`
call on purpose.
"""

import pathlib
import subprocess

import pytest

ROOT = pathlib.Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "tools" / "playtest-server.sh"


@pytest.fixture
def run(tmp_path):
    """Run `up` against a path, with an isolated $TMPDIR and no docker on PATH."""
    tmpdir = tmp_path / "tmp"
    tmpdir.mkdir()

    def go(*args):
        return subprocess.run(
            ["bash", str(SCRIPT), "up", *[str(a) for a in args]],
            capture_output=True,
            text=True,
            cwd=ROOT,
            env={
                "PATH": "/usr/bin:/bin:/usr/sbin:/sbin",
                "TMPDIR": str(tmpdir),
                "HOME": str(tmp_path),
                "DW_MUTEX_DIR": str(tmp_path / "mutex"),
            },
        )

    return go


def campaign_dir(base: pathlib.Path) -> pathlib.Path:
    """The marker `delvec build` itself reads first."""
    d = base / "a-campaign"
    d.mkdir(parents=True)
    (d / "world.json").write_text("{}")
    return d


def kind_of(result) -> str:
    """The kind the script says it read, from the line it prints every run."""
    for line in result.stdout.splitlines():
        if line.startswith("subject: "):
            return line.rsplit("->", 1)[-1].strip()
    return ""


# ---------------------------------------------------------------------------
# The four shapes a creator can be holding
# ---------------------------------------------------------------------------


def test_a_directory_holding_world_json_is_a_campaign(run, tmp_path):
    result = run(campaign_dir(tmp_path))
    assert kind_of(result) == "campaign", result.stdout + result.stderr


def test_a_campaign_is_a_campaign_even_holding_a_stray_nbt(run, tmp_path):
    """`world.json` is asked FIRST, so today's behaviour on a campaign is untouched.

    The order is the whole guarantee: a campaign that ships a loose `.nbt` beside
    its documents must not become a browse world of that one file, which is what
    a prefab-first reading would do — silently, and only to the campaigns that
    happen to carry one.
    """
    d = campaign_dir(tmp_path)
    (d / "stray.nbt").write_bytes(b"\x1f\x8b")
    result = run(d)
    assert kind_of(result) == "campaign", result.stdout + result.stderr


@pytest.mark.parametrize("name", ["the-castle.json", "keep-gate-room.nbt"])
def test_a_prefab_file_is_a_prefab(run, tmp_path, name):
    """A manifest and a single piece are both prefabs, and neither needs a flag."""
    f = tmp_path / name
    f.write_bytes(b"\x1f\x8b")
    result = run(f)
    assert kind_of(result) == "prefab", result.stdout + result.stderr


def test_a_directory_of_nbt_is_a_prefab(run, tmp_path):
    d = tmp_path / "build"
    d.mkdir()
    (d / "gate.nbt").write_bytes(b"\x1f\x8b")
    result = run(d)
    assert kind_of(result) == "prefab", result.stdout + result.stderr


# ---------------------------------------------------------------------------
# ...and it refuses rather than guessing
# ---------------------------------------------------------------------------


def test_a_directory_that_is_neither_is_refused_naming_both_shapes(run, tmp_path):
    """No third branch guesses.

    "Build it as whichever branch ran" is how a tool answers confidently about
    the wrong artifact, so a directory carrying neither marker is a refusal that
    names what each kind looks like.
    """
    d = tmp_path / "empty"
    d.mkdir()
    (d / "notes.txt").write_text("nothing to build here")
    result = run(d)
    assert result.returncode != 0, result.stdout
    assert "neither a campaign nor a prefab" in result.stderr, result.stderr
    assert "world.json" in result.stderr, result.stderr
    assert ".nbt" in result.stderr, result.stderr
    assert kind_of(result) == "", "nothing may be classified after a refusal"


def test_a_missing_path_is_refused_before_anything_is_claimed(run, tmp_path):
    result = run(tmp_path / "nowhere")
    assert result.returncode != 0
    assert "no such path" in result.stderr, result.stderr


@pytest.mark.parametrize(
    "flag,value,reason",
    [
        ("--lang", "zh_cn", "campaign's translation"),
        ("--prefabs", "somewhere", "library a CAMPAIGN builds from"),
        ("--stage-anyway", "because", "staging gate"),
        ("--acknowledge-red", "3", "--stage-anyway"),
    ],
)
def test_a_campaign_flag_handed_to_a_prefab_is_refused_not_dropped(
    run, tmp_path, flag, value, reason
):
    """A flag silently dropped is a creator who believes it took effect.

    `--prefabs` is the one that matters most: it reads as though it would point
    this at a prefab, which is the mistake the whole two-kinds change exists to
    make impossible to make quietly.
    """
    f = tmp_path / "the-castle.json"
    f.write_bytes(b"{}")
    result = run(f, flag, value)
    assert result.returncode != 0, result.stdout
    assert reason in result.stderr, result.stderr


def test_the_staging_gate_is_never_skipped_on_the_campaign_path():
    """The gate's invocation is inside the campaign branch, and only there.

    The prefab path exists to serve a building; it may not become the way a
    campaign reaches the owner without the gate. Read from the script's text
    rather than from a run, because proving it by running would need the
    container this file deliberately does not start — and the property is about
    which branch the call sits in.
    """
    text = SCRIPT.read_text()
    calls = [l for l in text.splitlines() if "staging-gate.py" in l and "python3" in l]
    assert len(calls) == 1, f"the gate is invoked {len(calls)} time(s): {calls}"
    campaign_branch = text.index('if [ "$SUBJECT_KIND" = "campaign" ]; then')
    prefab_branch = text.index("# ---- a prefab: the browse world")
    assert campaign_branch < text.index(calls[0]) < prefab_branch, (
        "the staging gate must be invoked inside the campaign branch and nowhere else"
    )
