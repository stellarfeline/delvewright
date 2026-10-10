# spec-0101: A body watches — a still body turns to face the player who is nearest, by the game's own look-at re-issued from the delve's clock, and yields the turn to any walk the story gives it

- **Status**: Proposed
- **Ground**: written against engine `507a5e0e2` (`origin/main`). Read: `docs/reference/compiler.md` §2 stage-2 `npcs` and stage-5 `actors[]`, §4 "A walked body faces where it is walking", "One body, one live walk driver", "A cutscene is pure observation", "One tick's command chain stays under the game's limit" and `docs/reference/delvec/compiler/chain.md`; spec-0082 §5.7 (an assembly's aim); spec-0034 (one body type shared by both body classes); `crates/delvec/src/compiler/emit/{npc,actor,functions}.rs`.
- **What it is for**: a figure that stands in a doorway, on open ground or in the room the party is working through, and is facing them whenever they look at it — wherever they move, for as long as they are near. It never walks on its own; the story may walk it, vanish it or spawn it elsewhere with the verbs the engine has. One map may hold several. On `507a5e0e2` a body faces its anchor's declared `facing` until a walk turns it, and the one run-time turn the engine makes toward a player (spec-0082 §5.7) belongs to a display assembly, happens once per wind-up and lands on a proved facing, because a blow must land on proved cells. A watcher deals no blow. A content record names this as blocked on a ruling against §4's refusal of a per-tick turn; §3.3 is the ruling.
- **Research**: §2 is this spec's record. Each statement is **cited** (the pinned server jar read by mapped name, the pinned command tree, a compiler record) or **authored** (this spec chooses).
- **Numbers**: spec `0101`. Two diagnostics: `DW-new-1` (§5.1) and `DW-new-2` (§5.2). One DSL surface change, `watch` on both body classes (§4): the next minor `dsl_version`, this spec states no version literal. No ADR: nothing here moves a founding decision — the mechanism is a vanilla command (ADR-0003) emitted by the compiler (ADR-0001), and §3.3 restates a `compiler.md` invariant's quantifier without weakening it.
- **Non-goals**: a head that turns while the body stays (§2.1: every command path turns a living body's head and body together); a turn eased over ticks (§3.3); a body that walks toward a player (that is a mob's own AI, reached by `unleash-actor`); a relocation verb (§4.3: the existing `move-npc` / `move-actor`, `despawn-*` + `spawn-*` and the `approach{range}` trigger compose it); a watch on a wave mob (vanilla's AI owns its facing), on an unleashed twin (same) or on a display assembly (§2.4, §3.1 — a `billboard` on a display part is a different object class's surface, proposed to the planner as a one-line widening and not written here).

## 1. The thing, and the object it belongs to

**Authored.** Watching is a fact about **a body that stands still under the compiler's own stillness** — the stage-2 NPC and the stage-5 actor's puppet, the two classes summoned `NoAI` or `immovable` with a `Rotation` the compiler wrote (§2.3). So `watch` is one type on both, through one closed enumeration of body sites (`dsl::body_watch_sites`, the shape `body_traversal_sites` and `body_skin_sites` already take: a third body class is a compile error at every consumer until it is handled), because facing belongs to the body, not to the verb that first wanted it turned. The run-time turn is one line per watching body in the root `tick`, emitted by one function, read by the PackTest and the bot alike.

## 2. What vanilla does (cited)

Bytecode citations are of the pinned server jar (`versions.toml` `server_jar_sha256` `f83b8e09…1726`, inner jar sha256 `ec47239a…fcebc`), read with `javap -c` and the official mappings (sha1 `5621e925…5955`) by mapped name. Command-tree citations are of `crates/delvec/data/commands-1.21.11.json`.

### 2.1 The command, and what it turns

- `rotate <target> facing entity <target> [anchor]` is in the pinned tree: `rotate → target → facing → entity → facingEntity → facingAnchor` (`minecraft:entity_anchor`, `eyes|feet`), permission `gamemasters`, which a datapack function has.
- `RotateCommand.rotate(CommandSourceStack, Entity, LookAt)` hands the body to `LookAt.LookAtEntity.perform`, which for a body that is not a player calls `Entity.lookAt(anchor, anchor.apply(target))`; `tp … facing entity` takes the same `LookAt`.
- `Entity.lookAt(EntityAnchorArgument.Anchor, Vec3)`: the from-point is the anchor applied to the body, so `eyes` is eye to eye; yaw and pitch are `atan2` in degrees (`57.2957763671875`) written through `setYRot`, `setXRot`, `setYHeadRot`, with the previous-tick rotations set equal to the new ones. Nothing is interpolated on the server.
- `LivingEntity.lookAt` calls that and then writes `yHeadRotO = yHeadRot`, `yBodyRot = yHeadRot`, `yBodyRotO = yBodyRot`: **a living body's body and head turn together, at once.** `Entity.load`, on reading `Rotation`, calls `setYRot`, `setXRot`, `setYHeadRot` and `setYBodyRot` in turn, so a `data merge` lands them together too. No command turns a living body's head apart from its body.
- The entity selector has `distance`, `sort`, `limit`, `x_rotation` and `y_rotation` (`EntitySelectorOptions` string constants); `@p` is `sort=nearest,limit=1` from the executing position.

### 2.2 The game's own watcher, and why it is not this one

`LookAtPlayerGoal`: `DEFAULT_PROBABILITY = 0.02f`; `canUse` returns false whenever `random.nextFloat() >= probability`, else takes the mob's current target or the nearest player (or living thing) within `lookDistance`; `start` sets `lookTime = adjustedTickDelay(40 + random.nextInt(40))`; `canContinueToUse` ends the look when the target is gone, out of distance or the time is spent; `tick` points the `LookControl` at the target's eyes. A vanilla mob looks at a player for two to four seconds at a time, beginning on a one-in-fifty roll each idle tick. Goal sets are per-species code, not data; nothing in a goal set runs under `NoAI` (`Mob.isEffectiveAi` is `LivingEntity.isEffectiveAi && !isNoAi`), and `Mannequin.isEffectiveAi` is false while `immovable`. A `movement_speed` of zero stops a mob walking and nothing else: its other goals run, and knockback is velocity.

### 2.3 The compiler's stillness (cited from `emit`)

- A stage-2 NPC body is `summon <base_entity> … {NoAI:1b,Invulnerable:1b,Silent:1b,PersistenceRequired:1b,NoGravity:1b,Rotation:[<yaw>f,0f],Tags:["dw_npc","dw_npc_<n>"],…}`, or with a `skin` `summon minecraft:mannequin … {profile:{…},immovable:1b,pose:"standing",Invulnerable:1b,Silent:1b,Rotation:[<yaw>f,0f],…,Tags:["dw_npc","dw_npc_<n>"]}` (`emit/npc.rs`). The dialogue hitbox carries `dw_npc_<n>` and not `dw_npc`.
- A stage-5 puppet is `summon <entity> … {NoAI:1b,Silent:1b,PersistenceRequired:1b,NoGravity:1b,Invulnerable:<inv>b,…,Rotation:[<yaw>f,0f],…}` or the mannequin form with `immovable:1b`, tagged `dw_actor_<id>` and `dw_pup_<id>` (`emit/actor.rs`); the unleashed twin carries `dw_actor_<id>` alone.
- `Mannequin` declares profile, immovable, description, pose and layer members, `isImmobile` and `isEffectiveAi`, and no tick, rotation or look member of its own; `Avatar` declares the main arm and the model parts. Both inherit `LivingEntity.lookAt`.
- A body the story removes is tagged `dw_unseen`, parked under the world and killed five ticks later (`emit/removal.rs`).
- Every player in a cutscene carries `dw_cutscene` for the cutscene's length; a classed player carries `dw_class_<c>` (`compiler.md` §4, §3).
- The root `tick` function is assembled line-group by line-group, each group empty and byte-identical for a campaign that declares nothing of it (`emit/functions.rs`); the chain cost model charges one unit per `execute` sub-command stage that redirects and one per executing source for the command the chain ends in (`chain.md`).

### 2.4 The display's billboard

`Display.BillboardConstraints` is `FIXED | VERTICAL | HORIZONTAL | CENTER`, saved under `TAG_BILLBOARD`. The client draws a billboarded display turned toward its own viewer, each display entity about its own origin: a one-part figure faces every viewer at once, which no server-side body can, and a figure of several parts comes apart.

## 3. The ranking and the ruling (authored)

### 3.1 Three mechanisms, ranked

1. **`rotate <body> facing entity <player> eyes`, re-issued each tick from the root `tick`** — the chosen one. A first-class vanilla verb that exists for exactly this, exact to the game's own arithmetic, continuous at the game's own rate, available to both body classes, carrying no invented motion (§3.3), at three chain units a watcher a tick (`as`, `at`, the `rotate` for its one source — §2.3), measured on every build by `DW0984`.
2. **A display part's `billboard`** — first-class and free of server cost, per viewer. It is a property of a display entity, which is spec-0082's object class, not a body's; one part only. Not written here; proposed to the planner as a one-line widening of spec-0082's part declaration.
3. **The game's look goal with movement disabled** — excluded. It is intermittent by design (§2.2), it exists only on species whose Java goal set carries it, and it needs AI on, which returns the body's whole goal set (a hostile acquires and attacks, a villager wanders in place and panics). It cannot be told to watch continuously, and it is not a mechanism a creator can configure.

### 3.2 Two players

The body faces the nearer, and changes when they cross; a tie is `@p`'s to settle. A player inside a cutscene is never looked at: the camera is not in the room (`tag=!dw_cutscene`, the exclusion the stealth judge, `damage-players`, the lethal volumes and the health bars already carry). A party stand-in (spec-0095) is a mannequin, not a player, and draws no look.

### 3.3 Why this is not the refused turn

`compiler.md` §4 "A walked body faces where it is walking" refuses yaw **easing**: "any interpolation between two bearings would be invented motion the nav proof never made", and names a polling hack as not licensed by the absence of a vanilla "turn over N ticks". Both halves hold here, and neither is touched:

- **Nothing is interpolated.** Each tick's facing is the game's own `lookAt` from that tick's positions (§2.1). The compiler computes no bearing, eases nothing and invents no intermediate frame; the client's own entity interpolation is the game's, as it is for every rotation packet.
- **The primitive exists.** What vanilla lacks is the standing order "keep facing", not the turn: `rotate … facing entity` is the turn. Re-issuing a primitive from the clock is the shape every live driver in the engine already takes — the walk drivers, the stealth judge, the lethal volumes, the loop polls and the assembly clip drivers all run from the tick root — and the game's own watcher is itself a per-tick `LookControl` (§2.2).

The invariant's quantifier is restated in `compiler.md` so the two cannot be read apart: *the compiler never eases a turn; a body's facing on any tick is a vanilla computation from that tick's state — the walked body's from its leg, the watching body's from the player it faces.*

## 4. The surface

### 4.1 The declaration

On `npcs[]` and on `actors[]`, one optional field of one shared type:

```json
"watch": { "who": "nearest", "within": 8 }
"watch": { "who": { "class": "class/warder" }, "within": 6 }
```

- `who` — `"nearest"`, or `{ "class": "class/<id>" }`: the nearest player wearing that class. No default: the caller knows whom the figure is watching.
- `within` — a whole number of blocks `≥ 1`, measured as the game measures `distance`: from the body's feet to the player's. No default: the figure's reach is the creator's. A non-integer, a zero or a missing field is the shape refusal (`DW0100`).

### 4.2 The states

- **Watching**: from the tick the body exists, on every tick a player `who` names is within `within`; the body turns to face that player's eyes with its own, body and head together (§2.1).
- **Holding**: no such player within `within`: the selector matches nobody, the command does nothing, the body keeps its last facing. No return to the home facing is invented.
- **Yielding**: while a walk driver moves the body (`move-npc` / `move-actor`), no watch line turns it — two writers of one body's yaw in one tick is the defect class §4 "One body, one live walk driver" names. The watch resumes on the tick after the arrival tick, so the walked body's arrival turn stands for one tick at most before the nearest player claims it, which is the same player the arrival turn faces. A walk a later walk supersedes never arrives; the later one does, and the watch resumes then.
- **Gone**: a body the story removes (`dw_unseen`) is not turned; an unleashed actor's twin is not a watcher (the selector names the puppet marker); a `despawn-npc` leaves nothing to turn; a `deferred` NPC and a `spawn-actor` puppet watch from the tick they are summoned, because the selector is by tag.

### 4.3 Where a body appears in several places

A body in several places is several declared bodies, or one body the story walks. Relocation is composed from what exists: `move-npc` / `move-actor` to a mark (a walk, under §4.2 yielding), `despawn-npc` / `despawn-actor{vanish}` with a second body's `spawn-npc` / `spawn-actor` at the next place (a figure that is gone when the party comes near and stands further on), and the `approach{range}` trigger or any other beat to fire them. This spec adds no verb.

### 4.4 Emission

One generated function `watch_tick`, called from the root `tick` by one line when the campaign declares a watcher, holding one line per watching body:

```
execute as @e[tag=<body>,tag=dw_watch,tag=!dw_unseen,limit=1] at @s run rotate @s facing entity @p[distance=..<within>,tag=!dw_cutscene(,tag=dw_class_<c>)] eyes
```

`<body>` is `dw_npc_<n>` with `tag=dw_npc` for an NPC (the body, never the hitbox) and `dw_pup_<id>` for a puppet. Every watching body is summoned with the tag `dw_watch`; the start function of each of its walk drivers (`mv_<npc>_<to>`, `ma_<actor>_<to>`) removes it and the driver's arrival tick restores it — that is §4.2's yield in bytes, and a body without `watch` carries none of it. The class filter is emitted only for a `who.class`. A campaign that declares no watcher emits no `watch_tick`, no `dw_watch` and no `tick` line: byte-identical.

### 4.5 Determinism and multiplayer

Emission is a pure function of the declaration (ADR-0006). Which player a body faces on a tick is a function of that tick's live positions, as every selector's answer is; two players are §3.2.

## 5. The proofs

### 5.1 `DW-new-1` — a watch for a class nobody plays

`who.class` names a class stage 3 does not declare. Build tier (exit 3), judged over the campaign (a stage-2 NPC may name a class the creator writes at stage 3, exactly as `DW0197` judges a stage-2 body against stage 5), naming the body and the class.

### 5.2 `DW-new-2` — a watch nobody can draw

No cell of the walked population `P` (`lethal::walked_population`, the population `DW0938` and spec-0094 read) has its standing point within `within` of the body's feet point. The declaration is inert — nothing could ever turn the body — and an inert declaration is refused, not ignored (`DW0370`, `DW0914`, `DW0454`). Build tier (exit 3), naming the body, `within`, and the nearest walked cell with its distance. Every build prints `watch binding: W watcher(s) declared, over P walked cell(s), D drawable, R refused` — zeroes included — and `validation/watchers.json` records, per watcher, its body tag, feet point, `who`, `within`, and the drawable cell §5.3's test stands on.

### 5.3 The generated PackTest `watch_<body>`

One per watching body, in the batch model. The body is summoned (a deferred or staged body through its own entrance); the test dummy is placed, feet at the standing point of the drawable cell whose bearing from the body differs most from the body's home facing (and the test asserts that difference is at least 10°, so the turn is observable; a watcher with no such cell is reported in the binding line as `unobservable` and the test is not emitted), wearing `dw_class_<c>` when the watch names a class; `watch_tick` is run; the test asserts `@e[tag=<body>,y_rotation=<yaw−1>..<yaw+1>]` matches, `yaw` the game's own `atan2(−dx, dz)` in degrees from the body's feet to the dummy's. Then the dummy is moved beyond `within` and `watch_tick` run again: the yaw is unchanged (holding). For a `who.class` watch the dummy first stands without the tag and draws nothing. Pitch is not asserted: eye heights differ by species and the horizontal turn is the claim. A watching body with a walk driver additionally gets `watch_yield_<body>`: the driver is claimed (`walk_claim`), the start is run, `dw_watch` is asserted absent, the driver is driven to its arrival tick, `dw_watch` is asserted present.

### 5.4 The bot

The critical path record gains `watchers[]` from `validation/watchers.json`. At every waypoint the bot reaches within `within` of a watcher it can draw (`nearest`, or a class the bot wears), it waits one tick and asserts through its command channel that `@e[tag=<body>,y_rotation=<yaw−2>..<yaw+2>]` matches, `yaw` from the body's feet to its own position. Binding line: `[watch] W watcher(s) in the record, K within reach on the proven path, K asserted`. A watcher the path never comes within reach of is reported, never failed. No new step action: the assertion rides the legs the path already has, as the climb hop does (spec-0099 §6).

### 5.5 What the machine does not prove

That the turn is visible as a figure turning is a client fact, confirmed on the demo level, as spec-0082 §5.7 confirms a display's drawn yaw. That two players see the body choose between them is `@p`'s nearest-sort (§2.1), asserted in the emitted selector by test and seen on the demo level with two players.

## 6. The gallery's obligation, and the demo level

`npc/warden` (the gatekeeper at `anchor/warden`, a plain villager body that walks to `anchor/pocket`) gains `watch{who: "nearest", within: 8}`; `actor/standard-bearer` (the skinned husk mannequin at `anchor/march`, walked to `anchor/vantage`) gains `watch{who: {class: "class/warder"}, within: 6}`. Between them every unit is bound — both body classes, both `who` shapes, the mob and the mannequin branch, and the yield on two drivers — and the gallery README names them. Two probes: `gallery/probes/a-watcher-for-a-class-nobody-plays` (`who.class` `class/pilgrim`, `DW-new-1`) and `gallery/probes/a-watcher-nobody-can-reach` (`actor/rafter-spider` at `anchor/rafters` with `watch{who: "nearest", within: 1}`, `DW-new-2`; the probe's `within` is confirmed against the built `P` when the element lands, and raised to the largest unreachable whole number if 1 is reachable).

`docs/demo-levels.md` row: **The Still Ones** — a short street of doorways, a figure standing in each: a skinned mannequin, a plain mob body, and one that watches only the warder's class, every one turning to face whoever is nearest as the party walks the street and holding its last look once they pass out of reach; at the street's end a fourth figure walks away through an `approach` trigger as the party comes near and watches again from where it stops. Two players confirm the choice between them; the level is where the turn is looked at: does the figure read as watching. Status `pending`.

## 7. Decisions

1. **The mechanism is `rotate … facing entity … eyes` from the clock** (§3.1, cited §2.1): first-class, exact, continuous, both body classes, no invented motion.
2. **The look goal is excluded** (§3.1, cited §2.2): intermittent by design, species-bound, and inseparable from the rest of a mob's AI.
3. **A display part's billboard is another class's surface** (§3.1, cited §2.4): not written here; one line to the planner.
4. **§4's refusal stands and is restated, not weakened** (§3.3, authored): the compiler eases no turn; every tick's facing is a vanilla computation from that tick's state.
5. **`watch` is one type on both body classes** (§1, §4.1, authored): `who` and `within`, both required, no defaults.
6. **Holding, not returning** (§4.2, authored): out of reach the body keeps its last look; a return turn would be invented.
7. **A walk owns the yaw while it runs** (§4.2, §4.4, authored over §4 "One body, one live walk driver"): the watch yields by tag and resumes on arrival.
8. **No relocation verb** (§4.3, authored): the existing verbs and the `approach` trigger compose every relocation the capability asks for.
9. **Two refusals** (§5.1, §5.2): a class nobody plays; a watch nobody can draw — the inert-declaration rule, with its binding line.
10. **Head and body turn together** (§2.1, cited; non-goal): no command path exists for a head-only turn, so none is offered.
11. **The pitch is the game's and unasserted** (§5.3, authored): `eyes` is eye to eye; the claim under test is the horizontal turn.

## Acceptance criteria

1. `delvec schema --stage all` lists `watch.who` and `watch.within` under both `npcs[]` and `actors[]`, from one Rust type; `dsl::body_watch_sites` is a closed enumeration over the two body classes, and `crates/dsl` tests assert a document with `watch` on a wave mob is refused as an unknown field. *Vacuous if* the type were declared twice: the test asserts one schema definition is referenced from both sites.
2. `delvec validate` refuses `within: 0`, `within: 2.5` and a `watch` missing `who` or `within` with `DW0100`, and `who: {class: "class/pilgrim"}` on a campaign whose stage 3 declares no such class with `DW-new-1`, naming the body and the class. *Instrument: `crates/delvec/tests/watch.rs`.*
3. A body whose feet point lies farther than `within` from every standing point of `P` is `DW-new-2`, naming the body, `within` and the nearest walked cell with its distance; raising `within` to that distance's ceiling passes. Every build prints the `watch binding` line of §5.2 with `W`, `P`, `D`, `R`; a campaign with no `watch` prints `0 watcher(s) declared` and emits no `watch_tick`, no `dw_watch` and no `tick` line (byte-identical to `507a5e0e2`'s emission for the same documents). *Vacuous if* `P` were empty: the test asserts `P > 0` on the fixture.
4. For each watching body, `watch_tick` holds exactly the §4.4 line with that body's tag, `within` and class filter, and nothing else; the body's summon carries `dw_watch`; each of its walk start functions removes `dw_watch` and each driver's arrival tick restores it; a body without `watch` has none of these bytes. *Instrument: `crates/delvec/tests/watch.rs` over the emitted functions, byte for byte.*
5. `crates/delvec/tests/move_supersede.rs`'s interpreter, extended to `tag` add/remove, runs a two-leg watching body through a start, a superseding start and the second arrival, and reads `dw_watch` absent from the first start through the second arrival tick and present after. *Vacuous if* the first arrival restored it: the superseded driver must die without arriving (the existing contract).
6. The generated `watch_<body>` PackTests of §5.3 pass on the gallery for `npc/warden` and `actor/standard-bearer`, asserting the turn (`y_rotation` within ±1° of the game's bearing), the hold, and for the standard-bearer the class filter; `watch_yield_<body>` passes for both. Perturbing the build by dropping the `tick` line that calls `watch_tick` reds every `watch_<body>` test and no other. *Vacuous if* the dummy stood where the home facing already pointed: the test asserts the bearing differs from it by at least 10°.
7. `validation/watchers.json` on the gallery primary point lists two watchers with body tag, feet point, `who`, `within` and the test's drawable cell, and `tools/ci/gallery-baseline.py` attributes it. *Vacuous if* it were hand-written: it is regenerated by the build and compared.
8. The CI `gallery bot` job prints `[watch] 2 watcher(s) in the record, K within reach on the proven path, K asserted` with `K ≥ 1`, and the assertion of §5.4 passes at every such waypoint. *Vacuous if* the path never came within reach of either: `K ≥ 1` is asserted on the gallery, and the element owes it — if the proven path passes neither watcher within its `within`, the watch moves to a body the path does pass, never the other way round.
9. `gallery/probes/a-watcher-for-a-class-nobody-plays` is refused with `DW-new-1` and `gallery/probes/a-watcher-nobody-can-reach` with `DW-new-2`; `check-gallery-coverage.py` reports 0 units in neither state. *Instrument: `tools/ci/check-gallery-coverage.py`.*
10. `docs/reference/compiler.md` carries the `watch` row under stage 2 and stage 5, the `watch_tick` emission under §3, the restated §4 quantifier of §3.3, and both codes; `docs/reference/playtest-methodology.md` describes the bot's watch assertion and its binding line; `docs/demo-levels.md` carries the row of §6; the gallery README names both watchers and both probes; `tools/ci/check-dw-codes.py` passes.
