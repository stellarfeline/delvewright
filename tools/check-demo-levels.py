#!/usr/bin/env python3
"""A capability that reaches an author reaches the demo queue in the same change.

## The defect this exists to end

`CLAUDE.md` requires the change that lands a mechanic to add its row to
`docs/demo-levels.md`, and the queue is not a showcase backlog — it is the form
an engine capability's confirmation takes, so a capability with no demo cannot
be put in front of anyone. Nothing invoked that rule. It lived in a doc line,
which is the UNRUN shape `CLAUDE.md` names, and it behaved exactly as that shape
predicts: over 416 changes on `main` the file was edited six times, sixteen of
its rows were seeded retroactively in one sitting, and for the twelve days after
that seeding not one row was added while the compiler, the grammar and the
render layer all gained author-facing surface. Three capabilities landed on one
day; one carried its row.

## The signal, and why it is this one and not a better one

There is no syntactic mark on a diff that says "this is a mechanic", and the
candidates were measured over the whole history rather than argued about. Two
rows are ranges because two defensible readings of one question disagree, and
the range is stated rather than the more convenient end of it:

| candidate signal                               |  fires on | rows in the queue |
| ---------------------------------------------- | --------: | ----------------: |
| touches `crates/` and `docs/reference/`         |       191 |                23 |
| subject line begins `feat(`                     |       140 |                23 |
| a new `DW` diagnostic code                      | 101 – 112 |                23 |
| adds a heading to `docs/reference/`             |        94 |                23 |
| adds a row to a `Since`-columned surface table  |   27 – 53 |                23 |
| a `--flag` token appears in `tools.md`          |        31 |                23 |
| **a new long flag on a binary under `crates/`** |    **16** |                23 |

`DW`: 101 counts the change that first puts a code under `crates/`, 112 also
counts one that names it in the docs first. `Since`: 27 counts net growth of
that table, 53 counts every row added, which is larger because a rewritten row
is a delete and an add.

The first six are not gates, they are noise generators: a check that fires on
one change in three is routed around inside a week, and being routed around is
worse than not existing. The runner-up is worth naming, since it is the obvious
extension if this gate's recall proves too low — a new row in a `Since`-columned
surface table really is new authoring surface. It is not the demand here because
a row there is a FIELD, not a mechanic: `world.outro` is an optional string with
a translation key, and a gate that asks for a demo level when the schema grows
one of those is asking on an ordinary change.

The signal chosen is different in kind. Every one of the
sixteen landed an engine capability — the compiler itself, the schematic
importer, `--lang` and i18n, the admission pipeline, `snapshot`,
`blocking-chart`, the edit stage's `--batch`, shot calibration, panorama
emission, the contact sheet, `fmt --check`, the prefab loop, `--reachable-floor`,
`--symmetric`, the zone audit, and the aimable camera — and the one thing a
reader would call plumbing (`--exclusions`) rode in beside a real capability on
the same change. Replayed against this gate's own extractor, two of the sixteen
added a row and would be green; the other fourteen are the measurement of the
defect.

The scope is what keeps it that clean: only the binaries under `crates/`, every
one of which `docs/reference/tools.md` classes `agent` or `human` — the tools an
authoring session actually runs. That is a property of the object, not a label a
change picks for itself, and it is why the CI-plumbing flags that arrived over
the same window (`--if-stale`, `--base`, `--depth`, `--locked`, `--admin`) are
outside this gate by construction rather than by an allowlist somebody maintains.

## What this does NOT catch, stated because a gate's recall is not optional

A capability with no command-line surface is invisible here. The worked example
is the grammar's guard-exhaustion refusal, which landed the same day as the
aimable camera, owes a row exactly as much, and adds no flag — its whole surface
is what an existing refusal prints. So this gate's recall against the queue is
roughly a third, and it is a floor under the rule, never a proof of it. Nothing
measured beat it without firing on ordinary changes, and a gate that fires on
ordinary changes protects nothing.

There is deliberately **no opt-out**. A flag whose capability is genuinely not
worth a demo still owes the sentence saying so, and any escape hatch here would
be one the defect can supply for itself — "this one does not need a demo" is
precisely what someone omitting a row already believes.

## The other half: the queue may not rot

The demand above only bites on a change that adds a flag. On every other change
this gate still binds, because a queue whose rows cite things that no longer
exist is a queue nobody can build from — and "covered by" coverage is explicitly
allowed to rot in the file's own rules. Every `DW` code, `spec-NNNN` number and
`ADR-NNNN` number a row cites must resolve in this tree.

## And it reads the queue the way the queue's reader reads it

Both demands above are counted off the mechanic-demo table, so where that table
ENDS is load-bearing, and the answer is not this script's to invent. A pipe
table in CommonMark's GFM extension runs from a header row through a delimiter
row to "the first empty line, or beginning of another block-level structure".
This parser applies that rule and no other. It used to apply a different one —
iterate to the next `## `, keep every line beginning with `|` — under which a
blank line did not end anything, so a row detached from the table by a blank
line was an entry for this gate and a paragraph of literal pipe characters for
every renderer and every human. That is the whole obligation satisfiable in the
letter while void in the reading, and it is one-directional falsifiability: the
gate could only fail in the direction that does not drift.

Silently dropping such a row would rebuild the same defect facing the other way,
so a row no table contains is a **finding naming its line**, never a row and
never a discard. The binding line states how many there are on both sides.

Deterministic, offline, stdlib-only python3. States its binding counts; zero
binaries, zero flags, zero rows or zero citations is a red, not a pass.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from lib.gitbase import BaseUnresolved, resolve_base  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
QUEUE = "docs/demo-levels.md"
CRATES = "crates"

BIN_NAME = re.compile(r"^\s*name\s*=\s*\"([^\"]+)\"", re.MULTILINE)
ATTR_START = re.compile(r"#\[(?:arg|clap)\(")
LONG_NAMED = re.compile(r"\blong\s*=\s*\"([A-Za-z0-9][A-Za-z0-9-]*)\"")
LONG_BARE = re.compile(r"\blong\b\s*(?:[,)]|$)")
FIELD_NAME = re.compile(r"(?:pub(?:\([^)]*\))?\s+)?([a-z_][a-z0-9_]*)\s*:")

DW_CODE = re.compile(r"DW[0-9]{4}")
ADR_REF = re.compile(r"ADR-([0-9]{4})")
BARE_NUMBER = re.compile(r"(?<![0-9A-Za-z])([0-9]{4})(?![0-9A-Za-z])")

MECHANIC_HEADING = "## Mechanic demos"

# The two shapes CommonMark's GFM table extension is bounded by. A delimiter
# cell is hyphens with an optional leading or trailing colon; a table ends at
# the first blank line or at the start of another block-level structure, which
# is every construct below. None of them can begin with `|`, so no row of a
# table can be mistaken for one.
DELIMITER_CELL = re.compile(r"^:?-+:?$")
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


# --------------------------------------------------------------------- git ---
def git(*args: str) -> str:
    return subprocess.run(
        ["git", *args], cwd=ROOT, capture_output=True, text=True, check=True
    ).stdout


def tree_paths(ref: str | None, prefix: str) -> list[str]:
    """Every path under `prefix`, in the working tree (`ref is None`) or at `ref`."""
    if ref is None:
        base = ROOT / prefix
        if not base.is_dir():
            return []
        return sorted(
            str(p.relative_to(ROOT)) for p in base.rglob("*") if p.is_file()
        )
    out = git("ls-tree", "-r", "--name-only", ref, f"{prefix}/")
    return [line for line in out.split("\n") if line]


def read_text(ref: str | None, path: str) -> str:
    if ref is None:
        return (ROOT / path).read_text(encoding="utf-8", errors="replace")
    try:
        return git("show", f"{ref}:{path}")
    except subprocess.CalledProcessError:
        return ""


# ------------------------------------------------------------------- flags ---
def _balanced(text: str, open_at: int, pair: str = "()") -> int:
    """Index just past the delimiter closing the one that opens at `open_at`.

    String literals are stepped over, so a `default_value = "a)b"` cannot end the
    attribute early.
    """
    opener, closer = pair
    depth = 0
    i = open_at
    while i < len(text):
        c = text[i]
        if c == opener:
            depth += 1
        elif c == closer:
            depth -= 1
            if depth == 0:
                return i + 1
        elif c in "\"'":
            quote = c
            i += 1
            while i < len(text) and text[i] != quote:
                i += 2 if text[i] == "\\" else 1
        i += 1
    return len(text)


def _skip_decorations(text: str) -> str:
    """Drop the attributes and doc comments stacked above a struct field."""
    while True:
        text = text.lstrip()
        if text.startswith("#["):
            text = text[_balanced(text, text.index("["), "[]") :]
            continue
        if text.startswith("//"):
            text = text.split("\n", 1)[1] if "\n" in text else ""
            continue
        return text


def flags_in_source(text: str) -> set[str]:
    """Every long flag a clap derive in `text` declares.

    `#[arg(long = "no-validate")]` names itself; a bare `long` takes the field's
    own name with underscores kebab-cased, which is clap's rule.
    """
    found: set[str] = set()
    for match in ATTR_START.finditer(text):
        open_at = match.end() - 1
        end = _balanced(text, open_at)
        body = text[open_at:end]
        named = LONG_NAMED.search(body)
        if named:
            found.add(named.group(1))
            continue
        if not LONG_BARE.search(body):
            continue
        # A bare `long` names the field this attribute decorates, so read past
        # the attribute's own `]` and anything else stacked above the field.
        field = FIELD_NAME.match(_skip_decorations(text[end:].lstrip().lstrip("]")))
        if field:
            found.add(field.group(1).replace("_", "-"))
    return found


def binary_crates(ref: str | None) -> dict[str, str]:
    """`crate directory -> binary name` for every crate under `crates/` that
    declares a `[[bin]]`. Discovered, never listed: a seventh binary inherits
    this gate with nobody remembering."""
    out: dict[str, str] = {}
    for path in tree_paths(ref, CRATES):
        if not path.endswith("/Cargo.toml"):
            continue
        text = read_text(ref, path)
        if "[[bin]]" not in text:
            continue
        section = text.split("[[bin]]", 1)[1]
        name = BIN_NAME.search(section)
        if name:
            out[path.rsplit("/", 1)[0]] = name.group(1)
    return out


def flag_owners(ref: str | None) -> tuple[dict[str, set[str]], int, int]:
    """`flag -> {binary names declaring it}`, plus crate count and file count."""
    crates = binary_crates(ref)
    owners: dict[str, set[str]] = {}
    files = 0
    for path in tree_paths(ref, CRATES):
        if not path.endswith(".rs"):
            continue
        crate = next((c for c in crates if path.startswith(c + "/src/")), None)
        if crate is None:
            continue
        files += 1
        for flag in flags_in_source(read_text(ref, path)):
            owners.setdefault(flag, set()).add(crates[crate])
    return owners, len(crates), files


# ------------------------------------------------------------------- queue ---
def _cells(line: str) -> list[str]:
    return [c.strip() for c in line.strip().strip("|").split("|")]


def _is_delimiter_row(line: str, width: int) -> bool:
    """GFM's delimiter row: cells of hyphens with an optional leading or
    trailing colon, and the same cell count as the header row above it. Both
    conditions are the spec's; failing either means there is no table at all."""
    line = line.strip()
    if "|" not in line:
        return False
    cells = _cells(line)
    return len(cells) == width and all(DELIMITER_CELL.match(c) for c in cells)


