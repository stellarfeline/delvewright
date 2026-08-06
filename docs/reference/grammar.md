# Box-split grammar back end — live behavior record

What `crates/grammar` (package `delvewright-grammar`) does **today**. spec-0027
is the decision record; this page is the behavior record, and any PR that
changes the crate's surface updates it in the same PR.

It is a **library**, not a tool: no binary, no `delvec` path, nothing in
[`tools.md`](tools.md). It ships in no delve — generation-time only (ADR-0003).
The engine depends on it nowhere; `crates/compiler` names it as a *dev*-dependency
only, to test the export seam of §7 from both sides.

## 1. Model

A **grammar program** is data: named rules over integer voxel boxes. Expanding
one against a box and a `u64` seed derives a **voxel model** — a dense grid of
full block states.

Every scope in a derivation is a box plus an **orientation**: a permutation
mapping the rule's local `X`/`Y`/`Z` onto world axes. That is what lets one rule
be reused turned 90°, and what `reorient` manipulates.

```text
Program ─ expand(program, region, {seed, limits, orientation}) ─▶ VoxelModel
        ─ export_prefab(program, region, options, id) ──────────▶ .nbt + .json
```

## 2. Program surface

| Element | Form | Notes |
|---|---|---|
| `name` | string | provenance label |
| `start` | rule name | expanded into the whole region |
| `params` | name → i64 | size/kind controls; read by `{"expr":"param"}` |
| `palette` | role → paint | style controls; a paint is a block-state string or a weighted list |
| `rules` | name → `[alternative]` | each alternative is `{weight, when, body}` |

**Rule bodies** (`op`): `fill` (a role or an inline paint), `void` (air), `skip`
(leave as-is), `call`, `split`, `reorient`, `mark`.

