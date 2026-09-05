# spec-0045 — Fence keys: why an accepted document reds, and what each layer owes

Status: Proposed

Relates: ADR-0018 §7 (program-document version fence), spec-0026 (per-area walk
datum), spec-0030 (`flood`), spec-0039 (gallery campaign). Refines the `Binds`
classification (`crates/dsl/src/diagnostic.rs`, `crates/dsl/src/fence.rs`);
changes no authoring surface.

## 1. What was measured

The released campaign `nobodys-cave-island` was reported red on a newer engine
under `DW0367`, `DW0141`, `DW0478`, and the repair effort went into the
campaign — the end the constitution forbids. Re-measured per diagnostic, on the
current main engine (`delvec 1.1.0 / dsl 0.11.0`), on the spec-0026 integration
engine (`dsl 0.12.0`, not yet on main), and on three content states (current
main, the frozen release trees, the unreleased working branch of the incident):

- **The campaign builds green on the current main engine against the current
  library** (exit 0, warnings only). The build's own fence line reports one
  `Since` finding grandfathered (`DW0429`, fenced at 0.11): the obligation
  fence, holding, on the very campaign the report was about.
- **`DW0478`** (respawn seat inside a hostile's perception range) reds the
  frozen release trees under the current engine, exit 3. It is a completability
  defect the campaign always shipped — the check had examined zero objects
  while it was keyed to `rest == true` (the unbound vacuity mode), and widening
  it to the object class made the latent soft-lock visible. The campaign tree
  on content main carries the correction; the frozen release trees do not, and
  reproduce through their pinned engine (`versions.toml`, both directions).
  Correctly `EveryVersion`. Not a fence defect.
- **`DW0141`** never fired on the released campaign. It fired on an unreleased
  working branch whose stage-7 document adopted `flood` at the number an
  in-flight engine had given that surface; the number was found already shipped
  naming a different surface, `flood` was re-ledgered to 0.12.0, and the
  document's claim went stale. An adoption item on active work (a one-line
  version raise, already made), whose general form — a claimed number is never
  reused — is already a gate (`tools/check-version-ledger-uniqueness.py`). Not
  a fence defect: the fence refusing a stale claim is the fence working.
- **`DW0367`** (non-void horizon piece without a walk datum) is the genuine
  gap. On the spec-0026 engine it reds the campaign — release trees and
  content main alike — exit 3, with nothing in the campaign changed: the
  obligation's discharge lives in prefab **metadata**, a shared document with
  no version key, and the code is classified `EveryVersion` with that cost
  stated in its own doc comment. The campaign fence cannot drop what is
  declared to bind every version.

So the fence mechanism is not broken. It exists at the compiler
(`Binds`/`DwCode`/`Fenced::apply` — no constructor for "did not say", one
exit), it exists for grammar **surface** (ADR-0018 §7,
`crates/grammar/src/version.rs`), and it held on the same build that motivated
this spec. Of the engine's classified rules, 227 are `EveryVersion` and 7 are
`Since` — the designed distribution, not a defect: most rules judge what a
document says, and fencing those would let old campaigns write new surface.
What is missing is narrower: **two kinds of rule have no fence key at all** —
an obligation discharged in an unversioned shared artifact, and a grammar
**gate** (`crates/grammar/src/gates.rs` carries eleven gate declarations and no
binding declaration on any; every gate applies its current demand at every
declared program version, and the zone audit is a required CI context).

## 2. Why an accepted document reds on a newer engine — four causes, four guards

| cause | worked instance | classification | guard |
|---|---|---|---|
| 1. Version claim gone stale (a re-ledgered number) | `DW0141` on `flood` | surface rule, `EveryVersion` — correct | exists: version-ledger uniqueness |
| 2. New or widened **obligation** — going green requires *adding* a declaration | `DW0367`; the l10n key widening | `Since`, keyed per §3 | exists at the compiler; extended by §3–§4 |
| 3. New or widened **defect proof** — going green requires *correcting* what the document already asserts | `DW0478` | `EveryVersion` — correct; the red is a finding | the findings ledger + exclusion row; releases reproduce via pins |
| 4. Changed **emission** at an unchanged declared version | the 26 × `DW0364` incident | an engine regression, by definition | exists: the required campaign-builds job over the pinned content |

