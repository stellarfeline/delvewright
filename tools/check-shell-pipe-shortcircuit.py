#!/usr/bin/env python3
"""Forbid short-circuiting consumers on the right of a pipe in repo shell scripts.

WHY THIS EXISTS

`tools/playtest-server.sh` intermittently reported "server did not come up" for a
server that was up, healthy, and had already logged `Done (` exactly once. The
readiness probe was:

    set -euo pipefail
    docker logs "$NAME" 2>&1 | grep -q 'Done ('

`grep -q` exits the instant it matches. That closes the pipe while `docker logs`
is still writing, so `docker logs` dies of SIGPIPE with status 141 — and
`pipefail` promotes the *pipeline* to 141. The pipeline therefore reports FAILURE
precisely because the match succeeded early. Measured on a live container with
one matching line in 79: 18 false negatives in 30 runs.

The producer's size decides how often it bites, so every one of these is a coin
flip weighted by output length — which is why it read as flakiness for months.

The class is "a consumer that stops reading before its producer stops writing",
which is `grep -q`, `grep -m N`, and `head -n N`. It is NOT specific to
`pipefail` being set in the same file: a helper function sourced into a caller
that sets `pipefail` inherits it (that is exactly how `validation/mutex.sh` runs),
so the rule applies to every shell script in the repo.

THE PRESCRIBED IDIOM — capture, then test without a pipe:

    out="$(docker logs "$NAME" 2>&1 || true)"
    if [[ $out == *"Done ("* ]]; then ...          # fixed string
    if [[ $out =~ ^[0-9a-f]{40}$ ]]; then ...      # regex
    first="${out%%$'\\n'*}"                         # replaces `| head -1`

A here-string (`grep -q PAT <<<"$out"`) is also safe — bash materialises it into
a file, so there is no writer to signal — but the native `[[ ]]` forms spawn no
process at all and are preferred.

Exit 0 clean, 1 with one finding per line.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

# Frozen record, not live tooling: docs/experiments/ preserves an experiment
# exactly as it was run (M2 jigsaw seed stability). Rewriting it would falsify
# the record. Nothing in CI or the authoring loop executes it.
EXCLUDED_PREFIXES = ("docs/experiments/",)

# The one justified line-level exemption. `validation/world-settings-entrypoint.sh`
# is byte-locked to the heredoc in `validation/Dockerfile.delve` (their identity is
# itself a CI gate, validation/check-world-settings.sh), so ANY edit there — even a
# comment — moves the SHIPPED delve image and leaves the validator-only merge gate.
# It is also not a live bug: the file sets `set -e` WITHOUT `pipefail`, and the
# SIGPIPE lands on `sed`, whose status the pipeline discards. It is listed here so
# the exemption is visible rather than absent, and it must be closed the next time
# that entrypoint is touched under the player-facing gate.
# Empty on purpose, and it is the point: every early-exit-on-a-pipe in the repo
# has been removed rather than allowlisted. The last entry here was
# `validation/world-settings-entrypoint.sh`'s `prop()`, latent because that script
# sets `set -e` without `pipefail`; it was rewritten to let `sed` stop by itself.
# Re-adding an entry means accepting a coin flip somewhere — say why, in the entry.
EXEMPT_LINES: set[tuple[str, int]] = set()

# A pipe, then a consumer that can stop reading early. `head`/`tail` are only a
# hazard with an explicit line count that is not the whole stream — `tail -n 30`
# reads to EOF and is fine, `head -n 1` is not.
PATTERNS = (
    (
        re.compile(
            r"\|\s*grep\b[^|&]*?"
            r"(?:-[a-zA-Z]*q[a-zA-Z]*\b|--quiet\b|--silent\b|-[a-zA-Z]*m\s*\d|--max-count)"
        ),
        "grep -q / grep -m N stops reading at the first match",
    ),
    (
        re.compile(r"\|\s*head\b(?:\s+(?:-n\s*)?-?\d+)?(?=\s|$|\|)"),
        "head -N stops reading after N lines",
    ),
)


def shell_files() -> list[str]:
    out = subprocess.run(
        ["git", "ls-files", "*.sh"],
        cwd=REPO,
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    return [
        p
        for p in out.splitlines()
        if p and not p.startswith(EXCLUDED_PREFIXES)
    ]


def main() -> int:
    findings: list[str] = []
    exemptions_used: set[tuple[str, int]] = set()
    scanned = 0
    for rel in shell_files():
        scanned += 1
        for lineno, line in enumerate(
            (REPO / rel).read_text(encoding="utf-8").splitlines(), start=1
        ):
            code = line.split("#", 1)[0] if not line.lstrip().startswith("#") else ""
            for pattern, why in PATTERNS:
                if pattern.search(code):
                    if (rel, lineno) in EXEMPT_LINES:
                        exemptions_used.add((rel, lineno))
                    else:
                        findings.append(f"{rel}:{lineno}: {why}\n    {line.strip()}")
                    break

    # An allowlist entry that no longer matches anything is rot: either the line was
    # fixed (delete the entry) or it moved (the entry now silences a DIFFERENT line).
    # Fail either way — a stale exemption is how an allowlist becomes a blind spot.
    for stale in sorted(EXEMPT_LINES - exemptions_used):
        findings.append(
            f"{stale[0]}:{stale[1]}: STALE EXEMPTION — EXEMPT_LINES names this line "
            f"but nothing here matches any more. Fixed? delete the entry. Moved? the "
            f"entry is now silencing the wrong line."
        )

    # Vacuity guard: a lint that scanned nothing is not a pass (CLAUDE.md —
    # "a green gate that binds to nothing is VACUOUS"). State the binding count.
    if scanned == 0:
        print("check-shell-pipe-shortcircuit: FAIL — scanned 0 shell scripts", file=sys.stderr)
        return 1

    if findings:
        print(
            f"check-shell-pipe-shortcircuit: {len(findings)} finding(s) "
            f"across {scanned} shell script(s)\n",
            file=sys.stderr,
        )
        for f in findings:
            print(f, file=sys.stderr)
        print(
            "\nUnder `set -o pipefail` a consumer that exits early kills its producer\n"
            "with SIGPIPE (141) and the pipeline reports FAILURE *because the match\n"
            "succeeded*. Capture first, then test without a pipe:\n"
            '    out="$(cmd 2>&1 || true)"\n'
            '    [[ $out == *"needle"* ]]        # fixed string\n'
            "    [[ $out =~ ^re$ ]]              # regex\n"
            "    first=\"${out%%$'\\n'*}\"          # instead of `| head -1`\n",
            file=sys.stderr,
        )
        return 1

    print(f"check-shell-pipe-shortcircuit: OK — {scanned} shell scripts, no early-exit pipe consumers")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
