#!/usr/bin/env python3
"""Bind every recorded owner decision to a place in the tree that proves it landed.

WHY THIS EXISTS

This project's decisions are made in conversation and then written down —
into an ADR, a spec's Status line, or a dated annotation in `CLAUDE.md`. Writing
them down is where the process stopped. Nothing ever checked that a written-down
decision was BUILT, and nothing noticed when the thing that implemented it
stopped existing. Four ways a decision died, all observed:

- never recorded at all — a framework the owner invented, used to commission
  research, and which survives only in a chat transcript;
- recorded as prose inside a spec and never built — the ranking half of
  spec-0028, whose scoring backends nothing in this repo installs;
- built, then left with no entry point — `crates/grammar`, adopted as THE
  prefab back end, has no binary and zero callers in `crates/compiler/src`, so
  on `main` only `cargo test` can reach it;
- proven, with the evidence written to a session scratch directory that no
  longer exists.

Each was found months later, in conversation, by the owner. That is the most
expensive possible detector. This turns it into an ordinary red.

THE CHECK IS BIDIRECTIONAL, and the second direction is the load-bearing one.

- FORWARD: every row whose status is `landed` names a binding — a path, and
  optionally a regex that must match inside it. The path must exist and the
  pattern must match. A decision whose implementation is deleted, renamed or
  refactored away goes red on the PR that does it, instead of being discovered
  a milestone later.
- REVERSE: every documented decision channel must appear in the ledger. An
  accepted ADR, a spec whose Status records owner approval, and every dated
  `(owner …)` annotation in `CLAUDE.md` are the three places a decision is
  written down; each one must have a row. Without this direction the ledger
  measures only what someone remembered to add, which is the failure it exists
  to end. Silence is how it happened last time.

`open` rows are NOT failures — they are the point. A decision that is recorded
and deliberately not yet built is a real state, and this prints every one of
them with its age so it cannot rot unseen. What is forbidden is a decision that
is neither bound nor declared open.

Exit 0 clean, 1 with one finding per line.
"""

from __future__ import annotations

import re
import sys
from datetime import date
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LEDGER = ROOT / "docs" / "decisions.md"

# One row: | id | date | decision | source | status | binding |
ROW = re.compile(
    r"^\|\s*(?P<id>DEC-\d{4})\s*\|\s*(?P<date>\d{4}-\d{2}-\d{2})\s*\|"
    r"(?P<decision>[^|]*)\|\s*(?P<source>[^|]*?)\s*\|\s*(?P<status>[a-z-]+)\s*\|"
    r"\s*(?P<binding>[^|]*?)\s*\|\s*$"
)

# `constitutive` is a real category, not an escape hatch, and the difference is
# enforced: it is for a decision that defines what the repository IS, where there
# is no discrete artifact to point at and no way for it to silently vanish
# (ADR-0001's "the LLM never writes mcfunction" is not a file). Such a row still
# binds — to its own ADR still existing and still reading `Status: Accepted`, so
# deleting or downgrading the ADR reds. That is a weak binding and it is labelled
# weak. A decision that DOES have a discrete deliverable may not use it: the
# checker refuses `constitutive` outside `docs/adr/`.
# `open` and `unenforced` are BOTH true states and both are printed on every
# run, because they are different failures and the difference matters:
#
#   open        a deliverable was decided and has not been built. It can be
#               finished. spec-0027's prefab back end was adopted as THE back
#               end and has no entry point outside `cargo test`.
#   unenforced  a rule that constrains behaviour, recorded, with no mechanical
#               check behind it — it holds only as long as every agent
#               remembers it. This is the more valuable of the two lists: it is
#               the exact set of decisions that can silently stop being obeyed,
#               and every one of this project's expensive process failures came
#               from it. A row moves off this list by naming the check that now
#               enforces it, never by being re-worded.
#
# Neither is a failure. Filing a decision under the wrong one is: calling an
# unenforced rule `open` says it was never built, which is false and buries the
# rows that were.
STATUSES = {"landed", "open", "unenforced", "superseded", "constitutive"}

# The three channels a decision is written down in. Each yields (key, label);
# every key must be the `source` of some ledger row.
def documented_decisions() -> list[tuple[str, str]]:
    out: list[tuple[str, str]] = []

    for adr in sorted((ROOT / "docs" / "adr").glob("[0-9]*.md")):
        text = adr.read_text(encoding="utf-8")
        if re.search(r"^-\s*\*\*Status\*\*:\s*Accepted", text, re.M):
            out.append((f"adr/{adr.stem.split('-')[0]}", f"{adr.name} (Accepted)"))

    for spec in sorted((ROOT / "docs" / "specs").glob("spec-*.md")):
        text = spec.read_text(encoding="utf-8")
        m = re.search(r"^-?\s*\*\*Status\*\*:?(.*)$", text, re.M)
        if m and re.search(r"owner|approved|implemented", m.group(1), re.I):
            out.append((f"spec/{spec.stem.split('-')[1]}", spec.name))

    # Keyed by the rule's own bold label plus the decision date, never by line
    # number: a line number changes every time anything above it is edited, so a
    # line-keyed ledger would red the whole file on an unrelated paragraph and
    # get relaxed within a week. A renamed rule is a real semantic change and
    # SHOULD cost a ledger touch; a reflowed paragraph should not.
    claude = (ROOT / "CLAUDE.md").read_text(encoding="utf-8")
    for m in re.finditer(r"\(owner[^)]*?(\d{4}-\d{2}-\d{2})[^)]*\)", claude):
        bullet = claude.rfind("\n- ", 0, m.start())
        segment = claude[bullet : m.start()] if bullet >= 0 else claude[: m.start()]
        bold = re.search(r"\*\*(.+?)\*\*", segment, re.S)
        label = re.sub(r"\s+", " ", bold.group(1)).strip() if bold else ""
        if not label:
            out.append((f"claude/?@{m.group(1)}", f"CLAUDE.md {m.group(0)} — no bold label"))
            continue
        slug = re.sub(r"[^a-z0-9]+", "-", label.lower()).strip("-")[:44]
        out.append((f"claude/{slug}@{m.group(1)}", f"CLAUDE.md **{label[:60]}** ({m.group(1)})"))

    return out


