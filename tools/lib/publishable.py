#!/usr/bin/env python3
"""Which files in this repo are served to a stranger by crates.io, derived.

WHY THIS IS A SHARED MODULE AND NOT A COPY IN EACH GATE

Two gates ask the same question — "which READMEs does a stranger read?" — and
answer it for different reasons (`check-crates-io-readmes.py` bans internal
references on them; `check-reference-versions.py` binds their version claims to
the build). The rule this repo has already paid for (CLAUDE.md) is
that a correct rule living privately inside ONE caller leaves the next caller
nothing to reuse, so the next caller writes a weaker version. The derivation
lives here once; each gate imports it.

THE DERIVATION

Every crate manifest under `crates/*/Cargo.toml` whose `[package] publish` is
not `false`, resolved through its `[package] readme` key. Nothing is named by
hand, so a crate that later becomes publishable — by deleting one
`publish = false` line — inherits both gates with **no edit to either gate**.

Globbing the directory rather than reading `[workspace] members` is deliberate
and load-bearing: `crates/render` is deliberately EXCLUDED from the workspace
(it carries its own `[workspace]` table, to keep a git dependency out of every
other crate's resolution), so a members-only derivation would never see it — and
`publish = false` is the only thing that keeps it off crates.io. A gate blind to
the one crate whose publishability is not visible from the workspace table is
the wrong gate.

The glob is then cross-checked against the root manifest: every path the root
`[workspace] members` or `exclude` names must have been discovered. A crate that
moves out from under `crates/` therefore reds as a derivation-shape error
instead of silently dropping out of both gates' binding count.

`readme` resolution follows Cargo: a string is a path relative to the manifest
directory; `true` or an absent key means `README.md` beside the manifest;
`false` means the crate deliberately publishes no front page and contributes no
file. A crate that DECLARES a readme path which does not exist is a derivation
error, never a silent zero.

Deterministic, offline, Python 3 stdlib only.
"""

from __future__ import annotations

import os
import tomllib
from dataclasses import dataclass
from pathlib import Path, PurePosixPath


class DerivationError(Exception):
    """The tree no longer has the shape the derivation reads.

    Raised rather than returned: every caller must treat it as red. A gate that
    caught this and carried on with an empty set would be the exact vacuity
    ("it matched zero objects") this repo names.
    """


@dataclass(frozen=True)
class PublishableCrate:
    """One crate crates.io may serve, and the page it serves for it."""

    name: str
    version: str
    rust_version: str | None
    manifest: Path
    readme: Path | None  # None only when `readme = false`

    def rel(self, root: Path) -> str:
        return PurePosixPath(os.path.relpath(self.manifest.parent, root)).as_posix()

    def readme_rel(self, root: Path) -> str:
        assert self.readme is not None
        return PurePosixPath(os.path.relpath(self.readme, root)).as_posix()


def _workspace_declared_paths(root: Path) -> list[str]:
    """Every crate path the root manifest names, member or excluded."""
    manifest = root / "Cargo.toml"
    if not manifest.is_file():
        return []
    ws = tomllib.loads(manifest.read_text(encoding="utf-8")).get("workspace", {})
    out: list[str] = []
    for key in ("members", "exclude"):
        for entry in ws.get(key, []):
            if isinstance(entry, str) and "*" not in entry:
                out.append(entry.rstrip("/"))
    return out


def discover(root: Path) -> list[PublishableCrate]:
    """Every publishable crate under `root`, sorted by name.

    Raises `DerivationError` when the tree's shape stops matching — no crate
    manifests at all, a manifest that will not parse, a declared readme that is
    not there, or a workspace-declared crate the glob did not reach.
    """
    manifests = sorted((root / "crates").glob("*/Cargo.toml"))
    if not manifests:
        raise DerivationError(
            f"no crate manifests under {root / 'crates'} — the workspace layout "
            "moved, and this derivation now returns nothing. Fix the glob in "
            "tools/lib/publishable.py; do NOT let it return an empty set."
        )

    found_dirs = {
        PurePosixPath(os.path.relpath(m.parent, root)).as_posix() for m in manifests
    }
    for declared in _workspace_declared_paths(root):
        if declared not in found_dirs:
            raise DerivationError(
                f"the root Cargo.toml names `{declared}`, which is not under "
                "`crates/*/` — this derivation globs that directory, so the "
                "crate is invisible to every gate built on it. Widen the glob "
                "in tools/lib/publishable.py."
            )

    # `[workspace.package]` is where an inherited `version` / `rust-version`
    # lives (a member says `version.workspace = true`); read it once so a page's
    # claim binds to the number cargo will actually publish.
    try:
        ws_pkg = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
        ws_pkg = ws_pkg.get("workspace", {}).get("package", {})
    except (OSError, tomllib.TOMLDecodeError) as exc:
        raise DerivationError(f"{root / 'Cargo.toml'}: {exc}") from exc

    def inherited(pkg: dict, key: str):
        value = pkg.get(key)
        if isinstance(value, dict) and value.get("workspace") is True:
            if key not in ws_pkg:
                raise DerivationError(
                    f"a crate inherits `{key}` from the workspace and the root "
                    f"Cargo.toml's [workspace.package] has no `{key}`"
                )
            return ws_pkg[key]
        return value

    crates: list[PublishableCrate] = []
    for manifest in manifests:
        try:
            data = tomllib.loads(manifest.read_text(encoding="utf-8"))
        except (OSError, tomllib.TOMLDecodeError) as exc:
            raise DerivationError(f"{manifest}: {exc}") from exc
        pkg = data.get("package")
        if not isinstance(pkg, dict):
            continue  # a virtual manifest publishes nothing
        # Cargo: `publish = false` forbids publishing; a LIST names the
        # registries it may go to, which still makes the README a served page.
        if pkg.get("publish", True) is False:
            continue

        # Cargo: a string is a path relative to the manifest; `true` means
        # `README.md` beside it; `false` means "publish no front page"; an ABSENT
        # key auto-detects `README.md` and is content for it not to be there.
        # The distinction is why a missing file is an error only when the
        # manifest DECLARED one — a broken declaration is a defect, a crate that
        # simply has no README is a crate with no page.
        declared = "readme" in pkg
        readme_key = pkg.get("readme", True)
        readme: Path | None
        if readme_key is False:
            readme = None
        else:
            name = "README.md" if readme_key is True else str(readme_key)
            readme = manifest.parent / name
            if not readme.is_file():
                if declared:
                    raise DerivationError(
                        f"{os.path.relpath(manifest, root)} declares "
                        f"`readme = {name!r}` and that file does not exist — "
                        "crates.io would serve the crate with no front page."
                    )
                readme = None

        crates.append(
            PublishableCrate(
                name=str(pkg.get("name", manifest.parent.name)),
                version=str(inherited(pkg, "version") or ""),
                # Cargo spells it `rust-version`; tomllib does not fold the
                # hyphen, so reading `rust_version` silently yields None and the
                # page's minimum-Rust claim binds to nothing while staying green.
                rust_version=(
                    str(inherited(pkg, "rust-version"))
                    if "rust-version" in pkg
                    else None
                ),
                manifest=manifest,
                readme=readme,
            )
        )

    return sorted(crates, key=lambda c: c.name)


def readmes(root: Path) -> list[PublishableCrate]:
    """The publishable crates that actually serve a README page."""
    return [c for c in discover(root) if c.readme is not None]