**`split`** cuts one local axis into pieces: `absolute` pieces take a fixed block
count, `relative` pieces share what is left. `rounding` (`truncate` — the
default and upstream's only behaviour — `start`, `end`, `middle`) says where the
indivisible remainder goes; `repeat` tiles the pattern across the axis and clamps
the last piece; `orient` hands every child a new orientation. Children are
matched to pieces in order, and cycled when `repeat` produced more pieces than
children.

**`reorient` / `orient`** name a child axis as `local_*`, `world_*`, `smallest`,
`largest`, or `split_axis` (the axis being cut; splits only). Unnamed axes are
completed to a permutation: keep an axis where possible, otherwise complete the
cycle the request started (asking for "my Z is the old X" swaps X and Z),
otherwise take the lowest free axis.

**Guards** (`when`): `always` (default), `otherwise`, `cmp` over integer
expressions of literals / params / scope dimensions with `+ - * / % max min`,
`all` / `any` / `none_of`, and `orientation` (matches an exact axis mapping — how
a directional stair or door picks its facing).

**Selection**: every non-`otherwise` alternative whose guard holds is a
candidate; if none hold, the `otherwise` alternatives are; among candidates the
seeded PRNG draws by `weight`. **Two guards that can hold at once are a
probabilistic choice, not a priority order** — guards meant as a decision must be
mutually exclusive.

The IR serialises to JSON (`serde`), which is the authoring form; block states
are their vanilla string, e.g. `"minecraft:oak_stairs[facing=east,half=top]"`.

## 2b. `mark` — anchor declarations

An anchor is **metadata**, not geometry. No composition of `fill` / `split` can
express "this cell is where the boss stands", and reading one back out of the
block pattern afterwards is a guess — which the layering rule forbids. So the
rule that shapes a space declares it while it still has the box in hand.

`mark` wraps a body (like `reorient`) rather than being a statement, because a
rule body is one node: that way a mark can sit on any child of any split and
annotate exactly the piece that child owns. Use `{"op": "skip"}` as the body when
the declaration is all that is wanted.

```json
{ "op": "mark", "mark": { "anchor": "courtyard", "at": "floor_center" },
  "body": { "op": "void" } }
```

| Field | Meaning |
|---|---|
| `anchor` | kebab-case stem. The exported key is `anchor/<stem>`, i.e. the DSL's `anchor/<kebab>` id — a mark cannot name an anchor the DSL could not reference. |
| `at` | which cell (flattened into the mark object, see below) |
| `facing` | `north`/`south`/`east`/`west`. Omitted, it is **derived**: a grammar orientation is a permutation without reflection, so the derived facing is the negative direction of the world axis the scope calls local `Z` — `north` when that is world `Z`, `west` when it is world `X`. A scope whose local `Z` is *vertical* has no cardinal facing and says so rather than guessing. |
| `index` | `unique` (default) → `anchor/<stem>`; `auto` → `anchor/<stem>-<n>`, `n` counting from 1 per stem in expansion order — how a rule that runs once per tower gives every tower an anchor without knowing how many there are. Matches the hand-built `anchor/alcove-1…` convention. |

`at` is one of:

| `at` | Cell |
|---|---|
| `corner_min` | the scope's minimum corner |
| `floor_center` | lowest **world** `Y`, centred on world `X`/`Z`. Gravity is a world fact, so this one position ignores the scope's local axis names |
| `face_center` (+ `axis`, `side`) | the given **local** axis pinned to `min`/`max`, the other two centred |
| `offset` (+ `x`, `y`, `z` expressions) | **local** cells from the minimum corner |

Centres round down on an even extent (the lower-middle cell) — it has to be one
of the two, and the same one every time (ADR-0006).

Marks collect into `Expansion::anchors` (a `BTreeMap`, keyed by exported name),
**not** into the `VoxelModel`: a mark writes no blocks, and folding metadata into
the block grid would change what `canonical_bytes` means. The export writes them
into the prefab metadata's `anchors` map in the hand-built `{pos, facing}` shape,
`pos` local to the structure; `PrefabRegistry` reads a grammar prefab's anchors
with the same code path as a hand-built one (`crates/compiler/tests/grammar_prefab.rs`).

Refusals: a non-kebab stem is a `Program::validate` error (before any expansion);
a mark aimed outside its own scope, an underivable facing, and two marks
producing the same name are expansion errors — the collision names both rules.
Two marks on the same **cell** under different names are legal, as in the
hand-built prefabs.

## 3. Determinism (ADR-0006)

Same program + same region + same seed → byte-identical `VoxelModel`, asserted by
a double-expand test over every library program at five seeds, plus a
seed-sensitivity test over a probabilistic program, and over the declared
anchors (names, cells and per-stem numbering alike). All randomness is one
splitmix64 stream from the caller's seed; all maps are `BTreeMap`; cells iterate
`x`, then `y`, then `z`; nothing reads the clock, the environment or a path.
`VoxelModel::canonical_bytes` is the comparison/hash form.

Expansion holds no global state — two programs cannot influence each other, which
is regression-tested.

The same promise is asserted one layer out, on the bytes that actually ship: a
double-**export** test over every library program at four seeds compares the
`.nbt` and the metadata JSON byte for byte (§6).

## 4. Failure is loud

The interpreter has no silent degradation. `Program::validate` runs before any
expansion (unknown rule/role/param, empty rule or split, child/piece mismatch on
a non-repeating split, zero weights, an `orientation` guard that is not a
permutation — a guard nothing could ever match — and a `mark` whose anchor stem
is not kebab-case). During expansion: `NoApplicableRule`,
`Split{Overflow|ZeroStride}`, `Orient`, `BadSize`, `Eval`, `PaletteFull` (more
than 65 536 distinct block states in one model), `MarkOutsideScope`,
`MarkFacingNotCardinal`, `AnchorCollision`, and the `DepthLimit` / `ScopeLimit` /
`VolumeLimit` budgets. Errors carry the rule name and print as prose, never as a
`Debug` struct.

The three budgets live on `Limits` and are inputs, never silent clamps:
`max_depth` and `max_scopes` turn an unguarded recursive rule into a diagnostic
instead of a hang; `max_volume` (default 2²⁴ cells) is checked *before* the dense
model is allocated, so an absurd region is an error rather than an OOM kill —
the one failure mode that reports nothing at all. A caller who means to build
something enormous raises the limit explicitly.

Writing outside the model's own region is a caller defect, not an input:
`VoxelModel::set` asserts it in a debug build and drops the write (never wraps
it) in a release one.

These are Rust error values, **not** `DW` diagnostics: the craft-rule diagnostics
of spec-0027 §4 are a later phase and will own a DW range then.

Consequence for authors: a region too small for a program's absolute sizes is an
error, not a building with pieces outside its box. Each library program documents
its minimum region.

## 5. Rule library — ported buildings

Ported from `yawgmoth/GDMC25` (BSD-3-Clause; see
[`ACKNOWLEDGEMENTS.md`](../ACKNOWLEDGEMENTS.md)) and reachable as
`library::{temple, castle, church}`.

| Program | Controls | Smallest region that expands (measured) |
|---|---|---|
| `temple` | `roof` (pitched/flat/capped/open), `column_height`, `column_size`; role `marble` | X ≥ `6 + 2*column_size`, Y ≥ `1 + column_height + roof height` (5 pitched / 3 flat / 1 capped / 0 open), Z ≥ 7 |
| `castle` | `large_tower`, `small_tower`, `great_hall`, `wall_height`, `wall_width`, `tower_height`; role `stone`; declares `anchor/courtyard` | both horizontal extents ≥ `2*large_tower + 2`, Y ≥ `tower_height + 1` |
| `church` | guards only; roles `wall`, `glass`, four `roof_*` stair facings, two door pairs | height must follow width (the roof steps in 2 per course): Y ≥ 9 and Y ≳ X − 3; 15 × 16 × 30 is comfortable |

Ports are faithful except where a module says otherwise; the three substantive
divergences are recorded at their code: the temple's colonnade repeats to fit the
box instead of being fixed at four columns, the church's one-wide ridge course is
guarded (upstream splits it anyway and writes outside the region), and
constraint `largest` returns the maximum (upstream returned the minimum — a
copy/paste bug).

Two library claims are pinned by arithmetic rather than prose, because prose
rots: the temple's colonnade really does follow the box, and a nine-deep box at
`column_size` 1 really does give upstream's four columns
(`tests/library.rs`).

## 5b. Staging vocabulary — original rules with gates

`library::{cliff_path, watch_bay, rafter_hall, ambush_door, store_room}` are
**original Delvewright content** (licence `original`; nothing ported, no ledger
entry owed). They are the drowned-bell remake's grammar vocabulary — W1 (path
and hazard geometry) and W2 (interior ambush) — and they are a different kind of
rule from §5: a temple is judged by looking at it, these are judged by a
**machine gate about how the space plays**. Every gate below is an assertion in
`crates/grammar/tests/staging.rs` over the expanded model, and each has been
shown to go red when the geometry is wrong.

