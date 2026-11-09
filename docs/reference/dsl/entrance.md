# `delvewright_dsl::entrance`

The reference page for `crates/dsl/src/entrance.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../compiler.md#5-diagnostics-catalog).

The module holds the registry of **kind actions** (spec-0132 §2.2): records keyed by `(kind, action)`, each stating what the action is in a player's eyes, the mechanism as the NBT the twin's summon carries, its length in ticks, what the body can and cannot do while it runs, what it ends with, and the record it was measured in. It ships the action `enter` for `minecraft:warden` (the emerge, `warden-behaviour.md` §6) and `minecraft:wither` (the charge-up, `native-entrance.md` §1); `actors[].entrance: "native"` plays the record of the twin's kind.

## Diagnostics

### DW01xx — validation (`dsl`; severity error; exit 1)

This module's rows of a section whose prose is on the [`delvewright_dsl::diagnostic` page](diagnostic.md#dw01xx--validation-dsl-severity-error-exit-1).

| Code | Meaning |
|------|---------|
| `DW1081` | **An entrance its kind does not have** (spec-0132 §4). An actor declares `entrance: "native"` and the registry holds no `enter` record for the twin's kind — `entity` as written, since a `skin` dresses only the puppet. Names the actor, the kind, the action (`enter`) and every kind that has one. Path `/content/actors/<i>/entrance`. Prescription: drop the field, or give the body a kind that enters. Validation tier (exit 1), `dsl::validate` through `actor_checks`. |
| `DW1082` | **A declaration about the free body on an actor the free body never comes to** (spec-0132 §4). Two arms, one shape — the shape `DW0909` and `DW0913` use for a bar and a kill bundle on a body nothing unleashes. The first: an `entrance` or an `outrun` on an actor no `unleash-actor` names anywhere (`dsl::unleashed_actors`), path `/content/actors/<i>/<field>`, raised by `dsl::validate` (exit 1). The second: an `outrun` actor loose on no step of the exported critical path (`compiler::plan`'s pursuers, `critical-path.json` format 6 — every unleash of it fires after the path's last walked step, or it is despawned before the next begins), raised at build (exit 3), where the path is derived. Names the actor, the field and the arm. Prescription: unleash the actor on the path, or drop the field. The next field of this shape is a third arm here, never a new code. |
| `DW1083` | **A puppet surface on an actor that enters** (spec-0132 §4). An actor declaring `entrance` has no puppet life: its first appearance is its `unleash-actor`, at its own mark. A `spawn-actor` or `move-actor` naming it at any effect root and any depth (path: the effect's pointer — an `ambushes[]` entry never writes one, since the sugar omits the spawn of an entering actor, so this fires only on a hand-written spawn), or `vulnerable`, `watch` or `answers` on it (path `/content/actors/<i>/<field>`) — each a property of a body that stands caged. Names the actor, the surface and where it is entered. Prescription: remove the surface. Validation tier (exit 1), `dsl::validate` through `actor_checks`. |
