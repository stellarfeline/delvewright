# Delvewright — Agent Constitution

Delvewright is an automated production line that outputs **self-contained Minecraft
adventure "delves" on demand** for a fixed group of 1–4 players. A delve is a 2–3 hour
(10h ceiling), story-driven, box-garden (箱庭) adventure map: adventure mode, class
selection with pre-provided gear, zero grind. It ships as a versioned OCI image — one
`docker run` = a joinable dungeon — and must be **provably completable by machine**
before a human spends their one QA hour on it.

The creator is an agent (ADR-0012): the LLM writes the DSL, the human gives ideas
and plays the result. Every authoring surface — DSL, CLI, skill — is agent-facing.

Founding decisions live in `docs/adr/`. Read the ADR index before proposing
architecture.

## This file is half of the constitution

This file holds what anyone building Delvewright must obey to produce a correct
artifact. The other half — how this deployment is run: dispatch, review, merge,
staging, decisions — is **`CLAUDE.local.md`**, gitignored, loaded by the same memory
loader and carrying the same force. `tools/planner-state.sh` (bound to `SessionStart`
and `UserPromptSubmit --if-stale 12`) refuses by name when it is absent. Without it
you have half a constitution: say so and ask before improvising anything about
dispatch, review, merge or staging.

**Neither file is edited without the owner's confirmation in conversation.** Rules
are added rarely; a lesson goes into a tool, a diagnostic or `docs/reference/`
first. A rule already here is not restated.

## Architecture (settled — see ADRs, do not relitigate)

- **DSL → compiler → datapack** (ADR-0001): campaigns are schema-enforced JSON written
  by the LLM; a deterministic compiler emits the datapack. The LLM **never** writes raw
  mcfunction.
- **Staged DSL** (ADR-0002): world/setting → NPCs → classes/gear → campaign quest plan
  → quest expansion. Each stage is a schema; later stages condition on earlier outputs.
- **Vanilla-first** (ADR-0003): the player-facing server runs pinned vanilla + datapack
  only. Mods (PackTest, Carpet) exist solely in tooling/validation.
- **Prefabs + jigsaw** (ADR-0004): maps assemble from a `.nbt` prefab library via
  vanilla jigsaw/template_pool with compiler-controlled seeds. No block-by-block
  generation. GDPC is a documented fallback, not built.
- **Two-layer validation** (ADR-0005): static quest-graph reachability + command
  validation at compile time; PackTest + mineflayer critical-path bot at runtime.
- **Determinism** (ADR-0006): same DSL + same seed → byte-identical datapack and world.
  Hard invariant, tested from day one.
- **OCI packaging** (ADR-0010): delve = pinned server + world + config + datapack image.
- **Pinned MC version** (ADR-0009): **Minecraft Java 1.21.11**, a long-term constant.
- **Compiler foundation** (ADR-0011): Rust-native compiler; beet/mecha only as an
  independent CI cross-check, never as the emission path.
- **Product form** (ADR-0012): a Claude Code skill (`/new-delve`) is the generation
  front-end; Claude Code is the agent runtime; the generated DSL documents are the
  artifact of record; building an agent runtime from scratch is permanently out of
  scope.

## Forbidden zones

- **No raw mcfunction authored by an LLM** — all commands come from the compiler.
- **No mods on the player-facing server** — validation-layer only.
- **No nondeterminism in the compiler**: no wall-clock time, no unseeded RNG, no
  hash-order iteration, no absolute paths in output.
- **No CC BY-NC / ND / unknown-license assets, ever.** Original, CC0, CC BY, MIT,
  Apache-2.0, or GPL-3.0-compatible only (ADR-0013). Record provenance in prefab
  metadata.
- **No grind mechanics in delve design**: no mining/leveling loops, resource farming,
  or base building.
- **No runtime LLM in shipped delves**: all content, dialogue included, is authored at
  generation time as pre-written branching options (spec-0001).
- **The production host is prod-only** — never target a delve-hosting machine for dev
  or tests.
- **Generated campaigns/worlds do not live in this repo** — they ship via GitHub
  Releases / OCI registry (content licensed separately from GPL code; ADR-0007).
- **No feature without an approved spec** in `docs/specs/`.

## Repository layout

