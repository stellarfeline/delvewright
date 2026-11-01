# `delvec::compiler::plot`

The reference page for `crates/delvec/src/compiler/plot.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW1074 — a part not built as declared (`compiler::plot`; spec-0125; error; detail tier)

A plot's piece names each part it builds on the contract spaces and out-of-walk regions that are it, with `part` (`grammar.md` §2d). A part's built play space is the union of the xz projections of every region naming it, counted in columns — the piece's own declarations, which the contract gates prove against the bytes. Judged by `plot::judge`, one implementation: shapes 1, 2 and 4 read the piece's metadata in `detail::check`, the funnel every verb's validation goes through (`delvec detail --check` and `build` included); `delvec detail` judges all four over the expansion and the piece's own flood (`grammar::contract::walked_cells`, the walk `contract-reachability` takes); the build judges the walk again over each plot piece's shipped templates, beside the plot ledger.

| Code | Meaning |
|------|---------|
| `DW1074` | **A part not built as declared.** One code, four shapes, each named first in the message with the part, its `use` and the numbers, and both remedies — the piece's and, when the plan is what is wrong, the plan's: *unbuilt* — a declared part no space and no out-of-walk region names; *undeclared* — a `part` naming an id the node does not declare, or any `part` on a piece whose node declares no `holds[]`; *not walked* — a part declared walked (`reached`, the default) none of whose columns holds a cell the piece's own flood reaches from its entry; a `reached: false` part asks nothing; *out of size* — a part whose built play space is under three quarters or over five quarters of its declared `cells`, naming both numbers and the range. |

**Binding.** `delvec detail` prints `<place>: parts: B of K bound, W of R walked` before its verdict for a place whose node is a plot or whose contract names a part. The build prints `plot program: P plot(s), K part(s) built B of K; coverage min–max %, remainder min–max %` and writes `validation/plot-program.json` (`compiler.md`).
