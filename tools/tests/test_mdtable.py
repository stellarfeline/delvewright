"""Guards for `tools/lib/mdtable.py`.

The red this exists to prevent: a gate counts rows out of a markdown table by a
rule the markdown renderer does not use, so an obligation is met in the letter
and void in the reading. The half most likely to rot silently is the agreement
with the renderer, so the last test binds this module to every tracked markdown
file in the repository and compares its row count against `pandoc`'s GFM reader
— a second instrument whose failure mode is unrelated to this one's. It skips
when pandoc is absent rather than passing quietly about nothing.
"""

from __future__ import annotations

import re
import shutil
import subprocess
import sys
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "tools"))

from lib import mdtable  # noqa: E402

HEAD = "| A | B |\n| --- | --- |\n"


def test_a_plain_table_yields_its_body_rows():
    rows, orphans = mdtable.read(HEAD + "| 1 | 2 |\n| 3 | 4 |\n")
    assert [r.cells for r in rows] == [("1", "2"), ("3", "4")]
    assert [r.lineno for r in rows] == [3, 4]
    assert rows[0].header == ("A", "B")
    assert orphans == []


def test_a_blank_line_ends_the_table_and_the_row_below_is_an_orphan():
    rows, orphans = mdtable.read(HEAD + "| 1 | 2 |\n\n| 3 | 4 |\n")
    assert [r.cells for r in rows] == [("1", "2")]
    assert orphans == [(5, "| 3 | 4 |")]


def test_a_heading_ends_the_table():
    rows, orphans = mdtable.read(HEAD + "| 1 | 2 |\n## Next\n| 3 | 4 |\n")
    assert [r.cells for r in rows] == [("1", "2")]
    assert orphans == [(5, "| 3 | 4 |")]


def test_a_list_item_ends_the_table():
    rows, orphans = mdtable.read(HEAD + "| 1 | 2 |\n- a bullet\n")
    assert [r.cells for r in rows] == [("1", "2")]
    assert orphans == []


def test_pipes_inside_a_fence_are_not_rows():
    text = "```\n| not | a table |\n| --- | --- |\n| still | not |\n```\n"
    rows, orphans = mdtable.read(text)
    assert rows == []
    assert orphans == []


def test_a_fence_ends_a_table_it_follows():
    rows, orphans = mdtable.read(HEAD + "| 1 | 2 |\n```\n| 3 | 4 |\n```\n")
    assert [r.cells for r in rows] == [("1", "2")]
    assert orphans == []


def test_a_delimiter_whose_width_disagrees_opens_no_table():
    """GFM: the header and delimiter rows must have the same cell count. The
    failure direction matters — every line becomes a reportable orphan, never a
    silently kept row."""
    rows, orphans = mdtable.read("| A | B | C |\n| --- | --- |\n| 1 | 2 | 3 |\n")
    assert rows == []
    assert [ln for ln, _ in orphans] == [1, 2, 3]


def test_colon_anchored_delimiters_are_delimiters():
    rows, _ = mdtable.read("| A | B |\n| :-- | --: |\n| 1 | 2 |\n")
    assert [r.cells for r in rows] == [("1", "2")]


def test_a_second_table_under_the_same_heading_is_a_table():
    rows, orphans = mdtable.read(HEAD + "| 1 | 2 |\n\n| C | D |\n| --- | --- |\n| 3 | 4 |\n")
    assert [r.cells for r in rows] == [("1", "2"), ("3", "4")]
    assert orphans == []


def test_a_line_of_prose_with_a_pipe_is_neither_a_row_nor_an_orphan():
    """Only a line a reader would call a row on sight — one that starts with a
    pipe — can be an orphan. Prose mentioning `a | b` is not a claim about a
    table and must not be reported as one."""
    rows, orphans = mdtable.read("Some prose about a | b in passing.\n")
    assert rows == []
    assert orphans == []


def test_the_live_repository_has_no_detached_rows():
    """Bound to the real documents. A detached row here is a live defect: the
    diagnostics catalog carried twenty-one of them, and the gate that reads it
    counted every one as a documented diagnostic."""
    files = subprocess.run(
        ["git", "-C", str(REPO), "ls-files", "*.md"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout.split()
    assert len(files) > 50, len(files)
    found = {}
    for rel in files:
        _, orphans = mdtable.read((REPO / rel).read_text(encoding="utf-8", errors="replace"))
        if orphans:
            found[rel] = orphans
    assert not found, found


@pytest.mark.skipif(shutil.which("pandoc") is None, reason="pandoc not installed")
def test_the_row_count_agrees_with_pandocs_gfm_reader():
    """The cross-check that makes the rest of this file mean something: a second
    reader, written by other people against the same specification, must put the
    same number of rows in the same files."""
    files = subprocess.run(
        ["git", "-C", str(REPO), "ls-files", "*.md"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout.split()
    disagreements = []
    mine = theirs = 0
    for rel in files:
        rows, _ = mdtable.read((REPO / rel).read_text(encoding="utf-8", errors="replace"))
        html = subprocess.run(
            ["pandoc", "-f", "gfm", "-t", "html", rel],
            cwd=REPO,
            capture_output=True,
            text=True,
            check=True,
        ).stdout
        n = sum(b.count("<tr") for b in re.findall(r"<tbody>(.*?)</tbody>", html, re.S))
        mine += len(rows)
        theirs += n
        if n != len(rows):
            disagreements.append((rel, len(rows), n))
    assert mine > 1000, mine  # a zero agreement is not an agreement
    assert not disagreements, disagreements
    assert mine == theirs, (mine, theirs)
