# spec-0087: A body the size of a hill — an organic giant is sculpted from a declared form, lies on its own ground, and is walked on and inside by the proofs the engine already has

- **Status**: Approved
- **Ground**: written against engine `ce3f7fe7` (`origin/main`), read only — `World::neighbors_fp` and `World::body_moves` in `crates/delvec/src/compiler/nav.rs`; `ground_entry`, `standable_cells` and `reachable_from` in `crates/delvec/src/schem/nav.rs`; `step_allowed`, `MAX_AUTO_STEP_16`, `MAX_JUMP_RISE_16` and `keep_out_box` in `crates/dsl/src/metrics.rs`; `LethalVolume` and `Guard` in `crates/dsl/src/stages.rs`; `PrefabMeta`, `License` and `GeneratedBy` in `crates/dsl/src/prefab.rs`; `convert::build_region` and `stairs::derive_shape` in `crates/delvec/src/schem/`; the pass order, the prefab-metadata section and the rows for `DW0210`, `DW0322`, `DW0345`, `DW0710`, `DW0751`, `DW0752`, `DW0801`, `DW0885`, `DW0886`, `DW0891`, `DW0894` and `DW0921` in `docs/reference/compiler.md`; `docs/reference/prefab-procedure.md` §0. The research it builds on is `docs/reference/organic-structures.md` and `tools/spike-organic-voxel/`, read whole, with its path-traced comparison sheet looked at. The coverage map `docs/reference/dsl-coverage.md` on branch `docs/dsl-coverage` at `9f3f2935` was read and every claim this spec leans on was re-read on `ce3f7fe7` (§2.3). One rig was run for this spec, `tools/spike-organic-stranded/`, against a release build of `ce3f7fe7` (cargo exit 0 asserted); its record is `tools/spike-organic-stranded/observations.json`.
- **What it is for**: a creator must be able to build a hill-sized organic body — a stranded creature of about 140 × 60 × 45 blocks — that players walk on and inside. The grammar back end has no smooth curve, no diagonal and no noise by design, and the one route the procedure offers for such a form is "in-house generator work": an engine task, not an authoring step. The campaign record that needs the body names four engine needs, stated here without its story: the body as a prefab, generated from a declared form, deterministic, licensed as original; walk and light proofs over the inside and the back; climbing and jumping where a route needs them; a killing volume live only from a story stage on. This spec delivers the first two. §6 names the other two, and what else was found, as follow-ups with their dependencies, and says why they are not this spec.
- **Research**: §2 is this spec's research record. Each statement is marked **cited** (a source file at the named revision, a published source, a measurement in the tree or in the rig's record, or a constitution rule) or **authored** (this spec chooses).
- **Numbers**: spec 0087. The diagnostics are `DW_ORGANIC_FORM` = `DW0951` and `DW_ORGANIC_BODY` = `DW0952`, allocated by the planner. `dsl_version` does not move: the form document is a library-asset document beside prefab metadata, exported by `delvec schema --stage sculpt-form` and deliberately absent from `--stage all` (the rule `prefab-metadata` already follows), and no stage document changes shape.
- **Non-goals**: a mesh as the form (the research's second front end; it needs a provenance record and the ADR-0013 licence gate at the input, and is follow-up E); a body that floats (follow-up A); a `clouds` surround (follow-up D); climbing and gap-jumping as route moves (follow-up B); a lethal volume gated by story stage (follow-up C); any campaign content, including the stand-in massing a campaign may place until a sculpted body exists; the look of a body's own interior light, which this spec leaves to a demo level (§4.4).

## 1. The defect, and the object it belongs to

**Cited**: `docs/reference/prefab-procedure.md` §0 routes "genuinely un-statable by axis-aligned boxes: a smooth curve, a diagonal, a profile whose step varies independently of the box, a vault bending on two axes at once, or noise/terrain" to **in-house generator work**, "an engine task, not an authoring step". The research record (`organic-structures.md` §1) measured what the grammar produces when asked anyway: a 128-block skeleton whose curves came out as one-block stair-step corrugation and whose skull read as a ridged box, with no walk proof because the instruments seed from grade and the piece floated.

**Cited** (research §3, §7): a fitter that samples each block as 2 × 2 × 2 octants and chooses air, a full block, a slab or a straight stair by least occupancy error removes the corrugation; vanilla derives the stair corners, so `DW0801` holds by construction; every piece it produced passed `delvec prefab audit` with stairs examined equal to stairs written; the pieces are deterministic, rebuilt and compared by hash. A stylised implicit form — tapered capsules, ellipsoids and discs stated as numbers — reads as the thing at a glance where a faithful scan does not, and what it still lacks is design work, not capability.

**Authored — the object.** The capability is a **producer of prefabs**: a second back end beside the grammar, taking a declared form and writing the ordinary prefab a campaign binds — structure parts and a metadata document — so that every gate downstream of admission is unchanged and nothing in the DSL's stage documents learns a new word. It is a subcommand of the one binary (**cited**: ADR-0023), `delvec sculpt`. It writes only what it can prove about its own output, in the same way the grammar back end does: the always-on gates run at generation, and the measurements the piece owes — `walk_y`, `shown_faces`, the anchors — are read back off the blocks it just laid, never typed (**cited**: `compiler.md`, prefab metadata: "a MEASUREMENT of the piece, written by the generator that built it").

## 2. Research record

### 2.1 What the research settled (cited, `docs/reference/organic-structures.md`)

- **No licence-clean tool fits sub-block shapes.** Of the prior art surveyed (research §2), the one portable voxeliser places full blocks only and dithers with an unseeded RNG; the tools that do fit stairs and slabs are paid, closed or unpublished. The octant fit is therefore authored, and is the engine's to port.
- **The back end is seven steps**: octant fit; thin-plate refit where the fit says air; stair corners by `delvec::schem::stairs::derive_shape`; a shading tone from the smoothed surface normal, openness and low-frequency seeded noise; a palette family picked by tone; `bone_block` axis by the longest local run; output as structure parts. The research's Rust-port sketch (§6) names seeded PCG for the one RNG.
- **Scale**: the reference image's creature measures 280–350 blocks long; below about 220 a 2.6-block rib cannot keep a 3-block gap. A 140-block body is a different animal at a different scale, and this spec takes no rule from those numbers beyond the one that is general: **the input decides the read** — a form is designed for its scale, in blocks, and the fitter reproduces it.
- **What the research could not do**, on a *floating* skeleton (research §6, §7): the light probe refused with `DW0752` (no ground-level entrance on any vertical face) or measured from whichever bone tip touched a side face; `delvec prefab planes` measured `walk_y` off three flipper-tip cells; the build refused with `DW0322` (void drops), `DW0886` (blocks on box faces over a void with no `shown_faces`) and `DW0921` (157 soft-lock places); `delvec schem convert` refuses a schematic over 10,000,000 cells (`DW0710`), so the 448-block piece was written as z-slabs. The research named these as needs: instruments seeded from a declared entry, a part writer past the schematic cap, a `clouds` surround, and the craft decisions behind the three correct refusals.

### 2.2 What the rig measured on a stranded body (cited, `tools/spike-organic-stranded/observations.json`)

The rig states a stranded body as implicit primitives over the research's own fitter (`tools/spike-organic-voxel/`, run unchanged): a torso, head, tail and fluke resting on a four-course **apron** of `packed_mud` that fills the piece's footprint, one interior cavity whose floor is the apron, one wound at ground level through the west flank into it, one ramp cut into the east flank rising half a block per block onto the back, and one blowhole through the back. Box 49 × 62 × 146, 443,548 cells, 131,393 of them filled (118,190 full, 4,747 slab, 8,456 stair), one connected component, no island dropped. Instrument: a release build of `ce3f7fe7`. Deterministic: the same `.schem` hash on every run of the same form (`7daaf265…`), and the earlier form's own hash on both of its runs.

| Instrument | Reading | What it says for this spec |
|---|---|---|
| `delvec schem convert` | tiled 2 × 2 × 4 into 16 parts (`DW0701`, advisory) | a 140-class body is 4.4% of the schematic cap and needs no slab writing; the cap is a fact about the `.schem` reader, which §3.6 takes off the path anyway |
| `delvec prefab audit` | `pass`, stairs examined 8,456 of 8,456 written, 0 underspecified | the fitter's output is admissible as it stands |
| `delvec prefab planes --write` | `walk_y: 4`; 5,460 of 11,108 standable cells stand on that plane | **the ground in the piece is the walk plane**, measured by the one rule every generator uses (lowest standable local y) |
| `delvec prefab lighting` | bound **386 entry cells** at grade; **9,768 of 11,108** standable cells measured; `admits_sky: true`; 3,445 (35.3%) below light 3 at a clear night, 0 under full daylight (`DW0751`, advisory) | **the probe seeds from grade when the ground is in the piece** — the research's `DW0752` is a floating-piece finding, not an organic-body finding — and it reads the cavity through the wound and the blowhole |
| `delvec build`, as authored | exit 2, `DW0210`: 859 of 9,769 reachable walkable cells below light 3 | the engine checks the body's light exactly as it checks a room's; the body carries no light, and the finding is correct |
| `delvec build`, the light gate stood down for the rig only (`mitigation: night-vision` on the stub's area — a rig device, never a remedy) | exit 3; the critical path **walked** entry → inside → back (210 route cells, no `DW0311`); `DW0921` refused **38 places, 110 cells** a body gets into and cannot leave, of 10,931 it can reach | **the route proofs already walk inside and onto the back** once the way is built as steps; what refuses is the hull: ledges under the body's widest line that a body reaches by walking down the flank and cannot leave because the drop below is past the survivable fall |

Two consequences. First, **the second engine need — walk and light proofs on non-ground surfaces — is met for a stranded body by one shape rule, not by new instruments: the ground the body lies on is part of its piece.** That is the site shape spec-0060 already describes (a thing together with its ground, inside one box), and every instrument then binds from grade. The research's entry-seeded instruments remain the need of a body with no ground under it, which is follow-up A. Second, **the hull owes the leave proof, and it owes it at generation**: a creator who learns of 38 pockets at the campaign's build has spent a build to learn what the generator could have said over the piece alone.

### 2.3 The coverage map's claims, re-read on `ce3f7fe7`

| Row | The map says | On `ce3f7fe7` |
|---|---|---|
| M04 | `LethalVolume = {id, region, message, damage_type, shown_by}`, no gate, always live | **Holds.** `stages.rs` declares exactly those five fields; `DW0891` is raised "once per volume over the final assembled world" (`compiler.md`). |
| M05 | vertical climbing is not a move | **Holds.** `neighbors_fp` admits cardinal steps of `{0, −1, +1}` cells gated by `step_allowed`; `body_moves` says in its own words that "diagonal jumps and climbing (ladders, vines) are not moves here". |
| M06 | jumping across a gap is not a move | **Holds for every route proof; one nuance.** `World::body_moves` — the relation `DW0921` floods — does model a jump across a gap inside `jump_max_gap` for its rise, with constants measured by `tools/spike-jump-arc/simulate.mjs`; "a route proof may not use any of this: it must never prove a way the bot cannot walk on cue". So the body model knows the move and the route proofs refuse it by design. |
| M07 | the voxel nav admits −1 only | **Holds for routes**; `body_moves` admits a fall to the first standable floor no deeper than `unarmoured_survivable_fall_blocks` (22). |
| M01 | `HorizonBase` = `void`/`ocean`/`valley`; the sky base is Proposed and unbuilt | **Holds**; nothing under `crates/delvec/src` emits a cloud. |

A rise of one full block is a `Jump` under `classify_rise_16` (16/16 > 9/16) and is a route move when the head cell is clear; a half-block rise is a `Walk`. That is why the rig's ramp rises half a block per block: the fitter turns a continuous slope into slabs and stairs, and the route proof walks it as a stair.

## 3. The surface

### 3.1 The command

```
delvec sculpt <form.json> -o <dir> [--seed N] [--id <prefab-id>]
delvec schema --stage sculpt-form
```

**Authored**, on `delvec grammar expand`'s shape. `--seed` is the expansion seed and is recorded, as the grammar records it, in the metadata's `generated_by` (`generator: "sculpt"`, `program`: the form's `id`, `program_hash`: `sha256:` over the canonical JSON of the form as sculpted, `seed`, `region`: the box). Same form, same seed, same engine: byte-identical parts and metadata (ADR-0006), asserted by building twice. The output is written to `<dir>` as `<id>.nbt` and `<id>.json` when every axis is at most 48, else as parts `<id>.x<i>y<j>z<k>.nbt` under a `structure_set` manifest — the two packagings `PrefabMeta::from_json` already reads and no third.

### 3.2 The form document

**Authored.** A schema-checked JSON document (`deny_unknown_fields`, as every document of this engine). Its fields, and what each is for:

| Field | Shape | What it is |
|---|---|---|
| `form_version` | string | the surface's version, fenced like a grammar program's |
| `id` | `prefab/<kebab>` | the prefab id the output carries |
| `box` | `[x, y, z]` | the piece's extent in blocks; the apron fills its footprint |
| `sub` | `2` or `4` | sub-voxels per block per axis for the octant fit (the research used 4) |
| `ground` | `{block, top}` | the apron: full blocks of `block` at local `y ∈ [0, top]` wherever the fitted body leaves air; **required** — a form with no ground is refused naming follow-up A |
| `sink` | integer ≥ 0 | how many courses of the body lie below the apron's top; a designer's number, stated once |
| `solids[]` | ordered | the body: `{op: add \| cut, shape, …, noisy?}` with `shape` one of `capsule` (`from`, `to`, `radius_from`, `radius_to`, `stretch_y?`), `ellipsoid` (`centre`, `radii`), `disc` (`centre`, `axis`, `radius`, `height`), `box` (`from`, `to`), `shelf` (a walkway: `path[]` of `[x, y_feet, z]` knots, `width`, `clearance`, `depth`; the shelf is solid below the feet surface interpolated along the path and clear for `clearance` above it) |
| `noise` | `{amplitude, cell}` | the seeded low-frequency weathering applied to every `noisy` solid |
| `palette` | four tones, each `{full: [[block, weight]…], family}` | the material by tone, bleached to weathered, exactly the research's measured ramp as the gallery's first value; every `full` block and the `family`'s stair and slab must exist in 1.21.11 and the family must have both shapes |
| `lights[]` | `{at: [x, y, z], block}` | light **placed where the room is designed**: a light-emitting block at a cell, replacing whatever the fit put there |
| `anchors` | `{name: {pos, facing?, role?, region?}}` | prefab metadata's own anchor shape, verbatim; at least one `role: entry` |

Four refusals are the document arm, one code (`DW_ORGANIC_FORM`, validation tier, exit 1), raised where the form is read and before anything is fitted: no `ground`; no anchor with `role: entry`; a palette block, a ground block or a shelf that does not exist or whose family lacks a stair or a slab (an unknown id is the ordinary `DW0193` family); and **a palette tone or a ground block that emits light** — the floor of a body is never paved with a glowing block; light enters through `lights[]` and through cuts that reach the sky, and nothing else. The light-emission fact is read from the relight pass's own block-light emission table (`light::emission` in `crates/delvec/src/compiler/light.rs`, held to the pinned game's `getLightEmission` for every block state by `crates/delvec/tests/emission_table.rs`), so the rule cannot disagree with it.

### 3.3 What `sculpt` does, in order

**Authored**, a port of the research's back end into `crates/delvec/src/sculpt/`, with every rule the engine already owns taken from where it lives rather than copied:

1. Stamp `solids[]` in order into a solid sub-voxel grid of `box × sub`, `add` and `cut`, noise from one seeded PCG stream where `noisy`.
2. Octant fit: air, full, bottom or top slab, or a straight stair of any facing and half, by least occupancy error with the research's cost bias against sub-block shapes.
3. Thin-plate refit where the fit says air, from the solid thickened by one sub-voxel.
4. Stair `shape` by `schem::stairs::derive_shape` — the fitter never chooses a corner.
5. Tone by normal, openness and noise; block by tone and `palette`; `bone_block` and every other axis-bearing block by the longest local run.
6. The apron: `ground.block` at every air cell with `y ≤ ground.top`. Then `lights[]`, each replacing its cell.
7. Islands: a disconnected group of blocks smaller than four is dropped and counted; the count is printed and recorded.
8. Output through `convert::build_region` — the workspace's one structure-template byte boundary — tiled at 48, with every placed state completed to every property the block has, as every other producer's is.
9. Metadata: `prefab_id`, `structure` or `structure_set`, `anchors` as declared, `lighting: {profile: "unmeasured"}` (a generated piece places blocks, not photons, and declares that a probe is owed — the grammar's rule, `compiler.md`), `license` as `{source: "original", spdx: "GPL-3.0-or-later", provenance: the form's id and hash}`, `generated_by` as §3.1, and the two measurements of §3.4.

### 3.4 What the piece says about itself, and what refuses it

**Authored**, each on a rule that already exists:

- **`walk_y`** is measured off the fitted blocks by the one rule every generator uses — the lowest local y holding a standable cell, through the engine's own standable predicate (`prefab_invariants::walkplane::measure_walk_y`) — never read from `ground.top`. Because the apron fills the footprint, the two agree, and a test says so.
- **`shown_faces`** is derived from the bytes: a side the apron or the body puts blocks on is a finished exterior surface. The apron reaches every side, so all four are written, and `DW0885`'s second arm (a side declared shown with no block on it) can never fire on a sculpted piece.
- **The entry stands and is met from grade.** After fitting, every anchor's cell is standable (the gallery generator's rule: the geometry is carved around the anchor table and the metadata printed from it), and the `role: entry` anchor is in the flood `schem::nav::ground_entry` seeds from the box's vertical faces at grade. A form whose fit buries its entry, or whose entry stands on a ledge grade does not reach, is refused `DW_ORGANIC_BODY` (exit 3) with the cell named.
- **No pocket.** The leave relation `World::body_moves` — the relation `DW0921` floods, with its measured constants — is run over the piece alone, from every declared anchor, with the apron as ground and the box's outside as "gone" (a body that leaves the box is out of the question, as the boundary clock's business). A reachable cell from which no walk, fall, jump or swim leads back to a cell the entry reaches is a pocket. The command prints `pockets: P place(s), C cell(s), of R cell(s) a body can reach from E anchor(s)`, and refuses on `P > 0` with `DW_ORGANIC_BODY`, naming up to six places with the way in, as `DW0921` does. The remedy is in the form, in its own terms: a `shelf` out of the pocket, a `cut` that opens it, more `sink`, a different profile. The rule is one function shared with `DW0921`, never a second implementation.
- **The grammar's always-on gates** run over the output as they run over an expansion: `blocks-exist`, `states-complete` (`DW0737`), `stair-shape` (`DW0801`) and `non-empty`, each printing its binding count, each red when it examined nothing (`gates::seal_zero_bindings`).

The back's edge is a cliff edge: a fall from it onto the apron that exceeds the survivable fall is a death the player can see coming, exactly as a cliff in a grammar piece is, and no rule here hides it or fences it. A killing volume a campaign declares under a hole in the body is `DW0891`'s business, unchanged.

### 3.5 The way in, the way up, and the inside

**Authored.** Walking *inside* and *on* the body needs no new move: a `cut` through the hull at grade is a door, a `cut` ellipsoid is a room whose floor is the apron, and a `shelf` rising at most half a block per block is a stair the route proofs walk today (§2.3). The campaign record's own phrasing — a spine stair "built as steps" — is a `shelf`. A route that genuinely needs a ladder or a gap is follow-up B, and a form may not pretend otherwise: a `shelf` whose knots rise more than one block per block of path is refused `DW_ORGANIC_FORM`, because the fit of such a slope is a wall.

### 3.6 Scale

**Cited**: a 140 × 60 × 45 body is 378,000 cells, 3.8% of the schematic reader's cap of 10,000,000; the research's 320-block reference whale is about 4.6 million, and its 448-block one is over it. **Authored**: `sculpt` writes parts through `convert::build_region` and never constructs a schematic, so `DW0710`'s cap is not on its path and the only ceilings are the 48-block part and the machine's memory; the command prints the cell count and the part count it wrote.

## 4. What the engine checks and what it leaves to design

### 4.1 Light

**Cited rulings**: light is placed while the room is designed and the engine only checks; a brightness failure is repaired by re-arranging the room or raising the density of what already lights it, never by an algorithm over a lightless scene; paving a floor with glowing ore is the type specimen of inventing. **Authored on them**: the form's `lights[]` is the designer's placement; the sculpted output declares `unmeasured`; `delvec prefab lighting` measures it from grade and reports the distribution of dark cells (`DW0751`); a build measures the body's reachable cells with every other area's (`DW0210`). The emissive-palette refusal (§3.2) is what keeps the anti-example out of the mechanism.

### 4.2 The open craft question, and the room left for it

**Authored.** Which block reads as a body's own light — rather than as a lit building inside a body — is unanswered. Two candidates are on the table and neither has been looked at on a built body. This spec decides nothing about it, and is shaped so that the answer needs no engine change: the block is the `block` of a `lights[]` entry, a vanilla id checked against the registry and the light-emission table and nothing else; the look is judged on the demo level §7 queues, at playable scale, on a body with cuts that admit daylight and lights placed by hand; what that level settles is written into the form that the content library keeps, and into `docs/reference/` as a craft record, not into this spec or the engine.

### 4.3 Danger is visible

**Cited ruling**: a killing volume may not reach a cell that reads to a player as safe floor; the repair is to move the hazard. **Authored**: nothing here adds a hazard. A pit cut into the body with a volume at its bottom is `DW0891`'s case, judged over the assembled world as today; a hole in the back is a hole the player sees. Follow-up C says what changes when the volume is live only from a stage.

### 4.4 Playable scale

**Cited rule**: buildings are judged at playable scale — does it read as the thing, and does the inside belong to it. **Authored**: the acceptance of a sculpted body is on the demo level, by walking it, never on a campaign's renders; the research's path-traced sheet is the record of what the fitter can reach, not a judgement of any piece this spec will produce.

## 5. Where it enters the procedure, the tools and the skill

**Authored**, each an obligation of the implementing pull request:

- `docs/reference/prefab-procedure.md` §0: the smooth-curve row's route becomes `delvec sculpt` over a form document, with this spec's refusals named; the row stops being the one where making a prefab becomes extending the engine.
- `docs/reference/tools.md`: the `sculpt` command, its flags, its binding lines, and the form document's fields; `docs/reference/compiler.md`: the two codes, the sculpt module in the pass order of `delvec prefab`'s family, and the metadata it writes; `tools/ci/check-dw-codes.py` holds the codes bidirectionally.
- The `/new-delve` skill's `references/new-pieces.md`: the form route as a mandatory step beside the grammar route, with the measurement lines a creator reads back (`pockets`, the entry flood, `walk_y`, `shown_faces`).

## 6. Follow-ups, named, with their dependencies

**Authored.** The capability the campaign record calls one letter is four object classes, and a pull request is one whole feature; each follow-up below is its own spec, numbered by the planner.

| Follow-up | Object class | What it is | Depends on |
|---|---|---|---|
| **A. Entry-seeded piece instruments** | the piece instruments (`prefab lighting`, `prefab planes`, the admission walk) | where no vertical face carries grade, seed from the declared `role: entry` anchor instead; what §2.2 shows the apron already does for a body on the ground, for a body with none under it | this spec's form (an `entry` to seed from); the sky base of spec-0026 §4 for a reason to float |
| **B. Player movement repertoire** | the player body (`nav`, the harness bot) | climbing a climbable column and clearing a gap as **route** moves: one table measured against vanilla physics, read by every route proof, performed by the bot on cue (M05, M06; coverage map rank 3) | nothing in this spec; a `shelf` is how a form avoids needing it |
| **C. A lethal volume live from a story stage** | the region declaration | `lethal_volumes[]` carries the shared `Guard` (coverage map rank 6); the nav premises hold the volume per quest configuration as `region_state_at` holds a gate seal; `DW0891` is judged per configuration the critical path passes, as `DW0921` is (M04); `dsl_version` moves | nothing in this spec; its demo is a collapse over a pit in a grammar room, not a body |
| **D. A `clouds` surround** | the horizon | a backdrop analytically known so the proofs read it as void (research §6; the pinned Chunky core's `Sky` can draw block clouds) | spec-0026 §4 |
| **E. A mesh as a form** | the form document | the research's first front end: a mesh plus a provenance record, refused unless it names an ADR-0013 licence, fitted by this spec's back end | this spec |

## 7. The gallery's obligation, and the demo level

**Authored**, against `gallery/` and `tools/ci/gallery-prefabs.py` at `ce3f7fe7`:

1. **A gallery form.** `gallery/forms/<id>.json` — a small body with a ground, one cavity entered at grade, one `shelf` onto its back, one `lights[]` entry, and an `entry` anchor — is sculpted by `tools/ci/gallery-prefabs.py` into the gallery's prefab directory beside the generator's pieces and the detailed place; the script's binding line gains `forms sculpted`, and zero is a red.
2. **Bound.** An overlay point places the piece and walks a reach objective inside it and one on its back, so the gallery's `DW0921` ledger, `DW0210` survey and `piece-exposure.json` all bind over a sculpted body with non-zero denominators; perturbing the form's `shelf` out of existence reds the route (`DW0311`), perturbing its `lights[]` away reds `DW0210`.
3. **Refusal-proven.** Three probes: `a-body-with-no-ground` (`DW_ORGANIC_FORM`), `a-body-paved-with-light` (a palette tone of `glowstone`; `DW_ORGANIC_FORM`), `a-body-with-a-pocket` (a `cut` pit three deep with no `shelf`; `DW_ORGANIC_BODY`), each the primary plus one declared edit the engine rejects by name.
4. **The gate sees the form.** `tools/ci/check-gallery-coverage.py` enumerates the form document's units from `delvec schema --stage sculpt-form` and binds them against the gallery form, as it was made to see the camera record; its run line reports the form's units enumerated and bound, with 0 in neither state.
5. **Demo level** (`docs/demo-levels.md`): one row, `pending (blocked on 0087)`, a body at the stated scale or above with the inside and the back walked, which is where the interior light is looked at (§4.2) and where the body is judged at playable scale (§4.4).

## 8. Decisions

None for the owner. Everything here is an engine-internal surface shaped by rules already given; the one craft question (§4.2) is answered by looking at a demo level, not by deciding now. For the planner: allocate the two codes; number the follow-ups; and read §2.2's second consequence as the brief for the implementing round — the leave proof at generation is the half of this spec most likely to be deferred and least safe to defer.

## Acceptance criteria

Each is checked against the tree at `ce3f7fe7`, where none holds; the implementation lands them all in one pull request.

1. **Surface.** `delvec sculpt <form.json> -o <dir> [--seed N] [--id]` exists; `delvec schema --stage sculpt-form` exports the form document's schema and `--stage all` does not include it; a test in `crates/delvec/tests/` holds `compiler.md`'s field list for the form to the struct in both directions.
2. **Determinism.** A test sculpts the gallery form twice at one seed and asserts byte-identical parts and metadata; `generated_by` carries `generator: "sculpt"`, the form's `program_hash`, `seed` and `region`, and re-sculpting from those alone reproduces the hash.
3. **One byte boundary, one stair rule.** The sculpt module writes parts only through `convert::build_region`; stair `shape` comes only from `schem::stairs::derive_shape`; `grep` over `crates/delvec/src/sculpt/` finds no schematic reader and no second shape derivation; `delvec prefab audit` over the gallery form's output reports `pass` with stairs examined equal to stairs written and `underspecified` 0.
4. **The grammar's gates run here.** `blocks-exist`, `states-complete`, `stair-shape` and `non-empty` run over every sculpt through the same gate functions the grammar calls, print their binding counts, and red on zero through `gates::seal_zero_bindings`; a test perturbs a palette block to a misspelt id and asserts the refusal names the gate.
5. **Document arm.** Each of the four `DW_ORGANIC_FORM` shapes (§3.2) and the steep-shelf shape (§3.5) is asserted by a test over a form fixture, raised before any fitting (the test asserts no output directory is written), with the remedy of each a row in `crates/delvec/tests/remedy_reachability.rs`.
6. **Ground, entry, planes, faces.** On the gallery form's output: `walk_y` written equals what `delvec prefab planes` measures over it, and equals `ground.top + 1`; `shown_faces` lists every side the bytes put blocks on and `DW0885`'s second arm is not raised when the gallery builds; the `entry` anchor's cell is in `schem::nav::ground_entry`'s flood, asserted by a test; a fixture whose fit buries its entry is refused `DW_ORGANIC_BODY` naming the cell.
7. **Pockets.** The sculpt's leave relation is `World::body_moves` (one function, asserted by the test that enumerates the relation's callers); the command prints `pockets: P place(s), C cell(s), of R cell(s) a body can reach from E anchor(s)` on every run; the probe `a-body-with-a-pocket` is refused `DW_ORGANIC_BODY` with `P ≥ 1`, and the same form with a `shelf` declared out of the pit sculpts green with `P = 0` (a `remedy_reachability.rs` row); the rig's own form, committed as a fixture, is refused with a non-zero `P` at this spec's revision, the number written in the commit body.
8. **Light.** A palette tone, a ground block or a shelf material whose block emits light is refused `DW_ORGANIC_FORM`, read from `light::emission` and nothing else (asserted by a test over a tone of `glowstone` and a tone of `sea_lantern`); the gallery form's output declares `lighting.profile: unmeasured`; `delvec prefab lighting` over it binds `entry_cells > 0` and `measured_cells > 0`; the gallery builds green with the form's `lights[]` and reds `DW0210` with them removed.
9. **Scale.** A test sculpts a form whose box exceeds 10,000,000 cells at `sub: 2` and asserts parts are written and `DW0710` is never constructed on the path; the command prints cells and parts written.
10. **Gallery** (spec-0039, §7 here). `gallery/forms/<id>.json` exists and `tools/ci/gallery-prefabs.py` sculpts it with a `forms sculpted: 1` binding line; an overlay point walks inside it and onto its back; the three probes are refused by name; `check-gallery-coverage.py` enumerates the form's units from `delvec schema --stage sculpt-form` and binds them with 0 in neither state; the regenerated `gallery/baseline/` moves and the commit body attributes every moved row.
11. **Docs and skill, same change.** `docs/reference/compiler.md` gains the two codes and the metadata `sculpt` writes; `docs/reference/tools.md` gains the command and the form's fields; `docs/reference/prefab-procedure.md` §0's smooth-curve row names `delvec sculpt`; `.claude/skills/delvewright/skills/new-delve/references/new-pieces.md` carries the form route as a mandatory step; `tools/ci/check-dw-codes.py` is green.
12. **Demo level.** `docs/demo-levels.md` carries the row this spec adds, moved from `pending (blocked on 0087)` to pending when the pull request merges, and the level that closes it walks the inside and the back of a body of at least 140 × 60 × 45 blocks through the full ladder.
