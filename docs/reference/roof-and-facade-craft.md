# Rooflines and facades at Minecraft scale

Reader: the agent writing a detail piece's program, or the engine work that hands
it a frame. This is the research record behind spec-0098's `roof` declaration and
behind the skill's step-9 guidance on what a building's outside owes. Every rule
is marked **[cited]** with its source or **[authored]** — reasoned here from a
cited rule, with no source of its own. Sources are read for their rules; nothing
of their text, images or builds is taken (ADR-0013: an unlicensed or
non-commercial source is ideas only).

## 1. Sources

| Source | Terms | What it is |
|---|---|---|
| [Minecraft Wiki — Tutorial: Roof construction guidelines](https://minecraft.wiki/w/Tutorial%3ARoof_construction_guidelines) | CC BY-NC-SA 3.0 (the wiki's licence); ideas only | The community's standing reference on roof pitch, shape and proportion in blocks |
| [WesterosCraft — Building fundamentals: Exteriors](https://westeroscraft.com/docs/building/fundamentals/exteriors) | No licence stated; ideas only | A build team's house style for facades and roofs, written for applicants |
| [US Navy — Roof Framing (NAVEDTRA), via InspectAPedia](https://inspectapedia.com/roof/Roof-Framing-NAVEDTRA.pdf) | US Government work | Rise, run, pitch and span as framers define them; the roof shapes by name |

## 2. Pitch — how many courses a roof rises

**[cited]** In blocks there are three standard pitches: **63.4°**, one course of
rise per column (double blocks); **45°**, stairs, one course per column with the
stair's step; **22.5°**, alternating upper and lower slabs, one course per two
columns. Other pitches are repeating patterns of two or three blocks (slab,
upside-down stair, stair: five courses per seven columns). (Minecraft Wiki.)

**[cited]** Wetter regions carry steeper roofs. (Minecraft Wiki, stated as a rule
of thumb.)

**[cited]** Pitch is rise over run; a framer's span counts the overhang: a house
38 wide with a one-foot overhang each side has a 40-foot roof span. (NAVEDTRA.)

**[authored]** From the two above, the courses a gable needs above its eaves
course over a roof span `W` cells (the shell footprint plus both eaves):

| pitch | courses above the eaves course |
|---|---|
| 22.5° (slabs) | `⌈(W − 1) / 4⌉` |
| 45° (stairs) | `⌈(W − 1) / 2⌉` |
| 63.4° (blocks) | `W − 1`, in practice capped by §4 |

The ridge of an odd span is one column wide; of an even span, two. **[cited]** On
an odd-width building a steep roof comes to a point over the central feature.
(Minecraft Wiki.) **[authored]** So a door centred on an odd span puts the ridge
over the door; a building whose gable end should read symmetric is given an odd
interior extent plus two wall cells, which is odd again.

## 3. Eaves — the roof overhangs the wall

**[cited]** "Most roofs should include an eave or overhang. A roof that cuts
straight to the wall looks unfinished." (WesterosCraft.) **[cited]** "Depth that
exists purely to break up a flat wall without any structural logic tends to look
arbitrary" — depth that serves the structure is named: overhangs, buttresses,
window reveals, sill projections. (WesterosCraft.)

**[authored]** One cell of overhang is the common idiom and is what `eaves: 1`
declares; deep eaves (`2`) suit low, wide roofs and verandas. The eaves course
sits level with the wall's top course and one cell beyond it, which is why the
engine's roof zone begins at the ceiling course rather than above it
(spec-0098 §3). An eave that would reach a neighbour's wall stops at it, as a
real eave does; the engine clips it and prints where.

## 4. Proportion — how tall a roof is against its walls

**[cited]** For an inhabited building the roof is one to two storeys tall, a
storey being about 4–5 blocks; a 20-wide building under a 45° gable gets a roof
10–11 blocks tall, and splitting the building into sections brings each section's
rise to about 5–7. A one-block parapet and a one-block walkway bring the apparent
height down by about three. (Minecraft Wiki.)

**[authored]** By a place's size: a small room (clearance
3–4, footprint 8–16) carries a 45° gable of 4–8 courses, which is a roof roughly
as tall as its walls; a hall (clearance 8, footprint 16–32) at 45° would rise
8–16 courses and reads as a barn, so a hall is either sectioned across its short
axis (several gables), given the 22.5° slab pitch (4–8 courses), or roofed flat
behind a parapet. The plan declares `courses` from this table and the engine
reserves exactly that; it does not pick a pitch, because the pitch is a judgement
and the number is what the whole has to keep clear of other places.

## 5. Shape vocabulary

**[cited]** Gable (a ridge, two slopes, a triangular end wall); hip (four
slopes, no gable end); shed (one slope); gambrel and mansard (two pitches per
side, more headroom in the top storey); cross-gable and cross-hip (a T or an L).
The eave is the roof's lower edge, the rake its sloped edge, the ridge the top
line, a hip the outward sloping intersection and a valley the inward one.
(NAVEDTRA; Minecraft Wiki.)

**[authored]** In a piece these are the grammar's one taper recursion with the
paint set to mass over air (`grammar.md` §3: "a pitched roof and a pointed arch
are the same program with the paint inverted"), a hip being the taper on both
axes and a cross-gable two tapers crossing. A roof is drawn inside the roof zone
the plan reserved and nowhere else: the frame is the bounding box of what the
place owns, and a ridge that wants one more course is a plan edit.

## 6. Facade — what a wall owes a body standing in the street

**[cited]** Avoid long unbroken runs of one block on any surface; use one colour
of timber and daub per build; mix two complementary woods for variation; from
inside, a roof's underside is not left as plain blocks. (WesterosCraft.)

**[cited]** Choose the roof material by region and standing: slate for the
wealthy or urban, thatch for the low and middle, wood widely, sod in the cold.
(WesterosCraft.)

**[authored]** The engine hands the piece its full facade — the one-cell ring
the wall stands in, from the floor course to the eaves — and the whole hands the
seam's cells for the door. What the piece owes the street, from the sources
above: depth with a structural reason (a sill, a reveal, a buttress, the eave),
a window rhythm that does not leave a run of one block longer than a storey is
tall, and a roof that overhangs. What it must not do: open a way the plan did
not allocate (`DW0838`), stand a solid side in the party's open air without
declaring it shown (`DW0885`), or paint the party wall a neighbour owns
(`DW0987`).

## 7. Where two grounds meet — stitching

**[cited]** Terrain rendered as tiles that are each meshed on their own opens
cracks at tile boundaries; the fixes are to hang a skirt down from each edge or
to make neighbouring tiles agree on the heights along their shared edge, and
the second is the proper one (Ulrich's chunked LOD, as summarised in the TUM
terrain-rendering tutorial, [`Terrain.pdf`](https://www.cs.cit.tum.de/fileadmin/w00cfj/cg/Research/Tutorials/Terrain.pdf),
and [a Khronos forum answer on chunked-LOD cracks](https://community.khronos.org/t/chunked-lod-cracks/72110)).

**[cited]** The GDMC settlement challenge judges a generated settlement on its
*adaptability* to the terrain it was given, beside functionality, narrative and
aesthetics ([arXiv 1803.09853](https://arxiv.org/abs/1803.09853)); its entries
read the world's heightmap first and build on it, and a report on one notes
that vanilla village paths ignoring terrain height leave buildings unreachable
([williamcwi/GDMC](https://github.com/williamcwi/GDMC)). Landscape first, then
structures, is the order the practice settled on.

**[authored]** Translated to this engine: the site's terrain is one heightfield
the whole owns, and every place is a tile stitched to it along its boundary —
the place's edge ground meets the terrain within one block, or a solid face
stands between them (a plinth's retaining wall, a sunken yard's revetment), or
a seam declares the crossing. Inside its boundary a place shapes its ground
freely. What is refused is the crack: air between the two surfaces under the
higher edge, which is Ulrich's gap in blocks. There is no skirt: a skirt hides
a crack and this engine refuses one.

## 8. The gap this record leaves

No source read here states a rule for **window spacing** or for a facade's
**base / body / top** proportion; both were searched for and not found in a
source the project may read. They are recorded as open rather than invented. A
creator wanting them researches again or decides by eye at playable scale
(`CLAUDE.md`: buildings are judged at playable scale).
