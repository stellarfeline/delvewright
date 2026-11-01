# ADR-0037: Peak memory is bounded in map size — every whole-map stage runs over a chunked world store with a fixed working set

- **Status**: Proposed
- **Date**: 2026-11-01
- **Source**: the requirement that a larger map need no more memory and that every whole-map stage process the map in bounded chunks; the memory profile of `delvec build` and `delvec detail --all` over the town campaign (95 places, site region 588 × 176 × 830) and the gallery, taken with two instruments that share no configuration — heaptrack (allocation sites, Linux) and the process's own resident set read from `/proc` once a second beside its timestamped stderr, with `/usr/bin/time -v` and macOS `/usr/bin/time -l` as the whole-run figures; the engine tree at the revision this record lands with; CLAUDE.md "Host hardware never caps a capability", "Debug doctrine", "A craft question … is researched"; ADR-0006; spec-0113 §4.7 and §7; the sources cited in §4.
- **Supersedes**: nothing. Refines spec-0113 §7 (cost and caching of the reach graph) by naming the structures it keeps and bounding them.

## Context

### What the measurement found

A build holds the whole map in memory, several times over, and the largest holdings are proportional to the volume of the site region or to the number of cells a body can reach — never to anything a host could be asked to size in advance. Measured on the town (whole site region 86 M cells; 18.4 M placed non-air cells, 8.8 M of them inside the judged columns; 1 quest configuration; 1,366,129 rest states and 8,882,406 edges in the reach graph) and on the gallery (23 configurations, 10,441 rest states, 407,307 edges):

| Run | Peak RSS | Wall | Instrument |
|---|---|---|---|
| `delvec build`, town, under `time` | 14.78 GB | 7 min 59 s | `/usr/bin/time -v`, Linux, 16 CPU |
| `delvec build`, town, sampled | 15.09 GB (14.06 GiB) | 8 min 34 s | `/proc` sampler, Linux, 16 CPU |
| `delvec detail --all`, town | 16.13 GB (15.03 GiB) | 10 min 18 s | `/proc` sampler, Linux, 16 CPU |
| `delvec build`, the widened town: the region grown by 100 cells on every horizontal side (143 M cells, same 95 places; 3,300 written chunks, 99,338,305 ground cells) | 17.09 GB (15.92 GiB) | 10 min 26 s | `/proc` sampler, Linux, 16 CPU |
| `delvec build`, gallery | 1.34 GB | 35 s | `/usr/bin/time -l`, macOS |

Where it goes, by allocation site at the peak (heaptrack, town build; the structures named are the ones that hold the bytes, not the functions that filled them):

Peak heap under heaptrack was 9.38 GB (the traced run serialises its 16 threads' allocations and shows less than the free run's resident set; 8.98 GB of it was still allocated at exit, held by the process-wide memos). By allocation site, the largest holders at their own peaks:

