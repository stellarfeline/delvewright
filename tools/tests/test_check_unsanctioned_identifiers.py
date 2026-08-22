r"""Guards for `tools/check-unsanctioned-identifiers.py`.

The shape it exists to prevent is not a single bad line — it is REFILL. A sweep
took the repository from 1,751 unresolvable citations to a floor, and the count
had already grown back twice inside a week, because every merged branch carried
its own in and nothing looked. A one-off tidy of this class is a tidy that is
undone by the third merge after it.

So the tests below assert the two directions a ratchet has to hold, and the four
ways a green here could mean nothing:

  * a NEW citation of each kind reds (the direction the defect arrives from);
  * a REMOVED citation reds until the floor moves down with it, and is green the
    moment it does — the floor is a ceiling that falls, never an amnesty;
  * `--tighten` REFUSES to raise a number, so the one convenient way to turn a
    red green is closed;
  * a stale floor entry reds, because an exemption that has outlived its reason
    is how the next one gets waved through;
  * examining zero files reds, because a check that matched nothing must never
    look like a check that passed;
  * the things that merely LOOK like citations — a colour literal, a
    single-digit row reference, a plain date with nobody attached, an ordinary
    sentence about a decision — do not red, or the gate gets loosened by whoever
    hits the false positive first.

The last of those carries most of the weight for `role-attribution`, the kind
that answers the half of the rule the others do not: no repository artifact
records WHO decided something. A gate that reddened every sentence containing
the word *decision* would be worse than the gap it closes, so the person half
and the decision half of the cue must be BOUND to each other, and the tests
below drive both directions of that binding.
"""

from __future__ import annotations

import importlib.util
import shutil
import subprocess
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[2]
CHECKER = REPO / "tools" / "check-unsanctioned-identifiers.py"


