# ADR-0035: A prefab is placed by a plan, never drawn by a solver — a region's piece holds library prefabs beside authored work, and a whole-region prefab is the case of one

- **Status**: Proposed
- **Source**: spec-0106 §1 (the record of two campaigns), §3.3 and §3.8; the
  research record `docs/research/how-a-large-level-is-actually-built.md` §1.3
  (modular kits), §1.4 (the whole owns space), §1.5 (procedural generation
  achieves coherence top-down or not at all), §1.6 (vanilla jigsaw and
  Minecraft build-team practice); the tree at the ground revision spec-0106
  names: `crates/delvec/src/compiler/solver.rs`, `compiler/pool.rs`,
  `compiler/massing.rs`, `compiler/seating.rs`, `compiler/faces.rs`,
  `crates/dsl/src/prefab.rs` (`Connector`), `crates/dsl/src/world.rs`
  (`Area`, `Pieces`), `crates/dsl/src/detailplan.rs` (`Detail.piece`),
  `docs/reference/grammar.md` §2 (the program surface, its weighted
  alternatives) and §5 (the rule library), `docs/reference/compiler.md`
  (*`areas[]`*, *L2 massing verbs*, *`fragment`*, *`detail-plan`*); the content
  library's `prefabs/pools.json` and the campaigns that draw from it.
- **Supersedes**: ADR-0004 in full, as a decision. Its verification record
  (the jigsaw seed-stability experiment) and its amendment (the compiler, not
  the game, places every template) stand as history and as the mechanism every
  piece is still placed by.
- **Amends**: ADR-0032 §2, whose sentence "`areas[]` remains the engine's
  assembly surface for library pools and is not a map route the skill
  presents" is replaced by §2 below: `areas[]` does not exist, and a library
  piece enters a map through its site plan or through a region's program.
