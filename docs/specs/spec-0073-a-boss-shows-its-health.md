# spec-0073: A boss shows its health — the health bar as a property of the fight

- **Status**: Draft
- **Ground**: written against engine `509ebc58` (`origin/main`), read only — `Wave`, `WaveMob`, `Actor`, `EncounterTier` in `crates/dsl/src/stages.rs`; `wave_tag`, `wave_total`, `undefeated_reseat_waves`, `reseat_actors` in `crates/delvec/src/compiler/plan.rs`; `hostile_actors` in `crates/delvec/src/compiler/combat.rs`; the wave live census, `wave_census_one_<wave>`, `unleash_<actor>` and `emit_reseat_undefeated_packtests` in `crates/delvec/src/compiler/emit.rs`; the pinned command tree `crates/delvec/data/commands-1.21.11.json`; `docs/reference/compiler.md` (`waves[].tier`, `actors[].tier`, the `bonfire` emission row, the l10n key scheme) and `docs/reference/i18n.md`; the gallery (`gallery/quests.json`, `gallery/probes/`); and the content branch `origin/campaign/vesperhold` of `delvewright-campaigns` (`campaigns/vesperhold/quests.json`).
- **What it is for**: a player fighting a fight the campaign bills as a fight can see how it is going. In the owner's playtest of `vesperhold`, *The Porter* (`wave/porter`, `elite`) and *The Ringer Unmade* (`wave/ringer`, `boss`) were fought with no readout: `bossbar` appears in the pinned command tree and nowhere else in the engine.
- **Research**: §7 is this spec's research record. Every rule below is marked **cited** (the record, a vanilla behaviour proven on the pinned server, or a constitution rule requires it) or **authored** (this spec chooses).
- **Numbers**: no ADR (no settled decision moves). **Four DW codes**: `DW0909` for §8.1 (a bar on a body whose health cannot move), `DW0910` for §8.2 (a bar with nothing to title it), `DW0911` for §8.3 (a colour or style the pinned game does not draw), `DW0912` for §8.4 (the advisory on a `boss`-tier fight with no bar). **`dsl_version` `0.31.0`** (ADR-0024; `0.30.0` on `main`): §2 adds a field to two object classes, so a document that carries it stops parsing under the previous number; the value is typed in `crates/dsl/Cargo.toml` and nowhere else.
- **Non-goals**: a victory banner (`narrate` on the wave's `kill` objective already says *Great enemy felled* in the campaign's own words); a per-phase bar (a phase is a second wave); a bar for players (the HUD has hearts); any change to what `tier` does to emission; the mineflayer tier reading the bar (§9).

## 1. The finding

The two fights that had no bar are one-mob waves: `wave/porter` is one `minecraft:vindicator` named *The Porter* with `max_health` 90, `tier: elite`; `wave/ringer` is one body named *The Ringer Unmade*, `tier: boss`. The engine already reads a body's health every tick — the wave live census (`execute store result score … if entity @e[tag=dw_wave_<id>,nbt=!{Health:0.0f}]`) — and `wave_census_one_<wave>` already reads `data get entity @s Health` and `attribute @s minecraft:max_health get` for the harness. Nothing shows any of it to a player. The vanilla primitive that does is `/bossbar`: present in the pinned tree in every form §6 uses, emitted by nothing.

## 2. Where a bar lives — a declared property of the fight, on both shapes a fight takes

**Rule (authored).** `health_bar` is an optional property of `waves[]` and of `actors[]`, one shared type on both:

```
health_bar: { title?, range, color?, style? }
```

`title` is a player-visible string (§5), `range` an integer 4..=64 in blocks (§4), `color` and `style` vanilla vocabulary (§5). Absent ⇒ no bar, and emission is byte-identical to today for every campaign that declares none.

