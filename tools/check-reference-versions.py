#!/usr/bin/env python3
"""Version-claim gate for the documents a reader ACTS on (bidirectional).

Two document sets, one question: does the version a reader is told match the
version the build has? `docs/reference/compiler.md` is where an authoring
session learns which `dsl_version` to write; the crates.io front pages are where
a stranger learns whether they can use the crate at all. Both are bound here,
against the same constants, by EQUALITY in both directions.

`docs/reference/compiler.md` is the authoritative current-behavior record for
`delvec` (CLAUDE.md Methodology), and its very first factual claim is the
"Versions (as of this doc)" line: the delvec version, the DSL version, the
pinned Minecraft version, and the full list of `dsl_version` values a campaign
may declare. A reader ACTS on that line — it is where an authoring session
learns which `dsl_version` to write into a stage envelope.

That line was bound to nothing. It read `delvec 0.1.0`, `dsl 0.8.0` and listed
`0.2.0 … 0.8.0` while the build was at `delvec 1.1.0`, `dsl 0.9.0` and accepted
`0.9.0` — every other gate green, because no gate related the two. The body of
the same document described the v0.9 surface correctly; only the header a reader
consults first was wrong. `tools/check-dw-codes.py` keeps the DIAGNOSTICS
catalog honest against the source in both directions; nothing did the same for
the versions, and the skill's own gate — in the campaigns repository, where the
page lives — binds only the *skill's* `verified_with` to the engine, never the
reference doc's.

## What is bound, and to what

Every claim below is bound by EQUALITY, not by "at least" — a claim may be
neither stale-older nor prematurely-newer:

- `delvec <X>`  == `crates/compiler/Cargo.toml` `[package] version`
  (the same constant the skill gate calls the engine version)
- `dsl <Y>`     == `crates/dsl/src/envelope.rs` `SUPPORTED_DSL_VERSION`
- `mc <Z>`      == `versions.toml` `[minecraft] version`
- the bold supported-`dsl_version` list == `crates/dsl/src/envelope.rs`
  `SUPPORTED_DSL_VERSIONS` **minus** `RESERVED_DSL_VERSIONS`, as an ORDERED
  sequence. A reserved version is in the ledger and refused: the number is held
  so a second change cannot take it, and `is_supported_version` says no. Binding
  the page to the whole ledger would make this gate DEMAND a doc promise the
  build refuses.
- the `DW0102` catalog row's `{…}` set == the same accepted list, because
  `DW0102` fires on exactly `!is_supported_version(version)`
  (`crates/dsl/src/validate.rs`) and its row restates that set by hand

That last one is a second instance of the same defect, found while fixing the
first: the row read `{0.2.0 … 0.8.0}` with `0.9.0` accepted and tested.
`tools/check-dw-codes.py` was green on it and always would be — that gate proves
a code EXISTS in both source and doc and is asserted by a test, never that the
BEHAVIOR the doc ascribes to it is the behavior the code has. A code's prose is
otherwise unbound, and this is the mechanically checkable slice of it.

Equality in both directions is the point. A gate that only rejected a version
NEWER than the build is exactly the shape that let a storybook ship a stale
`v1.0` marker through the whole `v1.1` release with a green check:
the stale-older direction is the one that actually happens, because docs are
written once and the build moves.

The ordered-sequence comparison matters too: the list doubles as the reading
order for the "additive superset" claim beside it, and a set comparison would
pass on a shuffled list.

## The same claims, on the pages a stranger reads

`crates/compiler/README.md` and `crates/dsl/README.md` are rendered VERBATIM as
the crates.io front pages of `delvec` and `delvewright-dsl`, and each states the
Minecraft version, the `dsl_version` window and the minimum Rust — the three
facts that decide whether a visitor can use the crate at all. Those were the
same numbers this file already owned, and on those two pages they were bound to
NOTHING. The next `dsl_version` bump would have made a stranger-facing page
wrong, in the direction drift actually goes: a doc is written once and the
build moves.

The file set is DERIVED, never listed — every crate under `crates/*/` whose
`[package] publish` is not `false`, resolved through its `[package] readme`
(`tools/lib/publishable.py`, shared with `tools/check-crates-io-readmes.py`). A
crate that later becomes publishable inherits this gate with no edit here.
Examining zero pages is a red, not a pass.

Two rules per page, and the second is the one that catches prose:

1. **The three labelled claims must be present and equal to the build** —
   `**Minecraft**: Java Edition <mc>`, ``**Campaign format**: `dsl_version`
   `<first>` through `<last>` `` and `**Rust**: <rust-version> or newer`. Present
   AND equal: a page that quietly drops its compatibility section stops telling a
   stranger the one thing they need, so an absent claim is a shape error (exit 2),
   never a silent pass.
2. **No unbound version literal anywhere on the page.** Every `X.Y.Z` on the page
   must be one of the build's own constants — the pinned Minecraft version, a
   supported `dsl_version`, or a publishable crate's `version` / `rust-version`.
   Rule 1 alone binds only the compatibility bullets; the `delvec` page states the
   Minecraft version three times, and "the vendored 1.21.11 Brigadier command
   tree" is prose that rule 1 cannot see. Under rule 2 an `mc` bump reds every
   stale mention at once, with line numbers.

Deterministic, offline, no dependencies (Python 3 stdlib). Run from the repo
root:
    python3 tools/check-reference-versions.py
Exit 0 = the header matches the build, 1 = it drifted (see stderr), 2 =
usage/IO error, or a source file no longer matches the expected shape — fix the
regex, never loosen the check (CLAUDE.md debug doctrine).
"""

