# `delvewright_dsl::prefab`

The reference page for `crates/dsl/src/prefab.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW0842–DW0845, DW0848 and DW0882 — the detail plan (`compiler::detail` + `dsl::prefab` + `delvec detail`; spec-0050, spec-0058)

This module's rows of a section whose prose is on the [`delvec::compiler::detail` page](../delvec/compiler/detail.md#dw0842dw0845-dw0848-and-dw0882--the-detail-plan-compilerdetail--dslprefab--delvec-detail-spec-0050-spec-0058).

| Code | Rule |
|---|---|
| `DW0848` | **A piece's declared footprint class disagrees with its bytes.** Prefab metadata carries an optional `footprint_class` naming a metrics `size-class.*` rung (`DW0812` refuses a name the table does not define, as for any document naming a table entry). A piece declaring one is refused when its own structure size could serve no box of that class: horizontal extents off the class's range, off the kit grid's quantum, or a height under the class's clearance plus the one floor course a piece owns. The field is optional for the library at large, and a piece bound by a `details[]` row is held to exact frame equality by `DW0843` whether or not it declares. Validation tier (exit 1) in the compiler, admission-failing in `delvec prefab`. **Binding: pieces declaring a class, against declaration documents read.** |