- **Constrained by**: ADR-0001, ADR-0003, ADR-0006, ADR-0022, ADR-0032, the
  general-engine rule, the no-hacks rule, ADR-0013 (every prefab's provenance).

## Context

ADR-0004 decided that maps are assembled from a prefab library by jigsaw
mechanics — pieces with declared sockets, pools of weighted members, a solver
drawing from a seed — and that map quality is a curation problem. Two decisions
since have taken the assembly's work away from it. ADR-0022 placed the whole
before any part: a layout graph and a site plan state every place and every
connection before a block exists, so the topology a pool draw used to produce
is authored. ADR-0032 made one map route of it and let a piece own its outside.
What ADR-0004's mechanism still did at the ground revision was seat a piece or
a pool draw on the compiler's fixed 256-block stride through `areas[]`, which
spec-0106 removes: a region of a map — a place of the site plan built the
whole-site way — is the only placement, and a region must still be able to use
prefabs.

Two models were put to the research: a prefab is **not** a whole region (it
cannot by itself meet a region's duties to the outside — seams, ground, the
whole's bounds — so a region holds one or more prefabs plus material authored
for it), or a prefab **is** a whole region (its openings declared in advance,
seated as one region and joined to the others).

**What the research says, cited.**

- *Modular kits* (research §1.3; Bethesda's Skyrim kit practice, STRONG;
  Perry's modular design, STRONG): a kit piece is a part whose footprint is its
  full bounds, whose connection points are first-class pieces of matching size
  class, and whose layouts are authored *in* the kit's grid by a designer;
  transitions between kits are planned pieces, not discoveries; a kit is born
  paired with a pilot layout. A prefab is a part inside an authored layout, and
  the layout is a person's.
- *The whole owns space* (research §1.4; World Partition, STRONG; the Witcher
  3 and Breath of the Wild, MEDIUM/STRONG): engines keep the whole as one
  persistent artifact and place authorial chunks *by* it; the macro layout is
  authored first and renegotiated while soft. No source found a hard per-zone
  spatial refusal.
- *Generation* (research §1.5; Dormans, STRONG; Smith & Mateas, STRONG;
  Unexplored and Spelunky, MEDIUM): "no source defends assembling
  independently designed parts as a way to obtain a coherent whole. Parts-first
  appears only where a topology pass has already allocated every part its
  slot." Spelunky instantiates rooms from hand-authored templates *inside* a
  carved path; Wave Function Collapse, the strongest local-constraint
  assembler, "stays at the decoration layer". Bottom-up generators produce
  levels that "lack overall structure".
- *Minecraft* (research §1.6; minecraft.wiki, MEDIUM; Microsoft's Bedrock
  jigsaw documentation, STRONG): vanilla jigsaw assembles by recursive
  name-matched attachment inside a bounding box, degrades silently to a hole
  when no candidate fits, and has no reachability concept at any scale; the
  48-block template cap is why every large vanilla structure is a piece
  library. The best-documented build team (WesterosCraft) lays out plots by
  hand inside per-region style guides and fills them; a plot is a footprint a
  builder fills, never a draw.
- *The tree*: the engine already binds any admitted prefab as a whole place
  (`detail-plan.details[].piece`, held to the frame exactly by `DW0843` and to
  the plan's seams by `DW0844` through the piece's contract faces); Vesperhold
  is one prefab that is one whole map. A prefab *inside* a piece has one
  channel, the stage-7 `fragment` stamp, which writes over a bound frame's
  bytes with nothing comparing the two (spec-0106 §1, the campaign record's
  row on the stamp), and no channel inside a program: the grammar's operations
  split, call, bind, void, reorient, skip, mark, claim and compare, and none
  places a `.nbt`. The grammar's rules already carry weighted alternatives
  drawn from the campaign seed.
- *The goal* (spec-0106 §1): the joins that read best are the ones drawn inside
  one program; a join at a declared opening is a kit join, right for a doorway
  and never for ground.

## Decision

1. **A prefab is a part a plan or a program places; nothing draws one.** A
   library piece enters a map in exactly two ways, which are one mechanism at
   two grains: **as a whole place** — bound by `detail-plan.details[].piece`
   (or `delvec detail --piece`), its size the frame's and its contract's faces
   the plan's seams, which is the case of a region that is one prefab with its
   openings declared in advance; and **inside a place's program** — by a
   grammar operation that places the piece at the scope's box, imports its
   anchors and its contract, and leaves the expansion one piece, so every proof
   reads one piece and no second write ever reaches a bound frame. The
   evidence supports the conclusion the question allowed: a region's contents
   are prefabs plus authored work, and a whole-region prefab is the case of one
   prefab whose box is the region and whose openings answer the seams. The
   research is unanimous that the second grain is the kit's — a part inside a
   layout a person authored — and the first grain is this engine's own
   finding, verified on one campaign. What settles the two as one object is
   extraction (spec-0106 §3.9): a prefab lifted from a built campaign is a
   cuboid of designed work whose openings are the plan's seams at its
   boundary, whose ground is what the place owned and whose names are its
   stations — the declared-openings property the second model asked for,
   derived rather than typed — and whether it is then a whole region or a
   part is decided by the box it is seated in, not by the prefab.
2. **`areas[]` does not exist; the site plan is the one placement.** The
   sentence of ADR-0032 §2 that kept `areas[]` for pools is replaced by this
   one, and the skill's step 2 is unchanged.
3. **Assembly is designed, never drawn. The pool and jigsaw assembly is
   deleted entirely.** The jigsaw solver (`compiler::solver`'s layout solve
   and `seal_layout`), `compiler::pool`, `prefab_pool` and `pieces{min, max}`,
   `prefabs/pools.json`, the socket-carving verb and the jigsaw `Connector`
   vocabulary as an assembly contract, the L2 massing verbs (`swap-piece`,
   `insert-piece`, `remove-piece`, `rewire-socket`, `reseed-piece`), the
   seating tool and its gate, and the diagnostics only they raise, go. No
   seed draws a piece anywhere — not on the map and not inside a region: at
   this scale how pieces are assembled is a design, and where a prefab stands
   and what it joins is stated by the author. The grammar operation of §1
   names one piece at one scope, and which piece stands where is a function
   of the program's documents alone: an expansion in which a prefab's
   placement depends on the seed — a prefab operation reached through a
   weighted choice among alternatives — is refused by a named expansion gate.
   The campaign seed keeps every role it has that is not a prefab's placement
   (surface detail, dressing, the grammar's other weighted alternatives,
   ADR-0006).
4. **The stage-7 `fragment` stamp is removed.** A prefab inside a region is
   placed where the region is authored, by §1's grammar operation; a second
   channel that writes over a bound piece's bytes after the fact, with no
   ownership check, is the defect the campaign record names and is struck
   rather than guarded.
5. **A prefab's duties are its box's.** A piece bound as a whole place meets the
   region's duties — the seams it answers (`DW0844`), the ground it owns
   (`base: {"owned": n}`, spec-0106 §3.1) or is handed, the bounds it stands
   inside (`DW0826`) — exactly as a grammar-built piece does, because the
   gates read the piece and never its provenance (spec-0050). A prefab that
   cannot meet them is not a whole place; it is a part of one, placed by the
   program of the place that does.

## Consequences

- spec-0106 §3.3 and §3.8 are the implementation of §1–§4; its gallery
  elements and probes are the binding.
- The content library keeps every admitted piece and loses `pools.json`; the
  pool members (keep, cave, island) remain bindable as places or placeable
  inside a program. A released campaign is built by the engine it pins and is
  owed nothing (CLAUDE.md).
- `compiler::faces` judges mating by contract faces only; the `by_socket` arm
  goes with the sockets.
- Map quality is still a curation question where a library piece is used, and
  a composition question where a program is written; neither is a draw.
- CLAUDE.md's architecture line for ADR-0004 is replaced by one for this
  record, with the owner's confirmation.

## Alternatives considered

- **Keep `areas[]` for pools only** (ADR-0032 §2 as written). Declined: it
  remains a second way to place a map, which spec-0106's direction forbids,
  and the pool assembly it keeps is the bottom-up composition the research
  rejects for structure.
- **Keep a seeded draw inside a region** — a solver over a region's interior,
  or a pool as a rule with weighted prefab alternatives, for variety among
  like parts. Declined, as a ruling and on the evidence: the interior
  topology of a region is the author's program (the Vesperhold record), a
  solver drawing it is the loss of structure Dormans names, and a weighted
  prefab alternative is a pool in all but name — the seed deciding where a
  piece stands. Variety among like parts is authored: the author places the
  cottages, each by name.
- **Model 1 alone** (a prefab is never a whole region). Declined: the engine
  binds one, Vesperhold is one, and a region whose piece is one admitted
  prefab meets every duty the gates read; refusing it would refuse the one
  verified whole.
- **Model 2 alone** (a prefab is always a whole region). Declined: a kit piece
  is a part by every source, and a cottage from the library placed inside a
  region's program is the ordinary use of a library.

## Revisit triggers

- A region whose author cannot state its parts by name — a catacomb of a
  thousand like cells — with evidence that a program's own repetition
  (research §1.5's constraint-first sources; the grammar's `split` and
  `call`) cannot author it: the trigger for re-opening this record's §3, never
  for a second placement authority.
- A library piece that must be mated to another library piece at a declared
  socket inside one program, where the contract's faces are not enough to
  state the mating.
