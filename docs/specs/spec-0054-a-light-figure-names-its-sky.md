# spec-0054: A light figure names its sky

- **Status**: Proposed
- **Question**: every library prefab carries a lighting record, and nothing can
  re-judge one: the probe that produces it is invoked by nothing, and the
  record's sky, threshold, binding and instrument kind are folded into the
  free-text `method` string — so a hand-written figure committed over a
  measured one reds nothing, and a gate cannot be built because the record
  does not say which quantity its number is. Measured on the library today: a
  fresh probe disagrees with 21 of 36 committed records, and the record format
  cannot distinguish the three different reasons why. This spec gives the
  lighting record a machine-readable measurement context — the sky the figure
  was taken at, the threshold, the binding, the instrument kind, and a content
  hash of the probed blocks — written only by the instrument, so that a
  probe-and-compare gate can decide whether a committed figure is still true.
- **ADRs**: 0004 (prefab metadata is the library's contract surface), 0005
  (two-layer validation — this is the static layer reaching the library's own
  records), 0006 (determinism — the probe is a pure function of blocks, sky
  and threshold, which is what makes exact comparison a legitimate gate)
- **Specs**: 0001 (the lighting contract this record implements), 0007 (the
  admission probe's role), 0010 (the sky model — `effective_sky(time,
  weather)` is the vocabulary this spec freezes figures against, never a
  parallel one), 0027 (the grammar back end exports `unmeasured`, which stays
  correct), 0045 (the fence precedent this spec's cost is priced by: a
  parse-breaking metadata key is a `dsl_version` matter)
- **Non-goals**: the gate's own wiring into content-repo CI (a content-repo
  deliverable; this spec makes it buildable and §5 names the binding it owes).
  Any change to the probe's light model, to `DW0210`/`DW0211`, or to the
  campaign surface `areas[].lighting` (a different type, untouched). Amending
  the spec-0001 lit/dim/dark ladder (§1.4 names a discrepancy; resolving it is
  spec-0001's contract, not this record's). Live-probe tooling.

## 1. The measured ground

Engine measured at `800c958e`; content library at `71ee2120` (its `main`).

### 1.1 The record

`crates/dsl/src/registry.rs::Lighting` (re-exported by `delvewright_schem` and
`delve-admit` — one authority) carries `profile`, `measured_min_light`,
`measured` (a date), `rationale`, `method`. A hand-written `Deserialize`
enforces the one machine rule the record has: a measured profile carries
`measured_min_light` + `measured`, an `unmeasured` one carries neither. The
sky the minimum was taken at, the dark threshold, the binding counts, the
daylight figure and whether the instrument was a live server or the static
estimator exist only as prose inside `method`. A gate on that prose reds on
rewording; a gate on the number alone compares two different quantities — the
committed hand-written records hold daylight figures in the same field where
the probe writes night figures.

### 1.2 The instrument already computes everything the gate needs

`delve-admit lighting` (`crates/admit/src/light.rs`, `main.rs::run_lighting`)
floods the piece at both ends of the engine's sky table and prints a
machine-readable report carrying `profile`, `measured_min_light`,
`min_light_daylight`, `dark_threshold`, `assumed_sky {profile_taken_at,
daylight}` and `binding {standable_cells, entry_cells, measured_cells}`. The
information is computed and printed; it is merely not **recorded** — `--write`
(`meta.rs::set_lighting_from_probe`) folds all of it into the `method`
sentence and keeps only the night minimum as a number.

### 1.3 Nothing invokes the instrument

Zero invocations of `delve-admit lighting` in this repository's workflows and
tools, and zero occurrences of `lighting` in the content repository's
`tools/prefab-audit.py` (its per-PR gate runs `delve-admit audit` only). The
only occurrences are doc lines (`docs/reference/prefab-procedure.md` §9,
`tools.md`, `grammar.md`), and a doc line is not an invocation.

### 1.4 The population, probed

37 prefab documents on the library's `main`; 36 carry a lighting record
(`pools.json` is the pool manifest). Probing every piece with the current
engine and comparing figures: **15 agree, 21 disagree**, in three classes the
record cannot tell apart:

- **9 hand-written day figures in the night field** (`cave-shore`, 4 `hero-*`,
  4 `island-*` open-air pieces): committed `lit/15` with prose like
  *"sky-lit (block-light estimate not applicable)"*; the probe measures
  night minima of 0–4, five of them `dark`. A correction re-probing all nine
  is in flight on the content branch `content/an-open-air-piece-is-measured-now`
  (`87fa767`, unmerged); its nine figures reproduce this spec's probe run
  exactly — an independent cross-check of both.
- **11 figures from retired instruments**: 8 `cave-*` interiors measured by
  the pre-sky source-emission estimator, and 3 `keep-*` pieces whose live-1.21.11
  figures differ by 1–2 from the static estimate — a legitimate
  instrument difference the record has no field to declare.
- **1 unbindable** (`hello-room`): the probe binds zero cells — its way in is
  a jigsaw socket — and refuses (`DW0752`), while the committed record claims
  `lit/8` with an empty `method`.

A discrepancy inside the probe's own contract, named rather than resolved
here: the registry documents `lit` as min ≥ 7 and `dim` as 3–6, while the
probe's verdict is two-way (`dark` below the threshold, else `lit`) — so the
probe writes `lit/3` where the documented ladder says `dim`. The population
carries zero `dim` records. §3.3 scopes `dim` out of instrument verdicts.

## 2. Why the obvious repairs are refused

- **A gate over `method` prose** is a private parse of free text — it reds on
  rewording and passes on a reworded lie.
- **Write-and-diff** (run `--write`, diff the tree) mutates the artifact it
  judges. The gate must probe and compare, never write.
- **Relaxing `Lighting`'s `deny_unknown_fields`** trades the defect for a
  worse one the field's doc comment already names: a misspelled measurement
  key becomes its own silent absence.
- **A sibling top-level field** (tolerated by the document's `extra` map, so
  no fence) splits a measurement from its figures. The context belongs to the
  object it describes — the lighting block — and a shape chosen to dodge a
  version fence is a design decision made by a distribution cost.
- **`unmeasured` as the out for a failing record** is an opt-out the defect
  itself supplies — *nobody measured it* is precisely the finding. The piece
  that genuinely cannot be statically measured already has an honest refusal
  (`DW0752`), and §3.3 gives it an honest record kind instead of an exemption.

## 3. The decision

### 3.1 The surface

The lighting record grows one key, `taken` — the measurement context, written
only by the instrument, one serde shape shared verbatim by the probe's printed
report and the written record so the two cannot drift:

```json
"lighting": {
  "profile": "lit",
  "measured_min_light": 9,
  "measured": "2026-08-22",
  "method": "…prose breadcrumb, unchanged in role…",
  "taken": {
    "kind": "static-estimate",
    "sky": 4,
    "dark_threshold": 3,
    "daylight": { "sky": 15, "min_light": 11 },
    "binding": { "standable_cells": 12, "entry_cells": 3, "measured_cells": 12 },
    "blocks_sha256": "…"
  }
}
```

- `kind`: `static-estimate` | `live-probe` — which instrument took the figure,
  as an enum, deciding what a gate can re-derive.
- `sky`: the effective sky level `measured_min_light` was taken at, as the
  literal the measurement froze (spec-0010's `effective_sky` vocabulary; the
  static estimator writes its night-floor value). A figure whose sky is named
  can be re-read after the sky table moves; one whose sky is implicit is
  silently re-read against a different instrument.
- `dark_threshold`: the threshold the profile verdict used.
- `daylight`: the second figure and its sky. Required for `static-estimate`
  (the probe always computes it when bound); absent for `live-probe`.
- `binding`: the three counts the probe reports. Required for
  `static-estimate`; a figure whose binding is not recorded beside it cannot
  be read afterwards. Absent for `live-probe`.
- `blocks_sha256`: a canonical content hash of the probed blocks — the zone's
  size plus its non-air cells as `(pos, qualified block name)` in sorted
  order, computed by one authority the probe and the gate both call. Canonical
  rather than file bytes, so re-tiling or palette permutation of the same
  building does not fake staleness. This is the field a hand-writer cannot
  supply without running the instrument — which then writes the true figures.

The record does **not** name an engine revision: figures are re-derived
against the pinned engine, and the pin (`admit-ref` in the content
repository) is the instrument's name, held in exactly one place.

### 3.2 Parse rules (extending the existing hand-written `Deserialize`)

1. `taken` is only legal on a measured profile; `unmeasured` with `taken` is
   refused — a claim and its absence cannot both be true.
2. A measured profile **without** `taken` still loads: it is a legacy record,
   and being unverifiable is the gate's finding, not a parse failure —
   otherwise every existing library would stop loading on the day the engine
   moves.
3. With `taken` present, the verdict must be the instrument's:
   `profile == dark` iff `measured_min_light < dark_threshold`, else `lit`.
   `dim` with `taken` is refused (`dim` is a reviewed atmosphere
   classification with a `rationale`, not an instrument verdict; the
   population carries zero).
4. `taken` keeps `deny_unknown_fields`, for the same reason `lighting` does.
5. `kind`-conditional presence per §3.1 (`daylight`/`binding` required for
   `static-estimate`, refused for `live-probe`).

### 3.3 What a gate can now decide (the semantics this surface must support)

Per record, probe-and-compare, mutating nothing: no lighting block → legacy
(existing absence semantics); `unmeasured` → the measurement is owed — a
finding, never an exemption; measured without `taken` → unverifiable, remedy
is one `--write` re-probe; `kind: static-estimate` → recompute the hash and
re-probe: a hash mismatch is a stale piece, a `sky`/`dark_threshold` mismatch
against the current model is a moved instrument, and a figure or profile
mismatch at equal hash, sky and threshold is a record the instrument did not
produce — each red names which input moved, and green requires exact equality,
which determinism (ADR-0006) makes a fair demand; `kind: live-probe` → the
figures are not statically re-derivable, so the gate checks the hash (the
piece has not changed since the measurement) and §3.2's consistency, and
reports static drift as advisory. `hello-room` is the one live case: unbindable
statically (`DW0752`), so it stays red as unverifiable until a live probe
records it, and that red is the truth.

## 4. What it costs

- **A `dsl_version` fence moves.** `Lighting` refuses unknown keys by design,
  so `taken` is a hard parse failure for pre-spec engines — the exact
  precedent spec-0045 names ("a metadata key whose *parse* breaks old engines
  remains a `dsl_version` matter"). The number is handed by the planner at
  dispatch; this document deliberately does not restate a ledger head.
  Released campaigns are untouched: they reproduce via their pinned engine
  and frozen tree and never meet a new record.
- **Migration**: the 36 records re-probe with `--write` under the post-spec
  engine — mechanical and deterministic for 35 of them; `hello-room` cannot
  (§3.3) and is carried as an open finding, not exempted. The in-flight
  open-air correction branch (`87fa767`) writes the pre-spec shape; it lands
  first and is re-emitted, or is superseded by the migration — the planner
  sequences the two, since both rewrite the same nine files.
- **Creators author nothing new**: the block is written by the instrument;
  the only behavioural change is that a hand edit now has a place to be
  caught.

## 5. Acceptance criteria — each stating what would make it vacuous

All eight are owed by the implementation (nothing below exists at
`800c958e`); every claim about current behaviour they rest on is §1's, each
measured against the trees named there before being written down.

1. `--write` on a bound piece produces a record whose `taken` equals the
   printed report's context object field-for-field, asserted on a
   single-template piece and on a tile-set manifest. *Vacuous if* the test
   compares the record to a re-run of the same writer rather than to the
   report, or exercises only the single shape.
2. The parse refusals of §3.2 each have a fixture asserting the refusal and
   its message: `unmeasured`+`taken`; `dim`+`taken`; verdict/figure
   disagreement; a `live-probe` record carrying `binding`; a
   `static-estimate` record lacking `daylight`. *Vacuous if* a fixture fails
   for any other reason — each asserts the named refusal, and each has a
   green twin differing only in the offending key.
3. Every record on the content library's `main` at the migration revision
   loads under the post-spec engine — the legacy-tolerance rule proven
   against the real population, not a fixture. *Vacuous if* proven against a
   fixture directory the test itself writes.
4. A pre-fence engine refuses a record carrying `taken`, and the post-fence
   engine refuses a pre-fence document only where §3.2 says so — the fence
   demonstrated in both directions, the pre-fence half against the pinned
   pre-spec engine named by revision. *Vacuous if* both halves run one
   engine.
5. The canonical block hash is one authority: the probe and the verify path
   call the same function, asserted by test identity on the symbol, and
   re-tiling a fixture zone (same cells, different tile split) leaves the
   hash unchanged while editing one block changes it. *Vacuous if* the
   invariance half is asserted without the sensitivity half.
6. Verify (probe-and-compare) reds a record whose figure the instrument did
   not produce: a fixture with a hand-edited `measured_min_light` at an
   unchanged hash reds naming the figure; the same edit *plus* a matching
   hand-edited verdict still reds. *Vacuous if* the perturbation is one an
   unrelated check catches first — the fixture piece parses, audits and
   loads green apart from the planted figure.
7. Verify separates its three reds: stale piece (edited blocks, hash
   mismatch), moved sky model, and unfaithful figures — three fixtures, each
   asserting its named cause and not the others'. *Vacuous if* one fixture's
   perturbation trips two causes and the test accepts either.
8. Verify's report states its binding — records examined, against the
   population of library documents — and a zero binding is a nonzero exit.
   *Vacuous if* the count is asserted only in the green case.

## 6. Order of work

1. Fence: `dsl_version` reservation (number handed by the planner at
   dispatch), per-stage fence rows, adoption round scheduled for active
   campaigns per the version-adoption rule.
2. `Lighting::taken` + §3.2 rules; the shared context shape moved to where
   the probe's report and `set_lighting_from_probe` both consume it; the
   canonical hash in the same authority.
3. `delve-admit lighting --verify` (probe-and-compare; DW codes handed at
   dispatch, catalog rows and code-asserting tests per the DW-coverage rule).
4. Library migration (content repository, sequenced against `87fa767`).
5. The content-repo binding: verify joins the event that admits a record —
   the per-PR prefab audit — so a hand-written figure cannot merge; that
   wiring is the content repository's PR, named here so the gate is not
   correct-but-UNRUN.
