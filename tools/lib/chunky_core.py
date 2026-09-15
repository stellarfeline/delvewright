"""The pinned Chunky core: what it IS, whether a Chunky home holds it, and the
classpath that renders with it — one rule for the installer, the render entry
point and the shot-set report.

## What "the pin" means

`versions.toml [render]` names the core every emitted scene was verified against
(`chunky_core`), the source revision it is built from (`chunky_revision`), and a
**content digest** of the core jar (`chunky_core_content_sha256`). A name is not
an identity — the update site serves today's jar under any name it is asked for
(`docs/reference/tools.md` §4a) — and a jar's own checksum is not one either,
because a jar is a zip and a zip records when it was written. So the digest is
over what the jar CONTAINS:

- every file entry, by name, in byte order of the name;
- each entry's sha256 over its bytes, except that a `.properties` entry has its
  `#` comment lines removed first — the build writes a timestamp there
  (`Version.properties`), and a comment is not read by the program.

Measured when the pin was recorded: the core the launcher installed and a
source build at `chunky_revision` differ in exactly one entry, the timestamp
comment of `se/llbit/chunky/main/Version.properties`, over 1276 entries; both
answer the same digest.

## What a home holding the pin looks like

Chunky's own layout: `<home>/versions/<core>.json` names the libraries the core
runs with (name, md5, size), and each one is a file in `<home>/lib/`. A launcher
install writes that record, and so does `validation/chunky-install.sh`. A home
holds the pin when that record exists, every library it names is in `lib/` with
the recorded md5, and the core jar answers the pinned content digest.

    python3 tools/lib/chunky_core.py digest <jar>
    python3 tools/lib/chunky_core.py verify --home <chunky home>
    python3 tools/lib/chunky_core.py classpath --home <chunky home>
    python3 tools/lib/chunky_core.py main-class --home <chunky home>
    python3 tools/lib/chunky_core.py record --home <chunky home> --libs <dir> --timestamp <iso>

`verify`, `classpath` and `main-class` exit 0 when the home holds the pin, 3 when it does not
(the message names the pin, the home, what is there and the remedy), and 2 when
the pin cannot be read. Stdlib only.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import shutil
import sys
import zipfile

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import versions  # noqa: E402

INSTALLER = "validation/chunky-install.sh"


def _normalised(name: str, data: bytes) -> bytes:
    if not name.endswith(".properties"):
        return data
    lines = data.split(b"\n")
    return b"\n".join(line for line in lines if not line.lstrip().startswith(b"#"))


def content_sha256(jar: pathlib.Path) -> str:
    """The content digest of a jar, as the module docstring defines it."""
    h = hashlib.sha256()
    with zipfile.ZipFile(jar) as z:
        names = sorted(i.filename for i in z.infolist() if not i.is_dir())
        for name in names:
            entry = hashlib.sha256(_normalised(name, z.read(name))).hexdigest()
            h.update(f"{entry}  {name}\n".encode())
    return h.hexdigest()


def md5(path: pathlib.Path) -> str:
    return hashlib.md5(path.read_bytes()).hexdigest().upper()


class Pin:
    """The two `[render]` pins that say what the core is."""

    def __init__(self, path: pathlib.Path | None = None):
        self.core = versions.chunky_core(path)
        self.digest = versions.chunky_core_content_sha256(path)


class NotHeld(Exception):
    """The home does not hold the pin; the message says what it holds instead."""


def installed_cores(home: pathlib.Path) -> list[str]:
    lib = home / "lib"
    if not lib.is_dir():
        return []
    return sorted(p.stem for p in lib.glob("chunky-core-*.jar") if p.is_file())


def held_classpath(home: pathlib.Path, pin: Pin) -> list[pathlib.Path]:
    """The classpath that renders with the pin, or [`NotHeld`] naming why not."""
    record = home / "versions" / f"{pin.core}.json"
    jar = home / "lib" / f"{pin.core}.jar"
    others = [c for c in installed_cores(home) if c != pin.core]
    tail = (
        f" Other cores in {home / 'lib'}: {', '.join(others)}." if others else ""
    ) + f" Install the pin with `{INSTALLER}`, which builds it from source at the pinned revision."
    if not jar.is_file():
        raise NotHeld(f"the pinned core {pin.core} is not installed: no {jar}.{tail}")
    got = content_sha256(jar)
    if got != pin.digest:
        raise NotHeld(
            f"{jar} is named as the pin and is not it: its content digest is {got}, "
            f"the pin's is {pin.digest}.{tail}"
        )
    if not record.is_file():
        raise NotHeld(
            f"the pinned core {pin.core} is in lib/ but {record} is missing, so nothing "
            f"names the libraries it runs with.{tail}"
        )
    try:
        libraries = json.loads(record.read_text(encoding="utf-8"))["libraries"]
        entries = [(lib["name"], lib["md5"].upper()) for lib in libraries]
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as e:
        raise NotHeld(f"{record} is not a Chunky version record ({e}).{tail}") from e
    if f"{pin.core}.jar" not in [name for name, _ in entries]:
        raise NotHeld(f"{record} does not name {pin.core}.jar among its libraries.{tail}")
    classpath = []
    for name, want in entries:
        path = home / "lib" / name
        if not path.is_file():
            raise NotHeld(f"{record} names {name}, and {path} does not exist.{tail}")
        if md5(path) != want:
            raise NotHeld(
                f"{path} does not carry the md5 {record} records for it ({want}).{tail}"
            )
        classpath.append(path)
    return classpath


def main_class(jar: pathlib.Path) -> str:
    """The `Main-Class` the core jar's own manifest names."""
    with zipfile.ZipFile(jar) as z:
        manifest = z.read("META-INF/MANIFEST.MF").decode("utf-8")
    for line in manifest.splitlines():
        if line.startswith("Main-Class:"):
            return line.split(":", 1)[1].strip()
    raise NotHeld(f"{jar} names no Main-Class in its manifest")


