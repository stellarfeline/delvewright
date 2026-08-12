# spec-PENDING: The spatial contract — spaces, edges, levels, closure, and coverage

- **Status**: Proposed (ADR-PENDING-map-design-pipeline; owner ruling
  2026-08-12; trial-0001 both runs are the motivating red; amended against the
  `tools/spike-spatial-contract` prototype, whose `run-evidence.sh` is the
  fixture seed)
- **ADRs**: PENDING (decision), 0004 (extended), 0006 (determinism), 0018 §7
  (the `Program` version fence this rides)
- **Non-goals**: parameterised / cross-program `call` (task #107, own spec);
  local-direction `mark` facing (`grammar.md` §7, own spec); overlay; the
  positional index; sightline/density/craft claims; campaign-side reachability
  diagnostics over unlock logic (follow-up fence); jigsaw connector emission
  (unblocked by §3, delivered separately); any geometry **generation** from the
  contract.

## 1. Surface

### 1a. Program IR (fenced at the next `Program` version)

- `contract` block on `Program`: `{ entry: <space>, edges: [Edge],
  no_body_majority_ack?: <string> }`.
- Declaration node `{ "op": "space", "space": <kebab>, "envelope":
  "enclosed" | "open_top" | "open", "body": … }` — wraps a body like `mark`,
  writes no blocks, claims **the scope's box** (never literal coordinates; a
  parametric program resolves its contract per expansion exactly as `mark`
  resolves anchors). **Multiple declarations claiming one name union**: a
  space is the union of its claimed boxes, and its envelope is stated once
  (a second differing envelope for the same name is refused). This is what
  lets a stepped cross-section — the cathedral nave, the ordinary case per
  the prototype — be one space rather than a forced split whose upper box
  has no standable cells and can never go green.
- `{ "op": "no-body", "region": <kebab>, "kind": "sealed" | "open",
  "reason": <non-empty string>, "body": … }` — standable-but-out-of-play
  cells. `kind` and `reason` are **required per region**: `sealed` claims the
  cells are unreachable (checked, §2.6); `open` claims exterior decoration
  open to sky (checked, §2.6). A `no_body` region **may nest inside a
  space** (the one licensed overlap; `rafter_hall`'s intentionally
  unreachable perches are the proving case). Two different *spaces* may
  abut but never overlap.
- `Edge`: `{ a: <space | "exterior">, b: <space | "exterior">, class:
  "walk" | "stair" | "drop" | "barred" | "vision", rise?: <i64>,
  via?: <box>, bar?: { region: <scope-declared>, block: <role> } }`.
  - `rise` is the **declared level relation**, checked as
    `min_y(b) − min_y(a)` over the resolved boxes: optional on `walk`
    (default 0 — "these two rooms meet on one surface", the seam claim the
    bell zones are built on), **required** on `stair` (≥ 1) and `drop`
    (≤ −1), meaningless and refused on `vision`. This field is what makes
    the bell drift family expressible: the prototype showed a one-course
    seam error green on every topology obligation — `connected` steps ±1 —
    and red on nothing until the level relation existed.
  - `drop` is directed a→b. `bar` is required exactly on `barred`.
  - **`exterior` is a face, not a node**: an exterior edge declares the
    piece's face contract (§2.8) and contributes no connectivity — the
    reachability walk never routes through it. (Prototype: exterior-as-node
    made any two exterior-doored spaces mutually reachable; deleting Z7's
    stair edge stayed green; `barred` gating was defeated.)
- Collected into `Expansion::spaces` / `Expansion::no_body` /
  `Expansion::edges` (BTreeMaps), never into the `VoxelModel`.

### 1b. Prefab metadata

`spatial_contract` block in the metadata JSON: always the **resolved**
contract of the expansion that produced the bytes (literal boxes are correct
there because the bytes are frozen with them; a re-parameterised program is a
*different expansion* and export writes its own resolved contract). Hand-built
and ingested pieces get the same block via `delve-admit space` /
`delve-admit no-body` / `delve-admit edge` (owns its block, leaves the rest
untouched, like `socket`); their bytes never re-parameterise, so literal boxes
are the natural form there. Every declared anchor gains a `resolves_to` field
(§2.7). Absent block = legacy metadata, exactly as `lighting` distinguishes
absent from `unmeasured`.

### 1c. Checker

One checker over (block grid, resolved contract), callable from
`delve-grammar expand` (on the expansion, before export — a red writes no
`.nbt`) and from `delve-admit audit` (on delivered `.nbt` bytes + the resolved
contract in metadata). Same bytes + same resolved contract → same verdict,
whichever door (AC6). Both invocations are bound to the events they guard;
there is no separately-run gate.

## 2. Obligations (each a gate with a verdict and a binding count)

1. **well-formed**: `entry` names a declared space; every edge endpoint is a
   declared space or `exterior`; `bar.region` lies on the shared boundary of
   its edge's endpoints; `rise` present/absent per class as §1a; two spaces
   overlap nowhere; `no_body` overlaps nothing except a space it nests
   wholly inside. Validate-time where the inputs allow, expansion-time
   otherwise.
