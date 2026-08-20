# spec-0046: An entry is a role, not a spelling

- **Status**: Proposed
- **Question**: a piece's **entry point** — the cell a body arrives at when it
  enters the area — is identified by matching the anchor's *name* against a list
  of spellings (`plan::ENTRY_ANCHOR_NAMES` = `spawn`, `entry`). Two of the three
  producers of prefab metadata can write those names. The third, the grammar back
  end, structurally cannot: every anchor it exports comes from `Mark::name`, which
  prefixes `anchor/` unconditionally, so a generated zone can never declare an
  entry point at all. Consequence, measured below: a generated zone bound to its
  own area is never transported into, and the crossing is judged as a walk it was
  never meant to be. This spec moves the identification from the **name** to a
  declared **role on the anchor**, which every producer can write and no producer
  has to spell.
- **ADRs**: 0002 (staged DSL — the role is authored where the place is authored),
  0006 (determinism), 0011 (the compiler owns the resolution)
- **Specs**: 0027 (grammar prefab back end — `mark` is the surface that grows),
  0036 §1b/§2.7 (`resolves_to`, the precedent for a compiler-read property on an
  anchor), 0041 §3 ("anchor naming" was deferred there; this is that question)
- **Non-goals**: letting the grammar export a bare, unprefixed anchor name — a
  `mark` must not be able to name an anchor the DSL could not reference, and
  breaking that is a larger loss than the one being fixed. Adding a `spawn` case
  anywhere. Changing what the entry point is *used for*. Any change to how a
  campaign references an ordinary anchor. Campaign content.

## 1. The measured ground

Instrument: `delvec` at engine revision `dcaf68fc`, prefab library at the content
checkout the `campaigns/` symlink resolves to. Every claim was demonstrated on
that tree before this document was written.

- **The grammar cannot spell either name.** `crates/grammar/src/expand.rs:1680`
  is the only site in the crate that inserts an exported anchor, and its key is
  `Mark::name` (`crates/grammar/src/ir.rs:791`), which returns
  `anchor/<stem>` or `anchor/<stem>-<n>` — no branch returns a bare name. The
  invariant is deliberate and documented (`docs/reference/grammar.md:413`): *a
  mark cannot name an anchor the DSL could not reference.*
- **The two hand-written generators can, and disagree with each other.**
  `prefabs/generator`, `prefabs/cave-generator` and `prefabs/tidal-keep-generator`
  write `spawn`; `prefabs/island-generator` writes `entry`. The alias list exists
  only to reconcile those two spellings, and its own doc comment gives the reason:
  the compiler owns the resolution rather than leaving it to per-tileset folklore.
- **The consequence is a red, not a silence.** Renaming `cave-shore`'s entry
  anchor from `spawn` to `entry` — one key, same cell, same facing — used to fail
  `branch-transport` with `DW0311`: *the player cannot walk from `[5, 65, 8]` to
  `[262, 66, 1]`*, a crossing that was never meant to be walked. That instance is
  fixed (both spellings now resolve through one function, and
  `crates/compiler/tests/entry_anchor_aliases.rs` pins it), which is what makes
  the remaining gap visible rather than tangled up with it: **the fix reaches
  every name in the list, and the grammar can write none of them.**
- **Zero documents change meaning under this spec.** No `anchor/entry` or
  `anchor/spawn` key exists anywhere in the prefab library or in any campaign, so
  no existing anchor acquires a role it did not have.

## 2. Why the obvious repairs are refused

Named because each is what a reader reaches for first, and two of them are this
project's own recorded failure shapes.

1. **Add `anchor/spawn` / `anchor/entry` to the alias list.** This is the list
   growing a spelling per producer, which is the defect the list was written to
   end. It also silently reserves a stem: a generated zone whose author names a
   mark `entry` for its own reasons acquires the campaign's start point without
   asking. The role would still live in a string nobody declared.
