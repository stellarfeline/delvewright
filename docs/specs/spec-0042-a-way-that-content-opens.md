# spec-0042: A way that content opens

- **Status**: Proposed
- **Question**: a zone whose design breaks a stair the campaign later repairs
  cannot declare a spatial contract. `contract-reachability`'s one escape is a
  `barred` edge, proved by **voiding** a region — an obstruction taken away.
  The broken flight is the dual: treads that are not there, which content puts
  back. A region **filled** at delve time has no edge class, so the spaces
  above the break cannot be declared reachable however they are claimed, and
  coverage is global, so the zone ships no contract at all and the whole-map
  composition round has nothing to compose. The general form is core genre
  vocabulary: every one-way shortcut opened from the far side, every lowered
  bridge, every placed ladder, every collapsed stair a player repairs.
- **ADRs**: 0020 (the contract is the checker), 0018 §7 (the `Program` version
  fence), 0006 (determinism), 0001 (the compiler emits everything)
- **Specs**: 0036 (extended — every obligation and §0 untouched), 0031 (the
  runtime region verbs this binds to), 0040 (composition consumes the result;
  its seam obligations are not touched), 0041 (sibling in flight; claims
  document version `1.6.0`, which is why this spec claims `1.7.0`)
- **Non-goals**: ways across assembly seams (face contract, spec-0036 §2.8 /
  `DW0780`); reversible or repeating ways (`close-gate` semantics unchanged —
  a way opens once); partial or staged openings; any change to the proving
  zone's geometry or any campaign content; the optional-root footing
  asymmetry in general (§1.6d — named as its own finding); `vision` edges
  (no traversal claim, so nothing to be contingent about).

## 1. The measured ground

Every claim below was verified on the current engine before anything was
designed; the twin fixtures are distilled into §7's red/green pairs. The
proving case is the bell campaign's tower zone, whose production log records
the refusal; its committed program declares no `contract` block.

1. **The class list is closed and its one contingency is void-only.** The five
   classes are refused by name at parse (`unknown variant `filled`, expected
   one of `walk`, `stair`, `drop`, `barred`, `vision``); `barred`'s second
   half is proved on a copy with the bar region turned to air
   (`with_voided`, `crates/grammar/src/contract.rs`), and the reachability
   walk's opening machinery keys on bar names alone. There is no fill
   direction anywhere in the checker.
2. **The dual reds and cannot be stated.** A minimal broken-flight piece (two
   spaces, a `stair` edge whose transit volume holds no treads): edge proof
   red ("its transit volume holds no standable cell"), reachability red
   naming all nine cells of the space above the break; the red writes no
   `.nbt`. Its repaired twin — treads present — greens both gates; the delta
   is exactly the tread cells. A `barred` edge over the same gap reds the
   other way: "with the bar region voided the two ends still do not connect
   through it" — the escape's proof obligation runs only in the removal
   direction.
3. **No opt-out reaches the gate.** `expand` has no contract override; the
   only acknowledgement (`no_body_majority_ack`) touches a different gate.
   Declaring the upper storeys out of the walk is the repair spec-0036 §0
   exists to refuse, and §2.6 refuses it mechanically: computed kinds give a
   stranded interior nothing to classify as — nested, so no `facade`;
   unwalled, so no `sealed`; unanchored, so no `posted` — and red.
4. **The runtime half exists and is general.** `fill-region` / `clear-region`
   are campaign effects (dsl `0.10.0`, spec-0031), and the completability
   model credits a runtime fill as solid **and as footing** from the quest-DAG
   point its effect fires (`RegionWrite` / `RegionState`,
   `crates/compiler/src/plan.rs`, `nav.rs`; the `DW0544` counterfactual
   exercises the footing direction).
5. **The prefab contract crosses into the compiler as bytes, not as claims.**
   `PrefabMeta::spatial_contract` is deserialised whole, and the compiler
   reads exactly one field of it: `faces` (`crates/compiler/src/faces.rs`,
   `DW0780`/`DW0781`). `edges`, `bar`, `via`, `spaces`, `no_body` are read
   nowhere outside the grammar crate. **No campaign-side check links any
   contract edge — including today's `barred` — to the effect that opens it.**
