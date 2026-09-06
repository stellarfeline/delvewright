# spec-0043: An open space carries its shadow

- **Status**: Proposed
- **Question**: spec-0041 §6 records the one refusal it left standing: Z3's
  flooded ward floor holds 266 standable cells under decks and tower that can
  be neither `open` (own blocks overhead) nor `enclosed` (boundaries run into
  open basins). This round re-measured the whole neighbourhood of that record
  on the current engine before designing anything. The refusal reproduces to
  the cell — and the record is wrong twice, in ways that make the gap worse
  than it says: the covered water **is** declarable today, once as `enclosed`
  strips hand-cut along a contour no author owns, and once as a `no_body`
  region that computes `facade`, greens, and silently removes every one of
  those walkable cells from the reachability proof. The missing thing is not
  an envelope kind. It is the **binding** of the sky demand: the fact it
  demands is per-cell and computed, and the license it secures is per-space
  and declared. This spec moves the demand to the checker-computed partition
  and adds no surface at all.
- **ADRs**: 0020 (the contract is the checker), 0006 (determinism), 0018 §7
  (why no fence rides this — no document surface changes)
- **Specs**: 0036 (extended — §0 and every excuse verbatim; only §2.3's sky
  demand is re-bound), 0041 (independent; its §6 finding is what this
  answers), 0042 (untouched)
- **Non-goals**: any new envelope kind, field, or document surface; any
  threshold (an aperture fraction is a craft rule, not a proof); the `facade`
  kind's own demand (§3, named residual); anchors, `no_body`, edge classes,
  reachability, coverage — all verbatim; any campaign content.

## 1. The measured ground

