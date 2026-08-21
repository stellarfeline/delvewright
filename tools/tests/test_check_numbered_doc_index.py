"""Guards for `tools/check-numbered-doc-index.py`.

The defect is a COPY that nothing compares against its source. Seven of
thirty-seven spec rows disagreed with the spec file they point at, seven spec
files had no row at all, and `ADR-0020` was missing from the ADR index — while
link checking, `check-doc-dupes.py` and `check-numbered-doc-uniqueness.py` were
all green, because each of those asks about the table's SHAPE and none asks
whether what it SAYS is true.

So the tests below assert, in order: that each of the five rules fires on the
shape it exists for; that the diagnostic names the AUTHORITY, because a reader
who cannot tell which of two disagreeing statements is the source repairs the
wrong one half the time; that a document with no readable status is REPORTED
rather than filled in, since writing a status nobody ruled on forges the fact
the gate protects; that editorial dress is not a disagreement, or the gate reds
on correct prose and teaches people the red means nothing; and that a series
with nothing on either side is a red, not a pass.
"""

from __future__ import annotations

import importlib.util
import re
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[2]
CHECKER = REPO / "tools" / "check-numbered-doc-index.py"


def load():
    spec = importlib.util.spec_from_file_location("cndi", CHECKER)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


SPEC_SERIES = {
    "name": "spec",
    "dir": "docs/specs",
    "index": "docs/specs/README.md",
    "file": re.compile(r"^spec-(\d{4})-.+\.md$"),
    "row": re.compile(r"^\|\s*\[spec-(\d{4})\]\(([^)]+)\)\s*\|"),
    "label": lambda num: f"spec-{num}",
    "section": re.compile(r"(?i)^#{2,6}\s.*\bacceptance criteria\b"),
}

# Rule 6's obligation, appended to every fixture spec that is not itself about
# rule 6 — a fixture testing status parsing must not also red for a missing
# section, or every assertion below would be about two rules at once.
CRITERIA = "\n## Acceptance criteria\n\n1. It builds.\n"


class Tree:
    def __init__(self, root: Path):
        self.root = root
        (root / "docs" / "specs").mkdir(parents=True)

    def doc(self, name: str, body: str) -> None:
        (self.root / "docs" / "specs" / name).write_text(body, encoding="utf-8")

    def spec(self, num: str, slug: str, status: str) -> str:
        name = f"spec-{num}-{slug}.md"
        self.doc(name, f"# spec-{num}: {slug}\n\n- **Status**: {status}\n\n"
                       f"## Context\n{CRITERIA}")
        return name

    def index(self, rows: list[str]) -> None:
        head = "# Specs\n\n| Spec | Title | Status |\n|------|-------|--------|\n"
        (self.root / "docs" / "specs" / "README.md").write_text(
            head + "".join(r.rstrip() + "\n" for r in rows), encoding="utf-8"
        )

    def checker(self):
        mod = load()
        mod.ROOT = self.root
        mod.SERIES = (dict(SPEC_SERIES),)
        return mod


@pytest.fixture
def tree(tmp_path):
    return Tree(tmp_path / "repo")


def row(num: str, slug: str, status: str, *, target: str | None = None) -> str:
    return f"| [spec-{num}]({target or f'spec-{num}-{slug}.md'}) | {slug} | {status} |"


# ----------------------------------------------------------------- it binds --

def test_an_index_that_tells_the_truth_passes_and_states_its_binding(tree, capsys):
    tree.spec("0001", "alpha", "Accepted")
    tree.spec("0002", "beta", "Draft")
    tree.index([row("0001", "alpha", "Accepted"), row("0002", "beta", "Draft")])
    assert tree.checker().main() == 0
    out = capsys.readouterr().out
    assert "OK" in out
    assert ("spec: 2 document(s), 2 row(s), 2 status(es) compared, "
            "2 acceptance-criteria section(s)") in out


def test_a_series_with_nothing_on_either_side_is_a_finding(tree, capsys):
    """A green gate that binds to nothing is vacuous, not a pass."""
    tree.index([])
    mod = tree.checker()
    assert mod.main() == 2
    captured = capsys.readouterr()
    assert "VACUOUS" in captured.out
    assert "binds to nothing" in captured.err


def test_a_missing_index_is_a_finding_not_a_quiet_pass(tree, capsys):
    tree.spec("0001", "alpha", "Accepted")
    assert tree.checker().main() == 2
    assert "NO INDEX" in capsys.readouterr().out


# ------------------------------------------------- where the index ends --

def test_a_row_detached_from_the_index_table_is_a_finding(tree, capsys):
    """An index is navigation, and a blank line ends a pipe table — so a row
    below one renders as a paragraph of literal pipe characters and sends no
    reader anywhere. It used to count as an entry, which made "every document is
    indexed" true for the gate and false on the page."""
    tree.spec("0001", "alpha", "Accepted")
    tree.spec("0002", "beta", "Draft")
    tree.index([row("0001", "alpha", "Accepted"), "", row("0002", "beta", "Draft")])
    assert tree.checker().main() == 1
    err = capsys.readouterr().err
    assert "docs/specs/README.md:7 is an index row that no table contains" in err
    # And the document it should have indexed is now reported as unindexed,
    # which is the whole point: the obligation is unmet, not met invisibly.
    assert "spec-0002" in err


