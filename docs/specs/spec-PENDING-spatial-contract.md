# spec-PENDING: The spatial contract — spaces, edges, closure, and coverage

- **Status**: Proposed (ADR-PENDING-map-design-pipeline; owner ruling
  2026-08-12; trial-0001 both runs are the motivating red)
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

- `contract` block on `Program`: `{ entry: <space>, edges: [Edge] }`.
- Declaration node `{ "op": "space", "space": <kebab>, "envelope":
  "enclosed" | "open_top" | "open", "body": … }` — wraps a body like `mark`,
  writes no blocks, claims the scope's box. `index: unique | auto` as `mark`.
  `{ "op": "no-body", "body": … }` claims the scope's box as deliberately
  standable-but-unowned (exterior cornices, decorative ledges).
- `Edge`: `{ a: <space | "exterior">, b: <space | "exterior">, class:
  "walk" | "stair" | "drop" | "barred" | "vision", via?: <box>,
  bar?: { region: <declared by a scope, same mechanism as space>, block: <role> } }`.
  `drop` is directed a→b. `bar` is required exactly on `barred`.
- Collected into `Expansion::spaces` / `Expansion::edges` (BTreeMaps), never
  into the `VoxelModel`.

### 1b. Prefab metadata

`spatial_contract` block in the metadata JSON, written by export from the
expansion, or by `delve-admit space` / `delve-admit edge` for hand-built and
ingested pieces (owns its block, leaves the rest untouched, like `socket`).
Every declared anchor gains a `space` field (its containing space, or absent
for a piece with no contract). Absent block = legacy metadata, exactly as
`lighting` distinguishes absent from `unmeasured`.

### 1c. Checker

One checker over (block grid, contract), callable from `delve-grammar expand`
(on the expansion, before export — a red writes no `.nbt`) and from
`delve-admit audit` (on delivered `.nbt` bytes + metadata). Both invocations
are bound to the events they guard; there is no separately-run gate.

## 2. Obligations (each a gate with a verdict and a binding count)

1. **well-formed**: `entry` names a declared space; every edge endpoint is a
   declared space or `exterior`; `bar.region` lies on the shared boundary of
   its edge's endpoints; two spaces overlap only if identical (abutting is
   legal; true overlap is refused at this version). Validate-time where the
   inputs allow, expansion-time otherwise.
