# `delvec::compiler::healthbar`

The reference page for `crates/delvec/src/compiler/healthbar.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW0909–DW0912 — a fight shows its health (`dsl::healthbar` / `compiler::healthbar`; error + advisory; exit 1 / 0)

This module's rows of a section whose prose is on the [`delvewright_dsl::diagnostic` page](../../dsl/diagnostic.md#dw0909dw0912--a-fight-shows-its-health-dslhealthbar--compilerhealthbar-error--advisory-exit-1--0).

| Code | Meaning |
|------|---------|
| `DW0911` | **A colour or style the pinned game does not draw** (spec-0073 §8.3). `color` or `style` is not among the literals the pinned command tree lists under `bossbar set <id> color` / `… style`, read through `CommandTree::literals_under` — the message prints them, from the tree, never from a copy; there is no Rust enum of colours. Validation tier (exit 1). |
