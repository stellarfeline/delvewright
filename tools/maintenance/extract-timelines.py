#!/usr/bin/env python3
"""Check (or rewrite) the two vendored timelines against the PINNED server jar.

`crates/dsl/data/timeline-day-1.21.11.json` and `timeline-moon-1.21.11.json`
are `data/minecraft/timeline/day.json` and `moon.json` of the pinned 1.21.11
server, copied byte for byte with one trailing newline appended (the only
difference `delvec fmt --check` asks for; the jar's files end at `}`). Every
reading of the clock the engine makes — the moon's eight phase names, the sun's
eased angle track, the sky-light and monster-burn keyframes — is taken from
these files (spec-0081 §4.1).

    tools/maintenance/extract-timelines.py <server.jar>            # compare; exit 1 on any difference
    tools/maintenance/extract-timelines.py <server.jar> --write    # rewrite the vendored files

The jar is the launcher download `versions.toml` `[minecraft]` pins
(`server_jar_url`, `server_jar_sha256`) and is refused unless its sha256 matches
the pin. The timelines live in the bundled inner jar
`META-INF/versions/<version>/server-<version>.jar`. A comparison that finds a
difference refuses and names the file: a pin bump is a `--write` and a review of
the diff, never a silent drift.

Offline once the jar is on disk, stdlib-only python3. A REGENERATION tool: CI
reads the committed files and never runs this.
"""

from __future__ import annotations

import argparse
import hashlib
import io
import pathlib
import re
import sys
import zipfile

ROOT = pathlib.Path(__file__).resolve().parents[2]
VERSIONS_TOML = ROOT / "versions.toml"
DATA = ROOT / "crates" / "dsl" / "data"
TIMELINES = ("day", "moon")


def fail(msg: str) -> None:
    sys.exit(f"extract-timelines: {msg}")


def read_pin() -> dict[str, str]:
    text = VERSIONS_TOML.read_text(encoding="utf8")
    section = text.split("[minecraft]", 1)
    if len(section) != 2:
        fail(f"{VERSIONS_TOML} has no [minecraft] section")
    body = section[1].split("\n[", 1)[0]
    out = {}
    for key in ("version", "server_jar_sha256"):
        m = re.search(rf'^{key}\s*=\s*"([^"]+)"', body, re.M)
        if not m:
            fail(f"versions.toml [minecraft] has no {key}")
        out[key] = m.group(1)
    return out


def extract(jar: pathlib.Path, version: str) -> dict[str, bytes]:
    with zipfile.ZipFile(jar) as outer:
        inner_name = f"META-INF/versions/{version}/server-{version}.jar"
        try:
            inner_bytes = outer.read(inner_name)
        except KeyError:
            fail(f"{jar} holds no {inner_name}")
    sys.stderr.write(
        f"  {inner_name}: sha256 {hashlib.sha256(inner_bytes).hexdigest()}\n"
    )
    out = {}
    with zipfile.ZipFile(io.BytesIO(inner_bytes)) as inner:
        for name in TIMELINES:
            path = f"data/minecraft/timeline/{name}.json"
            try:
                raw = inner.read(path)
            except KeyError:
                fail(f"the bundled server jar holds no {path}")
            out[name] = raw + b"\n"
    return out


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    ap.add_argument("jar", type=pathlib.Path, help="the pinned launcher server jar")
    ap.add_argument("--write", action="store_true", help="rewrite the vendored files")
    args = ap.parse_args(argv)

    pin = read_pin()
    got = hashlib.sha256(args.jar.read_bytes()).hexdigest()
    if got != pin["server_jar_sha256"]:
        fail(f"{args.jar}: sha256 {got}, pin says {pin['server_jar_sha256']} — refusing")
    sys.stderr.write(f"  {args.jar.name}: sha256 matches the pin\n")

    extracted = extract(args.jar, pin["version"])
    differ = []
    for name, data in extracted.items():
        dest = DATA / f"timeline-{name}-{pin['version']}.json"
        if args.write:
            dest.write_bytes(data)
            sys.stderr.write(f"  wrote {dest.relative_to(ROOT)} ({len(data)} bytes)\n")
            continue
        have = dest.read_bytes() if dest.is_file() else None
        if have != data:
            differ.append(str(dest.relative_to(ROOT)))
        else:
            sys.stderr.write(f"  {dest.relative_to(ROOT)}: identical ({len(data)} bytes)\n")
    if differ:
        fail(
            "the pinned jar's timelines differ from the vendored files: "
            + ", ".join(differ)
            + ". Rerun with --write and review the diff; nothing reads a timeline the "
            "pinned jar does not ship"
        )
    print(f"extract-timelines: {len(extracted)} of {len(TIMELINES)} timeline(s) checked")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
