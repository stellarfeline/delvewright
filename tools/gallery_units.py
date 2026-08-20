#!/usr/bin/env python3
"""The surface-unit enumeration and the schema-guided binder (spec-0039 §3).

Two halves live here because they must agree about what a unit IS, and the only
way to guarantee that is to derive both from the same walk of the same document:

- `enumerate_units(schema)` — what the DSL *declares*.
- `bind_document(schema, stage, doc)` — what an authored document *writes*.

Both are driven by `delvec schema --stage all` and **nothing else**. There is no
parser of `stages.rs` here, and there must never be one: the compiler's own
export is the single enumeration authority, the same doctrine that made
`for_each_effect_root` the only list of effect roots in the workspace. A second
enumeration would be a second authority, and the two would disagree exactly when
it mattered — on the surface a change had just added.

## What a unit is

A **surface unit** is one thing a campaign author can write:

- a named schema **property** — `Area.prefab`, `QuestEffect::open-gate.anchor`;
- an enum or tagged-union **variant** — `Horizon::ocean`, `QuestEffect::open-gate`.

Recursively, across every stage. A unit's id is its declaring type plus its own
name, so it is stable under reordering and readable without the schema in hand.

**Registry values are not units** (§3): potion ids, block ids, sound ids are
data, and exhausting them would make the gallery a registry dump. This needs no
filter, and that is a fact about the export rather than a claim: the DSL models
every registry id as a transparent newtype over `string`, so a registry never
reaches the schema as an enum at all. The one large `oneOf` in the export is
`QuestEffect` (36 variants), which is authoring surface in full. Should a future
type inline a registry, `REGISTRY_VALUED` below is where it is excluded, with
its reason — the exclusion is a code change under review, never a config knob.

## Edge semantics, decided here (spec-0039 §9 leaves them to the implementation)

1. **`anyOf` with a `null` branch is optionality, not a union.** schemars renders
   `Option<T>` that way. The `null` branch is not a unit — writing "absent" is
   not writing a surface — and the non-null branch is descended into.
2. **A variant's tag property is the variant, not a property of it.** `type` on
   `QuestEffect::open-gate` carries `{"const": "open-gate"}`; counting it as a
   property as well would double every variant and let a gallery bind a variant
   twice while binding no field of it.
3. **A `$ref` is followed for binding and NOT for enumeration.** Each named type
   is enumerated once, at its own definition. Enumerating through refs would
   multiply `AnchorId.` by every site that references it and make the unit count
   a function of the reference graph rather than of the surface.
4. **A map's value schema is descended into; its keys are not units.** Keys of
   `on_objective_complete` are campaign ids, which is data.
5. **An `allOf` is treated as a conjunction** — every branch contributes.

Each decision surfaces as a unit-count change the baseline header makes visible,
which is what §9 asks of them.
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field

# Types whose `enum`/`oneOf` members are vanilla registry VALUES rather than
# authoring surface (spec-0039 §3). Empty today, and measured rather than
# assumed: the DSL models every registry id as a transparent newtype over
# `string`, so no registry reaches the export as an enum. Kept as the named
# place a future inlined registry is excluded, with its reason beside it.
REGISTRY_VALUED: dict[str, str] = {}


@dataclass(frozen=True)
class Unit:
    """One thing an author can write."""

    id: str
    kind: str  # "property" | "variant"
    owner: str  # the declaring type
    stages: tuple[str, ...]
    doc: str  # first line of the schema description, for the reader-facing index

    def __lt__(self, other: "Unit") -> bool:
        return self.id < other.id


@dataclass
class Binding:
    """Where a unit is written, across the whole binding domain."""

    sites: list[str] = field(default_factory=list)  # "build:/json/pointer"


def _first_line(node: dict) -> str:
    d = node.get("description") or ""
    line = d.strip().split("\n", 1)[0].strip()
    return line


def _null_branch(node: dict) -> bool:
    return node.get("type") == "null"


def _variant_tag(variant: dict) -> tuple[str | None, str | None]:
    """`(tag_property, tag_value)` for an internally-tagged variant.

    A unit-only variant (`{"const": "ocean"}`) returns `(None, "ocean")`; an
    object variant returns the property carrying the `const` and its value.
    """
    if "const" in variant and variant.get("type") == "string":
        return None, str(variant["const"])
    props = variant.get("properties") or {}
    required = variant.get("required") or []
    for name in required:
        p = props.get(name)
        if isinstance(p, dict) and isinstance(p.get("const"), str):
            return name, p["const"]
    # Not required, but still a lone const — accept it rather than silently
    # dropping the variant, and let the caller's fallback name it if absent.
    for name, p in props.items():
        if isinstance(p, dict) and isinstance(p.get("const"), str):
            return name, p["const"]
    return None, None


class Enumerator:
    """Walks a merged schema export and yields every surface unit."""

    def __init__(self, export: dict):
        self.export = export
        self.defs: dict[str, dict] = {}
        self.def_stages: dict[str, set[str]] = {}
        for stage, doc in export.items():
            for name, node in (doc.get("$defs") or {}).items():
                prev = self.defs.get(name)
                if prev is not None and prev != node:
                    raise SystemExit(
                        f"schema export declares `{name}` two different ways "
                        f"(stage `{stage}` disagrees with an earlier stage). The "
                        "unit set is not well defined while that is true."
                    )
                self.defs[name] = node
                self.def_stages.setdefault(name, set()).add(stage)
        self.units: dict[str, Unit] = {}

    def _add(self, uid: str, kind: str, owner: str, stages: set[str], doc: str) -> None:
        prev = self.units.get(uid)
        if prev is not None:
            self.units[uid] = Unit(
                uid, kind, owner, tuple(sorted(set(prev.stages) | stages)), prev.doc or doc
            )
            return
        self.units[uid] = Unit(uid, kind, owner, tuple(sorted(stages)), doc)

    def run(self) -> dict[str, Unit]:
        for name, node in self.defs.items():
            self._type(name, node, self.def_stages[name])
        for stage, doc in self.export.items():
            root = {k: v for k, v in doc.items() if k != "$defs"}
            title = doc.get("title") or f"Envelope<{stage}>"
            self._type(title, root, {stage})
        return self.units

    def _type(self, owner: str, node: dict, stages: set[str]) -> None:
        """Enumerate one named type. Never follows `$ref` (edge semantics 3)."""
        if owner in REGISTRY_VALUED:
            return
        self._properties(owner, "", node, stages)
        for variant in node.get("oneOf") or []:
            self._variant(owner, variant, stages)
        # `anyOf` at type level is a union too when it is not Option-shaped.
        branches = [b for b in (node.get("anyOf") or []) if not _null_branch(b)]
        if len(branches) > 1:
            for variant in branches:
                self._variant(owner, variant, stages)
        for branch in node.get("allOf") or []:
            self._properties(owner, "", branch, stages)

    def _variant(self, owner: str, variant: dict, stages: set[str]) -> None:
        tag_prop, tag_value = _variant_tag(variant)
        if tag_value is None:
            # An untagged branch of a union carries no name of its own; its
            # properties still belong to the owner and are enumerated there.
            self._properties(owner, "", variant, stages)
            return
        uid = f"{owner}::{tag_value}"
        self._add(uid, "variant", owner, stages, _first_line(variant))
        self._properties(uid, tag_prop or "", variant, stages)

    def _properties(self, uid: str, skip: str, node: dict, stages: set[str]) -> None:
        for name, sub in (node.get("properties") or {}).items():
            if name == skip:
                continue  # the tag IS the variant (edge semantics 2)
            self._add(f"{uid}.{name}", "property", uid, stages, _first_line(sub))
            self._inline(uid, name, sub, stages)

    def _inline(self, uid: str, prop: str, sub: dict, stages: set[str]) -> None:
        """Descend an anonymous subschema hanging off a property.

        Anything with a `$ref` is a named type and is enumerated at its own
        definition instead (edge semantics 3).
        """
        if "$ref" in sub:
            return
        inner = f"{uid}.{prop}"
        if sub.get("type") == "array" and isinstance(sub.get("items"), dict):
            self._inline(uid, prop, sub["items"], stages)
            return
        if isinstance(sub.get("additionalProperties"), dict):
            self._inline(uid, prop, sub["additionalProperties"], stages)  # values only
        branches = [b for b in (sub.get("anyOf") or []) if not _null_branch(b)]
        if len(branches) == 1:
            self._inline(uid, prop, branches[0], stages)
            return
        for b in branches:
            self._variant(inner, b, stages)
        for v in sub.get("oneOf") or []:
            self._variant(inner, v, stages)
        if sub.get("properties"):
            self._properties(inner, "", sub, stages)


class Binder:
    """Walks an authored document *guided by the schema* and records bindings.

    Guided, never grepped (§3): a grep cannot tell `"type": "open-gate"` the
    variant from the string `"open-gate"` in a piece of prose, and it cannot see
    a property whose name collides with one on another type. The walk descends
    the JSON and the schema in lockstep, so every binding it records is a place
    the compiler will actually read that unit from.
    """

    def __init__(self, enumerator: Enumerator):
        self.e = enumerator
        self.bound: dict[str, list[str]] = {}
        self._hits = 0

    def _hit(self, uid: str, pointer: str) -> None:
        self.bound.setdefault(uid, []).append(pointer)
        self._hits += 1

    def _resolve(self, node: dict) -> tuple[dict, str | None]:
        """Follow one `$ref`, returning the target and its type name."""
        ref = node.get("$ref")
        if not ref:
            return node, None
        name = ref.rsplit("/", 1)[-1]
        target = self.e.defs.get(name)
        if target is None:
            return node, None
        return target, name

    def walk(self, stage_schema: dict, doc, label: str) -> None:
        root = {k: v for k, v in stage_schema.items() if k != "$defs"}
        title = stage_schema.get("title") or "Envelope"
        self._value(root, title, doc, f"{label}#")

    def _value(self, schema: dict, owner: str, value, ptr: str, hops: int = 0) -> None:
        schema, named = self._resolve(schema)
        if named is not None:
            owner = named
        if isinstance(value, list):
            item = schema.get("items")
            if isinstance(item, dict):
                for i, v in enumerate(value):
                    self._value(item, owner, v, f"{ptr}/{i}")
            return
        if not isinstance(value, dict):
            return
        # A union: pick the branch this value actually satisfies, and bind that
        # branch's variant unit. A value matching no branch is not this walk's
        # problem — `delvec validate` owns that verdict and has already run.
        branches = [b for b in (schema.get("oneOf") or [])]
        branches += [b for b in (schema.get("anyOf") or []) if not _null_branch(b)]
        if branches:
            for branch in branches:
                b, _ = self._resolve(branch)
                tag_prop, tag_value = _variant_tag(b)
                if tag_value is None:
                    continue
                if tag_prop is None:
                    continue  # a bare string const cannot match an object
                if value.get(tag_prop) == tag_value:
                    uid = f"{owner}::{tag_value}"
                    self._hit(uid, ptr)
                    self._object(b, uid, value, ptr, skip=tag_prop)
                    return
            # A lone non-null branch is `Option<T>`, and T may itself be a union:
            # `play-sound.at` is `anyOf: [{$ref: SoundAt}, null]` and `SoundAt` is
            # a `oneOf` of three tagged variants. Descending straight to
            # `_object` here treats that union as a record, finds no
            # `properties`, and binds NOTHING — silently, on a value the campaign
            # really wrote. `hops` bounds the unwrapping so a self-referential
            # single branch cannot loop.
            if len(branches) == 1 and hops < 8:
                self._value(branches[0], owner, value, ptr, hops + 1)
                return
            # An UNTAGGED union — `CastEntry` is "one placement, or a list of
            # them", and neither branch carries a discriminator. Nothing above
            # can match it, so without this the whole `CastPlacement` surface
            # reads as unbound on a campaign whose every quest writes a cast.
            # The value's own JSON type is the discriminator vanilla serde uses
            # here, and it is the one this walk uses too.
            if hops < 8:
                for branch in branches:
                    b, _ = self._resolve(branch)
                    if b.get("properties") or isinstance(
                        b.get("additionalProperties"), dict
                    ):
                        self._value(branch, owner, value, ptr, hops + 1)
                        return
            return
        self._object(schema, owner, value, ptr)

    def _object(self, schema: dict, owner: str, value: dict, ptr: str, skip: str = "") -> None:
        props = schema.get("properties") or {}
        for key, v in value.items():
            if key == skip:
                continue
            sub = props.get(key)
            if sub is None:
                ap = schema.get("additionalProperties")
                if isinstance(ap, dict):
                    self._value(ap, owner, v, f"{ptr}/{_esc(key)}")
                continue
            child = f"{owner}.{key}"
            self._hit(child, f"{ptr}/{_esc(key)}")
            self._scalar_variant(sub, child, v, f"{ptr}/{_esc(key)}")
            # An ANONYMOUS subschema belongs to its site, and the enumeration
            # names its members that way (`TrapEffect.dispense.item`). Passing
            # the parent's own name here instead binds `TrapEffect.item`, which
            # is a unit that does not exist — so the real one reads as unbound
            # while the walk reports a hit. `_value` overrides this the moment it
            # resolves a `$ref`, which is exactly when the type has a name.
            self._value(sub, child, v, f"{ptr}/{_esc(key)}")

    def _scalar_variant(self, schema: dict, uid_base: str, value, ptr: str) -> None:
        """Bind a unit-only variant written as a bare string (`"horizon": "ocean"`).

        The owner name is threaded through every hop: an `Option<Horizon>` is
        `anyOf: [{$ref: Horizon}, {type: null}]`, so the name only appears on the
        inner branch. Losing it there names the unit after its *site*
        (`WorldContent.horizon::ocean`) instead of after its declaring type
        (`Horizon::ocean`), and the enumeration — which walks each type once, at
        its own definition — declares the latter. The two then never meet: the
        variant reads as bound at a name no unit has, and as unbound at the name
        that does. Both halves stay green and the gate proves nothing.
        """
        if isinstance(value, list):
            item = schema.get("items")
            if isinstance(item, dict):
                for i, v in enumerate(value):
                    self._scalar_variant(item, uid_base, v, f"{ptr}/{i}")
            return
        if not isinstance(value, str):
            return
        target, named = self._resolve(schema)
        owner = named or uid_base
        branches = list(target.get("oneOf") or [])
        branches += [b for b in (target.get("anyOf") or []) if not _null_branch(b)]
        for b in branches:
            bb, _ = self._resolve(b)
            tag_prop, tag_value = _variant_tag(bb)
            if tag_prop is None and tag_value == value:
                self._hit(f"{owner}::{value}", ptr)
                return
        # Not a member of this level. A union of unions gets one more look:
        # `CastPlace` is `anyOf: [CastAbsence, AnchorId]`, so `"offstage"` is a
        # member of a NESTED oneOf and stopping here leaves every `CastAbsence`
        # variant unbound on a campaign that writes them. Descending only into
        # branches that are themselves unions keeps this from wandering.
        for b in branches:
            bb, _ = self._resolve(b)
            if bb.get("oneOf") or bb.get("anyOf"):
                before = self._hits
                self._scalar_variant(b, owner, value, ptr)
                if self._hits > before:
                    return


def _esc(key: str) -> str:
    return key.replace("~", "~0").replace("/", "~1")
