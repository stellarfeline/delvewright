# `delvec::compiler::lethal`

The reference page for `crates/delvec/src/compiler/lethal.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW0510–DW0512, DW0891, DW0922–DW0923 — lethal volumes (`compiler::nav` / `compiler::lethal` / `dsl::validate`; spec-0031, DSL v0.10; spec-0062)

A lethal volume is **geometry that kills**, so most of its completability
reasoning is not a check of its own: [`nav::World`] carries its cells as
impassable — one of the six premises `nav::Premises::of_plan` applies to every
world built from a campaign, on every arm — and every route proof in the engine
inherits that for free: the critical path, the checkpoint no-stranding proof
(`DW0315`), the branch paths, the trap forced-cell set, the exported harness
waypoints. That is the same move `close-gate`'s seal makes, and for the same
reason: a fourth consumer inherits the proof instead of re-deriving it.

`DW0510` exists because the *fix* for a blocked route differs in kind. A generic
`DW0311` sends the author to look for a wedged doorway or a void gap; here the
geometry is walkable and a **declaration** closed it. So the failure is derived
from a counterfactual — the leg is re-routed over the identical world with
lethality removed — and names the volumes covering that route.

`DW0510` has a **second shape**, and it is the same rule — *a route may not
depend on a cell a body may not be proven to stand in*: the counterfactual lifts
the placed **furniture** too (`World::without_exclusions`), and a route that
exists only over a table is refused naming the furniture anchor (spec-0065). A
walked `move-npc` or `move-actor` leg that cannot route is asked the furniture
counterfactual alone (`World::without_furniture`) before `DW0307` / `DW0325`, and
refused the same way.

`DW0511` is the one obligation routing cannot see, and it is one rule because it
is one defect: **a body put here by declaration rather than by walking.** Three
families fall under it. A *respawn seat* (entry spawn, `set-checkpoint`,
`bonfire`) means the party dies on arrival and is re-seated to die again, forever
— `/spawnpoint` is only a hint and the engine re-seats on the death edge, so
nothing downstream can rescue it. A *posted body* (a stage-2 NPC's anchor, a
per-quest `cast` placement, a stage-5 actor's anchor) is deleted on the first
tick: the volume's entity sweep exempts the engine's own machinery types and
deliberately not content bodies, so the delve loses its speaker in silence while
every static proof stays green. A *wave seat* is the third: the cell the seating pass stands one of a wave's mobs
on. Same rule, same code, different prescription — the author wrote no position
there, so what they can move is the wave's `anchor`, the volume's `extent` or the
body the wave carries.

It is asked **before** the route proofs. A volume that swallows a posted place
usually closes the route to it as well, so both findings arrive together, and
`DW0510` then prescribes moving the volume when what is wrong is that the Keeper
is standing in the pit. The specific cause is the more useful message and the
route closure is its symptom; the seat proof reads the plan and the seating and
never the routes, so nothing is lost by asking it first.

`DW0891` is the rule that comes between them, and its whole content is one
sentence: **danger is visible, or the engine refuses it.** The keep-out
below answers a question the route proofs never ask — *does this volume, as
declared, reach a cell the player would read as safe floor?* If it does, the
declaration is refused and the creator moves the volume; the repair for an
invisible hazard is to move the hazard, never to mark walkable-looking ground
unwalkable, because the compiler knows which stone kills and the player does not.
A hazard meant to be flush — a magma floor, a bed of fire, a burning strip — says
what shows it with `shown_by`, and that declaration is checked against the
assembled bytes per caught cell rather than taken as a word.

Two consequences worth stating where a reader of this section will meet them.
**The walk graph is unchanged**: the router still refuses the whole keep-out, and
what `DW0891` guarantees is that the cells it loses are cells a player could see
were dangerous — a pit's keep-out lies under its rim and removes no walked cell
at all, and a magma floor's removes the magma, which no route should cross.
**The reach cycle dissolves for every hazard the rule moves**: a volume at the
bottom of a pit has a keep-out that never rises above its own top, so the rim is
footing at `radius: 1` and `DW0850` and `DW0881` have nothing to disagree about.
For a flush signalled hazard the anchor is still a cell no body stands on, and
that case is what `DW0881`'s footing SET answers.

`DW0922` and `DW0923` ask what the others cannot: **where a wave's own bodies
get to.** The volume kills every body that is not a player, which is the
mechanism working when the party leads a mob in, and a defect when the wave
gets there unled: it thins itself before the party touches it, on every life,
and what a member drops lands in the box. The movement is a mob's, not a
player's (`World::mob_moves`): `World::body_moves` — the relation `DW0921`
floods — with the jump arc taken away, because a mob's ground pathfinder never
plans a leap across open air, asked of the mob's own footprint, plus one move a
player route never takes: a mob stands on a **barrier top** (fence, wall, shut
gate) it can step or jump up to from something higher, walks along it, and
drops off it. That is how vesperhold's choir drowned: from the wet flags onto a
floor lantern, from its top onto the well's curb, along the curb over the shut
gate, and into the water. A mob in water is taken to reach all of that water —
the undead sink and a drowned dives — which is wider than the truth for a mob
that floats and can only refuse more. The flood is bounded by the stack's
pursuit, its follow range (a lane's `aggro_radius`, else the declared
`follow_range`, else `nav::DEFAULT_FOLLOW_RANGE`) — the radius `DW0478` measures
a force by — around the seats the seating pass chose; idle wandering beyond it is
not modelled. As with `DW0478`, the remedy is never a shorter `follow_range`:
that retunes the fight to hide the placement. A fall off an edge inside
that range is followed to whatever stops it at any depth, and a body that falls
through a volume is in it. Every gate anchor is judged open: a barred way the
story or a lever ever opens is open for the rest of the delve, and a gate that
nothing opens is better built as wall. **`DW0923` is the same reach through a
barrier the party can leave open** — every fence gate, door and trapdoor a hand
opens (`blockshape::is_player_openable`; the two iron ones open only to
redstone), removed (`World::with_openings_open`) — raised only where the
as-built reach is clear. The repair it prescribes is the one vesperhold took:
take the opening away, a jump across a dry cut that the party makes and a mob,
which makes no gap jumps, does not.

#### A volume live from a story stage (spec-0088)

A `lethal_volumes[]` entry with a `when` is not a premise of the world:
`nav::Premises::of_plan` carries only the volumes with no `when`, and a staged
volume reaches a proof's world through the region model and no other door.
`World::region_state_at` — the function every route proof already asks — adds
to its `RegionState` the cells and `(id, box)` of every staged volume that **may
be live** at that arrival, and `World::with_region_state` applies them as a
premise volume is applied: impassable, widened by the walker's body, never
floor, named in a refusal. So the critical path (`DW0311`/`DW0510`), the branch
paths, the checkpoint proofs (`DW0315`/`DW0316`, `reachable_under_every_quest_state`),
`DW0921`, `world_while_next` (`DW0924`), the exported waypoints and the
world-edits replay inherit the staged volume without a line of their own. A
campaign that stages nothing builds a byte-identical world and region state.

**The gated-region evaluation** is `nav::liveness_of`, called by
`World::staged_liveness` and nothing else that builds a lethal set. It reads the
gate's terms against the path's `plan::RegionEvents` — beside the region writes,
every `set-flag` the campaign can perform (`FlagEvent`: an effect's step and
forcedness from `plan::firing_of`; a dialogue option's flag unforced at step 0
and forced at the step a `talk-to` on the path takes it; a disarm's flag
unforced at step 0; a setter in a quest the path's world never completes, or an
unforced setter of a branch flag the world never holds, dropped) and the
guaranteed replay's datum values (`DataReplay`) — under the region model's own
ancestry. Two readings:

* **may be live** — every required flag has a setter the configuration credits
  (a forced firing its ancestry contains, or any unforced one, credited at step
  0 as an unforced fill is); no forbidden flag has a forced setter its ancestry
  contains; every numeric term may hold (the replay's value satisfies it, the
  replay cannot date it, or an unforced root writes the datum at all);
* **is live** — every required flag set by a forced firing its ancestry
  contains; no forbidden flag with any setter credited; every numeric term
  decided true by the replay.

Flags are never cleared, so both readings are monotone in the flags along a
path. **The residual is named**: the configuration model assumes the party
moves through the DAG together, so a body left behind a seal is outside every
per-configuration proof — for a staged volume exactly as for a `close-gate`.

**`DW0891` is judged per configuration.** A staged volume has no one assembled
world, so it is judged in every configuration the critical path — and each
reachable branch's own path — passes in which it may be live, and in **the last
configuration before each switch-on**: a body standing in a volume's keep-out
when its gate flips is killed in the same server tick (`tools/spike-staged-lethal/ at aa5454feebf2`,
three of three trials at zero ticks), so the floor it stood on must read as
danger in that configuration's own bytes. Each configuration's world is its
region state with every exclusion lifted, its population `reachable_walkable`
from `lethal::population_roots`, and its bytes `RegionState::blocks_over` — the
assembled block map with every forced write it credits laid as the block its
command writes (a fill's block, air for a clear or an unseal; a `RegionEvent`
carries the block beside its class of write). A `collapse` is not a region write
in this model and lays nothing. The before-the-flip refusal is the **fourth
shape**. A staged volume judged in no configuration is refused naming the
enumeration.

**Posted places and seated waves are judged as if every volume were live**
(`DW0511`, `DW0922`/`DW0923`): a post, a seat or a reach that meets a staged
volume is the same defect arriving later, and each message says so.

**Binding**: `danger-visibility binding: N volume(s), S staged; judged over C
configuration(s) as K (volume, configuration) pair(s) against walked populations
of M..M′ cell(s); X caught, Y shown, Z read as safe floor; D declaration(s) of E
borne out by the bytes; R of N volume(s) reached by a body the engine models (in
P of K pairs)`. `validation/lethal-gate.json` carries `staged[]` — per staged
volume its `gate_terms` and `configurations: {judged, may_live, is_live, of}`
(the last three over the critical path's configurations) — and
`danger_visibility.volumes[]` holds one row per (volume, configuration) with the
configuration's first arrival step, whether the volume may be live there, and
that row's population.

#### A body has a width, and the volume kills what it touches

A volume kills through an `@e[x=…,dx=…]` selection, and **vanilla adjudicates a
box selector on hitbox intersection, not on the cell a body stands in**. A box
selector's `x`/`dx` pair is the world span `[x, x+dx+1]` — which is why `dx=0`
selects a whole block and not a plane — so the inclusive cell box `lo..=hi` is
the volume `[lo, hi+1]`, and any body whose collision box meets it dies.

Every reading of "inside a volume" in the engine is therefore a question about a
BODY, and there is one answer to it: `dsl::metrics` holds `Body`,
`selection_aabb`, `body_meets_volume` and `keep_out_box`, beside `step_allowed`
and for the same reason. Two readings, and the direction of each is the point:

* a **seated** body has a known position (a summon writes literal coordinates),
  so `body_meets_volume` is exact — which is what makes a 1.4-wide mob on the
  cell beside a face a refusal and a 0.6-wide one on the same cell not;
* a **walker** does not, because its cell fixes it only to `[c, c+1)`, so
  `keep_out_box` is the volume widened by half a width — one cell horizontally
  for every body up to two blocks wide, and `ceil(height)` cells downward. That
  is the set a recovery stake's near lip is chosen outside of, and the set
  `death-plan.json` carries to the bot tier as each volume's `keep_out`.
* a walker's **feet** are not at its cell floor when it stands on a partial
  block: they are on that block's collision top (`World::feet_16_fp`), half a
  block down on a bottom slab and 5/16 down on an upward dripstone tip. So the
  question every proof asks of a walker in a cell — the router's keep-out
  (`World::meets_lethal_fp`), the reach flood (`World::reach_into_volumes`), the
  danger-visibility population (`DW0891`) and the blind reach (`DW0943`) — is
  `World::body_can_meet_volume`: `metrics::feet_can_meet_volume`, the keep-out
  horizontally and `[feet, feet + height)` against `[lo.y, hi.y + 1)` vertically,
  with the feet the model stands it at. At a full-cube support it is
  `keep_out_box` exactly (`feet_below_the_floor_reach_the_course_they_stand_on`
  sweeps every cell round a volume for two bodies). A body standing on the tips
  of a dripstone pit is caught by a volume drawn in the tip course, as it is in
  the game; read from the cell floor it was caught by nothing, and the pit read
  as a place a body cannot leave (`DW0921`).

The refusing reading is deliberately the opposite direction from
`reach::certainly_completes_from`, which asks the same geometry about a volume
that GRANTS something and so demands the certain case: a generous rule is safe
for a reward and unsound for a hazard.

The cost is stated rather than hidden: **a one-cell lane beside a killing volume
is not a route.** A body walking it has 0.2 blocks of clearance and no way to
hold it, which is a hazard the engine will not prove a party through. A room that
needs such a lane is a room that needs another way.

Binding (playtest-methodology rule 1): a campaign with a volume emits
`validation/lethal-gate.json` — volumes declared vs. resolved, world cells closed,
posted places examined (`respawn_seats_examined`, which counts both families),
critical-path legs routed, PackTest templates generated (one per volume, and a
`lethal_<id>_shut` one more per staged volume), `staged[]` (spec-0088: per staged
volume its gate terms and how many configurations judged / may hold it live /
hold it live / exist), and `danger_visibility`: the walked population, one row
per (volume, configuration) with its caught and shown cells, and how many
declared signals the bytes bear out. A
campaign with no volume emits **no file at all**, so
a file that exists and reports zero is a finding rather than an absence.

| Code | Meaning |
|------|---------|
| `DW0511` | **A posted place inside the volume.** Somewhere the campaign requires the party or a declared body to BE lies inside a lethal volume: the entry spawn, a `set-checkpoint` / `bonfire` cell (the death loop), an NPC anchor, `cast` placement or actor anchor (a body the volume deletes on the first tick), or a cell the wave-seating pass stands a mob on. Build-tier (exit 3), `compiler::lethal`, asked before the route proofs. **Judged on the BODY, not the cell**: each place carries the hitbox of what lands on it, seated at the coordinates a summon writes, and `metrics::body_meets_volume` decides — so a post whose cell is outside the box and whose body is not is refused, and the message says so with the hitbox's dimensions. The message names the post and the volumes covering it. Prescription: move the post clear of the volume's FACES, or shrink its `extent`; for a wave seat there is no post to move, so it names the wave's `anchor`, the volume's `extent` and the body the wave carries instead.  **The quantifier is every volume, staged or not** (spec-0088 §6): a place that meets a volume live from a story stage is refused as if it were live, and the message says the volume is live from a story stage and the body is judged against it as live. |
| `DW0954` | **A staged volume the forced route never meets live** (spec-0088). Advisory (warning), `compiler::lethal`: the critical path passes no configuration in which the volume **is** live, so the ladder cannot exercise it, the bot's death loop will find it shut, and only the configurations in which it may be live were judged for visibility. Names the volume, the step its gate was first read at, and the term that never held. No change is required — a hazard the party need never arm is a design. |
| `DW0922` | **A seated wave whose members can reach a lethal volume.** From the cells the seating pass stands a wave's mobs on, a member gets its hitbox into a volume by the movement a mob has, within its follow range: walking, a step or jump onto a block at most one higher, a barrier top it can climb to from something higher, a drop off any edge (followed through the volume at any depth), and sinking in water; never a gap jump. Judged over the world with no lethal exclusion, every gate anchor open and every fence gate shut, as a mob finds it (`compiler::lethal::wave_reach` over `World::reach_into_volumes`). Build-tier (exit 3), asked after `DW0891` and before the route proofs. The message names the wave, the body, its seat, the turns of its way, the cell it gets in at and how, and every other (wave, volume) pair the same way. Prescription: move the seat (the wave's `anchor`) or move the hazard (behind a rise of two blocks, or across a gap of open air). Never buy the distance by shrinking `follow_range` — the term reads the same radius `DW0478` reads, and for the same reason a retune that hides a placement is no repair — never ring the hazard with blocks nobody can see, never delete the volume.  A staged volume is judged as live (spec-0088 §6), and the route in the message says so. |
| `DW0923` | **A seated wave that reaches a lethal volume through a barrier the party can leave open.** The same reach with every fence gate, door and trapdoor a player opens by hand removed, where the as-built reach is clear. A player who opens one may leave it open and die, and the wave re-seated after that death walks through. Build-tier (exit 3), right after `DW0922`. The message names the barriers on the way, by cell and block. Prescription: take the opening away — a jump across a dry cut, which the party makes and a mob never does — or move the wave's `anchor` or the volume so the opened way does not join them. **Binding**: `wave-lethal binding: N seated wave(s), N stack(s) over N seat(s), … N cell(s) a member can reach as built, N with all N barrier cell(s) a player can open left open; …` on every build that declares a volume, refusals included, and `validation/wave-lethal.json` (both codes' ledger) when it holds. |

#### The wording is a consequence of the blow, never a prediction of it

**Vanilla refuses damage far more often than "the target is dead" suggests, and
it says so while doing nothing.** A player is invulnerable for **59 ticks (~3 s)
after respawning**, and `/damage` — like `/kill` — reports success and changes
nothing (spec-0031 spike). A totem, `resistance 5` and an already-dead entity
refuse it the same way.

So a volume that printed its wording *before* swinging would tell a player *the
undertow takes you* once per tick for three seconds while they stood in it alive
— the delve asserting an outcome that did not happen: a site that does not
read the command's response.

The obvious fix is wrong, and this was measured rather than reasoned about.
`execute store success ... run damage` is **inert here**: on the pinned 1.21.11
toolserver a PackTest dummy in `playerGameType: 0` with `Invulnerable: 0` and
`Health: 20f` took `damage @s 1000 minecraft:fall`, ended on `Health: 20f`, and
the command answered **success = 1**. Reading a response that does not carry the
answer is the same defect as reading none. The guard therefore reads the
**outcome** — the player's health after the blow (`#leth_hp dw.sys`, reset to a
sentinel first so a failed `data get` cannot leave a previous player's zero
behind) — which covers every refusal in one rule and needs no list of them.

