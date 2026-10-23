# Clear width

What a door, a stair or a road owes along its whole length, and how a cell grid carries it. Agent-facing: the record behind spec-0120's rule that a fixed structure never blocks part of a way. The consumer is the compiler's clear-width check; this page is where its rules come from.

Every rule is marked **[cited]** with its source or **[authored]** — reasoned here from a cited rule, with no source of its own. The building codes are copyrighted texts and are paraphrased, never copied (ADR-0013: ideas only); the US accessibility standard is a federal publication and is quoted where its words are the rule.

## 1. The term

**[cited]** A door's *clear width* is the opening a body passes, measured between the face of the open door and the stop, and nothing may project into it below 34 inches (ADA Standards 2010, 404.2.3). A walking surface's clear width is likewise the width free of anything a body meets (403.5.1). The term names **what is free**, not what was drawn: a 36-inch doorway with a post in it is not a 36-inch doorway.

**[cited]** The engine already uses the word in this sense: a seam's `opening` is *clear width* and *clear height* in cells (`crates/dsl/src/metrics.rs`, `Opening`), and the stand-in lays a stair's treads across exactly that width — *a stair the width of its doorway is what a body can actually walk up* (`crates/delvec/src/compiler/blockout.rs`, `tread`).

## 2. The rules practice agrees on

| Rule | Source | Standing |
|---|---|---|
| The width of an egress path is **not diminished along the path**; nothing is placed in a component's minimum width except the projections the code names | IBC 2021 §1003.6 (paraphrased) | cited |
| The projections admitted are small and named: a fully opened door leaf, up to 7 inches and never past half the width; trim and decoration, 1½ inches a side; handrails, 4½ inches a side at or below rail height | IBC 2021 §1005.7.1, §1005.7.2, §1014.8 (paraphrased) | cited |
| A stair's **landing is as wide as the stair** it serves, and as deep as the stair's width or 48 inches, whichever is less; a door onto a landing never takes more than half of it | IBC 2021 §1011.6 (paraphrased) | cited |
| A door's **manoeuvring clearance extends the full width of the doorway** on both sides, and the floor inside it has no change of level | ADA 404.2.4, 404.2.4.4 | cited |
| A protruding object **does not reduce the clear width** an accessible route requires | ADA 307.5 | cited |
| A doorway is narrower than the hallway it opens from, to leave room for its frame; a hallway is at least twice the player's width | *The Level Design Book*, metrics (CC BY-NC-SA: ideas only) | cited |

**[authored]** What the table says in one sentence: a way is as wide as the narrowest thing on it, so a structure that takes part of a door, a stair or a road has made the way narrower than its builder declared — and every code above treats that as a defect of the structure's placement, never of the way's declaration.

## 3. What the sources do not give, and what the engine takes

- **A depth for the approach.** The codes size a landing by the stair's width or 48 inches and a door's clearance by approach direction (48–60 inches). On a 1-metre cell grid 48 inches is one cell and a fraction. **[authored]** The engine's approach is **one cell** on each side of a way — the cell a body stands in to pass — the whole-cell floor of the cited figure. It admits more than a two-cell reading would; a wider approach is a recorded next shape (spec-0120 §11), never a loosening of this one.
- **A sub-cell projection.** The codes admit a rail 4½ inches into a stair's width because a rail is 4½ inches. **[cited]** The engine models a fence, a wall or a rail post as a cell a body cannot pass (`Collision::TallBarrier`; `DW0451` records that these *fill their cell for pathing but are a narrow post or panel in reality*). **[authored]** So there is no sub-cell projection for the engine to admit: a rail inside a way's clear width takes a whole column of it, and a rail that is part of a way stands **beside** the declared width, in the next column over — the shape the gallery's and the demo's stairs take.
- **A width for a road.** The codes size corridors by occupant load, a number this engine has no use for. **[authored]** A road's width is the author's: a place that is a way declares its travel axis and its clear width on its site-plan box (spec-0120 §1.4), and its run is judged along its length as a stair's is; its mouths are doors and its fronts are contacts. Nothing is read from a box's extent or a place's label.

## 4. How a cell grid carries the rules

**[authored]**, each from the cited rule beside it.

- A way's **cross-section** is the author's declaration: a portal's `width × height`; a stair run's treads across the opening's width; a contact's span; a piece way's region (`via`/`bar`) across its travel axis. Nothing is measured to find the width, and the ledger's `width` states this declaration even where a run is counted across another axis (a flight along the wall under its opening); the bytes are measured to find what is **free** of it (§1).
- A way's **corridor** is its cross-section swept along its travel: the plane or region, every course of a stair's run, and one approach cell at each end (§3). The corridor is judged in **columns** — one per cell across the width — and a column is **clear** when a body can be in it (the nav model's one predicate), so a beam above a body's head blocks nothing and a slab a body steps onto blocks nothing (IBC §1003.6 names projections, not everything in the volume).
- A way is **whole** when every column of its corridor is clear in every state its cells can take; **narrowed** when some are and some are not. Fully shut is the gate's own state and the gate's own check (`DW0836`, `DW1011`); narrowed is this rule's (ADA 307.5, IBC §1011.6).
- The way's **own** gate region and `way` region are its shut state and are never its obstruction; a gate region of **another** way standing in this one's corridor is an obstruction in every state it takes (IBC §1003.6: a component is not interrupted by a building element other than its own).
- **No narrowing is excused** (spec-0120 §4): a structure inside a way's declared width takes a column of it, whoever meant it. A thing meant to stand beside a way — a rail, a newel, a lamp post — stands outside the declared width, in the next column over, which is where the codes put a handrail (§3); a way narrower than its declaration is corrected in the world or in the declared width, never by a document that says the narrowing is meant. The codes have no clause admitting a *fixed* obstruction in a way's clear width.

## Sources

- ADA Standards for Accessible Design (2010), chapters 3 and 4, at access-board.gov — a US federal publication, quoted.
- International Building Code 2021, chapter 10 (§1003.6, §1005.7, §1011.2, §1011.6, §1014.8), read through a public code viewer — copyrighted, paraphrased.
- *The Level Design Book*, *Metrics* (CC BY-NC-SA 4.0) — ideas only.
