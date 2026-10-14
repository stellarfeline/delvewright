# Dividing a map into regions

## Contents

- [What a region is](#what-a-region-is)
- [The verified ceiling of one region](#the-verified-ceiling-of-one-region)
- [Recommendation 1 — one region until a signal says divide](#recommendation-1--one-region-until-a-signal-says-divide)
- [Recommendation 2 — divide where the design already has an edge](#recommendation-2--divide-where-the-design-already-has-an-edge)
- [Recommendation 3 — the division depends on what is being made](#recommendation-3--the-division-depends-on-what-is-being-made)
- [Where established practice informs this and where it stops](#where-established-practice-informs-this-and-where-it-stops)

Every recommendation here is **authored** unless marked **cited**; each states
its reasoning and the signal that would show it wrong. The page is revised as
campaigns are made: record a signal that fires here, with the campaign that
showed it.

## What a region is

A region is a place authored as one whole. Inside it you work the whole-site
way: one program draws the ground, the walls, the roofs, the water and the
places within, and the engine checks it. The engine's object is the
layout-graph node with its box; a region is one place.

## The verified ceiling of one region

**Cited** (the content tag `release/vesperhold/v1.1.0`,
`prefabs/vesperhold.json`, `campaigns/vesperhold/DESIGN.md`, `GENERATION.md`):
a 172 × 104 × 292 piece — 50 224 columns, 71 103 standable cells, 43 303
reachable on foot — holding 42 designed places, 144 named spots (137 points,
7 gate regions), 3 shortcut doors, 13 quests with 37 objectives and 16 waves,
about an hour of play, drawn by one 52.9 MB program from one generator, built
and played to completion.

A region larger or denser than this is **unverified**: nothing says it fails,
and nothing says it builds. Read a new region against the census the engine
prints for every place — its frame's extent and cells (`delvec allocation`),
its templates, its share of the world-build chain (`DW0984`).

## Recommendation 1 — one region until a signal says divide

**Authored.** A whole built as one program joins its places and dresses its
ground better than the same places built as separate pieces, because inside
one program nothing is a declared cell and you see everything; the losses of
dividing are confined to the boundaries, so the fewer boundaries the better,
down to none.

**The signal** (authored, not measured): divide when the brief exceeds
Vesperhold on any one count — more than about 40 designed places, a footprint
over about 50 000 columns (about 170 × 290), more than about 150 named spots,
or more than one hour of play inside one continuity — or when the brief itself
names two wholes the fiction treats as separate (two castles, a town and its
harbour, an island and its lighthouse).

**What would show it wrong**: a region at or under the threshold that cannot be
finished (lower the threshold to the size that was finished), or a two-region
map at the threshold whose seam the owner's walk cannot find (raise it to the
size that still reads whole).

## Recommendation 2 — divide where the design already has an edge

**Cited**: Lynch's edge, "the boundaries between two phases, linear breaks in
continuity" (*The Image of the City*). A seam is a crossing cut at a declared
cell on a declared face, drawn by two authors from one sentence; it costs least
where the player already expects a break — a river, a curtain wall and its
gate, a cliff and its stair, a change of ground — and most through continuous
fabric, a street or a hall.

**Signal**: a render of two regions at their seam that reads as a cut.

## Recommendation 3 — the division depends on what is being made

**Authored**, reasoned from the ceiling:

| the whole | divide it | the reasoning | what it trades |
|---|---|---|---|
| a small town, a camp, one castle and its approach | **do not divide**: one region | under the ceiling; every join is drawn, the ground is one design | all of the detail; one author's whole effort |
| a city, a castle complex of several wholes, a town and its harbour | **one region per castle, never per house**: a cluster of like buildings sharing one continuity is one region, each building a place inside its program | a house is below any author's ceiling, and a boundary between two houses runs through continuous fabric | consistency held by the whole's reference sheet, the palette (`detail-plan.palette`) and every seam's `form`; weakest at seams through fabric |
| a continent, a coast, a valley system | **one region per landscape unit** — a valley, a bay, a ridge — bounded by water, ridgeline or cliff | the player reads the landscape, so a region's budget goes to ground | less detail per building; consistency held by a shared heightmap at the boundary |

## Where established practice informs this and where it stops

**Cited**: engines keep the whole as one persistent artifact and hand it
authorial chunks (Unreal's World Partition); the macro layout is drawn first
and renegotiated while soft (The Witcher 3's pins on a map, Breath of the
Wild's overlay); build teams fix per-region style and lay out plots before
building; kits fix standards before designers build. None of these sizes an
authorial unit to one author's attention — streaming cells are sized to memory
and draw distance — so the sizing rule above is authored, the division lines
are cited, and how stable an agent's authoring is at a given size is what the
next campaigns measure.
