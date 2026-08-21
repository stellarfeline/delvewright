#!/usr/bin/env python3
"""An index of numbered documents agrees with the documents it indexes.

## The defect this exists to end

`docs/specs/README.md` and `docs/adr/README.md` are tables with a status
column. Neither is the authority for a status — each document's own `Status:`
line is — so both tables are COPIES, and a copy nothing compares against its
source drifts the moment either side is edited alone. Measured before this gate
existed: seven of thirty-seven spec rows disagreed with the spec file they point
at (three said `Proposed` for a spec the file called `Accepted`, three said
`Approved` for a spec the file called `Draft`), seven spec files had no row at
all — including `spec-0039`, the gallery — and `ADR-0020` was missing from the
ADR index. Nothing in `tools/` opened either file.

The drift is invisible to every other gate. Link checking passes: the links are
all valid. `check-doc-dupes.py` passes: no key is repeated. `check-numbered-doc-
uniqueness.py` passes: no number collides. Each is asking about the table's
SHAPE; none asks whether what the table SAYS is true.

A hand-correction of the tables is not a fix. It is this project's UNRUN shape —
correct on the day it lands, stale at the next merge, with nothing that could
say so. The check comes first; the correction is what makes the check pass.

## What is the authority, and what is the copy

The **document's own `Status:` line is the authority**. The index row is the
copy. Every diagnostic below says which file to edit, because a reader who
cannot tell which of two disagreeing statements is the source repairs the wrong
one half the time.

The one exception is a document with no readable status: that is reported and
**never repaired by this tool or by the reader of its output**. Writing a status
into a document nobody ruled on invents the fact the gate exists to protect.

## The six rules, over every series in `SERIES`

1. **Every document has a row.** A spec or ADR absent from its index is
   invisible to anyone reading the index, which is the only place the set is
   enumerated.
2. **Every row points at a document that exists.** A row whose link target is
   gone is a reference a reader cannot resolve.
3. **Every row's number matches its link target's number.** `[spec-0031](spec-0032-….md)`
   is a copy-paste that reads correctly and sends the reader elsewhere.
4. **Every row's status equals the document's own.** Compared as the STATUS,
   not as the cell text: a parenthetical gloss (`Accepted (M4)` against
   `Accepted (implementation deferred to M4)`) is editorial and is not a
   disagreement. Two different status WORDS are.
5. **Every document's status is readable.** A `Status:` field in the document's
   header block whose value opens with a recognised word.
6. **Every spec carries an Acceptance criteria section** (spec series only —
   the series entry that declares a `section` pattern is the one this rule
   binds). The obligation is stated by `docs/specs/README.md`'s own preamble
   and by CLAUDE.md's Conventions line, and until this rule existed it was a
   doc line, which is not an invocation: the sentence was true-by-declaration
   while four specs carried no such section. A heading containing the phrase
   counts, however it is numbered or dressed (`## Acceptance criteria`,
   `## 4. Acceptance criteria — …`, `### Acceptance criteria (v0.3)`); a
   mention in prose does not — criteria live in a section a reader lands on.
   The rule demands the section, not its quality: whether a criterion is
   machine-checkable stays with review, and a subject that makes a criterion
   genuinely unwritable by machine is answered inside the section, in one
   sentence, with what a human checks instead — never by an exemption here.

## Vocabulary, and what this gate does NOT rule on

`STATUSES` is the set of words that may appear. It is descriptive: it is what
CLAUDE.md states for ADRs (`Proposed` / `Accepted` / `Superseded`), plus the
words the spec corpus actually uses (`Draft`, `Approved`, `Implemented`). This
gate takes no position on which lifecycle is right, on whether `Approved` and
`Accepted` should be one word, or on what order the words come in — those are
decisions, and a checker is not where a decision gets made. It refuses exactly
two things: a word no document has ever used, and a copy that disagrees with its
source.

Adding a word is therefore a diff to a checker, which is never a mechanical
change — the same reason `check-unsanctioned-identifiers.py` keeps its floor in
its own source.

## Binding count

Every run prints, per series, documents examined, rows examined and statuses
compared — and, where rule 6 binds, acceptance-criteria sections found, so a
green states how many specs it actually proved sectioned. A series with zero
documents or zero rows is a FAIL, not a pass
(CLAUDE.md: a green gate that binds to nothing is VACUOUS) — the directory
moved, was renamed, or the pattern stopped matching.

## Where it is bound

A step of the existing `docs (local link check)` job, on every push and every
pull request. NOT a new job: branch protection matches a required context by its
name string, so a new job either blocks every pull request until it is
registered or is advisory, and an advisory job is a job that does not gate.

Deterministic, offline, stdlib-only python3. Run from anywhere:
    python3 tools/check-numbered-doc-index.py
Exit 0 = the indexes tell the truth, 1 = findings, 2 = examined nothing.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from lib import mdtable  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent

# The words a status may be. Descriptive, not prescriptive — see the docstring.
STATUSES = ("Proposed", "Draft", "Accepted", "Approved", "Implemented", "Superseded")
STATUS_WORD = re.compile(r"(?i)(?<![\w-])(" + "|".join(STATUSES) + r")(?![\w-])")

# A `Status:` field, however it is dressed: `- **Status**: X`, `Status: **X**`,
# `- Status: X`. All three spellings are live in `docs/`, and a pattern that
# admitted only the commonest one reads two ordinary formatting variants as
# documents with no status at all — which is a measurement asking about the
# wrong key and getting an honest answer.
STATUS_FIELD = re.compile(r"^\s*(?:[-*+]\s*)?\*{0,2}Status\*{0,2}\s*:\s*(.*)$")

# The header block a status field lives in: everything before the document's
# first `##` section. `spec-0002` states `**Status: Implemented**` again inside a
# later section about one milestone's slice of the work; that is prose about a
# part, and the header is what speaks for the whole.
SECTION = re.compile(r"^##\s")

# A heading whose text contains the phrase, however numbered or dressed. It is
# anchored to a heading marker, so prose that merely mentions the phrase cannot
# satisfy rule 6 — and if this pattern ever stops matching, every spec reds at
# once rather than the rule going silently green.
ACCEPTANCE_SECTION = re.compile(r"(?i)^#{2,6}\s.*\bacceptance criteria\b")

# One entry per numbered series that carries an index with a status column.
# A third series is one entry here, not a second script. A series with a
# `section` key is additionally subject to rule 6; the ADR series carries none,
# because an ADR records a decision and owes no acceptance criteria.
SERIES = (
    {
        "name": "spec",
        "dir": "docs/specs",
        "index": "docs/specs/README.md",
        "file": re.compile(r"^spec-(\d{4})-.+\.md$"),
        "row": re.compile(r"^\|\s*\[spec-(\d{4})\]\(([^)]+)\)\s*\|"),
        "label": lambda num: f"spec-{num}",
        "section": ACCEPTANCE_SECTION,
    },
    {
        "name": "adr",
        "dir": "docs/adr",
        "index": "docs/adr/README.md",
        "file": re.compile(r"^(\d{4})-.+\.md$"),
        "row": re.compile(r"^\|\s*\[(\d{4})\]\(([^)]+)\)\s*\|"),
        "label": lambda num: f"ADR-{num}",
    },
)


def strip_markup(cell: str) -> str:
    text = re.sub(r"\[([^\]]*)\]\([^)]*\)", r"\1", cell)
    text = text.replace("`", "").replace("*", "").replace("_", "")
    return re.sub(r"\s+", " ", text).strip()


def status_word(text: str) -> str | None:
    """The status a piece of text states, or None.

    The FIRST recognised word wins. `Draft, design approved (three rules …)` is
    a draft whose direction was approved, and reading the last word would call
    it approved; `Proposed (strategy approved — …)` is the same shape. The
    words after the first are prose about the status, not the status.
    """
    m = STATUS_WORD.search(text)
    return m.group(1).capitalize() if m else None


def document_status(path: Path) -> tuple[int, str, str | None] | None:
    """(line number, raw field value, status word) for the header `Status:`."""
    for n, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        if SECTION.match(line):
            return None
        m = STATUS_FIELD.match(line)
        if m:
            raw = strip_markup(m.group(1))
            return n, raw, status_word(raw)
    return None


def index_rows(
    path: Path, row_re: re.Pattern
) -> tuple[list[tuple[int, str, str, str]], list[tuple[int, str]]]:
    """(line number, number, link target, status cell) per body row, and the
    index rows no table contains.

    An index is navigation: its whole value is that a reader opens the page and
    finds the document. A blank line ends a pipe table, so a row under one
    renders as a paragraph of literal pipe characters — present for a gate
    matching a regex against lines, absent for everyone the index is for. The
    table is therefore read by `tools/lib/mdtable.py`, and a detached row is a
    finding rather than a silently accepted entry.
    """
    rows: list[tuple[int, str, str, str]] = []
    in_table, detached = mdtable.rows_matching(
        path.read_text(encoding="utf-8"), row_re
    )
    for row in in_table:
        m = row_re.match(row.line.strip())
        rows.append(
            (row.lineno, m.group(1), m.group(2), strip_markup(row.cells[-1]) if row.cells else "")
        )
    return rows, detached


def check_series(spec: dict) -> tuple[list[str], str]:
    findings: list[str] = []
    directory = ROOT / spec["dir"]
    index = ROOT / spec["index"]
    name = spec["name"]

    if not index.is_file():
        return ([f"{name}: {spec['index']} does not exist — the index this gate "
                 f"compares against is gone, so nothing was compared."],
                f"{name}: NO INDEX")

    documents = {}
    if directory.is_dir():
        for p in sorted(directory.iterdir()):
            m = spec["file"].match(p.name) if p.is_file() else None
            if m:
                documents[m.group(1)] = p

    rows, detached = index_rows(index, spec["row"])
    for lineno, line in detached:
        findings.append(
            f"{name}: {spec['index']}:{lineno} is an index row that no table "
            f"contains — a blank line above it ended the table, so a reader "
            f"opening the index sees a paragraph of literal pipe characters "
            f"where this entry should be:\n    {line[:100]}\n    Delete the "
            f"blank line above it so the row rejoins the index table."
        )
    compared = 0

    for n, num, target, cell in rows:
        label = spec["label"](num)
        path = (index.parent / target).resolve()
        if not path.is_file():
            findings.append(
                f"{name}: {spec['index']}:{n} — row {label} links to {target!r}, "
                f"which does not exist. Point it at the file, or delete the row."
            )
            continue
        m = spec["file"].match(path.name)
        if not m or m.group(1) != num:
            findings.append(
                f"{name}: {spec['index']}:{n} — row {label} links to {path.name!r}, "
                f"a different document. The row reads correctly and sends the "
                f"reader elsewhere."
            )
            continue

        found = document_status(path)
        rel = path.relative_to(ROOT).as_posix()
        if found is None or found[2] is None:
            where = f"{rel}:{found[0]}" if found else rel
            value = f" says {found[1]!r}" if found else " has no `Status:` field in its header"
            findings.append(
                f"{name}: {where}{value} — no readable status, so "
                f"{spec['index']}:{n} has nothing to be checked against. "
                f"Recognised: {', '.join(STATUSES)}. Do NOT invent one: a status "
                f"written into a document nobody ruled on is the fact this gate "
                f"exists to protect, forged."
            )
            continue

        doc_line, doc_raw, doc_status = found
        row_status = status_word(cell)
        compared += 1
        if row_status is None:
            findings.append(
                f"{name}: {spec['index']}:{n} — row {label} states {cell!r}, which "
                f"is not a status. {rel}:{doc_line} says {doc_status!r}. The "
                f"document is the authority; edit the index row."
            )
        elif row_status != doc_status:
            findings.append(
                f"{name}: {spec['index']}:{n} — row {label} says {row_status!r}; "
                f"{rel}:{doc_line} says {doc_status!r}. The document is the "
                f"authority and the index is the copy, so unless the document is "
                f"itself wrong, edit {spec['index']} to read {doc_status!r}."
            )

    indexed = {num for _n, num, _t, _c in rows}
    for num, path in sorted(documents.items()):
        if num in indexed:
            continue
        rel = path.relative_to(ROOT).as_posix()
        found = document_status(path)
        status = found[2] if found else None
        findings.append(
            f"{name}: {rel} has no row in {spec['index']} — the index is the only "
            f"place this set is enumerated, so a document missing from it is a "
            f"document nobody browsing finds. Add a row"
            + (f" stating {status!r}." if status else
               ", once the document states a readable status of its own.")
        )

    section_re = spec.get("section")
    sections = 0
    if section_re is not None:
        for _num, path in sorted(documents.items()):
            rel = path.relative_to(ROOT).as_posix()
            lines = path.read_text(encoding="utf-8").splitlines()
            if any(section_re.match(line) for line in lines):
                sections += 1
            else:
                findings.append(
                    f"{name}: {rel} has no Acceptance criteria section — every "
                    f"spec states its acceptance criteria as machine-checkable "
                    f"assertions ({spec['index']} preamble; CLAUDE.md "
                    f"Conventions), and a spec without them names a feature "
                    f"nothing can prove finished. Add the section; where the "
                    f"subject makes a criterion genuinely unwritable by machine, "
                    f"the section says so and names what a human checks instead. "
                    f"Do NOT satisfy this rule with an empty heading."
                )

    summary = (f"{name}: {len(documents)} document(s), {len(rows)} row(s), "
               f"{compared} status(es) compared")
    if section_re is not None:
        summary += f", {sections} acceptance-criteria section(s)"
    if not documents or not rows:
        findings.append(
            f"{name}: examined {len(documents)} document(s) in {spec['dir']!r} and "
            f"{len(rows)} row(s) in {spec['index']} — one side is empty, so every "
            f"rule above matched nothing. A check that binds to nothing is not a "
            f"check that passed."
        )
        summary += " (VACUOUS)"
    return findings, summary


def main() -> int:
    findings: list[str] = []
    summaries: list[str] = []
    for spec in SERIES:
        f, s = check_series(spec)
        findings.extend(f)
        summaries.append(s)

    print("check-numbered-doc-index: binding —")
    for s in summaries:
        print(f"  {s}")
    sys.stdout.flush()  # the binding count reads first, not after the findings

    if any("VACUOUS" in s or "NO INDEX" in s for s in summaries):
        for f in findings:
            print(f"  {f}", file=sys.stderr)
        return 2

    if findings:
        print(f"\ncheck-numbered-doc-index: FAIL — {len(findings)} finding(s). An "
              f"index that disagrees with the document it points at is a copy "
              f"nothing compared against its source.\n", file=sys.stderr)
        for f in findings:
            print(f"  {f}\n", file=sys.stderr)
        return 1

    print("check-numbered-doc-index: OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
