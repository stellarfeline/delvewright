#!/usr/bin/env python3
"""A review render reaches the scene's declared sample budget.

`delvec scene` writes each scene's budget as `sppTarget`. Chunky re-saves a scene
it renders with the `-target` it was run with, so a draft (`-target 64`) leaves
64 in the file and the next pass, run without `-target`, finds its goal already
met and renders nothing more. This is the one rule that closes that, shared by
every render that goes through `validation/chunky.sh`:

  * the budget is recorded beside the scene, in `<scene>.budget`, the first time
    the scene is rendered while its file still holds what the emitter wrote;
  * a render that names no `-target` is handed that recorded budget;
  * emission deletes `<scene>.budget` with the caches, so a re-emitted scene is
    judged by its new budget.

Usage: chunky_budget.py resolve <scene-dir> <scene> [--explicit]
Prints the `-target` value to add (nothing when none is owed): `--explicit` says
the caller named its own `-target`, which is honoured and only recorded around.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path


def resolve(scene_dir: Path, scene: str, explicit: bool) -> str | None:
    sidecar = scene_dir / f"{scene}.budget"
    if not sidecar.is_file():
        try:
            target = json.loads((scene_dir / f"{scene}.json").read_text())["sppTarget"]
        except (OSError, ValueError, KeyError, TypeError):
            return None
        if not isinstance(target, int) or isinstance(target, bool) or target <= 0:
            return None
        sidecar.write_text(f"{target}\n")
    if explicit:
        return None
    text = sidecar.read_text().strip()
    return text if text.isdigit() and int(text) > 0 else None


def main(argv: list[str]) -> int:
    if len(argv) not in (4, 5) or argv[1] != "resolve" or (len(argv) == 5 and argv[4] != "--explicit"):
        print(__doc__, file=sys.stderr)
        return 2
    out = resolve(Path(argv[2]), argv[3], len(argv) == 5)
    if out:
        print(out)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
