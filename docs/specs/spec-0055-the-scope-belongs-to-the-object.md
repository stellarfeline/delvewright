# spec-0055: The scope belongs to the object — an anchor reference resolves where its object stands

- **Status**: Proposed
- **Question**: the catalog's own rule is that **the scope of uniqueness for an
  anchor name is the AREA** (`DW0857`, landed for the four gate verbs), and an
  in-flight change widens that refusal to every reference no scope settles
  when more than one area provides the name (`DW0859`, on the
  resolution-authority branch §1 names). But the DSL gives almost no object a
  scope to be settled by: of
  the object schemas that carry an anchor reference, exactly one (`Npc`)
  declares an `area`. The flat vectors under the quests document — traps,
  shortcuts, timed gates, loot, lethal volumes, ambushes, actors, waves,
  shops, environment triggers — are campaign-wide and record no area at all,
  so for them a multiply-provided name is a refusal with **no legal spelling
  of the intent**: the author cannot say which building they meant, and both
  recorded prohibitions (below) forbid saying it on the reference. What does
  an anchor reference *mean* for an object that has no area — and what
  surface makes the answer expressible rather than documented?
- **ADRs**: 0001 (DSL → compiler; the LLM writes schema-enforced JSON),
  0002 (staged DSL; areas are a stage-1 fact later stages condition on),
  0006 (determinism — resolution must never depend on sort order)
- **Specs**: 0041 (non-goal preserved verbatim: qualification of anchors,
  never — an anchor is the campaign's id), 0045 (fence keys — the claim
  fence this surface rides), 0039 (the gallery element this surface owes)
- **Reference**: `docs/reference/compiler.md` rows `DW0142`, `DW0857`,
  `DW0859`; the resolution authority `AnchorTable::resolve` /
  `AnchorScope` in `compiler::plan`
