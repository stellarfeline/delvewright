# A fish on a line, an axolotl in a bucket, a cat's coat — what the pinned game does

Reader: the content work that composes an errand out of vanilla acts — fishing for a body that asks for fish, carrying a creature in a bucket from one pond to another, a cat that speaks — and the engine work that judges each step's outcome (a `collect` with `gathered`, `docs/reference/compiler.md`). Every statement is **[cited]**, read from the pinned jar at the read site below, or **[authored]**, a judgement made here from what was read. Established once and treated as constant (CLAUDE.md, *A check guards only what our toolchain owns*); the read site is stated so the reading can be repeated when the pin moves.

## The read site

- The server jar `versions.toml [minecraft].server_jar_url`, sha256 `f83b8e093865806f931c7e34aae41b177d4c076335263dd124c75d6d65dd1726` (the pin), its inner `META-INF/versions/1.21.11/server-1.21.11.jar`.
- The ProGuard mappings piston-meta publishes for the version, `downloads.server_mappings`, sha1 `5621e9253f05fd57872bbe7f8ddf5f9a7d525955`, which give every class and member below its Mojang name; no obfuscated name is written here.
- `javap -p -c -s` from a JDK 21 over `Bucketable`, `axolotl.Axolotl`, `BucketItem`, `MobBucketItem`, `ItemUtils`, `player.Player` (`mayUseItemAt`, `interactOn`), `ServerPlayerGameMode` (`useItem`), `FishingRodItem`, `projectile.FishingHook`, `feline.Cat`, `variant.VariantUtils` and `commands.SummonCommand`, members resolved by name and descriptor; and the jar's own data files `data/minecraft/loot_table/gameplay/fishing.json`, `data/minecraft/loot_table/gameplay/fishing/fish.json` and `data/minecraft/cat_variant/*.json`.

## 1. A water bucket picks up an axolotl

**[cited]**

