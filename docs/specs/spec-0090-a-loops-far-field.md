# spec-0090: A loop's far field — a visible difference far off is admitted while the jump moves it less on screen than station 4's did

- **Status**: Draft
- **Revises**: spec-0086 §4.1, §4.3 (the eyes and what a closed view must hold), §4.4–§4.6 (where identity is demanded) and §8 (the binding). spec-0086 stays the record of everything else: the slab, the gate, the seal, the exercise step, the emission, the bot.
- **Ground**: written against engine `e258b9f0` (`origin/main`) — `compiler::loop` (`eyes`, `sight`, `grow`, `check_one`, `LoopBinding`), `compiler::edit::check_batch_invariants`, `nav::walk_cells`, `nav::World::blocks_camera`, `light::LightModel::flood`, `tests/endless_corridor.rs` and `tests/common/corridor.rs`. The evidence is the eldritch spike's station 4, `tools/spike-eldritch-visuals/gen.py` on branch `research/eldritch-visuals` at `2bbb1f28` (constants `COR_*`, `LAMP_*`, `PERIOD`, `LOOP_*`, `ROOM_*`; functions `corridor/build`, `corridor/loop`, and the station-5 lines of `build`), read only.
- **What it is for**: a loop whose goal stays in view and never comes nearer. The exit's light is ahead down a long straight hall, the party walks or runs at it, and every crossing puts them back so the exit stays where it was. spec-0086 refused that hall: it required the view out of the landing to be closed by a wall, a turn or fog before any cell could differ from its image. Station 4 had no fog and no turn, its far end in view 60 blocks past the slab and 72 past the landing, and it was walked on a client and read as seamless. This spec admits what station 4 shows: far off, a difference the jump moves less than station 4's largest is not seen.
- **Research**: every rule below is marked **cited** (a line of the tree at `e258b9f0`, a line of the spike at `2bbb1f28`, a measurement in §5 with its instrument, or a constitution rule) or **authored** (this spec chooses). The one perceptual fact this spec rests on is a single client walk of station 4 by one person, recorded as seamless; nothing here is taken from the perception literature, and §10 says what that does not cover.
- **Numbers**: spec `0090`. No new DW code: `DW0947` carries the far-field refusal in a new shape, `DW0946` and `DW0948` name the near field in theirs. No DSL surface moves, so `dsl_version` stays `0.35.0`.
- **Non-goals**: a view into open sky or out of the built volume (still `DW0947`'s open-view shape, §2); a jump the far field hides because the player looks away (the rule judges every direction, §3.4); a body caught mid-jump (§10).

## 1. What changes, in one paragraph

**Authored.** The view out of the landing is split at a distance from the eye, the **near range**, derived from the offset (§3.3). Inside it the loop is what spec-0086 made it: every visible cell is its image under the offset, block for block and light for light, in every configuration. Past it, a visible cell may differ from its image, and the engine measures what that difference does on screen at the jump — the **shift**, the angle at the eye between the cell's two sightlines, before the move and after it (§3.1) — and admits it while the shift is at most the **far-field threshold** (§4), the largest shift station 4's own far field makes (§5). A difference over the threshold is refused (`DW0947`), naming the worst one. A far light is judged by its lit area (§3.2); a far body is judged as a far feature (§6). The view still has to close inside the built volume (§2).

## 2. The span and the eyes

**The view must still close** (spec-0086 §4.3, cited, unchanged in kind). A seen open cell outside every placed piece, above the build into open sky, or `SPAN_REACH` (128) cells past the landing slab is `DW0947` in its open-view shape. What lies there is not modelled, so nothing can be compared to it. A far end in view is a built thing: a room, a doorway, a lit exit.

**The span is found by a walk, not grown face by face** (authored). It starts at the cells the eyes stand in and steps to each neighbour of a seen open cell. A seen cell that blocks the camera is part of the view and ends the walk there. An open cell no eye sees ends it by geometry, and one seen only past the eye's fog end ends it by fog. The span box covers every cell the walk asked about. That is the closure spec-0086's growth had, since a seen open cell on a face grew the box one course past it. It is computed at a cost a 170-block hall can pay: the face-by-face growth re-sight every face cell from every eye on each step, and a far end 72 blocks off made that the build's longest pass.

**The eyes approaching the loop** (authored, over the cited poll). spec-0086 judged the eyes of the landing cells: the standing eye at each cell's centre and at the hitbox's four horizontal corners. A walking body is not moved from a cell centre. Vanilla selects on hitbox intersection (spec-0086 §4.2, cited), so the poll first finds a body when its hitbox's leading face is inside the slab's approach face, with its centre half a body-width (`metrics::PLAYER_WIDTH / 2`, 0.3) outside it. The judged eyes therefore gain the **catch band**: for a horizontal slab, the standing eye at that centre, at the middle of the passage cell and at each side of the hitbox across it. Every judged eye is written where it lands (`e′`); the eye it was moved from is `e′ − d`. On the long-gallery fixture the eyes go from 15 to 24 (three standable landing cells × (5 + 3)). An eye on the route before the catch band watches a world that does not move. The goal it sees far off is the design this spec admits, so no proof is owed about it. Station 4 as the spike built it is refused from a catch-band eye (§5.3), which no landing-cell eye reached.

## 3. The near field and the far field

### 3.1 The shift

**Authored**, over the cited move. The server keeps the body's facing across the move, and the client is told one fully relative update (spec-0086 §2). So a world point `p` the eye sees moves on screen by exactly the angle between `p − (e′ − d)` and `p − e′`. That is the parallax of the offset at `p`. A point on the offset's line through the eye does not move. A point beside the eye moves by the full angle between its two sightlines. A point far off moves by about `|d| / distance`. A periodic hall hides this for every cell whose image is the same: the screen shows the image where the cell was. Where the cell and its image differ, the difference is a feature that moved by its shift.

The **shift of a cell** is the largest shift of its nine points — its centre and its eight corners, each a hair inside the cell (the points spec-0086 §4.3's sightline already ends on), taken over every judged eye that sees the cell inside its fog end. The fog end is the farthest any configuration's fog lets that eye see.

### 3.2 A difference in block, in light, and a lit area

**Authored.** A visible cell **differs** when its block state differs from its image's, or when its light does at either sky (spec-0086 §4.4–§4.5, cited: the same `BlockMap`, the same `light::LightModel::flood` at the campaign's darkest and brightest reachable skies). A light shows on the faces it falls on, not in the air cell that carries the level. So a far light difference is judged over its **lit area**: the air cell whose light differs, together with every non-air neighbour whose face that light falls on. The shift is measured over that area's cells exactly as for a block difference. A lit exit seen down a hall is therefore judged by the walls and floor its light reaches, not by the empty doorway. Measured on station 4 (§5): judged at the air cell alone, a lit lintel 60 blocks past the slab shifted under the threshold; judged over its lit area it shifts 1.3418° and is refused.

### 3.3 The near range

**Authored**, derived from the threshold. The near range of an offset of length `|d|` is the least distance at which a one-cell difference can shift less than the threshold. A cell whose centre lies on the offset's line, beyond both eyes, shifts least. Its least-shifting corner stands `√½` off that line, and its shift between distances `r + ½` and `r + ½ + |d|` falls as `r` grows. The near range is the `r` where it meets the threshold (`compiler::loop::near_range`, by bisection). A cell is **in the near field** when its centre is nearer than the near range to some judged eye, before the jump or after it.

| Offset | Near range at 1.2852° |
|---|---|
| 6 blocks | 10.6 blocks |
| 12 blocks | 13.8 blocks |

A difference inside the near range would be refused by the shift anyway. The near field states it as the strict periodicity it is, with spec-0086's prescription (*make the sections the same*), because inside it no other remedy exists.

### 3.4 The verdict

**Authored.** In each configuration (spec-0086 §4.6, cited, unchanged):

1. A near-field cell that differs in block is `DW0946`, naming up to six cells with both states, the count, the configuration and the near range. One that differs in light is `DW0946`, naming the cell, both levels, the sky and the near range.
2. Every far-field difference is measured. If any shifts more than the threshold, the configuration is `DW0947`, naming how many do and the worst one: its cell, both states or both levels, the configuration, the eye, its distance, the shift and the threshold. The prescription is to put the difference farther from the eye, shorten the offset, or make the sections the same.

The rule judges every direction. A body crossing a slab may face any way (spec-0086 §2 keeps its facing, whatever it was). The engine cannot know which way a player looks at the jump, and a player fleeing something looks back.

## 4. The threshold

| Datum | Value | Kind |
|---|---|---|
| Measure | the shift of §3.1: `atan2(|u × v|, u · v)` for `u = p − (e′ − d)`, `v = p − e′`, in degrees, over a cell's nine points and the judged eyes that see it | authored |
| Threshold `FAR_FIELD_SHIFT_DEGREES` | **1.2852°** | measured (§5), rounded up at the fourth decimal |
| Calibration geometry | station 4 (§5.1) | cited (`gen.py` at `2bbb1f28`) |
| Calibration reading | 1.285125° at spike `[4095, 64, 4260]`, from the eye `[4097.8, 65.62, 4206.8]` | measured, two methods (§5.2) |
| What the reading is | the lit area of the walking course beside the west wall, 42 blocks past the slab and 54 past the landing: light 5 there against 3 at its image, because the lamp a bay past the last one is missing | measured |
| Near range | derived (§3.3): 10.6 blocks for a 6-block offset, 13.8 for 12 | derived |
| The one perceptual observation | station 4 walked forward on a client, read as seamless | cited (the spike's record) |

**Why this is the threshold** (authored). Station 4 is the only loop on record a person walked on a client and judged. Every shift its far field makes was on screen during that walk and was not seen. Its largest shift is therefore the largest one known to go unseen, and a geometry whose every far difference shifts no more than that shows nothing station 4 did not. The threshold is not a perceptual constant from the literature and claims no general truth about vision. It is the edge of the evidence, and it moves only when new evidence (another walked geometry) does.

## 5. The calibration

### 5.1 The geometry

**Cited**, with each departure named. `tests/common/station4.rs` writes the spike's hall at the spike's own coordinates less a fixed origin. Its walls are stone brick, `x 4094..4098`, `y 64..67`. The passage is `x 4095..4097` with a dark oak plank floor and red carpet down the centre. A polished deepslate pillar pair and a hanging soul lantern stand every 6 blocks on the spike's phase (`z ≡ 4190 mod 6`), up to a full period short of the far end. The slab is the spike's selection, `x 4095..4097, y 64..66, z 4218`, and the landing is 12 back. Past the far end is station 5's shell, floor, catalysts, sensors and shrieker. Its departures:

- **The approach.** The spike's hall began 30 blocks behind its slab, open in daylight. A body crossing facing back sees that mouth 18 blocks off after the jump and 30 before, the same jump as an end 18 ahead, and the spike's walk faced forward and never judged it. The calibration hall starts 84 behind the slab (72 behind the landing), farther from every eye than the far end, so its largest shift is read from the view the walk had.
- **The rooms.** A small lit porch behind the rear mouth. In the end room, station 5's pool, ceiling veins and entities are left out, and four lanterns hang under its roof so a body can stand in it. The hall's own darkness between its last lamp and its doorway is admitted by the area's `night-vision` mitigation (`DW0210`), not by a block.

### 5.2 The reading, by two methods

| Instrument | Reading |
|---|---|
| `delvec` at this spec's branch: `station_4_calibrates_the_far_field_threshold` (`tests/endless_corridor.rs`), the binding line and `loop-gate.json`'s `far_largest_shift_at` | 1.2851° (printed to four decimals), cell `[4095, 64, 4260]`, eye `[4097.8, 65.62, 4206.8]` |
| `tools/spike-seamless-loop/far_field.py`: the hall rebuilt from `gen.py`'s constants, its own block-light flood (lantern 15, soul lantern 10, catalyst 6, sensor 1, decrement 1 through non-opaque cells), its own voxel walk (Amanatides & Woo), no engine code | 1.285125°, the same cell, the same eye, light 5 against 3 |

The second method agrees at the far ends either side of the calibration: 54 past the slab, 1.691003° (engine 1.6910, refused); 56, 1.285125° (engine 1.2851, green); 62, 1.009820° (engine 1.0098, green). `tools/tests/test_far_field.py` runs it and holds its reading against the engine's constant.

### 5.3 What the threshold admits and refuses

**Measured** with the engine at this spec's branch over the station-4 hall, its far end moved, an offset of 12 or 6, the exit unlit or lit (its doorway lintel glowstone). The largest shift is in degrees, and "refused" is `DW0947` unless marked.

| Far end past the slab | 12, unlit | 12, lit | 6, unlit | 6, lit |
|---|---|---|---|---|
| 18 | refused `DW0946` (near) | refused `DW0946` (near) | — | — |
| 36 | 5.4506 refused | 5.9692 refused | 2.0431 refused | 2.2063 refused |
| 42 | 3.4014 refused | 3.6523 refused | 1.3583 refused | 1.4460 refused |
| 48 | 2.3257 refused | 2.4658 refused | 0.9674 | 1.0198 |
| 54 | 1.6910 refused | 1.7771 refused | 0.7236 | 0.7574 |
| 60 | **1.2851 (station 4)** | 1.3418 refused | 0.5615 | 0.5845 |
| 66 | 1.0098 | 1.0491 | 0.4483 | 0.4647 |
| 72 | 0.8145 | 0.8429 | 0.3662 | 0.3782 |

So a goal that never comes nearer stands about 60 blocks past the slab for a 12-block jump, and about 48 for a 6-block one. A lit exit needs a few blocks more than an unlit one, because its light reaches back along the walls. A goal a dozen blocks ahead is inside the near field of every offset a 6-block bay allows, and is refused. The spike's hall as built, its rear mouth 30 behind the slab, is refused at 3.7302° by that mouth (`DW0947`), read from a catch-band eye.

## 6. Bodies and volumes

**Authored.** A compiler-placed body in the span is judged by where it stands. Inside the near field it is `DW0948`, unchanged: a body has an identity the move cannot repeat. Past it, a body is a far feature, and its shift is measured over its feet and head cells from every judged eye, seen or not. Over the threshold it is `DW0947`, naming the body. On station 4, a figure posted in the end room is admitted, and the same figure 30 blocks down the hall is refused.

A declared volume's tiling (spec-0086 §4.6) is judged in the near field. A volume is not seen; the blocks that show a pit are judged as blocks. The tiling exists for the bodies the loop carries, which are put down and walk in the near field. **This is a loosening** of spec-0086 §4.6: a lethal or teleport volume past the near field without its image is no longer refused.

## 7. What this loosens and what it tightens

**Declared, in these words.** spec-0086 §4.1's claim (*everything the body could see from `p + d` is what it could see from `p`, cell for cell*) is **loosened** to *the near field is cell for cell, and the far field differs by no more than station 4's shift*. spec-0086 §4.6's volume arm is **loosened** to the near field (§6), and its body arm is **loosened** to admit a far body under the threshold. The eyes are **tightened** by the catch band (§2). The refusals of a view into sky, out of the build, or past `SPAN_REACH` are unchanged.

## 8. Refusals, the binding, the ledger

| Code | Shape | Names |
|---|---|---|
| `DW0946` | a near-field difference, in block or in light | as spec-0086, plus the near range |
| `DW0947` | three shapes: open view (unchanged); **far field**, N far differences over the threshold; **far body** over the threshold | open view: the eye, its fog end, the open cell, the face. Far field: the worst difference's cell, both states or levels, configuration, eye, distance, shift, threshold. Far body: the body, its cell, eye, distance, shift, threshold |
| `DW0948` | a body in the near field | as spec-0086, plus the near range |

**Binding** (cited form, spec-0086 §8, extended): `loop binding: L loop(s); slab cells S; eyes E (fog end F..F′ blocks as the kernel reads it); span B cells closed in G steps, frontier cells closed by geometry Cg and by fog Cf, open faces 0; visible cells V compared as blocks and as light at 2 skies over C configuration(s), N of them in the near field (R..R′ blocks); far-field differences D, largest shift X° of T°; volumes in span M, bodies in the near field 0, bodies in the far field K; forced route meets … , exercise steps …`. It is printed on every build that declares a loop, refusals included, **at whichever pass refuses**: the world-edits batch replay prints its own binding before a refusal it raises. A batch it admits prints nothing, and the final build prints its own, so a build prints one binding line. `validation/loop-gate.json` gains `far_field: "spec-0090"`, `far_field_shift_degrees`, and per row `near_range`, `near_visible`, `far_differences`, `far_largest_shift_degrees`, `far_largest_shift_at {cell, eye}` and `bodies_in_far_field`; its `unchecked` gains the mid-jump eye.

## 9. The gallery

**Authored**, under spec-0039. The gallery's own loop sees no far field: its view closes inside one bay, and its binding says `far-field differences 0`. That zero is reported, not passed: the far field is bound by the gallery's long hall (`area/long-hall`, `prefab/gallery-long-hall`, `loop/the-hall-that-runs-on`), a straight station-4 hall with a 6-block offset and a lit exit 54 blocks past its slab, whose binding admits a non-zero count of far differences. No route enters that area, so the route proof reports its loop as unmet (`DW0950`, an advisory); its world proofs run like every loop's. Probes, each the gallery plus one declared edit: `an-exit-too-near-to-hide` (a wall across the long hall 36 blocks past the slab, `DW0947` far field, raised at the world-edits replay) and `a-figure-down-the-long-hall` (the hall moth posted 16 blocks past the slab, `DW0947` far body).

## 10. What the engine does not check, by name

**Authored.** Added to spec-0086 §10's list and to `loop-gate.json`'s `unchecked`. (6) **A body caught mid-jump.** Its eye stands up to 1.2 blocks higher in a three-high passage. The calibration walk was a walk, so a jumping eye has never been judged by anyone, and the threshold is not extended to it. The rest is stated without a ledger line. The threshold is one person's single client walk of one geometry. It says nothing about a different field of view, a different render distance, a moving camera or a different block palette; a second walked geometry is how it moves.

## Acceptance criteria

Checked against this spec's branch. None of 1–9 holds at `e258b9f0`.

1. **The calibration.** `station_4_calibrates_the_far_field_threshold` builds station 4 green, reads its largest shift off the binding line, and asserts it is at most `FAR_FIELD_SHIFT_DEGREES` and within `2e-4` of it. It asserts `loop-gate.json`'s `far_largest_shift_degrees` is `1.2851` at the cell one west of and 42 past the slab's crossing cell, and that the near range is `13.8`. `tools/tests/test_far_field.py` runs `far_field.py` for station 4 and asserts `1.285125` and that the engine constant is that rounded up at the fourth decimal.
2. **The near field.** Station 4 with its far end 18 past the slab is `DW0946` naming the near field.
3. **The far field.** Station 4 with its far end 54 past the slab is `DW0947` naming *a light far off reaches it*, a shift over the threshold, and a non-zero far-difference count on the binding.
4. **A lit exit.** Station 4 with a glowstone lintel 66 past the slab builds green under the threshold. The same exit 60 past is `DW0947` naming the light.
5. **The approaching eye.** The spike's hall as built is `DW0947`, and the eye its message names stands 0.3 short of a whole-block face (the catch band). The binding states 24 eyes.
6. **Bodies.** An NPC posted in station 4's end room builds green with `bodies in the far field 2`. The same NPC 30 blocks down the hall is `DW0947` naming it. The long-gallery fixture's NPC in the hall stays `DW0948`, with `bodies in the near field 2`.
7. **Each gate reds alone.** Opening the threshold to 90° reds criteria 1–6. Making the near field empty reds criterion 2 alone. Removing the catch band reds criterion 5 alone. Judging a light at its own cell reds criteria 1, 3 and 4 (recorded in the implementing commit).
8. **The binding at the batch replay.** A world-edits batch that stands a lantern in the landing bay is refused `DW0946` *after world-edits batch*, and a `loop binding:` line precedes the refusal.
9. **Unit.** `shift_at` is zero on the offset's line, equals the two-sightline angle beside the eye, and tends to `atan(|d| / r)` far off. `near_range` brackets the threshold within 0.01 blocks for offsets 6, 12 and 24 and grows with the offset.
10. **Gallery** (spec-0039). The gallery's binding admits a non-zero count of far differences on `loop/the-hall-that-runs-on` with its largest shift under the threshold; the two probes of §9 are committed and each refused by its named code under `tools/ci/check-gallery-coverage.py`; the regenerated `gallery/baseline/` attributes every moved row.
11. **Docs and skill.** `docs/reference/compiler.md` states the far field in the loop's *Seamlessness* record, the binding, the `DW0946`/`DW0947`/`DW0948` rows and the ledger fields. The `/new-delve` skill's endless-corridor step states the near/far rule with §5.3's distances. `docs/specs/README.md` carries this spec's row. `docs/demo-levels.md` carries the row this spec queues.
