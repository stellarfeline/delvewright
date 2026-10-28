# `delvec::compiler::bodies`

The reference page for `crates/delvec/src/compiler/bodies.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW105x — a harmless body says what it is (spec-0119; `dsl::validate` + `compiler::bodies`; error)

This module's rows of a section whose prose is on the [`delvewright_dsl::body` page](../../dsl/body.md#dw105x--a-harmless-body-says-what-it-is-spec-0119-dslvalidate--compilerbodies-error).

| Code | Meaning |
|------|---------|
| `DW1055` | **A harmless monster-kind body in reach with no line** (spec-0119 §6). A body whose shipping kind (`BodyRef::worn_entity`) the pinned game discards on peaceful (`dsl::removed_on_peaceful`, read from the jar — the game's own statement of which kinds are monsters), that is inert (`BodyRef::inert`: every NPC, every actor not `vulnerable`), that no `strike` / `strike-npc` trigger rides (`BodyRef::struck_rider`), and that a walking player can strike at some station it is declared to stand on — its home mark, every `move-npc` / `move-actor` destination (`bodies::stations`) — declares no `answers.struck` (or, an actor, no `answers.touched`). "Can strike" is `bodies::reach_cells`: the one stance question `nav::interaction_stances` (standable, walk-reachable cells whose eye sees the body's box within the 3-block entity reach, the call the holder stance pass `DW1037` makes) less the cells the body fills, over `world.body_region_rooted(edit::anchor_starts(plan))` (the reach graph's standing states, spec-0113). Build tier (exit 3), judged over the assembled world. Names each body, its kind and why the game calls it a monster, the station and the stance cell that reaches it, what a player meets there, and each missing line's l10n key. **Binding**: every build prints `body answers (DW1055): B bodies — M monster-kind, I inert, R in reach, O lines owed, D declared, V volunteered on kinds not owed, X refused; S out of reach, T carried by a strike trigger`, zeroes included, the body count the denominator, and writes the same to `validation/body-answers.json` with one row per body. Prescription: write each missing line in the campaign's own words (one `Refusal` saying what the thing is, `game-writing.md` F2), or stage the body where no walked route reaches it, or give it a `strike` trigger if the blow is meant to do something. |
