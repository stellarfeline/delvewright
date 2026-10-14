# spec-0110: A shot frames its subject — every cutscene shot declares what it shows, and the build proves the subject is in the frame, in sight, present and drawn for the whole shot

- **Status**: Proposed
- **Ground**: written against engine `a845fa1145d050205488028de3235b1ff4f77e3f` (`main`), read only — `CameraShot`, `CameraSubject`, `ShotStyle`, `check_cutscene_shape` (`DW0199`), `cutscene_style_checks` (`DW0348`, `DW0349`) in `crates/dsl/src/cutscene.rs`; `Verb::Cutscene` in `crates/dsl/src/quest/verb.rs`; `Assembly` in `crates/dsl/src/assembly.rs`; `PlaceRef` in `crates/dsl/src/stealth.rs`; `despawned_ref_check` and `NPC_SPAWNED_LATE` (`DW0195`, `DW0198`) in `crates/dsl/src/npc.rs`; `expand_shot`, `resolve_subject`, `ExpandedShot`, `ShotFrames`, `AimTrack`, `cutscene_units`, `stand_in_open_air` in `crates/delvec/src/compiler/camera.rs`; `check_cutscenes` (`DW0308`, `DW0347`, `DW0956`) in `crates/delvec/src/compiler/nav/cutscene.rs`; `World::blocks_camera` and `walk_cells` in `crates/delvec/src/compiler/nav/world/body.rs`; `cutscene_fns` in `crates/delvec/src/compiler/emit/cutscene.rs`; `assembly_functions`, `Placed::world_cells`, `spawn_fn` in `crates/delvec/src/compiler/assembly.rs`; `place_bounds` in `crates/delvec/src/compiler/horizon.rs`; `Plan::zone_box` in `crates/delvec/src/compiler/plan/stealth.rs`; the `occluded` and `out_of_frame` rules of the snapshot manifest, the *Shot styles* table, the *Seamlessness* sight rule, the `view_distance` row and the `entity-broadcast-range-percentage` paragraph of `docs/reference/compiler.md`; `docs/notes/camera-dossier.md` §4; `docs/reference/showcase-shots.md` §2a. The pinned 1.21.11 game, read from the client jar (sha256 `1473c9489ac50fda3c435049a76a70d61a10b8610db27f5ba9d8756b686cd3bd`) and the dedicated server's bundled `META-INF/versions/1.21.11/server-1.21.11.jar` (sha256 `ec47239a8de246335e1d54f6ac319bd35641778eb4b6a6da06372840d02fcebc`), disassembled with `javap -c -p` (OpenJDK 17.0.20.1) and named through Mojang's official 1.21.11 mappings (`client.txt` sha1 `031a68bebf55d824f66d6573d8c752f0e1bf232a`, `server.txt` sha1 `5621e9253f05fd57872bbe7f8ddf5f9a7d525955`, both from the version manifest's `downloads`).
- **What it is for**: a cutscene shot that shows what it is meant to show. In the first full playtest of a 55-place campaign a whole cutscene framed only sea and sky, a shot was aimed at a stretch of ground where nothing happened, and shots meant to show a creature's assemblies showed nothing — while one assembly appeared only when a player stood on it. Nothing in the engine could have refused any of these: a shot cannot say what it frames, and no check reads the frame, the sight line, or whether the thing is there and drawn.
- **Research**: every game fact is read from the pinned bytes and marked **cited** with the method and class it was read from; the cinematography and game-camera practice is marked **cited** with its source; every rule built on them is **authored**.
- **Numbers**: `spec-0110`; **two DW codes**, `DW1033` (a shot that does not show its subject, §6.1) and `DW1034` (a body subject that is not there to be seen, §6.2). A shot with no subject is a shape defect and takes `DW0199`; a style on a landscape takes `DW0348`; an unknown npc, actor, assembly or place takes `DW0112`; a subject with two kind keys or an unknown key takes `DW0100` from the schema; a shot aimed past the served chunk distance keeps `DW0956`: no second code for a rule that exists. **`dsl_version` moves to `0.48.0`**; it numbers a surface and promises nothing.
- **Non-goals**: a compile-time search for a camera placement (the dossier's compile-time ClearShot, §3; the engine judges the shot the creator wrote and never moves it); a rule about composition beyond the frame's edge — rule of thirds, headroom, fill (§3 says why: no source gives a figure, and a check never restricts); a runtime raycast or any client-side device; a subject that is a player (the party is shown by stand-ins, spec-0095, and the camera cannot know where a player stands); the music a cutscene hears (spec-0109, §8.1).

## 1. The defect

**Finding, from reading the tree.** `CameraShot.subject` exists only for a styled shot and is refused on an unstyled one (`DW0348`); `look_at` aims the camera at a mark but declares nothing about what the mark is. `check_cutscenes` proves three things per shot — the dolly and the client's chords clear every solid (`DW0308`), the aim stays inside the served chunk distance (`DW0956`), the pan stays inside the angular budget (`DW0347`) — and nothing about the frame or the sight line; `compiler::camera`'s module doc names world-aware framing as future work. An assembly's parts are `block_display`s riding an `item_display` root, summoned by `asm_summon_<s>` with no `view_range`, so every client culls them at the game's default distance (§2.3); the cutscene emission neither summons nor reads an assembly. `delvec snapshot` already computes `occluded` and `out_of_frame` for every known target from a stated camera — the one place in the tree where the frame and the sight line are read — and no gate consumes it.

Which of the two causes — the cull distance or a spawn beat ordered after the cutscene — hid each assembly in the playtest is not established; this spec refuses both.

## 2. What the game gives

### 2.1 The lens is the camera entity's position

**Cited: `ServerPlayer.tick`, `Camera.setup`, `EntityType.<clinit>`, `EntityDimensions`.** While a player spectates an entity (`getCamera() != this` and the camera is alive), every server tick snaps the player to the camera's position and rotation (`absSnapTo(x, y, z, yRot, xRot)`) and moves the player's chunk tracking with it (`ServerChunkCache.move`): the player *is* where the camera is, for chunks and for entities alike. The client places its lens at the entity's interpolated position plus the entity's eye height; `block_display`, `item_display` and `text_display` are registered `sized(0, 0)`, and a sizeless body's default eye height is `0.85 × 0 = 0`, so the lens is the display's position exactly — the point the keyframe `tp` carries and the client tweens (`compiler::camera`, spike-measured).

### 2.2 The frame

**Cited: `Options.<init>`, `GameRenderer.getFov`, `GameRenderer.tickFov`, `GameRenderer.getProjectionMatrix`, `Main.main`.**

- The field of view is the client's `options.fov`: an integer in `30..=110`, default **70**. It is the **vertical** angle: the projection is `Matrix4f.perspective(fov × π/180, aspect, 0.05, far)`, and JOML's `perspective` takes the vertical angle; the horizontal angle follows from the window's aspect. The default window is **854 × 480** (`Main`'s `--width`/`--height` defaults).
- While the camera entity is not an `AbstractClientPlayer`, the movement modifier is exactly `1.0` (`tickFov`): a spectated display never sprints, flies or sneaks, so no speed-of-motion zoom applies to a cutscene. The dying-entity narrowing applies only to a `LivingEntity` camera; a display is not one.
- A camera whose cell holds **water or lava** narrows the angle by `lerp(fovEffectScale, 1.0, 0.85714287)`: at the default `fovEffectScale` 1.0, by 6/7 — 70° becomes 60°.
- Nothing on the server can read or set any of these. The frame is the viewer's.

### 2.3 What the client draws of a body

**Cited: `Entity.shouldRender`, `Entity.shouldRenderAtSqrDistance`, `Display.shouldRenderAtSqrDistance`, `Display.defineSynchedData`, `Display.readAdditionalSaveData`, `Display.updateCulling`, `Display.affectedByCulling`, `LevelRenderer` (the entity pass), `EntityRenderer.shouldRender`, `Options.<init>`.**

- **Every entity has a distance beyond which the client does not draw it.** The base rule: the body is drawn when `d² < (size × 64 × viewScale)²`, where `size` is `AABB.getSize()` of its bounding box — the mean of its three extents — and `viewScale` is a client-wide static. A mannequin (`0.6 × 1.8`, the NPC body the engine summons for a skinned NPC) has size 1.0 and vanishes at 64 × viewScale blocks; a villager or zombie (`0.6 × 1.95`) at 67.2 × viewScale.
- **A display entity replaces the size with its own `view_range`**: drawn when `d² < (view_range × 64 × viewScale)²`. `view_range` is a float synched to the client, default **1.0**, read from the NBT key `view_range` with no clamp. This is the one knob the server holds over how far a body is drawn, and only display entities have it.
- **`viewScale` is set by the client every frame**: `clamp(renderDistance / 8, 1.0, 2.5) × entityDistanceScaling`, where `renderDistance` is the client's effective render distance in chunks and `entityDistanceScaling` is `options.entityDistanceScaling`, a double in `0.5..=5.0`, default **1.0**. At the default, a display with `view_range` 1.0 is culled at 64 blocks on a client at render distance 8 or below, at 96 blocks at render distance 12, at 160 blocks at 20 or above.
- **A display with no `width`/`height` is never frustum-culled** (`affectedByCulling` is false when either is 0; the engine emits neither), so the distance rule is the whole of the client's decision for an assembly part; a body with a bounding box is also tested against the frustum on its box inflated by 0.5 — which cannot hide a body that the frame proof of §5.4 finds inside the frame.
- **A body is drawn only in a section the client has built and can see**: the entity pass skips an entity unless `isOutsideBuildHeight(y)` or `isSectionCompiledAndVisible(pos)` holds for its block position. A client whose own render distance is below the server's, or whose occlusion culling hides the section, draws nothing there, however the server tracks it.

### 2.4 What the server sends

**Cited: `ChunkMap.addEntity`, `ChunkMap$TrackedEntity.updatePlayer`, `getEffectiveRange`, `scaledRange`, `DedicatedServer.getScaledTrackingDistance`, `DedicatedServerProperties.<init>`, `EntityType.<clinit>`.**

- An entity is sent to a player when the **horizontal** distance (x and z only) between them is at most `min(effectiveRange, playerViewDistance × 16)`, the entity's chunk is tracked by that player, and the entity type agrees to be broadcast. `effectiveRange` is the entity type's `clientTrackingRange × 16`, raised to the largest of its passengers' ranges, then scaled by `entity-broadcast-range-percentage / 100`.
- `clientTrackingRange` is 10 chunks (160 blocks) for `block_display`, `item_display`, `text_display`, `interaction`, `villager` and `armor_stand`; 8 (128 blocks) for `zombie`; 32 (512 blocks) for `mannequin` and `player`; 0 for `marker`.
- `entity-broadcast-range-percentage` defaults to **100** and is clamped to `10..=1000` when read from `server.properties`.

### 2.5 The world the proof reads

**Cited: `compiler::nav`, `compiler::camera`.** Spike measurement 2 of `compiler::camera` holds: the server never sees the client's in-between poses, so the proof reads the client's — the keyframes `ExpandedShot::frames` emits and the linear tween of position and rotation the client draws between them. The sight rule already has one home: a segment is walked by `nav::walk_cells` (the exact 3-D grid walk `DW0308` uses) and is stopped by any cell `World::blocks_camera` holds for; the loop proof (*Seamlessness*) and the snapshot's `occluded` read the same rule.

## 3. What the craft says

- **An establishing shot shows the relationship between a scene's figures and objects.** "A shot that sets up, or establishes, the context for a scene by showing the relationship between its important figures and objects … a long or extreme-long shot at the beginning of a scene". **[cited]** Wikipedia, [*Establishing shot*](https://en.wikipedia.org/wiki/Establishing_shot); the same source grounds `showcase-shots.md` §2a. A landscape shot therefore *is* a declaration — "this shot is of the place, not of a thing in it" — and the engine takes it as one (§4.3), never as the absence of one.
- **A subject at the edge of the frame is not shown.** Broadcast production keeps essential action inside an **action safe area** 3.5% in from each edge of a 16:9 picture, and graphics 5% in. **[cited]** EBU, [*R 95 — Safe areas for 16:9 television production*](https://tech.ebu.ch/publications/r095). This is the one composition figure a source gives for "inside the picture", and it is the margin §5.4 uses; nothing in it says where inside the frame a subject should sit, and §5.4 says nothing about that either.
- **Game cutscene cameras are judged on two things: is the target visible, and is it unobstructed.** Cinemachine's ClearShot "guarantee[s] a clear view of the target" by having each candidate camera's "shot quality evaluator extensions analyze the shot for target obstructions, optimal target distance, and so on", and chooses the best. **[cited]** Unity, [*Cinemachine Clear Shot Camera*](https://docs.unity3d.com/Packages/com.unity.cinemachine@3.1/manual/CinemachineClearShot.html). Those systems *recover* from an obstructed shot at run time; this engine's world is static and its subjects move on compiler-planned tracks, so the same two questions are decidable at the build and a bad shot cannot ship (`docs/notes/camera-dossier.md` §4, *Visibility*). **[cited]** — the dossier's finding; the decision to judge rather than search is **authored** (non-goals).
- **No source gives a fill fraction, a headroom rule or a thirds rule as a pass/fail figure** (`showcase-shots.md` §5 records the same gap), and none is adopted: a check confirms a declared intent and never restricts a composition. **[authored]**

## 4. The surface

**Authored**, on §2 and §3.

### 4.1 Every shot declares its subject

`subject` becomes a field of **every** shot, styled or not, and is **required**; `subject_b` stays `two-shot`'s second subject. A subject is one of:

| Spelling | The object class | Reference point | Sight box |
|---|---|---|---|
| `{"anchor": "anchor/<a>", "offset"?: [x,y,z]}` | a mark | the cell's centre | the cell |
| `{"npc": "npc/<n>", "offset"?: […]}` | a body (stage 2) | the feet cell's centre, one block up (the torso, as today) | the body's two cells, feet and head |
| `{"actor": "actor/<a>", "offset"?: […]}` | a body (stage 5) | as `npc` | as `npc` |
| `{"assembly": "assembly/<s>"}` | an assembly (spec-0082) | the centre of its envelope | its envelope: the union of `Placed::world_cells` over every frame of every clip, turned to its `facing` |
| `{"place": "node/<p>"}` | a place of the site plan | the centre of `place_bounds` | the place's box |
| `{"region": {"anchor": "anchor/<a>", "extent": [x,y,z]}}` | a region of blocks | the centre of `Plan::zone_box` | the box |
| `"landscape"` | nothing: an establishing shot of the place the camera stands in | none | none |

A moving body (an `npc`/`actor` with a sibling `move-npc`/`move-actor`, `resolve_subject`'s rule) carries its reference point and sight box along its planned track per tick, as the aim already does. `offset` is in world axes, as every mark is (spec-0066).

### 4.2 The aim follows the subject unless told otherwise

A shot with a subject and no `look_at` aims at the subject's reference point (static or tracking), on an unstyled shot as on a styled one; `look_at` overrides the aim and nothing else; `"landscape"` with no `look_at` faces along the direction of travel. This is the rule `expand_shot` already applies to styled shots, now one rule for every shot; the v0.4 travel-aim default survives only for the landscape.

### 4.3 A landscape is a declaration, not an exemption

`"landscape"` passes §5's frame and sight proofs with a count of zero objects stated, as CLAUDE.md requires of a declared zero. It is refused under a `shot_style` (`DW0348`: a style builds its dolly from a subject's geometry and a landscape has none) and as `subject_b`.

### 4.4 An assembly says how far it is seen from

`Assembly` gains **`seen_from`**, optional, a distance in blocks (`1..=512`): the farthest a player's eye may stand from the mark and still see the thing. Its floor is derived, never typed (§5.8): the engine emits a `view_range` that covers every shot framing the assembly and every standable cell of the place it stands in, and `seen_from` only raises it — a creator who wants the figure in the sea seen from the far shore says so once, on the object.

### 4.5 What the existing codes now say

- `DW0199` (`check_cutscene_shape`) gains the shape **a shot with no `subject`**, in both spellings; a single-shot cutscene carries `subject` beside `path` and `seconds`.
- `DW0348` loses `subject` from its style-only list — `subject_b`, `dist`, `degrees` and `bearing` remain style-only — and gains **a landscape under a style** and **a landscape as `subject_b`**.
- `DW0112` extends from unknown npc/actor to an unknown `assembly` or `place`.

## 5. The proof

**Authored**, over §2's cited facts, in `compiler::framing` (a module of its own: the frame and the sight line are properties of a shot, and the showcase camera and the snapshot share them, §5.3).

### 5.1 The proof frame

The build judges every shot against **the client the game ships**: vertical field of view 70° on an 854 × 480 window (aspect 1.779), `entityDistanceScaling` 1.0 and the `viewScale` floor 1.0 (the render distances at or below 8 chunks; a higher render distance only draws farther), `fovEffectScale` 1.0, `entity-broadcast-range-percentage` 100. Where the lens's cell holds water or lava at a tick, the vertical angle for that tick is 60°. A player who changes any of these changes what every game's cutscene shows them; §5.9 names what that can hide and the binding line states how much slack each shot has.

### 5.2 The poses judged

Per shot, every driver tick `t ∈ [0, ticks]`: the camera's position and rotation at `t` are the keyframe pose when `t` is a keyframe tick, else the linear interpolation of position, yaw and pitch between the keyframes that bracket it — the client's tween (`compiler::camera`, measurements 2–3). A static shot has one pose. A moving subject's reference point and sight box at `t` are read from its track at `t`.

### 5.3 One sight rule

`compiler::framing::sight(camera_pose, fov_v, aspect, subject) -> Sight { in_frame, lines_clear: u8 }` is the one function that answers "is this inside the picture and in view", and `delvec snapshot`'s `occluded` / `out_of_frame` manifest fields are computed by calling it: one rule, no private copy per gate; `crates/delvec/tests/framing_snapshot_agree.rs` holds the two consumers to one result on the gallery's render plan.

### 5.4 In the frame

The reference point is projected through the proof frame; it is **in frame** when it lies in front of the near plane and inside the action-safe rectangle — `|x| ≤ 0.93` and `|y| ≤ 0.93` in normalised device coordinates (EBU R 95, §3). Nothing else about where it lies is judged.

### 5.5 In sight

Nine sight lines are walked from the lens to the sight box: its centre and the eight corners of the box inset by 0.1 block (the snapshot's rule). A line is **clear** when no cell it crosses is one `World::blocks_camera` holds for, excluding the subject's own cells (a mark that *is* a block — a campfire, a bell — must not hide itself; a place's walls must not hide the place). The subject is **occluded** at a tick when all nine lines are blocked; the count of clear lines per tick is a measurement the ledger keeps. Entities are not blocks and never occlude; the party's stand-ins, other bodies and particles are not in the model (§5.9).

### 5.6 Present

A body subject (`npc`, `actor`, `assembly`) is **present** at the shot's first tick when its placing is ordered before the cutscene on every path that reaches it, and no unconditional removal is ordered between. The ordering model is the one `dsl::npc` already uses for `DW0195` and `DW0198` — same effect list at an earlier index, an earlier `sequence` step on the same timeline, a quest-graph ancestor's `on_complete` or a prerequisite objective's bundle; a flag-gated effect and a reaction bundle (`on_respawn`, `on_caught`) are branches and count for nothing — lifted out of `dsl::npc` into a shared ordering function with one home, and extended to `spawn-assembly`/`despawn-assembly` and to an actor's spawn and despawn. An NPC placed at setup is present from the first tick; an assembly is never placed at setup (`spawn_fn` is called only by `spawn-assembly`), so an assembly subject always owes a `spawn-assembly` before the cutscene — a spawn in the same effect list ahead of the cutscene is one tick ahead of the first `spectate`, because `cs_<bare>` schedules `cs_tick_<bare>` for the next tick.

### 5.7 Sent

A body subject is **sent** to the viewer at a tick when the horizontal distance from the lens to the body is at most the server's tracking range for its type (§2.4: `clientTrackingRange × 16`, never above the served chunk distance — `view_distance × 16`, spec-0091). For an assembly the body is the root and every part, at their world positions in the pose the assembly is in. The engine does not raise `entity-broadcast-range-percentage` (§11): a subject farther than its tracking range is refused, and the remedy is the camera's distance.

### 5.8 Drawn

- **An assembly is made drawable by the engine.** `asm_summon_<s>` emits `view_range` on the root and on every part: `max(1.0, seen_from / 32, d_max / 32)` rounded up to two decimals, where `d_max` is the greatest distance over every shot framing the assembly, every tick, from the lens to each part's position, and over the farthest standable cell of the place the mark stands in (`place_bounds` of the node that owns the anchor, the nav model's standable set) from the mark. The divisor 32 is 64 × 0.5: the derived range holds at the client's *lowest* `entityDistanceScaling`, because the knob is free and the thing should not vanish for anybody. An assembly no shot frames keeps the derived floor from its own place.
- **A body with no knob is judged at the proof frame.** For an `npc`/`actor` the cull radius is `size × 64` with `size` the mean extent of its summoned type's bounding box (the pinned entity registry); a tick at which the lens is farther than that from the body is refused. The binding line reports, per shot, the `entityDistanceScaling` below which its farthest body subject vanishes (`distance / (size × 64)`), so a creator sees the slack.

### 5.9 What the engine cannot control, named

The viewer's `options.fov`, window aspect, `entityDistanceScaling`, `fovEffectScale` and render distance; the client's occlusion culling of the section a body stands in (§2.3, last bullet); a body hidden by another entity, a stand-in, a particle or weather; the client's frame rate between ticks. Each is listed by name in `validation/shot-subjects.json` under `unchecked`, as the loop ledger lists its own.

## 6. The codes

### 6.1 `DW1033` — a shot does not show its subject (build tier, `compiler::framing`)

A shot whose `subject` (or `subject_b`) at some judged tick is out of frame (§5.4) or occluded (§5.5). The message names the cutscene, the shot index, the subject, the tick, the lens position and aim at that tick, which of the two failed and — for occlusion — the blocking cell of the centre line; the remedies are the path, `look_at`, `bearing`/`dist` on a styled shot, or a hard cut into two shots. It never fires on `"landscape"`.

### 6.2 `DW1034` — a body subject is not there to be seen (build tier, `compiler::framing`)

A body subject that is not present at the shot's first tick (§5.6: no placing ordered before, or an unconditional removal between — the message names the placing and the removal it found and the order it needs), not sent at some tick (§5.7: the message names the tick, the horizontal distance and the type's tracking range), or, for an `npc`/`actor`, not drawn at some tick at the proof frame (§5.8: the message names the distance and the body's cull radius). An assembly never fails §5.8: the engine wrote its range.

## 7. The binding line and the ledger

Every build with a cutscene prints `shot subjects: S shot(s) in C cutscene(s), S declare a subject (L landscape, M marks, B bodies, A assemblies, P places, R regions); T tick(s) judged, 0 out of frame, 0 occluded, clear lines Q of 9·T; bodies: B+A present, sent, drawn — farthest F blocks, hidden below entity distance s` — every figure computed from the declarations with its denominator, zeroes included, `s` the smallest slack of §5.8 — and writes `validation/shot-subjects.json`: per shot the subject, its reference point, per-tick `in_frame` and `lines_clear`, the presence derivation (the placing effect's path and the ordering that reaches the cutscene), the farthest distance against the tracking range and the cull radius, every assembly's emitted `view_range` with the three terms it was the maximum of, and `unchecked` (§5.9). A record that claims to be a measurement carries the canonical hash of the shot list it measured (`cutscene_digest`).

**The live half.** The bot playthrough (`harness/`) is a client the server tracks at the camera: during each cutscene whose shots frame a body, the run report records whether that body (the assembly's root tag, the NPC's or actor's tag) was spawned to the bot before the shot's first tick and remained spawned through the shot, read from the bot's entity table — the §5.6 and §5.7 claims observed from the only seat that can observe them. What the client draws cannot be read from a bot; §5.8 is confirmed by eye on the demo level (§9).

## 8. The pairs

### 8.1 spec-0109 — a cutscene hears the place its camera stands in

The subject is what a shot *shows*; `hears` is what the camera's own cell *plays*. A camera standing in `node/quay` framing `{"place": "node/tower"}` hears the quay and shows the tower, and both proofs read the same per-tick poses (§5.2, `ExpandedShot::frames` and the tween). No field of this spec says anything about sound, and `hears` says nothing about sight.

### 8.2 spec-0106 — a region holds a whole site

A subject in another place is reached by naming it: `{"place": "node/<other>"}`, or a mark, body or assembly that stands there. The sight proof reads the assembled world across every place (the model is the whole map, as the loop proof's is), so a seam wall between the camera's place and the subject's is an occluder like any other; the served chunk distance (`DW0956`) and the tracking range (§5.7) bound how far across the map a subject can be; a scenery place (`reached: false`) is a legitimate subject — being seen is what it is for.

## 9. What the gallery, the record, the skill and the demo owe

- **The gallery element.** Every shot of the gallery's five cutscenes declares its subject, one of each kind across them: the muster shot keeps `{"anchor": "anchor/muster"}`; the stall shot frames `{"place": "node/near-hall"}`; the parade's `two-shot` keeps its two subjects; one shot frames `{"assembly": "assembly/sentinel"}` after its `spawn-assembly`; one frames a `region`; one frames an `npc` and one an `actor`; the marshal's approach shot is `"landscape"`. `assembly/sentinel` declares `seen_from`. Bound by perturbation: moving a framed shot's `look_at` one block moves its `cs_tick` yaw and the ledger's `in_frame`; raising `seen_from` moves the `view_range` byte in `asm_summon_sentinel`.
- **The probes**, the primary plus one declared edit each: `a-shot-of-sea-and-sky` (the muster shot's `look_at` lifted 20 blocks) → `DW1033`; `a-shot-through-the-hall-wall` (the keeper framed from the far side of the hall's wall) → `DW1033`; `a-shot-of-a-thing-not-yet-standing` (the sentinel's `spawn-assembly` moved after the cutscene) → `DW1034`; `a-shot-of-a-body-across-the-map` (the keeper framed from a path 200 blocks off under `view_distance: 20`) → `DW1034`; `a-shot-that-names-nothing` (`subject` deleted) → `DW0199`; `a-landscape-pushed-in-on` (`"landscape"` under `push-in`) → `DW0348`; `a-shot-of-a-place-the-plan-lacks` (`{"place": "node/nowhere"}`) → `DW0112`.
- **The record.** `docs/reference/compiler.md`: the `cutscene` row (every shot's `subject`, the seven kinds, the aim rule, `"landscape"`), the *Shot styles* paragraph (`subject` no longer style-only), the stage-5 `assemblies[]` row (`seen_from` and the emitted `view_range`), the `DW0199`/`DW0348`/`DW0112` quantifiers, `DW1033` and `DW1034`, the binding line and ledger under the build output, the `entity-broadcast-range-percentage` paragraph (why it stays 100), and the snapshot manifest's `occluded`/`out_of_frame` naming `framing::sight` as their one source; `docs/reference/tools.md` where the snapshot is inventoried. `docs/notes/camera-dossier.md` §4's *Visibility* item is closed by pointer.
- **The skill.** `references/quest-capabilities.md`: every shot says what it frames; a shot of the sea says `"landscape"`; a thing the shot shows is spawned by a beat before the cutscene; how far a thing is seen from is said on the thing. Held back until the release that carries this surface is pinned (`tools/ci/check-skill-page.py`, as spec-0109 criterion 12 has it).
- **The demo level**, a `docs/demo-levels.md` row queued by the implementing change: **The Thing Across the Water** — a jetty on an ocean horizon, a hall with a keeper behind it, and a figure of block displays on a sea-stack ninety blocks off that rises when a bell is rung. Three cutscenes: an establishing shot over the bay declared `"landscape"`; the bell's cutscene, which frames the figure as it rises — spawned by the beat before, drawn at the `view_range` the build derived; a shot of the keeper through the hall's open door. Looked at on the walk: whether the figure reads at ninety blocks, and whether it is still there with the client's entity distance at 50% and its render distance at 8. Beside it the refusals as transcripts: the bay shot aimed twenty blocks up (sea and sky, `DW1033`); the keeper framed through the hall's wall (`DW1033`); the figure's `spawn-assembly` moved after its cutscene (`DW1034`); the camera backed to two hundred blocks over `view_distance: 20` (`DW1034`, tracking); the keeper shot with its `subject` deleted (`DW0199`).

## 10. Acceptance criteria

`delvec` is this tree's binary, built from the crate manifests this tree carries. Each criterion is red on the ground revision by the shape named and green on the implementing tree.

1. **Surface.** `delvec schema --stage quests` exports `subject` as required on every shot in both cutscene spellings, with the seven kinds of §4.1 and `"landscape"`; `delvec schema --stage quests` exports `seen_from` on `assemblies[]`; `DSL_VERSION` is `0.48.0`. *Red on the ground: `subject` is optional and names three kinds; `seen_from` is absent.* `crates/dsl/tests/v48_shot_subject.rs`.
2. **Shape.** A shot with no `subject` is `DW0199` naming the shot; `"landscape"` under a `shot_style` or as `subject_b` is `DW0348`; `subject` on an unstyled shot is accepted; an unknown `assembly` or `place` is `DW0112`; a subject naming two kinds is `DW0100`. `crates/dsl/tests/v48_shot_subject.rs`, one fixture per shape.
3. **Aim.** An unstyled shot with a `subject` and no `look_at` emits `cs_tick` yaw/pitch computed toward the subject's reference point at every keyframe; with `look_at` the aim is `look_at`'s; `"landscape"` with no `look_at` emits the travel aim byte-for-byte as the ground revision's unstyled shot does. `crates/delvec/tests/framing_aim.rs`.
4. **One sight rule.** `delvec snapshot` on the gallery's render plan writes `occluded` and `out_of_frame` equal, target for target, to `framing::sight` called with the manifest's camera, and the two are computed by one function (`grep -rn "fn sight" crates/delvec/src` finds one definition). `crates/delvec/tests/framing_snapshot_agree.rs`.
5. **Frame.** A fixture whose shot's reference point projects at `|y| = 0.95` at tick 0 is `DW1033` naming the tick and "out of frame"; moved to `|y| = 0.90` it builds green; a lens cell of water at a tick narrows the judged angle to 60° and a fixture that passes at 70° and fails at 60° is `DW1033` only with the water. `compiler::framing::tests`.
6. **Sight.** A fixture with a one-block pillar on the centre line and the eight corner lines clear builds green with `lines_clear` 8 at that tick; a wall across all nine is `DW1033` naming the blocking cell; a bell framed as `{"anchor": …}` whose own cell is the only block on the line builds green (the subject's cells are not occluders); a place framed from outside with its roof on every line to its centre builds green when a corner line is clear. `compiler::framing::tests`.
7. **Present.** An assembly framed with its `spawn-assembly` in the same list ahead of the cutscene builds green; after the cutscene, in a later `sequence` step, behind a `requires_flags`, or with a `despawn-assembly` between, is `DW1034` naming the ordering; an NPC placed at setup is present with no beat; an actor whose spawn is a quest-graph ancestor is present. The ordering function has one definition, called by `DW0195`, `DW0198` and `DW1034` (`grep -rn` finds one). `crates/delvec/tests/framing_presence.rs`, `crates/dsl/tests/npc_lifecycle_shared.rs`.
8. **Sent.** An assembly subject whose root stands 161 blocks horizontally from the lens at some tick is `DW1034` naming the tick, 161 and 160; at 159 it builds green with its `view_range` derived to cover it; a mannequin NPC at 100 blocks under `view_distance: 20` is sent (its type's range is 512, capped at 320) and is `DW1034` by §5.8 alone, the message naming the cull radius 64 and not a tracking range. `compiler::framing::tests`.
9. **Drawn.** `asm_summon_<s>` carries `view_range:` on the root and every part equal to `max(1.0, seen_from/32, d_max/32)` to two decimals, and the ledger names which term won; an assembly no shot frames carries the floor from its own place; a villager NPC framed from 70 blocks is `DW1034` (cull radius 67.2) and from 60 builds green with slack `60 / 67.2` on the binding line. `crates/delvec/tests/framing_drawn.rs`, `crates/delvec/tests/assembly.rs`.
10. **Binding line and ledger.** Every build with a cutscene prints §7's line with every figure derived and its denominator, and writes `validation/shot-subjects.json` with `unchecked` naming §5.9's items and the digest of the shot list; a cutscene-less campaign prints and writes neither (byte-identical to the ground). `crates/delvec/tests/framing_ledger.rs`.
11. **The live half.** The bot's run report carries, per cutscene with a body subject, the body's spawned-before and spawned-through facts read from the bot's entity table; on the demo level every row reads true. *Measured on the demo level's walk; recorded in its README with the engine revision.*
12. **Gallery.** §9's element builds green; `tools/ci/check-gallery-coverage.py` reports 0 units in neither state; each probe is refused with its named code and no other.
13. **Record and skill.** The rows of §9, in the change that lands the code; `tools/ci/check-dw-codes.py`, `check-stated-counts.py` and `check-skill-page.py` green.
14. **Demo-level row.** `docs/demo-levels.md` names **The Thing Across the Water**.

**Debt, named.** §5.8's claim that an assembly *is drawn* at the client is confirmed by eye on the demo level (criterion 11 cannot read a client's frame); §5.9's items are not checked by anything and are listed, not closed.

## 11. Decisions

- **`subject` is required on every shot.** A shot that frames nothing in particular says `"landscape"`; the engine never infers what a shot is about. The alternative — optional, judged only when present — leaves the sea-and-sky shot unrefusable, which is the defect.
- **The proof frame is the client the game ships** (70°, 854 × 480, entity distance 1.0), with assemblies made safe down to the lowest entity-distance setting because the knob is free, and the slack reported for bodies that have no knob. The alternative — proving every shot at the client's minimum field of view — refuses most wide shots for a setting almost nobody uses.
- **The margin is EBU R 95's action-safe 3.5%, and nothing more is judged about composition.** The alternative — a fill or thirds rule — has no source figure and would restrict.
- **Occlusion at any judged tick is a refusal**, with the clear-line count reported; no tolerated fraction is invented. A dolly that passes behind a pillar is split into two shots.
- **`entity-broadcast-range-percentage` stays 100**: a body farther than its type's tracking range is refused rather than tracked farther. Raising it scales every entity's tracking for the whole delve for the sake of one shot; the camera moves instead.
- **How far an assembly is seen from belongs to the assembly** (`seen_from`), derived to a floor from its shots and its place, never typed as a `view_range` literal.
- No ADR.

## 12. What stayed out, and why

- A compile-time placement search (ClearShot): the engine judges the creator's shot and tells them why; moving the camera for them is a design decision the engine does not make.
- A `player` subject: the party is shown by stand-ins where the players stood, and no build-time pose exists to frame.
- A subject that is a `wave` or a mob of a fight: a fight's bodies stand where the fight puts them; a shot of a fight frames the `place` or `region` it happens in.
- Hiding by other entities, particles or weather, and the client's own render distance and occlusion culling: no server-side fact decides them (§5.9).
- The rate at which a subject crosses the frame, headroom, thirds: no cited figure, and a check never restricts.
