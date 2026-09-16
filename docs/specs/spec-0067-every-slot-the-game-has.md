# spec-0067: Every slot the game has

- **Status**: Accepted
- **Ground**: written against engine `495fca44` (`origin/main`), read only, and
  against four records of the pinned game. Two are the ones
  `crates/delvec/data/PROVENANCE.md` already names: the `misode/mcmeta`
  `1.21.11-summary` at commit `c976eb3b`, whose `item_components/data.min.json`
  was fetched for this spec and read back at sha-256
  `51b191e13f86813ca02f1498942e5bc235947edb71eb8105a78401670b3665c4` — the
  digest `PROVENANCE.md` pins — and the vendored
  `crates/dsl/data/entity-tags-1.21.11.json`. Two are added by this revision:
  the pinned client's **equipment assets** (`misode/mcmeta` tag `1.21.11-assets`
  at commit `c5b87628`, the 44 files under `assets/minecraft/equipment/`, whose
  sorted `sha256sum` list hashes to
  `150b585538e2295db9daee16e3fce4c23ac5bcdddc85358208b8e7adadbf78bb`), and the
  pinned client's **entity renderers**
  (`net.minecraft.client.renderer.entity`), read from a decompiled 1.21.11
  client under Mojang's published mappings, whose `SharedConstants.WORLD_VERSION`
  is 4671 — the DataVersion `PROVENANCE.md` pins. The copy read is the GitHub
  mirror `rrrRex1024/minecraft-1-21-11-source` at `fb136698`; it is an
  unlicensed redistribution, read for facts and adopted from in nothing
  (ADR-0013); the reproducible instrument is the pinned client jar under the
  official mappings. Every count below is computed from those records by a
  script over the whole file, never a hand count; the wiki page cited is named
  where it is used. The shape of this document is spec-0062's.
- **What it is for**: a campaign dresses a body in anything the pinned game
  can put on it — a barded and saddled horse at the gate, a wolf in armour, a
  llama under a carpet, a ghast in a harness — and is refused, by name, when it
  puts a piece where the game will not show it **on that body**: a chestplate
  on a horse, a sword in a creeper's hand, a helmet on a wolf.
- **Research**: the slot vocabulary is a fact of the pinned game, read from
  two of its own records: the `minecraft:equippable` item component's `slot`
  field (the wiki page *Data component format/equippable*, which lists the
  eight values) and the `allowed_entities` field of the same component in the
  pinned item data, which names entity types and entity-type tags the vendored
  tag file resolves. **Which of the eight a body shows** is a fact of the
  pinned client, read from the renderer registered for each entity type and
  the equipment layers it carries (§4.1), cross-checked against the wiki pages
  *Armor* (§ Mob armor), *Carved Pumpkin* (§ Usage) and *Slot*, the entity-type
  tags, and the equipment assets. Every rule below is marked **cited** or
  **authored**.
- **Numbers**: no spec or ADR beyond this one. **One new DW code**, `DW0898`
  (§5). **`dsl_version` moves**: `equipment` gains two fields and
  `drops[].slot` two values.
- **Non-goals**: a per-entity hitbox for the horse and the other mounts
  (`nav::entity_dims` has no row for them and falls back to the humanoid box;
  §7.1 names it as the adjacent gap it is); rideable mounts, a player on a
  saddle; a wave mob dropping its saddle (drops stay the `elite` / `boss`
  rule); trims, dyes and any other item component; the harness's or the
  saddle's runtime behaviour on a puppet; whether a body *uses* what it holds
  (a pillager fires its crossbow, a villager does not swing its sword — an AI
  question, not a slot question).

## 1. The defect, in numbers

**Finding, from reading the tree and the pinned data.**

`MobEquipment` (`crates/dsl/src/stages.rs`) names six slots — `head`, `chest`,
`legs`, `feet`, `main_hand`, `off_hand` — and `MobEquipment::slots()` returns
a fixed array of six; `EquipSlot`, the enum a `drops[].slot` entry names, has
the same six; `emit::strip_drops_line` writes six `drop_chances` keys by hand.
Writing `body` or `saddle` into an actor's `equipment` is refused at `DW0100`
naming those six. The same `MobEquipment` is worn by a `WaveMob` and an
`Actor`; an `Npc` carries none. Nothing in the tree asks whether the body
being dressed shows the slot: the six are offered to a horse, a wolf and a
creeper exactly as to a zombie.

