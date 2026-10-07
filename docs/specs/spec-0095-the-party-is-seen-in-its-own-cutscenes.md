# spec-0095: The party is seen in its own cutscenes

- **Status**: Proposed
- **Ground**: written against engine `5726e7f6a` (`feat/the-thing-beyond-the-fog`), read only: `emit::cutscene_fns` (the cutscene bracket), `emit::removal_lines` (the engine's one removal of a body it placed), `compiler::affordance::check_fixtures` (the two summon classes), and the pinned 1.21.11 game itself: `net.minecraft.world.entity.decoration.Mannequin`, `net.minecraft.world.item.component.ResolvableProfile` and `net.minecraft.world.level.storage.loot.functions.FillPlayerHead`, disassembled with `javap` from the pinned client jar and named through Mojang's official 1.21.11 client mappings; every command below was then run on the pinned 1.21.11 dedicated server.
- **What it is for**: a cutscene that can show the party. Today a cutscene puts every player in spectator riding a dolly camera, so their bodies leave the world for its length. A wide shot of the boat the party is standing in shows an empty boat. The first scene to ask is the owner's re-shot reveal on the demo level **The Thing Beyond the Fog**: the camera pulls back from the figure in the sea until the boat, with the party in it, and the figure share the frame.
- **Research**: every game fact is read from the pinned bytes or measured on the pinned server, and is marked **cited** with how it was read; the rules built on them are **authored**.
- **Numbers**: `spec-0095`; **one DW code**, `DW0971` (§5); `DW0972` and `DW0973` were handed and are not consumed. The cutscene gains a field inside the unpublished `dsl_version` the unreleased format changes share; this spec states no version literal.
- **Non-goals**: a stand-in that moves or acts (it is a body standing where a player stood, nothing more); a seated or crouching stand-in (§2.4); a cutscene's own cast of actors, which `actor`s already are.

## 1. The defect

**Finding, from reading the tree.** `cutscene_fns` saves one return mark at `@p`, sets `gamemode spectator @a` and restores at `cs_end_<bare>`. Nothing stands where the players stood. `delvec schema --stage all` carries no field on `cutscene` that says anything about the party.

## 2. What the game gives

### 2.1 The body

**Cited: `Mannequin` bytecode and mappings, and the pinned server.** `minecraft:mannequin` is summonable (`crates/delvec/data/entities-1.21.11.json`) and draws the player model. Its save data reads `profile` (a `ResolvableProfile`, the codec of a player head's `minecraft:profile` component), `hidden_layers`, `main_hand`, `pose`, `immovable`, `description` and `hide_description`; it is a living entity, so it holds `equipment` in every armour slot and both hands. The engine already summons it for every skinned NPC.

### 2.2 The skin, without a name

**Cited: `FillPlayerHead.run` bytecode.** When the item is `minecraft:player_head` and the loot context's target entity is a player, the function sets `minecraft:profile` to `ResolvableProfile.createResolved(player.getGameProfile())`: the player's whole profile as the server holds it, its `textures` property and signature included. On an online-mode server that property is the skin Mojang signed at login; on an offline-mode server the profile carries none, and every client draws the default skin for that UUID, as it draws the player.

**Measured on the pinned server.** `execute as <player> run loot replace entity <mannequin> armor.head loot <table>` with a table of one `player_head` filled from `this` writes `{name, id}` of the player onto the head (`Probe`, its UUID). `data modify entity <mannequin> profile set from entity <mannequin> equipment.head.components."minecraft:profile"` carries it to the mannequin; a profile carrying a `textures` property with a signature is carried with both intact (read back field for field). No name is typed into a command and nothing is looked up over the network.

### 2.3 The gear

**Measured on the pinned server.** `item replace entity <mannequin> <slot> from entity <player> <slot>` copies `armor.head`, `armor.chest`, `armor.legs`, `armor.feet`, `weapon.mainhand` and `weapon.offhand`; a slot the player has empty copies as air and empties the mannequin's. A mannequin killed with `/kill` while wearing armour and holding a sword leaves no item entity (none found two seconds later): the copy is never duplicated into the world.

### 2.4 The pose

**Cited.** `Mannequin.VALID_POSES` holds standing, crouching, swimming, fall_flying and sleeping. A crouch read from the player (`entity_properties` `flags.is_sneaking`) could not be measured with the harness client (mineflayer 4.37.1 holding sneak was not reported sneaking by the pinned server), so this spec does not build it: the stand-in stands. Sitting is not a pose; it is riding, and a stand-in riding the player's vehicle is a different verb.

## 3. The surface

**Authored.**

```json
{ "type": "cutscene", "party": "absent", "shots": [ … ] }
```

- **`party`** — `present` (the default) or `absent`, on the cutscene, in both spellings.
- **`present`**: every player in play is shown by a stand-in where they stood, facing as they faced, wearing their own skin and a copy of their armour and held items, for the cutscene's whole length.
- **`absent`**: no stand-ins. The bodies leave the scene: a vision, a memory, a scene somewhere else.

**Why on by default.** The bodies are what the world holds while the camera flies; a cutscene that removes them states something about the scene, so it is the creator who states it. A default of `absent` would write one design (cutscenes are disembodied) into every cutscene a creator did not think to mark, and the wide shot of the boat would be wrong by default. The field is a mechanism with two values, not a feature of one scene: a different game sets `absent` where its story needs it.

## 4. Emission

**Authored, on §2's cited and measured parts.**

- **`start`**, before `gamemode spectator @a`: `execute as @a[tag=!dw_cutscene,gamemode=!spectator] at @s run function <ns>:cs_standin`, then the new stand-ins are claimed as `dw_standin_<bare>`. A player already watching (a respawn wait, another cutscene) or out of the body in spectator gets none.
- **`cs_standin`** (one shared function, run as the player, at the player): summon a `minecraft:mannequin` (`pose:"standing"`, `immovable`, `NoGravity`, `Invulnerable`, `Silent`, `hide_description`), turn it to the player's yaw with a level head, fill a player head from `<ns>:standin_profile` into its head slot, copy the head's profile onto it, then copy all six slots from the player.
- **`cs_end_<bare>`**: the stand-ins leave by the engine's one unseen removal (`removal_lines`, `Exit::Unseen`): moved straight down to y −128 and removed there five ticks later, so no death animation plays where the player has just returned.
- **The class.** A stand-in is a body, not a place: it carries neither `dw_fixture` nor `dw_borne`, and a region verb that moves bodies in a box during the cutscene carries it as it would carry the player.
- **The name.** An `absent` cutscene's functions are named `cs_<bare>_absent`, so two cutscenes with the same shots and different parties are two functions.
- **PackTest** `standin`: the real `cs_standin`, run as the template's dummy, leaves one mannequin on the dummy whose `profile.id` is the dummy's UUID, whose yaw is the dummy's, which wears the dummy's chestplate and holds its stick, and which wears no head where the dummy wears none.

## 5. The refusal

**Authored.** `DW0971`, build tier (exit 3), an emission self-check over the shipped datapack against the declarations: a cutscene declared `present` whose start places no stand-in, places them after `gamemode spectator`, does not claim them, or whose end does not remove them; or a cutscene declared `absent` whose start places some. It cannot fire on a correct build. Every build with a cutscene prints `stand-in binding: C cutscene(s) examined, P present (stand-ins placed and removed), A absent (none placed)` and writes `validation/stand-in-gate.json`.

## 6. What the gallery, the record and the skill owe

- **The gallery element.** The hall's second muster cutscene (the actor parade shot by shot style) is `party: absent`; the other two stay `present`. Bound by perturbation: removing the field moves `cs_muster_3_2_12687a8f_absent` back to `cs_muster_3_2_12687a8f` with the stand-in lines in it.
- **The record.** `docs/reference/compiler.md`: the stand-in paragraph under *A cutscene is pure observation*, and `DW0971`.
- **The skill.** `references/quest-capabilities.md` under *Things that change the world*.
- **The demo level.** **The Thing Beyond the Fog**, `docs/demo-levels.md`.

## 7. Acceptance criteria

`delvec` is this tree's binary, built from the crate manifests this tree carries.

1. **The surface.** `delvec schema --stage all` exports `cutscene.party` with the two values `present` and `absent`.
2. **The default.** A cutscene with no `party` emits, in its start and before `gamemode spectator @a`, the `cs_standin` call over `@a[tag=!dw_cutscene,gamemode=!spectator]`, and in its end the unseen removal of `dw_standin_<bare>`; `party: present` is byte-identical to leaving it out. *`crates/delvec/tests/v36_standin.rs`.*
3. **The opt-out.** `party: absent` emits `cs_<bare>_absent` with no stand-in line, and a campaign whose only cutscene is absent ships no `cs_standin`, no `standin_profile` loot table and no `standin` PackTest. *`crates/delvec/tests/v36_standin.rs`.*
4. **The refusal.** Each shape of §5 is `DW0971`. *`compiler::standin::tests`.*
5. **The live half.** The `standin` PackTest is green on the pinned server in the demo level's suite.
6. **The gallery.** §6's element builds green; `tools/ci/check-gallery-coverage.py` reports 0 units in neither state.
7. **The record and the skill.** The rows of §6, in the change that lands the code; `tools/ci/check-dw-codes.py` green.
8. A demo-level row names **The Thing Beyond the Fog**.

## 8. Decisions for the owner

- The party is **seen by default**, and a cutscene opts out with `party: absent`.
- The stand-in **stands**: crouching is not built (unmeasured), sitting is out of scope.
- No ADR.
