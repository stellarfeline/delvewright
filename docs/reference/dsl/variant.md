# `delvewright_dsl::variant`

The reference page for `crates/dsl/src/variant.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../compiler.md#5-diagnostics-catalog).

A body's `variant` is one field on every class that summons a body — a stage-2 NPC, a stage-5 actor, a wave mob, a wave mob's mount and a vehicle — judged and written through the pinned table `crates/dsl/data/entity-variants-1.21.11.json`; the surface and its emission are in [`compiler.md`](../compiler.md) (the stage-2 row `variant` and the stage-5 row `variant` (bodies)), what the pinned game carries in [`body-variants.md`](../body-variants.md).

## Diagnostics

### DW1131 — a variant property the body does not carry (`dsl::variant`; DSL v0.83; error)

| Code | Meaning |
|------|---------|
| `DW1131` | **A variant property the body does not carry.** Raised at the property, for every body `dsl::body_variant_sites` walks: the entity the body ships as — a skinned NPC or actor is a `minecraft:mannequin` — carries no such property in the pinned table (a `collar` on a horse; anything on a zombie or a mannequin). The message lists the properties that entity does carry, or says it carries none. Validation tier (exit 1). Prescription: a property the message lists, or no `variant`. |

### DW1132 — a variant value the property does not take (`dsl::variant`; DSL v0.83; error)

| Code | Meaning |
|------|---------|
| `DW1132` | **A variant value the property does not take.** Raised at the property: the value is not one the pinned game holds for that property of that entity — a registry id the registry does not hold (`minecraft:ashen`, a wolf's coat, on a cat), a dye or a name the list does not hold. Values are written as the game writes the component: a registry id with its namespace, a name without one. The message lists every value the property takes. Validation tier (exit 1). Prescription: a value the message lists. |
