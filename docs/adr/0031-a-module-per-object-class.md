# ADR-0031: A module per object class — no file that every feature touches

- **Status**: Proposed
- **Date**: 2026-10-08
- **Source**: the rule that a capability belongs to the object class it acts on
  (CLAUDE.md, "general engine"), carried from the DSL surface to the files
  that hold it. Measured against the engine tree at
  `bba876d1351dd2aeba021c95f435961f2421ff27` (`bba876d13`, the commit that
  landed the integration batch before this record) with `wc -l`, with `git diff --name-only` over
  every first-parent landing on `main`, with `git merge-tree --write-tree`
  replayed over every merge commit reachable from a remote ref in the three
  months before that commit, and with the GitHub Actions job and step timings
  of the forty `ci.yml` runs before it. The craft question — how a large compiler organises its source — is
  answered from the rustc developer guide, pages `compiler-src.md`,
  `overview.md` and `diagnostics/error-codes.md` under
  `rustc-dev-guide.rust-lang.org`, read at the pages themselves; every
  quotation below is from those pages. Every other claim is authored here and
  says so where it is not a measurement.
- **Refines**: ADR-0025 (one package: the split is by module inside the one
  engine crate, never by crate), ADR-0006 (every step is proven by the
  emission being byte-identical), spec-0039 (the gallery is where a surface
  is bound; its authoring form changes, its binding does not).

## Context

### What was measured

Seven source files and four documents carry most of the tree's growth, and the
same files carry most of its merge conflicts. Size at `bba876d13`:

| File | Lines |
|------|------:|
| `crates/delvec/src/compiler/emit.rs` | 28043 |
| `crates/delvec/src/compiler/nav.rs` | 18467 |
| `crates/dsl/src/validate.rs` | 9925 |
| `crates/dsl/src/stages.rs` | 9757 |
| `docs/reference/compiler.md` | 8978 |
| `crates/delvec/src/compiler/plan.rs` | 8529 |
| `harness/src/executor.ts` | 8037 |
| `crates/dsl/src/siteplan.rs` | 4533 |
| `crates/delvec/src/main.rs` | 4183 |
| `docs/playtest-findings.json` | 3006 |
| `docs/reference/tools.md` | 2151 |

How often a landing on `main` touched each, over the 146 first-parent landings
since the crate rename that produced the `crates/delvec` path (each landing is
one pull request, squashed or merged; a file is counted once per landing):

| File | Landings of 146 |
|------|---:|
| `docs/reference/compiler.md` | 82 |
| `docs/reference/tools.md` | 77 |
| `crates/delvec/src/compiler/emit.rs` | 38 |
| `docs/playtest-findings.json` | 27 |
| `gallery/quests.json` | 26 |
| `.github/workflows/ci.yml` | 26 |
| `crates/delvec/src/main.rs` | 24 |
| `docs/specs/README.md` | 22 |
| `crates/delvec/src/compiler/nav.rs` | 21 |
| `crates/delvec/src/compiler/plan.rs` | 20 |
| `docs/demo-levels.md` | 19 |
| `gallery/README.md` | 19 |
| `crates/dsl/src/stages.rs` | 16 |
| `crates/dsl/src/validate.rs` | 13 |

How often each file *conflicted*: every merge commit reachable from a remote
ref in the measurement window was replayed with `git merge-tree --write-tree`
over its two parents, and the files the replay reports as conflicted were
counted. 295 merges were replayed; 122 had at least one conflicted file. The counts below
are merges, not hunks:

| File | Conflicted merges of 122 |
|------|---:|
| `gallery/baseline/delta.json` | 64 |
| `docs/reference/compiler.md` | 54 |
| `gallery/baseline/manifests.json` | 48 |
| `gallery/baseline/header.json` | 42 |
| `docs/reference/tools.md` | 37 |
| `docs/specs/README.md` | 32 |
| `docs/demo-levels.md` | 24 |
| `crates/dsl/src/lib.rs` | 20 |
| `gallery/quests.json` | 13 |
| `crates/delvec/tests/remedy_reachability.rs` | 13 |
| `gallery/README.md` | 12 |
| `crates/delvec/src/compiler/emit.rs` | 12 |
| `harness/src/executor.ts` | 11 |
| `crates/dsl/src/stages.rs` | 10 |
| `crates/dsl/src/diagnostic.rs` | 10 |
| `crates/delvec/src/compiler/lethal.rs` | 8 |
| `crates/delvec/src/compiler/plan.rs` | 7 |
| `crates/delvec/src/compiler/nav.rs` | 7 |

Three readings of these tables decide the design:

1. **Documents conflict more than source.** The one reference page, the tool
   inventory, the spec index and the demo queue conflict two to five times as
   often as the largest source file. Each is a single file every feature
   appends to, at the same anchor, so the conflicts carry no semantic content.
2. **The baseline is a different class.** `gallery/baseline/*` is a generated
   artifact; CLAUDE.md already rules that it is reset to one side wholesale and
   regenerated after the merge commit exists, never three-way merged. Its
   conflict count measures how often two branches both moved emission, which a
   file layout cannot change. It is out of scope here and named so.
3. **Size and conflict count disagree.** `siteplan.rs` is the sixth-largest
   source file and conflicted in no merge; `lib.rs` is 150 lines and
   conflicted in twenty, because it holds `pub use` lists that every new type
   is appended to. The hot list is the conflict table, not the size table.

Inside `compiler.md`, the 82 landings were mapped to the section each hunk fell
in at that revision: §4 "World / build output" (29 landings), the diagnostics
catalog §5 (39), the preamble (27 — the version line the publish gate rewrites
on every bump), the stage-5 surface table (15) and the verb→emission table
(17). The whole page is hot, not one section of it.

Inside `emit.rs`: 425 top-level functions; `build_with_warnings` is 2430 lines
and calls into 56 distinct compiler modules; `emit_functions` is 2150 lines and
assembles the `load`/`tick` bodies from 51 per-object helpers; the PackTest
emitters (`emit_*_packtest*`) run from line 16598 to 26170. The file is every
object's emission, every object's PackTest and the orchestration of both, in
one module. `nav.rs` is the same shape for proofs — the voxel `World`, the A*
router, and then one proof per object (checkpoints, timed gates, hazards,
ambushes, shortcuts, lanes, respawn safety, stealth, traps, boundary, sea,
volleys) — 37 DW codes declared in it. `plan.rs` holds the `Plan` struct
(one field per object), the naming scheme, the forced walk, and one
`collect_*` per object. `validate.rs` holds the validation of every object
plus three bundles named for the DSL version that introduced them
(`v03_checks`, `v04_checks`, `v06_checks`), which ADR-0024 made history
narration. `stages.rs` holds the serde type of every object in one file, and
`diagnostic.rs` declares 138 of the DSL crate's codes in one `codes` module.
The tree already has ninety object modules under `compiler/` — `lethal.rs`,
`stake.rs`, `respawn.rs`, `loop.rs`, `cast.rs` — each holding an object's
analysis while its emission, its PackTests and its route proof sit in the
three pass files. The split is half done; the half that is done is the shape
the rest takes.

### The private copies now visible

`StateCompare::holds(i32)` is declared once in the DSL crate. The compiler
re-derives it: `flow::Datum::satisfies` and `loop::GateState::satisfies` are
the same four-arm match over `i64` with an undatable arm, `nav.rs` carries it a
third time as a closure, and `emit.rs` carries the inverse — `state_drive_value`
and `state_candidates`, the value that satisfies or breaks a term — which the
cast-ladder proof evaluates separately. `CompareOp::` is matched in nine
compiler files (`link` 9, `plan` 8, `emit` 7, `loop` 5, `view/beat` 4,
`flow` 4, `cast` 4, `nav` 2). `npc_stands_at` is written twice —
`eclipse.rs` and `pressable.rs` — with different bodies for one question, and
`emit.rs` calls the `pressable` one. Both are the class CLAUDE.md names: a
general mechanism privately re-implemented inside one verb. A file layout does
not create these; a file in which every verb is written side by side hides
them, because a reader never sees the two bodies on one screen.

### CI

Over the last forty `ci.yml` runs, per-job medians: `rust (fmt, clippy,
test)` 25.8 min (its `cargo test` step 19.4 min of a 21.3-min job on `main`),
`gallery` 10.6 min, `tier 2` 8.7, `published crates` 8.9, `harness` 8.0,
`gallery bot` 5.7. The gallery job's maximum was 45.0 min; the four runs above
28 min were all pull-request runs on that batch's integration branch, three of
them red. On `main` the gallery job's steps are: coverage and probes 4.1 min,
every point served 2.1, baseline 1.3, pieces 1.0, views 0.9, whole map 0.4. The
job is one required context, so a red in the probes and a red in the baseline
are one red, found one at a time. The long pole of the whole run is the `rust`
job, not the gallery.

