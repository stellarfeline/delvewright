# Trial 0003 — Halgrave, whole

The subject is the question the first two trials walked up to and neither could
ask: **given a finished story and eight independently designed, independently
reviewed zone prefabs, can an authoring agent compose them — plus the
connecting and decorative pieces the whole needs — into one complete, handsome,
coherent castle?**

Trial 0001 measured whether one recognisable building could be stated. Trial
0002 measured whether a playable one could. Both were single programs designed
as wholes. This one measures composition: eight parts, each designed against
its own reference by a run that never saw the others, becoming one place that
reads as one place. Nobody has yet established that they do.

The method under test is the settled one (spec-0040, Accepted): **bottom-up
layered design.** Each zone program is a designed sub-module, like a function;
the whole map is the complete program that calls them and writes what no part
can know — the massif, the seams, the silhouette. The opposite is refused by
the spec and by this brief alike: a shell fitted over a whitebox is a second
design object whose every agreement with the interior is a hand-computed
constant nothing checks.

**This brief and its failure criteria are written down before the run, and no
run may change them.** The material handed to the agent may be corrected
between runs where the ladder in §Attribution attributes a failure to it, and
every correction is recorded in the run's own row; the rubric and the F-rows
never move. This record carries no verdict yet, so `check-trial-verdicts`
holds it red by design until run 0's rubric lands — that red is the gate
saying "brief, not result", and it is the intended state.

## The brief given to the agent

> Compose the whole of Halgrave — the drowned-bell campaign's citadel — as one
> map program: `design/programs/map.json` of `the-drowned-bell-r2`, document
> version `1.5.0`, with its own `zones.json` entry (region, seed, claims), on
> the campaign's own development branch.
>
> The eight zone programs are called **as they stand**. Include all eight;
> allocate each a box by `split`; bind their params and palette roles per call
> site; do not edit any zone's rule bodies. The map program writes everything
> else itself: the rock the zones are carved from and stand on, the connective
> pieces between them, the curtain, roofscape and ground that carry the
> silhouette, and the one sea.
>
> The geometry of record is written, not pictured: `map-brief.md` and
> `map-zones.md` fix the massing (six ground planes against the standing tide
> at one level), the ten seams and their rises, and the three facts a plan of
> boxes loses — the ward is a hole below the gate that admits it, the cistern
> is beneath rather than beside, the tower is a climb that stands clear. The
> five reference views (front elevation, west elevation, site plan with the
> arrival route, aerial, section) are rank-only reference, never targets.
>
> Every seam in `map-zones.md`'s seam table becomes a declared contract edge
> of the map's own contract, with its class and rise; every whole-map datum —
> the sea plane first — is one map param bound down, with guard identities so
> a drifting floor refuses at expansion naming both numbers. The map's entry
> is the flat at the causeway head, and the contract's reachability must carry
> a body from there to the belfry floor.
>
> Deliver: the program and its manifest entry; every gate transcript with its
> binding counts; a double-expansion determinism check; the render set,
> including named square-on elevations of the identity faces and a cutaway
> section; and the decision log this trial's §Attribution names. A zone that
> cannot supply what the whole requires, with its params bound and its box
> honest, is a **finding stated in the fixed formula** — "Zk cannot supply X;
> its program lacks Y; the nearest workaround is Z" — and that finding is
> worth more than a workaround that fakes X. The same holds in reverse: "this
> already works, and here is how" is a sharper deliverable than manufactured
> work. Record, as you go, what the toolchain was like to use.

## What the agent may read

- The reader-facing engine documents — `docs/reference/grammar.md`,
  `prefab-procedure.md`, `tools.md`, the `/new-delve` skill — the corpus
  through `delve-grammar list` / `show`, and the tools' own `--help`.
- `docs/specs/spec-0040-map-composition.md`. The method is prescribed, not
  under discovery; withholding the prescription would manufacture failures
  this trial would then misattribute to the model.
- The campaign's design tree on its development branch: the story documents,
  `map-brief.md`, `map-zones.md`, the five reference views and their prompts,
  the eight zone programs, `zones.json`, the generation record, the zone
  review sets and concept images.

