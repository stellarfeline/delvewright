#!/usr/bin/env python3
r"""Every inline python a repo shell script runs must pin its stdout newline.

WHY THIS EXISTS (v1.0.0 release run, 2026-08-06 — Actions run 31205291235)

The first-ever run of `.github/workflows/engine-release.yml` failed on exactly one
of five shelf targets:

    build-release-binaries: 'x86_64-pc-windows-msvc' is not in versions.toml
    [engine].targets

The triple WAS in `versions.toml` and WAS in the workflow matrix. The target list
reaches bash through `tools/build-release-binaries.sh`'s `read_manifest()`, a
`python3` heredoc that `print`s the values, read back with `while IFS= read -r`.
**On Windows, python's text-mode stdout translates every `\n` it writes into
`\r\n`** — so each value arrived as `x86_64-pc-windows-msvc\r`. `IFS= read -r`
strips the `\n` and keeps the `\r`, and `[ "$k" = "$t" ]` is then false forever.
The four Linux/macOS targets were green, which is precisely why nobody saw it:
the bug is invisible on every runner but one, and the eleven green checks on the
PR that introduced the script (#318) never ran that one.

THE RULE

An inline python program that writes to stdout, run from a repo shell script or
workflow `run:` block, must declare:

    sys.stdout.reconfigure(newline="\n")

That makes the interpreter's platform irrelevant at the point where the value is
produced, so no consumer — a `$(...)`, a `<(...)`, a `> file` the shell reads
later, a `>> "$GITHUB_OUTPUT"`, or a caller capturing the enclosing shell
FUNCTION — has to know or care.

WHY THE RULE IS "EVERY PRINTING PROGRAM" AND NOT "EVERY CAPTURED ONE"

Because capture is not decidable from the invocation. `read_manifest()` — the
site that actually broke the release — has no redirect, no pipe and no
substitution on its own line; it is a function body, and the capture happens at
three separate call sites. A checker that reasoned about the invocation would
have passed the one bug it exists to catch. Requiring it unconditionally costs
one line and has no downside: pinning `\n` on a stream nobody captures changes
nothing.

WHAT IS OUT OF SCOPE, AND WHY

  * `python3 some_script.py` — no inline program text to check. A committed
    `.py` file is not a shell/python boundary; it is python all the way down.
  * a program with no `print(` / `sys.stdout` — it communicates by exit status
    (e.g. `check-publishable.sh`'s field probe), so it has no newlines to get
    wrong.
  * python invoked INSIDE a container (`docker run` / `docker exec` in the same
    logical command). It executes in one of this repo's pinned Linux images by
    construction, where the interpreter's default already is `\n`, and the
    program text sits behind two layers of shell quoting where an edit is the
    riskier move.

Exit 0 clean, 1 with one finding per line.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

# Frozen record, not live tooling (same rationale as
# tools/check-shell-pipe-shortcircuit.py): rewriting a preserved experiment would
# falsify it, and nothing in CI or the authoring loop executes it.
EXCLUDED_PREFIXES = ("docs/experiments/",)

GUARD = re.compile(r"""sys\.stdout\.reconfigure\(\s*newline\s*=\s*['"]\\n['"]\s*\)""")
WRITES_STDOUT = re.compile(r"\bprint\s*\(|\bsys\.stdout\.write\b")
PYTHON = re.compile(r"\bpython3?\b")
HEREDOC = re.compile(r"<<-?\s*(['\"]?)([A-Za-z_][A-Za-z0-9_]*)\1")
IN_CONTAINER = re.compile(r"\bdocker\s+(?:run|exec|compose\s+exec)\b")


def scan_files() -> list[str]:
    out = subprocess.run(
        ["git", "ls-files", "*.sh", ".github/workflows/*.yml", ".github/actions/*/*.yml"],
        cwd=REPO,
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    return [p for p in out.splitlines() if p and not p.startswith(EXCLUDED_PREFIXES)]


def logical_lines(lines: list[str]) -> list[tuple[int, str, int]]:
    """Join backslash continuations: (1-based start lineno, joined text, last index).

    A `docker run ... \\` whose `python3 -c` sits on the NEXT physical line is one
    command, and the container rule has to see it as one.
    """
    out: list[tuple[int, str, int]] = []
    i = 0
    while i < len(lines):
        start, parts = i, [lines[i]]
        while parts[-1].rstrip().endswith("\\") and i + 1 < len(lines):
            i += 1
            parts.append(lines[i])
        out.append((start + 1, " ".join(p.rstrip().rstrip("\\") for p in parts), i))
        i += 1
    return out


def quoted_after_dash_c(text: str, at: int) -> str | None:
    """The program text of a `-c` invocation starting at `at`, or None."""
    m = re.compile(r"-c\s+(['\"])").search(text, at)
    if not m:
        return None
    quote, j = m.group(1), m.end()
    buf: list[str] = []
    while j < len(text):
        if text[j] == "\\" and quote == '"' and j + 1 < len(text):
            buf.append(text[j : j + 2])
            j += 2
            continue
        if text[j] == quote:
            return "".join(buf)
        buf.append(text[j])
        j += 1
    return "".join(buf)


def programs(rel: str, lines: list[str]) -> list[tuple[int, str]]:
    """Every inline python program in a file: (1-based lineno, program text)."""
    found: list[tuple[int, str]] = []
    for lineno, text, last_idx in logical_lines(lines):
        bare = text.lstrip()
        if bare.startswith("#"):
            continue
        code = text.split(" #", 1)[0] if bare.startswith(("- ", "#")) else text
        m = PYTHON.search(code)
        if not m or IN_CONTAINER.search(code):
            continue
        here = HEREDOC.search(code, m.end())
        if here:
            delim, body = here.group(2), []
            for line in lines[last_idx + 1 :]:
                if line.strip() == delim:
                    break
                body.append(line)
            found.append((lineno, "\n".join(body)))
            continue
        inline = quoted_after_dash_c(code, m.end())
        if inline is not None:
            found.append((lineno, inline))
    return found


def main() -> int:
    findings: list[str] = []
    scanned = examined = 0
    for rel in scan_files():
        scanned += 1
        lines = (REPO / rel).read_text(encoding="utf-8").splitlines()
        for lineno, body in programs(rel, lines):
            if not WRITES_STDOUT.search(body):
                continue  # communicates by exit status; no newlines to get wrong
            examined += 1
            if not GUARD.search(body):
                findings.append(
                    f"{rel}:{lineno}: inline python writes to stdout without pinning "
                    f"its newline mode"
                )

    # Vacuity guard (CLAUDE.md: a gate that binds to nothing is not a pass).
    # Two counts, because either can silently go to zero: the file glob, and the
    # inline programs inside those files.
    if scanned == 0 or examined == 0:
        print(
            f"check-python-shell-newlines: FAIL — vacuous: {scanned} file(s), "
            f"{examined} stdout-writing inline python program(s)",
            file=sys.stderr,
        )
        return 1

    if findings:
        print(
            f"check-python-shell-newlines: {len(findings)} finding(s) across "
            f"{scanned} file(s)\n",
            file=sys.stderr,
        )
        for f in findings:
            print(f, file=sys.stderr)
        print(
            "\nPython's text-mode stdout writes `\\r\\n` on Windows, and the `\\r`\n"
            "survives command substitution and `IFS= read -r` alike — so a value\n"
            "compares unequal to itself on one runner and only that runner. Pin it\n"
            "where the value is produced, as the first statement after the imports:\n"
            '    sys.stdout.reconfigure(newline="\\n")\n',
            file=sys.stderr,
        )
        return 1

    print(
        f"check-python-shell-newlines: OK — {examined} stdout-writing inline python "
        f"program(s) across {scanned} shell/workflow file(s), all newline-pinned"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
