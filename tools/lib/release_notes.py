"""`RELEASE-NOTES.md`, read one way: the notes of the release being prepared, and what makes them a summary.

## What the file is

The repository releases three things by name and version (`tools/lib/release_tags.py`).
`RELEASE-NOTES.md` at the root holds the notes of the release being prepared —
one entry per line that release names, at most one entry per line, replaced
wholesale by the pull request that prepares the next release — each headed by
the Release's title, the tag with `--v` read as a space:

    ## delvec 1.15.0

    ### Highlights

    - a few bullets: what this release means to the creator who installs it

    ### <a group of changes>

    - the changes worth stating, as bullets, under as many groups as the entry
      needs — the Keep a Changelog kinds (Added, Changed, Deprecated, Removed,
      Fixed, Security) or the areas of the engine they touch

    A closing paragraph may say what the entry leaves out.

The entry is written before the release, by the agent that prepares it, from
the changes merged since the previous release of the same line; the release
workflow publishes it as the Release's notes below the opening
`tools/ci/release-notes.py` reads from the tree, with a compare link to the full
merged list. The published Release keeps each version's text; the tree keeps
only the next release's, so no record of past versions accumulates here.
Nothing here is a list of merged pull-request titles: that list is one click
away, and a Release's notes are what a reader needs instead of it.

## What makes an entry a summary, machine-checkably

`shape` judges an entry and returns findings, one per defect:

- a `Highlights` section with one to `HIGHLIGHTS_CEILING` bullets;
- at least one group of changes, each with at least one bullet,
  `CHANGES_CEILING` bullets in all;
- every line inside a section (a bullet, its indented continuation, or a line
  of prose), none before the first section;
- no line that is a merged change's subject again. Both sides are read through
  `normalize`: casefolded, a conventional-commit prefix and a trailing
  pull-request suffix removed, inline markup and the final period dropped. A
  line that survives that is the author's sentence, not the commit's.

`parse` returns every entry with its own structural findings, and the file's:
a heading that is not a Release title, a title twice, a line with two entries.

Stdlib only. This module is the one reader of the file; `tools/ci/release-notes.py`
publishes through it and `tools/ci/check-release-notes.py` judges through it.
"""

from __future__ import annotations

import pathlib
import re
import sys
from dataclasses import dataclass, field

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import release_tags  # noqa: E402

FILE = "RELEASE-NOTES.md"
HIGHLIGHTS = "Highlights"
KEEP_A_CHANGELOG_GROUPS = ("Added", "Changed", "Deprecated", "Removed", "Fixed", "Security")
HIGHLIGHTS_CEILING = 7
CHANGES_CEILING = 40

_ENTRY = re.compile(r"^## (?P<title>\S.*?)\s*$")
_SECTION = re.compile(r"^### (?P<name>\S.*?)\s*$")
_BULLET = re.compile(r"^[-*] (?P<text>\S.*)$")
_CONTINUATION = re.compile(r"^\s+(?P<text>\S.*)$")
_HEADING_ANY = re.compile(r"^#{1,6} ")

_CONVENTIONAL_PREFIX = re.compile(r"^[a-z]+(?:\([^)]*\))?!?:\s*")
_PR_SUFFIX = re.compile(r"\s*\(#\d+\)\s*$")
_MARKUP = re.compile(r"[`*_]")
_SPACES = re.compile(r"\s+")


@dataclass
class Entry:
    title: str
    name: str
    version: str
    tag: str
    line: int
    body: str
    highlights: list[str] = field(default_factory=list)
    groups: dict[str, list[str]] = field(default_factory=dict)
    prose: list[tuple[str, str]] = field(default_factory=list)  # (section, line of prose)
    findings: list[str] = field(default_factory=list)

    @property
    def change_count(self) -> int:
        return sum(len(bullets) for bullets in self.groups.values())


def normalize(text: str) -> str:
    """The comparison form of a line and of a commit subject."""
    text = _MARKUP.sub("", text.strip())
    text = _PR_SUFFIX.sub("", text)
    text = _CONVENTIONAL_PREFIX.sub("", text)
    text = _SPACES.sub(" ", text).strip().rstrip(".").strip()
    return text.casefold()


def _title_tag(title: str) -> str | None:
    parts = title.split(" ")
    if len(parts) != 2:
        return None
    try:
        return release_tags.tag_for(parts[0], parts[1])
    except release_tags.Refused:
        return None


