# What a creator agent writes instead of the grammar

The question: when a creator agent is asked for a designed building, what does it author, and what does established practice say the authoring medium for that should be? This record answers that and stops. ADR-0030 consumes it.

## 1. Measurements

**Instrument**: a Python walk over the parsed JSON counting every object carrying an `op` key, by value; file sizes by `wc -c`. **Object**: `campaigns/doune-castle-tour/design/programs/castle.json` and the generator beside it, content repository revision `7318202`.

| measure | value |
|---|---|
| `castle.json` | 10 686 414 bytes |
| top-level `rules` / `params` | 1 rule / 0 params |
| nodes by `op` | `fill` 16 836, `void` 7 471, `split` 4 271, `mark` 31 (28 609 of 28 609 nodes carry one of these four; no `call`, no `bind`, no `repeat`) |
| generator (`build_castle.py` + `castle/*.py`) | 2 207 lines |
| other stage documents of the same campaign | `quests.json` 27 074 bytes, `dialogue.json` 25 569 bytes — no generator beside them |

**Second object, not measured here**: an unreleased campaign's work products, as its creator agent reported them — a generator of about 2 100 lines, a program of about 11 MB, produced by painting a voxel grid and serialising it as "one column per cell as a stack of spans, equal columns merged along x, equal rows merged along z". Its generator's docstrings and function list were read directly; its program was not parsed. Counts for it are the creator's, not this record's.

## 2. What the generators contain

Read from the function lists of both generators.

| what the script does | how often | what it stands for |
|---|---|---|
| paint a box, clear a box, replace inside a box | everywhere | the painter's model: later work overwrites earlier |
| `gable`, `pyramid`, `rose`, `lancet`, round-tower tests | second generator's vocabulary; the first has `in_round`, `crenel`, `stair_height` | solids that are not boxes |
| loops over a stride, left/right symmetry | every wall, arcade and parapet | `repeat` and `mirror` |
| functions with size arguments called at positions | towers, rooms, windows, furniture | a parameterised stamp |
| `settle_stairs` | once, run before every emit | a derivation the engine demands (`DW0801`) and does not perform |
| a gate *region* written into the prefab manifest after expansion | once per expansion | an anchor kind the program surface cannot declare |
| walkability search, fall-and-return search, shortcut length, reach check, slice printer | five scripts | private copies of compiler judgements, asked during authoring |

Nothing in either generator computes a value the grammar's integer expression algebra could not. The host language is used for vocabulary, stride and symmetry.

## 3. The quest stage is a different finding

The same run produced a 960-line script that writes `quests.json` and `dialogue.json`. It contains almost no loops: it is one literal with about twenty constructor functions. Each constructor removes something the stage makes a creator repeat — an effect's `happening.subject` restating the effect's own target; a three-level object to say "at this anchor"; a shop offer's price written as four conditional effects because an offer has no price. That is an ergonomics finding about the stage's surface and is specified separately; it does not bear on the medium question.

## 4. Established practice

| claim | source | strength |
|---|---|---|
| The production descendant of split grammars carries solids, booleans and occlusion queries beside `split`: `roofGable`, `roofHip`, `roofPyramid`, `roofRidge`, `roofShed`, `primitiveCube/Cylinder/Sphere/Cone/Disk/Quad`, `insert`, `extrude`, `taper`; `union`, `subtract`, `intersect`; `inside`, `overlaps`, `touches` | Esri, ArcGIS CityEngine *CGA reference* — operation index, read in full | **cited** |
| Box-split grammars were adopted here for seed-varied typologies on the premise that models are geometrically weak | spec-0027 §1, §3 | **cited (own record)** |
| The generation framework of the GDMC competition places blocks imperatively and overwriting, with a geometry module of solids | GDPC README (MIT) — `editor.placeBlock`, `geometry.placeCuboid` | **cited**; the full function list was not read (the documentation page returned 404) |
| WorldEdit's region and shape commands are the same model | — | **from memory, unverified** |
| Models writing voxel-construction code execute far more often than they are spatially right; geometric construction and multi-object composition are the hard categories | VoxelCodeBench, arXiv 2604.02580 | **abstract only** — the paper's numbers and API were not read (the PDF exceeded the fetch limit) |
| A Minecraft building can be produced by a model emitting an intermediate layer representation with a repair pass | T2BM, arXiv 2406.08751 | **abstract only** |