The pinned game has eight. The `equippable` component's `slot` takes `head`,
`chest`, `legs`, `feet`, `body`, `mainhand`, `offhand`, `saddle` [cited — the
wiki page named above, and the pinned client's `EquipmentSlot` enum, whose
serialised names are those eight]. In the pinned item registry **84 items**
carry the component, over **seven** distinct slot values: `body` 44, `head`
16, `chest` 8, `feet` 7, `legs` 7, `saddle` 1, `offhand` 1 (the shield); no
vanilla item declares `mainhand`, because a hand takes anything. The two the
DSL lacks are the two a horse needs: `body` is where horse armour, wolf
armour, a llama's carpet, a nautilus's armour and a happy ghast's harness go;
`saddle` is the saddle.

The item data also says **who may wear what**: of the 84, 45 carry
`allowed_entities` — `#minecraft:can_wear_horse_armor` (6 items),
`#minecraft:can_equip_harness` (16), `#minecraft:can_wear_nautilus_armor`
(5), `#minecraft:can_equip_saddle` (1), `minecraft:wolf` (1) and
`[minecraft:llama, minecraft:trader_llama]` (16, the carpets); the 39 armour
pieces, heads, the pumpkin, the elytra and the shield carry none. The vendored
entity-tag file resolves every one of those tags:
`can_equip_saddle` is eleven types (camel, camel husk, donkey, horse, mule,
nautilus, pig, skeleton horse, strider, zombie horse, zombie nautilus),
`can_wear_horse_armor` is horse and zombie horse, `can_wear_nautilus_armor`
two, `can_equip_harness` one.

**But a body does not have eight slots.** The server stores all eight on
every living entity: `LivingEntity.canUseSlot` answers true for every slot,
and a `/summon`'s `equipment` compound is read whole through
`EntityEquipment.CODEC` with no filter [cited — the pinned client; the wiki
page *Slot* says the same in words: *all living entities support these slots
although not all mobs show or make use of the items*]. What a player sees is
decided elsewhere: by the renderer the client registers for that entity type,
and the fixed set of equipment layers it carries. Counted over the pinned
entity registry (157 ids), **92 are living entities** and 65 are not — boats,
minecarts, projectiles, displays, markers — and carry no equipment at all. Of
the 92, **16** show the humanoid six; **7** more show a head item without
armour; **9** show `body`; **11** show `saddle`; **6** show the main hand
only; **48 show nothing at all**; **63 hold hands the player never sees**
(§4.1 has every name).

Fit judged by the item alone — its own `equippable.slot` and
`allowed_entities` — passes a diamond chestplate on a horse, which declares
`chest` and no allowed list, and the game never shows it. That is the same "structurally perfect NBT
nobody sees" defect the spec exists to refuse, left open for every armour
piece on every non-humanoid body and for every hand piece on 63 of the 92.

## 2. The row's general form, corrected

**Authored.** The row proposes deriving the slot set from the `equippable`
`slot` values the pinned item registry uses. That derivation gives **seven**
slots, not eight: it cannot see `mainhand`, which no item declares and every
body has. A slot set derived that way would drop the hand the DSL already
models, or keep it by a hand-written exception — the second authority the row
was trying to remove. The fix is to name the right authority and use the
registry as its cross-check:

1. **The vocabulary is the game's equipment-slot set**, eight values, held as
   data in `crates/dsl` with `VanillaRule` provenance naming the wiki page it
   was pinned from, and pinned by the test that lands it (spec-0062 §3's
   pattern for the hurting-block set). It is the type `EquipSlot` deserialises
   into and enumerates from; nothing else lists slots.
2. **The registry is the cross-check, not the source.** A test asserts every
   `equippable.slot` value the pinned item data uses is in the vocabulary,
   with the counts of §1 as the numbers it prints — so a slot the game gains
   and an item uses reds the test the day the data is re-pinned, which is the
   property the row wanted, and `mainhand` is in the set because the game says
   so rather than because a script happened to find it.
3. **The slot set a body has is a property of its entity type**, held as a
   second pinned table beside the vocabulary (§4.1). The vocabulary says what
   a slot can be called; the body table says which of them this body shows,
   and for which kind of piece. Nothing else answers "does this body show
   this slot" — not the item, not a per-verb exception.
4. **The item data is the other half of the fit authority** (§4.2): a horse
   armour in `chest`, a saddle on a villager. Both are structurally perfect
   NBT the server stores and never shows.

## 3. The surface

**Authored.**

```json
{ "id": "actor/destrier", "entity": "minecraft:horse", "anchor": "anchor/gate",
  "equipment": {
    "body":   "minecraft:iron_horse_armor",
    "saddle": "minecraft:saddle"
  } }
```