```
CLAUDE.md            # this file
docs/adr/            # architecture decision records (numbered, immutable once Accepted)
docs/specs/          # approved specs, one per feature
docs/reference/      # live behavior records: compiler.md, tools.md, i18n.md, grammar.md,
                     #   playtest-methodology.md, skill-workflow.md, prefab-procedure.md,
                     #   worktree-bootstrap.md, distribution-size.md
docs/ROADMAP.md      # milestones
crates/              # Rust workspace: dsl / compiler / grammar / orchestrator / admit /
                     #   schem / render
gallery/             # the ENGINE's own campaign: one instance of every surface the DSL
                     #   declares, built on every PR, never released or staged
prefabs/             # tileset GENERATORS + shared invariants; the .nbt library lives in
                     #   the CONTENT repo, reached through the `campaigns/` dev symlink
harness/             # mineflayer bot tests (TypeScript)
tools/               # auxiliary Python/shell tooling — never shipped in delves
packtest/            # PackTest templates
validation/          # docker compose: headless server + bot, same image as CI & prod
```

## Methodology

- **Make it work.** The goal is a 20+ scene, unified-appearance delve authored from a
  fresh content clone through `/new-delve` alone. A gate, tool or abstraction that
  does not move that goal is negative value at this stage and is removed, and the
  argument for keeping it is itself the waste. This does not relax determinism,
  never-weaken-a-check-to-get-green, or a diagnostic owing a test: those measure
  whether the thing works.
- **Spec-driven**: specs carry machine-verifiable acceptance criteria. No spec, no
  feature — write it first.
- **No hacks at any layer**: if vanilla provides a primitive content needs, the DSL
  exposes it first-class; if the only implementation is a lower-layer hack, the feature
  is excluded until vanilla provides one.
- **A craft question the record does not answer is researched against established
  practice, never invented.** State per rule whether it is cited or authored; name
  unsupported claims as such; land the research under `docs/reference/`; record the
  gap against the line that should have covered it. Unlicensed sources are ideas-only
  (ADR-0013, `ACKNOWLEDGEMENTS.md`).