import pathlib
import re
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent / "lib"))

from lib import mdtable  # noqa: E402
from publishable import DerivationError, readmes  # noqa: E402

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
DOC = REPO_ROOT / "docs" / "reference" / "compiler.md"
COMPILER_CARGO_TOML = REPO_ROOT / "crates" / "compiler" / "Cargo.toml"
ENVELOPE_RS = REPO_ROOT / "crates" / "dsl" / "src" / "envelope.rs"
VERSIONS_TOML = REPO_ROOT / "versions.toml"

# `- Versions (as of this doc): `delvec 1.1.0`, `dsl 0.9.0`, `mc 1.21.11`.`
DOC_VERSIONS_RE = re.compile(
    r"Versions \(as of this doc\):\s*`delvec ([^`]+)`,\s*`dsl ([^`]+)`,\s*"
    r"`mc ([^`]+)`"
)

# The bold list that follows it:
#   Supported campaign `dsl_version`: **`0.2.0`, ..., `0.9.0`**
DOC_SUPPORTED_RE = re.compile(
    r"Supported campaign `dsl_version`:\s*\*\*(.+?)\*\*", re.DOTALL
)

# `[package]` ... `version = "1.1.0"` — first `version =` at line start wins,
# which is the package version in both crate manifests here.
CARGO_VERSION_RE = re.compile(r'(?m)^version\s*=\s*"([^"]+)"')

RS_SUPPORTED_ONE_RE = re.compile(
    r'pub\s+const\s+SUPPORTED_DSL_VERSION\s*:\s*&str\s*=\s*"([^"]+)"\s*;'
)
RS_SUPPORTED_ALL_RE = re.compile(
    r"pub\s+const\s+SUPPORTED_DSL_VERSIONS\s*:\s*&\[&str\]\s*=\s*&\[(.*?)\]\s*;",
    re.DOTALL,
)

# `pub const RESERVED_DSL_VERSIONS: &[(&str, &str)] = &[("0.12.0", "OPEN_WAY_SINCE")];`
#
# A reserved version is IN the ledger and NOT accepted: it is held so a second
# change cannot take the number, and `is_supported_version` refuses it. So every
# claim this gate binds — the header list, the `DW0102` row, the crate pages'
# `<first>` through `<last>` — is bound to the ledger MINUS its reservations. Bind
# them to the whole ledger instead and this gate would force the doc to promise a
# version the build refuses, which is the stale-claim defect it exists to stop,
# arriving through the gate itself.
RS_RESERVED_RE = re.compile(
    r"pub\s+const\s+RESERVED_DSL_VERSIONS\s*:\s*&\[\(&str,\s*&str\)\]\s*=\s*&\[(.*?)\]\s*;",
    re.DOTALL,
)
RESERVED_ROW_RE = re.compile(r'\(\s*"([^"]+)"\s*,\s*"([^"]+)"\s*\)')

# `[minecraft]` ... `version = "1.21.11"`
MC_VERSION_RE = re.compile(r'(?ms)^\[minecraft\]\s*$.*?^version\s*=\s*"([^"]+)"')

