# spec-0062: Two rules over one cell

- **Status**: Proposed
- **Ground**: written against engine `c90da006` (`origin/main`) and the branch
  `fix/a-body-has-a-width` at `8756b398`, read only — it widens a killing
  volume's impassable set by the body that walks into it. The defect lives on
  that branch and nowhere else — `main` has no widened volume, so nothing here
  reproduces on it — and the branch is where it blocks, red on the tests the
  cycle produces. The round that found it
  measured the defect by building; this spec re-derives the geometry it names
  from the branch's own `metrics::keep_out_box` formula and the fixture's
  prefab document (§1.1), and says so where it does.
- **Diagnostics**: none allocated, and none needed — §6 says why. Spec number
  0062 was verified free across all 74 engine remote refs (file names and
  text) at writing; `spec-0061` is on 8 of them, which is the cross-check that
  the scan reads refs.
- **Research**: `docs/reference/reach-and-hazard-volumes.md` — where
  established practice places an arrival trigger and a killing volume, per
  rule cited or authored. §3 and §4 rest on it.
- **Non-goals**: the widening itself (its size is a fact about a body and the
  selector, not a design choice); the ordering of the seat proof before the
  route proofs, settled on the branch; the harness; any content work; a DSL
  offset for a volume's region (§8).

## 1. The defect: two rules, one parameter, no value that satisfies both

**Cited**, from the ledger row and the branch, except where marked.

The fixture `crates/delvec/tests/fixtures/lethal-volume` declares one killing
volume, `lethal/the-drop`, of extent `[0, 0, 0]` on `anchor/exit`, and one
`reach-anchor` objective, `obj/exit`, on the same anchor at `radius: 2`. Its
story is *"They reach the road's edge and stop short of the drop."* On
`origin/main` it builds with every rule silent. On the branch:

- at `radius: 2`, **DW0881** refuses: cells inside the completion volume stand
  on floor no body can walk to the anchor's footing from without leaving the
  volume. Its remedy: *"Lower `radius` until the volume covers only the
  anchor's own floor"*.
- at `radius: 1` — the remedy taken — **DW0850** refuses: *"no cell of that
  volume is standable in the final assembled world"*. Its remedy: *"Fix the
  geometry or move the anchor onto the floor; never widen the volume"*.
- with the branch's added side door removed, **DW0510** refuses: the widening
  seals the room's only doorway, which opens onto the drop's edge.

One of the cells DW0881 names belongs to the doorway the prefab was built
with, not to anything the fixture added. spec-0060 §3 made *every remedy a
diagnostic names is reachable* a tested rule; DW0881's is not reachable here,
and DW0850 forbids the way back. That is the defect. The widening is not in
question.

### 1.1 The geometry, re-derived

**Authored**, by arithmetic on `metrics::keep_out_box` as the branch writes it
and on `campaigns/prefabs/hello-room.json` — a reading of the formula, not a
run of the instrument. `hello-room` is 11×6×11: floor at local y=0, walk plane
at y=1, a dividing wall at z=6 with one doorway two cells wide at x=4..5
(`anchor/door`, iron bars until `open-gate`), `anchor/exit` at `[5, 1, 8]`
beyond it, `spawn` at `[5, 1, 2]` before it.

| quantity | value |
|---|---|
| the volume | the one cell `[5, 1, 8]` |
| its keep-out for a player body (0.6 × 1.8) | `[4, 0, 7]..=[6, 1, 9]` — one cell horizontally, one course down |
| completion cube at `radius: 1` | `[4, 0, 7]..=[6, 2, 9]` — **the keep-out plus a course of air**; 0 standable cells (9 on `main`) |
| completion cube at `radius: 2` | `[3, -1, 6]..=[7, 3, 10]`; 8 standable cells (17 on `main`): the west lip `[3, 1, 7..9]`, the east lip `[7, 1, 7..9]`, the doorway `[4, 1, 6]`, `[5, 1, 6]` |
| footing cells nearest the anchor, by distance² | three at 4: `[3, 1, 8]`, `[5, 1, 6]`, `[7, 1, 8]` |
| the footing DW0881 roots its walk at | `[3, 1, 8]` — the `(distance², cell)` tie-break of `World::snap` |
| the party's population, from `spawn` | the near room, the doorway, and the west lip through the added side door; the east lip is unreachable and is not judged |

