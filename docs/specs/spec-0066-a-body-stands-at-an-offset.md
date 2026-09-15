# spec-0066: A body stands at an offset

- **Status**: Accepted
- **Ground**: written against engine `495fca44` (`origin/main`), read only
  (its difference from `93c9802e` is two CI files; no compiler line moved), and
  against the released content revision `73182027`,
  `campaigns/doune-castle-tour/quests.json` (sha-256
  `c86c22a25aeab486d759f8e45a5ae3927ccc39ffec942ce27a11471ace69d99f`) and
  `prefabs/doune-castle.json` (sha-256
  `4857f90c69912802ea62c0abdddf1dcafa453c3eb75e12795a393a52c70069f9`),
  fetched at that revision. Every count below is a value read from those
  documents or a line read in the tree; nothing was measured by building. The
  shape of this document is spec-0062's.
- **What it is for**: a scene that stands several bodies in a rank — a guard
  drawn up under a gate, a choir, a row of mourners — declares one mark and
  the places around it, instead of spending a prefab anchor per body; and the
  destination a body walks to takes the same offset, so the rank can be
  *formed* by walking, not only spawned in formation.
- **Research**: no craft question is open; the question is which object class
  carries a position. The record consulted is the engine's own: `DW0896`
  (*a mark is one cell, and a cell holds one body*, `compiler::cohabit`),
  `DW0461`'s place arm and `plan::body_station` / `Plan::body_point` (the one
  resolution of where a body is), `body_sites` (the one walk of the campaign's
  bodies), the three camera types that already carry `{anchor, offset}`
  (`CameraWaypoint`, `AnchorSubject`, `CameraTarget`) and the ledger entry in
  `tools/check-capability-ownership.py` that accepts them as three types on the
  ground that *nothing wants a generic anchor+offset*, spec-0062 §9 (the
  refusal of an offset on a killing volume's region) and spec-0055 (an anchor
  reference resolves where its object stands). Every rule below is marked
  **cited** or **authored**.
- **Numbers**: no spec or ADR beyond this one. **One new DW code**, `DW0897`
  (§5.3). **`dsl_version` moves** (the minor): bodies, destinations, the cast ledger and a
  sound's point gain a field; two field names change.
- **Non-goals**: an offset on a killing volume's region or on any block-
  addressed anchor — a gate, a prop cell, a trap, a container, a trigger's
  `at`, a volley's slot, a wave's forming point (§7); a formation vocabulary
  (`rank: 6, spacing: 2`) — that is arithmetic the creator does, and a
  primitive encodes a mechanism, never a design decision; sub-block positions;
  facing derived from the offset; any change to how a camera aims at a body.

## 1. The defect, and what the released campaign spent on it

**Finding, from reading the tree and the content.**

