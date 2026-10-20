"""Read the demo queue, `docs/demo-levels.md`, the way its reader reads it.

Two tools ask the queue a question. `tools/ci/check-demo-levels.py` asks which
mechanics have a row; `tools/creator/staging-gate.py` asks which rows name a
campaign as their level. Both read the one table under `## Mechanic demos`,
through `tools/lib/mdtable.py`, and both read it here.

A row names its level by the level's address in the content repository,
`campaigns/<id>`, in its Status cell (the last). A branch name is not an
address: a branch is where work sits until it merges, and a demo's directory is
what stays.
"""

from __future__ import annotations

import re

from lib import mdtable

__all__ = ["PATH", "HEADING", "read", "naming"]

PATH = "docs/demo-levels.md"
HEADING = "## Mechanic demos"


def read(text: str) -> tuple[list[tuple[int, tuple[str, ...]]], list[tuple[int, str]]]:
    """`(rows, orphans)` of the mechanic-demo table.

    Each row is `(line number in the file, its cells)`; each orphan is
    `(line number in the file, the stripped line)` — a row-shaped line no table
    contains, which a reader sees as a paragraph of literal pipes. A document
    with no `## Mechanic demos` heading has no rows and no orphans.
    """
    if HEADING not in text:
        return [], []
    before, body = text.split(HEADING, 1)
    # `body` resumes on the heading's own line, so line k of it is that line + k.
    heading_lineno = before.count("\n") + 1
    section = body.split("\n## ", 1)[0]
    rows, orphans = mdtable.read(section)
    return (
        [(heading_lineno + r.lineno - 1, r.cells) for r in rows],
        [(heading_lineno + n - 1, line) for n, line in orphans],
    )


def _address(campaign: str) -> re.Pattern[str]:
    # The whole path segment: `campaigns/the-ferry` is not `campaigns/the-ferry-bell`.
    return re.compile(rf"(?<![\w/.-])campaigns/{re.escape(campaign)}(?![\w-])")


def naming(text: str, campaign: str) -> list[int]:
    """The line numbers of the rows whose Status cell names `campaigns/<campaign>`."""
    rows, _ = read(text)
    pat = _address(campaign)
    return [lineno for lineno, cells in rows if cells and pat.search(cells[-1])]