So at `radius: 2` the doorway cells are inside the cube, are footing the party
stands on, and cannot walk to `[3, 1, 8]` without crossing the keep-out —
DW0881 names them. At `radius: 1` the cube is the keep-out — DW0850 names the
volume. Two things in that table are structural rather than accidental:

1. **For a body up to two blocks wide, the keep-out of a one-cell volume IS
   the radius-1 completion cube** (less a course of air on top that is footing
   only when the volume's top cell is solid). An anchor inside a killing cell
   can never complete at `radius: 1`. The smallest radius that reaches footing
   is 2, and for a volume of extent `e` it is `e + 2`.
2. **DW0881's reference is arbitrary when the anchor has no footing of its
   own.** Three cells tie at distance² 4; the rule roots at one of them by
   lexicographic order and calls the other two islands off-floor. Had the tie
   fallen on `[5, 1, 6]`, the west lip would have been the offender. A verdict
   decided by a tie-break is not a rule about the world.

The second fact is the older defect and the widening merely exposed it: any
anchor with no footing of its own — a lever set in a wall between two rooms,
an altar block, a cell that kills — has today a reference chosen by tie-break.

## 2. What each rule protects

**Cited**, from the engine at `c90da006` and the branch.

| rule | the fact it protects | its object | direction |
|---|---|---|---|
| the keep-out (`World::standable_fp` on the branch; named by `DW0510`) | a body whose hitbox meets a killing volume dies; vanilla selects on intersection, so the cell beside a face is not footing | the assembled world's standable set | refuses cells |
| `DW0850`, occupiable arm | an objective completes only where a body can be; a volume with no footing in it is an objective nothing completes | the completion cube's standable cells | wants the cube **larger** until it holds footing |
| `DW0881` | `reach` means arriving; every cell the cube completes from can walk to the anchor's footing inside the cube | the same standable cells, walked | wants the cube **smaller** until it covers one floor |

The keep-out is upstream of both: it decides which cells are standable, and
both reach rules read standability. The two reach rules read the same cells
and disagree about one parameter, `radius`, in opposite directions. Neither
knows the other exists: DW0881 names a radius without asking whether DW0850
admits it; DW0850 forbids widening without asking whether the widening it
forbids is the one DW0881 would accept.

## 3. Which rule yields: neither, at a cell — and the reach rules stop needing one

**Authored.**

1. **The keep-out does not yield.** It is not a placement rule the compiler
   chose; it is the selector's own arithmetic (research §1, last rule) applied
   to a walker whose position inside its cell is unknown. A reach rule that
   counted a keep-out cell as footing would be proving the party through a
   cell the branch has measured the bot dying in. The keep-out enters the
   `World` before any objective is judged, and no objective rule may count a
   keep-out cell as footing or demand one.
2. **The reach rules do not yield either; they are re-stated so that they
   never need a keep-out cell.** What DW0881 needs is a reference to walk to.
   Today that reference is one cell, and when the anchor is not itself footing
   the cell is a tie-break (§1.1). The reference becomes **the anchor's footing
   set**: the anchor's own cell when it is standable; otherwise every standable
   cell of the footprint at the minimum distance² from the anchor. DW0881's
   demand is then: every footprint cell the party can stand on can walk, inside
   the footprint, to **some** cell of the footing set.
3. **This is a re-specification of DW0881's reference and is declared as a
   loosening in those words**, bounded exactly: for an anchor that is footing
   (the gallery's `anchor/loft`, the mezzanine case with its nine offenders,
   every raised-anchor shape the rule was written for) the set is one cell and
   nothing changes. For an anchor that is not footing, the rule stops refusing
   whichever islands the tie-break happened not to pick and starts refusing
   only floor that is farther from the anchor than the footing is — a hall
   three courses under a lever set in a loft wall is still refused, because the
   loft cells beside the lever are nearer.
4. **Where the two reach rules disagree about `radius`, DW0850's floor
   binds.** The smallest radius whose cube holds footing — `r_min`, the
   Chebyshev distance from the anchor to its footing set — is a fact about the
   world, and no radius below it is ever named as a move by any rule. DW0881's
   "lower `radius`" is bounded below by `r_min`; DW0850's "never widen" is
   retired as a sentence about the creator's radius (§5.3) — the lesson it
   carried was about the emitter's fixed cube, and it lives in the module note
   already.

## 4. The anchor at the volume is a legitimate design

**Authored, on research §1 and §3.**

*Reach the road's edge*, *reach the lever beside the pit*, *reach the lip of
the shaft* are one shape: the objective's subject is a killing place and the
party arrives at its edge. Established practice puts an arrival trigger where
the body will be — the threshold, not the thing beyond it — and this engine
already treats an anchor on a solid affordance the same way: a `reach` on an
altar block completes from the cell in front of it, through the same snap.
A killing cell is the same case with a wider margin: the footing nearest it
is the lip, two cells out instead of one.

So the ruling: **a `reach` whose anchor lies inside a killing volume, or
inside its keep-out, is legitimate, and it completes from the volume's lip.**
What makes it expressible:

1. **The radius that reaches the lip exists and the engine names it.** For a
   player body the lip of a one-cell volume is at Chebyshev distance 2, of a
   volume with extent `e` at `e + 2`, of a raised anchor at whatever the
   geometry says; `r_min` is computed, not guessed, and is what DW0850 names
   when the authored radius is below it (§5).
2. **The footing set (§3.2) makes every side of the lip an arrival.** The party
   that reaches the drop from the doorway has reached the drop's edge exactly
   as the party that reaches it along the west wall has.
3. **The creator still has the other shape.** A story that wants the party on
   one particular side — *the lever*, not *the pit* — anchors the lever: a
   prefab anchor on the lip cell, with the volume on its own anchor. Two
   things at two places are two anchors, which is the existing model; a prefab
   that offers one anchor where the design needs two lacks an anchor, and
   adding one is prefab metadata, not engine work.

What the ruling refuses, and why: a `reach` whose footing set is **empty at
every radius up to the authored one** — an anchor buried in solid mass, or a
volume so large that its lip lies beyond the radius the creator wrote — is
DW0850 (§5.2), because no tolerance around a place nobody can approach is a
tolerance. It is not refused for being at the volume; it is refused for
having no lip within reach.

## 5. What is said, once

**Authored.** The general form the ledger row asks for: *a configuration
refused by both rules at every value of the parameter they disagree about is
refused once, in one diagnostic, naming what would have to move.*

### 5.1 One judgement, two arms

`compiler::reach` gains one function over one `World`:

```
judge(site, radius) -> Ok
                     | TooTight  { footing: [cell], r_min }      // DW0850's occupiable arm
                     | OffFloor  { cells: [cell], footing: [cell] } // DW0881's rule
```

It computes the footprint at `radius`, the footing set (§3.2), `r_min`, and
the backwards walk, with one definition of each; `check_reach_completion` and
`check_reach_footprint` become its two callers and hold no geometry of their
own. That is the *no private copy per gate* rule applied to the pair: today
each check derives the standable cells and the reference cell separately.

### 5.2 A remedy is checked against the other arm before it is named

At the authored radius `r` the judgement is taken, and **before any message
names a radius, the judgement is taken again at the radius it would name**:

| the pair of verdicts | at `r_min` | what is said | code |
|---|---|---|---|
| `Ok` | — | nothing | — |
| `TooTight` at `r`, `Ok` at `r_min` | `Ok` | the cube holds no footing; the nearest footing is `footing` at distance `r_min`; **set `radius` to `r_min`** (verified), or move the anchor onto that footing | `DW0850` |
| `TooTight` at `r`, `OffFloor` at `r_min` | `OffFloor` | the cube holds no footing, and at `r_min` — the first radius that does — the objective completes from `cells`, which cannot walk to `footing`; **no radius from 1 to `r_min` answers from this anchor**; move the anchor onto its footing, or change what stands between | `DW0850` |
| `OffFloor` at `r`, `Ok` at `r_min < r` | `Ok` | the cube completes from `cells` off the anchor's floor; **set `radius` to `r_min`** (verified), or move the anchor | `DW0881` |
| `OffFloor` at `r`, `OffFloor` at `r_min` (or `r == r_min`) | `OffFloor` | as the third row, over the range 1 to `r` | `DW0850` |

A radius above the authored one is never proposed — that is DW0881's own
"never widen" and it stands. The range a "no radius" sentence claims is the
range that was examined, `1..=max(r, r_min)`, in those words; a passing radius
beyond it is not denied, it is not claimed.

**What would have to move** is named from the same judgement: the footing set
with its distance, the off-floor cells by floor as DW0881 prints them today,
and — when the anchor's own cell lies inside a keep-out box — the volume by id
and its keep-out box, read from the `World` through the same widened set
`lethal_volumes_over` names for DW0510. How it knows is therefore nothing new:
the world already answers which volume refuses a cell; the message asks it for
the anchor's cell.

### 5.3 Two sentences retired, one kept

- DW0850's *"never widen the volume to reach the body, which is how this
  defect was closed once before"* leaves the message. It was true of the
  emitter's fixed cube; the creator's `radius` is the creator's tolerance, and
  a rule that names a verified `r_min` and then forbids reaching it names an
  unreachable remedy. The lesson stays in `compiler::reach`'s module note,
  which already records it.
- DW0881's *"Lower `radius` until the volume covers only the anchor's own
  floor"* leaves the message in that open-ended form. A radius is named or it
  is not; "until" sends the creator down a ladder whose bottom rung is DW0850.
- DW0881's *"Never widen the volume"* stays: a larger radius is never the
  remedy for completing from the wrong floor.

### 5.4 Order at the build

The order the reach at a killing volume meets rules in, one refusal per build,
each naming only remedies the next admits: `DW0511` (a posted body inside a
volume — the branch asks it first), `DW0510` (no route to the anchor with the
keep-out in force, or no footing within `SNAP_RADIUS`), then `judge` at the
authored radius (`DW0850` or `DW0881` as §5.2). `DW0510`'s footing arm fires
when `r_min` would exceed `SNAP_RADIUS`; §5.2's rows fire below that; the two
cannot both be true of one site.

### 5.5 The fixture, under this contract

The `lethal-volume` fixture as the branch commits it — the drop on
`anchor/exit`, `radius: 2`, the side door carved — builds green: the footing
set is the three lip cells, the doorway walks to `[5, 1, 6]`, the west lip to
`[3, 1, 8]`. At `radius: 1` it is DW0850 naming `radius: 2` as a verified
move. Neither the fixture's anchor nor its story is wrong; the story is the
shape §4 admits.

## 6. Why no new code

**Authored.** The state "no radius answers" is reported under DW0850 and not
under a code of its own, by the rule this engine already uses to split codes:
a code differs when the remedy differs in kind. In that state the remedy is the
anchor's placement relative to its footing — DW0850's whole subject (*the
volume that completes a reach and the footing a body can reach it from are the
same place*). DW0881 is a tolerance question, and a tolerance question presumes
a good radius exists; where none does, it is not DW0881's to say. One
configuration, one code at every value of the parameter, which is what
"refused once" means here.

## 7. Why the reachability rule did not catch it, and what it must be

**Cited** from `crates/delvec/tests/remedy_reachability.rs` and
`tools/check-dw-codes.py` at `c90da006`; the counts were taken by reading.

### 7.1 Three quantifiers, each too narrow

1. **The cross-check's subject.** `tools/check-dw-codes.py` obliges a row for
   *"a message that names a base or a document as a move"*
   (`MOVE_SUBJECT_RE`: `void`/`ocean`/`valley`, `horizon`, "site plan", a
   `*.json`). It binds 7 codes today, every one with a row. DW0881's move names
   a field (`radius`); DW0850's, DW0510's and DW0511's name an anchor, an
   objective, a post or an `extent`. None is a base or a document, so none
   owes a row, and the file holds **0 rows for each of the four** — read, not
   grepped for a count. The same reading finds moves of that kind in the
   messages of DW0317, DW0544, the body-clearance, wave-room, shortcut-gain,
   camera and volley-zone codes as well; the widened quantifier will
   enumerate them, and that number is to be verified at implementation.
2. **The assertion.** A row asserts that the move *"reaches a different
   verdict"*. A different refusal is a different verdict: a row that took
   DW0881's move and met DW0850 would have passed as written. Of the 18 rows,
   two accept a non-green terminal today (`assert_ne!(code, 1)` on the runoff
   row; `assert_ne!(code, "DW0885")` on the shown-side row), each with a stated
   reason. The shape is what matters: a cycle between two rules is invisible
   to "different".
3. **The scope.** Every row is one rule's move. The defect is a property of
   two rules over one parameter, and no row spans two.

### 7.2 What the quantifiers become

1. **Subject: any move.** The cross-check's marker becomes an imperative
   addressed to the author — the verb set gains `Move`, `Lower`, `Raise`,
   `Shrink`, `Give`, `Set` beside the numbered forms — standing near a base, a
   document, **a backticked field** (`radius`, `extent`, `anchor`) **or a
   named object** (the anchor, the post, the volume, the objective, the wave).
   The tool prints the count it binds, and a code added later that tells the
   author to move anything owes a row.
2. **Terminal: green, or a named successor with its own row.** A row's move
   ends at exit 0, or at a code the row names, for which a row exists taking
   that code's move from that state; a code met twice on one such chain is a
   cycle and a red. The two rows that accept a non-green terminal today are
   re-stated in that form or their fixtures redesigned so the move ends green.
3. **Pairs are rows.** For every pair of codes whose moves name one parameter
   in opposite directions (§8 lists the members), a row takes the first code's
   move on the configuration that has no admissible value and asserts the
   result is **one** diagnostic that names no value of that parameter as a
   move. For this pair that is the third row of §5.2's table: `radius` 1 and
   `radius` 3 on the raised-anchor-over-a-hall world both end at DW0850 saying
   "no radius from 1 to 3", and neither message contains "set `radius`".

## 8. Scope: the class, and what makes a pair a member

**Authored.** This spec rules on the class and lands the pair that opened it.

A pair of rules is a member when: (1) both judge the same cells of one
assembled world — the standable set, or a subset of it; (2) each names as a
move a value of one parameter the other also constrains; (3) the moves point
in opposite directions. Membership at `c90da006`:

| pair | shared cells | parameter | directions | status |
|---|---|---|---|---|
| `DW0850` occupiable / `DW0881` | the completion cube's standable cells | `radius` | larger / smaller | **this spec** |
| `DW0344` / `DW0885` / `DW0855` | a piece's outward faces and walk cells | `horizon` | a cycle of three | spec-0060, rows landed |
| `DW0510` / `DW0511` | a volume's keep-out | `extent`, the volume's anchor | both shrink or move the volume — same direction | not a member; their order is settled on the branch |
| `DW0510` footing arm / `DW0850` | footing near a reach anchor | `radius` | DW0510 names none | not a member; §5.4 states their boundary |

Out of scope, named: the magnitude of the keep-out (a body's width and the
selector's rule, not a choice); a region offset for `lethal_volumes[]` so a
volume and an objective could share one anchor at different cells (a second
placement vocabulary for one object class — refused on the same ground
spec-0060 §4 refuses a per-piece `horizon`); the harness's `keep_out`; wave
seats; anchors in solid mass beyond what the footing set already answers; any
edit to the ledger, the fixture on the branch, or the content repository.

## 9. The gallery's obligation

**Authored**, against the gallery at `c90da006`: two killing volumes
(`lethal/west-pit`, `lethal/east-pit`, each extent `[1, 2, 1]`), five `reach`
objectives on the primary and two on the site-plan overlay, none anchored at a
pit; `a-reach-that-completes-from-the-floor-below` probes DW0881 at
`anchor/loft`.

1. **A bound element**: one `reach` objective anchored on a pit's own anchor,
   at the radius the engine names for a `[1, 2, 1]` volume — 3, its lip being
   two cells out from a box three wide — completing from the lip, on a quest
   the population reaches. Perturbing its `radius` to 2 must move the verdict
   (DW0850 naming `radius: 3`), which is the byte-moves test spec-0039 asks
   for.
2. **A refusal probe** for the third row of §5.2's table: a raised anchor whose
   `r_min` cube also covers the hall — DW0850 with "no radius" in its message
   and no "set `radius`". The gallery's loft is that geometry with the anchor
   lifted one course into the air above `anchor/loft`; the probe is one edit
   if the generator declares such an anchor, and is otherwise a declared debt
   of the element.
3. **The existing DW0881 probe stays green and unchanged**: `anchor/loft` is
   footing, so its footing set is one cell and §3.3 changes nothing there.

## 10. Acceptance criteria

Machine-checkable; each names its instrument, and each was checked against the
tree at `c90da006` before being written. Where the tree cannot yet satisfy a
criterion the verdict is recorded as a debt, not a pass.

1. **One judgement.** `compiler::reach` exports one function that returns the
   verdict of §5.1 for a site and a radius; `check_reach_completion` and
   `check_reach_footprint` call it and derive no standable set, footing or walk
   of their own (asserted by a test that perturbs the footing rule in one place
   and sees both codes' messages move). *Tree: debt — two functions each derive
   their own; no shared judgement exists.*
2. **The footing set.** A test builds the wall-between-two-rooms world (an
   anchor in a one-thick wall, footing on both sides inside the cube) and
   asserts DW0881 reports zero off-floor cells; the mezzanine test
   (`a_raised_anchor_whose_volume_reaches_the_floor_below_is_refused`) still
   reports off-floor cells greater than zero and fewer than the footprint.
   *Tree: debt for the first half — the reference is one cell chosen by
   `World::snap`'s tie-break; the second half passes today and is the pin.*
3. **The remedy is verified before it is named.** A test over the
   raised-anchor-over-a-hall world at `radius` 1 and at `radius` 3 asserts
   DW0850 both times, a message containing "no radius from 1 to 3", and no
   "set `radius`" in either; the same world with the hall floor removed at
   `radius` 1 asserts DW0850 naming "set `radius` to 3" and the campaign at
   `radius: 3` building green. *Tree: debt — DW0850's message names no radius
   and forbids widening; DW0881's names "lower" without a number.*
4. **The fixture on the branch.** `crates/delvec/tests/fixtures/lethal-volume`
   as the branch commits it (the drop on `anchor/exit`, `radius: 2`, the side
   door) builds green under the keep-out, and at `radius: 1` is DW0850 naming
   `radius: 2`; two builds of each are byte-identical (ADR-0006). *Tree: debt —
   on the branch the fixture is red at DW0881; on `main` the keep-out does not
   exist and the criterion has nothing to bind to.*
5. **Reachability, three quantifiers.** `tools/check-dw-codes.py` binds every
   code whose message names a field or an object as a move and prints the
   count; `remedy_reachability.rs` holds a row for each move DW0850 and DW0881
   name, each ending green; every row's terminal is exit 0 or a named successor
   with its own row; a pair row takes DW0881's move on the no-radius world and
   asserts one DW0850 naming no radius. *Tree: debt — the cross-check binds 7
   codes on bases and documents; 0 rows for DW0850 or DW0881; 2 of 18 rows end
   at a non-green terminal.*
6. **The gallery.** §9.1's element is bound (its radius perturbed to 2 moves
   the verdict), §9.2's probe is committed or declared as a debt of the
   element, and `a-reach-that-completes-from-the-floor-below` is unchanged and
   green. *Tree: debt for 9.1 and 9.2 — no gallery `reach` is anchored at a
   pit; 9.3 passes today.*
7. **The record.** `docs/reference/compiler.md` carries the re-stated DW0850
   and DW0881 rows (the footing set, `r_min`, the verified remedy, the retired
   sentences) in the PR that lands §5; `tools/check-dw-codes.py` is green with
   zero new allowlist entries. *Tree: debt — the rows describe the one-cell
   reference and the retired sentences.*
8. **Binding.** The reach-footprint binding line gains, per build, the number
   of sites whose footing set has more than one cell and the number whose
   `r_min` exceeded the authored radius, so a green over a campaign with no
   hazard-centred reach reads as such. *Tree: debt — the line reports five
   counts, none of these.*
9. A row for the player-visible half — a `reach` that fires at the lip of a
   drop the party can see and not step into — is queued in
   `docs/demo-levels.md` when §5 lands. *Tree: not yet due.*

## 11. Out of scope, restated in one line

The keep-out's size; a region offset; the harness; wave seats; content; the
ledger and the fixture on the branch — §8 names each and stops.
