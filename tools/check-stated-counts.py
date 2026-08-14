#!/usr/bin/env python3
"""A number stated in a reference document is bound to the thing that decides it.

A reference page that says "ten teaching programs" is asserting a fact about the
build, and nothing computed it. The number was typed by whoever last counted, and
it goes stale the way every uncomputed number goes stale — silently, in the
direction of the tree moving on. This is not a tidiness concern: an authoring
session reads `grammar.md` §2c to decide what the language can express, and "the
nine below" over a table of ten is a page that tells it a technique does not
exist.

THE SHAPE THIS EXISTS FOR — an integration, not an edit. Two branches changed the
idiom index's *intent*. One added a tenth technique (`idiom-arguments`) and moved
the count in `tools.md`, `prefab-procedure.md`, the `/new-delve` skill and the
index table. The other, which never touched the library, added a section above
that table opening "Not a tenth technique and not a row in the table: the nine
below…". `git merge-tree` resolves the pair with ZERO conflict markers — they are
different regions of the file — and the merged tree ships a page whose own
sentence contradicts its own table three lines down. `check-doc-dupes`,
`check-dw-codes`, `check-reference-versions` and `check-grammar-ir-compat` are
all green on that tree. Docs merge as text and are never re-read; a textual
conflict count measures nothing (CLAUDE.md, integration of #395+#400+#402+#403).

WHAT THIS CHECKS. Not the idiom count. An idiom-count check would be keyed to the
verb that first needed it, and the next reference page to state a number would
have no surface — so the fix would look like a second bespoke check, which is the
defect rather than the fix (CLAUDE.md: a capability belongs to the object class
it acts on). The object class is *a count stated in prose*, and the machinery is:

  ORACLE    a named computation over the tree that yields the true number, the
            evidence it came from, and the PHRASINGS that state it. A phrasing
            captures one number and carries an OFFSET, which is how an ordinal
            claim ("not a tenth technique") states a cardinal fact (count + 1).
  SITE      a page, optionally one section of it, that states that oracle.

Binding a second count is a SITE row and at most one oracle function. It is never
a second script.

BINDING, AND WHAT A ZERO MEANS. Every site reports how many claims it matched on
every run, and a site that matched ZERO is a FAILURE, not a pass (CLAUDE.md: a
green gate that binds to nothing is vacuous). A site goes to zero when every
sentence stating its count is reworded away — exactly when this gate would
otherwise go quietly dark over that page. Per-phrasing hit counts are printed
too, but a phrasing matching zero is NOT a failure: the phrasings are the
vocabulary for stating a count, not an assertion that this page uses each one,
and requiring every one to bind would mean the registry could only ever describe
the tree in front of it.

There is deliberately NO allowlist and no per-claim opt-out. An escape hatch here
would be satisfied by the very drift it exempts (CLAUDE.md, the sixth vacuity
mode: an opt-out must be secured by a property the defect cannot supply), so the
only ways to green are a true number or a page that does not state one.

STRUCTURAL CLAIMS. A document can also state a set by ENUMERATING it. `grammar.md`
§2c's table is the idiom index, so its rows are checked against the library's
`idiom-*` programs by SET, not by count: a program added to the library with no
row, or a row naming a program that does not exist, is a red no count comparison
would catch. It is also what stops that table serving as an oracle for the
technique count after it has quietly stopped describing the library.

WHAT IT DOES NOT CHECK. Every number in prose. Most counts in `docs/reference/`
are narrative measurements of one run ("28 of 63 roofed cells unreached") and
name no enumerable set; binding those would mean inventing an oracle per
sentence. The registry below is the counts that name a set the repo enumerates,
and it is grown by hand — a count nobody has registered is unbound, and this
script cannot tell you it exists. Nor does it bind a claim about a TOOL'S OUTPUT
to what the tool prints; that is the same class and is not yet built.

KNOWN UNBOUND, at the time this was written: `docs/reference/tools.md` and
`docs/reference/compiler.md` state the campaign's effect-root count as five and
as seven, while `EffectRootKind::ALL` carries eight. Three prose sites, one
oracle, all wrong — the same class, found by the survey that produced this
registry. It is not registered here because the phrase "N roots" appears in
~40 narrative sentences across the workspace and telling the claims from the
history needs the prose edit that fixes them, which belongs to whoever owns
those pages.

Deterministic, offline, stdlib-only. Exit 0 = pass, 1 = a finding.
"""

from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent

# --------------------------------------------------------------------- numbers