def load(path: Path):
    spec = importlib.util.spec_from_file_location("cui", path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


class Tree:
    """A tiny git repository, because the gate reads `git ls-files`.

    Listing TRACKED files is the point: an untracked scratch file is not a
    repository artifact, and a gate that walked the filesystem would red on
    every worker's own notes.
    """

    def __init__(self, root: Path):
        self.root = root
        (root / "tools").mkdir(parents=True)
        self._git("init", "-q")
        self._git("config", "user.email", "t@example.com")
        self._git("config", "user.name", "t")

    def _git(self, *args: str) -> None:
        subprocess.run(["git", *args], cwd=self.root, check=True)

    def commit(self, files: dict[str, str]) -> None:
        for rel, body in files.items():
            p = self.root / rel
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(body, encoding="utf-8")
        self._git("add", "-A")
        self._git("commit", "-qm", "x")

    def checker(self, floor: dict[str, int] | None = None,
                allowed: dict[str, str] | None = None):
        dst = self.root / "tools" / "check-unsanctioned-identifiers.py"
        shutil.copy(CHECKER, dst)
        self._git("add", "-A")
        self._git("commit", "-qm", "checker", "--allow-empty")
        mod = load(dst)
        mod.SELF = dst.resolve()
        mod.REPO = self.root.resolve()
        mod.FLOOR = dict(floor or {})
        mod.ALLOWED = dict(allowed or {})
        return mod


@pytest.fixture
def tree(tmp_path):
    return Tree(tmp_path / "repo")


# ---------------------------------------------------------------- the floor --

def test_a_clean_tree_passes_and_states_what_it_examined(tree, capsys):
    tree.commit({"docs/a.md": "The compiler refuses an oversize region (DW0312).\n"})
    assert tree.checker().main([]) == 0
    out = capsys.readouterr().out
    assert "OK" in out
    assert "files examined" in out
    assert "bytes examined" in out


def test_a_new_task_id_is_a_finding(tree, capsys):
    tree.commit({"docs/a.md": "Mob placement (task #41) seats each mob on a cell.\n"})
    assert tree.checker().main([]) == 1
    out = capsys.readouterr().out
    assert "docs/a.md:1" in out
    assert "task-id" in out
    assert "task #41" in out


@pytest.mark.parametrize("line", [
    "Fixed in PR #361 and never regressed.\n",
    "This is the shape PRs #301/#302/#321 fixed thirteen times.\n",
    "A docs-only change reddened tier 2 (#388).\n",
    "The massing verbs landed in PR 3.\n",
])
def test_a_new_pull_request_number_is_a_finding(tree, capsys, line):
    tree.commit({"docs/a.md": line})
    assert tree.checker().main([]) == 1
    assert "pr-number" in capsys.readouterr().out


def test_a_dated_attribution_is_a_finding(tree, capsys):
    tree.commit({"docs/a.md": "`requires_item` is HELD (owner ruling, 2026-08-03).\n"})
    assert tree.checker().main([]) == 1
    out = capsys.readouterr().out
    assert "dated-attribution" in out
    assert "2026-08-03" in out


@pytest.mark.parametrize("line", [
    "The reset stays unconditional on purpose (owner ruling): a trigger fired.\n",
    "Aggro lock (owner directive, round 8): the hostile comes for the striker.\n",
    "A corridor-only piece exposes no wave seat (owner decision).\n",
    "The score RANKS and never gates, per the owner's plane ruling.\n",
    "A row may close with `owner-ruled` and a justification.\n",
    "The ceiling is never a target, because the owner has ruled so.\n",
    "She ruled it not a defect, so the row closes.\n",
    "The bonfire offers two options, by the ruling of the owner.\n",
])
def test_a_dateless_role_attribution_is_a_finding(tree, capsys, line):
    """The half the gate was blind to. The person cue existed only to tell an
    attributed date from a harmless one, so removing the date turned a finding
    green — and removing the date is precisely the wrong repair, because it
    deletes the half a reader could at least place."""
    tree.commit({"docs/a.md": line})
    assert tree.checker().main([]) == 1
    assert "role-attribution" in capsys.readouterr().out


def test_deleting_the_date_does_not_make_an_attribution_green(tree, capsys):
    """Both spellings of one sentence, in one tree: the dated citation is a
    finding for the date, the dateless one is a finding for the person."""
    tree.commit({
        "docs/a.md": "`requires_item` is HELD (owner ruling, 2026-08-03).\n",
        "docs/b.md": "`requires_item` is HELD (owner ruling).\n",
    })
    assert tree.checker().main([]) == 1
    out = capsys.readouterr().out
    assert "docs/a.md:1" in out and "dated-attribution" in out
    assert "docs/b.md:1" in out and "role-attribution" in out


def test_an_attribution_does_not_bind_across_a_blank_line(tree, capsys):
    """The same paragraph rule `is_attribution` holds. Two unrelated facts, one
    ending in a role and the next opening on a decision, are not an attribution
    — and a pattern that crossed the gap would red every page with a heading."""
    tree.commit({"docs/a.md": "The page the reviewer drives.\n\nDecisions live in an ADR.\n"})
    assert tree.checker().main([]) == 0
    assert "OK" in capsys.readouterr().out


def test_a_wrapped_attribution_is_still_one(tree, capsys):
    """Prose wraps, and a pattern clipped to a single line goes quiet on the
    commonest spelling of the thing it is looking for."""
    tree.commit({"docs/a.md": "The marker is the one piece of machinery allowed (owner\ndirective).\n"})
    assert tree.checker().main([]) == 1
    assert "role-attribution" in capsys.readouterr().out


def test_the_message_says_the_repair_is_to_state_the_rule_impersonally(tree, capsys):
    """A role attribution repaired by deleting the date is not repaired."""
    tree.commit({"docs/a.md": "Composition is art direction (owner ruling).\n"})
    assert tree.checker().main([]) == 1
    out = capsys.readouterr().out
    assert "impersonally" in out
    assert "Deleting the date is not the repair" in out


def test_a_task_id_is_one_citation_not_two(tree, capsys):
    """`task #41` must not also be billed as pull request 41."""
    tree.commit({"docs/a.md": "See task #41.\n"})
    mod = tree.checker()
    assert mod.main([]) == 1
    out = capsys.readouterr().out
    assert "1 occurrence(s)" in out


# ------------------------------------------------- what only LOOKS like one --

def test_a_plain_date_is_not_an_attribution(tree, capsys):
    """ADRs are the one place history legitimately lives, and a verified licence
    date is evidence a reader may need to re-check."""
    tree.commit({
        "docs/adr/0009-pin.md": "- **Date**: 2026-07-29\n\n1.21.11 shipped 2025-12-09.\n",
        "docs/ACKNOWLEDGEMENTS.md": "| gl-matrix | MIT | Licence verified 2026-08-14 |\n",
    })
    assert tree.checker().main([]) == 0
    assert "OK" in capsys.readouterr().out


@pytest.mark.parametrize("line", [
    # Every one of these is ordinary prose in this repository today.
    "Specs stay historical decision records; this file is what it does today.\n",
    "An architecture decision record is a sanctioned identifier.\n",
    "The pool name is a decision taken once, at the start.\n",
    "The campaign owner's id is stamped into the manifest.\n",
    "The reviewer opens the contact sheet and picks a candidate.\n",
    "The route planner ruled out a diagonal, so the grammar has none.\n",
    "A gate that binds to nothing has decided nothing.\n",
    "Flagged for the owner rather than decided here.\n",
    '"Read the far side too," she says.\n',
    "Her lantern is the only light in the rope room.\n",
])
def test_ordinary_prose_about_the_software_is_not_an_attribution(tree, capsys, line):
    """The half that is easy to skip. A gate that reds every use of the word
    *decision*, or every sentence with the word *owner* in it, is worse than the
    gap it closes: it teaches the people who see it that this red means nothing,
    and the first person to hit it loosens it. None of these pairs a person with
    a decision, so none of them is a finding."""
    tree.commit({"docs/a.md": line})
    assert tree.checker().main([]) == 0
    assert "OK" in capsys.readouterr().out


def test_a_date_inside_a_path_is_not_dated_by_a_nearby_plural(tree, capsys):
    """Measured, not assumed. The role kind's cue carries plurals because
    `owner decisions` is an attribution when the two words are bound. Carrying
    them in the DATED cue instead — where either half alone is enough — makes
    CLAUDE.md's own sentence a dated attribution on the strength of a date
    inside a path a reader can open. The plurals live only in the bound halves."""
    tree.commit({"CLAUDE.md":
                 "Founding decisions live in `docs/adr/` and originate from the\n"
                 "kickoff handoff (`docs/handoff-2026-07-29.md`).\n"})
    assert tree.checker().main([]) == 0
    assert "OK" in capsys.readouterr().out


def test_a_colour_literal_is_not_a_citation(tree, capsys):
    tree.commit({"page.css": "body { background: #313745; color: #1e1e1e; }\n"})
    assert tree.checker().main([]) == 0


def test_a_shorthand_colour_is_not_a_citation_either(tree, capsys):
    """`border: 1px solid #0006` was budgeted as pull request 6 by a permanent
    FLOOR entry — a false positive the gate paid to hide, which kept its own
    count green while one of the five things it counted was not a finding. The
    repair of a false positive is the exemption, never the floor."""
    tree.commit({"page.css": ".swatch { border: 1px solid #0006; outline: #f0f0; }\n"})
    assert tree.checker().main([]) == 0
    assert "OK" in capsys.readouterr().out


# The exemption above is the direction a checker gets routed around, so each of
# the four ways a citation could hide behind it is asserted to STILL be a
# finding. A rule keyed only to "a digit run of a colour's length" would have
# gone quiet on every one of them.

def test_the_colour_exemption_does_not_reach_a_stylesheet_comment(tree, capsys):
    """A pull-request reference written into a stylesheet HAS to sit in a
    comment or a string — CSS code has no syntax for prose. That is the property
    the defect cannot supply, and it is what makes this exemption safe."""
    tree.commit({"page.css": "/* the ground colour, fixed in #510 */\nbody { color: #313745; }\n"})
    assert tree.checker().main([]) == 1
    out = capsys.readouterr().out
    assert "page.css:1" in out
    assert "#510" in out


def test_the_colour_exemption_does_not_reach_a_stylesheet_string(tree, capsys):
    tree.commit({"page.css": '.note::after { content: "see #510"; color: #0006; }\n'})
    assert tree.checker().main([]) == 1
    assert "#510" in capsys.readouterr().out


def test_the_colour_exemption_does_not_leave_the_stylesheet(tree, capsys):
    """`#510` in prose is pull request 510, and `#313745` in prose is a citation
    too — neither file is a stylesheet, so neither is a colour."""
    tree.commit({"docs/a.md": "Fixed in #510.\n", "docs/b.md": "The panel is #313745.\n"})
    assert tree.checker().main([]) == 1
    out = capsys.readouterr().out
    assert "docs/a.md:1" in out
    assert "docs/b.md:1" in out


def test_a_digit_run_that_is_not_a_colours_length_stays_a_finding(tree, capsys):
    """`#18` is two digits. CSS hex colours are three, four, six or eight, so
    even in stylesheet code this is a citation."""
    tree.commit({"page.css": ".a { border-color: #18; }\n"})
    assert tree.checker().main([]) == 1
    assert "#18" in capsys.readouterr().out


def test_a_single_digit_reference_is_left_alone(tree, capsys):
    """Row numbers, list indices and numbered sections inside the document that
    writes them resolve for the reader; this repository has never numbered a
    pull request below ten."""
    tree.commit({"docs/a.md": "Because of row 1 the second consumer has no surface (#5).\n"})
    assert tree.checker().main([]) == 0


def test_a_scoreboard_holder_is_not_a_citation(tree, capsys):
    tree.commit({"docs/a.md": "The countdown is `#<id> dw.wave`; totals sit on `#wcen_n`.\n"})
    assert tree.checker().main([]) == 0


# ------------------------------------------------------------- the direction --

def test_a_removal_reds_until_the_floor_moves_and_is_green_when_it_does(tree, capsys):
    """The ratchet turning. A file floored at two, now carrying one, is a red
    that names the number to write — and green as soon as it is written."""
    tree.commit({"docs/a.md": "One citation left: task #41.\n"})
    assert tree.checker({"docs/a.md": 2}).main([]) == 1
    out = capsys.readouterr().out
    assert "lower it to 1" in out

    assert tree.checker({"docs/a.md": 1}).main([]) == 0
    assert "OK" in capsys.readouterr().out


def test_a_floor_is_not_a_licence_to_add_one_back(tree, capsys):
    tree.commit({"docs/a.md": "task #41 and task #42.\n"})
    assert tree.checker({"docs/a.md": 1}).main([]) == 1
    assert "floor 1" in capsys.readouterr().out


def test_an_allowlisted_file_may_carry_its_own_subject(tree, capsys):
    """The sibling gate's own fixtures: a test that proves a citation is
    rejected has to write one down."""
    tree.commit({"tools/tests/t.py": 'BAD = ["Fixed in task #157.", "Rewritten in PR #388."]\n'})
    assert tree.checker(allowed={"tools/tests/t.py": "fixtures for the sibling gate"}).main([]) == 0
    assert "allowlisted files ...... 1" in capsys.readouterr().out


def test_an_exemption_that_outlived_its_reason_is_a_finding(tree, capsys):
    tree.commit({"tools/tests/t.py": "nothing unresolvable here\n"})
    assert tree.checker(allowed={"tools/tests/t.py": "fixtures for the sibling gate"}).main([]) == 1
    assert "but carries none" in capsys.readouterr().out


def test_a_stale_floor_entry_is_a_finding(tree, capsys):
    tree.commit({"docs/a.md": "Nothing unresolvable here.\n"})
    assert tree.checker({"docs/a.md": 3}).main([]) == 1
    assert "delete the entry" in capsys.readouterr().out


def test_tighten_refuses_to_raise_a_floor(tree, capsys):
    """The one convenient way to turn this red green, closed."""
    tree.commit({"docs/a.md": "task #41 and task #42.\n"})
    mod = tree.checker({"docs/a.md": 1})
    before = mod.SELF.read_text(encoding="utf-8")
    assert mod.tighten() == 1
    assert "REFUSING to tighten" in capsys.readouterr().out
    assert mod.SELF.read_text(encoding="utf-8") == before


def test_tighten_refuses_to_floor_a_file_that_never_had_one(tree, capsys):
    tree.commit({"docs/new.md": "task #41\n"})
    mod = tree.checker({})
    assert mod.tighten() == 1
    assert "REFUSING to tighten" in capsys.readouterr().out


def test_tighten_writes_the_lowered_floor_back(tree, capsys):
    tree.commit({"docs/a.md": "One left: task #41.\n"})
    mod = tree.checker({"docs/a.md": 4})
    assert mod.tighten() == 0
    assert '"docs/a.md": 1,' in mod.SELF.read_text(encoding="utf-8")


# ------------------------------------------------------------------ vacuity --

def test_examining_no_files_is_a_finding(tree, capsys):
    """A gate that matched zero objects is not a pass."""
    mod = tree.checker()
    mod.tracked_files = lambda: []
    assert mod.main([]) == 2
    assert "examined nothing" in capsys.readouterr().out


def test_the_message_says_what_to_do_with_the_sentence(tree, capsys):
    """A finding that does not say how to repair it gets repaired the wrong way:
    the citation is swapped for a note about what it used to assert, which is a
    changelog on a page that should state the present tense."""
    tree.commit({"docs/a.md": "Mob placement (task #41).\n"})
    assert tree.checker().main([]) == 1
    out = capsys.readouterr().out
    assert "present-tense" in out
    assert "used to assert" in out
