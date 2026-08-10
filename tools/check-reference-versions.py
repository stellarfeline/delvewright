#!/usr/bin/env python3
"""`docs/reference/compiler.md` version-header gate (bidirectional).

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
the versions, and `tools/check-skill-version.py` binds only the *skill's*
`verified_with` to the engine, never the reference doc's.

## What is bound, and to what

Every claim below is bound by EQUALITY, not by "at least" — a claim may be
neither stale-older nor prematurely-newer:

- `delvec <X>`  == `crates/compiler/Cargo.toml` `[package] version`
  (the same constant `tools/check-skill-version.py` calls the engine version)
- `dsl <Y>`     == `crates/dsl/src/envelope.rs` `SUPPORTED_DSL_VERSION`
- `mc <Z>`      == `versions.toml` `[minecraft] version`
- the bold supported-`dsl_version` list == `crates/dsl/src/envelope.rs`
  `SUPPORTED_DSL_VERSIONS`, as an ORDERED sequence
- the `DW0102` catalog row's `{…}` set == the same `SUPPORTED_DSL_VERSIONS`,
  because `DW0102` fires on exactly `!is_supported_version(version)`
  (`crates/dsl/src/validate.rs`) and its row restates that set by hand

That last one is a second instance of the same defect, found while fixing the
first: the row read `{0.2.0 … 0.8.0}` with `0.9.0` accepted and tested.
`tools/check-dw-codes.py` was green on it and always would be — that gate proves
a code EXISTS in both source and doc and is asserted by a test, never that the
BEHAVIOR the doc ascribes to it is the behavior the code has. A code's prose is
otherwise unbound, and this is the mechanically checkable slice of it.

Equality in both directions is the point. A gate that only rejected a version
NEWER than the build is exactly the shape that let a storybook ship a stale
`v1.0` marker through the whole `v1.1` release with a green check (engine #342):
the stale-older direction is the one that actually happens, because docs are
written once and the build moves.

The ordered-sequence comparison matters too: the list doubles as the reading
order for the "additive superset" claim beside it, and a set comparison would
pass on a shuffled list.

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

# `[minecraft]` ... `version = "1.21.11"`
MC_VERSION_RE = re.compile(r'(?ms)^\[minecraft\]\s*$.*?^version\s*=\s*"([^"]+)"')

# The DW0102 catalog row restates the same set by hand:
#   | `DW0102` | Unsupported `dsl_version` (not in `{0.2.0,…,0.9.0}`). |
DOC_DW0102_RE = re.compile(r"\|\s*`DW0102`\s*\|[^|]*?not in `\{([^}]*)\}`")

QUOTED_RE = re.compile(r'"([^"]+)"')
BACKTICKED_RE = re.compile(r"`([^`]+)`")


def fail_shape(what: str, path: pathlib.Path, knob: str) -> int:
    sys.stderr.write(
        f"error: could not find {what} in {path.relative_to(REPO_ROOT)} — the\n"
        f"       source was renamed or reshaped. Update {knob} in\n"
        f"       tools/check-reference-versions.py. Do NOT loosen the check.\n"
    )
    return 2


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
    real_supported = QUOTED_RE.findall(m.group(1))

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
        if not detail:
            detail.append("same members, different ORDER (the list is read in order)")
        problems.append(
            "  the supported `dsl_version` list disagrees with "
            "crates/dsl/src/envelope.rs SUPPORTED_DSL_VERSIONS\n"
            f"      doc:   {', '.join(doc_supported)}\n"
            f"      build: {', '.join(real_supported)}\n"
            + "".join(f"\n      {d}" for d in detail)
        )

    m = DOC_DW0102_RE.search(doc_text)
    if m is None:
        return fail_shape(
            "the `DW0102` catalog row's ``not in `{…}``` set", DOC, "DOC_DW0102_RE"
        )
    doc_dw0102 = [v.strip() for v in m.group(1).split(",") if v.strip()]
    if doc_dw0102 != real_supported:
        problems.append(
            "  the `DW0102` catalog row restates the supported set and it "
            "disagrees with\n"
            "      crates/dsl/src/envelope.rs SUPPORTED_DSL_VERSIONS (DW0102 "
            "fires on exactly\n"
            "      `!is_supported_version(version)` — crates/dsl/src/validate.rs)\n"
            f"      row:   {{{','.join(doc_dw0102)}}}\n"
            f"      build: {{{','.join(real_supported)}}}"
        )

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
        return 1

    print(
        "reference version header OK: "
        f"delvec {real_delvec}, dsl {real_dsl}, mc {real_mc}; "
        f"{len(real_supported)} supported dsl_version value(s) "
        f"({', '.join(real_supported)}) matched in order, "
        "and the DW0102 row restates the same set. "
        "Bound by equality in both directions."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
