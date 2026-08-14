#!/usr/bin/env python3
"""No internal reference on a page a stranger lands on.

WHAT THIS GUARDS

`crates/compiler/README.md` and `crates/dsl/README.md` are rendered VERBATIM as
the crates.io front pages of `delvec` and `delvewright-dsl`. They are the only
documents in this repository whose reader has never seen this repository. Both
were written as crate-local development notes and served to that reader for
months: the `delvec` page opened with "The deterministic compiler (spec-0002,
ADR-0001/0006/0011)" — a citation a stranger cannot resolve and gains nothing
from — and the `delvewright-dsl` page opened on `spec-0001`. They were rewritten
(#388) and nothing stopped that regressing, because the rule behind it
(CLAUDE.md **Audience separation in docs**, owner 2026-08-02) was held by memory
alone.

WHAT IS CHECKED, AND WHY ONLY THESE PATTERNS

Six unambiguous shapes. Each names something that exists only inside this
project, so none of them can appear in honest prose about a Minecraft compiler:

  spec-NNNN      an owner-approved spec number
  ADR-NNNN       an architecture decision record number
  DWNNNN         a compiler diagnostic code
  task #N        an internal task id
  PR #N          a pull-request number
  a markdown link whose target is repo-relative

`DW\\d{4}` deliberately does NOT fire on the phrase "a stable `DW####` code",
which is exactly what these pages legitimately want to say about the diagnostic
surface — `####` is not four digits. That is the whole reason the pattern is
digit-anchored rather than a word match on `DW`.

The repo-relative link rule is what keeps a page self-contained: crates.io
serves the markdown with no repository around it, so `[the reference](docs/...)`
is a link into nothing. An absolute `https://github.com/...` URL is fine and is
how these pages point at the deep documentation.

WHAT IS DELIBERATELY **NOT** CHECKED

The other half of the rule — CLAUDE.md **A reader-facing document is written in
the present tense of the current version** (owner, 2026-08-11) — is NOT checked
here, and that is a decision, not an omission.

Its tells are "now", "still", "since", "originally", "as of vN", "used to". Four
of those five are ordinary English that honest present-tense prose uses
constantly ("the schemas are generated from the Rust types, so the two cannot
disagree" wants "so"; "still" and "since" appear in any comparison). A checker
for them would red on correct content — and **a gate that reds correct prose is
worse than no gate**, because it teaches the people who see it that this
particular red means nothing, and the next red they ignore is a real one. The
present-tense rule stays a review obligation and stays `unenforced` in
`docs/decisions.md`, which is the honest record of it.

What IS mechanically checkable about that rule is its obvious leak — an internal
reference number on a crates.io front page — and that is precisely the six
patterns above. This gate closes that half and claims no more.

BINDING

The file set is DERIVED, never listed: every crate under `crates/*/` whose
`[package] publish` is not `false`, resolved through its `[package] readme` key
(`tools/lib/publishable.py`). A crate that later becomes publishable inherits
this gate with no edit here. Examining zero READMEs is a red, not a pass.

There is no allowlist. Fenced code blocks are scanned like every other line: a
gate a page can step out of by wrapping three backticks around the citation is
not a gate. If a page one day needs a diagnostic code in a sample, the fix is to
write the sample without one — these pages are short and their reader cannot
resolve the code anyway.

Deterministic, offline, no dependencies (Python 3 stdlib). Run from anywhere:
    python3 tools/check-crates-io-readmes.py
Exit 0 = every published page is self-contained, 1 = an internal reference
reached a stranger's page (see stderr), 2 = the derivation broke — no publishable
crate, no README, or a tree shape this script no longer reads. Fix the tree or
the derivation; never loosen the check (CLAUDE.md debug doctrine).
"""

from __future__ import annotations

import pathlib
import re
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent / "lib"))

from publishable import DerivationError, discover  # noqa: E402

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent

# (label, pattern, what a stranger experiences). Order is the report order.
INTERNAL_PATTERNS: list[tuple[str, re.Pattern[str], str]] = [
    (
        "spec number",
        re.compile(r"(?i)\bspec-\d{4}\b"),
        "names a file in docs/specs/ that a stranger cannot open",
    ),
    (
        "ADR number",
        re.compile(r"(?i)\badr-\d{4}\b"),
        "names a file in docs/adr/ that a stranger cannot open",
    ),
    (
        "diagnostic code",
        re.compile(r"\bDW\d{4}\b"),
        "cites one diagnostic out of a catalogue the page does not carry "
        "(say `DW####` and link the catalogue instead)",
    ),
    (
        "task id",
        re.compile(r"(?i)\btask\s+#\d+"),
        "names an internal task nobody outside this project can look up",
    ),
    (
        "pull-request id",
        re.compile(r"(?i)\bPR\s+#\d+"),
        "reads as a changelog entry, and cites a number the page does not resolve",
    ),
]

