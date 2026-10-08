# `delvewright_dsl::diagnostic`

The reference page for `crates/dsl/src/diagnostic.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW01xx — validation (`dsl`; severity error; exit 1)

| Code | Meaning |
|------|---------|
| `DW0100` | Document does not conform to its stage schema (unknown field / wrong type / missing required field, incl. persona). Parse-time. |
| `DW0101` | `stage` field ≠ document slot. |
| `DW0102` | The document's `dsl_version` is not the one this engine accepts, `0.36.0`; the message names it (ADR-0024). Raised per stage document by `dsl::validate::envelope`, and for an l10n sidecar under `DW0180`. |
| `DW0103` | `campaign_id` differs across stages. |
| `DW0110` | Malformed id syntax (not kebab-case / wrong-missing prefix). **The message names the form of the type it rejected**, derived from that id type's own `PREFIX` — `` `dlg/<kebab>` `` for a dialogue node, `` `class/<kebab>` `` for a class — rather than restating the general rule beside three fixed examples. One macro in `dsl::validate::syntax` is the single path every id type's syntax refusal goes through, so the answer comes from the type at every site: `ids::syntax_form`. The per-section refusals that spell their own prefix by hand (`wave/`, `trigger/`, `trap/`, `shortcut/`, `ambush/`, `timed-gate/`, `loot/`) are the same fact copied, which is why the general path did not have it. |
| `DW0111` | Duplicate id in namespace (incl. two dialogue trees for one NPC). |
| `DW0112` | Dangling / forward / undeclared reference (incl. persona relationship to unknown NPC). An **area** reference resolves against the campaign's placement authority and its prescription comes from there (see the remediation contract above): a `Prefabs` campaign is told to declare it in stage-1 `world.areas`, a `SitePlan` campaign is told its one area is `area/site` and explicitly told NOT to declare it, and a campaign with neither is told both branches. Three sets in `dsl::validate` resolve an area id — npc/quest-plan references, and a stage-7 edit script's `batches[].area` — and all three prescribe from the same authority. |
| `DW0130` | Quest `depends_on` cycle. |
| `DW0131` | `finale` is not a declared quest. |
| `DW0132` | `finale` is not the convergent sink (some quest is not a transitive dependency of finale). |
| `DW0140` | Objective `after` cycle. |
| `DW0142` | Anchor not provided by the area's bound prefab — or, on a site-plan campaign, not among the names the derivation places. The predicate is `AnchorProviders`; the prescription is the placement authority's (see the remediation contract above), so a derived map is given the synthesized vocabulary instead of being sent to prefab metadata it does not have and told not to invent a name it is required to invent. **Silent while the derived vocabulary is unknowable** — a site-plan campaign with no `layout-graph.json`, where `DW0824` is the finding: `AnchorProviders` contributes no set for `area/site` and the whole campaign defers, the same path a `prefab_pool` takes. |
| `DW0143` | Item id not in the pinned 1.21.11 registry (kit / `collect` / `interact.requires_item` / `give-item`). |
| `DW0150` | Planned quest (stage 4) has no stage-5 expansion. **Two readings, one code, and the discriminator is whether stage 5 declares any quests at all.** Where it declares some and this id is not among them, the refusal is per plan entry and names its two ordinary remedies — write the expansion, or drop the entry — plus how many quests stage 5 does declare, which is what says *mismatch* rather than *unwritten*. Where it declares **none**, the campaign is between the stage-4 plan and the stage-5 quests, every planned quest is unexpanded by construction, and the two remedies are both wrong: writing the expansions IS the next authoring step, and the plan is not a mistake to delete. That case is **one** diagnostic on `/content/quests`, on the model `DW0874` sets — it names every planned quest, says the state is an authoring state rather than a fault, says why the refusal still stands (a plan entry with no expansion has no trigger, no objective and no completion, so nothing of it is emitted), and says there is **no cheaper way out**: the schema-minimal stage-5 quest is refused again by `DW0481` once per quest and `DW0460` once per NPC live in it, so writing empty expansions raises the count instead of lowering it. That last sentence is a measurement and `crates/delvec/tests/plan_awaiting_expansion.rs` takes it; the wording is `crates/dsl/tests/dw0150_plan_awaiting_expansion.rs`. Severity, code and exit are identical in both readings — a plan awaiting expansion cannot build, and a warning would let an unbuildable campaign read as buildable at the step where the difference decides whether anyone writes stage 5. |
| `DW0151` | Stage-5 quest not planned in stage 4. |
| `DW0152` | Stage-2 NPC has no stage-6 tree. |
| `DW0153` | Stage-6 tree references an NPC not in stage 2. |
| `DW0162` | Stage-7 edit script structurally invalid (spec-0017): an edit names a region no earlier `select` in its batch defined (region refs are strictly backward within a batch), a `union`/`intersect` lists < 2 regions / a `subtract` removes nothing, a box `min` > `max` on an axis, a surface band `from` > `to`, a palette recipe is empty or carries a non-positive/non-finite `weight`/`scale`, a `matching` list is empty, or a morph `by`/`passes` is 0. (Unknown recipe/matching block ids reuse `DW0193`; id syntax `DW0110`; duplicate batch/region names `DW0111`.) |
| `DW0170` | `kill`/`spawn-wave` references an undeclared `wave/<id>`. |
| `DW0171` | A killed wave is never spawned by any `spawn-wave`. |
| `DW0172` | `requires_flags` references a flag no `set-flag` produces. The producer scan descends every nested effect list (`sequence` steps, `on_respawn`/`on_caught`/`on_arrive`), so a `set-flag` nested in a timeline still counts as a producer (no spurious fire). |
| `DW0173` | Wave-mob `entity` is not a known vanilla entity id. |
| `DW0180` | l10n sidecar absent / inconsistent envelope / under-covers inventory (also if `en` is declared). Compiler-level. The inventory it demands coverage of spans **every effect root emission can lower** — including `traps[].payload` and a dialogue option's `set-checkpoint` `on_respawn` bundle, so a string in either is translated. |
| `DW0181` | l10n sidecar has an orphan key (over-coverage). Compiler-level. |
| `DW0182` | A player-visible string — authored English (the whole l10n inventory) or any sidecar translation — contains the reserved completion-marker sigil `[dw:complete`. That chat sequence is the validation bot's completion oracle (§4 "The completion-marker channel"); content carrying it could forge a passing critical-path step, so the sigil is **reserved**, not merely discouraged. Reword the line. |
| `DW0183` | (i18n v2, spec-0029) A player-visible string — authored or translated — contains a character from the reserved private-use block `U+E000..U+F8FF`. That block is how the compiler carries an l10n key from the stage docs to the text component the string is emitted into (`dsl::l10n::TR_SIGIL`), so content carrying it could impersonate a translation tag; it also has no glyph in any Minecraft font. Remove the character. |
| `DW0184` | (i18n v2, spec-0029) A declared `world.languages` code does not resolve to a language file the **pinned client actually loads** (`dsl::mclang::CLIENT_LANGS`, derived from Mojang's 1.21.11 asset index), so its `assets/delvewright/lang/<code>.json` would sit under a filename no client ever asks for and the language would ship invisible. Also fires on an ambiguous bare code (`zh`, `sr`, `be` — several regions, no `<lang>_<lang>`), because guessing the region is how a language ships invisible. Use a code the client loads. A language is never silently dropped. |
| `DW0190` | An image id a campaign declares is malformed or duplicated: a body's `skin.texture_id`, or a `world.textures[]` row's `id` (spec-0084 §6.4). |
| `DW0192` | Wave-mob `effects[].effect` not a known 1.21.11 status-effect id. |
| `DW0193` | `set-block`/`interact.prop` block id not a known 1.21.11 block id (base id checked; a malformed blockstate suffix `id[…]` — unbalanced `[]`, empty, or non-`key=value` tokens — reuses this code). |
| `DW0194` | Environment-trigger id malformed/duplicated, or `approach` `range` 0. |
| `DW0196` | Area `lighting.min_light` out of range (must be 1..=14). spec-0010. |
| `DW0199` | A `cutscene` effect's shape is invalid: it mixes the multi-shot `shots` list with the single-shot `path`/`seconds` fields, declares neither, omits `seconds` on a single shot, or gives a shot with an empty camera `path`. The two spellings normalize to one shot list, so this is where the shape is policed and emission may then assume a well-formed, non-empty list. |
| `DW0340` | Trap declaration structurally invalid (spec-0011): a malformed/duplicate `trap/<id>`, an `at`/`disarm.via` that no area's prefab provides, or a `disarm.via` that collides with the trap's own trigger anchor. The `at` remedy names what a trap actually needs of a piece — **one point anchor, under any name** — and says that a spec-0022 command `payload` needs nothing beyond that cell, because the compiler emits the detection; the `dispenser` socket belongs to a legacy `dispense` effect and the `trigger_block` to a flag-gated trap (`DW0363`). Sending an author to carve trap hardware for a payload trap sends them to build something no build reads. |
| `DW0341` | A trap `dispense` payload item id is not in the pinned 1.21.11 registry (spec-0011; mirrors `DW0143`). |
| `DW0866` | **An optional quest inside the finale's dependency closure** (spec-0051 §8.1), including a `finale` that declares itself `mandatory: false`. Validation tier (exit 1), `dsl::validate`. The delve cannot be completed without the quest, so calling it optional is a claim the completability proof would then rest on — and the skip world, in which no optional objective is ever completed, is exactly the world where the finale never fires. The closure is asked of `QuestPlanContent::spine`, the ONE authority on it; the declaration is asked of `QuestPlanContent::optional`, the ONE authority on the other half. **Its mirror image is `DW0132`**, which is the convergence refusal for a MANDATORY quest the closure does not reach: the two are opposite errors and a shared message could prescribe neither. **Co-fires with `DW0867` whenever a mandatory quest's `depends_on` names an optional one**, necessarily — such a dependency is inside the closure by construction — and the two are kept apart because they prescribe different repairs: `DW0867` names the edge to cut, this one names the claim to withdraw. Prescription: set `mandatory: true`, or cut the `depends_on` chain that puts it in the closure. |
| `DW0867` | **A mandatory quest whose `depends_on` edge or stage-5 `quest-complete` trigger names an optional quest** (spec-0051 §8.2). Validation tier (exit 1), `dsl::validate`. The party may never play elective content, so a mainline beat waiting on it stops the delve in the skip world. **Refused at the edge, naming the edge**, which is where an author can act. One rule over **two** edge kinds, so one code: `depends_on` orders the plan, and the stage-5 trigger is what actually arms the quest at runtime — nothing ties the two together (a `quest-complete` trigger resolves against the stage-5 quest set, never against stage 4), so a campaign can spell this edge with either alone. The trigger arm is the one that reaches this code by itself; see `DW0866` for why the `depends_on` arm always co-fires. **The reverse directions are both legal and deliberately unreported**: an optional quest may `depends_on` a mandatory one (that is a strand's attachment to the spine) and may be triggered by a mandatory completion (a skipped quest still activates — §5). Prescription: mark the named quest mandatory, or move the edge onto the spine. |
| `DW0868` | **A mainline key behind participation** (spec-0051 §8.3): a mandatory quest's objective whose `requires_flags` names a flag every producer of which is rooted in an optional quest. Validation tier (exit 1), `dsl::validate`. A party that plays only the mainline can never open that beat, so the delve is not completable with zero optional participation. **The producer partition is conservative in the safe direction**: one producer anywhere else — a mandatory quest, an environment trigger, a trap disarm, a dialogue option, `on_death` — takes the flag out of the set. Dialogue is counted as non-optional deliberately, because whether an option is reachable only inside an optional quest's scene is a cast-ladder question this rule cannot answer and answering it wrongly would refuse a correct campaign. **`DW0204` is the compensating stronger check behind it** — the participation-minimal replay credits only the exported path's own producers, so this shape fails there too; what the edge buys is a message that names the strand instead of a walk that stops. **The `requires_state` and `dropped_by` chains of §8.3 are NOT covered here** and reach only `DW0204`. A flag nothing produces at all is `DW0172`, not this. Prescription: move the `set-flag` onto a mandatory quest, mark the producing quest mandatory, or drop the gate. |
| `DW0348` | A `shot_style` declaration is semantically invalid (spec-0015): a styled shot with no `subject`; style params (`subject`/`subject_b`/`dist`/`degrees`/`bearing`) on an unstyled shot; `subject_b` off `two-shot` (or a `two-shot` without one); `degrees` off `orbit-arc` or outside `45..=120`; `dist` outside `1..=48`; `bearing` outside `-360..=360`. Validation-tier (exit 1), `dsl::validate`. |
| `DW0349` | A `side-track`/`low-follow` shot whose subject provably cannot move: those styles dolly *with* a moving subject, so the subject must be an npc/actor with a matching `move-npc`/`move-actor` in the same effect group or the same `sequence` timeline (an `anchor` subject can never move; reaction lists `on_arrive`/`on_caught`/`on_respawn` start a fresh scope — their firing time is statically unknowable). Validation-tier (exit 1), `dsl::validate`. Prescription: add the move alongside the cutscene, or use a static style (`locked-off`, `push-in`). |
| `DW0357` | A `carrier: "one"` `give-item` sits in a nested bundle with no acting player — a `move-npc`/`move-actor` `on_arrive`, a `bonfire`'s `on_rest`, or a `sequence` step of a timeline started where nobody acted (a polled trigger, a trap, a shortcut) (spec-0018; the seams are `QuestEffect::nested_effect_dispatch`, spec-0085). Those run with the server command source, so the single prop would reach nobody. A `sequence` step under a root that has an actor keeps it (the timeline carries its actor), and `set-checkpoint.on_respawn` / `begin-stealth.on_caught` are dispatched per player and do have an `@s`. A root's own top level is not refused: a polled root lowers the give to the party. Validation-tier (exit 1), `dsl::validate`. Prescription: drop `carrier` (arm the whole party), or move the hand-off onto the beat a player completes. |
| `DW0350` | A `use` trigger anchored where an NPC stands. Right-click on an NPC already belongs to its dialogue advancement; a second interaction hitbox in the same cell makes the client's entity ray-pick ambiguous, and whichever entity loses the tie is silently dead — the soft-lock class that starved the giant's dialogue of every right-click. Left-click triggers are exempt (a left-click has no dialogue meaning): they ride the NPC's own hitbox instead of summoning a second one. Validation-tier (exit 1), `dsl::validate`. Prescription: move the trigger to its own anchor, express the interaction as a dialogue option, or — if the NPC's body is genuinely the target — use `on: strike-npc`, which takes no anchor at all. |
| `DW0377` | A `timed-gate` declaration (spec-0016 §4) is structurally invalid: a malformed or duplicate `timed-gate/<id>`, an `open_ticks` or `closed_ticks` of 0 (a gate that never opens, or never closes — that is `open-gate`/`close-gate`, not a clock), a `phase` at or beyond the full cycle, two timed gates driving one region (two clocks race every tick and the region's state becomes emission order, not design), a gate a `shortcut` already owns (a clock would re-seal what `DW0372` exists to forbid re-sealing), or a `disarm.via` anchor no area's prefab provides / one that IS the gate anchor (the jam lever cannot stand inside the span the portcullis closes on). Validation-tier (exit 1), `dsl::validate`. |
| `DW0375` | An `ambush` declaration (spec-0016 §3) is structurally invalid: a malformed or duplicate `ambush/<id>`, an empty `actors` list (an ambush that springs nothing), or the same actor listed twice — `spawn-actor` is idempotent, so the second one is a silent no-op and the ambush is half the size it reads as. Validation-tier (exit 1), `dsl::validate`. Deliberately does **not** require a `telegraph`: the un-telegraphed ambush is core souls vocabulary. Everything else about an ambush is checked as the trigger it desugars to (`DW0194`, the anchor seals, `DW0350`). |
| `DW0371` | A `shortcut` declaration (spec-0016 §2) does not resolve: a malformed or duplicate `shortcut/<id>`, a `gate`/`unlock` anchor no area's prefab provides, or an `unlock` equal to its own `gate` — the mechanism belongs on the far side of the door it opens, which is the entire point of the pattern. Validation-tier (exit 1), `dsl::validate`; anchor resolution stays lenient for pool areas the compiler resolves later, and for a site-plan campaign whose `layout-graph.json` is absent (`DW0824`), where the derived vocabulary is unknowable rather than empty. The id and self-gate arms are unaffected by either — they judge what the declaration says. |
| `DW0372` | A `close-gate` effect targets a gate a `shortcut` owns (spec-0016 §2). A shortcut opens **permanently** — that is the pattern — so permanence is made structural rather than left to authoring discipline: there is simply no way to spell the re-seal. The scan descends nested effect lists, so a `close-gate` buried in a `sequence` step is caught. `close-gate` on any other gate (the point-of-no-return staging beat) is untouched. Validation-tier (exit 1), `dsl::validate`. |
| `DW0389` | A `close-gate` effect targets the gate of a `timed-gate` that declares a `disarm` (`docs/notes/souls-design-language.md` §5.2). A disarm suppresses the clock **permanently with the gate resting OPEN** — a jammed portcullis stays up — so, exactly as for a `shortcut` (`DW0372`), permanence is structural rather than left to authoring discipline: there is no way to spell the re-arm. The scan descends nested effect lists, so a `close-gate` buried in a `sequence` step is caught. A `close-gate` on a timed gate with **no** `disarm` is untouched — that clock is still a clock and the point-of-no-return beat may seal it. Validation-tier (exit 1), `dsl::validate`. |
| `DW0381` | A wave's TD `lane` / `summon` declaration (spec-0016 §6) is structurally invalid or internally contradictory: an empty `waypoints` list, a waypoint anchor no area's prefab provides, a repeated consecutive waypoint (the squad would be sent where it already stands, and vanilla re-rolls a patrol target on arrival), an `aggro_radius` outside `4..=64`, a mob whose `attributes.follow_range` disagrees with `aggro_radius`, or `lane` together with `summon: aggro-edge`. The `follow_range` clause is the subtle one: release radius and perception radius must be the same number, because a patrolling raider that targets a player it cannot engage HOLDS GROUND instead of marching — the squad stalls mid-lane with every other proof green. Validation-tier (exit 1), `dsl::validate`; anchor resolution stays lenient for pool areas the compiler resolves later. |
| `DW0382` | A lane wave fields a non-raider species (spec-0016 §6). `Patrolling`/`patrol_target` are Raider NBT: on any other mob they are simply dropped and it stands where it spawned — the silent no-op class. **The lane roster is Mojang's, never ours**: it is vanilla's own `#minecraft:raiders` tag, read from the vendored entity-type tag table (`crates/dsl/data/entity-tags-1.21.11.json`, regenerate with `tools/maintenance/extract-entity-tags.py`), the same rule `DW0496` follows for `#minecraft:burn_in_daylight`. For 1.21.11 it holds evoker, illusioner, pillager, ravager, vindicator and witch. Three independent readings of the pinned server jar agree on that six: the tag itself; the entity types whose constructed class is a `PatrollingMonster`; and the entity types whose class is a `Raider`. The three NBT keys are string constants of exactly one class an entity is built from, `PatrollingMonster`, whose own `registerGoals` adds the `LongDistancePatrolGoal` every subclass inherits — so honouring the NBT and having the goal are the same membership question. `tools/maintenance/check-patrol-types.py` re-derives all of it from the pinned jar and refuses on any disagreement. Validation-tier (exit 1). Prescription: use `summon: aggro-edge`, which needs no patrol AI, for everything else. |
| `DW0383` | A lane wave fields fewer than 2 mobs (spec-0016 §6). A lone patroller sets `Patrolling:0b` on ITSELF when it finds no companion within its follow range (vanilla, live-verified), so a one-mob lane cancels its own routing. Validation-tier (exit 1). |
| `DW0384` | A lane `pillager` is not holding a crossbow (spec-0016 §6). Its only attack goal is the crossbow goal, so on acquiring a target it has nothing runnable to do — while the patrol goal is meanwhile blocked BY that target — and it freezes in place indefinitely (live-verified deadlock). The compiler arms pillagers by default, so this fires only on an explicit `equipment.main_hand` override, which is exactly the remaining way into the deadlock. Validation-tier (exit 1). |
| `DW0385` | A `summon: aggro-edge` wave mob declares no `attributes.follow_range` (spec-0016 §6). That radius IS the summon ring — the distance at which the mob perceives the party — so it is authored, never guessed: the compiler will not fabricate a vanilla default it cannot verify against the pinned server. Validation-tier (exit 1). |
| `DW0370` | A wave declares `respawns_on_rest: true` but the campaign declares **no** `bonfire` (spec-0016 §1) — nothing can ever fire the re-seat, so the field is a silent no-op, the defect class this compiler always makes loud. Validation-tier (exit 1), `dsl::validate`; the scan descends every nested effect list (a `bonfire` inside a `sequence` step counts) over quests and triggers. Prescription: add the bonfire the re-seat hangs off, or drop the field — never leave a dead declaration in the DSL. |
| `DW0499` | A wave declares **both** `tier: boss` and `respawns_on_rest: true` (spec-0016 §1, spec-0023; stage bosses never respawn on rest). `tier` and `respawns_on_rest` are two fields on the SAME wave declaration — the only place a "boss" billing and a "re-seat on rest" contract can land on one another: an actor carries `tier` too (spec-0023's "other shape an elite takes"), but has no `respawns_on_rest` field at all — an actor is killed by hand, never by a `kill` objective, and the bonfire re-seat machinery only ever re-summons **waves** — so an actor-shaped boss is structurally incapable of expressing this violation, and the check is scoped to the one shape that can. A rest-respawning boss re-fight breaks the retry economy that rule protects: a boss is the campaign's named fight, not trash pressure the party grinds back down every rest. Validation-tier (exit 1), `dsl::validate`; checked unconditionally of whether a `bonfire` exists — the combination is forbidden on its own terms, not merely inert like `DW0370`. Prescription: drop `respawns_on_rest` if the encounter really is the boss, or drop `tier: boss` (bill it `elite` instead) if it is meant to re-seat. |

### DW03xx — build / solver / nav (`compiler`; error; exit 3, `stage:"build"`)

This module's rows of a section whose prose is on the [`delvec::compiler::nav` page](../delvec/compiler/nav.md#dw03xx--build--solver--nav-compiler-error-exit-3-stagebuild).

| Code | Meaning |
|------|---------|
| `DW0427` | **A press answer addressed to a click vanilla cannot attribute**. A trigger declares `audience: presser` on something other than an `on: use` or an `on: step` (`EnvTrigger::attributes_its_actor`). `minecraft:player_interacted_with_entity` is the only vanilla criterion that runs a function as the player who clicked, and it fires on right-clicks alone; a step is a player standing in the cell, which the poll's own selector names; a left-click is recorded in the interaction entity's `attack` NBT as a UUID no command can become, and an `approach` is not attributed. Approximating it — polling the record and assuming the nearest player is the striker — is exactly the downstream folklore CLAUDE.md's no-hack rule excludes, so the capability is refused rather than faked. `dsl::validate::press_answer_checks`, validation tier (exit 1). Prescription: make it an `on: use` or `on: step` trigger, or drop `audience` and let the beat address the party. |
| `DW0428` | **An authored trigger id in the compiler's reserved `dw-` namespace**. The compiler synthesizes triggers of its own — the press answer every sealed gate and shortcut door gives (`trigger/dw-press-seal-<anchor>`, `trigger/dw-press-door-<shortcut>`) — and two triggers sharing an id would share one `dw_trig_…` tag and one emitted function, so one of them would silently disappear. Reserving the prefix makes the collision impossible by construction rather than improbable. `dsl::validate::press_answer_checks`, validation tier (exit 1). Prescription: rename it; any kebab id not opening with `dw-` is the campaign's. |
| `DW0429` | **A sealed body the campaign never answers**. A `shortcuts[]` door bars a gate from world-load, or a `close-gate` seals a wall, and nothing says what it answers when the party presses it — no `use` trigger anchored on it, and for a `close-gate` no authored `sealed_hint`. A player who walks the long way round, arrives at the wrong side of a door and pushes on it is told nothing; that is the press a shortcut loop most invites, and a sealed wall is the same defect one verb over. **One rule for both**, because two objects of one class with two defaulting policies is exactly the "capability keyed to the verb" defect this surface is CLAUDE.md's worked example of. The compiler had every ingredient to invent a line here and deliberately does not: a baked default decides the door's tone on the author's behalf and never discloses that it did, while an error makes the author say it (the no-hacks rule at a new site). It binds at every `dsl_version` the engine accepts (ADR-0024: there is one). Discharged by ANY `use` trigger on the body — `QuestsContent::answers_press_at`, the same predicate the synthesis reads — or, for a `close-gate`, by an authored `sealed_hint`; not by a `strike`, which is a different gesture. `dsl::validate::press_obligation_checks`, validation tier (exit 1). Prescription: the message carries the trigger JSON verbatim, and a test parses that prescription and asserts it clears the diagnostic, so it cannot come to name a field the schema does not have. |
| `DW0329` | A `sequence` effect is nested inside another `sequence` (directly, or reachable via a nested `move-actor` `on_arrive`) — timelines do not recurse (spec-0014). Validation-tier (exit 1), `dsl::validate`. Flatten the inner steps into the outer timeline (shift their `at_ticks`). |

### DW043x — geometry & container proofs (stair orientation; spec-0021 loot; `collect` container adoption)

Two unrelated families sharing a number block: proofs that a *block* is the
block the content meant, rather than proofs about quests or timelines.

| Code | Meaning |
|------|---------|
| `DW0432` | A **positional container fill** declares more stacks than a vanilla chest or barrel has slots (27): a `loot` entry's `items`, or a `collect` whose own stack plus `fill_count` padding exceeds 27. Validation-tier (exit 1). Slots are assigned positionally, so every stack past the 27th would be dropped without a word. Prescription: split the contents across more than one container, or lower `fill_count` — a container that reads full does not need to overflow. |
| `DW0433` | An enchantment id — on an `equipment` piece, a `loot` stack, or (spec-0075) a `give-item` at any effect root — is not in the pinned 1.21.11 enchantment registry. Validation-tier (exit 1). The registry is the 43-id `enchantment` list from the same misode/mcmeta 1.21.11 summary the item registry comes from. The message calls out the classic trap explicitly: vanilla's curse ids are `minecraft:binding_curse` and `minecraft:vanishing_curse`, never `curse_of_binding`. |
| `DW0434` | An enchantment level outside `1..=255`, the range the `minecraft:enchantments` component can store. Validation-tier (exit 1). Levels **above an enchantment's survival maximum are deliberately allowed** — exceeding it from a command is legal vanilla and is precisely how a set-piece elite is built, so the compiler does not overrule that design call. `0` means "not enchanted" and is silently dropped by the game, which is why it is rejected rather than ignored. |
| `DW0437` | An `interact` declares `missing_item_hint` without a `requires_item`. Validation-tier (exit 1). The hint exists to answer a click that arrives without the required item **in hand**; with no item gate there is no such click, so the authored line is dead content that could never narrate — and an author who wrote one plainly meant to gate the interaction. Prescription: add the `requires_item` the hint is about, or drop the hint. |
| `DW0435` | Two **positional container fills** claim one anchor: two `loot` entries, or a `loot` entry and a `collect`'s adopted `container`, or two adopted collects. Validation-tier (exit 1). Slots are assigned positionally from `container.0`, so the later fill overwrites the earlier one slot-for-slot and the loser's items never reach the player — and for two collects it is worse: whichever activates second replaces the first objective's items with its own. Prescription: give each fill its own container anchor (prefabs may expose several), or fold the items into one — never rely on declaration order to combine them. |
| `DW0436` | A **single-slot fill**'s `count` exceeds the item's `minecraft:max_stack_size` in the pinned 1.21.11 registry. Validation-tier (exit 1). Covers every DSL surface that compiles to `item replace … container.<n> with <item> <count>`: a `loot[]` stack, a `collect` objective's prop chest, and a trap's `dispense` payload. The command fails **SILENTLY** above the cap — the slot ships empty, the server logs nothing — which is the same silent-failure class `DW0431` exists for: `minecraft:rabbit_stew` (cap 1) declared `count: 2` puts nothing in the chest. The cap is Mojang's own data, vendored per MC pin as `crates/delvec/data/item-stack-sizes-1.21.11.json` (regenerate with `tools/maintenance/extract-item-stack-sizes.py`; a test pins its key set equal to the item registry's) — never a hand-maintained table. 1.21.11 uses exactly three caps: 1, 16, 64. Skipped when the item id is unknown, since that is already `DW0143`. Prescription: lower the count, or add more entries/containers. Do NOT rely on the game splitting the stack — `give` does, `item replace` does not. |

### DW044x — command-driven trap payloads (spec-0022)

This module's rows of a section whose prose is on the [`delvec::compiler::nav` page](../delvec/compiler/nav.md#dw044x--command-driven-trap-payloads-spec-0022).

| Code | Meaning |
|------|---------|
| `DW0440` | A trap declares **no consequence at all** — neither the legacy redstone `effect` (spec-0011 `dispense`) nor a spec-0022 command `payload`. A trigger with nothing downstream of it is scenery, but the completability proofs still model its cell as a hazard, so it is a content mistake rather than a deliberate no-op. Validation-tier (exit 1), `dsl::validate`. Prescription: give the trap a `payload` (`volley`, `collapse`, `damage-players`, `play-sound`, `narrate`, `set-flag`, `spawn-wave`, …). |
| `DW0441` | A payload verb's vanilla id is not in the pinned 1.21.11 registry, or is of the wrong kind: a `volley` `projectile` must be an **entity** id, a `collapse` `falling_block` / `then_floor` a **block** id. Validation-tier (exit 1), `dsl::validate`; mirrors `DW0143`/`DW0341`. |
| `DW0443` | A `volley`'s `salvos` (1..=16) or `interval` (1..=200 ticks) is out of range. A volley fires its whole kill zone every salvo, so the entity count is `salvos x standable cells`; past the cap that is a server hazard rather than a trap, and salvos spread wider than the interval cap stop reading as one event. Validation-tier (exit 1), `dsl::validate`. |

### DW0898 — a piece where the body shows it (`dsl::equipment`; error; exit 1)

The server stores all eight equipment slots on every living entity and a
`/summon`'s `equipment` compound is read whole; the client draws a slot only
when the renderer registered for the entity type carries a layer that draws
it. A piece in a slot its body does not draw is NBT nobody sees.

| Code | Meaning |
|------|---------|
| `DW0898` | **A piece is declared where the pinned game will not show it on that body** (spec-0067). `dsl::equipment::fit_checks`, called from `dsl::validate`, validation tier (exit 1), once per piece of every `equipment` a wave mob or an actor declares. The body is the entity the puppet wears (`BodyRef::worn_entity`: a skinned actor is a `minecraft:mannequin`). One code, three shapes, every shape that holds named in one message: **the body does not show it** — the body table has no entry for that entity type at that slot for the piece's kind (a chestplate on a horse, a sword in a creeper's hand, an iron helmet on a villager, whose head draws an item and not armour, any piece on a body that is not a living entity); the message lists the slots the body draws with their kinds, and for a hand it names `attributes` as the way to write a held weapon's number; **the wrong slot** — the item's `equippable.slot` is another slot (a helmet in `legs`, horse armour in `chest`); the hands take any item and are exempt; **the wrong body** — the item's `allowed_entities` exclude the body (a saddle on a zombie, horse armour on a skeleton horse), with every admitted entity spelled out, tags expanded. **Not judged**: an item with no `equippable` component in a slot the body draws for an `item` (a carved pumpkin or a block on a zombie's head). **The kinds**: `armour` (an asset and a `head`/`chest`/`legs`/`feet` slot, `HumanoidArmorLayer`), `wings` (an asset with a `wings` layer), `animal` (an asset with a `*_body`/`*_saddle` layer), `item` (no asset, or no component). **The two tables**: the body table (`crates/dsl/data/entity-slots-1.21.11.json`, 92 rows over the 157 pinned entity types, each naming its renderer; groups: 16 humanoid six, 5 head item and both hands, 2 head item and main hand, 4 main hand only, 2 both hands only, 5 body and saddle, 6 saddle only, 4 body only, 48 nothing; the evoker, illusioner, vindicator and panda draw their hand only in a state and count as showing it) is authored from the pinned client's renderers and held by `crates/delvec/tests/equipment_tables.rs` to `#minecraft:can_equip_saddle`, to the body items' allowed lists plus the skeleton horse, and to the equipment assets' 15 body/saddle layer types; the item table (`crates/delvec/data/item-equippable-1.21.11.json`, 84 items, `tools/maintenance/extract-item-equippable.py`) is injected through `ItemRegistry::equippable`, and a registry that does not know an item judges nothing. **Binding**: `equipment binding: B body(ies) dressed over K entity type(s), P piece(s) declared over S slot(s) in use, F piece(s) with a registry-declared slot, A with an allowed-entity list, R refused (DW0898).` on every run of the validation funnel, zeroes included. **Emission**: the two new keys ride the component-era `equipment` / `drop_chances` compounds in `EquipSlot::ALL`'s order, the drop-strip line zeroes all eight, and the generated `v06_actor_equipment_<body>` PackTest asserts every filled slot on the puppet and on the unleashed twin — the live proof that the pinned server stores each key. **Prescription**: dress another body, choose a piece the body draws, move the piece to its declared slot, or change the entity. Gallery probe: `a-chestplate-on-a-horse`. |

