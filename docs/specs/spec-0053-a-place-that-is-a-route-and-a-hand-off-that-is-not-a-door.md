# spec-0053: A place that is a route, and a hand-off that is not a door

- **Status**: Accepted
- **Ground**: spec-0049 §11 reserves its own falsifier — *"the first campaign
  brief this vocabulary cannot state without a workaround is the evidence."*
  This spec records that falsifier being met, decides what it demands, and
  fixes the shape of the answer. Evidence measured on: engine `ba81399c`;
  content `eaa3dc3e` (`design/bell-r2-stages`: `design/programs/zones.json`,
  `design/reference/map-brief.md`) and `659f9dd1` (`campaign/bell-r2`:
  `prefabs/z*.json` piece metadata). Every count below was re-measured at
  those revisions, not quoted.
- **DSL / Diagnostics**: this spec allocates **no** `dsl_version` and **no**
  DW code. The surfaces below need both; they are handed to the
  implementation rounds at dispatch, per the standing allocation rule.
  Existing codes are referenced by number because they are already landed.
- **Non-goals**: every building-metric **value** (the gym walk calibrates
  them — spec-0049 §2.3, unchanged and not displaced by this spec); any
  campaign content, including how the evidence campaign re-partitions; new
  edge or face classes (§5); stair or barred contacts (§8).

## 1. The decision

The metrics vocabulary gains two things, both widenings of bindings that
exist, neither a new mechanism:

1. **A way class** — a second kind of place classification beside the
   size-class ladder, for a place whose footprint is bounded in one axis and
   free in the other: a road, a causeway, a corridor, a duct.
2. **A contact seam** — a second kind of seam connection beside the named
   standard opening, for two places that simply meet along a front rather
   than through a doorway.

Everything else the evidence red-listed is **not** a vocabulary gap: it is
either an uncalibrated number the gym walk settles (the drop cap, the
near-miss small openings, the kit quantum) or existing vocabulary the
evidence artifacts predate (the `drop` and `barred` classes). §5 sorts it.

## 2. The evidence, and which reading is right

A campaign measured against the standards reported: 1 of 8 footprints fits
any size-class rung (and that one's height is under its rung's clearance);
4 of 21 declared faces match a standard opening; all 21 faces are class
`walk` against a design with drop and barred hand-offs; one shortcut drops
6.5 against the cap of 5; 4 of 8 footprints are off the kit grid. All
confirmed by re-measurement at the pinned revisions. Every verdict rests on
provisional numbers — `DW0813` says so on every run — so two readings were
possible: **(1)** the vocabulary is missing a shape, or **(2)** the numbers
are wrong and a calibrated table admits everything.

**The test that separates them: calibration moves a value; it cannot change
the kind of constraint an entry states.** So ask of each red — *does any
value the gym walk could defensibly set admit this shape while the entry
still classifies?*

- **The drop of 6.5** (a shortcut falling from a +9 terrace to a +2.5
  yard): yes. The cap is a policy seed with a physical ceiling of 22; 5
  costs one heart, 6.5 costs two, and which is the right comfort line is
  exactly a walkable judgement. Reading (2). The cap's value is the gym's
  to move — **this spec does not move it**, and if the walk keeps it at 5,
  the campaign's shortcut is what changes.
- **The small openings** (3×2, 2×2, 4×2, 2×5, 8×3, …): within calibration
  or adoption distance of the standard set. Reading (2), plus content
  adopting standards.
- **A footprint of 16×72**: no. For any rung to admit it, that rung must
  span 16..72 on an axis — a class in which a 16×16 room and a 72×72 hall
  are the same thing has stopped classifying. The failure is by kind, not by
  margin, and it recurs (16×72 road, 11×76 hall, 41×125 tower-and-ward).
  Reading (1).
- **An opening of 55×3** (and its sibling 57×3 on the opposite face of the
  same tower, and 21×4, 41×4): no. The width of a front along which two
  places meet is a fact of those two boxes' shared face — per-campaign
  geometry, continuous — and a *standard* that enumerates it becomes a new
  named entry per campaign, which is the enumeration going stale exactly as
  the size ladder does. Reading (1).

**The engine's own instrument had already found reading (1) from the other
direction.** The metrics gym's coverage line (`DW0840`) reports
`corridor.min-width` and `corridor.min-clearance` unreachable, and its
recorded reason is the finding: *"the site plan has no surface for a place
that is not a box with a size class — so a two-wide corridor cannot be
spelled at all."* For those entries, reading (2)'s remedy — walk the gym
first — is not even executable: no bay can instantiate an entry no document
can spell, so **the calibration path itself is blocked by the vocabulary
gap**. Two independent instruments — a campaign measured in the content
repository and the gym's unreached-entry count in the engine — agree, and
they share no configuration.