### What the craft record says

The rustc developer guide, on why that compiler is many crates: "The compiler
is a huge codebase; it would be an impossibly large crate." "By breaking the
compiler into multiple crates, we can take better advantage of
incremental/parallel compilation", and "we try to have as few dependencies
between crates as possible". Its shape: "a collection of around 50
interdependent crates ranging in size from tiny to huge"; "Most of the other
`rustc_*` crates depend on `rustc_middle`, which defines a lot of central data
structures in the compiler"; "At the top of the dependency tree is
`rustc_driver` and `rustc_interface`". `rustc_middle`'s own documentation says
its hooks "let you write tcx methods in downstream crates and call them in this
crate, reducing the amount of code that needs to be in this crate (which is
already very big)". On error codes: codes "are defined in the compiler in the
`diagnostics.rs` files found in each crate", every code has one explanation
file, `rustc_error_codes/src/error_codes/E0806.md` for `E0806`, and the
`error_codes!` macro in that crate's `lib.rs` lists them "in its proper
numerical order".

Two of these transfer and one does not. The hub pattern transfers: `Plan` is
this engine's `TyCtxt`, the one record every pass reads, and it is kept thin
the way `rustc_middle` is — each object owns its plan record and the hub holds
one field per object. The one-record-per-code pattern transfers: rustc keeps
one file per code beside a per-crate declaration; this record keeps one page
per declaring module and one row per code in it, with the declaration already
registered at link time (`dw_code!`, `delvec codes`). The unit does not
transfer: rustc splits by crate because its objects are its intermediate
representations and a phase is a crate; ADR-0025 refused crates as boundaries
here because none of them encoded a capability boundary. The unit here is the
module, and the object class is the game object the DSL declares, which is the
boundary the constitution already names.

## Decision

### 1. The rule

**A module belongs to the object class it acts on, never to the pass or the
verb that touches it.** An object is a thing the DSL declares — a wave, a
lethal volume, a timed gate, a shop, an NPC, a trigger — or a thing the
compiler owns in the same way — the voxel world, the forced walk, the naming
scheme. Every object's types, validation, plan record, proofs, emission and
PackTests live in that object's module. A pass module holds only the mechanism
the pass is — the A* router, the `BTreeMap<path, bytes>` output, the batch
model — and the ordered list of object modules it drives. A file over about
1500 lines is a directory of files, split by the same rule one level down.

Two features about different objects therefore edit different files. Two
features about one object edit one object's files, and that is the collision
the design accepts: it is the only kind that carries semantic content.

The lists that remain — `Plan`'s fields, `Command`'s variants, the `Verb` and
`Objective` enums, the ordered emitter and proof lists, `mod.rs` — are
registries. A registry conflict is one adjacent added line; it is the whole
residue, and the design does not try to remove it with link-time registration,
because emission order is a byte of the output and the explicit list is what
keeps that order a fact a reader can see.

### 2. The target trees

#### `crates/dsl/src/`

```
lib.rs            pub mod lines, and one `pub use <object>::*;` per module whose
                  items are in the root namespace — no per-type re-export list
                  anywhere
envelope.rs       Campaign and the stage documents (unchanged)
diagnostic.rs     Diagnostic, DwCode, ExitTier, Subject, DECLARED, dw_code!
                  — the mechanism only; the `codes` module is dissolved
ids.rs            the id grammar, dup_check (unchanged)
validate/mod.rs   validate_campaign_with: the ordered list of object checks,
                  AnchorProviders, and nothing that is about one object
world.rs          WorldContent, Horizon*, Boundary, Area, AreaLighting, Pieces,
                  TextureOverride, Atmosphere*, Climate, RespawnWait, WorldTime
                  + world_checks, horizon_param_checks, lighting_range_checks,
                  texture_checks and the codes they raise
body.rs           Locomotion, BodyTraversal, BodyRef, Body*Site + body_traversal_checks
npc.rs            NpcsContent, Npc, NpcSkin, Persona + deferred_npc_checks, despawned_ref_check
dialogue.rs       DialogueContent, NpcDialogue, DialogueNode/Option/Effect + dialogue checks
class.rs          ClassesContent, Class, KitItem, Carrier, PotionContents + kit item,
                  kit potion, flask and item-gate checks
quest_plan.rs     QuestPlanContent, BranchPoint, BranchDecl, Happening, PlannedQuest
                  + plan, partition, mainline_key, branch_point_checks, happening_subject_checks
quest/mod.rs      QuestsContent, Quest, Trigger, Visibility, Guidance + references,
                  after_ordering, cross_stage
quest/objective.rs Objective
quest/verb.rs     Verb, ParticleAt, SoundAt, NarrateStyle, the volley and collapse
                  defaults
quest/effect.rs   QuestEffect, Guard, EffectAudience, NestedDispatch, BonfireLabels
state.rs          StateDecl, StateScope, StateDisplay, CompareOp, StateWrite,
                  StateCompare + state_checks, read_after_write_checks,
                  gate_contradiction_checks — and the one evaluator (§3)
wave.rs           Wave, WaveMob, WaveLane, WaveSummon, MobEquipment, EquipSlot,
                  MobDrop, MobAttributes, EncounterTier + lane, difficulty,
                  drop and equipment checks
onkill.rs         OnKill, KillFires + on_kill_checks (an existing object module)
actor.rs          Actor, Facing, DespawnStyle, SequenceStep
                  + the actor shape checks now in v06_checks
cutscene.rs       CameraShot, ShotStyle, CameraSubject, CutsceneParty + the
                  cutscene shape checks now in v06_checks
mark.rs           Mark, offset_cell
stealth.rs        StealthZone
effects.rs        the effect roots, EffectSite, for_each_campaign_effect
serde_fields.rs   the serde defaults and skip predicates the stage types share
                  (private)
trigger.rs        EnvTrigger, TriggerOn, TriggerAudience, Prop + press_answer_checks,
                  press_obligation_checks
trap.rs           Trap, TrapTrigger, TrapEffect, Lethality, TrapReset, TrapDisarm + trap checks
timed_gate.rs     TimedGate, TimedGateDisarm + timed_gate_checks
ambush.rs         Ambush + ambush_checks
shortcut.rs       Shortcut + shortcut_checks
loot.rs           Loot, LootItem + loot_checks, collect_container_claim_checks
assembly.rs       Assembly, AssemblyHitbox, AssemblyStrikes, Strike* + assembly_checks, lock_shape_checks
cast.rs           CastEntry, CastAbsence, CastPlace, CastPlacement, CastDialogue, CastBarks
lethal.rs         LethalVolume, DamageKind + lethal_volume_checks, lethal_stage_checks
loop.rs           the loop + loop_checks
economy.rs        Shop, Stake, Purchase (absorbs purchase.rs) + economy_checks, shop_anchor_checks
world_edits.rs    the edit stage's types + world_edits_checks
siteplan/         mod.rs (SitePlanContent, PlanBox, Seam, Volume, Identity, …),
                  place.rs, pack.rs, seam.rs, region.rs, measure.rs, check.rs
```

`gate.rs`, `healthbar.rs`, `onkill.rs`, `fight.rs`, `equipment.rs`,
`firework.rs`, `lightning.rs`, `perception.rs`, `effects.rs`, `placement.rs`,
`layout.rs`, `detailplan.rs`, `l10n.rs`, `metrics.rs`, `chrome.rs` and the
registries are already object modules holding their own types and checks;
they are the shape, and they stay. The three version-named bundles are
dissolved into the objects they check. `uniqueness` and `references` are
dissolved the same way: each collection checks its own ids and resolves its
own names. The 138 codes in `diagnostic::codes` are declared in the module
that raises them, as `admit/diag.rs`, `view/diag.rs` and `schem/diag.rs`
already do; `check-dw-codes.py` resolves a constant per module and needs no
change for that.

#### `crates/delvec/src/compiler/`