### The W1 local frame

All five rules share one frame, and it is not arbitrary:

> **Local `Y` is up. Local `Z`-max is the approach end; travel runs toward local
> `Z`-min.** Length is turned onto the box's longer horizontal axis.

A `mark` with no explicit `facing` derives one, and the derived facing is
*always* the negative direction of the world axis the scope calls local `Z`
(§2b) — a rule can only aim an anchor down-axis. Travel is defined that way so
every derived facing points at the thing its anchor is about. The price is that
**indexed anchors number against travel** (a split visits its pieces low to
high, so declaration order is fixed by the axis while the facing points the
other way down it): `anchor/niche-1` is the *last* niche the player meets. With
`mark` as it stands the two cannot both follow travel, and a wrong facing is
wrong *data* where a numbering convention is only documentation. See §7 for the
primitive that would remove the trade-off.

### `cliff_path` — the knockback niche

A one-wide ledge along a drop face, with 1-deep × 2-high recesses cut into the
inner wall every `spacing_min ..= spacing_min + 3` cells (a uniform seeded draw
over whichever of the four spacings the remaining path has room for).

| | |
|---|---|
| Controls | `spacing_min` (6), `niche_height` (2), `watch_back` (3); roles `rock`, `corpse` |
| Smallest region | 3 × (`niche_height` + 2) × 3, and at least as long as it is wide |
| Anchors | `anchor/niche-<i>` — inside each recess, facing the ledge (derived through a `reorient` that names the across-path axis as local `Z`; this is why the ledge is at local `X`-min). `anchor/niche-watch-<i>` — a ledge cell `watch_back` up-path, facing down-path. |
| Variants | weighted alternatives per slot: teach (2) — one recess with a corpse prop, no occupant; test (3) — one empty recess; twist (1) — two adjacent recesses, each with its own anchor pair |