**Not** `crates/**` — the sandbox is arranged so the sources are absent rather
than merely forbidden — and not this record beyond the brief block above. The
trial measures what an authoring agent can do from the material an authoring
agent actually has.

## Why this brief and not another

Per demand, whether the vocabulary already names it — so the trial cannot fail
merely for lack of a word, and so a failure lands on the right bucket:

| Demand | Named by |
|---|---|
| compose eight documents into one | the `include` list, document version `1.5.0`, prefixes and anchor renames |
| give each part a box and a frame | `split`, `reorient` — with the recorded rule that a piece's own `largest` frame turns it in a wrong-shaped box, which the parts already guard by refusal |
| set a part's datums and materials per call site | `bind` over prefixed params **and** palette roles |
| state a seam as a proof | the contract block: `walk` / `stair` / `drop` / `barred` edges with `rise`, and the recorded rule that included claims arrive prefixed and **the destination's contract must classify them** |
| the one-way fall at the gate breach | the `drop` class, and the manifest's `allow_falls` claim |
| designed ground | ordinary rules (spec-0040 §3b); a sea cliff with a cut road already exists in the corpus in exactly this shape |
| one sea at one level | a fluid role filled to one param's height; a contained sea already judges green in the shore zone |
| photograph the identity faces | `delve-render piece --view name=…,face=…[,cutaway=true]`, which frames a tiled zone |

What is genuinely under test, and these are the clauses the trial exists for:

1. Whether `include` + per-call-site `bind` reaches **everything** a whole
   must set, at eight-zone scale — the surface has been exercised at two and
   three includes, never at eight plus a massif.
2. Whether the boxes the site plan implies satisfy the parts' own guards, and
   what a part's refusal costs the whole.
3. Whether one map contract can classify eight included programs' regions and
   carry closure, edge proofs and reachability across every seam — the
   checker's obligations have never bound at map scale.
4. Trial 0002's transferable number, one scale up: how many of the ten seams
   line up by construction, and how many by a constant nothing checks.
5. What the parts, which predate spec-0040, fail to owe the whole under its
   §4 part obligations — missing params, missing contracts, hard-coded datums,
   unrenameable stems. The parts were never designed to be composed; whether
   they can be is half of the question.

Known inherited state, recorded here so the run is not credited or blamed for
it: the gate ward is `UNDECIDED` on four world-frame `iron_bars` runs
(`DW0742`) at its declared region — the run records what composition does to
that state, in either direction, as a finding; and the rule library's one
recorded red (`DW0800`, a body can stand on water as far as `nav` knows) is a
named engine gap the sea's walkability claims must be read against.

## Rubric

| # | Question | Answer |
|---|---|---|
| R1 | Does the front elevation read as **one** castle — one place, not eight buildings touching? | yes / partial / no |
| R2 | Does the delivered geometry agree with the written massing — the six planes, the ward a hole, the cistern beneath, the tower a climb that stands clear? | per fact, agrees / disagrees, checked against the text |
| R3 | Do the machine gates pass with stated non-zero bindings? | yes / no, with the counts |
| R4 | What was missing? | named, per bucket, via §Attribution |
| R5 | Of the ten seams, how many line up by construction and how many by hand-computed constant? | per seam |
| R6 | What did each part fail to owe the whole? | per zone, the debt list |

R1 is judged by the planner from named square-on elevations held beside the
five reference views, and recorded with the shots so a later reader can
disagree. R2 is checked fact-by-fact against `map-brief.md`'s own sentences —
the geometry lives in the text precisely so drift is checked rather than
eyeballed. R3, R5 and R6 are measurements. Every judged verdict declares its
instrument bound, per the existing gate over this directory.

## Failure criteria — fixed before the run

Any row below is a failure of the run; every failure goes through the ladder.
Each row names the count that proves it examined something.

