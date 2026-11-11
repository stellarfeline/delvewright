# `delvewright_dsl::vehicle`

The reference page for `crates/dsl/src/vehicle.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW1096–DW1099 — a body carries another (`dsl::vehicle` / `compiler::nav::vehicle`; spec-0135; error)

This module's row of a section whose prose is on the [`delvec::compiler::nav::vehicle` page](../delvec/compiler/nav/vehicle.md#dw1096dw1099--a-body-carries-another-dslvehicle--compilernavvehicle-spec-0135-error).

| Code | Meaning |
|------|---------|
| `DW1096` | **An entity that carries no rider**, or a mount nobody can steer. A `vehicles[]` entity that is not a boat or raft, the rider's `minecraft:minecart`, or a mount the pinned game steers on ground (`crates/dsl/data/entity-steering-1.21.11.json`): a chest boat, a hopper or chest cart, a llama, a zombie; a nautilus, which the game steers in water, a medium the engine does not model for a mount. Raised at validation (exit 1) by `dsl::vehicle`. The second arm is raised at build (`emit::routes`, after the route proof) under the same code: a `ride` step on a mount whose steering item (a carrot on a stick for a pig, a warped fungus on a stick for a strider) the party does not hold by that step — in every class's kit, or from a `give-item` on an objective completed before it. Prescription: an entity of the steering table; the steering item in every kit or given before the ride. |