CARDINALS = {
    "one": 1, "two": 2, "three": 3, "four": 4, "five": 5, "six": 6,
    "seven": 7, "eight": 8, "nine": 9, "ten": 10, "eleven": 11, "twelve": 12,
    "thirteen": 13, "fourteen": 14, "fifteen": 15, "sixteen": 16,
    "seventeen": 17, "eighteen": 18, "nineteen": 19, "twenty": 20,
    "thirty": 30, "forty": 40, "fifty": 50,
}
ORDINALS = {
    "first": 1, "second": 2, "third": 3, "fourth": 4, "fifth": 5, "sixth": 6,
    "seventh": 7, "eighth": 8, "ninth": 9, "tenth": 10, "eleventh": 11,
    "twelfth": 12, "thirteenth": 13, "fourteenth": 14, "fifteenth": 15,
    "sixteenth": 16, "seventeenth": 17, "eighteenth": 18, "nineteenth": 19,
    "twentieth": 20,
}

#: A cardinal, as a word or as digits. Interpolated into every phrasing so that a
#: page switching from "ten" to "10" cannot silently stop being checked.
CARD = "(?:%s|\\d{1,4})" % "|".join(sorted(CARDINALS, key=len, reverse=True))
#: An ordinal, word-only. "the 10th technique" is not a phrasing this repo uses,
#: and accepting digits here would match list markers and version numbers.
ORD = "(?:%s)" % "|".join(sorted(ORDINALS, key=len, reverse=True))


def parse_number(token: str) -> int | None:
    """`"ten"` / `"10"` / `"tenth"` -> 10. `None` if it is not a number word."""
    t = token.strip().lower()
    if t.isdigit():
        return int(t)
    return CARDINALS.get(t, ORDINALS.get(t))


# --------------------------------------------------------------- reading files

_FENCE_RE = re.compile(r"^\s*(```|~~~)")
_HEADING_RE = re.compile(r"^(#{1,6})\s")


def strip_code_fences(text: str) -> str:
    """Blank out fenced code blocks, keeping line numbers intact.

    A shell transcript is not a claim: `delve-grammar expand --region 15x9x3`
    carries numbers that name nothing enumerable.
    """
    out, fence = [], None
    for line in text.splitlines():
        m = _FENCE_RE.match(line)
        if fence is None and m:
            fence = m.group(1)
            out.append("")
            continue
        if fence is not None:
            out.append("")
            if m and m.group(1) == fence:
                fence = None
            continue
        out.append(line)
    return "\n".join(out)


def section_of(text: str, heading: str) -> str:
    """The region under the first heading matching `heading`, line numbers kept.

    Ends at the next heading of the same or higher level, so subsections belong
    to their parent. Raises when the heading is absent: a section anchor that
    matches nothing would silently reduce its site to zero claims, which is the
    unbound mode this gate exists to refuse.
    """
    pat = re.compile(heading, re.M)
    lines = text.splitlines()
    start = None
    for i, line in enumerate(lines):
        if pat.match(line):
            start = i
            break
    if start is None:
        raise LookupError(f"no heading matches {heading!r}")
    level = len(_HEADING_RE.match(lines[start]).group(1))
    end = len(lines)
    for j in range(start + 1, len(lines)):
        m = _HEADING_RE.match(lines[j])
        if m and len(m.group(1)) <= level:
            end = j
            break
    return "\n".join(
        line if start <= i < end else "" for i, line in enumerate(lines)
    )


# --------------------------------------------------------------------- oracles

_LIBRARY_MOD = "crates/grammar/src/library/mod.rs"
_GRAMMAR_MD = "docs/reference/grammar.md"
_INDEX_SECTION = r"^## 2c\. "

_PROGRAMS_RE = re.compile(
    r"pub const PROGRAMS: &\[LibraryProgram\] = &\[(?P<body>.*?)^\];", re.S | re.M
)
_ENTRY_RE = re.compile(r'^\s*\("(?P<id>[a-z0-9\-]+)",', re.M)
#: A row of the §2c index table: `| 7 | Symmetry | `idiom-mirror` | … |`, or the
#: composition demonstration, whose first cell is an em dash rather than a number.
_INDEX_ROW_RE = re.compile(
    r"^\|\s*(?P<n>\d+|—|-)\s*\|[^|]*\|\s*`(?P<id>idiom-[a-z0-9\-]+)`\s*\|", re.M
)


def library_program_ids(root: pathlib.Path) -> list[str]:
    """Every id in `library::PROGRAMS` — what `delve-grammar list` names."""
    src = (root / _LIBRARY_MOD).read_text(encoding="utf-8")
    m = _PROGRAMS_RE.search(src)
    if not m:
        raise LookupError(
            f"{_LIBRARY_MOD}: the `PROGRAMS` table did not parse. It is the "
            "oracle behind every library-program count stated in the reference; "
            "if its declaration moved, update _PROGRAMS_RE here."
        )
    return [e["id"] for e in _ENTRY_RE.finditer(m["body"])]