```
mod.rs            pub mod lines; DELVEC_VERSION, MC_VERSION, PACK_FORMAT, DATA_VERSION
plan/mod.rs       Plan<'a> — one field per object — PlanError, AnchorTable, and the
                  constructor that calls each object's plan() in order
plan/naming.rs    safe_local, the *_score / *_tag / marker_line scheme
plan/path.rs      CriticalPath, build_critical_path, Step, PathFiring, path_triggers,
                  region events, strict ancestor steps — the forced walk
plan/anchors.rs   resolve_anchor, required_anchors_for_area, collect_furniture,
                  AnchorScope, AnchorHit
plan/area.rs      AreaPlacement, PiecePlacement, PlacedTemplate, area_base_y
emit/mod.rs       build, build_with_warnings — the ordered list of object emitters
                  and object proofs it drives — BuildFailure, BuildOutput
emit/text.rs      tr, snbt_*, json_bytes, fmt_f64, lines, put_json, sha256_hex
emit/functions.rs the load / tick / init bodies assembled from each object's lines
emit/manifest.rs  emit_manifest, emit_critical_path, critical_path_json
emit/server.rs    emit_server, lang_assets, the resource pack
emit/packtest.rs  packtest_header, pin_dummy, packtest_preamble, packtest_guards
                  — the batch model, and nothing about one object
nav/mod.rs        the ordered list of object proofs over a World, and the DW codes
                  of the world, the router and, until Phase C, every object proof
nav/world/mod.rs  World, Cells, Premises, the derived worlds, Ambient, Sea,
                  built_volume, Liveness, StagedVolume
nav/world/body.rs Footprint, entity_dims, standability, the move model, step
                  costs, cell_center, walk_cells
nav/route/mod.rs  the A* router, visited positions, critical_route_cells,
                  LegRoute, Configuration
nav/route/region.rs RegionState, the region state a leg is routed under, blame
nav/route/leg.rs  judge_leg, decompose, route_with_links, RouteBinding
nav/<object>.rs   one object's proofs over the World, and their tests
nav/testkit.rs    the synthetic worlds the tests of more than one object share
<object>/mod.rs       the object's plan record (XPlan) and plan(campaign) -> Vec<XPlan>
<object>/check.rs     the object's proofs — what nav.rs and the object's own
                      file hold today; its DW codes are declared here
<object>/emit.rs      the object's setup and tick lines, functions, advancements,
                      dialogs, loot tables
<object>/packtest.rs  the object's PackTests and watch claims
```

The objects, named by what the three pass files already cluster by:
`objective`, `quest`, `dialogue`, `npc`, `actor`, `cutscene`, `sequence`,
`wave`, `class`, `checkpoint` (bonfire, respawn wait, party wipe, mend),
`timed_gate`, `shortcut`, `seal`, `stake`, `economy` (shop), `state`,
`lethal`, `loop`, `stealth`, `trap` (gate hardware, payloads, volley,
collapse), `loot`, `trigger` (env, step, press), `boundary` (playable region,
night vision), `atmosphere`, `teleport`, `branch`, `cast`, `healthbar`,
`onkill`, `lightning`, `firework`, `standin`, `assembly`, `link`, `hazard`,
`lane`, `respawn`, `sea` (fluid escape, seepage, ocean window). An existing
`lethal.rs` becomes `lethal/check.rs`; nothing it does moves out of it. A
sibling reaches another object through `crate::compiler::<object>`, with
`pub(crate)` visibility; nothing is exported from the crate that is not today.

#### `crates/delvec/src/main.rs`

```
main.rs           Cli, Command (one variant per subcommand), main() — the
                  dispatch and the exit-code table, nothing else
cli/campaign.rs   validate, analyze, build, textures: load_or_refuse, validate_loaded,
                  run_build, resolve_build_kind, read_structures, read_skins
cli/l10n.rs       l10n inventory, l10n apply
cli/document.rs   fmt, schema, codes, allocation
cli/metrics.rs    metrics, calibrate, rig
cli/report.rs     print_diags, print_one_diag, print_build_error, report_binding_notes,
                  write_file
cli/view.rs       snapshot, cameras --preview, the camera stand check, blocking-chart
cli/edit.rs       edit apply, edit preview
```

`cli/` is a module of the binary, as `detail.rs` is. The view and edit arms
are `cli/view.rs` and `cli/edit.rs`, beside the binary's loader they call;
the library's `compiler/view/cli.rs` keeps the render arms and takes the
binary's stand check as a closure. `tools/lib/clap_surface.py` reads `main.rs`
first and then every crate source, so `Command` staying in `main.rs` is what
keeps the one parser's first-enum rule true.

#### `harness/src/executor.ts`

The same rule, in the harness: `harness/src/executor/<object>.ts`, one file per
verb object the bot performs, and `executor.ts` the dispatch. It is listed
because it was measured (11 conflicted merges); its step is the smallest and
is sized in §7.

### 3. How the one-authority rules survive

| Rule | Where the one authority lives after the split |
|------|------|
| one markdown parse | `tools/lib/mdtable.py`, unchanged; a reader of a directory concatenates the rows of every page through it, and a row no table contains is still a finding naming its file and line |
| one DW catalog reader | `tools/lib/dwcatalog.py`: `catalog_pages`, `catalog_rows`, `documented_codes`, `mentioned_codes`, `module_of`, `page_for(crate, module)`, moved out of `check-dw-codes.py`; read by `check-dw-codes`, `staging-gate` and `check-reference-versions`, and their tests (`check-diagnostic-messages`, `check-skill-page` and `check-numbered-doc-uniqueness` never read the catalog: the first two name `check-dw-codes` in prose and the skill-page gate reads *declarations* through its `CONST_RE`) |
| one declaration per code | `dw_code!` in the module that raises it, registered in `DECLARED`, printed by `delvec codes` — unchanged; what changes is that the declaring module is now the object's |
| one record per code | one catalog row, in the page whose path is the declaring module's path (§4); `check-dw-codes.py` holds row-in-page by the registry's `module` field, so a code whose declaration moves and whose row does not is a red |
| one test per code | unchanged; resolution is already per module |
| one findings reader | `tools/lib/findings.py`: `load_ledger` moved out of `staging-gate.py`, over a directory; read by `staging-gate`, `check-gallery-stageable`, `check-json-canonical` and the tests |
| one demo-queue reader | `check-demo-levels.py`'s `parse_queue`, over a directory, through `mdtable` |
| one numbered index | generated by one writer (`tools/planner/next-numbered-doc.py --index --write`) from each document's own `Status:` line; `check-numbered-doc-index.py` verifies the committed index equals the generated one |
| one clap surface | `tools/lib/clap_surface.py`, reading a declaration at every visibility through `tools/lib/rust_source.py` |
| one gallery point | `tools/ci/gallery_domain.py::materialise`, which gains the assembly of a stage document from its object files (§4) |
| one text authority | `dsl::chrome` for every compiler-written string, unchanged; for prose, one page per module and a cross-reference links rather than restates |
| one state evaluator | `dsl::state::StateCompare`: `holds(i32)`, `holds_datum(Option<i64>) -> Option<bool>` (the undatable arm, from `flow` and `loop`), `witness(satisfy: bool) -> i32` (the drive value, from `emit`); `flow::Datum::satisfies`, `loop::GateState::satisfies`, the `nav` closure and `emit`'s two tables are deleted and call it; `plan::state_terms_of`, `link`, `cast` and `view::beat` are enumerated by the same step and each either calls it or is recorded as rendering an op rather than evaluating it |
| one stand-anchor question | `compiler::npc::stands_at(plan, anchor)`; `eclipse.rs` and `pressable.rs` call it, and the two bodies are reconciled in that step with a test on the case where they disagreed (a deferred NPC) |

A fourth private-copy shape becomes visible only after the split: two object
modules holding byte-identical function bodies. `check-source-dupes.py`
measured 3930 false positives for a raw three-line window and declined it;
a function-body rule (two `fn` items in different modules whose bodies are
identical after whitespace normalisation) is a different unit and is measured
by the step that proposes it before it is adopted. Its false-positive count is
not known and is not claimed here.

### 4. How the documents split

**The page for a module is at the module's path.** `docs/reference/compiler.md`
keeps §1 (pipeline, pass order, CLI contract, exit tiers), the cross-object
invariants of §4 (determinism, the `@s` rule, the completion-marker channel,
the PackTest batch model, sealing) and §6. Everything about one object — its
stage-table rows from §2, its verb rows from §3, its paragraphs of §4 "World /
build output", its catalog rows and essays from §5 — moves to one page:

```
docs/reference/delvec/compiler/<object>.md     mirrors crates/delvec/src/compiler/<object>/
docs/reference/delvec/compiler/nav.md          the world and the router
docs/reference/delvec/compiler/plan.md         the hub, the naming scheme, the forced walk
docs/reference/delvec/compiler/emit.md         the output model, text primitives, manifest
docs/reference/delvec/admit/diag.md            as today's module, at its path
docs/reference/dsl/<object>.md                 mirrors crates/dsl/src/<object>.rs
```

