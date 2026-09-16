#!/usr/bin/env python3
"""Put the pinned `delvec` on this machine, or refuse with a code that says why.

WHAT THIS REMOVES, AND WHY IT IS A SCRIPT AT ALL

It saves the agent no effort it would otherwise spend. It **removes its
choices**. Which archive, which checksum tool, how to read a `SHA256SUMS` row,
whether a failed download is retried, whether a mismatched digest is "probably
fine" — every one of those is a place two agents could diverge and two creators
end up with two toolchains, from one page. The engine's version decides which
bytes a campaign compiles into; a creator who differs in it gets different
bytes, which is the determinism ADR-0006 promises. So this is the one
acquisition on the page that is not left to the agent, and the reason is
exactness, never convenience.

The shell form it replaces got one thing wrong that no reader would have caught.
It extracted the archive's row with `grep " $ARCHIVE\\$"`, and coreutils writes
a `SHA256SUMS` row in two forms — `<digest>  <name>` in text mode and
`<digest> *<name>` in binary mode. Measured against the shelf the pin beside
this script names: four of its five rows are the text form and the Windows row
is the binary form, so the pattern matched four archives and missed exactly one
platform. The parser below reads both, and the guard beside it drives it with
that published file as a fixture.

THE HOST MAP IS THE ENGINE'S OWN LIST, NOT A COPY

`[engine].targets` in the engine tree at the pinned `ref` is the authority for
what the shelf carries. This script computes a target triple from
`platform.system()` and `platform.machine()` and then asks whether that triple
is in the engine's list — so a target added upstream is a line in the engine's
manifest and not a guess here, and a host outside the list takes the source
build, which is the answer ADR-0023 §2 gives for exactly that machine.

EXIT CODES, BECAUSE THE PAGE BINDS TO THEM RATHER THAN TO PROSE

    0  the pinned engine is unpacked and answering
    2  this script cannot run at all (a pin, a checkout or a flag is unusable)
    3  no target for this host          -> the page takes the source build
    4  download failed                  -> the page takes the source build
    5  checksum mismatch                -> a REFUSAL: never the floor, never a retry
    6  the binary's version is not the pin's  -> stop

3 and 4 are the two the page may fall through; 5 and 6 are refusals. The
difference is the whole reason these are separate numbers: a checksum mismatch
says the bytes are not the release's, and building from source instead would be
answering a question nobody asked.

    python3 scripts/fetch-delvec.py --into ~/.delvewright/bin
"""

from __future__ import annotations

import argparse
import hashlib
import os
import pathlib
import platform
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib
import urllib.error
import urllib.request

EXIT_UNUSABLE = 2
EXIT_NO_TARGET = 3
EXIT_DOWNLOAD = 4
EXIT_CHECKSUM = 5
EXIT_VERSION = 6

DOWNLOAD = "https://github.com/{repo}/releases/download/{release}"
ARCHIVE = "delvec-{release}-{target}.tar.gz"
CHECKSUMS = "SHA256SUMS"

# `<64 hex>` then whitespace then an optional binary-mode `*` then the name.
# BOTH forms, because coreutils writes both and the published file carries both:
# `sha256sum <file>` writes two spaces, `sha256sum -b <file>` writes a space and
# a `*`. A pattern that reads one of them is a pattern that works on four
# platforms out of five and says nothing on the fifth.
ROW_RE = re.compile(r"^(?P<digest>[0-9a-f]{64})[ \t]+\*?(?P<name>\S.*?)[ \t]*$")

# `delvec 1.2.3, dsl 0.4.5, mc 1.21.11` — the number is the first field. The
# example numbers are invented: only the first field is read, and an example
# that happened to be the tree's own numbers would be a version restated where
# nothing checks it.
VERSION_RE = re.compile(r"^delvec\s+(?P<version>\d+\.\d+\.\d+)\b")

# `platform.machine()` says several things for one architecture, and which one
# depends on the OS and on the interpreter's own build. Every spelling here is
# folded to the one the release archives are named with.
CPU = {
    "x86_64": "x86_64",
    "amd64": "x86_64",
    "x64": "x86_64",
    "aarch64": "aarch64",
    "arm64": "aarch64",
}
OS_SUFFIX = {
    "linux": "unknown-linux-gnu",
    "darwin": "apple-darwin",
    "windows": "pc-windows-msvc",
}