2. **coverage**: every standable cell lies in ≥ 1 declared space or `no_body`
   region. Binding: standable count, covered count. Uncovered > 0 → red,
   cells listed.
3. **closure**: for every `enclosed` space (and the side faces of
   `open_top`), every boundary cell is non-passable, except cells inside a
   declared edge's opening (`via` when given, else the discovered openings on
   the shared face), faces shared with an abutting declared space, and faces
   shared with an abutting `no_body` region. Binding: boundary cells
   examined. Any unexplained passable boundary cell → red, cell and writing
   rule named (a cell written by a weighted mix says so — erosion into an
   enclosed shell is a conflict of declared intents and reds loudly).
   **Named residual**: the `no_body` exemption means a sub-body-height visual
   breach into an `open` region (a daylight slot too small to walk) is not
   machine-caught here — a *walkable* breach is caught by §2.6's
   unreachability proof instead. The residual belongs to render review; if it
   recurs as an owner finding it gets a machine form then (ADR revisit
   trigger).
4. **edge proof**, per class, over the union of the two endpoint spaces plus
   the opening, using the existing predicates (`nav::connected`,
   `nav::reachable_with_fall`) — **and in every class the declared `rise`
   equals the measured `min_y(b) − min_y(a)`**:
   - `walk`: connected both ways; rise as declared (default 0).
   - `stair`: connected both ways; rise as declared, ≥ 1.
   - `drop` (a→b): reachable under walk-and-fall; **not** reachable b→a
     under the plain step; rise as declared, ≤ −1.
   - `barred`: **not** connected while `bar.region` stands; connected
     through exactly that opening with the region voided (a pure re-check on
     a copy; deterministic).
   - `vision`: no traversal claim; exempts its opening from closure; no rise.
5. **reachability — per cell, not per space**: every standable cell of every
   declared space, minus cells inside nested `no_body` regions, is reached by
   the voxel walk from `entry` with `barred` bars standing and drops
   available forward only. Binding: cells examined, cells reached. Unreached
   > 0 → red, counted per space. Per-space graph reachability is one line
   from vacuous ("declare it all one space"); per-cell is what run 1's
   honest contract actually reds on — coverage green, 
   reachability red with the stranded gallery counted. A space unreachable
   only while bars stand is re-walked with named bar sets opened and the
   required set printed per space (the campaign-side unlock check consumes
   this later); unreachable under every opening → red.
6. **the `no_body` obligation** — what separates "aisle roof" from "sealed
   belfry", and what closes the escape hatch the prototype found (an
   all-`no_body` contract passed the unamended obligations on a broken
   artifact in 26 lines):
   - `sealed`: every standable cell of the region is **not** reached by the
     §2.5 walk, even with every bar opened. A reachable cell in a `sealed`
     region → red (it is play space wearing a `no_body` label — declare it a
     space or wall it off).
   - `open`: every cell of the region is open to sky (no solid above within
     the artifact). A roofed-over `open` region → red.
   Binding: regions, cells per kind.
7. **anchors**: every declared anchor resolves to a contract element — the
   closed extent (interior + boundary) of a covered space, a declared edge's
   opening or bar region, or a `no_body` region (the latter printed as a
   finding). Binding: anchor count, by element kind. Closed extent, not
   interior: a block-addressing anchor is a boundary fact —
   `boulder_stair`'s `volley-slot` is a ceiling cell of the run's space, and
   `far_side_bar`'s `gate` sits inside its own bar region — both first-party
   cases must resolve without special cases (AC9).
