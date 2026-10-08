# `delvewright_dsl::r#loop`

The reference page for `crates/dsl/src/loop.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW0945–DW0950 — an endless corridor (`compiler::loop` / `dsl::validate` / `compiler::plan`; spec-0086; error + one advisory)

This module's rows of a section whose prose is on the [`delvec::compiler::loop` page](../delvec/compiler/loop.md#dw0945dw0950--an-endless-corridor-compilerloop--dslvalidate--compilerplan-spec-0086-error--one-advisory).

| Code | Meaning |
|------|---------|
| `DW0949` | **A loop whose release is not a fact about the party, or that has none.** A loop with no gate term; a `requires_state` term or a `counts` naming a `player`-scoped datum; a `teleport` effect inside `on_cross`, at any depth. Validation tier (exit 1), `dsl::loop`. Prescription: a `party` datum, a flag, or a release the party reaches. |
