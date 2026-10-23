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

The manifest URL is the one `tools/maintenance/check-patrol-types.py` and
`tools/maintenance/derive-client-langs.py` already carry in the engine, and the version is
the engine's own `[minecraft]` pin — read from the checkout, never restated.
Point the same walk at `downloads.server` instead and it reproduces that same
file's committed `server_jar_url` and `server_jar_sha1` exactly, which is how a
reader knows the walk lands on the right game rather than merely on *a* jar.

**The bytes are checked twice.** Mojang's own sha1, verified as they arrive,
proves the transfer; the engine's `[minecraft].client_jar_sha256` — the pin every
frame `delvec` draws is drawn against — proves they are the jar this engine
pins, so a jar Mojang republished is refused rather than drawn from.

        0  the jar is in place, its sha1 Mojang's and its sha256 the engine's pin
    2  the engine checkout or its pin is unusable
    4  the download failed
        5  a digest does not match — a REFUSAL: nothing is written, nothing retried

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
    return pinned(engine)["version"]


def pinned(engine: pathlib.Path) -> dict:
    """The engine checkout's `[minecraft]` table: `version`, and the client
    jar's `client_jar_sha256` when the engine pins one."""
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
    table = data.get("minecraft", {})
    version = table.get("version")
    if not isinstance(version, str) or not version:
        raise Refusal(EXIT_UNUSABLE, f"{manifest} has no `[minecraft].version`")
    return table


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


def default_out(version: str) -> pathlib.Path:
    """Where the jar lands by default — the last rung of every `delvec` arm's jar
    ladder (`--textures`, `$DELVEWRIGHT_CLIENT_JAR`, then this file)."""
    return pathlib.Path("~/.delvewright/client").expanduser() / f"minecraft-{version}.jar"


def run(engine: pathlib.Path, out: pathlib.Path | None, fetch=get) -> int:
    pin = pinned(engine)
    version = pin["version"]
    out = out if out is not None else default_out(version)
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
    want = pin.get("client_jar_sha256")
    got256 = hashlib.sha256(raw).hexdigest()
    if want and got256 != want:
        raise Refusal(
            EXIT_DIGEST,
            f"the client jar for {version} hashes sha256 {got256} and the engine "
            f"pins {want} (`[minecraft].client_jar_sha256`). **Refused** — nothing "
            f"is written: Mojang's own sha1 matched, so these are bytes Mojang "
            f"publishes now and not the bytes this engine draws its frames with.",
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
        default=None,
        help=(
            "where the jar lands (default: ~/.delvewright/client/minecraft-<version>.jar "
            "— the last of the three paths every texture-reading tool tries)"
        ),
    )
    args = ap.parse_args(argv)
    try:
        return run(
            args.engine.expanduser(),
            None if args.out is None else args.out.expanduser(),
        )
    except Refusal as refusal:
        print(f"fetch-client-jar: {refusal}", file=sys.stderr)
        return refusal.code


if __name__ == "__main__":
    raise SystemExit(main())
