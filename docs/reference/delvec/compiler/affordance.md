# `delvec::compiler::affordance`

The reference page for `crates/delvec/src/compiler/affordance.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW0540–DW0542 and DW0545 — status effects, the region teleport, and the fixture class (`dsl::validate` / `compiler::teleport` / `compiler::affordance`; spec-0031)

This module's rows of a section whose prose is on the [`delvewright_dsl::quest::check` page](../../dsl/quest/check.md#dw0540dw0542-and-dw0545--status-effects-the-region-teleport-and-the-fixture-class-dslquestcheck--compilerteleport--compileraffordance-spec-0031).

| Code | Meaning |
|------|---------|
| `DW0545` | **An engine fixture is reachable by a box.** Either an engine-summoned hitbox, mark, display or cutscene stand-in (a `minecraft:mannequin` tagged `dw_standin`, spec-0095 — an NPC's skinned mannequin is a body and is not one) declares neither class tag (`dw_fixture` / `dw_borne`), or a selector narrowed by a positional box (`@e[x=…]`) does not carry `tag=!dw_fixture`. Build-tier (exit 3), `compiler::affordance`, emission self-check over the shipped datapack. **A compiler defect, never an authoring one** — no campaign JSON can cause it and none can fix it; the message is addressed to whoever is changing the engine. Prescription for a new affordance: summon it declaring the class. For a new region verb: negate the class, never a `type=…` roster — a type cannot tell an NPC's dialogue hitbox from a recovery stake's marker, and a moving verb must carry the first and leave the second. |