Every claim below was demonstrated on the current binary. The instrument pair
for the headline number: the checker itself, and a standalone NBT reader over
the shipped tiles sharing nothing with the expander but the bytes on disk.
The proving geometry is the committed Z3 program (bell-r2, 40x10x60, seed 1,
no contract) plus measurement claims at three scopes — the lit water
(`open_water`'s air band), the water under the arcade decks (`arcade_bay`'s
arch band), the water under the tower oversail (`plinth_water`'s air band) —
and a distilled fixture: a stone yard, seven cells of it under sky and a
five-cell strip under an awning open along its whole flank.

1. **The recorded refusal reproduces to the cell.** The honest declaration —
   the ward's water as ONE space, envelope `open` — reds closure: *"266 of
   its standable cell(s) have this piece's own blocks overhead — a roofed
   room cannot be downgraded out of closure."* The independent reader agrees:
   2695 standable cells, 1864 of them footed on water, 266 of those with own
   solid overhead. The same gate simultaneously reports **binding 0** — an
   all-`open` contract gives closure nothing to examine, so the breach is
   announced by a gate that bound to nothing.
2. **`enclosed` refuses the same truth the other way** (1372 boundary cells
   of open sky), and `open_top` refuses identically to `open`. A partly
   covered space has no single envelope: the sky demand refuses the cover,
   the closure demand refuses the sky.
3. **The split is expressible and green — at box granularity.** On the
   distilled fixture, basin `open` + covered strip `enclosed` + a walk edge
   passes every gate: the strip's open flank is excused as an abutting
   declared space, which is existing doctrine. So the record's "can be
   neither" holds only for one space. What the split cannot reach is the
   **contour**: lit-versus-covered is computed from the whole model — any
   rule's mass casts shadow (the crossing ramp, the arch crowns, the tower) —
   while a claim's box comes from one rule's scope. On the real Z3 the honest
   box-level split still leaves 126 boundary cells unexcused, each wanting
   another hand-cut box that re-derives some other rule's geometry, and every
   parameter change re-cuts the seam. Review shape 3: the general mechanism
   exists and its binding cannot reach the object. The vocabulary also
   inverts: the openest corner of the map becomes declarable only as
   `enclosed`.
4. **The computed out-of-walk kinds do not all refuse — one accepts, and its
   green is the worst spelling on offer.** The covered water declared
   `no_body` computes **`facade`** and passes, on the fixture and on Z3
   (288 standable cells, "every one reached by the air outside the piece"),
   because at-grade openness supplies exterior air for free — the kind built
   for cornices swallows walkable floor. The cost is silent: those cells
   leave the reachability targets (1828 → 1540 on Z3). The record's "nor any
   computed out-of-walk kind" is false, and what is true is worse: the only
   green spelling of the partly-roofed space that needs no contour arithmetic
   is a lie that removes play floor from every walk proof.
5. **Nothing else is implicated.** The water surface is one level (the tide
   plane), so the one-floor rule holds; under the single-space declaration
   reachability walks all 1828 claimed cells, covered ones included. The gap
   is the sky demand's binding and nothing beside it.

## 2. The widening — the demand moves to the computed partition

For every `open` and `open_top` space the checker partitions the space's
cells by a fact it already knows how to read: a cell with any of the piece's
own solid above it in its column (within the region) is **shadow**; the rest
are **lit**. Computed, never declared — spec-0036 §0's corollary, the author
picks nothing and there is no field to pick with.

Two demands replace the per-space sky rule:

- **The lit volume must hold at least one standable cell.** An all-shadow
  `open` or `open_top` space is refused exactly as today: a roofed room
  cannot be downgraded out of closure.
- **The shadow volume's boundary is examined as an `enclosed` space's
  boundary is** — every boundary cell non-passable except the three existing
  excuses (a declared opening's `via`, an abutting declared space, an
  abutting out-of-walk region). Cells of the same space, lit or shadow, are
  interior — which is what makes the overhang's mouth legal with no new
  excuse kind: the mouth opens into the space's own air.

`open_top` keeps its side-face examination over the whole space; the widening
replaces only its sheltered-cell refusal. `enclosed` is untouched.

The verdict enumerates every shadow-bearing open envelope — lit count, shadow
count, shadow boundary cells examined — per instance, so a reviewer sees each
one. Closure's binding count includes the shadow boundary, which also retires
§1.1's oddity of a breach reported by a gate that bound to nothing.

Under this, Z3's water is one space with one name and one word: `open`. The
checker finds the 266, examines their boundary (deck soffits, piers, the
water below, the space's own lit air at every mouth), and the declaration
says what the place is.

## 3. What is deliberately not changed, and the one residual

- **The split stays legal.** A contract that declares the covered strip as
  its own `enclosed` space keeps its green (AC5) — granularity remains the
  author's choice; it just stops being forced.
- **`facade` keeps its demand.** Its acceptance of walkable at-grade floor
  (§1.4) is a spec-0036 §2.6 residual this spec names and measures rather
  than fixes: no positive fact cleanly separates a cornice from a walkable
  court strip, and any candidate (walk-connection, height) is entailed by
  one defect or another. What this spec does change is the economics: the
  honest spelling becomes one word, so nothing pushes an author toward the
  lie. The residual is pinned by measurement (AC6) so it is tracked, not
  remembered.
- **The lit boundary stays unexamined**, exactly as every fully-open space's
  whole boundary is today — openness to the world is what `open` declares,
  and a piece's own outer face belongs to the face contract. Stated here so
  the next round checks it rather than discovers it: the widening examines
  strictly more boundary than today on every declaration it newly admits, and
  exactly the same set on every declaration green today.

## 4. Why this opt-out is not the sixth vacuity mode

Tolerating a covered cell inside an open space is an opt-out from "sky over
every cell", so it owes §0's question: what does it demand, and could the
defect supply it? It demands two facts. First, own-solid overhead — which is
the defect's own signature, and here it buys nothing but **more
examination**: solid overhead is what switches the cell's boundary
obligation ON. Second, a fully accounted shadow boundary — the same demand
`enclosed` makes, which stranding and breach cannot supply: a hidden room's
hole is a passable boundary cell with no claimed excuse, red by name. The
defect the old rule caught — the roofed room downgraded out of closure — is
refused verbatim (no lit cell), and the adversarial forms that were
impossible before (they could never be declared) arrive with a proof
obligation they did not have. No acknowledgement string exists anywhere in
this surface, and no second hatch is added to the gate it re-binds.

## 5. No version fence, and what holds instead

ADR-0018 §7 fences ride document surfaces; this spec adds none — no node, no
field, no envelope keyword. `crates/grammar/src/version.rs` (ledger through
`1.6.0` reserved for spec-0041) and both ledgers of
`tools/check-version-ledger-uniqueness.py` are untouched; this spec claims no
number in either. What holds in a fence's place is the compatibility
obligation itself, as an acceptance criterion: a space green today under
`open`/`open_top` has no sheltered standable cell, so the lit-empty arm
cannot fire and the shadow the new examination binds is empty or breachless
on every green contract in the corpus and both campaign trees — asserted by
re-verdict, not argued (AC4). A divergence found there is a finding about a
claimed box swallowing air nothing accounts for, and it stops the PR rather
than being absorbed.

## 6. Acceptance criteria — each stating what would make it vacuous

1. **The partly-roofed pair, distilled from Z3.** The awning fixture as one
   `open` space: red today naming its 30 sheltered cells (kept as the red
   fixture); green under the widening, with the enumeration naming lit and
   shadow standable counts (56 / 30) and closure's binding including the
   shadow boundary. *Vacuous if* the shadow count is zero, or the green
   verdict's closure binding excludes the shadow boundary — a green by
   non-examination is §1.1's vacuity wearing the fix's clothes.
2. **The roofed room stays dead, both envelopes.** A fully covered space
   declared `open`, and the same declared `open_top`, each red naming the
   empty lit volume, message intent unchanged. *Vacuous if* either fixture
   holds a lit standable cell — the red would then never exercise the
   lit-empty arm.
3. **Shadow teeth, both signs.** The awning fixture with one support wall
   breached into in-region unclaimed air: red naming the breach cell on the
   shadow boundary. The same bytes with that air claimed by an abutting
   space: green. *Vacuous if* the red twin's breach cell is excused by any
   abutting claim — the tooth never bit.
4. **Compatibility is re-verdicted, not asserted.** Every corpus program and
   existing contract fixture, and every committed contract-bearing program in
   both campaign trees, yields a byte-identical verdict; the double-expand
   suite extends over a shadow-bearing green. *Vacuous if* the population
   examined contains no `open` and no `open_top` envelope — assert at least
   one of each bound.
5. **The split stays legal.** The basin-`open` + strip-`enclosed` + walk-edge
   fixture is green before and after, verdict-identical. *Vacuous if* the
   strip holds no sheltered cell — the fixture would no longer witness the
   granularity it protects.
6. **The residual is pinned.** The covered strip declared `no_body` still
   computes `facade` and still shrinks the reachability binding by exactly
   its standable count — measured and asserted, so §3's residual exists as a
   fixture, not a memory. *Vacuous if* the region holds no standable cell
   (the kind gate would red on emptiness instead of computing anything).
7. **One checker, two doors.** `delvec prefab audit` agrees with `expand` on a
   piece whose contract carries a shadow-bearing `open` space — same bytes,
   same resolved contract, same verdict and enumeration. *Vacuous if* the
   admit-side fixture's contract has no shadow.
8. **Diagnostics keep named-gate form** and every new refusal arm (lit-empty,
   shadow-boundary breach) is test-asserted per the DW-coverage convention;
   this spec claims no code numbers in prose. *Vacuous if* an arm ships
   allowlisted instead of asserted.

## 7. Order of work

1. Checker: the partition, the two demands, the enumeration lines, the
   binding count; the distilled fixtures of §6 in the same PR.
2. `delvec prefab` door parity (no metadata change — the contract block is
   already carried whole).
3. Docs in the same PR: `grammar.md` §2d's closure row and envelope prose.
4. Bell adoption, scheduled with this milestone per version-adoption
   discipline even though no version moves: Z3 declares its water as one
   `open` space — the shadow's standable count at the pinned region and seed
   is 266, the number this round measured twice — inside the zone's full
   contract, which also owes the remaining coverage work (decks, causeway,
   tower interior) and spec-0041's own adoption items. The round summary
   names both.
