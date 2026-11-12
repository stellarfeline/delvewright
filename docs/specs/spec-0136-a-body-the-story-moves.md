# spec-0136: A body the story moves — a staged body hops, takes a pose, turns, holds an item and changes size, a particle and a sound happen where it stands, and an effect of that kind repeats and staggers across a cast

- **Status**: Proposed
- **Ground**: written against `feature/festival-day` at `3f68c4d52` (`delvec 1.16.0`, `dsl 0.74.0`), read for: the envelope (`when`, `audience`, `in`) and `EffectAudience` in `crates/dsl/src/quest/effect.rs`; `Verb`, `addresses_players`, `addresses_bodies`, `changes_world`, `ParticleAt`, `SoundAt` in `quest/verb.rs`; `BodyRef`, `BodyWatch`, `body_sites`, `body_watch_sites` in `crates/dsl/src/body.rs`; `EquipSlot` in `wave.rs` and the fit rule `equipment::judge` (`DW0898`); `perception_checks` (`DW0942`); `npc_summon_rows` and the walk drivers `movenpc_fns` in `crates/delvec/src/compiler/emit/npc.rs`; `actor_puppet_summon` and `actor_fns` in `emit/actor.rs`; `emit_quest_effect`, `body_selector`, `emit_play_sound`, `emit_particle` in `emit/effect.rs`; `sequence_sites` and `sequence_fns` in `emit/sequence.rs`; the watch and its yield in `compiler/watching.rs`; the unseeded-score rule (`DW0495`) in `compiler/seeding.rs`; the root `tick` and `load` in `emit/functions.rs`; spec-0039, spec-0085 §3, spec-0101, spec-0134 §9.
- **What it is for**: a story's crowd that is glad. A festival day ends on a firework show with the townsfolk gathered under it, hopping and bouncing for joy. A campaign can stand a body in the world and walk it; it cannot make a standing body move where it stands. This spec gives a staged body the moves the pinned game has, each as the one vanilla primitive that makes it, and gives an effect of that kind a cadence, so a crowd moves as a rolling wave and not as one frame.
- **Research**: `docs/reference/npc-moves.md`, measured on the pinned server and read from the pinned jar. Every vanilla fact below is **[cited]** from it by section; every rule is **[authored]** here.
- **Numbers**: spec `0136`; `dsl_version` `0.75.0`; `DW1101`, `DW1102` and `DW1103`, all three used (§7). `critical-path.json` format 10 was handed and is not used: no harness step changes (§8).
- **Non-goals**: a "celebrate" verb (a primitive is a mechanism, never what it is for — the crowd's joy is a creator's arrangement of these moves); a move the game does not have (§9); a body the story does not stage (a wave's mob moves by its own AI); any campaign's ending.

## 1. The body, and how an effect names it **[authored, on cited facts]**

**[cited]** (`body.rs`, `BodyRef`): the engine's staged bodies are two classes, a stage-2 NPC and a stage-5 actor; each stands on a mark, is walked by a compiler-emitted route, and carries the tag the engine addresses it by. A wave mob is deliberately not one: it is driven by the game's AI. **[cited]** (spec-0134 §9): the body an effect acts on is its `audience` — `{"wave": id}` or `{"actor": id}` on `give-effect`, `clear-effect` and `damage`.

**[authored]** A move is addressed the same way, so there is one way to say *this body*. `audience` gains two arms: **`{"npc": id}`**, one NPC, and **`{"bodies": [{"npc": id} | {"actor": id}, …]}`**, a cast in the order written. A move's audience is one of `{"npc"}`, `{"actor"}`, `{"bodies"}`; anything else — absent, `party`, `actor`, a wave — is refused under `DW0942`'s rule (an audience the verb cannot address) with the move's own message. The split by what a verb can address is now three-way: `give-effect`, `clear-effect` and `damage` address a wave's and an actor's bodies and never an NPC (an NPC is never harmed and never fights — `DW0942`, alone or inside a cast); the six moves address staged bodies and never a wave's; every other verb addresses players. `in` is refused on a move as on any verb that addresses no player.

**[authored]** *Which body of an actor.* A move that writes a position or a facing (`hop`, `pose`, `turn`, `face`) addresses the body as the story stood it: an NPC's body, or an actor's puppet (`dw_pup_<id>`), never the twin an `unleash-actor` frees — that one moves by its own AI and a written position would fight it. A move that writes neither (`hold`, `resize`) and the place of a particle or a sound address the actor's body whoever wears it, puppet or twin (`dw_actor_<id>`): a boss that grows as it stands up is one `resize`. An NPC's click box is never addressed by any of them: the place a player clicks stays where it was.

## 2. What the game gives a body **[cited]**

From `docs/reference/npc-moves.md`: a hop by releasing `immovable` and writing `Motion`, or by one teleport per tick (§1, §9); five poses a mannequin keeps (§2); a facing by `rotate`, head and body together (§3); an item in any of eight slots by `item replace` (§4, §9); a size by the `scale` attribute, 0.0625–16, sent to a client every second tick (§5); particles and a sound at the body by `execute at` (§7). It gives no arm swing, no wave of a hand, and no turn of the head apart from the body (§3, §6).

## 3. The moves **[authored, each on its cited primitive]**

### 3.1 `hop {height?}`

The body rises and falls along the game's own jump and lands where it stood. `height` is the peak above its feet, 0.25–4 blocks; absent is the game's jump (1.25 blocks, 12 ticks).

**The primitive is the teleport, and the record says why.** **[cited]** (§9): one relative teleport per tick, on a body that stays `immovable`, moved a mannequin on a full block, a mannequin on a slab and a `NoAI`, `NoGravity` villager through the same arc and returned each to its starting height bit for bit, with `immovable` reading `1b` on every logged tick and its facing unchanged. **Rejected: release `immovable`, write `Motion`, lock again.** **[cited]** (§1, §9): `Motion` does nothing at all to a body that is `NoAI` and `NoGravity` — every unskinned NPC and puppet — so it is not one primitive for the class; two released mannequins standing 0.3 blocks apart pushed each other 1.08 blocks apart in two seconds, so a released crowd does not stand where it was put; the landing is the floor's to decide and the re-lock is a tick count, so a body whose flight is cut short or lengthened is frozen wherever that tick finds it; and a body released for twelve ticks is, for those ticks, a loose body. The teleport has none of these, so the body is never released at all.

**The arc is exact.** `dsl::moves::hop_arc` computes the game's jump (impulse 0.42, gravity 0.08, drag 0.98 — the record's measured heights) and rounds each tick's height to 1/1024 of a block. A step is then a binary fraction the game adds to a height without rounding, the steps sum to exactly zero, and no height is below the start. For a `height`, the impulse whose peak is that height is found by bisection.

