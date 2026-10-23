#!/usr/bin/env python3
"""A Release's notes, read from the tree at the released commit (ADR-0028 §2, ADR-<next>).

WHY THIS EXISTS

Three things are released from this repository, and a stranger reading one
Release must be told which of the three it is, the facts that decide whether it
is the one they need, and what the release changes for them. Every value here is
READ from the tree at the commit being released — none is typed into a workflow,
where it would be a second statement of a number `versions.toml` already holds,
and none is generated from the merged pull requests' titles, which is the list a
reader opens the notes to avoid.

THE OPENING, from the tree:

  delvec           the engine version, the `dsl_version` the binary speaks
                   (`[engine].dsl_crate_version`), the crates.io page
  delvewright-dsl  the format version and its crates.io page
  delvewright      the engine release tag the page ships at, the
                   `requires_delvec` window, the `dsl_version` that pinned engine
                   speaks (read from the engine tree AT the pin, where this
                   checkout carries the tag), and the plugin root's tree hash at
                   the release commit

THE SUMMARY, from `CHANGELOG.md` at the same commit (`tools/lib/changelog.py`):
the entry headed by this Release's title — `### Highlights`, then the change
groups — judged a summary before it is printed: a release whose tree has no
entry for its version, or whose entry is the merged changes' titles again, is
REFUSED here, which every release workflow reaches before it builds or writes
anything. The merged changes are the commits between the previous release of
the same line (`release_tags.previous`) and the released commit.

THE COMPARE LINK, derived: everything merged since the previous release of this
line, so the full list stays one click away and never in the notes.

Offline, stdlib only, `git` for the tree. Exit 0 printed, 1 the tree lacks a
value or the entry is not a summary, 2 usage.

Usage:
  python3 tools/ci/release-notes.py (delvec|delvewright-dsl|delvewright) --commit REV
      [--repo DIR] [--repository OWNER/NAME]
"""

from __future__ import annotations

import argparse
import json
import pathlib
import subprocess
import sys
import tomllib

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "lib"))

import changelog  # noqa: E402
import release_tags  # noqa: E402

REPO = pathlib.Path(__file__).resolve().parents[2]
REPOSITORY = "stellarfeline/delvewright"
PLUGIN_ROOT = ".claude/skills/delvewright"
PAGE_PIN = f"{PLUGIN_ROOT}/skills/new-delve/versions.toml"
PAGE = f"{PLUGIN_ROOT}/skills/new-delve/SKILL.md"
MANIFEST = f"{PLUGIN_ROOT}/.claude-plugin/plugin.json"


class Missing(Exception):
    pass


def git(repo: pathlib.Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True)


def show(repo: pathlib.Path, rev: str, path: str) -> str:
    result = git(repo, "show", f"{rev}:{path}")
    if result.returncode != 0:
        raise Missing(f"`git show {rev}:{path}` failed: {result.stderr.strip()}")
    return result.stdout


def engine_table(repo: pathlib.Path, rev: str) -> dict:
    try:
        return tomllib.loads(show(repo, rev, "versions.toml"))["engine"]
    except (KeyError, tomllib.TOMLDecodeError) as exc:
        raise Missing(f"{rev}:versions.toml has no readable [engine]: {exc}")


def requires_delvec(page: str) -> str:
    """`metadata.requires_delvec` out of the page's frontmatter."""
    lines = page.split("\n")
    if not lines or lines[0].strip() != "---":
        raise Missing(f"{PAGE} has no frontmatter")
    in_metadata = False
    for line in lines[1:]:
        if line.strip() == "---":
            break
        if line.startswith("metadata:"):
            in_metadata = True
            continue
        if in_metadata and line.startswith("  requires_delvec:"):
            return line.split(":", 1)[1].strip().strip('"')
        if line and not line.startswith(" "):
            in_metadata = False
    raise Missing(f"{PAGE} frontmatter carries no metadata.requires_delvec")