| # | Failure | Bound to |
|---|---|---|
| F1 | The composition is incomplete: composed-prefix count ≠ 8, a zone omitted, or any zone program's bytes differ from their reviewed state | the expand transcript's per-prefix count; `git diff` over the eight files |
| F2 | Expansion refuses at the recorded region and seed and the run does not resolve it by legitimate means | the refusal transcript |
| F3 | Any always-on gate red (outside the exclusions record, which must fail with exactly its codes), or any contract obligation red **or zero-bound**: closure, coverage, edge proof, reachability, no-body, anchors, exterior faces | each obligation's own binding count |
| F4 | Fewer than the seam table's ten rows realised as declared interior edges with proven rises | interior-edge count ≥ 10 |
| F5 | A whole-map datum stated more than once, or the perturbation demo fails: moving one zone's floor param against the sea plane must refuse at expansion naming both numbers, and restoring it must green | the guard-identity list; the red→green transcript |
| F6 | The belfry space is not reached from the entry by contract reachability, or zero reachable standable cells in the tower's bands measured off the delivered bytes | reachability target-cell count; the band measurement, taken with a reader calibrated against the engine's own counts |
| F7 | Double expansion is not byte-identical | the tile and manifest comparison |
| F8 | R1 is `no`, or any R2 fact disagrees in the delivered geometry | the shots and the byte-level re-derivation |

Success is the absence of every row, with R1 at `yes` or `partial`; the
trial's answer to its question is R1–R3 jointly, and R4–R6 are the deliverable
whatever the answer is.

## Attribution — how a failure lands on one of four causes

A failed composition means one of four different things, with four different
consequences: the **model** could not do it (try a different tier), the
**brief** was underspecified or misleading (rewrite the material), the
**language** cannot express it (an engine spec), the **library** has no piece
or a deficient piece for it (a prefab round). A trial that cannot tell them
apart produces an opinion. The distinction is forced by evidence the run is
instrumented to record, then adjudicated by probe — never argued from the
result alone.

**What the run must record, or attribution is impossible:**

1. **A decision log, per brief clause**: either the mechanism used, or the gap
   in the fixed formula — *"there is no way to state X; the nearest workaround
   is Y"* / *"no piece provides X; searched: …"*.
2. **The corpus-search transcript**: the `list` output and every `show` read.
3. **Every machine refusal, verbatim.**
4. The program, entry, hashes, full gate transcripts with counts, shots.
5. **The part-debt list** (R6), per zone, against spec-0040 §4's part
   obligations.
6. The agent's own account of where it got stuck — asked in the brief, not
   afterwards.

**The ladder, per failed criterion:**

1. **Trace to the brief.** Every F-row and rubric fact cites the clause that
   demanded it. A miss no clause demanded is a **brief** failure, whatever
   else it is; the clause is quoted in the finding.
2. **Read the decision log.** A failure the log never mentions — the agent
   believed the clause satisfied — is presumptively **model**, pending step 3.
   A failure the log names as a gap goes to step 3 as a claim.
