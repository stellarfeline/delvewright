#!/usr/bin/env python3
"""Merge-artifact gate for the documentation tree.

The failure this exists to kill: `shortcuts[]` and
`waves[].respawns_on_rest` each appeared **twice** in the stage-5 DSL table of
`docs/reference/compiler.md`. Nothing was wrong with either copy — they were the
same rows, appended at the same anchor by two branches that were merged in a
catch-up burst. Git merged both hunks cleanly because they were
additions at the same place, and no check looked at documentation *shape*, so the
reference — the one authoritative record of compiler behavior (CLAUDE.md
Methodology) — silently started documenting one key twice.

That class is structurally invisible to every other gate: link checking passes,
the DW-code gate passes (it only knows DW codes), and a human reading a
2500-line reference top to bottom is not a check. It has to die in CI, not in
vigilance (CLAUDE.md debug doctrine: preserve the lesson in the strongest
available form — here, a tooling default).

Three rules, over `docs/**/*.md` plus `README.md`:

1. **No markdown table may carry two body rows with the same first-cell key.**
   The first cell of a doc table is its key column — a DSL field, a verb, a CLI
   flag, a diagnostic code. Two rows under one key means either a duplicated row
   (merge artifact) or two contradicting descriptions of one thing; both are
   defects. Keys are compared after stripping inline markup (backticks, bold,
   italic, links) and casefolding, so `` `shortcuts[]` `` and `shortcuts[]`
   collide as they should. Rows whose first cell is empty or a pure filler dash
   are continuation rows and are skipped.

2. **No file may repeat a heading.** Same level, same text, twice in one file is
   a doubled section — the other shape the same merge produces.

3. **No git conflict markers.** `<<<<<<<` / `>>>>>>>` at line start, and
   `=======` only while a conflict is open, so a setext `=======` underline is
   never mistaken for one.

Fenced code blocks (``` and ~~~) are skipped for rules 1 and 2: a code sample
may legitimately show a repeated pipe-table line or a `#` comment. Rule 3 scans
every line — a conflict marker inside a fence is still a conflict marker.

If a doc genuinely needs the same key twice in one table, **restructure the doc**
(split the table, or qualify the keys) rather than allowlisting: a key column
whose keys are not unique is not a key column. ALLOWLIST exists for cases where
that is impossible, and every entry needs a written justification.

Deterministic, offline, no dependencies (Python 3 stdlib). Run from the repo root:
    python3 tools/check-doc-dupes.py [path ...]
Exit 0 = clean, 1 = artifacts found (see stderr), 2 = usage/IO error.
"""

import pathlib
import re
import sys

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
DEFAULT_TARGETS = ("docs", "README.md")

# `docs/notes/private/` is gitignored by design (CLAUDE.md *Privacy in repo
# artifacts*) — it exists on a workstation and never in CI. Scanning it would make
# the local run and the CI run disagree, which is the one thing a gate may never
# do, so it is skipped in both.
SKIP_DIR_NAMES = frozenset({"private"})

# Same-table duplicate keys that are legitimate and cannot be restructured away.
# Keyed by (repo-relative posix path, casefolded key). Keep EMPTY if at all
# possible — see the module docstring: prefer restructuring the table. Every
# entry needs a one-line justification.
ALLOWLIST: dict[tuple[str, str], str] = {}

FENCE_RE = re.compile(r"^\s{0,3}(`{3,}|~{3,})")
HEADING_RE = re.compile(r"^(#{1,6})\s+(.*?)\s*#*\s*$")
TABLE_SEPARATOR_RE = re.compile(r"^\s*\|?[\s:|-]*-[\s:|-]*\|?\s*$")
LINK_RE = re.compile(r"\[([^\]]*)\]\([^)]*\)")
FILLER_KEY_RE = re.compile(r"^[-–—.…*_\s]*$")


def is_table_line(line: str) -> bool:
    return line.lstrip().startswith("|")


def split_row(line: str) -> list[str]:
    """Cells of a pipe-table row. Leading/trailing pipes are structural; a `\\|`
    is an escaped pipe inside a cell, not a separator."""
    body = line.strip()
    placeholder = "\x00"
    body = body.replace(r"\|", placeholder)
    if body.startswith("|"):
        body = body[1:]
    if body.endswith("|"):
        body = body[:-1]
    return [c.replace(placeholder, "|").strip() for c in body.split("|")]


def normalize_key(cell: str) -> str:
    """A table key with inline markup removed, casefolded.

    `` `shortcuts[]` ``, `**shortcuts[]**` and `shortcuts[]` are the same key; a
    duplicate row that picked up different emphasis on the way through a merge
    must not slip past.
    """
    text = LINK_RE.sub(r"\1", cell)
    text = text.replace("`", "").replace("*", "").replace("_", "")
    text = re.sub(r"<[^>]+>", "", text)
    return re.sub(r"\s+", " ", text).strip().casefold()


def normalize_heading(level: str, text: str) -> str:
    body = LINK_RE.sub(r"\1", text)
    body = body.replace("`", "").replace("*", "").replace("_", "")
    return f"{len(level)}:{re.sub(r'\s+', ' ', body).strip().casefold()}"


def content_lines(lines: list[str]) -> list[tuple[int, str]]:
    """(1-based line number, text) for every line outside a fenced code block."""
    out: list[tuple[int, str]] = []
    fence: str | None = None
    for n, line in enumerate(lines, start=1):
        m = FENCE_RE.match(line)
        if fence is None:
            if m:
                fence = m.group(1)[0]
                continue
            out.append((n, line))
        else:
            if m and m.group(1)[0] == fence:
                fence = None
    return out


