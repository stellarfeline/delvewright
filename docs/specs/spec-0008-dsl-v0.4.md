# spec-0008: DSL v0.4 — expressiveness (dialogue state, props, narration, live threats, presentation)

- **Status**: Approved (owner, 2026-07-31, via chat) — implementation immediately
- **Source**: the `nobodys-cave` generation run (content PR #1, `GENERATION.md` gap
  register) + owner direction. ADRs: 0001 (DSL→compiler), 0003 (vanilla-first),
  0006 (determinism).
- **Design principle (owner, 2026-07-31, now in CLAUDE.md)**: *no hacks at any
  layer*. A vanilla-intended primitive the content needs → first-class DSL verb.
  No vanilla primitive → the feature is excluded until vanilla grows one; we never
  ship a workaround downstream.

## New DSL surface

All additive; `dsl_version` bumps to `0.4.0`; v0.3 documents stay valid.

1. **Dialogue state** (closes gaps 2 + 4's spoiler leak):
   `DialogueEffect::SetFlag { flag }` (mirrors the quest effect) and
   `DialogueOption.requires_flags: Vec<FlagId>` (mirrors the objective gate;
   ungated options unchanged). Validation: flag references resolve; a
   flag-gated option that would make a critical-path node unreachable is a
   DW-diagnostic, not a runtime surprise.

2. **Props** (closes gap 14, first half): `Objective::Interact.prop: { block }` —
   the compiler `setblock`s the prop at the anchor and it *is* the affordance
   (exactly as `collect` already uses a real chest); lantern hologram remains the
   fallback when omitted. General form `QuestEffect::SetBlock { anchor, block }`.
   Block ids validated against the pinned 1.21.11 registry.

3. **Narration** (closes gap 5): `QuestEffect::Narrate { text, style?, sound? }`
   — `style` ∈ chat (default) / title / subtitle; text enters the l10n key
   inventory like any player-visible string.

4. **Wave tuning** (closes gap 1): `WaveMob.attributes?: { max_health,
   attack_damage, movement_speed, follow_range }` (emitted as 1.21.11 attribute
   components) and `WaveMob.effects?: [{ effect, amplifier }]` (permanent,
   ambient). Enables e.g. a weakened live warden as a survivable stealth threat —
   the warden's native vibration-hunting provides sound-stealth for free.

5. **NPC lifecycle** (closes gap 3): `QuestEffect::DespawnNpc { npc }` and
   `QuestEffect::MoveNpc { npc, to_anchor, speed? }` — movement is compiler-emitted
   per-tick teleports along a path derived from solved geometry (client
   interpolation smooths it; spike-verified). The interaction hitbox moves in
   lockstep. Validation: no dialogue objective may target an NPC after its
   despawn on any reachable path (DW-diagnostic).

6. **Player-skin NPCs**: `Npc.skin?: { profile }` switches `base_entity` to
   `minecraft:mannequin` (vanilla, added 1.21.9) with the given skin profile;
   poses/label/equipment mapped from existing NPC fields. Emitter templates come
   from the presentation spike. Non-skinned NPCs unchanged.

7. **Environment triggers** (closes gap 6's "the world answers", owner-promoted):
   stage-5 `triggers`:
   `{ id, at: anchor, on: strike | use | approach { range }, requires_flags?,
   once?, effects: [QuestEffect] }`.
   Emission uses **vanilla-intended primitives only**: `strike`/`use` = a
   `minecraft:interaction` entity's `attack`/`interaction` records (that is the
   entity's designed purpose); `approach` = `distance` selector on the tick.
   **Excluded on principle** (no vanilla primitive; would require raycast/poll
   hacks): look-at detection, break-attempt detection on real blocks. Documented
   here so nobody re-litigates.

8. **Named given items** (closes gap 11): `GiveItem.name?`, matching `KitItem`.

## Harness (gap 7 subset — what stealth validation needs)

- Sneak-aware movement: `critical-path.json` steps may carry `sneak: true`
  (authored via a quest-level hint the compiler derives; exact plumbing is the
  implementer's call) → bot walks that leg sneaking, sprint disabled.
- Death = fast fail with a clear diagnostic (bot death handler; no more
  60-second pathfinder timeouts from a spawn-point respawn), re-`select-class`
  on respawn for retry runs.
- Out of scope here: full combat retry strategies; the ladder proves the safe
  route is completable, not that combat is balanced.

## Explicitly deferred (with the cheap substitute that covers them)

- **Multiple mechanical endings** — SetFlag-gated epilogue branches deliver the
  player-facing split; per-ending advancements/credits need the DW0132
  single-sink model reworked. Revisit when a campaign needs machine-visible
  endings.
- **Generic player-state conditions** (sneak/sprint/health/held-item predicates)
  — the warden's native senses cover sound-stealth; no framework until a real
  campaign exceeds vanilla mechanics.
- **Cutscene camera** (spectator + `spectate` a scripted camera entity) —
  pending the presentation spike's verdict; lands in this spec via addendum if
  clean, else v0.5.

## Acceptance criteria

- [ ] Each new field/verb: schema + validation + emission + at least one
      compiler unit test; `deny_unknown_fields` everywhere; determinism
      double-build stays byte-identical.
- [ ] Flag-gated dialogue: PackTest proves a gated option is absent before its
      flag and present after.
- [ ] Props: PackTest proves the prop block exists at the anchor only once the
      objective activates (composes with the gap-13 activation-gating fix).
- [ ] Narrate: emitted text round-trips the l10n inventory (`--lang` build
      swaps it).
- [ ] Wave attributes: PackTest asserts the summoned mob carries the attribute
      values.
- [ ] Lifecycle: PackTest proves despawn removes entity + hitbox; move ends
      with entity at target anchor.
- [ ] Mannequin NPC: ladder passes with a skinned NPC on the critical path
      (talk-to works through the same interaction hitbox).
- [ ] Triggers: PackTest fires strike/use/approach and asserts effects ran,
      `once` honored, `requires_flags` honored.
- [ ] Harness: a campaign with a live hostile wave + sneak-marked leg passes
      the ladder; a forced bot death produces the new diagnostic, not a
      timeout.
- [ ] `nobodys-cave` (separate task): reworked with v0.4 verbs — real stealth
      finale, flag-gated name consequences, props, mannequin crew — and passes
      the full ladder.
