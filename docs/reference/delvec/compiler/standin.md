# `delvec::compiler::standin`

The reference page for `crates/delvec/src/compiler/standin.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW0971 — a cutscene's stand-ins agree with its declaration (`compiler::standin`; error; exit 3)

| Code | Meaning |
|------|---------|
| `DW0971` | **A cutscene's stand-ins disagree with its `party`** (spec-0095 §5). Build tier (exit 3), `compiler::standin::check`, an emission self-check over the shipped datapack against every cutscene's declared party (deduplicated as the drivers are): a `present` cutscene whose start places no stand-in, places them after `gamemode spectator`, does not claim them as `dw_standin_<bare>`, or whose `cs_end_<bare>` does not remove them; or an `absent` cutscene whose start places some. A compiler defect, never an authoring one: it cannot fire on a correct build. |