- `MobEquipment` gains `body` and `saddle`; each takes an `EquipItem` (bare id,
  or `{item, enchantments}`) exactly as the six do. The DSL keeps its own
  spellings for the hands (`main_hand`, `off_hand`) and the NBT keys stay
  where they are (`EquipSlot::nbt`).
- `MobEquipment::slots()` is derived from `EquipSlot::ALL`, never a literal
  array; `strip_drops_line` writes one `drop_chances` key per slot of the same
  enumeration. A ninth slot, should the game grow one, is one enum arm.
- `drops[].slot` accepts the eight; a horse elite that leaves its saddle is
  `{"slot": "saddle"}` under the same `DW0490` / `DW0491` rules.
- The eight are offered to every body at the schema; which of them a given
  body may fill is decided at validation (§4), where the body's entity type is
  known. A body whose type shows none of the eight — a creeper, a warden, a
  boat — is refused piece by piece under §5, never silently dressed.
- Emission is unchanged in form: the component-era `equipment:{…}` /
  `drop_chances:{…}` summon NBT, with the two new keys and chance `0.0f` on
  every undeclared slot. The generated gear PackTest, which already asserts
  the summoned body holds what the campaign dressed it in, asserts the new
  keys the same way; that assertion is the live proof that the pinned server
  reads `saddle` and `body` off `/summon` NBT, and the spec states it as the
  proof rather than assuming the keys.

## 4. Fit — what the body shows, and what the registry says an item is for

### 4.1 What a body shows

**Cited** for every fact (the pinned client's renderers, its equipment
assets, the item data, the entity-type tags; the wiki pages named); **authored**
for the rule, the decision and the pinning.

**The rule.** A slot is shown on a body when the client renderer registered
for the body's entity type (`EntityRenderers`; `player` and `mannequin` share
`AvatarRenderer`) carries an equipment layer that draws that slot, and the
layer accepts the piece. The layers, and what each accepts:

- `HumanoidArmorLayer` — `head`, `chest`, `legs`, `feet`. Draws a piece only
  when its `equippable.slot` is that slot **and** it has an `asset_id`
  (`HumanoidArmorLayer.shouldRender`), through the asset's `humanoid` /
  `humanoid_leggings` layers. A helmet in `legs` is not drawn here or
  anywhere.
- `CustomHeadLayer` — `head`. Draws whatever the armour layer does not claim:
  a carved pumpkin, a skull, a block, any item without an asset.
- `WingsLayer` — `chest`. Draws a piece whose asset has a `wings` layer (the
  elytra).
