# spec-0097: A skin wears its second layer — the skin toolchain paints the overlay shell, a sheet is drawn to its own model's boxes, paint no box reads is refused, and a mannequin states which layers it hides

- **Status**: Approved
- **Ground**: written against engine `109355653` (`origin/main`) with the research page `docs/reference/second-layer-skins.md` (branch `docs/second-layer-skins` at `e4d21e939`, carried into this branch). Read: `tools/creator/skin/delve_skin/{compose,wardrobe,preview,cli,catalog}.py` and `tests/test_skin.py`; `NpcSkin`, `SkinModel` and `TextureOverride` in `crates/dsl/src/stages.rs`; `body_skin_sites` in `crates/dsl/src/lib.rs`; `read_skins` in `crates/delvec/src/main.rs`; `resolve` and `judge` in `crates/delvec/src/compiler/textures.rs`; the two mannequin summons in `crates/delvec/src/compiler/emit.rs`; `skin_png` in `prefabs/gallery-generator/src/main.rs`.
- **What it is for**: an NPC with a beard that stands half a texel proud of the jaw, hair with a lip at the fringe, a hood that frames the face, a collar that rings the neck; and a mob retexture that lands on the boxes the mob actually builds. The Stranding's first `drowned_outer_layer` was painted in the player's overlay layout on a model that samples the base layout, and drew a two-pixel stripe on the top of the crown and nothing else; nothing refused it.
- **Research**: §2 is this spec's record. Each statement is **cited** (the research page, the pinned client read by the extractor of §3, or a line of the tree) or **authored** (this spec chooses).
- **Numbers**: spec `0097`. Three diagnostics: `DW0978` (paint no box samples, §4.1), `DW0979` (paint only on faces a body standing level with the model cannot see, §4.2), `DW0980` (a mannequin layer hidden twice, §5). One DSL surface change, `NpcSkin.hidden_layers` (§5); this spec states no version literal — the `dsl_version` bump it owes is assembled with the others into the next minor.
- **Non-goals**: new model geometry (a brim, a crest, a bun, a cloak, a jutting beard — the overlay is a shell and stays one, §2.1); slim composition (`model: slim` is still refused by the composer); a composer for models whose base boxes differ from the player's (villager, piglin, the skeleton family — they get a part table and the refusal, not a wardrobe, §6.4); the zombie villager and the parched, whose vanilla sheets the extractor's cross-check could not reconcile with their models (§3.3).

## 1. The thing, and the object it belongs to

**Cited** (research page, "Which models carry a second layer"): an entity's geometry is fixed in the client by its layer definition, and a texture can only fill the boxes that definition builds. A player-model mannequin builds six overlay boxes — `hat` (grow 0.5), `jacket`, both sleeves, both pants (grow 0.25) — each a child of its base part. A zombie or husk builds only the `hat`. A villager builds a `hat` and an 8×20×6 robe at `0,38`. A piglin builds the jacket, sleeves and pants at the player positions and no hat. A drowned, stray or bogged outer layer is a separate sheet whose boxes are a grown copy of the base, at the base positions.

**Authored — the objects.** Three capabilities, each on the object it acts on:

- **Which box a pixel reaches** is a property of the *model*: one table per model, measured from the pinned client (§3), read by the compiler and by the skin toolchain alike. Never the player's table for a mob.
- **Whether a sheet's paint reaches any box** is a property of the *sheet against its model*, judged wherever a sheet enters a delve: a mannequin skin (`skins/<id>.png`, against `player` or `player_slim` by its `skin.model`) and a `world.textures[]` row whose `replaces` is a texture the table binds to a model (§4).
- **Which overlay layers a mannequin draws** is a property of the *mannequin's skin*: `NpcSkin.hidden_layers`, emitted as the mannequin's own `hidden_layers` field (§5).

## 2. What the tree does today

### 2.1 The overlay is a shell

**Cited** (research page, recommendation 1): each overlay box is its base box grown by a fixed amount on every side, so it stands off the base by exactly that and can carve inside its own outline with transparency (`entityCutoutNoCull`: a pixel draws or is discarded), but cannot grow past it.

