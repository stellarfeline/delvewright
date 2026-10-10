# `delvec::compiler::attached`

The reference page for `crates/delvec/src/compiler/attached.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Every block the build writes is one the server keeps

A block whose pinned `canSurvive` rule asks a neighbour for something — a wall torch, a lever or a ladder a full face behind it, a torch or a lantern a face sturdy at its centre, a flower its soil, a carpet anything but air, a door's upper half its lower half — is removed by the server the first time a shape update reaches it when no neighbour gives it. The rule is the jar's, measured: `delvewright_dsl::support` reads `crates/dsl/data/support-1.21.11.tsv` and `support-bases-1.21.11.tsv`, written by `tools/maintenance/dump-support.py`, which asks the pinned server's own `canSurvive` over a level holding the block and one neighbour, for every neighbour cell and every blockstate of the registry.

**The world judged** is the one `DW0955` compares: the load world (`view::beat::load_blocks` over `view::beat::picture_base` — pieces, world edits, world-load seals, relight fixtures, trigger props, in the critical path's configuration at its first step), every block that needs support; then every distinct configuration the critical path passes (`nav::path_configurations`), each cell its forced writes lay and their six neighbours, judged again over that configuration's bytes. An unforced write is not laid by the model and a branch's configurations are not walked: neither is judged. A cell nothing was written to is air, except under the `ocean` horizon, where the generator's water (`floor_top < y ≤ level`), stone and bedrock stand outside every placed piece's box. The pass runs after the route proofs, so a climbable a forced leg needs is named by `DW0991` first.

**What it cannot judge** is counted, never refused: a block whose table row is `unjudged` (a crop asks the light, a vine or glow lichen reads a cell beyond its six neighbours, a big dripleaf stem needs two neighbours at once) is listed by block on the binding line, and so is a block the table does not hold.

**A superset of what the server shows.** The server drops an unheld block only when an update reaches it; a block nothing ever updates stays, held by nothing, until a gate, an edit or a runtime write next to it moves. `DW1002` refuses the block the rule does not hold, and `DW0955` can see only the ones an update reached during the boot it compares.

**Binding line**, printed on every build that reaches it: `attached blocks (DW1002): A of N non-air cell(s) at load need a neighbour to stay — H held, D dropped; C configuration(s) the critical path passes re-judged R cell(s) around their writes, E dropped there; unjudged by the measured rule: U cell(s) (block xn, …); not in the table: K cell(s) (…)`.

**The same rule everywhere a block is chosen.** The relight pass sites a fixture only where the measured rule holds it, reading every cell a runtime region write can change as air (`light::LightModel::with_volatile`), and `DW0354` judges an edit's placed blocks by the same table. Nothing in the engine states a support rule by hand.

## Diagnostics

### DW1002 — a block the server drops (`compiler::attached`; error; exit 3)

| Code | Meaning |
|------|---------|
| `DW1002` | **A block the build writes is one the pinned server drops.** Its measured `canSurvive` rule (`delvewright_dsl::support`) is met by none of its neighbours in the load world, or in a configuration the critical path passes, around the writes that configuration lays. Build-tier (exit 3), `compiler::attached`, after the route proofs. The message names each dropped block (up to eight, then the count): its state, its cell, the piece whose box holds it, whether it falls at load or once the critical path reaches step N, and for every neighbour that could hold it, that neighbour's cell, what it must give ("a full (sturdy) north face", "a down face sturdy at its centre", "be one of minecraft:dirt, …") and the block standing there. Prescription: repair it where the block is authored — give it the neighbour it asks for, hang it on a face that holds it, or choose a block that needs nothing there. Gallery: `probes/a-lever-hung-on-a-pane`. |
