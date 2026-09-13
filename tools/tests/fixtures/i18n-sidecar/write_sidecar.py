#!/usr/bin/env python3
"""Write one l10n sidecar with `tools/i18n-translate.py`'s own writer.

The driver exists so the writer can be judged by the engine's own formatter over
a REAL FILE rather than over a string: `crates/delvec/tests/i18n_sidecar.rs`
runs this, then runs `delvec fmt --check` on what it produced. Every sidecar the
tool had ever written was refused by that check (`DW0773`, error tier) — the
envelope was in dict-insertion order rather than canonical key order, and an
existing sidecar's `dsl_version` was carried forward where `delvec fmt` stamps
the version this engine implements.

The inventory below is deliberately shaped to red both of those:

* the envelope keys sort differently from the order the writer builds them in
  (`campaign_id` < `content` < `dsl_version` < `kind` < `lang` < `source`), and
* `dsl_version` is an old one, so a writer that preserves it leaves a document
  `delvec fmt --check` refuses.

Usage: write_sidecar.py <output-path> <delvec-binary>
"""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path

TOOL = Path(__file__).resolve().parents[3] / "i18n-translate.py"

INVENTORY = {
    "campaign_id": "keep-trial",
    # Not the version this engine implements: `delvec fmt` stamps it (ADR-0024),
    # so a document that still declares this one is not canonical.
    "dsl_version": "0.3.0",
    "lang": "zh-cn",
    "declared": True,
    "sidecar_present": False,
    "world_title": "The Stone Keep",
    "npcs": [],
    "entries": [
        {"key": "world.title", "en": "The Stone Keep"},
        {"key": "npc.keeper.name", "en": "The Keeper"},
        {"key": "quest.greet.goal", "en": "Meet the Keeper."},
    ],
}

CONTENT = {
    "world.title": "石垒要塞",
    "npc.keeper.name": "守关人",
    "quest.greet.goal": "去见守关人。",
}


def load_tool():
    spec = importlib.util.spec_from_file_location("i18n_translate", TOOL)
    mod = importlib.util.module_from_spec(spec)
    sys.modules["i18n_translate"] = mod
    spec.loader.exec_module(mod)
    return mod


def main(argv: list[str]) -> int:
    out, delvec = Path(argv[1]), argv[2]
    t = load_tool()
    inv = t.parse_inventory(json.loads(json.dumps(INVENTORY)))
    t.write_sidecar(out, inv, t.merge_content(inv, CONTENT), [delvec])
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