# The DW0102 catalog row restates the same set by hand:
#   | `DW0102` | Unsupported `dsl_version` (not in `{0.2.0,…,0.9.0}`). |
#
# It is looked for among the rows a TABLE holds, not anywhere in the file. A
# blank line ends a pipe table, so a row under one renders as a paragraph of
# literal pipe characters — it would restate the set for this gate and show a
# reader nothing. `compiler.md` carried twenty-one such rows at once.
DOC_DW0102_ROW = re.compile(r"^\|\s*`DW0102`\s*\|")
DOC_DW0102_RE = re.compile(r"\|\s*`DW0102`\s*\|[^|]*?not in `\{([^}]*)\}`")

QUOTED_RE = re.compile(r'"([^"]+)"')
BACKTICKED_RE = re.compile(r"`([^`]+)`")

# --- the crates.io front pages ---------------------------------------------

# `- **Minecraft**: Java Edition 1.21.11.`
README_MC_RE = re.compile(r"\*\*Minecraft\*\*:\s*Java Edition\s+`?(\d[\d.]*\d)`?")
# ``- **Campaign format**: `dsl_version` `0.2.0` through `0.10.0`.``
README_FORMAT_RE = re.compile(
    r"\*\*Campaign format\*\*:\s*`dsl_version`\s+`([^`]+)`\s+through\s+`([^`]+)`"
)
# `- **Rust**: 1.97.1 or newer.`
README_RUST_RE = re.compile(r"\*\*Rust\*\*:\s*`?(\d[\d.]*\d)`?\s+or newer")

# Any dotted numeric run, wherever it sits in the prose; the caller keeps the
# three-component ones. Matching greedily and filtering afterwards is what makes
# `GPL-3.0-only` (two components) and `1.2.3.4` (four) fall out on their own.
#
# The right-hand guard is `(?!\w)` and NOT `(?![\w.])`, which is the shape this
# first shipped with and was silently blind: a version at the end of a sentence
# — `Java Edition 1.21.11.`, which is how BOTH published pages write it — is
# followed by a full stop, so a lookahead that forbids a trailing dot matched
# nothing on either page and rule 2 examined zero literals while printing green.
# Caught by the test that plants a stale literal in prose.
VERSION_LITERAL_RE = re.compile(r"(?<![\d.])\d+(?:\.\d+)+(?!\w)")

# Version literals on a published page that are deliberately NOT one of this
# build's constants — a third-party version a reader has to know, say. Keyed by
# (repo-relative posix README path, literal), value = the justification.
#
# EMPTY ON PURPOSE, and the empty state is the design: every number on these two
# pages today is a number this repo owns, so any new one is a claim that needs a
# reason written down. A stale entry (naming a page no longer scanned) is
# reported, so this cannot rot into a licence to hardcode.
UNBOUND_VERSION_LITERALS: dict[tuple[str, str], str] = {}


def fail_shape(what: str, path: pathlib.Path, knob: str) -> int:
    try:
        shown = path.relative_to(REPO_ROOT)
    except ValueError:
        shown = path
    sys.stderr.write(
        f"error: could not find {what} in {shown} — the\n"
        f"       source was renamed or reshaped. Update {knob} in\n"
        f"       tools/check-reference-versions.py. Do NOT loosen the check.\n"
    )
    return 2


class PageShapeError(Exception):
    """A published page no longer carries a claim this gate binds.

    Raised, not returned, so it can never be mistaken for "the claim is fine".
    A page that drops its compatibility section stops telling a stranger the one
    thing they need in order to use the crate, which is a finding about the page
    — the caller turns this into the same loud exit 2 a reshaped source gets.
    """

    def __init__(self, what: str, path: pathlib.Path, knob: str) -> None:
        super().__init__(what)
        self.what, self.path, self.knob = what, path, knob