- `ItemInHandLayer` (and the avatar's `PlayerItemInHandLayer`) — both hands,
  any item. `CrossedArmsItemLayer`, `WitchItemLayer`, `FoxHeldItemLayer`,
  `DolphinCarryingItemLayer`, `PandaHoldsItemLayer` — the main hand only.
- `SimpleEquipmentLayer(<type>)`, `WolfArmorLayer`, `LlamaDecorLayer` — `body`
  or `saddle`, for a piece whose asset has that layer type. The pinned assets
  use **18** layer types; 15 of them name their body (`horse_body`,
  `pig_saddle`, `camel_husk_saddle`, …), the other three are `humanoid`,
  `humanoid_leggings` and `wings`.

So a piece has a **kind**, derived from the item data: **armour** (29 items —
an asset and a `head`/`chest`/`legs`/`feet` slot), **wings** (1, the elytra),
**animal** (45 — an asset with a `*_body` or `*_saddle` layer; every one of
them carries `allowed_entities`), and **item** (the 9 equippables with no
asset — eight heads and the pumpkin, and the shield — plus every item with no
`equippable` at all). A body's slot is listed with the kinds its layers draw.

**The table, by group.** 92 living entity types in the pinned registry of
157; the groups sum to 92.

| Group | Entity types | Shows |
|---|---|---|
| The humanoid six | 16: zombie, husk, drowned, zombie villager, skeleton, stray, bogged, parched, wither skeleton, piglin, piglin brute, zombified piglin, giant, player, mannequin, armor stand | `head`/`chest`/`legs`/`feet` armour; `head` item; `chest` wings; both hands. The giant is the one of the 16 with no head-item and no wings layer. |
| Head item and both hands, no armour | 5: evoker, illusioner, pillager, vindicator, copper golem | `head` item; both hands — the evoker only while casting, the illusioner while casting or aggressive, the vindicator while aggressive. |
| Head item and main hand | 2: villager, wandering trader | `head` item; main hand (crossed arms). |
| Main hand only | 4: fox, dolphin, panda (only while sitting), witch | main hand. |
| Both hands only | 2: allay, vex | both hands. |
| Body and saddle | 5: horse, zombie horse, skeleton horse, nautilus, zombie nautilus | `body` (`horse_body`; `nautilus_body`) and `saddle` (each its own type; the two nautiluses share `nautilus_saddle`). The skeleton horse draws `horse_body` and no horse armour admits it — §4.2 shape 3 closes what the layer alone would open. |
| Saddle only | 6: camel, camel husk, donkey, mule, pig, strider | `saddle`, each its own type. |
| Body only | 4: wolf, llama, trader llama, happy ghast | `body` (`wolf_body`; `llama_body`; `happy_ghast_body`). |
| Nothing | 48: armadillo, axolotl, bat, bee, blaze, breeze, cat, cave spider, chicken, cod, cow, creaking, creeper, elder guardian, ender dragon, enderman, endermite, frog, ghast, glow squid, goat, guardian, hoglin, iron golem, magma cube, mooshroom, ocelot, parrot, phantom, polar bear, pufferfish, rabbit, ravager, salmon, sheep, shulker, silverfish, slime, sniffer, snow golem, spider, squid, tadpole, tropical fish, turtle, warden, wither, zoglin | no slot. The snow golem's pumpkin and the enderman's block are flags on the entity, not equipment. |

Per slot: `mainhand` is shown by 29 bodies, `offhand` 23, `head` 23 (16 with
armour, 22 with an item; 7 with an item and no armour), `chest`/`legs`/`feet`
16, `saddle` 11, `body` 9. **Hands held but not shown: 63** — the 48 that
show nothing and the 15 that show only `body` or `saddle`.

**The cross-checks, run for this spec.** The saddle rows equal
`#minecraft:can_equip_saddle` exactly (11 = 11). The body rows are the union
of the item `allowed_entities` lists (horse, zombie horse, nautilus, zombie
nautilus, happy ghast, wolf, llama, trader llama) plus one body no item
admits, the skeleton horse. The 15 body/saddle layer types the pinned assets
declare are exactly the types the rows name. The wiki page *Armor*'s list of
mobs whose armour renders in Java equals the table's armour rows less player,
mannequin and armor stand, whose own pages carry the fact. The wiki page
*Carved Pumpkin*'s list of eighteen entities that visibly wear a pumpkin is the
table's head-item set less parched, copper golem, player and mannequin — the
page predates those four.

**Stored but not shown is refused.** [authored] Every piece the server would
store and the client would not draw is refused by §5: armour on a horse, a
helmet on a wolf, a sword in a creeper's hand. The held-item case is the one
with a mechanic behind it — a held weapon's attribute modifiers apply to any
living entity's attack, drawn or not — and it is refused for the reason
spec-0062 refuses a killing volume under safe-looking floor: the engine knows
and the player does not. The remedy for the number is `attributes`, which the
DSL already has on both bodies. The four bodies whose hand is drawn only in a
state (three illagers, the panda) **count as shown**: the game draws the piece
in the state the mob fights in; a NoAI actor of those species does not show
its hand until it is unleashed, and the skill says so.

**How the table is pinned, and what re-checks it.** [authored] No pinned data
file answers the humanoid half. The entity-type tags name the saddle and
body wearers, and the equipment assets name the body and saddle layer types,
but nothing in the game's data says which entity types carry
`HumanoidArmorLayer`, `CustomHeadLayer` or a hand layer — that is client
code. So the table is **authored from the pinned client's renderers under
Mojang's published mappings**, vendored as
`crates/dsl/data/entity-slots-1.21.11.json` — one row per living entity type
in the pinned registry, keyed by namespaced id, each `{renderer, slots}`,
where `slots` maps the game's slot name to `{kinds, layer?, when?}`: the
kinds drawn (a hand lists all four), the body/saddle layer type, and the
state a conditional hand is drawn in — with its provenance row in
`PROVENANCE.md`. The three cross-checks below are
`crates/delvec/tests/equipment_tables.rs`. It is re-derived by the same reading when ADR-0009 moves
the pin. Three machine cross-checks hold it to the data every day between:
(a) the saddle rows equal `#can_equip_saddle`; (b) the body rows contain the
union of the item allowed lists and the excess is exactly the named list
(today: skeleton horse); (c) the body/saddle layer types in the pinned
equipment assets — themselves pinned by the digest in the ground line — equal
the types the rows name. The fourth is live: the generated gear PackTest
proves the pinned server stores every emitted slot. **Nothing in the tree
renders an entity**, so visibility has no machine proof; §7.2 names that as
the gap, and the demo level (§6) is where an eye confirms the table once.

### 4.2 The three shapes

**Cited** for the facts (the pinned item data, the table above); **authored**
for the rule.

A declared piece is held to the table and the item data at validation, where
the `DW0143` item check already runs. The body is the entity the puppet
actually wears (`BodyRef::worn_entity`, the one authority
`nav::actor_body_entity` also reads — a skinned actor is a mannequin),
resolved against ids and the entity-type tags through the vendored tag file.
A piece is refused when any of three shapes holds, and the diagnostic states
every shape that holds:

1. **The body does not show this piece in this slot.** The table has no
   entry for the body's type at that slot for the piece's kind. An iron
   chestplate on a horse; a saddle on a zombie's `saddle`; a sword in a
   creeper's hand; an iron helmet on a villager's head (the villager draws a
   head *item*, not head *armour*). A body with no row — a boat, a minecart
   — fails this shape on every slot.
2. **The item declares a different slot.** Its `equippable.slot` is not the
   slot written: a `diamond_helmet` in `legs`, an `iron_horse_armor` in
   `chest`, a `saddle` in `body`. The hands are exempt — `main_hand` and
   `off_hand` take any item, because the game renders a held helmet as a
   held helmet; the shield's own `offhand` declaration is the one registry
   value that names a hand, and it is satisfied trivially.
3. **The item's allowed-entities list excludes this body.** A saddle on a
   zombie horse is admitted; a horse armour on a mule, a horse armour on a
   skeleton horse, a wolf armour on a fox: refused, naming the item, the body,
   and the entities the item admits.

**Not judged:** a piece with no declared slot, in a slot the body shows for
its kind — a carved pumpkin on a guard's head, a stone block on a zombie's
head, a diamond block in a pillager's hand. The game accepts them, several
are idioms, and neither the table nor the registry states a contrary fact.

**The pair with `DW0496`.** That diagnostic prescribes `equipment.head` as
the remedy for a daylight-burning body, and this spec would refuse the remedy
on three of the tag's members that show no head — zombie horse, zombie
nautilus, phantom. `DW0496`'s prescription is therefore read from the table:
a head piece is prescribed only for a body that shows one; the rest get the
roofing prescription alone, as the phantom already does. A gate that names a
remedy owes a check that the remedy is reachable; the test is §8.

The item side reads a vendored derivation,
`crates/delvec/data/item-equippable-1.21.11.json`: `items`, item id →
`{slot, asset_id?, allowed_entities?, kind}` (`allowed_entities` always a
list), and `asset_layers`, every equipment asset's layer types, extracted from
`item_components/data.min.json` and the equipment assets by
`tools/extract-item-equippable.py`, which pins both source digests and the counts of §1
and §4.1 (84 items; seven slot values; 45 with an allowed list; 29 armour, 1
wings, 45 animal, 9 item; 44 assets, 18 layer types, 15 of them a body or
saddle layer) and refuses a source whose digest or counts differ.
`PROVENANCE.md` carries its row. The compiler injects it into validation
through `ItemRegistry::equippable`; a registry that carries no equippable
data judges nothing.

## 5. The refusal

**Authored.** One new code, `DW0898`, validation tier (exit 1),
`dsl::equipment::fit_checks` called from `dsl::validate`, three
shapes of one rule — *a piece is declared where the game will show it, on
this body*:

- **the body does not show it** — names the item, its kind, the slot written,
  the body's entity type, and the slots that body shows (with kinds); the
  remedy is another body or another piece, and for a held weapon meant as a
  number, `attributes`;
- **the wrong slot** — names the item, the slot written, and the slot the
  item declares; the remedy is the slot;
- **the wrong body** — names the item, the body's entity, and the entities
  the item admits (ids and tag members spelled out, so the creator does not
  open the tag file); the remedy is the entity or the piece.

Why validation tier: every fact is in the documents, the table and the
pinned data; a build is not needed to learn that a saddle does not go on a
villager. Why one code: all three shapes are the pinned game contradicting a
declaration about where a piece will be seen, and the three remedies never
conflict; a diagnostic that holds several shapes says so in one message.
Every run of the validation funnel every subcommand goes through prints
`equipment binding: B body(ies) dressed over K entity type(s), P piece(s)
declared over S slot(s) in use, F piece(s) with a registry-declared slot, A
with an allowed-entity list, R refused (DW0898).` — zeroes included.

## 6. What the gallery, the record and the skill owe

**Authored.**

- **The gallery element** (spec-0039). A horse actor in the annex —
  `minecraft:horse`, `body: minecraft:iron_horse_armor`, `saddle:
  minecraft:saddle` — spawned by the existing staging beat; bound by
  perturbation: changing the armour's id moves the `summon` line's `equipment`
  compound. Units: `MobEquipment.body`, `MobEquipment.saddle`, and
  `EquipSlot::body` / `EquipSlot::saddle` through a tiered horse's
  `drops[]` or through the probe. Vanilla item and entity ids are data, never
  units.
- **The probe.** One committed probe for the new code: the horse given
  `minecraft:iron_chestplate` in `chest` (shape 1 — the body shows no chest),
  refused by `validate`. The other shapes are unit tests, since one probe
  carries one code: a helmet in `legs` (shape 2), a saddle on `actor/sergeant`,
  a zombie (shapes 1 and 3 together), an iron helmet on a villager's head
  (shape 1 by kind).
- **The record.** `docs/reference/compiler.md`: the `equipment` surface row
  (eight slots, the per-body table, the fit rule), the new code's row,
  `DW0490`'s row (the slot list it enumerates), `DW0496`'s row (the
  prescription now read from the table), and the PackTest section for the
  gear assertion; `crates/delvec/data/PROVENANCE.md` gains the rows for the
  item derivation, the equipment assets and the body table; `docs/reference/tools.md`
  gains the extractor's row — in the pull request that lands the code.
- **The skill.** A creator reads it from `references/quest-capabilities.md`
  under *Items, containers and loot* (the equipment shape, the eight slots,
  that a body shows only some of them, and that a piece is refused where the
  game would not show it on that body) and under *Bodies* (the groups of
  §4.1 in one table — who takes armour, who takes only a head item, who takes
  `body` and `saddle`, who takes nothing; the four species whose hand shows
  only when they fight; a horse is dressed through `body` and `saddle`; its
  footprint is the humanoid default until §7.1 is closed, so a mounted set
  piece is posted, not routed).
- **The demo level.** A row in `docs/demo-levels.md` when the code lands: a
  stable yard — a barded and saddled horse, a harnessed ghast overhead, a
  wolf in armour at the gate, a pillager in a pumpkin — each dressed piece
  visible from the arrival cell. It is the one place the table's visibility
  is confirmed by an eye (§4.1, §7.2).

## 7. Scope, and what is named out of it

**Authored.**

### 7.1 The mount's body

`nav::entity_dims` lists twenty-odd entities and falls back to `0.6 × 1.95`
for the rest; `horse`, `donkey`, `mule`, `camel`, `strider` and `llama`
are not in it. A horse actor is therefore routed and clearance-checked as a
humanoid. This spec dresses the horse and does not size it; the gap is
recorded as a ledger row against `entity_dims` with the binding computed
over the actors and wave mobs whose entity falls back, and the skill page
says a mount is posted, not walked, until that row closes.

### 7.2 The rest

- Visibility has no machine proof: nothing in the tree renders an entity, so
  the body table is held by the three data cross-checks of §4.1 and by the
  demo level's eye, and is re-derived from the client when the pin moves. A
  render surface that draws a dressed body would close this; it is not built
  here.
- Riding, mounting, a player in the saddle — vanilla behaviour no puppet has.
- Item components beyond `equippable`: trims, dyes, custom model data.
- Drops from ordinary mobs — the no-grind rule stands.

## 8. Acceptance criteria

Machine-checkable; each names its instrument. Each verdict states the tree
that carries this spec's implementation; a criterion that tree cannot satisfy
is recorded as a debt.

1. **The vocabulary.** `EquipSlot::ALL` has eight arms; a test asserts the
   set equals the eight the wiki page lists, with `VanillaRule` provenance
   naming the page; `MobEquipment::slots()` and `strip_drops_line` derive
   their keys from it, asserted by a test that counts `drop_chances` keys in
   the emitted strip line against `EquipSlot::ALL.len()`. *Tree: met —
   `equipment_tables.rs::the_slot_vocabulary_is_the_eight_the_game_names` and
   `emit.rs::the_strip_line_zeroes_every_slot_the_game_has`.*
2. **The registry cross-check.** A test over `item-equippable-1.21.11.json`
   asserts every `slot` value it holds is in `EquipSlot::ALL`, prints the
   per-slot counts, and asserts them equal to §1's (`body` 44, `head` 16,
   `chest` 8, `feet` 7, `legs` 7, `saddle` 1, `offhand` 1; 84 items; 45 with
   an allowed list) and the kinds of §4.1 (29 armour, 1 wings, 45 animal, 9
   item). *Tree: met —
   `equipment_tables.rs::every_slot_the_item_data_declares_is_in_the_vocabulary`.*
