# `delvewright_dsl::diagnostic`

The reference page for `crates/dsl/src/diagnostic.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW01xx — validation (`dsl`; severity error; exit 1)

| Code | Meaning |
|------|---------|
| `DW0100` | Document does not conform to its stage schema (unknown field / wrong type / missing required field, incl. persona). Parse-time. |
| `DW0102` | The document's `dsl_version` is not the one this engine accepts, `0.38.0`; the message names it (ADR-0024). Raised per stage document by `dsl::envelope::envelope_checks`, and for an l10n sidecar under `DW0180`. |
| `DW0110` | Malformed id syntax (not kebab-case / wrong-missing prefix). **The message names the form of the type it rejected**, derived from that id type's own `PREFIX` — `` `dlg/<kebab>` `` for a dialogue node, `` `class/<kebab>` `` for a class — rather than restating the general rule beside three fixed examples. One macro, `dsl::ids::id_syntax!`, is the single path every id type's syntax refusal goes through, so the answer comes from the type at every site: `ids::syntax_form`. The per-section refusals that spell their own prefix by hand (`wave/`, `trigger/`, `trap/`, `shortcut/`, `ambush/`, `timed-gate/`, `loot/`) are the same fact copied, which is why the general path did not have it. |
| `DW0111` | Duplicate id in namespace (incl. two dialogue trees for one NPC). |
| `DW0112` | Dangling / forward / undeclared reference (incl. persona relationship to unknown NPC). An **area** reference resolves against the campaign's placement authority and its prescription comes from there (see the remediation contract above): a `Prefabs` campaign is told to declare it in stage-1 `world.areas`, a `SitePlan` campaign is told its one area is `area/site` and explicitly told NOT to declare it, and a campaign with neither is told both branches. Two sets resolve an area id — `dsl::world::declared_area_ids`, which npc and quest-plan references read, and a stage-7 edit script's own for its `batches[].area` (`dsl::world_edits`) — and both prescribe from the same authority. |
| `DW0142` | Anchor not provided by the area's bound prefab — or, on a site-plan campaign, not among the names the derivation places. The predicate is `AnchorProviders`; the prescription is the placement authority's (see the remediation contract above), so a derived map is given the synthesized vocabulary instead of being sent to prefab metadata it does not have and told not to invent a name it is required to invent. **Silent while the derived vocabulary is unknowable** — a site-plan campaign with no `layout-graph.json`, where `DW0824` is the finding: `AnchorProviders` contributes no set for `area/site` and the whole campaign defers, the same path a `prefab_pool` takes. |
| `DW0143` | Item id not in the pinned 1.21.11 registry (kit / `collect` / `interact.requires_item` / `give-item`). |
| `DW0170` | `kill`/`spawn-wave` references an undeclared `wave/<id>`. |
| `DW0172` | `requires_flags` references a flag no `set-flag` produces. The producer scan descends every nested effect list (`sequence` steps, `on_respawn`/`on_caught`/`on_arrive`), so a `set-flag` nested in a timeline still counts as a producer (no spurious fire). |
| `DW0173` | Wave-mob `entity` is not a known vanilla entity id. |
| `DW0180` | l10n sidecar absent / inconsistent envelope / under-covers inventory (also if `en` is declared). Compiler-level. The inventory it demands coverage of spans **every effect root emission can lower** — including `traps[].payload` and a dialogue option's `set-checkpoint` `on_respawn` bundle, so a string in either is translated. |
| `DW0181` | l10n sidecar has an orphan key (over-coverage). Compiler-level. |
| `DW0182` | A player-visible string — authored English (the whole l10n inventory) or any sidecar translation — contains the reserved completion-marker sigil `[dw:complete`. That chat sequence is the validation bot's completion oracle (§4 "The completion-marker channel"); content carrying it could forge a passing critical-path step, so the sigil is **reserved**, not merely discouraged. Reword the line. |
| `DW0183` | (i18n v2, spec-0029) A player-visible string — authored or translated — contains a character from the reserved private-use block `U+E000..U+F8FF`. That block is how the compiler carries an l10n key from the stage docs to the text component the string is emitted into (`dsl::l10n::TR_SIGIL`), so content carrying it could impersonate a translation tag; it also has no glyph in any Minecraft font. Remove the character. |
| `DW0184` | (i18n v2, spec-0029) A declared `world.languages` code does not resolve to a language file the **pinned client actually loads** (`dsl::mclang::CLIENT_LANGS`, derived from Mojang's 1.21.11 asset index), so its `assets/delvewright/lang/<code>.json` would sit under a filename no client ever asks for and the language would ship invisible. Also fires on an ambiguous bare code (`zh`, `sr`, `be` — several regions, no `<lang>_<lang>`), because guessing the region is how a language ships invisible. Use a code the client loads. A language is never silently dropped. |
| `DW0190` | An image id a campaign declares is malformed — a body's `skin.texture_id`, or a `world.textures[]` row's `id` — or two `world.textures[]` rows share an `id` (spec-0084 §6.4). A skin's `texture_id` names a file, and two bodies may wear it (spec-0097 §4.3). |
| `DW0192` | Wave-mob `effects[].effect` not a known 1.21.11 status-effect id. |
| `DW0193` | `set-block`/`interact.prop` block id not a known 1.21.11 block id (base id checked; a malformed blockstate suffix `id[…]` — unbalanced `[]`, empty, or non-`key=value` tokens — reuses this code). |
| `DW0196` | Area `lighting.min_light` out of range (must be 1..=14). spec-0010. |

### DW03xx — build / solver / nav (`compiler`; error; exit 3, `stage:"build"`)

This module's rows of a section whose prose is on the [`delvec::compiler::nav` page](../delvec/compiler/nav.md#dw03xx--build--solver--nav-compiler-error-exit-3-stagebuild).

| Code | Meaning |
|------|---------|

### DW043x — geometry & container proofs (stair orientation; spec-0021 loot; `collect` container adoption)

This module's rows of a section whose prose is on the [`delvewright_dsl::loot` page](loot.md#dw043x--geometry--container-proofs-stair-orientation-spec-0021-loot-collect-container-adoption).

| Code | Meaning |
|------|---------|
| `DW0432` | A **positional container fill** declares more stacks than a vanilla chest or barrel has slots (27): a `loot` entry's `items`, or a `collect` whose own stack plus `fill_count` padding exceeds 27. Validation-tier (exit 1). Slots are assigned positionally, so every stack past the 27th would be dropped without a word. Prescription: split the contents across more than one container, or lower `fill_count` — a container that reads full does not need to overflow. |

### DW0898 — a piece where the body shows it (`dsl::equipment`; error; exit 1)

The server stores all eight equipment slots on every living entity and a
`/summon`'s `equipment` compound is read whole; the client draws a slot only
when the renderer registered for the entity type carries a layer that draws
it. A piece in a slot its body does not draw is NBT nobody sees.

| Code | Meaning |
|------|---------|
| `DW0898` | **A piece is declared where the pinned game will not show it on that body** (spec-0067). `dsl::equipment::fit_checks`, called from `dsl::validate`, validation tier (exit 1), once per piece of every `equipment` a wave mob or an actor declares. The body is the entity the puppet wears (`BodyRef::worn_entity`: a skinned actor is a `minecraft:mannequin`). One code, three shapes, every shape that holds named in one message: **the body does not show it** — the body table has no entry for that entity type at that slot for the piece's kind (a chestplate on a horse, a sword in a creeper's hand, an iron helmet on a villager, whose head draws an item and not armour, any piece on a body that is not a living entity); the message lists the slots the body draws with their kinds, and for a hand it names `attributes` as the way to write a held weapon's number; **the wrong slot** — the item's `equippable.slot` is another slot (a helmet in `legs`, horse armour in `chest`); the hands take any item and are exempt; **the wrong body** — the item's `allowed_entities` exclude the body (a saddle on a zombie, horse armour on a skeleton horse), with every admitted entity spelled out, tags expanded. **Not judged**: an item with no `equippable` component in a slot the body draws for an `item` (a carved pumpkin or a block on a zombie's head). **The kinds**: `armour` (an asset and a `head`/`chest`/`legs`/`feet` slot, `HumanoidArmorLayer`), `wings` (an asset with a `wings` layer), `animal` (an asset with a `*_body`/`*_saddle` layer), `item` (no asset, or no component). **The two tables**: the body table (`crates/dsl/data/entity-slots-1.21.11.json`, 92 rows over the 157 pinned entity types, each naming its renderer; groups: 16 humanoid six, 5 head item and both hands, 2 head item and main hand, 4 main hand only, 2 both hands only, 5 body and saddle, 6 saddle only, 4 body only, 48 nothing; the evoker, illusioner, vindicator and panda draw their hand only in a state and count as showing it) is authored from the pinned client's renderers and held by `crates/delvec/tests/equipment_tables.rs` to `#minecraft:can_equip_saddle`, to the body items' allowed lists plus the skeleton horse, and to the equipment assets' 15 body/saddle layer types; the item table (`crates/delvec/data/item-equippable-1.21.11.json`, 84 items, `tools/maintenance/extract-item-equippable.py`) is injected through `ItemRegistry::equippable`, and a registry that does not know an item judges nothing. **Binding**: `equipment binding: B body(ies) dressed over K entity type(s), P piece(s) declared over S slot(s) in use, F piece(s) with a registry-declared slot, A with an allowed-entity list, R refused (DW0898).` on every run of the validation funnel, zeroes included. **Emission**: the two new keys ride the component-era `equipment` / `drop_chances` compounds in `EquipSlot::ALL`'s order, the drop-strip line zeroes all eight, and the generated `v06_actor_equipment_<body>` PackTest asserts every filled slot on the puppet and on the unleashed twin — the live proof that the pinned server stores each key. **Prescription**: dress another body, choose a piece the body draws, move the piece to its declared slot, or change the entity. Gallery probe: `a-chestplate-on-a-horse`. |

### DW046x — the NPC scene ledger (`compiler::cast`; spec-0020)

This module's rows of a section whose prose is on the [`delvec::compiler::cast` page](../delvec/compiler/cast.md#dw046x--the-npc-scene-ledger-compilercast-spec-0020).

| Code | Meaning |
|------|---------|

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

### DW048x — branch-complete narrative verification (`compiler::branch`; spec-0025)

This module's rows of a section whose prose is on the [`delvec::compiler::branch` page](../delvec/compiler/branch.md#dw048x--branch-complete-narrative-verification-compilerbranch-spec-0025).

| Code | Meaning |
|------|---------|
| `DW0931` | **A celestial time whose shape states nothing a sky can show** (spec-0081 §6). One rule about one value's shape — a time is one body, one position and, where the moon shows, one phase — broken four ways, each raised where the value is entered: (1) the object names **neither or both** of `sun` / `moon` (the `DW0160` exclusivity shape, under this code because the object is a time); (2) a **`phase` where the moon is below the horizon** — `{"sun": "high", …}`, `{"moon": "below", …}`, `{"sun": "just-risen", …}` — the message naming the moon's altitude there; (3) **`world.time` with no `phase` where the moon is at or above the horizon** — the party sees it, so nobody may leave it to a default; the message lists the eight names; (4) a **`phase` on a `set-time`, a design row or a camera equal to the world's** (a restatement: a phase left out is the world's). A phase outside the eight names and a position outside the six are `DW0100`, not a fifth shape. `delvewright_dsl::celestial::shape_findings` is the one statement of the four rules: `delvec validate` applies it to the world, every `set-time` at every effect root and depth, every dialogue `set-time` and every design row (`celestial::check`, validation tier, exit 1), and the design gate applies it to every camera's celestial `sky.time` (`compiler::design::check`). Binding: the `clock:` lines and the `clocks:` summary every run prints (stage 1, *Celestial time*). **Every move it names is reachable** (`remedy_reachability.rs::dw0931_every_named_move_validates`). Prescription: NAME ONE BODY; REMOVE `phase` where nobody can see the moon; STATE `phase` on the world's time where the moon is up; REMOVE a `phase` that restates the world's. |

