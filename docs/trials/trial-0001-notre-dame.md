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
approach, 12 exit, 3560 standable, connected), `audit` pass over the zone
manifest — which is the only thing it will judge, since it refuses a lone tile
by name — 11 of 11 anchors eye-eligible with zero render diagnostics.
Determinism re-verified.

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

Run 2026-08-12 against `main` at `744c41e`, the commit that landed the idioms.
Same brief, same rubric, same sandbox arrangement — verified to contain zero
`.rs` files, and `delve-grammar list` showed 33 programs of which 10 are the
`idiom-*` set.

**The artifact.** 145 rules, 11 palette roles, region **31 × 64 × 93**, seed 1,
program hash `sha256:d43ec535…`. Four tiles and a manifest. Gates:
`blocks-exist` 28, `non-empty` 184512 (28633 filled), `traversable` 80 (49
approach, 31 exit, 4982 standable, connected), `audit` pass over the whole zone,
12 of 12 anchors eye-eligible, with **one** render diagnostic — `DW0727`, the
north-tower anchor's own cell is solid, so its eye shot is taken from one cell
off the anchor.

Verified by the planner rather than accepted: the gate report was reproduced by
re-running the expansion, and determinism was re-checked by expanding into a
fresh directory — all four `.nbt` and the manifest byte-identical.

**Rubric.**

| # | Run 0 | Run 1 |
|---|---|---|
| R1 silhouette | **no**, and a square-on elevation later confirmed it | **partial** at the time, and the binding constraint moved out of the language; **re-judged `yes`** once a camera could frame the west front — see below |
| R2 interior | **yes** as a Gothic nave | **yes** as a Gothic nave, and now under a stepped pointed vault rather than a flat slab |
| R3 gates | **yes**, `lighting` unrunnable | **yes**, `lighting` unrunnable — identically, from an independent agent |
| R4 missing | six items | four of the six unchanged, two new, one withdrawn |

### What the idioms bought

The agent states the counterfactual itself: without the idiom index it would
have read the escalation row and asked for a Rust generator, "which would have
been the wrong call." Idiom 3 became every roof, every arch head, every window
head, the vault, the flèche and the chevet plan — written eight times in
different axes and paints. Idiom 7 became the rose window directly, where run 0
had to derive the mirrored-body technique for itself and record it as a
correction to the documents.

So the measurement the trial was built to take: **the idioms did not raise the
language's ceiling — they raised how much of it an author reaches on the first
attempt.** Run 1 carries 145 rules against run 0's 113 — 28% more — over a
region of 184 512 cells against 98 496, 1.87× the volume, and spends none of
that on rediscovery.

That conclusion is about the *language*, and it survives. It is not a conclusion
about the two buildings: on R1 they differ, and the trial under-reported run 1
because no camera could take the picture. See the re-judgement below.

### One documented impossibility falsified

Run 0 recorded "two roofs cannot form a valley", and `prefab-procedure.md` §6
implies a crossing needs a Rust generator. It does not. The union of two prisms
has a **plus**-shaped cross-section at every course, and a plus is a partition,
so the recursion peels the *ring* rather than the box — four rules, true valleys
at all four re-entrant corners, both ridges at one height, any width. The step
that makes it work is that at each level the recursion box's own X extent is
already the width the Z-band needs, so the arm rules never track height.

This is the second overclaim of the same shape found by the same probe (run 0
killed "no curve → escalate"). Both said *escalate to Rust* about something the
grammar states in four rules. Under §4.6 a tenth idiom is earned only by a
failed trial, and this was a success against a false claim — so it belongs
inside idiom 3 and as a correction to §6, not as a new row. Flagged for the
owner rather than decided here.

### Confirmed twice, independently