**The driver.** A `hop` sets the body's own count (`dw.hop`, the arc's ticks) and its arc's number (`dw.hop_arc`), only if the body is not already counting (`unless score @s dw.hop matches 1..`); the root `tick` calls `hop_tick`, which runs `hop_step` as every body with a count, which moves it by that tick's step and counts down. So a body asked to hop while it is in the air does not hop, and a hop survives its chunk unloading: the count is the body's, and it resumes when the body is ticked again.

**A walk and a hop never hold one body together.** A walk writes the body's whole position every tick; the steps of a hop that outlived it would be added to its last waypoint and leave the body under the floor. So a `hop` on a body any `move-npc` / `move-actor` leg is carrying is not taken (it asks each of that body's leg latches), and the start of every leg of a body that some `hop` names resets its count: the walk takes the body and its first waypoint puts it on the ground.

**A hop passes through no block** (`DW1103`). A teleported body collides with nothing, so a ceiling lower than the arc does not stop it: the head goes into the block and comes out again, and a player sees a figure sink into a beam. The compiler knows the arc and the room. For every `hop` and every body it names, at the cell the body is placed on (its mark, `Plan::body_point`), `compiler::hop` sweeps every cell above the cell its feet stand in, up to the cell the top of its body reaches at the arc's peak (the peak plus the body's own height, `nav::entity_dims`), in every column its width covers; a cell holding a block a body cannot pass (`blockshape::passes_body`) refuses the build, naming the hop, the body, the cell and the clearance. The game's own jump carries a 1.8-block body's top 3.05 blocks above its feet, so in a room three blocks high it is refused and a hop of one block is not. The sweep is at the placed cell: a body a walk has taken under a lower ceiling is not judged there, and the body's size is its entity's — a `resize` is not followed (§9).

