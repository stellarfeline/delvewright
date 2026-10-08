# `delvewright_dsl::wave`

The reference page for `crates/dsl/src/wave.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW01xx — validation (`dsl`; severity error; exit 1)

This module's rows of a section whose prose is on the [`delvewright_dsl::diagnostic` page](diagnostic.md#dw01xx--validation-dsl-severity-error-exit-1).

| Code | Meaning |
|------|---------|
| `DW0381` | A wave's TD `lane` / `summon` declaration (spec-0016 §6) is structurally invalid or internally contradictory: an empty `waypoints` list, a waypoint anchor no area's prefab provides, a repeated consecutive waypoint (the squad would be sent where it already stands, and vanilla re-rolls a patrol target on arrival), an `aggro_radius` outside `4..=64`, a mob whose `attributes.follow_range` disagrees with `aggro_radius`, or `lane` together with `summon: aggro-edge`. The `follow_range` clause is the subtle one: release radius and perception radius must be the same number, because a patrolling raider that targets a player it cannot engage HOLDS GROUND instead of marching — the squad stalls mid-lane with every other proof green. Validation-tier (exit 1), `dsl::validate`; anchor resolution stays lenient for pool areas the compiler resolves later. |
| `DW0382` | A lane wave fields a non-raider species (spec-0016 §6). `Patrolling`/`patrol_target` are Raider NBT: on any other mob they are simply dropped and it stands where it spawned — the silent no-op class. **The lane roster is Mojang's, never ours**: it is vanilla's own `#minecraft:raiders` tag, read from the vendored entity-type tag table (`crates/dsl/data/entity-tags-1.21.11.json`, regenerate with `tools/maintenance/extract-entity-tags.py`), the same rule `DW0496` follows for `#minecraft:burn_in_daylight`. For 1.21.11 it holds evoker, illusioner, pillager, ravager, vindicator and witch. Three independent readings of the pinned server jar agree on that six: the tag itself; the entity types whose constructed class is a `PatrollingMonster`; and the entity types whose class is a `Raider`. The three NBT keys are string constants of exactly one class an entity is built from, `PatrollingMonster`, whose own `registerGoals` adds the `LongDistancePatrolGoal` every subclass inherits — so honouring the NBT and having the goal are the same membership question. `tools/maintenance/check-patrol-types.py` re-derives all of it from the pinned jar and refuses on any disagreement. Validation-tier (exit 1). Prescription: use `summon: aggro-edge`, which needs no patrol AI, for everything else. |
| `DW0383` | A lane wave fields fewer than 2 mobs (spec-0016 §6). A lone patroller sets `Patrolling:0b` on ITSELF when it finds no companion within its follow range (vanilla, live-verified), so a one-mob lane cancels its own routing. Validation-tier (exit 1). |
| `DW0384` | A lane `pillager` is not holding a crossbow (spec-0016 §6). Its only attack goal is the crossbow goal, so on acquiring a target it has nothing runnable to do — while the patrol goal is meanwhile blocked BY that target — and it freezes in place indefinitely (live-verified deadlock). The compiler arms pillagers by default, so this fires only on an explicit `equipment.main_hand` override, which is exactly the remaining way into the deadlock. Validation-tier (exit 1). |
| `DW0385` | A `summon: aggro-edge` wave mob declares no `attributes.follow_range` (spec-0016 §6). That radius IS the summon ring — the distance at which the mob perceives the party — so it is authored, never guessed: the compiler will not fabricate a vanilla default it cannot verify against the pinned server. Validation-tier (exit 1). |
| `DW0370` | A wave declares `respawns_on_rest: true` but the campaign declares **no** `bonfire` (spec-0016 §1) — nothing can ever fire the re-seat, so the field is a silent no-op, the defect class this compiler always makes loud. Validation-tier (exit 1), `dsl::validate`; the scan descends every nested effect list (a `bonfire` inside a `sequence` step counts) over quests and triggers. Prescription: add the bonfire the re-seat hangs off, or drop the field — never leave a dead declaration in the DSL. |
| `DW0499` | A wave declares **both** `tier: boss` and `respawns_on_rest: true` (spec-0016 §1, spec-0023; stage bosses never respawn on rest). `tier` and `respawns_on_rest` are two fields on the SAME wave declaration — the only place a "boss" billing and a "re-seat on rest" contract can land on one another: an actor carries `tier` too (spec-0023's "other shape an elite takes"), but has no `respawns_on_rest` field at all — an actor is killed by hand, never by a `kill` objective, and the bonfire re-seat machinery only ever re-summons **waves** — so an actor-shaped boss is structurally incapable of expressing this violation, and the check is scoped to the one shape that can. A rest-respawning boss re-fight breaks the retry economy that rule protects: a boss is the campaign's named fight, not trash pressure the party grinds back down every rest. Validation-tier (exit 1), `dsl::validate`; checked unconditionally of whether a `bonfire` exists — the combination is forbidden on its own terms, not merely inert like `DW0370`. Prescription: drop `respawns_on_rest` if the encounter really is the boss, or drop `tier: boss` (bill it `elite` instead) if it is meant to re-seat. |

### DW043x — geometry & container proofs (stair orientation; spec-0021 loot; `collect` container adoption)

This module's rows of a section whose prose is on the [`delvewright_dsl::loot` page](loot.md#dw043x--geometry--container-proofs-stair-orientation-spec-0021-loot-collect-container-adoption).

| Code | Meaning |
|------|---------|
| `DW0433` | An enchantment id — on an `equipment` piece, a `loot` stack, or (spec-0075) a `give-item` at any effect root — is not in the pinned 1.21.11 enchantment registry. Validation-tier (exit 1). The registry is the 43-id `enchantment` list from the same misode/mcmeta 1.21.11 summary the item registry comes from. The message calls out the classic trap explicitly: vanilla's curse ids are `minecraft:binding_curse` and `minecraft:vanishing_curse`, never `curse_of_binding`. |
| `DW0434` | An enchantment level outside `1..=255`, the range the `minecraft:enchantments` component can store. Validation-tier (exit 1). Levels **above an enchantment's survival maximum are deliberately allowed** — exceeding it from a command is legal vanilla and is precisely how a set-piece elite is built, so the compiler does not overrule that design call. `0` means "not enchanted" and is silently dropped by the game, which is why it is rejected rather than ignored. |

### DW046x — the NPC scene ledger (`compiler::cast`; spec-0020)

This module's rows of a section whose prose is on the [`delvec::compiler::cast` page](../delvec/compiler/cast.md#dw046x--the-npc-scene-ledger-compilercast-spec-0020).

| Code | Meaning |
|------|---------|
| `DW0469` | (**warning**; exit 0) A campaign stages actors meant to **fight** — unleashed into a real-AI twin, or declared `vulnerable` — but declares no `waves[]` and no `world.difficulty`, so it ships the derived `difficulty=peaceful` and a monster among them is discarded on the tick it spawns. "Meant to fight" is read off the campaign's own declarations (`unleash-actor`, `vulnerable`), never guessed from the species: the pinned entity registry is a membership set with no mob-category data, so *is this a monster* is exactly the question the compiler cannot answer — which is why this is advisory. Prescription: declare `world.difficulty`. |

### DW0490–DW0493 — declared drops (`dsl::wave`)

**A mob may wear many pieces, but what it leaves behind is a declared subset —
usually one piece, never automatically everything.** The
DSL says WHICH pieces drop; quest items may be declared as drops too. All four
codes are validation-tier (exit 1), in `dsl::validate::check_drops`. An
undeclared slot keeps drop chance `0.0f`.

| Code | Meaning |
|------|---------|
| `DW0490` | **A drop nobody wears.** A `drops[]` `slot` entry (one of the eight slots of `EquipSlot::ALL`, `body` and `saddle` included) does not name a distinct slot the same entity's own `equipment` fills — the slot is empty, or the same slot is declared twice. A body can only leave behind a piece it wore, and only once. The message names both sides: the slot asked for, and the slots actually filled. Prescription: equip the slot, or declare one the kit fills. |
| `DW0491` | **Drops on an untiered fight.** `drops[]` on a wave or actor that is not billed `elite` or `boss`. Only a named fight leaves anything behind; making rank-and-file gear lootable is grind, which the constitution forbids, and the failure would be silent (a farmable mob looks exactly like an unfarmable one in the DSL). Prescription: declare the encounter's `tier`, or remove the drops. |
| `DW0492` | **An unsourced drop-gated collect.** A `collect` `dropped_by` is not backed by the wave it names: the wave declares no `{item}` drop of this objective's item (the message lists what it *does* declare), the objective asks for more copies than the wave's mobs can yield, or the objective also adopts a `container` — the item comes off a body or out of a box, never both. Prescription: declare the drop on the wave's mob, lower the count, or drop whichever provisioning the beat does not use. |
| `DW0493` | **A prize that arrives before the fight.** A `collect` `dropped_by` is not ordered after a `kill` objective for that wave — not through the intra-quest `after` graph, not through a quest this one `depends_on`. Without that edge the objective reads as active from the campaign's first tick over an item that does not exist yet, and "kill the boss, take its key, open the door" is an authoring intention the quest graph cannot check. Prescription: add the `kill` and list it in this objective's `after`, or put the kill in a quest this one depends on. |

#### The vanilla primitives, and why these numbers

Both halves are vanilla, verified against the **pinned 1.21.11 jar** rather than
folklore:

- **Worn pieces** ride the `equipment` / `drop_chances` compounds the compiler
  already writes. A declared slot gets **`2.0f`**, not `1.0f`. Vanilla's
  `DropChances` record (class `cgi`) names both numbers itself:
  `withGuaranteedDrop(slot)` writes the constant `2.0f`, and `isPreserved(slot)`
  is `chance > 1.0f`. `Mob.dropCustomDeathLoot` (class `chn`) reads both — a slot
  at exactly `0.0f` is skipped outright, and a **preserved** slot both drops when
  the killing blow was not a player's *and* skips the durability randomization
  that a chance of `≤ 1.0` applies to a damageable item. At `1.0f` a boss axe
  would drop with a die-rolled amount of damage on it, which is not a
  deterministic drop. (The same `2.0f` is what vanilla's own
  `SaddleEquipmentSlotFix` datafixer writes for a saddle a horse always drops.)
- **Quest items** have no slot, and hanging one in an off-hand the author never
  dressed would be exactly the downstream workaround the no-hack rule forbids.
  1.21.11 answers the slot-less half with its own primitive: `Mob` reads
  `DeathLootTable` (and `DeathLootTableSeed`) straight off summon NBT through the
  `ResourceKey<LootTable>` codec, and `dropAllDeathLoot` rolls it on death. The
  compiler writes `DeathLootTable:"minecraft:empty"` on every actor; a
  declared item drop points the same field at
  `data/<ns>/loot_table/dw_drop/{actor_<id>|wave_<wave>_<i>}.json` — one pool,
  one roll, one `minecraft:item` entry per declared item, no RNG (ADR-0006). A
  declared display `name` becomes `minecraft:set_name` with `target:
  "custom_name"` (both targets confirmed in the jar), the **same component** a
  `collect`'s `item_name` writes into a container stack, so the key a boss leaves
  on the ground and the key a barrel hands over are the same item.

**Removal is not a death the player earned.** Every removal the compiler performs
itself ends in `/kill` — in place for a `despawn-actor` `kill`, under the world
for every other removal (§4 "A body the story removes is never seen to die") —
which is an ordinary death, and a preserved slot
survives a non-player kill — so an elite the story re-cages would shed its axe on
every rest. The `unleash` that removes the puppet and both `despawn-actor` styles
therefore strip the declaration off the body first, with two intended primitives
composed: `execute as @e[tag=…] run data merge entity @s` (single-entity by
construction, which is what `data merge` requires) writing `0.0f` on every slot
and an empty death loot table. Emitted only for actors that declare drops.