**What the PackTest tier can and cannot witness.** A PackTest fake player is
**permanently undamageable**, not merely spawn-invulnerable: the same dummy stood
inside a volume whose loop swings every tick and was still at `Health: 20f` after
**202 ticks**, far past the 59-tick window, with `minecraft:generic` refused
identically. So a player *death* cannot be witnessed at this tier at all, and the
generated templates do not pretend to — that claim belongs to the bot tier, which
drives a real client. What the dummy is ideal for is the opposite direction: a
body that provably never dies must never produce the claim, which makes it a
standing fixture that never expires. The two generated templates split on exactly
that line, and each is red for its own reason (measured): `lethal_<id>` fails when
the damage amount is stripped, `lethal_<id>_claim` fails when the player line is
deleted from the driver. That second binding is why the claim template drives the
**driver** and not the kill function — a template calling the kill function
directly stays green with the player path deleted. **A staged volume** (spec-0088)
gets a third: `lethal_<id>` sets the gate **open** (every required flag 1, every
forbidden flag reset, every numeric datum on the value `DatumSet::pick` chooses)
and drives `lethal_<id>_tick` rather than `lethal_<id>`, so a stripped sweep reds
it; `lethal_<id>_shut` sets the gate shut by exactly one term (the first required
flag reset, else the first forbidden flag set, else the first numeric datum on a
value its first term refuses), drives `lethal_<id>_tick`, and asserts the dummy
still at `Health` 20 — so a stripped guard reds it. Both put back every score they
touched.

#### Why the wording is a component, and not a custom damage type

Vanilla's own spelling for "a death message the pack wrote" is a datapack
`damage_type` with a `message_id`, whose key the client resolves from a lang
file. It is **rejected here**, and the reason is an existing invariant rather than
a preference: vanilla builds that message with no `fallback` field, so a player
who declines the resource-pack prompt would read a raw `death.attack.…` key —
which spec-0029 §3 makes the delve's playable-in-English guarantee against, and
which `DW0185` would not catch (the emitted literal is the key, not the authored
string). The wording therefore travels the one path every player-visible string in
this engine travels, `emit::tr` → `{"translate":…,"fallback":…}`, and vanilla's
own broadcast still fires, worded by the declared `damage_type`: the party reads
*who* died, the victim reads *what the place was*.