3. **The extractor.** `tools/extract-item-equippable.py` pins the item-data
   digest `51b191e1…` and the equipment-asset list digest `150b5855…`, pins
   the counts, refuses a mismatch by exit status, and is reproducible
   byte-for-byte (two runs, one digest); `PROVENANCE.md` carries the rows.
   *Tree: met — two runs write sha-256 `6440584b…`; a perturbed asset
   directory exits 1.*
4. **The body table.** `crates/dsl/data/entity-slots-1.21.11.json` has one
   row per living entity type in the pinned registry — 92 rows, denominator
   157, each naming its renderer class — and a test asserts the group counts
   of §4.1 (16 humanoid six; 5 + 2 head item without armour; 4 main hand
   only; 2 both hands only; 5 body and saddle; 6 saddle only; 4 body only; 48
   nothing) and the per-slot counts (`mainhand` 29, `offhand` 23, `head` 23,
   `chest`/`legs`/`feet` 16, `saddle` 11, `body` 9). *Tree: met —
   `equipment_tables.rs::the_body_table_has_a_row_per_living_entity_type_in_the_stated_groups`.*
5. **The table's data cross-checks.** Three tests: the saddle rows equal
   `#minecraft:can_equip_saddle` from the vendored tag file; the body rows
   contain the union of the item allowed lists and the excess is exactly
   `[minecraft:skeleton_horse]`; the body/saddle layer types named by the rows
   equal the 15 such types in the pinned equipment assets. Each prints its
   binding with denominator. *Tree: met — `equipment_tables.rs`, three
   tests.*