Two further dissolutions, named so the evidence is not over-read. Several
elongated footprints are artifacts of the zone-as-unit partition ADR-0022
§5 retires —
a 41×125 zone bundles a tower, its ward and its approach, which the site
plan separates into their own boxes — so part of the "1 of 8" dissolves
under re-partition. But not all of it: the design brief itself states *"one
cut ledge, one body wide, climbing across the whole seaward face"* — the
route shape is a brief fact, not a partition accident. And the all-`walk`
faces are the superseded partition's piece metadata, not a missing class
(§5).

## 3. The way class

A new building-metric kind, `way-class.<name>`, beside `size-class.<name>`:

- **Entry shape**: `{min_width, max_width, min_clearance}`, in cells, each
  entry carrying `calibrated` like every building metric. A way class bounds
  the **cross-section** — the axis a body feels — and says nothing about the
  run, because a route's length is per-campaign geometry, not a standard.
- **Seeds**: `way-class.corridor` **subsumes** the freestanding
  `corridor.min-width` and `corridor.min-clearance` entries — they become
  its fields rather than gaining a consumer, so there is one authority and
  the gym's unreached-entry finding retires by the entries becoming
  spellable, not by deletion. One broader seed (`way-class.road`) lands
  beside it. Values are seeds for the gym, per the standing discipline;
  this document fixes shape and mechanism only.
- **Declaration**: a layout-graph node declares **exactly one of**
  `size_class` or `way_class` (unknown name: `DW0812`, with the kind added
  to the resolve vocabulary). The completability proof reads neither — a
  way class is a node classification, and this spec deliberately adds **no
  edge or face class**, so the proof's meaning set is untouched.
- **Geometry** (the way branch of `DW0832`): the box's **shorter**
  horizontal extent is the width and must lie in the class's range;
  clearance as for size classes; and the run — the longer extent — must
  **strictly exceed the class's `max_width`**. The elongation demand is
  structural rather than a new constant: it is what a room cannot supply.
  A square box can never qualify (its "run" equals its width, which cannot
  both satisfy `≤ max_width` and exceed it), so declaring a room a way to
  escape the ladder is refused by the object's own shape, which is the
  property the opt-out rule requires.
- **Pacing** (`DW0822`, both call sites, thresholdless as ever): a way
  leg's traverse is its **measured run** — the plan-site and bytes-site
  measurements use the box's long extent, a real number rather than a class
  constant. At the graph site no geometry exists yet, so the projection
  states its way legs as unprojected in its binding line rather than
  inventing a number.

## 4. The contact seam

A seam's connection becomes a choice of two kinds — the existing named
opening (a **portal**: a body crosses at these allocated cells, through a
standard a table names), or a **contact**:

- **Declaration**: the seam states a span of the shared face (defaulting to
  the whole of it) instead of an opening name. **What a contact means when
  two places simply meet**: the boundary is continuous ground — the
  derivation writes **no wall along the span** (and wall as ever outside
  it), and crossing is legitimate anywhere along it the step rule admits.
- **What the proof reads, exactly**: the author allocates *where* the
  places meet; the **engine measures** the crossing profile from assembled
  bytes — which columns of the span a body crosses under the step rule.
  The closure crosses the edge iff the measured profile contains at least
  one passable column of body width. `DW0838`'s allocation set for the edge
  is the span, so a crossing outside it stays a refusal — seams remain
  allocated, never discovered. `DW0836`'s independent-observer duty extends:
  a contact whose measured profile is empty is refused on bytes, and the
  realized rise per crossing column must match the declared rise.
- **No door checks apply**: `DW0829`'s standard-name resolution and sill
  rule are portal checks; a contact has no opening name to resolve and no
  single sill. Calling a 55-cell front a door would make every downstream
  door check wrong; the spec says so instead of shoehorning.
- **Classes**: a contact carries `walk` or `drop` (a rim falling to a lower
  court is a genuine broad hand-off; `DW0831`'s cap applies to its declared
  rise unchanged). `stair`, `barred` and `vision` contacts are **excluded
  until a campaign brief demands one** — the standing falsifier, re-armed.
- **The floor that keeps it honest**: a contact's span must be **wider than
  the broadest standard opening in the table**. Structural, derived from
  the table rather than seeded: anything narrower could have been a portal,
  so a doorway declared a contact to dodge the standard set is refused by
  its own width — again a property the defect cannot supply. The evidence
  spans (21, 41, 55, 57) all clear it; a 3-wide slot does not.

## 5. What is NOT a vocabulary gap, sorted

- **The all-`walk` faces.** The answering classes exist at every level:
  `drop | barred` are graph-edge classes (spec-0049 §3.1, `opens_from`
  spelling the one-side-openable door first-class), seam classes, contract
  edge and face classes, and the grammar's contract checker accepts
  drop/barred edges to `exterior` (refusing only a declared rise toward
  exterior, correctly — the seam owns the rise). The 21 `walk` faces are
  the superseded partition's pieces not declaring what the design says. That
  is adoption of existing vocabulary, and this spec adds nothing for it.
- **The drop cap, the small openings, the kit quantum.** Numbers. The gym
  walk is still owed, is not displaced by this spec, and is the only party
  that may move them. The two new entry kinds land provisional and join
  `DW0813`'s ledger and the gym's bays like every other building metric.

