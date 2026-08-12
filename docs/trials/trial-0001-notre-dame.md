# Trial 0001 — Notre-Dame de Paris

The spec-0033 §4.5 authoring trial. Owner set the subject, 2026-08-12: a
building whose fine structure is so much of its identity that the usual
Minecraft answer is to build it at several blocks per real metre. That is the
answer this project does not take (DEC-0071), which is what makes it the right
probe — it asks whether the back end can carry a recognisable cathedral with
the detail deliberately thrown away.

**This brief is written down before either run, and neither run may change it.**
A brief adjusted after seeing a result measures nothing. Both runs use this
file verbatim.

## The two runs

| Run | Documents the agent may read | Measures |
|---|---|---|
| **0** | the reader-facing docs as they stand *before* the spec-0033 idioms land | today's ceiling |
| **1** | the same documents *after* the idioms land | what the idioms bought |

Run 0 is the red half. Running only run 1 would show a result and prove nothing
about its cause.

## What the agent may read

The reader-facing documents only — `docs/reference/grammar.md`,
`docs/reference/prefab-procedure.md`, `docs/reference/tools.md`, the
`/new-delve` skill — plus the library corpus through `delve-grammar list` and
`delve-grammar show`, and the tools' own `--help`.

**Not** `crates/grammar/src/**` or any other engine source. The trial exists to
measure what an authoring agent can do from the material an authoring agent
actually has; the planner's own arcade probe read `ir.rs`, which is exactly the
advantage under test. The sandbox is arranged so the sources are absent rather
than merely forbidden.

## The brief given to the agent

> Build Notre-Dame de Paris as a grammar program, at playable scale — a party
> of one to four walks through it in adventure mode. It is a cathedral-sized
> cathedral, not an enlarged model: fine detail is out of scope by decision.
> What must survive is the silhouette and the interior's character.
>
> The silhouette: a twin-towered west front, the two towers of equal height and
> flat-topped, with a great round window between them above the portals; a long
> nave under a steep roof running east from that front; a transept crossing it;
> a spire over the crossing; an apse closing the east end; buttressing along the
> flanks.
>
> The interior: a tall central vessel flanked by lower aisles, separated by an
> arcade of pointed arches; a band of windows above the arcade; a stone floor
> walkable from the west doors to the east end.

No reference image is supplied. The referent is among the most documented
buildings in the world, and a text brief keeps the trial clear of third-party
image licensing (ADR-0013). The palette is therefore chosen from the fiction
and measured against the ranked list, which `prefab-procedure.md` §2 already
allows.

## Rubric — identical for both runs

| # | Question | Answer |
|---|---|---|
| R1 | From an exterior shot alone, and without being told, would a viewer name the building? | yes / partial / no |
| R2 | Does an eye-level interior shot read as the inside of that building? | yes / partial / no |
| R3 | Do the machine gates pass, `traversable` included, with non-zero bindings? | yes / no |
| R4 | What was missing? | the idiom or primitive, named |

R4 is the deliverable that matters most. Under spec-0033 §4.6 a failed trial is
the **only** thing that justifies adding a tenth idiom, so an unnamed failure
buys nothing. "It could not do it" is not a finding; "there is no way to state
X, and the nearest workaround is Y" is.

R1 and R2 are judged by the planner, and the judgement is recorded with the
shots so a later reader can disagree with it.

## What is recorded, per run

The program, the region and seed, the full gate report, the shots, the four
rubric answers, and the agent's own account of where it got stuck. Results land
in this file as a section per run.

## Run 0 — result

Run on `integration/prefab-stack`, 2026-08-12. Sandbox verified to contain zero
`.rs` files; the agent had the four reader-facing documents, the three binaries,
`block-appearance.py`, and the library through `delve-grammar list` / `show`.

**The artifact.** 113 rules, 13 palette roles, region **27 × 48 × 76**, seed 1,
program hash `sha256:cc1bdc05…`. Past the axis cap, so it shipped as two tiles
and a manifest — which the agent reports as a non-event it never had to think
about. Gates: `blocks-exist` 19, `non-empty` 98496, `traversable` 59 (47
approach, 12 exit, 3560 standable, connected), `audit` pass over both tiles,
11 of 11 anchors eye-eligible with zero render diagnostics. Determinism
re-verified.

**Rubric.**

