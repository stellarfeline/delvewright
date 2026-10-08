# `delvec::compiler::onkill`

The reference page for `crates/delvec/src/compiler/onkill.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW0913–DW0915 — a kill pays only where a kill can pay (`dsl::onkill`, `compiler::onkill`; error; exit 1)

**An `on_kill` bundle is a claim about a body being killed, so the engine checks
that the claim can be exercised and that the one judgement it owes is stated
where it binds.** `fires` decides whether a body that comes back after the party
has met it pays again — whether the fight can be farmed — and the engine does
not make that decision for a creator in either direction.

| Code | Meaning |
|------|---------|
| `DW0914` | **`fires: every-kill` on a fight that never comes back** (spec-0074 §8.2). `compiler::onkill::check_on_kill_fires`, raised from the compiler side at validation tier (the `DW0343` precedent — "comes back" reads the rest points, which the DSL crate does not model). `onkill::fight_comes_back` is false: no `bonfire` re-seats it (a `respawns_on_rest` wave, a billed `elite`/`boss` wave, an unleashed actor) and no seating beat can fire more than once (the list is `waves[].on_kill`'s row in §2). There the two values coincide, so the declaration binds to nothing. Path: `/content/…/on_kill/fires`; the message names the fight and what it lacks to come back. Prescription: state `first-kill` or leave `fires` off, or make the fight come back. |
| `DW0915` | **No `fires` on a fight that comes back** (spec-0074 §8.3). The same function and predicate as `DW0914`, which is what makes the two one pair: on a fight that comes back `every-kill` and `first-kill` pass and an absent `fires` reds; on one that does not, `every-kill` reds and the other two pass (`crates/delvec/tests/on_kill.rs`). Path: the bundle; the message names the site that brings the fight back — the bonfire and the field, the billing, the unleash, or the repeating beat's pointer — and the two judgements to choose between. Build time asks the same predicate through `Plan::fight_comes_back` with the plan's collected bonfires; before any build it reads `delvewright_dsl::declares_bonfire`, the walk `DW0370` asks. `delvec validate` states `on_kill binding: <bundles> of <fights> fight(s) carry a bundle, …` among what it examined. |