6. **So the defect is three missing bindings, not a missing mechanism** — the
   third review shape, asked first as instructed:
   - a. the grammar's contingent-edge mechanism binds to one delta sign
     (remove) and, through `barred`'s shape, to one traversal class (a walk);
   - b. a campaign effect cannot address a contract region: `fill-region`
     targets an anchor-centred box (`StealthZone { anchor, extent }`), which
     can neither name an offset cell nor a named export — the same
     too-narrow binding spec-0031's lift round recorded;
   - c. the exported-route verifier (`DW0314`,
     `nav::verify_exported_routes`) judges every leg against the base
     assembled world with **no region state**, while the router routes each
     leg **with** it — so a leg over runtime-laid floor is exported and then
     refused, pinned in `crates/compiler/tests/v10_lift.rs`;
   - d. and one soundness asymmetry, recorded here as a finding for its own
     round: `collect_region_events` keeps a fill from an *optional* root, so
     footing can be credited from an effect that may never fire. This spec
     closes it for ways (§2.5) and does not silently rewrite it in general.

## 2. Surface

### 2.1 `way` — the contingency belongs to the traversal edge

Any traversal edge (`walk`, `stair`, `drop`) may carry one optional field:

```json
{ "a": "ringing-floor", "b": "stair-foot", "class": "stair", "rise": 4,
  "via": "first-flight",
  "way": { "opens": "laid", "region": "broken-flight", "block": "tread" } }
```

- `opens` is `"laid"` (the region is empty as built; opening fills it with
  `block`) or `"cleared"` (the region stands as built in `block`; opening
  voids it). `block` is a palette role; a role bound to a mix is refused, as
  a bar's is — a way is one material.
- `region` is claimed by rules exactly as every contract region is.
  Constraints, each §0-shaped (the region must be somewhere the defect cannot
  put it): a `laid` region holds **no artifact solid as built**, and every
  way region is **disjoint from every space and from every other edge's
  `via`, bar and way region**, and lies within its own edge's opening — on
  `stair`/`drop`, inside the transit volume; a `walk` or `drop` carrying a
  way declares `via` as a **transit volume** (disjoint from spaces, abutting
  both endpoints), exactly as `stair` does, because the laid cells must
  belong to the edge. An unconstrained way region is a build-anything hatch
  and is the first thing §7's teeth kill.
