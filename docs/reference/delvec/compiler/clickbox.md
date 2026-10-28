# `delvec::compiler::clickbox`

The reference page for `crates/delvec/src/compiler/clickbox.rs`: the one rule every compiler-placed click box is built by, and the diagnostics-catalog row of the DW code this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## The rule

The client's crosshair takes an entity only when its box is strictly nearer the eye than the block the same ray hits; at equal distance the block wins [cited — the pinned client's `GameRenderer.pick` keeps the entity hit only when nearer than the block hit]. A click box is therefore built to enclose the block it stands over, never to share a face with it.

`ClickBox::fitted_to(cell, block)` builds every click box over a pressable's block (spec-0117 §3a). It reads the block's outline — the pinned jar's `getShape` for that blockstate, from `crates/dsl/data/outline-shapes-1.21.11.tsv` through `delvewright_dsl::blockshape::server_outline` (the union over the states a press can leave the block in) — takes its envelope `E`, and with `m` = `CLICK_MARGIN` (0.01 block) builds a box `max(x extent, z extent) + 2m` wide (an `interaction` is square in plan) and `y extent + 2m` tall, its floor `m` under `E`'s; centred on `E` on the longer horizontal axis, and on the other moved by the least amount that keeps it inside the body's cells expanded by `m`. A door's lower half is fitted over both its cells. Arithmetic is exact decimal, written as `f` floats.

| Block | Emitted |
|-------|---------|
| floor lever facing north | `X.5 Y-0.01 Z.5 {width:0.52f,height:0.395f,…}` |
| wall button facing north, unpowered | `X.5 Y.365 Z.8125 {width:0.395f,height:0.27f,…}` (outline centre `Z.9375`, moved `0.125` toward the cell) |
| a door's lower half, any face | `X.5 Y-0.01 Z.5 {width:1.02f,height:2.02f,…}` |
| campfire | `{width:1.02f,height:0.4575f,…}` |
| chest | `{width:0.895f,height:0.895f,…}` |
| iron bars (a gate's own answer) | `{width:1.02f,height:1.02f,…}` |

`ClickBox::sized` builds an assembly's declared hitbox: each face within `CLICK_MARGIN` of a block-grid plane is pushed out to `CLICK_MARGIN` past it (e.g. 1.5 × 2.5 on a cell floor → `{width:1.5f,height:2.51f}` at `Y-0.01`).

Not built by the rule: an NPC's dialogue hitbox and an actor's answer box (borne by a body that stands in air and moves) and a recovery stake's marker (summoned at a runtime position, `~ ~ ~`).

## Diagnostics

### DW1013 — a click box fits its block (`compiler::clickbox`; error; exit 3)

| Code | Meaning |
|------|---------|
| `DW1013` | **A click box that does not fit its block.** Build tier (exit 3), `compiler::clickbox::check`, an **engine self-check** read off the finished tree after emission, beside `DW0545`. Every `summon minecraft:interaction` in every shipped function is a subject, read with the shared command tokenizer. A summon whose class tag is `dw_borne` (an NPC's dialogue hitbox, or an actor's answer box, spec-0119) is counted as borne, by its bearer (`dw_abody_<id>`, `dw_npc_<id>`); one with a relative coordinate is counted as runtime-positioned; an assembly hitbox (`dw_asm`) is judged against the block grid (every face at least `MIN_PROTRUSION`, 0.001 block, off it); every other is **placed** and judged against the block it stands over — the block `setup_finish` places at that cell, or the gate's block for a gate's own answer (spec-0117 §3a). With the box as the server builds it (position as a double, `width`/`height` as the `f` floats written), every box of the block's outline must lie inside it by at least `MIN_PROTRUSION` on every face (the message names the face: west, east, floor, top, north, south), and the box must lie inside the block's cell(s) expanded by `CLICK_MARGIN`. A box over a cell that holds no block (air), or over a blockstate with no outline, is refused naming the cell. **Why the face test is the whole proof:** every outline box strictly inside the click box means every ray from an eye outside the box that reaches the block crosses the box's surface at least `MIN_PROTRUSION` earlier, so the pick takes the box — every click ray, not a sample. Reach is separate: the entity ray is traced 3.0 blocks against the block ray's 4.5. An unreadable interaction summon is refused under the same code. The remedy is the emitter's: build the box through `ClickBox::fitted_to`. Nothing in a campaign causes or repairs it. **Binding.** Every build prints `DW1013 binding: J of S interaction summon(s) placed at a cell and judged against its block (<tag family> <n>, …); T fitted to a two-cell body; A assembly hitbox(es) judged against the grid; B borne by a body (N NPC dialogue hitbox(es), A actor answer box(es)), R at a runtime position` and writes `validation/click-box.json` (`code`, `summons`, `judged`, `placed` by tag family, `two_cell`, `assembly`, `borne`, `borne_by`, `runtime`, `margin`, `min_protrusion`). |
