# spec-0127: Places meet seamlessly on living ground — a side with no declaration stands no wall, the fill takes surface rules and grows trees, the whole's proofs read the declaration, and a place's author checks against the whole

- **Status**: Proposed
- **Ground**: written against the engine branch that built the town (`feat/town-starter-3` @ `fa16f5581`, 56 commits over `main`), read whole — `crates/dsl/src/siteplan/{mod,place,seam,claim,ground,fillcheck}.rs`, `crates/delvec/src/compiler/{blockout,burial,holders,render_plan}.rs`, `crates/delvec/src/compiler/nav/width.rs`, `crates/delvec/src/admit/allowlist.rs`, `crates/delvec/src/detail.rs`; `docs/reference/compiler.md` (*The map pipeline — `site-plan`*: `boxes[].sides`, `seams[].contact`, `fill`; *The blockout*; `delvec detail --check`), `docs/reference/delvec/compiler/{blockout,burial,holders,nav}.md`, `docs/reference/delvec/compiler/nav/width.md`; spec-0098 §2b, §2c, §14; spec-0105 §9; spec-0122 §6.1; spec-0123 §5; spec-0125. Evidence: the town (content `demo/town-starter-3`), a hillside town of some ninety-five places built through `/new-delve` step 9, whose refusals and repairs are the commit record of the branch.
- **What it is for**: the town needed its places to meet as one ground — a square running into its streets, a street into the plots along it — and its ground to read as a hillside and not a plane of one block. Each of those was reached by an engine change made while a place was being built, and this spec is the record of them, so that `/new-delve` on `main` carries what the town needed. Every statement is **[ruling]** (a judgement made at the town and taken as given), **[cited]** (a reading of the tree or a constitution rule) or **[authored]** (this spec chooses).
- **Numbers**: spec `0127`. No new diagnostic: every code this spec touches exists (`DW0730`, `DW0836`, `DW0838`, `DW0885`, `DW0990`, `DW1024`, `DW1057`, `DW1064`, `DW1065`, `DW1066`). `boxes[].sides`'s default and `fill.strata[]` change what a site plan says; they ship under `dsl_version` `0.69.0`, the number spec-0125 took on the same branch, because no release carries `0.68.0` or `0.69.0` and a `dsl_version` numbers a surface at a release, not a commit (CLAUDE.md: *Nothing owes compatibility to anything already built*).
- **Non-goals**: a biome, a climate or a vegetation type the engine infers (a stratum is a rule the author writes, §2.2); a tree the author places by hand (a placed tree is a piece's or the edit script's); a second place generator (§8 names the one); any change to what a `contact` seam allocates (§4.1); a wall the engine stands for the author on a side they did not declare (§2.1).

## 1. The rulings **[ruling]**

1. **A side with no declaration stands no wall.** The town's square, its streets and the plots along them meet as one paved ground; writing `"sides": {"east": "open", "west": "open", …}` on ninety boxes to say so was the author declaring the ordinary case, and a box that said nothing stood walls round a street. The ordinary case is seamless; a wall is the choice an author makes in words.
2. **Ground meets across a seam.** Where two seamless places met, a strip of the fill's grass showed between their pavings (the square and the cathedral): the ring's fixed ground reached the walk plane on an open side. The floor course of an open side is the place's own, so its paving runs to the seam.
3. **The hillside is not one block.** A town on a slope wants rock where it is steep, grass where it is flat, mud by the water and trees where nothing is built, and no author places them cell by cell. The fill carries rules, after the pinned game's own surface rules, and grows trees where a body still walks under them.
4. **A check confirms intent; it never restricts** (CLAUDE.md). `DW1024` refused a place holding more than two scenery containers; how many scenery containers a place holds is the author's. The arm that refused a container that matters among scenery stays.
5. **A town's signs of life are made of potted flowers.** `DW0730` refused `minecraft:potted_red_tulip` and its family while admitting the empty pot and the bare flower; a pot with a flower in it is the same decoration.
6. **The place author's check sees what the build sees.** A refusal the battery raised at commit — a leak, a crack at the plot's edge, a face standing in open air — sent the author round the loop with a report of a verb they had not run. `delvec detail --check` runs those proofs for the checked place over the whole as the run would leave it.
7. **A proof reads what the author declared.** With the open runs deleted, the leak proof still refused a body crossing at the ring corner where two open runs meet, and a body resting on the top of a wall the proof itself had stood. A seamless border is a declared opening along its whole length, its ring corners included, and the deletion invents no floor.

