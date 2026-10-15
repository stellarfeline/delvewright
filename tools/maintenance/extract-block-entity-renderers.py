#!/usr/bin/env python3
"""Derive the blocks whose block model draws nothing, from the pinned client jar.

`crates/delvec/data/block-entity-renderers-<version>.json` is what the
proximity highlight reads (spec-0104 §7.2): a highlighted block is outlined
through a `minecraft:block_display` of its own state, and a `block_display`
draws the block's MODEL. A block the client draws through a block-entity
renderer instead — a chest, a sign, a banner, a skull, a bed, a shulker box —
has a model with no elements, so its display would draw nothing and glow
around nothing. For those the engine summons an `item_display` of the block's
item instead.

The table has two halves:

    derived    every block whose blockstate file references only models that,
               followed up their `parent` chain, carry no `elements` — read off
               `assets/minecraft/blockstates/*.json` and
               `assets/minecraft/models/**.json` in the jar;
    authored   blocks whose model draws only part of the thing, so the derived
               rule cannot see them, each with its reason. These are the
               engine's statement and are confirmed by eye on the demo level.

The jar is REFUSED unless its sha256 is `versions.toml` `[render]
textures_sha256` (the client jar the texture census pins). The table states
that sha256 in its header.

    python3 tools/maintenance/extract-block-entity-renderers.py <client jar>
    python3 tools/maintenance/extract-block-entity-renderers.py <client jar> --check

Human-in-the-loop: never run by CI or by a build. Output is `delvec fmt`
canonical (sorted keys, two-space indent, raw UTF-8, one trailing newline).
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
import zipfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(REPO / "tools" / "lib"))
import versions  # noqa: E402

BLOCKSTATES = "assets/minecraft/blockstates/"
MODELS = "assets/minecraft/models/"

# Blocks whose model draws part of the thing and a block-entity renderer the
# rest. The derived rule reads a model with elements as drawn, so these are
# stated here, each with the part the model leaves out.
AUTHORED = {
    "minecraft:bell": "the block model draws the frame or the wall bracket; the bell itself is drawn by the bell's block-entity renderer",
}


def die(msg: str) -> "None":
    print(f"extract-block-entity-renderers: FAIL — {msg}", file=sys.stderr)
    raise SystemExit(1)


def model_refs(state: dict) -> list[str]:
    refs: list[str] = []
    for v in (state.get("variants") or {}).values():
        for m in v if isinstance(v, list) else [v]:
            refs.append(m["model"])
    for part in state.get("multipart") or []:
        apply = part["apply"]
        for m in apply if isinstance(apply, list) else [apply]:
            refs.append(m["model"])
    return refs


def has_elements(jar: zipfile.ZipFile, names: set[str], ref: str) -> bool:
    seen: set[str] = set()
    while ref and ref not in seen:
        seen.add(ref)
        path = ref.split(":", 1)[-1]
        if path.startswith("builtin/"):
            return False
        name = f"{MODELS}{path}.json"
        if name not in names:
            die(f"model `{ref}` is referenced and not in the jar")
        model = json.loads(jar.read(name).decode("utf-8"))
        if model.get("elements"):
            return True
        ref = model.get("parent", "")
    return False


def derive(jar: zipfile.ZipFile) -> list[str]:
    names = set(jar.namelist())
    out: list[str] = []
    for name in sorted(names):
        if not name.startswith(BLOCKSTATES) or not name.endswith(".json"):
            continue
        state = json.loads(jar.read(name).decode("utf-8"))
        refs = model_refs(state)
        if refs and not any(has_elements(jar, names, r) for r in refs):
            out.append("minecraft:" + name[len(BLOCKSTATES) : -len(".json")])
    return out


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("jar", help="the pinned Minecraft client jar")
    ap.add_argument("--check", action="store_true", help="compare against the committed table, write nothing")
    args = ap.parse_args()

    pin = versions.load()
    mc = pin["minecraft"]["version"]
    want = pin["render"]["textures_sha256"]
    got = hashlib.sha256(Path(args.jar).read_bytes()).hexdigest()
    if got != want:
        die(f"{args.jar} has sha256 {got}; versions.toml [render] textures_sha256 is {want}")
    with zipfile.ZipFile(args.jar) as jar:
        derived = derive(jar)
    if not derived:
        die("no blockstate references only element-less models — the table would bind nothing")
    overlap = sorted(set(derived) & set(AUTHORED))
    if overlap:
        die(f"authored rows the jar already derives: {overlap}")
    doc = {"authored": AUTHORED, "client_jar_sha256": got, "derived": derived, "minecraft": mc}
    text = json.dumps(doc, indent=2, sort_keys=True, ensure_ascii=False) + "\n"
    out = REPO / "crates" / "delvec" / "data" / f"block-entity-renderers-{mc}.json"
    print(
        f"extract-block-entity-renderers: {len(derived)} derived, {len(AUTHORED)} authored, "
        f"from {args.jar} (sha256 {got})"
    )
    if args.check:
        have = out.read_text(encoding="utf-8") if out.exists() else None
        if have != text:
            die(f"{out.relative_to(REPO)} differs from what this jar derives — regenerate and commit it")
        print(f"extract-block-entity-renderers: {out.relative_to(REPO)} is what this jar derives, byte for byte")
        return 0
    out.write_text(text, encoding="utf-8")
    print(f"extract-block-entity-renderers: wrote {out.relative_to(REPO)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
