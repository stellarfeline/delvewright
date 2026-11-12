# `delvewright_dsl::moves`

The reference page for `crates/dsl/src/moves.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW1101–DW1102 — a body the story moves (`dsl::moves`; spec-0136; error)

A staged body — a stage-2 NPC or a stage-5 actor — hops, takes a pose, turns, faces, holds an item and changes size, and an effect of that kind keeps a cadence. The moves, their emission and the cadence are in [`compiler.md`](../compiler.md) (the stage-5 rows `hop` … `repeat`, and the emission rows of the same names); the vanilla facts are in [`npc-moves.md`](../npc-moves.md); a hop's headroom is `DW1103`, on the [`delvec::compiler::hop` page](../delvec/compiler/hop.md). This module also raises, at the pointers its rules own: `DW0942` (a move whose audience names no staged body), `DW0112` (a body no stage declares, in a cast or an `at`), `DW0100` (a `height`, `by`, yaw, `scale`, `times`, `every_ticks` or `stagger_ticks` outside its bound; a body listed twice; an empty list), `DW0143` and `DW0898` (a held item the game does not hold, or does not draw in that slot on that body).

| Code | Meaning |
|------|---------|
| `DW1101` | **A move its body cannot make.** A `pose` naming a body that does not ship as a `minecraft:mannequin` — only a skinned body has the five poses; on any other the game has no `pose` field to write — raised at `pose` once per such body, naming it and the entity it ships as. Or a `face` `to: "watch"` naming a body that watches nobody: it declares no `watch` and is not an NPC a player talks to (or declares `"watch": "none"`), so there is no watch to hand its facing back to; raised at `to`. Validation-tier (exit 1), `dsl::validate`. Prescription: give the body a `skin` or leave it out of the `pose`; declare the body's `watch`, or face it to a yaw or a mark. |
| `DW1102` | **A cadence the effect cannot keep.** `repeat` or `stagger_ticks` on a verb that happens once (`Verb::keeps_cadence` is false — every verb but the six moves, `particle` and `play-sound`), raised at the field: what such a verb leaves behind is read by the build's proofs as one event, and its repetition is steps of a `sequence`. `stagger_ticks` on an effect naming fewer than two bodies, raised at `stagger_ticks`. A cadence beside `audience: actor`, raised at `audience`: a repeated or staggered effect runs from the scheduler, which has no acting player. `repeat.every_ticks` shorter than the move, raised at `repeat/every_ticks`: a `hop`'s own arc in ticks (a body asked to hop before it has landed does not hop, so fewer hops would happen than are declared), or the 2 ticks a `resize` is held before a client is told (a mannequin's attributes are sent every second tick). Validation-tier (exit 1), `dsl::validate`. |
