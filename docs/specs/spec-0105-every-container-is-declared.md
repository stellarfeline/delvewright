# spec-0105: Every container is declared — a container is loot, a device or scenery; a loot fill is never empty; when any container is scenery, the arrival says so in the campaign's words before a body can reach one

- **Status**: Proposed
- **Ground**: written against engine `5961145d8` (`origin/main`), read only — `Loot`, `LootItem` and `loot_checks` in `crates/dsl/src/loot.rs`; `container_slots`, `is_container`, `container_anchors` and `ContainerBinding` in `crates/delvec/src/compiler/loot.rs`; `class_apply_fns` in `crates/delvec/src/compiler/emit/class.rs`; `Verb::Narrate` / `NarrateStyle` in `crates/dsl/src/quest/effect.rs` and its lowering in `crates/delvec/src/compiler/emit/effect.rs`; `QuestsContent` in `crates/dsl/src/quest/mod.rs`; `block_entities_omitted` in `crates/delvec/src/compiler/view/world.rs`; the admission allowlist `crates/delvec/src/admit/allowlist.rs`; the `loot[]`, `collect`, `guidance`, `interact`, `triggers[]`, `narrate` and class-select rows of `docs/reference/compiler.md`. The pinned 1.21.11 game, read from the dedicated server jar `versions.toml` pins (sha256 `f83b8e09…dd1726`), whose bundled `META-INF/versions/1.21.11/server-1.21.11.jar` has sha256 `ec47239a8de246335e1d54f6ac319bd35641778eb4b6a6da06372840d02fcebc`, disassembled with `javap -c -p` (JDK 21.0.3) and named through Mojang's official 1.21.11 server mappings (`server.txt`, sha1 `5621e9253f05fd57872bbe7f8ddf5f9a7d525955`, from the piston-meta version document sha1 `718cd84b…6c5c4`); the registries summary `registries/data.min.json` the vendored data derives from (`crates/delvec/data/PROVENANCE.md`, sha256 `7efb1849…cf28f9`, re-fetched and matched). The campaign the finding comes from, read at its own branch of the content repository.
- **What it is for**: a container the player opens is a promise. In the campaign that raised this, every barrel and chest in reach was empty, and a player spent the first hour opening them. The engine fills a container a `loot` entry names and knows nothing about the containers no entry names, so it could not say that the campaign had declared none of its eleven to hold anything. After this spec, every container in the built world is one of three things — **loot** (a fill stands in it), a **device** (opening or striking it is what a trap, trigger or objective does), or **scenery** — and a campaign with scenery containers says so once, in its own words, at the moment the party arrives, before any body can stand in front of one. The mechanism is general: a stripped town, a looted crypt, a warehouse where only the marked crate matters.
- **Research**: §2 is this spec's research record. Each rule is marked **cited** (a reading of the pinned bytes or data, a reading of the tree, a constitution rule, a named source) or **authored** (this spec chooses). Nothing below was run on a server; what the bot tier must observe is named in §5.3.
- **Numbers**: no ADR. **Three DW codes**, handed for this spec — **`DW1018`** (the containers built and the containers declared disagree, §6.1, build tier), **`DW1019`** (a container declaration that holds nothing, §6.2, validation tier) and **`DW1024`** (scenery containers arranged as a storeroom, or a container that matters among them, §9.3, build tier). A fill into a cell that is not a container stays `DW0431` / `DW0438`; two fills of one container stay `DW0435`; a notice's sound takes `DW0326`, its `art` text `DW0328` / `DW0330`: no second code for a rule that exists. The surface changes, so the implementation takes **`dsl_version` 0.43.0** (verified free across every remote ref); it numbers a surface and promises nothing.
- **Non-goals**: making a scenery container unopenable (§2.3 finds the vanilla primitive and §8 says why it is not used here); a keyed container — a chest that opens to the party member holding the right item — which the same primitive is for and which is a spec of its own; a per-container decoration list (§3 says why one declaration is the right number); loot tables or any randomised fill (ADR-0006); the contents of a container a prefab's `.nbt` carries (the block map carries blockstates only, §2.4, so a piece cannot smuggle a filled chest in); a hover prompt or highlight on an openable container (the pinned client draws none, and a glowing marker is the defect `docs/reference/game-writing.md` §8 L1 names).

## 1. The defect, and the object it belongs to

**Cited** (the tree at `5961145d8`): `loot[]` fills **a container the prefab already placed**, positionally, by anchor (`compiler.md`, `loot[]` row); `container_anchors` enumerates the anchors whose cell holds a container, so the compiler can offer a remedy — but only anchored cells, and a program's `scatter` or `fill` places a barrel at a cell no anchor names. Nothing enumerates the containers no fill claims. `Loot.items` is `Vec<LootItem>` and `loot_checks` refuses a fill of 28 stacks (`DW0432`) but not a fill of zero, so a declared container may be declared empty. The `/new-delve` skill asks, in prose, that every reachable container hold consumables or props (`references/quest-capabilities.md`, *Furnish the containers*), which is the UNRUN shape: a sentence nothing invokes.

**Cited** (the campaign, at its branch): its `quests.json` declares no `loot` and its `guidance` shows announcements; eight of its detail programs name a barrel or a chest — `chapel-crypt`, `coyle-house`, `customs-house`, `fish-market`, `harbour-office`, `net-lofts`, `stomach`, `whalers-shed` — eleven mentions in all, counted on the programs' text (a mention is one verb argument, which may place one cell or many; the built count is what §5.1 measures). Every container it places is therefore scenery, and nothing told the player so.