## 2. The surface — stage 4 **[authored; cited: `compiler.md` *site-plan*]**

### 2.1 `boxes[].sides`: absent is `open`

`sides` stays `{east?, west?, south?, north?}`, each `"wall"` or `"open"` (spec-0123 §5.1). **A key absent is `open`**: no wall stands on an undeclared side and the ground meets across the seam; a box with no `sides` is seamless all round. `wall` is the explicit choice that stands one. Two spellings of an open side are distinguished where a check reads them: a side **declared `open` in words** is a way — the clear-width check judges it (`DW1057` narrowed, `DW1065` built shut) and the render plan draws a threshold shot at it; a side **with no declaration** is seamless, owes no clear width and has no front. Both stand no wall, and both are declared openings to the leak proof (§4.1). `delvec fmt` writes the object's keys in the engine's side order. `PlacedBox::open_sides` is every side not declared `wall`; `PlacedBox::declared_open_sides` is the subset declared `open`.

### 2.2 `fill.strata[]`: surface rules on an `open` fill

An `open` fill may carry `strata[]`, each `{when, surface, below, trees?}`:

```json
{ "when": { "y_min": 70, "steep": 2, "patch": { "scale": 6, "min": 400, "salt": 3 }, "area": { "min": [0, 0], "max": [63, 63] } },
  "surface": "minecraft:stone", "below": "minecraft:stone",
  "trees": { "per_mille": 25, "log": "minecraft:spruce_log[axis=y]", "leaves": "minecraft:spruce_leaves[persistent=true]", "height": [5, 7] } }
```