def index_table_rows(root: pathlib.Path) -> list[tuple[str, str]]:
    """`(first cell, program id)` for every row of the §2c idiom index table."""
    # Fences first, then the section: a `# comment` line inside a shell block
    # is indistinguishable from an H1, and would end §2c above its own table.
    body = section_of(
        strip_code_fences((root / _GRAMMAR_MD).read_text(encoding="utf-8")),
        _INDEX_SECTION,
    )
    rows = [(m["n"], m["id"]) for m in _INDEX_ROW_RE.finditer(body)]
    if not rows:
        raise LookupError(
            f"{_GRAMMAR_MD} §2c: the idiom index table did not parse — zero "
            "rows. Every technique count in the reference is measured against "
            "it, so an unparsed table is a red rather than a count of nought."
        )
    return rows


def oracle_library_programs(root: pathlib.Path) -> tuple[int, str]:
    ids = library_program_ids(root)
    return len(ids), f"{_LIBRARY_MOD} PROGRAMS[], {len(ids)} ids"


def oracle_idiom_programs(root: pathlib.Path) -> tuple[int, str]:
    ids = [i for i in library_program_ids(root) if i.startswith("idiom-")]
    return len(ids), f"{_LIBRARY_MOD} PROGRAMS[] `idiom-*`: {', '.join(ids)}"


def oracle_idiom_techniques(root: pathlib.Path) -> tuple[int, str]:
    """The numbered rows of the §2c index table.

    The table IS the index — "the nine below" points straight at it — so it is
    the thing that decides the number. It does not get to decide alone:
    `structural_claims` binds its rows to the library's `idiom-*` set by
    identity, so a table that has stopped describing the library cannot serve as
    anyone's oracle.
    """
    rows = index_table_rows(root)
    numbered = [r for r in rows if r[0].isdigit()]
    return len(numbered), (
        f"{_GRAMMAR_MD} §2c index table, {len(numbered)} numbered rows "
        f"of {len(rows)}"
    )


#: id -> (what the number counts, how to compute it, how prose states it).
#: A phrasing is `(regex capturing one number, offset)`; the captured number is
#: expected to equal the oracle plus the offset.
ORACLES: dict[str, dict] = {
    "library-programs": {
        "describe": "programs `delve-grammar list` names",
        "compute": oracle_library_programs,
        "phrasings": [
            (rf"\b({CARD}) library programs\b", 0),
        ],
    },
    "idiom-programs": {
        "describe": "teaching programs in the `idiom-*` block",
        "compute": oracle_idiom_programs,
        "phrasings": [
            (rf"\b({CARD})\s+teaching programs\b", 0),
        ],
    },
    "idiom-techniques": {
        "describe": "techniques of the idiom index",
        "compute": oracle_idiom_techniques,
        "phrasings": [
            (rf"\b({CARD})\s+techniques\b", 0),
            (rf"\bthe ({CARD}) below\b", 0),
            (rf"\bany of the ({CARD})\b(?![-\w])", 0),
            (rf"\busually one of the ({CARD})\b(?![-\w])", 0),
            (rf"\bof the ({CARD}) at once\b", 0),
            # `an?`, because the ordinal that follows it decides the article:
            # "not a tenth technique" and "not an eleventh technique" are the
            # same claim, and a pattern that only reads `a` stops binding the
            # sentence at exactly the count where the sentence changed.
            (rf"\bnot an? ({ORD}) technique\b", 1),
        ],
    },
}

#: The pages that state those counts. `section` narrows the search to one region
#: (see `section_of`); without it the whole page is searched.
SITES: list[dict] = [
    {"oracle": "idiom-programs", "path": "docs/reference/tools.md"},
    {"oracle": "library-programs", "path": _GRAMMAR_MD},
    {"oracle": "idiom-techniques", "path": _GRAMMAR_MD, "section": _INDEX_SECTION},
    {"oracle": "idiom-techniques", "path": "docs/reference/prefab-procedure.md"},
    # The skill reads the same index, and the tooling-sync rule lands it in the
    # same PR as the reference — the same claim, in the place the second
    # consumer of a class always turns out to live.
    {"oracle": "idiom-techniques", "path": ".claude/skills/new-delve/SKILL.md"},
]