def test_the_same_row_attached_is_an_entry(tree, capsys):
    """The other direction, so the red above is about the blank line alone."""
    tree.spec("0001", "alpha", "Accepted")
    tree.spec("0002", "beta", "Draft")
    tree.index([row("0001", "alpha", "Accepted"), row("0002", "beta", "Draft")])
    assert tree.checker().main() == 0


# ------------------------------------------------------------- the five rules --

def test_a_row_that_disagrees_with_its_document_is_a_finding(tree, capsys):
    tree.spec("0040", "map-composition", "Accepted")
    tree.index([row("0040", "map-composition", "Proposed")])
    assert tree.checker().main() == 1
    err = capsys.readouterr().err
    assert "row spec-0040 says 'Proposed'" in err
    assert "says 'Accepted'" in err


def test_the_diagnostic_names_which_file_is_the_authority(tree, capsys):
    """Without this the reader repairs the wrong one half the time."""
    tree.spec("0040", "map-composition", "Accepted")
    tree.index([row("0040", "map-composition", "Proposed")])
    assert tree.checker().main() == 1
    err = capsys.readouterr().err
    assert "The document is the authority and the index is the copy" in err
    assert "edit docs/specs/README.md to read 'Accepted'" in err


def test_a_document_with_no_row_is_a_finding(tree, capsys):
    """`spec-0039`, the gallery, was absent from the index for as long as it
    existed, and the index is the only place the set is enumerated."""
    tree.spec("0001", "alpha", "Accepted")
    tree.spec("0039", "gallery-campaign", "Accepted")
    tree.index([row("0001", "alpha", "Accepted")])
    assert tree.checker().main() == 1
    err = capsys.readouterr().err
    assert "spec-0039-gallery-campaign.md has no row" in err
    assert "Add a row stating 'Accepted'." in err


def test_a_row_pointing_at_a_document_that_is_gone_is_a_finding(tree, capsys):
    tree.spec("0001", "alpha", "Accepted")
    tree.index([row("0001", "alpha", "Accepted"),
                row("0002", "beta", "Draft")])
    assert tree.checker().main() == 1
    assert "does not exist" in capsys.readouterr().err


def test_a_row_whose_number_does_not_match_its_link_is_a_finding(tree, capsys):
    """It reads correctly and sends the reader elsewhere."""
    tree.spec("0031", "runtime-state", "Draft")
    tree.spec("0032", "economy", "Draft")
    tree.index([row("0031", "runtime-state", "Draft", target="spec-0032-economy.md"),
                row("0032", "economy", "Draft")])
    assert tree.checker().main() == 1
    err = capsys.readouterr().err
    assert "a different document" in err
    assert "sends the reader elsewhere" in err


# -------------------------------------------- a status is reported, not forged --

def test_a_document_with_no_status_field_is_reported_and_never_filled_in(tree, capsys):
    tree.doc("spec-0001-alpha.md", f"# spec-0001: alpha\n\n## Context\n{CRITERIA}")
    tree.index([row("0001", "alpha", "Accepted")])
    assert tree.checker().main() == 1
    err = capsys.readouterr().err
    assert "no `Status:` field in its header" in err
    assert "Do NOT invent one" in err


def test_a_status_word_nobody_uses_is_a_finding(tree, capsys):
    tree.spec("0001", "alpha", "Ratified")
    tree.index([row("0001", "alpha", "Ratified")])
    assert tree.checker().main() == 1
    err = capsys.readouterr().err
    assert "no readable status" in err
    assert "Proposed, Draft, Accepted, Approved, Implemented, Superseded" in err


# ------------------------------------------- what only LOOKS like a disagreement --

def test_an_editorial_gloss_is_not_a_disagreement(tree, capsys):
    """`Accepted (M4)` against `Accepted (implementation deferred to M4)` is one
    status wearing two coats. A gate that reds on correct prose teaches the
    people who see it that this red means nothing."""
    tree.doc("spec-0014-creator.md",
             "# spec-0014\n\n- **Status**: Accepted (implementation deferred to M4)\n"
             + CRITERIA)
    tree.index(["| [spec-0014](spec-0014-creator.md) | Creator | Accepted (M4) |"])
    assert tree.checker().main() == 0
    assert "OK" in capsys.readouterr().out


def test_the_first_recognised_word_is_the_status(tree, capsys):
    """`Draft, design approved (three rules …)` is a draft whose direction was
    approved; reading the last word would call it approved."""
    tree.doc("spec-0019-rehearsal.md",
             "# spec-0019\n\n- **Status**: Draft, design approved (three rules)\n"
             + CRITERIA)
    tree.index([row("0019", "rehearsal", "Draft", target="spec-0019-rehearsal.md")])
    assert tree.checker().main() == 0


