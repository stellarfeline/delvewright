# spec-0059: A box is placed by its seam, and the grid is derived

- **Status**: Proposed
- **Ground**: written against engine `66906e39`, implemented on top of
  ADR-0023 and ADR-0024 (`e2f251c6`) and finalised against what was built.
  Amends spec-0049 §4.1 (the site-plan document); the departure is recorded
  in spec-0049 §9 the way §4.4's is. Every count below was measured with
  `tools/site-plan-scalars.py`, committed beside this spec, on the gallery's
  site-plan point (`gallery/overlays/site-plan/`). The 24-place plan the
  finding was first measured on no longer exists in any tree, so its numbers
  (236 typed scalars, 117 of them procedural) are quoted from the finding and
  not re-measured; the gallery is the acceptance object.
- **DSL**: `dsl_version` **0.20.0** (a format change is a minor step; the
  `delvewright-dsl` crate version is the same number, ADR-0024). One arm, no
  fence, no shim.
- **Diagnostics**: **DW0883** is allocated to this spec, for the one refusal
  that has no name (§5). Every other refusal reuses a code whose meaning is
  unchanged.
- **Non-goals**: making sightline ends, view eyes and volume regions relative
  to a box (§10); deriving a box's `floor` (a plane is named, §2); any layout
  algorithm that chooses a face — the face is a judgement (§4); the metrics
  values; any campaign content.

## 1. The defect