A stage-2 `Npc` and a stage-5 `Actor` each declare `anchor: AnchorId` and
nothing else about where they are (`crates/dsl/src/stages.rs`); writing
`offset` on an actor is refused at `DW0100`, the schema naming the twelve
fields an actor does take. `Plan::body_point` resolves a body to the anchor's
cell and the summon is written at that cell's centre. The field `offset:
[i32; 3]` exists in the DSL on exactly three types, all of them camera
geometry — a dolly waypoint, an aim target and an anchor subject — where it
was added because *framing* is the campaign's decision and no piece can
anticipate it.

The released castle tour draws up a captain and six men-at-arms. To do it the
piece document declares seven spawn anchors (`anchor/muster-gate-captain`,
`anchor/muster-gate-1` … `-6`, at zone-local `[77, 8, 45]` down to
`[77, 8, 35]`, two cells apart), and the campaign's `move-actor`s walk the
seven bodies to seven more (`anchor/muster-rank-captain`,
`anchor/muster-rank-1` … `-6`) by way of `anchor/muster-form-captain`:
fifteen named places in the piece for one formation, of the piece's 31
anchors. Before those anchors were cut, the same seven bodies stood on one
anchor, and `DW0896` now refuses that — its prescription reads *give each body
its own anchor — one anchor is one cell, so a rank of bodies needs a mark
apiece*. The prescription is right about the cell and wrong about the remedy
it can name: the only remedy the surface offers is a prefab edit, which is
not the campaign's to make.

## 2. The row's general form, kept and widened

**Authored**, against `CLAUDE.md`'s *This is a general engine* paragraph.

The row says: a body the compiler stages declares its position as an anchor
plus an integer block offset, the same declaration a camera station and a
camera subject already carry, so the offset belongs to the class of things
that have a body and a position rather than to the verb that first wanted it —
the quantifier `body_sites` already walks. That is right, and it is one class
short in each of two directions.

1. **A body has more than one position.** A `move-npc` / `move-actor`
   destination is a place a body is *put*, exactly as its spawn is — the
   castle's rank is walked into, not spawned into — and the cast ledger's `at`
   is a claim about where a body *is* that `DW0461` compares to the resolved
   cell. An offset on the spawn alone would let a rank be summoned and never
   formed, and would break `DW0461`'s place arm the first time a body stood at
   an offset the ledger could not spell. So the class is **the mark**: the
   anchor-and-offset a body is declared to be at, wherever the campaign says
   so (§3).
2. **The three camera types are the same class already, typed three times.**
   The ownership ledger accepts them as distinct on the ground that nothing
   else wants a generic `{anchor, offset}`; this spec is the second consumer,
   and the ground is gone. One type, `Mark`, is what all of them are (§4.4);
   the three roles in a shot stay three *fields* (`path`, `look_at`,
   `subject`) and stop being three types.

What the row's form gets right is what this spec builds on: the walk is
`body_sites`, the resolution is `Plan::body_point`, and every consumer of a
body's cell — the summon, `DW0450`, `DW0511`, `DW0896`, `DW0461`, the camera's
static aim at a body — reads the resolved mark and changes nothing else.

## 3. The mark

**Authored.**

```json
{ "anchor": "anchor/muster", "offset": [0, 0, -2] }
```

A **mark** is an anchor and an integer block offset from it, default
`[0, 0, 0]`, in world axes after the piece's placement — the same integer
block offset the camera types carry today (`CameraWaypoint.offset`: *integer
`[x, y, z]` block offset from the anchor*), so a creator who has written a
dolly has written a mark. Its cell is the anchor's resolved cell plus the
offset. It resolves through the anchor table exactly as the anchor alone does —
area-scoped for an object that declares an area, across every placed piece for
one that does not (`Plan::body_point`'s rule, unchanged) — and a mark whose
anchor does not resolve is the dangling reference `DW0325` / `DW0345` /
`DW0360` already own. Two marks with different offsets from one anchor are two
cells; two marks with equal offsets are one cell, and `DW0896` judges them as
it judges two bodies on one anchor today.

An offset says *where beside this place*; it does not say *which place*. That
is the line spec-0062 §9 drew for volumes and this spec keeps: a killing
volume's box is a fact about the geometry, the piece knows where the pit is,
and the box sits on an anchor at the pit; a rank of bodies is staging, no
piece can know how many stand in it, and the campaign says so with the same
tool it already uses to frame a shot. Both rulings stand, and they are about
two different classes.

## 4. Where a mark is written

**Authored.** Every site that puts a body somewhere, or states where one is,
takes the mark. Enumerated, with the JSON each site writes:

### 4.1 Bodies

`Npc` and `Actor` gain `offset` beside `anchor` — the body *is* at the mark,
so the two fields sit on the body as they sit on a waypoint:

```json
{ "id": "actor/man-at-arms-3", "entity": "minecraft:mannequin",
  "anchor": "anchor/muster", "offset": [0, 0, -6], "facing": "south" }
```

`BodyRef` answers `mark()` where it answered `anchor()`; `body_sites` is
untouched; a third body class enters by existing.

### 4.2 Destinations

`move-npc` and `move-actor` replace `to_anchor: AnchorId` with `to: Mark`;
`teleport` replaces its string `to` with the same object:

```json
{ "type": "move-actor", "actor": "actor/man-at-arms-3",
  "to": { "anchor": "anchor/rank", "offset": [0, 0, -6] } }