Gates:

1. **The recess is exactly one deep** — air at the anchor cell and the cell
   above, backing wall immediately behind, floor below, lintel above, ledge in
   front. An occupant's hitbox sits inside it and a swing from the ledge reaches
   it; one deeper and the niche becomes a room the player must enter.
2. **The ledge is the only route** — the standable-cell graph connects the two
   ends, and with the `X`-min lane deleted it does *not*. A recess beside a wide
   path is decoration. The test also asserts every standable cell is either the
   lane or a declared recess, so the lane is one wide by measurement.
3. **Each watch cell sees its niche's mouth** — the same sightline walk as
   `watch_bay` below, to the ledge cell the recess opens onto. Deliberately
   *not* into the recess: a 1-deep recess off a 1-wide ledge is geometrically
   invisible from anywhere down the path, which is precisely what makes it an
   ambush. The legible thing is the contested ground.

### `watch_bay` — observability hardware

A gated passage whose approach end carries a roofed 2×2 bay, walled on three
sides, open only toward the hazard span, with the lane running past it.

| | |
|---|---|
| Controls | `approach` (8), `span` (3), `head` (4), `bay_height` (2), `obstruct` (0 — a test knob) ; role `stone` |
| Smallest region | 6 × (`head` + 2) × (`approach` + `span` + 4) |
| Anchors | `anchor/watch` — the bay cell nearest its open face, facing the span. `anchor/gate` — the span's floor centre, for the campaign's `timed-gate` / `volley`. |

Gates:

1. **Standoff** — `approach` ≥ 6 is enforced *by the rule*: the plan has one
   guarded alternative and no `otherwise`, so a shorter approach is a
   `NoApplicableRule` refusal, not a quietly smaller bay. Measured watch-to-span
   Chebyshev distance is `approach + 1`.
2. **Sightline to every standable span cell**, walked with the same
   Amanatides–Woo traversal and the same eye/centre-mass heights the compiler's
   `DW0388` uses (1.62 / 1.0, both endpoint cells exempt). Deliberately stronger
   than `DW0388`, which asks for sight to *some* cell at 5: the point of
   generating the bay is that the campaign-level proof cannot then fail. That
   proof still runs later, on the assembled world with real hazard declarations
   — what this rule guarantees is that the bay it places **can** satisfy it.
3. **The gate has teeth** — `obstruct = 1` stands one pillar in the bay's own
   column of the approach and the sightline check must go red, while the passage
   stays walkable (so what was caught is blindness, not impassability).

### `rafter_hall` — the rafter perch

A hall whose truss layer is somewhere a body waits. At `h-3` a course of
**corbels** carries `bracket` cells in from each side wall; at `h-2`, over each
corbel's inner end, is a standable cell. Slices repeat every `beam_period` along
the hall and alternate which side is declared a perch.

| | |
|---|---|
| Controls | `beam_period` (4), `bracket` (2), `span_beams` (0 — a test knob); roles `stone`, `timber` |
| Smallest region | the density cap ties width to length, so this is a curve, not a triple: interior `X · Z · period ≥ 24·Z + 24·period`, plus `X ≥ 2·bracket + 3`, `Y ≥ 6`, `Z ≥ period`. 10 × 6 × 12 is the smallest trussed hall at the defaults, pinned from both sides in `tests/staging.rs` |
| Anchors | `anchor/perch-<i>` — a corbel's inner cell. `anchor/hall-door` — the floor cell at the centre of the approach end, which is where the sightline gate stands |
| Variants | `Y < 6` is a **hall with no truss**: same shell, same door anchor, no perches, and not an error. Both shapes are asserted |

**The centre span is open on purpose.** The obvious full-span truss fails gate 1
and cannot be tuned into passing it: an eye on the floor is below the beam plane
and a perch is above it, so every ray crosses the plane over a run of about
`0.42 × distance` cells, and past ~9 cells of hall that run always contains a
nearer beam. A spanning truss hides its own far rafters. Corbels leave the nave
clear, and a ray to any perch crosses the beam plane while still only 16–58% of
the way to that perch's wall — inside the open span at every hall length.

