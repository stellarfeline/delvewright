#!/usr/bin/env python3
"""The grammar program document cannot grow a field that rides through in silence.

A `Program` (`crates/grammar/src/ir.rs`) is a long-lived on-disk document
(ADR-0018 §4/§5). Two shapes of new surface, and they are not equally safe:

- **A new variant of a tagged enum is safe.** `Node`, `Cond`, `Expr`, `Size` and
  `MarkAt` are internally tagged, so an engine that predates the variant meets an
  `"op"` / `"cond"` / … it does not know and fails loud at `serde`; and every
  exhaustive `match` in the crate forces an arm for it at compile time.
- **A `#[serde(default)]` struct field is not.** It rides through every walk
  untouched in BOTH directions. An engine that predates the field deserialises
  the document with the field's default, expands, gates green, and writes
  different geometry with nothing at all to say about it. This is exactly what
  `reorient.mirror` would have done: a document with `"mirror": {"x": true}`
  handed to a pre-mirror engine builds the unreflected shape and passes every
  gate.

ADR-0018 §7 answers it with two mechanisms, and this script is what binds them
to the source rather than to a doc line:

1. **Closed schemas** (§7.3). Every deserialisable object type in the IR carries
   `#[serde(deny_unknown_fields)]`, so an engine meeting a document from a newer
   engine refuses it by name instead of dropping what it does not know. This is
   the mechanical half: it needs nobody to remember anything.
2. **A version ledger** (§7.1/§7.4). Every optional field is listed in
   `docs/reference/grammar.md` §2e with the document version that introduced it,
   checked in BOTH directions — a field with no row is a red, and a row with no
   field is a red. Anything above the floor must name a `*_SINCE` constant that
   `version.rs` declares AND that `ir.rs` actually refuses on, so "fenced" cannot
   mean "a constant exists".

Both gates print their binding count. A zero binding is a FAILURE, not a pass: a
sweep that matched no types has stopped measuring the thing it was written for.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
IR = ROOT / "crates/grammar/src/ir.rs"
GEOM = ROOT / "crates/grammar/src/geom.rs"
VERSION_RS = ROOT / "crates/grammar/src/version.rs"
REFERENCE = ROOT / "docs/reference/grammar.md"

SOURCES = {"ir": IR, "geom": GEOM}

# The document version every field that has always existed is attributed to.
FLOOR = "1.0.0"

# Types that cannot carry `deny_unknown_fields`, each with the reason and each
# demonstrated rather than assumed. This table may only SHRINK: an entry that has
# since gained the attribute is a failure, so a fixed exemption cannot rot into a
# permanent one.
EXEMPT: dict[str, str] = {
    "ir::Mark": (
        "`at` is #[serde(flatten)], which serde cannot combine with "
        "deny_unknown_fields — the attribute compiles and then reads every "
        "flattened key as unknown, refusing every well-formed mark. Held by the "
        "§2e ledger instead."
    ),
    "ir::Edge": (
        "`class` is #[serde(flatten)], the same incompatibility as `ir::Mark`: "
        "with the attribute the engine refuses its own edges by name "
        "(`unknown field \'class\'`). Held by the §2e ledger instead."
    ),
}

TYPE_RE = re.compile(
    r"#\[derive\(([^)]*)\)\]\n((?:\s*#\[[^\n]*\]\n)*)\s*(?:pub )?(struct|enum) (\w+)"
)


def object_types() -> list[tuple[str, str, bool, bool, bool]]:
    """Every deserialisable type that reads a JSON **object with named fields**.

    Returned as `(qualified_name, kind, has_deny, is_untagged, has_flatten)`.
    Unit-only enums (`Axis`, `Facing`, …) are excluded because they deserialise
    from a string and an unknown one is already a serde error — they have no
    field to hide. Untagged enums are reported but not required to carry the
    attribute: serde cannot apply it there, and their variants are covered
    through the struct types they hold.

    `has_flatten` is why this returns a fact rather than a verdict. A type with a
    `#[serde(flatten)]` field CANNOT carry `deny_unknown_fields`: the attribute
    compiles and then reads every flattened key as unknown, so the engine refuses
    its own documents. Telling an author to add it there is advice that breaks the
    build in a way `cargo build` cannot see — which is how `ir::Edge` got it
    during the #413+#417 integration and shipped an engine that could not parse an
    edge until one CLI round-trip test caught it.
    """
    out: list[tuple[str, str, bool, bool, bool]] = []
    for module, path in SOURCES.items():
        src = path.read_text()
        for m in TYPE_RE.finditer(src):
            derives, attrs, kind, name = m.groups()
            if "Deserialize" not in derives:
                continue
            body = src[src.index("{", m.end()) + 1 : src.index("\n}\n", m.end())]
            if kind == "enum" and not re.search(r"^\s+[A-Z]\w*\s*[({]", body, re.M):
                continue  # a unit-only enum: a string, closed by construction
            out.append(
                (
                    f"{module}::{name}",
                    kind,
                    "deny_unknown_fields" in attrs,
                    "untagged" in attrs,
                    "serde(flatten)" in body,
                )
            )
    return out


FIELD_RE = re.compile(
    r"#\[serde\(([^)\]]*default[^)\]]*)\)\]\n((?:\s*#\[[^\n]*\]\n)*)\s*(?:pub )?(\w+):"
)


def optional_fields() -> list[tuple[str, str]]:
    """Every `#[serde(default…)]` field, as `(owning type, field name)`.

    The owner is the nearest preceding type header, which is how the source reads
    and is stable under reordering.
    """
    out: list[tuple[str, str]] = []
    for module, path in SOURCES.items():
        src = path.read_text()
        headers = [(m.start(), m.group(4)) for m in TYPE_RE.finditer(src)]
        for m in FIELD_RE.finditer(src):
            owner = None
            for pos, name in headers:
                if pos < m.start():
                    owner = name
                else:
                    break
            if owner is None:
                sys.exit(f"FAIL: optional field {m.group(3)!r} in {path} has no owning type")
            out.append((f"{module}::{owner}", m.group(3)))
    return out


LEDGER_ROW = re.compile(
    r"^\|\s*`([\w:]+)\.(\w+)`\s*\|\s*`([\d.]+)`\s*\|\s*([^|]*?)\s*\|", re.M
)


def ledger() -> dict[tuple[str, str], tuple[str, str]]:
    """The §2e table of `docs/reference/grammar.md`.

    `(type, field) -> (version, fence)`, where *fence* is `—` at the floor, a
    `*_SINCE` constant name, or `via Type.field` for a field only reachable
    through another ledgered one.
    """
    text = REFERENCE.read_text()
    start = text.find("## 2e.")
    if start < 0:
        sys.exit("FAIL: docs/reference/grammar.md has no `## 2e.` section to hold the ledger")
    end = text.find("\n## ", start + 1)
    section = text[start : end if end > 0 else len(text)]
    return {
        (t, f): (v, fence.strip("` "))
        for t, f, v, fence in LEDGER_ROW.findall(section)
    }


def main() -> int:
    failures: list[str] = []

    # --- Gate 1: closed schemas. ------------------------------------------
    types = object_types()
    checked = [t for t in types if not t[3]]
    for name, _kind, has_deny, _untagged, has_flatten in checked:
        if name in EXEMPT and has_deny:
            failures.append(
                f"{name} is listed in EXEMPT but now carries deny_unknown_fields — "
                f"drop the exemption (this table may only shrink)"
            )
        elif name not in EXEMPT and not has_deny and has_flatten:
            # Named as the incompatibility it is, never as a missing attribute:
            # adding one here compiles and then refuses every well-formed
            # document, which no build or clippy run can see.
            failures.append(
                f"{name} has a #[serde(flatten)] field, which serde CANNOT combine with "
                f"deny_unknown_fields — the attribute would compile and then read every "
                f"flattened key as unknown. Do not add it. Add an EXEMPT entry in "
                f"tools/check-grammar-ir-compat.py naming the flattened field, and hold "
                f"the type through the §2e ledger instead."
            )
        elif name not in EXEMPT and not has_deny:
            failures.append(
                f"{name} is a document object type without #[serde(deny_unknown_fields)]: "
                f"an engine that predates a field added to it would DROP that field "
                f"silently. Add the attribute, or add an EXEMPT entry in "
                f"tools/check-grammar-ir-compat.py saying why serde cannot."
            )
    flattening = {t[0] for t in checked if t[4]}
    for name in EXEMPT:
        if name not in {t[0] for t in checked}:
            failures.append(f"EXEMPT names {name}, which is not a document object type any more")
        elif name not in flattening:
            # The stated reason, held to the source. Every exemption so far is
            # `flatten`; one that is not has to say so here rather than inherit a
            # reason that stopped being true.
            failures.append(
                f"EXEMPT names {name}, which no longer has a #[serde(flatten)] field — the "
                f"reason the exemption records is not the reason any more. Close it, or "
                f"replace the entry with the incompatibility that is now true."
            )
    print(f"closed-schema   bound {len(checked):3}  object type(s) examined, "
          f"{len(checked) - len(EXEMPT)} closed, {len(EXEMPT)} exempt")
    if not checked:
        failures.append("closed-schema examined ZERO types — the sweep binds to nothing")

    # --- Gate 2: the version ledger, both directions. ---------------------
    fields = optional_fields()
    rows = ledger()
    for key in fields:
        if key not in rows:
            failures.append(
                f"{key[0]}.{key[1]} is #[serde(default)] with no row in grammar.md §2e. "
                f"Every optional field states the document version that introduced it, "
                f"because an engine older than that version builds a different world from "
                f"the same document."
            )
    for key in rows:
        if key not in fields:
            failures.append(
                f"grammar.md §2e has a row for {key[0]}.{key[1]}, which is not a "
                f"#[serde(default)] field of that type any more — stale ledger"
            )

    version_src = VERSION_RS.read_text()
    ir_src = IR.read_text()
    since_consts = dict(
        re.findall(r'pub const (\w+_SINCE): &str = "([\d.]+)";', version_src)
    )
    # Every `FencedConstruct` in ir.rs, as (guard source, since-constant). The
    # guard is the enclosing `if`: what the refusal actually LOOKS AT. Matching
    # only the constant would let any field claim the fence of any other field at
    # the same version, which is a green that binds to nothing.
    refusals = [
        (m.group(1), m.group(2))
        for m in re.finditer(
            r"if ([^\n]*?)\s*\{\s*return Err\(ProgramError::FencedConstruct\s*\{"
            r"[^}]*?since:\s*(\w+)",
            ir_src,
            re.S,
        )
    ]
    fenced = 0
    for (owner, field), (declared, fence) in sorted(rows.items()):
        where = f"{owner}.{field}"
        if declared == FLOOR:
            if fence != "—":
                failures.append(
                    f"{where} is at the {FLOOR} floor but names a fence {fence!r}; "
                    f"the floor needs none and the column must read `—`"
                )
            continue
        if fence.startswith("via "):
            target = fence[4:].strip()
            key = tuple(target.rsplit(".", 1))
            if key not in rows:
                failures.append(
                    f"{where} inherits its fence from {target}, which has no ledger row"
                )
            elif rows[key][0] != declared:
                failures.append(
                    f"{where} is at {declared} but inherits from {target}, which is at "
                    f"{rows[key][0]} — an inherited fence must be the same version"
                )
            elif rows[key][1] == "—":
                failures.append(f"{where} inherits its fence from {target}, which is unfenced")
            else:
                fenced += 1
            continue
        if fence not in since_consts:
            failures.append(
                f"{where} is at {declared}, above the {FLOOR} floor, and names fence "
                f"{fence!r}, which version.rs does not declare as a `*_SINCE` constant"
            )
            continue
        if since_consts[fence] != declared:
            failures.append(
                f"{where} says {declared} but {fence} is {since_consts[fence]}"
            )
            continue
        have = sum(
            1
            for guard, const in refusals
            if const == fence and re.search(rf"\b{re.escape(field)}\b", guard)
        )
        # Counted, not merely found. A field name is not unique across types —
        # `mirror` is a field of BOTH `Reorient` and `Cond::Orientation` — so
        # "some refusal reads `mirror`" would stay green after one of the two was
        # deleted. One row of this ledger owes one refusal.
        want = sum(
            1
            for (o2, f2), (_v2, fen2) in rows.items()
            if f2 == field and fen2 == fence and not fen2.startswith("via ")
        )
        if have < want:
            failures.append(
                f"{where} names fence {fence}: {want} ledger row(s) claim it and ir.rs has "
                f"only {have} ProgramError::FencedConstruct refusal(s) under a guard that "
                f"READS `{field}`. A constant is not a fence, and one refusal cannot stand "
                f"for two fields."
            )
            continue
        fenced += 1
    print(f"version-ledger  bound {len(fields):3}  optional field(s) examined, "
          f"{len(rows)} row(s), {fenced} above the {FLOOR} floor with a fence ir.rs "
          f"refuses on")
    if not fields:
        failures.append("version-ledger examined ZERO fields — the sweep binds to nothing")
    if not refusals:
        failures.append(
            "found ZERO ProgramError::FencedConstruct refusals in ir.rs — the fence half "
            "of this check binds to nothing"
        )

    if failures:
        print("\nFAIL:")
        for f in failures:
            print(f"  - {f}")
        return 1
    print("\nOK: the grammar IR is a closed schema and every optional field is ledgered.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
