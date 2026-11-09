"""`scripts/script-review.py`, the step-5b handout, perturbed toward each thing it owes.

The script a fresh reviewer reads is derived from the campaign and typed by
nobody: quests in `depends_on` order, objectives in `after` order, each
`talk-to`'s dialogue as the cast ledger shows it, and nothing of what the author
meant. The questions and the review's form come from the engine page, so a
page that stops yielding them is a refusal, never an empty handout. One rig,
one thing moved per test.
"""

from __future__ import annotations

import json
import pathlib
import subprocess
import sys

import pytest

REPO = pathlib.Path(__file__).resolve().parents[2]
SKILL = REPO / ".claude" / "skills" / "delvewright" / "skills" / "new-delve"
SCRIPT = SKILL / "scripts" / "script-review.py"
PAGE = REPO / "docs" / "reference" / "script-review.md"

DESIGN_SENTINEL = "DESIGN-ONLY-SENTINEL"
PERSONA_SENTINEL = "PERSONA-ONLY-SENTINEL"
HAPPENING_SENTINEL = "HAPPENING-ONLY-SENTINEL"
UNREACHED_SENTINEL = "UNREACHED-NODE-SENTINEL"


def envelope(stage: str, content: dict) -> dict:
    return {"campaign_id": "rig", "content": content, "dsl_version": "0.0.0", "stage": stage}