def parse(text: str) -> tuple[list[Entry], list[str]]:
    """Every entry of the file, each with its structural findings, and the
    file-level findings. Text before the first entry is the file's header and is
    not read."""
    lines = text.split("\n")
    entries: list[Entry] = []
    findings: list[str] = []
    starts = [(n, m.group("title")) for n, m in ((n, _ENTRY.match(ln)) for n, ln in enumerate(lines, 1)) if m]
    for i, (start, title) in enumerate(starts):
        end = starts[i + 1][0] - 1 if i + 1 < len(starts) else len(lines)
        body_lines = lines[start:end]
        tag = _title_tag(title)
        if tag is None:
            findings.append(
                f"{FILE}:{start}: heading {title!r} is not a Release title (`<name> <major>.<minor>.<patch>`, "
                f"<name> one of {', '.join(release_tags.NAMES)})"
            )
            continue
        name, version = release_tags.parse(tag)
        entry = Entry(title=title, name=name, version=version, tag=tag, line=start, body="\n".join(body_lines).strip())
        _read_sections(entry, body_lines, start)
        entries.append(entry)

    seen: dict[str, int] = {}
    for entry in entries:
        if entry.title in seen:
            findings.append(f"{FILE}:{entry.line}: {entry.title!r} is already an entry at line {seen[entry.title]}")
        seen.setdefault(entry.title, entry.line)
    first: dict[str, Entry] = {}
    for entry in entries:
        if entry.name in first and first[entry.name].title != entry.title:
            findings.append(
                f"{FILE}:{entry.line}: {entry.title!r} beside {first[entry.name].title!r} at line "
                f"{first[entry.name].line}; the file holds one entry per line, the release being prepared"
            )
        first.setdefault(entry.name, entry)
    return entries, findings


def _read_sections(entry: Entry, body_lines: list[str], start: int) -> None:
    section: str | None = None
    bullets: list[str] | None = None
    last_was_bullet = False
    for offset, raw in enumerate(body_lines, 1):
        n = start + offset
        if not raw.strip():
            last_was_bullet = False
            continue
        m = _SECTION.match(raw)
        if m:
            section = m.group("name")
            if section == HIGHLIGHTS:
                if entry.highlights:
                    entry.findings.append(f"{FILE}:{n}: a second `### {HIGHLIGHTS}` in {entry.title!r}")
                bullets = entry.highlights
            else:
                if section in entry.groups:
                    entry.findings.append(f"{FILE}:{n}: a second `### {section}` in {entry.title!r}")
                bullets = entry.groups.setdefault(section, [])
            last_was_bullet = False
            continue
        if _HEADING_ANY.match(raw):
            entry.findings.append(
                f"{FILE}:{n}: heading {raw.strip()!r} inside {entry.title!r}; an entry holds `###` sections only"
            )
            section, bullets, last_was_bullet = None, None, False
            continue
        if section is None or bullets is None:
            entry.findings.append(f"{FILE}:{n}: {raw.strip()!r} stands before the first section of {entry.title!r}")
            continue
        b = _BULLET.match(raw)
        if b:
            bullets.append(b.group("text").strip())
            last_was_bullet = True
            continue
        c = _CONTINUATION.match(raw)
        if c and last_was_bullet:
            bullets[-1] = f"{bullets[-1]} {c.group('text').strip()}"
            continue
        entry.prose.append((section, raw.strip()))
        last_was_bullet = False


def shape(entry: Entry, subjects: list[str] | None) -> list[str]:
    """The findings that stop `entry` being a summary. `subjects` are the merged
    changes' commit subjects since the previous release of the line, or None when
    the line has no previous release (nothing to be a copy of)."""
    findings = list(entry.findings)
    where = f"{FILE}:{entry.line} ({entry.title})"
    if not entry.highlights:
        findings.append(f"{where}: no `### {HIGHLIGHTS}` with a bullet; the summary opens with what this release means")
    elif len(entry.highlights) > HIGHLIGHTS_CEILING:
        findings.append(
            f"{where}: {len(entry.highlights)} highlights; at most {HIGHLIGHTS_CEILING} — the rest are changes"
        )
    if not entry.groups:
        findings.append(f"{where}: no group of changes (`### <group>` after the highlights) with a bullet")
    for group, bullets in entry.groups.items():
        if not bullets:
            findings.append(f"{where}: `### {group}` has no bullet")
    if entry.change_count > CHANGES_CEILING:
        findings.append(
            f"{where}: {entry.change_count} changes listed; at most {CHANGES_CEILING} — a summary states the "
            f"changes worth stating and links the rest"
        )
    if subjects is not None:
        copies = {normalize(s): s for s in subjects if normalize(s)}
        lines = [(HIGHLIGHTS, b) for b in entry.highlights]
        lines += [(g, b) for g, bullets in entry.groups.items() for b in bullets]
        lines += entry.prose
        for label, text in lines:
            hit = copies.get(normalize(text))
            if hit is not None:
                findings.append(
                    f"{where}: `### {label}` line {text!r} is the merged change's own title "
                    f"({hit!r}); a summary is written for the reader, not copied from the log"
                )
    return findings


def entry_for(entries: list[Entry], title: str) -> Entry | None:
    for entry in entries:
        if entry.title == title:
            return entry
    return None


def compare_url(repository: str, previous: str, tag: str) -> str:
    return f"https://github.com/{repository}/compare/{previous}...{tag}"