def structural_claims(root: pathlib.Path) -> list[str]:
    """Sets a document states by enumeration, checked as sets not as counts."""
    errors = []
    rows = index_table_rows(root)
    table_ids = {i for _, i in rows}
    library_ids = {i for i in library_program_ids(root) if i.startswith("idiom-")}
    if missing := sorted(library_ids - table_ids):
        errors.append(
            f"{_GRAMMAR_MD} §2c's index table has no row for "
            f"{', '.join(missing)}, which `library::PROGRAMS` carries. The "
            "table is the index a creator reads, and a technique no program in "
            "it demonstrates does not exist in practice (§2c)."
        )
    if extra := sorted(table_ids - library_ids):
        errors.append(
            f"{_GRAMMAR_MD} §2c's index table has a row for "
            f"{', '.join(extra)}, which `library::PROGRAMS` does not carry — "
            "`delve-grammar show --program <id>` refuses it."
        )
    if len(table_ids) != len(rows):
        errors.append(
            f"{_GRAMMAR_MD} §2c's index table has {len(rows)} rows naming only "
            f"{len(table_ids)} distinct programs — a row is duplicated."
        )
    return errors


# ------------------------------------------------------------------- machinery


def check_site(root: pathlib.Path, site: dict) -> tuple[int, list[str], list[str]]:
    """`(claims matched, errors, per-phrasing report lines)` for one site."""
    oracle_id = site["oracle"]
    oracle = ORACLES[oracle_id]
    expected, evidence = oracle["compute"](root)

    path = root / site["path"]
    if not path.exists():
        return 0, [
            f"{site['path']} is missing, and SITES says it states the "
            f"{oracle_id} count. Either the page moved — update SITES — or the "
            "count is now stated nowhere."
        ], []

    text = strip_code_fences(path.read_text(encoding="utf-8"))
    if "section" in site:
        try:
            text = section_of(text, site["section"])
        except LookupError as e:
            return 0, [f"{site['path']}: {e}"], []
    lines = text.splitlines()

    matched, errors, report = 0, [], []
    for pattern, offset in oracle["phrasings"]:
        rx = re.compile(pattern, re.I)
        hits = 0
        for n, line in enumerate(lines, 1):
            for m in rx.finditer(line):
                hits += 1
                matched += 1
                got, want = parse_number(m.group(1)), expected + offset
                if got == want:
                    continue
                claim = (
                    f"claims there is no number {got}"
                    if offset
                    else f"claims there are {got}"
                )
                errors.append(
                    f"{site['path']}:{n} states {m.group(0)!r}, which {claim}. "
                    f"There are {expected} {oracle['describe']} "
                    f"[{evidence}].\n"
                    f"    {lines[n - 1].strip()[:150]}"
                )
        report.append(f"      {hits:>2} × {short(pattern)}")
    return matched, errors, report


def short(pattern: str) -> str:
    """A phrasing with the number alternation folded back to `<N>`, for reading."""
    return "/" + pattern.replace(CARD, "<N>").replace(ORD, "<Nth>") + "/"


def main() -> int:
    errors: list[str] = []
    report: list[str] = []
    total_claims = 0

    for oracle_id, oracle in ORACLES.items():
        try:
            value, evidence = oracle["compute"](ROOT)
        except (LookupError, OSError) as e:
            errors.append(f"oracle {oracle_id} could not be computed: {e}")
            continue
        report.append(f"  {oracle_id} = {value}   [{evidence}]")

    for site in SITES:
        where = site["path"] + (
            f"  [section {site['section']}]" if "section" in site else ""
        )
        try:
            matched, site_errors, phrasings = check_site(ROOT, site)
        except (LookupError, OSError) as e:
            errors.append(f"{where}: {e}")
            continue
        total_claims += matched
        errors.extend(site_errors)
        report.append(f"    {where}: {matched} stated {site['oracle']} count(s)")
        report.extend(phrasings)
        if matched == 0 and not site_errors:
            errors.append(
                f"{site['path']} states no {site['oracle']} count at all, and "
                "SITES says it does. A site that binds to zero is a finding, "
                "not a pass: the sentence that stated the count was reworded or "
                "removed, and this gate has gone dark over that page. Re-state "
                "it, add the new phrasing to ORACLES, or drop the site."
            )

    try:
        errors.extend(structural_claims(ROOT))
    except (LookupError, OSError) as e:
        errors.append(f"the §2c index table could not be read: {e}")

    if total_claims == 0:
        errors.append(
            "zero stated counts examined across the whole registry — it binds "
            "to nothing, which is vacuous rather than green."
        )

    for e in errors:
        print(f"check-stated-counts: FAIL — {e}", file=sys.stderr)
    print("\n".join(report))
    print(
        f"check-stated-counts: {'FAIL' if errors else 'OK'} — "
        f"{len(ORACLES)} oracles, {len(SITES)} prose sites, "
        f"{total_claims} stated counts examined."
    )
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
