# A place reads as what it is

Reader: the agent writing one place's program at step 9, and the engine work that hands it a frame. This is the research record behind spec-0123: how big a building is cut, how it is divided into rooms, how it shows its trade from the threshold and from the street, what makes its outside look lived in, and what a way owes its approach. Every rule is marked **[cited]** with its source, **[authored]** — reasoned here from a cited rule or from the engine, with no source of its own — or **[ruling]** — a finding of the side-by-side playtest of one hillside town built twice, taken as given. Sources are read for their rules; nothing of their text, images or builds is taken (ADR-0013: an unlicensed or non-commercial source is ideas only). Rooflines and facades are `roof-and-facade-craft.md`; light is `interior-lighting.md`; the width of a way is `clear-width.md`.

## 1. Sources

| Source | Terms | What it is |
|---|---|---|
| [WesterosCraft — Building fundamentals: Interiors](https://westeroscraft.com/docs/building/fundamentals/interiors) | No licence stated; ideas only | A build team's house style for interiors, written for applicants: rooms, purpose, occupancy |
| [WesterosCraft — Building fundamentals: Exteriors](https://westeroscraft.com/docs/building/fundamentals/exteriors) | No licence stated; ideas only | The same team on what an exterior communicates |
| [Minecraft Wiki — Tutorial: Settlement guide](https://minecraft.wiki/w/Tutorial:Settlement_guide) | CC BY-NC-SA 3.0 (the wiki's licence); ideas only | What a village, a town and a city are made of, and how buildings meet the street |
| [Minecraft Wiki — Tutorial: Adding beauty to constructions](https://minecraft.wiki/w/Tutorial:Adding_beauty_to_constructions) | CC BY-NC-SA 3.0; ideas only | Gardens, hedges, depth, worn floors, and the furniture of each room |
| Worch and Smith, *What Happened Here? Environmental Storytelling* (GDC), as relayed by [Nieman Storyboard](https://niemanstoryboard.org/2011/01/14/harvey-smith-on-environmental-storytelling-and-embedding-narrative/) | All rights reserved; ideas only, paraphrased | The practice of a space telling what it is and what happened in it through its objects |
| [Wikipedia — Signage](https://en.wikipedia.org/wiki/Signage) | CC BY-SA 4.0 | The history of the trade sign: a picture hung over the street for a public that could not read |
| [ADA Standards, chapter 3 — Floor and ground surfaces](https://www.access-board.gov/ada/guides/chapter-3-floor-and-ground-surfaces/) | US federal publication | Changes in level along an accessible route |
| International Building Code 2021, §1003.5 *Elevation change* and §1011.6 *Stairway landings*, read through a public code viewer | Copyrighted; paraphrased | A step in a path of travel, and how deep a landing is |

## 2. The cut and the size

**[ruling]** A site is cut with **one box per building or connector**; this cut's layout and seams were judged better than a cut of two terraces. A road, a flight of steps, a bridge, a yard and a square's centrepiece are each a place with a box of their own.

**[cited]** A village's buildings each connect to the street and not usually to each other; a town has larger open centres and denser structures; a settlement reads as genuine when it has a market place or, failing that, small trade shops. (Settlement guide.) A yard — a square space out back for vegetables, washing, animals or storage — is part of a house where the plot allows. (WesterosCraft.)

**[ruling]** A building's play space is **about twice** what the first cut gave it, so its author has room for rooms. The first cut's trade buildings stood on 64 to 121 cells of play space (8 × 8 to 11 × 11, read from its site plan); the rule puts a trade building at about 130 to 240 cells.

**[authored]** From the ruling and §3's room rule: a trade building's play space is at least **12 × 12**, and at least **14 × 14** where a workshop and a shop front both stand; a one-room house may stay smaller. These are the numbers a geometry brief states per trade and a site plan binds with `identities[]` (`box-extent`, `ge`), so the size is a declaration the plan is held to and never a class the engine infers.

## 3. Rooms, and reading as a trade

**[cited]** Divide the interior into distinct rooms with specific purposes before any wall goes up; place them as a household used them — work and storage downstairs, sleeping upstairs, the kitchen at the hearth; a single undivided room rarely reads as a lived-in space, and a two-block ceiling reads as cramped. (WesterosCraft.) Ceilings of three to four blocks feel spacious; hallways are two wide in a small house and three or four in a large one; a kitchen is a raised table, a sink, a range and a worktop, a bedroom a bed, a chest dresser and something on the wall. (Adding beauty.)

**[cited]** Most householders worked from home: a carpenter's downstairs workshop, a blacksmith's tools at the hearth, a weaver's loom. Every block serves a purpose; a small detail that shows occupancy — one mug on a barrel — beats a pile of crates that only takes up space. (WesterosCraft.)

**[cited]** In environmental storytelling the elements combine into a larger picture while each has significance on its own; the player assembles *what happened here* from the objects and their arrangement, and a scene that says nothing about the premise wastes its place. (Worch and Smith.)

**[cited]** For a public that could not read, a picture hung over the street named the trade — the bush of a tavern, the pole of a barber, the three balls of a pawnbroker; hanging signs became so common that cities limited their size and projection. (Signage.)

**[ruling]** A plain house with a loom and wool dumped in it does **not** read as a weaver's. An open-walled smithy, and a cartographer's shop with a reception room in front of an inner map room, do read as their trades. The best single room seen was a spawn-side room with a table and carpet.

**[authored]** The rule, from the four citations and the ruling. A building reads as its trade when **four things hold at once**:

1. **The working object stands in its working position**, not stored: the loom with a web on it and the bench before it, the anvil beside the forge with the quench at hand, the drawing table under the window with the finished map on the wall. A tool on a shelf is storage; a tool in use is a trade.
2. **The rooms follow the work**: the room a customer enters is not the room the work is done in, and the door between them says so — a counter room and a workroom, a shop and a store, a kitchen and a hall. A trade building has at least two rooms.
3. **The front says the trade before the door is opened**: a sign of the trade's own object hung over the street, the goods shown at a window or a counter, or the workroom itself open to the street where the trade was worked in the open (a smithy, a stall).
4. **Every room has a use a player could name**, and nothing is filler: a crate stack, a lone furnace, a bare bed in a shop are the cells the ruling judged bad.

**[authored]** The test, at the threshold: stand in the door and name the trade with no sign read. A frame from the threshold is the engine's picture of this test (spec-0123 §3.4), and the review names the trade from the frame before it reads the label.

## 4. Exterior life

**[cited]** A garden of shrubs, flowers, a few trees and potted plants; a hedge of one or two leaf blocks on a log; stairs and slabs as edging; worn floors in cracked and mossy variants where feet pass; a wall that looks empty gets something hung on it. (Adding beauty.) An animal pen, a vegetable patch or a wood-chopping place gives a plain house character. (WesterosCraft.)

**[ruling]** The first cut had too few of these: bushes, potted flowers and non-functional props that make the outside feel lived in.

**[authored]** Where the outside lives in this engine: a street's verge (the columns of a way place's extent outside its `clear`, `clear-width.md` §3), a yard or garden strip cut as its own open-sided place (spec-0123 §5), and the commons dressed by the stage-7 edit script (`scatter`, `plant`, `fragment`). Nothing stands in a way's clear width; a prop against a house wall stands in the street's verge, so the street's author dresses the house's foot, and the handout tells it what house it is.

## 5. The approach to a wide way

**[cited]** A stairway landing is as wide as the stair it serves and as deep as the stair's width (to 48 inches); a door onto it never takes more than half of it. (IBC §1011.6, through `clear-width.md`.) The engine confirms a way's declared width along its whole corridor and **one approach cell** at each end (spec-0120 §3).

**[ruling]** A three-wide grand stair with a one-wide crooked approach in one room is a defect; in the case seen a newel and a fireplace left one cell. A wide way's approach is as wide as the way, at least three; features such as a hearth and a stair stand three to five cells apart.

**[authored]** The engine judges the width and the first approach cell; the depth of the approach beyond that cell is the author's, from the ruling: a landing as deep as the way is wide, and nothing fixed — a hearth, a newel, a pillar — inside that landing. A newel stands beside the stair's width (`clear-width.md` §3), and a hearth's cheeks stand outside the landing.

## 6. The ground at a seam

**[cited]** A change in level above half an inch on an accessible route is a ramp, not a step (ADA 303); a change of less than twelve inches in a path of travel is a slope, and a single riser is admitted only where a code names it. (IBC §1003.5.)

**[ruling]** At a seam between places the ground must not leave a trench that a body walking the main road falls into and must detour to get out of.

**[authored]** The ring row across a seam at grade is walked at the sill: both pieces draw their ground to the sill at the crossing, and the side the plan hands the row to paves it. A cell of a way's corridor lower than the cells before and after it along the travel is a trench; `DW0921` asks a different question — whether a body is soft-locked — and a trench a body can jump out of is not a pocket, so its zero is not evidence against the ruling. What a check would measure is spec-0123 §11.

## 7. Rule provenance

| rule | § | provenance |
|---|---|---|
| one box per building or connector | 2 | **ruling** |
| buildings meet the street, not each other; a settlement has a market or trade shops | 2 | **cited** — Settlement guide |
| a yard is part of a house | 2 | **cited** — WesterosCraft |
| about twice the play space | 2 | **ruling**; the cells **authored** from the first cut's boxes |
| 12 × 12 and 14 × 14, bound by the brief | 2 | **authored** |
| rooms with purposes, placed as a household used them | 3 | **cited** — WesterosCraft, Adding beauty |
| the working object, occupancy over filler | 3 | **cited** — WesterosCraft |
| objects combine into what happened here | 3 | **cited** — Worch and Smith |
| the trade sign over the street | 3 | **cited** — Signage |
| the four-part trade rule and the threshold test | 3 | **authored**, from the citations and the ruling |
| gardens, hedges, worn floors, pens | 4 | **cited** — Adding beauty, WesterosCraft |
| where the outside lives: verge, open-sided yard, stage 7 | 4 | **authored** |
| a landing as wide and as deep as the stair | 5 | **cited** — IBC §1011.6 |
| the engine judges one approach cell | 5 | **cited** — spec-0120 |
| the landing's depth and what stands clear of it | 5 | **authored**, from the ruling |
| a step in a route is a ramp or a declared riser | 6 | **cited** — ADA 303, IBC §1003.5 |
| the ring row is walked at the sill; a trench is a local low in a corridor | 6 | **authored** |