| # | Answer |
|---|---|
| R1 silhouette | **no** — "a Gothic cathedral", not Notre-Dame unprompted. The distinctive moves are present in kind (twin flat-topped towers joined by a horizontal gallery rather than a gable, rose over three portals, flèche, stepped chevet); what carries recognition and is absent is the flying buttresses of the east end, excluded by the no-diagonal rule. |
| R2 interior | **yes** as a Gothic nave, not as Notre-Dame's. Pointed arcade, glazed aisles, clerestory band, stone paving running the length to a lit east window. Three storeys where the referent has four; flat slab where it has a sexpartite vault; 45° heads where it has two-centred ones. |
| R3 gates | **yes** for the three expansion gates and `audit`; **the lighting step could not be run at all** — see below. |
| R4 missing | named in full below. |

### R4 — what the language could not state

- **No positional index.** An extent cannot depend on where a scope sits along
  an axis, so a polygonal apse became three hand-written facets with literal
  insets, which is why the program carries an equality guard on the region
  rather than a minimum.
- **No non-constant profile step.** Inside a taper recursion the step is always
  one cell per side per course, because `1` is the only absolute piece that
  still leaves a relative remainder to recurse into. Every arch is therefore a
  45° point; a two-centred arch, an ogee and column entasis are not expressible
  as recursions at all. This is the single largest fidelity loss in the interior.
- **`call` takes no arguments** — the one clearly missing primitive, with a
  measured cost. 29 of 113 rules are byte-identical to another once role names
  and call targets are erased (19 shape-groups; the wall-plane idiom alone has
  8 copies). Nothing keeps copies in step: the taper was changed once and its
  glazed twin had to be remembered, and every gate would have stayed green if it
  had not been. The workaround — a Python generator emitting the JSON — is worse
  than the duplication, because the artifact of record stops being the artifact
  the tool consumes. Task #107.
- **No overlay.** Siblings partition, so nothing runs *around* a building: the
  aisle lean-to spans three siblings of one split and was built in three pieces
  at three hand-computed heights, where a one-course error opens a slot of
  daylight that **no gate reads**. Three conceptually identical string courses
  are authored three times. Two roofs cannot form a valley.
- **`mark` cannot state a local facing and cannot mark a region.** All 11
  anchors derive `north`; the two anchors that should look *across* the nave
  cannot, so their eye shots look along an aisle instead of at the arcade they
  are about. A third worked example for the facing spec already open in
  `grammar.md` §7, from an unrelated genre.
- **A role bound to a directional block state breaks silently under `largest`.**
  Stair facings are world-cardinal; hand the program a region taller in X than Z
  and every roof stair, voussoir and cornice faces the wrong way while every
  gate passes. Defended with a region guard — a program-level patch for an
  IR-level hole.

### What the documents got wrong

- **"No curve → escalate to a Rust generator" is an overclaim.** The rose window
  is two rules that are each other's size-list reversed, producing a chamfered
  octagon at any odd size that re-centres itself. The true statement: **a
  grammar orientation cannot mirror, but a rule body can be written mirrored,
  and that is enough for any shape with a mirror plane.** Task #94.
- **The taper recursion is the general shape, not a fact about stairs.** One
  three-rule recursion is the nave roof, every apse roof, both gables — and with
  `void` in place of `fill`, every opening in the building. `church` already
  contains half of it and never notices the inversion.
- **The weighted-paint JSON is documented nowhere.** The agent recovered it by
  running `strings` on the binary. It is the cheapest high-value feature in the
  language — one role, six weighted glasses, is the entire reason the windows
  read as stained glass.

### What the trial found in the tools

- **The lighting probe binds to the region box, not to player space**, so a
  free-standing building reports `dark` at any lighting design — a gate that can
  only fail. The agent did not weaken it: it wrote its own measurement, which
  found a **real** defect the tool could never surface (48 interior floor cells
  unlit on the nave centre line, between candles), fixed the program, and
  remeasured to min 4 with none below threshold. The tool still says `dark`.
  Task #106.
- **`lighting` is the third door.** `audit` and `render` refuse a lone tile by
  name; `lighting` dies on a manifest with a gzip error — so procedure §7 is not
  completable for any building past the cap — and on a fragment it *succeeds*,
  manufacturing an anchorless `spdx: UNKNOWN` document beside a correctly
  provenanced zone. Task #105.
- **`traversable`'s approach binding counts any standable cell in the face**:
  47 reported, 3 real doors, the rest window sills and belfry louvres. True
  claim, misleading count. Task #108.
- `check` cannot see a recursion exhausting its axis, which is region-dependent;
  and `rounding: start` is owed by every surface, not only floors, because an
  unwritten wall cell is a slot of daylight and no gate reads it either.

### Dropped by the playable-scale decision (DEC-0071), not by oversight

Flying buttresses (diagonal), the triforium (four storeys do not fit in twenty
courses), ribbed vaults (two axes at once), tracery, tympana, sculpture, and the
two-centred arch profile.

## Run 1 — result

Not yet run.
