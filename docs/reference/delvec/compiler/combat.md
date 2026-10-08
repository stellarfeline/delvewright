# `delvec::compiler::combat`

The reference page for `crates/delvec/src/compiler/combat.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW047x — combat winnability (`compiler::combat`; spec-0023)

The arithmetic half of spec-0023's three combat proofs. The ruling behind them:
"the average player can win" is not a provable claim and is not
pretended — the machine proves a fight is REACHABLE, RETRIABLE and
**structurally winnable**, and leaves human skill open on purpose. These are the
structural half; the retry loop and the muster belong to the bot ladder
(spec-0003 / §8 below).

Runs only for a campaign with at least one `kill` step on the compiled critical
path, over the SEATED wave-spawn cells (`plan_wave_spawns`, so it reasons about
where mobs actually land rather than where the anchor is). Build-tier (exit 3)
except `DW0475`, which warns (exit 0). None of it judges whether a fight can be
won or how hard it is: that is judged on a human playtest.

**Every number is Mojang's own, or the answer is "unproven".** Weapon damage and
armour come from the vendored `minecraft:attribute_modifiers` default components
(`data/item-combat-1.21.11.json`); a damage
type's armour behaviour and difficulty `scaling` come from
`data/damage-types-1.21.11.json` (the `#minecraft:bypasses_armor` tag + the
registry's own `scaling` field). Mojang publishes **no** per-entity default
attributes, so mob base health is genuinely unknowable at build time — the
numeric bound therefore runs only where `attributes.max_health` is declared, and
`DW0475` says so rather than inventing a health table (the same refusal as
`nav::DEFAULT_FOLLOW_RANGE` and `clearance::MODEL_MARGIN`).

**The Easy-halving trap, stated once.** `WorldDifficulty`'s doc comment gives the
Easy formula `min(dmg/2+1, dmg)`, and applying it here would be wrong by 2× in
the LENIENT direction. Difficulty scaling is a property of the damage TYPE, and
`damage-players` emits a bare `/damage <target> <amount> <type>` with **no
attacker** — so the eight types whose `scaling` is
`when_caused_by_living_non_player` (everything the DSL exposes except
`explosion`) are not scaled at all. The one type that does scale (`explosion`,
`always`) is also the one armour reduces, so it is not adjudicated either; a test
pins that pairing so a future MC pin breaking it fails loudly.

