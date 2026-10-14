# `delvec::compiler::containers`

The reference page for `crates/delvec/src/compiler/containers.rs`: the container census (spec-0105) and the diagnostics-catalog row of every DW code this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## The census

Every build reads the **edited assembled model** — the cells every later proof reads — and collects every cell whose block is one of the twelve container kinds of `delvewright_dsl::blocks::CONTAINER_KINDS_1_21_11`. To those it adds each container a declaration has the compiler place at runtime: a `collect`'s own chest (neither `container` nor `dropped_by`), and a `use` / `strike` trigger's or an `interact` objective's container `prop`. Each cell is attributed to its **place** — on a site-plan campaign the place whose play space holds it (else whose footprint column holds it, highest floor at or under the cell), otherwise the area whose placed piece holds it; a cell in no place is `(outside every place)` — and to the claims made on it:

| Claim | From | Cell |
|---|---|---|
| `loot` | a `loot[]` entry | its anchor's cell |
| `collect` | a `collect` adopting `container`; a `collect`'s own chest | the adopted cell; the `anchor` cell |
| `device` | a `trapped-chest` trap; a `use` / `strike` trigger or an `interact` objective whose cell holds a container or whose `prop` is one | the `at` / `anchor` cell |
| `decoration` | nothing | the remainder |

A cell may carry `loot` and `device` together (the real chest among false trapped chests). Rows are in cell order (ADR-0006).

**The binding line**, on every build, beside the `container binding` line:

```
container census: N container cell(s) in the assembled world (<block>: n, …); L loot, C collect, V device, D decoration; notice <declared|none>; by place: <place>: n, …; arrangement: M mixed place(s), K over the cap of 2 decoration cell(s) per place.
```

