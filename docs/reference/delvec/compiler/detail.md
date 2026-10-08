# `delvec::compiler::detail`

The reference page for `crates/delvec/src/compiler/detail.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW0842–DW0845, DW0848 and DW0882 — the detail plan (`compiler::detail` + `dsl::prefab` + `delvec detail`; spec-0050, spec-0058)

Stage 6 of the map pipeline: a place is detailed inside the box the whole gave
it. The document is `detail-plan.json` (§2); this is what judges it.

**What invokes each check, and what happens without it.** `DW0842`–`DW0845` run
in `validate_loaded`, the one funnel every `delvec` subcommand's validation goes
through — `build` included — so a defect cannot reach a datapack by skipping
`delvec validate`. `delvec detail` (spec-0058) runs `DW0843`–`DW0845` and `DW0848`
once more, through the same `check`, over the piece it is about to write and the
row it is about to write — an in-memory registry holding the library plus that
piece — so every one of them refuses **before any file is written**, in the
words validation prints afterwards. `DW0848` runs at `delvec prefab audit` — the admission
event, where the library's integrity lives — and again wherever a `details[]` row
consumes the piece, so a piece admitted without the check cannot be consumed unjudged. The
frame itself is computed in `Plan::build`, which is the only constructor a world
can be reached through.

**No opt-out exists.** A place is bound or unbound, and the kind is determined by
whether a row exists rather than chosen among demands: there is no
acknowledgement field, no exemption list and no severity an author selects.

| Code | Rule |
|---|---|
| `DW0842` | **The binding does not bind.** A `detail-plan` in a campaign with no site plan (the limiting case, naming the missing document); a `place` naming no layout-graph node; two rows for one place; a `piece` the prefab library does not hold; an `anchors` key that is not a name this place owes; an `anchors` value naming no anchor of the piece. Validation tier (exit 1). **Binding: rows resolved, against the plan's box count.** **Folded at a zero box count**: when the site plan resolves no box every row misses by construction, so one line states the count, names every row and defers to the primary that already said it — `DW0824` when the campaign carries no `layout-graph.json`, and the plan's own emptiness otherwise. With a map to be wrong about it refuses per row. |
| `DW0843` | **The piece is not the shape of its allocation.** The piece's structure size differs from the handed frame on any axis — the refusal prints both extents, the axis and the direction. **Undersize refuses exactly as oversize does**: the box is the footprint, so a smaller building means a smaller box, which is a site-plan edit, taken visibly. Also under this code: a bound piece declaring no spatial contract, because the equivalence instrument would have nothing to read and a place detailed with such a piece would be a hole in the proof rather than a finding in it. Validation tier (exit 1), metadata only. **Binding: pieces measured.** **Deferral**: a `details[]` row is judged against a frame and a seam set the SITE PLAN computed, so where the plan has already refused one of them — the place's box off the kit grid (`DW0825`), or a seam the plan writes on this place and does not resolve (`DW0828`/`DW0829`) — the line still refuses on its own terms and says what it stands downstream of. The primary is in another document, where the reader cannot otherwise see the relation. |
| `DW0844` | **The piece's openings are not the plan's seams.** Both directions, from metadata, before any byte assembles: a seam this box must answer with no aligned face opening of a compatible class, and a face of the piece answering no seam — the *discovered* seam, at the earliest tier there is. Alignment means the face's opening cells answer the seam's allocated cells across the party plane, or **at** them for a seam lying in the piece's own floor course. Deliberately redundant with `DW0836`/`DW0838` and **not** their replacement: this reads declarations and names the piece and the seam at validation, they read bytes at build and remain the independent observers, and a piece that lies in its metadata passes here and reds there. Validation tier (exit 1). **Binding: seams required, and declared faces examined.** **Deferral**: a `details[]` row is judged against a frame and a seam set the SITE PLAN computed, so where the plan has already refused one of them — the place's box off the kit grid (`DW0825`), or a seam the plan writes on this place and does not resolve (`DW0828`/`DW0829`) — the line still refuses on its own terms and says what it stands downstream of. The primary is in another document, where the reader cannot otherwise see the relation. |
| `DW0845` | **An owed anchor has no standing.** An owed name left unbound; one bound to a piece anchor that declares no cell (a region answers a gate, and a gate region is never owed by a place); or one bound to an anchor the piece's own contract resolves into something a body cannot be at — a `no_body` region, a bar, a transit volume. Validation tier (exit 1). **Binding: owed names checked over every bound place.** |

**The owed names** are the subset of the synthesized vocabulary whose bearer is a
given box: its own `anchor/node-…`, `spawn` when it is the entry node, and each
`anchor/unlock-…` whose opening side it is. A gate region (`anchor/seam-…`) is
never owed — it stands in a party plane the whole owns. `dsl::siteplan::owed_anchors`
answers, beside `synthesized_anchors`, and a test proves the two partition rather
than agree. The `anchors` map re-binds each owed name to an anchor of the piece,
so a kit piece keeps its own vocabulary and a campaign keeps its own: the quest
layer bound those names to places at stage 3, before any detail existed, and
detailing must never force a quest edit.

**`delvec allocation <place>` / `--all`** emits the handed allocation as JSON:
the frame's extents, the datum in piece-local coordinates, every seam of the box
in piece-local coordinates with its face, cells, class, rise and the answering
class the table above requires, the owed anchor names, and the detail plan's
palette. It is derived from the site plan on every invocation and is **an input
to nothing** — no gate, no build step and no check ever reads what it prints, so
a file made of it is a copy with no consumer and its staleness has no vector into
the build. Every obligation is recomputed from the plan at every validation.