- **`NoAI` and `Invulnerable` do not stop the pickup.** `Axolotl.mobInteract` returns `Bucketable.bucketMobPickup(player, hand, this)` when it is present, and the pickup asks exactly two things: the item in the hand is `Items.WATER_BUCKET`, and the body `isAlive()`. Neither `isNoAi` nor `isInvulnerable` is read. `Player.interactOn` refuses only a spectator; an adventure-mode player reaches `mobInteract`. The body is then removed (`discard`), and `CriteriaTriggers.FILLED_BUCKET` fires for the player with the new bucket.
- **What the bucket keeps.** `Bucketable.saveDefaultDataToBucketTag` writes the body's custom name as the bucket's `minecraft:custom_name` component, and into its `minecraft:bucket_entity_data` component `NoAI`, `Silent`, `NoGravity`, `Glowing` and `Invulnerable` (each only when set) and `Health`. `Axolotl.saveToBucketTag` adds the `minecraft:axolotl/variant` component and `Age`, and `HuntingCooldown` when the memory is set. Nothing else is written: **`Tags` are not kept**, and neither is any other field (the body's UUID among them).
- **What the poured body gets back.** `MobBucketItem.spawn` creates the body from the bucket stack (`EntityType.create` with the stack's default config, reason `BUCKET`), which applies the stack's `custom_name`, then calls `loadFromBucketTag` on `bucket_entity_data` — `NoAI`, `Silent`, `NoGravity`, `Glowing`, `Invulnerable`, `Health`, and for an axolotl `Age` and `HuntingCooldown` — and `setFromBucket(true)`. An axolotl from a bucket persists (`requiresCustomPersistence` and `removeWhenFarAway` both read `fromBucket`).

**[authored]** A body the story names keeps its name and its stillness through the bucket and loses its tags. Anything the engine finds a body by through a tag — a cast member, a watched body, a census selector — does not find the poured axolotl; it is found by kind, place or name, or the story hands it a tag again once it is poured.

## 2. Emptying an axolotl bucket in adventure mode

**[cited]**

- **The pour is a block placement, and adventure refuses it without `can_place_on`.** `MobBucketItem` does not override `use`; `BucketItem.use` casts its ray with `ClipContext.Fluid.NONE` for a full bucket (it looks through water to the block behind it), and refuses (`InteractionResult.FAIL`) unless `Level.mayInteract` and `Player.mayUseItemAt(target.relative(face), face, stack)` both hold. `mayUseItemAt` is true for a player whose abilities allow building; for any other — an adventure-mode player — it asks `ItemStack.canPlaceOnBlockInAdventureMode` of the block at `target.relative(face).relative(face.getOpposite())`, which is **the block the player's ray hit**. So the bucket must carry a `minecraft:can_place_on` component whose predicate matches the block the player clicks: the pond's floor or rim block, not the water.
- **A bucket the player fills themselves carries no `can_place_on`.** `Axolotl.getBucketItemStack` is `new ItemStack(Items.AXOLOTL_BUCKET)`, and `ItemUtils.createFilledResult` puts that stack in the player's hand (or inventory) as it is: no component of the water bucket that scooped the axolotl is copied onto it.

**[authored]** In adventure mode a player can scoop an axolotl (§1) and cannot pour it out of the bucket they scooped it into: that bucket has no `can_place_on`, whatever the water bucket carried. Only an `axolotl_bucket` handed over already carrying `can_place_on` for the receiving pond's block can be poured by hand. Whether the errand's carry is built another way is a design question; nothing in the pinned game lets the scooped bucket be poured in adventure mode as it stands.

## 3. Fishing in adventure mode

**[cited]**

- **An adventure-mode player can fish.** `ServerPlayerGameMode.useItem` refuses only a spectator before it calls `ItemStack.use`, and `FishingRodItem.use` casts or reels in with no game-mode or build check.
- **The catch.** `gameplay/fishing` draws one entry: `gameplay/fishing/fish` weight 85 (quality −1), `gameplay/fishing/junk` weight 10 (quality −2), and `gameplay/fishing/treasure` weight 5 (quality 2) **only while the hook is in open water** (`fishing_hook` `in_open_water`). `gameplay/fishing/fish` draws cod 60, salmon 25, pufferfish 13, tropical fish 2. `FishingHook.retrieve` passes the rod's Luck of the Sea level plus the player's `luck` as the table's luck; with luck 0 the weights stand as written.
- **The wait.** `FishingHook.catchingFish` sets the lure time to `Mth.nextInt(100, 600)` ticks less the rod's Lure speed, then the fish's approach to `Mth.nextInt(20, 80)` ticks. Each tick counts down by 1, plus 1 on a quarter of the ticks while it rains at the cell over the hook, less 1 on half the ticks while that cell cannot see the sky.

**[authored]** With an unenchanted rod and no luck, a cast lands a cod with probability 0.85 × 0.60 = **0.51** in open water and (85 ÷ 95) × 0.60 = **0.537** in a pool too small to be open water. Five cod take on average 5 ÷ 0.51 ≈ **9.8 casts** (≈ 9.3 in a small pool), and under open sky in clear weather a cast waits 6–34 s, about 20 s on average (under a roof about twice as long): **about three and a half minutes** of fishing for five cod. A player catches salmon, pufferfish and junk on the way; a body asking for cod is asking for the 60 % of the fish table that is cod.

## 4. A cat's coat

**[cited]**

- **The coat is the `variant`.** `Cat` keeps its coat in `DATA_VARIANT_ID`, a holder of the `minecraft:cat_variant` registry, saved and read as the NBT key **`variant`** (`VariantUtils`, the string constant `variant`), a variant id such as `"minecraft:tabby"`; its collar is `CollarColor`. As a component the coat is `minecraft:cat/variant` and the collar `minecraft:cat/collar` (`Cat.applyImplicitComponents`).
- **The default.** A cat made without `finalizeSpawn` has `CatVariants.BLACK` (`minecraft:black`) and a red collar: `Cat.defineSynchedData` defines the variant as `DEFAULT_VARIANT`, which the static initialiser sets to `CatVariants.BLACK`, and `DEFAULT_COLLAR_COLOR` to `DyeColor.RED`.
- **When it is random.** `Cat.finalizeSpawn` picks the coat with `VariantUtils.selectVariantToSpawn` over the spawn conditions of `data/minecraft/cat_variant/*.json`: of the variants whose condition holds at the spawn cell, those of the highest priority, one at random. Ten of the eleven variants hold everywhere at priority 0; `all_black` holds at priority 0 under a moon of brightness 0.9 or more, and at priority 1 inside a structure of `#minecraft:cats_spawn_as_black` (a cat there is always `all_black`).
- **`/summon` calls `finalizeSpawn` only without an NBT argument.** `SummonCommand.createEntity` takes a flag the three forms pass: `summon <entity>` and `summon <entity> <pos>` pass true, `summon <entity> <pos> <nbt>` passes false, and `finalizeSpawn` runs only when the flag is true.

**[authored]** A cat summoned with no NBT has a random coat. A cat summoned with any NBT — and every body the engine summons carries NBT, its tags at least — is `minecraft:black` with a red collar unless the NBT names `variant` (and `CollarColor`). A cat whose coat the story means is given it by `variant` in its summon, or `minecraft:cat/variant` on the egg or stack it comes from.

## 5. Which bodies a water bucket picks up

**[cited]** Read from the same jar and mappings (§ The read site), by `javap -p -c` over every top-level class of the jar.

- **The implementers.** Exactly two classes declare `implements Bucketable`: `axolotl.Axolotl` and `fish.AbstractFish`. The concrete subclasses of `AbstractFish` are `fish.Pufferfish` and `frog.Tadpole` directly, and `fish.Cod`, `fish.Salmon` and `fish.TropicalFish` through `fish.AbstractSchoolingFish`.
- **The use.** `Axolotl.mobInteract`, `AbstractFish.mobInteract` and `Tadpole.mobInteract` each call `Bucketable.bucketMobPickup(player, hand, this)`, which compares the hand's item to `Items.WATER_BUCKET` and the body's `isAlive()` before it takes the body (§1).
- **The yield.** `getBucketItemStack` returns a new stack of `Items.AXOLOTL_BUCKET` (`Axolotl`), `COD_BUCKET` (`Cod`), `SALMON_BUCKET` (`Salmon`), `PUFFERFISH_BUCKET` (`Pufferfish`), `TROPICAL_FISH_BUCKET` (`TropicalFish`) and `TADPOLE_BUCKET` (`Tadpole`).

**[authored]** A player holding a water bucket who right-clicks a living axolotl, cod, salmon, pufferfish, tropical fish or tadpole is handed that kind's bucket: six uses, one tool, the engine's `bodies::BODY_USES`. Other vanilla uses that hand an item (a bucket on a cow, goat or mooshroom; a bowl on a mooshroom; shears) are not read here and are not in the table.
