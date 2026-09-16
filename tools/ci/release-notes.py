#!/usr/bin/env python3
"""The opening of a Release's notes, read from the tree at the released commit (ADR-0028 §2).

WHY THIS EXISTS

Three things are released from this repository, and a stranger reading one
Release must be told which of the three it is, and the facts that decide whether
it is the one they need. Every value here is READ from the tree at the commit
being released — none is typed into a workflow, where it would be a second
statement of a number `versions.toml` already holds.

  delvec           the engine version, the `dsl_version` the binary speaks
                   (`[engine].dsl_crate_version`), the crates.io page
  delvewright-dsl  the format version and its crates.io page
  delvewright      the engine release tag the page ships at, the
                   `requires_delvec` window, the `dsl_version` that pinned engine
                   speaks (read from the engine tree AT the pin, where this
                   checkout carries the tag), and the plugin root's tree hash at
                   the release commit

The generated changelog GitHub appends below this opening is asked for by the
workflow, between consecutive tags of the same line (`tools/lib/release_tags.py
previous`).

Offline, stdlib only, `git` for the tree. Exit 0 printed, 1 the tree lacks a
value, 2 usage.

Usage:
  python3 tools/ci/release-notes.py (delvec|delvewright-dsl|delvewright) --commit REV [--repo DIR]
"""

from __future__ import annotations

import argparse
import json
import pathlib
import subprocess
import sys
import tomllib

REPO = pathlib.Path(__file__).resolve().parents[2]
PLUGIN_ROOT = ".claude/skills/delvewright"
PAGE_PIN = f"{PLUGIN_ROOT}/skills/new-delve/versions.toml"
PAGE = f"{PLUGIN_ROOT}/skills/new-delve/SKILL.md"
MANIFEST = f"{PLUGIN_ROOT}/.claude-plugin/plugin.json"


class Missing(Exception):
    pass


def show(repo: pathlib.Path, rev: str, path: str) -> str:
    result = subprocess.run(["git", "-C", str(repo), "show", f"{rev}:{path}"], capture_output=True, text=True)
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


def notes(line: str, repo: pathlib.Path, rev: str) -> str:
    commit = subprocess.run(
        ["git", "-C", str(repo), "rev-parse", "--verify", f"{rev}^{{commit}}"], capture_output=True, text=True
    )
    if commit.returncode != 0:
        raise Missing(f"{rev} is not a commit: {commit.stderr.strip()}")
    sha = commit.stdout.strip()

    if line == "delvec":
        e = engine_table(repo, sha)
        return (
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
        return (
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
        # The pin may name a tag this repository has not written yet: a plugin
        # release dispatched inside ADR-0029 §4's interval is exactly that state,
        # and its notes are still owed. So an unresolvable tag is PRINTED as
        # unborn rather than raised — the one fact a reader needs is which engine
        # release the page ships at, and that is the name itself.
        try:
            speaks = engine_table(repo, f"{pin['ref']}^{{commit}}")["dsl_crate_version"]
            speaks = f"**{speaks}**"
        except Missing:
            speaks = "the version that release states — the tag is not written yet"
        tree = subprocess.run(
            ["git", "-C", str(repo), "rev-parse", f"{sha}:{PLUGIN_ROOT}"], capture_output=True, text=True
        ).stdout.strip()
        if not tree:
            raise Missing(f"{sha}:{PLUGIN_ROOT} is not a tree")
        return (
            f"**`delvewright` {manifest['version']}** — the Delvewright Claude Code plugin (`/delvewright:new-delve`), "
            f"one of the three things this repository releases (the others are the `delvec` binary and the "
            f"`delvewright-dsl` format crate). This release moved `main` to this version. It does not by "
            f"itself deliver anything: the marketplace serves the plugin root at the engine release tag the "
            f"entry names, so this page reaches creators when an engine release is dispatched after it "
            f"(ADR-0029 §5).\n\n"
            f"- the page ships at engine release `{pin['ref']}`, and installs that engine\n"
            f"- it accepts `delvec` `{window}`\n"
            f"- that engine speaks `dsl_version` {speaks}\n"
            f"- plugin root tree `{tree}`, tagged at `{sha}`\n"
            f"- to hold a fixed page, add the marketplace at the ENGINE release it ships at: "
            f"`/plugin marketplace add stellarfeline/delvewright@{pin['ref']}`\n"
        )
    raise Missing(f"{line!r} is not a released line")


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description="the opening of a Release's notes")
    ap.add_argument("line", choices=("delvec", "delvewright-dsl", "delvewright"))
    ap.add_argument("--commit", required=True)
    ap.add_argument("--repo", type=pathlib.Path, default=REPO)
    args = ap.parse_args(argv)
    sys.stdout.reconfigure(newline="\n")  # CRLF-proof: tools/ci/check-python-shell-newlines.py
    try:
        print(notes(args.line, args.repo, args.commit), end="")
    except (Missing, KeyError, json.JSONDecodeError) as exc:
        print(f"release-notes: FAIL — {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
