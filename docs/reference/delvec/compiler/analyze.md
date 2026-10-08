# `delvec::compiler::analyze`

The reference page for `crates/delvec/src/compiler/analyze.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW02xx — analysis (`compiler::analyze` reachability + `compiler::light` lighting; error; exit 2)

`DW0210`/`DW0211` are emitted by the assembled-world light model
(`crate::light`), surfaced through the build path but mapped to exit 2 (analysis
tier) in `main`; `DW0201`–`DW0204` come from `compiler::analyze` over the
branch-coherent flow model (`compiler::flow`).

**The emitter table never overestimates (`crate::light::emission`).** Both gates
are only sound if the modelled light is a *lower* bound on the game's — a block
modelled brighter than vanilla lets a genuinely dark area ship unmitigated. The
table is evaluated over each block's **actual blockstate** (the assembled map
carries full states). Blocks absent from the table emit 0 (an underestimate, the
safe direction). A state-dependent block is never collapsed onto its brightest
state: `sea_pickle` is `3 + 3·pickles` when waterlogged and **0 when dry**;
`redstone_ore` is **0** idle and 9 when `lit`; `respawn_anchor` is **0** at
`charges=0`; `amethyst_cluster` is 5 (buds 4/2/1); `brewing_stand` and
`brown_mushroom` are **1**; `glow_lichen` is 7 where it is attached to a face and
**0** in its faceless default state (what a bare `minecraft:glow_lichen` places);
`glow_item_frame` is **0** (it is an entity, not a block, and emits no block light
in Java — 7 is a Bedrock value); the furnace family reports 13 when `lit`. Blocks
whose `lit`/`charges`/`berries` state has a *bright* default (campfire, soul
campfire, redstone torch) still evaluate bright from a bare id, so the compiler's
own relight fixtures are unaffected.

**The values are measured against the pinned game, not cited.** Every entry is
checked by `crates/delvec/tests/emission_table.rs` against
`crates/delvec/tests/fixtures/light/emission-1.21.11.tsv` — 1419 rows that
collapse all **29,671** blockstates of the pinned 1.21.11 server jar onto the
properties that can change their light, every value being what the game's own
`BlockState.getLightEmission()` returns. `tools/maintenance/dump-block-light.py` regenerates
the fixture and refuses any jar whose sha256 is not the `versions.toml` pin;
`--check` re-derives it and diffs. Three assertions: `emission ≤ game` over every
blockstate (the contract); exact equality everywhere the table is not
deliberately taking a minimum; and the set of blocks measuring *below* the game
equals the declared set, so a future Minecraft's new emitter reds here rather
than silently costing a designer their fixture.

**Every light-emitting block of 1.21.11 is modelled**, including the ones a
designer reaches for first. Candles are **3 per candle and only while `lit`**,
which defaults to false — so a shipped candle is dark at any count and four lit
ones are 12; all seventeen candle ids (plain and dyed) and all seventeen
candle-cake ids (3 when lit) behave the same. Copper bulbs are **15 / 12 / 8 / 4**
by oxidation stage and only while `lit` (default false), waxed and unwaxed alike;
copper lanterns are **15 at every stage**, which is not the same rule; `copper_torch`
and `copper_wall_torch` are 14. `sculk_catalyst` is 6, `nether_portal` 11,
`firefly_bush` 2, and `dragon_egg`, `end_portal_frame`, `sculk_sensor` and
`calibrated_sculk_sensor` are 1.

**Where the world re-derives the property at load, the entry is the minimum over
the states the world can reach.** The model evaluates the blockstate the world
*ships* with; it does not simulate redstone, block entities, weathering or player
action. `redstone_lamp` has no `onPlace` and its `neighborChanged` schedules an
unlight the first time any neighbour updates while no signal is present — which
structure assembly does — so a shipped `lit=true` lamp is not a stable
configuration and the entry is **0**. `trial_spawner` and `vault` have their state
owned by a block entity, giving **0** and **6** respectively. `copper_bulb` is not
in that set: its `onPlace` runs `checkAndFlip`, which returns without touching
`lit` whenever the neighbour signal already agrees with `powered`, so a bulb
shipped lit in a room with no redstone stays lit.

**Light passability and nav passability are one rule
(`crate::light::passes_light`).** A block whose cell a body can occupy passes
light, and which cells those are is decided by the one collision table,
`delvewright_dsl::blockshape::collision_class` — the answer
`assembled::occupancy_of` builds the nav model from. `Air`, `Thin` (collision top
below 8/16: trap triggers, carpets, `snow` at 1–4 layers, candles, flower pots,
every no-collision plant, and every no-collision fixture — torches of every kind,
signs, hanging signs, banners, buttons, levers, rails, `redstone_wire`, `light`,
`structure_void`) and `FenceGate` pass light by construction, and so does a
`Fluid` cell (water, lava, seagrass, kelp, bubble columns). A class the collision
table gains is light-passing the day it is added. The rest of the opacity table is
the light model's own: blocks a body cannot occupy that still pass light — glass
and panes, `iron_bars`, chains, lanterns, campfires, `end_rod`, `ladder`,
`scaffolding`, `pointed_dripstone`, `cobweb`, `lily_pad`, `sea_pickle`, `snow` at
5–8 layers, and `oak_fence`. Anything else is **opaque**, which under-measures
light and is the safe direction only for a block a walker cannot enter: a cell a
body walks through, measured at light 0 while the game lights it, is a `DW0210`
no relighting can clear. Vanilla 1.21.11 agrees for the passable classes: every
empty-collision and thin block has `filterLight = 0` (pinned `minecraft-data`
`.../pc/1.21.9/blocks.json`: all 16 pressure plates, `tripwire`, `tripwire_hook`,
all 20 carpets, `snow`, all 12 fence gates), against `filterLight = 15` for
`stone`/`dirt`/`oak_planks`/`cobblestone`/`deepslate`/`sand`/`gravel`/`obsidian`/
`snow_block`. Blocks vanilla also calls transparent but the collision table calls
**solid or tall** (fences, walls, slabs, stairs, doors, trapdoors, chests) stay
opaque: no body occupies their cell, so their opacity can only make the gate
stricter. `light::tests::every_nav_passable_block_passes_light` asserts the
implication over every state of every block in the pinned registry, through the
real classifier, and prints its binding count.

| Code | Meaning |
|------|---------|
| `DW0201` | Finale quest can never complete (unreachable finale). |
| `DW0202` | Quest can never be triggered (dead quest — its trigger source never completes). |
| `DW0203` | Objective can never be completed **in any branch** (deadlock: unsatisfiable `after` chain, an unproducible `requires_flags` gate, or a `talk-to` completing option unreachable through the trigger/`after`/dialogue graph). |
| `DW0358` | A declared `min_players: n` (n ≥ 2) has **no n-agent division of labour** (spec-0018). Completability is proven with `min_players` agents: n = 1 is the unchanged single-agent proof, and n ≥ 2 additionally requires the proven playthrough to contain an AND-join with n arms that are *independently reachable at the join's frontier* — the replay state just before its earliest arm — with no arm waiting on a sibling, a flag a sibling sets, or a quest that is not active yet (`flow::Flow::divide`). Names the widest join and how many arms it actually offers, or says the campaign has no AND-join at all. Reported on `world`/`/content/min_players`, exit 2. Prescription: split one beat into n `after`-arms completable from the same frontier, or lower `min_players`. |

**The branch-coherent flow model (`compiler::flow`).** Reachability is not one
union fixpoint over "every `set-flag` anywhere". A **choice group** is a dialogue
node with ≥2 options that each set a flag — taking one means not taking its
siblings, so the options are XOR alternatives. A **world** picks one alternative
per group (the product over the *flag-reading* groups only, capped at 512;
groups past the cap stay unconstrained).
The fixpoint runs **per world**, and a quest/objective is reported unreachable
only when it is unreachable in **every** world — so the branch model makes
`DW0202`/`DW0203` strictly more precise, never looser.

A flag producer is conditional on its gating context:

| Producer | Available when |
|----------|----------------|
| `set-flag` in `on_objective_complete[o]` | `o` is completable **and** every `requires_flags` gate on the enclosing effect chain is satisfied |
| `set-flag` in a quest's `on_complete` | that quest completes, same gate rule |
| `set-flag` on a dialogue option | the option is reachable from **one of the roots the campaign can put a body in front of** (below) through options whose own gates are satisfied, and is the world's selected alternative of its group |
| `set-flag` in an environment trigger's `effects` | the trigger's `requires_flags` are satisfied — **ambient** (a `strike`/`use`/`approach` trigger is player-initiated and has no DAG position). The exported path still owes the act: `Flow::trigger_debts` replays the path with trigger producers withheld until performed, and every flag a step reads (its objective's `requires_flags`, a `talk-to`'s taken option's `requires`) that only a trigger supplies becomes a `trigger` step before that reader |
| `set-flag` in a `traps[].payload` | the trap's `requires_flags` are satisfied (ambient, same reasoning — the party can always walk over and spring it) |
| a trap's `disarm.sets_flag` | the trap's `requires_flags` are satisfied (ambient, same reasoning) |
| `set-flag` in a `shortcuts[].on_unlock` | always (ambient, ungated — `DW0373` proves the far-side `unlock` is walkable while the gate is sealed) |
| `set-flag` in a `shops[].offers[].effects` | always (ambient behind the effect's own flag gates; the offer's numeric price is a runtime balance no flag model dates) |
| `set-flag` in an `on_respawn` / `on_caught` reaction bundle | **never** — reaction bundles fire at statically unknowable times, so nothing inside one is a producer (the conservative stance `compiler::continuity` takes) — whether the bundle is rooted in the quests stage or hung off a **dialogue option's** `set-checkpoint` |
| `set-flag` in the campaign's `on_death` or a fight's `on_kill` | **never** — a death fires at a moment nothing names, and no kill is forced to be credited to a player |

Consequences worth stating plainly: a `set-flag` gated on the very flag it sets
(the "re-affirm the branch" idiom) produces nothing; a flag produced only on the
`flag/flee` branch cannot satisfy a gate on the `flag/wait` branch; and flags set
from dialogue, triggers, trap payloads and trap disarms are first-class
producers, so those legitimate shapes are not spurious `DW0203`s.

**A dialogue tree has more than one door, and the model walks from all of them.**
The roots a reachability walk is seeded from — `Flow::entry_roots` — are the
tree's declared stage-6 `root` **plus every node a quest's `cast` ledger names as
that NPC's scene**, and a ledger root counts once its quest is active and its
placement's `requires_flags` hold. A ledger root is not a shortcut into the tree:
right-click opens it directly for that quest's duration, which is what the ledger
is for (spec-0020) — an NPC's right-click being a different scene per quest. So a
node no `next` link reaches is reachable when the ledger opens it, its options'
flags are producible, and a branch that forks there is not `DW0482`. The
quantifier is **per world**: a per-branch cast clause carries the branch's flag,
so its root opens the node in the worlds holding that flag and nowhere else.
`forbids_flags` is ignored here for the same reason the option walk ignores it —
the model is monotone, and a negative gate that closes later cannot un-reach a
node the party has already stood in. `"unchanged"` needs no resolution: it
carries forward a root some earlier quest already declared, which is already in
the union. `Flow::scene_root` asks a narrower question — which ONE root a
right-click opens at a given instant, where later declaration wins — because it
models the emitted `dw.cast` dispatch; reachability is the union over the whole
playthrough. The DSL half has always asked the wider question:
`NpcDialogue::reachable_from` is the one authority for "what can this tree show,
entered here", and `DW0120`'s orphan walk and `DW0858` both seed it from a root
SET.

**Which effect lists those rows range over is not `flow`'s to decide.** Both
halves of the model — the producer scan in `Flow::new` and the
gate-flag inventory `flow::gate_flags`, which is what decides whether a choice
group is enumerated as XOR worlds or left unconstrained — walk
`plan::for_each_effect_root`, so the proof cannot believe in fewer firings than
the datapack performs. A hand-rolled walk enumerating a subset of the roots is the
defect the single enumeration exists to prevent: a `set-flag` in a
`traps[].payload` is then a producer **nowhere** in the proof while the emitted
`trap_fire_<trap>.mcfunction` really sets it (an objective gated on it dies as a
spurious `DW0203`), and a `requires_flags` *inside* such a payload is not counted
as a flag read at all — so a branch choice that only such a gate reads never
splits the worlds, and one world holds two mutually exclusive branch flags at
once. The table above is a **policy per root**; the roots themselves are
inherited, and the match on them is exhaustive, so a new root cannot be added
without `flow` deciding what it means. The payload takes the ambient stance of the
environment trigger and the trap `disarm` beside it, and the dialogue-hosted
`on_respawn` bundle is reached but never credited, which is the reaction-bundle
rule the identical quests-stage bundle obeys.

**The exported critical path is one branch (`DW0204`).** `compiler::plan` does
not walk the finale's whole stage-4 `depends_on` closure. It walks the
**playthrough** the flow model proves: the first world (deterministic
enumeration order, all-first-alternative first) whose finale quest completes,
restricted to the quests that complete in it, with each `talk-to` taking the
completing dialogue option that belongs to that branch. Before export, the
sequence is **replayed** step by step through the flag/objective/quest state
machine: every step's quest must be active, its `after` prerequisites completed,
its `requires_flags` set and its `forbids_flags` unset *at that position*, its
completing dialogue option reachable *at that position*, and `campaign-complete`
must fire exactly at the final step. The first violation is `DW0204`, naming the
step. Every reachable branch's exported path (`Flow::playthrough_in` over the
world `branch::realize` picks, the sequence `validation/branch-path-<slug>.json`
publishes) is replayed by the same rule, and its violation is `DW0204` naming the
branch and the file: the branch runs walk that file as the bot ladder walks the
critical path, and the harness fails a walk whose `campaign-complete` arrives
while objective steps remain. A branch path orders every quest its world
completes by `depends_on` (`flow::dag_order`); a quest that can end the delve
(any `campaign-complete` in its effect lists, nested ones included) is held
until no other quest is ready, so an optional strand that becomes ready beside
the ending is walked before it. `compiler::plan`'s gate-aware reachability (`DW0306`) judges the same
sequence, so the static proofs and the exported bot contract agree by
construction. When no world completes the finale the campaign is already
`DW0201`; the model then degenerates to the whole closure so the geometry-only
commands (`chart`, `snapshot`) still run on an unanalyzable campaign.

**Optional participation can never gate the mainline.** The contract is that
*the mainline must be completable with zero optional participation*. Optionality
is not a DSL declaration — it is **derived**: the **mainline** is exactly the
critical path above, the participation the campaign requires to reach
`campaign-complete`; every other act a player may take (a side objective, a
non-path dialogue option, an elective trigger/trap/wave) is optional.

*The producer half is `DW0204`.* The replay is already the participation-minimal
walk: it credits only the mainline's own producers — the taken option's flags,
on-path completion bundles, and the ambient trigger/trap flags any player can
fire — so a mainline objective gated on a flag only an off-path quest or an
unselected option sets fails the replay.

*The order half needs no rule* (spec-0093 §6.3). Every objective driver the
compiler emits — the dialogue button that fires `complete-objective` included —
goes through `pending_guard` (quest active ∧ `after` complete ∧ `requires_flags` ∧
`forbids_flags` ∧ `requires_state` ∧ not yet complete), so a button that would
walk the party past a load-bearing beat is not drawn until that beat is done, and
a press that arrives early completes nothing. The canonical instance — a button
completing `obj/climb-out` (`after obj/surf`) beside one completing `obj/muster`
(whose bundle spawns the drowned) from campaign start — cannot be emitted.
`DW0191` is unchanged: it counts an author-written gate on the one completing
option, which could deadlock a beat; the engine's pending guard opens exactly
when the objective activates and cannot.

**`forbids_flags` and producibility (conservative).** The reachability
fixpoint models `requires_flags` producibility (a gating flag must be producible
by an already-completable producer on the same branch) but deliberately
**ignores** `forbids_flags`: whether a forbidden flag is set when an element is
needed depends on play order — full temporal reasoning the existence fixpoint
does not attempt. An element with a negative gate is therefore treated as
fireable, so `forbids_flags` can never cause a spurious `DW0202`/`DW0203`. The
**compensating stronger check** is the `DW0204` path replay, which does have a
concrete order and enforces every negative gate at its real position on the
exported path. The other static guarantees that hold: every `forbids_flags`
reference resolves to a produced flag (`DW0172`), and a completing dialogue
option gated only by `forbids_flags` still counts as gated for `DW0191`.
