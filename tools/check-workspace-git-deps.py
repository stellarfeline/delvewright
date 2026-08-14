#!/usr/bin/env python3
"""No cargo workspace in this repo resolves a git dependency (except one).

A git dependency is loaded during RESOLUTION of the workspace that declares it,
not during the build of the crate that uses it: cargo has to read the manifest at
the pinned rev before it can validate `Cargo.lock`, so it clones before it decides
what to compile. A cargo workspace resolves ALL of its members, and neither
`-p <one-crate>` nor `--locked` nor marking the dependency `optional` narrows that
(all three measured — see the note beside `exclude` in /Cargo.toml).

So one git dependency anywhere in a workspace is a network reach that every cargo
command against that workspace must complete first, whether or not it builds the
crate that declared it — and every CI job that runs cargo then answers for that
host's uptime. `delvewright-render`'s Nucleation pin put five required status
checks behind two repositories they never build, and a transient TLS failure on
that reach reddened `tier 2` on a docs-only PR (#388). This repo refuses the same
shape at two other sites: the `docs` job's `lychee --offline`, and task #41's
single Mojang fetch for the whole tier-2 job.

WHAT THIS CHECKS. Every `Cargo.lock` in the repo, which is exactly the resolved
dependency graph of the workspace that owns it. A package with a `source` of
`git+…` is a git dependency in that graph. `ALLOWED` names the locks that may
carry one, each with the reason it is quarantined there; a lock that is
allowlisted and carries NONE is also reported, since an allowlist entry that has
expired is how an exemption outlives its reason.

WHAT IT DOES NOT CHECK. That the excluded crate is still excluded — it does not
need to. If `crates/render` re-enters the root workspace, the root lock gains the
git packages and this reds on the lock, which is the property that actually
matters. Nor does it check registry (crates.io) dependencies: those are the
package manager working as designed, are content-addressed and cached, and
removing them is not on the table.

Deterministic, offline, stdlib-only. Exit 0 = pass, 1 = a finding.
"""

from __future__ import annotations

import os
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent

# lock path (repo-relative) -> why a git dependency is permitted there.
ALLOWED: dict[str, str] = {
    "crates/render/Cargo.lock": (
        "the render layer pins Nucleation by git rev because no crates.io "
        "release carries its `rendering` feature (versions.toml [render]); the "
        "crate is its own workspace so that this reach belongs to it alone, and "
        "one named CI step fetches it"
    ),
}

# Directories that are not this repo's own source. `campaigns` is a checkout of
# the CONTENT repo (a symlink locally, a checkout in CI) and is gated by its own
# CI; build/vendor trees are not authored here.
SKIP_DIRS = {".git", "target", "node_modules", "campaigns", "content-repo", "dist"}

_PACKAGE_RE = re.compile(
    r"^\[\[package\]\]$(?P<body>.*?)(?=^\[\[|\Z)", re.M | re.S
)
_FIELD_RE = re.compile(r'^(?P<key>name|version|source) = "(?P<val>[^"]*)"$', re.M)


def git_packages(lock: pathlib.Path) -> list[str]:
    """`name version` for every package in `lock` sourced from a git remote."""
    found = []
    for block in _PACKAGE_RE.finditer(lock.read_text(encoding="utf-8")):
        fields = {m["key"]: m["val"] for m in _FIELD_RE.finditer(block["body"])}
        if fields.get("source", "").startswith("git+"):
            found.append(
                f"{fields.get('name', '?')} {fields.get('version', '?')} "
                f"({fields['source']})"
            )
    return found


def locks() -> list[pathlib.Path]:
    """Every `Cargo.lock` this repo authors. Symlinks are never followed."""
    out = []
    for dirpath, dirnames, filenames in os.walk(ROOT, followlinks=False):
        dirnames[:] = sorted(
            d
            for d in dirnames
            if d not in SKIP_DIRS
            and not d.startswith(".")
            and not pathlib.Path(dirpath, d).is_symlink()
        )
        if "Cargo.lock" in filenames:
            lock = pathlib.Path(dirpath, "Cargo.lock")
            if not lock.is_symlink():
                out.append(lock)
    return sorted(out)


def main() -> int:
    found_locks = locks()
    if not found_locks:
        print(
            "check-workspace-git-deps: FAIL — examined 0 Cargo.lock files. This "
            "gate binds to nothing; the repo layout moved out from under it.",
            file=sys.stderr,
        )
        return 1

    errors: list[str] = []
    n_git = 0
    for lock in found_locks:
        rel = str(lock.relative_to(ROOT))
        pkgs = git_packages(lock)
        allowed = rel in ALLOWED
        if pkgs and not allowed:
            n_git += len(pkgs)
            errors.append(
                f"{rel} resolves {len(pkgs)} git dependenc"
                f"{'y' if len(pkgs) == 1 else 'ies'}:\n"
                + "".join(f"    {p}\n" for p in pkgs)
                + "  Every cargo command against this workspace must clone those\n"
                "  before it does anything, including commands that never build\n"
                "  the crate that declared them — which puts every CI job that\n"
                "  runs cargo here behind that host's uptime (#388). Move the\n"
                "  crate that needs it into its own workspace (`exclude` in the\n"
                "  root Cargo.toml + a `[workspace]` table of its own), or add\n"
                "  this lock to ALLOWED in tools/check-workspace-git-deps.py\n"
                "  with the reason it must stay."
            )
        elif allowed and not pkgs:
            errors.append(
                f"{rel} is allowlisted for a git dependency and has none. The "
                f"exemption has outlived its reason — drop it from ALLOWED "
                f"(recorded reason: {ALLOWED[rel]})."
            )
        elif pkgs:
            n_git += len(pkgs)

    for e in errors:
        print(f"check-workspace-git-deps: FAIL — {e}", file=sys.stderr)

    verdict = "FAIL" if errors else "OK"
    print(
        f"check-workspace-git-deps: {verdict} — {len(found_locks)} Cargo.lock "
        f"examined, {len(ALLOWED)} allowlisted, {n_git} git dependenc"
        f"{'y' if n_git == 1 else 'ies'} resolved in total."
    )
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
