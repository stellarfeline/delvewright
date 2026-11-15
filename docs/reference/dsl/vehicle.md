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

