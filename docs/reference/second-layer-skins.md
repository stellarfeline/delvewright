# Second-layer skins: which models carry an overlay, and where it reads

For the agent who maintains the skin toolchain (`tools/creator/skin/`) and anyone drawing a `world.textures[]` row for a humanoid mob. It answers four questions: which 1.21.11 models have a second layer and where its UVs are; whether a resource-pack retexture is the same mechanism; what the toolchain draws today; and how beards, hair, hoods and collars should be authored.

Every rule is marked **cited** (read from the pinned client or another named source) or **authored** (a judgement made here). Mojang's code is not redistributable: this page cites classes and methods by name and states their values. It copies no code.

## Instrument

- Client jar: `minecraft-client-1.21.11.jar`, sha256 `1473c9489ac50fda3c435049a76a70d61a10b8610db27f5ba9d8756b686cd3bd` (the `[viewer] textures_sha256` pin in `versions.toml`), sha1 `ba2df812c2d12e0219c489c4cd9a5e1f0760f5bd`, which equals piston-meta's `downloads.client.sha1` for 1.21.11.
- Mappings: piston-meta `downloads.client_mappings` for 1.21.11, sha1 `031a68bebf55d824f66d6573d8c752f0e1bf232a`, 155819 lines.
- Method: `javap -c -p` on the obfuscated class, with class, method and field names translated back through the ProGuard mappings. The constants passed to `CubeListBuilder.texOffs`, `addBox`, `CubeDeformation` and `LayerDefinition.create` were read in call order. No decompiler was used. Every value below is a bytecode constant, so none of it is inferred.
- Cross-check (second method, no shared configuration): opaque-pixel counts per UV region of the vanilla textures in the same jar, compared with the box-unwrap area each model cube samples (see "Cross-check").

Units: one model unit is one texel on these 64-wide textures. `grow` is `CubeDeformation`, the outward growth on each side of a box. A 0.5 grow turns an 8×8×8 head into a 9×9×9 shell.

## The box unwrap

**Cited** from `skinpy.skin.Face.new` (skinpy-extended 1.0.1, MIT), an independent implementation, and matched by the area counts below. A box of width `w`, height `h` and depth `d` at `texOffs(u, v)` samples:

