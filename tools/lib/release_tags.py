"""The release tag grammar, stated once: `<name>--v<major>.<minor>.<patch>` (ADR-0028 §1).

## What it is

This repository releases three things, each by its name and its version:

    delvec--v1.6.0            the creator binary       (.github/workflows/engine-release.yml)
    delvewright-dsl--v0.26.0  the format crate          (.github/workflows/dsl-crate-publish.yml)
    delvewright--v1.4.3       the Claude Code plugin    (.github/workflows/plugin-release.yml)

The version is a strict semver triple — no leading zero, no prerelease, no build
metadata. A workflow's `on.push.tags` filter can only say "starts with the name
and has three dotted digit runs"; the strict half is this module, which every
release workflow's identity step runs before anything is built, so a tag the
filter lets through and the grammar refuses (`delvec--v01.6.0`) stops there.

## Why one module

Three workflows and two CI gates parse, build or compare these tags. A private
regex per caller is five authorities for one grammar; this is the one, and the
workflows reach it through the CLI below the way shell reaches
`tools/lib/versions.py`.

## The six `v<semver>` tags

`v1.0.0` … `v1.5.0` are published `delvec` releases from before the grammar
(ADR-0028 §7). They are never a valid tag for anything new — `parse` refuses
them — and they are read in exactly one place: `previous`, for the `delvec`
line only, so the first `delvec--v*` release's generated notes start from the
last release of the same line rather than from whatever the repository tagged
last.

Stdlib only. CLI exit codes: 0 answered, 1 refused, 2 usage.
"""

from __future__ import annotations

import pathlib
import re
import subprocess
import sys

NAMES = ("delvec", "delvewright-dsl", "delvewright")
SEPARATOR = "--v"

_NUM = r"(0|[1-9][0-9]*)"
TAG_RE = re.compile(r"^(" + "|".join(re.escape(n) for n in NAMES) + r")--v" + _NUM + r"\." + _NUM + r"\." + _NUM + r"$")
VERSION_RE = re.compile(r"^" + _NUM + r"\." + _NUM + r"\." + _NUM + r"$")
LEGACY_RE = re.compile(r"^v" + _NUM + r"\." + _NUM + r"\." + _NUM + r"$")
LEGACY_LINE = "delvec"


class Refused(ValueError):
    """A tag or version outside the grammar, with the reason."""


def version_key(version: str) -> tuple[int, int, int]:
    m = VERSION_RE.match(version)
    if not m:
        raise Refused(f"{version!r} is not a strict `major.minor.patch` (no leading zero, no suffix)")
    return int(m.group(1)), int(m.group(2)), int(m.group(3))


def parse(tag: str) -> tuple[str, str]:
    """`(name, version)` for a tag in the grammar; `Refused` for anything else."""
    m = TAG_RE.match(tag)
    if not m:
        hint = ""
        if LEGACY_RE.match(tag):
            hint = (
                " A bare `v<semver>` tag is the pre-grammar `delvec` shape; the six that "
                "exist stay as published history and nothing new is tagged that way."
            )
        raise Refused(
            f"tag {tag!r} is not `<name>--v<major>.<minor>.<patch>` with <name> one of "
            f"{', '.join(NAMES)}.{hint}"
        )
    return m.group(1), f"{m.group(2)}.{m.group(3)}.{m.group(4)}"


def tag_for(name: str, version: str) -> str:
    if name not in NAMES:
        raise Refused(f"{name!r} is not one of the released things: {', '.join(NAMES)}")
    version_key(version)
    return f"{name}{SEPARATOR}{version}"


def title(tag: str) -> str:
    """The Release title: the tag with `--v` read as a space."""
    name, version = parse(tag)
    return f"{name} {version}"


def previous(tag: str, tags: list[str]) -> str | None:
    """The newest tag of the same line strictly older than `tag`, or None.

    Only the same line: notes generated between the repository's last two tags
    would interleave three histories. For `delvec` alone, when no `delvec--v*`
    tag is older, the newest older legacy `v<semver>` tag is its predecessor.
    """
    name, version = parse(tag)
    mine = version_key(version)
    same: list[tuple[tuple[int, int, int], str]] = []
    legacy: list[tuple[tuple[int, int, int], str]] = []
    for other in tags:
        other = other.strip()
        m = TAG_RE.match(other)
        if m and m.group(1) == name:
            key = (int(m.group(2)), int(m.group(3)), int(m.group(4)))
            if key < mine:
                same.append((key, other))
            continue
        lm = LEGACY_RE.match(other)
        if lm and name == LEGACY_LINE:
            key = (int(lm.group(1)), int(lm.group(2)), int(lm.group(3)))
            if key < mine:
                legacy.append((key, other))
    if same:
        return max(same)[1]
    if legacy:
        return max(legacy)[1]
    return None


def identity(name: str, tag: str, version: str) -> str:
    """Refuse unless `tag` is in the grammar and names `name` at `version`."""
    got_name, got_version = parse(tag)
    expected = tag_for(name, version)
    if tag != expected:
        raise Refused(
            f"tag {tag!r} names {got_name} {got_version}, and the tagged tree states "
            f"{name} {version}: the tag must be {expected!r}."
        )
    return expected


USAGE = """usage: release_tags.py <command> [args]

  identity <name> <tag> <version>   refuse (exit 1) unless <tag> == <name>--v<version>, strictly
  parse <tag>                       print "<name> <version>", refused (exit 1) outside the grammar
  tag <name> <version>              print <name>--v<version>
  title <tag>                       print the Release title
  previous <tag> [--repo DIR]       print the newest older tag of the same line (git tag -l), or nothing
"""


def main(argv: list[str]) -> int:
    sys.stdout.reconfigure(newline="\n")  # CRLF-proof: tools/check-python-shell-newlines.py
    if not argv:
        print(USAGE, file=sys.stderr)
        return 2
    command, rest = argv[0], argv[1:]
    try:
        if command == "identity" and len(rest) == 3:
            tag = identity(rest[0], rest[1], rest[2])
            print(f"ok: {tag} is in the grammar and names {rest[0]} {rest[2]} (1 of 1 tag judged)")
            return 0
        if command == "parse" and len(rest) == 1:
            name, version = parse(rest[0])
            print(f"{name} {version}")
            return 0
        if command == "tag" and len(rest) == 2:
            print(tag_for(rest[0], rest[1]))
            return 0
        if command == "title" and len(rest) == 1:
            print(title(rest[0]))
            return 0
        if command == "previous" and len(rest) in (1, 3):
            repo = pathlib.Path(".")
            if len(rest) == 3:
                if rest[1] != "--repo":
                    print(USAGE, file=sys.stderr)
                    return 2
                repo = pathlib.Path(rest[2])
            listed = subprocess.run(
                ["git", "-C", str(repo), "tag", "-l"], check=True, capture_output=True, text=True
            ).stdout.splitlines()
            got = previous(rest[0], listed)
            print(f"release_tags: {len(listed)} tag(s) read; previous of {rest[0]}: {got or '(none)'}", file=sys.stderr)
            if got:
                print(got)
            return 0
    except Refused as exc:
        print(f"release_tags: REFUSED — {exc}", file=sys.stderr)
        return 1
    print(USAGE, file=sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