### DW050x — runtime state (`dsl::state`; spec-0031)

This module's rows of a section whose prose is on the [`delvewright_dsl::state` page](state.md#dw050x--runtime-state-dslstate-spec-0031).

| Code | Meaning |
|------|---------|
| `DW0500` | **An undeclared datum.** A `state/<kebab>` reference — in a `requires_state` comparison or in one of the three verbs — names a datum the stage-5 `state` list does not declare. Unlike a flag, whose set is exactly what some `set-flag` produces, a datum is declared because its scope and its initial value are facts no use site can supply: an undeclared reference is not "a datum that happens to start at zero", it is a datum with no defined multiplayer semantics at all. Prescription: declare it, or fix the id. |

### DW0510–DW0512, DW0891, DW0922–DW0923 — lethal volumes (`compiler::nav` / `compiler::lethal` / `dsl::validate`; spec-0031, DSL v0.10; spec-0062)

This module's rows of a section whose prose is on the [`delvec::compiler::lethal` page](../delvec/compiler/lethal.md#dw0510dw0512-dw0891-dw0922dw0923--lethal-volumes-compilernav--compilerlethal--dslvalidate-spec-0031-dsl-v010-spec-0062).

| Code | Meaning |
|------|---------|
| `DW0953` | **A gate that cannot stage a lethal volume** (spec-0088). Two shapes, validation tier (exit 1), `dsl::validate`, with no world built: `when: {}` — a stage with no term (an always-live volume is spelled by leaving `when` out) — and a `requires_state` term on a `player`-scoped datum, raised at the check site of `DW0503` with the volume's own code (a volume's liveness is a fact about the place: a term one player satisfies and another does not is a pit that kills one body and spares the next, and the sweep's entity half has no player to read). The message names the volume and the term. Prescription: leave `when` out, or name a flag or a `party`-scoped datum. |
| `DW0956` | **A view aimed past the served view distance** (spec-0091). What a body is farther from than the served radius (`16 × world.view_distance` blocks, floor 10 chunks) is never sent to its client, so the view cannot render. Five shapes of one rule: a declared `view_distance` outside `10..=32` (validation tier, exit 1, `dsl::viewdistance`, on `world` `/content/view_distance`); a site-plan `sightlines[i]` longer than the radius, or a `views[i]` whose `look_at` is farther from its `eye` than it (validation tier, on `site-plan` `/content/sightlines/{i}` / `/content/views/{i}`); a showcase camera of `design/cameras.json` whose subject — the first solid cell on its central ray inside the loaded scene, else where that ray enters it — is past the radius (build tier, exit 3, `view::camera::prove_showcase`, the third shape of the showcase proof); a cutscene keyframe farther from its aim (`look_at`, a static subject, or a moving subject's position at that tick; a shot facing along its travel has no aim and is counted, not judged) than the radius (build tier, exit 3, `nav::check_cutscenes`, after the clip checks). Every message names the distance, what is served, and the prescription: `world.view_distance: <the fewest chunks that serve it>` (one edit to one field, asserted reachable by `tests/view_distance.rs`), or the two ends nearer; a distance past 512 blocks names the ceiling instead. **Binding**: the two `view distance binding:` lines (the world row above). |

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
