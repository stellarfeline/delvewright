"""`.github/dependabot.yml` covers every manifest this repository actually holds.

A dependabot config is a claim about a repository, and it is the kind of claim
nothing re-checks: a workspace added next month is simply not watched, and the
absence looks exactly like the presence — no red, no warning, no pull request.
That is the unbound vacuity mode, and it is why the config's directory list is
asserted against `git ls-files` here rather than trusted.

The glob alternative (`directories: ["/prefabs/*"]`) would have been derived and
silent: a glob that matches nothing is the same empty green in a shape that
looks like the fix.

Stdlib only — the tools suite installs pytest and nothing else, and a config
gate that needed a YAML library would be a gate a creator cannot run.
"""

from __future__ import annotations

import re
import subprocess
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent.parent
CONFIG = REPO / ".github" / "dependabot.yml"


def tracked(pattern: str) -> list[str]:
    r = subprocess.run(
        ["git", "-C", str(REPO), "ls-files", "-z", "--", pattern],
        capture_output=True,
        text=True,
        check=True,
    )
    return [p for p in r.stdout.split("\0") if p]


def entries() -> list[tuple[str, str]]:
    """(ecosystem, directory) pairs, read without a YAML library.

    The file is written in one shape — `- package-ecosystem: X` followed by
    `directory: "/…"` — and the parse asserts it saw both halves of every entry,
    so a reshaped file reds here instead of being read as an empty config.
    """
    text = CONFIG.read_text(encoding="utf-8")
    eco = re.findall(r"^\s*-\s*package-ecosystem:\s*(\S+)", text, re.M)
    dirs = re.findall(r'^\s*directory:\s*"([^"]+)"', text, re.M)
    assert len(eco) == len(dirs), (
        f"{len(eco)} `package-ecosystem` key(s) against {len(dirs)} `directory` "
        "key(s) — this reader has stopped matching the file it reads, and a "
        "reader that mis-parses reports coverage nobody has"
    )
    assert eco, "parsed ZERO update entries; an empty parse agrees with everything"
    return list(zip(eco, dirs))


def cargo_workspaces() -> set[str]:
    """Every cargo WORKSPACE root, derived from the tree.

    A workspace member is updated through its workspace root, so the population
    is the manifests carrying a `[workspace]` table — not every `Cargo.toml`.
    """
    out = set()
    for rel in tracked("*Cargo.toml"):
        if "[workspace]" in (REPO / rel).read_text(encoding="utf-8"):
            d = str(Path(rel).parent)
            out.add("/" if d == "." else "/" + d)
    return out


def test_every_cargo_workspace_in_the_tree_is_watched():
    want = cargo_workspaces()
    have = {d for eco, d in entries() if eco == "cargo"}
    assert want, "found ZERO cargo workspaces; the derivation has stopped working"
    missing = sorted(want - have)
    assert not missing, (
        f"{len(want)} cargo workspace(s) in the tree, {len(have)} watched; "
        f"unwatched: {missing}. A workspace nothing watches has no advisory "
        "remedy, and its absence looks exactly like its presence."
    )


def test_no_entry_names_a_directory_that_is_not_there():
    """The other direction: an entry for a deleted workspace is a config error
    dependabot reports where nobody in this repository reads it."""
    stale = [
        d
        for eco, d in entries()
        if eco == "cargo" and not (REPO / d.lstrip("/") / "Cargo.toml").is_file()
    ]
    assert not stale, f"dependabot names cargo directories with no manifest: {stale}"


def test_every_npm_manifest_is_watched():
    want = {
        "/" + str(Path(rel).parent) if str(Path(rel).parent) != "." else "/"
        for rel in tracked("*package.json")
        if "node_modules" not in rel
    }
    have = {d for eco, d in entries() if eco == "npm"}
    assert want, "found ZERO npm manifests; the derivation has stopped working"
    assert not (want - have), f"unwatched npm manifest(s): {sorted(want - have)}"


def test_the_actions_this_repository_runs_are_watched():
    assert ("github-actions", "/") in entries(), (
        "every action here is held at a MAJOR tag (`.github/pins.toml`, policy "
        "`floating`), so a tag can move under CI with nothing in any diff"
    )
