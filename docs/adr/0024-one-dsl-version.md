# ADR-0024: One `dsl_version` — the engine accepts exactly the number it implements

- **Status**: Accepted
- **Date**: 2026-09-05
- **Source**: the constitution's methodology rule "Nothing owes compatibility to
  anything already built. `dsl_version` numbers a surface; it promises nothing";
  measured against the tree at the revision this ADR was written from.

## Context

ADR-0016 §Decision item 1 promised that a campaign document keeps compiling on
every later engine: "versioned per stage, per-stage fences, old formats always
compile." The engine kept that promise with a mechanism in five layers:

- a **ledger** of eighteen accepted numbers (`SUPPORTED_DSL_VERSIONS`), a
  reservation list, a hand-written `*_SINCE` constant per surface, and one
  `is_vNN` predicate per number;
- **per-stage fences** — every surface introduced after 0.2.0 was refused
  (`DW0141`) below the number that introduced it, at over eighty sites in
  `dsl::validate`, `dsl::layout`, `dsl::siteplan`, and the compiler's emitters
  kept a second, byte-identical arm for each document below the number;
- the **obligation fence** — every diagnostic code declared whether it binds at
  every version or only from one (`Binds`), and `Fenced` dropped the findings a
  campaign's declared number excused it from, with a per-key copy of the same
  rule inside the l10n inventory;
- a **harness mirror** of the ledger (`SUPPORTED_DSL_VERSIONS` in
  `harness/src/critical-path.ts`);
- **checkers** holding the layers in step: the harness mirror against the
  ledger, the ledger's numbering against every other branch, and a spec
  (spec-0045) proposing how to classify the next fence.

The cost was measured on the engine's own repository. Every surface change was
written twice, once for the new number and once as the grandfathered arm, and
tested twice. The engine's fixtures declared numbers from `0.2.0` to `0.19.0`
and the gallery was a mosaic of five numbers, so a test proving an emitter
proved the old arm as often as the live one. A campaign's build depended on
eleven per-document numbers, so "what does this campaign compile to" had no
single answer. Two branches allocating the same number for different surfaces
was a live failure that needed its own gate. None of it moved a delve closer to
shipping, and the constitution now rules the promise it served out of scope.

## Decision

1. **The engine accepts exactly one `dsl_version`: the one it implements.**
   `delvewright_dsl::DSL_VERSION` is that number. A stage document, an l10n
   sidecar, or any other envelope declaring a different number is refused at
   the envelope with `DW0102`, which names the one accepted number.
2. **`dsl_version` stays a required field of every envelope.** It numbers the
   surface a document was written against, so an engine can say plainly why it
   refuses a document. It promises nothing about any other engine.
3. **No version predicate exists anywhere in the engine.** No ledger,
   reservation, `*_SINCE` constant, `is_vNN` predicate, per-stage fence,
   grandfathered emission arm, `Binds` classification, obligation fence, per-key
   l10n entry version, or harness allowlist. A `DwCode` carries its id, exit
   tier and subject, and nothing about when it applies: every rule applies to
   every document the engine accepts.
4. **A surface change bumps `DSL_VERSION` and moves every document this
   repository holds — fixtures, gallery, probes — to the new number in the same
   change.** A document that stops compiling is changed or deleted, per the
   constitution; nothing is kept behind an old number.
5. **A released campaign is built by the engine it pins** (`versions.toml`
   `[engine]`, ADR-0010, ADR-0016 items 2 and 3, which stand). A campaign moves
   to a new number by an adoption round that bumps its engine pin and edits its
   documents together; the content repository's skill page moves its version
   line with the pin.
6. **The harness reads an artifact's `version` as provenance**, a non-empty
   string it records and never judges. The artifact format the bot depends on is
   carried by each artifact's own `format_version`, which is unchanged.

## What this supersedes, by section

- **ADR-0016 §Decision item 1** ("`dsl_version` (format) — unchanged: versioned
  per stage, per-stage fences, old formats always compile") is superseded by
  Decision 1–4 above. **ADR-0016 §Decision item 2**'s clause "format
  compatibility is guaranteed by the fences, so an engine may release many
  times within one format era" is superseded: an engine release may still
  happen without a `DSL_VERSION` bump, but because the number did not move,
  not because a fence guarantees anything. The rest of item 2 (the engine's
  own semver line, the `versions.toml` pin), item 3 (the skill's version and
  the engine range it drives), and both Consequences stand.
- **ADR-0015 §Decision, Concurrency rule**, the clause "Mechanical field
  additions that follow an established idiom (e.g. the version-fence pattern)
  may parallelize" is superseded: the idiom no longer exists, and a field
  addition is a surface change under Decision 4. The composition-first rule,
  its two promotion gates, and the rest of the concurrency rule stand.
- **spec-0045** (Proposed, never approved), which proposed how to classify the
  next fence, is withdrawn with the mechanism it refined.
- **ADR-0018 §7**, the grammar `Program` document's own version fence, is a
  different document class with its own ledger and is not touched by this
  decision.

## Consequences

- One number, one build: what a campaign compiles to is a function of its
  documents and the engine, never of eleven per-document numbers.
- Every emitter and every rule has one arm, and every fixture in this
  repository exercises it.
- A finding is never excused by a number. A campaign that needs an exemption
  from a rule needs the rule changed or the document changed.
- Deleted with this decision: `crates/dsl/src/fence.rs`; the ledger, its
  reservations, the `*_SINCE` constants and the `is_vNN` predicates in
  `crates/dsl/src/envelope.rs`; `Binds` and the split `DwCode` constructors in
  `crates/dsl/src/diagnostic.rs`; the `KeyEntry` version in
  `crates/dsl/src/l10n.rs`; every `reserved_vNN` check and every grandfathered
  emission arm; `DW0133`, `DW0141` and `DW0465`, whose only verdict was a
  number; the harness allowlist; `tools/check-harness-dsl-version.py`;
  `tools/check-version-ledger-uniqueness.py`; spec-0045.
- Kept: `tools/check-reference-versions.py`, reduced to binding the one number
  a reader is told to the build, and `tools/check-storybook-version.py`, which
  binds a storybook's engine marker to the campaign's own documents and is
  unaffected by how many numbers an engine accepts.

## Revisit triggers

- A second engine line that must build a campaign at a number it does not
  implement. That is a second engine, pinned by the campaign, not a fence.