def opening(line: str, repo: pathlib.Path, sha: str) -> tuple[str, str]:
    """`(version, opening text)` for the line at `sha`."""
    if line == "delvec":
        e = engine_table(repo, sha)
        return e["version"], (
            f"**`delvec` {e['version']}** — the Delvewright creator binary, one of the three things this "
            f"repository releases (the others are the `delvewright-dsl` format crate and the `delvewright` "
            f"Claude Code plugin).\n\n"
            f"- speaks `dsl_version` **{e['dsl_crate_version']}**\n"
            f"- also on crates.io: https://crates.io/crates/delvec/{e['version']}\n"
            f"- built from `{sha}`\n"
        )
    if line == "delvewright-dsl":
        e = engine_table(repo, sha)
        v = e["dsl_crate_version"]
        return v, (
            f"**`delvewright-dsl` {v}** — the Delvewright stage-document format crate, one of the three things "
            f"this repository releases (the others are the `delvec` binary and the `delvewright` Claude Code "
            f"plugin). Its version is the `dsl_version` a document declares.\n\n"
            f"- crates.io: https://crates.io/crates/delvewright-dsl/{v}\n"
            f"- the `.crate` attached here is the tarball crates.io serves, byte-identical; `SHA256SUMS` "
            f"carries the sha256 the registry index records\n"
            f"- tagged at `{sha}`\n"
        )
    if line == "delvewright":
        manifest = json.loads(show(repo, sha, MANIFEST))
        try:
            pin = tomllib.loads(show(repo, sha, PAGE_PIN))["engine"]
        except (KeyError, tomllib.TOMLDecodeError) as exc:
            raise Missing(f"{PAGE_PIN} has no readable [engine]: {exc}")
        window = requires_delvec(show(repo, sha, PAGE))
        # `plugin-release.yml` runs only at a written engine tag, so there the pin
        # resolves. Run by hand on a commit whose pin is still unborn, the tag is
        # PRINTED as unborn rather than raised — the one fact a reader needs is
        # which engine release the page ships at, and that is the name itself.
        try:
            speaks = engine_table(repo, f"{pin['ref']}^{{commit}}")["dsl_crate_version"]
            speaks = f"**{speaks}**"
        except Missing:
            speaks = "the version that release states — the tag is not written yet"
        tree = git(repo, "rev-parse", f"{sha}:{PLUGIN_ROOT}").stdout.strip()
        if not tree:
            raise Missing(f"{sha}:{PLUGIN_ROOT} is not a tree")
        return manifest["version"], (
            f"**`delvewright` {manifest['version']}** — the Delvewright Claude Code plugin (`/delvewright:new-delve`), "
            f"one of the three things this repository releases (the others are the `delvec` binary and the "
            f"`delvewright-dsl` format crate). Its version is the version of the engine release it ships "
            f"at, and it is tagged at that release's commit: the marketplace serves the plugin root at the "
            f"engine release tag the entry names, so these are the bytes a creator installing from that tag "
            f"receives, and an installed creator updates to them because the version moved with the pin.\n\n"
            f"- the page ships at engine release `{pin['ref']}`, and installs that engine\n"
            f"- it accepts `delvec` `{window}`\n"
            f"- that engine speaks `dsl_version` {speaks}\n"
            f"- plugin root tree `{tree}`, tagged at `{sha}`\n"
            f"- to hold a fixed page, add the marketplace at the ENGINE release it ships at: "
            f"`/plugin marketplace add {REPOSITORY}@{pin['ref']}`\n"
        )
    raise Missing(f"{line!r} is not a released line")


def merged_subjects(repo: pathlib.Path, previous: str, sha: str) -> list[str]:
    result = git(repo, "log", "--format=%s", f"{previous}..{sha}")
    if result.returncode != 0:
        raise Missing(f"`git log {previous}..{sha}` failed: {result.stderr.strip()}")
    return [s for s in result.stdout.split("\n") if s.strip()]


def summary(line: str, version: str, repo: pathlib.Path, sha: str, repository: str) -> str:
    """The changelog entry for this release, judged, with its compare link; `Missing` when the
    tree has none or the entry is not a summary."""
    tag = release_tags.tag_for(line, version)
    title = release_tags.title(tag)
    try:
        text = show(repo, sha, changelog.FILE)
    except Missing:
        raise Missing(f"{sha[:12]} has no {changelog.FILE}; a release publishes the entry written for it there")
    entries, file_findings = changelog.parse(text)
    entry = changelog.entry_for(entries, title)
    if entry is None:
        raise Missing(
            f"{changelog.FILE} at {sha[:12]} has no entry `## {title}`: a Release's notes are a summary written "
            f"before the release, and a release with none is refused "
            f"({len(entries)} entr{'y' if len(entries) == 1 else 'ies'} read)"
        )
    tags = git(repo, "tag", "-l").stdout.split("\n")
    previous = release_tags.previous(tag, tags)
    subjects = merged_subjects(repo, previous, sha) if previous else None
    findings = [f for f in file_findings if f"{changelog.FILE}:{entry.line}:" in f] + changelog.shape(entry, subjects)
    if findings:
        raise Missing(f"the entry `## {title}` is not a summary:\n  " + "\n  ".join(findings))
    print(
        f"release-notes: `## {title}` at {changelog.FILE}:{entry.line}; {len(entry.highlights)} highlight(s), "
        f"{entry.change_count} change(s) in {len(entry.groups)} group(s); compared with "
        f"{len(subjects) if subjects is not None else 0} merged subject(s) since "
        f"{previous or '(no previous release of this line)'}",
        file=sys.stderr,
    )
    if previous:
        link = f"Everything merged since `{previous}`, for the record: {changelog.compare_url(repository, previous, tag)}"
    else:
        link = "This is the first release of its line; there is no earlier release to compare with."
    return f"\n{entry.body}\n\n{link}\n"


def notes(line: str, repo: pathlib.Path, rev: str, repository: str) -> str:
    commit = git(repo, "rev-parse", "--verify", f"{rev}^{{commit}}")
    if commit.returncode != 0:
        raise Missing(f"{rev} is not a commit: {commit.stderr.strip()}")
    sha = commit.stdout.strip()
    version, head = opening(line, repo, sha)
    return head + summary(line, version, repo, sha, repository)


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description="a Release's notes: the opening the tree states, the summary written for it")
    ap.add_argument("line", choices=release_tags.NAMES)
    ap.add_argument("--commit", required=True)
    ap.add_argument("--repo", type=pathlib.Path, default=REPO)
    ap.add_argument("--repository", default=REPOSITORY, help="OWNER/NAME on GitHub, for the compare link")
    args = ap.parse_args(argv)
    sys.stdout.reconfigure(newline="\n")  # CRLF-proof: tools/ci/check-python-shell-newlines.py
    try:
        print(notes(args.line, args.repo, args.commit, args.repository), end="")
    except (Missing, KeyError, json.JSONDecodeError, release_tags.Refused) as exc:
        print(f"release-notes: FAIL — {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