def check_published_pages(
    root: pathlib.Path, real_mc: str, real_supported: list[str]
) -> tuple[list[str], list[str], set[str], int]:
    """Bind every version claim on every crates.io front page to the build.

    Returns (problems, pages examined, the constants pages may name, how many
    version literals rule 2 actually looked at). Raises
    `DerivationError` when the file set cannot be derived and `PageShapeError`
    when a derived page has lost a claim — both are exit 2 at the caller.
    """
    crates = readmes(root)
    if not crates:
        raise DerivationError(
            "no publishable crate serves a README, so this gate examined zero "
            "pages. A green that binds to nothing is not a pass (CLAUDE.md): "
            "either every crate gained `publish = false` deliberately, or the "
            "derivation in tools/lib/publishable.py stopped matching the tree."
        )

    # What a page is allowed to say: the pinned game version, any `dsl_version`
    # the build accepts, and any publishable crate's own version or minimum
    # toolchain. Anything else is a number nothing in the build owns.
    known: set[str] = {real_mc, *real_supported}
    for crate in crates:
        known.add(crate.version)
        if crate.rust_version:
            known.add(crate.rust_version)

    problems: list[str] = []
    pages: list[str] = []
    literal_counts: list[int] = []

    for crate in crates:
        rel = crate.readme_rel(root)
        pages.append(rel)
        text = crate.readme.read_text(encoding="utf-8")

        m = README_MC_RE.search(text)
        if m is None:
            raise PageShapeError(
                "the `**Minecraft**: Java Edition <version>` compatibility claim",
                crate.readme,
                "README_MC_RE",
            )
        if m.group(1) != real_mc:
            problems.append(
                f"  {rel}: `Minecraft Java Edition {m.group(1)}` != `{real_mc}` "
                "in the build\n"
                "      source of truth: versions.toml [minecraft] version"
            )

        m = README_FORMAT_RE.search(text)
        if m is None:
            raise PageShapeError(
                "the ``**Campaign format**: `dsl_version` `<first>` through "
                "`<last>``` claim",
                crate.readme,
                "README_FORMAT_RE",
            )
        want_lo, want_hi = real_supported[0], real_supported[-1]
        if (m.group(1), m.group(2)) != (want_lo, want_hi):
            problems.append(
                f"  {rel}: `dsl_version {m.group(1)} through {m.group(2)}` != "
                f"`{want_lo} through {want_hi}` in the build\n"
                "      source of truth: crates/dsl/src/envelope.rs "
                "SUPPORTED_DSL_VERSIONS minus RESERVED_DSL_VERSIONS "
                "(first and last)"
            )

        m = README_RUST_RE.search(text)
        if m is None:
            raise PageShapeError(
                "the `**Rust**: <version> or newer` claim",
                crate.readme,
                "README_RUST_RE",
            )
        if crate.rust_version is None:
            problems.append(
                f"  {rel}: the page states a minimum Rust of {m.group(1)} and "
                f"`{crate.rel(root)}/Cargo.toml` declares no `rust-version` — "
                "the claim is bound to nothing, and `cargo install` is where a "
                "stranger finds out"
            )
        elif m.group(1) != crate.rust_version:
            problems.append(
                f"  {rel}: `Rust {m.group(1)} or newer` != `{crate.rust_version}` "
                "in the build\n"
                f"      source of truth: {crate.rel(root)}/Cargo.toml "
                "[package] rust-version"
            )

        # Rule 2 — the one that reaches prose the labelled claims never touch.
        literals_seen = 0
        for n, line in enumerate(text.splitlines(), start=1):
            for literal in VERSION_LITERAL_RE.findall(line):
                if literal.count(".") != 2:
                    continue  # `GPL-3.0-only`, `delvewright-dsl = "0.1"`
                literals_seen += 1
                if literal in known or (rel, literal) in UNBOUND_VERSION_LITERALS:
                    continue
                problems.append(
                    f"  {rel}:{n}: version literal `{literal}` is not one this "
                    "build owns\n"
                    f"      the build's constants are: "
                    f"{', '.join(sorted(known))}\n"
                    "      a stale mention in prose is exactly how a published "
                    "page goes wrong"
                )
        # The count is the binding, and it is stated because this rule has
        # already been vacuous once: with a lookahead that forbade a trailing
        # dot, rule 2 matched nothing on either real page and reported green.
        # Rule 1 guarantees at least four literals on any page that reaches
        # here, so zero means the scanner, not the page.
        if literals_seen == 0:
            problems.append(
                f"  {rel}: rule 2 examined ZERO version literals on a page whose "
                "labelled claims all matched\n"
                "      — VERSION_LITERAL_RE stopped matching. Fix the regex; a "
                "rule that scans nothing is not a pass"
            )
        literal_counts.append(literals_seen)

    scanned = set(pages)
    for path, literal in sorted(UNBOUND_VERSION_LITERALS):
        if path not in scanned:
            problems.append(
                f"  UNBOUND_VERSION_LITERALS carries ({path!r}, {literal!r}), "
                "which names a page outside the scanned set — remove it rather "
                "than letting it rot into a licence to hardcode"
            )

    return problems, pages, known, sum(literal_counts)