## 6. Refusals owed

Codes allocated at implementation dispatch; each refusal is falsifiable and
its tripping shape is named:

| Refuses | Trips on |
|---|---|
| A node declaring both `size_class` and `way_class`, or neither where one is required | a node carrying both fields |
| A way-classed box off its class: width (shorter extent) outside the range, clearance under the minimum, or run not exceeding `max_width` | a 32×40 box declared `corridor`; a square box declared any way; a box one cell under clearance |
| A contact span off the shared face, or not wider than the broadest standard opening | a 3-wide contact (a door dodging the standard set); a span leaving the face `DW0828` established |
| A contact of class `stair`, `barred` or `vision` | `class: stair` on a contact seam |
| A contact nothing can cross: measured profile empty over assembled bytes | a contact allocated where the massing walls the span (perturbed-derivation fixture, the spec-0049 §13.8 manner) |
| An unknown way-class name | `way_class: "highway"` — existing `DW0812`, kind added |

## 7. What the engine does not know

Stated so the implementation cannot drift into knowing it:

- **The length of a route.** Measured into pacing; never a standard, never
  a rung.
- **The width of a front where two places meet.** A fact of the two boxes;
  never a named opening. In particular, **no standard opening is added
  whose dimensions are this campaign's measured geometries** — an
  `opening.gate-front` of 21×4 would be content wearing a standard's
  clothes, the exact workaround §11 forbids.
- **Which places are routes.** The author declares the class; the geometry
  must supply the elongation and the cross-section; the engine measures
  both. *"This face is fine"* is never a declaration the engine accepts —
  a contact's crossing profile is computed from bytes, not asserted.

## 8. The general-engine test, and the falsifier re-armed

A way class serves a village lane, a canyon rim trail, a ship's gangway and
a mine gallery identically; a contact serves a green meeting a lane, a
shore meeting a field, a courtyard pair, a rim over a lower court. Widths,
clearances and the class names themselves are table entries a creator
re-fixes for their own fiction; nothing above encodes a cliff, a bell or a
souls-like. **Falsifier, carried forward**: the next campaign brief this
vocabulary cannot state without a workaround is the evidence — the known
candidates being a stair contact (terraced meeting), a barred contact (a
portcullis the width of a front), and a place bounded in neither axis that
is not an expanse. Each is excluded today and decided on that brief.

## 9. Acceptance criteria

Machine-checkable; each names its verdict's instrument. These bind the
implementation rounds; the two current-tree facts cited (the gym's
unreached entries, the subsumed corridor entries) are true at `ba81399c`.

1. `delvec metrics` exports every `way-class.*` entry with `calibrated`,
   and the freestanding `corridor.min-width` / `corridor.min-clearance`
   entries no longer exist as separate keys — their floors re-assert inside
   `Metrics::self_check` against the corridor way class, and the
   `metrics_version` digest test moves per its own rule.
2. `delvec schema --stage all` shows a layout-graph node accepting exactly
   one of `size_class` / `way_class`, and a seam accepting exactly one of
   opening-name / contact-span; `tools/check-gallery-coverage.py` is green
   with every new unit bound in the gallery domain or refusal-proven.
3. Every refusal in §6 has an allocated code, at least one test asserting
   it, and a fixture the compiler refuses with it; `tools/check-dw-codes.py`
   is green in both directions with zero new allowlist entries.
4. The way branch of the box-geometry check reds each of §6's three way
   trips in committed fixtures; a way-classed box that satisfies all three
   builds green end to end through the existing derivation and battery.
5. The metrics gym instantiates way bays (the corridor widths its plan
   already names), and its `DW0840` line no longer names the corridor
   entries — demonstrated by the gym build's own output before and after.
6. A blockout over a contact seam leaves exactly the span open (no wall
   cell inside it, wall everywhere outside it on that face), double-build
   byte-identical; the empty-profile refusal is produced by a perturbed
   derivation in a test, not by hand-authored bytes.
7. `DW0822`'s graph-site line states unprojected way legs in its binding;
   its plan-site and bytes-site figures for a way leg equal the box's long
   extent; all sites remain thresholdless.
8. `docs/reference/compiler.md` carries the new rows and the two-kind seam
   and node tables; the docs job, `tools/check-doc-dupes.py` and
   `tools/check-reference-versions.py` are green.

## 10. Not settled here

- **Every value** — way widths, clearances, the contact floor's consequence
  once opening widths move, the drop cap, the quantum: the gym walk, which
  this spec makes *more* executable (§3) and does not replace.
- **Stair / barred / vision contacts** — excluded, falsifier re-armed (§8).
- **The evidence campaign's re-partition** and the fate of its pieces —
  content work, in the content repository, under ADR-0022's disposition of
  the superseded artifacts.
- **Whether the near-miss small openings become standards** — the gym
  decides on the walked set, never on this campaign's inventory.
