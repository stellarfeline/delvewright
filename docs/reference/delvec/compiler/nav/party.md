# `delvec::compiler::nav::party`

The reference page for `crates/delvec/src/compiler/nav/party.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW1141 — a party carry with no seat for every member (`compiler::nav::party`; error; exit 3)

A party carry — a `teleport` whose subject is the party (`"from": "party"`), and the compiler's own crossing into an area's entry point (`cross_<area>`) — puts each member on a seat of their own: the mark's cell, then the nearest cells around it, nearest ring first (Chebyshev distance 1 to 3), within a ring the nearest to the mark and then the least cell. A seat is a cell a body may be put down on (`World::is_safe_seat`, the respawn seat's rule: standable, meeting no block that hurts a body) that a body on the mark reaches (`World::body_reach`). Derived over the world as shipped, as a respawn seat is, after the respawn seats are fixed (`nav::fix_party_seats`), and fixed on the plan for the emitter (`Plan::party_seats`).

| Code | Meaning |
|------|---------|
| `DW1141` | **A party carry with no seat for every member** (`nav::fix_party_seats`). The mark is not a cell a body may be put down on, or fewer than four such cells (`PARTY_SEATS`, the most members a party has) that a body on the mark walks to stand within three blocks of it. The message names the teleport's pointer, the root that fires it, the mark and its cell, and the seats found, and the remedy is to move the mark onto open floor with room for four players side by side; for a crossing it names the area entered and its entry point, and the remedy is to put the area's entry anchor on such floor. Build tier (exit 3). Tests: `crates/delvec/tests/it/party_teleport.rs`, `crates/delvec/tests/it/party_crossing.rs`. |
