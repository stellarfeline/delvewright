"""`scripts/handout.py`, the per-place handout, perturbed toward each refusal it owes.

The script's whole verdict comes from `delvec` — `allocation --all` for the
facts and the wave bound, `detail --check` for the refusal a repair hands on — so every test here runs it
against a stub `delvec` that answers with the recorded allocations of
`tools/tests/fixtures/handout/` and the recorded block lists of `blocks.json`
(each held equal to the real binary's output by
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
"""A `delvec` that answers `--version`, `allocation --all`, `grammar blocks` and `detail` from files."""
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
    bound = here / "concurrency.txt"
    sys.stderr.write(bound.read_text() if bound.is_file() else "concurrency: 2 = min(3, max(1, 4 / 2))\n")
    sys.exit(0)
if args == ["grammar", "blocks"]:
    blocks = here / "blocks.json"
    if not blocks.is_file():
        sys.exit(2)
    sys.stdout.write(blocks.read_text())
    sys.exit(0)
if "detail" in args:
    if "--check" not in args:
        sys.exit(97)  # the handout never commits
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
        shutil.copy2(FIXTURE / "blocks.json", self.bin / "blocks.json")

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
    assert f'"{rig.delvec}" --prefabs "{rig.prefabs}" detail --check "{rig.campaign}" node/hall' in text
    # The wave bound, quoted from the engine; the freshness line; the absent operating half.
    assert "`concurrency: 2 = min(3, max(1, 4 / 2))`" in text
    assert "**Which neighbours are detailed is as of this handout**" in text
    assert "a neighbour may have landed since" in text
    assert "`CLAUDE.local.md`" in text and "binds you in nothing" in text
    assert "The commit, `delvec detail` without `--check`, is the orchestrating session's, never yours." in text
    assert "--region 14x10x18" in text
    # No refusal section on a first authoring.
    assert "## The refusal this attempt repairs" not in text
    # stdout: the path, then the one-line prompt.
    lines = out.stdout.strip().split("\n")
    assert lines == [str(handouts[0]), f"Read and execute {handouts[0]}. Report, then exit."]
    assert "handout binding: `node/hall` attempt 1" in out.stderr
    assert "neighbours 2; seams 2; voids 5; owed names 3; refusal lines 0" in out.stderr
    assert "concurrency 2; design rows" in out.stderr
    assert "handout: concurrency: 2 = min(3, max(1, 4 / 2))" in out.stderr


def test_the_wave_bound_is_read_from_the_engine_never_assumed(rig: Rig):
    (rig.bin / "concurrency.txt").write_text("concurrency: 1 = min(3, max(1, 1 / 2))\n", encoding="utf-8")
    out = rig.run()
    assert out.returncode == 0, out.stderr
    assert "`concurrency: 1 = min(3, max(1, 1 / 2))`" in rig.handouts("hall")[0].read_text(encoding="utf-8")


def test_a_delvec_that_states_no_wave_bound_is_refused(rig: Rig):
    (rig.bin / "concurrency.txt").write_text("", encoding="utf-8")
    out = rig.run()
    assert out.returncode == 2
    assert "stated no `concurrency:` line" in out.stderr
    assert rig.handouts("hall") == []


def test_the_blocks_a_place_may_write_are_the_engine_s_lists(rig: Rig):
    """Which blocks are refused (`DW0730`) and which are containers is read from
    `delvec grammar blocks`, saved beside the handout, never typed."""
    out = rig.run()
    assert out.returncode == 0, out.stderr
    handout = rig.handouts("hall")[0]
    blocks = json.loads((FIXTURE / "blocks.json").read_text(encoding="utf-8"))
    assert handout.with_name("blocks.json").read_text(encoding="utf-8") == (FIXTURE / "blocks.json").read_text(encoding="utf-8")
    text = handout.read_text(encoding="utf-8")
    assert f"## The blocks you may write ({len(blocks['admitted'])} admitted, {len(blocks['refused'])} refused)" in text
    for r in blocks["refused"]:
        assert f"`{r['block'].removeprefix('minecraft:')}`" in text
    assert f"**Containers** ({len(blocks['containers'])}): " in text
    assert "`smoker`" in text and "`blast_furnace`" in text
    assert f"refused blocks {len(blocks['refused'])}; containers {len(blocks['containers'])}" in out.stderr
    # Moved: the engine refuses one more block, and the handout says so.
    blocks["refused"].append({"block": "minecraft:stone_bricks", "route": None})
    (rig.bin / "blocks.json").write_text(json.dumps(blocks), encoding="utf-8")
    out = rig.run(place="node/yard")
    assert out.returncode == 0, out.stderr
    assert "`stone_bricks`" in rig.handouts("yard")[0].read_text(encoding="utf-8").split("## The blocks you may write")[1].split("**Containers**")[0]


def test_a_delvec_with_no_block_lists_is_refused(rig: Rig):
    (rig.bin / "blocks.json").unlink()
    out = rig.run()
    assert out.returncode == 2
    assert "grammar blocks` exited 2 without the block lists" in out.stderr
    assert rig.handouts("hall") == []


def test_a_starter_and_its_allocation_stand_beside_the_handout(rig: Rig):
    """Step 1 is a program that already passes, never research (spec-0122 §6.3)."""
    out = rig.run()
    assert out.returncode == 0, out.stderr
    handout = rig.handouts("hall")[0]
    starter = handout.with_name("starter.py")
    allocation_file = handout.with_name("allocation.json")
    assert starter.read_bytes() == (SKILL / "scripts" / "starter.py").read_bytes()
    assert json.loads(allocation_file.read_text(encoding="utf-8")) == allocation("node/hall")
    text = handout.read_text(encoding="utf-8")
    assert "## The steps, in the order you need them — each names its tool and what it costs" in text
    assert f'--allocation "{allocation_file}" --out "{rig.campaign / "programs" / "hall.json"}"' in text
    assert str(rig.campaign / "design" / "generator" / "hall.py") in text
    assert "It passes every judgement `detail --check` makes of the place itself as it stands" in text
    assert "not the engine's source, not the engine's gallery, not another campaign" in text
    assert "gallery/overlays/site-plan/programs" not in text, "the gallery is not on the reading list"
    # The starter, run on the handed allocation, writes a program for this place.
    prog = rig.root / "hall.json"
    r = subprocess.run([sys.executable, "-I", str(starter), "--allocation", str(allocation_file), "--out", str(prog)], capture_output=True, text=True)
    assert r.returncode == 0, r.stderr
    doc = json.loads(prog.read_text(encoding="utf-8"))
    assert doc["params"]["handed/datum-y"] == allocation("node/hall")["datum_y"]
    assert {e.get("via") or e["bar"]["region"] for e in doc["contract"]["edges"]} == {"landing-hall-way", "seam-hall-yard"}


def test_the_steps_run_in_the_order_an_author_needs_each_naming_its_tool_and_cost(rig: Rig):
    """Start, measure the palette, draw, look alone, look in place, check: each
    step names the tool it runs and what it costs, the palette is measured
    before anything is drawn, and the loop is the cheap steps — the check is
    never told to run after every edit or as often as one likes."""
    out = rig.run()
    assert out.returncode == 0, out.stderr
    text = rig.handouts("hall")[0].read_text(encoding="utf-8")
    steps = text.split("## The steps, in the order you need them")[1].split("## What you may do")[0]
    heads = ["1. **Start** — `starter.py`", "2. **Measure the palette, never guess it** — `block-appearance.py`",
             "3. **Draw** — your generator", "4. **Look at the piece alone** — `grammar expand`",
             "5. **Look at it in its surroundings** — `frames --campaign`", "6. **Check** — `detail --check`"]
    at = [steps.index(h) for h in heads]
    assert at == sorted(at), "the steps are in order"
    for key, cost in (("check", "12 to 13 min, peaking at 18.8 GB"), ("frames", "about 3 min and 11 GB"), ("generator", "about 1.5 s")):
        assert cost in steps, key
    assert "Steps 1 to 4 are seconds: run them after every edit." in steps
    assert "Run it as often as you like" not in text
    assert "check after every edit" not in text


def test_the_camera_conventions_are_stated_and_a_camera_stands_in_a_places_frame(rig: Rig):
    out = rig.run()
    assert out.returncode == 0, out.stderr
    text = rig.handouts("hall")[0].read_text(encoding="utf-8")
    assert "the bearing turns clockwise seen from above" in text
    assert "90 due west looking east" in text
    assert "**A `face=up` picture has south at its top and west at its right**" in text
    assert "--camera x,y,z,yaw,pitch[,fov]` with `--frame <place>`" in text


def test_the_handout_says_whether_the_campaigns_notice_covers_scenery(rig: Rig):
    """Scenery containers are allowed exactly where the campaign declares its
    decoration notice (`DW1018`), and never beside a container a `loot`
    entry fills in the place (`DW1024`): the handout reads both from the
    quests document and says which holds here."""
    out = rig.run()
    assert out.returncode == 0, out.stderr
    text = rig.handouts("hall")[0].read_text(encoding="utf-8")
    assert "**This campaign declares no decoration notice**" in text
    quests_path = rig.campaign / "quests.json"
    doc = json.loads(quests_path.read_text(encoding="utf-8"))
    body = doc.get("content", doc)
    body["containers"] = {"decoration_notice": {"style": "chat", "text": "Not yours to open."}}
    quests_path.write_text(json.dumps(doc), encoding="utf-8")
    out = rig.run(place="node/yard")
    assert out.returncode == 0, out.stderr
    text = rig.handouts("yard")[0].read_text(encoding="utf-8")
    assert "**This campaign declares its decoration notice** — players are told “Not yours to open.”" in text
    assert "No `loot` entry fills an owed name of this place" in text
    owed = allocation("node/landing")["owed_anchors"][0]
    body["loot"] = [{"id": "loot/x", "anchor": owed, "contents": []}]
    quests_path.write_text(json.dumps(doc), encoding="utf-8")
    out = rig.run(place="node/landing")
    assert out.returncode == 0, out.stderr
    assert f"a `loot` entry fills `{owed}` — so no scenery container may stand in it (`DW1024`)" in rig.handouts("landing")[0].read_text(encoding="utf-8")


def test_the_generator_path_is_the_campaigns_own_where_one_exists(rig: Rig):
    out = rig.run()
    assert out.returncode == 0, out.stderr
    assert str(rig.campaign / "design" / "generator" / "hall.py") in rig.handouts("hall")[0].read_text(encoding="utf-8")
    own = rig.campaign / "design" / "generator" / "lower" / "yard.py"
    own.parent.mkdir(parents=True)
    own.write_text("# the campaign's own\n", encoding="utf-8")
    out = rig.run(place="node/yard")
    assert out.returncode == 0, out.stderr
    text = rig.handouts("yard")[0].read_text(encoding="utf-8")
    assert f"write it at `{own}`" in text
    assert str(rig.campaign / "design" / "generator" / "yard.py") not in text


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
    assert "already passes `delvec detail --check`" in out.stderr
    assert "commit it with `delvec detail`" in out.stderr
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
    assert "detail --check" in refusal_cmd(text)
    refusal = handouts[1].with_name("refusal-2.txt")
    assert refusal.is_file()
    assert verdict in refusal.read_text(encoding="utf-8")
    assert "refusal lines 4" in second.stderr
    assert "a repair of the program the last agent left" in second.stderr


def test_a_commit_the_battery_refused_is_handed_back_with_the_commit_s_own_output(rig: Rig):
    """`--check` cannot see the battery, so the commit's captured output is the refusal."""
    rig.program("hall")
    rig.plant_detail(0, "")  # the check passes: only the commit's battery refused
    captured = rig.root / "commit-hall.txt"
    captured.write_text(
        "node/hall: `prefab/three-places-hall` written from `programs/hall.json`\n"
        "DW0885 [error] build: area `area/site` places `prefab/three-places-hall`, whose side is empty\n",
        encoding="utf-8",
    )
    out = rig.run("--from", str(captured))
    assert out.returncode == 0, out.stderr
    text = rig.handouts("hall")[0].read_text(encoding="utf-8")
    assert "The orchestrator's commit of it refused" in text
    assert "DW0885 [error] build" in text
    assert "detail --check" not in refusal_cmd(text)


