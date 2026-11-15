# `delvewright_dsl::vehicle`

The reference page for `crates/dsl/src/vehicle.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW1096–DW1099 — a body carries another (`dsl::vehicle` / `compiler::nav::vehicle`; spec-0135; error)

This module's row of a section whose prose is on the [`delvec::compiler::nav::vehicle` page](../delvec/compiler/nav/vehicle.md#dw1096dw1099--a-body-carries-another-dslvehicle--compilernavvehicle-spec-0135-error).

| Code | Meaning |
|------|---------|
| `DW1096` | **An entity that carries no rider**, or a mount nobody can steer. A `vehicles[]` entity that is not a boat or raft, the rider's `minecraft:minecart`, or a mount the pinned game steers on ground (`crates/dsl/data/entity-steering-1.21.11.json`): a chest boat, a hopper or chest cart, a llama, a zombie; a nautilus, which the game steers in water, a medium the engine does not model for a mount. Raised at validation (exit 1) by `dsl::vehicle`. The second arm is raised at build (`emit::routes`, after the route proof) under the same code: a `ride` step on a mount whose steering item (a carrot on a stick for a pig, a warped fungus on a stick for a strider) the party does not hold by that step — in every class's kit, or from a `give-item` on an objective completed before it. Prescription: an entity of the steering table; the steering item in every kit or given before the ride. |

### DW1115 — a count of vehicle bodies the engine cannot seat (`dsl::vehicle`; spec-0135 §13; error)

A vehicle may stand several bodies: `count` (a constant, or `{base, per_player}` — the wave's count, spec-0134 §4) and `spacing`, body k at the mark plus k × `spacing`; the surface and its emission are in [`compiler.md`](../compiler.md) (the stage-5 row `vehicles[].count`). A name on a vehicle of several bodies is `DW0983`.

| Code | Meaning |
|------|---------|
| `DW1115` | **A count of vehicle bodies the engine cannot seat.** A `count` of no body, raised at `count`. More than one body with no `spacing` (or a spacing of `[0, 0, 0]`), raised at `spacing`: every body would stand in one cell. A count that scales with the party on a vehicle seated `at-load`, raised at `count`: the party is counted when the story seats the vehicle, and no player is present while the world is built — declare `seated: by-story` and seat it with `seat-vehicle`. Validation tier (exit 1). |

### DW1114 — a seat an actor cannot take, or an act a seated actor cannot do (`dsl::vehicle`; spec-0135 §13; error)

A vehicle may seat an actor: `rider`; the surface, its emission and the proofs that read it are in [`compiler.md`](../compiler.md) (the stage-5 row `vehicles[].rider`); the vanilla facts are in [`vehicles-animals-and-targets.md`](../vehicles-animals-and-targets.md) §16. A `rider` naming no declared actor is `DW0112`. A wave body's own mount (`waves[].mobs[].mount`) is not a vehicle and is held to the registry (`DW0173`) and to what the body shows (`DW0898`).

| Code | Meaning |
|------|---------|
| `DW1114` | **A seat an actor cannot take, or an act a seated actor cannot do.** Raised at the vehicle's `rider`: a vehicle of several bodies (one actor sits one body); an actor already the rider of another vehicle; an actor whose own mark (`anchor` and `offset`) is not the vehicle's `at` — a seated actor is placed aboard where its vehicle stands, and every proof of the actor reads its own mark. Raised at the actor's `entrance`: an entrance is the kind's own arrival, a lone body's (a warden digging out of the ground). Raised at the effect: a `hop` whose audience names a seated actor — a hop moves its body by `tp`, and the pinned game's `Entity.teleport` calls `stopRiding` unless the move is a passenger's, so the hop would set the actor down — walk the vehicle with `move-actor`, which carries the actor aboard; a `move-actor` of an actor seated on a boat or a minecart, whose walk is proven on ground, where neither kind moves. Raised at an objective's `ride`: a ride the party is declared to take on a vehicle whose one seat its actor sits. Validation tier (exit 1). |
