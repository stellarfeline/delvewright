# `delvewright_dsl::fmt`

The reference page for `crates/dsl/src/fmt.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW07xx — asset, render and authoring tooling (spec-0007; `delvec` subcommands)

This module's rows of a section whose prose is on the [`delvec::admit::diag` page](../delvec/admit/diag.md#dw07xx--asset-render-and-authoring-tooling-spec-0007-delvec-subcommands).

| Code | Tool | Meaning |
|------|------|---------|
| `DW0770` | `delvec fmt` | Authored JSON is not valid JSON, located at `line:col` (exit 1). Reported instead of formatted — `fmt` never guesses at a repair. |
| `DW0771` | `delvec fmt` | **A duplicate object key.** JSON's grammar allows one and `serde_json` silently keeps the LAST, so one of the two values is already being discarded without a word; formatting would make that loss permanent and invisible, so `fmt` refuses and writes nothing (exit 1). Delete or rename whichever occurrence is wrong. |
| `DW0772` | `delvec fmt` | Internal error: the formatter's own output is not equivalent to its input, so nothing was written (exit 1). The self-check runs on **every** file `fmt` writes — it re-parses the rendered text and compares arrays index-wise, objects as maps. Its whole purpose is that an array reordering (which changes the game) fails here instead of shipping. A `DW0772` is a compiler bug; report it. |
| `DW0773` | `delvec fmt --check` | A file is not in canonical form, with the line of the first difference (exit 1). Fix by running `delvec fmt <path>` — never by hand. |
| `DW0774` | `delvec fmt` | The given paths matched **zero** JSON files (exit 1). A formatter or a `--check` that binds to nothing is vacuous, not a pass (CLAUDE.md), and a stale path in a CI step is exactly how this gate would rot into a green no-op. |