class Refusal(Exception):
    """A refusal carrying the exit code the page's failure table binds to."""

    def __init__(self, code: int, message: str) -> None:
        super().__init__(message)
        self.code = code


def host_target(system: str, machine: str) -> str | None:
    """The release-archive target triple for `(system, machine)`, or None.

    None is not an error here — it is `EXIT_NO_TARGET`'s input, and the page's
    answer to it is the source build. Kept as a pure function of its two
    arguments so the guard beside this file can drive it with each of the five
    published pairs and one that is none of them.
    """
    cpu = CPU.get(machine.lower())
    suffix = OS_SUFFIX.get(system.lower())
    if cpu is None or suffix is None:
        return None
    return f"{cpu}-{suffix}"


def read_pin(path: pathlib.Path) -> tuple[str, str, str]:
    """`(repo, release, ref)` from the manifest beside the page."""
    try:
        data = tomllib.loads(path.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as exc:
        raise Refusal(EXIT_UNUSABLE, f"{path} is unusable: {exc}") from exc
    engine = data.get("engine")
    if not isinstance(engine, dict):
        raise Refusal(EXIT_UNUSABLE, f"{path} has no `[engine]` table")
    out = []
    for key in ("repo", "release", "ref"):
        value = engine.get(key)
        if not isinstance(value, str) or not value:
            raise Refusal(EXIT_UNUSABLE, f"{path} has no `[engine].{key}`")
        out.append(value)
    return out[0], out[1], out[2]


def engine_targets(engine: pathlib.Path, ref: str) -> list[str]:
    """`[engine].targets` in the engine tree AT `ref`, never at its HEAD.

    A checkout is a moving thing and the pin is not. Reading the working tree
    would map this host against whatever revision somebody last checked out,
    which is exactly the wandering the pin exists to stop.
    """
    if not (engine / ".git").exists():
        raise Refusal(
            EXIT_UNUSABLE,
            f"{engine} is not a git checkout of the engine. Init I2 clones it at "
            f"the pinned revision before this step runs; without it the target "
            f"list this script maps the host against cannot be read, and "
            f"guessing one would be a map nobody wrote down.",
        )
    proc = subprocess.run(
        ["git", "-C", str(engine), "show", f"{ref}:versions.toml"],
        capture_output=True,
    )
    if proc.returncode != 0:
        raise Refusal(
            EXIT_UNUSABLE,
            f"{engine} cannot serve versions.toml at {ref[:8]}: "
            f"{proc.stderr.decode('utf-8', 'replace').strip()}",
        )
    try:
        data = tomllib.loads(proc.stdout.decode("utf-8"))
    except (UnicodeDecodeError, tomllib.TOMLDecodeError) as exc:
        raise Refusal(
            EXIT_UNUSABLE, f"the engine's versions.toml at {ref[:8]} is unusable: {exc}"
        ) from exc
    targets = data.get("engine", {}).get("targets")
    if not isinstance(targets, list) or not targets:
        raise Refusal(
            EXIT_UNUSABLE,
            f"the engine at {ref[:8]} declares no `[engine].targets`, so this "
            f"script has no list to map the host against. A map it invented "
            f"would be a second authority for the shelf.",
        )
    return [t for t in targets if isinstance(t, str)]


def digest_for(text: str, name: str) -> str | None:
    """The digest `SHA256SUMS` records for `name`, in either coreutils form."""
    for line in text.splitlines():
        m = ROW_RE.match(line)
        if m is not None and m.group("name") == name:
            return m.group("digest")
    return None


def fetch(url: str) -> bytes:
    try:
        with urllib.request.urlopen(url, timeout=300) as fh:
            return fh.read()
    except (urllib.error.URLError, OSError, TimeoutError) as exc:
        raise Refusal(EXIT_DOWNLOAD, f"could not download {url}: {exc}") from exc


def unpack(archive: pathlib.Path, into: pathlib.Path) -> None:
    into.mkdir(parents=True, exist_ok=True)
    with tarfile.open(archive, "r:gz") as tar:
        # `filter="data"` refuses an absolute path, a `..` escape and every
        # special file kind. It is the default from 3.14 and available from
        # 3.11.4; on an older 3.11 patch the keyword is absent, and the archive
        # has already been held to a published digest by the time this runs.
        try:
            tar.extractall(into, filter="data")
        except TypeError:
            tar.extractall(into)


def binary_version(binary: pathlib.Path) -> str | None:
    proc = subprocess.run([str(binary), "--version"], capture_output=True, text=True)
    if proc.returncode != 0:
        return None
    m = VERSION_RE.match(proc.stdout.strip())
    return m.group("version") if m else None


def run(
    pin: pathlib.Path,
    engine: pathlib.Path,
    into: pathlib.Path,
    system: str,
    machine: str,
) -> int:
    repo, release, ref = read_pin(pin)
    targets = engine_targets(engine, ref)

    target = host_target(system, machine)
    if target is None or target not in targets:
        raise Refusal(
            EXIT_NO_TARGET,
            f"the shelf carries no archive for {system}/{machine}"
            + (f" ({target})" if target else "")
            + f". Release {release} publishes: {', '.join(targets)}. "
            f"This machine builds from source (ADR-0023 §2).",
        )

    base = DOWNLOAD.format(repo=repo, release=release)
    name = ARCHIVE.format(release=release, target=target)
    sums = fetch(f"{base}/{CHECKSUMS}").decode("utf-8", "replace")
    want = digest_for(sums, name)
    if want is None:
        raise Refusal(
            EXIT_CHECKSUM,
            f"{CHECKSUMS} on release {release} carries no row for {name}, so "
            f"nothing binds those bytes to that release. This is a refusal and "
            f"not a reason to take the source build: the shelf and its manifest "
            f"disagree, and only the publisher can settle that.",
        )

    blob = fetch(f"{base}/{name}")
    got = hashlib.sha256(blob).hexdigest()
    if got != want:
        raise Refusal(
            EXIT_CHECKSUM,
            f"{name} hashes {got}, and {CHECKSUMS} says {want}. **Refused.** "
            f"Nothing is extracted, nothing is downloaded again, and the source "
            f"build is not a way around this — the published {CHECKSUMS} is the "
            f"only thing binding those bytes to release {release}.",
        )

    into = into.expanduser()
    with tempfile.TemporaryDirectory(prefix="fetch-delvec-") as tmp:
        staged = pathlib.Path(tmp) / name
        staged.write_bytes(blob)
        unpack(staged, pathlib.Path(tmp) / "x")
        found = [
            p
            for p in sorted((pathlib.Path(tmp) / "x").rglob("*"))
            if p.is_file() and p.name in ("delvec", "delvec.exe")
        ]
        if not found:
            raise Refusal(
                EXIT_UNUSABLE,
                f"{name} carries no `delvec` executable. The archive verified "
                f"against its published digest, so this is the shelf's shape "
                f"having changed rather than a corrupt download.",
            )
        into.mkdir(parents=True, exist_ok=True)
        binary = into / found[0].name
        shutil.copy2(found[0], binary)
        binary.chmod(binary.stat().st_mode | 0o111)

    version = binary_version(binary)
    expected = release.lstrip("v")
    if version != expected:
        raise Refusal(
            EXIT_VERSION,
            f"{binary} answers `delvec {version}` and the pin names {release}. "
            f"The shelf served a different engine than this page was written "
            f"against. Stop: every refusal, picture and diagnostic below would "
            f"come from an engine nobody chose.",
        )

    print(
        f"fetch-delvec: ok — target {target}, archive {name}, "
        f"sha256 {got}, delvec {version} at {binary}"
    )
    print(f"  add to PATH: {into}")
    return 0


def main(argv: list[str] | None = None) -> int:
    here = pathlib.Path(__file__).resolve().parent.parent
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--into",
        type=pathlib.Path,
        default=pathlib.Path("~/.delvewright/bin"),
        help="where the unpacked binary lands (default: ~/.delvewright/bin)",
    )
    ap.add_argument(
        "--pin",
        type=pathlib.Path,
        default=here / "versions.toml",
        help="the manifest beside the page (default: the skill root's versions.toml)",
    )
    ap.add_argument(
        "--engine",
        type=pathlib.Path,
        default=pathlib.Path(
            os.environ.get("DELVEWRIGHT_ENGINE", "~/.delvewright/engine")
        ),
        help=(
            "the engine checkout Init I2 made. `[engine].targets` is read out of "
            "it AT the pinned ref, never at its HEAD."
        ),
    )
    args = ap.parse_args(argv)
    try:
        return run(
            args.pin,
            args.engine.expanduser(),
            args.into,
            platform.system(),
            platform.machine(),
        )
    except Refusal as refusal:
        print(f"fetch-delvec: {refusal}", file=sys.stderr)
        return refusal.code


if __name__ == "__main__":
    raise SystemExit(main())
