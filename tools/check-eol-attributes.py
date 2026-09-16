#!/usr/bin/env python3
r"""Every tracked file a machine runs through its first line is pinned to LF.

WHY THIS EXISTS

A Windows checkout with git's Windows default (`core.autocrlf=true`) writes every
text file to disk with CRLF unless an attribute says otherwise. A shell script
that arrives that way does not fail at the line that is wrong; it fails at line
one, because the kernel reads `#!/usr/bin/env bash\r` and looks for an
interpreter literally named `bash\r`:

    /usr/bin/env: 'bash\r': No such file or directory

`validation/staging-admission.sh` arrived that way on a contributor's machine,
and `*.sh text eol=lf` in `.gitattributes` repairs it. But `*.sh` is the class
that broke, not the class that can break: the same first line is read by the
kernel for every `*.py` under `tools/`, every `*.mjs` spike, and every file
carrying the executable bit whatever its name. A pattern list maintained by hand
covers the extension somebody remembered.

WHAT IT ASSERTS

The population is DERIVED from the tree, never listed: a tracked file is in it
when its index mode is `100755` (some machine may exec it), or when its first
bytes are a shebang. Every file in that population must carry `eol=lf`, and the
authority on that is **git itself** — `git check-attr eol`, the same resolution
a checkout performs, rather than a second parse of `.gitattributes` that could
disagree with the one that matters.

`--working-tree` adds the assertion that only a real Windows checkout can make:
the files, as they exist on disk, hold no CRLF pair. An attribute that is
declared and not honoured (a stale index, a pattern the checkout's git reads
differently) passes the first assertion and fails this one.

WHAT A SHEBANG IS, AND WHAT IT IS NOT

`#!` at byte zero, with the third byte not `[`. Rust's inner attribute
`#![allow(dead_code)]` opens a file with those same two bytes and is not a
shebang — a crate module is never handed to the kernel to execute. Counting it
would demand an `eol=lf` pattern for a whole language on the strength of one
false positive.

WHAT IS OUT OF SCOPE

A file whose content CRLF corrupts without a first line being read — a
Dockerfile, a fixture compared byte for byte — is not derivable from this rule
and is not judged here. `.gitattributes` may pin more than this gate demands;
this gate is a floor, not the whole of the file.

States its binding count with its denominator; a population of zero is a red.

Usage:
  python3 tools/check-eol-attributes.py [--repo DIR] [--working-tree]

Exit 0 clean, 1 with findings or on a vacuous run.
"""

from __future__ import annotations

import argparse
import pathlib
import subprocess
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent

EXEC_MODE = "100755"
WANT = "lf"


def tracked(repo: pathlib.Path) -> list[tuple[str, str]]:
    """Every tracked path with its index mode, from `git ls-files -s`."""
    out = subprocess.run(
        ["git", "ls-files", "-s", "-z"],
        cwd=repo,
        capture_output=True,
        check=True,
    ).stdout
    rows: list[tuple[str, str]] = []
    for record in out.split(b"\0"):
        if not record:
            continue
        meta, _, path = record.partition(b"\t")
        rows.append((meta.split(b" ", 1)[0].decode(), path.decode("utf-8", "surrogateescape")))
    return rows


def has_shebang(path: pathlib.Path) -> bool:
    """`#!` at byte zero — but not Rust's inner attribute `#![…]`."""
    try:
        with path.open("rb") as fh:
            head = fh.read(3)
    except OSError:
        return False
    return head[:2] == b"#!" and head[2:3] != b"["


def population(repo: pathlib.Path) -> tuple[list[str], int, int, int]:
    """(paths, tracked total, shebang count, executable count), paths sorted."""
    paths: list[str] = []
    total = shebangs = executables = 0
    for mode, rel in tracked(repo):
        total += 1
        is_exec = mode == EXEC_MODE
        is_shebang = has_shebang(repo / rel)
        if is_exec:
            executables += 1
        if is_shebang:
            shebangs += 1
        if is_exec or is_shebang:
            paths.append(rel)
    return sorted(paths), total, shebangs, executables


def eol_attributes(repo: pathlib.Path, paths: list[str]) -> dict[str, str]:
    """What git resolves `eol` to for each path — the checkout's own answer."""
    proc = subprocess.run(
        ["git", "check-attr", "--stdin", "-z", "eol"],
        cwd=repo,
        input="\0".join(paths).encode("utf-8", "surrogateescape"),
        capture_output=True,
        check=True,
    )
    fields = proc.stdout.split(b"\0")
    resolved: dict[str, str] = {}
    for i in range(0, len(fields) - 2, 3):
        path = fields[i].decode("utf-8", "surrogateescape")
        resolved[path] = fields[i + 2].decode()
    return resolved


def carries_crlf(path: pathlib.Path) -> bool:
    try:
        return b"\r\n" in path.read_bytes()
    except OSError:
        return False


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--repo", type=pathlib.Path, default=REPO)
    ap.add_argument(
        "--working-tree",
        action="store_true",
        help="also assert the files on disk hold no CRLF pair (the Windows job)",
    )
    args = ap.parse_args(argv)
    repo = args.repo.resolve()

    paths, total, shebangs, executables = population(repo)

    # Vacuity guard (CLAUDE.md): a gate that matched nothing is not a pass.
    if not paths or total == 0:
        print(
            f"check-eol-attributes: FAIL — vacuous: {len(paths)} file(s) in the "
            f"population of {total} tracked file(s)",
            file=sys.stderr,
        )
        return 1

    resolved = eol_attributes(repo, paths)
    findings: list[str] = []
    for rel in paths:
        value = resolved.get(rel, "unspecified")
        if value != WANT:
            findings.append(f"{rel}: eol is `{value}`, not `{WANT}`")

    materialised = 0
    if args.working_tree:
        for rel in paths:
            if carries_crlf(repo / rel):
                findings.append(f"{rel}: materialised with CRLF in the working tree")
            else:
                materialised += 1

    binding = (
        f"{len(paths)} of {total} tracked file(s) are run through their first line "
        f"({shebangs} by shebang, {executables} by the executable bit)"
    )

    if findings:
        print(f"check-eol-attributes: {len(findings)} finding(s) — {binding}\n", file=sys.stderr)
        for f in findings:
            print(f, file=sys.stderr)
        print(
            "\nA file whose first line the kernel reads must reach disk with LF on\n"
            "every platform, or a Windows checkout hands the interpreter a name\n"
            "ending in `\\r`. Add a pattern covering it to `.gitattributes`:\n"
            "    <pattern> text eol=lf\n",
            file=sys.stderr,
        )
        return 1

    tail = f"; {materialised} materialised with LF" if args.working_tree else ""
    print(f"check-eol-attributes: OK — {binding}, every one pinned `eol=lf`{tail}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