### DW0945–DW0950 — an endless corridor (`compiler::loop` / `dsl::validate` / `compiler::plan`; spec-0086; error + one advisory)

This module's rows of a section whose prose is on the [`delvec::compiler::loop` page](../delvec/compiler/loop.md#dw0945dw0950--an-endless-corridor-compilerloop--dslvalidate--compilerplan-spec-0086-error--one-advisory).

| Code | Meaning |
|------|---------|
| `DW0949` | **A loop whose release is not a fact about the party, or that has none.** A loop with no gate term; a `requires_state` term or a `counts` naming a `player`-scoped datum; a `teleport` effect inside `on_cross`, at any depth. Validation tier (exit 1), `dsl::validate`. Prescription: a `party` datum, a flag, or a release the party reaches. |

### DW046x — the NPC scene ledger (`compiler::cast`; spec-0020)

This module's rows of a section whose prose is on the [`delvec::compiler::cast` page](../delvec/compiler/cast.md#dw046x--the-npc-scene-ledger-compilercast-spec-0020).

| Code | Meaning |
|------|---------|
| `DW0469` | (**warning**; exit 0) A campaign stages actors meant to **fight** — unleashed into a real-AI twin, or declared `vulnerable` — but declares no `waves[]` and no `world.difficulty`, so it ships the derived `difficulty=peaceful` and a monster among them is discarded on the tick it spawns. "Meant to fight" is read off the campaign's own declarations (`unleash-actor`, `vulnerable`), never guessed from the species: the pinned entity registry is a membership set with no mob-category data, so *is this a monster* is exactly the question the compiler cannot answer — which is why this is advisory. Prescription: declare `world.difficulty`. |

