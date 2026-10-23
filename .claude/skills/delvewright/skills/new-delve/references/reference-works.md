# Reference works — the best existing example of a kind of map

## Contents

- [When to consult this page](#when-to-consult-this-page)
- [The works at a glance](#the-works-at-a-glance)
- [An organic structure the party enters: The Beached Thing](#an-organic-structure-the-party-enters-the-beached-thing)
- [A giant statue: The Thing Beyond the Fog](#a-giant-statue-the-thing-beyond-the-fog)
- [The ceiling of a complex single box: Vesperhold](#the-ceiling-of-a-complex-single-box-vesperhold)
- [Several areas connected in three dimensions: The Treehouse Camp](#several-areas-connected-in-three-dimensions-the-treehouse-camp)
- [How a reference work is added](#how-a-reference-work-is-added)

## When to consult this page

**At the design step, before a form, a statue, an organic interior or a single
large box is authored**, find the nearest work below, read its record, and start
from its form or program. Meet at least its level of detail: a campaign whose
giant is a handful of solids with no lights, when a work of the same subject
made with the same tool already stands at hundreds of solids with lights and
per-part materials, has invented a worse one.

A reference work shows how to build **one kind of map**. It is the content
repository's public history, read by URL and never copied into the campaign:
take the structure, the approach and the scale, and write the campaign's own
subject. Every URL below names a tag or a commit of
`stellarfeline/delvewright-campaigns`, never a branch, so it reads the same
tomorrow. Fetch a file with `curl -fsSL <raw url>`; read a directory by its
tree URL or from a clone checked out at the tag or commit.

A work's story is not what it is a reference for. Read the form, the layout and
the record of how it was built; leave its plot alone.

## The works at a glance

| kind of map | work | pinned at | read first |
|---|---|---|---|
| an organic structure the party enters | The Beached Thing | commit `7a67e92d77a3831cd3aca26005b84a1862e1c02e` | `HOW-IT-IS-BUILT.md` in its directory |
| a giant statue | The Thing Beyond the Fog | commit `7a67e92d77a3831cd3aca26005b84a1862e1c02e` | `HOW-IT-IS-BUILT.md` in its directory |
| the ceiling of a complex single box | Vesperhold | tag `release/vesperhold/v1.1.0` | `campaigns/vesperhold/DESIGN.md` |
| several areas connected in three dimensions | The Treehouse Camp | tag `release/the-treehouse-camp/v1.0.1` | `campaigns/the-treehouse-camp/DESIGN.md` |

## An organic structure the party enters: The Beached Thing

**The reference for**: a body the size of a hill, sculpted by `delvec sculpt`
from a declared form, that the party walks into through an opening, climbs onto,
and finds lit from inside. A rotting sperm whale on a mud flat.

**Take from it**

- **The form's structure.** One form file holds everything: a `box` of
  148 x 60 x 146, 192 solids (180 `add`, 10 `cut`, 2 `shelf`) built from
  capsules, discs, ellipsoids and boxes, 5 `anchors`, 5 `sink` entries burying the
  belly, `noise`, a `ground` apron of packed mud that is part of the piece, and
  5 `lights` entries. Parts are named by anatomy (head, jaw, trunk, flippers,
  hump, rump, flukes, wound), not by geometry.
- **Per-part materials.** Every part declares its own material family, a full
  block with a matching stair and slab, so tones follow the anatomy and not the
  sculpt's shading: one hide, a browner lower flank, a few large sloughed
  patches with defined edges, a lighter fluke edge, bone only where bone is bare,
  flesh red only where the body is open. The mud bank the party looks from is its
  own material, so it reads as ground.
- **The way in and the way up are cut into the form.** A `cut` opens the wound at
  grade; a `shelf` is the only way onto the back, and the record shows the walk
  from grade failing to reach the back when the shelf is removed.
- **The lighting approach.** No lantern stands on a floor. Every source is set
  into the body's own surface through `lights[].hull`, in seeded Poisson-disk
  scatter over walls and vault: bay A recessed behind a stair cover with a slot,
  bay B embedded flush, a dense band at floor height beside a sparser band above,
  and a mass of flesh standing in each wide room so no cell is farther than a
  light reaches. The two rooms differ in kind, which is the craft question the
  level answers.
- **Scale at playable size.** The whale is 143 blocks long, about 9.5 blocks to
  a metre; the head is a third of the length. A mud bank west of the body is the
  vantage from which the whole animal reads broadside inside the served view
  distance. The record states the angles it subtends.
- **Research recorded per rule**, each marked cited or authored, with the
  ideas-only sources named. Keep that habit.

**Size and surfaces**: one piece, 148 x 60 x 146; `delvec sculpt`, `lights[].hull`
(`embedded`, `recessed`), `shelf`, `sink`, `noise`, a vantage and a short quest
walking the vantage, two bays and the back. The build reports 0 pockets in
22,746 reachable cells and every walkable cell lit.

**Do not copy**: the proportion table, which is a whale's; take the method of
stating each proportion with its source. The record says its machine ladder last
ran on an older engine and that its staging gate refuses it on the design record
a reference level does not carry; it shows how to build the body, not a shippable
delve.

**URLs** (commit `7a67e92d77a3831cd3aca26005b84a1862e1c02e`)

- tree: <https://github.com/stellarfeline/delvewright-campaigns/tree/7a67e92d77a3831cd3aca26005b84a1862e1c02e/references/the-beached-thing>
- how it is built: <https://github.com/stellarfeline/delvewright-campaigns/blob/7a67e92d77a3831cd3aca26005b84a1862e1c02e/references/the-beached-thing/HOW-IT-IS-BUILT.md>
- the form: <https://github.com/stellarfeline/delvewright-campaigns/blob/7a67e92d77a3831cd3aca26005b84a1862e1c02e/references/the-beached-thing/forms/the-beached-thing.json>
- the form, raw: <https://raw.githubusercontent.com/stellarfeline/delvewright-campaigns/7a67e92d77a3831cd3aca26005b84a1862e1c02e/references/the-beached-thing/forms/the-beached-thing.json>
- how it is built, raw: <https://raw.githubusercontent.com/stellarfeline/delvewright-campaigns/7a67e92d77a3831cd3aca26005b84a1862e1c02e/references/the-beached-thing/HOW-IT-IS-BUILT.md>

## A giant statue: The Thing Beyond the Fog

**The reference for**: a huge figure that is not entered, seen from far off and
made to read from its silhouette, sculpted from a declared form. A figure
waist-deep in a sea, seen through fog and revealed by a lightning strike.

**Take from it**

- **The form's structure.** A `box` of 91 x 97 x 36, 175 solids (173 `add`, 2
  `cut`) of boxes, capsules and ellipsoids; the figure rises 96 blocks. It is
  built for the one side its camera sees, and the form's box ends behind it.
- **Silhouette first.** The record builds a hierarchy of large, medium and small
  shapes (torso and raised wings; mantle and claw; feelers, wing bones, eyes) and
  states which rule was cited and which was authored after a sculpt read wrongly.
  Each wrong first reading is written down with its repair; take that loop: sculpt, look at the
  outline, name what reads as the wrong thing, change the form.
- **Per-part materials.** Four palette families (dark prismarine, polished
  deepslate, polished blackstone, blackstone), a distinct material for the part
  that must read against its neighbour (the feelers against the chest), and a
  detail part scalloped and proud of its surface.
- **The lighting approach.** The figure is not lit from inside. Eight cells of a
  glowing block are set into two cut sockets as the eyes, via `lights[].at`, so
  the one point the eye goes to is the one point that glows; the rest is read by
  the sky the story gives it.
- **Scale at playable size.** The figure stands about seventy blocks beyond the
  ferry's bow, the nearest point a body can see it from, and is inside the
  served view distance. The record measures the clearing the camera sees against the pinned client.

**Size and surfaces**: one piece, 91 x 97 x 36; `delvec sculpt`, hand lights,
`cutscene`, `lightning`, a party shown in its own cutscene, `set-atmosphere` in a
`sequence`, a ferry link.

**Do not copy**: the cutscene, the bolt and the stand-ins verify other
capabilities; this work is the reference for the figure's form, not for how a
reveal is staged. The record says its stand-in and clearing readings were taken
in a real client, which a still frame cannot show.

**URLs** (commit `7a67e92d77a3831cd3aca26005b84a1862e1c02e`; no tag exists yet)

- tree: <https://github.com/stellarfeline/delvewright-campaigns/tree/7a67e92d77a3831cd3aca26005b84a1862e1c02e/references/the-thing-beyond-the-fog>
- how it is built: <https://github.com/stellarfeline/delvewright-campaigns/blob/7a67e92d77a3831cd3aca26005b84a1862e1c02e/references/the-thing-beyond-the-fog/HOW-IT-IS-BUILT.md>
- the form: <https://github.com/stellarfeline/delvewright-campaigns/blob/7a67e92d77a3831cd3aca26005b84a1862e1c02e/references/the-thing-beyond-the-fog/forms/the-thing-beyond-the-fog.json>
- the form, raw: <https://raw.githubusercontent.com/stellarfeline/delvewright-campaigns/7a67e92d77a3831cd3aca26005b84a1862e1c02e/references/the-thing-beyond-the-fog/forms/the-thing-beyond-the-fog.json>
- how it is built, raw: <https://raw.githubusercontent.com/stellarfeline/delvewright-campaigns/7a67e92d77a3831cd3aca26005b84a1862e1c02e/references/the-thing-beyond-the-fog/HOW-IT-IS-BUILT.md>
- key frames: <https://github.com/stellarfeline/delvewright-campaigns/tree/7a67e92d77a3831cd3aca26005b84a1862e1c02e/references/the-thing-beyond-the-fog/review>

## The ceiling of a complex single box: Vesperhold

**The reference for**: how much one box can hold. A castle on a crag with
forty-two places, built as one whole, a single grammar piece holding the whole
site (approach, crag, castle and undercroft), not as several boxes built
separately. Treat it as the upper bound of what one box can hold; a map that
would need more is several boxes.

**Take from it**

- **One piece, one program.** `prefab/vesperhold` is expanded at 172 x 104 x 292,
  seed 1. A generator under `design/generator/` paints the site into a voxel grid
  from one module per part (terrain, approach, walls, ward, cathedral,
  undercroft, keep, bell tower, gardens), reads every position from one layout
  file, and emits the grammar's partition as `design/programs/vesperhold.json`.
  The layout file is the authority both the generator and the layout chart read.
- **Heights as a table.** Valley floor, castle floor, keep dais, wall walks,
  tower rooms, bell deck and undercroft each have one stated height; every
  place is read against it.
- **The place table.** `DESIGN.md` lists every place with its region and what the
  player does there, and marks which are on the road and which are pockets off it
  (at least one pocket from every region the road crosses).
- **Light at the scale of a site.** Lighting profile `dark`: 4,829 of 43,198
  floor cells reachable on foot sit below light 3 under a clear night sky, and
  the area's torch pass lights the rest at build. Read how that was measured
  before choosing a number of your own.
- **Anchors and gates stated by the generator.** 137 point anchors and seven gate
  regions, with the generator refusing two anchors in one cell.

**Size and surfaces**: one piece, 172 x 104 x 292; grammar programs, `valley`
horizon, anchors, gate regions, lit by profile; the campaign has four classes and
a combat plan around it.

**Do not copy**: its placement. It binds the piece through `areas[]`, which the
engine refuses beside a site plan (`DW0839`); a campaign draws this as
a site plan of one box and no seam, whose one piece is the whole site. Its record
says a site plan was dropped because the derived walls and roofs could not give
the castle a designed exterior; a site plan's one piece draws its own
walls and roof inside the frame the plan hands it (step 9), so the reason is
one to re-check, not to repeat. The program is stored through
git-lfs: `git lfs pull` after a checkout, or the raw URL returns a pointer.

**URLs** (tag `release/vesperhold/v1.1.0`, commit `894cd9cabe589f08bee1aaaedfe0063138cc6b7b`)

- tree: <https://github.com/stellarfeline/delvewright-campaigns/tree/release/vesperhold/v1.1.0/campaigns/vesperhold>
- design record: <https://github.com/stellarfeline/delvewright-campaigns/blob/release/vesperhold/v1.1.0/campaigns/vesperhold/DESIGN.md>
- generation record: <https://github.com/stellarfeline/delvewright-campaigns/blob/release/vesperhold/v1.1.0/campaigns/vesperhold/GENERATION.md>
- the generator: <https://github.com/stellarfeline/delvewright-campaigns/tree/release/vesperhold/v1.1.0/campaigns/vesperhold/design/generator>
- design record, raw: <https://raw.githubusercontent.com/stellarfeline/delvewright-campaigns/release/vesperhold/v1.1.0/campaigns/vesperhold/DESIGN.md>
- the layout, raw: <https://raw.githubusercontent.com/stellarfeline/delvewright-campaigns/release/vesperhold/v1.1.0/campaigns/vesperhold/design/generator/castle/layout.py>

## Several areas connected in three dimensions: The Treehouse Camp

**The reference for**: a map of several structures at different heights, joined
by connectors a body walks or climbs. Houses on four giant trees, joined by rope
bridges and rope ladders.

**Take from it**

- **The site plan, in numbers.** A 112 x 112 site on heightmap terrain, eleven
  boxes (nine entered, two scenery only) standing at eight different floor
  heights from y 66 to y 95, three bridges that are their own way-classed places,
  and two rope-ladder seams. Its `DESIGN.md` section 4 is the geometry brief in
  full: every box's footprint, floor, headroom and roof in one table.
- **A connector is a place.** A bridge is a box of its own with its seams at each
  end declaring their opening; a ladder is a seam that a body climbs. Both are in
  the site plan and the layout graph, not described around them.
- **Scenery places.** A box never entered (a trunk base, a crown) closes a tree
  without being a destination.
- **Light placed with the place.** Each platform and bridge carries its own
  lanterns and fire; the finale adds light on a camp that is already lit.
- **Safety stated.** A rail on every edge a body can reach, the deepest fall
  below the survivable one, and every way from the ground leading back to the
  start.

**Size and surfaces**: 112 x 112 site, eleven boxes; `site-plan.json`,
`geometry-brief.json`, `layout-graph.json`, `detail-plan.json`, eleven programs,
`world-edits.json`, rope ladders and bridges, an `open` terrain fill.

**Do not copy**: how it splits one tree. The Watch Tree is divided into three
stacked boxes to test an extreme case, and the record calls that split the
challenge case; A
tree that is one structure is one box. Divide a map by meaning (a place the party
enters is a box; scenery and connectors are separate only when they are places);
the partitioning guide the districts feature adds, when the engine carries it,
states that rule.

**URLs** (tag `release/the-treehouse-camp/v1.0.1`, commit `3eeb000a98ea38127fdc8d5c270f8f82144eaf88`)

- tree: <https://github.com/stellarfeline/delvewright-campaigns/tree/release/the-treehouse-camp/v1.0.1/campaigns/the-treehouse-camp>
- design record: <https://github.com/stellarfeline/delvewright-campaigns/blob/release/the-treehouse-camp/v1.0.1/campaigns/the-treehouse-camp/DESIGN.md>
- site plan, raw: <https://raw.githubusercontent.com/stellarfeline/delvewright-campaigns/release/the-treehouse-camp/v1.0.1/campaigns/the-treehouse-camp/site-plan.json>
- geometry brief, raw: <https://raw.githubusercontent.com/stellarfeline/delvewright-campaigns/release/the-treehouse-camp/v1.0.1/campaigns/the-treehouse-camp/geometry-brief.json>
- layout graph, raw: <https://raw.githubusercontent.com/stellarfeline/delvewright-campaigns/release/the-treehouse-camp/v1.0.1/campaigns/the-treehouse-camp/layout-graph.json>
- design record, raw: <https://raw.githubusercontent.com/stellarfeline/delvewright-campaigns/release/the-treehouse-camp/v1.0.1/campaigns/the-treehouse-camp/DESIGN.md>

## How a reference work is added

**Every demo level the owner approves that verifies how to build a kind of
map becomes the reference for that kind. A level verifying something else (for
example rendering) does not.** Reference levels live whole in the content
repository's `references` directory, one subdirectory per id, and are released apart from the flagship
campaigns, under `reference/<id>/vX.Y.Z`.

When a demo level is approved, its revision is tagged on the content repository,
and a section is added here naming the capability, the kind of map it is the
reference for, what to take, its size and surfaces, and what not to copy, each read at
the revision and described from what was read. Its URLs name that tag or commit
of the public content repository, never a branch, and the work is not a reference
until they resolve. The queue of demo levels states the same rule and points
here.
