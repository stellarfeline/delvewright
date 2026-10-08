# `delvec::compiler::pressable`

The reference page for `crates/delvec/src/compiler/pressable.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW03xx — build / solver / nav (`compiler`; error; exit 3, `stage:"build"`)

This module's rows of a section whose prose is on the [`delvec::compiler::nav` page](nav.md#dw03xx--build--solver--nav-compiler-error-exit-3-stagebuild).

| Code | Meaning |
|------|---------|
| `DW0426` | **A click trigger is anchored where a player can never click it**. The unbound-vacuity class as a check, and the rule that would have caught the gap it came from: the trigger declares an anchor, a click and a full effect bundle, validation passes, emission runs, and the press lands on nothing — so the beat never happens and every board stays green. Fires when a `strike`/`use` trigger's `at` resolves to no placed piece, so there is no cell to give it a body at. (`strike-npc` carries no anchor and rides its NPC's own hitbox; `approach` is a radius test with no entity — neither is in scope.) `compiler::pressable::body_at` + `emit::check_trigger_bodies`, build-tier (exit 3). It walks `Plan::emitted_triggers_unlocalized` — the campaign's own triggers **and** the press answers the compiler synthesizes — so a compiler-owned press is proven to land exactly as an authored one is. The bodies it resolved are published as `validation/press-bodies.json` (`examined`, `unbound`, `reason`, and per press the trigger, the click, the anchor and WHICH body it landed on — riding a seal, arming a region shell, or a point in open air): an error-tier proof that ships is equally silent on a campaign with no click triggers at all, and only the count separates the two. Prescription: anchor it on a place a prefab provides — anchor names come from prefab metadata, never invented — or drop the trigger. |
