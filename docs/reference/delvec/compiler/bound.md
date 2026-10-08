# `delvec::compiler::bound`

The reference page for `crates/delvec/src/compiler/bound.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW0960 — the boundary and the world agree (`compiler::bound`; error; exit 3)

| Code | Meaning |
|------|---------|
| `DW0960` | **The declared boundary disagrees with the world** (spec-0092 §10). Build tier (exit 3), `compiler::bound::judge`, asked after boundary safety (`DW0322`). Two shapes of one rule. **A place a body is put, outside a region that returns**: a critical-path step's cell, or a link's or gather's `to`, outside the playable region — the clock takes the body back within a second. Every such cell is an anchor inside a placed piece (`DW0897` holds teleport destinations to their piece), and the region contains every piece, so on today's derivation this shape cannot fire on a document that validates: it stands as the check that the region and the places agree, and its binding line counts the places it read. **A region that does not return, round a world a body can leave**: `returns: false` while a reachable walkable cell (`World::reachable_walkable_rooted`) lies outside the region, or the walk region enters an open sea body (`nav::open_sea_entry`, the labelling `DW0322` judges stranding over). The message names the cell; the remedy is to seal the edge or let the boundary return. Every build that assembles a world prints `boundary binding: …` — whether the region returns, the places examined and outside, the reachable cells examined, the refusals. |
