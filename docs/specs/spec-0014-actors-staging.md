# spec-0014 — Scripted actors & staging verbs (v0.6)

- **Status**: Draft (planner-authored per owner direction, 2026-08-01)
- **Motivation**: the island remake's dramaturgy (herding flock, giant NPC that
  becomes a real threat, rhythm-synced cutscenes, stealth beats, positional
  sound, per-ending art titles) needs first-class verbs. Foundation: task #46
  live-verified that no vanilla primitive commands real-AI walking, and that a
  **NoAI puppet moved by compiler-emitted per-tick tp** produces real walk
  animation (rel_entity_move deltas; <8-block steps; yaw along the tangent or
  it moonwalks). Real-AI commanded movement is therefore excluded (no-hack);
  puppets are the primitive. Also the substrate for future tower-defense lanes.

## DSL surface (stage 5, v0.6)

**`actors[]`** — scripted stage entities, distinct from stage-2 NPCs (no
dialogue, any mob type): `{id, entity, name?, skin?, anchor, facing?,
vulnerable?: false}`. Emitted NoAI/Silent/no-loot, tag `dw_actor_<id>`,
Invulnerable unless `vulnerable` (a damageable, knockback-immune puppet is the
tower-defense creep). `skin` → mannequin, as stage-2.

Effects (usable wherever `set-flag` is: objectives, triggers, dialogue):

| Effect | Behavior |
|--------|----------|
| `spawn-actor{actor}` / `despawn-actor{actor, style: vanish\|kill}` | Summon / remove; `kill` plays the vanilla death animation (cutscene deaths), `vanish` is silent removal. |
| `move-actor{actor, to_anchor, speed?, on_arrive[]}` | A*-planned per-tick tp using the **actor's hitbox footprint** (per-entity dims table; warden 0.9×2.9), yaw along tangent. Concurrent movers allowed — a flock is N synchronized `move-actor`s. Unroutable → **DW0325**. `move-npc` becomes a thin wrapper over the same planner. |
| `unleash-actor{actor}` | Replace the puppet with a real-AI twin (same type/pos/name/attributes), same tag. For the "attack the idle giant → real unwinnable fight" beat. Re-caging = despawn + `spawn-actor` (idempotent), typically from a checkpoint `on_respawn` (spec-0012). |
| `actor-anim{actor, anim}` | Reserved surface. The anim enum (attack-swing, roar-pose, sleep-pose, hurt-flash, …) is populated **only** with animations the live spike proves a vanilla primitive can trigger on command; anything without a primitive is excluded per the no-hack doctrine, and the beat is staged another way (camera cut + sound). |
| `sequence{steps: [{at_ticks, effects[]}]}` | Deterministic timeline: one schedule chain firing effect groups at exact ticks. This is how two actors play separate animations rhythm-synced (owner's cutscene model). Nestable effects = any in this table + v0.5 set; no recursion into `sequence` (**DW0329**). |
| `play-sound{sound, at?: anchor\|actor\|players, volume?, pitch?}` | Positional or per-player sound. `sound` validated against the vendored pinned-1.21.11 sound-event registry → **DW0326** unknown. |
| `begin-stealth{zones: [{anchor, extent}], on_caught[], grace_ticks?: 20}` / `end-stealth` | While active, per-tick: every player must be inside some zone **and** sneaking; a player failing both for `grace_ticks` fires `on_caught` (typically kill → checkpoint respawn). Zones are the "shadows"; presented via dark cells but judged by region — deterministic, provable. |

## Proof obligations (compile-time)

1. `move-actor` path exists over the final assembled model for the actor's
   footprint (DW0325 names actor, leg, first blocked cell).
2. Every stealth zone is standable and reachable from the player's position at
   the beat that activates it (**DW0327**).
3. Sound ids resolve (DW0326); `sequence` acyclic/non-nested (DW0329); actor
   ids unique, anchors resolve (existing machinery).
4. Determinism: all emission (paths, timelines) byte-identical across builds.

## Art titles (narrate extension)

`narrate` gains `style: "art"` — renders via a resource-pack **custom font**
(large glyph provider) so each ending can flash big art text. Font asset must
be original or license-allowlisted (ADR-0013), provenance in
`docs/ACKNOWLEDGEMENTS.md`. Text outside font coverage → **DW0328**.

## Diagnostics

DW0325–DW0329 as above (numbers may shift at implementation; the DW gate and
`docs/reference/compiler.md` are authoritative). All v0.6 surface is reserved
under `dsl_version <= 0.5` (DW0141 pattern).

## Validation / acceptance criteria

1. Negative fixtures assert DW0325–DW0329 by code; every new code has a
   code-asserting test (CI gate).
2. PackTest: puppet reaches destination cell & fires `on_arrive`; `unleash`
   swaps to an AI mob; stealth check kills a non-sneaking fake player after
   grace and spares a sneaking one in-zone; `sequence` fires at exact ticks.
3. Determinism: double-build byte-identity on a campaign using every verb.
4. Client-eyes spike (recorded, not CI): walk gait for warden + sheep reads
   as walking (cadence/yaw), per task #46's packet-level evidence.

## Non-goals

Real-AI pathfinding control (excluded, task #46); combat behavior scripting for
unleashed mobs (vanilla AI takes over); crowd avoidance between concurrent
movers (paths are authored not emergent); runtime branching inside `sequence`
(branch via flags/objectives instead).

## Addendum (v0.6.1) — close-gate & damage-players

Two staging effects added while authoring the island remake; both landed as
separate engine PRs. Decision-record form; `docs/reference/compiler.md` is the
authoritative current-behavior record.

- **`close-gate{anchor}`** — the physical dual of `open-gate`. Where `open-gate`
  fills a gate region with air, `close-gate` fills it with the block the gate
  anchor's prefab metadata declares (the island boulder's `minecraft:basalt`),
  re-sealing an opened threshold into a wall — the owner's "point of no return by
  geometry, not narration". Emission mirrors `open-gate`: a deterministic
  `fill <region> <block>` (no `replace` clause). A gate anchor that declares no
  fill `block` cannot be sealed → new **DW0343** (validation tier; compiler-side,
  since the block is prefab metadata). Anchor existence reuses `open-gate`'s
  DW0142. **Completability:** the occupancy model treats every gate as passable
  (the conservative "the needed gate opens" stance DW0306 proves); `close-gate`
  is the dual — each walked critical / checkpoint-forward leg is routed with any
  gate whose latest firing before it is a `close` (not reopened) forced **solid**,
  so a forced path that must re-cross a sealed gate fails **DW0311** (DW0315 from
  a checkpoint). A later `open-gate` before the leg reopens it.

- **`damage-players{amount, in?, damage_type?}`** — the real consequence a stealth
  `on_caught` / souls beat needs, over vanilla `/damage`. Per-`@s`
  (`damage @s <amount> <type>`): top-level hits every player once, in `on_caught`
  the caught player. `amount` in half-hearts (≥ 40 lethal through golden apples).
  `in {anchor, extent}` narrows to acting players inside the anchor-centred box
  (stealth-zone box model; anchor DW0142), staying per-`@s`. `damage_type` is a
  **curated enum** of vanilla types that respect `keepInventory` and do NOT bypass
  totems (no `out_of_world`/`generic_kill`); default `generic`; unknown value =
  schema DW0100 (no new registry). Named `damage_type`, not `type`, because the
  effect enum is internally tagged on `type`. Per-effect `requires_flags` allowed
  (per-`@s` verb). No new DW code. A `v06_damage` PackTest asserts a dummy's health
  drops.
