# Click attribution — which presses on a body the pinned game names the player for

For whoever designs a rule that answers a press on a body — a blow or a right-click — to the player who made it. What the pinned Minecraft 1.21.11 server records about each press on each body shape the compiler stages, measured on the running game. Each line is **measured** (by the instrument named below, at the reading recorded here), **cited** (a line of the tree or a published source) or **authored** (what this engine takes from it). Vanilla behaviour is established once and treated as constant (CLAUDE.md); this page is where it was established.

## 1. The question

The engine answers a press through an advancement criterion rewarded with a function that runs as the acting player (`crates/dsl/src/trigger.rs`, `TriggerAudience::Presser`: "the one vanilla primitive that runs a function *as the player who clicked*"). Whether a press can be answered to its presser is therefore whether a criterion fires for it. The two bodies the compiler stages — the `NoAI`, `Invulnerable` puppet an NPC or an actor ships as, and the `minecraft:interaction` hitbox it summons beside one (`crates/delvec/src/compiler/emit/npc.rs`, `{width:1.0f,height:2.0f,response:1b,Invulnerable:1b}`) — are each asked, for a left-click and a right-click, in the game mode a delve ships (adventure).

## 2. The instrument

`tools/maintenance/probe-click-attribution.py` with `probe-click-attribution.mjs`: boots the pinned server jar (refused unless its sha256 is the `versions.toml` pin), loads a datapack of one advancement per `(criterion, body tag)` whose reward counts on `@s` and revokes itself, stages the bodies on a stone floor, and drives a mineflayer bot (the harness's pinned version) that attacks or right-clicks each body by entity. Two readings per act, sharing no configuration: the scores the criteria wrote, and the `attack` / `interaction` record vanilla writes on the interaction entity, whose `player` is compared with the bot's own UUID. A positive control — a zombie puppet that is **not** invulnerable — proves the bot's blow reaches the server; a run in which it does not fire is reported as having measured nothing. The committed table is `EXPECT` in the driver; the tool exits 1 when the game disagrees with it.

## 3. What the pinned game does

Measured, seven acts, two runs, zero disagreements, bot in adventure mode, standing two blocks from each body:

| Body, press | `player_hurt_entity` | `player_interacted_with_entity` | Record on the entity |
|---|---|---|---|
| `minecraft:zombie` `{NoAI:1b,Invulnerable:1b}`, left-click | does not fire | — | none |
| `minecraft:zombie` `{NoAI:1b,Invulnerable:1b}`, right-click | — | does not fire | none |
| `minecraft:interaction` `{response:1b,Invulnerable:1b}`, left-click | **fires, as the attacking player** | — | `attack.player` = the bot's UUID |
| `minecraft:interaction` `{response:1b,Invulnerable:1b}`, right-click | — | **fires, as the clicking player** | `interaction.player` = the bot's UUID |
| `minecraft:zombie` `{NoAI:1b}` (not invulnerable; the control), left-click | fires | — | — |
| the puppet and the interaction on one cell, the interaction left-clicked | fires for the interaction; not for the puppet | — | `attack.player` = the bot's UUID |
| the puppet and the interaction on one cell, the interaction right-clicked | — | fires for the interaction; not for the puppet | `interaction.player` = the bot's UUID |

- **Measured.** A left-click on a `minecraft:interaction` entity fires `minecraft:player_hurt_entity` with the interaction as `this`, granted to the attacking player, whether or not the entity is `Invulnerable`; the reward function runs as that player. The entity's `attack` record names the same player by UUID.
- **Measured.** A left-click on an `Invulnerable` living body fires no criterion and leaves no record. A right-click on a living body whose kind has no use interaction (a zombie) fires no criterion.
- **Measured.** A right-click on a `minecraft:interaction` fires `minecraft:player_interacted_with_entity` as the clicking player — the dispatch every NPC dialogue, `interact` objective and press answer already runs on (`docs/reference/compiler.md`, the press-answer section).
- **Not read.** The jar's bytecode was not read for this page; the finding rests on the measurement alone. The sites a reader of the jar would look at are the interaction entity's attack handling and the player's attack path, and reading them is the second method a pin bump owes if the measurement is ever in doubt.

## 4. What the tree held before this page, and what follows

- **Cited** (`crates/dsl/src/trigger.rs`, `DW0427` and `EnvTrigger::attributes_its_actor`; `docs/reference/compiler.md`, the press-answer section): "Vanilla offers no such criterion for a left-click"; a left-click "is recorded in the interaction entity's `attack` NBT — a UUID no command can become", so `audience: presser` on a `strike`, `strike-npc` or `strike-assembly` trigger is refused. **Authored:** the first half is false against the pinned game for the one carrier the compiler uses — every `strike`-class trigger is dispatched from a `minecraft:interaction` (`compiler::pressable`, `emit/trigger.rs` `trigger_carrier_tag`), and `player_hurt_entity` on that entity names the striker as the right-click criterion names the presser. The second half stays true and does not matter: the UUID record is the second reading, not the dispatch.
- **Authored — what follows.** A blow on an interaction-carried body can be answered to the player who struck it by the same shape every press answer uses (an advancement rewarded with a function run as `@s`), with `minecraft:player_hurt_entity` in place of `minecraft:player_interacted_with_entity`. `DW0427`'s quantifier shrinks to the acts that have no carrier entity at all (`approach`); `strike`-class triggers join `use` and `step` in `attributes_its_actor`. The change is made by the specification that first needs it and lands with its implementation; until then the refusal stands as written and this page is the record it will be corrected from.
- **Authored — what does not follow.** The living body itself still names nobody: a blow on an `Invulnerable` puppet is silent to every criterion, so a press on a body is attributable only where the compiler stands an interaction entity for it. Which bodies get one is a design decision made where the body is declared, never inferred here.

## 5. Sources

- `tools/maintenance/probe-click-attribution.py`, `tools/maintenance/probe-click-attribution.mjs` (this repository), run against the server jar whose sha256 is the `versions.toml` `[minecraft]` pin, with mineflayer at the version `harness/package.json` pins.
- `crates/dsl/src/trigger.rs` (`TriggerAudience`, `DW0427`); `crates/delvec/src/compiler/emit/npc.rs` (the hitbox summon); `crates/delvec/src/compiler/emit/trigger.rs` (`trigger_carrier_tag`, `trigger_record`); `docs/reference/compiler.md`, the press-answer section.