def main() -> int:
    for p in (DOC, COMPILER_CARGO_TOML, ENVELOPE_RS, VERSIONS_TOML):
        if not p.is_file():
            sys.stderr.write(f"error: {p} not found (run from the repo root)\n")
            return 2

    doc_text = DOC.read_text(encoding="utf-8")
    rs_text = ENVELOPE_RS.read_text(encoding="utf-8")

    m = DOC_VERSIONS_RE.search(doc_text)
    if m is None:
        return fail_shape(
            "the `Versions (as of this doc): `delvec X`, `dsl Y`, `mc Z`` line",
            DOC,
            "DOC_VERSIONS_RE",
        )
    doc_delvec, doc_dsl, doc_mc = m.group(1), m.group(2), m.group(3)

    m = DOC_SUPPORTED_RE.search(doc_text)
    if m is None:
        return fail_shape(
            "the bold ``Supported campaign `dsl_version`: **…**`` list",
            DOC,
            "DOC_SUPPORTED_RE",
        )
    doc_supported = BACKTICKED_RE.findall(m.group(1))

    m = CARGO_VERSION_RE.search(COMPILER_CARGO_TOML.read_text(encoding="utf-8"))
    if m is None:
        return fail_shape("a `version = \"…\"` line", COMPILER_CARGO_TOML,
                          "CARGO_VERSION_RE")
    real_delvec = m.group(1)

    m = RS_SUPPORTED_ONE_RE.search(rs_text)
    if m is None:
        return fail_shape("`pub const SUPPORTED_DSL_VERSION`", ENVELOPE_RS,
                          "RS_SUPPORTED_ONE_RE")
    real_dsl = m.group(1)

    m = RS_SUPPORTED_ALL_RE.search(rs_text)
    if m is None:
        return fail_shape("`pub const SUPPORTED_DSL_VERSIONS`", ENVELOPE_RS,
                          "RS_SUPPORTED_ALL_RE")
    ledger = QUOTED_RE.findall(m.group(1))

    # The ledger minus its reservations — what the build actually accepts, and
    # therefore what every claim below is bound to. A ledger with no reservation
    # list is the ordinary case and leaves this a no-op.
    m = RS_RESERVED_RE.search(rs_text)
    reserved = dict(RESERVED_ROW_RE.findall(m.group(1))) if m else {}
    real_supported = [v for v in ledger if v not in reserved]
    if not real_supported:
        return fail_shape(
            "at least one ACCEPTED `dsl_version` (the ledger is entirely reserved)",
            ENVELOPE_RS,
            "RS_RESERVED_RE",
        )

    m = MC_VERSION_RE.search(VERSIONS_TOML.read_text(encoding="utf-8"))
    if m is None:
        return fail_shape("`[minecraft]` `version = \"…\"`", VERSIONS_TOML,
                          "MC_VERSION_RE")
    real_mc = m.group(1)

    problems: list[str] = []
    for label, claimed, real, source in (
        ("delvec", doc_delvec, real_delvec, "crates/compiler/Cargo.toml [package] version"),
        ("dsl", doc_dsl, real_dsl, "crates/dsl/src/envelope.rs SUPPORTED_DSL_VERSION"),
        ("mc", doc_mc, real_mc, "versions.toml [minecraft] version"),
    ):
        if claimed != real:
            direction = "STALE (the build moved on)" if claimed < real else "AHEAD of the build"
            problems.append(
                f"  `{label} {claimed}` in the doc != `{real}` in the build "
                f"-- {direction}\n"
                f"      source of truth: {source}"
            )

    if doc_supported != real_supported:
        missing = [v for v in real_supported if v not in doc_supported]
        phantom = [v for v in doc_supported if v not in real_supported]
        detail = []
        if missing:
            detail.append(f"accepted by the build but NOT listed: {', '.join(missing)}")
        if phantom:
            detail.append(f"listed but NOT accepted by the build: {', '.join(phantom)}")
        held = [v for v in phantom if v in reserved]
        if held:
            detail.append(
                "of those, RESERVED and therefore refused: "
                + ", ".join(f"{v} (held for {reserved[v]})" for v in held)
                + " -- a reserved number is in the ledger to stop a second change "
                "taking it, not to be offered to an author"
            )
        if not detail:
            detail.append("same members, different ORDER (the list is read in order)")
        problems.append(
            "  the supported `dsl_version` list disagrees with "
            "crates/dsl/src/envelope.rs SUPPORTED_DSL_VERSIONS minus "
            "RESERVED_DSL_VERSIONS\n"
            f"      doc:   {', '.join(doc_supported)}\n"
            f"      build: {', '.join(real_supported)}\n"
            + "".join(f"\n      {d}" for d in detail)
        )

    catalog_rows, detached_rows = mdtable.rows_matching(doc_text, DOC_DW0102_ROW)
    m = next(
        (
            found
            for found in (DOC_DW0102_RE.search(r.line) for r in catalog_rows)
            if found
        ),
        None,
    )
    if detached_rows:
        # Said before `fail_shape`, because `fail_shape`'s remedy is "update the
        # regex" and that is the wrong repair here: the pattern is right and the
        # document is broken. A gate that names a remedy owes the RIGHT one.
        lines = "\n".join(
            f"      {DOC.name}:{lineno}  {line[:88]}" for lineno, line in detached_rows
        )
        print(
            "error: the `DW0102` catalog row is in no table:\n"
            f"{lines}\n"
            "       A blank line above it ends the pipe table, so the page a "
            "reader opens shows a\n"
            "       paragraph of literal pipe characters and the supported set "
            "is stated to nobody.\n"
            "       Delete that blank line. Do NOT loosen DOC_DW0102_RE — the "
            "pattern is correct.",
            file=sys.stderr,
        )
        return 1
    if m is None:
        return fail_shape(
            "the `DW0102` catalog row's ``not in `{…}``` set", DOC, "DOC_DW0102_RE"
        )
    doc_dw0102 = [v.strip() for v in m.group(1).split(",") if v.strip()]
    if doc_dw0102 != real_supported:
        problems.append(
            "  the `DW0102` catalog row restates the supported set and it "
            "disagrees with\n"
            "      crates/dsl/src/envelope.rs SUPPORTED_DSL_VERSIONS minus "
            "RESERVED_DSL_VERSIONS (DW0102\n"
            "      fires on exactly\n"
            "      `!is_supported_version(version)` — crates/dsl/src/validate.rs)\n"
            f"      row:   {{{','.join(doc_dw0102)}}}\n"
            f"      build: {{{','.join(real_supported)}}}"
        )

    # The same constants, on the pages a stranger reads. The file set is derived
    # from the manifests, so this needs no edit when a crate becomes publishable.
    try:
        page_problems, pages, known, n_literals = check_published_pages(
            REPO_ROOT, real_mc, real_supported
        )
    except DerivationError as exc:
        sys.stderr.write(
            f"error: the publishable-crate derivation broke — {exc}\n"
            "       Fix tools/lib/publishable.py or the tree; do NOT let this "
            "gate examine zero pages.\n"
        )
        return 2
    except PageShapeError as exc:
        return fail_shape(exc.what, exc.path, exc.knob)

    if problems:
        sys.stderr.write(
            "docs/reference/compiler.md's version header has drifted from the "
            "build.\n"
            "It is the authoritative current-behavior record and its header is "
            "the first\nthing an authoring session reads, so a stale claim there "
            "is acted on.\n\n" + "\n".join(problems) + "\n\n"
            "Fix the DOC to match the build (or, if the build is what is wrong, "
            "fix the\nbuild) -- do not relax this gate.\n"
        )
    if page_problems:
        sys.stderr.write(
            ("\n" if problems else "")
            + "a crates.io front page states a version this build does not "
            "have.\n"
            "These pages are rendered VERBATIM to a stranger, and the "
            "compatibility facts\nare what decide whether they can use the crate "
            "at all.\n\n" + "\n".join(page_problems) + "\n\n"
            "Fix the PAGE to match the build (or, if the build is what is wrong, "
            "fix the\nbuild) -- do not relax this gate.\n"
        )
    if problems or page_problems:
        return 1

    print(
        "reference version header OK: "
        f"delvec {real_delvec}, dsl {real_dsl}, mc {real_mc}; "
        f"{len(real_supported)} supported dsl_version value(s) "
        f"({', '.join(real_supported)}) matched in order, "
        "and the DW0102 row restates the same set. "
        "Bound by equality in both directions."
    )
    print(
        f"crates.io page versions OK: {len(pages)} published page(s) "
        f"[{', '.join(pages)}] state Minecraft, `dsl_version` window and "
        f"minimum Rust equal to the build, and all "
        f"{n_literals} version literal(s) on them are among the build's "
        f"{len(known)} constant(s). Derived from the manifests, not listed."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
