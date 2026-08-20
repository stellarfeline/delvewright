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
    single-digit row reference, a plain date with nobody attached — do not red,
    or the gate gets loosened by whoever hits the false positive first.
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
