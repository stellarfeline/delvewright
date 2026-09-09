#!/usr/bin/env python3
"""Print the newest JDK 21+ already on this machine, or exit 1 having found none.

WHY THIS IS A SCRIPT

A machine whose default `java` answers below 21 very often HAS a 21 sitting
beside it, unselected. Installing a JDK is the user's action on their own
machine; choosing among the ones already on their disk is the agent's, and
halting for something already present spends the user's session on a `PATH`
line. That much is a decision the page states. What is left is an ALGORITHM —
where JDKs live on three platforms, and how to ask one its version — and an
algorithm two agents would write two ways is exactly what a script is for.

THE ONE RULE THAT MAKES THE ANSWER RIGHT: ASK THE BINARY

Never read a version off a directory name. On a Homebrew machine `openjdk@20`,
`@22` and `@23` are all symlinks to whatever `openjdk` currently is, so the name
says 20 and the binary answers 26 — and `openjdk@21`, the one that is genuinely
21, is keg-only and does not appear in `/usr/libexec/java_home -V` at all. A
search that trusts the path both rejects the right JDK and accepts the wrong
one, and neither failure announces itself.

Prints `<major> <java home>` on the newest qualifying JDK and nothing else, so
the caller can read it with one split. Exit 1 and no output means there is none,
which is the caller's cue to halt and hand the install to the user.

    python3 scripts/find-jdk.py [--minimum 21]
"""

from __future__ import annotations

import argparse
import os
import pathlib
import re
import shutil
import subprocess
import sys

# `java -version` writes to stderr, and its first line is
# `openjdk version "21.0.5" 2024-10-15`. `[^"]*` and not `.*`: a greedy match
# runs to the CLOSING quote and captures the empty string, so every JDK would
# silently read as version "" and be skipped.
VERSION_RE = re.compile(r'^[^"]*"(?P<major>\d+)(?:[.\-+"].*)?$')

# Where a JDK sits, per platform. Each entry is a directory whose immediate
# children are candidate java homes; a missing one is not an error, because
# `/usr/lib/jvm` is absent on macOS and `/Library/Java/...` on Linux.
ROOTS = (
    "/Library/Java/JavaVirtualMachines",
    "/usr/lib/jvm",
    "/usr/java",
    "/opt/homebrew/opt",
    "/usr/local/opt",
    "/opt/java",
    "C:/Program Files/Java",
    "C:/Program Files/Eclipse Adoptium",
)

# A macOS java home sits one level deeper than the bundle directory.
NESTED = ("Contents/Home", "libexec/openjdk.jdk/Contents/Home", "")


def java_major(home: pathlib.Path) -> int | None:
    """The major version `home`'s own binary reports, or None if it is not one."""
    for exe in ("java", "java.exe"):
        binary = home / "bin" / exe
        if binary.is_file() and os.access(binary, os.X_OK):
            break
    else:
        return None
    try:
        proc = subprocess.run(
            [str(binary), "-version"], capture_output=True, text=True, timeout=30
        )
    except (OSError, subprocess.SubprocessError):
        return None
    first = (proc.stderr or proc.stdout).splitlines()
    if not first:
        return None
    m = VERSION_RE.match(first[0].strip())
    return int(m.group("major")) if m else None


def candidates() -> list[pathlib.Path]:
    """Every directory that might be a java home, in no particular order."""
    out: list[pathlib.Path] = []
    for root in ROOTS:
        base = pathlib.Path(root)
        if not base.is_dir():
            continue
        try:
            children = sorted(base.iterdir())
        except OSError:
            continue
        for child in children:
            for tail in NESTED:
                home = child / tail if tail else child
                if (home / "bin").is_dir():
                    out.append(home)
    # `/usr/libexec/java_home -V` on macOS lists what the OS itself knows about,
    # which is a different population from the directory walk and misses the
    # keg-only ones the walk finds. Both, then, and the versions are still asked
    # of the binaries rather than taken from either listing.
    tool = shutil.which("/usr/libexec/java_home") or "/usr/libexec/java_home"
    if pathlib.Path(tool).is_file():
        try:
            proc = subprocess.run(
                [tool, "-V"], capture_output=True, text=True, timeout=30
            )
            for line in (proc.stderr or "").splitlines():
                path = line.strip().rsplit(" ", 1)[-1]
                if path.startswith("/"):
                    out.append(pathlib.Path(path))
        except (OSError, subprocess.SubprocessError):
            pass
    # A `JAVA_HOME` already set is a candidate like any other, judged the same way.
    if os.environ.get("JAVA_HOME"):
        out.append(pathlib.Path(os.environ["JAVA_HOME"]))
    seen: set[pathlib.Path] = set()
    unique: list[pathlib.Path] = []
    for home in out:
        try:
            resolved = home.resolve()
        except OSError:
            continue
        if resolved not in seen:
            seen.add(resolved)
            unique.append(resolved)
    return unique


def best(minimum: int) -> tuple[int, pathlib.Path] | None:
    found = [
        (major, home)
        for home in candidates()
        if (major := java_major(home)) is not None and major >= minimum
    ]
    return max(found, key=lambda pair: (pair[0], str(pair[1]))) if found else None


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--minimum",
        type=int,
        default=21,
        help=(
            "the major the pinned game declares (default: 21). It is a flag "
            "rather than a constant so a guard can drive the refusal arm."
        ),
    )
    args = ap.parse_args(argv)
    hit = best(args.minimum)
    if hit is None:
        print(
            f"find-jdk: no JDK {args.minimum}+ on this machine. Installing one is "
            f"the user's action; hand it over and stop.",
            file=sys.stderr,
        )
        return 1
    major, home = hit
    print(f"{major} {home}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
