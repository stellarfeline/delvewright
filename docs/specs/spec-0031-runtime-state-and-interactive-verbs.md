# spec-0031: Runtime state, and the verbs that need it

- **Status**: Draft (owner design session 2026-08-08)
- **ADRs**: 0001 (the compiler emits everything), 0003 (vanilla-first),
  0006 (determinism)
- **Related**: spec-0011 (trap hardware), spec-0022 (traps v2 — redstone keeps
  only the physical trigger), spec-0032 (economy and the recovery stake)

## Why this exists

Four capabilities were commissioned in one session — a lift, a currency, a
shop, a death-and-recovery loop. Surveyed against vanilla 1.21.11 and against
the engine, they turned out to share one missing primitive and a small set of
missing verbs. Building any of them alone would have produced a private copy of
the shared part, which is the defect CLAUDE.md names first.

## The missing primitive: clearable, comparable runtime state

The only author-visible state today is `FlagId`: boolean, party-wide, and
**monotonic — no verb clears a flag.** That is sufficient for "this has
happened" and for nothing else.

A lift needs `car_at_floor` and `ride_in_progress`; a purse needs a balance; a
recovery stake needs an amount. All three are numeric, all three must go *down*
as well as up, and every one of them is read as a **condition on some other
effect** rather than as an effect of its own.

**Decision.** Runtime state becomes a declared, named, integer-valued datum with
set / add / clear verbs, and **the comparison goes into the shared gate struct**
— the `requires_flags` / `forbids_flags` pair carried today by 16 effect
variants, all 5 objective kinds, triggers, traps, cast placements, dialogue
options and branch declarations.

The gate struct is the correct home because the comparison's consumers are
exactly the gate struct's consumers: "this door opens at 500", "this line is
withheld below 200", "this lever does nothing while the car is moving", "this
wave spawns only on a third visit". Hanging the comparison off a shop verb — the
first consumer to ask for it — would leave the second consumer with no surface,
and the fix would be a second bespoke field. Generality is decided at the FIRST
site; retrofitting at the second costs a `dsl_version` bump, per-stage fences,
and an adoption round on every active campaign.

Scope of a datum (per-player vs party-wide) is declared, not inferred. Note the
existing test that forces party scope on anything classified as progression:
that classification must be explicit, or the multiplayer semantics get decided
by accident.

## The missing verbs

Each is a general mechanism the engine already performs internally and has never
exposed. Nothing here is new capability at the emission layer; all of it is
surface.

### `on_death` — an effect root

The engine has `on_respawn` and nothing that runs at the moment of death. With
an `on_death` root, "the purse is dropped on death" stops being an engine
feature and becomes content expressed in a general one — which is the test this
project applies before adding any surface.

It joins the effect-root set, which means it is visited by `for_each_effect_root`
and therefore covered by every walker that already exists: reachability, l10n
inventory, effect-history replay, the dangling-function check. The recurring
defect in this family is a hand-rolled walk that enumerates some roots and not
others (#301, #302, #321, and a sixth instance found the same day as this spec:
`Shortcut.on_unlock` is an effect bundle emission lowers and no root visits).

Death detection must ride the existing edge (`dw.deaths` / `dw.death_ack` /
`cp_respawn_check`), not a second detector. The stake needs the death
*position*, which the existing edge deliberately reads post-respawn, so the
capture happens before the ack — via the pre-respawn advancement or the
read-only `LastDeathLocation` player NBT, whichever a live spike confirms fires
for non-entity deaths (void, fall, drowning). That question is load-bearing for
a souls-shaped delve and is answered by measurement, never by recall.