def parse_queue(ref: str | None) -> tuple[list[list[str]], list[tuple[int, str]]]:
    """The mechanic-demo table's data rows, and the rows no table contains.

    Parsed the way the thing that CONSUMES this file parses it. A pipe table in
    CommonMark's GFM extension runs from a header row, through a delimiter row,
    to `the first empty line, or beginning of another block-level structure` —
    so a blank line ENDS the table, and a row under that blank line is not in
    it. Every renderer this document is read through obeys that; the parser
    this replaced did not, and so counted a detached row as an entry while the
    page showed it as a paragraph of literal pipes.

    That gap is what made the demo-queue obligation satisfiable in the letter
    and void in the reading: the row is there for the gate and absent for the
    reader. So the detached rows are returned alongside, to be named as a
    finding rather than silently counted or silently dropped — dropping them
    would leave the gate falsifiable only in the direction that does not drift.

    Returns `(rows, orphans)`, each orphan a `(line number in {QUEUE}, text)`.
    """
    text = read_text(ref, QUEUE)
    if MECHANIC_HEADING not in text:
        return [], []
    before, body = text.split(MECHANIC_HEADING, 1)
    # `body` resumes on the heading's own line, so index k of it is that line + k.
    heading_lineno = before.count("\n") + 1

    lines = body.split("\n")
    rows: list[list[str]] = []
    orphans: list[tuple[int, str]] = []
    i = 0
    while i < len(lines):
        line = lines[i].strip()
        if line.startswith("## "):
            break
        if not line or "|" not in line:
            i += 1
            continue
        header = _cells(line)
        if i + 1 < len(lines) and _is_delimiter_row(lines[i + 1], len(header)):
            i += 2
            while i < len(lines):
                row = lines[i].strip()
                if not row or BLOCK_START.match(row):
                    break
                rows.append(_cells(row))
                i += 1
            continue
        # A pipe line with no delimiter row under it opens no table, so the
        # renderer shows it as a paragraph. Name it rather than count it.
        orphans.append((heading_lineno + i, line))
        i += 1
    return rows, orphans