**Why these two classes.** A bar acts on a *fight*, and the engine's two fight classes are exactly the two that carry `tier` (spec-0023) and the two spec-0016 §1's undefeated re-seat is defined over: the wave (`dw_wave_<id>`, `undefeated_reseat_waves`) and the actor (`dw_actor_<id>`, `hostile_actors`). Both already own a tag over every body they field, which is what a bar sums over (§3); one type on both is the `equipment` precedent (one shape on `WaveMob` and `Actor`, *so the two surfaces cannot drift*).

**Why declared, not derived from `tier`.** `tier` is *a declaration, never a knob: the compiler is forbidden from scaling content from it*, and it reaches emission in exactly one place (`docs/reference/compiler.md`, `waves[].tier`). A bar drawn because `tier: boss` would be a second place and would make the billing a knob. The two are also different judgements: an ambush elite whose whole point is the un-telegraphed first kill (spec-0016, the 初见杀 ruling) is billed `elite` and wants no bar; a choir of six drowned nobody bills wants one. The creator states each. A `boss`-tier fight without a bar is legal and builds; it is **advised** (`DW0912`, warning tier, never blocking — §8.4), because a campaign's named fight with no readout is the finding this spec exists for. `elite` and `ordinary` fights are never advised: that is the room an un-telegraphed first kill or an ambush elite needs.

**Why not on `waves[].mobs[]`.** A mob entry is a *kind* of body (`entity`, `count`, `attributes`, `equipment`, `drops`); the wave is the encounter (`tier`, `respawns_on_rest`, `lane`, `summon`). A bar per entry would need a per-entry body tag the engine does not emit and would make *boss with adds* one wave with a bar on one entry — but the same shape is already expressible as two waves (§3), which is how the re-seat, the kill objective and the floor gate already see a fight. No second tag, no second field.

## 3. What a bar tracks

**Rule (authored).** A bar's **value** is the sum of `Health` over every live body wearing the fight's tag, read from the server every tick (`nbt=!{Health:0.0f}` — a dying body is not a standing one, the live-census rule). A bar's **max** is the sum of `max_health` over the fight's bodies, captured **at every summon of those bodies** (`spawn_<wave>`, `spawn_actor_<id>`, `unleash_<id>`, and the re-seat entries that re-summon them), from the server's own `attribute … max_health get` — never a species table the compiler cannot verify (`DW0475`, the census's own rule). Captured rather than re-summed per tick because a stack of three at one dead body must read two-thirds, not full.

| shape | how it is written | what the player sees |
|---|---|---|
| one named body (*The Porter*, *The Ringer Unmade*) | one wave, `count: 1`, `health_bar` | one bar, that body's health |
| a boss with adds | the boss as one wave with `health_bar`; the adds as a second wave without; both spawned by the same beat | one bar, the boss only |
| a duo (Ornstein and Smough) | two waves, each with `health_bar` | two bars, one per body (the game shows several bars at once — §6) |
| a body that is many (Four Kings, Godskin Duo, a choir) | one wave, `count` > 1 or several entries, `health_bar` | one bar, the sum; it drops as members fall |
| a staged elite (the kneeling knight) | an actor with `health_bar`, `unleash-actor`ed somewhere | one bar over the **unleashed twin** — the puppet is not yet a fight |
| a damageable puppet nothing unleashes (a TD creep) | an actor with `vulnerable: true` and `health_bar` | one bar over the puppet — its health can move |

For an actor the selector is `tag=dw_actor_<id>`, narrowed by `tag=!dw_pup_<id>` unless the actor is `vulnerable`: the bar binds to the bodies whose health can move, and §8.1 refuses the one combination in which none can.

## 4. Who sees it, and when

**Rule (cited — §7: the bar appears when the fight is engaged, per player).** A player sees a bar while **some live body of the fight stands within `range` blocks of that player** and the player is not watching a cutscene (`tag=!dw_cutscene`, the audience exclusion every player-directed effect carries). Nothing else: no arming beat, no flag, no party-wide switch. `range` is the creator's statement of the arena (the same 4..=64 bound `lane.aggro_radius` carries; out of range is `DW0100`, the schema-stated bound restated at the document tier, the `firework` precedent). It is required, not defaulted from `follow_range`: the bar is meant to be seen on crossing the threshold, before the body has perceived anyone, and the two radii are different facts.