A site plan is hand-typed absolute geometry half of which the compiler can
already derive, and one edit cascades. On the gallery's plan, 38 of 144 typed
scalars are `boxes[].min` (14) and `seams[].at` (24). The compiler computes
the face two boxes share in order to refuse the author's copy of it
(`DW0828`'s remedy names the face they really share), and the sill of every
one of the gallery's twelve seams equals `max(floor(a), floor(b))` — a number
the datums already state. Changing one box's extent by one block is refused
as `DW0825`, and every box east of it is re-typed by hand, each with its
seams, to get the plan green again.

The creator is an agent. A procedural derivation is handed by the tool, never
typed; an error is refused where it is entered. So placement becomes
**relational**: a box says what it is, a seam says where two boxes meet, and
the compiler packs the graph onto the grid.

## 2. The document, amended (spec-0049 §4.1)

What changes in `site-plan.json`, and nothing else moves:

- **`boxes[]`** — `{node, extent: [dx, dz], floor, ceiling, min?: [x, z]}`.
  `min` is a **pin**: optional, a creative choice of where this box stands in
  world coordinates. A box without one stands where the packing puts it (§3).
  Everything else is as before: `extent` on the kit grid (`DW0825`), `floor`
  a named datum or a `y`, `ceiling` a clearance or `"open"`.
- **`seams[]`** — `{edge, face, at?, meets?, opening? | contact?, stair_in?}`.
  `face` is which face **of the edge's `a` box** the seam sits on, as before.
  `at` is where the crossing sits on **`a`'s** face and `meets` where it sits
  on **`b`'s**, each an **offset from that box's own low corner along the
  face**, never a world coordinate:
  - on a vertical face (`east`/`west`/`north`/`south`) an integer — cells along
    the face's horizontal axis (`z` for east/west, `x` for north/south);
  - on a horizontal face (`up`/`down`) `[dx, dz]` — cells along `x` and `z`.
  Both default to **centred** (§3). **The seam carries no sill**, as it
  carries no rise: the sill is `max(floor(a), floor(b))`, which the plan has
  already stated by putting the two places on the planes it put them on.
- **`region`**, `datums`, `volumes`, `identities`, `sightlines`, `views` and
  `lighting` are unchanged and stay authored in world coordinates. Extent
  flows down: the region is the brief's number, and the packing must fit
  inside it (`DW0826`).

**What is authored and what is derived, per object**:

| Object | Authored (creative) | Derived (procedural) |
|---|---|---|
| box | `node`, `extent`, `floor`, `ceiling`, optional pin `min` | its corner, when not pinned; its cells; its headroom when `open` |
| seam | `edge`, `face`, `opening` or `contact`, `stair_in`, optional `at`, optional `meets` | the corner of the box it places; the crossing's world cells; the sill; the rise |

## 3. The packing rule

Stated so that a second implementation reproduces the grid. All arithmetic is
integer; `corner(B) = [x0, z0]` is a box's low corner, `ext(B) = [dx, dz]`,
`floor(B)` its walk plane, `clear(B)` its headroom (the class minimum when
`open`). A box occupies `x0 ..= x0 + dx - 1`, `z0 ..= z0 + dz - 1`,
`floor ..= floor + clear - 1`.

**The width a crossing is centred by.** For a portal, the standard opening's
`width` (and on a horizontal face its `height` along `z`). For a contact with
an `extent`, `extent[0]` (and `extent[1]` on a horizontal face). For a contact
with no `extent` the crossing is the whole face, so the width is the face and
both offsets default to `0`.

**Defaults.** On a vertical face with along axis `L` (`1` for east/west,
`0` for north/south): `at = max(0, (ext(a)[L] - w) div 2)` and
`meets = max(0, (ext(b)[L] - w) div 2)`. On a horizontal face the same per
axis with `[w_x, w_z]`. `div` rounds toward negative infinity; a crossing
wider than its face centres at the corner, and whether it fits is `DW0829`'s
or `DW0876`'s question, as before.

**Placing `b` from a placed `a`** across face `F` of `a`:

- `east`: `x0(b) = x0(a) + dx(a) + 1`; `west`: `x0(b) = x0(a) - dx(b) - 1`;
  `south`: `z0(b) = z0(a) + dz(a) + 1`; `north`: `z0(b) = z0(a) - dz(b) - 1`
  — the one shell cell between two connected places (`DW0828`'s gap), as
  today. Along the face: `corner(b)[L] = corner(a)[L] + at - meets`.
- `up`/`down`: `corner(b) = corner(a) + at - meets` on both axes. The floors
  are authored; the one-cell vertical gap between `a`'s ceiling course and
  `b`'s floor (or the reverse) is judged as today (`DW0828`).

**Placing `a` from a placed `b`** solves the same equations for `corner(a)`.

**Order.** The seeds are the pinned boxes. Then, repeatedly, scan `seams[]`
in document order: a seam with exactly one end placed places the other end; a
seam with both ends placed is **checked** (§5) and places nothing; a seam with
neither end placed is skipped this pass. Stop when a full pass places nothing.
A `vision` edge has no seam and places nothing.

**Crossing cells.** On a vertical face the crossing runs
`corner(a)[L] + at ..= corner(a)[L] + at + w - 1` along `L`, and
`sill ..= sill + h - 1` upward from `sill = max(floor(a), floor(b))`, in the
shared wall's plane. On a horizontal face it runs from `corner(a) + at` over
`[w_x, w_z]` in the shared course. Every rule that judged a seam's world
cells before (`DW0829` fit and step, `DW0830` run, `DW0831` fall, `DW0836`,
`DW0838`, `DW0876`, `DW0877`) judges exactly these cells; none of them
changes.

**What is printed.** `delvec validate` hands the derivation back: one line per
box with its corner and whether it was pinned or which seam placed it, and a
binding line — `N box(es) placed: P pinned, D derived through S seam(s), in C
component(s)`. A derived corner is read from that output, never typed into
the document.

## 4. Every scalar of the plan, classified

The instrument is `tools/site-plan-scalars.py`: a scalar is a JSON number or
one of the plan's enum-valued strings (`face`, `cmp`, `axis`, `of`, `role`,
`fixture`, a ceiling's `"open"`); ids, references and notes are not scalars.
The class is a property of the field, stated once in the tool.

| Field | Class | Why |
|---|---|---|
| `region.min[]`, `region.extent[]` | creative | the brief's number flowing down |
| `datums[].y` | creative | a plane the design chose |
| `boxes[].extent[]` | creative | how big the place is |
| `boxes[].floor.y`, `boxes[].ceiling.clearance`, `ceiling: "open"` | creative | which plane, how much headroom, sky or not |
| `boxes[].min[]` | **procedural**; a typed pin is creative | determined by the seams once the rule is fixed; pinned where the author wants the whole to stand |
| `seams[].face` | creative | which side the door is on is a judgement, not a derivation (§10 names the layout algorithm this refuses) |
| `seams[].at`, `seams[].meets` | **procedural** (centred); a typed offset is creative | where along a face the door sits, when it is not the middle |
| `seams[].at[1]` today — the sill | **procedural, no spelling** | `max(floor(a), floor(b))` |
| `seams[].contact.extent[]` | creative | how wide the front is |
| `volumes[].region.*`, `volumes[].role` | creative | mass the whole owns |
| `identities[].cmp/measure.of/measure.axis` | creative | what the plan promises the brief |
| `sightlines[].from/to[]`, `views[].eye/look_at[]` | creative | world coordinates, still (§10) |
| `lighting.*` | creative | the fixture and its level |

**Counts** (gallery site-plan point, `python3 tools/site-plan-scalars.py
--form old gallery/overlays/site-plan/site-plan.json` at `66906e39`, then
`--form new` on the same plan rewritten to this form): **144 typed → 137
typed; procedural 38 → 0.** The 7 boxes lose 12 of 14 `min` scalars (the entry
keeps its pin); the 12 seams lose all 24 `at` scalars and gain 29 offset pins,
because the gallery's grid was laid by hand to no rule and reproducing it
byte-for-byte pins 11 `at` and 12 `meets` (§8). The same plan written to the
defaults — every offset omitted — types **108**; that is the figure an author
writing fresh pays, and it is not committed because it moves the baseline.

## 5. Refusals

| Code | Rule |
|---|---|
| **`DW0883`** | **A box is not placed exactly once.** Two shapes of one claim. (1) A connected component of the seam graph in which no box carries a pin: nothing places it, so it has no cells for any rule to judge — named with every box in the component and the prescription to pin one of them (the entry's, when it is there). (2) A pinned box the packing also reaches, with a different corner: the pin and the seam that derived the corner disagree — named with the box, the pin, the derived corner and the placing seam. Validation tier (exit 1). Binding: components examined, pins compared. |
| `DW0828` | unchanged meaning, three more shapes of it: an `at` or `meets` that lies off its own box's face (`at < 0`, or `at + w > ext`); an offset of the wrong dimensionality for the face (an integer on `up`, a pair on `south`); and, on a seam whose both ends are already placed, a crossing that is not the same cells seen from `a` (`corner(a)[L] + at`) and from `b` (`corner(b)[L] + meets`), or a gap across the face that is not one cell — which is exactly how a loop that does not close, or a pin that contradicts a seam, is refused: at the seam, naming both corners and how each was obtained. |
| `DW0825` | unchanged: an extent off the kit grid, named per box. |
| `DW0826` | unchanged: a derived corner is judged against the region exactly as a typed one; the message names the box and how its corner was obtained. |
| `DW0829`, `DW0830`, `DW0831` | unchanged: judged on the derived cells and the derived sill. The sill-step half of `DW0829` reads `max(floor(a), floor(b)) - floor(source)`, so a walk between planes more than a step apart is refused as today and the remedy is the same (a stair). |
| `DW0843`, `DW0844` | unchanged: the frame handed to a detail piece is the derived box; the openings it must answer are the derived crossings. |
| `DW0841` | unchanged rule; what the freshness key is over is spec-0050 §2's, and this form change moves every walk record once, in the adopting change. |

No refusal is weakened. A plan that was refused before is refused after with
the same code; a plan that is refused after was, in the old form, a plan the
author had to get right by arithmetic.

## 6. Determinism

The packing takes no seed and reads no clock: same plan, same graph, same
table → the same corners, and two `delvec build` runs of a site-plan campaign
stay byte-identical (spec-0049 §13.4 holds unchanged). The scan is document
order; `delvec fmt` never reorders arrays, so formatting cannot move a box.

## 7. The regeneration property

Edit one box's `extent` and run `delvec validate`: no other box's row in the
document changes, and every refusal names that box or one of its seams.
Boxes downstream of it move in world coordinates — that is the point — and
they move without being re-typed. Two residues, named: a whole that no longer
fits the region is `DW0826` on the box that left, which is the region's
finding; and a sightline end, view eye or volume region typed in world
coordinates against a box that moved is the residue §10 records.

## 8. The gallery, the fixtures, the gym, the baseline

- The gallery's site-plan point adopts this form and its **grid does not
  move**: the packing reproduces the hand-typed corners exactly — `[21, 4]`,
  `[21, 21]`, `[4, 13]`, `[40, 12]`, `[24, 16]`, `[24, 38]`, `[36, 38]` —
  with the entry box pinned and the 29 offsets §4 counts pinned where the
  hand-laid grid is off-centre, and the blockout's sha256 is the one the walk
  record already carried. What the baseline records as moved is the number:
  every point's `dsl_version` header, every stamped envelope in `inputs`,
  and the JSON outputs that carry the version string (11 of 529 per
  `areas[]` point, 4 of 280 on the site-plan point); the other 3 454 emitted
  paths are byte-identical. The site plan is hand-written in the overlay (the
  gallery generator emits prefabs, not the plan), so the adoption is an edit
  of that document.
- Coverage: `PlanBox.min` stays bound (the pin); `Seam.at` stays bound;
  `Seam.meets` is bound by the same seams — 959 units, 955 bound, 4
  refusal-proven, none in neither. `DW0883`'s probe
  (`gallery/probes/nothing-places-the-whole`) is the primary's graph embedded
  relationally plus one declared edit that deletes the entry box's `min`; the
  engine refuses it with `DW0883`.
- `crates/delvec/tests/fixtures/blockout/` and every inline plan in
  `crates/dsl/tests/` and `crates/delvec/tests/` adopt. The metrics gym's
  generator (`compiler::gym`) emits the relational form: the first bay
  pinned, the spine's seams at `at: 1, meets: 1` — the cells they stood at.
- The three walk records (`gallery/overlays/site-plan/`, two probes) are
  regenerated from the adopted plans' canonical hashes, by the tool, in the
  same change.

## 9. `dsl_version` and adoption

The number moves to **0.20.0** and every document in this repository adopts
in the same change (ADR-0024 decision 4). `delvec fmt` writes the number: it
rewrites `dsl_version` to the one the engine implements on every envelope it
formats, so adoption of a campaign is `delvec fmt` plus the edit this form
asks for. The content repository's skill page moves with its pin; its new §2B
text is drafted in §11 and is not this spec's to land.

## 10. The two-artifact question, decided (ADR-0022 revisit trigger)

ADR-0022 says: "Every layout-graph edit is immediately a site-plan edit and
the agreement check between the two artifacts never fires alone — merge them
into one." spec-0049 §10 deferred that to a CI-visible count of `DW0824`
firing alone, and no such count was ever built.

**The two documents remain. Decided here, once.** Reasons:

1. **The premise does not hold, by enumeration of the graph's own fields.** Of
   the fourteen field kinds a layout graph carries, nine have no site-plan
   consequence at all — `intent`, `note`, `gating`, `one_way`, `shortcut`,
   `opens_from`, `goal`, `critical_path`, `beats`. Those are the mission →
   space bridge, and editing them is the graph's ordinary iteration. Only
   `nodes[].id`, `size_class`/`way_class`, `edges[].id/a/b/class` and `entry`
   reach the plan. A graph edit is not "immediately a site-plan edit"; the
   ones that are, are exactly the ones `DW0824` is for.
2. **The stages condition on different inputs** (ADR-0002): the graph on the
   quest documents; the plan on the brief and the metrics table. One document
   with two conditioning levels is the shape the staged DSL exists to avoid,
   and the ordering tooth "a plan validates only against a graph" (spec-0049
   §7.1) is a property of two files that one file with optional halves would
   have to re-invent as a rule.
3. **The cost the trigger weighed is the cost this spec removes.** The
   agreement check was expensive because a graph edit forced coordinates to be
   re-typed; with the relational form a graph edit that adds a node costs one
   box row and one seam row of judgement and no arithmetic.

History, at the strength it has: of 5 distinct commits touching the gallery's
graph or plan at `66906e39`, 3 touched both, 1 the graph only (a spec-0052
adoption), 1 the plan only; the fixture campaign's 3 touched both every time.
Too thin to decide on, and not what this decision rests on.

What `DW0824` is after this spec: the check that a box and a seam exist for
every node and traversal edge — unchanged — and now the *only* cross-document
obligation, since a seam carries no coordinate the graph could disagree with.

## 11. The skill page, step 2B — the text the content repository lands

Replaces the two bullets that begin "A box is the play space, and connected
boxes sit exactly ONE CELL apart" and "Seams are allocated, not discovered":

> - **A box says what it is; a seam says where two boxes meet; the engine
>   derives the grid.** A box is `{node, extent, floor, ceiling}` — `extent`
>   is the interior a body stands in, on the kit grid. Write `min` on **one**
>   box (the entry) to say where the whole stands in the region; write it on
>   no other box unless you mean to assert its corner, because a pin that
>   disagrees with the seams is `DW0883`.
> - **A seam is `{edge, face, opening | contact, at?, meets?, stair_in?}`.**
>   `face` is the side of the edge's `a` box the crossing is on. The engine
>   puts `b` one cell beyond that face — the wall — and the crossing in the
>   **middle** of both faces. When the door is not in the middle, say where:
>   `at` is cells from `a`'s low corner along the face, `meets` cells from
>   `b`'s (an integer on a wall face; `[dx, dz]` through a floor or ceiling).
>   A seam that closes a loop places nothing: both boxes already stand, and
>   the engine checks that its cells are the same seen from either side
>   (`DW0828`).
> - **Nothing about a sill, a rise or a wall thickness is written anywhere.**
>   The sill is the higher of the two floors; the rise is their difference;
>   the wall is the one cell the packing leaves between neighbours.
> - `delvec validate` prints every box's derived corner and which seam placed
>   it. Read your `volumes[]`, `sightlines[]` and `views[]` against that
>   output — they are still world coordinates.

## 12. Acceptance criteria

Machine-checkable; each names its instrument.

1. `delvec schema --stage site-plan` exports `boxes[].min` as optional and
   `seams[].at` / `seams[].meets` as the offset form (integer or
   `[integer, integer]`), with no sill component; a `seams[].at` of three
   numbers is `DW0100`.
2. `python3 tools/site-plan-scalars.py --form new
   gallery/overlays/site-plan/site-plan.json` reports **procedural 0** and
   **typed 137**; the tool refuses a new-form plan that types a procedural
   scalar and refuses a field it does not classify.
3. `python3 tools/gallery-baseline.py` (the verify arm) is green over the
   adopted gallery, and the site-plan point's blockout sha256 is unchanged;
   the only emitted paths that move are the ones carrying the version
   string, enumerated in the baseline's commit.
4. Determinism: two builds of the blockout fixture are byte-identical
   (spec-0049 §13.4, unchanged), and a test reverses `seams[]` in a plan with
   a loop and asserts every packed corner is unchanged.
5. `DW0883` has a test asserting each shape (an unpinned component; a pin the
   packing contradicts) and a committed gallery probe; `tools/check-dw-codes.py`
   is green with zero new allowlist entries. The three new `DW0828` shapes
   each have a red test (`crates/dsl/tests/v14_site_plan.rs`).
6. Regeneration: a test widens one box by one quantum and asserts every new
   diagnostic names that box or one of its seams, with §7's sightline residue
   asserted as exactly one `DW0824` at the sightline; the region case is its
   own test naming the box that left and how it was placed.
7. `delvec fmt` rewrites `dsl_version` to `0.20.0` on every envelope it
   formats; `tools/check-json-canonical.py` is green over the tree.
8. `docs/reference/compiler.md` carries the amended site-plan surface table,
   the `DW0883` row, the amended `DW0828` row and the packing rule stated as
   §3 states it; `tools/check-doc-dupes.py`, `tools/check-diagnostic-messages.py`
   and the docs job are green.
9. The gym generator emits the relational form and `cargo test -p delvec
   --test gym` is green.
11. `delvec validate` on a site-plan campaign prints one placing line per box
    with its corner and provenance, and the binding line carries the pinned /
    derived / component counts.
10. A row for this mechanic is queued in `docs/demo-levels.md` (**The Hung
    Hall**).

## 13. Not settled here

- **World coordinates that name a cell in a box** — a sightline's ends, a
  view's eye and look-at, a detail piece's frame checks — stay absolute. When
  the box they lie in moves, `DW0824` (a sightline end outside its box) or a
  render that looks at nothing is the finding. The general form is a point
  stated as `{node, at: [dx, dy, dz]}` from the box's corner, which is a
  capability of "a point in a place" and belongs to every object that names
  one; it is not built here so that this spec moves one thing. Ledger row,
  not a doc line.
- **A face chosen by the engine** — a graph embedding that picks which side a
  door is on — is refused as a design decision the engine has no business
  making; the falsifier is a creator who wants exactly that, at which point
  it is a first-class surface (`face: "any"`) or nothing.
