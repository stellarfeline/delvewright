# `delvec::compiler::observer`

The reference page for `crates/delvec/src/compiler/observer.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW01xx — validation (`dsl`; severity error; exit 1)

This module's rows of a section whose prose is on the [`delvewright_dsl::diagnostic` page](../../dsl/diagnostic.md#dw01xx--validation-dsl-severity-error-exit-1).

| Code | Meaning |
|------|---------|
| `DW0926` | **Engine self-check: a shipped selector would read a watcher** (spec-0077 §5). On every build, a positional player selector in the shipped datapack (`@a`/`@r`/player-typed `@e` with a box or `distance` term; `@p`; player-typed `@n`) that neither excludes `dw_cutscene` nor stands at an `observer::ALLOWED` site. Build-tier (exit 3), `compiler::observer::check`, read off the shipped bytes after emission. A DEFECT IN DELVEC, never the campaign's: the message says so and asks for a report; the repair is the emitter adding the guard, or the site named in `observer::ALLOWED` with its reason. |
