# `delvec::compiler::mark`

The reference page for `crates/delvec/src/compiler/mark.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW0897 — a mark stays in the room its anchor names (`compiler::mark`; error)

A mark is an anchor and an integer block offset, one type (`dsl::Mark`, spec-0066): a body's `anchor` + `offset`, a walk's or teleport's `to`, a cast row's `at`, a sound's point and every camera position. The offset says where beside a place, never which place.

| Code | Meaning |
|------|---------|
| `DW0897` | **A mark's offset leaves the placed piece its anchor belongs to.** Build-tier (exit 3), `compiler::mark`, run beside `DW0896` before any occupancy model. The rule quantifies over the three places a campaign puts a body at a mark: every body `dsl::body_sites` walks (resolved through `Plan::body_anchor_site`, the same area-scoped-then-global rule `Plan::body_point` applies), every `move-npc` / `move-actor` / `teleport` destination at every depth of every effect root (`dsl::for_each_campaign_effect`; a `move-npc` destination in the beat's scope through `plan::body_station`, exactly as its walk resolves), and every cast row spelled as a mark (in the beat's scope, as the ledger's station is). The mark's cell must lie inside `Plan::piece_bounds(area, anchor cell)` — the box wave seating and anchor seating are confined to — inclusive. Without it, arithmetic on an offset stands a body on the horizon or inside the next area, and every proof that reads a body as being in its anchor's room judges the wrong room. A camera position and a sound point are marks and are outside the rule: a dolly is placed where the framing wants it, and a shot of a building is taken from outside it. A mark whose anchor does not resolve is skipped; `DW0325`/`DW0345`/`DW0360` own dangling references. The message names the body or verb, the JSON pointer, the anchor, the offset, the anchor's cell and area, the cell reached and the box left, and every further refused mark in one line. Prescription: shorten the offset so the cell stays inside the piece, or name an anchor of the piece the body is meant to stand in; `remedy_reachability.rs` takes the move (`[40, 0, 0]` shortened to `[3, 0, 0]`) and ends green under `DW0897` and `DW0896`. Gallery probe `an-offset-out-of-the-room`. **Binding**: `mark binding: B body site(s), M with a non-zero offset; D destination(s), N with a non-zero offset; C cast row(s) at a mark; R refused for leaving the piece (DW0897).` on every build, zeroes included — a campaign that writes no offset prints its zeros and reads as checked. |
