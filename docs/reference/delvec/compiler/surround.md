# `delvec::compiler::surround`

The reference page for `crates/delvec/src/compiler/surround.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW0821/DW0836–DW0839 — the derived blockout (`compiler::blockout`; error + two advisories)

This module's rows of a section whose prose is on the [`delvec::compiler::blockout` page](blockout.md#dw0821dw0836dw0839--the-derived-blockout-compilerblockout-error--two-advisories).

| Code | Rule |
|---|---|
| `DW0854` | **The surround's inner slope has grown a standable staircase.** `compiler::emit`, build tier (exit 3), run inside the world block beside the boundary proofs. A walk flood starting on the surround's own gap-floor cells reached a column outward of the crest line, so the landform no longer bounds the map and a body can walk out of the delve over the mountains. **Why it is a check and not an argument.** The generator already guarantees this by construction: no surround column stands exactly one block above the gap-floor datum, so the floor's own walkable component is bounded above by the datum and the first thing outward of it is a two-block riser, which vanilla's auto-step and jump cannot take. But that is a property of what the generator wrote, and between the generator and the world there is a gravity settle, possibly a stage-7 edit script, and a palette whose blocks may be a different height — any of which can put back the riser the generator never wrote. So the proof reads BYTES: it floods the same `nav::World` every route proof uses and asks the same step rule, sharing none of the generator's arithmetic, which is what makes it an observer of the derivation rather than a restatement of it. The generator runs its own flood too, over its finished tile contents, so a violation dies at generation as well — two proofs of one property at two moments, and the later one is the one that is about the shipped world. Binding: standable gap-floor cells the flood starts from, stated on every surround build, because a flood that started from nowhere passes for free and looks exactly like this one. |