### 2.2 The toolchain paints the base only

**Cited** (research page, "What the toolchain draws today"): `delve_skin` addresses the six base parts through skinpy-extended 1.0.1, which defines parts only at the base origins; every overlay region of the three goldens has 0 opaque pixels. The mannequin draws all six overlay boxes (`Mannequin` starts at `ALL_LAYERS`), so each samples transparent pixels: present and empty. `docs/reference/tools.md` claims the composer "cannot" give a figure "a hair silhouette off the cube" for want of the overlay; the overlay does not give one either (§2.1), so the line overclaims what the overlay would buy.

### 2.3 Nothing judges where a sheet's paint lands

**Cited**: `read_skins` reads `skins/<id>.png` and checks only that it exists (`DW0309`). `textures::judge` checks that a `world.textures[]` file is a PNG, that its size is a whole multiple of vanilla's frame, and that it is not vanilla's bytes (`DW0940`); it never asks which pixels a model samples. So the Stranding's drowned sheet, 64×64 and not vanilla, was admitted.

### 2.4 The mannequin's hidden layers

**Cited**, settled by the extractor of §3 where the research page left it open: `Mannequin.HIDDEN_LAYERS_FIELD` is `hidden_layers`; `ALL_LAYERS` is 127; running `Mannequin.LAYERS_CODEC` on a one-element list `["<id>"]` yields `127 & ~mask(<id>)` for each of the seven `PlayerModelPart` ids (`cape`, `jacket`, `left_sleeve`, `right_sleeve`, `left_pants_leg`, `right_pants_leg`, `hat`), and encoding 127 yields `[]`. So the field is a list of the ids the mannequin does not draw, and omitting it draws all of them. **Cited**: the cast entry's `hidden_layers` reaches only the catalog card, and the compiler emits no `hidden_layers`.

## 3. The model-part table

### 3.1 What it holds