```

A destination mark is snapped and routed exactly as a destination anchor is
today (`snap_standable_fp` within `SNAP_RADIUS`, then the leg); the snap
starts from the mark's cell. A body's `on_arrive` fires at the cell the leg
ends on, as now.

### 4.3 The cast ledger

`cast[].at` accepts, beside `"offstage"`, `"dead"` and a bare anchor id, the
mark object. `DW0461`'s document arm compares marks (anchor and offset); its
place arm compares the cells `plan::body_station` resolves, unchanged in
kind. A cast row for a body at an offset spells the offset; a row that names
the anchor alone for such a body is the mismatch the rule exists to refuse.

### 4.4 Cameras and a sound's point

`CameraWaypoint`, `AnchorSubject` and `CameraTarget` become the one type; the
JSON a shot writes does not change by a byte, and the ownership ledger's
entry for the three is retired as CLOSED with this section as its reason.
`SoundAt::Anchor` gains the same `offset`, default zero, so a `play-sound` at
a point is a mark like everything else that names a point — the class spec-
0068 puts a firework into.

### 4.5 Facing

Unchanged: a body faces its declared `facing`. A rank faces the way each body
says, which is what a drill does.

## 5. What is checked

**Authored.**

### 5.1 Everything that checks an anchor checks the mark

`DW0450` (a body inside solid geometry), `DW0511` (a posted body inside a
killing volume), `DW0896` (two live bodies on one cell), `DW0359` (a body on an
affordance), `DW0461` (the ledger against the world), the daylight, aggro and
combat-plan readers, the camera's static aim at an `npc` / `actor` subject —
every one reads the mark. A body's cell comes from `Plan::body_point`, which
adds the offset for every body `body_sites` walks; the summons, the snapshot
and render-plan posts and the critical path's `talk-to` position add the same
offset to the anchor cell they resolve, and a destination and a cast row add
theirs after `plan::body_station` resolves the anchor. A `strike` or `use`
trigger at an anchor rides an NPC's hitbox only while that NPC's offset is
zero, since only then does the body stand on the anchor's own cell. A walk
driver's function name carries a non-zero offset (`mv_<npc>_<anchor>_o<x>_<y>_<z>`,
a negative component spelled `m<n>`), so a walk to a bare anchor keeps its
name and two walks to one anchor at two offsets are two drivers. `DW0461`'s
place arm prints the document arm's sentence when the two marks differ, and
the two-buildings sentence when two equal marks resolve to two places.

### 5.2 `DW0896`'s prescription

The rule does not move; its remedy gains the move that is now the campaign's
to make: *give each body its own mark — an offset apiece from one anchor — or
make the campaign prove they take turns*. A gate that names a remedy owes a
check that the remedy is reachable, and `remedy_reachability.rs` carries the
row: seven live bodies on `anchor/exit` of the hello-world fixture are refused,
and the same seven at offsets `[-3, 0, 0]` through `[3, 0, 0]` end green under
`DW0896` and `DW0897` both. This is a loosening of the row's geometry: the
bodies stand one block apart along x, because the fixture's south room is nine
cells wide and a rank two blocks apart over thirteen cells does not fit it;
what the row asserts — seven bodies, one anchor, an offset apiece, green — is
unchanged.

### 5.3 The one new refusal — an offset that leaves the room

A mark's cell must lie inside the placed piece its anchor belongs to
(`Plan::piece_bounds`, the box wave seating and anchor seating are already
confined to). Without this, arithmetic on an offset is a way to stand a body
anywhere in the world — on the horizon, inside the next area — which is a
second placement vocabulary wearing a small field, and it is the vacuous
shape for every geometry proof that assumes a body is in the room its anchor
named. Refused at build tier (exit 3), where the piece boxes are known, with
the anchor, the offset, the cell it reaches and the box it left in the
message; the remedy is the offset. One code, `DW0897` (`compiler::mark`); a
mark on a body, a destination and a cast row all meet it. A camera position
and a sound point are marks and are outside the rule: a dolly is placed where
the framing wants it, and a shot of a building is taken from outside it.

### 5.4 The binding line

`mark binding: B body site(s), M with a non-zero offset; D destination(s), N
with a non-zero offset; C cast row(s) at a mark; R refused for leaving the
piece (DW0897).` on every build, zeroes included — a campaign that writes no
offset prints its zeros and reads as *checked*.

## 6. What the gallery, the record and the skill owe

**Authored.**

- **The gallery element** (spec-0039). The generator cuts `anchor/usher`
  alone; `actor/hall-usher` stands on it and `actor/hall-page` at
  `anchor/usher + [4, 0, 0]`. `npc/marshal` and `actor/sergeant` stand at
  `anchor/muster + [1, 0, 0]`, one cell, so the handoff pair still shares its
  cell, and the marshal's five cast rows naming his stand spell the mark. The
  `move-actor` that walks the standard-bearer to `anchor/vantage` walks it to
  `anchor/vantage + [2, 0, 0]`; the teleport lands at `anchor/vantage +
  [-1, 0, 0]`; the chest-close sound plays at `anchor/pedestal + [0, 1, 0]`.
  Bound by perturbation: changing the page's offset moves the coordinate
  triple in its `summon` line, and changing the destination offset moves the
  last `tp` of the bearer's tick function. Units bound, as the coverage gate
  enumerates them from the schema: `Actor.offset`, `Npc.offset`,
  `SoundAt::anchor.offset`, and `Mark.offset` — the one unit a destination's,
  a cast row's and a camera's offset share, since they are one type.
- **The probes.** `two-bodies-on-one-mark` stands as it is — replacing the
  page's anchor with the usher's, both offsets zero, is still one cell — and
  its `why` names the offset as the remedy; its edit removes the page's
  offset. One new probe for §5.3, `an-offset-out-of-the-room`: the page's
  offset set to `[40, 0, 0]`, refused at `build` with `DW0897`.
- **The record.** `docs/reference/compiler.md`: the `DW0896` row's
  prescription, the `DW0461` row's document arm, the surface rows for `Npc`,
  `Actor`, `move-npc`, `move-actor`, `teleport`, `cast`, `play-sound` and the
  camera types, the new code's row and the binding line — in the pull request
  that lands the code. `tools/check-capability-ownership.py`: the
  `AnchorSubject`/`CameraTarget`/`CameraWaypoint` entry retired.
- **The skill.** A creator reads it from `references/quest-capabilities.md`
  under *Bodies* (a body stands at a mark: anchor plus offset; a rank is one
  anchor and six offsets; the destination takes the same) and under *The
  story layer* (the cast `at` spells the mark when the body has one).
- **The demo level.** A row in `docs/demo-levels.md` when the code lands: a
  guard drawn up under a gate from one anchor, then walked into a second rank
  from one more — the released castle's closing scene on two anchors instead
  of fifteen.

## 7. Scope, and what is named out of it

**Authored.** Every other anchor reference in the DSL addresses a *block or a
place the piece owns*, and none of them takes an offset here, each for the
reason the object gives:

- a killing volume's `region` — spec-0062 §9's refusal, kept;
- a gate anchor (`open-gate`, `close-gate`, a shortcut, a timed gate) — the
  region and the block are the piece's declaration;
- `set-block`, `interact.prop`, a trap, a `loot` container, a `collect`
  container — a block cell the piece authored, and an offset here would be
  placing blocks by arithmetic;
- a trigger's `at`, a volley's `from_anchor`, a wave's forming anchor, a
  checkpoint or bonfire seat — a point the piece declares for the purpose,
  and each has its own proof rooted at that point;
- the entry, which is a role the piece declares (spec-0046).

If a second consumer arrives for any of these, the question is asked again of
that object's class, not answered by copying the field.

## 8. Acceptance criteria

Machine-checkable; each names its instrument, and each was checked against the
tree at `495fca44` before being written. Each verdict states what the
implementing tree measures.

1. **One type.** `crates/dsl/src/stages.rs` declares `Mark {anchor, offset}`
   once; `CameraWaypoint`, `AnchorSubject` and `CameraTarget` are gone or are
   aliases of it; `tools/check-capability-ownership.py` check C reports no
   structural twin among them and its ledger entry is removed; the check's
   binding count is printed and non-zero. *Tree: met — the three types are
   gone; check C examines 73 structs, matches 2 groups, 0 unjustified, and the
   entry is removed.*
2. **The surface.** `delvec schema --stage all` exports `offset` on `Npc` and
   `Actor`, `to` as an object with `anchor` and `offset` on `move-npc`,
   `move-actor` and `teleport`, the mark form of `cast[].at`, and `offset` on
   `SoundAt::anchor`; `to_anchor` appears nowhere in the export; under the
   `dsl_version` the implementing round is handed. *Tree: met at the handed `dsl_version` —
   `offset` on `Npc` and `Actor`, `to` a `$ref` to `Mark` on all three verbs,
   `CastPlace` carrying a `Mark` arm, `offset` on the `anchor` arm of
   `SoundAt`, `to_anchor` 0 times.*
3. **One resolution.** `Plan::body_point` returns the anchor's cell plus the
   offset; a test declares two actors on one anchor with offsets `[0,0,0]` and
   `[1,0,0]` and asserts two summon lines one block apart and no `DW0896`;
   the same two with equal offsets assert `DW0896`. *Tree: met —
   `crates/delvec/tests/mark.rs`.*
4. **Destinations.** A test walks an actor to a mark two cells from its
   anchor and asserts the last `tp` of its tick function lands on that cell's
   centre; a destination mark inside solid geometry snaps as a destination
   anchor does today, asserted by the same test with the mark moved into a
   wall. *Tree: met — `crates/delvec/tests/mark.rs`; the in-wall case asserts
   the walk's target equals `World::snap_standable_fp` of the mark's cell.*
5. **The ledger.** A test declares a body at an offset and a cast row naming
   the bare anchor and asserts `DW0461`; the same row spelling the mark is
   green. *Tree: met — `crates/delvec/tests/mark.rs`.*
6. **The room.** A test sets an offset that reaches a cell outside
   `Plan::piece_bounds` of the anchor's piece and asserts the new code at
   build tier, naming the cell and the box; an offset reaching the piece's
   last interior cell is green. *Tree: met — `crates/delvec/tests/mark.rs`,
   `DW0897`; the green offset reaches the piece box's high corner.*
7. **The remedy is reachable.** `remedy_reachability.rs` carries the row for
   `DW0896`'s offset move, ending green; the new code's own move (shorten the
   offset) has a row ending green. *Tree: met — `dw0896_an_offset_apiece_from_one_anchor_ends_green`
   and `dw0897_shortening_the_offset_ends_green` (§5.2's loosening).*
8. **Cameras unchanged.** The gallery's cutscene emission is byte-identical
   before and after the type unification, asserted by the gallery baseline
   (`gallery/baseline/manifests.json` rows for the cutscene functions do not
   move in the implementing pull request except where §6's element moves
   them, each attributed). *Tree: met — building origin/main's gallery source
   restamped with this engine moves no `cs_` row; `cs_tick_muster_3_2_12687a8f`
   moves only with the marshal's offset and the bearer's destination, each
   attributed in the baseline commit.*
9. **The binding line.** Every build prints §5.4's line; the gallery primary
   reports `M ≥ 1`, `N ≥ 1`, `C ≥ 1`. *Tree: met — the gallery primary
   prints M = 3, N = 2, C = 5.*
10. **The gallery.** §6's element builds green in every declared language;
    perturbing the page's offset moves its summon coordinate; the new probe
    is refused at build with the new code; `two-bodies-on-one-mark` is still
    refused with `DW0896`; `tools/check-gallery-coverage.py` reports 0 units in
    neither state; two builds are byte-identical (ADR-0006). *Tree: met — the
    baseline builds all 8 domain points green; the page's offset moved from 4
    to 3 moves its summon from x 17.5 to 16.5; the probe is refused with
    `DW0897` and `two-bodies-on-one-mark` with `DW0896`; coverage reports
    881 units, 877 bound, 4 refusal-proven, 0 in neither state; two builds of
    the primary write 545 files with one tree hash.*
11. **Determinism.** No consumer of a mark iterates a hash-ordered container;
    the double-build gate is green. *Tree: met — no hash-ordered container is
    added; the double-build gate is green.*
12. **The record and the skill.** The rows and pages of §6, in the pull
    request that lands the code. *Tree: met.*
13. A demo-level row is queued when the code lands. *Tree: met — The
    Muster.*

## 9. Decisions for the owner

- A creator writes a body's place as **`anchor` plus an optional `offset`**,
  and a walk's destination as **`to: {anchor, offset}`** — `to_anchor` goes
  away; the alternative is keeping `to_anchor` and adding a `to_offset` beside
  it, refused as a second field for one place.
- The cast ledger's `at` **spells the offset** when the body has one — the
  alternative is comparing anchors only, which would let the ledger and the
  world disagree by a few blocks unrefused.
- An offset that **leaves the piece is refused** (one new DW code) — the
  alternative is admitting it and trusting `DW0450`, which cannot see a body
  standing on the horizon.
- The three camera position types become **one type** with no change to what
  a creator writes — the alternative is a fourth copy on bodies, which the
  ownership gate would have to be told to accept.
- `play-sound`'s point **gains the offset** — the alternative is leaving sound
  as the one point without one.
- **`dsl_version` moves**; no ADR.