def test_a_commit_output_that_never_names_the_place_is_refused(rig: Rig):
    captured = rig.root / "commit-yard.txt"
    captured.write_text("node/yard: written\n", encoding="utf-8")
    out = rig.run("--from", str(captured))
    assert out.returncode == 2 and "never names `node/hall`" in out.stderr
    assert rig.handouts("hall") == []


def refusal_cmd(text: str) -> str:
    """The `$ …` line the refusal section quotes: the command that gave the verdict."""
    return next(line for line in text.split("\n") if line.startswith("$ "))


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
    # `skip_serializing_if`: `Option::is_none` for `palette` and `roof`, and
    # `Vec::is_empty` for `sides` (a place declaring no open side, spec-0123)
    # and `parts` and `bands` (a place that is not a plot, spec-0125);
    # `Option::is_none` for `way` (a place that is not a way).
    optional = {"palette", "roof", "sides", "parts", "bands", "way"}
    for a in recorded:
        assert set(a) <= fields, set(a) - fields
        assert fields - set(a) <= optional, fields - set(a) - optional


# ---------------------------------------------------------------------------
# spec-0123 §3.3, §3.5, §5.3: what the place is, and the side that stands no wall
# ---------------------------------------------------------------------------

FRAME_SENTENCE = (
    "your open side stands no wall; build the frame the wall would have been one cell inside the ring, "
    "and stand what you open there in it"
)