- `when` is every condition the column must meet, each optional: `y_min` / `y_max` (the column's surface `y` in the band), `steep` (its surface differs from one of its four neighbours' by at least this many blocks), `patch` (a value noise of cell `scale` blocks, seeded by the world's `seed` mixed with `salt`, reads at least `min` per mille at the column), `area` (the column lies in the rectangle, corners inclusive).
- **The first stratum a column meets, in document order, gives its `surface` and `below`**; a column no stratum meets takes the fill's own. The rules are read over the region's columns on a `flat` terrain as on a `heightmap` (a flat terrain has no steep column). The fixed ring ground under a place reads the same column (`Ground::column_blocks`), so a plot on the rock stands on rock.
- `trees` grows a trunk of `log` (height drawn in `height`, inclusive) and a crown of `leaves` on `per_mille` of the stratum's columns, one per 5 × 5 neighbourhood, never within four cells of any place's footprint ring, never where the region's top would cut it, and only where a body walks out under its crown — the lowest leaf layer three or more over the ground of every column it covers (`Ground::trees`). Trees are written after the terrain and before any place.
- Every block in a stratum or a tree is a block state (`DW0193`). Every draw is seeded (ADR-0006): the same plan and seed grow the same trees.

## 3. The derivation **[authored; cited: `compiler.md` *The blockout*, `claim.rs`]**

- **The floor course of an open side is the place's.** The ring's fixed ground on an open side — and at a corner both of whose sides are open — stops under the floor course (`Site::fixed_top`, the one answer `Site::is_fixed` and `Site::fixed_cells` read), so the place's paving runs across the seam and no strip of the fill's surface shows between two places that meet there. The stand-in writes that floor course in the place's accent.
- **A stair cut in an open side stands no frame ring**: the flight crosses in the open air; a stand-in on an open side otherwise derives as spec-0123 §5.2 states.
- **A site plan's loaded extent is its whole region**: the fill is ground the whole lays over the region around the places, as a valley horizon's landform is, so a frame is never cropped to the places.

## 4. The checks **[authored; cited: `blockout.md`, `width.md`, `holders.md`, `nav.md`]**

### 4.1 The leak proof, `DW0838`

- **Every open side's run is a declared opening**, deleted with the seams: two places that leave their shared face unwalled meet by the plan's declaration, and a join through it is never unallocated. A contact's span stays the edge's allocation set, and a crossing outside it is a refusal — unless both places leave that face unwalled.
- **A seamless border is a declared opening along its whole length** (`PlacedBox::seamless_border`): for two places sharing a face neither walls, the gap over their common span and the two ring corners where their rings meet, from the lower walk plane to the higher top, are deleted too. The corners lie in neither open run and are reached from a wall head — a low garden wall, a perturbed shell — so a body walking a wall head to the corner and dropping into the neighbour crosses where the plan said the ground runs on.
- **The deletion invents no floor** (`blockout::floor_made`): a cell standable only because the deletion walled the cell under it is kept off by every flood.
- **Each leak names its route**, the fewest moves of the reach graph from the place to the cell it reaches, each move's kind and shape and the cell it ends in (`World::body_route`), so the place is closed where the body crosses.
- Claim 2 of `DW0836` reads an open side's run as a declared opening of the shared wall, never a leak.
- **Binding**: the battery line states seamless borders deleted with the cells they add beyond the open sides' runs, and the cells of floor the deletion made kept off.

### 4.2 The clear-width check and the threshold frame read the words

`DW1057` and `DW1065` judge a side **declared `open`** (`declared_open_sides`); an undeclared side is seamless and owes no clear width — under the seamless default every flight side facing a slope read as an open side built shut. The render plan draws a threshold shot at a side declared `open`; an undeclared side has no front.

### 4.3 A stair crosses an open side

`DW1064` refuses a `walk` portal — a door, an arch — cut in a side declared open; a `stair` seam (`stair_in`) crosses an open side in the open air and is admitted. A contact carries no stair, so without this the remedy `DW1064` named was unreachable for a flight between two seamless places.

### 4.4 A leak, a crack and a sightline are never a trapdoor's

`DW1066`'s second run — the battery over the worlds with every trapdoor's shape lifted — is taken only for a refusal a stopped body raises (`DW0836`, `DW0837`, `DW0877`, `DW0986`, `blockout::STOPPED`). A leak (`DW0838`), a crack (`DW0990`) and a sightline (`DW0821`) stand, and a battery whose refusals are only those is not run twice.

### 4.5 `DW1024` keeps one arm

`DW1024` refuses a container that matters (loot, a collect, a device) standing among scenery containers in one place, and nothing else: the storeroom arm, `MAX_SCENERY_PER_PLACE`, the census line's cap clause and the ledger's `max_scenery_per_place` and `over_cap_places` are gone. The census line states how many scenery containers each place holds. This departs from spec-0105 §9.2 by the rule of §1.4 and by the signal spec-0105 §9 named as what would show the cap wrong: a place refused at three where a reader at playable scale sees ordinary furniture.

### 4.6 `DW0730` admits every potted plant

`Allowlist::default_building` adds every registry id beginning `minecraft:potted_` to its exact set, read from the pinned registry; a `from_file` override replaces the default whole, this family included. The regression direction is one way: it can only let a block through that was refused before.

## 5. The place author's check sees the whole **[authored; cited: spec-0122 §6.1, `compiler.md` *`delvec detail --check`*]**

`delvec detail --check <campaign> <place>` keeps spec-0122's shape — the commit's own preparation and judge, writing nothing — and then assembles the whole in memory as the run would leave it, the edit script replayed, and runs over it for the checked place alone, through the build's own implementations: the leak proof flooded from that place (`blockout::leaks`), `DW0990`'s crack shape along its plot edge (`blockout::stitches`), and `DW0885` with the checked piece (`burial::check` with `only`, the party's walk taken as the build takes it from the plan's anchors, every declared way open). A refusal is the one the build would raise for that place, at exit 3, named with its cell or its route. It prints the leak proof's binding, `part sizes:` per bound part against the band `DW1074` allows (`plot::size_band`, handed by `delvec allocation` as `bands`), and `peak rss`. The agreement test of spec-0122 §6.1 compares the codes each run prints up to the commit's write; the battery's codes stay outside it because `--check` runs only these three proofs and only from the checked place.

## 6. Gallery elements and refusal probes **[authored]**

- The site-plan overlay declares `sides` on every box in words, so the seamless default is exercised by the places that leave sides undeclared and the walls by those that say `wall`; `node/exit` and `node/causeway` declare `open` sides.
- The site-plan overlay's `fill` carries two strata binding every field of §2.2: a wood in the region's south-east corner (`y_min`, `y_max`, `area{min, max}`, `surface`, `below`, `trees{per_mille, log, leaves, height}`), on ground no place claims and small enough that the night relight still sites a torch under every crown (`DW0211`); and rock on steep patches (`steep`, `patch{scale, min, salt}`).
- Probes, each the primary plus one declared edit, refused with the code it names under `check-gallery-coverage`: `a-rail-a-body-steps-over` — the gantry's program edited so its rail is a course of planks a body jumps onto, with the air over it claimed as the deck's, so the body falls off the deck into the yard it hangs over, two places sharing no wall (`DW0838`, named at the detail commit and at the build); `a-wall-built-low-beside-the-front` — the far hall's program edited so the end of its south wall past the causeway front's span stands one course high with its head claimed as floor: a passable cell of a shared wall's plane outside the seam's allocation (`DW0836`, the second claim); the existing `a-door-in-an-open-side` (`DW1064`), `an-open-side-built-shut` (`DW1065`), `a-post-in-an-open-side` (`DW1057`), `a-key-among-empty-chests` (`DW1024`).

## 7. Pages **[authored]**

`docs/reference/compiler.md` (*site-plan*: `boxes[].sides`, `seams[].contact`, `fill`; *The blockout*: the stand-in row; `delvec detail --check`; `validation/holders.json`), `docs/reference/delvec/compiler/blockout.md` (`DW0838`), `burial.md` (`DW0885`), `holders.md` (the arrangement rule, `DW1024`), `nav.md` (`DW1066`), `nav/width.md` (the population), `docs/reference/dsl/siteplan.md`, `docs/reference/tools.md` (`detail --check`), and the skill's `references/detail.md` and `references/placement.md` carry every change above in the PR that merges it.

## 8. The second spec: a place is drawn, and the engine says where a body goes **[authored]**

The same branch carries the place generator, which is a spec of its own and takes the next number the planner allocates: `delvec grammar survey` (what the engine says of a drawn model — standable cells, the reach graph's flood, sealed air, joined spaces, blocks the server drops, blocks the palette refuses, containers, malformed states) and `delvec grammar blocks` (the pin's blocks admitted, refused and counted as containers); the starter's `Model` and `run(build)` in `scripts/starter.py` — cells in piece-local coordinates, parts, marks, `Model.roof`, `Model.show`, the full-state block constructors, every cell claimed from the survey's answers — and the handout's `blocks.json` with its *The blocks you may write* section; and a contract space spanning as many levels as its blocks give it, `contract-well-formed`'s one-floor clause gone and `contract-reachability` refusing a fall-in pocket inside a space spanning more than two levels (`fall_in_pockets`). Those surfaces merge with this spec's PR and are held to the second spec's criteria when it is written.

## 9. Rejected **[authored]**

- *A `seamless` third value of a side.* A side has two states a wall can be in; the words distinguish a way (`open`) from the ordinary meeting of two grounds (absent), and a third word would name the default.
- *A biome-driven surface.* The pinned game's surface rules read a biome; a delve's fill reads the author's rule, so the author decides where the rock shows.
- *Placing trees by the edit script.* The script dresses what the whole built; a tree is part of the ground's rule, and a seeded draw per column keeps it deterministic and off every place's ring.
- *A seamless group.* The first reading let every pair of a group of seamlessly joined places through; in the town every place was one group, so a crossing through a side one place walls was no longer refused. Every pair stays judged and only the border's cells are deleted.

## Acceptance criteria

Each criterion is checked against the tree at `fa16f5581` plus this spec's PR; a criterion the tree cannot yet satisfy is marked **debt** with what is owed.

1. `boxes[].sides` with a key absent derives no wall on that side and the stand-in's floor course across the ring; a box with no `sides` is seamless all round; `wall` stands one. Asserted: `crates/delvec/tests/blockout.rs` `an_open_side_raises_no_wall_above_the_floor_course` and the fixtures of `crates/dsl/tests/v14_site_plan.rs` that declare their walls; `compiler.md` states the default.
2. `fill.strata[]` accepts `{when{y_min?, y_max?, steep?, patch{scale, min, salt?}?, area{min, max}?}, surface, below, trees{per_mille, log, leaves, height}?}`; the first stratum a column meets gives its blocks and the fill's own stand where none does, on a heightmap and on a flat terrain alike (a flat terrain has no steep column); a block that is not a block state is `DW0193`. Asserted by `crates/dsl/tests/v14_site_plan.rs` `a_stratum_gives_its_columns_their_blocks_and_grows_trees_clear_of_every_ring`: the column kinds inside and outside the wood, the blocks at and under the surface, a flat column never steep, every trunk inside the wood and four or more cells from every ring, a crown drawn, the draw equal twice, and `DW0193` on a block that is not a block state. **Debt**: the three-course clearance under every crown is not asserted over a sloped terrain.
3. On an open side the ring's fixed ground stops under the floor course and the floor course is the place's (`Site::fixed_top`, the one answer `Site::is_fixed` and `Site::fixed_cells` read). **Debt**: no test asserts it; a `claim.rs` unit test of `fixed_top` on an open side against a walled one, and a `blockout.rs` test reading the mass map at a seamless seam for no cell of the fill's surface at the walk plane, are owed.
4. `DW0838` deletes every open side's run and every seamless border with the seams, invents no floor and names each leak's route; asserted by `a_seamless_border_is_crossed_anywhere_along_it` (red when the border's cells are not deleted, measured in the commit that landed it), `an_open_side_is_a_stand_ins_opening_onto_the_commons`, `the_top_of_a_walled_opening_is_floor_only_where_the_world_had_none` (`floor_made`); the battery line carries `seamless border(s)`, `cell(s) beyond the open sides' runs` and `cell(s) of floor the deletion made kept off`.
5. `DW1057` and `DW1065` judge only sides declared `open` in words, and the render plan draws a threshold shot only at those; `place_reads.rs` `the_site_plan_point_draws_a_threshold_shot_per_door_and_open_side` asserts the shot count over the site-plan point. **Debt**: no test perturbs a side from `open` to absent and asserts the way count and the shot count both fall by one; owed in `crates/delvec/tests/place_reads.rs`.
6. `DW1064` refuses a `walk` portal in a side declared open (`v14_site_plan.rs` `dw1064_a_portal_in_a_side_declared_open`) and admits a `stair` seam there. **Debt**: the stair admission has no test; a `v14_site_plan.rs` case with `stair_in` across an open side passing is owed.
7. `DW1066`'s second run is taken only for a `STOPPED` refusal; asserted by `a_leak_is_never_a_trapdoors_and_is_not_run_twice`.
8. `DW1024` refuses a container that matters among scenery and caps nothing; asserted by `any_number_of_scenery_containers_in_a_place_passes_dw1024` and `tests/holders.rs` `many_scenery_containers_in_one_place_build`; `validation/holders.json` carries no `max_scenery_per_place` or `over_cap_places`; the probe `a-key-among-empty-chests` reds `DW1024` and no probe names a storeroom.
9. `DW0730` admits every `minecraft:potted_*` id of the pinned registry; asserted by `every_potted_plant_in_the_pin_is_admitted`.
10. `delvec detail --check` runs the leak proof, `DW0990`'s crack shape and `DW0885` for the checked place over the whole assembled in memory, writes nothing, and prints the leak proof's binding and `part sizes:`; asserted by `tests/starter_program.rs` (a far plot's roof left undeclared is `DW0885` at exit 3 from `--check`), `tests/blockout.rs` (`blockout::leaks` names the hollow edge's `DW0990` for the landing and nothing for a whole place) and `tests/detail_check_agrees.rs` (the codes each run prints up to the write are equal; `--check` leaves the library and `detail-plan.json` byte-identical).
11. The gallery's site-plan overlay declares a stratum binding every field of §2.2 and `check-gallery-coverage` enumerates and binds every `Stratum`, `StratumWhen`, `Patch`, `StratumArea` and `Trees` unit; a probe refused `DW0838` is committed; the gate passes with its counts stated.
12. The pages of §7 carry every change of §2–§5; `check-dw-codes.py`, `check-skill-page.py` and the docs link check stay green.
13. Executed once on the town through `/new-delve` step 9: every street and plot of content `demo/town-starter-3` detailed with the leak proof, `DW0885` and `DW0990` green from `detail --check` before commit, and the whole's battery `0 DW0838` — the measurement is the branch's commit record (the `detail --check` and whole-town figures in the commit that landed `blockout::leaks`).