- **This is a general engine.** A primitive encodes a mechanism, never a design
  decision about what it is for; a creator making a different game must be able to
  configure it. A capability belongs to the object class it acts on, not to the verb
  that first needed it — a second bespoke field is the defect. Review shapes: a hook
  keyed to a verb rather than its object class; a general mechanism privately
  re-implemented inside one verb; a general mechanism whose binding is too narrow to
  reach the objects it should (ask what it fails to reach and why, and whether the
  wider site can express the rule's quantifier, before adding a surface).
- **Debug doctrine**: a red check is information. Never weaken a check, test or
  threshold, and never reroll a seed, to get green; fix the root cause or escalate. An
  intermittent red is a finding, never re-run. Preserve a lesson in the strongest
  form available: compiler diagnostic > tooling default > generator invariant > docs.
  A measurement that is a deliverable is cross-checked by a second method with an
  unrelated failure mode; a computed key is itself a measurement; a frozen measurement
  names its instrument by exact revision; a count equal to its fetch limit is the
  limit; a scripted replacement asserts its match count; an instrument is rebuilt
  (cargo's exit status on its own line) before a comparison runs. Non-trivial ad-hoc
  shell runs under `bash -c`, never the interactive zsh.
- **Vacuity**: a green gate that binds to nothing is not a pass — unbound, unfenced,
  unemitted, unrun (nothing invokes it: a doc line is not an invocation), untraversed,
  or secured by an opt-out the defect can supply. Every validation artifact states its
  binding count and denominator; a zero binding is a finding. A gate is done when the
  event it guards cannot happen without it. Where two gates guard one artifact, read
  them as a pair: a remedy one prescribes and the other refuses is the pair's defect.
- **A checker reads a document the way its consumer reads it**, cross-checked against
  a real implementation of the format, with one shared parse rule. **A command whose
  response nobody reads cannot fail**: live commands go through `tools/lib/rcon.*`;
  emitted commands are checked against the pinned command tree by the emitter.
- **A finding is closed when its general form is a diagnostic re-run against the
  current build**, or an explicit record says only the instance was fixed. A
  capability-gap finding blocks staging. The findings ledger is audited from round 1
  before any build is staged.
- **CI is the sole arbiter** (ADR-0008). Every CI job is a required status check;
  `.github/required-status-checks.txt` and `tools/check-required-contexts.py` hold the
  names in lockstep. CI green is admission to verification, not grounds to merge:
  tests prove a change broke nothing, never that it fixed the target.
- **Everything runs on the creator's own machine, from source.** Binary distribution
  is an optimisation, never the guarantee; every skill owns an `Init` section that
  establishes the toolchain from source before any work begins. A distribution
  question never decides a capability question.
- **A release is built from a frozen approved tree**, named exactly; only release
  plumbing may be added on top, each file named.
- **Tiered testing**: unit + static on every push; PackTest on PR; full bot
  playthrough on release candidates. **PR-based flow even solo**; both repositories
  (`stellarfeline/delvewright`, `stellarfeline/delvewright-campaigns`) are public
  (ADR-0017).
- **Nothing owes compatibility to anything already built.** A change that stops an
  existing document compiling is not a defect: the document is changed or deleted, no
  justification, no shim, no migration. `dsl_version` numbers a surface; it promises
  nothing.
- **Docs are the only persistent memory, and each fact has one home.**
  `docs/reference/compiler.md` is the live record of `delvec` (surface, emission,
  invariants, every DW code; `tools/check-dw-codes.py` enforces the code subset
  bidirectionally). A PR that changes compiler behaviour, or adds or changes an
  authoring tool, updates `docs/reference/` and every skill it touches in the same
  PR; `docs/reference/tools.md` is the inventory of the whole tool surface. Specs
  and ADRs stay historical decision records; ADRs are the only place history lives.
- **Write short documents, each for ONE reader, in the present tense of the current
  version.** Agent-facing docs may be arbitrarily technical; player-facing docs carry
  only what that reader needs to act. No "used to", no version narration, no internal
  reference numbers a stranger cannot resolve.
- **A campaign is never the engine's test surface; the gallery is** (spec-0039).
  Every engine surface owes a gallery element in the same PR: the coverage gate
  enumerates units from `delvec schema --stage all`, and a unit is bound in the gallery
  or refusal-proven by a committed probe (the primary plus one declared edit) the
  engine rejects with a named code — no prose exemption. A campaign that stops building
  adopts or is deleted. **Every new mechanic owes a demo level** row in
  `docs/demo-levels.md`; an engine capability is confirmed on a demo level, never on a
  campaign's renders.
- **Buildings are judged at playable scale**: the silhouette carries recognition, the
  interior belongs to the theme, and grandeur is playable content — a big empty room is
  a small building that costs more to walk across. When the vanilla block that names a
  thing is too small for the weight the story gives it, the thing is built out of
  blocks.
- **A clean auto-merge is not evidence of semantic compatibility.** Enumerate what each
  branch claims to do and re-demonstrate every claim on the merged tree; re-read merged
  docs; a generated artifact is reset to one side wholesale and regenerated after the
  merge commit exists, never three-way merged.
- Repeated workflows become skills once done manually twice.

## Conventions

- **English-first** for every repo artifact; i18n translates from English.
- Rust: workspace at `crates/`, edition 2024, `cargo fmt` + `clippy -D warnings` clean.
  `prefabs/*-generator` are their own workspaces.
- TypeScript (harness only): strict mode; assertions and navigation, never game logic.
- ADRs: sequential, status field, cite sources; never edit an Accepted decision —
  supersede it. Specs: `spec-NNNN-<slug>.md` with a machine-checkable "Acceptance
  criteria" section. Numbers (spec, ADR, DW code, `dsl_version`) are allocated by the
  planner across every remote ref, never picked by a round.
- Commits/PRs: conventional, small, one concern each. Commit messages and every `gh`
  text argument come from a file, never inline.
- **Privacy in repo artifacts**: no personal information, no verbatim personal speech,
  no record of who decided what or when. Sanctioned identifiers are ADR numbers, spec
  numbers and DW codes; a task id, PR number or dated attribution is not one.
- **Attribution ledger**: any adopted library, ported algorithm or paper gets its entry
  (verified license) in `docs/ACKNOWLEDGEMENTS.md` in the same PR.
- **DW-diagnostic coverage**: every DW diagnostic is asserted by at least one test.

## Environments

- **Dev**: a developer workstation (macOS). Everything must run locally.
- **CI-equivalent**: the `validation/` docker compose profile — the same image CI uses.
- **Prod**: a delve-hosting single-board computer; release images are multi-arch
  (amd64 + arm64).