def section(text: str, heading: str) -> str:
    """The body of one `## ` section of a handout."""
    start = text.index(f"## {heading}")
    end = text.find("\n## ", start + 3)
    return text[start : end if end != -1 else len(text)]


def test_the_place_section_carries_every_reference_the_trade_rule_and_the_style_rule(rig: Rig):
    # One reference fetched as a picture, the other recorded as a page.
    refs = rig.campaign / ".out" / "references" / "hall"
    refs.mkdir(parents=True)
    (refs / "1.png").write_bytes(b"\x89PNG\r\n\x1a\n")
    (refs / "index.json").write_text(
        json.dumps(
            {
                "place": "node/hall",
                "rows": [
                    {"n": 1, "url": "https://example.org/keepers-hall.png", "kind": "image", "file": ".out/references/hall/1.png"},
                    {"n": 2, "url": "https://example.org/timber-ceilings", "kind": "page", "file": None},
                ],
            }
        ),
        encoding="utf-8",
    )
    out = rig.run()
    assert out.returncode == 0, out.stderr
    text = rig.handouts("hall")[0].read_text(encoding="utf-8")
    body = section(text, "What this place is, and what it is built from")
    # The section stands after the campaign's breath and before the design rows.
    assert text.index("## The campaign, in one breath") < text.index("## What this place is") < text.index("## This place's rows")
    assert "- intent: `hub`; note: The keeper's hall" in body
    assert "2 human-built reference(s) to work from. Looked at, never copied: an unlicensed or non-commercial source is ideas only." in body
    assert "<https://example.org/keepers-hall.png> — A keeper's hall with the desk facing the door: take where the keeper stands. Terms: CC BY 4.0." in body
    assert f"Fetched: `{refs / '1.png'}`" in body
    assert "<https://example.org/timber-ceilings> — A page of timber ceilings over stone rooms: take the beam spacing. Terms: none stated. Not fetched as a picture" in body
    # A roofed place takes the trade rule and its threshold test, then the style rule.
    assert "**This place reads as what it is when four things hold at once.**" in body
    assert "**The test**: stand in the door and name what the place is with no sign read." in body
    assert "**This place is outside" not in body
    assert body.index("four things hold at once") < body.index("**Build it in one style**")
    for part in ("**One module**", "**Openings follow construction**", "**Lines run through**", "**A landmark reads by its silhouette from the approach**"):
        assert part in body, part
    assert "No door is named per kind of building" in body
    assert "open sides 0; references 2" in out.stderr