2. **Give `mark` a boolean, e.g. `spawn: true`.** A second bespoke field keyed to
   one role. The next role that needs identifying — a respawn seat, a boss
   approach, a camera datum — gets a third field, and the first site is where
   generality is decided.
3. **Derive the entry from the spatial contract's `entry` space.** Closer, and
   worth stating why it is not enough: the contract's `entry` names a *space*, a
   volume of many cells. An entry point is one cell with a facing. Deriving one
   from the other is inference, which ADR-0020 §4 refuses for exactly the reason
   it would apply here — the derived cell would be unfalsifiable, and the author
   would have no way to say *this* threshold rather than that one.

## 3. The decision

**The role belongs to the anchor, because an anchor is the object class that
carries "a named place in a piece".** The precedent already exists on the same
struct: `Anchor::resolves_to` is a compiler-read property saying what kind of
place an anchor landed in. A role is its declared sibling.

- `Anchor` (prefab metadata) grows an optional `role: Option<String>`, a
  kebab-case term from a closed vocabulary the compiler owns. This spec opens the
  vocabulary with exactly one term, `entry`, and adding a second is its own
  change.
- `mark` (grammar) grows an optional `role`, written through to the exported
  anchor's metadata. The anchor's *key* is untouched — it stays `anchor/<stem>`,
  so the DSL-referenceability invariant holds.
- The compiler resolves an area's entry point by **role first**, falling back to
  `ENTRY_ANCHOR_NAMES` when no anchor in the area declares one. The fallback is
  the compatibility path for every piece that predates the role, and it is what
  keeps this spec from obliging an adoption round on the shipped library.
- Two anchors in one area declaring `role: entry` is a refusal, not a
  first-wins: an area has one place the party arrives at, and picking silently is
  how a moved spawn becomes a mystery. It needs a DW code and a catalog row.
- The generators adopt the role in the same milestone; when every producer
  declares it, the alias list is deleted and its removal is the adoption round's
  own acceptance criterion. Until then it stays, and it stays *documented as a
  fallback* rather than as the mechanism.

## 4. Acceptance criteria — each stating what would make it vacuous

1. A grammar program whose `mark` declares `role: entry` exports metadata whose
   anchor carries that role, and a campaign binding that zone to an area resolves
   its entry point. *Vacuous if* the fixture's area is the only area, or the two
   consecutive critical objectives sit in the same area — then no crossing exists
   and the resolution is never asked for. The fixture states its area count and
   asserts a crossing is promised.
2. The same campaign built from the same grammar program **without** the role
   fails, and fails naming the missing role. *Vacuous if* it fails for any other
   reason: the test asserts the code, not merely non-zero exit.
3. Every consumer of the entry point resolves a role-declared anchor: transport,
   `setworldspawn`, first-join placement, the class-apply teleport, the POV shot
   planner, the trap-safety start set, the gate-deadlock start node. *Vacuous if*
   asserted per call site — a fourth consumer written later would not be covered.
   Asserted instead over the one resolver every consumer is required to call,
   plus a test that no source file outside it matches an entry-anchor name.
4. Two `role: entry` anchors in one area is the refusal of §3, with a catalog row
   and a test asserting the code. *Vacuous if* the fixture puts them in different
   areas.
5. The shipped library, declaring no roles anywhere, builds byte-identically to
   its pre-spec output. *Vacuous if* measured by re-running the same build twice;
   measured against artifacts produced by the pinned pre-spec engine.

## 5. Order of work

1. Metadata: `Anchor::role`, the closed vocabulary, the duplicate refusal.
2. Compiler: role-first resolution behind the existing single resolver; the
   fallback documented as a fallback.
3. Grammar: `mark` role, written through export; `grammar.md` §2d.
4. Generators adopt the role; the alias list is deleted, and the deletion is the
   acceptance criterion of that step rather than a later tidy.
5. `compiler.md` catalog + §4, `prefab-procedure.md`, in the same PRs.
6. Demo level row queued by this spec's PR in `docs/demo-levels.md`.