def write_record(
    home: pathlib.Path, libs: pathlib.Path, pin: Pin, timestamp: str
) -> pathlib.Path:
    """Copy a built core and its libraries into `home` and write the version
    record that names them, after holding the core to the pinned digest."""
    core = libs / f"{pin.core}.jar"
    if not core.is_file():
        raise NotHeld(f"the build produced no {core}; it built something other than the pin")
    got = content_sha256(core)
    if got != pin.digest:
        raise NotHeld(
            f"the build at the pinned revision produced {core} with content digest {got}, "
            f"and the pin's is {pin.digest}; nothing was installed"
        )
    jars = [core] + sorted(p for p in libs.glob("*.jar") if p != core)
    (home / "lib").mkdir(parents=True, exist_ok=True)
    (home / "versions").mkdir(parents=True, exist_ok=True)
    libraries = []
    for jar in jars:
        shutil.copyfile(jar, home / "lib" / jar.name)
        libraries.append(
            {"name": jar.name, "md5": md5(jar), "url": "", "size": jar.stat().st_size}
        )
    record = home / "versions" / f"{pin.core}.json"
    record.write_text(
        json.dumps(
            {"name": pin.core, "timestamp": timestamp, "libraries": libraries}, indent=2
        )
        + "\n",
        encoding="utf-8",
    )
    return record


def _main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(prog="chunky_core.py")
    sub = ap.add_subparsers(dest="cmd", required=True)
    d = sub.add_parser("digest")
    d.add_argument("jar", type=pathlib.Path)
    for name in ("verify", "classpath", "main-class"):
        s = sub.add_parser(name)
        s.add_argument("--home", type=pathlib.Path, required=True)
    r = sub.add_parser("record")
    r.add_argument("--home", type=pathlib.Path, required=True)
    r.add_argument("--libs", type=pathlib.Path, required=True)
    r.add_argument("--timestamp", required=True, help="the revision's commit time, ISO 8601")
    args = ap.parse_args(argv)

    if args.cmd == "digest":
        print(content_sha256(args.jar))
        return 0
    try:
        pin = Pin()
    except versions.PinError as e:
        print(f"chunky_core: {e}", file=sys.stderr)
        return 2
    try:
        if args.cmd == "record":
            print(write_record(args.home, args.libs, pin, args.timestamp))
            return 0
        classpath = held_classpath(args.home, pin)
    except NotHeld as e:
        print(f"chunky core: {e}", file=sys.stderr)
        return 3
    if args.cmd == "main-class":
        try:
            print(main_class(classpath[0].parent / f"{pin.core}.jar"))
        except NotHeld as e:
            print(f"chunky core: {e}", file=sys.stderr)
            return 3
    elif args.cmd == "verify":
        print(
            f"chunky core: pinned {pin.core} is installed at {args.home / 'lib'}, content "
            f"digest verified, with {len(classpath) - 1} librar(ies) at their recorded md5"
        )
    else:
        print(os.pathsep.join(str(p) for p in classpath))
    return 0


if __name__ == "__main__":
    raise SystemExit(_main(sys.argv[1:]))