The mapping is one function: strip the crate root, replace `::` with `/`,
append `.md`, under `docs/reference/<crate directory>/` (`dsl`, `delvec`); a
crate root's page is `docs/reference/<crate>.md`, beside the directory, as
`foo.rs` sits beside `foo/`. The module is the *file's* module: a code declared
in an inline module (`dsl::diagnostic`'s `pub mod codes { … }`) has its row on
the file's page, `dsl/diagnostic.md`. `check-dw-codes.py` applies the mapping
to the declaring module it already computes from the source for every constant
(`module_of`), cross-checked against the registry's `module` field when
`--delvec` is given — that field is `module_path!()` and so also names the
inline module and spells raw identifiers (`compiler::r#loop`); it agrees when
it is the file's module followed only by inline modules that file declares —
and requires the code's catalog row to be in that page, so
the docs split follows the code split one PR at a time without a plan of its
own: when a code's declaration moves from `compiler::nav` to
`compiler::timed_gate::check`, its row moves from `nav.md` to
`timed_gate.md` in the same PR or the gate reds. A page may say more than its
rows; nothing on it may be a row for another module's code, and a module page
that mirrors no source file is a red, so a page moves or goes with its module.
Where one §5 section held rows of several modules, its prose goes to the page
of the module holding most of its rows, and each other module's page carries
its own rows under the same heading with a link to that prose. `check-doc-dupes`
already runs over `docs/**/*.md` and needs no change. The exit-tier table in
§1 is a census derivable from `delvec codes` and is deleted; the gate already
holds the registry equal to the declarations by tier.

**The demo queue** becomes `docs/demo-levels/<spec-or-object>.md`, one small
table per mechanic with today's three columns, and `docs/demo-levels/README.md`
holding the rules of the queue. `check-demo-levels.py` reads every page in the
directory through `mdtable`, concatenates the rows, and keeps every assertion
it makes today, including "parsed zero rows is a failure".

**The findings ledger** becomes `docs/playtest-findings/<id>.json`, one
finding per file, with `docs/playtest-findings/ledger.json` carrying
`ledger_version` and `docs/playtest-findings/README.md` carrying the rules
now in `_readme`. "Append, never rewrite" is then a new file, which is the one
edit git never conflicts on. `check-json-canonical.py` already sweeps JSON
under `docs/`; the ledger glob in `.github/ci-reach.toml` becomes the
directory.

**The spec and ADR indexes** are generated. Each document's own `Status:` line
was already the authority and the table a copy the checker compared; the copy
is now written by the tool and the checker compares the committed table to the
generated one, which makes it a generated artifact under the rule that already
governs the baseline.

**The tool inventory** becomes `docs/reference/tools/<surface>.md`: one page
per `delvec` surface (`compiler`, `schem`, `grammar`, `sculpt`, `prefab`,
`render`, `chunky`, `cameras`, `harvest`), one per `tools/` directory
(`creator`, `ci`, `planner`, `maintenance`, `lib`), and one each for
`validation`, `harness` and the generators; `tools.md` keeps the preamble and
the class vocabulary. `check-stated-counts.py`'s `SITES` row for the idiom
count moves to the page that states it; `check-demo-levels.py` reads the
agent/human class off the directory.

**The gallery** keeps one quests document for the compiler and changes how it
is authored: `gallery/quests/<object>.json` holds one top-level array each —
`triggers`, `waves`, `actors`, `traps`, `state`, … — as `{"why": "…",
"items": [...]}`, and `gallery_domain.materialise` assembles `quests.json`
from them, arrays in the schema's order, items in file order. The committed
`gallery/quests.json` is deleted; every point, probe and overlay already reads
a materialised directory, and probe patch paths such as
`/content/quests/1/...` address the assembled document unchanged. The README's
"what it holds" cell for `quests.json` dissolves into each file's `why`. The
other stage documents stay whole: none of them was measured hot. This is an
authoring form of the engine's own test campaign, not a DSL surface: the
compiler receives one `quests.json` exactly as it does today.

### 5. How every checker follows

| Checker | Reads today | Reads after | Through |
|---------|-------------|-------------|---------|
| `check-dw-codes.py` | `compiler.md` §5 | `compiler.md` and every page under `docs/reference/{delvec,dsl}/` | `dwcatalog.py`; gains row-in-page |
| `staging-gate.py` | `compiler.md`, `playtest-findings.json` | the catalog pages, the findings directory | `dwcatalog.py`, `findings.py` |
| `check-gallery-stageable.py` | the findings ledger via `staging-gate` | the findings directory | `findings.py` |
| `check-reference-versions.py` | `compiler.md`'s version line and the `DW0102` row | the version line in `compiler.md`; the `DW0102` row on whichever catalog page holds it (`dsl/diagnostic.md` until B2 moves the declaration to the envelope); `--write` writes the row where it is | `dwcatalog.py` |
| `check-stated-counts.py` | `SITES` rows naming `tools.md`, `grammar.md`, `compiler.md` | the same oracles; `SITES` paths updated (the DW02xx emitter-table counts are on `delvec/compiler/analyze.md`) | unchanged mechanism |
| `check-diagnostic-messages.py`, `check-numbered-doc-uniqueness.py` | no catalog read | unchanged | unchanged |
| `check-skill-page.py` | `stages.rs` at the pinned engine; no catalog read | the one declaration of `WorldContent` under `crates/dsl/src/` at the pinned engine, refused when declared twice | a declaration search, so an engine pinned before and after B1 both read |
| `check-doc-dupes.py` | `docs/**/*.md` | unchanged | unchanged |
| `check-demo-levels.py` | `demo-levels.md` | `docs/demo-levels/*.md` | `mdtable.py` |
| `check-numbered-doc-index.py` | the two index tables | the same tables, compared to the generator's output | the writer |
| `check-json-canonical.py` | the ledger among its sweep | the ledger directory | its existing sweep |
| `check-capability-ownership.py` | `STAGES = crates/dsl/src/stages.rs` | the stage-surface modules, a registry (`DSL_STAGE_MODULES`); its per-file `HAPPENING_NONE_ALLOWED` entries name the new files | the registry |
| `check-anchor-providers.py` | `validate.rs` as the one site of the broad question | `validate/mod.rs` | its file rule, renamed |
| `check-effect-roots.py` | `compiler/plan.rs` in `ALLOWED` | `compiler/plan/anchors.rs` | its ledger, renamed |
| `check-structure-emitters.py` | per-file ledger | the same files at their new paths | its ledger, renamed |
| `check-source-dupes.py` | `crates/**/*.rs` | unchanged | unchanged |
| `tools/lib/clap_surface.py` | `main.rs` then every crate source | unchanged | a declaration at every visibility, through `rust_source.py` |
| `tools/lib/version_sites.py` | `compiler.md` as two version sites (the header and the `DW0102` row) | `compiler.md` as one (the header) and `dsl/diagnostic.md` as one (the `DW0102` row) | its rows |
| `gallery_domain.py`, `gallery-build.py`, `gallery-baseline.py`, `check-gallery-coverage.py` | `gallery/*.json` | the assembled point | `materialise` |

A checker that holds a per-file ledger (`check-capability-ownership`,
`check-effect-roots`, `check-structure-emitters`, `check-anchor-providers`)
is renamed to the new path in the step that moves the file, and that step
plants the defect the ledger guards to show it still reds — the perturbation
rule of CLAUDE.md, applied to a path rename.

### 6. CI

The gallery job splits into four required contexts, each a job with its own
`name:`; the pieces are generated once and handed down as an artifact, the way
`delvec-binary` already hands the engine down:

| Today's step | New job (`name:`) |
|--------------|-------------------|
| generate the gallery's pieces and skins | `gallery pieces (generator output)` |
| the gallery analyzes green and builds; every declared surface is written or proven refused | `gallery coverage (every surface written or refused)` |
| every declared view is produced; the seating answer is the build answer; the whole-map illustration path; the cameras stand where the record says | `gallery views (render plan, whole map, cameras)` |
| emission baseline + expected-warnings ledger; the walked whole rebuilds from a committed act; every point the ladder builds can be served | `gallery baseline (emission, warnings, served points)` |

No step is dropped, merged with a weaker one, or made advisory: the table is
the whole job, and each new job carries its steps' `if: '!cancelled()'`
chaining as today. `gallery bot` and `tier 2` are unchanged. The four names
enter `.github/required-status-checks.txt` and the old one leaves it in the
same PR, under the file's own three-part rename procedure (protect the new
contexts, merge, drop the old), with `check-required-contexts.py` holding the
file and `ci.yml` in lockstep in both directions as it does now;
`.github/ci-reach.toml` gains one group per job, held by `check-ci-reach.py`.
What this buys is measured, not estimated: on `main` the critical path of the
gallery drops from 10.6 min to the longest of its parts (coverage, 4.1 min
plus pieces), and a red in one class is a red in one context.