@pytest.mark.parametrize("field", [
    "- **Status**: Accepted",
    "Status: **Accepted**",
    "- Status: Accepted",
])
def test_every_live_spelling_of_the_status_field_is_read(tree, capsys, field):
    """A pattern that admitted only the commonest spelling read two ordinary
    formatting variants as documents with no status at all — a measurement
    asking about the wrong key and getting an honest answer."""
    tree.doc("spec-0010-relight.md",
             f"# spec-0010\n\n{field}\n\n## Context\n{CRITERIA}")
    tree.index([row("0010", "relight", "Accepted", target="spec-0010-relight.md")])
    assert tree.checker().main() == 0


def test_a_later_sections_status_line_does_not_speak_for_the_document(tree, capsys):
    """`spec-0002` states `**Status: Implemented**` again inside a section about
    one milestone's slice of the work; the header speaks for the whole."""
    tree.doc(
        "spec-0002-cli.md",
        "# spec-0002\n\n- **Status**: Approved\n\n## Verbs\n\n"
        "**Status: Implemented** for the branching half.\n" + CRITERIA,
    )
    tree.index([row("0002", "cli", "Approved", target="spec-0002-cli.md")])
    assert tree.checker().main() == 0


# ------------------------------------- every spec carries its acceptance criteria --

def test_a_spec_without_an_acceptance_criteria_section_is_a_finding(tree, capsys):
    """The obligation lived in `docs/specs/README.md` and CLAUDE.md as a doc
    line — true-by-declaration while four specs carried no section, with
    nothing comparing the sentence to the corpus."""
    tree.doc("spec-0016-souls.md",
             "# spec-0016\n\n- **Status**: Draft\n\n## Mechanics\n")
    tree.index([row("0016", "souls", "Draft", target="spec-0016-souls.md")])
    assert tree.checker().main() == 1
    err = capsys.readouterr().err
    assert "spec-0016-souls.md has no Acceptance criteria section" in err
    assert "machine-checkable" in err


def test_a_spec_with_no_index_row_still_owes_its_criteria(tree, capsys):
    """Rule 6 binds the document set, not the indexed set — a spec nobody
    indexed is exactly the one nothing else is looking at."""
    tree.spec("0001", "alpha", "Draft")
    tree.doc("spec-0002-beta.md", "# spec-0002\n\n- **Status**: Draft\n\n## Body\n")
    tree.index([row("0001", "alpha", "Draft")])
    assert tree.checker().main() == 1
    assert "spec-0002-beta.md has no Acceptance criteria section" in capsys.readouterr().err


@pytest.mark.parametrize("heading", [
    "## Acceptance criteria",
    "## 4. Acceptance criteria — each stating what would make it vacuous",
    "### Acceptance criteria (v0.3)",
    "## Validation / acceptance criteria",
])
def test_every_live_dressing_of_the_section_heading_counts(tree, capsys, heading):
    """The corpus numbers, subtitles and parenthesises the heading; a pattern
    admitting only the plain spelling would red half the sectioned specs."""
    tree.doc("spec-0001-alpha.md",
             f"# spec-0001\n\n- **Status**: Draft\n\n{heading}\n\n1. It builds.\n")
    tree.index([row("0001", "alpha", "Draft")])
    assert tree.checker().main() == 0


def test_a_prose_mention_is_not_a_section(tree, capsys):
    """Criteria live in a section a reader lands on; a sentence that promises
    them is the doc-line shape this rule exists to end."""
    tree.doc("spec-0001-alpha.md",
             "# spec-0001\n\n- **Status**: Draft\n\n## Plan\n\n"
             "The acceptance criteria will be written once the surface lands.\n")
    tree.index([row("0001", "alpha", "Draft")])
    assert tree.checker().main() == 1
    assert "no Acceptance criteria section" in capsys.readouterr().err


def test_a_series_that_declares_no_section_obligation_is_not_subject(tree, capsys):
    """ADRs record decisions and owe no acceptance criteria — the rule keys off
    the series' own `section` declaration, never off every series."""
    tree.doc("spec-0001-alpha.md",
             "# spec-0001\n\n- **Status**: Draft\n\n## Body\n")
    tree.index([row("0001", "alpha", "Draft")])
    mod = tree.checker()
    series = dict(SPEC_SERIES)
    del series["section"]
    mod.SERIES = (series,)
    assert mod.main() == 0
    assert "acceptance-criteria" not in capsys.readouterr().out

def test_the_repositorys_own_indexes_agree_with_their_documents(capsys):
    """The gate over the real `docs/specs` and `docs/adr`. This is the binding
    that matters: the two tests above prove the rules fire, and this one proves
    they are pointed at something."""
    mod = load()
    assert mod.main() == 0
    out = capsys.readouterr().out
    assert "OK" in out
    assert "VACUOUS" not in out
