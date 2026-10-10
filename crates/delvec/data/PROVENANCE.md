# Vendored Minecraft 1.21.11 data — provenance

These files pin the game data the compiler validates against (ADR-0009 = MC
1.21.11, ADR-0011 = vendored command tree + item registry). They change only if
ADR-0009's revisit triggers fire.

**The block tables and the entity-type tags live in `crates/dsl/data/`**, and
this file is their provenance too — one record for one pinned game version,
rather than a second copy that can fall a version behind. `blocks-1.21.11.json`,
`blockstate-shape-props-1.21.11.json` and `block-defaults-1.21.11.json` sit
beside the module that reads them (`delvewright_dsl::blocks`, re-exported as
`delvec::schem::blocks`): a module can only `include_str!` a file its own crate
ships, and that module is the format's — read by the engine, the CPU render
surface included, and read by the prefab generators through their dependency on
it.
`entity-tags-1.21.11.json` sits there for the sibling reason:
both validation tiers ask which entity types do X, and the DSL crate cannot
`include_str!` a file it does not ship. `particles-1.21.11.json` sits there for the
same reason: the `particle` verb's id is refused in `dsl::validate` (`DW0941`). Every reproduce command below names the
path it writes.

## Route taken

Mojang's official data generator was **not** run locally: it requires Java 21 and
only Java 17 was available on the build host (`java -version` →
`openjdk 17.0.20`). Per spec-0002 task guidance, the fallback route was used:
the 1.21.11 **summary** was fetched from the community-maintained
[`misode/mcmeta`](https://github.com/misode/mcmeta) repository, which republishes
Mojang's generated reports verbatim per version.

`misode/mcmeta` is a mirror of Mojang's own generated data (produced by the same
`--reports` generator), so the item registry and command tree here are Mojang's,
not third-party reconstructions.

## Sources

- Repo: `https://github.com/misode/mcmeta`
- Ref: tag `1.21.11-summary` @ commit `c976eb3b2cfcb9f205171527dec46b266afa3ac9`
- Retrieved: 2026-07-30
- `version.json` confirms: `id 1.21.11`, `data_version 4671`,
  `data_pack_version 94` (minor `1`) — i.e. pack format 94.1.

### Downloaded source files (checksums as fetched)

| Source URL (raw.githubusercontent.com/misode/mcmeta/1.21.11-summary/…) | SHA-256 |
|---|---|
| `registries/data.min.json` | `7efb184902cfef62b431bc9826ebcbcde2c23746e5624326ffcf922e15cf28f9` |
| `commands/data.min.json`    | `f2477dfadbeff5707dce1083f90d5dc88f9130bf860ac1c134ffc1de1982b7f6` |
| `item_components/data.min.json` | `51b191e13f86813ca02f1498942e5bc235947edb71eb8105a78401670b3665c4` |
| `data/damage_type/data.min.json` | `0ce7edc377446ecddfd1c3b74b32e2dc3b248edc4035275134fb821e98a6c7ad` |
| `data/tag/damage_type/data.min.json` | `794ce6343293660b5f32d6a78f7a374623bb785d18dfc5ce3cbdeb3093b0161d` |
| `data/tag/entity_type/data.min.json` | `5523f45b7ddb178cd9f8bbe998458cc070910a74bc6c551a37b9279f5d73f844` |
| `data/tag/block/data.min.json` | `ff73a0c7f08cb8276a48daa39c104a34d79f0aebd872b37de9c9dc137d49082f` |
| `data/recipe/data.min.json` | `811e914cf45fc801146103442811285342327a6bb2f46641a58120a131e31918` |
| `version.json`              | `be02c05f3cce0e39a4ae855c01b3dda2f572078d575f4b6b2fd824cc8a137d62` |

## Vendored files (derived, committed here)