**Implementation notes (#346), recorded so they are not re-derived.**

`on_death` is campaign-wide, one bundle at `/content/on_death` — deliberately
**not** a field on a checkpoint. *Where you come back* is a checkpoint's
property; *that you died* is true everywhere in the delve. A per-checkpoint
bundle would be N copies of one fact with N chances to forget one. Phase-
specific behaviour uses the per-effect gate every root already carries, so no
second gating surface appears.

**When to desugar and when to add a root — the rule the sixth blind spot
yielded.** `Shortcut.on_unlock` became a root rather than being desugared into a
trigger, and the general form is worth keeping: *desugar when the sugar's whole
meaning is the general construct; add a root when the bundle hangs off an object
that has runtime machinery of its own.* An ambush **is** a trigger — the trigger
is the entirety of what it emits — so it desugars. A shortcut's unlock is not:
its detection is a once-only sentinel poll that, in the same function, clears
the gate region, retires the affordance and kills the wrong-side bodies.
Desugaring it would have put **two independent detectors on one event**, which
is precisely the defect the death edge is built to avoid. The same reasoning is
why `traps[].payload` is a root and not sugar.

**What is proven, and what is not.** The compile-time shape is proven; the
runtime behaviour is not, and the reason is worth stating rather than
discovering later. A generated PackTest cannot prove the corpse-side positive,
because it drives a fake player and a fake player is alive; and no generated
template was added, because no campaign declares `on_death` yet, so it would
bind to zero campaigns and be vacuous — a green that means nothing. **The
obligation this creates: the first campaign to declare `on_death` must carry a
bot-tier proof of the corpse-side fire.** It is not optional, and it is not the
campaign author's discretion.

### Region fill / clear

`open-gate` and `close-gate` already fill and clear a declared region with a
declared block at runtime, and `close-gate` already teaches the completability
model to treat the region as solid from its point in the quest DAG.
`collapse`'s `then_floor` already paves standable ground at runtime, and the
completability proof already reasons about that surface.

So the capability exists twice, privately, inside two verbs. `set-block` — the
only general spelling — places **one** block at an anchor.

**Decision.** The capability moves to the object class it acts on: a declared
region can be filled or cleared. `open-gate` / `close-gate` become configured
uses of it and keep their names. The completability modelling moves with it, so
a third consumer inherits the proof instead of re-deriving it.

### Status effect, granted and cleared

The engine emits status effects internally (the night-vision area is a
self-rescheduling region-scoped grant) and exposes none. A verb retires the
hard-coded case and unblocks any effect a designer wants.

**Grant with a duration; do not rely on a later clear.** A sequence that ends
with "remove blindness" leaves a player permanently blind whenever the sequence
does not reach its end — a logout, a crash, an interrupted chain. A duration
expires on its own. This is a property of the verb's *use*, and the diagnostic
that enforces it belongs with the verb.

### Teleport players

The emission exists (inter-area transport, cutscene marker moves). There is no
verb. The selector must be a **region** — everything inside this volume — not
"whoever is standing on this block": half a foot over the edge, a player
mid-jump and a player sneaking on the lip all need one deterministic answer.

### Lethal volume

A declared volume that kills whatever enters it, with an author-written death
message.

This is a mechanism, not a fiction. A cliff whose fall must be fatal, a lava
pit, an acid pool, an out-of-bounds plane, the bottom of a lift shaft — all the
same mechanism, differently dressed. The alternative considered and rejected for
the cliff case was making the horizon void: that changes the approved art to
obtain a behaviour, and it serves exactly one fiction.

## The worked example, which is deliberately NOT a verb

A lift is the strongest available proof that the surface above is sufficient,
because it needs every part of it. **A `lift` verb must not be added.** Once
these primitives exist a lift is a `sequence` of them, and so is a rising
drawbridge, a materialising bridge, an opening wall, a sinking floor, a
summonable cargo platform.

**Owner design of record (2026-08-08).** Transport is by teleport — not by
levitation, not by a ridden entity, and not by any redstone mechanism. One car
exists, and it exists at exactly one floor at a time.

Controls, all one operation `car → floor N`:

- one call lever per floor, mounted on the shaft wall, meaning "the car comes to
  this floor";
- one lever inside the car, part of the car's own blocks, therefore created and
  destroyed with it, meaning "go to the other floor".

Ordering, as ticks of one `sequence`:

| step | tick | what happens |
|---|---|---|
| 1 | 0 | gate: the car is not already at the destination, and no ride is in progress |
| 2 | 0 | set `ride_in_progress` |
| 3 | 0 | grant blindness for the whole sequence plus slack |
| 4 | 1 | fill the destination car region (floor blocks + the in-car lever) |
| 5 | 2 | teleport every player and entity inside the old car volume to the new one |
| 6 | 3 | clear the old car region |
| 7 | 4 | set `car_at_floor`; clear `ride_in_progress` |

**Invariant: the car always exists somewhere.** Create before clear, never the
reverse — there is no tick at which a save could be loaded with no car.

Rulings on the cases, all owner decisions of 2026-08-08 unless marked:

- **Everyone on the car travels**, players and entities alike. A cargo lift is
  the same mechanism.
- **Calling the car to a floor destroys the car at the other floor**, and
  anyone standing on it comes along. One car means one car.
- **Pulling a call lever at the floor the car already occupies is a no-op** —
  never a destroy-and-recreate, which would drop the occupants for a tick.
  (Planner ruling; follows from the invariant.)
- **A pull during a ride is ignored, not queued** (planner ruling; determinism
  over convenience).
- **Falling down the shaft is death.** A lethal volume occupies the bottom
  layer of the shaft. The car's ground-floor position sits directly above it, so
  a player who jumps from an upper floor while the car is *down* lands on the
  car and takes ordinary fall damage — lethal or not by height, per vanilla —
  while the same jump with the car *up* falls the whole shaft into the lethal
  volume. Interrupted rides and deliberate jumps are treated identically: no
  special protection.
- **A recovery stake may never be placed on a block that runtime can remove**
  (planner ruling): a stake left on the car would be deleted by the next ride.
  See spec-0032's placement rule.

## Acceptance criteria

1. A campaign may declare a named integer datum with an explicit scope
   (per-player or party) and set / add / clear it; a datum read without ever
   being written, and a datum written but never read, are each diagnosed.
2. A numeric comparison is accepted anywhere the flag gate is accepted —
   every effect variant, objective kind, trigger, trap, cast placement and
   dialogue option — verified by a test that enumerates the gate's consumers
   from the type, not from a hand-written list.

   **Correction (implementation, #348): a branch declaration is not a gate
   consumer.** This criterion originally listed it. `BranchDecl.flags` is a
   *pinning* declaration — "these flags are SET on this branch" — read by the
   chronicle and the branch proofs; it is not a condition, and `purse == 500`
   has no meaning in it. The distinction is worth stating because the two read
   alike in JSON and only one of them gates anything. Measured consumer count:
   six classes over 28 declaring sites.

   **Correction (implementation, #348): the gate is a borrowed view, not a
   flattened struct.** The obvious shape — one `Gate` struct
   `#[serde(flatten)]`ed into each site — is not available: serde rejects
   `flatten` in combination with `deny_unknown_fields`, which all 76 stage
   structs carry and which is what turns an author's typo into `DW0100` instead
   of silence. Taking the flattened struct would have meant deleting
   `deny_unknown_fields` from 25 sites, i.e. weakening an existing check to
   obtain a tidier type — which the debug doctrine forbids. So the fields sit on
   all 28 sites and a from-the-type test is what makes "every consumer carries
   the whole gate" a property rather than a habit. What the criterion demands is
   the property; the struct was only ever one way to get it.
3. `on_death` is a member of the effect-root set and is visited by
   `for_each_effect_root`; a test enumerates roots from the type and fails when
   any root is unvisited by any walker.
4. Filling and clearing a declared region is expressible without naming a gate;
   `open-gate` and `close-gate` emit byte-identically to their pre-change output
   for every existing campaign.
5. The completability proof treats a runtime-filled region as solid, and a
   runtime-cleared region as passable, from the quest-DAG point at which the
   effect fires — the existing `close-gate` semantics, now shared.
6. A status-effect grant carries a duration; a grant whose only removal is a
   later effect in the same sequence is diagnosed.
7. A teleport selects by region. A test asserts the selection is total: every
   entity in the volume is either moved or explicitly excluded by a declared
   filter.
8. A lethal volume kills on entry and states the death message from the
   campaign's own strings, entering the l10n inventory like any player-visible
   string. The completability proof treats its cells as **impassable** — not
   merely solid; nothing may stand on top either — so a campaign whose only
   route to an objective crosses one fails to compile, naming the volume.

   **Correction (implementation, #347): three things this criterion got wrong.**
   - It did not say through which *channel* the message is stated. The obvious
     reading — vanilla's death screen, via the damage type's `message_id` — is
     blocked by spec-0029 §3: vanilla builds that component with no `fallback`,
     so a player who declines the resource pack is shown a raw
     `death.attack.…` key, and `DW0185` cannot catch it because the literal in
     the emission is the key rather than the authored string. The message is
     delivered through the engine's own translate-with-fallback path instead.
   - It omitted the completability half entirely, which lived only in the prose
     above. Geometry that kills interacts with reachability; a criterion that
     does not say so leaves the most important half unproven. Folded in above.
   - **"Kills whatever enters it" is not literally implementable.** Engine
     machinery lives in the world — a volume drawn across a cutscene dolly would
     erase the camera. Five machinery types are exempt by name. Content bodies
     deliberately are not, which is what makes a posted NPC inside a volume a
     diagnosable authoring error rather than a silent deletion on tick one.
9. **The lift is authored entirely in campaign JSON**, with no verb naming a
   lift, and its full sequence — including the call from a floor the car is not
   at — is exercised by a PackTest template and by the bot tier.

   **Correction (implementation, the lift round): this criterion is PARTLY
   satisfiable, and what blocks the rest is worth more than the lift.** The
   fixture is `crates/compiler/tests/fixtures/lift`; the proofs are
   `crates/compiler/tests/v10_lift.rs`. No engine surface was added — the whole
   ride is one `sequence` over the five primitives above, and a test enumerating
   every name in all seven stage schemas (derived from the types, not from a
   list) asserts nothing in the DSL is named after a lift.

   *Satisfied.* The seven-step table emits step for step at ticks 0/1/2/3/4;
   create-before-clear holds; and **both planner rulings need no surface of their
   own** — "a call at the floor the car already occupies is a no-op" is the
   gate's `not-equals` term spliced into the tick line the trigger already had,
   and "a pull during a ride is ignored, not queued" is the
   `data remove entity @s interaction` that runs whether or not the gate opened,
   so nothing survives to be replayed. Neither was designed for; both fell out.

   *Not satisfied, each recorded as an executing test that reds the day it
   becomes authorable.*

   - **The lever inside the car is `DW0542`, and the refusal is right.** A
     `teleport` moves entities and not blocks, **and its `to` is a point**, so an
     affordance riding a car is torn off its lever and stacked on the destination
     anchor. The second half of that is the part this spec did not see: a
     region→point *gather* is a portal, and a car is a region→region
     *translation*. The consequence is not cosmetic — with call levers meaning
     "the car comes to this floor", a rider inside the car has no way to say "go
     to the other floor", so **a car cannot be commanded from inside it**. The
     fix is not a field on a trigger and not a type exemption in the selector
     (which would tear an NPC's dialogue hitbox off its body); it is a decision
     about affordances that belong to runtime-written blocks, and it is
     unmade.
   - **A runtime region cannot name a cell at an offset from an anchor.** Every
     one is `StealthZone { anchor, extent }`: a box *centred* on a prefab anchor
     with unsigned half-extents. A car needs its deck one block below the riders,
     its arrival cell one above that deck, and the shaft-bottom lethal volume one
     below the ground-floor deck. None is nameable, so a lift's geometry is
     authored in NBT (a prefab shipping an anchor per cell) rather than in
     campaign JSON — and no prefab in the library ships a shaft. **The general
     region language already exists one stage away**: stage 7's `select` takes
     `box {min,max}` in a `piece-local` or `anchor-relative` frame, plus
     `union` / `intersect` / `subtract`. Stage 5 cannot see it. That is
     CLAUDE.md's third defect shape — the general mechanism exists and its
     binding is too narrow to reach the objects it should — and the answer is not
     a fourth mechanism.
   - **"The car always exists somewhere" is authored, not enforced.** A sequence
     that clears its only car before filling the next compiles green.

   *Runtime tier.* The two `teleport_<key>` templates prove step 5 of each ride
   on the pinned toolserver (CI pass `PackTest suite — lift`). The sequence's own
   timeline is **debt**: the engine generates templates per verb and has none for
   a `sequence`. The bot tier is debt too, and for the reason spec-0032 records
   for the stake — no bot run exists for a fixture campaign.

   **A fourth finding, from the first CI run of that pass.** `lift:campaign`
   failed on CI — *Expected #party dw.campaign to match 1, but got 0 on tick 0* —
   over a tree that had gone green locally. Root cause, read off the emission and
   not re-run: the generated `campaign` template resets `dw.campaign` and the
   first quest's `dw.qa_<q>`, then re-drives `complete_o_<obj>`; the chain is
   latched by `dw.q_<q>` (`check_q_<q>` reads `unless score #party dw.q_<q>
   matches 1`), which the template never clears. The draft fixture put both
   critical-path objectives on `spawn` — hello-room has four anchors, two are car
   stations whose cells a `fill-region` makes solid (`DW0314` forbids routing
   through them) and one is the upper call lever — and `join_place` teleports
   every joining player onto that cell, so the world tick completed the campaign
   with nobody doing anything and the template's replay was a no-op. Whether it
   red depended on batch order, which the compiler does not control:
   byte-identical packs, verdict by ordering — the `v06_spawn_idempotent` class
   from a new direction.

   Two defects, one fixed here. The **fixture** was wrong — a delve that finishes
   itself while the party stands still is not a lift proof — and its finale is
   now gated on a flag only a ride sets, held by
   `v10_lift::the_finale_cannot_be_completed_by_standing_still` (measured
   red→green by deleting the `requires_flags`). The **generated template** is
   also wrong, generally: it does not meet the batch model's own "own init" rule,
   because the scores its drive depends on are not the scores it initializes.
   That is an open finding for its own round — the fix is clearing `dw.o_*` /
   `dw.q_*` for the quests it re-drives, which moves `packtest-datapack/` bytes
   for every campaign and cannot ride a PR whose whole claim is that it changes
   no engine source.

   The reusable half: **a green PackTest suite cannot prove order-independence.**
   It can only fail to disprove it. Anything that must hold whatever the batch
   order is has to be asserted about the emission.
10. Every gate above states its binding count, and a zero binding is a failure.

## Settled by live measurement (#349, pinned 1.21.11)

Both questions are answered. Full data: `docs/notes/death-and-teleport-spike.md`,
`tools/spike-death-teleport/observations.json` (4140 samples), re-runnable.

### The death signal — and a wrong premise in this spec

**This spec asked the wrong question.** It said "the pre-respawn death
*advancement*". The engine's edge is not an advancement: `emit_checkpoint_functions`
arms `dw.deaths`, a vanilla **`deathCount` scoreboard criterion**, guarded by
`unless data entity @s {Health:0.0f}` — and that guard is precisely what defers
`cp_respawn_check` to the first tick after respawn.

Measured, 5 causes × 3 repeats, every repeat agreeing:

| cause | `deathCount` edge | fires pre-respawn | corpse `Pos` is the death position | `LastDeathLocation` |
|---|---|---|---|---|
| void | yes | yes | yes (`floor` = LDL block) | yes, same tick |
| fall | yes | yes | yes, exact | yes, same tick |
| drowning | yes | yes | yes, exact | yes, same tick |
| lava | yes | yes | yes, exact | yes, same tick |
| mob (control) | yes | yes | yes, exact | yes, same tick |

So the load-bearing unknown resolves **favourably**: the score edge is armed on
the corpse for every cause, `LastDeathLocation` is written on the same tick, and
the corpse's position is stable while the death screen is up (measured drift
0.000 — a corpse stops falling). `on_death` can fire corpse-side and capture the
position for all five causes.

**An advancement would have been the wrong instrument**, which is the reusable
lesson: `entity_killed_player` covers 1 of the 5 causes, and
`entity_hurt_player` fires on the *first* damage event with HP still 16–20, so
it means "was hurt", never "died here". Had this spec's premise been implemented
as written, four of five death causes in a souls-shaped delve would have gone
unaccounted.

### Fall damage across a mid-fall teleport

**It carries, and applies at the destination.** `fall_distance` after − before =
exactly `0.0000` in 46/46 teleport trials, including teleports 143 and 157
blocks straight *up*. Landing damage is `floor(fall_distance) − 3`.

| fall distance when teleported onto the car | damage | outcome |
|---|---|---|
| 1.145 / 5.688 / 10.807 / 18.773 | 0 / 2 / 7 / 15 | survived |
| 20.279 | 17 | survived, 3 HP |
| 23.435 / 25.083 / 30.297 | ≥20 | died |

**An arriving lift car does not catch a falling player past roughly 20 blocks of
fall — it is the surface they die on.** This agrees with the owner's design of
record, which already says a player who jumps a shaft with the car below takes
ordinary fall damage, lethal or not by height.

Still unmeasured, and stated rather than assumed: the exact lethal boundary
between 20.279 and 23.435 (free fall quantises `fall_distance` in ~3-block steps
there, so `floor − 3` is a fit, not a measurement); whether a real vanilla client
matches the bot; **what does reset fall distance** — only same-dimension
`/teleport` was tested, so the mechanism that would make a lift *safe* is
unmeasured; and moving destinations.

### Two incidental findings that constrain the verbs above

- **1.21.11 rejects every legacy camelCase gamerule.** All eight probed answer
  `Incorrect argument for command` and change nothing; several were reworded
  (`doMobSpawning` → `spawn_mobs`, `doDaylightCycle` → `advance_time`). The
  compiler already emits the new names. The general lesson is not the rename: it
  is that **neither offending site reads the command's response**, so a rule
  that silently stopped applying looked exactly like one that worked.
- **A respawned player is invulnerable for 59 ticks (~3 s), and `/kill` answers
  `Killed <player>` while doing nothing.** This bears directly on the lethal
  volume and on the shaft-bottom volume: a player who respawns into or falls
  into one within three seconds of respawning does not die, and any emission
  that assumes `/kill` is synchronous is wrong. It also explains two things
  previously read as instrument flakiness.
