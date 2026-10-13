# Reference: dividing a map into districts

## Contents

- [What a district is](#what-a-district-is)
- [The test for one district](#the-test-for-one-district)
- [The bounds](#the-bounds)
- [Connectors](#connectors)
- [Landmarks and what crosses](#landmarks-and-what-crosses)
- [Worked example one The Stranding](#worked-example-one-the-stranding)
- [Worked example two The Treehouse Camp](#worked-example-two-the-treehouse-camp)
- [Recording the partition](#recording-the-partition)

Read at step 2, after the layout graph's nodes and edges are written and before
the site plan. A **district** is the unit one worker authors when a campaign is
authored in parallel (spec-0103); the direction that asked for it calls the
unit an area, and the two words name one thing. Every rule below is marked
**cited** — a named source, or a record of a real campaign — or **authored**,
meaning this page chooses and says so. A count outside the bounds below is
advice to re-read the partition, never a refusal.

## What a district is

**Cited.** Kevin Lynch, *The Image of the City* (MIT Press, 1960), holds that a
person carries a city in five kinds of element: paths, edges, districts, nodes
and landmarks. A district is "the medium-to-large sections of the city,
conceived of as having two-dimensional extent, which the observer mentally
enters 'inside of,' and which are recognizable as having some common,
identifying character." The character comes from "thematic continuities which
may consist of an endless variety of components: texture, space, form, detail,
symbol, building type, use, activity, inhabitants, degree of maintenance,
topography." An edge is a break in that continuity — "the boundaries between
two phases" — a shore, a wall, a change of level. A path is a channel of
movement and a node is a junction or a concentration; neither is a district.

**In the engine's terms** (authored): a district is a set of the layout
graph's places, every place in exactly one district, and the places of one
district connected to one another through the graph's own edges. A district has
no box and no coordinates: it is a design judgement about which scenes belong
together, recorded before any geometry exists.

## The test for one district

Apply the five tests in order to a candidate set of places. A set that passes
all five is one district.

1. **One continuity** (cited: Lynch). Name the thematic continuity in one
   phrase — the material and the use together: "limewashed cottages on a
   cobbled street", "black mud and wreckage under sky", "flesh and bone lit by
   what grows in it". If the phrase needs an *and* between two unlike things,
   it is two districts.
2. **An edge all round** (cited: Lynch). Walk the boundary in the graph: every
   edge that leaves the set should cross a break — water, a wall with a door
   in it, a stair or drop between levels, a change of the site's `fill`, a
   barred way. A boundary that cuts through continuous ground with nothing to
   mark it is a boundary the player will not feel, and a place on the wrong
   side of it.
3. **A name in the fiction** (authored). The design calls the set something —
   the harbour, the flat, the inside of the body, the Watch Tree. A set the
   design has no word for is a set the player has no word for either.
4. **One building or one region** (authored, on the direction this page serves).
   A building with its own interior and its own beats is a district on its
   own — a chapel and its crypt, a lighthouse — when it would otherwise sit in a
   district of a different continuity. A region of like buildings — a street,
   a camp, a field of wrecks — is one district, and each of its buildings is a
   place inside it. Never split one building into districts: a tree's roots,
   house and crown are places, and they belong to one district together.
5. **Few crossings by the spine** (authored). Count how many times the
   authored critical path crosses the set's boundary. One crossing in and one
   out is the usual shape; a boundary the spine crosses four times has cut a
   whole the story treats as one, and the two sides should be merged or the
   boundary moved to where the story's gates already are.

The boundaries that pass test 2 most cleanly are the plan's own `barred` seams
and `carry` edges: a gate the story opens is an edge the player already reads.
Both worked examples below have every boundary at a gate, a stair, a drop, a
shore or a carry.

## The bounds

A district is sized by meaning, never by the number of workers. The bounds
below are about what one worker can author in one run, and they advise.

- **Lower bound** (authored): a district holds at least one reached place where
  a quest beat or a declared station stands. A set with places and no beat is
  geometry nobody acts in; it is part of the district whose continuity it
  shares. One place can be a district when it is one whole building with its
  own interior and its own beats (test 4); a one-place district that is a
  stretch of road is too fine.
- **Upper bound** (authored, on two campaigns): at most about twelve places
  and at most one act of the quest plan. The Treehouse Camp (eleven places,
  one act) was completed by one agent on this route; The Stranding
  (fifty-five places, four acts) was not. Two points make a line, not a law:
  the number is revised when the demo level for spec-0103 adds a third, and a
  district above it passes with its count stated.
- **A scenery place counts toward the bound** (authored): a crown nobody enters
  is still a piece its worker draws.

## Connectors

A connector that is itself a place — a bridge, a long stair over a gap, a
corridor between two halls — is a place of its own with its own piece (the
site-plan rule). It belongs to exactly one district.

**The rule** (authored, on Lynch's observation that a path takes its identity
from its destination): a connector place belongs to the district whose
approach it is — the district at its far end from the campaign's `entry`,
walking the critical path. The player reads the rope bridge as the way *to*
the Watch Tree, so the Watch Tree's worker builds it and the Loom House's
worker builds only the gateway it leaves from. Where the critical path crosses
a connector both ways, the first crossing decides.

A connector that is a seam and no place — a doorway, a contact along a front,
a ladder hung on a wall — belongs to nobody. It is the boundary, and its
contract (the opening, the sill and rise, the landing on each side, the facing,
and what passes through it) is handed to both workers in their packets. Each
designs its own side against the same sentence, the seam's `form`.

## Landmarks and what crosses

**Cited.** A landmark is "a distant object seen from many vantage points"
whose silhouette orients the player (Shaver, *Invisible Intuition*, GDC 2018;
Lynch names figure-background contrast as its principal factor). A landmark
stands in one district — the one whose ground it rises from — and every
district that sees it declares the sight: a `vision` edge or a plan
`sightlines[]` entry, which the packets carry to both sides. The lighthouse
belongs to the breakwater; the flat, the Narrows and the body's back all
declare that they see it.

**Authored.** List, for every boundary, what passes through it: the party (and
in which direction the spine takes it), an NPC who walks from one district to
another, an item given on one side and spent on the other, the sky and the
fluid on each side, a sound heard across, a line of sight. Everything on that
list belongs to the whole, not to either district: the planner writes it, and
each worker's packet states it under the boundary it crosses.

## Worked example one The Stranding

Fifty-five places, sixty-nine edges, four acts (the content branch
`campaign/stranding`). Nine districts; every boundary falls on a stair, a drop,
a barred way, a shore or a carry.

| district | continuity | places | n | boundaries |
|---|---|---|---|---|
| Wrackham, the streets | slate roofs and limewash on cobbles, descending | coach-road, cliff-steps, high-street, harbour-lane, harbour-office, coyle-house, net-lofts | 7 | the drop into the basin; the lane's walk into the fish market; the lane's walk onto the pier |
| The harbour | stone quays, water, a working market | boatyard, fish-market, seawall, customs-house, slipway-stair, pier | 6 | the three above; the stair up to the chapel; the stair down from the boatyard to the whalers' shed; the barred slipway gate onto the flat |
| The chapel | one sacred building, rubble stone, with its crypt | seamens-chapel, chapel-crypt | 2 | the stair from the seawall |
| The breakwater arm | a stone arm into the sea with its lighthouse | whalers-shed, ropewalk, breakwater, lighthouse | 4 | the stair from the boatyard |
| The flat | black mud, wreckage and pillars under sky | pilgrims-way, wreck-field, carved-pillars, marracks-launch, mast-platform, mud-wrecks-west, mud-wrecks-east, mud-pillars-west, mud-pillars-east, mud-narrows-west | 10 | the barred slipway gate; three walks onto the Narrows and the skiff stage |
| The Narrows and the crossing | a stone ridge between black water, and the two ferry houses | narrows, narrows-shelf, skiff-stage, near-ferry-house, sea-skiff, far-ferry-house | 6 | the three walks from the flat; the barred far-house door onto the landing; the carries |
| The body's bank | hide and mud around the body, outside | far-landing, jaw-bank, flank-ridge, tail-bank, tail-road, run-bank | 6 | the far-house door; the walk into the mouth; the walk out of the breach; the stair down from the tail flank |
| Inside the body | flesh, bone and sculk, lit by what grows | mouth, throat, rib-cathedral, stomach, heart-chamber, breach, spine-stair, spine-top | 8 | the mouth; the breach; the stair up onto the crown |
| The back | the hide, open to the sky, falling in terraces | crown, brow, back-upper, back-middle, back-lower, tail-flank | 6 | the stair from the spine top; the stair down to the tail bank |

Why these and not others. The chapel is a district of its own by test 4: one
building, an interior, two beats, and a continuity (rubble stone, sacred use)
the harbour's quays do not share. The bank is one district although it is
walked at two ends of the body, because the edge from the run bank to the jaw
bank joins them and the hide is one continuity. The mud field beside the
Narrows stays with the flat because it is mud, and putting it with the Narrows
would add a fourth boundary seam where three suffice (test 2 asks for the
fewest crossings). The spine crosses each boundary once on the way out; the
return across the water is the ending's and crosses once more.

## Worked example two The Treehouse Camp

Eleven places, eleven edges, one act (the content tag
`release/the-treehouse-camp/v1.0.1`). The camp splits each tree into boxes a
different way — one tree into three stacked boxes — to test how a structure in
height is built; that is a box decision and not a district one, and it is not a
pattern for this page. Two districts.

| district | continuity | places | n | boundaries |
|---|---|---|---|---|
| The camp | one clan's houses on rope and timber, on and around the Hearth Tree | root-glade, hearth-house, hearth-crown, long-bridge, loom-house, low-bridge, seed-house | 7 | the barred rope gate at the Loom House's mouth onto the High Bridge |
| The Watch Tree | the tallest tree and the climb to its lantern hook | high-bridge, watch-house, watch-crown, watch-roots | 4 | the same gate; three `vision` edges from the crown down onto the camp |

Why not one district per tree. Four trees would give four districts of one,
two, five and three places over a twenty-minute level, two of them one house
with one beat each: below the lower bound, and against test 1, because the
whole camp is one continuity — one clan, one material, one builder. The one
edge the player feels is height with a gate across it: the High Bridge is tied
shut until the lantern is lit, and everything past it is the climb. So the gate
is the boundary, and the High Bridge belongs to the Watch Tree by the connector
rule — it is the way to the Watch Tree, entered from the Loom House. The Watch
Tree's three stacked boxes are three places of one district, as a building's
parts should be.

## Recording the partition

The partition is `districts[]` in `layout-graph.json`: one entry per district,
its id, the ids of its places, and a `note` carrying the continuity phrase from
test 1 and the fiction's name from test 3. Write the note: it is the first line
of every packet the district's worker receives. A layout graph with no
`districts[]` is one whole authored by one agent, and that is a legitimate
record for a small map. Show the table beside the layout graph at step 4, with
the counts; the engine's own census of the partition is advice and never a
refusal.