6. **The surface.** `delvec schema --stage all` exports `body` and `saddle`
   on `MobEquipment` and the two variants on `EquipSlot`, under the
   `dsl_version` the implementing round is handed. *Tree: met — the export
   at the implementing tree's `delvewright_dsl::DSL_VERSION`.*
7. **Emission.** A test dresses a horse actor in armour and saddle and asserts
   the summon NBT carries `equipment:{body:{…},saddle:{…}}` and
   `drop_chances` with `body:0.0f,saddle:0.0f`; the generated gear PackTest
   for that campaign asserts both keys on the live body; two builds are
   byte-identical (ADR-0006). *Tree: met —
   `emit.rs::a_barded_and_saddled_horse_carries_body_and_saddle_keys`; the
   gallery's `v06_actor_equipment_minecraft_horse` PackTest asserts both keys
   on the puppet and the twin, and runs live on the PackTest tier.*
8. **Fit, shape 1.** A test declares `minecraft:iron_chestplate` in `chest`
   on a `minecraft:horse` and asserts the new code naming the slots a horse
   shows (`body`, `saddle`); `minecraft:iron_sword` in `main_hand` on a
   `minecraft:creeper` is refused naming no shown slot; `minecraft:iron_helmet`
   in `head` on a `minecraft:villager` is refused (head armour) while
   `minecraft:carved_pumpkin` in `head` on the same villager is green; an
   `equipment` on a non-living entity is refused on every slot. *Tree: met —
   `equipment_fit.rs::a_body_that_does_not_draw_the_slot_is_refused`.*