Four findings reproduced by a second agent that could not see the first's work:
`call` takes no arguments (#107), `lighting` dies on a manifest and manufactures
an `UNKNOWN`-licence document from a fragment (#105), the lighting probe binds
to the box rather than player space (#106), and `traversable`'s approach count
is standable cells rather than ways in (#108). None is a matter of taste.

`call`'s cost **grew with scale**: 29 of 113 rules were duplicates in run 0
(26%), 44 of 145 in run 1 (30%), 27 shape-groups. The worst case is one pointed
arch recursion written four times because neither the axis nor the paint can be
passed. Two partial workarounds are worth documenting because they are real and
undocumented: an `absolute` size takes an expression of `dim`, so anything
derivable from the scope's own extents needs no argument; and `reorient` is the
one thing that *can* be handed to a call — one rose-window rule family serves the
west rose and both transept roses. Neither can pass a paint, a size or a role.

### New in run 1

- **Mirroring is a silent-defect generator.** One `transept_arm` rule served both
  arms; because a split lays its pieces low-to-high, the end wall landed on the
  outer face of one arm and the crossing-facing face of the other, leaving an
  opening to the sky in the south flank. `blocks-exist`, `non-empty`,
  `traversable` and `audit` all passed, and four 45° orbit renders did not show
  it; it was found by reading one eye shot. Idiom 7 presents a mirrored rule body
  as an enabling technique — it is equally a way to ship a hole, because nothing
  in the toolchain can tell you the copy you did not write is missing.

  **Scope of the claim, established on the delivered bytes.** The agent found and
  repaired this during authoring, so it describes an intermediate state rather
  than the artifact. The shipped run-1 zone is X-mirror symmetric to within **36
  cells, all at z 5–6 / y 32–33 — the west towers**, not the transept; run 0 is
  symmetric exactly. Nobody may cite the hole as a property of the delivered
  zone.

  **A fourth reading, on the bytes and on the program, retracts the substitute
  finding.** The section previously asserted that at the transept band, z 53–59,
  both flanks stood open for seven blocks with no end wall and no side wall. The
  delivered bytes do not say that. In each flank — the wall is two blocks thick,
  x 2–3 and x 27–28 — there is an aperture five blocks wide at z 54–58 running
  from y 1 to y 9, narrowing to three at y 10 and to one at y 11, solid above:
  a pointed arch, eleven courses tall. Its jambs at z 53 and z 59 are solid stone
  from y 0 to y 31, and the nave flank two blocks further out at z 52 is solid
  from y 0 to y 30. Both flanks carry the same aperture, so the zone remains
  X-mirror symmetric.

  It is not a hole. `transept_portal_wall` splits the flank 2 stone / 5 open /
  2 stone and hands the middle to `tp_open`, which is eight courses of `void`
  under `head_void_z` — the taper recursion that makes every other arch head in
  the program. The transept portal is authored, and it is the size a transept
  portal is.

  What the bytes do carry at that band is one measured fact, stated without a
  verdict: the zone's own ground slab at y 0 runs x 0–30 everywhere except
  z 53–59, where it stops at the building line, x 2–28. So the five threshold
  cells of each portal are standable and reachable on foot from the grade
  entrance, and the cell outside each is not floor but nothing — the walk ends at
  the doorway. Whether that is a defect depends on what the piece is placed into,
  which the zone alone cannot say.

  **The lesson that survives is about the probe, not the building.** Three
  successive probes each returned a plausible number about this band and each was
  wrong in a different way, and the seven-block figure is the tell: it is the
  extent of the *ground slab's* notch, read as if it were the extent of a missing
  wall. A measurement that reports one object's extent under another object's
  name cannot be caught by re-reading it — only by asking what, exactly, the
  probe bound to.
- **The renderer had no camera an author could aim, and for this subject that
  was decisive.** The shot set was fixed: four exteriors at yaw 45/135/225/315,
  `top` at pitch 90, eye shots at pitch 0. **There was no square-on elevation of
  any face**, so a building whose identity is one elevation could not be
  photographed, and that alone was the whole of R1's "partial" — which the
  re-judgement below then confirmed by taking the picture. The obvious fix failed
  instructively: a parvis with an eye anchor facing the west front looks straight
  through the central portal, and because the orbit cameras fit the model bbox, a
  forecourt long enough to frame a 49-block front shrinks the building in *every*
  exterior frame. The workaround that did work deserves to be standard practice —
  give the building a high place a body can stand and anchor there; the shot from
  the north tower down the roof to the flèche is the only image in the set that
  is a photograph rather than a model view. And the parapet must be **one**
  course: at two it sits exactly at eye height and the shot is a wall.
- **`traversable`'s axis and direction are undocumented and inverted from what an
  author would guess.** Three probe expansions established it: the axis is always
  world Z, never the longest axis, and **approach = Z-max, exit = Z-min** — so the
  west portals are the "exit" and the apse is the "approach". This drove a real
  design decision (an axial chevet door, without which the gate would have
  refused correctly and confusingly). Extends #108, which was about the count.

### Withdrawn from run 0's list

"Two roofs cannot form a valley" — see above.

### What a body can reach, measured on both artifacts

Neither run was asked for an upper floor, and neither is faulted for not building
one. The measurement is recorded because it is a property of the delivered zones
that later rounds will cite, and because the gate that passed does not imply it.

|  | run 0 | run 1 |
|---|---|---|
| standable cells | 3560 | 4982 |
| reachable on foot from the grade entrance | 1548 (43.5%) | 2267 (45.5%) |
| **reachable above the ground band** | **0** | **0** |
| unreachable, open to the sky | 1835 | 2478 |
| unreachable, **under a roof** | 177 | 237 |

`traversable` passes on both, because it proves a route between two ground-level
faces and nothing about the rest of the floor.

The distinction in the last two rows is the whole finding. Most stranded floor is
aisle roof and tower deck — standable, open to the sky, and nobody's defect. The
actionable part is small and locatable: run 0's two west-tower ground chambers
with no door (60 cells each, x 1–6 and 20–25, y 1, z 65–74); run 1's 84-cell
chamber over the **apse** (x 12–18, y 23, z 81–92 — run 1's west front is at
z-min, its east end at z-max), and its two belfry floors (41 cells each,
x 1–9 and 21–29, y 35–36, z 4–12).

### Judgement on R1 and R2, recorded so a later reader can disagree

The planner's read of the shots differs from the agent's on R1 and is recorded
as such. The exterior does read as a Gothic cathedral, and the moves that
identify this one are present in kind: twin flat-topped towers joined by a
horizontal gallery rather than a gable, a long steep nave roof, a flèche over
the crossing rather than a tower, a stepped chevet. Under DEC-0071 the
silhouette is what carries recognition, and this silhouette carries most of it —
the planner would put R1 at partial-leaning-yes where the agent put it at
partial-leaning-no. What is genuinely lost is the flanks: flat walls where the
buttressing should be, excluded by the no-diagonal rule.

R2 needs no qualification. The interior reads as a nave without being told:
a tall central vessel, a pointed arcade on piers, a glazed band above, stone
paving running the full length, the west rose closing the vista. One of the
agent's own criticisms is visible in the same frame: the pier sconces read as
lit *bands* rather than fixtures, because a `fill` of a light role covers a
whole 2 × 1 × 2 pier.

The second criticism, that run 1's vault reads as a dark void, was a judgement
of one frame and does not separate the runs. Measured on the bytes, the block
light on the nave vault's soffit is **0 along its whole length in both runs** —
77 centre-line cells in run 1, 74 in run 0, none of them above the dark
threshold. Neither vault is lit; run 0's simply photographs brighter, because
run 0's nave is low enough that its ceiling fills the top of an eye shot while
run 1's stands four courses higher and is almost out of frame at pitch 0. Any
band-average of the two frames measures wall, not vault, and moves by fifteen
points depending on which nave shot is picked — so no such average is quoted
here.

## R1 re-judged, with a camera that can frame the west front

Run 1's R1 answer carried its own disclaimer: *"there is no square-on elevation
of any face, so a building whose identity is one elevation cannot be
photographed, and that alone is the whole of R1's `partial`."* An aimable camera
now exists — `delve-render piece <manifest> --view name=…,face=…` — so the
disclaimer is testable. Both zones were re-photographed from their delivered
bytes, square-on to the west front, no cutaway:

- run 0: `--view name=west-elev,face=south` (its west front is at z-max)
- run 1: `--view name=west-elev,face=north` (its west front is at z-min)

**Run 0: still `no`.** The elevation does show the identifying moves — twin
flat-topped towers, a horizontal gallery between them in place of a gable, a
centred rose, three portals, a flèche behind. They do not compose. The towers
are narrow and stand *behind* a broad screen wall rather than forming its outer
thirds, so they read as turrets on a keep; the gallery sits above the wall head
instead of between the tower bases; there is no Gallery-of-Kings band and no
vertical division of the front into three bays; and the rose is large and low
enough to become the subject of the whole elevation. The dominant impression is
a blank white plane with a wheel in it, which is the opposite of the referent's
articulation. A viewer would say "a Gothic cathedral", exactly as recorded.

**Run 1: `yes`, up from `partial`.** Square-on, the front carries the complete
identifying set at close to the referent's proportions: two equal flat-topped
towers occupying the outer thirds and flush with the façade, each with paired
tall lancets; a horizontal gallery spanning between them at their base in place
of a gable; a rose centred above it, flanked by glazed lancet groups; a
horizontal band below the rose where the Gallery of Kings belongs; three portals
on the axis; the flèche rising between the towers; and the whole divided into
three bays by pilasters. What argues against it, visible in the same frame: the
ground storey is open across nearly the full width, so the portals read as an
arcade rather than as three deep portals in a wall — the densest zone of the
referent is the emptiest zone here.

**The finding is the change itself.** The trial reported the same building at
`partial` because the shot set could not point at the thing being judged, and
said so; the verdict still went into the record as a verdict. Under the rubric's
own question — *from an exterior shot alone, and without being told, would a
viewer name the building* — the answer for run 1 is yes, and the trial's summary
line therefore understates what the idioms produced. It does not change the
trial's conclusion about the *language*, which was measured on rule counts and
rediscovery, not on the photographs.

## Instrument bounds on the judged verdicts

A verdict bounded by the instrument rather than by the artifact says so at the
verdict. Every rubric answer above, and what it was judged from:

| Verdict | Bound | Judged from |
|---|---|---|
| R1 run 0 | artifact-bound | square-on west elevation, `delve-render piece --view name=west-elev,face=south`, no cutaway |
| R1 run 1 | artifact-bound | square-on west elevation, `delve-render piece --view name=west-elev,face=north`, no cutaway |
| R2 run 0 | artifact-bound | the 11 eye shots, plus block light re-derived on the delivered bytes |
| R2 run 1 | artifact-bound | the 12 eye shots, plus block light re-derived on the delivered bytes |
| R3 run 0 | instrument-bound — `delve-admit lighting` cannot read a tile-set manifest, so the lighting leg of the answer covers no measurement the tool took | the three expansion gates and `audit`, both re-run on the delivered program |
| R3 run 1 | instrument-bound — same unreadable manifest, reproduced independently | the three expansion gates and `audit`, both re-run on the delivered program |

## Claim audit

Every factual assertion this record makes about the two artifacts, re-derived
from the delivered bytes rather than from the run logs. `checked` means it was
reproduced; `corrected` means it was not, and the text above now states the true
value; `unverifiable` means the artifact needed is gone, or the claim is a
judgement and is marked in the text as one.

The instruments: `delve-grammar expand` re-run on each delivered program;
`delve-admit audit` / `lighting` and `delve-render piece` re-run on the
delivered zones; and a reader that loads the tile set into a dense grid and
transcribes `crates/grammar/src/nav.rs`'s own predicates. The reader is
calibrated before it is believed — it reproduces the engine's filled-cell,
standable, grade-entry, reachable, sheltered and pocket counts exactly on both
zones — because the three corrections this record has already taken were each a
plausible number from an uncalibrated probe.

| Claim | Status | Re-derived by |
|---|---|---|
| run 0: 113 rules, 13 palette roles | checked | counting the program's `rules` / `palette` |
| run 1: 145 rules, 11 palette roles | checked | same |
| regions 27×48×76 and 31×64×93, seed 1 | checked | the manifests, and re-expansion at those regions |
| program hashes `cc1bdc05…` / `d43ec535…` | checked | re-expansion recomputes both from the program files |
| 2 tiles / 4 tiles and a manifest | checked | the manifests' `parts` |
| run 0 gates: `blocks-exist` 19, `non-empty` 98496 (18167 filled), `traversable` 59 = 47 approach + 12 exit, 3560 standable, connected | checked | `delve-grammar expand --traversable`, re-run |
| run 1 gates: `blocks-exist` 28, `non-empty` 184512 (28633 filled), `traversable` 80 = 49 approach + 31 exit, 4982 standable, connected | checked | same |
| determinism: re-expansion is byte-identical | checked | all 2 + 4 `.nbt` compare identical to the delivered files |
| run 0: `audit` passes | checked, wording tightened | `audit` refuses a lone tile by name and passes over the manifest; it never judged "both tiles" separately |
| run 0: 11 of 11 anchors eye-eligible, zero render diagnostics | checked | the delivered shot manifest |
| run 1: 12 of 12 anchors eye-eligible, **zero** render diagnostics | **corrected** | the delivered shot manifest carries one `DW0727` warning on `anchor/north-tower-roofward` |
| run 0: all 11 anchors derive `north` | checked | the manifest's `facing` fields |
| `call` duplication: 29 of 113 rules (26%), 19 shape-groups, wall-plane idiom 8 copies | checked | canonicalising each rule with role names and call targets erased; the 8-member group is `aisle_wall_w/e`, `arcade`, `transept_end`, `tower_wall`, `facade`, `east_wall`, `apse_wall` |
| `call` duplication grew: 44 of 145 (30%), 27 shape-groups | checked | same |
| run 1 is "a third larger, region 1.9×" | **corrected** | 145 vs 113 is 28% more rules; 184512 vs 98496 is 1.87× |
| the rose is two rules that are each other's size-list reversed | checked | `rose_up` = [absolute 1, relative 1] over [fill, call]; `rose_down` = [relative 1, absolute 1] over [call, fill] |
| run 0's nave is under a flat slab | checked | cross-section: interior open to y 19, solid slab y 20–22 |
| run 1's nave is under a stepped pointed vault | checked | cross-section: open to y 23, stepped taper y 24–28 |
| `traversable` approach = 47 cells, 3 real doors, rest sills and louvres | checked | of the 47 cells on run 0's z-max face, 7 are at y 1 in three runs of x (9–10, 12–14, 16–17); the other 40 sit at y 23, 27, 30 and 42 |
| `traversable`'s axis is world Z; approach = Z-max, exit = Z-min | checked | `nav::ends`, and the face decomposition above |
| `delve-admit lighting` dies on a manifest with a gzip error | checked | `DW0732 … gzip decode: invalid gzip header`, both runs' binaries |
| `lighting` on a fragment succeeds and manufactures an anchorless `spdx: UNKNOWN` document | checked | `lighting --write` on one tile writes a sidecar with `"anchors": {}` and `"spdx": "UNKNOWN"` beside the correctly provenanced manifest |
| `audit` and `render` refuse a lone tile by name | checked | `DW0732` and `DW0721`, both runs' binaries |
| the lighting probe binds to the region box, so a free-standing building can only report `dark` | checked | on one run-1 tile it reports `profile: dark`, min 0 over 2438 floor cells; the building occupies x 2–28 of a 31-wide region, so the outside ground is in the measured set |
| run 0's remeasured interior: min light 4, none below threshold | checked | the run's own probe re-run on the delivered zone: ground floor min 4, 0 cells below 3 |
| run 0's authoring intermediate: 48 unlit cells on the nave centre line | unverifiable | the intermediate program was not kept; only the repaired artifact ships |
| run 1's X-mirror symmetry is within 36 cells at z 5–6 / y 32–33; run 0 is symmetric exactly | checked | 72 occupancy mismatches = 36 unpaired cells, all at those coordinates; run 0 has 0. Measured on occupancy — directional block states differ under a mirror by construction |
| run 1's transept band z 53–59: both flanks open seven blocks, no end wall, no side wall | **corrected** | each flank carries an authored pointed-arch portal, 5 wide at z 54–58, 11 courses tall, jambs solid; the seven-block figure is the ground slab's notch, not a wall's absence |
| "nothing walks there" at the transept | **corrected** | the five threshold cells of each portal are standable and reachable on foot from the grade entrance |
| reachability table: 3560/4982 standable, 1548/2267 reachable, 0 reachable above the ground band, 1835/2478 open, 177/237 sheltered | checked | `delve-grammar expand --reachable-floor`, and independently: every reachable cell in both zones is at y 1 |
| run 0's two doorless west-tower chambers, 60 cells each, x 1–6 and 20–25, y 1, z 65–74 | checked | the two largest unreachable pockets, exactly those bounds |
| run 1's 84-cell chamber at y 23 "over the west front" | **corrected** | it is over the apse: x 12–18, y 23, z 81–92 |
| run 1's two belfry floors at y 35–36 | checked | 41 cells each, x 1–9 and 21–29, z 4–12 |
| run 1's sandbox showed 33 programs, 10 of them `idiom-*` | checked | `delve-grammar list` from the run's own binary |
| both sandboxes contained zero `.rs` files | checked | `find` over both trees |
| run 1's vault "reads as a dark void" | unverifiable as stated, and reframed | a judgement of one frame; on the bytes both runs' vault soffits carry block light 0 for their whole length, so it does not separate them |
| R1 and R2 verdicts | judgements, and re-judged above | R1 is now artifact-bound for both runs; R2 stands |
| R4's language gaps: no positional index, no non-constant profile step, no overlay, `mark` cannot state a facing or mark a region, a directional role breaks under `largest` | unverifiable here | statements about the language's surface, not about these artifacts; each would need its own red demonstration against the current build |
| the agent's account of where it got stuck, and the counterfactual it states about the idiom index | unverifiable | first-person report, recorded as such |