### DW0890 — the approved hour is the built hour (`compiler::design`; error; exit 1)

This module's rows of a section whose prose is on the [`delvec::compiler::design` page](../delvec/compiler/design.md#dw0890--the-approved-hour-is-the-built-hour-compilerdesign-error-exit-1).

| Code | Meaning |
|------|---------|
| `DW0891` | **A killing volume the player cannot see** (spec-0062). One code, three shapes, one rule — *a killing volume and what shows it agree*. The ruling it transcribes is the owner's: **avoiding a lethal volume's collateral damage is never done by marking ground that looks walkable as unwalkable, because the player does not know.** The constraint belongs on the volume: it sits at the bottom of a pit, or far enough from the mouth that a body standing on solid ground cannot be caught. **The world arm** is `compiler::lethal::check_danger_is_visible`, build tier (exit 3), raised once per volume over the final assembled world — whether a cell is floor is a fact about the settled bytes and where the volume is at all is a fact about the solved layout, so nothing before assembly can answer it. Its two shapes: *caught floor that shows nothing*, naming the volume, its keep-out box, the caught cells by floor as `DW0881` prints them, and the declared signals if any; and *a declared signal the bytes do not hold* — a `shown_by` block under or in no caught cell, naming the block and the count of caught cells it was checked against, where a count of zero says the volume catches nothing and the declaration is what is wrong (`DW0887`'s shape, on a volume instead of a waterline). **The document arm** is `dsl::validate`, validation tier (exit 1): a `shown_by` id that is a known block and not one vanilla hurts a body with — nothing has to be placed to know it, and the message prints the set to choose from. **The terms, and each is a set the engine already computes**: the keep-out is `metrics::keep_out_box(Body::PLAYER, V)`, asked of each cell with the body's feet where the model stands it (`World::body_can_meet_volume`, so a body on a partial floor is caught by a volume in the course it stands on); the population is `World::reachable_walkable` over `World::without_exclusions` — lethal volumes and furniture (spec-0065) both lifted, so a furniture declaration over caught floor cannot hide it — rooted at every cell the party MAY be put at (`lethal::put_at_roots`, spec-0083 §3.8: the entry spawn, every `set-checkpoint` and `bonfire` seat, every crossing's entry point and every `teleport` destination, link or gather) rather than at the entry alone, because a party carried into an area stands on that area's floor — and a wider population here can only refuse more; the caught cells are the intersection; and a caught cell **shows** when the block **under** it or **in** it is one of `shown_by`. Under or in, never beside — a shore cell next to lava stands on stone and holds air, and the lava beside it is not what the body is on. **The population is the lethality-free one, and that is the whole binding.** On the world the router walks, the keep-out is already impassable, so a population taken from it can never contain a caught cell and this check would be green over every volume ever written while matching nothing. `the_population_is_the_lethality_free_one` computes it both ways on the `lethal-volume` fixture and asserts **zero** over the lethal-applied world and **nine** over the counterfactual, and the check is bound to the second. **What separates a lava lake from a kill zone over stone** is never the volume: the occupancy model classes water and lava alike as flooded — impassable, never floor — so no cell of a lake's surface is ever standable, ever in the population, ever caught. A volume drawn to the lake's *shore* catches the shore, which stands on stone and shows nothing, and is refused; the remedy is to draw it one cell in or one course under. **One code for three shapes** because they are one disagreement and every remedy each names is admitted by the others: signalling the floor satisfies the first and creates no fiction, deleting a fiction never uncovers floor, naming a hurting block never makes a floor read as safe. **Where it stands in the order**: `DW0511`, then this, then `DW0510`, then `DW0850` and `DW0881` — a volume that catches walked floor usually closes a route as well and often sits under a reach, so asked first the refusal names the cause and every later rule then judges a volume the player can see. It reads the plan's volumes, the population and the block map, never a route, so nothing is lost by asking it early. **Bound at both entry points**: `emit::build` and the world-edits batch replay (`compiler::edit::check_batch_invariants`), before the route proof at each — a gate bound at one of two doors is bound at neither. **It changes nothing about the walk graph**: the router still refuses every cell of the keep-out, the recovery stake still chooses its lip outside it, `death-plan.json` still carries it to the bot. What this guarantees is that the cells the walk graph loses are cells a player could see were dangerous. **Prescription, always the geometry's**: lower the volume so its keep-out's top course lies under the floor (a pit's volume sits on an anchor at the pit's bottom); draw its `extent` in so the keep-out stops one cell short of the floor; or author one of the blocks vanilla hurts a body with under those cells and declare it in `shown_by`. Never mark walkable-looking ground unwalkable. **Binding**: `danger-visibility binding: N volume(s) examined against a walked population of M cell(s); …; R of N volume(s) reached by a body the engine models.` on every build that declares a volume, refusals included, and the same numbers in `validation/lethal-gate.json`'s `danger_visibility`. **Its zero binding is said, not passed**: an empty catch over a volume no body can enter looks exactly like one over a volume clear of the floor, so each volume is also asked whether any modelled body gets its hitbox into it at all — a player walking, falling, jumping or swimming from the walked population (`World::body_moves`, which does not dive), a fall through it at any depth, or a wave member (`DW0922` / `DW0923`'s reach). A volume none reaches is a `DW0891` **warning** naming it — not a refusal, because the usual cause is a movement the engine does not model (a player diving to the bottom of a flooded shaft), and refusing would reject a legitimate design for an engine limitation. **A volume live from a story stage is judged per configuration** (spec-0088): in every configuration the critical path or a branch path passes in which it may be live, and in the last configuration before each switch-on, each over that configuration's world and bytes; the two shapes above name the configuration, and **the fourth shape** is raised in the configuration before a switch-on — *`lethal/<id>` goes live at step N of the path and a body may be standing on these cells when it does; nothing in the world before that beat says they kill* — prescribing, in order: roof or wall the cells off until the beat that opens them, lower the volume under the floor a body stands on before the flip, or author a hurting block visible before the flip and declare it. Its zero binding is asked per configuration, and a staged volume judged in no configuration is refused naming the enumeration. |

### DW0935–DW0938 — a fixed thing that can be hit and hits back (`dsl::validate` + `compiler::assembly`; error)

This module's rows of a section whose prose is on the [`delvec::compiler::assembly` page](../delvec/compiler/assembly.md#dw0935dw0938--a-fixed-thing-that-can-be-hit-and-hits-back-dslvalidate--compilerassembly-error).

| Code | Meaning |
|---|---|
| `DW0935` | **An assembly's rig cannot be emitted as declared** (spec-0082 §5.1, §5.5, §3.2). Validation tier (exit 1), `dsl::validate::assembly_checks` over `AnchorRegistry::rig` (the compiler's `PrefabRegistry` reads `<library>/rigs/*/rig.json`). The library holds no `rigs/<name>/rig.json`; the file does not parse as a rig document; or it breaks a structural rule `dsl::rig::check` states — no part, a part's block not a block state of the pinned registry (the `DW0193` rule), no clip, a clip with no frame, a frame that is not one transform per part, `ticks_per_frame` outside `1..=20`, a non-finite number, a scale of 0 on an axis, a zero-length quaternion, a clip name that is not kebab-case — each naming its field; or an `initial`, a strike step's `windup`/`strike`, or a `play-clip` (at any depth of any root) names a clip the rig lacks, the message listing the rig's clips; or a strike step's `ticks_per_frame` lies outside `1..=20`, naming the field. `delvec rig describe` refuses the same rig with the same code. Prescription: regenerate the rig with its generator, or name a clip the rig declares. |

### DW0968–DW0970 — a strike locks where the player stands (`dsl::validate` + `compiler::assembly`; error)

| Code | Meaning |
|---|---|
| `DW0969` | **A locked blow is declared where the lock derives it** (spec-0094 §5.2). Validation tier (exit 1), `dsl::validate::lock_shape_checks`. In a step with a `lock`: a top-level `damage-players` with an `in` (the blow's area is the cells the chosen pose comes down on — a written box is a second, fixed answer), a `damage-players` nested at any depth inside another effect's list (it cannot be moved with the lock), and the `lock` itself when the pattern declares `aim` (two rules choosing one turn), each naming the field. Prescription: drop the `in`, lift the `damage-players` to the top of `on_land` (a `when` on it is kept), or drop `aim` or `lock`. |
| `DW0970` | **An `arm-strikes` names an assembly that never strikes** (spec-0094 §3.3). Validation tier (exit 1), `dsl::validate::assembly_checks`, at every depth of every effect root: the named assembly declares no `strikes`, so there is no pattern to re-arm and the beat does nothing. Prescription: give the assembly a `strikes` pattern, or drop the effect. |

### DW0901 — a purchase adds up where it is written (`dsl::purchase`; error; exit 1)

**A price has no field, so the engine compares the copies of it.** spec-0032
rules that a price is a gate term and that an offer's refusal is authored: the
purchase behind `at-least <price>`, the apology behind `at-most <price − 1>`, the
charge an `add-state` of `−<price>`. That ruling stands, and its cost is that one
number is written four or five times — in the button's own words, on the gate of
what the player receives, on the gate of the charge, as the charge's `amount`,
and, minus one, on the arm that answers below it — with nothing else binding
them, so an offer gating on `at-least 15` and charging `16` would compile.

**The quantifier is every effect list whose effects charge a datum**, never a
shop: the rule reads the gate, so it binds wherever the shape occurs — a shop
offer, an environment trigger, a trap payload, a quest's `on_complete`, a
`sequence`'s step. A list is judged together with the gate of whatever it hangs
off (an offer's own gate, a trigger's or trap's arming gate, a nested list's
parent `when`), because both must hold for the charge to run; that is what lets
the correct spelling of a shop — gate the offer, leave the effects bare — pass
while gating the offer at 15 and charging 16 is refused.

| Code | Meaning |
|------|---------|
| `DW0901` | **A purchase whose literals do not add up** (spec-0071 §2). `dsl::purchase::purchase_checks`, called from the one validation funnel, **validation tier (exit 1)**; the `DwCode` declares build tier, which is what happens if the rule refuses with a build under way. One code, three shapes, one rule — *a charge and the gate beside it are one price*. **A charge deeper than its floor**: an effect charges `n` of a datum while the floor its own `when` and its list's enclosing gate leave open is `m < n`, so at a balance of `m` the charge leaves the datum at `m − n` — below what the creator gated on. The message names `n`, `m`, the datum and the balance it lands on, and names the enclosing gate when one is read. **A charge nothing floors, in a list that prices the same datum elsewhere**: the copy is missing rather than wrong, and it fires at any balance including one that cannot pay. A list that says nothing else about the datum it moves is a design, not a purchase, and is not judged — a campaign may mean a datum to go below zero. **A refusal arm that does not meet the sale**: an effect gated `at-most k` on the datum and nothing else on it is the arm that plays when the player cannot pay, so `k` must be `m − 1`; a lower `k` leaves a **gap** (`k+1..m−1`, balances at which the button does nothing at all) and a `k` at or above `m` leaves an **overlap** (`m..k`, balances at which the player is charged *and* told they cannot afford it). An effect declaring both a floor and a ceiling wrote an interval and is left alone. **The arithmetic is the gate's own** (`gate::DatumSet::min`/`max`), the same interval-with-holes the satisfiability verdict is taken from, so the two cannot disagree about what a term means. **Its neighbour is `DW0527`**, which catches the same overlap arising from ORDER — an apology whose gate is read after the debit that moved it — where this one catches it arising from the literals; both prescribe moving something the author wrote, and neither prescribes a field. **Prescription**: raise the gate to the charge, lower the charge to the gate, or set the answering arm's ceiling one below the sale. There is no `price` field to add, and adding one would be a second comparison surface for one meaning. **Binding**: `purchase binding: C charge(s) over D datum(s) bind P (list, datum) pair(s) of L effect list(s) walked, G of them behind a numeric gate, R refused (DW0901).` on every validation run, zeroes included. Gallery probe: `a-key-that-charges-more-than-it-asks`. |

### DW0909–DW0912 — a fight shows its health (`dsl::healthbar` / `compiler::healthbar`; error + advisory; exit 1 / 0)

**A bar is a claim about a fight, so a bar nothing can make true is refused where it is written.** `health_bar` (spec-0073) is optional on every wave and actor; the three refusals judge the declaration against the documents, and the advisory names the one billing whose fight a player cannot read. All four are raised at `delvec validate`, before any build: `DW0909`, `DW0910` and `DW0912` by `dsl::healthbar::health_bar_checks` inside `validate_campaign_with`, `DW0911` by `compiler::healthbar::check_vocabulary` from the same validation funnel, because the vocabulary it reads is the pinned command tree and that tree is `delvec`'s data (the `DW0343` precedent). A `range` outside `4..=64` is `DW0100`, the exported schema's own bound restated at the document tier (the `firework` precedent). The quantifier is every fight of either class — `dsl::fight::fights`, waves then actors, the one fight type `health_bar` and `on_kill` both read — and `delvec validate` states the binding: `health-bar binding: <with a bar> of <fights> fight(s) carry a bar (<w> wave(s), <a> actor(s) declared); <b> of <boss> boss-billed fight(s) carry one; <n> refused (DW0100/DW0909/DW0910), <m> advised (DW0912).`

| Code | Meaning |
|------|---------|
| `DW0909` | **A health bar over a body whose health cannot move** (spec-0073 §8.1). An actor declaring `health_bar` that is not `vulnerable` and that no `unleash-actor` names — at any effect root, at any nesting, through the one effect walk `combat::hostile_actors` also reads. Its only body is an `Invulnerable`, `NoAI` puppet, so the bar would sit full forever. Prescription: unleash it somewhere, mark it `vulnerable`, or remove the bar. Validation tier (exit 1). |
| `DW0910` | **A health bar with nothing to title it** (spec-0073 §8.2). `title` absent and the fight has no single name of its own — a wave of two entries or more, a one-entry wave whose entry has no `name`, an actor with no `name` — or `title` blank. The engine owns no wording that is right for every fight, so it does not invent one (the `lethal.<id>.message` rule). Prescription: state `title`, or name the body. Validation tier (exit 1). |
| `DW0912` | **Advisory: a fight billed `boss` declares no `health_bar`** (spec-0073 §8.4). **Warning tier, never blocking** — the build proceeds and exits 0. Fires for `tier: boss` only, on a wave and on an actor alike; `elite` and `ordinary` fights are never named by it, whatever they declare, because an unannounced elite is a legitimate design. The line names the fight and prescribes `health_bar`. |

### DW0913–DW0915 — a kill pays only where a kill can pay (`dsl::onkill`, `compiler::onkill`; error; exit 1)

This module's rows of a section whose prose is on the [`delvec::compiler::onkill` page](../delvec/compiler/onkill.md#dw0913dw0915--a-kill-pays-only-where-a-kill-can-pay-dslonkill-compileronkill-error-exit-1).

| Code | Meaning |
|------|---------|
| `DW0913` | **A bundle on a body no player can be credited with killing** (spec-0074 §8.1). `dsl::onkill::on_kill_checks`, called from the one validation funnel, **validation tier (exit 1)**. Two shapes: `on_kill` on a wave no beat seats — `delvewright_dsl::wave_area` is `None`, the same fact that leaves the wave without `spawn_<wave>`, a kill advancement or a kill reward — or on an actor no `unleash-actor` names (`delvewright_dsl::unleashed_actors`, the definition `combat::hostile_actors` reads) that is not `vulnerable`, whose body is `Invulnerable` for the whole delve. Path: the bundle (`/content/waves/<i>/on_kill`, `/content/actors/<i>/on_kill`); the message names the fight. Prescription: spawn or unleash it somewhere, mark the actor `vulnerable`, or remove the bundle. The same pass refuses an empty `effects` list as `DW0100` (the schema's `minItems: 1`, which serde does not enforce). |

### DW0939/DW0940 — a delve wears its own textures (`dsl::validate` + `compiler::textures`; error; exit 1)

| Code | Meaning |
|---|---|
| `DW0940` | **A row's file is not an image the named texture can be replaced by** (spec-0084 §6.2): not a PNG that decodes; not `k·w₀ × k·h₀` of vanilla's frame for one integer `k` (with a sidecar, not `k·w₀ × n·k·h₀`); a sidecar that is not vanilla's animation metadata; or bytes equal to vanilla's own (the census sha256), which replace nothing. `compiler::textures::resolve`, at `delvec validate` and again at build. The message names the file, its size, vanilla's, and the size it must have. Prescription: resize the image, make the strip a whole number of frames, correct the sidecar, or draw the image the delve wants there. |

### DW0919 — a currency stands where the sidebar can draw it (`dsl::validate`; error; exit 1)

**A standing display is a claim about the one slot that stands, so a declaration the slot cannot draw as written is refused where it is written.** `state[].display: sidebar` (spec-0076) is optional on every datum; `dsl::validate::state_checks` judges it against the declaration itself, at `delvec validate`, before any build. What stands is three `setup` lines (the `state[].display` surface row): the objective headed with the datum's translated `name`, its value painted gold, and the objective put in the slot, once, at world init. The quantifier is every `state[]` entry carrying `display`.

| Code | Meaning |
|------|---------|
| `DW0919` | **A standing display the sidebar cannot draw as declared** (spec-0076 §7). Three shapes under one rule, the `DW0520` shape: (1) two datums both declare `display: sidebar` — the slot holds one objective, and the refusal names both rather than picking by order, because which purse the party reads between changes is the creator's decision; (2) the datum has no `name` — the slot's heading is the display name, and without one the objective's internal id would stand on every screen, the one thing the slot must never show; (3) the datum is `party`-scoped — its value lives on the `#party` holder, and the sidebar hides every `#`-prefixed holder, so the display would be a heading over nothing; mirroring the value onto a visible fake player is a name a real player could carry under a label the engine would have to invent, and is not done. Path `/content/state/<i>/display`; the message names the datum (both, for shape 1). Prescription: keep one `display`, give the datum a `name`, or declare it `player`-scoped; a `party` purse keeps its announcement and stands nowhere, the recorded gap. Validation tier (exit 1). |

### DW048x — branch-complete narrative verification (`compiler::branch`; spec-0025)

This module's rows of a section whose prose is on the [`delvec::compiler::branch` page](../delvec/compiler/branch.md#dw048x--branch-complete-narrative-verification-compilerbranch-spec-0025).

| Code | Meaning |
|------|---------|
| `DW0931` | **A celestial time whose shape states nothing a sky can show** (spec-0081 §6). One rule about one value's shape — a time is one body, one position and, where the moon shows, one phase — broken four ways, each raised where the value is entered: (1) the object names **neither or both** of `sun` / `moon` (the `DW0160` exclusivity shape, under this code because the object is a time); (2) a **`phase` where the moon is below the horizon** — `{"sun": "high", …}`, `{"moon": "below", …}`, `{"sun": "just-risen", …}` — the message naming the moon's altitude there; (3) **`world.time` with no `phase` where the moon is at or above the horizon** — the party sees it, so nobody may leave it to a default; the message lists the eight names; (4) a **`phase` on a `set-time`, a design row or a camera equal to the world's** (a restatement: a phase left out is the world's). A phase outside the eight names and a position outside the six are `DW0100`, not a fifth shape. `delvewright_dsl::celestial::shape_findings` is the one statement of the four rules: `delvec validate` applies it to the world, every `set-time` at every effect root and depth, every dialogue `set-time` and every design row (`celestial::check`, validation tier, exit 1), and the design gate applies it to every camera's celestial `sky.time` (`compiler::design::check`). Binding: the `clock:` lines and the `clocks:` summary every run prints (stage 1, *Celestial time*). **Every move it names is reachable** (`remedy_reachability.rs::dw0931_every_named_move_validates`). Prescription: NAME ONE BODY; REMOVE `phase` where nobody can see the moon; STATE `phase` on the world's time where the moon is up; REMOVE a `phase` that restates the world's. |

### DW0490–DW0493 — declared drops (`dsl::validate`)

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

### DW050x — runtime state (`dsl::validate`; spec-0031)

Runtime state is a **declared** datum: a name, a scope (`player` / `party`) and
an initial value, written by `set-state`/`add-state`/`clear-state` and compared
against by `requires_state` in any gate. All four codes are validation-tier (exit
1), in `dsl::validate::state_checks`. A campaign that declares no datum emits
none of it: no scoreboard objective, no `state_seed` function, no tick clause,
no guard clause.

Both directions of the read/write ledger are errors, because each is a **vacuous
binding** in the CLAUDE.md sense and each is silent — the campaign compiles, the
datapack loads, and the delve plays as though the mechanism were live.

| Code | Meaning |
|------|---------|
| `DW0500` | **An undeclared datum.** A `state/<kebab>` reference — in a `requires_state` comparison or in one of the three verbs — names a datum the stage-5 `state` list does not declare. Unlike a flag, whose set is exactly what some `set-flag` produces, a datum is declared because its scope and its initial value are facts no use site can supply: an undeclared reference is not "a datum that happens to start at zero", it is a datum with no defined multiplayer semantics at all. Prescription: declare it, or fix the id. |
| `DW0501` | **Read, never written.** A gate's `requires_state` reads a declared datum that no verb anywhere in the campaign ever writes, so it can only ever hold its declared `initial` and every comparison against it was decided when the campaign was written. The gate is a constant wearing a condition's clothes — the numeric form of a combat floor examining zero enemies. Prescription: write it somewhere, or drop the comparison and say what you meant unconditionally. Its emitted-layer sibling is `DW0495`, which asks the same question of the commands rather than of the campaign, and therefore reaches the engine-internal objectives no campaign can declare. |
| `DW0502` | **Never read.** A declared datum that no gate's `requires_state` anywhere in the campaign ever reads. Either some verb writes it and nothing ever asks (an inert write — a counter nobody consults), or nothing touches it at all (a dead declaration). Runtime state exists to be compared against; a datum with no reader is bookkeeping no player can observe. Prescription: gate something on it, or delete the declaration and its writes. |
| `DW0503` | **No acting player.** A `player`-scoped datum is read or written where emission has no `@s` to resolve it against. Every such place is a property of the SITE, never of the verb, and there are three kinds. (1) **The root.** Six of the nine effect roots run with an acting player and three do not — a trigger's `effects`, a trap's `payload` and a shortcut's `on_unlock` are polled on the tick from the server command source (`Audience::Scheduled`), while `on_objective_complete` / `on_complete` are dispatched `as @a`, `on_death` / a dialogue `on_respawn` are the dying-or-respawning player's own, a shop offer is the buyer's, and a fight's `on_kill` (spec-0074) is the credited killer's. The answer is `EffectRootKind::runs_with_acting_player`, bound by equality to `emit::root_audience` over the closed root set — **except that a trigger answers per declaration**: `audience: presser` is dispatched by the interaction advancement and does have an `@s`, so the check asks `EffectRootSite::runs_with_acting_player` (which consults the trigger) and the kind-level answer stays the class default. Asking the kind would refuse a `player`-scoped read the emitter can serve. (2) **The seams inside a bundle**, one statement read by `DW0357` too (`QuestEffect::nested_effect_dispatch`, spec-0085): a `move-npc`/`move-actor` `on_arrive` and a `bonfire`'s `on_rest` drop the actor; a `set-checkpoint` `on_respawn` and a `begin-stealth` `on_caught` restore it; a `sequence` step keeps whatever its timeline was started with — under a root with an actor the timeline carries it by a tag, so a step there has one, and under a polled root it has none. **A fourth shape** (spec-0085): an effect declaring `audience: actor` where emission has no acting player — the same rule, *no `@s` where emission has none*, with the same remedy: move the beat onto a site a player drives, or address the party. (3) **The gates emission evaluates against the party holder** — an objective's activation guard, a trigger's arming gate, a trap's arming gate. Reads and writes are treated alike: a per-player score named from a sourceless function is `@s` with nothing to resolve it to, whether the command is a `scoreboard players set` or an `execute if score`. Prescription: declare the datum `party`-scoped if the whole party shares it, or move the read/write onto a site a player drives — a dialogue option, a cast placement, `on_death`, or an effect on a beat a player completes. |
| `DW0941` | **A particle the game does not draw from a bare id** (spec-0085 §4.3). A `particle` effect names an id the pinned registry (`crates/dsl/data/particles-1.21.11.json`, 115 types) does not hold, or one of the 18 whose type takes options (`dust`, `block`, `item`, `flash`, …) — the verb carries no options, so the game would refuse the command. Validation-tier (exit 1), `dsl::validate`. Prescription: a registered id a bare name spawns. |
| `DW0942` | **An audience on a party fact** (spec-0085 §3.3). An effect states the envelope's `audience` or `in` on a verb the emitter fires once for the world (`Verb::addresses_players` is false — a flag, a gate, a block, a region, a wave, an actor, an NPC, a camera, the time, a checkpoint, a bonfire, stealth, a timeline, a teleport, a volley, a collapse, a rocket, a state write). A box has no party and a world fact has no audience; a `sequence`'s steps each state their own. Validation-tier (exit 1), `dsl::validate`, naming the verb and the field. |
| `DW0944` | **A sight effect that ends under a camera** (spec-0085 §5.3). In one timeline, a `give-effect` of a sight effect (`dsl::perception::SIGHT`: `night_vision`, wind-down 200 ticks; `blindness` and `darkness`, 20 ticks authored from memory) whose window `[at_ticks, at_ticks + 20 × seconds)` overlaps a `cutscene` step's and ends at or after its start and before its end plus the wind-down — so it starts ramping down on screen. The message names the grant, the shot, the tick the grant ends and the `seconds` that clears it. Validation-tier (exit 1), `dsl::validate`. The `give-effect` half of the findings-ledger row whose general form is that a granted sight effect outlasts any authored camera it can overlap. |

#### Which sites can touch a per-player datum, and why it is decidable

Two closed sets answer it, and neither is a list anybody maintains.

`GateConsumer::evaluates_per_player` answers for a gate's own site, and returns
`Option<bool>`: a dialogue option's availability is computed per player into
`dw.dmask` and its `/trigger` handler runs `as @s` (`Some(true)`); a cast
placement selects a scene into a per-player `dw.cast` (`Some(true)`); an
objective's guard, a trigger's arming gate and a trap's arming gate are party
predicates by construction (`Some(false)`). **`Effect` answers `None`** — an
effect's gate is evaluated wherever its bundle runs, and that belongs to the
root. The `Option` is deliberate: a plain `true` for `Effect` would be right
for `on_objective_complete` and wrong for four of the ten roots, silently. An
eighth consumer class cannot compile without answering.

`EffectRootKind::runs_with_acting_player` answers for the root, exhaustively, and
`emit::root_audience` is the single place the emitter chooses a bundle's
audience — one function rather than a literal per call site, bound to
the DSL's answer by equality in `emit::tests::root_audience_matches_the_dsl`. A
root whose emitted audience moved without that answer moving with it would turn a
validated per-player read into an `@s` in a sourceless function, with every check
green; a tenth root fails the bind until both sides name it.

#### One gate, three fields

`requires_flags` / `forbids_flags` / `requires_state` are one object
(`dsl::gate::Gate`), and every consumer answers `gate()`. Two things keep that
from decaying:

- `crates/dsl/tests/gate_consumers.rs` enumerates the gate-declaring object
  schemas **from the generated JSON Schema** — derived from the Rust types, so
  the enumeration is complete by construction rather than by diligence — and
  fails when any of them declares part of the gate and not the rest. It states
  its binding count (`GATE_SITES` declaring schemas, `GateConsumer::COUNT`
  consumer classes) and asserts it exactly, so a new gate consumer is a
  deliberate diff rather than a silent one.
- An effect's gate is one `Guard` under `QuestEffect::when`, carried by every
  verb, so every verb is gatable on identical terms and
  `tools/ci/check-capability-ownership.py`'s `MODIFIER_HOLES` holds no gate
  field. A gate is one object: giving its comparison a different carrier set
  than its flags would make "which verbs are gatable" two different answers.

**Where a comparison IS evaluated, and where it is not.** A `requires_state`
comparison stays out of the monotone producibility fixpoint, exactly as
`forbids_flags` does: that fixpoint has no notion of *when*, and a comparison is
entirely about when. The compensating stronger check is the **path replay**,
which does have a concrete order — so a numeric gate is evaluated there, against
the value the path itself has produced by the time the gate is read, and
`DW0879` refuses one the path has already made unsatisfiable. `DW0501` is the
other half and asks a different question: whether the datum is driven at all.

### DW0847 — a gate that can never open (`dsl::validate`; every gate consumer)

| Code | Meaning |
|------|---------|
| `DW0847` | **A gate contradicts itself, so it can never open.** A flag on both `requires_flags` and `forbids_flags`, or `requires_state` terms on one datum that no integer satisfies (`at-least 5` with `at-most 3`, two different `equals`, a `not-equals` punching out the only pinned value). The thing carrying it — objective, effect, trigger, trap, dialogue option, cast placement, shop offer — is authored content that provably never happens. One rule over the whole closed consumer set (`dsl::gate::for_each_gate`), because satisfiability is a property of the **gate**, never of the verb that first needed the question answered — the first asker was the cast ladder's per-clause solver (`DW0846`), and a check written beside it would have left the other six classes with no surface. The arithmetic is `dsl::gate::DatumSet` (interval-with-holes intersection, exact emptiness), the same value-picker the solver drives generated `cast_ladder_*` phases from, so "can this open" and "at what value" have one authority. Validation tier (exit 1) — it judges an authored contradiction, a fact of the campaign alone. Distinct from `DW0501` (a satisfiable comparison whose datum nothing writes) and from the flow proofs' flag reachability: this is emptiness of the gate itself, before any question about what the campaign does at runtime. Prescription: fix the gate, or delete the thing it makes unreachable. |

### DW0540–DW0542 and DW0545 — status effects, the region teleport, and the fixture class (`dsl::validate` / `compiler::teleport` / `compiler::affordance`; spec-0031)

`DW0540` is the one rule in this family that is about a *pattern* rather than a
value, and it is the reason the surface is shaped the way it is. `give-effect`
has no infinite form: `seconds` is required and bounded, so a grant always ends
by itself. That can still be defeated by two effects that are individually fine —
grant blindness for an hour, clear it four ticks later — and then the clear is
the real removal, so any path that does not reach it (a logout, a crash, a death
mid-chain, a `sequence` whose remaining `schedule` never runs) leaves the player
blind for the rest of the hour.

The rule therefore fires on exactly the grants that are **still live** when their
clear arrives. Where the duration expires first, the duration is the removal and
there is nothing to say. "The same sequence" is mechanical: a bundle's own
timeline, where a plain member runs at offset 0 and a directly-nested
`sequence`'s members run at their step's `at_ticks` (nested sequences are
`DW0329`, so the expansion terminates). Conditional continuations — `on_arrive`,
`on_caught`, `on_respawn`, `on_rest` — are separate bundles with their own
timelines and are not folded in; a clear hanging off an arrival is strictly more
fragile than one on a fixed tick, and it is the mandatory duration, not this
rule, that keeps that case survivable.

`DW0542` is what stands where a runtime exemption list would otherwise be. A
`teleport`'s selector is total over bodies, so a volume drawn over an affordance
the engine anchored to a *block* would move the entity and leave the hardware: a
campfire, a lever or a sealed door still visible, still reachable, answering
nothing. The affordance set is not enumerated by this proof — it is
`eclipse::affordances`, the same authority `DW0359` measures bodies against, plus
the seal shells `DW0422` owns — so an affordance added to the engine enters this
proof by existing. Content bodies (NPCs, actor puppets, wave mobs) are
deliberately not refused: moving them is the mechanism working, and it is what
the cargo-lift ruling asks for.

Binding: `validation/teleport-gate.json` states how many teleports were declared
and resolved, how many cells their volumes cover, how many affordances were
examined, and how many PackTest templates were generated — a compile-time-only
green over a runtime mechanism is the vacuity that last number exists to make
visible. A campaign that declares no teleport emits no file at all, so a file
that exists and reports zero is a finding rather than an absence.

#### DW0545 — the fixture class: what a region verb selects

`DW0542` reaches every place whose cell the compiler knows. **A recovery stake's
marker has no such cell** — its position is the death point, or a row of the
compile-time placement table picked by the respawn seat in force. A lift that
carried the marker away from the position its ledger recorded would leave
`stk_gc` finding nobody holding a wager there on the next tick and retiring it:
the wager would not be uncollectable, it would be deleted.

The two obvious fixes are both defects CLAUDE.md names. *Teleport exempts engine
machinery* re-implements a general mechanism privately inside one verb; *the
stake ledger survives its marker moving* keys a capability to the wrong object,
making the stake compensate for a selector that grabbed something it should never
have grabbed. The question is upstream of both — **what does a content-authored
region verb select?** — and the measurement answering it is short:

| region verb | what its emitted selector reaches |
|---|---|
| `teleport` (`from`) | every **entity** in the box — the only verb with no filter at all |
| `lethal_volumes[]` (`region`) | every entity in the box minus six **types** (`@e`), plus every player (`@a`) |
| `give-effect` / `clear-effect` (`in`), `damage-players` (`in`), stealth zones, the night-vision area grant | **players only** (`@a`/`@s`) — no engine entity is reachable |
| `fill-region`, `clear-region`, `collapse`, `close-gate` | **blocks**; no entity selector exists |

So exactly two verbs quantify over non-player entities, and only they have the
question to answer.

The answer is a **class the object declares about itself**, not a roster any verb
holds. Every entity the engine summons carries one of two tags:

- **`dw_fixture`** — *a place.* Its position IS engine state: an affordance's
  `minecraft:interaction` hitbox, the `dw_marker` display beside it, a stake
  marker, a cutscene's return mark. Moving it does not move a thing, it rewrites
  a fact.
- **`dw_borne`** — *carried by a body.* Exactly one: an NPC's co-located
  dialogue hitbox, which must ride whatever its speaker rides.

A cutscene *camera* declares neither and that is deliberate: its own driver
re-asserts its position every tick, so it is a body the engine flies rather than
a place it recorded. Neither tag is authorable, and no campaign JSON can turn
either off.

Every box-narrowed entity selector then carries `tag=!dw_fixture` — **one negated
tag for the whole engine, forever**, which is what a type roster can never be. A
type cannot answer this question at all: an NPC's hitbox and a stake's marker are
both `minecraft:interaction`, and a teleport must move the first and leave the
second. `lethal_volumes[]` keeps its type roster as well, because that roster
makes a different and still-true claim — *do not aim `/damage` at a thing that
cannot take it*.

The two arms of the rule divide by **who can act on the defect**: a place whose
cell is known at compile time is *refused* (`DW0542`), because the author can
move it; a place only the runtime puts down is *skipped by the selector*
(`DW0545`), because nobody can.

`DW0545` is an emission self-check over the shipped datapack, in the `DW0420` /
`DW0421` family — it is `DW0421`'s rule (*only the owner may disturb an
affordance's hardware*) one verb wider, since moving hardware is disturbing it,
and one binding wider, since a region verb selects by box where `DW0421` reads a
tag. It fires on two clauses, and both are compiler defects rather than authoring
ones: a summon that declares neither class (the exclusion then protects nothing),
and a box-narrowed `@e` selector with no exclusion (the class exists and this verb
does not read it). It can never be caused or fixed by campaign JSON: it is an
engine self-check, run on every build.

**The runtime half is the only half that can witness a marker carried off**, and it
is generated rather than argued: one PackTest template per `teleport`, in a
campaign that declares a stake able to leave a marker, puts a real marker in a
real volume through the campaign's own `stk_fill_<id>`, rides the campaign's own
`teleport_<key>`, and asserts a plain body **left** the box while both halves of
the marker stayed. The body assertion is what stops it being one-directional —
without it, an engine whose teleport did nothing at all would pass. One template
per teleport rather than per (`teleport`, `stake`) pair because the marker is one
object: every stake summons the same two entities through the same `stk_place`,
so a second template for a second stake would race the first for one entity at
one position on the shared batch server. Every selector such a template writes
over the marker class is scoped to the place it is about, for the same reason.

Binding: `validation/fixture-gate.json` states how many entities declared each
class, how many box-narrowed selectors were examined, and how many runtime
templates were generated. Zero on either of the first two counts is reported as
`unbound` **with an `unbound_reason` naming which arm** — an empty class makes
every exclusion decorative, while zero selectors means the class is bound and the
clause the defect lives in is simply not exercised by this campaign. The two are
not the same finding and the ledger never makes a reader guess which one it is.

| Code | Meaning |
|------|---------|
| `DW0540` | **A grant whose removal is a later effect, not its own duration.** A `give-effect` is still live at the moment a `clear-effect` for the same effect fires in the same bundle. Validation-tier (exit 1), `dsl::validate`. The message carries both numbers the author needs — how long the grant runs, and how long the bundle actually needs it for. Prescription: set `seconds` to the span the effect should last and delete the `clear-effect`; a duration expires with no cooperation from anything. `clear-effect` is for effects this campaign did not grant. |
| `DW0541` | **A duration that is not a duration.** A `give-effect`'s `seconds` is zero or past `MAX_EFFECT_SECONDS` (50 000, derived from `MAX_POTION_DURATION_TICKS`), or its `amplifier` is past vanilla's unsigned byte. Validation-tier (exit 1), `dsl::validate`. Zero is the grant that never happens — the unbound-vacuity class as a number; the ceiling is vanilla's own field width, so a value above it is a duration typed in ticks or milliseconds. |

### DW0510–DW0512, DW0891, DW0922–DW0923 — lethal volumes (`compiler::nav` / `compiler::lethal` / `dsl::validate`; spec-0031, DSL v0.10; spec-0062)

This module's rows of a section whose prose is on the [`delvec::compiler::lethal` page](../delvec/compiler/lethal.md#dw0510dw0512-dw0891-dw0922dw0923--lethal-volumes-compilernav--compilerlethal--dslvalidate-spec-0031-dsl-v010-spec-0062).

| Code | Meaning |
|------|---------|
| `DW0512` | **A volume that kills in silence.** A `lethal_volumes[]` entry's `message` is blank. Validation-tier (exit 1), `dsl::validate`. There is no compiler-owned default that could be right for a cliff, a lava pit and an acid pool at once, so a blank wording is refused rather than papered over — a volume that kills while the player learns nothing is the vacuous pass CLAUDE.md names. Prescription: write the line the player reads as they die. |
| `DW0953` | **A gate that cannot stage a lethal volume** (spec-0088). Two shapes, validation tier (exit 1), `dsl::validate`, with no world built: `when: {}` — a stage with no term (an always-live volume is spelled by leaving `when` out) — and a `requires_state` term on a `player`-scoped datum, raised at the check site of `DW0503` with the volume's own code (a volume's liveness is a fact about the place: a term one player satisfies and another does not is a pit that kills one body and spares the next, and the sweep's entity half has no player to read). The message names the volume and the term. Prescription: leave `when` out, or name a flag or a `party`-scoped datum. |
| `DW0956` | **A view aimed past the served view distance** (spec-0091). What a body is farther from than the served radius (`16 × world.view_distance` blocks, floor 10 chunks) is never sent to its client, so the view cannot render. Five shapes of one rule: a declared `view_distance` outside `10..=32` (validation tier, exit 1, `dsl::viewdistance`, on `world` `/content/view_distance`); a site-plan `sightlines[i]` longer than the radius, or a `views[i]` whose `look_at` is farther from its `eye` than it (validation tier, on `site-plan` `/content/sightlines/{i}` / `/content/views/{i}`); a showcase camera of `design/cameras.json` whose subject — the first solid cell on its central ray inside the loaded scene, else where that ray enters it — is past the radius (build tier, exit 3, `view::camera::prove_showcase`, the third shape of the showcase proof); a cutscene keyframe farther from its aim (`look_at`, a static subject, or a moving subject's position at that tick; a shot facing along its travel has no aim and is counted, not judged) than the radius (build tier, exit 3, `nav::check_cutscenes`, after the clip checks). Every message names the distance, what is served, and the prescription: `world.view_distance: <the fewest chunks that serve it>` (one edit to one field, asserted reachable by `tests/view_distance.rs`), or the two ends nearer; a distance past 512 blocks names the ceiling instead. **Binding**: the two `view distance binding:` lines (the world row above). |
| `DW0957` | **An `interact` prop a step fires.** An `interact` objective's `prop.block` is a pressure plate or the tripwire string — a block that tells the player to walk onto it — while the objective completes on a right-click, so the step does nothing. The set is `dsl::stepped_blocks`, the pinned registry's ids that `TrapTrigger::is_trigger_block` accepts for a plate or a tripwire (every `*_pressure_plate`, and `minecraft:tripwire`; the hook is clicked, not stepped), held equal to vanilla's `#pressure_plates` tag plus the string by `tests/stepped_blocks_tag.rs`. Validation tier (exit 1), `dsl::validate` beside the prop's `DW0193`. Prescription: give the objective a block a hand works (a lever, a button), or make the step the act — a `trigger` with `on: step` at an anchor whose cell holds the plate. |

### DW0520–DW0527, DW0880 — trade and the recovery stake (`dsl::validate` / `compiler::stake`; spec-0032)

**There is no price diagnostic here, and its absence is the design.** A price is a
[`Gate`] term — the numeric comparison spec-0031 put in the shared gate rather than
in the verb that first asked for it — so everything that could go wrong with one is
already `DW0500`–`DW0503`: the datum must be declared, must be written somewhere,
must be read somewhere, and must be reachable at the scope the site evaluates at. A
shop that had grown a `price` field would have needed all four rules written a
second time, and the fifth consumer would have needed a fifth copy. `ShopOffer` is
therefore the **seventh gate consumer**, carrying `requires_flags` /
`forbids_flags` / `requires_state` like the other six and nothing of its own;
`crates/dsl/tests/gate_consumers.rs` asserts the positive half from the generated
schema and `crates/delvec/tests/v10_economy.rs` the negative half (no `price`,
`cost` or `compare` field exists anywhere in the shop's types).

`DW0520`–`DW0524` and `DW0527` are declaration and authoring rules and live in `dsl::validate`. `DW0525` and
`DW0526` are the **placement table's** proofs and live in `compiler::stake`,
because where a stake lands is a question about the solved layout — the same split
a lethal volume's `DW0512` and `DW0510`/`DW0511` make. `DW0880` lives there too,
and is about the marker rather than the anchor.

#### A marker is a PLACE, and a death leaves one place

The hardware is one class for the campaign — the `minecraft:interaction` box
tagged `dw_stk` and the glowing `minecraft:item_display` beside it, both summoned
by `stk_place` — and there is one of it at a place however many datums a death
forfeited there. That is forced rather than chosen. The placement table is keyed
on (respawn seat, death region) and never on the stake, so every stake one death
drops resolves to one anchor; and the rule degenerates to *leave it where the
player fell*, a position chosen at runtime that no compile-time separation can
reach. Four `1.0 × 2.0` boxes at one cell are coincident: any pick ray enters them
at the same distance and the client resolves the tie by entity iteration order,
which is exactly what `DW0878` refuses between two authored affordances.

So the place holds one box, and what was left there is counted in the per-player
ledger — where a wager always lived. One advancement fires one `stk_collect`,
which locates the place and offers it to every declared stake in turn, so one
right-click returns every datum that death left. **The place is the box the
player clicked**: the advancement only says some `dw_stk` box was used, so
`stk_collect` tags the player and runs `stk_pick` as every `dw_stk` interaction,
which keeps the one whose last user (`on target`) is that player with the latest
`interaction.timestamp` — the click just made — and the place's position is read
off it; no box, no collection. Choosing the box nearest the player instead would
offer the wagers at whatever place stood closest, so a player reaching past one
stake to click another would collect the near one (under `collect_by: anyone`,
another player's purse) and leave their own standing. `stk_ref` counts live wagers at
that position across every stake, and `stk_gc` — the one function permitted to
retire the hardware (`DW0421`) — deletes a place nobody has a wager at. Each
stake keeps its own forfeit rule, retention policy, collect rule, slots and
message: those are properties of a wager, not of a place.

What a place cannot hold is two answers to what it looks like, and that is
`DW0880`.

#### The placement rule, and why it is a table rather than a search

The rule: *the stake anchor is the point, on the walkable path from the respawn
point in force at the moment of death to the death point under the quest state in
force at that moment, that minimises distance to the death point.*

Read literally that is a runtime search, which ADR-0006 forbids. Read as a function
of three compile-time quantities it is a table, and every quantity already has an
owner: **walkable** is the same `nav::World` the completability proof runs on
(including a lethal volume's impassable cells, so "the near lip of the hazard"
falls out rather than being a second rule); **the quest state** is the DAG-indexed
sealing the region-write model establishes, swept over the seat's own arrivals by
`nav::reachable_under_every_quest_state`; and **the respawn point in force** is
engine state the runtime already keeps in `#cp dw.sys`.

The rule degenerates, which is why there is only one rule: a player who dies on
ground they can walk back to is at distance zero from themselves, so the anchor is
the death point. Only deaths whose position **cannot** host a stake need a row, and
there are exactly two kinds — a death inside a **lethal volume**, and a death on a
block **runtime can remove** (a lift car, a `close-gate` region, a `collapse`
floor; spec-0031's ruling that a stake left on the car would be deleted by the next
ride). Both are boxes, so the runtime lookup is a selector test on the corpse
(`@s[x=…,dx=…]`) rather than a search.

**What a region forbids an anchor is the region's own answer, and the two kinds
answer differently.** A runtime-mutable region acts on BLOCKS, so what it can
destroy is a marker in one of its own cells and cell containment is the whole
rule. A lethal volume acts on BODIES, through a vanilla selector the server
adjudicates against the body's whole hitbox — `@a[x=lo,dx=hi-lo,…]` covers
`[lo, hi + 1]` on each axis and matches on intersection, so it kills a
`metrics::PLAYER_WIDTH`-wide body whose feet cell is one outside the box.
`DeathRegion::holds_no_anchor` therefore refuses a lethal volume the shell of
cells `metrics::selector_reaches_body_in_cell` reports: one cell on every axis,
derived from the body and never chosen. An anchor inside that shell is a place
the delve invites the player to walk back to and then kills them for standing on,
and the bot ladder measured exactly that on the gallery — west pit
`[1,63,2]..[3,67,4]`, anchor `[1,65,5]`, three runs, three deaths at cell
`[3,65,5]` on the walk off it. The harness holds the same rule for the walk that
reaches the anchor (`volumeReachesCell`), so the two are written twice in two
languages and computed differently — a sweep of the cell's extent here, a clamp
to the nearest position there. Neither may drift: each side sweeps the same box
over the same 441-cell grid and states the same 175 reached
(`the_cell_rule_agrees_with_a_swept_body_box`,
`volumeReachesCell agrees with a swept body box, and with the compiler's count`),
so the agreement is a shared number rather than each doc comment asserting the
other's.

#### The three ways a stake can be pulled out from under itself

Two are `DW0526`'s, one is not, and the third is named rather than left silent.

| how | in scope? | why |
|---|---|---|
| **Runtime-mutable ground** — `close-gate`, `set-block`, `collapse`, a shortcut's or a timed gate's seal | yes | the case spec-0031's ruling was written for: a stake left on a lift car is deleted by the next ride. |
| **`fill-region` / `clear-region`** | yes, and it is *the same defect* | a `clear-region` deletes the block a marker stands on exactly as a departing car does. They enter through `QuestEffect::region_write` — the DSL's own answer to "which verbs rewrite a box" — so a later verb of that family is covered by existing rather than by being remembered. |
| **A `teleport`'s `from` box** | **no — a deliberate ruling; closed by `DW0545` one layer away** | a teleport moves *entities*, not blocks: the ground under the marker is untouched, and what moves is the marker itself, away from the position the collecting player's ledger recorded — after which `stk_gc` finds nobody holding a wager there and retires it, taking the wager with it. Different defect, different fix, and not one a box check on this axis could state — `DW0526` is about **footing**, and a marker's position is chosen at RUNTIME, so no compile-time geometry test knows where it will be. |

The teleport case cannot simply inherit the teleport's own `DW0542` either, and the
reason is the shape spec-0031 named when it refused to inherit `lethal_volumes[]`'s
exemption list into a verb that *moves* rather than *deletes*: `DW0542` tests the
affordance authority, which carries compile-time cells, and a stake has none to
offer it. Inheriting it would have produced a green that examined nothing.

**`compiler::stake` carries no teleport rule.** "The teleport exempts engine
machinery" would build a roster into one verb; "the stake ledger survives its
marker moving" would make the stake compensate for a selector that grabbed
something it should never have grabbed. The question is upstream of both —
*what does a region verb select?* — and a marker being a **place** is a property
of the marker, not of any verb. So the class is declared where the marker is
summoned and every box-narrowed selector reads it (`DW0545` above). That a
capability keyed to the object needs no cooperation from this module is the
point rather than a coincidence.

**Note the direction of the conservatism, because it is why this set is not
`Plan::region_events`.** The completability model deliberately drops a non-fill
write fired from an **optional** root — an optional firing may fill, never open —
because a route proof must not lean on a clear the party might never trigger. This
set needs the opposite: a `clear-region` in an `on_death` bundle the party may
never reach is still ground a stake must not stand on, because if they do reach it
the marker is gone. Same geometry, opposite direction, so the two lists cannot be
one.

**Two conservative simplifications, recorded rather than hidden.** spec-0032
already records one — *reachable under the quest state* stands in for *explored*,
which the engine does not track. The implementation adds a second: nothing
observable at runtime says which point of a respawn point's DAG span a death
happened at, so the reachable set used for a seat is the **intersection** over
every sealing configuration that can hold while that seat is in force. The anchor
is then reachable under all of them, which is strictly stronger than the rule as
written and needs no runtime discriminator for quest state at all. A campaign with
no `close-gate` has exactly one configuration and pays nothing.

The span swept is every critical-path arrival from the seat's own firing step to
`critical_path.len()` inclusive — **the arrivals past the last objective
included**, each of them carrying every objective on the path as fired (see *Every
arrival is keyed* above). A seat's configurations therefore hold the world as the
party leaves it at the end of the delve, which is the one a death after the last
beat respawns into.

#### What the runtime tiers can and cannot witness — stated, not implied

**A PackTest fake player is permanently undamageable and cannot die** (measured
twice, independently, on the pinned toolserver). So that tier cannot witness a
player death, and therefore cannot prove the edge from a death to a stake being
placed. Two templates are generated and both are honest about what they cover:
`v10_shop_purchase` drives an offer handler as its own dummy and proves the debit
and the refusal; `shop_enchanted_stack_<i>_<j>`, one per offer that hands over an
enchanted stack (spec-0075), empties its dummy, drives the offer's gate and each
stack's own `when` open as the buyer, probes the inventory before the purchase
(must read 0) and after (must read 1) for the item carrying exactly those
enchantments in the component the item writes them to; `v10_stake_<id>` drives `stk_drop_<id>` and the campaign's real
`stk_collect` and proves that the declared share leaves the purse, that a marker
really stands where the drop put it, that collecting returns **exactly** what was
taken, and that a second collection in the same breath returns nothing more; and
`v10_stake_two_datums` drives two forfeits from one position and proves that the
two leave **one** `minecraft:interaction` and one display, that one press returns
both datums, and that the place then retires. No template is generated for the
death edge itself — a template that bound to nothing and reported green is the
vacuity CLAUDE.md names, and it is worse than an absence because review cannot
see it.

**The death itself is a bot-tier proof.** spec-0032's acceptance criterion 9 —
*die in a lethal volume, respawn, walk back, collect, with the amount asserted* —
is carried by the harness's `death-loop` stage: it walks a real client into every
declared lethal volume, dies there, and asserts the volume's wording, the declared
forfeit, the stake's presence at the table's own anchor, the walk back, an exact
restore under a double right-click in one tick, the retirement of the collected
hardware, and the respawn seat — all against `validation/death-plan.json`, never
against the emission. It runs on every bot run whose build ships
`validation/death-plan.json` (`DELVEWRIGHT_DEATH_LOOP=0` skips it, and the report
records the skip); no CI job runs the bot ladder. Over the economy fixture:
`EULA=TRUE validation/bot-run.sh --project dw-death-loop --output
./delve-output-economy` over a build of `crates/delvec/tests/fixtures/economy`.

**The die-retry loop** — death → respawn at the governing checkpoint → walk back
→ re-engage — binds only where an ARMED checkpoint stands before a mandatory
encounter. `crates/delvec/tests/fixtures/die-retry` is the smallest campaign that
lets it bind, `crates/delvec/tests/die_retry_fixture.rs` holds it to that shape,
and the run report's `die_retry_binding` makes the zero loud on every other build.

**The first death is seeded.** `dw.death_seen` and `dw.death_ack` are `dummy`
objectives, so a player who has never died has no score in either — and `execute
if score @s A > @s B` with B unset does not fire (measured on the pinned 1.21.11
server; `scoreboard players add <e> <obj> 0` is what creates the entry at zero).
`cp_respawn_check` therefore seeds each acknowledgement it reads, ahead of the
comparison, so `on_death` and the checkpoint respawn dispatch fire on a player's
FIRST death; `v06_checkpoints` and `v10_on_death` assert the ORDER, not merely
the presence.

Binding (playtest-methodology rule 1): a campaign with a stake emits
`validation/stake-gate.json` — stakes declared, respawn seats and death regions the
table is keyed on (and how many of those regions are lethal volumes), quest-state
configurations enumerated, rows proved, distinct anchors resolved, runtime-mutable
cells excluded, stranded cells found, and how many of the declared stakes can
actually leave a marker. A campaign with no stake emits **no file
at all**, so a file that exists and reports zero is a finding rather than an
absence.

| Code | Meaning |
|------|---------|
| `DW0520` | **A stake that is not a personal wager.** A `stakes[]` entry's `state` names a datum the campaign never declares, or one declared `party`-scoped. Validation-tier (exit 1), `dsl::validate`. The scope half is stated as a rule because it is the multiplayer decision most likely to be made by accident: a shared purse turns a teammate's death into a penalty on everyone and nothing in the JSON would say so. Prescription: declare the datum `player`-scoped, or point the stake at one that is. |
| `DW0521` | **`drop-stake` names no declared stake.** Validation-tier (exit 1), `dsl::validate`. Prescription: declare it in `stakes[]`, or fix the id. |
| `DW0522` | **A stake nothing ever drops.** A declared stake that no `drop-stake` effect anywhere in the campaign leaves: its forfeit rule, its retention policy and its whole compile-time placement table describe a mechanism no beat can fire. Validation-tier (exit 1), `dsl::validate`. The vacuity rule `DW0502` states for a datum with no reader, applied to a whole feature. Prescription: drop it from a beat (`on_death` is the usual one), or delete the declaration. |
| `DW0523` | **A shop button that cannot answer.** A `shops[].offers[]` entry with no `effects` — drawn, pressable, inert — or a `shops[]` entry with no offers at all, which is worse than an empty shop: vanilla's 1.21.11 dialog codec rejects an empty action list at pack load. Validation-tier (exit 1), `dsl::validate`. A **refusal counts as an answer**, so an offer whose only effect is a `narrate` gated on `at-most <price − 1>` satisfies it — which is exactly the shape spec-0032 asks for. Prescription: give the offer effects, or delete it. |
| `DW0524` | **A forfeit above the whole purse.** A `forfeit` of kind `proportion` whose `percent` exceeds 100. Validation-tier (exit 1), `dsl::validate`. Prescription: 0–100, or `{"kind": "all"}`. |
| `DW0527` | **A comparison read after the bundle changed what it compares.** An effect's `requires_state` names a datum that an earlier effect in the same bundle writes **behind a gate on that same datum** — so the comparison is made on the far side of the boundary the bundle just tested. Warning-tier (exit 0), `dsl::validate`. Found in the emitted output of this feature's own first shop: written "purchase, then apology", buying your LAST coin debits it and the `at-most` apology — evaluated after the debit — then holds too, so the player is charged AND told they cannot afford it. The fix is always local: put every reading effect ahead of the write. An **unconditional** write followed by a comparison is deliberately NOT diagnosed — `set-state toll 0` and then a door gated on `toll at-most 0` is the ordinary sequenced idiom and plainly means the value the bundle just produced. **Its scope is ONE bundle's own effect list, and that is what it does not cover**: a write and a read four beats apart are two bundles, so a `clear-state` that empties a datum a later objective's gate depends on is invisible here. `DW0879` is that question, asked over the path rather than over a list. Prescription: reorder, or gate on something this bundle does not change. |

### DW0185 — untranslated player-visible literal (`compiler::emit`; error; exit 3)

| Code | Meaning |
|------|---------|
| `DW0186` | (i18n v2 addendum) A campaign l10n sidecar defines a key in the reserved `delvewright.` **chrome** namespace. Those are the engine's own on-screen strings — `New objective: `, `Choose your class`, a bonfire's default labels — owned by the compiler, shipped translated with it, authored by no campaign; a sidecar row under that prefix would be written into the language file and silently replace product chrome for that language. `DW0181` also flags it as an orphan; this names the reason. |
| `DW0187` | (i18n v2) An l10n sidecar row was translated from English the campaign no longer holds: its `source` entry differs from the key's canonical English (or names a key the sidecar does not translate). The translation is present, applied and wrong, and no key-set check can see it — `DW0180`/`DW0181` compare key SETS and a rewritten line moves no key. Load-bearing for entity display names, whose key belongs to the first site declaring a given text: renaming one body hands its key to another, so the stale row is not the one the author edited. Fix by re-translating the key and updating its `source` — `tools/creator/i18n-translate.py` does both. |
| `DW0188` | (i18n v2) An l10n sidecar records `source` provenance for only some of its rows, or none, so `DW0187` cannot see the rest. **Warning tier**, stating the unguarded row count: `source` is optional. The count exists so an unadopted sidecar is a reported number on every run rather than a silence that reads like a pass. Adopt by re-running `tools/creator/i18n-translate.py` — it records provenance for rows it already has, and retranslates nothing. |

#### The branch artifacts (validation metadata)

Two outputs, emitted only for a campaign that declares `branch_points`, both pure
functions of the campaign document and therefore byte-identical across builds
(ADR-0006). They live under `validation/` and are hashed into `manifest.json`
like `critical-path-waypoints.json` — **never** part of the shipped datapack.

- **`validation/branch-plan.json`** — per branch: its id, the alternative taken
  at each point, its flag assignment (`set` / `unset`), where its fork opens,
  what it `leads_to`, whether it is reachable, the **dialogue choices that enter
  it**, the endings it reaches, its **critical path computed under that branch**
  (the flow-level `quest` / `objective` / `talk_option` step list), and the names
  of its two companion files (`chronicle`, `path` — `path` is `null` exactly when
  the branch is unreachable). This is what the harness scripts a per-branch run
  from.
  An **entry choice** carries `npc`, the option's 1-based index across that NPC's
  tree, and — the field the harness actually uses — the `command` that takes it:
  `/trigger dw.dlg_<npc> set <n>`. A 1.21.11 dialog button is drawn by the
  CLIENT, so no bot can click one; every option the compiler emits is backed by
  the trigger line the button itself runs, and chatting it is the player-legal
  primitive the button stands for — the same substitution the exported critical
  path makes for `talk-to` steps (spec-0002). The command is
  emitted rather than left to the harness because reconstructing it means
  reproducing `safe_local`, i.e. game logic in a harness that holds none. The
  option index is resolved against **the tree of the NPC the step's own
  `talk-to` names** — the same ordinal in another NPC's tree is a different
  option of a different speaker.
- **`validation/branch-path-<branch>.json`** — one branch's **executable** path,
  emitted per reachable branch, in the ordinary `critical-path.json` contract
  (`format_version` 2, the same steps, the same `transport`/`sneak`/
  `cutscene_seconds` markers, the same spliced bonfire `rest` steps). Built by
  the *same* `plan::build_critical_path` the exported path is built by, driven by
  the playthrough of the world that realizes the branch (`Plan::branch_critical_path`)
  — so the branch a campaign already exports gets a **byte-identical** file, and
  "branch coverage" is coverage of the contract the ladder already proves rather
  than of a second, less-tested one. The branch's scripted dialogue choices are
  *inside* it: each `talk-to` step carries the `/trigger` line of the option
  belonging to that branch — and, since `cast::station` reads the flag state THIS
  branch holds at that step, its **position** as well: two branches that stage the
  same NPC at different anchors get two different cells for the same beat. A bonfire's `fire_step` (an index into the exported
  path) is translated onto a branch path through the **objective** its firing
  beat names, because a fire is armed by a beat and not by a position; a beat
  that does not happen on a branch arms nothing there. A step's `transport` is a
  **contract with the datapack**, not just a harness hint: emission carries every
  branch-only crossing as a flag-gated `teleport` in that objective's
  `complete_<obj>` bundle (`DW0494` above). Not emitted for an
  unreachable branch — there is no world that plays it, and `DW0482` has already
  failed the build. Each reachable branch also gets its own
  `validation/branch-waypoints-<branch>.json` ("World / build output" above), whose legs
  follow the branch path's own step order.

**A branch path is NAV-proven as well as FLOW-proven.** `DW0204`'s replay and the
`DW048x` proofs judge a branch's *story*: its steps are ordered, its gates
satisfied, its cast selected, its ending reached. The **geometry** proof is a
per-branch `DW0311` (`nav::check_branch_path`): every walked leg of every
reachable branch path is routed over the assembled world under the branch's own
causal gate seals, so a route that crosses a gate only a *sibling* branch opens
is refused at build. `validation/branch-runs.sh` plays each branch on the
dynamic layer.
- **`validation/branch-chronicle-<branch>.md`** — the 流水账: every reachable
  node's `happening` line in the order the compiled graph plays them, readable
  start to ending, followed by the undated ambient beats and the endings reached.
  The dated account carries four kinds of line — `quest`, `objective`, `choice`
  and `effect`. A **`choice` line is the dialogue option this branch takes to
  complete a `talk-to` beat**, and it is where a fork's divergence lives: a
  `DialogueEffect` carries no `happening` of its own, so the option is the only
  thing on that side of the campaign that says what the choice did to the story.
  The line names the option's own NPC, the node it stands in and its 1-based
  ordinal — `npc/marshal dlg/marshal-read#2` — so a citation table can point at
  it. The ordinal is resolved against **that NPC's tree**, the scope
  `flow::flatten_trees` assigns it in and `plan::plan_npc` emits it in; the same
  ordinal in another tree is a different option of a different speaker. An option
  carrying no `happening` contributes no line, and the compiler never invents one.
  The SKELETON (ordering, reachability, which nodes appear) is derived machine
  truth — it is exactly the order `Flow::journal` replays, which is exactly the
  order `Flow::replay` proves; only the flesh (each line's text) is authored,
  node-locally. This is the **decompilation principle** (spec-0025): the
  generation workflow is natural language → design doc → DSL, and whether the DSL
  matches the design is not something an LLM can check by simulating compilation
  in its head — so the compiler compiles the DSL *back* into natural language and
  the reviewer compares NL against NL. Narrative incoherence becomes a readable
  contradiction in sequence.

### DW07xx — asset, render and authoring tooling (spec-0007; `delvec` subcommands)

This module's rows of a section whose prose is on the [`delvec::admit::diag` page](../delvec/admit/diag.md#dw07xx--asset-render-and-authoring-tooling-spec-0007-delvec-subcommands).

| Code | Tool | Meaning |
|------|------|---------|
| `DW0741` | `delvec prefab`, `delvec validate` | An asset's licence is outside the ADR-0013 allowlist, or its record lacks a field the rule for that asset requires: a catalog card, or a `world.textures[]` row's `license` (`dsl::license::image_license_refusals` — `original` needs `source: original`, any other licence a `url`, `CC-BY-*` an `attribution`). Declared once, `dsl::codes::LICENSE_REFUSED`. |