def test_a_place_with_no_reference_says_so_and_still_carries_the_rules(rig: Rig):
    out = rig.run(place="node/landing")
    assert out.returncode == 0, out.stderr
    body = section(rig.handouts("landing")[0].read_text(encoding="utf-8"), "What this place is, and what it is built from")
    assert "- no reference is recorded for this place." in body
    assert "four things hold at once" in body and "**Build it in one style**" in body
    assert "references 0" in out.stderr


def test_an_open_side_is_stated_with_its_cells_and_the_frame_sentence(rig: Rig):
    out = rig.run(place="node/yard")
    assert out.returncode == 0, out.stderr
    text = rig.handouts("yard")[0].read_text(encoding="utf-8")
    side = next(s for s in allocation("node/yard")["sides"] if s["face"] == "south")
    lo, hi = side["cells"]
    assert (
        f"- your **south** side is open: cells [{lo[0]},{lo[1]},{lo[2]}]..[{hi[0]},{hi[1]},{hi[2]}], "
        f"nothing stands in them from the walk plane (y {side['walk_y']}) to the top of the play space, your ground runs across them."
    ) in text
    assert FRAME_SENTENCE[0].upper() + FRAME_SENTENCE[1:] + "." in text
    # A place open to the sky takes the outside paragraph, not the trade rule.
    body = section(text, "What this place is, and what it is built from")
    assert "**This place is outside" in body and "four things hold at once" not in body
    assert "**Build it in one style**" in body
    assert "open sides 1" in out.stderr