| Code | Meaning |
|------|---------|
| `DW0470` | A hostile the party is **required** to kill can never be damaged, so its `kill` objective can never complete and the delve soft-locks. Build-tier (exit 3), `compiler::combat`. Immunity is spelled one way on a wave mob: a `minecraft:resistance` effect at amplifier 4 (level V), which is 20%-per-level × 5 = 100% reduction against everything outside `#minecraft:bypasses_resistance` — the same fact the emitter already leans on for its PackTest scaffolding, so nothing in a player's kit can reach it. Only waves a critical-path `kill` step names are held to this; an optional wave may be as immortal as the content likes. (An `unleash-actor` twin is deliberately NOT covered: the twin summon carries no `Invulnerable` NBT whatever the actor's `vulnerable` flag says, so it is always killable.) Prescription: lower the amplifier to at most 3 (80% reduction — still an extremely tanky elite), or move the durability into `attributes.max_health`, where it becomes a number `DW0472` can bound. Do NOT delete the `kill` objective to silence it: an unkillable mob in the room is still an unkillable mob. |
| `DW0471` | A hostile the party is required to kill has **nowhere to be fought from** — no standable cell anywhere around its seated body, so no player can stand within reach and the `kill` objective can never complete. Build-tier (exit 3), `compiler::combat`. Deliberately **local**: a Chebyshev-1 ring around the columns the body's footprint occupies (widened by `nav::entity_dims`, the one dims table), over the elevations it spans. It says nothing about global connectivity, which is what keeps it free of the false positives a reachability flood would produce — a room legitimately shut behind a gate or a shortcut is not disconnected, and `check_critical_path` already owns that question. What it catches is what nothing else does: `DW0312` proves the spawn cell is standable, and a 1×1 pocket with a floor passes that while being unfightable. Prescription: move the wave `anchor` into open floor, or carve the pocket. Do NOT widen the wave spawn search — the mobs would simply be seated somewhere the author never staged. |
| `DW0472` | A mandatory encounter's **declared** health outlasts the best kit the party can field. Build-tier (exit 3), `compiler::combat`. A bound against a wrong number, not a difficulty opinion: whether a fight can be won is judged on a human playtest. **The unit is the ordinary swing** — fully charged, not critical, not a sweep — the one blow whose damage is a function of the kit alone. **The blow** is the player's base `attack_damage` 1.0 (`combat::PLAYER_BASE_ATTACK_DAMAGE`, the pinned server's `Player.createAttributes`) plus the largest weapon `attack_damage` modifier across every class kit — an iron sword is 6. A kit item carries no enchantments (the `classes` schema has no field), so there is no Sharpness term; a weapon found, given or bought later is not the kit. **What it lands as**, per stack, in the pinned game's own `f32` arithmetic and order (`LivingEntity.actuallyHurt` at 1.21.11; `combat::landed_blow`): armour first, keeping `1 − clamp(a − hit/(2 + t/4), a/5, 20)/25` of the blow, with `a` the floored armour attribute (capped 30) and `t` the toughness (capped 20); then `minecraft:resistance`, 20% per level; then `minecraft:protection` on pieces in the four armour slots, keeping `1 − min(p, 20)/25`. Armour and toughness are those of pieces worn in their own `equippable` slot (`combat::worn_as_armour`, the rule the muster's armour floor also reads). A species' own base armour (a zombie's 2) is in no published data and a `body`-slot piece's effect depends on the species, so neither is counted: every omission makes the blow land harder, so the count is the fewest swings the fight can take and a refusal never rests on an assumption. Difficulty is not a term: the game scales only damage a player takes (`Player.hurtServer`). **The count** is Σ `count × ceil(max_health / landed)` over the stacks that declare `attributes.max_health` — per body, since a killing blow's excess reaches nobody — and the gate refuses when it exceeds **400** (`TTK_BUDGET_HITS`). An iron sword's 6-point blow clears eight bare 20-HP zombies in 32, and the measured Unremembered Guard (34 HP, iron helmet + chestplate: the blow lands 4.8 on the counted armour 8, 4.32 on the 10 the server reads) in 8 a body either way. The message shows each stack's armour, toughness, resistance, protection, landed blow and per-body count, plus an indicative duration from the weapon's `attack_speed` for context only. Prescription: lower `max_health`, cut the stack `count`, lighten the armour, or put a stronger weapon in a kit. Do NOT raise the budget. |
| `DW0473` | An **unavoidable** scripted hit on the critical path kills a full-health player outright (landed damage ≥ 20). Build-tier (exit 3), `compiler::combat`. Scope is what the party can do nothing about: `damage-players` in a quest's own `on_complete` / `on_objective_complete` bundle (descending `sequence` steps, which are the same unconditional bundle on a timeline). Everything with counterplay is outside it on purpose — trap payloads, stealth `on_caught` and `move`-reaction bundles, dialogue-option effects, and any `damage-players` carrying a `within` zone, since standing elsewhere IS the dodge. spec-0016/0022's telegraph and saturation rules govern those. Only armour-bypassing damage types are adjudicated (the default `generic` is one): for the rest, what lands depends on what the player wears at that beat, which a slotless kit list does not state. The message shows the arithmetic AND the rule it used, naming the damage type's `scaling` explicitly so nobody re-derives the Easy halving wrongly. Prescription: lower the `amount` below 20, or move the consequence onto a beat the party can play around. |
| `DW0475` | (**warning**; exit 0) The numeric time-to-kill bound **could not be computed** for one or more mandatory encounters, so they ship with the structural proofs only (damageable, reachable, wired) and no arithmetic. Two causes, both stated per encounter: a stack that declares no `attributes.max_health` (Mojang publishes no per-entity defaults, so its health is unknown — see the block header), or a party whose kits carry no item with an `attack_damage` attribute at all, which means the damage output is unknown rather than zero (a bow's damage is projectile code and appears in no vanilla data; absence in the item table is a fact about attributes, never a claim of harmlessness). One finding per campaign, listing every affected encounter. Prescription: declare `attributes.max_health` to opt the encounter into `DW0472`. Deliberately advisory: an encounter left on vanilla stats is legitimate — the author just has to see that nothing arithmetic was proven about it. |