def campaign(root: pathlib.Path) -> pathlib.Path:
    c = root / "campaigns" / "rig"
    c.mkdir(parents=True)
    (c / "world.json").write_text(
        json.dumps(envelope("world", {"title": "The Rig", "premise": "A party arrives.", "target_minutes": 30}))
    )
    (c / "classes.json").write_text(
        json.dumps(
            envelope(
                "classes",
                {"classes": [{"id": "class/a", "name": "The Walker", "blurb": "Walks.", "kit": [{"item": "minecraft:bread", "count": 2, "name": "Bread"}]}]},
            )
        )
    )
    (c / "npcs.json").write_text(
        json.dumps(
            envelope(
                "npcs",
                {
                    "npcs": [
                        {"id": "npc/anna", "name": "Anna", "persona": {"motivation": PERSONA_SENTINEL}},
                        {"id": "npc/bo", "name": "Bo", "persona": {"motivation": "none"}},
                    ]
                },
            )
        )
    )
    # The plan's file order puts the finale FIRST and the hub LAST: the script must
    # order by `depends_on`, never by file order.
    (c / "quest-plan.json").write_text(
        json.dumps(
            envelope(
                "quest-plan",
                {
                    "quests": [
                        {"id": "quest/finale", "goal": "Finish.", "mandatory": True, "depends_on": ["quest/spoke"]},
                        {"id": "quest/spoke", "goal": "Do the spoke.", "mandatory": True, "depends_on": ["quest/hub"]},
                        {"id": "quest/side", "goal": "A side thing.", "mandatory": False, "depends_on": ["quest/hub"]},
                        {"id": "quest/hub", "goal": "Meet Anna.", "mandatory": True, "depends_on": []},
                    ]
                },
            )
        )
    )
    quests = {
        "quests": [
            {
                "id": "quest/hub",
                "trigger": {"type": "campaign-start"},
                "happening": {"verb": "learns", "text": HAPPENING_SENTINEL},
                "cast": {
                    "npc/anna": {"at": "anchor/a", "doing": "waiting", "dialogue": "dlg/anna-hello"},
                    "npc/bo": [
                        {"at": "anchor/b", "doing": "working", "dialogue": {"barks": ["Busy."]}, "forbids_flags": ["flag/x"]},
                        {"at": "anchor/b", "doing": "working", "dialogue": {"barks": ["Busy."]}, "requires_flags": ["flag/x"]},
                    ],
                },
                "objectives": [
                    {"id": "obj/meet", "type": "talk-to", "npc": "npc/anna", "title": "Speak to Anna", "hint": "By the well."},
                ],
                "on_objective_complete": {},
                "on_complete": [{"type": "narrate", "text": "Anna waves you off."}],
            },
            {
                "id": "quest/spoke",
                "trigger": {"type": "quest-complete", "quest": "quest/hub"},
                "cast": {"npc/anna": {"at": "anchor/a", "doing": "waiting", "dialogue": "dlg/anna-hello"}},
                # File order is reversed against `after`: the script must print
                # the lever first, the report second.
                "objectives": [
                    {"id": "obj/report", "type": "talk-to", "npc": "npc/anna", "title": "Tell Anna", "hint": "Back at the well.", "after": ["obj/lever"]},
                    {
                        "id": "obj/lever",
                        "type": "interact",
                        "anchor": "anchor/lever",
                        "title": "Pull the lever",
                        "hint": "In the mill.",
                        "compass": "hidden",
                        "prop": {"block": "minecraft:lever"},
                        "answers": {"not_yet": "Stiff.", "done": "Pulled."},
                    },
                ],
                "on_objective_complete": {"obj/lever": [{"type": "set-flag", "flag": "flag/x"}, {"type": "give-item", "item": "minecraft:stick", "count": 1, "name": "A Stick"}]},
                "on_complete": [],
            },
            {
                "id": "quest/side",
                "trigger": {"type": "quest-complete", "quest": "quest/hub"},
                "cast": {},
                "objectives": [{"id": "obj/reach", "type": "reach-anchor", "anchor": "anchor/hill", "radius": 3, "title": "Climb the hill", "hint": "Up."}],
                "on_objective_complete": {},
                "on_complete": [],
            },
            {
                "id": "quest/finale",
                "trigger": {"type": "quest-complete", "quest": "quest/spoke"},
                "cast": {},
                "objectives": [{"id": "obj/end", "type": "reach-anchor", "anchor": "anchor/a", "radius": 3, "title": "Return", "hint": "Back."}],
                "on_objective_complete": {},
                "on_complete": [{"type": "campaign-complete", "ending": "ending/home"}],
            },
        ],
        "triggers": [
            {"id": "trigger/note", "at": "anchor/note", "on": {"on": "use"}, "once": True, "effects": [{"type": "narrate", "text": "A note on the door."}]}
        ],
        "shops": [],
        "signs": [{"id": "sign/mill", "anchor": "anchor/sign", "front": ["", "Mill"], "back": ["", "Mill"]}],
        "state": [],
    }
    (c / "quests.json").write_text(json.dumps(envelope("quests", quests)))
    (c / "dialogue.json").write_text(
        json.dumps(
            envelope(
                "dialogue",
                {
                    "dialogues": [
                        {
                            "npc": "npc/anna",
                            "root": "dlg/anna-hello",
                            "nodes": [
                                {
                                    "id": "dlg/anna-hello",
                                    "text": "Hello, strangers.",
                                    "options": [
                                        {"label": "Hello.", "effects": [{"type": "complete-objective", "objective": "obj/meet"}]},
                                        {"label": "Why the well?", "next": "dlg/anna-well", "tooltip": "Ask about the well."},
                                        {"label": "The lever is pulled.", "requires_flags": ["flag/x"], "effects": [{"type": "complete-objective", "objective": "obj/report"}]},
                                    ],
                                },
                                {"id": "dlg/anna-well", "text": "It is the only one.", "options": [{"label": "Back.", "next": "dlg/anna-hello"}]},
                                {"id": "dlg/anna-lost", "text": UNREACHED_SENTINEL, "options": []},
                            ],
                        }
                    ]
                },
            )
        )
    )
    (c / "DESIGN.md").write_text(f"# Design\n\n{DESIGN_SENTINEL}\n")
    (c / "GENERATION.md").write_text("# Generation\n")
    return c


def brief(root: pathlib.Path, text: str = "A short walk; nobody fights.\n") -> pathlib.Path:
    b = root / "brief.md"
    b.write_text(text)
    return b


def run(campaign_dir: pathlib.Path, brief_file: pathlib.Path, engine: pathlib.Path = REPO, out: pathlib.Path | None = None):
    argv = [sys.executable, "-I", str(SCRIPT), str(campaign_dir), "--brief", str(brief_file), "--engine", str(engine)]
    if out is not None:
        argv += ["--out", str(out)]
    return subprocess.run(argv, capture_output=True, text=True)


@pytest.fixture
def rig(tmp_path):
    return campaign(tmp_path), brief(tmp_path)