def table_blocks(numbered: list[tuple[int, str]]) -> list[list[tuple[int, str]]]:
    """Contiguous runs of pipe-table lines, split on any non-table line."""
    blocks: list[list[tuple[int, str]]] = []
    current: list[tuple[int, str]] = []
    prev_n = None
    for n, line in numbered:
        if is_table_line(line) and (prev_n is None or n == prev_n + 1 or not current):
            current.append((n, line))
        elif is_table_line(line):
            if current:
                blocks.append(current)
            current = [(n, line)]
        else:
            if current:
                blocks.append(current)
            current = []
        prev_n = n
    if current:
        blocks.append(current)
    return blocks


def duplicate_table_keys(path: str, lines: list[str]) -> list[str]:
    errors: list[str] = []
    for block in table_blocks(content_lines(lines)):
        # A real table is header + separator + body; anything shorter is not a
        # keyed table (and a run without a separator is not a table at all).
        sep_index = next(
            (
                i
                for i, (_, line) in enumerate(block)
                if i > 0 and TABLE_SEPARATOR_RE.match(line.strip())
            ),
            None,
        )
        if sep_index is None:
            continue
        seen: dict[str, int] = {}
        for n, line in block[sep_index + 1 :]:
            cells = split_row(line)
            if not cells:
                continue
            key = normalize_key(cells[0])
            if not key or FILLER_KEY_RE.match(key):
                continue
            if key in seen:
                if (path, key) in ALLOWLIST:
                    continue
                errors.append(
                    f"{path}:{n}: table key {cells[0]!r} already used at line "
                    f"{seen[key]} of the same table — duplicated row, or two rows "
                    "documenting one key (merge the rows, or restructure the table)"
                )
            else:
                seen[key] = n
    return errors


def duplicate_headings(path: str, lines: list[str]) -> list[str]:
    errors: list[str] = []
    seen: dict[str, int] = {}
    for n, line in content_lines(lines):
        m = HEADING_RE.match(line)
        if not m:
            continue
        key = normalize_heading(m.group(1), m.group(2))
        if key.endswith(":"):  # a heading with no text
            continue
        if key in seen:
            errors.append(
                f"{path}:{n}: heading {line.strip()!r} repeats the one at line "
                f"{seen[key]} — doubled section (delete the copy, or make the "
                "headings distinct)"
            )
        else:
            seen[key] = n
    return errors


def conflict_markers(path: str, lines: list[str]) -> list[str]:
    errors: list[str] = []
    open_at: int | None = None
    for n, line in enumerate(lines, start=1):
        if line.startswith("<<<<<<<"):
            open_at = n
            errors.append(f"{path}:{n}: git conflict marker {line.strip()!r}")
        elif line.startswith(">>>>>>>"):
            open_at = None
            errors.append(f"{path}:{n}: git conflict marker {line.strip()!r}")
        elif open_at is not None and line.rstrip() == "=======":
            errors.append(
                f"{path}:{n}: git conflict marker '=======' (conflict opened at "
                f"line {open_at})"
            )
    return errors


def markdown_files(targets: list[pathlib.Path]) -> list[pathlib.Path]:
    found: set[pathlib.Path] = set()
    for target in targets:
        if target.is_dir():
            # Match skip names against the path BELOW the scanned root only —
            # the absolute path above it is none of the check's business (macOS
            # temp dirs live under `/private`, which would otherwise skip
            # everything and report a vacuous pass).
            found.update(
                p
                for p in target.rglob("*.md")
                if not SKIP_DIR_NAMES.intersection(p.relative_to(target).parts)
            )
        elif target.is_file() and target.suffix == ".md":
            found.add(target)
    return sorted(found)


def display_path(path: pathlib.Path) -> str:
    """Repo-relative posix path when the file lives under the repo, else its own
    path — the checker is runnable over an arbitrary directory (handy for
    scanning a doc tree outside the worktree) and must not crash on one."""
    try:
        return path.relative_to(REPO_ROOT).as_posix()
    except ValueError:
        return path.as_posix()


def check_file(path: pathlib.Path) -> list[str]:
    rel = display_path(path)
    lines = path.read_text(encoding="utf-8").splitlines()
    return (
        conflict_markers(rel, lines)
        + duplicate_headings(rel, lines)
        + duplicate_table_keys(rel, lines)
    )


def main(argv: list[str]) -> int:
    explicit = argv[1:]
    raw = explicit or list(DEFAULT_TARGETS)
    targets = [
        (REPO_ROOT / t) if not pathlib.Path(t).is_absolute() else pathlib.Path(t)
        for t in raw
    ]
    # An explicitly named path that does not exist is a typo and must be loud; a
    # DEFAULT target that does not exist is simply nothing to scan.
    missing = [t for t in targets if not t.exists()]
    if missing and explicit:
        for t in missing:
            print(f"error: no such path: {t}", file=sys.stderr)
        return 2

    files = markdown_files([t for t in targets if t.exists()])
    if not files:
        print("error: no markdown files under the given paths", file=sys.stderr)
        return 2

    errors: list[str] = []
    for path in files:
        errors.extend(check_file(path))

    # Report a stale allowlist entry rather than letting it rot into a licence
    # to duplicate: an entry that no longer suppresses anything must go.
    scanned = {display_path(p) for p in files}
    live = {apath for apath, _key in ALLOWLIST if apath in scanned}
    stale = sorted({p for p, _ in ALLOWLIST} - live)
    for p in stale:
        errors.append(f"ALLOWLIST entry for {p!r} names a file outside the scanned set — remove it")

    if errors:
        print("documentation duplicate/merge-artifact check FAILED:", file=sys.stderr)
        for e in errors:
            print(f"  - {e}", file=sys.stderr)
        return 1

    print(f"documentation duplicate check OK: {len(files)} markdown files clean.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