The classification test, sharpened — this is the operative sentence: **a fence
guards what a document must HAVE, never what it must not BE.** If going green
adds a declaration the document was never obliged to carry (a field, a key, a
string, a metadata datum), the rule is an obligation and binds `Since`; if
going green corrects something the document already asserts, the rule is a
defect proof and binds `EveryVersion`. This is how `Binds`' existing question
("could this flip on an unchanged campaign?") is answered when the flip comes
from a check that did not previously exist: ask what the repair *is*, not
whether there was a green before.

## 3. The fence key, per layer

The key is always **the declared version of the document that carries the
demand** — and which document that is legitimately differs per layer:

- **DSL surface** (stage schemas): the owning stage's `dsl_version`. Exists
  (`DW0141`). Unchanged.
- **Compiler obligations**: the same key. Exists (`Binds::Since` +
  `crates/dsl/src/fence.rs`). Unchanged.
- **Prefab-metadata obligations**: **the `dsl_version` of the stage whose
  declared content demands the behaviour.** The metadata document deliberately
  has no version of its own (tolerant reader, `crates/dsl/src/prefab.rs`) and
  gains none: it serves every campaign at every version simultaneously, so a
  version stamped on it would fence nothing — and a metadata demand always
  exists to serve some stage surface, whose version is the adoption event the
  version-adoption discipline already schedules. Consequence: `DW0367` binds
  `Since` at the version that introduces the per-area datum (0.12.0 as
  re-ledgered), and below the fence the engine keeps the placement behaviour
  those documents were accepted with — the spec-0013 global ocean datum —
  exactly as `void → BASE_Y` is already kept. A metadata key whose *parse*
  breaks old engines remains a `dsl_version` matter (the `lighting` precedent).
- **Grammar surface**: the program document's declared version. Exists
  (ADR-0018 §7). Unchanged.
- **Grammar obligations (gates)**: **the program document's declared version**,
  via a binding declaration on the gate itself. A gate judges emitted blocks,
  not authored JSON — but the adoption unit is the program document, the only
  versioned artifact in the chain, and judging derived output at the source's
  declared version is the same move the compiler's nav rules make when they
  judge the assembled world at the campaign's declared version. A gate is
  `every-version` when it is a defect proof over what the expansion *is*
  (`blocks-exist`, `reachable-floor`); `since(n)` when green requires the
  program to *declare* something programs did not previously carry. An
  admitted `.nbt` is unaffected either way: re-expansion is new work and meets
  current gates; the fence governs the audit of checked-in programs, whose red
  otherwise blocks unrelated engine work through the required zone-audit job.

## 4. Unskippable, by construction

What binds each piece, and what happens to a rule added without it:

- **Gate binding is compulsory.** `Gate` gains a mandatory binding field with
  no `Default` (the `DwCode` move): a gate written without deciding does not
  compile. The audit's report assembly is the only exit and drops a `since`
  gate's verdict on programs below its ordinal, **reporting the grandfathered
  count** the way `delvec` prints its fence line — a silent drop would be the
  fence wearing the UNRUN clothes.
- **A `Since` claim owes its demonstration.** Every `since` classification —
  DW code or gate — has a paired fixture: below the fence, grandfathered with
  the count stated; at the fence, red. A CI check in the existing docs job
  enumerates `since` construction sites from source and reds an unpaired site;
  a minimal, justified allowlist is the only exemption, per the DW-coverage
  convention.
