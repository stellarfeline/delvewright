# `delvec::compiler::nav::limit`

The reference page for `crates/delvec/src/compiler/nav/limit.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW1084 — a window shorter than its walk (`compiler::nav::limit`; error; exit 3)

The rule is spec-0133 §6. The walk is measured over the legs the critical path's route proof already proved ([`nav.md`](../nav.md), `DW0311`), so a window is judged against the same route the bot walks.

| Code | Meaning |
|------|---------|
| `DW1084` | **A limit shorter than the walk it allows** (spec-0133 §6–§7, `nav::limit::judge_limits`). For each limited objective the exported critical path completes, the walk ticks are the proven legs the path walks from the step at which the limit's clock starts (`activation`: the later of its last `after` prerequisite's step and the step that opens its quest; `quest-start`: the step that opens its quest, the spawn for a campaign-start quest; `{objective}`: that objective's activation) to the objective's own step, at the 4 t/block sprint model `DW0379` reads. A window (`ticks`) shorter than that walk is refused, naming the objective, the start event, the start cell and step, the walk in blocks and ticks, and the window; a window the walk equals is green. Not judged: a limited objective the critical path does not complete (an optional quest, a branch the path does not take) — counted as unwalked in the binding line. The walk is a floor, never a judgement: the activity a task asks beyond the walk is the author's call and the playtest's. **Binding line**, printed on every build that walks: `limit binding (DW1084): <n> objective(s) carry a limit; walk <w> of <ticks> tick(s) (<pct>%) — <obj>; …; <k> expiry world(s) exported, <t> tick(s) of waiting`, with an `unproven (no world kept)` clause naming a limit whose expiry witness no exported path carries; `validation/limit.json` states the same numbers. Build tier (exit 3). |