def queue_rows(ref: str | None) -> list[list[str]]:
    """Every data row of the mechanic-demo table, as its cells."""
    return parse_queue(ref)[0]


def citations(rows: list[list[str]]) -> tuple[set[str], set[str], set[str]]:
    """`DW` codes, ADR numbers and spec numbers the queue's rows cite."""
    dw: set[str] = set()
    adr: set[str] = set()
    spec: set[str] = set()
    for cells in rows:
        whole = " | ".join(cells)
        dw |= set(DW_CODE.findall(whole))
        adr |= set(ADR_REF.findall(whole))
        # A spec is cited in the MECHANIC cell as a bare number — `(0016 §1)`.
        # Strip the two other numbered forms first, or `ADR-0020` reads as spec
        # 0020 and `DW0801` as spec 0801.
        mechanic = ADR_REF.sub(" ", DW_CODE.sub(" ", cells[0]))
        spec |= set(BARE_NUMBER.findall(mechanic))
    return dw, adr, spec


def crate_sources() -> str:
    return "\n".join(
        (ROOT / p).read_text(encoding="utf-8", errors="replace")
        for p in tree_paths(None, CRATES)
        if p.endswith(".rs")
    )


# -------------------------------------------------------------------- main ---
def main() -> int:
    parser = argparse.ArgumentParser(
        description="A change that adds author-facing command-line surface adds "
        "its row to the demo queue, and the queue's citations still resolve."
    )
    parser.add_argument(
        "--base",
        default="origin/main",
        help="git ref to diff the checkout against (default: origin/main). "
        "Must already be fetched — this script performs no network I/O itself.",
    )
    args = parser.parse_args()

    try:
        base = resolve_base(ROOT, args.base, "check-demo-levels")
    except BaseUnresolved as unresolved:
        print(unresolved.message, file=sys.stderr)
        return 1

    findings: list[str] = []

    head_owners, head_crates, head_files = flag_owners(None)
    base_owners, base_crates, base_files = flag_owners(base)
    head_rows, head_orphans = parse_queue(None)
    base_rows, base_orphans = parse_queue(base)
    dw, adr, spec = citations(head_rows)

    print(
        f"binding: {head_crates} binary crate(s), {head_files} source file(s), "
        f"{len(head_owners)} long flag(s) here; {base_crates}/{base_files}/"
        f"{len(base_owners)} at {args.base}. Queue: {len(head_rows)} row(s) here, "
        f"{len(base_rows)} at {args.base}; citations examined: {len(dw)} DW code(s), "
        f"{len(spec)} spec number(s), {len(adr)} ADR number(s). Rows no table "
        f"contains: {len(head_orphans)} here, {len(base_orphans)} at {args.base}."
    )

    # ---- vacuity -----------------------------------------------------------
    if head_crates == 0 or base_crates == 0:
        findings.append(
            f"examined 0 binary crates under {CRATES}/ "
            f"({head_crates} here, {base_crates} at {args.base}) — the layout "
            "moved or no crate declares a `[[bin]]`. A check that binds to "
            "nothing is not a pass."
        )
    if not head_owners:
        findings.append(
            "found 0 long command-line flags in the binary crates — the clap "
            "derive shape this reads changed, and the demand below would be "
            "silently unfalsifiable from here on."
        )
    if not head_rows:
        findings.append(
            f"parsed 0 rows out of the mechanic-demo table in {QUEUE} — the "
            f"heading {MECHANIC_HEADING!r} or the table shape moved out from "
            "under this parser."
        )
    # ---- the row a reader cannot see --------------------------------------
    for lineno, line in head_orphans:
        findings.append(
            f"{QUEUE}:{lineno} is a table row that no table contains:\n"
            f"    {line}\n"
            "    A blank line ends a pipe table (CommonMark GFM tables: a "
            "table runs to `the first empty line, or beginning of another "
            "block-level structure`), so this row is rendered as a paragraph "
            "of literal pipe characters and is NOT an entry in the "
            f"{MECHANIC_HEADING[3:]} table. It counts for nobody who reads the "
            "page, so a mechanic whose only row is this one has no queue entry "
            "— the obligation is met in the letter and void in the reading.\n"
            "    Fix: delete the blank line above it so it joins the table, or "
            "if it is meant to stand alone, it is not a queue row and should "
            "not be written as one."
        )

    if not (dw or spec or adr):
        findings.append(
            f"the queue's rows cite 0 resolvable identifiers — every spec "
            "number, ADR number and DW code has gone from the table, so the "
            "rot check below examines nothing."
        )

    # ---- the demand --------------------------------------------------------
    new_flags = sorted(set(head_owners) - set(base_owners))
    if new_flags and len(head_rows) <= len(base_rows):
        lines = [
            f"this change adds {len(new_flags)} command-line flag(s) to an "
            f"author-facing binary and adds no row to {QUEUE}:",
        ]
        for flag in new_flags:
            owners = ", ".join(sorted(head_owners[flag]))
            lines.append(f"    --{flag}  ({owners})")
        lines.append(
            "    Every binary under crates/ is an `agent`- or `human`-class "
            "tool in docs/reference/tools.md, so a new flag on one is new "
            "surface an authoring session can reach — and a capability an "
            "author can reach owes a demo (CLAUDE.md: every new mechanic owes "
            "a demo level; the queue is the form its confirmation takes, not a "
            "backlog worked off later)."
        )
        lines.append(
            f"    Fix: add the row to the mechanic-demo table in {QUEUE}, in "
            "the same change. Name the mechanic the way an author would look "
            "for it, and make the demo concept concrete enough to build — what "
            "it contains, what a player does in it, and what it shows that a "
            "still picture could not. There is no opt-out: a flag whose "
            "capability needs no demo still owes the row that says so."
        )
        findings.append("\n".join(lines))

    # ---- the queue may not rot --------------------------------------------
    sources = crate_sources()
    for code in sorted(dw):
        if code not in sources:
            findings.append(
                f"{QUEUE} cites {code}, which no source file under {CRATES}/ "
                "emits. The demo it describes cannot be built against this "
                "tree: either the diagnostic was renamed and the row must "
                "follow it, or the row outlived its mechanic and says so."
            )
    for number in sorted(spec):
        if not list((ROOT / "docs" / "specs").glob(f"spec-{number}-*.md")):
            findings.append(
                f"{QUEUE} cites spec {number}, and docs/specs/ holds no "
                f"spec-{number}-*.md. A queue row whose spec a reader cannot "
                "open is a row nobody can build from."
            )
    for number in sorted(adr):
        if not list((ROOT / "docs" / "adr").glob(f"{number}-*.md")):
            findings.append(
                f"{QUEUE} cites ADR-{number}, and docs/adr/ holds no "
                f"{number}-*.md."
            )

    if findings:
        print()
        for finding in findings:
            print(f"FAIL: {finding}")
        print(f"\n{len(findings)} finding(s).")
        return 1

    print("OK: the demo queue is current and every citation in it resolves.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