### 3.2 `pose {pose}`

`standing`, `crouching`, `swimming`, `fall-flying`, `sleeping` — **[cited]** (§2) the five `Mannequin.VALID_POSES` admits; any other name is accepted by the game and silently becomes `standing`, so the set is closed in the schema. `data merge entity <body> {pose:"…"}`; the pose is kept until the next `pose`. Only a skinned body ships as a mannequin: a `pose` naming any other body is `DW1101`. A bounce is `crouching` and `standing` in turn.

### 3.3 `turn {by}` and `face {to}`

`turn` rotates the body `by` degrees from the way it faces (not 0, within ±180; positive is clockwise seen from above): `rotate @s ~<by> ~`. A spin is a `turn` that repeats. `face` turns it `to` a yaw in degrees (the pitch levelled), to a mark (`anchored eyes … rotate @s facing <the mark's cell centre>` — the head tilts to it, so a crowd looks up at a burst), or `to: "watch"`. **[cited]** (§3): head and body turn together; there is no head alone.

**A watching body.** **[cited]** (spec-0101, `watching.rs`): a body that watches is re-faced to its player every tick it carries `dw_watch`, and yields that tag to a walk. **[authored]** A `turn` and a `face` to a yaw or a mark take the tag the same way a walk's start does, so the turn is not undone on the next tick; `face` `to: "watch"` hands it back, as does the arrival of the body's next walk. `to: "watch"` naming a body that has no watch, declared or default, is `DW1101`.

### 3.4 `hold {slot, item?}`

`item replace entity <body> <slot> with <item>`; `item` absent empties the slot. `slot` is the engine's one `EquipSlot` (**[cited]** §4, §9: all eight are accepted on a mannequin). The item is judged by the rule an actor's declared `equipment` is (`DW0898`: the game draws that item in that slot on that body), against each body named; an id the game does not hold is `DW0143`. The item is a bare id: a stack with components is not carried here (§9).

### 3.5 `resize {scale}`

`attribute @s minecraft:scale base set <scale>`, 0.0625–16 (**[cited]** §5: outside it the game clamps, so the body would not be the size written — `DW0100`). Kept until the next `resize`. An NPC's click box keeps its size.

### 3.6 A particle and a sound at a body

`particle`'s `at` gains **`{"bodies": […], "up"?}`** and `play-sound`'s `at` gains **`{"at": "bodies", "bodies": […]}`**: the effect happens at each named body wherever it stands when the effect fires (`execute at <body> run …`), `up` blocks above its feet, seen and heard by the effect's audience of players as at a mark. `play-sound`'s `at: {"at": "actor"}` arm, which every build refused with `DW0335` because no position resolved for a live body, is this arm now: the arm and the code are removed.

## 4. The cadence **[authored]**

Two fields on the envelope, beside `audience`: **`repeat {times, every_ticks}`** — the effect happens `times` times, `every_ticks` apart, the first when it fires — and **`stagger_ticks`** — each body the effect names starts that many ticks after the one before it, in the order written. The bodies an effect names are its audience's when that names staged bodies, else those its `at` names.