Gates:

1. **Every perch is visible from `anchor/hall-door`**, walked with the same
   Amanatides–Woo traversal as `watch_bay`. Fairness in the souls grammar is
   carried by silhouette, not by sound (`docs/notes/souls-design-language.md`
   §4.3). Teeth: `span_beams = 1` rebuilds the full-span truss and 6 of the
   fixture's 7 perches go blind (0 of 7 at the default), while the nave stays
   walkable end to end — so what was caught is blindness, not a severed hall.
2. **At most one perch per 24 floor cells**, the smallest machine-checkable form
   of the Cathedral's monoculture critique (§4.1). Enforced *by the rule*: the
   cap is arithmetic in the guard, so a hall too narrow for its own rafters is a
   `NoApplicableRule` refusal. Teeth: the fixture at 8 wide would carry the same
   7 rafters over 150 floor cells — a genuine cap breach — and is refused.
3. **The rafters are geometry.** Every perch is standable on timber with
   headroom, and so is the next cell of the same beam. Two red sides in the same
   test: the centre of the nave is *not* standable at rafter height (or the
   truss would be a floor), and no walk from the ground reaches a perch (or it
   would be a mezzanine).

### `ambush_door` — the corner-ambush alcove

A wall across the box with one 1-wide opening, and immediately inside it, one
cell to the `+X` side, a blind pocket.

| | |
|---|---|
| Controls | `head` (3), `door_height` (2), `door_offset` (2), `expose` (0 — a test knob); role `stone` |
| Smallest region | `door_offset + 5 + expose` across, `head + 2` tall, 5 long — and at least as long as it is wide. 7 × 5 × 7 at the defaults |
| Anchors | `anchor/alcove` — the blind cell, facing the door lane (derived through a `reorient` naming the across-wall axis as local `Z`, which is why the alcove is on the `+X` side). `anchor/threshold` — the standable cell in the opening, facing the way the player walks through |

Gates:

1. **The alcove is blind from the approach** — the vocabulary's first
   *negative*-visibility gate, asserted cell by cell over every standable
   approach cell (54 in the fixture, 0 of which see it). Teeth: `expose = 1`
   widens the opening over the alcove's own lane and 29 of the 54 see it.
2. **One swing from the doorway's inside cell** — Chebyshev distance exactly 1,
   swept over `door_offset` so the adjacency is arranged rather than coincident
   with the default.
3. **The doorway is the only route** — the standable-cell graph connects
   approach to inside, and with the doorway's column deleted it does not (the
   same cut `cliff_path` uses on its ledge lane).

A blind alcove is *not* discoverable from the decision point, which §4.2 of the
dossier calls the unfair kind. That is deliberate and it is the **test** rung of
teach/test/twist: the rule declares `anchor/threshold` so a campaign has
somewhere to hang the telegraph that pays for it, and it does not pretend the
pocket is its own tell.

### `store_room` — the container tell

A storeroom whose far wall carries a row of barrels with exactly one `unbanded`
variant among them.

| | |
|---|---|
| Controls | none (the row is as long as the box); roles `stone`, `barrel`, `barrel_unbanded` |
| Smallest region | 5 × 5 × 3 — three barrels is the shortest row in which the odd one always has a neighbour |
| Anchors | `anchor/store-line` — the barrel at the approach end of the row. `anchor/tell` — the odd barrel's own cell, facing out into the room (hence the row sits at `X`-max) |

**Exactly one, without a counter.** A rule has no memory, so the invariant is in
the derivation's shape: `line_before_tell` either lays a plain barrel and
recurses or spends its draw and hands the rest to `line_after_tell`, a plain
fill that can never produce another; with one cell left neither guarded
alternative applies and the `otherwise` places the tell outright. The two
guarded alternatives overlap on purpose — that overlap *is* the position
distribution (§2), weighted 3:1 toward carrying on.

Gates:

1. **Exactly one tell, and the anchor is on it** — counted off the *blocks* over
   12 seeds, not off the anchors, with the rest of the row asserted to be plain
   barrels so "exactly one" is not an artefact of a one-cell row.
2. **The tell is in the line** — a barrel beside it on at least one side, and the
   row runs the whole lane.