def test_a_place_with_no_open_side_carries_no_frame_sentence(rig: Rig):
    out = rig.run()
    assert out.returncode == 0, out.stderr
    text = rig.handouts("hall")[0].read_text(encoding="utf-8")
    assert "### Open sides (0)" in text
    assert "none: every side of this place is a wall." in text
    assert FRAME_SENTENCE[0].upper() + FRAME_SENTENCE[1:] not in text
    assert FRAME_SENTENCE not in text


def test_a_neighbour_s_open_side_toward_the_place_is_named(rig: Rig):
    recorded = json.loads((rig.bin / "allocations.json").read_text(encoding="utf-8"))
    hall = next(a for a in recorded if a["place"] == "node/hall")
    yard = next(n for n in hall["neighbours"] if n["place"] == "node/yard")
    yard["open_toward"] = True
    (rig.bin / "allocations.json").write_text(json.dumps(recorded), encoding="utf-8")
    out = rig.run()
    assert out.returncode == 0, out.stderr
    text = rig.handouts("hall")[0].read_text(encoding="utf-8")
    nb = text[text.index("### node/yard") :]
    nb = nb[: nb.find("\n### ") if "\n### " in nb else len(nb)]
    assert "its side toward you is open" in nb
    assert text.count("its side toward you is open") == 1, "only the yard declares one"


def test_a_plot_is_handed_what_it_holds_its_binding_rule_and_its_shapes(rig: Rig):
    out = rig.run(place="node/yard")
    assert out.returncode == 0, out.stderr
    text = rig.handouts("yard")[0].read_text(encoding="utf-8")
    body = section(text, "What this plot holds")
    # After what the place is and what it is built from, before its design rows.
    assert text.index("## What this place is") < text.index("## What this plot holds") < text.index("## This place's rows")
    assert "This place is a plot: it holds the parts below and the ground between them; build them in any outline, inside the frame." in body
    assert "3 part(s) over 64 cell(s) of play space:" in body
    # Each part's band is the allocation's (`bands`, `plot::size_band`).
    assert "- `part/court` — ground: the court a body crosses from the hall; 50 cell(s), 78 % of the play space; walked; its spaces and regions claim 38 to 62 column(s) (DW1074)" in body
    assert "- `part/shed` — structure: a tool shed in the court's corner; 6 cell(s), 9 % of the play space; height 3 course(s) to the ridge; walked; its spaces and regions claim 5 to 7 column(s) (DW1074)" in body
    assert "- `part/bed` — ground: a herb bed nobody walks; 4 cell(s), 6 % of the play space; seen, never entered; its spaces and regions claim 3 to 5 column(s) (DW1074)" in body
    assert "a part's claimed columns stay inside the band each part line states" in body
    for line in (
        "- the entry side is the front;",
        "- an outbuilding stands behind or beside, one cell off;",
        "- a yard is enclosed by the structure it serves;",
        "- a precinct is a sequence from the gate;",
    ):
        assert line in body, line
    frame = section(text, "The frame, read before anything is drawn")
    assert "- plot: The box is sky-open, and the roof of every structure in it is yours, within the stated courses of air." in frame
    assert "parts 3" in out.stderr


def test_a_place_that_is_not_a_plot_has_no_plot_section(rig: Rig):
    out = rig.run(place="node/hall")
    assert out.returncode == 0, out.stderr
    text = rig.handouts("hall")[0].read_text(encoding="utf-8")
    assert "## What this plot holds" not in text
    assert "- plot: The box is sky-open" not in text
    assert "parts 0" in out.stderr
    # The hall's neighbour summary states the yard's parts by use.
    assert "- it is a plot holding: the court a body crosses from the hall (ground); a tool shed in the court's corner (structure); a herb bed nobody walks (ground)" in text