3. **Adjudicate the claim by probe, not argument.**
   - *Claimed language gap*: the planner (or a second agent that cannot see
     the run's work) attempts the minimal program stating X against the same
     pinned grammar version. Expressible → reclassified **model**. Not →
     **language**, recorded with its nearest workaround; both prior trials
     killed one "impossible" apiece this way, which is why the probe is
     mandatory.
   - *Claimed library gap*: the recorded corpus listing is re-checked. Piece
     present under a documented name → **model** (and a doc defect, if the
     name was findable nowhere the agent was given, is a **brief**-class
     finding). Genuinely absent, or present but unable to be composed — a
     needed param undeclared, no contract, a datum hard-coded — → **library**,
     with the deficient part and field named. The part-debt list is the
     evidence; it is checkable against the zone programs' own bytes.
4. **A model attribution is not believed until confirmed** by one of: the
   step-3 probe (the statement exists and the run did not find it), or a
   retry run at a different tier under this brief verbatim, same sandbox.
   Retry succeeds → model confirmed for run 0's tier. Retry fails the same
   way → the attribution moves back to brief-or-language and the probes are
   re-examined. Neither available → the finding is recorded **unattributed
   between model and language**, which is honest and closes nothing.

**Why the pairs separate.** Model vs language: the minimal-statement probe,
whose only shared premise with the run is the pinned grammar version. Model vs
brief: the clause trace against a brief frozen in this file. Model vs library:
the corpus and the zone programs are enumerable bytes, and the run's own
listing is recorded. Brief vs language: the table in §Why this brief
pre-registers, per demand, whether the vocabulary names it — a failed demand
marked *named* cannot be a vocabulary failure. Language vs library: a
*statement* that cannot be made anywhere is language; a statement the map
could make but only by re-authoring a designed part is library. Two causes
compounding is recorded as two findings, not averaged.

## Scope limit — what this trial cannot measure

**The artifact is looked at, not walked.** The compiler refuses tile-set
metadata (`DW0346` names the queued placement work), and the whole map
exceeds the per-axis template cap on every axis — so there is no world, no
server, no bot run, no relight, and no playtest of the composed whole. This
is a stated limit, not a workaround target: the run must not attempt the
placement capability, which is scheduled elsewhere.

Therefore this trial **can** establish: that the composition expands, holds
its contract — closure across seams, proven rises, a walk from the sand to
the belfry — and reads as one place in the render set. It **cannot**
establish: what the seams feel like to walk; lighting as experienced (the
lighting tool cannot read a tile-set manifest — both prior trials recorded
it); the belfry's actual view at eye level; encounter staging; or anything a
playtest is for. The route proof is existence, not experience. A verdict that
depends on any of these is out of this trial's reach and is not asked.

Two further bounds. Composed zones re-draw their weighted mixes (the draw
stream is sequential — spec-0040 §1.4), so the per-zone accepted renders
certify geometry, palette and distribution but not texture bytes; the
composed review is the appearance authority, and it looks specifically for
the two known re-texture risks — a loud mix member clumping under the new
draw, and an identity-carrying role landing its wrong member on one small
visible face. And the trial does not measure the map program's cost of
maintenance across later campaign rounds; R5's constant count is the proxy
and is recorded as a number, not a verdict.

## The gates, against the six vacuity modes

| Mode | Where it could hide here | What forecloses it |
|---|---|---|
| unbound | a contract green that examined no cells | every F-row names its count; the checker itself reds zero bindings on closure, edge proof and reachability; F1's prefix count and F4's edge floor are stated minimums |
| unfenced | a map document below `1.5.0` silently ignoring its include list | the loader refuses an unresolved include by name and validates each composed document against its declared version; the record quotes the version line |
| unemitted | judging a program that never expanded | every gate runs inside `expand`; export writes tiles only on green; the planner re-runs every recorded command on the delivered bytes before any verdict is written (the claim-audit obligation both prior trials carry) |
| UNRUN | the whole trial protocol living in this doc | the map's `zones.json` entry binds it to the campaign branch's zone-audit workflow on every push and pull request — after the trial, the composition is re-judged whether anyone remembers; inside the trial, no verdict is recorded before the re-run above |
| one-directional | a rise check that only catches drift one way | edge proofs assert `rise` by equality; the datum demo is red **and** green, both transcripts required; F6's threshold moves in the direction that historically happens (trial 0001's two artifacts each measured **zero** reachable cells above the ground band) |
| unsecured opt-out | three hatches exist | a zone cannot be marked out of the composition — the design of record places all eight, so the object decides, not the author; the exclusions record is inversion-shaped (exact codes, still judged) and **the run may not add an entry** — a new capability gap goes through the ladder as a finding instead; an `instrument-bound` verdict must name a blocker reproducible against the tool alone, without the artifact — a defect of the artifact cannot supply that — and no machine row may be declared instrument-bound |

## What is recorded, per run

The program and manifest entry; region, seed and hashes; the full gate report
with every binding count; the double-expansion comparison; the shots and shot
manifest, elevations named; the decision log, corpus transcript and refusal
transcripts; the part-debt list; the six rubric answers with instrument
bounds; and the planner's re-derivation of every factual claim from the
delivered bytes, in the claim-audit form the prior trials fixed.

## Run 0 — result

Not yet run.