3. **The tell moves with the seed** — 12 seeds put it in ≥ 3 distinct cells (9 in
   practice). A fixed tell is a landmark players learn once.

The default binding keeps both roles one material family and changes only the
variant: a spruce barrel and a spruce log, the same wood with its iron bands
missing. `barrel[open=true]` would be the neater mimic-breath pun and is not
usable — vanilla closes the lid the moment the structure loads, so the tell
would last exactly as long as nobody looked.

All five programs are in the generic library suites too: structural validity,
JSON round trip, palette-swap-moves-no-block over **every** role each binds, and
the double-expand determinism gate over model bytes *and* anchors
(`tests/library.rs`, `tests/determinism.rs`). Their anchors — including generated
`-<i>` names nobody hand-listed, and `store_room`'s seeded tell position —
round-trip through `PrefabRegistry` (`crates/compiler/tests/grammar_prefab.rs`).

### `boulder_stair` — the worn-tread tell (W), and the side pockets (S)

A hazard lane whose centre course takes a `smooth`-variant material down its
length while the side lanes keep the `rough` variant — a Sen's Fortress
telegraph built as paint, not shape. Every `pocket_period` cells its near-side
wall opens into a one-cell dodge.

| | |
|---|---|
| Controls | `head` (4), `pocket_height` (2), `pocket_period` (8); roles `rough`, `smooth` |
| Smallest region | `MIN_X` (5) × (`head` + 2) × `MIN_DEPTH` (= `MIN_X`, since the frame always makes local `Z` the *larger* of the two horizontal extents — a documented depth under the width minimum could never be reached) |
| Anchors | `anchor/stair-run` — the run's floor centre. `anchor/volley-slot` — the vault rib directly over the run's midpoint, for a dart trap; ordinary stone until a campaign binds it (§7: trap anchors are not yet expressible by a rule). `anchor/pocket-<i>` — each dodge, facing the lane |

