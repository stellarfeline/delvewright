# `delvec::compiler::resting`

The reference page for `crates/delvec/src/compiler/resting.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Every block the build writes reads as resting on what holds it

A flower pot on a bottom slab, a lantern on a carpet, a cart bed of two top slabs over the street: the server keeps each of them, and each shows half a course or more of air under itself, so a player reads it as floating. The judgement is made from the pinned jar's own outlines (`delvewright_dsl::blockshape::outline`, `crates/dsl/data/outline-shapes-1.21.11.tsv`), over the load world `DW1002` judges (`view::beat::load_blocks`), its unwritten cells answered as `DW1002` answers them.

- **The gap** under a block is the air between the bottom of its own outline and the top of the outline below it, over the block's footprint. A block is a **candidate** when the gap is half a course or more and comes from a half-height surface: the block is hung in the top half of its own cell (a top slab, a top trapdoor), or it stands on a block whose outline stops at half a course or lower (a bottom slab, a carpet, a bottom trapdoor). A block over air is an overhang its neighbours answer for, and no candidate.
- **Held elsewhere**: a block whose measured `canSurvive` rule reads no neighbour below — a hanging lantern, a wall torch, a ladder — is no candidate. A candidate is **held** when it, or a candidate it touches across a side face, touches across a side face a block that is no candidate (a wall, a post, a beam), or touches across its top face a full block that has a full block beside it (a ceiling, not a bale set on it).
- What is left **reads as floating**: reported per piece with each block, its cell, the gap in sixteenths and what stands under it.

Binding, printed on every build: `resting blocks (DW1076): C of N outlined cell(s) at load stand half a course or more over what is under them — H held at a side or from above, F read as floating; not in the outline table: K cell(s)`.

It is a report, never a refusal: whether a block hovers is the author's to see and repair, and the engine states what it measured. The repair is in the generator that draws the block (`references/detail.md`): stand it on a full-height top — a bottom slab one course up, a full block, a post — set it against what holds it, or choose a block that fills the gap.

## Diagnostics

### DW1076 — a block that reads as floating (`compiler::resting`; warning)

| Code | Meaning |
|------|---------|
| `DW1076` | **A block the build writes reads as floating.** Half a course or more of air stands between its outline and the outline under it — it is a top slab or top trapdoor, or it stands on a bottom slab, a carpet or another surface that stops at half a course — and no neighbour holds it at its side or from above. A warning (never refused), `compiler::resting`, over the load world after `DW1002`'s load arm. One warning per piece, naming up to twelve blocks (then the count): state, cell, the gap in sixteenths and the block under it. Prescription: repair it in the generator that draws it — a bottom slab one course up, a full block or a post under it, set against a wall, or a block that fills the gap. Unit-tested in `compiler::resting::tests`. |
