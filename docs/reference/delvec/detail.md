# `delvec::detail`

The reference page for `crates/delvec/src/detail.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW0842–DW0845 and DW0882 — the detail plan (`compiler::detail` + `dsl::prefab` + `delvec detail`; spec-0050, spec-0058)

This module's rows of a section whose prose is on the [`delvec::compiler::detail` page](compiler/detail.md#dw0842dw0845-and-dw0882--the-detail-plan-compilerdetail--dslprefab--delvec-detail-spec-0050-spec-0058).

| Code | Rule |
|---|---|
| `DW0882` | **The program asks for a value the whole does not hand** (spec-0058 §2.3, `delvec detail`). A grammar program reads the handing through parameters under the `handed/` prefix — `handed/datum-y`, and per seam `handed/seam/<edge stem>/{x0,y0,z0,x1,y1,z1,rise}` keyed by the layout-graph edge without its `edge/` prefix — and `delvec detail` binds every declared one from the allocation. A declared `handed/…` name the allocation does not hand — a seam this place does not have, a misspelling — would expand at its default in silence, a number standing where the plan's own figure belongs; it is refused at `detail`, before the program is expanded, naming the parameter and every name the allocation hands this place, so the repair is a rename in the program and never a number. A handed value the program does not declare is not a refusal: a program may hard-code a cell, and pays for it the day the plan moves it (`DW0844`). Validation tier (exit 1). **Binding: `handed/…` parameters declared, against names handed.** |