| Bytes at peak | Structure | Site |
|---|---|---|
| 2.54 GB | **The node bank**: `Variant.deps` (880 MB, every window cell a node's expansion read, 12 bytes each), the variants kept at `Bank::keep` (855 MB) and the run summaries (`Expansion::ran`, 288 + 264 MB) | `nav::world::reach::bank` |
| 1.18 GB | **The assembled world**: the `BTreeMap<[i32; 3], BlockState>` of every placed cell (766 MB, `assembled::placed_blocks`) and the judged copy (403 MB, `Assembled::judged_within`) | `compiler::assembled` |
| 1.03 GB | **Clones of the closure's predecessor map** (`Reach::preds`, one `([i32; 3], Edge)` per edge, cloned on return and again by the proofs that read it) | `nav::world::reach::body_reach_at`, `nav::leave` |
| 764 MB | **Node edge tables**: one `Vec<Edge>` per expanded node (`nodes`, `flown`), kept for the process | `reach::expand` |
| 638 MB | **The flight memo**: every flight kept by its start and the cells it read, process-wide in 256 shards; it stops recording at `MEMO_ENTRIES` (2^25 entries), so its size is a constant of the engine, never of the map | `reach::flights` |
| 468 + 176 MB | Clones of the closure's successor map (`Reach::succ`) | `body_reach_at` |
| 426 + 213 MB | **The dense window snapshot**: 4 bytes of code and 2 of raw membership per window cell, in 4,096-cell chunks (33,652 chunks) | `reach::Dense`, `raw_membership` |
| 343 + 168 MB | Launch simulation scratch (`Local`, `Shifted`) | `reach` |
| 343 MB | Edge lists collected per node | `reach::expand` |
| 256 MB | A further clone of `preds` | `nav::leave` |
| 230 MB | A `BTreeMap<[i32; 3], BlockState>` built by the write judge | `compiler::attached` |

In all, the reach graph and its memos hold about 7.5 GB of the 9.4 GB; the assembled maps about 1.4 GB.

Where the resident set stands at each stage (the `/proc` sampler beside the stderr lines, town build, Linux; GiB):

| Wall | RSS | After |
|---|---|---|
| 5 s | 0.43 | documents loaded, pieces mated |
| 11 s | 1.66 | the world assembled and the judged copy taken |
| 140 s | 4.60 | the light survey (`DW0210`) — the first reach closure flown under it |
| 216 s | 7.03 | the blockout and assembly bindings |
| 391 s | 9.50 | the blockout battery and the route proofs over the reach graph |
| 449 s | 10.51 | the write judge (`DW1002`, `DW1076`) |
| 482 s | 10.84 | threshold shots |
| **511 s** | **14.06** | **the review world and frames**: 58,175,073 ground cells of the ocean's layer gathered into one list at 32 bytes each, the light arrays over the written chunks' box, every chunk's payload |
| 514 s | 11.78 | the end: what the memos still hold |

`detail --all` reaches 15.03 GiB at the same stage, carrying the 95 prepared pieces (about 1.2 GiB more at every stage) over the same build.

The stages, in pass order, and what each holds whole:

1. **Assembly** (`assembled::assemble`): every placed non-air cell in one `BTreeMap<[i32; 3], BlockState>` — 18.4 M entries for the town, the 9.6 M beyond the judged columns included. The gravity settle walks it by column. The edit replay copies it (`Arc::make_mut`) once a script exists.
2. **The judged copy** (`Assembled::judged_within`): a second map of the 8.8 M cells inside the judged columns plus the barrier wall on its faces.
3. **Occupancy → the nav world** (`assembled::Occupancy`, `nav::World::from_occupancy`): one `BTreeSet` per cell class over the whole judged world, then `CellSet`s (a bitset over the bounding box where the class fills a 64th of it, a `BTreeSet` otherwise) and a `CellMap<Material>` that stays a `BTreeMap` over every non-air cell.
4. **Light** (`compiler::light`): one byte of passability and one of light per cell of the assembled box plus one layer, twice (sky and block): 2 × 2 × 86 M for the town; re-flooded per fixture the relight loop places. The region survey (`DW0210`) reads the reach closure.
5. **The reach graph** (`nav::world::reach`): a dense snapshot of the window — the built volume grown by 8 horizontally and 6 above — at 4 bytes of code and 2 of raw membership per cell (the town's window is about 93 M cells), the envelope index per column, and then, kept for the life of the process in `static` memos: every graph's node table (`nodes`, `flown`: one `Arc<NodeEdges>` per expanded node), the node bank (per flown node, every window cell its expansion read, at 8 bytes each, plus 32 run summaries that repeat its edges), and every closure taken (`Reach`: three `BTreeMap`s over the states, with each edge once in `succ` and once more in `preds`). A closure is cloned whole on every return, and a consumer that reads `preds` clones it again.
6. **The proofs over the closure** (`nav::leave`, `nav::horizon`, `nav::seat`, `nav::lethal`, `blind`, `sculk`, `light`): each takes its own `Reach`.
7. **The write judge** (`attached::judge_writes`, `resting::judge`): the load's 220,810 writes replayed in order over a world of their own, 16,959,995 changed cells judged with their neighbours; the outline table read over 9,072,903 cells.
8. **Emission**: text; small. The review world (`view::world`) builds every chunk's NBT payload in one map before writing region files.
9. **`detail --all`**: the 95 programs expanded and held (`Prepared`: the piece's NBT and tiles) until the whole is judged and written, then the battery, which is stages 1–8 again.

What grows with what:

- With **region volume** (the site plan's `region`, which the horizon's ground and sea fill whether or not a place stands there): the assembled map (18.4 M → 19.1 M placed cells when the region grows by 100 on every side), the review world's ground cells (58.2 M → 99.3 M) and its light arrays over the written chunks (2,014 → 3,300), the write judge. Measured, that growth is 1.86 GiB of peak for 57 M more region cells, all of it at the review-world stage. The reach graph does **not** grow with it: its window is the judged world's (the boundary's playable columns grown by 16, 270 × 512 columns for the town), and the widened town has the same 1,366,129 states and 8,882,406 edges.
- With the **playable region** under an `ocean` horizon: the rest states themselves — 1,201,111 of the town's 1,366,129 states are submerged sea cells inside the judged columns, carrying 7,016,576 of its 8,882,406 edges — and with them every structure of the reach graph.
- With **reached cells**: the node tables, the bank, every closure and every clone of one.
- With **configurations**: only the differing snapshot chunks (already shared in 4,096-cell chunks, spec-0113 §7) and one set of closures per configuration — the gallery's 23 configurations cost 1.34 GB in all; the town's one configuration costs ten times that.

### What the ruling asks

Peak memory is about constant in map size: a fixed working set, with every whole-map stage reading and writing the map in bounded chunks. Only a summary that crosses chunk borders may grow with the map, and the engine states its size before the build starts. No check gets weaker to fit: the reach closure, the leak floods, the light per region and the write judge keep their exact answers (CLAUDE.md "Debug doctrine"). Output stays byte-identical to the unchunked engine (ADR-0006). The hosting side meets the stated cost; the engine never refuses a map for its size (CLAUDE.md "Host hardware never caps a capability").

### Established practice

- **A chunked world store is the game's own representation.** Minecraft keeps blocks in 16 × 16 × 16 sections, each a palette and a packed index array, inside 16 × 16-column chunks inside 32 × 32-chunk region files, and loads and lights them one chunk at a time (the Anvil format the pinned server writes; the engine's own review world already emits it, `view::world`). Cited.
- **Light is flooded per chunk and spilled across borders.** The game's light engine and its re-implementations light one chunk's sections, then propagate across shared borders; the community specification of the algorithm (dktapps, *lighting-algorithm-spec*) does it in passes — sources per chunk, a flood that "must not influence any other chunks except the source chunk", then propagation "across their shared borders" over square groups of four chunks, after which a chunk that has sat in all four positions "is then fully light-populated"; replacing a chunk invalidates it and "the surrounding 8 chunks". Starlight (PaperMC) ties light sections to the chunk objects and holds "no separate state of its own", which is what lets it run per chunk in parallel. Cited.
- **Hierarchical path-finding over a portal graph.** HPA* (Botea, Müller and Schaeffer, "Near Optimal Hierarchical Path-Finding", *Journal of Game Development* 1, 2004, 7–28) "partitions the map into clusters and precomputes optimal local paths between entrance points of adjacent clusters", then plans "an abstract path through the cluster graph" and refines it inside each cluster. The reach graph's closure is reachability, not shortest path, so the abstraction is exact here rather than near-optimal: a state in one cluster reaches a state in another exactly when a chain of intra-cluster closures and inter-cluster edges joins them. Cited.
- **Breadth-first search over a graph that does not fit.** External-memory BFS (Munagala and Ranade, SODA 1999; Mehlhorn and Meyer, "External-Memory Breadth-First Search with Sublinear I/O", ESA 2002) keeps the frontier and one level's adjacency in memory and reads the graph by blocks; Mehlhorn and Meyer group nodes into clusters stored contiguously so that a block read serves many frontier nodes at once. The chunk is that cluster here. Cited.
- **Connected components over a volume too large to hold.** Isenburg and Shewchuk ("Streaming Connected Component Computation for Trillion Voxel Images", SoCG 2009 workshop) label a 10,000³ grid "using no more than 350 MB of main memory" with a streaming union-find that keeps only the components still active at the sweep front; Andres, Koethe, Kroeger and Hamprecht (arXiv:1009.6215, 2010) label "in a block-wise fashion" and reconcile labels across block faces by union-find. The engine's floods — the sea's seepage into the built volume, a piece's exposure, the party's air — are connected components of this kind. Cited.
- **Sparse volume libraries page leaf tiles in and out.** OpenVDB served voxel leaves through delayed loading for a decade before retiring it in 13.2.0 in favour of codecs; the lesson taken is to make the paged unit the storage unit (a section) and to keep paging out of the data structure's own API. Cited; the design choice drawn from it is authored.

## Decision

### 1. One world store, chunked, with a resident budget

The assembled world is held in **one section store**: 16 × 16 × 16-cell sections, each a palette of `BlockState`s and a packed index array, addressed by section coordinate; a section nothing wrote is absent and reads as what the level generator lays there (`plan::ocean_layer_at` under an ocean, air otherwise — the `Reached` distinction `Assembled` already keeps, now a property of the store). The `BTreeMap<[i32; 3], BlockState>` world and every whole-world `BTreeSet`/`BTreeMap` of cells derived from it (stages 1–3 above) are retired; the nav codes, the light bytes and the cell classes are **computed per section from the store** when a section is loaded, by the same per-cell functions that compute them today, and cached per section.

The store holds at most **B resident sections** (the budget; the engine's default is stated in `docs/reference/compiler.md` and sized so that the working set of every stage below fits in it); a section past the budget is written to a scratch file beside the output tree and read back on demand, least recently used first. The budget is a number of sections, so the working set is the budget times the per-section cost of the stage, independent of the map. **Neither the budget nor the section size is an input to any emitted byte** — proven in §4.

The judged world is a **column mask** over the store (the boundary's playable columns grown by 16, with the barrier wall as a rule of the mask's faces), not a copy.

### 2. Every whole-map stage reads the store by chunk

For each stage: how the map is chunked, what crosses a chunk border, and what may grow with the map.

| Stage | Chunking | Crosses a border | Grows with the map (stated) |
|---|---|---|---|
| **Assembly** | A template or a fill is clipped to the sections it covers and written section by section, in plan order. Gravity settles per column, inside one section column (every y-section of one (x, z) chunk), in the order the columns are met. | Nothing: a write is per cell, a settle per column. | Nothing. The store's scratch file is the map's size on disk, never in memory. |
| **Edit replay** | A batch's writes are applied to the sections they touch; the per-batch invariant re-proofs are the chunked proofs below over the sections the batch touched and their 1-section halo. | The halo. | Nothing beyond the proofs' own summaries. |
| **Cell classes and the nav world** | Per section, from the store, on load. The `World` keeps its premises (killing volumes, furniture, gates, seats) as the small declared lists it keeps today, and asks the store for a cell's class. | Nothing. | The premises: the campaign's declared objects, not its cells. |
| **Light** | The sky term is one top-down sweep per column (column-local). The block term is a flood per section over the section's own emitters and the light arriving at its six faces; a section whose face values changed re-enters its neighbours' queue; the flood runs to the fixpoint. Then every section is flooded once more with the fixed face values. The field is the pointwise maximum over every seed of `seed − distance`, so the fixpoint is the whole-world field whatever the section order (the same fact `compiler::light` already rests its determinism on). The relight loop's one ordered decision — the darkest deficient cell, ties by `(y, z, x)` — is a sort over the gathered deficient set, which is a list of dark cells, not a field. | The six face planes of a section: 6 × 256 light values, the light spill. | The deficient set: one entry per dark reachable cell, bounded by the dark count the diagnostic reports, never by the map. The per-region verdict (`DW0210`) is exact: it reads the fixpoint. |
| **The reach graph** | Partition the window into **reach chunks**: full-height columns S × S cells in (x, z), S a multiple of 16 and at least `WINDOW_MARGIN` (8), so that every launch out of a node, every cell `rest_kind` asks about where it lands, and every fall column lies inside the node's chunk or its eight neighbours. Expansion runs chunk by chunk with the 3 × 3 ring of snapshot sections resident; a node's edges whose target lies in another chunk are **portal edges**, written to the target chunk's inbound list. The envelope index (`Columns`) is per chunk. The closure from the roots is an external-memory BFS over chunks: a frontier per chunk; load a chunk, expand the nodes the frontier reached (lazily, as `body_reach_at` does today — a node flies only the launches that can reach an unreached rest cell), take its internal closure, push each crossing onto the neighbour's frontier, release the chunk; stop when every frontier is empty. The reached set is **one bit per window cell**; the edge lists live in the chunk's file and are read back when a proof asks for a node's successors or predecessors. | The portal edges at chunk faces; the frontier. | The reached bitmap: window cells / 8 bytes (11.6 MB for the town). The portal edges: one record per edge that crosses a chunk face, bounded by the states on chunk faces times the launch set — stated per build as "N portal edge(s) over F face cell(s)". The bank, the node tables and the closures of a chunk live with the chunk; a build that persists the bank (`DELVEC_REACH_CACHE`) persists it per chunk. |
| **The sea** | A water cell whose six neighbours are water, with no collision top in the 3 × 3 × 2 neighbourhood that would give it a climb-out and no premise cell within a launch of it, has exactly the six dive edges and nothing else (`reach::expand`). Such **plain-sea** cells are not stored as nodes: each connected component of plain sea inside the window is one summary — its member count, its surface (afloat) cells, and the non-plain water cells on its boundary, which are its edges — computed by the same block-wise connected-components pass as the leak floods. A body that reaches any cell of the component reaches every cell of it, and leaves it only through its boundary. This is exact for every consumer because every edge out of a plain-sea cell is a dive to a plain-sea or boundary cell, and the closure of the component is the component. | The component's boundary cells. | The summaries: one per component, with its boundary. The binding line keeps counting every state, summarised or not, so `DW0921` and the reach binding read the same numbers. |
| **The leak floods** (`DW0851` sea seepage, piece exposure, party air, `DW0318`) | Block-wise connected components: label per section, record the labels on each face, union across faces, relabel on the second pass (Andres et al.; Isenburg and Shewchuk). | The face labels. | The union-find over face labels: one entry per distinct label on a section face, stated as "L face label(s)". |
| **The write judge** (`DW1002`, `DW1076`) | Replay the load's writes in order over a copy-on-write view of the store: a write touches the sections it covers and their 1-cell halo; a changed cell is judged with its neighbours as it lands; a section no later write touches is released. The runtime arm judges each configuration's writes the same way. The outline check is per cell with a 1-cell halo. | The halo. | Nothing: each write's working set is its own sections. |
| **Emission** | Already streamed: a function is written when complete. | — | — |
| **The review world** (`view::world`) | Region file by region file (32 × 32 chunks): build one region's chunk payloads, write, release. The light it carries comes from the light stage's store, not a second flood. | Nothing. | No more than one region file's payloads at a time. |
| **`detail --all`** | Each place's program is expanded, judged against the whole and written in place order as today; the pieces held until the one judgement are the campaign's own `.nbt` bytes, whose size is stated with the run. The battery is the chunked build. | — | The prepared pieces: the sum of the pieces' bytes, a property of the campaign, stated. |

### 3. The engine states a build's cost before it starts

Before stage 1, `delvec build` and `delvec detail` print one line, `resource cost:`, computed from the documents alone — the site plan's region, the horizon, the configurations the quest plan enumerates, and the budget — stating the working set (budget × the largest per-section cost of any stage), each summary's bound (reached bitmap: window cells / 8; portal edges and face labels: an upper bound from the window's chunk-face area and the launch set; the plain-sea summaries: at most one per water body of the horizon), and the scratch file's size (placed sections × section bytes). The line is part of the build's binding output and is written into `validation/resource-cost.json` beside the actual peak the process measured at exit (`frameset::peak_rss_bytes`), so a stated bound that was passed is a red the test suite can see. The hosting side meets the stated cost; the engine does not refuse a map for it.

### 4. Determinism, and how it is proven

- No output byte depends on the section size, the reach-chunk size or the resident budget: a test builds the gallery at the smallest budget the engine accepts and at an unbounded one, and at two reach-chunk sizes, and asserts the output trees' hashes equal (the shape of the existing `--perturb` derivation check: vary what must not matter, assert nothing moved).
- The chunked reach graph agrees with the whole-window one on generated worlds: the agreement test `reach/agreement.rs` already asks the same cases of several consumers; it gains the chunked expansion as one more consumer, red on any difference in states, edges or binding counts.
- The chunked light field agrees with the whole-box flood on generated boxes, cell for cell, over several section orders.
- The double-build hash comparison of ADR-0006 stays as it is, and the town is built once on the unchunked engine and once on the chunked one with the output trees compared byte for byte before the first chunked stage merges; the comparison is recorded in the commit body that lands it.

### 5. Implementation order

By memory saved per unit of work, from the profile:

1. **The reach graph's process-wide memos** — the bank (2.5 GB), the node tables (1.1 GB), the closures and their clones (1.9 GB), the flight memo (0.6 GB), the snapshot (0.6 GB): about 7.5 GB of the town's heap, held to exit. First the bank, spilled to the scratch file (one slot per variant in memory): landed with this record and measured below. Then the closures: `body_reach_at` returns the memo's shared `Arc<Reach>` instead of a clone, a reader that grows a closure copies it only when it has a launch to fly (`Arc::make_mut`), the leave proof reads `preds` in place instead of cloning it, and a kept closure holds its edge lists at their length: landed and measured below. Then the closures' edge lists and the node tables kept by reach chunk in a scratch file under a resident budget, and the closure taken as the breadth-first search over reach chunks of §2, with the proofs that walk a closure reading it by chunk and the budget the working set a walk by chunk needs: landed and measured below. Still to come in this item: the window-wide snapshot paged by chunk with its 3 × 3 ring loaded per chunk, the node tables and the bank kept and persisted per chunk, the reached states held as the bitmap and per-chunk kinds rather than a map, and the portal-edge line. The flight memo is already bounded by a constant (`MEMO_ENTRIES`) and leaves this list.
2. **The review world** (the build's peak increment, about 3.3 GiB over the plateau): the ground cells walked rather than gathered (landed with this record), then region-by-region writing with the light taken per section.
3. **The assembled world and its judged copy** (1.4 GB, and the write judge's own map): the section store of §1, with the judged world as a mask.
4. **Light**: per-section flooding over the store; the arrays are a smaller holder today (a few hundred MB) and fall with the store.
5. **The sea summaries and the leak floods**: the plain-sea components, which take 1.2 M of the town's 1.37 M states out of the graph, and the block-wise floods.
6. **`detail --all`'s prepared pieces**: stated, not chunked; they are the campaign's own size.

## The first stage, demonstrated

The largest consumer is the reach graph's retained memos, and the largest structure among them is the node bank. Landed with this record: **the bank spills its variants to an unlinked scratch file and keeps one slot per variant in memory** (`reach/bank.rs`; the town keeps 166,307 variants over 2,053,002,194 bytes of spill). Measured with the `/proc` sampler on the same Linux machine, base engine against the engine with the spill alone:

| Map | Base peak RSS | Spilled bank | Saved | Output |
|---|---|---|---|---|
| town (86 M-cell region) | 14.06 GiB (15.09 GB) | 10.91 GiB (11.71 GB) | 3.15 GiB | byte-identical over all 267 files (sha256 of each) |
| widened town (143 M-cell region) | 15.92 GiB (17.09 GB) | 12.92 GiB (13.87 GB) | 3.00 GiB | byte-identical over all 270 files |
| gallery (macOS, `time -l`) | 1.34 GB | 1.36 GB | none: its bank is 80 MB and its peak lies elsewhere | byte-identical over all 1,570 files |

The saving is the bank's heap plus the allocator's slack over it, and it holds at both sizes because the bank's size is the town's reached nodes, which the region does not change. The peak stays at the review-world stage, so the second landed step is there: **the review world walks its ground cells instead of gathering them** (`view/world.rs`), measured the same way with both steps in:

| Map | Base peak RSS | Both steps | Saved | Output |
|---|---|---|---|---|
| town | 14.06 GiB (15.09 GB) | 9.45 GiB (10.14 GB) | 4.61 GiB (33 %) | byte-identical over all 267 files |
| widened town | 15.92 GiB (17.09 GB) | 9.96 GiB (10.69 GB) | 5.96 GiB (37 %) | byte-identical over all 270 files |
| town, `detail --all` | 15.03 GiB (16.13 GB) | 9.63 GiB (10.34 GB) | 5.40 GiB (36 %) | the content tree unchanged by the run, as before |

Growing the region by 57 M cells now costs 0.51 GiB of peak instead of 1.86 GiB: what is left of that growth is the assembled map's extra placed cells and the review world's light arrays over 3,300 chunks rather than 2,014, which the store of §1 and the region-by-region writer of §2 take in turn. The peak still falls at the review-world stage (9.45 GiB there over a 7.94 GiB plateau on the town), and the plateau is the reach graph's retained memos, the next thing in the order of §5. Wall clock on the shared machine moved between runs by as much as a minute and a half in either direction while another round's jobs ran beside these, so no timing is claimed here beyond the fact that the spill writes 2 GB once and the ground is walked three times instead of listed once.

## The closures, measured

Measured on the remote Linux machine (16 CPU) with two instruments that share no configuration: GNU `time -v` (the kernel's maximum resident set from `wait4`, and elapsed time) wrapping the `/proc` sampler (VmRSS once a second, stderr timestamped). The two agree on the peak to within 25 MiB on every run. The tree is the branch with `main` merged in. Wall time on the shared machine moved by up to three minutes between runs of one binary while another round's jobs ran, so wall time is recorded and no timing is claimed from it.

| Engine | Town peak, run 1 / run 2 | Town wall (time -v) | Widened-town peak | Output |
|---|---|---|---|---|
| `main`, without this record's steps | 14.12 GiB | 7 min 56 s | 15.89 GiB | — |
| the bank spill and the walked ground, merged with `main` | 9.31 / 9.33 GiB | 7 min 9 s / 9 min 12 s | 10.15 GiB | byte-identical to `main` over all 267 (town) and 270 (widened) files |
| closures shared, the kept closure the one the rounds grew | 9.58 / 9.54 GiB | 10 min 50 s / 9 min 17 s | 10.16 GiB | byte-identical |
| the same, its lists shrunk in place | 9.42 / 9.43 GiB | 9 min 1 s / 9 min 21 s | 10.20 GiB | byte-identical |
| closures shared, the memo keeping a one-sweep copy (landed) | 9.32 / 9.31 GiB | 8 min 36 s / 8 min 37 s | 9.94 GiB | byte-identical; the two town runs identical to each other |

Sharing the closures saves nothing measurable at the peak or on the plateau: the copies callers took were short-lived, and the allocator reused their memory. The per-site figures of the profile above (1.03 GB of `preds` clones, 0.64 GB of `succ` clones) are each site's own peak, not memory held at the same moment. Keeping the closure the rounds grew instead of a one-sweep copy raised the peak by about 0.25 GiB; keeping a one-sweep copy returns it to the merged figure. The remaining reach-graph memory is in the bank's slots, the node tables, the snapshot and the closures themselves, which only the per-chunk graph of §2 bounds.


## The closure by chunk, measured

The closure is taken as the breadth-first search over reach chunks of §2 (`World::close`, `World::fly_chunks`): the cell relation is closed with a frontier per chunk, each crossing pushed onto the frontier of the chunk it lands in; the launches are flown in rounds, every standing or holding state whose envelope still holds an unreached cell flying toward it, each judged against the closure as the round found it, grouped by chunk and joined while that chunk's lists are the ones read. The snapshot is still the window-wide one, so loading a chunk's 3 × 3 ring is loading nothing; the chunk is the unit of list traffic and of the visit count. The proofs whose answer is a set read a closure by chunk (`Reach::reaching`, `Reach::preds_by_chunk`, the learned route tree as a per-chunk least-cost search and one predecessor sweep, the blinding flood's layers); `DW0921`'s way in and the mob reach's breadth-first path keep their own order, which is their answer. The lists' resident budget is the working set a walk by chunk needs: 9 × the largest decoded chunk plus 9 × 2 × 64 KiB of buffered pushes (46.6 MiB on the town, its largest decoded chunk 5,295,392 bytes).

**The one change §4 allows, declared.** Before, a round's nodes were flown in batches of 2,048 in cell order, each batch re-judged against what the batches before it had grown, so which launches were flown — and with them the edges joined, the launches simulated and the arcs credited — depended on that global order. Every launch is now judged against the closure at the start of its round. The reached states are the same set (`reach::agreement::the_chunked_closure_is_the_one_every_launch_gives`: the closure equals flying every launch of every node, on 15 worlds at two sides); the binding line and `validation/reach-graph.json` now count the launches this definition flies. On the town: 1,366,129 states either way; 9,024,330 edges against 8,882,406 (walk +4,403, fall +30,323, jump +107,198; dive, climb and exit unchanged); 130,843,256 launches simulated against 121,972,219 (+7.3 %); 86,855 arcs credited against 86,799. A predecessor list reads in cell order of the predecessor, since edges into one cell are joined from many chunks. No player-facing byte moved: of the 267 town files and 270 widened-town files, only `validation/reach-graph.json` and the manifest line that hashes it differ from `main`, and the datapack, creator datapack, PackTest datapack and server directories hash alike (a build emits no world or resource pack of its own).

Measured on the remote Linux machine (16 CPU) with GNU `time -v` (the kernel's maximum resident set and elapsed time) wrapping the `/proc` sampler (VmRSS and `/proc/<pid>/io` once a second); the two agree on every peak below. Each engine force-rebuilt from its revision with cargo's status asserted.

| Engine | Town peak | Town wall | Widened-town peak | Widened-town wall |
|---|---|---|---|---|
| `main` fa77f6431 | 14.09 GiB | 7 min 5 s | 15.91 GiB | 7 min 21 s |
| the landed closures, 70e1b4744 | 9.41 GiB | 7 min 3 s | 10.01 GiB | 7 min 27 s |
| lists by chunk, closure whole-window, c5c7a1246 (from the branch's earlier round) | 8.07 GiB | 11 min 21 s | — | — |
| the closure by chunk, readers not yet by chunk, 256 MiB budget | 8.02 GiB | 8 min 49 s | 8.75 GiB | 9 min 8 s |
| and the set readers by chunk, 256 MiB budget | 7.98 GiB | 6 min 18 s | — | — |
| and the working-set budget, route tree not yet by chunk | 7.88 GiB | 6 min 54 s | — | — |
| **and the route tree by chunk (landed), run 1 / run 2** | **7.88 / 7.99 GiB** | **5 min 59 s / 6 min 1 s** | **8.49 / 8.50 GiB** | **6 min 22 s / 6 min 23 s** |

The two landed runs of each map are byte-identical to each other over every file. Growing the region by 57 M cells costs 0.51–0.61 GiB of peak, the same as before: none of it is the reach graph's.

The list traffic is what the readers' order cost: with the closure by chunk but the readers in their own order, 68,747 chunks were decoded back over 3,822,599,943 records (148 GB read through system calls, served from the page cache); with the set readers by chunk, 5,624 under the 256 MiB budget; under the working-set budget the learned route tree's whole-closure search alone caused 159 of 181 sampled decodes (one in 128 traced) and the total rose to 23,101; with the route tree by chunk, 4,662 over 204,757,519 records under the working-set budget. Disk traffic of a landed town build: 4.26 GB written (the lists' scratch file 2.14 GB, the bank's 2.13 GB), 6.86 GB read through system calls, under 0.1 MB of it from the device.

**How often a chunk is taken up.** The town's closures visit 547 (closure, chunk) pairs 2,643 times — a visit is one frontier closed or one round's flights in that chunk — at most 21 times for one chunk in one closure: 79 pairs once, 134 twice, 42 three times, 74 four, 35 five, 33 six, 32 seven, 29 eight, 25 nine, 20 ten, 17 eleven, 6 twelve, 3 each at 13 and 14, 1 at 16, 3 at 17, 5 at 18, and 3 each at 20 and 21. The widened town visits the same, since its window is the same. A chunk is taken up again when a later round's landings or a neighbour's crossings reach it; the count is bounded by rounds × the crossings that re-enter it, which no rule here bounds, and it is printed on every run.

## Consequences

- A build's resident memory is the budget plus the stated summaries; a map twice the size takes twice the disk under the scratch file and the same memory. The hosting side reads the `resource cost:` line.
- Every whole-world `BTreeMap<[i32; 3], _>` and `BTreeSet<[i32; 3]>` in the compiler becomes a per-section array or a declared list; a new pass that wants the whole world asks the store for it section by section, and a pass that collects cells into a whole-world set is the shape to refuse in review.
- The reach graph's edge lists leave process memory: a proof that walks `preds` or `succ` reads them from the chunk's file through the same `Reach` API; the API keeps its shape and loses its `Clone`.
- The node bank is per chunk, persisted per chunk; a run that sets `DELVEC_REACH_CACHE` keeps the same hit rule (every read cell equal) at a chunk's granularity.
- `spec-0113` §7's baseline table gains the memory column and the chunk-size invariance criterion.
- The scratch file is written under the output directory (`-o`) and removed at exit; a `--perturb` build, which has no output directory, uses the system temporary directory for it and nothing else.

## Revisit triggers

- A stage whose exact answer cannot be reached by a bounded-halo rule or a fixpoint across borders: it is named here before it is built, with the quantifier it would weaken.
- A map whose stated summaries exceed its working set: the summary is then the structure to chunk next, and this record is superseded with the measurement.