The `rust` job is the run's long pole at 25.8 min median, 19.4 of them `cargo
test`. The same split applies — `rust (fmt, clippy)` and the test shards by
crate, the determinism double-build staying in one shard — but `cargo test`
has no partitioning of integration tests across jobs, so a shard of the 239
`delvec` test files is either by name list (a registry) or by an instrument
that partitions (`cargo-nextest`, a pin and an attribution entry). That is a
measured step of its own and is not decided here.

### 7. Migration order

Every step is one pull request that leaves the tree green and the emission
byte-identical. The proof is the one the tree already has:
`tools/ci/gallery-baseline.py` without `--write` compares every point's
`manifest.json` — the compiler's SHA-256 index over every input and output
path — to the committed baseline, for all nine points, and the cross-OS
determinism job compares bytes across hosts; `cargo test --workspace` and
`clippy -D warnings` run as today. The one step that changes a baseline field
is A6, which moves `gallery_source_sha256` by changing the gallery's source
tree while every point's manifest stays; the baseline tool classifies that
mismatch itself. A step is sized so one worker finishes it in a few hours;
where a file is too large for that, the step is cut by object group and the
cuts are sequential because they delete from one file.

**Phase A — documents** (all six parallel; each half a day):

| Step | What moves | Gate that proves it |
|------|-----------|---------------------|
| A1 | `compiler.md` §5's rows and their sections → `docs/reference/{delvec,dsl}/<module>.md` at today's module paths; `dwcatalog.py`; the three readers; row-in-page added to `check-dw-codes` | `check-dw-codes` bidirectional and row-in-page green; `check-doc-dupes`; `staging-gate` tests |
| A2 | the findings ledger → one file per finding; `findings.py`; the three readers; `ci-reach.toml` | `test_staging_gate`, `check-gallery-stageable`, `check-json-canonical` |
| A3 | the demo queue → `docs/demo-levels/`; `check-demo-levels` over a directory | its tests; the zero-rows refusal still fires on an empty directory |
| A4 | the two index tables generated; the writer; the checker compares | `check-numbered-doc-index` on a planted stale row |
| A5 | `tools.md` → `docs/reference/tools/`; `SITES`; the class reader | `check-stated-counts` binding counts non-zero; `check-demo-levels` |
| A6 | `gallery/quests.json` → `gallery/quests/<object>.json`; `materialise` assembles; README cells → `why` | every point's manifest identical; coverage 1037 bound, 7 refusal-proven, 0 unaccounted, unchanged |

A1 moves §5 only. At today's module paths the other per-object prose has no
object page to go to — every §2 stage-table row documents `dsl::stages`, every
§3 verb row and most §4 "World / build output" paragraphs document
`compiler::emit` — so moving it in A1 would move it twice. It moves to its
object's page in the step that creates that object's module (B1 for the §2
rows, Phase C for §3 and §4). §1's exit-tier table stays where it is; no step
here deletes it.

**Phase B — dissolve each pass file** (B1, B3, B4, B5, B6, B7 parallel: they
are different files and meet only at one `pub mod` line each; B2 after B1):

| Step | What moves | Size |
|------|-----------|------|
| B1 | `stages.rs` → the DSL object modules of §2; `lib.rs` keeps one glob per module and no type list | one worker, mechanical; call sites that named `dsl::stages::` or a root alias change to the root or module path |
| B2a–c | `validate.rs` → each object module's checks, in three sequential cuts (world/body/npc/dialogue/class; quest/state/wave/actor/trigger; trap/timed gate/ambush/shortcut/loot/assembly/lethal/loop/economy); the version-named bundles dissolved; `diagnostic::codes` declarations move with their checks; the A1 row-in-page rule moves their catalog rows | three PRs, a few hours each |
| B3a | `emit.rs` PackTest emitters (lines 16598–26170) → `emit/packtest/<object>.rs`, `emit/packtest.rs` the batch model | one PR |
| B3b | the rest of `emit.rs` → `emit/<object>.rs` by its own section markers, `emit/{mod,text,functions,manifest,server}.rs` | one PR, after B3a |
| B4a | `nav.rs` → `nav/world/`, `nav/route/` and `nav/mod.rs` | one PR |
| B4b | the per-object proofs → `nav/<object>.rs` | one PR, after B4a |
| B5 | `plan.rs` → `plan/{mod,naming,path,anchors,area}.rs` and `plan/<object>.rs` | one PR |
| B6 | `main.rs` → `cli/*.rs`, the view and edit arms included | one PR; `clap_surface.py`'s own test, its surface compared before and after, and every `--help` of the built binary prove the surface unchanged |
| B7 | `siteplan.rs` → `siteplan/` | one PR; lowest priority — not measured as conflicting |
| B8 | `harness/src/executor.ts` → `executor/<object>.ts` | one PR; the harness job |

**What B1 found** (corrections to this record, made where B1 touched it):

- `quest.rs` measured about 3300 lines, so it is the directory `quest/` under the
  1500-line rule; the types the list above did not place (`Mark`, `StealthZone`,
  the camera shot, `EffectSite`, the serde helpers) have the modules named in §2.
- One glob per module is one glob per module *in the root namespace*. The
  modules reached by path (`blocks`, `blockshape`, `color`, `fluid`, `fmt`,
  `license`, `lightning`, `metrics`, `perception`, `rig`, `split`,
  `viewdistance`) take none: globbing them makes nine names ambiguous, `Body`
  among them, and an ambiguous root name is a removed one.
- A root alias is a per-item list. `l10n_inventory`, `l10n_plain` and
  `l10n_untag` are dropped; their callers name `l10n::inventory`, `l10n::plain`
  and `l10n::untag`.
- `detailplan::owed_anchors` was a forwarding second home of
  `siteplan::owed_anchors` and collided with it under the globs; it is deleted.
- Reading every file under `crates/dsl/src/` in `check-capability-ownership`
  widens checks C and D past the stage surface and reds on seven matches no
  ledger holds (`Gate`/`Guard`, `ArtNarrate`/`OptionLabel`, `Body`/`Opening`,
  `Campaign`/`RawCampaign`, and `Edge.gating`, `Edge.one_way`,
  `Edge.shortcut`). B1 keeps the population to the stage modules; widening it
  is its own step, which triages those seven.
- `purchase.rs` carries checks, so its absorption into `economy.rs` is B2's.

**What B2a found** (corrections to this record, made where B2a touched it):

- B2a renames `validate.rs` to `validate/mod.rs`, so the later cuts delete from
  the file's final path. `check-anchor-providers.py` names its authority by
  path relative to `crates/dsl/src` (`validate/mod.rs`), never by bare file
  name: `mod.rs` is a name many files hold.
- Diagnostic order is observable — `delvec` stable-sorts by group, so within a
  group the order checks run in is the order a creator reads. A block inside a
  bundle (`v04_checks`'s NPC skins, `v06_checks`'s flask rule,
  `anchors_and_items`'s NPC stations and kit items) therefore moves as a
  function of its object called from the position the block held; the bundle
  is dissolved when its last block has left.
- `body_traversal_checks` was in no cut; it is the body's and goes with B2a.
  `prefab_binding` (an area binds one piece or one pool) is the world's.
- The enchantment and equipment checks read item stacks — a wave mob's or an
  actor's `equipment`, a `loot` stack, a `give-item` — and no class field, so
  they are not `class.rs`'s; each goes with the object whose stack it reads, in
  B2b and B2c.
- `syntax`, `uniqueness`, `references` and `envelope` are each one pass over
  every collection. Splitting them by cut would make all three cuts edit the
  same four functions, so they stay whole in `validate/mod.rs` until B2c
  dissolves them once every object module exists.
- A code moves when every raise site in the DSL crate is in one module, and the
  compiler's uses follow it (`compiler::plan` names `world::HORIZON_PARAM` and
  `world::SURROUND_NO_REGION`, `compiler::textures` names
  `world::TEXTURE_PATH`). A code still raised from two DSL modules (`DW0112`,
  `DW0142`, `DW0143`, `DW0172`, `DW0190`, `DW0196`) stays in
  `diagnostic::codes` until a later cut leaves it one raiser. `DW0741` is
  raised in the DSL only by a texture row, but it is the licence rule's code —
  `delvec prefab` raises it for every asset it admits — so its object is
  `dsl::license`, not the world, and it stays where it is.
- Helpers the moved checks share with checks still in `validate/mod.rs`
  (`AnchorProviders`, `station_kind_diag`, `for_each_effect_deep`,
  `for_each_trigger_effect_deep`, `quest_ancestors`) stay there, `pub(crate)`,
  until the cut that moves their last caller.
- `purchase.rs` → `economy.rs` belongs to B2c, the cut that holds `economy`.

**What B2b found** (corrections to this record, made where B2b touched it):

- `quest/mod.rs` with the quest's checks would pass 1500 lines, so they are
  `quest/check.rs` (`dsl::quest::check`, page `dsl/quest/check.md`), split one
  level down by the §1 rule. It is a `pub` module: the codes it declares were
  public in `diagnostic::codes` and stay public.
- `cross_stage` is two checks: a planned quest against its expansion is the
  quest's; an NPC against its dialogue tree is the dialogue's
  (`dialogue::npc_tree_checks`).
- The verb rules that no object module owns — `give-effect`/`clear-effect`
  (`DW0540`/`DW0541`), the `sequence` nesting rule (`DW0329`), a
  `carrier: "one"` give (`DW0357`), `give-item` enchantments and the per-effect
  block/NPC/cutscene references — go with the quest's checks, beside
  `quest/verb.rs`, where those verbs are declared. `firework_checks` and
  `perception_checks`, in no cut, go to the existing `firework` and `perception`
  modules; the cutscene's shape and shot-style checks to `cutscene`.
- `v06_checks` walks a quest's or a trigger's bundle once for actor references
  and the `sequence` nesting rule together, so that walk is one function of the
  actor (`actor::actor_checks`) that calls `quest::check::check_no_nested_sequence`;
  splitting it would reorder diagnostics within a group.
- Equipment is the wave's type (`MobEquipment`), so `check_equipment` and the
  drops checks are `wave.rs`'s and the actor calls them. `check_enchantments`
  reads an item stack — an equipped piece, a `give-item`, a `loot` stack — and
  no object module holds the item stack, so it stays in `validate/mod.rs` with
  `DW0433`/`DW0434` in `diagnostic::codes` until B2c moves `loot`, its last
  other caller.
- Codes raised from two modules after this cut stay in `diagnostic::codes`:
  `DW0110`–`DW0112`, `DW0142`, `DW0143`, `DW0170`, `DW0172`, `DW0173`,
  `DW0190`, `DW0192`, `DW0196`, `DW0432`, `DW0500`, `DW0953`. Codes declared in
  `diagnostic::codes` but raised from one object module outside the three cuts
  (`equipment`, `purchase`, `healthbar`, `onkill`, `celestial`, `l10n`,
  `chrome`, `viewdistance`) are in no B2 step; dissolving `diagnostic::codes`
  needs a step that moves them.
- `world_edits_checks` is in no cut; B2c takes it with `split_blockstate`, the
  last caller of which it is.
- A catalog section whose rows split keeps its prose on the page holding most
  of its rows (§4): `DW050x` moves to `dsl/state.md` with `DW0500`'s row left
  on `dsl/diagnostic.md` under a pointer, and the status-effect section
  (`DW0540`–`DW0545`) to `dsl/quest/check.md`, where two of its four rows are.
- A private copy the split made visible: two inventories of "which flags
  exist". `produced_flags` (`set-flag`, dialogue, trap disarm) answers
  `DW0172` for objectives, effects and branch points; `collect_declared_flags`
  (the same, plus a timed gate's disarm) answers it for triggers, dialogue and
  traps. A flag only a timed-gate disarm sets is therefore refused on an
  objective and accepted on a trigger's own `requires_flags` (reproduced on the
  gallery: `flag/gate-jammed` planted on both raises one `DW0172`, at the
  objective). B2b moves both unchanged; the merge is its own step, proven by a
  test that plants that flag on both sites.

**What B2c found** (corrections to this record, made where B2c touched it):

- `validate/mod.rs` now holds the driver, `AnchorProviders`, `station_kind_diag`
  and the helpers two or more object modules call (`declares_bonfire`,
  `declares_checkpoint`, the three deep effect walks, `split_blockstate`,
  `check_block_field`, `collect_declared_flags`, `produced_flags`,
  `quest_ancestors`, `graph_has_cycle`). A helper with one object caller went
  to that object (`declared_endings` to `quest_plan`).
- `syntax`, `uniqueness` and `references` are each one function per collection
  (`<object>_id_syntax`, `<object>_id_uniqueness`, `<object>_dangling_refs`),
  called from the driver in the order the pass walked them, so the three passes
  are three runs of calls rather than one call per object. The rule each applies
  is the id grammar's and lives in `ids.rs`: the syntax macro (`id_syntax!`),
  `dup_check` and `dangling`. `envelope` is `envelope::envelope_checks`. The
  area-id set the NPC and quest-plan references shared is
  `world::declared_area_ids`; the stage-7 batch check computes the same set
  itself, a second copy left in place because merging the two is not a move.
- `check_enchantments` reads an enchantment map, and the one other rule about
  that map, `enchantment_component`, is in `wave.rs`, so it went there with
  `DW0433`/`DW0434`. `check_stack_count` and `MIN_CONTAINER_SLOTS` are the
  container fill's and went to `loot.rs` with `DW0435`/`DW0436`; a trap's
  `dispense` and a `collect` call them there.
- `split_blockstate` is not `world_edits_checks`' alone: `check_block_field`,
  shared by the quest and trigger checks, calls it too, so it stays in
  `validate/mod.rs`.
- `DW0102` stays in `diagnostic::codes`. Its constant is named `DSL_VERSION`,
  which is also the format number `envelope.rs` declares, so declaring the
  code there takes an inline module, and `check-dw-codes`' remedy scan reads
  a name the tree uses twice only through `codes::` — the move would unbind
  `DW0102`'s raise site from that scan. Moving it needs a renamed constant (a
  `delvec codes` change) or a scan that qualifies by the declaring module.
  `DW0101` and `DW0103` moved.
- 29 codes moved: `economy` 6 (`DW0520`–`DW0524`, `DW0901`), `trap` 5,
  `assembly` 3, `envelope`, `lethal`, `loot`, `shortcut`, `timed_gate` and
  `wave` 2 each, `ambush`, `loop` and `world_edits` 1 each. The codes still in
  `diagnostic::codes` are raised from two modules or more (`DW0100`, `DW0102`,
  `DW0110`–`DW0112`, `DW0142`, `DW0143`, `DW0170`, `DW0172`, `DW0173`,
  `DW0190`, `DW0192`, `DW0193`, `DW0196`, `DW0432`, `DW0500`, `DW0741`,
  `DW0953`) or from one module no B2 cut holds (`equipment` `DW0898`;
  `healthbar` `DW0909`, `DW0910`, `DW0912`; `onkill` `DW0913`; `celestial`
  `DW0931`; `l10n` `DW0180`–`DW0184`, `DW0187`, `DW0188`; `chrome` `DW0186`;
  `viewdistance` `DW0956`) or from no DSL module (`DW0940`, raised by
  `compiler::textures` alone). Dissolving `diagnostic::codes` is a step of its
  own.
- Catalog: the trade-and-stake section and the purchase section go to
  `dsl/economy.md` whole, the locked-strike section to `dsl/assembly.md`.
  `DW043x`'s rows end two each on `dsl/wave.md`, `dsl/loot.md` and
  `delvec/compiler/loot.md`; the tie is broken by the object the section
  describes, the container fill, so its prose is on `dsl/loot.md` (authored).
  The four pointer sections left without a row on `dsl/diagnostic.md` are
  dropped.
- `world.rs` (1668 lines) and `wave.rs` (1592) are past the §1 limit; both
  were past it before this step (1529, 1528), and each becoming a directory is
  a step of its own.

**What B3a found** (corrections to this record, made where B3a touched it):

- The suite is not one range. In `emit.rs` at `fb2042b30` it runs from the
  section header at line 16587 to the end of `emit_verb_packtests` at 26134,
  plus `emit_seal_packtest` (9028–9101, in the seal section) and the
  atmosphere templates with their four helpers and `CellBox` (26184–26424).
  All three moved.
- Three functions inside that range are read by emission outside the suite
  and stay in `emit.rs` for B3b: `campaign_complete_tail` and
  `quests_ending_tail` (the critical-path manifest) and `wave_machinery_waves`
  (the wave machinery).
- `emit_packtest` was not only a list: it wrote the campaign, sealed-state,
  declared-difficulty and hand-camera templates inline, and
  `emit_v06_packtests` ended by calling five other objects' emitters. The
  four bodies became functions in `packtest/{quest,world,creator}.rs` and the
  five calls moved into the driver, in the same order. `packtest/world.rs`
  and `packtest/creator.rs` are objects §2's list does not name.
- Three moved functions bundle several objects under a version or verb name
  and moved whole: `emit_v06_packtests` (checkpoint, stealth, cutscene freeze,
  `damage-players`) into `checkpoint.rs`, `emit_v04_packtests` (prop, NPC
  despawn and move, the dialogue-mask walk) into `npc.rs`, and
  `emit_verb_packtests` (one template per gameplay verb) into `objective.rs`.
  Phase C splits them by object.
- §5 names no per-file ledger that holds `emit.rs`.
  `check-structure-emitters` lists files that name `fastnbt::to_bytes`, and
  `emit.rs` names none; `check-effect-roots` allows `plan.rs` and `effects.rs`
  only; the `DW0185` scan reads emitted bytes, not source paths. No DW code is
  declared in the moved code, so no catalog row moves.
- A source scan that reads only the top level of `compiler/` loses whatever a
  step moves into a directory. `tests/atmosphere.rs`'s one-biome-reader scan
  was one; it walks the tree now.

**What B3b found** (corrections to this record, made where B3b touched it):

- Two functions were each over the 1500-line rule on their own, so a move
  alone could not meet it. `build_with_warnings` (2410 lines) keeps the
  order of its passes and calls four of them as functions: the assemblies,
  the blockout battery, the declaration proofs and the checks read off the
  finished tree in `emit/proofs.rs`, and the world block in `emit/routes.rs`,
  which returns its routes and ledgers as one `WorldProofs`. The world block
  belongs to `nav/` (§2); it waits in `emit/routes.rs` because B4 holds
  `nav.rs`. `emit_functions` (2138 lines) keeps its setup and tick bodies
  inline and calls its six per-object blocks as `<object>_fns` in `class`,
  `dialogue`, `quest` (two), `wave` and `objective`, in the same place and
  order.
- The twelve codes `emit.rs` declared stay declared in `emit/mod.rs`, which
  is the module `emit.rs` was, so `delvec codes` is byte-identical and no
  catalog row moves. They move with their checks in Phase C.
- `emit/mod.rs` re-exports each file with one glob, at the visibility of the
  file's most visible item, so `crate::compiler::emit::X` names the same item
  for every caller inside and outside the crate; `nav.rs`, `plan.rs` and the
  tests are not touched. Phase C changes those paths as each object folds.
- Files §2's list does not name: `proofs` and `routes` (above),
  `advancement` (one emitter over every right-clickable object, moved whole
  like `emit_v06_packtests`; Phase C splits it), `affordance`, `coords` (the
  cell-to-entity conversion, facing yaw, box selectors), `equipment` (the
  DSL's `equipment` object), `removal` (how a removed body leaves), `piece`
  (sentinels, template extents, chunk spans), `world`, `effect` and
  `tests.rs`.
- The text authority `DW0185` allows is `emit/text.rs`: `tr`, `tr_with`,
  `snbt_component`, `snbt_text_component`, and the scan
  `check_untranslated_literals`.
- `check-capability-ownership` names a summon's enclosing function with a
  pattern that accepted `pub` and `pub(crate)` only. Every private item that
  moved became `pub(super)`, so the checker lost the function of every
  interaction summon in `emit/` and crashed. It accepts any restricted
  visibility now; a planted unledgered `pub(super) fn` summon reds it. Two
  Rust source scans spell the same grammar as prefixes and will miss a
  `pub(super) fn` the same way when B4 or Phase C moves the code they watch:
  `nav.rs`'s callers-of-`liveness_of` test and
  `tests/footprint_call_graph.rs`.

**What B4a found** (corrections to this record, made where B4a touched it):

- The world measured about 2700 lines and the router about 2600, so both are
  directories under the 1500-line rule, as §2 now shows.
- Inside one pass directory, an item that was private to the pass file is
  `pub(in crate::compiler::nav)`, never `pub(crate)`, so its reach stays what it
  was. `RegionState` and its derivations stay private to `nav` because the point
  of them is one passability model. `pub(crate)` is for one object reaching
  another.
- The router's codes (`DW0311`, `DW0314`, `DW0317`, `DW0510`, `DW0544`,
  `DW0546`) stay declared in `nav/mod.rs`. Under §4's mapping, a declaration in
  `compiler::nav::route::leg` has its page at `nav/route/leg.md`, and §4 puts
  the router on `nav.md`. Every code of the world and the router is therefore
  declared in `nav/mod.rs`, and B4a moves no catalog row.
- `walk_cells` and `segment_meets_cell_16` are the world's own geometry, which
  `World` reads. They move to `nav/world/body.rs` so that the world never
  imports from the proofs.
- A module that re-exports its children with `pub use child::*` does not also
  import a child's item by name. The private import shadows the glob, and the
  item drops out of the parent's glob.
- §5 lists no tool ledger that names `nav.rs`. The per-file ledgers over it are
  Rust tests: `nav::tests::premise_declines_are_enumerated`,
  `staged_liveness_tests`, and `tests/{teleport_link,post_beat_camera,sculpt}.rs`.
  The first was keyed by file name. It is now keyed by path under `src/`,
  because `nav/world/mod.rs` and `sculpt/mod.rs` have the same name.

**What B4b found** (corrections to this record, made where B4b touched it):

- Every DW code stays declared in `nav/mod.rs`, and no `delvec codes` module
  field moves. A declaration in `compiler::nav::<object>` has its page at
  `nav/<object>.md` under §4, and Phase C moves it again to
  `<object>/check.rs`, so every row would move twice. Each object's codes move
  once, in its Phase C step, from `nav/mod.rs` and `nav.md`.
- The objects are `actor`, `ambush`, `checkpoint` (with retry cost, whose object
  is the rest point), `cutscene`, `furniture`, `hazard`, `lane`, `leave`,
  `lethal` (a body's reach into a volume), `npc`, `respawn`, `sea`, `shortcut`,
  `stealth`, `timed_gate`, `trap` (volley and collapse included) and `wave`
  (the optional elite). Three are named differently from §2's list. `horizon`
  holds `DW0322`, because boundary safety is a property of the world's horizon,
  not of the playable region `boundary` declares. `view` holds `DW0724`, which
  judges the render plan's derived cameras. `staging` holds what the `move-npc`
  and `move-actor` walks share: where a body was left on a branch, the smoothed
  and resampled polyline, the facing, and the timeline's own seals. Phase C
  decides whether `staging` folds into `npc` or `actor`, or stays a mechanism
  both read.
- `SPRINT_TICKS_PER_BLOCK` is the move model's, and five proofs read it, so it
  moves to `nav/world/body.rs`.
- An object's tests live in a `#[cfg(test)] mod tests` at the end of its file.
  The world's and the router's tests are out-of-line `#[cfg(test)] mod tests;`
  files (`world/tests.rs`, `route/tests.rs`), because each module with its
  tests is over 1500 lines. The fixtures that the tests of more than one object
  share are `nav/testkit.rs`.
- A source scan reads production through one rule,
  `crates/delvec/tests/common/source_scan.rs`. It removes only the item that a
  column-zero `#[cfg(test)]` applies to, and drops the file of an out-of-line
  test module. The premise scan stopped at the first such attribute and missed
  production in eight files. It now lives in
  `tests/premise_declines.rs`, and `teleport_link`'s nav scan reads through the
  same rule. `check-dw-codes` reads an out-of-line test module's file as test
  code, as it already read an inline one.

**What assembling B2, B3 and B4 found** (corrections to this record, made where
the assembly touched it):

- B3b and B4b each repaired a visibility-prefix scan separately, so the rule
  had five spellings in Python and four in Rust. It has one in each now:
  `tools/lib/rust_source.py`'s `VISIBILITY`, read by
  `check-capability-ownership`, `check-dw-codes` (`use` lines and out-of-line
  test modules), `check-demo-levels` and `check-source-dupes`; and
  `crates/delvec/tests/common/source_scan.rs`'s `unscoped` and `fn_name`, read
  by `footprint_call_graph.rs` and by the callers-of-`liveness_of` test, which
  leaves `nav/world/tests.rs` for `tests/liveness_of_callers.rs` so that it can
  read production through the same rule. `footprint_call_graph`'s "no
  footprint function of its own" scan still listed `fn`, `pub fn` and
  `pub(crate) fn`; through the rule, a `pub(super) fn` naming a footprint reds
  it.
- A page names the file its module is: `delvec/compiler/emit.md` and
  `delvec/compiler/nav.md` name `emit/mod.rs` and `nav/mod.rs`.

**What B5 found** (corrections to this record, made where B5 touched it):

- `impl Plan` was about 1360 lines. `plan/mod.rs` keeps the constructor
  (`build`, `build_with`, `relinked`, `build_linked`, `with_design_files`).
  Each accessor moves to its object's file, in that file's own
  `impl<'a> Plan<'a>` block. For example, `bonfires` and `flasks` go to
  `checkpoint.rs`, `body_point` to `body.rs`, and `branch_critical_path` to
  `path/`.
- The forced walk measured about 1950 lines with its region events, so under
  the 1500-line rule it is a directory: `path/mod.rs` (`Step`,
  `CriticalPath`, `build_critical_path`), `path/region.rs` (the region
  events and when each fires), `path/triggers.rs` (the triggers the walk
  performs and the step each beat fires at) and `path/ancestors.rs`.
- The effect-root walks (`EffectRoot`, `for_each_effect_root`,
  `for_each_gate_effect`) are in `plan/effects.rs`, which mirrors
  `dsl::effects`. §5's allowance in `check-effect-roots` follows
  `required_anchors_for_area` to `plan/anchors.rs`.
- The `plan.rs` allowance covered more than its reason said. The scan reports
  one window per file, and the first window in `plan.rs` was
  `required_anchors_for_area`. After the split, two more hand-rolled walks of
  R1+R2+R3 appear: `collect_v06_effects` (`plan/checkpoint.rs`) and
  `collect_open_gate_anchors` (`plan/gate_reach.rs`, which feeds the `DW0306`
  model). Each now has its own `ALLOWED` entry, marked as an open finding
  that has not been triaged. The exempt region drops from 8529 lines to the
  three files, and the rest of `plan/` is scanned.
- The ten codes `plan.rs` declared stay in `plan/mod.rs`, the module
  `plan.rs` was. `delvec codes` is byte-identical and no catalog row moves.
  They move with their checks in Phase C.
- Files §2 does not name: `surround` (the terrain around the map),
  `gate_reach` (the `DW0306` deadlock proof), `effects`, `body` and `naming`.
  `collect_v06_effects` and its collector move whole into `checkpoint.rs`,
  in the same way as B3a's `emit_v06_packtests`. Phase C splits off the
  stealth half.
- An item that was private to `plan.rs` and is read from a sibling becomes
  `pub(super)`. In `path/`'s children it becomes
  `pub(in crate::compiler::plan)`, or `pub(super)` where only `path/` reads
  it. An item read nowhere else stays private. That covers 29 items, two
  `AnchorTable` methods and the two fields of `PathLinks`.
- `tests/blockout.rs`'s single-caller scan used to recognise a definition of
  `build_with` only by a `pub fn` or `fn` prefix. It now accepts every
  visibility and has its own test.

**What B6 found** (corrections to this record, made where B6 touched it):

- The view and edit arms are `cli/view.rs` and `cli/edit.rs`, not
  `compiler/view/cli.rs` and `compiler/edit/cli.rs`. Those two are modules of
  the library, and every view and edit arm calls the binary's loader and
  reporters (`validate_stage`, `load_or_refuse`, `validate_loaded`,
  `read_structures`, `print_diags`, `print_build_error`), as `detail.rs` does.
  A library cannot call its binary. Placing the arms there would make all of
  `cli/` library code and export every subcommand body `pub`, which §8 rules
  out. The library's view `cli` already takes the binary's stand check as a
  closure (`run_cameras`, `run_place_camera`) for the same reason.
- `detail.rs` stays at `src/detail.rs`. Its module, `delvec::detail`, is
  `DW0882`'s registry `module`, and its catalog page is `delvec/detail.md`.
  A move into `cli/` would move that row, so it waits for the object's
  Phase C step.
- `main.rs` declared no DW code. `DW_SKIN_PNG_MISSING` is an alias of
  `textures::DW_IMAGE_MISSING` and moves with `read_skins`. `EXIT_INTERNAL`
  stays in `main.rs` as the exit-code table. `delvec codes` is byte-identical.
- Helpers more than one file reads have one home each. `read_structures`,
  `read_skins` and `has_error` are in `campaign.rs`, which every reader of a
  campaign's placed bytes goes through. `write_file` is in `report.rs`, read
  by calibrate, the view arms and edit. The view helpers that edit reads
  (`manifest_path_for`, `pull_into_open_air`, `sea_level_of`) are in
  `view.rs`, at `pub(super)`.
- The nested action sets moved with their arms: `EditAction` to `edit.rs`,
  and `RigAction` with `FacingArg` to `metrics.rs`, at `pub(crate)`.
  `clap_surface.py` recognised an enum, an `Args` struct and their fields
  only when private or `pub`, so the move made it read `edit` and `rig` with
  no flags (`--batch`, `--out` and `--facing` lost). It reads every
  visibility now. §7 named a test of its own that did not exist; it is
  `tools/tests/test_clap_surface.py`, and it reds with the old parser. The
  campaigns repository vendors the file and takes the change at its next pin
  bump.
- `check-skill-page` judges the page against the engine at `[engine].ref`,
  a release tag, so it cannot see a change on this branch. The surface proof
  is the parser's output compared before and after, and every `--help` of
  the built binary compared before and after.
- Three Rust source scans named `main.rs`. `tests/footprint_call_graph.rs`
  reads `cli/metrics.rs`. It also recognised a footprint definition by three
  visibility prefixes, and now it accepts any visibility.
  `tests/premise_declines.rs` and `tests/blockout.rs` name `cli/view.rs` and
  `cli/campaign.rs`.

**What assembling B2c, B5 and B6 found** (corrections to this record, made
where the assembly touched it):

- B5 and B6 each repaired a visibility-prefix scan with a private spelling:
  B5's `is_fn_definition` in `tests/blockout.rs`, B6's `defines_fn` in
  `tests/footprint_call_graph.rs` and its `VIS` regex in
  `tools/lib/clap_surface.py`. All three read through the one rule now:
  the two Rust scans through `common::source_scan::fn_name`, with one test of
  it carrying both scans' cases, and `clap_surface.py` through
  `tools/lib/rust_source.py`'s `VISIBILITY`, loaded from beside itself
  because its callers load it by path. A copy of `clap_surface.py` carries
  `rust_source.py` with it.
- B2c and B6 met where B6 moved a body B2c had edited: `run_rig_describe`
  names `assembly::ASSEMBLY_RIG`, in `cli/metrics.rs`. B2a and B5 met the
  same way: `plan/surround.rs` names `world::SURROUND_NO_REGION` and
  `world::HORIZON_PARAM`. Neither was a textual conflict inside the moved
  file; each was a modify/delete or a conflict in the file the body left.

**Phase C — fold by object** (fully parallel across objects; each a couple of
hours): for each object, `compiler/<object>/{mod,check,emit,packtest}.rs` is
formed from `<object>.rs`, `emit/<object>.rs`, `emit/packtest/<object>.rs`,
`nav/<object>.rs` and `plan/<object>.rs`; its codes are declared in
`check.rs`; its catalog rows move to its page under the A1 rule; the
per-file ledgers of §5 are renamed and perturbed. Two objects never share a
file after Phase B, so two Phase C steps never conflict except at a `mod.rs`
line.

**Phase D — the private copies** (after the Phase C steps of the objects
involved): D1 the state evaluator; D2 the stand-anchor question. Each is one
PR, and each is proven byte-identical by the baseline — which is the whole
point of doing them after the fold: a private copy that was semantically
different shows up as a moved manifest, and the step then records which body
was wrong.

**Phase E — CI** (independent; may run first): the gallery split of §6, one
PR, under the three-part rename procedure.

### 8. What this does not change

- **Emission bytes.** Every manifest in `gallery/baseline/manifests.json` is
  identical before and after every step; the warnings ledger is identical.
- **The DSL surface.** `delvec schema --stage all` is byte-identical across
  every step; `dsl_version` does not move, because no document changes
  meaning. The gallery's `quests.json` is assembled, not redefined.
- **DW codes.** The set of codes, their tiers, their subjects and their
  messages are unchanged; `delvec codes` differs only in the `module` field,
  which is the key the docs follow. No code is renumbered, retired or added by
  this record.
- **The compiler's public surface.** No `pub` item leaves the crate that is
  not exported today; the DSL crate's root namespace stays flat through one
  glob per module.
- **The baseline's conflict class.** The generated artifact is regenerated
  after a merge, as ruled already; this record does not re-file it.

## Consequences

- A feature touches the object it is about: its module, its page, its gallery
  array, its finding file, its demo row. Two features on different objects
  merge clean by construction, and the residue is one-line registry adjacency.
- The docs split follows the code split mechanically, through one gate that
  reads one registry field; nobody plans a docs migration.
- A page per module is a page a reader can find without an index, and a page
  nobody can find is a page nobody keeps current.
- The migration is thirty-odd small pull requests rather than one; each is
  proven by the gate the tree already has, and most of them can run three
  abreast under the dispatch cap.
- Two private copies become one home, and the shape that let them hide — one
  verb beside another in a file nobody reads whole — is gone.
- Costs: `mod.rs`, `Plan`, `Command` and the two ordered driver lists remain
  files every object-adding feature touches by one line. `cargo` build time
  does not change — one package, one compilation unit (ADR-0025). A reader who
  learned the pass files learns the object directories.

## Revisit triggers

- A file over 1500 lines appears under an object directory and is touched by
  two features in one quarter.
- The conflict replay of §Context, re-run a quarter after Phase C completes,
  shows a non-generated file conflicting in more than five merges.
- The function-body duplicate rule, once measured, has a false-positive count
  low enough to adopt — or high enough to record that the class is caught by
  review only.
- A `rust` test shard instrument is measured and adopted, or declined with its
  measurement.
