"""`scripts/handout.py`, the per-place handout, perturbed toward each refusal it owes.

The script's whole verdict comes from `delvec` — `allocation --all` for the
facts, `detail` for the refusal a repair hands on — so every test here runs it
against a stub `delvec` that answers with the recorded allocations of
`tools/tests/fixtures/handout/` (held equal to the real binary's output by
`crates/delvec/tests/handout_script.rs`) and with whatever `detail` verdict the
test plants. One rig, one thing moved per test.
"""

from __future__ import annotations

import json
import pathlib
import shutil
import subprocess
import sys

import pytest

REPO = pathlib.Path(__file__).resolve().parents[2]
SKILL = REPO / ".claude" / "skills" / "delvewright" / "skills" / "new-delve"
SCRIPT = SKILL / "scripts" / "handout.py"
FIXTURE = REPO / "tools" / "tests" / "fixtures" / "handout"
ALLOCATIONS = FIXTURE / "allocations.json"

STUB = r'''#!/usr/bin/env python3
"""A `delvec` that answers `--version`, `allocation --all` and `detail` from files."""
import pathlib, sys
here = pathlib.Path(__file__).resolve().parent
args = sys.argv[1:]
if args == ["--version"]:
    print("delvec 0.0.0-stub, dsl 0.0.0, mc 1.21.11")
    sys.exit(0)
if "allocation" in args:
    if "--all" not in args:
        sys.exit(99)
    sys.stdout.write((here / "allocations.json").read_text())
    sys.exit(0)
if "detail" in args:
    verdict = here / "detail-verdict.txt"
    sys.stderr.write(verdict.read_text() if verdict.is_file() else "")
    sys.exit(int((here / "detail-exit.txt").read_text()) if (here / "detail-exit.txt").is_file() else 0)
sys.exit(98)
'''


class Rig:
    def __init__(self, root: pathlib.Path) -> None:
        root = root.resolve()  # the script prints canonical paths
        self.root = root
        self.campaign = root / "campaign"
        shutil.copytree(FIXTURE / "campaign", self.campaign)
        self.prefabs = root / "prefabs"
        self.prefabs.mkdir()
        self.bin = root / "bin"
        self.bin.mkdir()
        self.delvec = self.bin / "delvec"
        self.delvec.write_text(STUB, encoding="utf-8")
        self.delvec.chmod(0o755)
        shutil.copy2(ALLOCATIONS, self.bin / "allocations.json")

    def plant_detail(self, exit_code: int, verdict: str) -> None:
        (self.bin / "detail-exit.txt").write_text(str(exit_code), encoding="utf-8")
        (self.bin / "detail-verdict.txt").write_text(verdict, encoding="utf-8")

    def program(self, stem: str) -> pathlib.Path:
        d = self.campaign / "programs"
        d.mkdir(exist_ok=True)
        p = d / f"{stem}.json"
        p.write_text("{}\n", encoding="utf-8")
        return p

    def run(self, *extra: str, place: str = "node/hall", campaign: pathlib.Path | None = None):
        return subprocess.run(
            [
                sys.executable,
                "-I",
                str(SCRIPT),
                str(campaign or self.campaign),
                place,
                "--engine",
                str(REPO),
                "--prefabs",
                str(self.prefabs),
                "--delvec",
                str(self.delvec),
                *extra,
            ],
            capture_output=True,
            text=True,
            check=False,
        )

    def handouts(self, stem: str) -> list[pathlib.Path]:
        d = self.campaign / ".out" / "handouts" / stem
        return sorted(d.glob("handout-*.md")) if d.is_dir() else []


@pytest.fixture
def rig(tmp_path: pathlib.Path) -> Rig:
    return Rig(tmp_path)


def allocation(place: str) -> dict:
    return next(a for a in json.loads(ALLOCATIONS.read_text(encoding="utf-8")) if a["place"] == place)


def test_a_first_authoring_hands_the_allocation_the_neighbours_the_seams_and_the_voids(rig: Rig):
    out = rig.run()
    assert out.returncode == 0, out.stderr
    handouts = rig.handouts("hall")
    assert [p.name for p in handouts] == ["handout-1.md"]
    text = handouts[0].read_text(encoding="utf-8")
    a = allocation("node/hall")
    # The allocation, verbatim: every top-level key stands in the embedded JSON.
    for key in a:
        assert f'"{key}":' in text, key
    # One section per adjacent box, each with the neighbour's own design row.
    for nb in a["neighbours"]:
        assert f"### {nb['place']} — on your {nb['face']} side" in text
    assert text.count("### node/") == len(a["neighbours"])
    assert "A low stone porch" in text, "the landing's note is quoted"
    # The brief's identities that measure this box, with the fact's number and note.
    assert "`fact/hall-span` eq box-extent on `x` = 12.0 blocks — The hall is twelve blocks wide" in text
    assert "`fact/hall-height` eq box-height = 8.0 blocks" in text
    assert "fact/landing-height" not in text, "the landing's fact is not the hall's"
    # Every void with its owner, and the fact that painting one is replaced.
    for v in a["voids"]:
        lo, hi = v["cells"]
        assert f"[{lo[0]},{lo[1]},{lo[2]}]..[{hi[0]},{hi[1]},{hi[2]}] — owned by {v['owner']}" in text
    assert "replaced by `structure_void` after expansion" in text
    # Every seam says who owns the plane; the hall owns one and not the other.
    assert "**You do NOT own the plane**: `node/landing`" in text
    assert "**You own the plane**" in text
    assert "**The bar is yours**" in text
    assert "whose `bar.region` is `seam-hall-yard`" in text
    # The owed names: the gate region answers through the barred edge, not a mark.
    assert "`anchor/seam-hall-yard` → the `barred` contract edge whose `bar.region` is `seam-hall-yard`" in text
    assert "`anchor/node-hall` → `mark` `node-hall`" in text
    # The frame's extent and the exact check command, the instrument named by path.
    assert "extent **14×10×18**" in text
    assert f'"{rig.delvec}" --prefabs "{rig.prefabs}" detail "{rig.campaign}" node/hall' in text
    assert "--region 14x10x18" in text
    # No refusal section on a first authoring.
    assert "## The refusal this attempt repairs" not in text
    # stdout: the path, then the one-line prompt.
    lines = out.stdout.strip().split("\n")
    assert lines == [str(handouts[0]), f"Read and execute {handouts[0]}. Report, then exit."]
    assert "handout binding: `node/hall` attempt 1" in out.stderr
    assert "neighbours 2; seams 2; voids 5; owed names 3; refusal lines 0" in out.stderr