| event | what happens | by which rule |
|---|---|---|
| a player walks into `range` of a live body | the bar appears for that player, at the fight's current health | §4 |
| a player leaves `range`, dies, or respawns at a bonfire | the bar disappears for that player | §4 |
| the last body dies | the bar disappears for everyone on the tick of the death | §3 (no live body ⇒ no value, hidden) |
| a party wipe | everyone respawns out of range; the bar is hidden; the undefeated re-seat (spec-0016 §1) re-summons the fight whole, the max is recaptured, and the bar reads full when the party walks back in | §3 + §4, nothing new |
| a bonfire rest with the fight still standing | the same re-seat; the bar reads full on return | §3 |
| `despawn-actor`, `campaign-complete`, a quest ending with the body gone | no live body ⇒ hidden | §3 |
| server restart, relog | the bar entry persists in `level.dat` (§6); the world-init function re-creates every declared bar from nothing, and the first tick re-derives value, max, audience and visibility, so no stale bar outlives one tick | §6 |

The set of bars the world holds is exactly the set the campaign declares: world init runs `bossbar remove` then `bossbar add` per declared bar (an `add` on an existing id fails — §6), so a bar from an earlier build of the same world cannot survive.

## 5. Title, colour, style

**Title (authored, the spec-0071 §3 reading rule).** `title` is optional. Absent, the bar is titled by the fight's own name: for an actor its `name`; for a wave the `name` of its **exactly one** mob entry. A wave with two entries or more, or a fight with no name and no `title`, is §8.2 — a bar with nothing to title it is refused, not defaulted, because there is no compiler-owned English that is right for a porter, a bell-ringer and a choir at once (the `lethal.<id>.message` rule). A stated `title` always wins.

A `title` enters the l10n inventory as `wave.<w>.health_bar.title` / `actor.<a>.health_bar.title` through `each_string`, exactly like every other player-visible string; a derived title emits the **name's own key** (`wave.<w>.mob.<i>.name`, `actor.<a>.name`), so the character is translated once and the bar cannot call it something else. The bar's name component is the i18n v2 `{"translate", "fallback"}` form the build already ships for every string.

