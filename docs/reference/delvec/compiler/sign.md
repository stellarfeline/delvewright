# `delvec::compiler::sign`

The reference page for `crates/delvec/src/compiler/sign.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW1075 — a sign text the sign cannot show (`compiler::sign`; spec-0128 §8; error; build tier)

A stage-5 `signs[]` entry writes the words of a sign the piece placed ([`compiler.md`](../../compiler.md), `signs[]`). Judged by `sign::check_signs` over the assembled world, the same block map the container proofs read.

| Code | Meaning |
|------|---------|
| `DW1075` | **The sign at the anchor cannot show the text.** One code, three shapes, each named with the entry, its anchor and its cell: *no sign* — the cell holds a block that is not a sign (read from the block-entity table, `registry::block_entity_type`: `minecraft:sign` for every standing and wall sign, `minecraft:hanging_sign` for every hanging and wall-hanging sign, 48 blocks); *two texts* — a second entry names a sign another entry already writes, so one `data merge` would overwrite the other; *too wide* — a line, in the English or in a declared language's translation, is wider than the board that sign draws, measured by the compiler's one font measure (`textfit::default_font_width` of the line with its markup dropped, plus `bold_widening`): 90 font px on a standing or wall sign, 60 on a hanging or wall-hanging sign (`SignBlockEntity` / `HangingSignBlockEntity.getMaxTextLineWidth()`, read from the pinned 1.21.11 client jar; the client draws only the first line `font.split` wraps each message into, so the rest is cut). Build-tier (exit 3); evaluated over the **edited** model when a stage-7 script exists. All three are silent in the game: a `data merge block` against a cell with no sign changes nothing, and a cut line ships cut. Prescription: shorten the line, move words to another line (four to a side), stand a wider sign, or point the entry at the anchor the piece declares at its sign's cell — never a `set-block` effect, because the sign is furniture the piece places. A text whose cell holds no sign and lies inside a place still standing in its stand-in (`Blockout::stand_in_holding`) is deferred to that place's detail, the rule the container proofs keep, and named on the stderr line `sign binding: N sign text(s), J judged against the assembled world (<board>: n, …), L line(s) measured over the English and K declared language(s), D deferred to the detail of a place still standing in its stand-in (…)`, printed for every build, zeroes included. |
