# `delvec::compiler::chain`

The reference page for `crates/delvec/src/compiler/chain.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).
What the module guards, and the world build it splits, are in [`compiler.md` §4](../../compiler.md#one-ticks-command-chain-stays-under-the-games-limit).

## The cost model

What the pinned game charges against one execution context's quota [cited — the three callers of `ExecutionContext.incrementCost` in the pinned 1.21.11 server jar]:

| Charged by | When | Counted as |
|------------|------|------------|
| `BuildContexts.execute` | once per `execute` sub-command stage that carries a redirect modifier | every `execute` sub-command keyword before `run`, as the pinned command tree names them — an upper bound, since a keyword can also be an argument's literal (`positioned as`) |
| `ExecuteCommand.execute` | once per executing source, for the command a chain ends in | one per line |
| `CallFunction.execute` | once per executing source, for a function call | one for the call, then the callee's body |

The limit is the default of `max_command_sequence_length` [cited — `GameRules.MAX_COMMAND_SEQUENCE_LENGTH`, `ldc 65536`, in the pinned server jar]. A function's **chain** is one for its call plus every line's cost, a call's line adding the callee's chain; `schedule function` runs nothing this tick and costs its own line. The count is **per executing source**: a line that forks over several entities runs its tail once per entity, a factor no reading of the bytes bounds.

A function that reaches itself in the same tick has no chain unless it is a **counted loop**: it calls itself only under `if score <h> <o> matches ..<n>`, adds one to that score unconditionally before the call and writes it nowhere else, and every line that enters it from outside last wrote the score with an unconditional `scoreboard players set <h> <o> <k>`. It then runs at most `n - k + 1` passes per entry, and its chain is that many passes. The creator overlay's aim ray is one (256 passes).

## Diagnostics

### DW0984 — one tick's command chain stays under the game's limit (`compiler::chain`; error; exit 3)

| Code | Meaning |
|------|---------|
| `DW0984` | **A shipped function's command chain could pass the game's limit.** Build tier (exit 3), `compiler::chain::check`, an **engine self-check** read off the finished tree after emission, over every function of every datapack the tree ships (the delve's own, the creator overlay, the PackTest pack) against one name space. A function whose chain — its own commands and every function it calls in the same tick, costed as above — exceeds `max_command_sequence_length` (65,536), or that reaches a call cycle that is not a counted loop, is refused: the server would stop it part-way with `Command execution stopped due to limit` and nothing would read the line. Since a caller's chain holds its callee's, refusing every function refuses every root. The message names how many functions refuse and the first; the remedy is the emitter's, which splits its work across ticks (`chain::steps`, `schedule function`), as the world build does. Nothing in the campaign repairs it. **Binding.** Every build prints `chain binding: F shipped function(s) measured, L counted loop(s), S world-build step(s); longest chain N (`f`) of a 65536 limit, per executing source; R refused (DW0984).` and writes `validation/chain-length.json`. |
