# `delvec::compiler::view::showing`

The reference page for `crates/delvec/src/compiler/view/showing.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW07xx — asset, render and authoring tooling (spec-0007; `delvec` subcommands)

This module's rows of a section whose prose is on the [`delvec::admit::diag` page](../../admit/diag.md#dw07xx--asset-render-and-authoring-tooling-spec-0007-delvec-subcommands).

| Code | Tool | Meaning |
|------|------|---------|
| `DW0895` | `delvec viewer` / `delvec render piece` / `delvec render batch` | **Floor under a roof that no body can walk to, and the step the walk was turned back at** (warning; `compiler::view::showing`). A roofed room with no way into it renders exactly like one with a door, and reads as finished. The engine holds both halves of the answer — the standable set and the walk that floods it from the piece's grade entrances — and this subtracts one from the other. It is a **report and never a refusal**: the engine cannot tell *unreachable* from *not meant to be reached* — a parapet is standable and nobody walks it, a sealed crypt is floor on purpose — and a gate that cannot draw that distinction is one an author learns to route around. Cells open to the sky are counted and never raised, for the same reason. What it can do is say **how much, where, and which step**: the roofed stranded count, the pockets it forms, the box of the most roofed one, and the frontier — every step the walk was refused at the edge of what it reached, classified by the step rule's own two ways of saying no (`metrics::step_allowed`). **The headroom arm is the one that is invisible at every scale below the whole building**: a standing body owns the cell it is in and the one above it, so a full-block rise is a jump and the cell two courses over the **takeoff** must be clear. A stairwell opening cut one course short therefore strands every storey above it while the flight itself walks, every render shows a floor at the top of it, and the same flight alone in an open box tests fully reachable. The message names the course to open, by cell. **Binding**: `enclosure: <id> — N of M standable cell(s) reachable … R refused step(s) at the edge of the walk, H of them one course of headroom short` on every run, zeroes included. |
