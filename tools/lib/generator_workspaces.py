"""The generator workspaces this repository holds — one discovery, every gate that asks.

A generator is a program outside the root cargo workspace that writes prefab
`.nbt` bytes: a tracked `Cargo.toml` that is not the root manifest, carries a
`[workspace]` table of its own (so the root's `cargo …--workspace` never reaches
it), and has a `src/main.rs` that serialises NBT (`fastnbt::to_bytes`). The
population is read off the disk, never listed, so a new generator is in it the
moment it is committed.

A manifest inside `tools/ci/fmt-workspaces.sh`'s exclusion prefix is a frozen
record (`docs/experiments/`), not a generator any gate owes anything; the prefix
is read from the sweep itself rather than restated here.

Two gates ask: `tools/ci/check-structure-emitters.py` (every generator is wired
into CI) and `tools/ci/check-generator-preservation.py` (every generator
deletes no key it did not write).
"""

from __future__ import annotations

import pathlib
import re
import subprocess
import tomllib

INGREDIENT = "fastnbt::to_bytes"


def fmt_excluded_prefix(root: pathlib.Path) -> str | None:
    """The path prefix `tools/ci/fmt-workspaces.sh` excludes, or None if unreadable."""
    script = root / "tools" / "ci" / "fmt-workspaces.sh"
    if not script.is_file():
        return None
    m = re.search(r'^EXCLUDED_PREFIX="([^"]*)"', script.read_text(), re.M)
    return m.group(1) if m else None


def tracked_manifests(root: pathlib.Path) -> list[str]:
    """Every git-tracked `Cargo.toml`, repo-relative, sorted."""
    out = subprocess.run(
        ["git", "-C", str(root), "ls-files", "-z", "--", "*Cargo.toml"],
        check=True,
        capture_output=True,
    ).stdout.decode()
    return sorted(f for f in out.split("\0") if f.endswith("Cargo.toml"))


def generator_workspaces(root: pathlib.Path) -> list[str]:
    """Every generator workspace, as the repo-relative directory of its manifest, sorted."""
    prefix = fmt_excluded_prefix(root)
    found: list[str] = []
    for rel in tracked_manifests(root):
        if "/" not in rel or (prefix and rel.startswith(prefix)):
            continue
        try:
            doc = tomllib.loads((root / rel).read_text())
        except (OSError, tomllib.TOMLDecodeError):
            continue
        if "workspace" not in doc or "package" not in doc:
            continue
        here = rel.rsplit("/", 1)[0]
        main = root / here / "src" / "main.rs"
        if main.is_file() and INGREDIENT in main.read_text(errors="replace"):
            found.append(here)
    return sorted(found)


def package_name(root: pathlib.Path, workspace: str) -> str | None:
    """The `[package] name` of a generator workspace — the binary cargo builds."""
    try:
        return tomllib.loads((root / workspace / "Cargo.toml").read_text())["package"]["name"]
    except (OSError, tomllib.TOMLDecodeError, KeyError):
        return None