# `[text](target)`, `![alt](target)`, and the reference definition `[label]: target`.
INLINE_LINK_RE = re.compile(r"!?\[[^\]]*\]\(\s*([^)\s]+)")
REFERENCE_LINK_RE = re.compile(r"^\s{0,3}\[[^\]]+\]:\s*(\S+)")

# An absolute target: a URL scheme, a protocol-relative URL, or an in-page anchor.
ABSOLUTE_TARGET_RE = re.compile(r"^(?:[a-zA-Z][a-zA-Z0-9+.\-]*:|//|#)")


def link_targets(line: str) -> list[str]:
    targets = [m.group(1) for m in INLINE_LINK_RE.finditer(line)]
    m = REFERENCE_LINK_RE.match(line)
    if m:
        targets.append(m.group(1))
    return [t.strip().strip("<>") for t in targets]


def check_page(rel: str, crate: str, text: str) -> list[str]:
    findings: list[str] = []
    for n, line in enumerate(text.splitlines(), start=1):
        for label, pattern, why in INTERNAL_PATTERNS:
            for m in pattern.finditer(line):
                findings.append(
                    f"{rel}:{n}: {label} {m.group(0)!r} on the crates.io page for "
                    f"`{crate}` — it {why}"
                )
        for target in link_targets(line):
            if not target or ABSOLUTE_TARGET_RE.match(target):
                continue
            findings.append(
                f"{rel}:{n}: repo-relative link target {target!r} on the "
                f"crates.io page for `{crate}` — crates.io serves this markdown "
                "with no repository around it, so the link goes nowhere. Use the "
                "absolute https://github.com/... URL, or drop the link"
            )
    return findings


def main() -> int:
    try:
        publishable = discover(REPO_ROOT)
    except DerivationError as exc:
        sys.stderr.write(f"error: the publishable-crate derivation broke — {exc}\n")
        return 2

    crates = [c for c in publishable if c.readme is not None]
    pageless = [c.name for c in publishable if c.readme is None]

    if not crates:
        sys.stderr.write(
            "error: the derivation found NO publishable crate with a README, so "
            "this gate examined zero pages.\n"
            "       A green that binds to nothing is not a pass (CLAUDE.md). "
            "Either every crate\n"
            "       gained `publish = false` — in which case say so here "
            "deliberately — or the\n"
            "       derivation in tools/lib/publishable.py stopped matching the "
            "tree.\n"
        )
        return 2

    findings: list[str] = []
    for crate in crates:
        rel = crate.readme_rel(REPO_ROOT)
        findings.extend(
            check_page(rel, crate.name, crate.readme.read_text(encoding="utf-8"))
        )

    if findings:
        sys.stderr.write(
            "a crates.io front page carries an internal reference.\n"
            "These files are rendered VERBATIM to a reader who has never seen "
            "this repository\n"
            "(CLAUDE.md: Audience separation in docs). Say the BEHAVIOUR as a "
            "plain fact, or\n"
            "delete the sentence -- a shorter true page beats a page that "
            "explains itself.\n\n"
            + "\n".join(f"  - {f}" for f in findings)
            + "\n"
        )
        return 1

    listed = ", ".join(f"{c.name} ({c.readme_rel(REPO_ROOT)})" for c in crates)
    print(
        f"crates.io front pages OK: {len(crates)} published page(s) examined "
        f"[{listed}], {len(INTERNAL_PATTERNS)} internal-reference pattern(s) + "
        "the repo-relative-link rule, 0 hits. "
        "The present-tense half of the rule is deliberately unchecked "
        "(see this script's docstring)."
    )
    # Named rather than silently dropped: a publishable crate with no front page
    # is outside this gate's reach, and the count that omits it must say so.
    if pageless:
        print(
            f"crates.io front pages: {len(pageless)} publishable crate(s) serve "
            f"NO page and were not examined [{', '.join(pageless)}] — crates.io "
            "would show them with no front page at all"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
