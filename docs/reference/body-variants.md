# A body's variant — what the pinned game carries and how a summon writes it

Reader: the engine work behind a body's `variant` (the DSL's one field on every class that summons a body — `compiler.md`, stage 2 and stage 5) and anyone reading the table it is judged and written by, `crates/dsl/data/entity-variants-1.21.11.json`. Every statement is **[cited]**, read from the pinned jar at the read site below or measured on the pinned server, or **[authored]**, a judgement made here. Established once and treated as constant (CLAUDE.md, *A check guards only what our toolchain owns*); the tools that read and measure it are named so the reading can be repeated when the pin moves.

## The read site

- The server jar `versions.toml [minecraft].server_jar_url`, sha256 `f83b8e093865806f931c7e34aae41b177d4c076335263dd124c75d6d65dd1726`, its inner `META-INF/versions/1.21.11/server-1.21.11.jar`, with the ProGuard mappings piston-meta publishes for the version (sha1 `5621e9253f05fd57872bbe7f8ddf5f9a7d525955`). No obfuscated name is written here.
- The reading: `tools/maintenance/extract-entity-variants.py` (with `variants/VariantDump.java`), JDK 21 — `javap -c` over `addAdditionalSaveData` and `applyImplicitComponent` on every entity type's class chain, reflection over the booted registries and enums, and the jar's own `data/minecraft/<registry>/*.json`.
- The measurement: `tools/maintenance/probe-entity-variants.py` on the pinned server — every value of every property of every type summoned and read back with `data get entity`: 23 types, 331 reads, 0 disagreeing.

## 1. Why a summoned body is always the default

