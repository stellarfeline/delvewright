"""Read a markdown pipe table the way the page that shows it is read.

## The defect this exists to end

Seven gates in `tools/` counted rows out of a markdown table, and every one of
them did it by matching a regular expression against lines — `startswith("|")`,
or a row pattern applied to the whole file, or to a section sliced at its
heading. None of them knew what a table was, so none of them could tell a row
inside one from a line of prose that happens to be full of pipe characters.

A blank line ends a pipe table. CommonMark's GFM table extension says a table
runs from its header row, through its delimiter row, to "the first empty line,
or beginning of another block-level structure" — so a row separated from its
table by one blank line renders as a paragraph of literal pipes, and every human
and every renderer sees it as prose. To those seven gates it was still a row.

That gap is not cosmetic, because those gates are what makes obligations bite.
A detached row still documented a diagnostic, still indexed a spec, still
declared what bounded a trial's verdict, still fenced a schema field — in the
letter, and for nobody who reads the page. Measured by perturbation on the live
documents: detaching the last row of its table left `check-dw-codes`,
`check-numbered-doc-index`, `check-trial-verdicts`, `check-grammar-ir-compat`,
`check-capability-ownership`, `check-stated-counts` and `check-reference-versions`
green, and pandoc's GFM reader drew the same row as a paragraph.

## Why one reader and not seven repairs

A general mechanism privately re-implemented at each site is the shape this
repository refuses by name: every proof and every diagnostic written for one
copy silently does not cover the others, and the copies drift. So the rule lives
here once, and a gate keeps its own row pattern and asks this module only the
question it cannot answer for itself — *is this line in a table at all*.

## What it implements, and where it stops

`read()` walks the document once and returns the body rows of every pipe table,
plus the lines that look like rows and belong to no table. A table is opened by
a header row directly above a delimiter row of the same cell count (both
conditions are the spec's; failing either means there is no table, which is why
a mistyped delimiter turns every row of a would-be table into an orphan rather
than into silence). It is closed by a blank line, by any block-level start, or
by the end of the document.

Fenced code is skipped, because a fence's contents are not markdown — a `|` in
a shell transcript is not a table row, and the reference documents are full of
them.

Not implemented, stated rather than assumed: indented (four-space) code blocks
are not recognised, so a pipe line indented four spaces is read as ordinary
text and, if it is not in a table, reported as an orphan. No document in this
repository does that, and the failure direction is a visible finding rather
than a silent pass.
"""

from __future__ import annotations

import re
from dataclasses import dataclass

__all__ = ["Row", "read", "body_rows", "orphans", "cells"]

#: A GFM delimiter cell: hyphens, optionally colon-anchored at either end.
DELIMITER_CELL = re.compile(r"^:?-+:?$")

#: A fence opener or closer. Its contents are not markdown and hold no tables.
FENCE = re.compile(r"^\s{0,3}(`{3,}|~{3,})")

#: The block-level starts that end a table. None of them can begin with `|`,
#: so no row of a real table is ever mistaken for one.
BLOCK_START = re.compile(
    r"""^(?:
          \#{1,6}(?:\s|$)          # ATX heading
        | >                        # block quote
        | (?:```|~~~)              # fenced code
        | (?:[-*_][ \t]*){3,}$     # thematic break
        | [-*+](?:\s|$)            # bullet list item
        | [0-9]{1,9}[.)](?:\s|$)   # ordered list item
        | <[A-Za-z/!?]             # HTML block
    )""",
    re.VERBOSE,
)

#: What a reader would call a table row on sight: a line that starts with a
#: pipe. It is the shape the seven gates were matching, so it is the shape whose
#: absence from any table has to be reportable.
ROW_SHAPED = re.compile(r"^\s{0,3}\|")


def cells(line: str) -> list[str]:
    """The cells of one row, outer pipes dropped and each cell stripped."""
    return [c.strip() for c in line.strip().strip("|").split("|")]


@dataclass(frozen=True)
class Row:
    """One body row of one table. `lineno` is 1-based, into the whole text."""

    lineno: int
    line: str
    cells: tuple[str, ...]
    header: tuple[str, ...]


def _is_delimiter(line: str, width: int) -> bool:
    line = line.strip()
    if "|" not in line:
        return False
    parts = cells(line)
    return len(parts) == width and all(DELIMITER_CELL.match(c) for c in parts)


def read(text: str) -> tuple[list[Row], list[tuple[int, str]]]:
    """`(body rows of every table, row-shaped lines no table contains)`.

    Each orphan is `(1-based line number, the stripped line)`.
    """
    lines = text.split("\n")
    rows: list[Row] = []
    detached: list[tuple[int, str]] = []
    fence: str | None = None
    i = 0
    while i < len(lines):
        raw = lines[i]
        opener = FENCE.match(raw)
        if fence is not None:
            if opener and opener.group(1)[0] == fence[0] and len(opener.group(1)) >= len(fence):
                fence = None
            i += 1
            continue
        if opener:
            fence = opener.group(1)
            i += 1
            continue

        line = raw.strip()
        if not line or "|" not in line:
            i += 1
            continue

        header = cells(line)
        if i + 1 < len(lines) and _is_delimiter(lines[i + 1], len(header)):
            head = tuple(header)
            i += 2
            while i < len(lines):
                body = lines[i]
                stripped = body.strip()
                if not stripped or BLOCK_START.match(stripped) or FENCE.match(body):
                    break
                rows.append(Row(i + 1, body, tuple(cells(stripped)), head))
                i += 1
            continue

        if ROW_SHAPED.match(raw):
            detached.append((i + 1, line))
        i += 1
    return rows, detached


def body_rows(text: str) -> list[Row]:
    return read(text)[0]


def orphans(text: str) -> list[tuple[int, str]]:
    return read(text)[1]