9. **Fit, shape 2.** A test declares `minecraft:diamond_helmet` in `legs` on
   a zombie and asserts the new code naming `head`; the same helmet in
   `main_hand` is green. *Tree: met —
   `equipment_fit.rs::an_item_in_a_slot_it_does_not_declare_is_refused`.*
10. **Fit, shape 3.** A test declares `minecraft:saddle` on a
    `minecraft:zombie` and asserts the new code naming both shapes 1 and 3
    and the eleven admitted types; `minecraft:iron_horse_armor` on a
    `minecraft:skeleton_horse` is refused by shape 3 alone; the same saddle
    on `minecraft:horse` is green; a skinned actor is judged as
    `minecraft:mannequin`. *Tree: met —
    `equipment_fit.rs::an_item_that_excludes_the_body_is_refused`.*
11. **Fit, silence.** A test declares `minecraft:carved_pumpkin` in `head` on
    a zombie and `minecraft:stone` in `head` and asserts neither is refused.
    *Tree: met — `equipment_fit.rs::an_undeclared_item_in_a_drawn_slot_is_silent`.*
12. **The `DW0496` pair.** A test stages a burning `minecraft:zombie_horse`
    under open sky and asserts the prescription names roofing and not
    `equipment.head`; the same for a zombie and asserts it names both. *Tree:
    met —
    `daylight.rs::the_prescription_names_a_head_piece_only_for_a_body_that_shows_one`.*