- Refused on `vision` and on any edge with an `exterior` endpoint (exterior
  has no cells; a seam-crossing way is the face contract's business).

**Proof, three parts per way-carrying edge**, extending spec-0036 §2.4/§2.5:

1. *Closed, on the bytes as shipped*: the class's own connectivity proof
   **fails** as built — "the way does not open anything: the two ends already
   connect" is the red when it holds. Same demand as "the bar does not bar
   anything", both signs.
2. *Open, on a copy with the delta applied* (laid: every region cell set to
   the role's block; cleared: the region voided): the class's proof **holds**
   — `walk` both ways, `stair` through its treads, `drop` forward-only —
   and `rise` is measured on the same copy, as `barred` already measures it
   on the voided one.
3. *Reachability*: the walk runs ways shut, then opens them cumulatively by
   name — generalising the existing bars machinery to both signs, opened
   laid cells joining the target set exactly as opened bar cells do. Every
   space reachable only under openings gets its required set **named** in
   the verdict ("reached only once `broken-flight` is laid"); unreachable
   under every opening is red.

### 2.2 `barred` is the cleared way's sugar, normalised

`barred { rise, bar, via }` means exactly `walk { rise, via }` +
`way { opens: "cleared", region: bar.region, block: bar.block }`, and the
checker **normalises it to that form** so one prover covers both spellings —
a second connectivity path for cleared ways would be the private-copy defect
one layer down. `barred` stays accepted (deleting a landed surface is not an
implementer's call); `way` on a `barred` edge is refused as double
declaration. The generalisation's free reach is its own evidence: a
portcullis over a climb — `stair` + cleared way — is now writable, which
`barred`'s walk shape never could state.

### 2.3 Export

The prefab metadata's `spatial_contract` gains `ways`: name, sign, the role
and its resolved block state, and the resolved cells. Anchors inside a way
region resolve to `way:<name>` as bar anchors resolve to `bar:<name>`.
Existing pieces' metadata is byte-identical — `bar` stays `bar`; the
normalisation is checker-internal.

### 2.4 `open-way` — the campaign effect, keyed to the object

A new quest effect names a placed piece and one of its exported ways:

```json
{ "type": "open-way", "piece": "z7-bell-tower", "way": "broken-flight" }
```

It emits the fill (laid: the exported cells with the exported block) or the
clear (cleared: the exported cells to air). **Geometry, block and sign all
come from the metadata and are never re-authored** — there is no equality
check between the effect and the way because there are no two authorities to
disagree. The effect carries the shared gate struct every effect carries. A
piece placed twice has two ways; the reference names the placement.
`open-gate` / `close-gate` are untouched — anchor-keyed verbs over their own
object (the gate anchor), not this one.

### 2.5 What the completability proof does with it

- A way is **shut until its `open-way` fires**; from that effect's quest-DAG
  point the region is solid-and-footing (laid) or passable (cleared) — the
  existing `RegionState` machinery, fed from prefab metadata instead of an
  authored box.
- **Footing is credited only from a forced opening.** An `open-way` on an
  optional root opens the way in play but proves nothing: required content
  whose only route crosses it is red naming the optional root. This is §1.6d
  closed for ways.
- **The exported-route verifier judges each leg against that leg's region
  state** — the same worldview the router used. The current disagreement is
  one instrument carrying two calibrations, and its false red is what makes
  runtime-laid floor unshippable today. Alignment is deliberate shared
  calibration — verifier and router are one instrument — so the independent
  observer for this claim is the runtime tier (§7.13), never a second static
  check.
- **Disposition is enumerated, per staged way, in the compile verdict**:
  opened by which effect at which DAG point, or never opened with the cell
  count behind it. A required element — objective anchor, staged body, any
  campaign reference — resolving into a space the piece's contract reaches
  only through a never-opened (or optional-only) way is **red naming both**.
  Nothing else about a never-opened way is red: a door that never opens is
  content. Which verdict a way gets is computed from what is staged behind
  it; the author picks nothing.

## 3. What reachability proves now, and what that costs

**No state axis.** The contract proves: the **closed state on the shipped
bytes** (the break really breaks — the beat is real, and a way cannot decorate
an already-open passage); each way's **open state on its own single-delta
copy** (openability is a geometric fact, not an assertion); and **union
reachability under monotone opening, with the seam named per space**. Two
things are deliberately not proved, and each has a stated guard. Cross-way
combinations are not enumerated — the disjointness constraints in §2.1 are
what make opening monotone, so opening more ways can never disconnect a
proved edge. And the grammar layer never proves an opening *happens* —
"happens" exists only where effects exist, so that half lives in §2.5, and a
piece can green with a way no campaign opens; the campaign compile, not the
piece, says so, by name, in the disposition line. Proving only the closed
state would ship the repaired tower unproven; proving only the open state
would let repaired bytes impersonate a break; the union with named seams is
the only reading under which both twins in §1.2 are decidable.

## 4. Why this opt-out is not the sixth vacuity mode

A way-carrying edge is an opt-out from "reachable as built", so it owes §0's
question: what does it demand, and could the defect supply it? Stranding
supplies **severance** for free — the closed proof alone would be the sixth
mode wearing a feature's clothes. The other two demands are what stranding
cannot supply: an **opening proved on geometry confined to the edge's own
declared volume**, and a **performed, forced, DAG-ordered effect in the
shipped campaign**. An author can always choose to author those — and then
the delve genuinely opens the way in play: the claim has been made true, not
excused. The escape's price is authored, owner-reviewable content — the same
property that secures `posted`, whose anchors are visible in every downstream
surface. No acknowledgement string exists anywhere in this surface, and no
second hatch is added to the gate it relaxes.

## 5. Version fences and adoption

- Grammar document **`1.7.0`** — `1.6.0` is claimed by spec-0041, in flight;
  whichever change lands second carries the other's number in
  `RESERVED_VERSIONS`, and `tools/check-version-ledger-uniqueness.py` holds
  both ledgers against `origin/main`. Fenced per the `version.rs` pattern: a
  `WAY_SINCE` constant, refusal naming construct and both versions.
- Campaign dsl **`0.12.0`** for `open-way`, per-stage fence and `DW0141`
  reservation pattern. No open branch claims either number (checked against
  every open branch and against the open pull-request list, two observers).
- The route-verifier alignment is not fenced: it brings one instrument to its
  router's calibration, and the divergence cases are enumerated as tests.
- The surface is opt-in; no existing program or campaign gains an obligation.
  The bell campaign's adoption item — the tower declares its full contract,
  the rope beat gains `open-way` — is scheduled within the same milestone,
  per version-adoption discipline.

## 6. Order of work

1. Grammar: `way` surface, `barred` normalisation, three-part proof,
   reachability generalisation, fence — fixtures are §1.2's twins, distilled.
2. Export + `delve-admit` halves (`edge` grows `way`; audit agrees with
   `expand` through both doors).
3. Compiler: `open-way` (dsl `0.12.0`), metadata-fed region events,
   forced-only footing for ways, route-verifier alignment, disposition
   enumeration and its reds.
4. Docs and skills in the same PRs: `grammar.md` §2d, `compiler.md` catalog,
   `prefab-procedure.md`, `tools.md`, `/new-delve`.
5. Tower adoption round in the content repo, same milestone.
6. Demo level (row queued by this spec's PR in `docs/demo-levels.md`).

## 7. Acceptance criteria — each stating what would make it vacuous

1. **Fence, both directions.** A document declaring `1.5.0` and writing `way`
   is refused naming the construct and both versions; the same document at
   `1.7.0` minus the `way` field compiles byte-identical at both versions.
   *Vacuous if* the refusal comes from serde rather than the fence, or only
   one direction is asserted.
2. **The twins.** The broken-flight fixture with no way declared reds edge
   proof and reachability (the current behaviour, kept as the red fixture);
   with `way { laid }` declared, every contract gate is green and the verdict
   **names** the seam: "reached only once `<way>` is laid", with non-zero
   binding on all three proof parts. *Vacuous if* the green verdict carries
   no named seam — a bare green cannot distinguish *reachable* from
   *reachable eventually*, which is the exact ambiguity this spec exists to
   remove.
3. **Closed-direction teeth.** The repaired twin's bytes plus a declared
   `way { laid }` is red: "the way does not open anything" naming the edge.
   *Vacuous if* the closed proof runs on the opened copy.
4. **Open-direction teeth.** A way whose applied delta still does not connect
   its ends is red (the mirror of `barred`'s second half); a declared `rise`
   one course off the value measured **on the opened copy** is red naming
   both numbers. *Vacuous if* rise is measured on the as-built model, where a
   laid stair has no measurable climb.
5. **Confinement.** A way region overlapping a space, another edge's opening,
   or (for `laid`) holding artifact solid as built, is refused at
   well-formed. *Vacuous if* any way region is accepted anywhere on the
   model — an unconfined delta is a build-anything hatch, §0's oldest shape.
6. **One prover.** Every existing `barred` fixture yields an identical
   verdict through the normalised path, and existing pieces' exported
   metadata is byte-identical; `barred` + `way` on one edge is refused; a
   `stair` carrying a cleared way proves green on a portcullis-stair fixture
   — the statement the old surface could not make. *Vacuous if* cleared ways
   retain a connectivity prover of their own, which is the private copy this
   corpus keeps finding.
7. **Export and determinism.** Metadata carries each way's name, sign,
   resolved block and cells; an anchor inside a way region resolves to
   `way:<name>`; the double-expand suite extends over ways; declaring a way
   moves no block bytes. *Vacuous if* determinism is asserted only over
   contracts without ways.
8. **The effect has one authority.** `open-way` emits exactly the exported
   cells with exactly the exported block (laid) or air (cleared), with sign
   derived from the metadata; there is no surface on the effect for a region,
   a block, or a sign. *Vacuous if* any of the three is authorable on the
   effect — two authorities plus an equality check is the defect this shape
   avoids, not a variant of the fix.
9. **Ordering has teeth.** A campaign whose objective stands beyond a laid
   way compiles when a forced `open-way` precedes it in the DAG; the same
   campaign with the effect moved after the objective, or onto an optional
   root, is red naming way, effect and objective. The fixture demonstrates
   the way is load-bearing: deleting the opening effect must produce the red.
   *Vacuous if* the objective is reachable by any other route, which makes
   every ordering assertion about this way inert.
10. **The verifier and the router agree.** A critical path over runtime-laid
    floor exports and verifies green; a test runs router and verifier over
    the same leg-region state and asserts agreement. This pair shares its
    calibration **by design** — they are one instrument — so this criterion
    claims consistency only; §7.13 is the independent observer. *Vacuous if*
    this AC is cited as proof the floor works in play.
11. **Disposition binds.** The compile verdict enumerates every staged way
    with opened-by or never-opened and the cell count behind it; a campaign
    staging a way-carrying piece whose enumeration binds zero ways is itself
    red. *Vacuous if* the enumeration is a log line rather than a gate with a
    binding count — an unbound green is spec-0036 §2.9's oldest finding.
12. **The proving zone adopts.** The tower's program declares its full
    contract at `1.7.0`, the broken flight as `stair` + `way { laid }`, at
    the pinned region and seed; every contract gate green; the storeys above
    the break named in the seam line. *Vacuous if* the fixture repairs the
    treads in bytes — the closed proof must bind on the shipped bytes, or
    the beat itself is unproven.
13. **Runtime proof.** The demo level's PackTest and bot tier cross the
    opened way — the first campaign to declare a way owes the runtime fire,
    as spec-0031 established for `on_death`; not optional, not the author's
    discretion. *Vacuous if* the bot's route never needs the way.
14. **Diagnostics covered.** Every new refusal carries a DW code minted at
    implementation against the then-current catalog, test-asserted per the
    DW-coverage convention; grammar-layer verdicts keep named-gate form.
    This spec deliberately claims no code numbers: two in-flight changes
    minting numbers in prose is how the recorded collision happened.
    *Vacuous if* a diagnostic ships allowlisted instead of asserted.
