#!/usr/bin/env python3
"""Download the pinned game's client jar from Mojang, or refuse on the digest.

WHY THIS IS A SCRIPT

Every picture in this pipeline is drawn with Minecraft's own textures, and the
jar is in no repository of this project — it is never redistributed. So it is
fetched, and a fetch has an algorithm: walk Mojang's version manifest to the
version the ENGINE pins, take that version's `downloads.client`, and check the
sha1 Mojang publishes on the bytes as they arrive. Two agents left to their own
commands would differ on the URL, on whether the digest is checked at all, and
on what a mismatch means. Whether the jar is downloaded or copied is still the
user's decision and this script is only the download arm of it.

WHAT IS NOT A CONSTANT HERE

The manifest URL is the one `tools/check-patrol-types.py` and
`tools/derive-client-langs.py` already carry in the engine, and the version is
the engine's own `[minecraft]` pin — read from the checkout, never restated.
Point the same walk at `downloads.server` instead and it reproduces that same
file's committed `server_jar_url` and `server_jar_sha1` exactly, which is how a
reader knows the walk lands on the right game rather than merely on *a* jar.

**The client half has no committed pin to agree with.** The sha1 checked below
is Mojang's own, verified on the bytes: it proves the transfer and the version,
and nothing in this project would notice if Mojang republished. Say that when
you report; do not write a pin of your own onto the page.

    0  the jar is in place and its sha1 is Mojang's
    2  the engine checkout or its pin is unusable
    4  the download failed
    5  the sha1 does not match — a REFUSAL: nothing is written, nothing retried

    python3 scripts/fetch-client-jar.py --engine ~/.delvewright/engine
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import sys
import tomllib
import urllib.error
import urllib.request

MANIFEST = "https://launchermeta.mojang.com/mc/game/version_manifest_v2.json"

EXIT_UNUSABLE = 2
EXIT_DOWNLOAD = 4
EXIT_DIGEST = 5


class Refusal(Exception):
    def __init__(self, code: int, message: str) -> None:
        super().__init__(message)
        self.code = code


def get(url: str) -> bytes:
    try:
        with urllib.request.urlopen(url, timeout=300) as fh:
            return fh.read()
    except (urllib.error.URLError, OSError, TimeoutError) as exc:
        raise Refusal(EXIT_DOWNLOAD, f"could not download {url}: {exc}") from exc


def pinned_version(engine: pathlib.Path) -> str:
    """`[minecraft].version` in the engine checkout — the game this project pins."""
    manifest = engine / "versions.toml"
    try:
        data = tomllib.loads(manifest.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as exc:
        raise Refusal(
            EXIT_UNUSABLE,
            f"{manifest} is unusable: {exc}. That file is where the game version "
            f"lives (ADR-0009); a version written on the page instead would be a "
            f"literal nothing checks.",
        ) from exc
    version = data.get("minecraft", {}).get("version")
    if not isinstance(version, str) or not version:
        raise Refusal(EXIT_UNUSABLE, f"{manifest} has no `[minecraft].version`")
    return version


def client_download(version: str, fetch=get) -> dict:
    """Mojang's `downloads.client` entry for `version`."""
    index = json.loads(fetch(MANIFEST))
    entry = next((v for v in index["versions"] if v["id"] == version), None)
    if entry is None:
        raise Refusal(
            EXIT_UNUSABLE,
            f"{version} is not in Mojang's version manifest. The engine pins a "
            f"game Mojang does not list, which is a question for the pin and not "
            f"something this script may work around.",
        )
    return json.loads(fetch(entry["url"]))["downloads"]["client"]


def run(engine: pathlib.Path, out: pathlib.Path, fetch=get) -> int:
    version = pinned_version(engine)
    download = client_download(version, fetch)
    raw = fetch(download["url"])
    got = hashlib.sha1(raw).hexdigest()
    if got != download["sha1"]:
        raise Refusal(
            EXIT_DIGEST,
            f"the client jar for {version} hashes sha1 {got} and Mojang publishes "
            f"{download['sha1']}. **Refused** — nothing is written and nothing is "
            f"downloaded again. Every texture in every picture below would come "
            f"from bytes nobody can name.",
        )
    out.parent.mkdir(parents=True, exist_ok=True)  # or 31 MB dies on the last line
    out.write_bytes(raw)
    print(f"fetch-client-jar: ok — {version} at {out}, {len(raw)} bytes, sha1 {got}")
    return 0


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--engine",
        type=pathlib.Path,
        default=pathlib.Path(
            os.environ.get("DELVEWRIGHT_ENGINE", "~/.delvewright/engine")
        ),
        help="the engine checkout whose `[minecraft].version` names the game",
    )
    ap.add_argument(
        "--out",
        type=pathlib.Path,
        default=pathlib.Path("~/.chunky/resources/minecraft.jar"),
        help=(
            "where the jar lands (default: ~/.chunky/resources/minecraft.jar — "
            "the last of the three paths every texture-reading tool tries)"
        ),
    )
    args = ap.parse_args(argv)
    try:
        return run(args.engine.expanduser(), args.out.expanduser())
    except Refusal as refusal:
        print(f"fetch-client-jar: {refusal}", file=sys.stderr)
        return refusal.code


if __name__ == "__main__":
    raise SystemExit(main())