**Which verbs keep one.** `Verb::keeps_cadence`: the six moves, `particle` and `play-sound`. None leaves anything a build's proof reads, so its happening again changes no verdict. Every other verb happens once (`DW1102`): a gate, a flag, a spawn or a rocket is read by the proofs as one event, and its repetition is steps of a `sequence`. **Rejected: `repeat` on `sequence`.** The build's proofs read a timeline's steps as each happening once (`rg 'Verb::Sequence'` over `crates/` lists them: `link`, `timeline`, `camera`, `promise`, `continuity`, `daylight`, `blind`, `combat`, `hold`, the route triggers, the manifest, the ending PackTests, `cutscene`, `perception`, `ordering`, `quest::check`); a repeat there multiplies world-changing effects under proofs that count them once. Recorded as the wider site this cadence would move to when those proofs can count (§9).

**What is refused** (`DW1102`): a cadence on a verb that happens once; `stagger_ticks` on an effect naming fewer than two bodies; a cadence beside `audience: actor` (a repeated effect runs from the scheduler, which has no acting player); and `every_ticks` shorter than the move — a hop's own arc (a body asked to hop before it lands does not hop, so fewer hops would happen than are declared), and the two ticks a size is held before a client is told (**[cited]** §5).

**Emission.** A cadenced effect is one call to `cad_<root>_<n>`, named by position as a timeline is. Its start function calls or schedules one function per group of bodies — the whole cast, or with a stagger each body at its delay; a group's function fires the effect for its bodies from the server command source and, when it repeats, counts down and re-schedules itself `every_ticks` later. `schedule` replaces a pending call of the same function, so an effect fired again while it runs restarts instead of doubling.

## 5. What stays true of a body that moved **[authored]**