**The object the declaration belongs to is the container, as a class.** The engine already treats the container as authored hardware the compiler fills and never places (spec-0021); a trap's trapped chest (`DW0917`) and a `collect`'s adopted container (`DW0438`) are the same class seen from two verbs. "Is this container loot, a device or scenery" is a question about every member of the class in the built world, so it is answered once, by a census over the assembled model, against the declarations that claim its cells — never by a flag on `loot[]` or a list keyed to the verb that first needed it (CLAUDE.md, *a capability belongs to the object class it acts on* — **cited**).

## 2. Research record

### 2.1 What a container is, in the pinned game

**Cited — the registry.** `registries/data.min.json` (the mcmeta `1.21.11-summary` republication of Mojang's generated reports; sha256 matches `PROVENANCE.md`) lists **49** `block_entity_type` entries; the static initialiser of `BlockEntityType` in the server jar registers the same 49 names, read back through the mappings. The registry names the kinds; it does not say which open to a click.

**Cited — the class hierarchy, bytecode.** The block entities that open an inventory screen to a player's click are exactly the subclasses of `BaseContainerBlockEntity`, which implements `Container`, `MenuProvider` and `Nameable` and carries the lock (§2.3): **`chest`, `trapped_chest`, `barrel`, `shulker_box`, `hopper`, `furnace`, `blast_furnace`, `smoker`, `brewing_stand`, `dispenser`, `dropper`, `crafter`** — twelve kinds (the `shulker_box` type stands behind seventeen block ids, one per colour and the undyed one). Their slot counts, read from each constructor's `NonNullList.withSize` argument: chest, trapped chest, barrel and shulker box **27**; hopper and brewing stand **5**; dispenser, dropper and crafter **9**; furnace, blast furnace and smoker **3** (input, fuel, result, in that slot order).

**Cited — what is not a container, by the same reading.** `decorated_pot` (`RandomizableContainer` + `ContainerSingleItem`, no `MenuProvider`: a click with an item inserts it, a click with nothing does nothing visible); `chiseled_bookshelf` and the 1.21.11 `shelf` (`ListBackedContainer`, no menu: a click takes or places one item in one slot); `lectern` (`MenuProvider`, but its menu is a reading screen over one book, not a storage inventory); `campfire` (`Clearable`, no menu); `jukebox` (`ContainerSingleItem`); `ender_chest` (no inventory of its own — it opens the clicking player's private store, which the compiler can neither fill nor empty); `beacon` and `enchanting_table` (menus with no storage). None of these shows a player an empty box, so none can cost the time the finding measures. **Authored**: the spec's container class is the twelve, and these are outside it.

**Cited — what can stand in a delve.** The admission allowlist (`admit/allowlist.rs`, the one palette every prefab and every detail program is judged by) admits, of the twelve, `chest`, `barrel`, `furnace`, `smoker` and `trapped_chest`, and none of the others. So in a delve built from admitted pieces the census of §5.1 ranges over **five block ids**; the engine's table carries all twelve, because the allowlist is a loadable file and the census must stay right when it widens.

### 2.2 What the tree holds today, measured

**Cited.** `container_slots` accepts `chest`, `trapped_chest`, `barrel` at 27 and nothing else, so a `loot` fill aimed at a furnace is refused as *not a container* (`DW0431`) though the game opens it. The compile-time world is a `BlockMap` of blockstates: `block_entities_omitted` counts the signs, banners, heads and lecterns whose picture needs block-entity NBT the map does not carry, which is also the fact that **every container the world is written with is empty until `setup_finish` fills it** — a prefab's `.nbt` cannot carry a stocked chest in. `class_apply_<c>` runs, in order: reset the class trigger, give the kit, tag the class where a flask or a watch needs it, set `dw.classed`, arm the campaign-start quests for the party, `teleport` the player to the spawn anchor (`emit/class.rs`, `class_apply_fns`); it is one-shot per player (`compiler.md`, class select row). `narrate{text,style?,sound?}` is the engine's one shape for a player-facing line, with `chat` the default style and `title`/`subtitle`/`art`/`actionbar` beside it, lowered by one emitter. Across the content repository at `main` and at the campaign's branch, no campaign declares an empty `loot` fill and none places a container block through `set-block`, `fill-region` or a `prop`, so §6.2's refusals bind nothing that exists (a measured zero, stated).

### 2.3 What the server does with a click on a container, and the primitive that refuses one

**Cited — bytecode.** A container block's `useWithoutItem` calls `Player.openMenu(MenuProvider)` (read in the chest, barrel, shulker box, hopper, dispenser, brewing stand and crafter block classes; the furnace family reaches the same call through its `openContainer`). `ServerPlayer.openMenu` asks the provider for a menu; `BaseContainerBlockEntity.createMenu(int, Inventory, Player)` returns one **only if `canOpen(player)`**, and otherwise calls `sendChestLockedNotifications(centre, player, displayName)` and returns `null`, on which `openMenu` opens nothing (a spectator is told `container.spectatorCantOpen`). `canOpen` is `LockCode.canUnlock(player)`: **`player.isSpectator() || predicate.test(player.getMainHandItem())`** — the `lock` field of the block entity, read by `LockCode.fromTag`, is an `ItemPredicate`, and a stack in the main hand that matches it opens the box. The notification is `displayClientMessage(translatable("container.isLocked", name), true)` — the action bar — plus a sound; the pinned `en_us.json` renders the key as **`%s is locked!`**, `%s` the container's display name.

So the pinned game has exactly one primitive for a container that will not open: the **`lock` component with a predicate no stack the party can hold satisfies** (a `custom_data` predicate the engine never issues is unsatisfiable by construction), set per block entity by one `data merge block` line at `setup_finish`. It is a vanilla primitive, not a hack. Its one player-facing word is *locked*, which promises a key. §8 records why this spec does not use it on scenery, and the non-goals name what it is for.

**Cited — the click is observable.** `ServerPlayerGameMode.useItemOn` fires `CriteriaTriggers.DEFAULT_BLOCK_USE` (`minecraft:default_block_use`) when a main-hand item interaction defers to the empty hand (`InteractionResult.TryEmptyHandInteraction`) and the block's own `useWithoutItem` consumes the action, so an advancement can record that a player opened a given container. This spec does not need it (§4 makes the ordering structural); it is recorded so the next spec that wants to know *that a box was opened* reads it here.

### 2.4 Established practice for a container that is only scenery

The question asked: how do games with lootable containers keep a player from opening the ones that hold nothing, and how do they tell the player which is which. Three answers are in the record; each is marked.

1. **Scenery is not interactable** — **cited**: the *Baldur's Gate 3* community wiki's container census counts 6808 interactable containers (39%) against 10604 decorative, non-interactable ones (61%); the split is a property of the object, and a player learns in the first room that a box without the use prompt is furniture. The pinned Minecraft client draws no use prompt and every container block opens, so this answer exists in the pinned game only as the `lock` of §2.3, with its word.
2. **Every openable container pays off** — **cited** as the complaint's shape rather than as a stated rule: the thread of player reports around the same game is of time spent opening containers that held nothing, and the suggested repair in those reports is the first answer. **Authored**: a container that opens and shows an empty grid is the defect in every register the finding was reported in.
3. **The level does not leave the player asking** — **cited**: Chung's readability practice — *do you want the player to be asking this question?* — and the *Skin Deep* team's answer of standardised metrics and banned ambiguous geometry so a player can tell at once what can be climbed, fitted through or exited; and the *Team Fortress 2* reading that a clear gap in detail between what matters and what does not filters the information a player needs. **Authored**, from this: where the object cannot carry the rule (the Minecraft barrel looks the same whether stocked or bare), the rule is **told once, before the first instance, and holds everywhere after**; a rule told after the player has already formed the expectation is a correction, which is what the finding describes.

What was not found: a developer statement of the "every chest holds an item" convention for the Souls series, which is therefore not relied on; a Minecraft map-making source on bare decorative chests (the hits were feature requests for locked and keyed chests, which say that mappers want the first answer, and nothing more).

**What the practice decides here.** Minecraft cannot take answer 1 without saying *locked*, so the campaign's rule is told by answer 3's method: one line, in the campaign's words, at the moment the player arrives, before any container can be reached — which is what the finding's general form already says (*when any decoration container exists, the opening tells the player in the campaign's words*). The engine proves the line is shown and that it is true (§6.1: a notice over a world with no scenery container is refused, because `game-writing.md` §8 L2 holds that whatever the text says of the world is true).

## 3. The surface

**Authored.** Stage 5 (`quests.json`) gains one object beside `guidance` and `loot[]`, named for the class it speaks for:

```json
"containers": {
  "decoration_notice": {
    "text": "The town was stripped the winter before the sea came. What is left in its barrels is salt and dust; what you need, you will be handed.",
    "style": "chat"
  }
}
```

1. **`containers`** — optional. The container class's home in stage 5; it holds one field today, and a later capability of the class (a keyed container) lands here rather than on a verb. Present and empty is `DW1019`.
2. **`decoration_notice`** — the `narrate` payload verbatim — `{text, style?, sound?}`, one type with `Verb::Narrate`'s fields, never a second copy. `text` enters the l10n inventory as `containers.decoration_notice` and is translated like any line; `style` defaults to `chat`, and `title`, `subtitle`, `art` and `actionbar` carry their existing checks (`DW0328`, `DW0330`); `sound` is validated by `DW0326`. Declaring it is the declaration that **every container no fill and no device claims is scenery, and this is the line that tells the player**. Its words are the creative judgement; what it covers is derived (§5.1) and printed.
3. **Nothing else is typed.** A per-container or per-place decoration list would be a procedural derivation entered by hand — the cells a program's `scatter` lands on are not an author's knowledge — and the constitution refuses an input that is neither a creative judgement nor handed by the tool (CLAUDE.md — **cited**). One line covers the class; the census says what it covered.

**`loot[]` reaches the whole class.** `container_slots` (the one reader of *is this block a container, and how big*) grows to the twelve kinds of §2.1 with their slot counts, so a fill may stock a smoker (a cooked fish in `container.2`, the result slot — positional, so a stack for `container.0` and `container.1` precedes it) and a `collect` may adopt one. `DW0432`'s validation-tier cap stays the largest count (27); the build tier compares the fill to the real container's slots and refuses a fill too large for it under the code that already judges the cell (`DW0431` for `loot`, as `DW0438` already does for `collect`).

**A loot fill is never empty.** `items` must hold at least one stack (`DW1019`); a container meant to hold nothing is scenery, and scenery is declared by the notice, not by an empty fill.

## 4. Emission

### 4.1 The notice

**Authored, on §2.2's cited order.** When `containers.decoration_notice` is declared, every `class_apply_<c>` gains the notice's lines **after `scoreboard players set @s dw.classed 1` and the party's quest arming, and before `teleport`** — lowered by the one `narrate` emitter with the acting player as its audience, exactly as a `narrate` in a presser-audience bundle is lowered, so a `chat` notice is the same `tellraw @s …` shape every chat line takes and a `title` notice the same `title @s …`. It fires once per player per world by the function's existing one-shot (`dw.classed`); a second player who classes later reads it at their own arrival. A campaign declaring no notice emits none of this and is byte-identical.

**Why arrival, and not a root the author chooses.** A notice must precede the first container a body can reach. In the configuration at campaign start, the standable set from the spawn already contains the first room's containers in every campaign this finding is about — the barrels are in the arrival street — so a notice at any later root would leave them unannounced, and an ordering proof over the nav model would refuse every placement but the arrival. The engine therefore places it at the arrival **by construction**, inside the function that warps the body in, before the warp: the ordering is structural, no nav proof is owed, and the author chooses the words and the style, not the moment. A notice is a party fact told to each body; it takes no `audience` and no `when`.

### 4.2 The census

**Authored.** A new compiler module, `compiler::containers`, reads the **edited assembled model** — the same bytes the nav premises are measured from, after the stage-7 replay — and collects every cell whose base id is in the container table. Against those cells it resolves the **claims** the declarations make:

| Claim | Where it comes from | Cell |
|---|---|---|
| `loot` | a `loot[]` entry | its `anchor`'s cell |
| `collect` | a `collect` adopting `container`; or a `collect` with neither `container` nor `dropped_by`, whose chest the compiler places at `anchor` and fills | the adopted anchor's cell; the `anchor` cell (counted from the declaration — the model holds air there until runtime) |
| `device` | a `traps[]` entry with `trigger: trapped-chest`; a `triggers[]` `use` or `strike` whose `at` cell holds a container or whose `prop` is one; an `interact` objective whose `anchor` cell holds a container or whose `prop` is one | the `at` / `anchor` cell |
| `decoration` | no claim | the remainder |

A cell may carry `loot` and `device` together (the real chest among false trapped chests holds gear, the design `DW0917`'s row keeps); `loot` with `collect`, or two of either, stays `DW0435`. A double chest is two cells and two rows; a runtime-placed `collect` chest is one row from its declaration. Iteration is the block map's own order (ADR-0006 — **cited**); two builds are byte-identical.

### 4.3 The ledger

**Authored.** `validation/containers.json` carries one row per container cell: `cell`, `block`, `slots`, `place` (the area or site-plan node whose piece holds the cell), `claims` (the kinds and the claiming ids), and a header: the counts of §5.1, the notice's resolved `en` text and style when declared (`null` otherwise), and the canonical hash of the census input (the edited model's digest), so the row is a measurement with its instrument named.

## 5. Proving it reaches the party

### 5.1 The binding line

**Authored.** Every build prints `container census: N container cell(s) in the assembled world (<block>: n, …); L loot, C collect, V device, D decoration; notice <declared|none>; by place: <place>: n, …` — every figure computed from the model and the declarations with `N` as the denominator, zeroes included; a campaign with no container prints `0 container cell(s)` and `notice none`, a measured zero. The line is printed beside the existing `container binding` line, which stays (it counts fills; this counts containers).

### 5.2 PackTest

**Authored.** The existing `class_apply_<c>` templates already drive each class's apply function; what a PackTest cannot observe is said: the notice is a client-bound chat or title packet. The command-tree walk every emitted line takes proves the notice's lines are well-formed, and the bot reads them. No new template is owed; the generated set stays bound by `DW0811`'s claims.

### 5.3 The bot

**Authored.** The harness reads `validation/containers.json`; when it names a notice, the bot records the chat (or title) text it receives between its class command and the completion of its first critical-path step and asserts the resolved `en` text arrived in that window; when it names none, the bot asserts no such line arrived. The listener goes through the harness's existing packet reading; no new live command is spoken and `critical-path.json`'s contract does not move.

## 6. Refusals

**Authored**, each raised at the document it is entered in, with the remedy named and a `remedy_reachability.rs` row each.

### 6.1 `DW1018` — the containers built and the containers declared disagree (build tier, `compiler::containers`)

- **`D > 0` and no notice.** The message lists every decoration cell with its block and place, and names both remedies: fill it — with the `ContainerAnchor` the engine already offers when an anchor stands on the cell, or by the program of the place that lands it where none does — or declare `containers.decoration_notice`. Zero is stated: a campaign whose every container is claimed passes with `D 0`.
- **A notice and `D = 0`.** Every container is loot or a device, and the line would tell the player something false of the world (`game-writing.md` §8 L2). The message names the notice and says the remedy is to delete it.

### 6.2 `DW1019` — a container declaration that holds nothing (validation tier, `dsl::loot`)

- a `loot[]` entry whose `items` is empty: a container meant to hold nothing is scenery, declared by the notice; the message says so and names the entry;
- `containers` present and empty;
- a `set-block` or `fill-region` whose block's base id is in the container table: a container placed at runtime stands empty for ever, because every fill runs at `setup_finish`, before it exists; the message names the effect and the two things a runtime container can be — a `collect`'s own chest, which the compiler places and fills, or a container the piece places, filled by `loot`.

## 7. The gallery, and the demo level

**Authored**, against `gallery/` at `5961145d8` (`loot/pedestal-case`, two stacks at `anchor/reliquary`; `obj/take-the-emeralds` adopting `anchor/case`; `obj/take-the-bone` provisioned by `wave/muster`; `trap/chest` on `anchor/strongbox`; the generator's further barrel in the ferry cabin, barrel at the corridor's end room and chest at the hall's end, which no declaration claims today).

1. **Bound.** `quests.json` declares `containers.decoration_notice` with `text`, `style` and `sound`; the generator's containers no declaration names are its scenery, so the census prints `D > 0` with each counted and the notice is emitted in every class's apply function. Every unit — `containers.decoration_notice.{text, style, sound}` — is written, and the coverage gate (`delvec schema --stage all`) binds them; the `narrate` payload's fields are enumerated once at their type (`gallery_units.py`).
2. **Refusal-proven**, one probe each, the primary plus a declared `patch`: `a-barrel-nobody-spoke-for` (remove `containers`) → `DW1018`; `a-loot-chest-with-nothing-in-it` (`loot/pedestal-case`'s `items` → `[]`) → `DW1019`; `a-notice-with-nothing-to-say` (`containers` → `{}`) → `DW1019`; `a-chest-conjured-at-runtime` (a `set-block` effect's block → `minecraft:chest`) → `DW1019`; `a-notice-over-a-stocked-town` (every scenery container filled or replaced, so `D = 0` under a declared notice) → `DW1018` — the last is the probe whose patch must reach the generator's unanchored cells, and if the patch cannot reach them the implementation records the debt rather than inventing a smaller claim.
3. **Perturbation acceptance.** Changing the notice's `text` moves the l10n row and nothing else; changing `style` moves the emitted line in every `class_apply_<c>`; removing the cabin barrel from the generator moves `N` and `D` on the census line and one row of the ledger; adding a `loot` entry on an unclaimed anchored cell moves one row's claim from `decoration` to `loot`.
4. **Demo level**, `docs/demo-levels.md` row: **The Stripped Town** — one street of three houses and a smokehouse; every cupboard in two of the houses is scenery, the third holds the one parcel the story needs, and the smoker holds a cooked fish in its result slot; the arrival line says the town was emptied. The level is the first read of whether the line, heard once at arrival, stops a player opening the barrels — and of whether a stocked smoker reads as a find.

## 8. What the player meets, and what was chosen

Only what reaches the player.

1. **One line at arrival, in the campaign's words, and no scenery container unannounced.** What the player meets: on stepping into the world, a line that says what the containers of this place hold; every barrel after it is what the line said. The alternative — the line at a moment the author picks — is refused by the ordering it would have to prove, since the first room's barrels are reachable before any later moment.
2. **A scenery container still opens, and shows nothing.** The vanilla way to refuse the click (§2.3) says *locked* in the action bar, which promises a key the campaign does not have; a player who reads it goes looking for one, and that is the same hour spent differently. The primitive is kept for the capability it names — a container that opens to the party member holding the right item, which is exactly the HELD semantics `requires_item` already has.
3. **A loot container is never empty, and a smoker can be loot.** What the player meets: a box that opens has something in it, and a cooked fish can be waiting in the smokehouse. The cost is the author's: a stack per declared container, in vanilla's slot order.
4. **A campaign that stocks everything says nothing.** A notice with no scenery to speak for is refused, so the player is never told the barrels are bare in a town where they are not.

## 9. Scenery containers are few, and a container that matters never stands among them

**Authored** (a product rule of this spec): opening any container a player can see is close to every player's instinct, and no notice overrides it. So the notice of §3 tells the player what the scenery holds; it does not license a room of it. Two rules follow, both judged on the census of §4.2 and nothing else.

1. **A container that matters is never hidden among scenery containers.** A cell carrying a `loot`, `collect` or `device` claim never shares its neighbourhood with a `decoration` cell. Searching a hundred chests for the one with the key is the forbidden shape; a trapped chest among loot chests is allowed, because every container in that neighbourhood pays off.
2. **Scenery containers stay sparse.** No neighbourhood holds more than **`MAX_SCENERY_PER_PLACE` = 2** decoration cells. A room that reads as a storeroom — many containers together, all empty — is refused; wherever containers cluster, they are loot or devices.

### 9.1 What "among" means

**Authored.** The neighbourhood of a container cell is **its place**: the census's `place` column (§4.3) — the area whose placed piece holds the cell, or, on a site-plan campaign, the place whose box holds it. Cells in no place form one neighbourhood of their own, printed as `(outside every place)`.

Why the place and not a reach radius on the walkable model:

- **It is the unit the player searches.** A place is the house, the shop, the crypt — the box the author designed as one room and the one the arrival's words and the quest text name. "Among" in the ruling is a reading of a room, and the place is the engine's room.
- **It exists on every build.** The census reads the edited assembled model and the plan's places; the nav model is built only when a campaign's verbs need it, so a radius over it would bind to nothing on a campaign that walks no leg — the UNBOUND shape (CLAUDE.md, *Vacuity*).
- **A grid radius ignores walls**: two containers either side of a party wall, in two houses, would be "among" each other, and a player who cannot see both does not search them as one set.

**The cost, stated**: a large outdoor place (a market square) is one neighbourhood however far apart its containers stand. The signal that would show the choice wrong is a creator refused for two containers in one place that a reader at playable scale does not see as one set — the remedy then is a finer neighbourhood (a radius *inside* a place), and the place stays its outer bound.

### 9.2 The threshold

**Authored**, from a cited practice. The *Oblivion* Construction Set wiki's dungeon tutorial (*My First Dugneon, Part Four*, UESP, CC BY-SA — ideas only, ADR-0013) holds that "an average-sized room that is not themed as a storage area usually should contain 1-3 loot containers, and often you'll want one or two of those to be 'junk' loot", and warns "Don't over-saturate the dungeon with loot containers." **Cited**: one or two non-paying containers per room is the practice's upper bound. The same page says "storage areas usually should have several containers, but typically all junk" — its *junk* holds something (a worthless stack), where this engine's scenery holds nothing, so that sentence does not carry over and the owner's ruling refuses exactly the empty storeroom it would allow. **Authored**: the cap is **2 decoration cells per place**; a double chest is two cells (§4.2) and counts two, because a double chest of nothing is the storeroom shape at half the floor. The figure is a constant (`MAX_SCENERY_PER_PLACE` in `compiler::containers`), printed on the census line, never typed by an author.

**The signal that would show it wrong**: a playtest finding of a player opening every container in a place that passed the cap (the cap is too high), or of a place refused at three where a reader at playable scale sees ordinary furniture, not storage (too low). Either moves the constant in one commit with the finding named.

### 9.3 The refusal — `DW1024` (build tier, `compiler::containers`)

`DW1018` judges whether the containers built agree with the containers declared; this judges how the declared ones are arranged, a different quantifier, so it takes its own code. **`DW1024`** (verified free on every remote ref before it was written here). Two arms, one message listing every offending place:

- **mixed**: a place holding a `loot` / `collect` / `device` cell and a `decoration` cell. The message lists the place, its matter cells with their claims, and its decoration cells with their blocks, and names the remedies: fill every scenery container in that place (it then pays off), or move the one that matters to a place where every container pays off.
- **storeroom**: a place holding more than `MAX_SCENERY_PER_PLACE` decoration cells. The message lists the place and its cells and names the remedies: fill some of them, or replace the surplus in the place's piece or program with furniture that is not a container.

Both arms run after `DW1018` on the same census; a campaign whose census has no decoration cell passes `DW1024` with its count stated (`0 mixed place(s), 0 over the cap`).

### 9.4 The gallery

Each arm is refusal-proven by a probe (`a-key-among-empty-chests` → `DW1024` mixed; `a-storeroom-of-nothing` → `DW1024` storeroom), the primary plus one declared patch; the primary itself passes both arms with its counts on the census line. A perturbation test for each arm lives in `compiler::containers`: moving a matter cell into a place with a decoration cell reds the mixed arm, and adding a third decoration cell to a place reds the storeroom arm, each from a model that passes.

## 10. The item-holder class, and the split

**Authored** (a product rule of this spec): quest items and loot are not confined to chests and barrels. An item sits where the scene would hold it — a sword on a stand, a map in a frame, a potion in a brewing stand, a book on a shelf — so `loot[]` and `collect` target an **item holder**, of which the container is one family. A delve runs in adventure mode, so **a holder qualifies only if a player in adventure mode can take its item** without a hack.

### 10.1 The class, read from the pinned game

**Cited — bytecode**, the same jar, mappings and `javap` reading as §2 (JDK 21 `javap -c -p`, names through `server.txt` sha1 `5621e925…`). The adventure gate for a block interaction is **not** on the use path: `ServerPlayerGameMode.useItemOn` checks only `GameType.SPECTATOR` before calling the block's `useItemOn` and then `useWithoutItem`; the adventure restriction lives in `Player.blockActionRestricted` (`GameType.isBlockPlacingRestricted`, then `ItemStack.canBreakBlockInAdventureMode` on the main hand), which guards breaking and placing, and in `Abilities.mayBuild`. Each candidate's take path was read for either.

| Holder | Take act | Adventure | Read at | Verdict |
|---|---|---|---|---|
| the twelve containers of §2.1 (brewing stand among them) | open the menu, take from a slot | allowed: `useItemOn` → `useWithoutItem` → `openMenu`, no ability check | `ServerPlayerGameMode.useItemOn`, `BaseContainerBlockEntity.createMenu` | **kept — this round** |
| `item_frame`, `glow_item_frame` (entities) | attack the frame: the item pops out, the frame stays | allowed: `ServerGamePacketListenerImpl$1.onAttack` refuses only item entities, experience orbs, the player and unattackable arrows; `BlockAttachedEntity.skipAttackInteraction` asks only `Level.mayInteract` (spawn protection, world border); `ItemFrame.hurtServer` → `shouldDamageDropItem` (any non-explosion damage, item present) → `dropItem` | those three methods | **kept — follow-up F1** |
| `armor_stand` (entity, equipment slots) | right-click the slot with an empty hand | allowed: `ArmorStand.interactAt` → `swapItem` checks the marker flag and the `DisabledSlots` bits, no ability; breaking the stand is gated (`hurtServer` reads `Abilities.mayBuild` of an attacking player) | `ArmorStand.interactAt`, `swapItem`, `hurtServer` | **kept — follow-up F2** |
| `chiseled_bookshelf` | right-click an occupied slot with an empty hand | allowed: `ChiseledBookShelfBlock.useWithoutItem` → `removeBook`, no ability check; holds `#minecraft:bookshelf_books` only | `ChiseledBookShelfBlock.useWithoutItem` | **kept — follow-up F3** |
| `shelf` (1.21.11) | right-click a slot with the main hand | allowed: `ShelfBlock.useItemOn` → `swapSingleItem`, no ability check; refused for the off hand | `ShelfBlock.useItemOn` | **kept — follow-up F3**; not admitted by `admit/allowlist.rs` today |
| `jukebox` | right-click: the disc is ejected as an item entity | allowed: `JukeboxBlock.useWithoutItem` → `popOutTheItem`, no ability check; holds `#minecraft:jukebox_playable`-carrying stacks only | `JukeboxBlock.useWithoutItem` | **kept — follow-up F3**; deliberately not admitted today |
| `decorated_pot` | a hand click with an empty hand only wobbles it (`useWithoutItem`: `DECORATED_POT_INSERT_FAIL`, `WobbleStyle.NEGATIVE`); the stack comes back only when the pot breaks | breaking by hand is refused (`blockActionRestricted`) unless the main hand carries `can_break`; a projectile shatters it: `DecoratedPotBlock.onProjectileHit` asks `Projectile.mayInteract` (for a player owner, `ServerPlayer.mayInteract` → `Entity.mayInteract` = true ∧ `ServerLevel.mayInteract`, **no game-mode check**) and `Projectile.mayBreak` (`#minecraft:impact_projectiles` ∧ game rule `projectiles_can_break_blocks`) | `DecoratedPotBlock.useWithoutItem`, `onProjectileHit`; `Projectile.mayInteract`, `mayBreak` | **excluded this round**, by name: the take needs the party to hold an impact projectile or a `can_break` tool, a real primitive that this spec does not yet make structural; follow-up F4 owes the build-time proof that the party holds one before the pot matters |
| `lectern` | the *Take Book* button | **refused**: `LecternMenu.clickMenuButton`, case 3, returns `Player.mayBuild()` — false in adventure — before `removeItemNoUpdate` | `LecternMenu.clickMenuButton` offset 91 | **excluded**, by name |
| `campfire` | none: `CampfireBlock` has no `useWithoutItem`; a stack leaves only when it finishes cooking and is popped out transformed | — | `CampfireBlock` | **excluded**, by name: nothing to take |
| `item_display` (entity) | none: `Display` overrides no interaction and is not pickable | — | `Display`, `Display$ItemDisplay` | **excluded**, by name |

### 10.2 What exists today

**Cited** (the tree): no DSL surface places an item frame, an armour stand or a display as a holder. An `actor` may puppet `minecraft:armor_stand` with `equipment`, but its gear carries drop chance zero and the puppet is a scripted body, not furniture (`actor.rs`). A prefab's `.nbt` carries an `entities` list the compiler never reads (`compiler/nbtread.rs`), so a frame a piece carried would stand outside every proof. The block-entity holders are blockstates in the block map like the containers.

### 10.3 The split

**Authored.** This round builds the holder class as the container family — the twelve kinds, their slot counts, `loot` / `collect` over all of them, the census, the notice and §9 — because the container is the only kept family whose take the engine already fills and proves (`item replace block … container.<n>`). Each other kept family is a named follow-up, each its own spec, each owing: the fill primitive (a frame's `Item`, a stand's equipment slot, a bookshelf's slot, a shelf's slot), the census rows for its instances (the entity families need the compiler to read and place template entities, which it does not today), a statement per kind of whether an empty instance counts as scenery for §9, and **a live check in adventure mode** (a PackTest or the harness bot) that the take act of §10.1 yields the item:

- **F1** — item frames and glow item frames.
- **F2** — armour stands (equipment slots).
- **F3** — chiseled bookshelf, shelf and jukebox (the last two with their admission).
- **F4** — the decorated pot, only if the party's impact projectile or `can_break` tool can be proven structurally.

The `/new-delve` skill text says the rule now, at the scope the engine reaches: put an item where the scene would hold it, and use a chest only where a chest belongs — a potion in a brewing stand, a cooked fish in a smoker's result slot are containers and reachable today.

## Acceptance criteria

Each criterion is checked against the tree at `5961145d8`; none is satisfied there. `delvec` is this tree's binary, built from its manifests.

1. **Surface.** `delvec schema --stage quests` exports `containers` with exactly `decoration_notice`, whose fields are `text`, `style`, `sound` and nothing else; `decoration_notice` and `Verb::Narrate` share one payload type (`grep` over `crates/dsl/src/` finds one declaration of the `text` / `style` / `sound` triple); the `containers` row in `compiler.md`'s stage-5 table matches the struct.
2. **The container table.** `delvewright_dsl::blocks` holds the twelve kinds of §2.1 with their slot counts, and a test asserts every member is a block id of the vendored `blocks-1.21.11.json` (the cross-check that shares no configuration with the jar reading); `PROVENANCE.md` records the table's source (the server jar and mappings digests named in **Ground**); `container_slots` and `is_container` read the table and nothing else; a `loot` fill and a `collect` adoption on a smoker build green with three stacks and are refused with four under `DW0431` / `DW0438` naming the real slot count.
3. **Emission.** For a campaign declaring a notice, every `class_apply_<c>` carries the notice's lines after the `dw.classed` line and before `teleport`, lowered by the one `narrate` emitter; for a campaign declaring none, every `class_apply_<c>` is byte-identical to today's; every line passes the pinned command tree.
4. **The census.** For the gallery, the census of §4.2 counts every cell of the twelve kinds in the edited model and attributes each claim of the table; a unit test builds a model with one chest claimed by `loot`, one trapped chest claimed by a trap and `loot` together, one barrel unclaimed and a `collect` with no container, and asserts the four rows and the counts `N 4, L 2, C 1, V 1, D 1`.
5. **Determinism.** Two builds of the gallery are byte-identical (existing gate); the ledger of §4.3 carries the census input's digest, and a test asserts two censuses of one model produce one ledger byte for byte.
6. **Refusals.** Each shape of §6.1–6.2 is refused with its code and remedy by a fixture under `crates/delvec/tests/`, with a `remedy_reachability.rs` row each; `tools/ci/check-dw-codes.py` is green with `DW1018` on `docs/reference/delvec/compiler/containers.md` and `DW1019` on `docs/reference/dsl/loot.md`.
7. **Binding line and ledger.** `delvec build` prints §5.1's line on every build (`0 container cell(s)` for a campaign with none, asserted in a CLI test); `validation/containers.json` has the row and header fields of §4.3 for every container cell.
8. **The bot.** `harness/` reads `validation/containers.json`, asserts the notice's resolved text within the window §5.3 names when one is declared and its absence when none is, and does so on the gallery's run; `critical-path.json`'s `format_version` does not move.
9. **Gallery** (spec-0039). §7.1's notice is bound and §7.2's probes refused with the codes named, with the coverage gate's counts on its own line; `gallery/baseline/` is regenerated after the merge commit exists with every moved row attributed.
10. **Docs and skill, same change.** `compiler.md`: the stage-5 row for `containers`, the class-apply row's new line, the census line and the ledger under the build output, the loot row's twelve kinds; the two catalog pages of criterion 6; the `/new-delve` skill's `references/quest-capabilities.md` replaces *give every reachable container consumables or props* with the three states of §1 — a container is loot, a device or scenery; scenery is declared once by the notice; a loot fill is never empty; furnaces and smokers are containers with three slots in vanilla's order — under *Items, containers and loot*; `docs/demo-levels.md` gains §7.4's row; `docs/specs/README.md` carries this spec's row.
11. **The finding closes on its carrier.** The ledger row `str-14` in `docs/playtest-findings.json` (on the branch that holds the campaign's ledger) takes `carrier: {kind: dw, code: DW1018}` and its `binding` the containers the census counts, so `tools/creator/staging-gate.py` reads the general form as a diagnostic re-run against the build, with a non-zero binding on that campaign.

12. **Arrangement** (§9). `compiler::containers` refuses with `DW1024` a place holding a matter cell and a decoration cell, and a place holding more than `MAX_SCENERY_PER_PLACE` (2) decoration cells; each arm has a fixture under `crates/delvec/tests/` and a `remedy_reachability.rs` row, a perturbation test from a passing model, and a gallery probe (§9.4); `DW1024` is documented on `docs/reference/delvec/compiler/containers.md` and `tools/ci/check-dw-codes.py` is green; the census line prints the mixed and over-cap place counts and the cap.
13. **Skill, same rule** (§9). The `/new-delve` skill's `references/quest-capabilities.md` says, under *Furnish the containers*: put few empty containers; never build a storeroom of empty chests; never hide a needed item among decoration; traps among real loot are fine.

14. **The holder class** (§10). `docs/reference/delvec/compiler/containers.md` carries §10.1's table (the kept families, the excluded kinds by name, each with its read site); the skill's *Items, containers and loot* says to put an item where the scene would hold it and to use a chest only where a chest belongs, naming the brewing stand and the smoker as containers a fill reaches today.

### Recorded debts

Checked against the implementing tree; each is a debt, never a pass.

- **Criterion 10, the skill.** `tools/ci/check-skill-page.py` refuses a page naming `DW1018` / `DW1019` while the page pins an engine release that lacks them. The `references/quest-capabilities.md` text is held back until the release that carries this surface is pinned.
- **Criterion 9, the baseline.** Regenerated on the feature branch against its merge base; the regeneration after the merge commit belongs to the integration that lands it.
- **§7.2, the fifth probe.** A probe is the primary plus a declared `patch` over the campaign documents; the gallery's scenery containers stand at cells no anchor names, so `D = 0` may not be reachable by a patch. If it is not, the probe is recorded as unreachable on `docs/reference/delvec/compiler/containers.md` and the `D = 0` arm of `DW1018` is held by its fixture alone.
- **§10.3, the holder families F1–F4.** Each kept non-container family, and its live adventure-mode check, is a follow-up spec; until it lands, `loot` and `collect` reach the container family only.
- **§4.2, a container a stage-7 edit places with contents.** The edit verbs write blockstates; none writes block-entity items, so the census's assumption that every container is empty until filled holds at this revision. A verb that gains the power to stock a container re-opens this row.
