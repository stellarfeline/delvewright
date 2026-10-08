# `delvec::compiler::ways`

The reference page for `crates/delvec/src/compiler/ways.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW0547–DW0549 and DW0555 — a campaign's contingent ways (`compiler::ways`; spec-0042)

A **way** is a region a piece's spatial contract declares its own traversal edge
contingent on: `laid` — empty as built, and opening fills it — or `cleared` —
standing in the way's block as built, and opening voids it. The prefab checker
proves the geometry on the bytes as shipped: the edge really is severed, applying
the delta really joins it, and every space reachable only under an opening is
named with the opening it needs. What it cannot prove is that anything ever
*opens* it — "happens" exists only where effects exist. That half is the
campaign's, and `open-way` is the whole of the surface.

**Three questions, one pass, at plan time.**

1. **Staging.** Which ways the placed pieces actually put in the world, with their
   world cells, their block and their sign, read through the same placement
   transform the face contract uses. A way is a fact about a PLACEMENT, not about
   a prefab: a piece placed twice puts two breaks in the world, so a reference
   that matches none or several names no one of them (`DW0547`). Ways of one name
   on several edges of one piece union, exactly as the grammar's own reachability
   walk unions them.
2. **Disposition.** Per staged way: which effect opens it, at which quest-DAG
   point, and whether the party is forced to cause that firing — or that nothing
   opens it. **A door that never opens is content**, so a never-opened way is
   reported and is not a finding by itself. Which verdict a way gets is computed
   from what is staged behind it; the author picks nothing.
3. **The one red that follows.** Required content standing beyond a way no forced
   opening precedes (`DW0548`).

**Whose reachability decides "beyond".** The piece's own: the contract's declared
graph, rooted at the `entry` space it declares, `vision` edges excluded and a
`drop` traversed forward only. That is deliberately the reading the grammar's
reachability walk takes, because the claim being consumed is the piece's claim.
Two consequences follow and both are stated rather than discovered — a space a
neighbour could reach through a mated exterior face is not counted as reached
(seams are the face contract's business, `DW0780`, and spec-0042 keeps ways off
them), and a `barred` edge is traversed (its bar is opened through the anchor
surface, which this pass does not model). Both make the red rarer, never commoner.

**What "in time" means, and it is not a second opinion.** An opening counts for a
required element when it is FORCED and the quest DAG guarantees it has already
fired: the predicate is `Plan::gate_fired_before`, the same strict-ancestor
relation the region-write model orders the world by, handed over rather than
re-derived. So an `open-way` on an objective a parallel branch merely interleaves
ahead does not count, and neither does one on the objective the party has to reach
by crossing the break.

**Known incompleteness: the way judge runs on the exported path only.** Its
openings and required elements are read off the exported path's `PathFiring`;
no branch path is judged. An element whose objective is off the exported path is
judged as a campaign reference (no `by_step`), and an `open-way` fired only from
a beat off that path is unforced there, so a way a branch opens for its own
branch-only element is refused. Direction: this can only turn a proof red, never
let a route ship; no fixture, gallery point or campaign reaches it.

**The binding count is the artifact.** Every build whose placed world stages a way
emits `validation/ways.json`: placed pieces, pieces declaring a way, ways staged,
opened, unforced-only, never-opened, `open-way` effects, required elements
examined, and one row per way with its cells, its block, its sign and every
opening that names it. A campaign that stages no way emits no file — a file
reading zero is a finding, and an absent file is the honest statement that there
was nothing to enumerate.

| Code | Meaning |
|------|---------|
| `DW0547` | **An `open-way` reference does not name exactly one placed way.** Either no placed piece stages a way of that name on that piece, or several do (the same prefab placed in two areas, or drawn twice by one pool). Build-tier (exit 3), `compiler::ways`. The message prints what the world does stage, way by way, with the area and placement index of each. Prescription: bind the way-carrying piece to one area, or give the second placement its own piece; a way is a fact about a placement, so one reference cannot open two of them. |
| `DW0548` | **Required content stands beyond a way no forced opening precedes.** A campaign-referenced anchor — an objective's target, an NPC stand, a wave spawn, a lane waypoint — resolves into a space the carrying piece's contract reaches only through a way that is never opened, opened only from a root the party can skip (a trap payload, a shop offer, a death bundle, a shortcut's far side), or opened at a quest-DAG point that does not precede the element. Build-tier (exit 3), `compiler::ways`. The message names the way, the effect (if there is one) and the element, and states which of the three it is; a never-opened way also carries the cell count of the building standing behind it. Prescription: give the way a forced `open-way` on an objective the quest DAG puts before this one. Nothing else about a never-opened way is a finding. |
| `DW0549` | **A placed piece declares a way the staging could not put in the world.** The placed pieces declare more distinct ways than reached the enumeration — a way whose `boxes` resolve to no cells at all. Build-tier (exit 3), `compiler::ways`. A way's whole content is the cells its opening writes: one that resolves to none is a break nothing can repair, and every disposition, ledger count and reachability verdict past that point would be stated over a world smaller than the one being shipped. The message carries both numbers. Prescription: fix the piece's metadata. |
| `DW0555` | **The way-reachability check examined zero required elements** (advisory). Ways are staged and no objective anchor, body or campaign reference resolves into a declared space of any way-carrying piece, so the dispositions are reported and nothing proves an opening is needed for anything. Warning, `compiler::ways`. Not a refusal: content behind no way at all is ordinary, and a delve whose ways are scenery is a delve. What is not acceptable is for that to be indistinguishable from a proof. |