It stands where it was put: a hop ends on the height it left (§3.1) and writes no other coordinate; no move here writes `x` or `z`. It never reads as loose: no line this spec emits writes `immovable`, `Motion`, `NoAI` or `NoGravity`. Both are proven once on the pinned server (the record's §9) and held by a compiler test over the emitted functions (§10.3).

## 6. Layers **[authored]**

| check | judges | layer |
|---|---|---|
| the hop's arc returns exactly, `immovable` never moves, the eight slots, `rotate`, `scale`, `Motion` on a `NoAI` body, released bodies pushing | vanilla behaviour | established once, `docs/reference/npc-moves.md` §9 |
| each move's command, the hop driver, the walk interlock, the cadence functions | the compiler's output | `crates/delvec/tests/it/body_moves.rs` |
| the arc's arithmetic | the compiler's output | `crates/dsl/src/moves.rs` unit tests |
| `DW1101`, `DW1102`, and the reused refusals on each shape | the compiler's output | `body_moves.rs`, and one gallery probe per code |
| every emitted command against the pinned command tree, and every score read against a seeded entry (`DW0495`) | the compiler's output | the emitter's own passes, on every build |
| `DW1103`: the swept cells, the refusal and its clear neighbours | the compiler's output | `body_moves.rs`, and a gallery probe |
| a declared hop moves its body and returns it | a campaign's content, on a running server | the generated PackTest `hop_<n>`, one per `hop` the campaign declares |

**The witness.** That a campaign's hop happens is a fact about that campaign on a running server — its body is there, its start line selects it, the driver the pack ships steps it — so it is read there, by PackTest. Per declared `hop`, `hop_<n>` summons the first body it names through that body's own entrance, starts the hop by the effect's own emitted line for that body, calls the pack's own `hop_tick` once per tick of the arc, and reads the body's `Pos[1]` in 1024ths three times: counting the arc's ticks after the start; exactly the arc's peak above where it stood on the peak tick; exactly where it stood, counting 0, after the last. A hop that never fires fails the first or the second reading. It calls `hop_tick` and not `tick`, which would run the campaign's content beside it; that `tick` calls `hop_tick` is the compiler's own test. No bot step is added.

## 7. The rules **[authored]**

| code | module | refuses |
|---|---|---|
| `DW1101` | `dsl::moves` | a `pose` naming a body that does not ship as a mannequin; a `face` `to: "watch"` naming a body with no watch |
| `DW1102` | `dsl::moves` | `repeat` or `stagger_ticks` on a verb that happens once; `stagger_ticks` with fewer than two bodies; a cadence beside `audience: actor`; `every_ticks` shorter than a hop's arc or a size's two ticks |
| `DW1103` | `compiler::hop` | a `hop` whose arc carries the body through a solid block at the cell it is placed on (build) |

Reused: `DW0942` (a move with no staged body; an NPC in the audience of an effect that harms), `DW0112` (a body no stage declares), `DW0100` (a `height`, `by`, yaw, `scale`, `times`, `every_ticks` or `stagger_ticks` outside its schema bound; an empty or repeating body list), `DW0143` (an unknown item), `DW0898` (an item the game does not draw there), `DW0360` (a `face` at a mark whose anchor resolves to no position, at build), `DW0495` (the emitter's own: an arc's number is never 0, so a body with no entry matches no arc). Removed: `DW0335`.

## 8. The gallery, the harness and the pages **[authored, under spec-0039]**

The primary binds every unit the schema gains: each move with its fields, the two audience arms, both `at: bodies` arms, `repeat` and `stagger_ticks`, on the near hall's bodies; each bound element moves an emitted byte when its declaration is perturbed. One probe per code of §7, named for what it does wrong, and the `sound-at-actor` probe goes with its code. The gallery's PackTest suite carries `hop_<n>` for each of its hops. The harness is unchanged and `critical-path.json` keeps its format: no step observes a move. Pages: `compiler.md` (the stage-5 rows, the emission rows, the contract), `dsl/moves.md` (`DW1101`, `DW1102`), `delvec/compiler/hop.md` (`DW1103`), `dsl/perception.md` (`DW0942`'s new shapes), `delvec/compiler/atmos.md` (less `DW0335`), `docs/reference/npc-moves.md` (the vanilla record), the `/new-delve` page `references/quest-capabilities.md` (*A body that moves*).

## 9. What it leaves out, by name **[authored]**

An **arm swing**, a wave of a hand and a **turn of the head alone**: the pinned game has no command that makes one (**[cited]** §3, §6), and a move with no primitive is excluded, not approximated. The **hurt reaction**: the one limb motion a command causes costs the body health and reads as a blow. A **glow**, a **speech bubble** (`description`), a mirrored main hand, a hidden skin layer, a changed skin: each is measured in the record (§7, §8) and none was asked for here. A **stack with components** in `hold`. A hop's headroom **anywhere but the body's placed cell**: a body a `move-npc` / `move-actor` has walked elsewhere hops there unjudged, and a body a `resize` has grown is swept at its entity's own size. Whether a pose, a 12-tick arc or a spin **reads** as joy on a client: the record states what a client is told, never a picture, and the playtest judges it. A move by a **wave's** body. `repeat` on a **`sequence`**, a rocket or any verb that happens once (§4). `in` on a move.

## 10. Acceptance criteria

1. **Surfaces.** `delvec schema --stage all` exports `hop`, `pose`, `turn`, `face`, `hold`, `resize`, `EffectAudience`'s `npc` and `bodies` arms, `ParticleAt`'s and `SoundAt`'s `bodies` arms and the envelope's `repeat` and `stagger_ticks` at `dsl_version` 0.75.0 with the bounds stated; `SoundAt` has no `actor` arm; every document the repository holds parses.
2. **Each move.** `crates/delvec/tests/it/body_moves.rs`: `each_move_emits_its_one_measured_command` reads the exact line of `pose`, `turn`, `face` (yaw and mark), `hold` (an item and an emptied slot, on an NPC and on an actor's body), `resize`, `particle` at a body with `up`, and `play-sound` at a body; `a_turn_takes_a_watching_bodys_facing_and_watch_hands_it_back` reads the yield line immediately before the turn and the tag's return.
3. **The hop.** `a_hop_never_releases_the_body_and_ends_where_it_began`: the start line and its `unless`; `load`'s two objectives and `tick`'s one line; every arc line a guarded relative teleport whose running sum equals the arc's height on that tick, never negative, and ends at exactly 0; no emitted line writes `immovable` but the summon, and no hop function contains `Motion` or `data`. `dsl::moves`'s unit tests: the default arc is the measured jump within 0.001 on each of 12 ticks; every arc ends at 0 and never dips; a declared height is the peak; every step is an exact binary fraction. `a_walk_and_a_hop_never_hold_one_body_together`: each of the body's leg starts resets its count before its driver is scheduled, the hop asks each leg's latch, and — the perturbation — a campaign with no `hop` carries no `dw.hop` line at all.
4. **The cadence.** `a_cadence_repeats_and_staggers_from_the_scheduler`: three bodies staggered by 3 with five repeats emit a start of three counters and delays 0, 3, 6, each group function its body's line, the count down and its own re-schedule at `every_ticks`; an unstaggered `particle` is one group; the effect itself is one call.
5. **Refusals.** `a_move_names_a_staged_body_the_campaign_declares` (`DW0942` on absent, `party` and wave audiences and on an NPC under `give-effect`; `DW0112` at each of four pointers); `a_number_outside_its_bound_and_a_misfit_item_are_refused` (`DW0100` ×5, `DW0143`, `DW0898`); `a_face_at_a_mark_nobody_placed_is_dw0360` (and the same face at a placed anchor building); `dw1101_a_move_its_body_cannot_make` (both shapes, and the skinned cast passing); `dw1102_a_cadence_the_effect_cannot_keep` (six shapes, and the same hop and size at their own lengths passing).
6. **Stands where it was put, never loose.** `docs/reference/npc-moves.md` §9 states the server run: three bodies, 12 ticks, the starting `y` read back bit for bit, `immovable` `1b` on every logged tick. Criterion 3 holds the emitted order.
7. **The gallery.** `check-gallery-coverage.py` reports 0 in neither state with the units of §8 bound; the gallery analyzes and builds, printing its `hop binding` line with 0 refused; one probe per code of §7 is refused with its code.
8. **Headroom.** `body_moves.rs::dw1103_a_hop_through_a_block_is_refused`: the game's jump of a 1.8-block body at `[10, 64, 10]` sweeps exactly the cells at y 65, 66 and 67 of its column; a block at y 67 is `DW1103` naming the body, the cell and 3 cells of clearance; the same block one cell higher, one column over, or at or under the feet is clear, as is a 0.25-block hop under it and a torch in the arc; a 1.4-block-wide body sweeps nine columns and is refused by the neighbouring one. `dw1103_is_asked_at_the_bodys_placed_cell_by_the_build`: the fixture's keeper, in a hall three blocks high, is refused the game's jump at `[11, 68, 12]` by the build and builds with a hop of one block. A gallery probe is refused with `DW1103`.
9. **The witness.** `body_moves.rs::every_declared_hop_has_its_running_server_witness`: one `hop_<n>` per declared hop; its start line is a line the pack itself carries; `hop_tick` is called once per tick of the arc; the four assertions are the count, the peak, the return and the count of 0, in that order around the peak tick's call. The gallery's PackTest suite runs `hop_<n>` green on the pinned server, and the same suite over a build whose `hop_tick` steps nobody reds them.
10. **Codes and pages.** `check-dw-codes.py`, `check-diagnostic-messages.py`, `check-capability-ownership.py`, `check-effect-roots.py`, `check-skill-page.py` and `version_sites.py verify` are green; `DW0335` appears on no live page.

## 11. Measurements this spec states

Written into the commit bodies that land the implementation: the server run of §5 with its instrument; the distance two released bodies pushed each other; the gallery's coverage line before and after.
