# `delvewright_dsl::board`

The reference page for `crates/dsl/src/board.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW1116 — a row a board cannot show (`dsl::board`; spec-0135 §13; error)

A board is a list of places the story shows in the sidebar; the surface and its emission are in [`compiler.md`](../compiler.md) (the stage-5 row `boards[]` and the verbs `show-board`, `hide-board`, `place-on-board`). A board verb naming no declared board is `DW0112`; a `place-on-board` whose audience is `actor` where no player acted is `DW0503`.

| Code | Meaning |
|------|---------|
| `DW1116` | **A row a board cannot show.** A `place-on-board` whose `audience` is a wave's bodies (`{"wave": …}`), raised at the effect: a wave is a crowd, and a row of places names one body. Or one that places an actor declaring no `name`, raised at the effect and naming the actor: its row would read as an id. Validation tier (exit 1). |