**S has no grammar of its own in the vocabulary doc.** "Side `alcove` splits
every 8 units (entry S safe pockets)" is the only place the pockets are
described, and it is inside W's own entry — the dispatch line's "W3 = W+S+M+X"
is the only place S reads as a fourth peer letter. What is built is a properly
named, fully gated rule (`pocket_niche`) *inside* `boulder_stair`, not a second
exported program (the IR has no cross-program `call`, so a standalone
`safe_pocket` program could not literally be what this rule's own split uses).
This is filed as an open question for the planner, not decided here.

Gates:

1. **The tread is exactly one material family, at two distress levels** — the
   spec-0027 §4 palette-role budget's own claim, proved against a **test-local
   mirror** of that not-yet-built diagnostic (§7 below; `crates/grammar/src/lib.rs`'s
   own "not built yet" note), scoped to the lane's own floor course. Teeth:
   read the same cells without the family fold and the smooth run's raw share
   genuinely clears the 10% accent ceiling — so the fold is load-bearing, not
   vacuous — and restyling the run to an unrelated material is still correctly
   caught as a genuine accent overrun under the same grouped reading.
2. **The lane is the only continuous route** — the same cut `cliff_path` and
   `ambush_door` use, here on the pocket band: it is solid everywhere except at
   pocket slots, so it cannot substitute for the lane end to end.
3. **A pocket is a one-cell dodge, visible from the lane** — standable, one
   deep, backed and lintelled like `cliff_path`'s niche, but (unlike
   `ambush_door`'s alcove) *not* blind: a dodge nobody can see coming is not an
   escape.
4. **`pocket_period` is a real control**, the same claim `cliff_path` makes for
   `spacing_min`: widening it thins the pockets out.

A box shorter than one `pocket_period` cannot tile even one pocket
(`make_split` checks the un-repeated pattern before it tiles) — the same shape
`rafter_hall` uses for a hall too short for its truss, a plain lane and no
`anchor/pocket-*` is a variant, not an error.

### `threshold_motif` — the boss-door threshold motif (M)

A doorway spanning the box's whole interior width, hung above walking height
with a bell-rope curtain tiled by `split_repeat` — so a motif taught in one
zone and rebuilt wider for another keeps the same strand density without being
retuned per size.

| | |
|---|---|
| Controls | `head` (4), `curtain_height` (2), `strand_period` (1), `single_strand` (0 — a test knob); roles `stone`, `curtain` |
| Smallest region | 3 × (`head` + 2) × 3 — and `head` ≥ `curtain_height` + 2, so there are two full cells of walk clearance under the curtain (one is not enough for `standable`, which also asks for the cell above the player's head) |
| Anchors | `anchor/threshold-narrate` — the doorband's floor centre (`FloorCenter`, so it re-centres at any width), for the beat taught once and cued again elsewhere |

Gates:

1. **Curtain density holds across box sizes** — the entry's whole reason to
   exist: a narrow and a wide doorway carry proportionally more strands, not a
   thinner spread. Teeth: `single_strand` collapses the curtain to one strand
   regardless of width, which is exactly what "the motif degrading" means, and
   the density check catches it at both sizes.
2. **The doorway is walkable beneath the curtain** — the band sits entirely
   above the walk clearance, so the passage connects end to end with or
   without the degrading knob.

### `broken_grate` — the broken grate (X)

A wall's own vent row, walked the same state-machine `store_room` uses for its
barrel line — `line_before_tell` either lays a plain grate and recurses or
spends its draw and hands the rest to `line_after_tell` — so every derivation
breaks exactly one grate cell, applied to a wall band instead of a floor row.

| | |
|---|---|
| Controls | `head` (3), `grate_height` (2); roles `stone`, `grate`, `grate_broken` |
| Smallest region | 3 × (`head` + 2) × `MIN_LINE` (3) — the same "three is the shortest row the odd one always has a neighbour in" proof `store_room` makes |
| Anchors | `anchor/grate-secret` — the broken cell, facing out into the room across the row |

Gates:

1. **Exactly one break, and the anchor is on it** — counted off the blocks'
   `(x, z)` over 12 seeds (a break is `grate_height` courses tall, so it is
   several blocks at one row position, not several breaks), with the rest of
   the row asserted to be plain grates.
2. **The break is in the row** — a plain grate beside it on at least one side.
3. **The break moves with the seed** — 12 seeds put it in ≥ 3 distinct places.
4. **The distress variant is not counted as an accent** — the same
   §4-diagnostic-mirror claim `boulder_stair` makes, over the row's own cells:
   an ungrouped reading of the same short row genuinely clears the accent
   ceiling, the family-grouped reading does not, and restyling the break to an
   unrelated material is still correctly caught.

`boulder_stair`, `threshold_motif` and `broken_grate` are in the generic
library suites too, the same promises as the five above: structural validity,
JSON round trip, palette-swap-moves-no-block over every role, double-expand
determinism over bytes and anchors, and their anchors — including
`boulder_stair`'s generated `pocket-<i>` names and `broken_grate`'s seeded
break position — round-trip through `PrefabRegistry`.

## 6. Export — freezing an expansion as a prefab

`export::export_prefab(program, region, options, id)` produces the two files a
prefab library holds: `<id>.nbt` (a vanilla structure template) and `<id>.json`
beside it. It takes the *program*, not a finished model, and expands it itself —
which is what makes the provenance row unforgeable, since the hash and seed in
the metadata cannot describe a different expansion than the one that produced
the bytes.

The `.nbt` comes from `delvewright-schem`'s `build_region`, the emitter the
`.schem` asset pipeline already uses: one structure writer, one set of
determinism guarantees (sorted palette, `x`→`y`→`z` cell order, gzip mtime 0).
A structure template is local-coordinate, so the region's **origin** does not
reach the output; its **size** does, and is the declared `structure.size`.

The metadata is the hand-built shape, minus what expansion cannot know:

```json
{
  "prefab_id": "prefab/grammar-temple",
  "structure": { "file": "grammar-temple.nbt", "id": "grammar-temple",
                 "size": [13, 14, 21], "data_version": 4671,
                 "generator": "crates/grammar" },
  "anchors": {},
  "lighting": { "profile": "unmeasured" },
  "license": { "source": "original", "spdx": "GPL-3.0-or-later",
               "note": "…", "provenance": "…",
               "generated_by": { "generator": "grammar", "program": "temple",
                                 "program_hash": "sha256:…", "seed": 7 } }
}
```

- **`generated_by`** is the spec-0027 §2 provenance row. `program_hash` is
  `sha256` over the program's canonical serde JSON bytes — content-addressed, so
  a program built in Rust and the same program parsed from JSON hash alike.
- **`anchors`** is exactly what the program's `mark` declarations produced (§2b),
  in the hand-built `{pos, facing}` shape with `pos` local to the structure.
  Nothing infers one from the block pattern afterwards — that is precisely the
  downstream folklore the no-hack rule forbids — so a program that marks nothing
  exports `{}`, as the temple above does, and an anchorless prefab loads and
  indexes normally. The castle, which marks, exports
  `"anchors": { "anchor/courtyard": { "pos": [20, 0, 12], "facing": "north" } }`
  over its 41×14×25 region.
- **No `connectors` key.** Jigsaw socketing of grammar prefabs waits on the
  tileset conventions; a guessed socket is worse than none.
- **`"profile": "unmeasured"`.** A lighting profile is a *measurement*, taken by
  the live 1.21.11 probe. Expansion places blocks, not photons, so it declares
  the true thing and admission to a campaign still runs the probe. `unmeasured`
  is not a synonym for an absent `lighting` block: absence means legacy metadata
  predating the field, this is a positive statement that a measurement is owed.
  A `lit`/`dim`/`dark` declaration still cannot omit `measured_min_light` /
  `measured`, and an `unmeasured` one may not carry them (`delvewright-dsl`
  refuses both at parse).

Refusals, all loud: an `id` that is not a lowercase-kebab path segment, an empty
region, a region past the vanilla 48-per-axis structure cap (tiling a prefab
into parts is a jigsaw design, not an export detail), and a model containing a
block the structure safety strip would replace with air — a grammar that asked
for a command block meant to, so shipping a silent hole is refused instead.

`PrefabRegistry` (the engine's reader) loads the result with no diagnostics;
`crates/compiler/tests/grammar_prefab.rs` tests that seam from both sides.

## 7. Not built yet

The §4 craft diagnostics, jigsaw connector emission, the JSON schema stage in
front of the IR, and the contact-sheet/curation loop. Later phases of spec-0027.

`mark` declares point anchors only. Gate-region anchors (`region` + `block`),
trap anchors (`dispenser`, `trigger_block`) and the entry names the engine
treats specially (`spawn`, `entry`) are expressible in prefab metadata but not
yet by a rule — each needs its own declaration, not a widened `mark`.

**A facing a rule cannot ask for.** A derived facing is the negative direction of
the world axis the scope calls local `Z`, and an explicit `facing` is a *world*
cardinal, so a rule that is reused under rotation cannot say "look the way my
local `+X` points". Since a split also always visits its pieces low-to-high
along that same axis, "anchors numbered in travel order **and** facing along
travel" is not expressible — §5b pays for the facings with the numbering. The
smallest primitive that would remove the trade-off is a **local-direction facing
spec** on `Mark::facing` (`local_x_min` / `local_x_max` / `local_z_min` /
`local_z_max`, resolved through the scope's orientation at expansion, exactly as
`AxisSpec` already resolves an axis name). It is additive, needs no new node
kind, and would let a rule aim an anchor in any of the four cardinals whichever
way the piece was turned.

**W2 met the same wall, and paid differently.** `rafter_hall`'s perches
alternate between the two side walls; both should look *across* at the nave, and
that is a facing along local `+X` for the left corbel and `-X` for the right.
Only the second is expressible, so both take the derived down-hall facing
instead — an occupant watching the ground the player is walking into, which is a
defensible reading of the move but not the one the geometry asked for. The
workarounds available inside the current IR are all worse than the shortfall:
put perches on one wall only (monoculture, the exact thing the entry's density
cap exists to prevent), or declare a *world* cardinal (which breaks the moment
the frame's `Largest` turns the piece 90°). `store_room` dodged it by choosing
which wall the barrel row sits against, which works for one row and does not
generalise to a rule with two symmetric sides.

That is now two worked examples with the same shape and one rule that only
avoided it by luck of layout. Still not built here — the red line for W2 was
compose-existing-verbs-only — but the case is no longer thin.