- **Non-goals**: an author-written qualifier on a reference (`DW0857`:
  "there is no second way, deliberately"; `AnchorScope`: "not a qualifier an
  author writes — that was considered and refused"); grammar-side anchor
  qualification (spec-0041); moving the flat vectors under areas (§5);
  inferring an object's area from the quests that exercise it (§5); any
  change to camera resolution — a camera flies across areas by design and
  keeps doing so.

## 1. The measured ground

Instruments: engine `0534b6d0` (`delvec` built fresh, sha256 `c39ebf11…`),
its `schema --stage all` export; the resolution-authority branch at
`a5e16b89` (unmerged — this spec assumes its refusals, not its merge);
content `campaign/bell-r2` at `8668e12c` and content `main` at `ffc7c4c`.

- **Census** (from the schema export — the single authority, never a parser
  of the source). 109 named object schemas across all stages; **21 carry a
  direct typed anchor reference** (`AnchorId`/`AnchorSubject` `$ref`);
  counting inline sub-schemas (quest-effect variants and the like) the
  population is 46. **By either count exactly one anchor-bearing schema —
  `Npc` — carries an `area`.** Two further schemas carry an `area` without
  bearing a reference of their own: `PlannedQuest` (the scope every
  quest-owned reference already resolves in at the DSL tier) and the
  world-edits `EditBatch` (edits are per-area). So the general mechanism —
  *declare where the object stands; resolve its references there* — exists,
  is the sanctioned pattern, and reaches almost nothing.
- **The unscoped classes** are the eleven flat vectors `DW0859` names, hung
  directly off `QuestsContent` with no area anywhere above them, plus the
  cutscene bodies of the dialogue stage (which play inside quests and so
  have a derivable scope the build tier does not yet use).
- **Collisions are the normal state, not an edge case**: five names provided
  by more than one area were measured on one eight-zone campaign, two on the
  critical path, at two content revisions by an instrument independent of
  the compiler. A pool area defers its anchors to the solver and takes its
  pieces' own vocabulary; the detail plan re-binds names per place
  (`Detail.anchors`), a pool does not — so two areas drawing on one tileset
  collide without any author act, and "rename" is not always a campaign-side
  act.
- **The by-name gap is stated in the resolver's own doc**: `resolve()` is
  the one authority, `Npc` and the cast path go through it, and the
  remaining helpers (`point_any`, `zone_box`, `gate_region_block_any`, the
  emitter's `anchor_point_any`) still take the first match across areas
  because the objects they resolve for have nothing to be scoped to.

## 2. What an anchor reference means

**An anchor name is the campaign's id; an anchor reference is that name
resolved in a scope; and the scope belongs to the OBJECT making the
reference — derived from where that object stands or plays, never written
on the reference.** Scope is lexical: a reference resolves in the scope of
its nearest scope-bearing ancestor in the document tree.

The scope-bearers, in the order a reference meets them walking outward:

1. **The activity the reference plays in.** A quest scopes everything it
   contains — objectives, effects, cast placements, the cutscene bodies its
   happenings run — to the quest's own `area`. A cast beat stands in the
   area the beat plays in, then the NPC's declared home (`DW0859`,
   unchanged). A wave's mobs materialize per spawn, so a wave resolves in
   the **spawning quest's** area — an event has the scope of the event, and
   a declared field there would be a second authority for a derived fact.
2. **The declared standing-place of the object.** `Npc.area` today; the
   standing flat classes gain the same field (§3).
3. **No scope.** A camera (by design), or a standing object that declares
   nothing: the reference means **the campaign's unique provider**. One
   provider is an answer from anywhere; more than one is the existing
   refusal (`DW0859`) — never a guess.

Crossing is unchanged at every rung: own scope first, otherwise the single
area that provides the name, otherwise refusal. **What is refused is the
ambiguity, not the crossing.**

This is review shape 1, and the wider site can express the quantifier: the
resolution authority already takes `AnchorScope::Area | Global` and already
answers `Found / Ambiguous / Missing`. Nothing about the rule is new; what
is missing is only the DSL surface that lets a standing object supply the
`Area` argument — which the resolver's own doc states is a version-ledger
question, not a compiler one.

## 3. The surface

One optional property, defined once and attached to the object class it
belongs to — **a thing that stands in the world** — never to a verb:

- **`area`** (`AreaId`, the stage-1 area set — the same type and meaning as
  `Npc.area`) on the nine standing classes of the quests document:
  `Trap`, `Shortcut`, `TimedGate`, `Loot`, `LethalVolume`, `Ambush`,
  `Actor`, `Shop`, `EnvTrigger`.
- **Excluded**: `Wave` (event-shaped; scope derived per spawn, §2.1);
  cameras (fly by design); inline sub-objects (`StealthZone`, disarms,
  lanes) — they inherit the containing object's scope lexically and declare
  nothing of their own.
- **Absent means today**: no declaration, no behaviour change, byte-for-byte.
  `EnvTrigger` in particular keeps its global default; the field only adds a
  spelling for the case that is currently a refusal.
- **Fenced at `dsl_version 0.17.0`** (allocated to this surface; the
  implementation appends the ledger row and the `*_SINCE` constant — this
  spec edits no ledger). A quests document declaring an earlier version and
  carrying the key is refused by the existing claim fence; earlier documents
  without it compile unchanged forever.

**The declaration must bind, or it is refused.** Each standing class has one
*placement* reference — the anchor the object bodily occupies (`Trap.at`,
`TimedGate.gate`, `Shortcut.gate`, `Loot.anchor`, `Actor.anchor`,
`Shop.anchor`, `Ambush.at`, `EnvTrigger.at`, the anchor of
`LethalVolume.region`). A declared `area` that does not provide the
placement anchor is a statement the world contradicts and is refused — the
sixth-vacuity test applied at design time: the field's one job is to say
where the object stands, and a declaration the defect (a dangling or
misaimed name) could satisfy would be no declaration at all. Auxiliary
references (`Trap.disarm.via`, `TimedGate` disarm, `Shortcut.unlock`)
resolve by the ladder from the declared scope and may still cross
unambiguously — an unlock lever may legitimately live one area over.

## 4. Refusals

Described by what they refuse; codes are allocated at implementation.

- **A declared `area` no stage-1 area declares.** The rule that already
  guards `Npc.area` reaches the new field; whether its quantifier covers the
  nine classes is asserted by test, and a new code exists only if it does
  not.
- **A declared `area` that does not provide the object's placement anchor**
  (§3). New refusal, build tier — a pool area's anchors are the solver's.
- **A multiply-provided name no scope settles** stays `DW0859`, unchanged —
  now escapable by the one act that states the fact, declaring where the
  object stands, and its prescription gains that remedy.

## 5. Considered and rejected

- **A per-reference qualifier** — recorded twice as refused, preserved. A
  qualifier decorates one lookup; a declared area is a fact about the object
  that scopes every reference it makes, is checkable against the world (§3),
  and is the pattern `Npc` already sanctions.
- **Campaign-unique names as the permanent semantics** (do nothing). §1: the
  colliding state is the normal output of pool-solved areas over a shared
  tileset, renaming is not always campaign-reachable, and a permanent
  uniqueness rule couples naming across areas — placing a second area could
  red an existing shortcut it never touched.
- **Moving the flat vectors under areas.** A total rewrite of every quests
  document, forced on every campaign, for a fact only some objects need to
  state. The flat vector *is* an un-chosen unit of authoring, but this
  surface does not foreclose re-opening it: a declared `area` migrates
  mechanically to any future nested form.
- **Inferring the area from the quests that exercise the object.** A
  standing object exercised from two areas makes the key ambiguous, and the
  inference inverts the knowledge direction: the author knows where the trap
  stands; a derivation is a hidden assumption about the caller's world.

## 6. Costs

- **Active campaign** (`the-drowned-bell-r2`, content `campaign/bell-r2` at
  `8668e12c`, quests document at `0.12.0`): 7 flat-vector entries — 4 waves
  (scope derived from the spawning quest, no edit), 2 lethal volumes and 1
  trigger (a declaration only where a placement name gains a second
  provider). The adoption round is the version raise on the quests document
  at the moment it first wants the surface; **zero rewrites are forced**,
  because absent means today.
- **Accepted and released campaigns** (`hollow-vigil`,
  `nobodys-cave-island`, `nobodys-cave`, `the-drowned-bell` on content
  `main` at `ffc7c4c`; frozen release trees): keep compiling unchanged. The
  fence that carries it is the quests-stage version-claim fence — a document
  declaring `<= 0.16.0` cannot carry the key and never needs it — plus
  pinned-engine reproduction (`versions.toml`) for the frozen trees.
  Unambiguous resolution is byte-identical by construction.
- **The migration is derived, not authored**: the refusal-on-ambiguity
  semantics already shipped, so no campaign owes a sweep. The only authored
  act this surface ever demands is one line, on one object, at the moment a
  name gains a second provider — and the alternative (rename) remains legal.
- **Gallery**: the nine new schema properties are units the coverage gate
  will enumerate the moment they land, so the implementing PR binds them in
  the same PR — with an element only the field can express (a standing
  object whose placement name two areas provide), proven non-inert by
  perturbation (§7.6).

## 7. Acceptance criteria

Checked against the tree named in §1 where they assert current behaviour;
marked **debt** where the implementation must make them true. Criteria 3, 7
and 8 presuppose the resolution authority (§1) has merged — they are
unsatisfiable before it, so the implementation follows that landing.

1. **True now, checked**: the `schema --stage all` export carries exactly
   one anchor-bearing object schema with an `area` property (`Npc`).
   **Debt**: after implementation the export shows the optional `area` on
   exactly the nine §3 classes and on nothing else new — asserted from the
   export, never from the source.
2. **Debt**: a quests document declaring `0.16.0` and carrying `area` on any
   §3 class is refused by the claim fence with a named code; the same
   document at `0.17.0` is accepted. One test asserts both directions.
3. **Debt (the motivating red→green)**: a fixture in which two areas provide
   one name — a trap referencing it is refused undeclared, and builds with
   `area` declared, in the same test.
4. **Debt**: the placement-binding refusal (§3) is asserted by a test naming
   its code (the DW-coverage gate binds it on landing).
5. **Debt**: a declared `area` naming no stage-1 area is refused; the test
   proves the existing rule's quantifier reaches the new field or a code is
   allocated.
6. **Debt**: the gallery element binds every new property and is proven
   non-inert — removing its declaration moves emitted bytes or reds the
   build. The emission baseline over every declaration-free fixture is
   byte-identical before and after the change.
7. **Debt**: every flat-vector resolution path goes through
   `AnchorTable::resolve`; the unscoped by-name helpers are removed or take
   a scope, asserted by compilation (no surviving unscoped caller), not by
   grep. A wave spawned by a quest whose area provides the wave's anchor
   name resolves there even when another area also provides it — red→green,
   since today's wave path takes the first match.
8. **Debt**: `docs/reference/compiler.md` is updated in the same PR — the
   `DW0859` row stops stating that the standing classes cannot express a
   scope, and its prescription names the declaration as a remedy.