def test_the_script_is_laid_out_in_play_order_not_file_order(rig):
    c, b = rig
    r = run(c, b)
    assert r.returncode == 0, r.stderr
    script = (c / ".out" / "script-review" / "round-1" / "script.md").read_text()
    hub, spoke, side, finale = (script.index(f"`quest/{q}`") for q in ("hub", "spoke", "side", "finale"))
    assert hub < spoke < side < finale
    assert script.index("Pull the lever") < script.index("Tell Anna")
    assert "*optional ·" in script[side:finale]


def test_a_talk_to_writes_out_the_tree_the_cast_shows_and_nothing_the_author_meant(rig):
    c, b = rig
    r = run(c, b)
    assert r.returncode == 0, r.stderr
    script = (c / ".out" / "script-review" / "round-1" / "script.md").read_text()
    assert "Hello, strangers." in script and "It is the only one." in script
    assert "(only if `flag/x`)" in script  # the gated option is shown with its gate
    assert "compass hidden" in script
    assert "before it is time: Stiff." in script
    assert "**A Stick**" in script and "remembers `flag/x`" in script
    assert "A note on the door." in script and "Mill" in script
    # Two placements showing the same barks are written once, their gates joined.
    assert script.count("- Busy.") == 1
    for sentinel in (DESIGN_SENTINEL, PERSONA_SENTINEL, HAPPENING_SENTINEL):
        assert sentinel not in script
    assert UNREACHED_SENTINEL not in script
    assert "2 of 3 node(s) rendered" in r.stderr
    assert "1 dialogue node(s) no cast root or stage-6 root reaches" in r.stderr


def test_the_handout_carries_the_brief_and_every_question_of_the_engine_page(rig):
    c, b = rig
    r = run(c, b)
    assert r.returncode == 0, r.stderr
    handout = (c / ".out" / "script-review" / "round-1" / "handout.md").read_text()
    assert "nobody fights" in handout
    page = PAGE.read_text()
    questions = [line for line in page.split("\n") if line.startswith("- **Q")]
    assert len(questions) >= 10
    for q in questions:
        head = q.split("**")[1]
        assert f"**{head}**" in handout
    assert "**Cited" not in handout and "**Authored" not in handout
    assert "](" not in handout.split("## The questions")[1].split("## The review")[0]
    assert "verdict: build" in handout
    assert f"{len(questions)} question(s) and 6 review step(s) read from" in r.stderr
    lines = r.stdout.strip().split("\n")
    assert lines[0].endswith("round-1/handout.md")
    assert lines[1] == f"Read and execute {lines[0]}. Report, then exit."


def test_rounds_are_counted_from_the_rounds_already_there(rig):
    c, b = rig
    assert run(c, b).returncode == 0
    r = run(c, b)
    assert r.returncode == 0
    assert (c / ".out" / "script-review" / "round-2" / "handout.md").is_file()
    assert "round 2:" in r.stderr


def test_out_writes_the_rounds_elsewhere_and_nothing_into_the_campaign(rig, tmp_path):
    c, b = rig
    out = tmp_path / "elsewhere"
    r = run(c, b, out=out)
    assert r.returncode == 0, r.stderr
    assert (out / "round-1" / "script.md").is_file()
    assert not (c / ".out").exists()


def test_a_campaign_without_step_5_is_refused(rig):
    c, b = rig
    (c / "dialogue.json").unlink()
    r = run(c, b)
    assert r.returncode == 1
    assert "dialogue.json" in r.stderr and "step 5" in r.stderr
    assert not (c / ".out").exists()


def test_a_missing_or_empty_brief_is_refused(rig, tmp_path):
    c, b = rig
    r = run(c, tmp_path / "nowhere.md")
    assert r.returncode == 2
    b.write_text("  \n")
    assert run(c, b).returncode == 2


def test_an_engine_page_that_yields_no_question_is_refused(rig, tmp_path):
    c, b = rig
    engine = tmp_path / "engine"
    page = engine / "docs" / "reference" / "script-review.md"
    page.parent.mkdir(parents=True)
    page.write_text(PAGE.read_text().replace("- **Q", "- **X"))
    r = run(c, b, engine=engine)
    assert r.returncode == 2
    assert "yields no question" in r.stderr
    page.unlink()
    r = run(c, b, engine=engine)
    assert r.returncode == 2
    assert "carries no script-review page" in r.stderr