**[cited]** `/summon <entity> <pos> <nbt>` does not call `finalizeSpawn`; the two forms without NBT do (`docs/reference/fishing-buckets-and-cat-coats.md` §4). `finalizeSpawn` is where a type picks its random coat (a cat's from the `cat_variant` spawn conditions, a sheep's colour, a horse's coat and markings, a llama's, an axolotl's, a panda's genes, a villager's type from the biome). Every body the engine summons carries NBT (its tags at least), so without a declared variant it is the type's default. Measured defaults, a body summoned with tags only:

| Type | Default |
|------|---------|
| cat | `variant:"minecraft:black"`, `CollarColor:14b` (red) |
| wolf | `variant:"minecraft:pale"`, `sound_variant:"minecraft:classic"`, `CollarColor:14b` |
| pig, cow, chicken, frog, zombie_nautilus | `variant:"minecraft:temperate"` |
| villager | `VillagerData:{type:"minecraft:plains",profession:"minecraft:none"}` |
| zombie_villager | `type:"minecraft:plains"`, and a **random** profession (two runs read `weaponsmith` and `nitwit`): the body draws one when it is made, not in `finalizeSpawn` |
| horse, llama, trader_llama, parrot, axolotl, tropical_fish | `Variant:0` (white / no markings; creamy; red_blue; lucy; a white kob on white) |
| rabbit | `RabbitType:0` (brown) |
| fox, mooshroom | `Type:"red"` |
| salmon | `type:"medium"` |
| sheep | `Color:0b` (white) |
| shulker | `Color:16b` (no dye: the purple shulker) |
| panda | `MainGene:"normal"`, `HiddenGene:"normal"` |
| painting | `variant:"minecraft:alban"` |

**[authored]** A zombie villager summoned without its profession stated is a different body on every build: the NPC emitter always writes `VillagerData` for a villager or zombie villager (a declared type or profession takes the place of plains or none); an actor, a wave mob, a mount or a vehicle writes it only when the body declares one.

## 2. What a variant property is

**[cited]** The game names its entity variants as data components `<entity>/<property>` — 25 of them in `DataComponents`: `villager/variant`, `wolf/variant`, `wolf/sound_variant`, `wolf/collar`, `fox/variant`, `salmon/size`, `parrot/variant`, `tropical_fish/pattern`, `tropical_fish/base_color`, `tropical_fish/pattern_color`, `mooshroom/variant`, `rabbit/variant`, `pig/variant`, `cow/variant`, `chicken/variant`, `zombie_nautilus/variant`, `frog/variant`, `horse/variant`, `painting/variant`, `llama/variant`, `axolotl/variant`, `cat/variant`, `cat/collar`, `sheep/color`, `shulker/color`. A type carries a component when a class on its chain names it in `applyImplicitComponent`; for every component that set of types equals the set whose chain holds the class that saves it (so `villager/variant` is a villager's and a zombie villager's, `llama/variant` a llama's and a trader llama's).

**[authored]** Three saved fields that change how a body looks and that no component carries are properties too: a horse's `markings`, a villager's or zombie villager's `profession`, and a panda's `gene` (written to both genes, so the gene shown is the one declared — a recessive gene shows only when both are it). The property is the component's name after the slash; a value is written as the component is (a registry id such as `minecraft:tabby`, or a name such as `blue`).

## 3. How a summon writes each one

**[cited]** Each key is a string constant of the saving class's `addAdditionalSaveData` (or of the lambda it stores through); the forms are the codecs it stores with, and every row was read back as written on the pinned server.

| Type | Property | Values | Summon NBT |
|------|----------|--------|-----------|
| cat | `variant` | `cat_variant` registry (11) | `variant:"<id>"` (`VariantUtils`) |
| cat, wolf | `collar` | the 16 dyes | `CollarColor:<dye id>b` |
| wolf | `variant` | `wolf_variant` (9) | `variant:"<id>"` |
| wolf | `sound_variant` | `wolf_sound_variant` (7) | `sound_variant:"<id>"` |
| pig, cow, chicken, frog | `variant` | their registries (3 each) | `variant:"<id>"` |
| zombie_nautilus | `variant` | `temperate`, `warm` | `variant:"<id>"` |
| painting | `variant` | `painting_variant` (51) | `variant:"<id>"` |
| villager, zombie_villager | `variant` | `villager_type` (7) | `VillagerData:{type:"<id>"}` |
| villager, zombie_villager | `profession` | `villager_profession` (15) | `VillagerData:{profession:"<id>"}` |
| fox | `variant` | `red`, `snow` | `Type:"<name>"` |
| mooshroom | `variant` | `red`, `brown` | `Type:"<name>"` |
| salmon | `size` | `small`, `medium`, `large` | `type:"<name>"` |
| parrot | `variant` | 5, ids 0–4 | `Variant:<id>` |
| llama, trader_llama | `variant` | 4, ids 0–3 | `Variant:<id>` |
| axolotl | `variant` | 5, ids 0–4 | `Variant:<id>` |
| rabbit | `variant` | 7 (`evil` is id 99) | `RabbitType:<id>` |
| horse | `variant` / `markings` | 7 coats / 5 markings | `Variant:<coat> \| <markings> << 8` |
| tropical_fish | `pattern` / `base_color` / `pattern_color` | 12 patterns (packed ids: size \| index << 8) / dyes | `Variant:<pattern> \| <base> << 16 \| <pattern colour> << 24` |
| sheep, shulker | `color` | the 16 dyes | `Color:<dye id>b` |
| panda | `gene` | 7 genes | `MainGene:"<name>",HiddenGene:"<name>"` |

**[authored]** A part of a packed key the body does not declare contributes id 0, which is the type's default (a horse with only a coat has no markings).

## 4. What the probe found besides

**[cited]** In the probe's first two runs a body was not found by `@e[tag=…]` immediately after the `summon` that reported it, for 4 and then 1 of 331 bodies; the probe now waits up to 2 s for the body before reading and prints when it waited, and in three runs it never waited. The engine's own PackTest reads the body after `setup` has run, so it does not depend on this.

## 5. A collar is drawn only on a tame body

**[cited]** The pinned client jar (`versions.toml [minecraft].client_jar_sha1` `ba2df812c2d12e0219c489c4cd9a5e1f0760f5bd`, client mappings for the version): `CatRenderer.extractRenderState` sets the render state's `collarColor` to `cat.isTame() ? cat.getCollarColor() : null`, and `WolfRenderer.extractRenderState` does the same for a wolf; the collar layer draws nothing for a null colour.

**[authored]** The engine summons no body tame, so a declared `collar` is saved and read back (the generated PackTest asserts it) and is not drawn.