13. **Drops.** A test declares `{"slot": "saddle"}` on a `boss` horse that
    wears one and asserts the emitted drop chance on `saddle`; the same drop on
    a horse without a saddle is `DW0490`. *Tree: met — the emission half in
    `emit.rs::a_barded_and_saddled_horse_carries_body_and_saddle_keys`, the
    `DW0490` half in `equipment_fit.rs::a_saddle_drop_needs_a_saddle`.*
14. **The binding line.** Every build prints §5's line; the gallery primary
    reports `K ≥ 2`, `F ≥ 1` and `A ≥ 1`. *Tree: met — the gallery primary
    reads `K` 3, `F` 9, `A` 2; `equipment_fit.rs::the_binding_counts_what_the_rule_examined`
    holds the line's shape and its zeroes.*
15. **The gallery.** §6's horse builds green; perturbing the armour id moves
    the summon line; the probe is refused at `validate` with the new code;
    `tools/check-gallery-coverage.py` reports 0 units in neither state. *Tree:
    met — `actor/destrier`; probe `a-chestplate-on-a-horse`.*
16. **The ledger row for §7.1** exists in `docs/playtest-findings.json` with a
    binding computed over the bodies whose entity falls back to the default
    box. *Tree: debt — a row with no carrier is `NO-GENERAL-FORM` on every
    campaign `tools/staging-gate.py` judges, the gallery included, so the row
    reds `tools/check-gallery-stageable.py` on the tree that lands it; it is
    held with the ledger's other open capability rows until a carrier for the
    mounts' footprint exists.*
17. **The record and the skill.** The rows and pages of §6, in the pull
    request that lands the code. *Tree: met — the record's rows and the
    skill page's two sections. The page describes the refusal without naming
    its code: `tools/check-skill-page.py` holds every code a page names to the
    page's engine pin, a release that predates `DW0898`, so the code's name is
    added to the page when the pin moves to a release that declares it.*
18. A demo-level row is queued when the code lands. *Tree: met — The Stable
    Yard.*

## 9. Decisions for the owner

- The slot vocabulary is **the game's eight**, held as pinned data and
  cross-checked against the item registry — the alternative is the ledger
  row's derivation from the registry alone, which yields seven and loses the
  main hand.
- **A body has the slots its entity type shows, not eight.** A zombie, a
  skeleton, a piglin, a player-shaped mannequin or an armor stand shows the
  six; a horse shows `body` and `saddle`; a wolf, a llama or a happy ghast
  shows `body`; a pig, a camel, a donkey or a strider shows `saddle`; a
  villager or a pillager shows a pumpkin or a skull on its head and what it
  holds, but no armour; 48 of the 92 living types — the creeper, the warden,
  the spider among them — show nothing. The table is read from the pinned
  client's renderers, because no data file the game ships answers the
  humanoid half; it is held to the tags and the equipment assets by three
  tests and re-read when the pin moves. The alternative is the game's own
  storage rule — every living entity stores all eight — which is exactly the
  rule that let a chestplate be put on a horse where nobody would see it.
- A piece declared **where the game will not show it on that body is
  refused** — the body lacks the slot, the item names another slot, the item
  excludes the body; one new DW code, three shapes in one message. **Stored
  but not visible is refused too**, a sword in a creeper's hand included; a
  number a creator wanted from a held weapon is written as `attributes`. The
  alternative is emitting it and letting a saddled villager or an armed
  creeper ship with nothing visible.
- An item the registry says nothing about (a pumpkin, a block), in a slot the
  body shows for it, is **not judged** — the alternative is refusing every
  non-armour item outside the hands, which would forbid an idiom the game
  supports.
- `DW0496` **prescribes a helmet only to a body that shows one**; a zombie
  horse or a phantom is told to roof the ground. The alternative is a pair of
  gates where one prescribes what the other refuses.
- The mounts' **hitbox is not sized here** — a horse stays a humanoid to the
  router until a separate row closes it; the alternative is widening this
  spec into the dims table.
- **`dsl_version` moves**; no ADR.
