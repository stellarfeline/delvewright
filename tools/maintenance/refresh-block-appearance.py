#!/usr/bin/env python3
"""Re-derive `crates/delvec/data/block-appearance-1.21.11.json` from a client jar,
and prove the result against the jar before it is committed.

## What the file is

The colour, coverage and model bounds of every block the pinned Minecraft
version has, at its default state. The CPU draft rasteriser (`delvec snapshot`,
`delvec cameras --preview`, `delvec edit preview`) paints from it, so a creator's
draft shows the same materials the GPU render textures from the jar. See
`crates/delvec/data/PROVENANCE.md`.

## Why the regeneration and the proof are one command

The jar is EULA-bound and is never committed, so CI has none and can only check
the table against itself. The single occasion a jar IS in hand is the occasion
the table changes — ADR-0009's pin moving — and that is this command. So the
proof runs here: the `derive-block-appearance` example writes the table, and the
jar-gated half of `crates/delvec/tests/preview_palette.rs` re-derives every entry
and compares. A regeneration whose proof is a test somebody may remember to type
is a regeneration nobody proved.

## Why an example and not a `delvec` subcommand

`delvec` is what an authoring session runs, so a flag on it is author-facing
surface that owes a demo level. Regenerating a table this repository commits is
not something a creator does — they never hold the file, and the release archive
they install cannot carry the program at all. The derivation itself is still the
one `delvec palette` and the interactive viewer run
(`compiler::view::blockcolor::Deriver`); only the caller differs.

The other half of the binding is in CI and needs no jar: the table records the
version its source declared, and `the_vendored_table_is_the_pinned_version_s`
holds it to the engine's pin. A pin bump that skips this command is red.

## Usage

    python3 tools/maintenance/refresh-block-appearance.py <minecraft-client.jar>

Run when ADR-0009's Minecraft pin moves, or when the derivation changes. The
table's path carries the pin — `crates/delvec/data/block-appearance-<pin>.json`,
named by `blockcolor.rs`'s `include_str!` — so a pin bump leaves one file per
version behind unless the superseded one is deleted in the same commit.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
CALLER = "refresh-block-appearance"


def die(msg: str) -> None:
    print(f"{CALLER}: FAIL — {msg}", file=sys.stderr)
    raise SystemExit(1)


def run(argv: list[str], **kw) -> subprocess.CompletedProcess:
    print(f"{CALLER}: $ {' '.join(argv)}", file=sys.stderr)
    return subprocess.run(argv, cwd=REPO, **kw)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("jar", help="the pinned Minecraft client jar (or an unpacked directory)")
    args = ap.parse_args()

    jar = Path(args.jar).expanduser()
    if not jar.exists():
        die(f"{jar} does not exist")

    # The engine's own pin, read from the crate rather than typed here.
    pin = None
    blocks_rs = (REPO / "crates/dsl/src/blocks.rs").read_text(encoding="utf-8")
    for line in blocks_rs.splitlines():
        if line.strip().startswith("pub const MC_VERSION"):
            pin = line.split('"')[1]
            break
    if not pin:
        die("crates/dsl/src/blocks.rs declares no MC_VERSION")
    out = REPO / "crates/delvec/data" / f"block-appearance-{pin}.json"

    r = run(
        [
            "cargo",
            "run",
            "--release",
            "-q",
            "-p",
            "delvec",
            "--example",
            "derive-block-appearance",
            "--",
            str(jar),
            str(out),
        ]
    )
    if r.returncode != 0:
        die(f"`derive-block-appearance` exited {r.returncode}")

    table = json.loads(out.read_text(encoding="utf-8"))
    entries = len(table.get("entries") or {})
    unresolved = table.get("unresolved") or {}
    if table.get("mc_version") != pin:
        die(
            f"{out.name} says mc_version {table.get('mc_version')!r} and the engine is pinned to "
            f"{pin!r}: {jar} is not the pinned jar"
        )
    if entries == 0:
        die(f"{out.name} holds no entry")

    # The proof. Re-derives every entry from the same jar through the test's own
    # copy of the derivation and compares it to what was just written.
    r = run(
        [
            "cargo",
            "test",
            "-p",
            "delvec",
            "--test",
            "preview_palette",
            "--",
            "--ignored",
            "--nocapture",
        ],
        env={**os.environ, "DELVEWRIGHT_CLIENT_JAR": str(jar)},
    )
    if r.returncode != 0:
        die(f"the jar-gated comparison exited {r.returncode} — the table is not what the jar says")

    print(
        f"{CALLER}: {out.relative_to(REPO)} — {entries} block(s) at Minecraft {pin}, "
        f"{len(unresolved)} unresolved, every entry re-derived from {jar} and compared",
        file=sys.stderr,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
