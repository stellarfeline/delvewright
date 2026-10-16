# `delvec::compiler::gatesignal`

The reference page for `crates/delvec/src/compiler/gatesignal.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW1047 — no redstone signal opens a shut gate (`compiler::gatesignal`; error; exit 3)

Asked in the world block right after the sculk proofs (`emit::routes::prove_world`), over the assembled world and every block a runtime write may lay (`sculk::runtime_writes`, `sculk::possible`: a `fill-region`, `set-block` or `close-gate` at any root and depth, every laid way and shortcut seal, both phases of every timed gate, all at once). It reads the plan and the blocks, never a route.

| Code | Meaning |
|------|---------|
| `DW1047` | **A redstone signal can open a shut gate.** Judged: every shut seam of the site plan (`siteplan::PlacedSeam::bar`, `anchor/seam-<edge>`) whose block, or any block a cell of it may hold, reads a signal (`blockshape::redstone_reader`) — today the closed iron trapdoor a floor is shut with (`SEAM_HATCH` over a drop's or a stair's hole, `SEAM_COVER` over a ladder's). **Out of scope**: a wall's bars, `minecraft:iron_bars`, read no signal, so they are counted as not judged and never refused. An iron trapdoor opens whenever `Level.hasNeighborSignal` holds [cited — `TrapDoorBlock.neighborChanged`; `compiler.md`, *No signal opens a shut gate*], which power reaches from a signal source in one of its six face neighbours, or from a source beside a conductor in one of them. The proof powers, from every cell that may hold a source (`blockshape::redstone_source`), its six face neighbours and the six neighbours of each that may hold a conductor (`blockshape::redstone_conductor`), reading every source as on and pointing everywhere [authored — it can only refuse more]. The message names the gate's anchor, the trapdoor cell, the source cell and block (as placed, or laid by which write), how the power gets there, and the remedy: move the source or the gate. |

**Binding.** Every build that assembles a world prints, before the verdict and zeroes included: `DW1047 binding: N shut gate(s) whose block reads a signal judged (F through a floor, C cell(s)); I shut gate(s) whose block reads none not judged; M signal source(s) examined (W laid by a runtime write), the nearest D block(s) from a judged cell` — or `no source to measure against`. The same counts land in `validation/gate-signal.json` (`gates`, `floor_gates`, `cells`, `inert_gates`, `sources`, `sources_written`, `nearest`), with `cells` stated also as `examined`, which the gallery's coverage gate refuses at 0.