def check_binding(binding: str) -> str | None:
    """None when the binding resolves, else why it does not."""
    path_part, _, pattern = binding.partition("::")
    target = ROOT / path_part.strip()
    if not target.exists():
        return f"path does not exist: {path_part.strip()}"
    if not pattern:
        return None
    if target.is_dir():
        return f"a pattern needs a file, not a directory: {path_part.strip()}"
    try:
        rx = re.compile(pattern.strip())
    except re.error as exc:
        return f"binding pattern is not a regex: {exc}"
    if not rx.search(target.read_text(encoding="utf-8", errors="replace")):
        return f"pattern {pattern.strip()!r} matches nothing in {path_part.strip()}"
    return None


def main() -> int:
    if not LEDGER.exists():
        print(f"check-decisions: FAIL — no ledger at {LEDGER.relative_to(ROOT)}")
        return 1

    findings: list[str] = []
    rows: dict[str, dict[str, str]] = {}
    sources: set[str] = set()

    for raw in LEDGER.read_text(encoding="utf-8").splitlines():
        m = ROW.match(raw)
        if not m:
            continue
        row = m.groupdict()
        rid = row["id"]
        if rid in rows:
            findings.append(f"{rid}: duplicate row id")
            continue
        rows[rid] = row
        for src in row["source"].split(";"):
            src = src.strip().strip("`")
            if src:
                sources.add(src)

    if not rows:
        print("check-decisions: FAIL — the ledger parsed to zero rows")
        return 1

    # FORWARD
    open_rows: list[dict[str, str]] = []
    unenforced_rows: list[dict[str, str]] = []
    landed = 0
    constitutive = 0
    for rid, row in sorted(rows.items()):
        status = row["status"]
        if status not in STATUSES:
            findings.append(f"{rid}: status {status!r} is not one of {sorted(STATUSES)}")
            continue
        if status in ("open", "unenforced"):
            (open_rows if status == "open" else unenforced_rows).append(row)
            if row["binding"].strip() not in ("", "—"):
                findings.append(f"{rid}: status is `{status}` but it names a binding")
            continue
        if status == "constitutive":
            binding = row["binding"].strip().strip("`")
            if not binding.startswith("docs/adr/"):
                findings.append(
                    f"{rid}: `constitutive` is for architecture-defining ADRs only — "
                    f"a decision with a deliverable binds to the deliverable"
                )
                continue
            why = check_binding(binding)
            if why:
                findings.append(f"{rid}: binding does not resolve — {why}")
            else:
                constitutive += 1
            continue
        if status == "superseded":
            if not re.match(r"DEC-\d{4}$", row["binding"].strip()):
                findings.append(f"{rid}: superseded rows bind to the superseding DEC id")
            elif row["binding"].strip() not in rows:
                findings.append(f"{rid}: superseded by {row['binding'].strip()}, which has no row")
            continue
        binding = row["binding"].strip().strip("`")
        if not binding or binding == "—":
            findings.append(f"{rid}: status is `landed` with no binding — say where it landed")
            continue
        why = check_binding(binding)
        if why:
            findings.append(f"{rid}: binding does not resolve — {why}")
        else:
            landed += 1

    # REVERSE — the direction that keeps the ledger honest
    for key, label in documented_decisions():
        if key not in sources:
            findings.append(
                f"{label}: a recorded decision with no ledger row "
                f"(expected some row to name source `{key}`)"
            )

    for f in findings:
        print(f"check-decisions: {f}")

    today = date.today()
    print(
        f"check-decisions: {len(rows)} decision(s) — {landed} landed and bound, "
        f"{constitutive} constitutive, {len(open_rows)} open, "
        f"{len(unenforced_rows)} unenforced, "
        f"{sum(1 for r in rows.values() if r['status'] == 'superseded')} superseded; "
        f"{len(documented_decisions())} documented decision(s) cross-checked"
    )
    def report(rows_: list[dict[str, str]], banner: str) -> None:
        if not rows_:
            return
        print(f"check-decisions: {banner}")
        for row in sorted(rows_, key=lambda r: r["date"]):
            try:
                age = (today - date.fromisoformat(row["date"])).days
            except ValueError:
                age = -1
            print(f"    {row['id']}  {row['date']}  ({age}d)  {row['decision'].strip()}")

    report(open_rows, "OPEN — decided, deliverable not built:")
    report(unenforced_rows, "UNENFORCED — a rule with no check behind it, held by memory alone:")

    return 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main())