2. **coverage**: every standable cell (nav's `standable`) lies in ≥ 1 declared
   space or `no_body` region. Binding: standable count, covered count.
   Uncovered > 0 → red, cells listed. `no_body` share is printed as a finding
   when it exceeds the covered share.
3. **closure**: for every `enclosed` space (and the side faces of `open_top`),
   every boundary cell is non-passable, except cells inside a declared edge's
   opening (`via` when given, else the discovered openings on the shared face)
   and faces shared with an abutting declared space. Binding: boundary cells
   examined. Any unexplained passable boundary cell → red, cell and writing
   rule named (a cell written by a weighted mix says so — erosion into an
   enclosed shell is a conflict of declared intents, and it reds loudly rather
   than shipping daylight).
4. **edge proof**, per class, over the union of the two endpoint spaces plus
   the opening, using the existing predicates (`nav::connected`,
   `nav::reachable_with_fall`):
   - `walk`: connected both ways.
   - `stair`: connected both ways, rise ≥ 1 measured off the model.
   - `drop` (a→b): reachable under walk-and-fall; **not** reachable b→a under
     the plain step.
   - `barred`: **not** connected while `bar.region` stands; connected through
     exactly that opening with the region voided (a pure re-check on a copy;
     deterministic).
   - `vision`: no traversal claim; exempts its opening from closure.
5. **reachability**: every declared space reachable from `entry` over the
   declared graph with `drop` edges directed and `barred` edges closed;
   a space unreachable that way must be reachable with some set of barred
   edges opened, and that set is printed per space (the campaign-side check
   against unlock logic consumes this later). Unreachable under every
   opening → red. Binding: spaces × edges walked.
6. **anchors**: every declared anchor lies inside a covered space (or
   `no_body`, printed as a finding). Binding: anchor count.
7. **exterior faces**: edges naming `exterior` are exported as the piece's
   face contract; `--traversable`'s claim is re-derived from them (entry-class
   exterior edge ↔ exit-class exterior edge), retiring the standable-face
   approach-count heuristic (task #108's miscount).
8. **vacuity findings**: `1 space, 0 edges`, all-`open` envelopes, and
   `no_body`-majority coverage are named findings in the verdict block, pass
   or fail — the contract can be written vacuously, and the counter is that a
   vacuous one cannot be written *silently*.

## 3. Determinism and transparency

- Contract data serialises canonically; the double-expand suite extends over
  `spaces`/`edges` exactly as it covers anchors.
- Wrapper transparency: inserting `space` / `no-body` / edge declarations into
  a program moves no block bytes — asserted the way `mark`'s transparency is.
- Checker verdicts are pure over (grid, contract): same inputs, same report
  bytes.

## 4. Order of work — the cheap falsifier comes first

1. **Prototype week, no IR change**: a standalone checker (offline script is
   acceptable) + hand-written contracts for (a) trial-0001 run 1's saved
   artifact and (b) bell Z7's expansion. Required outcome: it reds run 1's
   stranded gallery and transept notches and Z7's four recorded drifts
   (`shaft/sill + 1`, `flight/tread = 1`, `flight/landing_run = 4`,
   `ring_run = 22`), and the honest cathedral contract stays reviewable
   (bounded box count, measured and reported). Failure here reworks the
   obligations at the cost of days.
2. IR surface + checker in-engine, fenced; export + `delve-admit` halves.
3. Docs and skill in the same PR (`grammar.md`, `prefab-procedure.md` §1/§3/§4,
   `tools.md`; the `/new-delve` workflow gains contract-before-rules as a
   mandatory step).
4. Bell adoption round (same milestone): contracts for the eight zone
   programs, translated from `tests/zones.rs`'s topology assertions; the Rust
   suite keeps sightline/tell/density claims and gains "the checker catches
   the same drifts" fixtures.
5. Trial 2 (owner's Stormveil-class brief) runs against the reader-facing
   docs only, as trial-0001 did.

## 5. Acceptance criteria

1. A `Program` at the new version with no `contract` block: refused, naming
   the field (fenced obligation). At the old version: compiles byte-identically
   to today (fence test both directions, red-demo per the fence rules).
2. Fixtures distilled from the verified trial defects each red the named
   obligation with the defect's cells listed: a stranded upper storey (red on
   coverage or reachability), a rounded-split boundary slot (red on closure),
   a seam overshoot (red on closure), and a matching corrected twin of each
   is green. The run-1 artifact itself, under its hand-written contract, reds
   coverage; the count of uncovered cells equals the independent probe's
   stranded count (4 982 − 2 113 within declared spaces, or the declared
   remainder in `no_body`).
3. Each edge class has a passing fixture and a teeth fixture: `drop`'s teeth
   is a rescue ladder (reuses `drop_shaft`'s knob), `barred`'s is `unbarred`,
   `stair`'s is `broken_step`, `walk`'s is a sealed doorway. Each red names
   the edge.
4. Closure's mix interaction is pinned both ways: an `enclosed` space whose
   shell role carries weighted air reds closure at a seed that voids a shell
   cell, and the identical program with the space declared `open` is green
   with the all-`open` finding printed.
5. Bell Z7's contract passes all obligations at the fixture region/seed, and
   each of its four recorded drifts reds **through the contract checker** (not
   only through `tests/zones.rs`).
6. `delve-admit audit` on a hand-built `.nbt` whose metadata carries a
   contract runs the same checker and agrees with `expand`'s verdict on an
   exported copy of the same bytes (one checker, two doors, asserted).
7. Double-expand determinism holds over `spaces`/`edges`; wrapper
   transparency holds (block bytes unmoved by declaration insertion).
8. Every gate's report carries its binding count; a zero binding on any
   obligation is printed as a finding; the §2.8 vacuity findings appear on a
   deliberately vacuous fixture.
9. Anchors round-trip with their `space` field through `PrefabRegistry`
   (`crates/compiler/tests/grammar_prefab.rs` extended).
10. Trial 2 is run and recorded in `docs/trials/` with the same
    written-before rubric discipline as trial-0001, judged on: no structural
    defect of the three verified classes ships green (checked by an
    independent probe, not by the gates under test); the breach-walkway and
    the one-way drop are stated in edge classes without falsification; and
    the agent's account does not name the contract as the reason a stated
    concept was cut. **Any of those three failing is this spec's design
    failing**, and the record must say which.