8. **exterior faces**: edges naming `exterior` are exported as the piece's
   face contract; `--traversable`'s claim is re-derived from them
   (entry-class exterior edge ↔ exit-class exterior edge), retiring the
   standable-face approach-count heuristic (task #108's miscount).
9. **vacuity reds** (amended: a finding that does not gate is a finding
   nobody reads — prototype evidence): a **zero binding** on closure, edge
   proof, or reachability is **red**, not a finding. A `no_body` majority
   (its standable share exceeding the spaces') is **red** unless the
   contract carries `no_body_majority_ack` — and the per-region `reason`
   fields are already mandatory, so acknowledging costs writing something a
   reviewer will read. `1 space, 0 edges` remains a printed finding (a
   genuine one-room piece exists; its closure and reachability bindings are
   non-zero or it reds anyway).

## 3. Determinism and transparency

- Contract data serialises canonically; the double-expand suite extends over
  `spaces`/`no_body`/`edges` exactly as it covers anchors.
- Wrapper transparency: inserting `space` / `no-body` / edge declarations into
  a program moves no block bytes — asserted the way `mark`'s transparency is.
- Checker verdicts are pure over (grid, resolved contract): same inputs, same
  report bytes.

## 4. Order of work

1. **Prototype — done** (`tools/spike-spatial-contract/`,
   `feat/spatial-contract-prototype`). What it established is folded into §1
   and §2 above: cost bounded (cathedral 28 spaces / 21 edges / 84 lines /
   ~45 min; Z7 8 / 9 / 29 / ~15 min, green); the level relation is
   load-bearing (three of Z7's four drifts reach geometry and red **only**
   through `rise`; the fourth is refused upstream and produces no bytes —
   deliberately out of the checker's reach, see AC5); `no_body` is the escape
   hatch; per-space reachability and exterior-as-node are unsound. Its
   contracts and artifacts become the in-tree fixtures.
2. IR surface + checker in-engine, fenced; export + `delve-admit` halves.
3. Docs and skill in the same PR (`grammar.md`, `prefab-procedure.md`
   §1/§3/§4, `tools.md`; the `/new-delve` workflow gains
   contract-before-rules as a mandatory step).
4. Bell adoption round (same milestone): contracts for the eight zone
   programs, translated from `tests/zones.rs`'s topology **and level**
   assertions; the Rust suite keeps sightline/tell/density claims and gains
   "the checker catches the same drifts" fixtures.
5. Trial 2 (owner's Stormveil-class brief) runs against the reader-facing
   docs only, as trial-0001 did.

## 5. Acceptance criteria

1. A `Program` at the new version with no `contract` block: refused, naming
   the field (fenced obligation). At the old version: compiles byte-identically
   to today (fence test both directions, red-demo per the fence rules).
2. The run-1 artifact under its honest contract (ported from the prototype):
   **coverage green, per-cell reachability red**, and the unreached-cell
   count equals an independent probe's stranded count restricted to declared
   spaces — the assertion is the agreement of two implementations, not a
   pinned number. Closure red on the wall-less transept flanks with the
   cells listed. The corrected twin of each distilled defect fixture
   (stranded storey, boundary slot into a `sealed` region, seam overshoot)
   is green.
3. Each edge class has a passing fixture and a teeth fixture: `drop`'s teeth
   is a rescue ladder (reuses `drop_shaft`'s knob), `barred`'s is `unbarred`,
   `stair`'s is `broken_step`, `walk`'s is a sealed doorway — plus a **rise
   teeth** fixture per traversal class: geometry one course off the declared
   `rise` reds naming both numbers, on an artifact where every topology
   obligation is green (the prototype's Z7 seam case, pinned).
4. Closure's mix interaction is pinned both ways: an `enclosed` space whose
   shell role carries weighted air reds closure at a seed that voids a shell
   cell, and the identical program with the space declared `open` is green.
5. Bell Z7's contract passes all obligations at the fixture region/seed.
   Of its four recorded drifts, the **three that build** red through the
   contract checker (via `rise`); the fourth is asserted to **refuse before
   any bytes exist** — a refusal is the stronger channel, and the checker's
   silence over a non-artifact is correct, not a gap. The assertion pins
   both halves so a later change that lets it build cannot pass unnoticed.
6. `delve-admit audit` on a `.nbt` whose metadata carries a resolved
   contract runs the same checker and agrees with `expand`'s verdict for the
   same bytes and same resolved contract (one checker, two doors). A
   re-parameterised expansion is a different resolved contract by
   construction (§1b) and is asserted to carry different boxes, not to
   reuse stale ones.
7. Double-expand determinism holds over `spaces`/`no_body`/`edges`; wrapper
   transparency holds (block bytes unmoved by declaration insertion).
8. **The escape hatch is shut and stays shut**: the prototype's 26-line
   all-`no_body` contract for the broken artifact — verbatim, as a fixture —
   now exits non-zero (red on §2.6 `sealed` reachable-cells or §2.9
   majority), and the same contract with `no_body_majority_ack` added still
   reds on §2.6. A vacuous-binding fixture (all-`open` envelopes, zero
   closure binding) reds on §2.9.
9. First-party anchor resolution: `far_side_bar`'s `gate` (in its bar
   region), `boulder_stair`'s `volley-slot` (boundary cell), and
   `rafter_hall`'s perches (nested `no_body`, finding printed) all resolve
   under §2.7 with no per-rule exception, asserted over the library.
10. Non-box spaces: the run-1 nave declared as one union-of-boxes space is
    green on well-formed/coverage/closure where the prototype's forced
    two-box split produced a phantom forever-red space; the phantom
    decomposition is kept as the red fixture for the union rule's absence.
11. Exterior semantics: deleting Z7's stair edge from its contract reds
    reachability (the prototype's exterior-as-node counterexample, inverted
    into teeth); an exterior edge never appears in any reachability
    explanation the report prints.
12. Anchors round-trip with their `resolves_to` field through
    `PrefabRegistry` (`crates/compiler/tests/grammar_prefab.rs` extended).
13. Trial 2 is run and recorded in `docs/trials/` with the same
    written-before rubric discipline as trial-0001, judged on: no structural
    defect of the verified classes ships green (checked by an independent
    probe, not by the gates under test); the breach-walkway and the one-way
    drop are stated in edge classes without falsification; and the agent's
    account does not name the contract as the reason a stated concept was
    cut. **Any of those three failing is this spec's design failing**, and
    the record must say which.
