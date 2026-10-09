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
| `DW0843` | **The piece is not the shape of its allocation.** The piece's structure size differs from the handed frame on any axis — the refusal prints both extents, the axis and the direction. **Undersize refuses exactly as oversize does**: the frame is the place's claim (spec-0098 §2), so a frame that moved with the plan or its terrain is answered by re-detailing the place, and a different building is a site-plan edit, taken visibly. Also under this code: a bound piece declaring no spatial contract, because the equivalence instrument would have nothing to read and a place detailed with such a piece would be a hole in the proof rather than a finding in it. Validation tier (exit 1), metadata only. **Binding: pieces measured.** **Deferral**: a `details[]` row is judged against a frame and a seam set the SITE PLAN computed, so where the plan has already refused one of them — the place's box off the kit grid (`DW0825`), or a seam the plan writes on this place and does not resolve (`DW0828`/`DW0829`) — the line still refuses on its own terms and says what it stands downstream of. The primary is in another document, where the reader cannot otherwise see the relation. |
| `DW0844` | **The piece's openings are not the plan's seams.** Both directions, from metadata, before any byte assembles: a seam this box must answer with no aligned face opening of a compatible class, and a face of the piece answering no seam — the *discovered* seam, at the earliest tier there is. Alignment means the face's opening cells answer the seam's allocated cells across the party plane, or **at** them for a seam lying in the piece's own floor course. Deliberately redundant with `DW0836`/`DW0838` and **not** their replacement: this reads declarations and names the piece and the seam at validation, they read bytes at build and remain the independent observers, and a piece that lies in its metadata passes here and reds there. Validation tier (exit 1). **Binding: seams required, and declared faces examined.** **Deferral**: a `details[]` row is judged against a frame and a seam set the SITE PLAN computed, so where the plan has already refused one of them — the place's box off the kit grid (`DW0825`), or a seam the plan writes on this place and does not resolve (`DW0828`/`DW0829`) — the line still refuses on its own terms and says what it stands downstream of. The primary is in another document, where the reader cannot otherwise see the relation. |
| `DW0987` | **A piece paints a cell it does not own** (spec-0098 §7). A bound piece's template holds a block other than `minecraft:structure_void` at a void cell of its frame — a neighbour's facade, the party wall a connection gave the other side, a clipped eave, a cell of the site's fill. **Air counts as painting**: the game places a template's air, so air over a neighbour's wall carves it. Read off the piece's own `.nbt` at validation (`compiler::detail::check_voids`, beside the byte-claim read), named per cell — piece-local and world — with the owner the plan awards it to. A piece whose size is not its frame's is `DW0843`'s and is not opened here. Validation tier (exit 1). **Binding: bound pieces opened, void cells examined, painted** (`void binding:`). |
| `DW0845` | **An owed anchor has no standing.** An owed name left unbound; one bound to a piece anchor that declares no cell (a region answers a gate — a gate station, or the gate region over a `barred` seam whose plane the place owns, which must be bound to a gate anchor over exactly the seam's cells); or one bound to an anchor the piece's own contract resolves into something a body cannot be at — a `no_body` region, a bar, a transit volume. Validation tier (exit 1). **Binding: owed names checked over every bound place.** |

**The owed names** are the subset of the synthesized vocabulary whose bearer is a
given box: its own `anchor/node-…`, `spawn` when it is the entry node, each
station of its node, each `anchor/unlock-…` whose opening side it is, and each
`barred` seam's gate region (`anchor/seam-…`) whose plane this place owns
(spec-0098 §2): the piece that owns a plane ships the gate standing in it, bound
to a gate anchor of the piece whose region is exactly the allocated cells and
whose `block` is what `close-gate` writes back. `dsl::siteplan::owed_anchors`
answers, beside `synthesized_anchors`, and a test proves the two partition. The
`anchors` map re-binds each owed name to an anchor of the piece, so a kit piece
keeps its own vocabulary and a campaign keeps its own.

**The frame is the place's claim** (spec-0098 §2): the ground under its plot
from the claim's bottom (the lower of its floor course and the lowest terrain
around it), its floor course, play space and ring, and on a roofed place its
lid and declared roof zone — minus the ring's fixed ground and every cell the
ownership rule awards elsewhere. The frame is the bounding box of what the place
owns; every other cell of it is a **void** the piece holds `structure_void` at.
A seam the place owns the plane of is answered at the plane itself; one a
neighbour owns, at the place's own first layer beside it.

**`delvec allocation <place>` / `--all` — the handout** (spec-0098 §4) emits,
per place, JSON derived from the plan on every invocation and **an input to
nothing**: `brief` (the node's intent, note, class, stations and kind),
`palette`, `concept` (the design record's `concept/<place stem>` row, or its
named absence before step 9), `sheet` (the record's `reference/` rows), the
frame's `extent`, `datum_y`, `world_min` and `space` (the play space,
piece-local), `neighbours` (each place whose claim meets this one's, by side,
with its kind, floor, roof and the seams joining them), `views` (each plan view
whose eye sees the place, with the `delvec snapshot --camera` that frames it),
`ground` (the fill kind, the claim's bottom and the floor piece-local, the
terrain along the perimeter with its min and max, every fixed ring cell with its
block, and the terrain-shaped columns under the plot), `roof` (courses, eaves,
the lid and top `y`, and every clipped eave), `seams` (each piece-local with its
`form`, the other place, face, cells, class, rise, answering classes, whether
this place owns the plane, and the ring's ground under a vertical seam),
`owed_anchors`, and `voids` (each run of the frame the place does not own, with
its owner). Two invocations print the same bytes. Every obligation is
recomputed from the plan at every validation.
