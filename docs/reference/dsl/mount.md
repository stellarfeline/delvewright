# `delvewright_dsl::mount`

The reference page for `crates/dsl/src/mount.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW1114 — a rider asked to do what a rider cannot (`dsl::mount`; spec-0135 §13; error)

A wave mob stack or an actor may declare `mount`, the body it rides; the surface, its emission and the proofs that read it are in [`compiler.md`](../compiler.md) (the stage-5 rows `waves[].mobs[].mount` and `actors[].mount`); the vanilla facts are in [`vehicles-animals-and-targets.md`](../vehicles-animals-and-targets.md) §16. A mount's entity is held to the pinned registry (`DW0173`) and its equipment to what the body shows (`DW0898`).

| Code | Meaning |
|------|---------|
| `DW1114` | **A rider asked to do what a rider cannot.** A `hop` whose audience names an actor that declares `mount`, raised at the effect: a hop moves its body by `tp`, and the pinned game's `Entity.teleport` calls `stopRiding` unless the move is a passenger's, so the hop would set the rider down off its mount — walk the mount with `move-actor`, which carries the rider aboard. Or an actor that declares both `mount` and `entrance`, raised at `mount`: an entrance is the kind's own arrival, a lone body's (a warden digging out of the ground). Validation tier (exit 1). |