def test_the_entry_place_is_told_it_owes_the_spawn(rig: Rig):
    out = rig.run(place="node/landing")
    assert out.returncode == 0, out.stderr
    text = rig.handouts("landing")[0].read_text(encoding="utf-8")
    assert "- the ENTRY: the party spawns here (`spawn` is owed)" in text
    assert "`spawn` → `mark` `spawn`" in text
    assert "### node/hall — on your east side" in text
    assert "node/hall" in text and "### node/yard" not in text, "the yard is not adjacent to the landing"


def test_an_unallocated_place_is_refused_naming_every_place_the_plan_allocates(rig: Rig):
    out = rig.run(place="node/nowhere")
    assert out.returncode == 1
    assert "the plan allocates no box to `node/nowhere`" in out.stderr
    for place in ("node/landing", "node/hall", "node/yard"):
        assert f"`{place}`" in out.stderr
    assert rig.handouts("nowhere") == []


def test_a_directory_that_is_not_a_campaign_is_refused(rig: Rig, tmp_path: pathlib.Path):
    out = rig.run(campaign=tmp_path)
    assert out.returncode == 2
    assert "carries no `world.json`" in out.stderr


def test_a_place_that_is_not_a_node_id_is_refused(rig: Rig):
    out = rig.run(place="hall")
    assert out.returncode == 2
    assert "is not a place" in out.stderr


def test_a_program_that_already_passes_owes_no_handout(rig: Rig):
    rig.program("hall")
    rig.plant_detail(0, "node/hall: `prefab/three-places-hall` written\n")
    out = rig.run()
    assert out.returncode == 3
    assert "already passes `delvec detail`" in out.stderr
    assert rig.handouts("hall") == []


def test_a_refused_program_puts_the_verdict_into_the_next_attempt(rig: Rig):
    first = rig.run()
    assert first.returncode == 0, first.stderr
    rig.program("hall")
    verdict = (
        "DW0844 [error] node/hall: the piece answers no seam on its west face\n"
        "error: `node/hall`: the piece(s) `prefab/three-places-hall` would not bind; nothing was written.\n"
    )
    rig.plant_detail(1, verdict)
    second = rig.run()
    assert second.returncode == 0, second.stderr
    handouts = rig.handouts("hall")
    assert [p.name for p in handouts] == ["handout-1.md", "handout-2.md"]
    text = handouts[1].read_text(encoding="utf-8")
    assert "— attempt 2" in text
    assert "## The refusal this attempt repairs" in text
    for line in verdict.strip().split("\n"):
        assert line in text
    assert "exit status 1" in text
    refusal = handouts[1].with_name("refusal-2.txt")
    assert refusal.is_file()
    assert verdict in refusal.read_text(encoding="utf-8")
    assert "refusal lines 4" in second.stderr
    assert "a repair of the program the last agent left" in second.stderr


def test_a_detailed_neighbour_is_named_with_its_piece(rig: Rig):
    (rig.campaign / "detail-plan.json").write_text(
        json.dumps(
            {
                "campaign_id": "three-places",
                "content": {"details": [{"anchors": {}, "piece": "prefab/three-places-landing", "place": "node/landing"}]},
                # the fixture's own number, never a literal a bump would have to find
                "dsl_version": json.loads((rig.campaign / "world.json").read_text(encoding="utf-8"))["dsl_version"],
                "stage": "detail-plan",
            }
        ),
        encoding="utf-8",
    )
    out = rig.run()
    assert out.returncode == 0, out.stderr
    text = rig.handouts("hall")[0].read_text(encoding="utf-8")
    assert "- already detailed: its piece is `prefab/three-places-landing`" in text
    assert text.count("- not detailed yet") == 1, "the yard"


def test_the_stub_answers_with_the_allocation_struct_s_own_fields():
    """The recorded fixture carries exactly the fields `Allocation` serializes.

    A second method beside the crate test that compares bytes: the struct's
    field names are read off the Rust source, so a field added to the handout
    the engine prints and not re-recorded here is a red in this suite too.
    """
    import re

    source = (REPO / "crates" / "delvec" / "src" / "compiler" / "detail.rs").read_text(encoding="utf-8")
    body = re.search(r"pub struct Allocation \{(.*?)\n\}", source, re.S).group(1)
    fields = set(re.findall(r"^\s*pub (\w+):", body, re.M))
    recorded = json.loads(ALLOCATIONS.read_text(encoding="utf-8"))
    assert len(recorded) == 3
    optional = {"palette", "roof"}  # `skip_serializing_if = "Option::is_none"`
    for a in recorded:
        assert set(a) <= fields, set(a) - fields
        assert fields - set(a) <= optional, fields - set(a) - optional