- **An `EveryVersion` claim is an empirical claim, and the corpus is its
  falsifier.** The required campaign-builds job and zone audit build every
  campaign and zone program in the pinned content repo on every push. Its
  escape hatch is hardened — the vacuity-mode-6 question is "could the defect
  supply the opt-out?", and today it can, by writing an exclusion row:
  1. an exclusion row (campaign or zone) whose `expect_codes` name a code
     absent from the engine at `origin/main` is refused — a change cannot
     introduce an obligation and excuse its own breakage in one motion;
  2. a change that touches `crates/` may not add exclusion rows at all — rows
     are content bookkeeping, and they arrive with content corrections or pin
     moves.
  Both checks run in the same required job as the builds, held against
  `origin/main` the way the version-ledger check already is. A change carrying
  an unfenced obligation therefore reds on the pinned corpus and has no
  exclusion exit: its only ways forward are `Since`, or landing the discharge
  (content adoption plus pin move) first.
- **The residual, named honestly.** The corpus falsifies a misclassification
  only where a pinned campaign exercises the surface at an old version; the
  pinned corpus spans declared versions 0.6–0.8 plus current, and the frozen
  release trees stay out of CI deliberately (cause-3 reds are their normal
  state). A misclassified obligation on a surface no pinned campaign uses
  lands silently and is caught at the next adoption round. A fence spec that
  claimed total coverage would itself be the vacuity it legislates against.

## 5. Costs

- **No `dsl_version` bump, no program-version bump, no adoption round on any
  active campaign.** This spec adds no authoring surface and removes
  retroactive obligations; it adds none. Campaigns that raise a stage to
  0.12.0 take the walk-datum backfill as part of that adoption round, per the
  existing discipline.
- **The spec-0026 landing is re-shaped, not blocked.** With `DW0367` at
  `Since`, that engine work merges against the current content pin with no
  lockstep library migration — measured: the only red it causes on the pinned
  corpus today is `DW0367` itself (the other buildable pinned campaign fails
  with exactly its recorded exclusion codes, unchanged). The walk-datum
  backfill remains content work, on its own clock.
- **The superseded ocean placement path stays in the compiler behind the
  fence** — one guarded branch and one constant. Deleting it is a deprecation:
  its own versioned, owner-approved event, never a side effect.
- **Cause-3 rules pay an ordering cost.** An engine change that catches a
  genuine latent defect cannot merge while a pinned campaign is red and cannot
  self-excuse; the content correction and pin move land first. That cost is
  the point — the alternative is a green board over a red flagship.

## 6. Acceptance criteria

1. `crates/grammar`: `Gate` construction requires a binding declaration
   (compile-enforced; no `Default`). The audit JSON serializes each gate's
   binding, and a test asserts every gate id in the library audit reports one.
2. Gate-fence fixture pair: a program declaring a pre-fence version, audited
   by an engine carrying a `since` gate, reports that gate grandfathered with
   its binding count; the same program at the fence's version reds. Both
   asserted by gate id.
3. A CI check enumerates every `DwCode::since` site and every `since` gate
   from source and reds when one lacks its below/at-fence test pair; the
   allowlist is minimal and justified per row.
4. A CI check enumerates every row in `.github/zone-audit-exclusions.json` and
   reds when (a) an exclusion row's `expect_codes` name a code absent from
   `origin/main`'s code inventory, or (b) a change touching `crates/` adds an
   exclusion row. Runs in the required `content-pin` job that runs the
   zone-program audit.
5. `DW0367` is constructed with `Since` at the per-area-datum version. A
   stage-1 fixture at 0.6.0 declaring `horizon: ocean`, over a library without
   the datum, builds exit 0 with the fence line naming `DW0367` as
   grandfathered, and places its area origins at the global ocean datum; the
   same fixture with stage 1 at the datum's version reds `DW0367` when the
   datum is absent and greens when it is present.
6. `nobodys-cave-island` (content main) builds exit 0 on the engine that lands
   spec-0026, against the pinned library, with zero exclusion rows added.
7. `docs/reference/compiler.md` and `docs/reference/grammar.md` state the
   per-layer keys and the gate binding in the same change (the bidirectional
   DW-code check already binds the former).
