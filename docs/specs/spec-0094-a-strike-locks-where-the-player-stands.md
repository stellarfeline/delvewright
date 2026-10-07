# spec-0094: A strike locks where the player stands — an assembly turns to a chosen player's cell, takes the pose its rig proved for it, and a retract the story plays holds until the pattern is re-armed

- **Status**: Approved
- **Ground**: written against engine `59de6692` (`origin/main`, `delvec 1.8.2, dsl 0.35.1, mc 1.21.11`). Read with it: `Assembly`, `AssemblyStrikes`, `StrikeAim`, `StrikeStep`, `StealthZone`, `Verb::PlayClip` and `Verb::DamagePlayers` in `crates/dsl/src/stages.rs`; `assembly_checks` in `crates/dsl/src/validate.rs`; `judge`, `correspondence`, `struck_cells`, `turned_region`, `drawn_facings`, `witness_steps` and `assembly_functions` in `crates/delvec/src/compiler/assembly.rs`; `box_selector_args` in `crates/delvec/src/compiler/emit.rs`; spec-0082 (the assembly) whole, its §8 rows 3 and 10 above all; `docs/reference/compiler.md`'s assembly rows and `DW0935`–`DW0938`. The pinned server jar (`versions.toml` `[minecraft]`, sha256 `f83b8e09…1726`) was read with `javap` (JDK 21) through Mojang's official 1.21.11 server mappings (sha1 `5621e925…5955`) for every vanilla fact marked **cited (jar)** in §2.
- **What it is for**: a striking thing that strikes *where a player stands*, not at a fixed spot — a limb that turns to whoever it chose, reaches as far as that player is, and comes down there; and a thing the story sends down that stays down until the story says otherwise. The first is a creator's request; the second a defect a campaign hit.
- **Research**: §2 is the record. Each rule below is marked **cited** (the tree at `59de6692`, the pinned jar, spec-0082's measurements, a published page) or **authored** (this spec chooses).
- **Numbers**: spec `0094`; `DW0968` (a lock that cannot strike where it locks, build), `DW0969` (a locked blow declared where the lock derives it, validation), `DW0970` (an `arm-strikes` on a thing that never strikes, validation); a stage-5 surface change inside the unpublished `dsl_version` minor other unreleased format changes share; this spec states no version literal. No ADR: no settled decision moves.
- **Non-goals**: a limb that bends live to an arbitrary point (§2.1: vanilla has no primitive for it); a lock that follows a moving player after the wind-up begins (the wind-up is the dodge, and its length is the creator's); a hitbox that turns with the thing (an `interaction` is axis-aligned); a new wind-up field (§3.4: the schema has one).

## 1. The finding

**Cited (tree).** Three facts, read from the emission spec-0082 built:

1. **The blow lands at a fixed distance.** `strikes.aim {facings: N}` turns the root, at each wind-up, to the one of `N` facings nearest the bearing of the nearest player, and every `damage-players` box turns with it (`turned_region`). The box's distance from the mark never changes: a player standing closer or further than the box is never struck. The request is a strike that locks a player's position within a declared region and lands there.
2. **A retract the story plays is overwritten while anybody stays in `while_in`**. `asm_cue_<s>_<k>` — what `play-clip` calls — sets the clip the assembly returns to and plays it when no step is in flight. The tick's strike machine then begins a wind-up on **any** tick on which `sm` is idle and a player is in `while_in` (`execute if score #asm_<s>_sm dw.sys matches 0 if entity <while_in> run function …:asm_begin_<s>`). Nothing the story does can stand the pattern down, so the retract plays for one tick and the wind-up starts over it; a campaign's emitted retract matched the tentacle demo level function for function. The engine is at fault, not the authoring.
3. **`aim`'s "nearest player" is nearest to the arming box's low corner, not to the thing.** The target was `@a[x=…,dx=…,…,sort=nearest,limit=1]`. A selector's own `x`/`y`/`z` are the origin its `sort` measures from (§2.3). **Direction**: it chooses a different player among several, never an unproved blow — every facing the root can take was proved — so it cannot let anything ship that the proof did not see. It is fixed here because target choice is this spec's subject (§4.1).

## 2. What vanilla 1.21.11 gives

### 2.1 Turning, and bending

- **A root `tp` turns every rider by its own change of yaw, to any angle.** **Cited** (spec-0082 §8 rows 3, 10, measured on the pinned server): a vehicle teleported with a yaw turns every passenger by the difference, seated, with transformation and interpolation untouched; `rotate` turns the vehicle alone; a passenger teleported in place dismounts. The yaw is a float, so the turn is continuous — no facing grid is owed.
- **A display does not bend to a point.** **Cited** (spec-0082 §9, *Display*): a display entity's pose is its `transformation`, interpolated by the client between keyframes the server writes; there is no inverse kinematics and no live constraint toward a position. A pose the server can write is one somebody computed. So a limb's reach is a **set of poses** a rig generator computed, and choosing a pose is the only run-time act available. **Authored**: a lock is a turn (continuous, any yaw) plus a choice among poses (the step's `strike` and its `reaches`), both proved for every cell the lock can choose.
- **The hitbox does not turn.** **Cited** (*Interaction*): an `interaction` is axis-aligned, its box `width × height` about its position. **Authored**: the spawn-pose footprint rule of `DW0936` is asked at every turn a lock can take (§5.3).

### 2.2 Reading where a player stands

- **`data get entity <e> Pos[i]` is the floor of the coordinate.** **Cited (jar)**: `DataCommands.getNumeric` returns `Mth.floor(numeric.getAsDouble() * scale)` (bytecode: `vp.k()D`, `dmul`, `bgj.c(D)I`, where `bgj.c` is `Mth.floor(double)`); spec-0082 §8 row 10 measured the same (yaw −100.7 × 8 reads −806). At scale 1 a player's `Pos` is the cell their feet stand in.
- **Scoreboard `<` and `>` are min and max**, `-=` subtraction, the stored value an `int` written to storage with `execute store result storage … int 1` — **cited** (*Scoreboard*, the engine's own emission since spec-0082). Macro dispatch on storage ints (`$function …_$(c)_$(f)`) is the engine's existing frame dispatch.

### 2.3 Choosing a player

- **Vanilla's selector orders are `nearest`, `furthest`, `random` and `arbitrary`.** **Cited (jar)**: the four string constants of `EntitySelectorOptions` (`gy`), each binding an order (`EntitySelectorParser.ORDER_NEAREST`, `ORDER_FURTHEST`, `ORDER_RANDOM`, and the unsorted default). `arbitrary` is join order and states nothing an author would choose; **authored**: `nearest`, `furthest` and `random` are offered.
- **The sort origin is the selector's own `x`/`y`/`z` when it states them.** **Cited (jar)**: `EntitySelectorParser`'s position function builds `new Vec3(x == null ? pos.x : x, …)` (bytecode of the private `a(ftm)ftm`: `getfield E/F/G`, `ifnonnull`). So a box selector that also sorts measures from the box's corner. **Authored**: the target is chosen in two selectors — every player in the box is tagged, then `execute positioned <mark> run tag @a[tag=…,sort=<pick>,limit=1]` (§4.1).

### 2.4 Reach, body and timing

- **A player's attack reach is 3 blocks from the eye.** **Cited (jar)**: `Player.createAttributes` sets `entity_interaction_range` to `3.0` (`DEFAULT_ENTITY_INTERACTION_RANGE = 3.0f`) and `block_interaction_range` to `4.5`; `isWithinEntityInteractionRange(AABB, buffer)` is `aabb.distanceToSqr(eye) < (range + buffer)²`, and the server's `handleInteract` passes a buffer of `3.0` — a server tolerance, not a reach. `strand::STRIKE_REACH` (3.0) already reads this; nothing here changes it.
- **A body is 0.6 wide** (`Body::PLAYER`, spec-0062) and **walks at movement speed 0.1** (**cited (jar)**, `Player.createAttributes`: `movement_speed` `0.10000000149011612`), about 4.317 m/s walking and 5.612 m/s sprinting (**cited**, Minecraft Wiki, *Walking*, *Sprinting*) — about 0.22 and 0.28 blocks a tick.
- **Timing.** A tick is 50 ms. **Authored**: a lock is taken on the tick the wind-up begins; the blow lands `windup + hold + strike` ticks later (spec-0082 §3.2, `Clip::landing_ticks`). A player who moves off the locked cell in that time is not struck: at a sprint, a 20-tick wind-up and hold carries a body about 5.6 blocks. How long that window is stays the creator's, by spec-0016 §3's ruling (no telegraph rule); the staging record states it per step.

## 3. The surface

**Authored.**

### 3.1 `lock` on a strike step

```json
{ "windup": "windup", "hold": 10, "strike": "near",
  "lock": { "within": { "anchor": "anchor/reach", "extent": [2, 0, 2] },
            "pick": "nearest",
            "reaches": ["far"] },
  "on_land": [ { "type": "damage-players", "amount": 4 } ] }
```

- `within` — a `StealthZone`, the engine's one anchor-centred box. The step winds up only while some player's body is in it (and the pattern is armed, §3.3); its keep-out lies inside `while_in`'s, so a player outside the arming region is never locked (`DW0968`).
- `pick` — `nearest` | `furthest` | `random`, measured from the mark.
- `reaches` — further strike clips the lock may choose among, after the step's `strike`, in order. The step's candidates are `[strike, …reaches]`.
- **The blow is derived.** A locked step's top-level `damage-players` declares no `in`: its area is the cells the chosen clip comes down on at the locked turn (§4.2), the area spec-0082 §5.4 shape 2 demands a blow have, by construction. An `in` there, a `damage-players` nested inside another effect's list, or a locked step in an aimed pattern is `DW0969`.
- A lock is on the **step**, so one pattern can mix a locked blow, a fixed sweep and a feint, and two steps can lock by different rules.

### 3.2 What a lock does at run time

At the start of the step's wind-up: choose the player (§2.3), read the cell their feet stand in (§2.2), turn the root to the yaw the compiler proved for that cell, remember which candidate clip and which landing that cell takes, play the wind-up. At the swing, play the remembered clip; at the landing, run `on_land` with every top-level `damage-players` dealt over the remembered cell's derived area, every other effect as written.

### 3.3 `arm-strikes`, and what `play-clip` now does

- **`play-clip` stands the pattern down.** The clip it plays completes and holds its last frame (or loops), whoever stands in `while_in`; no wind-up begins. A step in flight still finishes and lands first (spec-0082 §3.3, unchanged).
- **`arm-strikes {assembly}`** re-arms the pattern, from its first step, on the next tick some player is in its arming region. Re-arming is therefore a rule the author writes — a `sequence` step after the clip's length, a trigger, a rest — never a property of standing still. `spawn-assembly` arms (a spawn is a fresh thing). An `arm-strikes` on an assembly that declares no `strikes` is `DW0970`.

### 3.4 The wind-up's length is already configurable

**Cited (tree).** `StrikeStep.hold` is an unbounded `u32` of ticks the wind-up's last frame is held; `ticks_per_frame` (1–20) paces the step's wind-up and strike clips; the wind-up clip's own frames and cadence are the rig's. The anticipation a player sees is `1 + (frames − 1) × cadence + hold` ticks, every term the creator's, and the staging record states it per step (`windup_ticks`, `hold`). No field is added; acceptance criterion 1 holds the existing ones to the schema.

## 4. Emission

**Authored, on §2.**

### 4.1 Choosing a target

`assembly::target_lines`: tag every player in the box not in a cutscene with a candidate tag; `execute positioned <mark cell centre> run tag @a[tag=<candidate>,sort=<pick>,limit=1] add <target>`; clear the candidate tag. The aimed pattern's wind-up uses the same lines (box `while_in`, `nearest`), which fixes §1's third fact.

### 4.2 A locked step

Per locked step `j` of assembly `<s>`, from the plan the judgement proved (`LockPlan`, carried from `assembly::check` to the emitter — never re-derived):

- `asm_lock_<s>_<j>`: choose the target in `within`; per axis, `data get entity <target> Pos[a]`, subtract the region's low corner, clamp to the region (`>` 0, `<` its size); store the three into `dw:asm <s>.l0/l1/l2`; `function …:asm_lockat_<s>_<j> with storage dw:asm <s>`; clear the target tag.
- `asm_lockat_<s>_<j>`: `$function …:asm_lockc_<s>_<j>_$(l0)_$(l1)_$(l2)`.
- `asm_lockc_<s>_<j>_<ix>_<iy>_<iz>`, **one per cell of `within`**, so every dispatch names a function that exists: `tp <root> <mark> <yaw> 0` to the proved cell it resolves to, and `data modify storage dw:asm <s>.r` (the play index of the candidate clip) and `.q` (the cell's index). A standable cell resolves to itself; any other cell — a body in the air over its floor, a player whose box reached in from the edge — to the proved cell in its own column below it, else to the nearest proved cell.
- The swing plays `$function …:asm_play_<s>_$(r)`; the landing runs `$function …:asm_land_<s>_<j>_$(q)`, one landing per proved cell, its `damage-players` through `region_damage_lines` over that cell's derived area.

A paced step (`ticks_per_frame`) plays every candidate as an emitted clip after the rig's own (`assembly::paced_slots`, `play_index`), as its wind-up and strike already were.

### 4.3 Arming

`#asm_<s>_armed` on `dw.sys`: set to 1 by the summon and by `asm_rearm_<s>`, to 0 by every `asm_cue_<s>_<k>`. A wind-up begins only `if score #asm_<s>_armed dw.sys matches 1`; for a pattern with a locked step, one begin line per step, the locked one asking for a player in its `within` instead of `while_in`.

## 5. What is checked

**Authored**, each rule naming the instrument it reuses.

### 5.1 `DW0968` — a lock strikes where it locks (build, exit 3)

`assembly::lock_plan`, over the walked population `P` (`lethal::walked_population`, the population `DW0938` reads). For every cell of `within` in `P`, the bearing from the mark cell's centre to the cell's centre; per candidate in order, the turn that points the clip's blow — the horizontal centroid of the cells its last frame stands in and its first does not (`assembly::aim_bearing`) — at the cell; the candidate is taken when, at that turn, the cell is among the cells its last frame comes down on (`struck_cells` of the last frame less the first, spec-0082 §5.4) and every one of those cells is caught only from inside `while_in` (shape 1). Refused, naming cells and why for the first three: a standable cell no candidate takes; a lock region whose keep-out leaves `while_in`'s; a lock region with no standable cell.

### 5.2 `DW0969` — a locked blow is the lock's to place (validation, exit 1)

`dsl::validate::lock_shape_checks`: a top-level `damage-players` with `in`, a nested `damage-players` at any depth, a locked step in an aimed pattern.

### 5.3 The hitbox at every lock turn

`DW0936`'s spawn-pose rule is asked at every turn a proved cell takes; one refusal names how many turns miss and the first.

### 5.4 The binding line

`assembly binding: A assembl(ies) declared, P part(s), C clip(s), H hitbox(es) examined, S strike step(s) checked over F facing(s) and L locked cell(s), R refused` — zeroes included. `validation/assembly.json` gains `locks`, each step's `locked`, and per locked cell (`facings[k]`, `k` the cell's index) its yaw, the clip that strikes it, its caught cells, its landing cells and the cell itself as its stand.

## 6. The bot

**Authored.** A locked first step is witnessed twice before the first `strike-assembly` step on the path: the bot stands on the locked cell nearest straight ahead and is struck, then on the cell turned furthest from it and is struck — the blow follows the body that moved — then on a cell outside the arming region's keep-out and is spared. Each `struck` witness carries the cell's index as `facing`, the lockable count as `facing_count`, its yaw and the clip; the harness's existing `witness-strike` parser and executor take them unchanged.

## 7. What the gallery, the probes, the record and the demo owe

- **The gallery.** `rig/gallery-arm` (`prefabs/gallery-generator`: a post and a club; `near` lays the club one to five cells in front, `far` three to eight) and `assembly/arm` in the near hall at `anchor/reach`: three locked steps, `nearest`, `furthest` and `random`, each striking `near` with `reaches: ["far"]`, the third paced. `play-clip rest` stands it down when the emeralds are taken; resting at the hearth re-arms it (`arm-strikes`).
- **The probes**: a lock with no `far` (`DW0968`, the cells only `far` reaches); a lock region past `while_in` (`DW0968`); a locked blow with an `in` (`DW0969`); a lock on the aimed sentinel (`DW0969`); an `arm-strikes` on a thing with no `strikes` (`DW0970`); a reach the rig lacks (`DW0935`).
- **Generated PackTests**: `asm_hold_<s>` — a dummy in the region that arms step 0, a cued clip, the real tick: no wind-up begins and the clip stays; the real `arm-strikes`, the real tick: one begins. `asm_lock_<s>_<j>` — the proved cells nearest and furthest from straight ahead written where the choice writes them, the real dispatch: the root reads the proved yaw and the storage the proved cell.
- **The record.** `docs/reference/compiler.md`: the `lock`, `arm-strikes` and `play-clip` rows, the three codes, the binding line, the target-choice rule.
- **The skill.** `references/quest-capabilities.md`'s assembly paragraph — a lock, its reaches, that the blow is derived, `arm-strikes`, and where the wind-up's length lives — lands with the release that carries the three codes: `check-skill-page.py` holds the page to the engine the plugin pins.
- **The demo level.** *The Reaching Arm* (`docs/demo-levels.md`): the tentacle in a pit locks onto a player who moves round the yard and slams down where they stand, at three reaches; three sword blows send it down, it stays down while the player stands in front of it, and it rises and strikes again only when the story re-arms it.

## 8. Acceptance criteria

Machine-checkable; each names its instrument. `delvec` is the implementing tree's own build; the gallery builds from `prefabs/gallery-generator`.

1. **The surface.** `delvec schema --stage all` exports `StrikeStep.lock` with `StrikeLock {within, pick, reaches}`, `LockPick` = `nearest`, `furthest`, `random`, and `QuestEffect` `arm-strikes {assembly}` (44 verbs), at the unpublished `dsl_version` minor (the authority, `crates/dsl/Cargo.toml`); `StrikeStep.hold` is an unbounded integer. *Instrument: `crates/dsl/tests/assembly_surface.rs`, `v29_firework.rs`.*
2. **The lock is proved per cell.** Over a one-part rig with a near and a far slab: every standable cell of the region is planned with the first candidate that comes down on it, at the turn that points its blow at it; a cell only the far slab reaches takes `far`; with `far` dropped those cells are `DW0968` naming them; a region one cell past `while_in` is `DW0968`; a region on no floor is `DW0968`. *Instrument: `compiler::assembly::tests`.*
3. **The blow is the limb's.** Every planned cell's area is non-empty, holds the cell, lies inside `while_in`, and satisfies `correspondence` both ways. *Instrument: `compiler::assembly::tests`.*
4. **The shape refusals.** An `in` on a locked blow, a nested locked blow, and `lock` in an aimed pattern are `DW0969`; `arm-strikes` on an assembly with no `strikes` is `DW0970`; a reach the rig lacks is `DW0935`. *Instrument: `crates/delvec/tests/assembly.rs`.*
5. **Emission.** A built locked step emits `asm_lock_<s>_<j>`, `asm_lockat_<s>_<j>`, one `asm_lockc_*` per cell of `within` each turning the root to its proved yaw, `asm_swingat_<s>`, `asm_landat_<s>_<j>` and one landing per proved cell; the choice is two selectors, the order `positioned` at the mark; two builds are byte-identical; every line is walked against `CommandTree::v1_21_11`. *Instrument: `crates/delvec/tests/assembly.rs`.*
6. **A retract holds, red without the fix.** The cue sets `armed` to 0, the begin line reads it, `arm-strikes` sets it to 1; the generated `asm_hold_<s>` passes in the required tier-2 PackTest job on the gallery; the same template against the tree before this change begins a wind-up over the cue. *Instrument: `crates/delvec/tests/assembly.rs`, the gallery's PackTest run.*
7. **Aim chooses from the mark.** The aimed wind-up's target is chosen by `target_lines`, never by a box selector that also sorts. *Instrument: `crates/delvec/tests/assembly.rs`.*
8. **The binding line and record** of §5.4 are printed on every build; the gallery reports `75 locked cell(s)` (three steps × the 25 cells of a 5 × 5 region) and 0 refused; the primary without the arm reports 0 locked cells. *Instrument: the gallery build, `tools/ci/gallery-baseline.py`.*
9. **The witness.** A build whose first locked step is on the path carries two `struck` witnesses at two different yaws, then `spared`. *Instrument: `compiler::assembly::tests`; the demo's bot run.*
10. **Gallery and probes.** The element of §7 builds green; the six probes are refused with their named codes; `check-gallery-coverage.py` reports 0 units in neither state. *Instrument: `tools/ci/check-gallery-coverage.py`.*
11. **Record, skill, demo.** `check-dw-codes.py`, `check-skill-page.py` (against the pinned release) and `check-demo-levels.py` green; the demo row is queued and the level is built on the content branch `demo/the-reaching-arm`, its ladder (build, PackTest, bot, staging gate) recorded in its README. *The client-side questions — whether the turn reads as one thing, whether the club lands where the player is hurt — are confirmed only on the demo level.*