- **`blocks-1.21.11.json`** — every 1.21.11 **block** with every property and its
  legal values, from `blocks/data.min.json` of the same summary (source SHA-256
  `178a12096f59f863758a6c685e5eb6de38721b376a30a30383e171d0799f3ee7`, retrieved
  2026-08-11). 1166 blocks, namespaced and sorted. The source's second element —
  the default state — is deliberately dropped: a validator needs to know which
  properties and values are legal, and a second copy of information nothing reads
  is a second thing that can go stale.
  **Why it exists**: the repo checked every emitted *command* against a pinned
  command tree and every item id against a pinned item registry, and checked an
  emitted **block** against nothing. `minecraft:chain` was renamed
  `minecraft:iron_chain` in 1.21.11 and kept being emitted; a structure template
  loads an unknown block as AIR, so the piece ships with the feature silently
  missing. Consumed by `delvec::schem::blocks` (the grammar export and
  `delvec prefab audit`'s `DW0733`) and by `prefabs/invariants/src/invariants.rs` +
  `prefabs/invariants/src/connections.rs` (the `prefab-invariants` crate every
  `prefabs/*-generator` depends on).
  **Note on the nearest existing check**: `DW0193` validates DSL-authored block
  ids against the *item* registry plus five technical ids
  (`ItemBackedBlockRegistry`). Measured against this registry, that proxy has
  **149 false rejects** (real blocks with no item form — wall signs, crops,
  `bubble_column`) and **492 false accepts** (items that are not blocks —
  `minecraft:diamond` passes as a block id). Widening `DW0193` onto this file is
  a `dsl_version`-scale change and is deliberately NOT done here.
  **Reproduce it**: `python3 tools/maintenance/extract-block-registry.py
  <blocks/data.min.json> crates/dsl/data/blocks-1.21.11.json`. The script
  pins and checks the source SHA-256 and the block count.

- **`blockstate-shape-props-1.21.11.json`** — per block, the properties named by
  `multipart` selectors in the block's own blockstate definition
  (`assets/minecraft/blockstates/<block>.json`, 1.21.11 client jar). 95 blocks.
  This is the **shape-carrying** property class `DW0735` fires on: a `variants`
  property the state omits picks one complete model (benign — the default is
  what the author meant), while a `multipart` property *assembles* the model,
  so an omitted connection property drops geometry — a `cobblestone_wall` with
  none written places as an isolated post, silently, in 20 of the 36 library
  prefabs when first measured (2026-08-14). The class is derived from Mojang's
  own blockstate definitions, never a hand-kept id list (CLAUDE.md: a
  capability belongs to the object class).
  Like the font metrics below, the client jar is EULA-bound and never
  committed; what is committed is the derived table of property names.
  **Reproduce it**: `python3 tools/maintenance/extract-shape-properties.py
  <minecraft-1.21.11-client.jar> crates/dsl/data/blockstate-shape-props-1.21.11.json`.
  The script pins the jar's `version.json` to `1.21.11` / DataVersion 4671 and
  cross-checks every derived property against `blocks-1.21.11.json` — a
  selector naming a property the registry does not define is a refusal.
  Consumed by `delvec::schem::blocks` (`shape_carrying` /
  `omitted_shape_carrying`), which serves `delvec prefab audit` and the grammar
  back end's `shape-complete` gate + export refusal, and by
  `prefabs/invariants/src/connections.rs`, which fills the properties this table names from the
  piece's own neighbours before a generator writes its bytes.

- **`block-appearance-1.21.11.json`** — how every 1.21.11 block **looks**: its
  alpha-weighted mean texture colour, its **roughness** (the standard deviation
  of that texture's brightness as a fraction of its own mean, one byte, so a
  wall reads as its material and not as a paint swatch), its mean alpha, and the
  union of its model elements, at its default state, with `minecraft:plains`
  tints. 1161 entries — the registry's 1166 less the 5 air-like states, which
  are absence rather than appearance — and 0 the derivation could not resolve.
  **Every value here is a scalar statistic about an image, and the file carries
  no spatial layout of any texture**: a mean, a standard deviation, a mean alpha
  and a bounding box, each one number or a handful, none of them saying which
  pixel is bright or where an edge falls. No arrangement, no downsampling, no
  thumbnail — nothing from which any part of a texture can be recovered. That is
  what makes it committable when the jar is not (ADR-0013, CLAUDE.md forbidden
  zones), the same footing the font metrics and the shape-carrying property
  table stand on, and it is why the draft rasteriser makes its own positional
  pattern and takes only the AMPLITUDE from here.
  **Why it exists**: what a block looks like is the client jar's answer, and the
  jar is EULA-bound and never committed — the same rule as the shape-carrying
  property table above and the font metrics below. This file carries that answer
  to every machine without one, so the CPU draft rasteriser (`delvec snapshot`,
  `delvec cameras --preview`, `delvec edit preview`) and the GPU path paint from
  one derivation rather than holding two opinions about a question with one
  answer. Consumed by `delvec::compiler::snapshot::block_color`. Its one
  derivation is `delvec::compiler::view::blockcolor::Deriver`, which resolves a
  blockstate to its model chain and its textures as the client does and which the
  interactive viewer runs live against a jar.
  **Reproduce it**: `python3 tools/maintenance/refresh-block-appearance.py
  <minecraft-1.21.11-client.jar>`, at a pin bump or when the derivation changes.
  It re-derives the table (`cargo run -p delvec --example
  derive-block-appearance`, which calls the one derivation rather than owning a
  second) and then **proves the result against the same jar**: the jar-gated
  half of
  `crates/delvec/tests/preview_palette.rs` re-derives every entry and compares,
  and the command fails if any differs. That comparison runs here and nowhere
  else, because the one occasion a jar is in hand is the occasion this file
  changes. The derivation is an example rather than a `delvec` subcommand because
  `delvec` is what an authoring session runs: a flag on it is author-facing
  surface owing a demo level, and a creator never holds this file. The derivation refuses an asset source whose `version.json` does not
  declare `1.21.11`, enumerates the registry rather than any list of its own, and
  writes canonical JSON; two runs of one jar give the same bytes.
  **What holds it between pin bumps**, with no jar and therefore in CI: the file
  records the version its source declared (`mc_version`), and
  `preview_palette.rs` holds that to the engine's own pin and holds the key set
  to `blocks-1.21.11.json`. A commit that moves ADR-0009's pin without running
  the command above is red.

- **`block-defaults-1.21.11.json`** — every 1.21.11 block's **default state**: the
  value the game resolves each unwritten property to. Same source and same pinned
  SHA-256 as `blocks-1.21.11.json`; that file keeps the source entry's first
  element (legal values), this one keeps the second. 1166 blocks, 777 of them with
  at least one property, namespaced and sorted.
  **Why it exists**: a structure template's palette may leave properties out.
  Vanilla fills them from the default state on load, so the file is legal and the
  running server places the right block — and every reader that is not a running
  server has to work it out. Guessing is not close: a bare
  `minecraft:cobblestone_wall` is a wall POST (`up=true`, every side `none`),
  while "the first legal value" gives `up=false` and `east=low`, which is a
  different block, and a review page that guessed drew a solid cube where a wall
  post stands. Distinct from `blockstate-shape-props-1.21.11.json`, which says
  WHICH properties the model is assembled from: this says what each of them means
  when it is not written. Consumed by `delvec::schem::blocks`
  (`default_state` / `unwritten`) and through it by the prefab review page.
  **Reproduce it**: `python3 tools/maintenance/extract-block-defaults.py
  <blocks/data.min.json> crates/dsl/data/block-defaults-1.21.11.json`. The
  script pins and checks the source SHA-256 and the block count, and refuses a
  default that is not one of its own property's legal values.

- **`collision-tops-1.21.11.tsv`** (in `crates/dsl/data/`) — the vertical extent
  of every 1.21.11 blockstate's **collision box**: the bottom and top, in
  sixteenths above the cell floor (`-` for an empty box), of what the game's own
  `BlockState.getCollisionShape(EmptyBlockGetter.INSTANCE, BlockPos.ZERO)` returns
  inside the pinned server jar (`versions.toml` `[minecraft]`
  `server_jar_sha256` `f83b8e09…1726`; server mappings sha1
  `5621e9253f05fd57872bbe7f8ddf5f9a7d525955`, both recorded in the file's
  header). 29,671 states collapse onto the properties that move the extent: 5,724
  rows, each naming only those properties. A bound that is not a whole sixteenth
  (a chain's 6.5) is written as a reduced fraction, never rounded.
  **Why it exists**: where a body standing on a partial block has its feet is
  the block's collision top, and the nav model used to treat every block it had
  no hand-written height for as a full cube — so a body on an upward dripstone
  tip stood a course above where vanilla puts it (0.6875 into the tip's cell),
  and a killing volume in the tip course was reached by no body. Consumed by
  `delvewright_dsl::blockshape::measured_collision`, which gives every block no
  hand rule names its measured floor height when the box rests on the cell floor
  and tops out at 8–15/16, and by
  `blockshape::tests::every_height_is_the_jars_or_the_full_cube_default`, which
  holds every hand-written height against it.
  **Reproduce it**: `python3 tools/maintenance/dump-collision-tops.py [--check]
  [--work DIR]` (JDK ≥ 21 on `PATH`, network for the mappings). It refuses a jar
  whose sha256 is not the pin, resolves every class and member from the mappings
  (`tools/maintenance/collision/CollisionTopDump.java` names none), asserts the
  dumper's state count equals the rows' coverage, and `--check` diffs against the
  committed file; two runs of one jar give the same bytes.

- **`faces-1.21.11.tsv`** (in `crates/dsl/data/`) — which of every 1.21.11
  blockstate's six faces are **full**, asked two ways inside the pinned server
  jar (same jar sha256 and mappings sha1 as the collision table, both recorded in
  the file's header): `sturdy` is the game's own
  `BlockState.isFaceSturdy(EmptyBlockGetter.INSTANCE, BlockPos.ZERO, face)`
  (`SupportType.FULL`: the support shape's face is the whole square), and `full`
  is `Block.isFaceFull(getCollisionShape(...), face)`. 29,671 states collapse
  onto the properties that move either answer: 4,256 rows.
  **Why it exists**: a block that hangs on another stays only while the face it
  hangs on is full (spec-0099). A ladder asks `sturdy` of the block behind it, a
  weeping, twisting or cave vine of the block it grows from, and a vine accepts
  either — so leaves, whose support shape is empty and whose collision is a full
  cube, hold a vine and not a ladder. A vertical extent cannot answer that, and a
  rule written from block names would. Consumed by
  `delvewright_dsl::blockshape::face_is_sturdy` / `face_is_full`, which the
  compiler's climb model reads to decide which climbables the world keeps.
  **Reproduce it**: `python3 tools/maintenance/dump-faces.py [--check] [--work
  DIR]` (JDK ≥ 21 on `PATH`, network for the mappings). Its pin, fetch, mapping
  and collapse steps are `dump-collision-tops.py`'s own, imported;
  `tools/maintenance/collision/FaceDump.java` names no obfuscated member; the
  dumper's state count must equal the rows' coverage.

- **`block-renames-1.21.11.json`** (in `crates/dsl/data/`) — the block-id
  **renames** the pinned game's DataFixerUpper applies on load: an id 1.21.11
  does not have → the id it becomes, with the greatest `DataVersion` at which
  the old id still existed. **1 rename**, from **3** ids that ever left the
  block registry across the 43 releases up to the pin.
  **Why it exists**: the registry answers *does the pin have this id*, and every
  check that judges a NAME — the palette allowlist, a palette screen, a render
  surface — needs the different question *what will the pin HOLD*. Judging a
  pre-pin template's palette as written made `delvec prefab audit` contradict
  itself inside one run: `DW0734` passed `minecraft:chain` at DataVersion 2975
  because the fixer migrates it, and `DW0730` refused the identical cell in the
  next breath because `minecraft:chain` is not a name at the pin.
  **What is derived and what is not.** Which ids disappeared, and when, is fully
  derived from the `block` array of each release's own `registries/data.min.json`
  — so `valid_through` is a *lower bound* on the renaming schema, because the fix
  lands inside the development cycle between two releases and the fixer schedule
  is in the game jar, which nothing here reads. A file at or below the bound
  certainly pre-dates the fix; above it the table says nothing and the caller
  refuses. What each id BECAME is derived from Mojang's own recipe graph: the
  crafting recipe whose ingredient side is byte-identical across the version step
  and whose result moved from the removed id to an id the step added. For
  `chain` → `iron_chain` (nugget/ingot/nugget of iron, 1.21.8 → 1.21.9) that
  pairing is forced — which matters, since the same step added ten chain blocks
  and the registry's nearest-name suggestion for `chain` is `copper_chain`.
  **What it deliberately leaves out**: a removal the recipe graph cannot pair.
  Two exist (`grass_path` at 1.17, `grass` at 1.20.3), both uncraftable, and
  both are ABSENT rather than guessed — absence fails closed, so the audit still
  refuses the id and a reviewer sees a name the pin does not have. Inventing the
  pairing is the invented vanilla data the section below refuses.
  Consumed by `delvewright_dsl::blocks::BlockRegistry::loaded_id_at`, and through
  it by `delvec prefab audit`'s allowlist (`DW0730`) and pre-pin warning
  (`DW0734`).
  **Reproduce it**: `python3 tools/maintenance/extract-block-renames.py
  crates/dsl/data/block-renames-1.21.11.json`. The script reads the same mcmeta
  mirror as the tables above, one `<version>-summary` branch per release; it
  refuses to run unless the newest release at or below DataVersion 4671 is the
  pin, and refuses any row whose `from` still exists at the pin, whose `to` does
  not, or whose bound is not below the pin.

- **`items-1.21.11.json`** — the `item` registry array from `registries/data.min.json`,
  each id namespaced (`minecraft:<id>`) to match DSL usage, de-duplicated and sorted.
  1505 items. Deterministic transform: `sorted(set("minecraft:"+i for i in item))`,
  pretty-printed with sorted keys.
- **`commands-1.21.11.json`** — the Brigadier command tree from `commands/data.min.json`,
  re-serialized deterministically (`json.dumps(sort_keys=True, indent=2)`) so it is
  diffable and byte-stable. Semantically identical to the source (key order only).
- **`entities-1.21.11.json`** — the `entity_type` registry array from
  `registries/data.min.json`, each id namespaced (`minecraft:<id>`), de-duplicated
  and sorted (same transform as the item registry). 157 entity types. Validates
  v0.3 wave mobs (`DW0173`).
- **`sounds-1.21.11.json`** — the `sound_event` registry array from
  `registries/data.min.json`, each id namespaced (`minecraft:<id>`), de-duplicated
  and sorted (same transform as the item/entity registries). 1838 sound events.
  Validates v0.6 `play-sound` / v0.4 `narrate.sound` ids (`DW0326`, spec-0014).
  **Reproduce it** (not a one-off — CLAUDE.md debug doctrine "automate the pitfall
  out of existence"): `python3 tools/maintenance/extract-sound-registry.py <registries/data.min.json>
  crates/delvec/data/sounds-1.21.11.json`. The script pins and checks the source
  SHA-256 and applies the transform `sorted(set("minecraft:"+i for i in sound_event))`,
  `json.dumps(indent=2, sort_keys=True)`.
- **`particles-1.21.11.json`** (in `crates/dsl/data/`) — every particle type the
  pinned game registers, each with `options`: whether the type takes options and
  so cannot be written as a bare id. 115 types, 18 of them options-taking.
  Validates the `particle` verb's id (`DW0941`, spec-0085). Taken from the pinned
  server jar itself, not a mirror: `tools/maintenance/extract-particle-registry.py`
  boots the game's registries, iterates the particle registry and asks each type
  twice — is it a `SimpleParticleType`, and is the type itself a `ParticleOptions`
  — refusing on any disagreement, then cross-checks the id set against the vanilla
  data generator's own `registries.json` report from the same jar. `--check`
  derives and compares against the committed file. Requires Java 21.

- **`item-stack-sizes-1.21.11.json`** — every item's `minecraft:max_stack_size`
  default component, from `item_components/data.min.json` in the same summary,
  namespaced to match DSL usage. 1505 entries — exactly the key set of
  `items-1.21.11.json` (a test pins that, so a future regeneration cannot let the
  two drift). Validates that a single-slot fill's `count` fits the stack
  (`DW0436`): `item replace block … container.<n> with <item> <count>` fails
  **silently** above the cap (rabbit stew caps at 1), the same silent-failure class
  `DW0431` exists for.
  **Reproduce it**: `python3 tools/maintenance/extract-item-stack-sizes.py
  <item_components/data.min.json> crates/delvec/data/item-stack-sizes-1.21.11.json`.
  The script pins and checks the source SHA-256, and refuses to default a missing
  component rather than silently assuming 64.

- **`item-combat-1.21.11.json`** — every item's `attack_damage` / `attack_speed` /
  `armor` / `armor_toughness` contribution, summed from the `add_value` modifiers of
  its `minecraft:attribute_modifiers` default component, plus its `minecraft:food`
  `nutrition`, from the same
  `item_components/data.min.json`. 127 entries (only items with a non-zero number).
  Feeds the spec-0023 time-to-kill arithmetic (`DW0472`) and the muster's armour
  floor. **Absence is
  a fact, not a gap**: an item missing here has no combat *attribute*, which is not
  the same as dealing no damage — a bow's damage is projectile code and appears in no
  vanilla data at all, which is exactly why `combat.rs` treats a projectile kit as
  "TTK not provable" instead of "TTK infinite".
  **Reproduce it**: `python3 tools/maintenance/extract-item-combat-stats.py
  <item_components/data.min.json> crates/delvec/data/item-combat-1.21.11.json`.
  The script refuses any non-`add_value` operation rather than mis-summing it.

- **`damage-types-1.21.11.json`** — every damage type's `scaling` field plus its
  membership of the vanilla `#minecraft:bypasses_armor` tag, from
  `data/damage_type/data.min.json` + `data/tag/damage_type/data.min.json`. 50 entries.
  Feeds the spec-0023 incoming-damage arithmetic (`DW0473`). **The finding this table
  pins**: eight of the nine damage types the DSL exposes are
  `when_caused_by_living_non_player`, and `damage-players` emits a bare
  `/damage <target> <amount> <type>` with **no attacker** — so an Easy campaign's
  scripted hits are *not* halved by the `min(dmg/2+1, dmg)` formula. Only
  `minecraft:explosion` (`always`) scales. Deriving the arithmetic from the
  difficulty formula alone would have been wrong by 2× in the lenient direction.
  **Reproduce it**: `python3 tools/maintenance/extract-damage-types.py <damage_type/data.min.json>
  <tag/damage_type/data.min.json> crates/delvec/data/damage-types-1.21.11.json`.

- **`block-classification-1.21.11.json`** — every block's **form** (its shape
  class) and material **family**, derived from vanilla's own block tags and
  recipe graph in the same summary. 1166 blocks → **788 families**, 128
  multi-member covering 506 blocks, largest **20** (deepslate). Consumed by
  `tools/creator/block-appearance.py`'s screen and mix report (spec-0035), and by
  `prefabs/invariants/src/connections.rs`, whose `fence` / `pane` / `wall` connection classes
  are this table's `form` rather than a name-matched list of its own.
  **Why it exists**: palette selection needed to answer "what shape is this" and
  "what material is this derived from", and the only alternative was name
  morphology — which mis-merges in both directions (`packed_mud` is not
  `mud_bricks`; `end_stone` is not `stone`) and is exactly the invented data the
  section below refuses. Form is Mojang's own answer (`#slabs`, `#stairs`,
  `#walls`, `#fences`, `#doors`, `#trapdoors`, `#buttons`, `#pressure_plates`,
  `#all_signs`, resolved transitively because `#logs` is a tag of tags); `pane`
  is the one form vanilla has no tag for and is read off the blockstate
  connection signature `{east,north,south,waterlogged,west}`, which catches glass
  panes, iron bars and copper bars — 26 blocks — and nothing else. Family is the
  connected components of "one block stock becomes another block": stonecutting,
  cooking, and crafting recipes with **exactly one ingredient, and it a block**.
  That last clause is load-bearing rather than fussy: `granite` is
  `diorite` + `quartz` and `andesite` is `diorite` + `cobblestone`, so counting
  "one block-valued ingredient among any others" welds the whole stone group into
  a 41-member component and makes diorite's family read 41 instead of 7.
  **What it deliberately does not do**: spec-0035 §3.4 recommends unioning the
  graph with `#planks`, `#logs`, `#wool`, `#terracotta`, `#stone_bricks`,
  `#sand`, `#dirt`, `#leaves` and `#copper`. Measured, that takes the largest
  family from 20 to **87** — a species' planks already reach its stairs, slabs,
  doors and buttons through the recipe graph, so welding the twelve species
  together welds everything downstream of them, and it breaks spec-0035's own
  45-member runaway guard. The purely-derived table is what ships;
  `--family-tags` and `--loose` reproduce both measurements.
  **Reproduce it**: `python3 tools/maintenance/extract-block-classification.py
  <tag/block/data.min.json> <recipe/data.min.json>
  crates/delvec/data/block-classification-1.21.11.json`. The script pins and
  checks both source SHA-256s and the block count, and picks each family's
  representative as its lexicographically smallest member so the output cannot
  depend on edge order (ADR-0006).

- **`entity-tags-1.21.11.json`** (in `crates/dsl/data/`) — vanilla's built-in
  `entity_type` tags, from
  `data/tag/entity_type/data.min.json` in the same summary: tag id (namespaced,
  so a lookup reads like the `#minecraft:<tag>` a datapack would write) → its
  sorted values. 46 tags. These are **Mojang's own answers to "which entity types
  do X"**, which is the only acceptable source for such a question here — the
  alternative is a hand-written species table, i.e. exactly the invented vanilla
  data this file's next section refuses.
  Feeds `DW0382` (which bodies may march a lane) via `#minecraft:raiders`, and
  that tag is the reason the table lives in the DSL crate: `DW0382` is decidable
  from the declaration alone and is therefore a validation-tier rule, and before
  it read the tag it answered from a five-species array the pinned game
  disagreed with. Choosing `#minecraft:raiders` for that question is itself a
  claim about the game, and `tools/maintenance/check-patrol-types.py` is what falsifies it:
  it re-derives the set three ways from the pinned server jar — the tag, the
  entity types whose class is a `PatrollingMonster`, and the entity types whose
  class is a `Raider` — and requires all three to name the same species. It also
  checks that of every class in the jar carrying `Patrolling` / `PatrolLeader` /
  `patrol_target` as string constants, exactly one stands in some entity's
  superclass chain, which is what makes the class test the whole answer rather
  than a plausible one.
  Feeds `DW0496` (daylight-burning staging) via `#minecraft:burn_in_daylight`,
  the tag the 1.21 engine itself tests before running a mob's sun-burn tick. The
  tag is about which types run that tick, **not** about which types the fire then
  hurts: `minecraft:wither_skeleton` is in it and is fire-immune, and fire
  immunity is a hardcoded entity-type property that appears in no vanilla data
  branch — so `daylight.rs` carries that one exclusion explicitly, cited, rather
  than pretending the tag alone is the whole rule.
  **Reproduce it**: `python3 tools/maintenance/extract-entity-tags.py
  <data/tag/entity_type/data.min.json> crates/dsl/data/entity-tags-1.21.11.json`.

- **`peaceful-despawn-1.21.11.json`** (in `crates/dsl/data/`) — the entity types
  the game **discards while the world is peaceful**, namespaced and sorted. 38 of
  the 157 types. Read straight out of the pinned server jar (`versions.toml`
  `[minecraft]` `server_jar_sha256`
  `f83b8e093865806f931c7e34aae41b177d4c076335263dd124c75d6d65dd1726`) with the
  official server mappings from piston-meta. Every ticked entity runs
  `Entity#checkDespawn()`, which five classes declare: `Entity` and
  `EnderDragon` (empty — never discarded), `Mob` and `WitherBoss` (discarded on
  peaceful unless the type's `EntityType#isAllowedInPeaceful()` holds;
  `EntityType.Builder#notInPeaceful()` clears it), and `ShulkerBullet`
  (discarded on peaceful unconditionally). The extractor asserts those five are
  the whole list, checks each body's bytecode reads exactly the facts its
  classification claims, and cross-checks the 37 types whose flag is false at
  runtime against the 37 registrations whose builder chain calls
  `notInPeaceful()` in `EntityType`'s static initialiser — two methods that
  share nothing but the jar. It lives in the DSL crate because the derived
  difficulty (`delvewright_dsl::derived_difficulty`) is read by validation
  (`DW0469`) and by the emitter alike. **Reproduce it**:
  `JAVA_HOME=<a JDK 21> python3 tools/maintenance/extract-peaceful-despawn.py`
  (`--check` to compare without writing).

- **`timeline-day-1.21.11.json`** and **`timeline-moon-1.21.11.json`** (in
  `crates/dsl/data/`) — **not** from the misode summary: read straight out of the
  pinned server jar, `versions.toml` `[minecraft]` `server_jar_sha256`
  `f83b8e093865806f931c7e34aae41b177d4c076335263dd124c75d6d65dd1726`, whose
  bundled `META-INF/versions/1.21.11/server-1.21.11.jar` (sha256
  `ec47239a8de246335e1d54f6ac319bd35641778eb4b6a6da06372840d02fcebc`) holds them
  at `data/minecraft/timeline/day.json` (sha256
  `6b6a64255d75579e0d3777d37174860c52899e01d77ea80a2daac87dd41719bf`) and
  `data/minecraft/timeline/moon.json` (sha256
  `947352a0fec398da6d227ab8becd32d5cbf35aa8ec47b0071de2d1273b35ccd3`). Copied byte
  for byte with **one trailing newline appended** — the jar's files end at `}`,
  and that newline is the only edit `delvec fmt --check` asks of a tracked JSON
  file. Feeds `delvewright_dsl::celestial` (spec-0081): the moon's eight phase
  names and their order, the `visual/sun_angle` keyframes and cubic-bezier ease
  the scenes' sun and the celestial position table read, the
  `gameplay/sky_light_level` ramp the light model judges, and the
  `gameplay/monsters_burn` window `DW0496` reads.
  **Check or reproduce it**: `python3 tools/maintenance/extract-timelines.py
  <server.jar> [--write]` — refuses a jar off the pin, and without `--write`
  refuses any difference between the jar's bytes and the committed ones.

- **`item-equippable-1.21.11.json`** — every item that carries the
  `minecraft:equippable` default component (84), from `item_components/data.min.json`
  above, with its declared `slot`, its `asset_id`, its `allowed_entities` (always a
  list) and its derived **kind** — `armour` (an asset and a `head`/`chest`/`legs`/`feet`
  slot), `wings` (an asset with a `wings` layer), `animal` (an asset with a `*_body`
  or `*_saddle` layer) or `item` (no asset) — plus `asset_layers`, every equipment
  asset's layer types. Counts: slots `body` 44, `head` 16, `chest` 8, `feet` 7,
  `legs` 7, `saddle` 1, `offhand` 1; 45 with an allowed list; kinds 29 / 1 / 45 / 9;
  44 assets, 18 layer types, 15 of them a body or saddle layer. Feeds `DW0898`
  (a piece where the body shows it, spec-0067) through `ItemRegistry::equippable`.
  The asset half is a second source: **the pinned client's equipment assets**,
  `misode/mcmeta` tag `1.21.11-assets` @ commit
  `c5b876288e0df01b5cd5798434b066ab97eff88c`, the 44 files under
  `assets/minecraft/equipment/`, whose sorted `sha256sum` listing (`<hex>  <name>`
  per line) hashes to `150b585538e2295db9daee16e3fce4c23ac5bcdddc85358208b8e7adadbf78bb`.
  **Reproduce it**: `python3 tools/maintenance/extract-item-equippable.py
  <item_components/data.min.json> <assets/minecraft/equipment dir>
  crates/delvec/data/item-equippable-1.21.11.json`. The script pins both digests and
  every count above, refuses a mismatch by exit status 1, and writes the same bytes
  on every run.

- **`entity-slots-1.21.11.json`** (in `crates/dsl/data/`) — **the body table**: one
  row per living entity type of the pinned registry (92 of the 157), each naming the
  client renderer class registered for it, and per slot the piece kinds its
  equipment layers draw, the body/saddle layer type, and the state a conditional
  hand is drawn in. **Authored, not extracted**: no data file the game ships says
  which entity types carry `HumanoidArmorLayer`, `CustomHeadLayer`, `WingsLayer` or a
  hand layer — that is client code. It is read from the pinned 1.21.11 client's
  `net.minecraft.client.renderer.entity` package under Mojang's published mappings
  (`EntityRenderers` for the registration, each renderer's class chain for its
  `addLayer` calls; `SharedConstants.WORLD_VERSION` 4671). The copy read is the GitHub
  mirror `rrrRex1024/minecraft-1-21-11-source` @ `fb136698`, an unlicensed
  redistribution read for facts and adopted from in nothing (ADR-0013); the
  reproducible instrument is the pinned client jar under the official mappings. A
  second reading by script over the same classes' `addLayer` calls agrees on every
  per-slot count; the one row it cannot reach by class chain is the ender dragon,
  whose renderer is not a `LivingEntityRenderer` and whose row draws nothing. Held
  every day by `crates/delvec/tests/equipment_tables.rs` to the entity-type tags
  (the saddle rows equal `#minecraft:can_equip_saddle`), to the item table (the body
  rows are the body items' allowed lists plus `minecraft:skeleton_horse`) and to the
  equipment assets (the 15 body/saddle layer types). Nothing in the tree renders an
  entity, so visibility itself is confirmed once by eye on the demo level. Re-read
  from the client when ADR-0009 moves the pin. Feeds `DW0898` and `DW0496`'s
  prescription.

- **`environment-attributes-1.21.11.json`** — every environment attribute the
  pinned game registers (45: 20 `admitted`, 5 `overridden`, 20 `gameplay`), each
  with the `AttributeTypes` field it is built from, the value shape a campaign
  writes (admitted ids only), the range its codec rejects outside of (or `null`),
  and the modifier the overworld's timelines key it with; plus `records`, the
  fields of the six record codecs a structured value is built from, each required
  or optional with its codec's range. Read for spec-0080 by
  `tools/maintenance/extract-environment-attributes.py` from the pinned server jar
  (sha256 `f83b8e093865806f931c7e34aae41b177d4c076335263dd124c75d6d65dd1726`, its
  bundled `META-INF/versions/1.21.11/server-1.21.11.jar`), the 1.21.11 client jar
  (sha256 `1473c9489ac50fda3c435049a76a70d61a10b8610db27f5ba9d8756b686cd3bd`) and
  Mojang's official 1.21.11 server mappings (piston-meta `server_mappings`, sha1
  `5621e9253f05fd57872bbe7f8ddf5f9a7d525955`). Method: (1) every `.class` of each
  jar scanned for `(visual|audio|gameplay)/[a-z_]+`, minus the strings that name a
  loot table in the same jar (`gameplay/fishing`, `gameplay/hero_of_the_village`
  and seven more) — the two jars' lists must be identical or the script refuses;
  (2) the ids `EnvironmentAttributes.<clinit>` registers must equal that list, and
  the registries summary's `environment_attribute` registry (mcmeta, sha256
  `7efb1849…cf28f9` above) holds the same 45; (3) type and range per id from the
  same `<clinit>` (`valueRange(AttributeRange.UNIT_FLOAT | NON_NEGATIVE_FLOAT)`,
  bounds from `AttributeRange.<clinit>`), validated at load by
  `EnvironmentAttribute.valueCodec`; (4) scope: `gameplay/` is out of scope
  (spec-0080 §2.4), a `visual/` id an overworld timeline keys with `override`
  (`dimension_type/overworld.json`, `#minecraft:in_overworld` expanded) is
  `overridden`, the rest `admitted`. Feeds `DW0928`.
- **The particle arm of `DW0928`** reads the one particle table,
  `crates/dsl/data/particles-1.21.11.json` (above, written by
  `tools/maintenance/extract-particle-registry.py`). The same script reads
  `ParticleTypes.<clinit>` in the pinned server jar through the same mappings as
  a second method — a type registered by `register(String, boolean)` returns a
  `SimpleParticleType`; at 1.21.11, 115 types, 97 simple, 18 taking options —
  and refuses when that reading disagrees with the table in any id or answer.

### What vanilla data does NOT provide (and what the compiler does about it)

Mojang publishes no per-entity default attributes — mob base `max_health` and
`attack_damage` live in code, and no branch of the mcmeta summary carries them.
The winnability arithmetic therefore runs its numeric time-to-kill bound **only**
where the campaign declares `attributes.max_health` on the stack, and says so out
loud (`DW0475`) rather than inventing a health table. Inventing one is the
"invented precision" this codebase already refuses for `DEFAULT_FOLLOW_RANGE`
(`compiler/nav/mod.rs`) and `MODEL_MARGIN` (`clearance.rs`).

## Default-font glyph metrics (measured, not vendored)

`crates/delvec/src/compiler/textfit.rs` carries the vanilla default font's glyph **advance
widths** (`DW0330`'s width model). These are *measured from the client jar*, which is
EULA-bound and must never be committed — so the numbers live as a Rust constant and
the measurement is reproducible instead of vendored.

**Reproduce it** (debug doctrine — "automate the pitfall out of existence"):

```sh
python3 tools/maintenance/extract-font-metrics.py <minecraft-1.21.11-client.jar>
```

Stdlib-only (its own PNG decoder — the sheets are 1-bit indexed + `tRNS`). Prints a
JSON report; `ascii.advances` is the 95-entry table (index = codepoint − 0x20) that
must equal `ASCII_ADVANCE`, and `bottom_line` carries the full-width advance.

What it establishes, all verified against 1.21.11 client bytecode rather than assumed:

- **Provider order is first-wins.** `minecraft:default` stacks
  `space → nonlatin_european → accented → ascii → unihex`. (`FontManager` prepends
  each provider then reverses the list; the two inversions cancel.) Only `ascii.png`
  serves printable ASCII.
- **Bitmap advance** = `round(inkColumns * height / cellHeight) + 1`. The ASCII sheet
  is 8×8 at height 8, so advance = ink + 1. 68 of 95 printable ASCII are 6 px.
- **Unihex advance** = `(right - left + 1) / 2 + 1`, but `size_overrides` in the font
  definition pin the CJK blocks to columns 0–15 and win outright over measured ink —
  so every Han glyph and every full-width punctuation mark is **9**, against a Latin
  letter's 6. Ratio **9:6 = 1.5**, not the 2× a "CJK counts double" rule assumes.
- **The trap**: `— – … " " ' ' ·` sit next to the CJK blocks and are common in Chinese
  copy, but `nonlatin_european.png` is declared *before* `unihex`, so they resolve to
  bitmap glyphs (9, 7, 8, 5, 5, 3, 3, 2) — **not** full-width. `PUNCT_ADVANCE` pins them.
- The unihex definition and `unifont.zip` are **not in the jar** (the jar's copy is an
  empty stub); they come from the downloaded asset store, which the script locates via
  the launcher's version manifest / asset index.
- Caveat: with the client's **Force Unicode Font** option the stack becomes
  `[space, unihex]` and Latin collapses to ~4, making the ratio 9:4. The model budgets
  against the vanilla default.

| Vendored file | SHA-256 |
|---|---|
| `items-1.21.11.json`    | `3965d9d5aabc0a2e6270b9e15c4faed76b67b93663d3136fa6ca6ca6f9371e8c` |
| `commands-1.21.11.json` | `8e48958913bbd604bc6a084fa04f139c6012fbe6706391c79b265158221ff6ac` |
| `entities-1.21.11.json` | `a10cc5f3dc042dfb632e87131823846011586d19bd97814bf62b1fa6e66160d2` |
| `sounds-1.21.11.json`   | `841adcd38b83410bed32d57bab909829ce796c1ecd959f2891fcafbf427bc16c` |
| `item-stack-sizes-1.21.11.json` | `a896955918220c489ab2225db6772cd417a0273d94d8dd691029572566e1b5ee` |
| `item-combat-1.21.11.json` | `362288eae4c77d9c53d91547b5735c00d739cafc95e1ab2ef57cd1343b9d29ff` |
| `damage-types-1.21.11.json` | `c3daed77f2557dc7fd784d373e74c1d67b45157bb812c8e4dee761db4696b6fd` |
| `blocks-1.21.11.json` | `e38653d774e3e837dbb74f8baa05d2687741e56eb7e702c03218c31bd2481087` |
| `block-defaults-1.21.11.json` | `98ba9886b8bdf648e8ff74ffe8c817932e987037111427343613eefa1c37da3d` |
| `block-renames-1.21.11.json` | `255937f801a71bb38fe92e7a5c16da74de934b88311b7ba68b62a0929e6756b5` |
| `block-classification-1.21.11.json` | `58f80ca8bee1ed84e4cc64c3f4fda9d26cfba5f993c015489f3352c824a0e13d` |
| `collision-tops-1.21.11.tsv` | `f4ea1e01f4463272ef527bafcbb36dbe4d8669e58fabfda5ec54c307469196bc` |
| `faces-1.21.11.tsv` | `ba718f855a73609ebd15f9ede83b43fec9f2bc1db2f53de0ba10ad8289ce8ad4` |
| `particles-1.21.11.json` | `a64121b11f5fe66ea4a03a16d655cd09dfe590e5434ea142688078b780b027c7` |

## Not committed

The Mojang server jar is **never** committed (Mojang EULA, ADR-0010). It was not
downloaded on this host (Java 21 unavailable). If a future refresh runs the
official generator, add the jar path to `.gitignore` before generating.

## Derived list vendored elsewhere

- **Enchantment ids (43, 1.21.11)** — the `enchantment` registry array from the
  same `registries/data.min.json` above, namespaced and sorted by the same
  transform. Because it is small and stable it is **inlined** as
  `delvewright_dsl::registry::ENCHANTMENT_IDS_1_21_11` rather than committed as
  a data file here, matching the precedent set by `EFFECT_IDS_1_21_11`. Used by
  `DW0433` to validate actor/wave `equipment` and stage-5 `loot` enchantments
  (spec-0021).