**Unsupported, and named as such**: that an ordered list of solids is *easier for a model to get right* than a partition. The evidence for it here is revealed preference — two independent runs chose to paint and then encode — not a controlled comparison. ADR-0030's port experiment is the comparison.

## 5. The gap this records

ADR-0018 named "creators come to need computed programs rather than written ones" as a revisit trigger and nothing watched for it: no tool reports the shape of a checked-in `Program`, so the trigger fired twice before anyone read a program's size.

## 6. Who owns the wall between a room and the outside

The question: a building of many rooms must be good outside and furnished inside, and thirty rooms must cost about thirty times one room to author, not more. Where two documents meet at a wall, which one owns it?

| claim | source | strength |
|---|---|---|
| Buildings with many occupants are delivered as a *base build* — "shell and core": the structure with column grid and clear floor heights, the envelope (external walls, glazing, roofs), the cores (stairs, lifts, risers) — and a separate *fit-out* (interior partitions, finishes, furnishing) | Designing Buildings wiki, *Shell and core*; trade definitions agree | **cited** (search summaries of the pages; the pages themselves were not read in full) |
| In the building-model standard an opening is a void of the wall: `IfcOpeningElement` is attached to its wall by `IfcRelVoidsElement`, and it "shall not participate in the containment relationship, i.e. it is not linked directly to the spatial structure"; spaces meet walls through space boundaries | buildingSMART, IFC 4.3 documentation, `IfcOpeningElement`, `IfcRelVoidsElement`, `IfcWall` | **cited** (quoted from the standard's pages via search summary) |
| The reference building is itself thick-walled: curtain "six feet thick and 39 feet high" with a parapet walk; in the gatehouse "a stair in the thickness of the walls climbs to the storeys above"; "window embrasures and mural chambers off the second-floor hall" | Canmore (Historic Environment Scotland) site record 24738; *The Castles of Scotland* | **cited** |
| Poché — the inked wall mass of Beaux-Arts plans — is treated as shaped, inhabited residue holding "hidden service spaces" | Castellanos Gómez, *Plan Poché*; Colquhoun via the same | **weak**: a definition and a publisher's summary; the argument that the thick wall reconciles interior shape with exterior form is the common reading of Venturi and is **from memory, unverified** |
| A Minecraft build team plans "the structural frame… an outline to get a general idea of the size of the build and a rough visualization of the interiors" before building, and partitions large shells into rooms afterwards | WesterosCraft wiki, *New Builders Guide* (search summary; the guideline pages returned 404 / 402) | **weak** |
| Research generators either grow rooms and derive the outside (bottom-up: a building "as a set of rooms… connected by doors or stairways and… equipped with windows") or complete an exterior over a fixed interior where "the footprint, wall geometry, and opening semantics must remain fixed" | Freiknecht & Effelsberg, *Procedural Generation of Multistory Buildings With Interior* (IEEE ToG 2020); *ShellMaker* (arXiv 2606.31680) | **abstract only**, both. What they share is the coupling object: footprint, wall geometry, openings — fixed by one side and read by the other |
| Bethesda's kit practice, this project's strongest source on parts fitting by construction, keeps interiors in separate cells behind load doors | `how-a-large-level-is-actually-built.md` §1.3 | **cited (own record)** — and therefore *not* a precedent for a continuous inside and outside; it is recorded so nobody reaches for it |

**Own record.** The released castle's gatehouse generator (content `7318202`, `castle/gatehouse.py`, `castle/common.py`): `wall_face(x, z, box, inset=2)` is "a building's wall ring"; on that ring, arrow loops are cut where `run % 6 == 2` and hall windows where `run % 5 == 1`, at each storey's sill; only a column that is not wall reaches "the interior, floor by floor", where one function per room lays that room's cells. A creator agent, unprompted, separated base build from fit-out and placed openings on the wall by a bay modulus.

**What this settles.** The coupling object between outside and inside is the wall with its openings, and every source that scales gives it one owner that is not the room. **What it does not settle** — authored in ADR-0030 §5 and tested by its port: that partitions belong to the building rather than to a storey-level fit-out; the bay module as the device that keeps openings clear of partitions (office planning grids aligned to the façade module are the practice this imitates — **from memory, unverified**); that a derived one-owner-per-cell mask is enough for round and irregular rooms; and the linear-cost claim itself, which is an argument from the absence of cross-references until the thirty-room measurement exists.
