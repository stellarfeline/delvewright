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
