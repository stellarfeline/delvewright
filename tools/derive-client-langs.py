#!/usr/bin/env python3
"""Derive the pinned client's language-file set from Mojang's own metadata.

`crates/dsl/src/mclang.rs` bakes the result in, because the compiler must never
reach the network during a build (ADR-0006). This script is how that table was
produced and how it is re-produced when the pinned Minecraft version moves
(ADR-0009): it walks version manifest -> the pinned version's metadata -> its
asset index, and prints every `minecraft/lang/<code>.json` it enumerates, plus
the sha1 of each document it read so the derivation is auditable.

    python3 tools/derive-client-langs.py            # pinned version from versions.toml
    python3 tools/derive-client-langs.py --version 1.21.11 --rust

Human-in-the-loop: run it, diff the printed table against `mclang.rs`, commit.
It is never run by CI or by a build.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
import urllib.request
from pathlib import Path

MANIFEST = "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json"
LANG_RE = re.compile(r"^minecraft/lang/(.+)\.json$")


def fetch(url: str) -> tuple[dict, str]:
    """Fetch a JSON document, returning it with the sha1 of its exact bytes."""
    with urllib.request.urlopen(url, timeout=120) as r:
        raw = r.read()
    return json.loads(raw), hashlib.sha1(raw).hexdigest()


def pinned_version(repo: Path) -> str:
    """The Minecraft version `versions.toml` pins (ADR-0009)."""
    text = (repo / "versions.toml").read_text(encoding="utf-8")
    section = text.split("[minecraft]", 1)
    if len(section) != 2:
        sys.exit("versions.toml has no [minecraft] section")
    m = re.search(r'^\s*version\s*=\s*"([^"]+)"', section[1], re.M)
    if not m:
        sys.exit('versions.toml [minecraft] declares no `version = "<version>"` pin')
    return m.group(1)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--version", help="Minecraft version (default: the versions.toml pin)")
    ap.add_argument("--rust", action="store_true", help="print the table as Rust literals")
    args = ap.parse_args()

    repo = Path(__file__).resolve().parent.parent
    version = args.version or pinned_version(repo)

    manifest, manifest_sha = fetch(MANIFEST)
    entry = next((v for v in manifest["versions"] if v["id"] == version), None)
    if entry is None:
        sys.exit(f"version `{version}` is not in the manifest")
    meta, meta_sha = fetch(entry["url"])
    index_url = meta["assetIndex"]["url"]
    index, index_sha = fetch(index_url)

    langs = sorted(
        m.group(1) for k in index["objects"] if (m := LANG_RE.match(k)) is not None
    )
    # `en_us` lives inside the jar, not in the asset index, so it is added here.
    langs = ["en_us"] + langs

    print(f"# minecraft            {version}")
    print(f"# version manifest     {MANIFEST}  sha1 {manifest_sha}")
    print(f"# version metadata     {entry['url']}  sha1 {meta_sha}")
    print(f"# asset index \"{meta['assetIndex']['id']}\"      {index_url}  sha1 {index_sha}")
    print(f"# codes                {len(langs)} ({len(langs) - 1} from the index + en_us)")

    if not args.rust:
        for c in langs:
            print(c)
        return 0

    line = "    "
    for c in langs:
        tok = f'"{c}", '
        if len(line) + len(tok) > 92:
            print(line.rstrip())
            line = "    "
        line += tok
    print(line.rstrip())
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
