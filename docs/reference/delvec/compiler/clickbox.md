# `delvec::compiler::clickbox`

The reference page for `crates/delvec/src/compiler/clickbox.rs`: the one rule every compiler-placed click box is built by, and the diagnostics-catalog row of the DW code this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## The rule

The client's crosshair takes an entity only when its box is strictly nearer the eye than the block the same ray hits; at equal distance the block wins [cited — the engine's statement of the tie-break, first written for the seal answers; the pinned client's `GameRenderer.pick` keeps the entity hit only when nearer than the block hit]. A box whose face lies on a face of a block it stands in therefore loses every ray that reaches that face.

`ClickBox` builds every `minecraft:interaction` the compiler places at a cell: the asked-for box (one block across and `n` tall for a press body, the declared size for an assembly hitbox), with each face that lies within `CLICK_MARGIN` (0.01 block) of a block-grid plane pushed outward to `CLICK_MARGIN` past it. A face further from the grid stays where it was asked for. Arithmetic is exact decimal, so the emitted text is what it reads as.

| Emitter | Box | Emitted |
|---------|-----|---------|
| `interact` objective (non-pressable prop or marker) | column, 2 tall | `X.5 Y-0.01 Z.5 {width:1.02f,height:2.02f,…}` |
| click trigger on a point (`Body::Point`) | column, 2 tall | same |
| click trigger on a gate region (`Body::Region`), each shell cell | column, 1 tall | `{width:1.02f,height:1.02f,…}` |
| `close-gate` seal answer, each shell cell | column, 1 tall | same |
| sealed shortcut door, each sealed-side approach cell | column, 1 tall | same |
| shortcut unlock, bonfire, shop, trap disarm, timed-gate disarm | column, 2 tall | `{width:1.02f,height:2.02f,…}` |
| assembly hitbox | declared size, faces near the grid pushed past it | e.g. 1.5 × 2.5 on a cell floor → `{width:1.5f,height:2.51f}` at `Y-0.01` |

Not built by the rule: an NPC's dialogue hitbox (borne by the NPC's body, which stands in air and moves it by `tp`) and a recovery stake's marker (summoned at a runtime position, `~ ~ ~`).

## Diagnostics

### DW1013 — a click box wins the pick (`compiler::clickbox`; error; exit 3)

| Code | Meaning |
|------|---------|
| `DW1013` | **A click box placed at a cell has a face on the block grid.** Build tier (exit 3), `compiler::clickbox::check`, an **engine self-check** read off the finished tree after emission, beside `DW0545`. Every `summon minecraft:interaction` in every shipped function is a subject, whichever emitter wrote it, read with the shared command tokenizer. A summon whose class tag is `dw_borne` (an NPC's dialogue hitbox) is counted as borne; one with a relative coordinate is counted as runtime-positioned; every other is **placed** and judged: its six faces, computed as the server builds the box (position as a double, `width`/`height` as the `f` floats written), must each lie at least `MIN_PROTRUSION` (0.001 block) off the block grid. A face on the grid ties a block in that cell along every ray reaching the face, and the block wins the click. **Why the face test is the whole proof:** a box that covers a cell with every face off the grid holds the cell's unit cube strictly inside it; every block outline lies inside its cell, so every ray from an eye outside the box that reaches the block crosses the box's surface at least `MIN_PROTRUSION` earlier, and an eye inside the box picks the box at distance zero — every click ray from every cell, not a sample. Reach is separate: the entity ray is traced 3.0 blocks against the block ray's 4.5, so the cells a box can be clicked from are those within 3.0. An unreadable interaction summon is refused under the same code. The remedy is the emitter's: build the box through `ClickBox`. Nothing in a campaign causes or repairs it. **Binding.** Every build prints `DW1013 binding: J of S interaction summon(s) placed at a cell and judged (<tag family> <n>, …); B borne by an NPC body, R at a runtime position — every judged box's faces at least 0.001 block off the block grid` and writes `validation/click-box.json` (`code`, `summons`, `judged`, `placed` by tag family, `borne`, `runtime`, `margin`, `min_protrusion`). |