| face | origin | size |
|---|---|---|
| top | `(u+d, v)` | `w×d` |
| bottom | `(u+d+w, v)` | `w×d` |
| right side (observer's left) | `(u, v+d)` | `d×h` |
| front | `(u+d, v+d)` | `w×h` |
| left side | `(u+d+w, v+d)` | `d×h` |
| back | `(u+2d+w, v+d)` | `w×h` |

A pixel outside every box's footprint is never sampled, whatever is painted there.

## Which models carry a second layer (cited)

A model's layer definition decides where it draws. The texture does not. `LayerDefinitions.createRoots` binds each `ModelLayers` entry to a builder, and the builder fixes every box, its `texOffs` and its grow.

### Player and mannequin: `PlayerModel.createMesh(CubeDeformation, boolean slim)`, 64×64

`ModelLayers.PLAYER` and `PLAYER_SLIM` are both `LayerDefinition.create(PlayerModel.createMesh(NONE, slim), 64, 64)`. The base comes from `HumanoidModel.createMesh` and is then overridden.

| part | parent | texOffs | box (wide) | grow |
|---|---|---|---|---|
| head | root | 0,0 | 8×8×8 | 0 |
| **hat** | head | **32,0** | 8×8×8 | **0.5** |
| body | root | 16,16 | 8×12×4 | 0 |
| **jacket** | body | **16,32** | 8×12×4 | **0.25** |
| right_arm | root | 40,16 | 4×12×4 (slim 3) | 0 |
| **right_sleeve** | right_arm | **40,32** | 4×12×4 (slim 3) | **0.25** |
| left_arm | root | 32,48 | 4×12×4 (slim 3) | 0 |
| **left_sleeve** | left_arm | **48,48** | 4×12×4 (slim 3) | **0.25** |
| right_leg | root | 0,16 | 4×12×4 | 0 |
| **right_pants** | right_leg | **0,32** | 4×12×4 | **0.25** |
| left_leg | root | 16,48 | 4×12×4 | 0 |
| **left_pants** | left_leg | **0,48** | 4×12×4 | **0.25** |

These are the six player overlay parts. Each one is a child of its base part, so it follows that part's animation exactly. The 0.5 and 0.25 grows agree with the Minecraft Wiki "Skin" page, which says the outer layer is 1 px bigger on the head and 0.5 px bigger on the body and limbs. That page is CC BY-NC-SA, so it is used as corroboration only (ADR-0013).

The mannequin renders through the same model. `ClientMannequin` extends `Mannequin` and implements `ClientAvatarEntity`, and `EntityRenderers.createAvatarRenderers` builds an `AvatarRenderer` for each `PlayerModelType`. `AvatarRenderer.extractRenderState` sets `showHat`, `showJacket`, `showLeftSleeve`, `showRightSleeve`, `showLeftPants` and `showRightPants` from `Avatar.isModelPartShown(PlayerModelPart.*)`, and `PlayerModel.setupAnim` reads those six flags.

The `Mannequin` constructor sets the customisation byte to `ALL_LAYERS`. `Mannequin.readAdditionalSaveData` reads `hidden_layers`, a list of `PlayerModelPart` ids, through `LAYERS_CODEC`, and falls back to `ALL_LAYERS`. **So a mannequin summoned without `hidden_layers` draws all six overlay parts.** The exact bit combinator in `LAYERS_CODEC` was not traced instruction by instruction, but the field name, the fallback and the `(byte & mask)` filter all agree with this reading.

### Zombie and husk: `HumanoidModel.createMesh(NONE, 0)`, 64×64

`ModelLayers.ZOMBIE` is `LayerDefinition.create(HumanoidModel.createMesh(NONE, 0), 64, 64)`. `HUSK` is the same definition through `MeshTransformer.scaling(1.0625)`.

| part | texOffs | box | grow |
|---|---|---|---|
| head | 0,0 | 8×8×8 | 0 |
| **hat** (child of head) | **32,0** | 8×8×8 | **0.5** |
| body | 16,16 | 8×12×4 | 0 |
| right_arm | 40,16 | 4×12×4 | 0 |
| left_arm | 40,16, mirrored | 4×12×4 | 0 |
| right_leg | 0,16 | 4×12×4 | 0 |
| left_leg | 0,16, mirrored | 4×12×4 | 0 |

The zombie has **one** second-layer part: the hat. It has no jacket, sleeves or pants. Its left limbs mirror the right ones, so the `16,48` and `32,48` regions of `zombie.png` and `husk.png` are never sampled. The vanilla `zombie.png` and `husk.png` leave the hat region fully transparent (0 opaque pixels), but the hat box is still built and drawn.

### Drowned: two models, two textures, one UV layout

`DrownedModel.createBodyLayer(CubeDeformation g)` calls `HumanoidModel.createMesh(g, 0)` and then replaces `left_arm` with `texOffs(32,48)` and `left_leg` with `texOffs(16,48)`. Both boxes take `g` and are not mirrored. `LayerDefinition.create(..., 64, 64)`.

- `ModelLayers.DROWNED` is `createBodyLayer(NONE)`, drawn with `textures/entity/zombie/drowned.png`.
- `ModelLayers.DROWNED_OUTER_LAYER` is `createBodyLayer(new CubeDeformation(0.25))`. `DrownedOuterLayer` bakes it and draws it with `textures/entity/zombie/drowned_outer_layer.png` through `RenderLayer.coloredCutoutModelCopyLayerRender`, which copies the base model's pose.

| part | texOffs | box | base grow | outer grow |
|---|---|---|---|---|
| head | 0,0 | 8×8×8 | 0 | 0.25 |
| hat | 32,0 | 8×8×8 | 0.5 | 0.75 |
| body | 16,16 | 8×12×4 | 0 | 0.25 |
| right_arm | 40,16 | 4×12×4 | 0 | 0.25 |
| left_arm | 32,48 | 4×12×4 | 0 | 0.25 |
| right_leg | 0,16 | 4×12×4 | 0 | 0.25 |
| left_leg | 16,48 | 4×12×4 | 0 | 0.25 |

**The drowned's outer layer is a full second copy of the base model, grown by 0.25 and sampled from its own texture at the base UV positions.** It has no jacket, sleeve or pants boxes. A pixel painted on `drowned_outer_layer.png` at a player-overlay position (`16,32`, `40,32`, `48,48`, `0,32`, `0,48`) is never sampled. Only the hat region `32,0` exists in both layouts, and on the outer model that hat is a 9.5×9.5×9.5 shell (0.75 grow) around a 0.25-grown head.

The Stranding hit exactly this. Its first `drowned_outer_layer` replacement had 16 opaque pixels, all at `u = 43–44, v = 0–7`. By the unwrap above, that region is the **top face** of the outer hat box (top face origin `(40,0)`, 8×8). Nothing else on the sheet was in a sampled region. So the model drew a 2-px stripe on top of the crown, a face that a player standing level with or below the mob does not see. Everything else drew nothing. A "fin crest" is not geometry on this model: no box sticks up off the head. That the stripe went unseen at eye level is **computed** from the UV map, not observed in game.

### Skeleton family: `SkeletonModel`, 64×32

- `ModelLayers.SKELETON` and `STRAY` are `SkeletonModel.createBodyLayer()`: `HumanoidModel.createMesh(NONE, 0)` followed by `createDefaultSkeletonMesh`, which replaces all four limbs with 2×12×2 sticks (`right_arm 40,16`, `left_arm 40,16` mirrored, `right_leg 0,16`, `left_leg 0,16` mirrored), and `LayerDefinition.create(..., 64, 32)`. The humanoid **hat at 32,0 (grow 0.5) survives**, so the skeleton's base model has a hat shell. `WITHER_SKELETON` is the same definition scaled by 1.2.
- `STRAY_OUTER_LAYER` is `create(HumanoidModel.createMesh(new CubeDeformation(0.25), 0), 64, 32)`, drawn with `stray_overlay.png` by `SkeletonClothingLayer`. `BOGGED_OUTER_LAYER` uses grow 0.2 and `PARCHED_OUTER_LAYER` uses 0.25, same shape. These outer layers are the **full humanoid with 4-wide limbs**, not sticks: hat at 32,0 (grow 0.25 + 0.5), mirrored left limbs, base UV positions, a 64×32 sheet. Like the drowned, the outer layer is a grown copy at the base positions.
- `PARCHED` (the base) is `SkeletonModel.createSingleModelDualBodyLayer()`, a single 64×64 sheet carrying both layers on its own cubes: body 16,16, a 8×1×4 belt at 28,0, outer body 16,48 (grow 0.025), head 0,0, outer head 0,32 (grow 0.2), outer arms 42,33 and 40,48 (3×12×3), outer legs 0,49 and 4,49 (3×12×3), and an emptied `hat`. The parched variant was not cross-checked against its texture.

### Villager and wandering trader: `VillagerModel.createBodyModel()`, 64×64

| part | texOffs | box | grow |
|---|---|---|---|
| head | 0,0 | 8×10×8 | 0 |
| **hat** | **32,0** | 8×10×8 | **0.51** |
| hat_rim | 30,47 | 16×16×1, rotated −π/2 about x | 0 |
| nose | 24,0 | 2×4×2 | 0 |
| body | 16,20 | 8×12×6 | 0 |
| **jacket** | **0,38** | 8×20×6 | **0.5** |
| arms (one part, crossed) | 44,22 ×2 (one mirrored), 40,38 | 4×8×4, 4×8×4, 8×4×4 | 0 |
| right_leg / left_leg | 0,22 (left mirrored) | 4×12×4 | 0 |

`VILLAGER` and `WANDERING_TRADER` share this definition. `VILLAGER_NO_HAT` is `createNoHatModel()`, which clears the head's children. The villager's hat and jacket are second-layer parts in **the same texture as the base, at their own positions**. The jacket is a 20-tall robe that hangs over the legs. `VillagerProfessionLayer` paints the type, profession and level textures over the same cubes through `renderColoredCutoutModel`, picking the hat or no-hat model from the texture's `VillagerMetadataSection` `hat` value (`none` / `partial` / `full`). Those are stacked textures on one geometry, not more geometry.

### Piglin family: `AbstractPiglinModel.createMesh(CubeDeformation)`, 64×64

`createMesh` calls **`PlayerModel.createMesh(g, false)`**, replaces `body` (16,16, 8×12×4), replaces the head through `addHead` (0,0, 10×8×8, plus a 4×4×1 snout at 31,1, tusks at 2,4 and 2,0, and ears at 51,6 and 39,6, each 1×5×4), and **clears `hat`**. The `AbstractPiglinModel` constructor calls `getChild` for `jacket`, `left_sleeve`, `right_sleeve`, `left_pants` and `right_pants`, which would throw if they were absent, and `setAllVisible` toggles them with the rest of the model. `PIGLIN`, `PIGLIN_BRUTE` and `ZOMBIFIED_PIGLIN` all use this definition (`$25` in `createRoots`).

**So a piglin has the player's jacket, sleeves and pants at the player positions, grow 0.25, and no hat.** Vanilla `piglin.png` leaves those five regions fully transparent (0 opaque pixels), but the boxes are drawn. The piglin's 10-wide head unwraps across `u = 0–36`, so `32,0` on a piglin sheet is head, not hat.

### What a cube draws

`HumanoidModel(ModelPart)` passes `RenderTypes.entityCutoutNoCull` (**cited**). This is alpha cutout, so a second-layer pixel either draws or is discarded. Inside the shell, transparency punches holes. It cannot shape the outline. For the player, the wiki page cited above says overlay layers draw partial alpha. Whether `AvatarRenderer` uses a translucent render type for a resource-pack mannequin texture was **not traced** and is unverified.

## Is a resource-pack retexture the same mechanism? (cited, then authored)

A resource pack can only replace texture files. Geometry is fixed in the client by the layer definitions above, and no 1.21.11 resource-pack surface changes an entity model (**cited**: models are built in code by `LayerDefinitions.createRoots`, not loaded from assets).

So a retexture **is** the second layer exactly when the target model already has a box at the painted UV, and does nothing otherwise:

| entity | second-layer boxes a retexture can fill | texture | positions |
|---|---|---|---|
| player / mannequin | hat, jacket, both sleeves, both pants | the skin itself | player overlay positions |
| zombie, husk | hat only | `zombie.png`, `husk.png` (same sheet as base) | 32,0 |
| drowned | a full grown copy of the base model | `drowned_outer_layer.png` (separate) | **base** positions |
| skeleton, wither skeleton | hat only | base sheet, 64×32 | 32,0 |
| stray, bogged | hat on base **and** a full grown humanoid outer | `stray_overlay.png` / `bogged_overlay.png` (separate, 64×32) | **base** positions, 4-wide limbs |
| villager, wandering trader | hat, jacket (robe) | same sheet as base | 32,0 and 0,38 |
| piglin, brute, zombified piglin | jacket, both sleeves, both pants (no hat) | same sheet as base | player overlay positions |

A sheet painted in the player layout on a model that does not have the player layout is the defect class: the paint goes to UVs that no cube samples.

## What the toolchain draws today (cited from the tree)

- `delve_skin` composes **one 64×64 wide-model player skin** per cast entry. It addresses the six base parts through skinpy-extended 1.0.1, whose `skin.py` carries `# TODO: Second layer` and defines parts only at the base origins (`0,0`, `16,16`, `40,16`, `32,48`, `0,16`, `16,48`).
- Measured on the three goldens in `tools/creator/skin/tests/fixtures/golden/`: every base region is fully opaque and every overlay region (`32,0`, `16,32`, `40,32`, `48,48`, `0,32`, `0,48`) has **0** opaque pixels.
- The skin is worn by a **mannequin**. `crates/delvec/src/compiler/emit.rs` summons `minecraft:mannequin` with `profile:{texture:"delvewright:npc/<id>/<texture_id>",model:"<wide|slim>"}`, for a skinned NPC and for an actor body. It emits no `hidden_layers`. The cast entry's `hidden_layers` reaches only the catalog card (`catalog.py`).
- So each shipped NPC draws all six overlay boxes, and every one samples transparent pixels. The overlay is **present and empty**. Filling it needs no engine change: the same PNG carries it.
- Mob textures (`world.textures[]`, spec-0084) are drawn outside this toolchain, by campaign-side scripts.

## Recommendation (authored)

1. **The overlay is a shell, not new geometry.** Each part is its base box grown by a fixed amount: 0.5 per side on the head, 0.25 per side on the body and limbs. A beard, hair, a hood or a collar "stands off" by exactly that and no more. It can be carved inside the shell with transparency, but the silhouette cannot grow past it. A brim, a jutting beard, a crest, a bun or a cloak cannot be built on any of these models. The toolchain's present refusal of those stays correct. What the overlay buys is depth: a beard that sits half a texel proud of the jaw, hair with a lip at the fringe, a hood opening, a collar ring.
2. **Address the overlay through the same face unwrap the composer already uses**, never as raw atlas arithmetic. That is a second part table beside skinpy's base table: `hat 32,0 8×8×8`, `jacket 16,32 8×12×4`, `right_sleeve 40,32`, `left_sleeve 48,48`, `right_pants 0,32`, `left_pants 0,48` (4×12×4 wide). Derive it from the pinned `PlayerModel.createMesh` with a maintenance extractor in the style of `tools/maintenance/extract-*.py`, not by hand. The face unwrap is the one in "The box unwrap".
3. **Placement, for each feature, on the shell faces that bound it:**
   - Beard: the hat's front face, lower rows, mirroring the base beard rows the composer already draws (chin 0, mouth 1, lip 2); the hat's two side faces at the jaw rows; the hat's bottom face along the front edge. Leave the base beard in place, so the face reads where the shell is transparent.
   - Hair: the hat's top face, its back face, and its side faces down to the length the `hair` axis names; the fringe on the hat front's top row or rows. `long` continues on the jacket's back face.
   - Hood: every hat face except an opening on the front face (the face rows stay transparent), with the hood's fall continuing on the jacket's top face and the upper rows of its back.
   - Collar: the jacket's top face around its rim and the top one or two rows of its four sides. A closed collar can also take the hat's bottom face at the neck line.
4. **For a mob with an overlay, the table is per model, never the player table.** Zombie and husk: the hat only, in the same sheet. Villager: hat 8×10×8 and the 8×20×6 robe at 0,38. Piglin: jacket, sleeves and pants at the player positions, and no hat. Drowned, stray and bogged: a separate sheet whose overlay is a grown copy of the **base** positions, drawn as a second base skin, not as an overlay layout. Derive each table from the pinned client the same way as the player table.
5. **Refuse paint no cube samples.** A sheet with opaque pixels outside its target model's footprint is the Stranding defect. A sheet whose opaque pixels fall only on faces a standing player cannot see (the tops of grown hats) is its milder form. Both are decidable from the derived part table and the unwrap. Refusing them at the point where a texture is declared is a toolchain or compiler obligation for a later spec. This round does not build it.

## Cross-check

Opaque-pixel counts per UV region in the pinned jar's own textures, against the unwrap area of the cube at that position (`2wd + 2dh + 2wh`):

| region (texOffs, box) | box area | `zombie` / `husk` | `drowned` | `steve` | `piglin` | `drowned_outer_layer` |
|---|---|---|---|---|---|---|
| head 0,0 (8×8×8) | 384 | 384 / 384 | 384 | 384 | 440 (10-wide head and snout) | 360 |
| body 16,16 (8×12×4) | 352 | 352 / 352 | 352 | 352 | 352 | 209 |
| right arm 40,16 (4×12×4) | 224 | 224 / 224 | 224 | 224 | 224 | 219 |
| right leg 0,16 (4×12×4) | 224 | 224 / 224 | 224 | 224 | 224 | 221 |
| left arm 32,48 (4×12×4) | 224 | 0 / 0 (mirrored) | 224 | 224 | 224 | 218 |
| left leg 16,48 (4×12×4) | 224 | 0 / 0 (mirrored) | 224 | 224 | 224 | 218 |
| jacket 16,32 | 352 | 0 / 0 | 0 | 0 | 0 | 0 |
| right sleeve 40,32 | 224 | 0 / 0 | 0 | 16 | 0 | 0 |
| left sleeve 48,48 | 224 | 0 / 0 | 0 | 16 | 0 | 0 |
| right pants 0,32, left pants 0,48 | 224 each | 0 / 0 | 0 | 0 | 0 | 0 |

Mojang's own `drowned_outer_layer.png` paints only the base positions and nothing at the player-overlay positions. That agrees with the bytecode reading that the outer model samples base UVs. Full regions equal the exact box-unwrap areas, which agrees with the unwrap table.

## Not established

- The `Mannequin.LAYERS_CODEC` bit arithmetic, read from its shape rather than traced instruction by instruction (see the mannequin section).
- The render type `AvatarRenderer` uses for overlay alpha on a resource-pack mannequin texture.
- `BoggedModel.createBodyLayer`, the bogged base, was not read. Only `BOGGED_OUTER_LAYER` was.
- Nothing on this page was observed in a running client. Every claim is read from the jar's bytecode and textures.