**Colour and style (authored).** `color` is one of the seven literals the pinned tree lists under `bossbar set <id> color` (`blue green pink purple red white yellow`), `style` one of the five under `… style` (`progress notched_6 notched_10 notched_12 notched_20`). Both are **strings validated against the pinned command tree** (§8.3), not Rust enums: the tree is the one authority the emitter already holds every command to, and an enum would be a second copy of it. (The `FireworkShape` enum is not this case — a firework's shape is item-component data with no command-tree entry to defer to.) Absent ⇒ the engine emits no `set color` / `set style` line and the game's own defaults stand (**cited**: white, progress — §6); the engine chooses no look of its own.

## 6. The vanilla primitive

Every form below is present in `crates/delvec/data/commands-1.21.11.json` at the ground revision (read by walking the tree, not from memory), and the emitter's command check holds every emitted line to that tree, so a form drifting out of it is a build failure, not a runtime surprise:

- `bossbar add <id> <component>` · `bossbar remove <id>` · `bossbar set <id> color <lit>` · `… style <lit>` · `… max <int>` · `… value <int>` · `… players` (no target) · `… players <targets>` · `… visible <bool>` · `bossbar get <id> value|max|players|visible`
- `execute store result bossbar <id> value|max` · `execute store result score <holder> <objective>` · `data get entity <target> <path> <scale>` · `attribute <target> <attribute> get <scale>` · `execute as/at/if entity/if score`, `tag`

**Emission shape (authored; the implementer owns the exact lines).** One bar id `<ns>:hb_<safe local id>` per declaring fight. World init: `remove`, `add` with the title component, the declared colour/style if any, `max 0`, `players` (none). Every tick, one line per bar into a per-bar function `hb_<key>` guarded by `execute if entity @e[<fight selector>,nbt=!{Health:0.0f},limit=1]`, else `bossbar set … visible false`. The function: zero the value holder; `execute as @e[<fight selector>,nbt=!{Health:0.0f}] run function <ns>:hb_acc_<key>` (one body: `store result score` of `Health 1`, add into the holder — the `wave_census_one` pattern); `execute store result bossbar <id> value run scoreboard players get <holder> dw.sys`; `… max` from the captured holder; audience — clear `dw_hb_<key>` from every player, `execute as @e[<fight selector>,nbt=!{Health:0.0f}] at @s run tag @a[distance=..<range>,tag=!dw_cutscene] add dw_hb_<key>`, `bossbar set <id> players @a[tag=dw_hb_<key>]`, `visible true`. Max capture: `scoreboard players set <max holder> dw.sys 0` then the same accumulation over `attribute @s minecraft:max_health get 1`, appended to every function that summons the fight's bodies.

**Known vanilla behaviour, and how each is settled.** The Minecraft wiki (`minecraft.wiki/w/Commands/bossbar`, `…/Boss_bar`; CC BY-NC-SA 3.0, so a pointer only, never a source of record) states: a custom bar is *stored in level.dat CustomBossEvents*; `add` on an existing id fails; `players` with no target makes the bar invisible to everyone; colour defaults to white, max to 100, value to 0, style to progress; several bars show at once without overlapping. Each of these that the design leans on is **proven on the pinned server by the generated PackTests of criterion 3**, which is the source of record; the wiki is where to look for what to prove.

## 7. Research record — how the reference games present a boss

Sources are Fextralife wiki pages (`darksouls.wiki.fextralife.com/Ornstein+and+Smough`, `…/Four+Kings`, `eldenring.wiki.fextralife.com/Godskin+Duo`), under Fextralife's custom licence: **ideas only** (ADR-0013). Each says, in its own words:

- *Ornstein and Smough*: the two bosses keep separate health pools; *when one dies and absorbs the other the survivor's health is restored*; the arena is sealed by a fog gate that lifts when both are down. → **Adopted**: one bar per fight, two fights ⇒ two bars (§3); a second phase is a second wave.
- *Four Kings*: *the Four Kings share an overall boss health bar*; more than four may spawn; *as soon as the boss' main health bar reaches zero, the fight will end regardless of how many kings you've defeated*. → **Adopted**: a many-bodied wave is one bar over the sum (§3). The reinforcement-until-empty mechanic is not adopted; a wave's `kill` objective ends when its bodies do.
- *Godskin Duo*: *a single shared health bar that must be fully depleted*; *their collective bar is depleted as you deal damage to either boss*. → the same shape as Four Kings; one wave with two entries.
- Common to all three (and to every boss in the series): the bar carries the boss's **name**, is drawn as a bar with no numeral, and appears when the player crosses into the arena — not when the boss notices them. → **Adopted**: name-titled (§5), `progress` by default (§5), distance-gated per player (§4).

What the record does not answer, and this spec authors: the range at which "crossing into the arena" is decided (a creator judgement, per fight); whether an unbilled fight may carry a bar (yes — the bar is not the billing).

## 8. Refusals, and one advisory

1. **A bar on a body whose health cannot move** (`DW0909`): an actor with `health_bar`, `vulnerable: false`, that no `unleash-actor` names. `Invulnerable` + `NoAI` is scenery; a bar over it would never move. Prescription: unleash it somewhere, mark it `vulnerable`, or remove the bar. The spec-0034 rule: a declaration is a claim, and a claim nothing can exercise is refused.
2. **A bar with nothing to title it** (`DW0910`): `title` absent and the fight has no single name to draw (§5). Prescription: state `title`, or name the body. Blank `title` is the same code (`DW0512`'s precedent for a blank required string).
3. **A colour or style the pinned game does not draw** (`DW0911`): `color` or `style` not among the literals the pinned command tree lists. The tree is `delvec`'s data (`crates/delvec/data/`, not `crates/dsl/data/`), so this is compiler-side at **validation tier (exit 1)** — the `DW0343` precedent for a check that needs data the DSL crate does not carry. Prescription: the message lists the literals, read from the tree — never a copy.
4. **A `boss`-tier fight with no bar** (`DW0912`, **advisory**): a wave or actor declaring `tier: boss` and no `health_bar`. **Warning tier, never blocking** — the `DW0453` shape: the build proceeds, the line names the fight and prescribes `health_bar`. Fires for `boss` only; `elite` and `ordinary` are never named by it, whatever they declare. Its quantifier is every fight of either class, so an actor billed `boss` is advised exactly as a wave is.
5. `range` outside 4..=64: `DW0100`, the exported schema's stated bound restated at the document tier (the `firework` precedent).

Every code owes a test (`tools/ci/check-dw-codes.py` holds the reference and the code set bidirectionally). The three refusals are raised at `delvec validate`, before any build, where the field is entered; the advisory is raised at the same point and stops nothing.

## 9. Declined, with the reason

- **A bar from `tier`** — §2; would make the billing a knob.
- **A warning for an `elite` with no bar** — the un-telegraphed first-kill ruling (spec-0016) makes the unannounced elite legitimate; only the `boss` tier is advised (§8.4).
- **A `count` bar (bodies left of N)** — a `notched_<n>` style over the summed health already reads as segments; a second value mode is a second field for one meaning.
- **A `follow_range` default for `range`** — §4; two different facts.
- **The mineflayer tier reading the bar** — the PackTest proves the bar from the server side (`bossbar get`), which is where the value is; a client-side check adds no fact the server does not already state. Left to a later playtest-methodology change if a bot-witnessed HUD is ever wanted.
- **A number on the bar** — none of the reference games draw one; a `title` may carry text but the engine writes no `{"score"}` into it.

## Acceptance criteria

Each criterion is checked against the tree at `509ebc58`: the surface it names exists (or is named as owed by the implementation), and none is satisfied today.

1. **Surface.** `delvec schema --stage all` exports `health_bar` on `waves[]` and `actors[]` with the four properties of §2 and no enum variants for `color`/`style` (§5); `dsl_version` is `0.31.0`. A campaign that declares no bar builds byte-identical to the previous engine (`gallery/baseline/` manifests for the primary and every overlay differ only where a bar was added, and `delta.json` attributes every row).
2. **Emission tests** (`crates/delvec/tests/emit.rs`, alongside the wave and actor emission tests there): a fixture with one bar on a `count: 1` wave emits the init lines, the tick line, `hb_<key>` and `hb_acc_<key>`, and the max capture inside `spawn_<wave>`; a bar on an unleashed actor emits the `tag=!dw_pup_<id>` narrowing and the capture in `spawn_actor_<id>`, `unleash_<id>` and `actor_restand_<id>`; a `vulnerable` never-unleashed actor emits no narrowing; a two-fight fixture emits two ids; a call-graph test (`crates/delvec/tests/call_graph_integrity.rs`) asserts every function that summons a bar-carrying body calls its capture. Perturbation: change `range` and the audience selector's `distance` moves; remove `color` and no `set color` line is emitted; build twice, byte-equal (ADR-0006).
3. **Generated PackTests** on the pinned server, in the family `emit_reseat_undefeated_packtests` belongs to, one per shape the campaign declares (wave, actor), driven with the real generated functions and a PackTest fake player (`pin_dummy`; a fake player is immune to `/damage`, so the fight's health is moved with `data modify … Health`, the pattern `souls_reseat_undefeated` already uses):
   - spawn the fight, stand the dummy inside `range`, run `hb_<key>`: `bossbar get <id> players` is 1, `visible` is true, `value` equals `max` and `max` equals the fixture's summed `max_health`;
   - set one body's `Health` to 1, run the function: `value` moved by exactly the difference;
   - teleport the dummy out of `range`, run: `players` is 0;
   - kill the last body, run: `visible` is false;
   - in a campaign with a bonfire: chip the fight, run the real `bonfire_rest_<i>`, stand the dummy in range, run: `value` equals `max` again;
   - two declared fights: both ids exist after `setup` and `bossbar list` returns 2 — the several-bars-at-once behaviour proven where it is used.
   The gallery's generated PackTests run in the required `tier 2` job (spec-0039 §8.11), so these run on every PR.
4. **Diagnostics.** Each of the three refusals (`DW0909`, `DW0910`, `DW0911`) has a red fixture in `crates/dsl/tests/` naming the literal it refuses. `DW0912` has a fixture on which it fires — a `boss`-tier wave with no `health_bar`, and a `boss`-tier actor with none — with the build still exiting 0, and a fixture on which it is silent that carries both a `boss` fight **with** a bar and an `elite` fight **without** one: the perturbation only this rule can tell apart. `tools/ci/check-dw-codes.py` is green and `docs/reference/compiler.md` carries all four rows. `gallery/baseline/warnings.json` gains no `DW0912` row, because every `boss` fight in the gallery carries a bar (criterion 5).
5. **Gallery** (spec-0039). `gallery/quests.json` binds `health_bar` on a one-entry named wave with no `title` (the derived title — `wave/lane`), on a two-entry wave with `title` and `style` (`wave/muster`), and on the unleashed boss actor with `color` (`actor/hall-moth`); `gallery/l10n/zh-cn.json` follows with the new title key. Three probes under `gallery/probes/`, each the primary plus one declared edit: a bar on `actor/hall-usher` (§8.1), `wave/muster`'s bar with `title` removed (§8.2), a bar with `color: crimson` (§8.3). `check-gallery-coverage.py` reports 0 units in neither state; the bar's stated binding count (fights carrying a bar, over fights declared) is printed and non-zero.
6. **Docs, same PR.** `docs/reference/compiler.md`: the two field rows beside `waves[].tier` / `actors[].tier`; an emission row for the bar (init, tick, capture sites, audience); the four DW rows; the l10n keys in the key inventory; the generated PackTest names. `docs/reference/i18n.md`: the derived title shares the name's key. `docs/demo-levels.md` gains the row **The Bell-Ringer's Landing (0073)** — *a short climb to one elite at a landing and one boss with two adds beyond a sealed door: the bar appears at the threshold, drops as the choir falls, vanishes on the kill, and reads full again after a rest* — status pending.
7. **The creator's page.** The `/new-delve` skill's quest-stage guidance (`.claude/skills/delvewright/skills/new-delve/references/quest-capabilities.md`, beside where `pitfalls.md` tells a creator to bill a fight `tier: "elite"`) names `health_bar` as the way a fight is shown, with the derived-title rule and the boss-with-adds spelling, in the same PR; `tools/ci/check-skill-page.py` green.
8. **The page says what the bar is, in the engine's terms.** The same guidance states, plainly and in this order: a health bar is **optional** on any wave or actor; declaring one draws a named bar over the fight's total health for every player within `range` of it, from the moment they enter that range until the last body falls; the compiler **warns** (`DW0912`, never a failure) only when a fight billed `tier: boss` declares none, and says nothing about any other tier. The wording is for a general engine: it names fights, bodies, ranges and tiers, and no genre or reference game — a reader of that section cannot tell what kind of game the creator is making. Checked in the implementing PR by reading the section against these three statements.