`crates/delvec/data/model-parts-1.21.11.json`, one entry per model key (`player`, `player_slim`, `zombie`, `husk`, `drowned`, `drowned_outer_layer`, `skeleton`, `wither_skeleton`, `stray`, `stray_outer_layer`, `bogged`, `bogged_outer_layer`, `villager`, `wandering_trader`, `piglin`, `piglin_brute`, `zombified_piglin`): the `ModelLayers` field it was read from, the texture size, the vanilla textures bound to it, and one row per box — part path, `u`, `v`, `w`, `h`, `d`, `grow`, `mirror`, `posed` (a rotation on its chain), and its `top` and `bottom` above the ground in model pixels at rest. Also the mannequin's field name and layer ids (§2.4), the standing player's eye height in pixels (`EntityType.PLAYER`'s dimensions, 25.92), and the instrument: extractor, dumper, the commit that last touched them, the jar's sha256 and sha1 and the mappings' sha1.

### 3.2 The instrument

**Authored.** `tools/maintenance/extract-model-parts.py` with `tools/maintenance/modelparts/ModelPartDump.java`: boot the client's registries, call `LayerDefinitions.createRoots()`, and write each bound layer's `LayerDefinition` by reflection — the objects the game builds, not constants read off bytecode — with every obfuscated name resolved from piston-meta's client mappings at run time. The jar is refused unless its sha256 is `versions.toml` `[render] textures_sha256` and its sha1 is piston-meta's; the mappings and libraries are verified by sha1. The table is never hand-written, and the extractor refuses to run from an uncommitted instrument.

### 3.3 The second method

**Authored.** Every bound vanilla texture is decoded by a standard-library PNG reader that shares nothing with the dumper, and every opaque pixel is classified: on a face some box samples, in an unwrap corner of a box (the `2(w+d) × (d+h)` rectangle less its faces), or elsewhere. **Cited**, the measurement (commit body of the table): vanilla's own sheets are not clean — `entity/player/wide/kai` carries 48 corner pixels, `entity/player/slim/efe` 4 elsewhere, `entity/villager/type/jungle` 2 elsewhere — so the refusal is per model: at least one bound texture keeps every opaque pixel on a face or a corner. Shifting the hat's `u` by one texel reds it. `parched` (100 pixels elsewhere on its only sheet) and `zombie_villager` (its profession sheets carry the villager's crossed-arms region, 160 pixels on `armorer`, which its own model does not build) fail it and are not bound.

## 4. Paint no box reads is refused

The footprint of a model on a sheet `k` times its texture size is the union, over its boxes, of the six face rectangles of the box unwrap (research page, "The box unwrap"), each scaled by `k`. One function in `delvec::compiler::skinparts` computes it and judges a sheet; both entry points call it.

### 4.1 `DW0978` — paint no box samples

A sheet with any pixel of non-zero alpha outside its model's footprint is refused. The message names the model, the count, and the box regions nearest the first stray pixels, and — where the stray paint sits wholly on another bound model's footprint, as a player-layout sheet on the drowned outer layer does — names that model as the layout the sheet was drawn to. **Authored**: any pixel, not a fraction. The composer never paints outside a footprint (§6), a corner left by a template editor is a pixel the creator did not mean, and a threshold would admit the Stranding's sheet, which was all stray but sixteen pixels.

### 4.2 `DW0979` — paint only where a body standing level with the model cannot see

A sheet whose every opaque pixel lies on the footprint, at least one does, and every one lies on an *unseen* face is refused. A face is unseen when it is the `up` face of an unposed box in the `head` or `body` subtree whose `top` is above the standing eye height, or the `down` face of such a box whose `bottom` is below it. **Authored**: limbs swing and a posed box is rotated, so neither is ever classified unseen; the judgement is at the model's own scale, which is the scale a texture is drawn at for every instance of the entity. The Stranding's sixteen pixels on the top face of the drowned outer hat are this case.

### 4.3 Where it runs, and why the compiler

- A `world.textures[]` row whose `replaces` is bound in the table: in `textures::resolve`, after `DW0940`, validation tier — so `delvec validate` and every build refuse it.
- A mannequin skin: in `read_skins`, against `player` for `model: wide` and `player_slim` for `slim`, build tier, beside `DW0309` — so `build` and `edit`'s build-tier proof refuse it. A texture two bodies name under two models is judged against both.

**Authored**: a compiler diagnostic, not a tool check, because the compiler is the one place every sheet passes on its way into a delve — a skin the composer never touched, a mob sheet drawn campaign-side — and the table it judges by is the compiler's vendored data. The composer cannot produce either refusal by construction (§6.1), and its tests assert that through the compiler's own rule rather than a second copy of it (§8).

## 5. A mannequin states which layers it hides

`NpcSkin` gains `hidden_layers`: a list of `SkinLayer` — `cape`, `jacket`, `left_sleeve`, `right_sleeve`, `left_pants_leg`, `right_pants_leg`, `hat`, the pinned `PlayerModelPart` ids, `left`/`right` being the model's own. Optional; absent or empty draws every layer, which is today's behaviour, and emits nothing. Non-empty, both mannequin summons (a staged NPC and an actor body) emit `hidden_layers:["<id>",…]` inside the summon's NBT, in authored order. **Authored**: a list, because the vanilla field is a list; a layer named twice is a mistake the creator made, refused where it is written as `DW0980` (validation tier), because two spellings of one fact is the shape the constitution refuses. The `SkinLayer` members are held equal to the table's `mannequin.layers` by a test.

The cast entry's `hidden_layers` in the skin toolchain is validated against the same ids and carried to the catalog card, so the card a creator picks from says what the mannequin will draw.

## 6. The composer paints the overlay

### 6.1 Addressing

**Authored.** The composer's canvas is built from the model's table, not from skinpy-extended's fixed layout: each box becomes a skinpy `BodyPart` at its own `texOffs` and size, so every pixel is still addressed as part, face, x, y. The composer's base part ids (`head`, `torso`, `left_arm`, …, skinpy's observer-relative names) are mapped to the model's parts by matching skinpy's own base origins against the `player` table's base boxes, never by name. A part's *shell* is its child box of equal size and positive grow (`hat` over `head`, `jacket` over `body`, …); on an outer-layer model (no box of grow 0) a top-level part is its own shell and the base is not painted. A composer part a model does not build, or builds only as a mirror of another box's UV, is addressed into a discarded buffer, so the seeded stream is consumed identically and the paint lands nowhere. The shell is painted after the whole base, so a sheet's base pixels are the pixels it composed before this spec.

### 6.2 Features

**Authored**, placed as the research page's recommendation 3 says:

- **Beard** (`facial_hair: beard`): the head shell's front at the chin, mouth and lip rows (columns as the base), its two sides at those rows, its whole bottom face. A moustache is the shell's lip row. The base beard stays.
- **Hair** (every length but `bald`): the head shell's top and back; its sides down to the length `hair` names; the fringe on the shell front's top row; for a length that frames the face, its outer columns. `long` continues on the body shell's back at the shoulder rows.
- **Hood** (new axis `hood`: `none`, default, or `up`): every head-shell face but the bottom, the front left open below the fringe row and inside the outer columns so the face reads through it; the fall on the body shell's top face and the top three rows of its back. Painted in the new palette key `hood` (default `tunic`), its rim in `hood_shadow` (default `hood` darkened). A hood replaces the hair shell.
- **Collar** (`collar` gains `high`): `closed`'s cloth at the throat, plus the body shell's top two rows on all four sides, in `tunic` with the lower row in `tunic_shadow`.

A feature that needs a shell its model does not build is refused by name (a `high` collar on a zombie, which has no body shell); a feature with a base half and a shell half keeps its base half where the shell is absent.

### 6.3 Which models it dresses

A cast entry gains `entity`: `mannequin` (default; `model` required as before) or a mob model the wardrobe fits (`model` refused: a mob has one shape). **Authored**: the wardrobe is written for a body whose head, torso and limbs are the player's size, so the set is derived from the table — every model whose composer parts' own boxes (base, or on an outer layer its grown box) have the player's base dimensions. At the pinned client that is `zombie`, `husk`, `drowned`, `drowned_outer_layer`, `stray_outer_layer` and `bogged_outer_layer`. Any other table key is refused by name, pointing at `python -m delve_skin parts <model>`. A cast entry's `hidden_layers` is a mannequin's and is refused on a mob.

### 6.4 Every model's table, for a sheet drawn elsewhere

`python -m delve_skin parts <model>` prints a model's boxes from the table — part, layer (base or shell), `texOffs`, size, grow, and the six face rectangles — for any key in the table, so a mob sheet drawn campaign-side is drawn to its own model's layout.

### 6.5 Previews

The four previews flatten each shell's opaque pixels onto the base face beneath before projecting, so a reviewer sees what the shell covers. The stand-off itself is not drawn; the README says so.

## 7. The gallery's obligation, and the demo level

- `npcs.json`: one skinned NPC declares `hidden_layers: ["cape"]` — bound when the summon's bytes move with it.
- `probes/a-layer-hidden-twice`: that list with `cape` twice — `DW0980`.
- The gallery generator's two skins paint only the `player` / `player_slim` footprint, read from the table, so the gallery's own skins pass `DW0978`.
- `world.json` gains a row replacing `minecraft:entity/zombie/drowned_outer_layer` with a generated sheet painted at the base positions of the outer model.
- `probes/a-texture-drawn-to-another-model`: that row pointed at a generated sheet painted in the player's overlay layout — `DW0978`.
- `probes/a-texture-painted-on-the-crown`: that row pointed at a generated sheet painted only on the top face of the outer hat — `DW0979`.
- `probes/a-skin-painted-off-its-boxes`: a skinned NPC pointed at a generated skin carrying paint in a head-unwrap corner — `DW0978`, build tier.
- `docs/demo-levels.md` gains a queued row: four NPCs (a beard, long hair, a hood, a high collar) and one mob of each overlay kind — a zombie with a hat shell, a villager with a robe, a piglin with a jacket, a drowned with an outer layer — judged at playable scale.

## 8. Decisions

1. The table is measured by running the client's own builders, not by reading bytecode constants; the research page's bytecode reading is the independent record it agrees with.
2. `DW0978` refuses any stray pixel; `DW0979` refuses only a sheet whose every pixel is unseen.
3. The refusal is a compiler diagnostic at both entry points (§4.3).
4. `hidden_layers` is a list on `NpcSkin`, the object it configures; a duplicate is `DW0980`.
5. The composer's overlay is automatic for every beard and every head of hair; a creator who wants a flat head hides `hat` on the mannequin.
6. `parched` and `zombie_villager` are left out of the table until a second method reconciles them.

## Acceptance criteria

1. `crates/delvec/data/model-parts-1.21.11.json` exists, states `client_jar_sha256` equal to `versions.toml` `[render] textures_sha256`, an `instrument.revision` that is a commit touching `tools/maintenance/extract-model-parts.py` or `tools/maintenance/modelparts/ModelPartDump.java`, and the 17 model keys of §3.1; a `delvec` unit test asserts the `player` model's six overlay boxes at `32,0`, `16,32`, `40,32`, `48,48`, `0,32`, `0,48` with grows 0.5 and 0.25, the `zombie` model's only grown box at `32,0`, and the `drowned_outer_layer` model's boxes at the base positions.
2. The extractor refuses unless, for each of the 17 models, at least one bound vanilla texture keeps every opaque pixel on a face or an unwrap corner, and prints every bound texture's counts; the table's commit body carries the run (59 textures).
3. A `world.textures[]` row replacing `minecraft:entity/zombie/drowned_outer_layer` with a sheet painted in the player's overlay layout fails `delvec validate` with `DW0978`; one painted only on the outer hat's top face fails with `DW0979`; one painted at the base positions passes. Each is a committed gallery probe and a unit test.
4. A mannequin skin with one opaque pixel in the head unwrap's top-left corner fails `delvec build` with `DW0978`; a slim skin is judged against `player_slim` (a pixel on the last column of the wide arm's back face refused, the same skin under `wide` admitted).
5. `NpcSkin.hidden_layers: ["hat", "jacket"]` emits `hidden_layers:["hat","jacket"]` in the staged NPC's and the actor's mannequin summon; an absent or empty list emits no `hidden_layers`; `["cape","cape"]` fails `delvec validate` with `DW0980`; a test holds the `SkinLayer` tokens equal to the table's `mannequin.layers`.
6. `pytest tools/creator/skin` passes; the three goldens are regenerated and the commit body attributes every moved byte to the overlay regions — their base regions are byte-identical to `109355653`'s.
7. The composer's tests read each feature back through the shell's own face addressing: a beard on the hat front's chin row, a fringe on its top row, `long` on the jacket back's shoulder rows, a hood leaving the face rows of the hat front transparent, `high` on the jacket sides' top two rows; and every fixture's composed PNG, judged by the compiler's rule through `delvec`'s own test fixture, draws neither `DW0978` nor `DW0979`.
8. `entity: zombie` composes a sheet with no opaque pixel at `32,48` or `16,48` and paint on the hat; `entity: drowned_outer_layer` paints at the base positions and nothing at the player overlay positions; `entity: villager` and `collar: high` with `entity: zombie` are refused by name; `python -m delve_skin parts piglin` names no `hat`.
9. The overlay's opaque pixels per part on each golden are counted by a reader that does not import `delve_skin` and recorded in a commit body.
10. `docs/reference/compiler.md` has rows for `DW0978`, `DW0979`, `DW0980` and the `hidden_layers` field; `docs/reference/tools.md` lists `extract-model-parts.py` and no longer claims the overlay would give a hair silhouette; the skill page that tells the agent how to author NPC skins describes the shell features, `hood`, `collar: high`, `entity` and `hidden_layers` without naming an unreleased DW code; `docs/demo-levels.md` carries the queued row of §7.
