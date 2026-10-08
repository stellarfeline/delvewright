# `delvec::compiler::teleport`

The reference page for `crates/delvec/src/compiler/teleport.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW0540–DW0542 and DW0545 — status effects, the region teleport, and the fixture class (`dsl::validate` / `compiler::teleport` / `compiler::affordance`; spec-0031)

This module's rows of a section whose prose is on the [`delvewright_dsl::quest::check` page](../../dsl/quest/check.md#dw0540dw0542-and-dw0545--status-effects-the-region-teleport-and-the-fixture-class-dslquestcheck--compilerteleport--compileraffordance-spec-0031).

| Code | Meaning |
|------|---------|
| `DW0542` | **A teleport volume over an affordance bound to hardware.** A `teleport`'s `from` volume covers an interaction affordance the engine placed on a block it also places — an interact objective, a click trigger, a bonfire, a shortcut unlock, a trap or timed-gate disarm, a sealed gate's answer. Build-tier (exit 3), `compiler::teleport`. The teleport moves the entity and not the block, so the player is left with something they can see and reach that answers nothing. **The same code answers a loop's slab** (spec-0086 §4.2): an affordance the engine places — an interact objective, a click trigger, a bonfire, a checkpoint seat, a shortcut unlock, a disarm — inside a `loops[]` slab is refused naming the loop, its slab, its offset and the affordance, because the body is moved off the thing it reached for. Prescription: move the affordance out of the volume, or shrink the volume's `extent` (for a loop: move the slab off the affordance); do NOT add a type exemption to the selector — that would tear an NPC's dialogue hitbox off its body. |