**The ledger** `validation/containers.json`, on every build: `census_input_sha256` (SHA-256 over every `(cell, state)` of the model the census read, in its order), `cells`, `loot`, `collect`, `device`, `decoration`, `by_block`, `by_place`, `max_scenery_per_place`, `mixed_places`, `over_cap_places`, `notice` (`{text, style}` with the line's plain English text, or `null`) and `rows[]` of `{cell, block, slots, place, claims[{kind, id}]}`.

**The arrangement rule** (spec-0105 §9). A place is the neighbourhood a player searches. `MAX_SCENERY_PER_PLACE` is **2**, authored from the *Oblivion* Construction Set wiki's dungeon practice (one to three containers per ordinary room, one or two of them junk); a double chest is two cells and counts two.

**The gallery.** The primary holds six container cells: the hall's five all claimed (`loot/pedestal-case`, `loot/ferryman-stores`, `loot/gallery-end-stores`, `obj/take-the-emeralds`, `trap/chest`), and the long hall's end chest, the one scenery container, spoken for by `containers.decoration_notice`. Seven probes refuse the shapes: `a-barrel-nobody-spoke-for` and `a-notice-over-a-stocked-town` (`DW1018`), `a-loot-chest-with-nothing-in-it`, `a-notice-with-nothing-to-say` and `a-chest-conjured-at-runtime` (`DW1019`), `a-key-among-empty-chests` and `a-storeroom-of-nothing` (`DW1024`). The second `DW1018` probe reaches the end chest's unanchored cell with a stage-7 batch framed on the anchor standing on top of it.

## The item-holder class

Spec-0105 §10: `loot[]` and `collect` target an **item holder**, of which the container is one family, and a holder qualifies only if a player in adventure mode can take its item. Read from the pinned 1.21.11 server jar (bundled `server-1.21.11.jar` sha256 `ec47239a…fcebc`, `javap -c -p`, names through `server.txt` sha1 `5621e925…`). The adventure gate on a block is not on the use path — `ServerPlayerGameMode.useItemOn` checks only `GameType.SPECTATOR` before the block's `useItemOn` / `useWithoutItem` — but in `Player.blockActionRestricted` (breaking and placing) and `Abilities.mayBuild`.

| Holder | How a player takes the item | Adventure | Read at | In the engine |
|---|---|---|---|---|
| the twelve containers | open the menu | allowed | `ServerPlayerGameMode.useItemOn`, `BaseContainerBlockEntity.createMenu` | `loot` / `collect` / census |
| `item_frame`, `glow_item_frame` | attack: the item pops out | allowed — `ServerGamePacketListenerImpl$1.onAttack`, `BlockAttachedEntity.skipAttackInteraction` (spawn protection only), `ItemFrame.hurtServer` → `shouldDamageDropItem` | those methods | follow-up F1 |
| `armor_stand` | right-click the slot, empty hand | allowed — `ArmorStand.interactAt` → `swapItem` reads the marker flag and `DisabledSlots` only; breaking is `mayBuild`-gated in `hurtServer` | those methods | follow-up F2 |
| `chiseled_bookshelf` | right-click an occupied slot | allowed — `ChiseledBookShelfBlock.useWithoutItem`; books only | that method | follow-up F3 |
| `shelf` | right-click a slot, main hand | allowed — `ShelfBlock.useItemOn` → `swapSingleItem`; not admitted by `admit/allowlist.rs` | that method | follow-up F3 |
| `jukebox` | right-click: the disc is ejected | allowed — `JukeboxBlock.useWithoutItem` → `popOutTheItem`; not admitted | that method | follow-up F3 |
| `decorated_pot` | an empty-hand click only wobbles; the stack returns when the pot breaks | breaking by hand is refused without `can_break`; a projectile shatters it (`DecoratedPotBlock.onProjectileHit`: `Projectile.mayInteract`, no game-mode check, and `mayBreak`: `#minecraft:impact_projectiles` and `projectiles_can_break_blocks`) | those methods | **excluded**; follow-up F4 only if the party's impact projectile or `can_break` tool is proven structurally |
| `lectern` | the *Take Book* button | **refused** — `LecternMenu.clickMenuButton` case 3 returns `Player.mayBuild()` | that method, offset 91 | **excluded** |
| `campfire` | none — no `useWithoutItem`; a stack leaves only cooked | — | `CampfireBlock` | **excluded** |
| `item_display` | none — `Display` overrides no interaction and is not pickable | — | `Display`, `Display$ItemDisplay` | **excluded** |

## Diagnostics

### DW101x / DW102x — the container class (spec-0105)

| Code | Meaning |
|------|---------|
| `DW1018` | The containers built and the containers declared disagree. Build-tier (exit 3), judged on the census after `DW0431` / `DW0438` have proven every fill names a real container. Two arms. **Scenery with no notice:** a container no `loot`, `collect`, trap, trigger or objective claims, and no `containers.decoration_notice` — a player would open it to an empty grid with nothing having told them the place holds nothing. The message lists every decoration cell with its block, cell, place and slots, and per cell names the anchor standing on it (a `loot` entry on that anchor fills it) or says no anchor does (the piece or program of its place lands it). Remedies: fill each one, replace it with furniture that is not a container, or declare the notice. **A notice with nothing to speak for:** every container is claimed, so the notice would tell the player something false about the world (`game-writing.md` §8 L2); the remedy is to delete it. A campaign whose every container is claimed and which declares no notice passes with `D 0`. |
| `DW1024` | Scenery containers arranged as a storeroom, or a container that matters standing among them (spec-0105 §9). Build-tier (exit 3), judged on the census after `DW1018`. Two arms, one message listing every offending place: **MIXED** — a place holding a `loot` / `collect` / `device` cell and a decoration cell; a player looking for what matters opens every one; remedies: fill every scenery container there, replace it with furniture that is not a container, or move the one that matters to a place where every container pays off (a trap among loot is allowed). **STOREROOM** — a place holding more than `MAX_SCENERY_PER_PLACE` (2) decoration cells; remedies: fill some, or replace the surplus in the place's piece or program. A census with no decoration passes with `0 mixed place(s), 0 over the cap`. |
