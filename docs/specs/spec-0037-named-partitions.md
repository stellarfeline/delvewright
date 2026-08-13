# spec-0037: Named partitions — what the citadel concept actually asks of the grammar

- **Status**: Proposed (DEC-0070, owner, 2026-08-12 — when an approved concept
  and the back end disagree, grow the back end, never cut the concept down;
  DEC-0071, owner, 2026-08-12 — a building is judged at playable scale, the
  silhouette carries the recognition. Every claim below is measured against the
  eight approved *Drowned Bell* concept images
  (`campaigns/the-drowned-bell-r2/design/concept/`, branch `campaign/bell-r2`)
  and against runs of `delve-grammar`; probe programs and commands in §5.)
- **Specs**: 0027 (the back end), 0033 (corpus/idiom discipline — §4.6 and
  §4.8 govern what this spec may and may not add), 0036 (spatial contract:
  *detects* spatial defects after expansion; this spec *removes* one class of
  them at the source — complementary surfaces, no overlap)
- **ADRs**: 0004 (prefabs+jigsaw), 0006 (determinism), 0018 §7 (the `Program`
  version fence this rides)
- **Non-goals**: curve primitives (§3.1 — disproved by probe), overlay (§3.2),
  a positional index (§3.3), reflection (§3.4), parameterised `call`
  (task #107, own concern, §3.5), any building-shaped vocabulary entry (§3.6),
  local-direction `mark` facing and region/trap anchors (`grammar.md` §7,
  already recorded, not image-driven).

## 1. The question and the measured answer

The eight concept images show a coastal citadel: drystone barrow mounds on a
shore; a ledge road down a sea cliff; a gatehouse whose passage runs under a
**semicircular arch** with a portcullis lattice, oculi and a floor channel; a
flooded ward crossed by a **causeway that doglegs** between freestanding
pointed arcades, under a jettied guard house; a roofless cloister of pointed
arches with a ragged crest; a trussed great hall with a hooded fireplace and
**stacked lancets**; a **barrel-vaulted** cistern with a circular crown
breach; a battered, buttressed bell tower with an open belfry and a walled
approach ramp. The eight zone programs on the same branch state none of this —
they compose the §5b staging pieces into a rock-carved keep.

The question this spec answers, element by element, is the one CLAUDE.md
requires first: **what does the existing general mechanism fail to reach, and
why?** The answer, measured by probe rather than argued:

- almost everything above is *already expressible* — including the two
  elements that looked most like missing engine surface, the round arch and
  the woven portcullis (§3.1, §3.2). The precedent held again: like PR #419's
  roof valley, the "missing generator" was a composition of existing
  constructs nobody had written down.
- exactly one thing cannot be **said**: that two splits in two rules are *one
  partition*. Every multi-rule alignment in the citadel — the dogleg
  causeway's three bands agreeing where the berm columns run, a facade's
  storey bands stacking their openings, a ward plan reused by every storey —
  is today a size list restated per rule, tied by author discipline that
  nothing states and nothing checks. §2 proposes the surface; §5 P2/P3 are
  the reds.

## 2. The primitive: named partitions

### 2.1 Surface

A program may declare partitions as named values, beside `params`:

```json
"splits": {
  "ward-plan": {
    "sizes": [ {"size":"absolute","blocks":{"expr":"int","value":4}},
               {"size":"absolute","blocks":{"expr":"int","value":1}},
               {"size":"relative","weight":{"expr":"int","value":1}},
               {"size":"absolute","blocks":{"expr":"int","value":1}},
               {"size":"absolute","blocks":{"expr":"int","value":4}} ],
    "rounding": "start"
  }
}
```

A split node may then apply one by name instead of carrying its own pattern:

```json
{ "op": "split", "axis": "x", "use": "ward-plan",
  "children": [ …five children… ] }
```

`axis`, `orient` and `children` stay at the use site — the point is the same
partition under **different** children. `sizes`, `rounding` and `repeat`
travel with the name; writing any of them beside `use` is a validate refusal.
Size expressions evaluate in the using scope, exactly as inline. Semantics of
an applied named split are identical to the same split written inline — the
construct adds a name, never a behaviour.

**The guarantee the surface exists for** (a theorem of ADR-0006, asserted as
a test): two applications of one named partition, on the same axis, over
scopes of equal extent on that axis, derive identical piece boundaries. The
tie between the causeway's three bands stops being discipline and becomes a
fact a reviewer can read and an edit cannot break in one copy.

### 2.2 Object class, in one line

**A partition is a value of the program — a `param` whose value is a piece
pattern — owned by no rule**; today it exists only inline in the rule that
first cut it, so the second rule needing the same partition has no surface,
and the restated copy is the defect (CLAUDE.md: generality is decided at the
first site; the trials were the first site, the citadel is the second).

### 2.3 What existing mechanisms fail to reach, and why

- **`params`** reach every *number* in a size list (probe P2v3: all five
  widths of the causeway plan read params; dialling `pa` and `pc` moved all
  three bands together, route still green under `--traversable`). They do not
  reach the list's **shape**: piece count, order, absolute-vs-relative kind,
  `rounding` and `repeat` are enum structure, not expression-valued, and are
  restated per copy. Nothing marks three lists as intended-identical, so a
  reviewer cannot tell drift from divergence.
- **A shared rule** cannot own the partition, because children are fixed in
  the rule body and the language has no positional index by which one body
  could learn which band it serves. The nearest in-IR workaround — smuggling
  the band identity through `orient` and dispatching on an `orientation`
  guard — is a covert argument channel, the downstream-folklore shape the
  no-hack rule forbids.
- **Parameterised `call`** (task #107) would reach it and much more; it is a
  strictly larger surface with its own open design, and nothing in the eight
  images is unstatable while it waits. Named partitions do not preempt it:
  if #107 lands later, a partition stays data, not control.

### 2.4 Refusals and mechanics

- Validate (before any expansion): unknown `use` name; `use` beside `sizes`,
  `rounding` or `repeat`; an empty named `sizes`; the same weight/size-kind
  rules as inline — enforced by the **same code path**, never a copy (the
  `ok()`-in-one-spike lesson).
- Naming: same identifier rules as rule names; `/` reserved for include
  prefixes.
- `compose::include` copies `splits` under `<prefix>/` and rewrites `use`
  references, exactly as it rewrites `call` and `Expr::Param`; the
  byte-identity seam promise extends to a splits-carrying source.
- Determinism: a pure name lookup; nothing changes.
- **Version fence**: `Program` documents carrying `splits` are fenced at the
  version ADR-0018 §7 introduces (the field lands with, or after, the
  required `version` field — never before it, or the fence has nothing to
  hold). The red-demo-across-a-version-fence caveat applies to CI fixtures.

## 3. Deliberately not proposed

Each entry names what the images seemed to ask for, and why no surface is
added. Probes are in §5.

1. **A curve/arc primitive** (z2's semicircular gate arch, z6's barrel
   vaults, the oculi). Refused — **disproved by probe P1**: the §2c taper
   recursion with its per-course step chosen by quadratic guards states the
   *exact* floor-√ rasterized semicircle at r = 6, 8 and 12 (0 cells of
   mismatch), 3.6–7.1% from the cell-centre analytic disc where the
   documented 45° point is 25–31% off; extruding the same program along `Z`
   is the barrel vault for free (7 identical slices measured). The finding is
   a documentation gap, not an engine gap: `grammar.md` §2c idiom 3 says the
   step "is not fixed at one cell" but no document or corpus program shows a
   *guard-selected* step, and trial 0001 R4 recorded "every arch is therefore
   a 45° point" as if it were an IR fact. Per spec-0033 §4.8 the probe
   program is owed to the **corpus**; whether it becomes a tenth idiom
   belongs to a failed trial (§4.6), not to this spec.
2. **Overlay / a second write pass** (the woven portcullis, two vaults
   crossing, "carve a doorway through a wall"). Refused: the portcullis is
   partition refinement — bar columns own the crossings, gap columns carry
   the horizontal runs (probe P4, one rule); crossing prisms are PR #419's
   four-rule ring-peel result, which the round profile of P1 enters
   unchanged; the doorway case is settled vocabulary (`grammar.md` §5c,
   `tee_passage`, and the two recorded rejections beside it).
3. **A positional index.** Refused: every citadel case that seemed to want
   one resolved into either the remaining-box recursion (P1 recovers height
   above springing as `r − Y`) or partition identity (§2). Trial 0001's
   polygonal-apse case is real but is not posed by these images at playable
   scale; surface added for it now would be speculative.
4. **Reflection / a mirroring orientation** (z7's return ramp, the
   switchback). Refused: a rule body written mirrored is recorded and
   sufficient (`grammar.md` §2c idiom 7, `stair_flight`'s dogleg note); the
   open `cliff_path` question stays open in §5c where it is recorded.
5. **Parameterised `call`** (#107). Not proposed here: a reuse and
   maintainability gap, not an inexpressibility — nothing in the eight images
   is unstatable for its lack (§2.3).
6. **Any building-shaped vocabulary** — `arch`, `arcade`, `belfry`, `vault`,
   `cloister` entries. Refused on CLAUDE.md's own test: authored content
   wearing a primitive's clothes (spec-0033 §2 records why catalogues cap
   authorship). The citadel's arcades, mounds, batters, crenellations,
   trusses, corbelled jetties, hooded fireplace, embrasures, floor channel,
   ragged crests and open belfry were each checked against the corpus and
   idioms and are compositions, not primitives.
7. **Ornament** — tracery, mouldings, voussoir detail, the bell as a
   modelled object. Refused under DEC-0071: detail, not silhouette. The other
   edge of the same ruling is why §3.1 was probed at all: a round arch
   against a pointed one *is* silhouette, and it was measured, not waved off.

## 4. What this spec does not close

- The always-on reachability measurement seeds from **every** side-face cell
  at grade, so a route severed mid-piece still reads 100% reachable when both
  fragments touch side faces (measured, P2: the severed causeway reports
  30/30 reachable while `--traversable` fails). That is §4c behaving as
  documented, but the number invites exactly the misreading its own docs
  warn about. Recorded as an observation for the gates' owner; nothing here
  changes it.
- spec-0036's contract will *declare* the causeway's route and catch the
  severed form at check time; named partitions make the drifted form
  unwritable at the source. Both are wanted; neither subsumes the other.
- Whether the eight zone programs are **rewritten** against the citadel
  concept is campaign work on `campaign/bell-r2`, out of scope here
  (spec-0033 §4.9's own boundary).

## 5. The probes — every red and green, reproducible

Programs in `docs/specs/spec-0037-probes/`; run with `delve-grammar` from
this tree. All expansions seed 1; all programs deterministic (guard-only),
so the seed is recorded, not load-bearing.

| # | Program | Region | Measured |
|---|---|---|---|
| P1 | `round-arch.json` | 16×8×1 (`r=8`), 12×6×1 (`--param r=6`), 24×12×1 (`r=12`) | opening = exact floor-√ semicircle: 0-cell mismatch at all three radii; vs cell-centre analytic disc 5.8% / 7.1% / 3.6%; 45°-point control 30.8% / 25.0% / 30.4% |
| P1v | `round-arch.json` | 16×8×7 | barrel vault by extrusion: all 7 `Z`-slices identical |
| P2 | `causeway-bend.json` | 15×4×24, `--traversable` | one X-partition restated in three rules; baseline: traversable pass (bound 2), 30/30 standable reachable |
| P2r | `causeway-bend-drift.json` | 15×4×24, `--traversable` | **the red**: one copy's plan edited (west flood 4→2, its neighbour 5→7 — the other two copies untouched); `check` ok, `blocks-exist`/`non-empty` pass, always-on reachability still 30/30 (100%) — `traversable` FAIL, no prefab written |
| P2v3 | `causeway-bend-params.json` | 15×4×24, `--param pa=2 --param pc=7 --traversable` | params tie the *numbers*: the dial moves all three bands together, pass — the partial mechanism §2.3 credits |
| P3 | `facade-stack.json` | 13×12×2 and 12×12×2 | three storey bands restating one centred split: openings at x∈[5,7] in **all** bands at both widths — within-axis alignment over equal extents is already by construction |
| P3r | `facade-drift.json` | 13×12×2 | **the red no gate reads**: one relative share (1→2) edited in `band_b` only; zero red gates, route claims inapplicable — and the middle lancet sits at x∈[4,6] under x∈[5,7]: a silhouette defect invisible to every existing check |
| P3t | `facade-stack-truncate.json` | 12×12×2 | default `truncate` on the even width leaves column x=11 unwritten in every band — the §2c daylight-slot behaviour, re-confirmed while probing |
| P4 | `portcullis.json` | 10×9×1 | the woven lattice from one rule by partition refinement; no overlay |

P2r and P3r together are the two faces of the missing statement: when the
drift happens to sever a declared route, an opt-in gate catches it; when it
merely mis-stacks a facade, nothing does and nothing can, because the tie was
never stated anywhere a checker could read.

## 6. Acceptance criteria

1. `Program` accepts a `splits` map and split nodes accept `use`; a program
   without `splits` serialises **byte-identically** to today, asserted over
   every library program by the existing JSON round-trip suite.
2. Each §2.4 refusal has a test asserting its error: unknown `use`; `use`
   beside `sizes` / `rounding` / `repeat`; empty named `sizes`. Validation
   is the same code path as inline splits (one function, asserted by test
   structure, not by review).
3. The alignment theorem is a test: a fixture applying one named partition
   from two rules over equal extents derives identical piece boundaries,
   read off the expanded model. The P3/P3r probe pair is ported as its
   red→green control: the inline twin reproduces the drift; the `use` twin
   has no second site to edit.
4. `compose::include` prefixes `splits` and rewrites `use`, and the include
   seam's byte-identity test covers a splits-carrying source.
5. `delve-grammar coverage` counts named-partition use as a construct with a
   binding count, and a corpus program reachable from `delve-grammar list`
   demonstrates it (spec-0033 §4.8: a corpus example, not an idiom entry).
6. The `splits` field is refused by a loader that does not know its
   `Program` version, per ADR-0018 §7; it does not land before the required
   `version` field exists.
7. Same-PR doc updates (`grammar.md` §2 table and §2c): the §2c sentence
   "there is no positional index and no way to say *this opening is the same
   cells as that one*" is re-scoped to what remains true after this spec —
   alignment across **unequal** extents and across axes is still the
   author's arithmetic.
8. The P1 probe program enters the corpus (`delve-grammar list`), closing
   §3.1's documentation finding; its entry states the guard-selected-step
   technique and cites this spec. No idiom-index entry is added by this
   spec (spec-0033 §4.6).
