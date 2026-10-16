# `delvec::compiler::presence`

The reference page for `crates/delvec/src/compiler/presence.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW1026 — one named character is one body (`compiler::presence`; error; exit 1)

A character's identity is its declared name as the player reads it: an NPC's `name` and an actor's `name`, markup dropped (`textstyle::visible`). Declarations carrying one name are one character to the player whatever their ids, so a character the story carries from place to place by despawning one declaration and spawning the next is legal, and a story state with two of them in the world is a person seen twice. One declaration is one body by construction (every `spawn-npc`/`spawn-actor` is guarded on the body's own tag). An assembly declares no name, and a cutscene stand-in wears the player's own profile only while that player is in spectator (`DW0971`); neither is a character's second body.

Judged at `delvec validate`, over every configuration — the exported critical path and every realised branch's play order — through the replay's own journal (`flow::JournalStep::fired_in_order`). World init puts every non-deferred NPC on stage and no actor. Each step applies the taken dialogue option's `spawn-npc` lines, then its fired lines **one list at a time**: one list's lines run in one function in one tick, so a `despawn-*` and a `spawn-*` side by side never show both bodies, while a nested list (a `sequence` step, an `on_arrive`) is a later state. World init and every state a spawn changes are judged. A `spawn-*` the play order cannot date (an environment trigger, a trap payload, a reaction bundle, an `on_kill`, a shop, a dialogue option no path takes) counts its body present in every state; a `despawn-*` it cannot date removes nothing; a body killed in a fight counts present until a `despawn-*` removes it. One refusal per name, at its first state.

`delvec validate` prints `presence binding (DW1026): <n> npc(s) and <a> actor(s) (<named> named) declared, <k> assembl(ies) with no name; <s> name(s) carried by more than one declaration, over <b> body(ies); <t> story state(s) judged (world init, and every state a spawn changes) over <c> configuration(s), <u> body(ies) an undatable spawn can bring in; <r> character(s) refused`, zeroes included.

| Code | Meaning |
|------|---------|
| `DW1026` | **One named character, two bodies at once.** Two or more declarations (NPCs, actors, or both) carry one visible name, and some story state of some configuration leaves two of them in the world. Path: the line that brought the second body in, or the second on-stage NPC's `name` for world init, or the dialogue option whose spawn did. Names the character, the bodies present, the configuration and the beat. Prescription: if they are one character, despawn the body that is leaving in the same effect list that brings the next one in; if they are different people, give each its own `name`. Validation tier (exit 1). |
