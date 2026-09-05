# Delvewright — Agent Constitution

Delvewright is an automated production line that outputs **self-contained Minecraft adventure "delves" on demand** for a fixed group of 1–4 players. A delve is a 2–3 hour (10h ceiling), story-driven, box-garden (箱庭) adventure map: adventure mode, class selection with pre-provided gear, zero grind. It ships as a versioned OCI image — one `docker run` = a joinable dungeon — and must be **provably completable by machine** before a human spends their one QA hour on it.

The creator is an agent (ADR-0012): the LLM writes the DSL, the human gives ideas and plays the result. Every authoring surface — DSL, CLI, skill — is agent-facing. Every input to a surface is either a creative judgement (the agent's argument) or a procedural derivation (handed by the tool, never typed); an error is refused where it is entered, not at the end.

Founding decisions live in `docs/adr/`. Read the ADR index before proposing architecture.

## This file is half of the constitution

This file holds what anyone building Delvewright must obey to produce a correct artifact. The other half — how this deployment is run: dispatch, review, merge, staging, decisions — is **`CLAUDE.local.md`**, gitignored, loaded by the same memory loader and carrying the same force. `tools/planner-state.sh` (bound to `SessionStart` and `UserPromptSubmit --if-stale 12`) refuses by name when it is absent. Without it you have half a constitution: say so and ask before improvising anything about dispatch, review, merge or staging.

**Neither file is edited without the owner's confirmation in conversation.** A rule is stated once and never restated; a lesson goes into a tool, a diagnostic or `docs/reference/` first. Rules state what to do, not why. **One paragraph or bullet per line; never break a line for length.**

**Agent memory** (the auto-memory directory) holds exactly three kinds of entry: the pointer to the note that initialises a session, facts about the owner that are not rules (preferences, context, ambitions), and addresses of external resources (repositories, dashboards, channels, test worlds). **A rule, ruling or lesson is never written to memory**; a memory that restates one is deleted. Each fact has one home.

## Architecture (settled — see ADRs, do not relitigate)

- **DSL → compiler → datapack** (ADR-0001): campaigns are schema-enforced JSON written by the LLM; a deterministic compiler emits the datapack. The LLM **never** writes raw mcfunction.
- **Staged DSL** (ADR-0002): world/setting → NPCs → classes/gear → campaign quest plan → quest expansion. Each stage is a schema; later stages condition on earlier outputs.
- **Vanilla-first** (ADR-0003): the player-facing server runs pinned vanilla + datapack only. Mods (PackTest, Carpet) exist solely in tooling/validation.
- **Prefabs + jigsaw** (ADR-0004): maps assemble from a `.nbt` prefab library via vanilla jigsaw/template_pool with compiler-controlled seeds. No block-by-block generation. GDPC is a documented fallback, not built.
- **Two-layer validation** (ADR-0005): static quest-graph reachability + command validation at compile time; PackTest + mineflayer critical-path bot at runtime.
- **Determinism** (ADR-0006): same DSL + same seed → byte-identical datapack and world. Hard invariant, tested from day one.
- **OCI packaging** (ADR-0010): delve = pinned server + world + config + datapack image.
- **Pinned MC version** (ADR-0009): **Minecraft Java 1.21.11**, a long-term constant.
- **Compiler foundation** (ADR-0011): Rust-native compiler; beet/mecha only as an independent CI cross-check, never as the emission path.
- **One binary** (ADR-0023): the engine ships one binary, `delvec`, the delve creator; every creator-facing capability, GPU rendering included, is a subcommand of it, and no cargo feature gates any of them. Release targets are gnu on Linux, darwin and windows-msvc. A creator obtains the per-platform release archive at the skill's `Init` or installs from crates.io; both channels are maintained and published by CI only; a developer builds from source.
- **Product form** (ADR-0012): a Claude Code skill (`/new-delve`) is the generation front-end; Claude Code is the agent runtime; the generated DSL documents are the artifact of record; building an agent runtime from scratch is permanently out of scope.

## Forbidden zones

- **No raw mcfunction authored by an LLM** — all commands come from the compiler.
- **No mods on the player-facing server** — validation-layer only.
- **No nondeterminism in the compiler**: no wall-clock time, no unseeded RNG, no hash-order iteration, no absolute paths in output.
- **No CC BY-NC / ND / unknown-license assets, ever.** Original, CC0, CC BY, MIT, Apache-2.0, or GPL-3.0-compatible only (ADR-0013). Record provenance in prefab metadata.
- **No grind mechanics in delve design**: no mining/leveling loops, resource farming, or base building.
- **No runtime LLM in shipped delves**: all content, dialogue included, is authored at generation time as pre-written branching options (spec-0001).
- **The production host is prod-only** — never target a delve-hosting machine for dev or tests.
- **Generated campaigns/worlds do not live in this repo** — they ship via GitHub Releases / OCI registry (content licensed separately from GPL code; ADR-0007).
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

- **Make it work.** The goal is a 20+ scene, unified-appearance delve authored from a fresh content clone through `/new-delve` alone. A gate, tool or abstraction that does not move that goal is removed, without a defence. Not relaxed by this: determinism, never-weaken-a-check-to-get-green, a diagnostic owing a test. Make the safe path the default with no per-case judgement.
- **Spec-driven**: specs carry machine-verifiable acceptance criteria. No spec, no feature — write it first. An acceptance criterion is checked against the tree before anything is built on it; a criterion the implementation cannot yet satisfy is a recorded debt, never a pass; any rewrite that reduces what a criterion asserts is declared as a loosening in those words.
- **No hacks at any layer**: if vanilla provides a primitive content needs, the DSL exposes it first-class; if the only implementation is a lower-layer hack, the feature is excluded until vanilla provides one. Applies at every boundary: NBT→compiler, compiler→DSL, DSL→skill.
- **A craft question the record does not answer is researched against established practice, never invented.** State per rule whether it is cited or authored; name unsupported claims as such; land the research under `docs/reference/`; record the gap against the line that should have covered it; research answers the question asked and stops. Unlicensed sources are ideas-only (ADR-0013, `ACKNOWLEDGEMENTS.md`).
- **This is a general engine.** A primitive encodes a mechanism, never a design decision about what it is for; a creator making a different game must be able to configure it. A capability belongs to the object class it acts on, not to the verb that first needed it; a second bespoke field is the defect. Review shapes: a hook keyed to a verb rather than its object class; a general mechanism privately re-implemented inside one verb; a general mechanism whose binding is too narrow to reach the objects it should (ask what it fails to reach and why, and whether the wider site can express the rule's quantifier, before adding a surface); a parameter inferred from arguments when the caller knows more (let the caller state it).
- **Debug doctrine**: a red check is information. Never weaken a check, test or threshold, and never reroll a seed, to get green; fix the root cause or escalate. An intermittent red is an under-specified test, never re-run. The trigger is not the cause: a fix that only works by undoing the recent change is aimed at the trigger. Preserve a lesson in the strongest form available: compiler diagnostic > tooling default > generator invariant > docs. A regression is named with its direction (can it only turn a proof red, or can it let something ship), never fixed quietly inside an unrelated change. A hand-written field in committed data that stands where a measurement belongs is part of the tool defect's blast radius.
- **Measurement doctrine**: a measurement that is a deliverable is cross-checked by a second method that shares no configuration with the first, varying only the suspected variable. A computed key is itself a measurement: check what the question resolved to. A frozen measurement names its instrument by exact revision, never through an indirection. A count equal to its fetch limit is refused, not reported. A scripted replacement asserts its match count. A `grep -c` counts lines and mentions, not obligations: read what the matches say. Enumerate the container and pipe that into the count; justify every exclusion. A zero that disagrees with an independent observer is the measurement failing. An instrument is force-rebuilt before a comparison runs, with cargo's exit status asserted on its own line; a dev-profile binary's hash is not a freshness check; a build failure is a gate failure, never a fallback; a suspected non-determinism is settled by building twice. A number is written down after it is measured, in a commit body. Probe presence by reading, treating only not-found as absent. Read a failing run from the top; a tool that fails before its comparison has not compared. A story that explains the data is not evidence: build the commit it blames. Commit before demonstrating; restore a perturbation from a scratch copy, never from git; a proof script does not change refs while the instrument lives in the tree. Non-trivial ad-hoc shell runs under `bash -c`, never the interactive zsh; capture a command's status before piping it.
- **Vacuity**: a green gate that binds to nothing is not a pass. The modes: unbound (matched zero objects), unfenced (the version never reached the surface), unemitted (declared, compiled, never emitted), unrun (nothing invokes it; a doc line is not an invocation; a helper called only from tests is unrun), untraversed (halted before the end — always `--no-fail-fast` before comparing failure sets), and an opt-out secured by a property the defect can supply (demand something the defect cannot produce; where the opt-out is a choice among kinds, the object determines the kind; a second hatch on one gate is the defect). Every validation artifact states its binding count computed from the objects, with its denominator; a zero binding is a finding unless the objects do not exist yet. A gate is bound to the event it guards, with every entry point enumerated; any override is explicit, printed, and shaped so it cannot become habit. Test a gate by perturbing toward the vacuous shape and checking it reds, with a perturbation only that gate could catch. A record that claims to be a measurement carries a canonical hash of what was measured. Full derivation: `docs/reference/playtest-methodology.md`.
- **Pairs**: where two gates guard one artifact, read them together; a remedy one prescribes and the other refuses is the pair's defect; a gate that names a remedy owes a check that the remedy is reachable. A documented procedure and the gate that judges it are a pair: ask whether anything has ever executed it green.
- **A checker reads a document the way its consumer reads it**, cross-checked against a real implementation of the format with the comparison committed, with one shared parse rule and no private copy per gate. A checker that skips regions audits what it skipped and prints the size. A resolve-by-name over a scope where names are not unique yields a candidate, not a match. A gate proves a surface is authored, never that it is right: ask what element would answer differently if the implementation were wrong. A diagnostic's quantifier is part of what it says; read it before describing what the check covers.
- **Every command's response is read.** Live commands go through the shared rejection rule (`tools/lib/rcon.{sh,mjs}`); emitted commands are checked against the pinned command tree by the emitter; both bound by `tools/check-live-commands.py`. A shared rule is extracted, never copied per call site.
- **A finding is closed when its general form is a diagnostic re-run against the current build**, or an explicit record says only the instance was fixed. A capability-gap finding blocks staging: the engine work lands before the next playtest, or the round summary says per item it is open and not to test it. The findings ledger is audited from round 1 before any build is staged. A "known gap" in a reference document is a ledger row, not a doc line. A claim about where a defect reproduces is made by reproducing it.
- **CI is the sole arbiter** (ADR-0008). Every CI job is a required status check; `.github/required-status-checks.txt` and `tools/check-required-contexts.py` hold the names in lockstep in both directions. A scheduled workflow is never a gate. CI green is admission to verification, not grounds to merge. "Green" is a property of a revision; a `DIRTY` or draft PR runs nothing, so `0 failed` is information only beside a non-zero check count.
- **Everything runs on the creator's own machine, from source.** Where a binary cannot carry a capability, the first run builds from source. Every skill owns an `Init` section that establishes the toolchain before any work begins, the release archive first and source as the floor; a tool that cannot be bundled is acquired at the step that needs it. A distribution question never decides a capability question. A reclaimer names every class of resource its subject holds and proves each gone.
- **A release is built from a frozen approved tree**, named exactly; only release plumbing may be added on top, each file named; a release refuses when the tree differs from the approved baseline by anything unnamed.
- **Tiered testing**: unit + static on every push; PackTest on PR; full bot playthrough on release candidates. **PR-based flow even solo**; both repositories (`stellarfeline/delvewright`, `stellarfeline/delvewright-campaigns`) are public (ADR-0017).
- **Nothing owes compatibility to anything already built.** A change that stops an existing document compiling is not a defect: the document is changed or deleted, with no justification, shim, flag or migration. `dsl_version` numbers a surface; it promises nothing.
- **Docs are the only persistent memory.** `docs/reference/compiler.md` is the live record of `delvec` (surface, emission, invariants, every DW code; `tools/check-dw-codes.py` enforces the code subset bidirectionally). A PR that changes compiler behaviour updates it in the same PR. A PR that adds or changes an authoring tool updates `docs/reference/` and every skill it touches in the same PR — LLM-facing tools as mandatory steps, human-in-the-loop tools as one-line advisory mentions; a validated pipeline enters the skill with the PR that makes it work; `docs/reference/tools.md` is the inventory of the whole tool surface. Specs and ADRs are historical decision records; ADRs are the only place history lives. A ledger constant is enumerated, never restated as a literal; a census derivable from the object is never hand-written; a prose note recording a code fact is a pointer, not a clearance. End every session by writing lessons back, in the strongest form.
- **Write short documents, each for ONE reader, in the present tense of the current version.** Agent-facing docs may be arbitrarily technical; player-facing docs carry only what that reader needs to act, never internal machinery. No "used to", no version narration, no internal reference numbers a stranger cannot resolve; a stripped reference is not replaced by narrating what it asserted.
- **A campaign is never the engine's test surface; the gallery is** (spec-0039). Every engine surface owes a gallery element in the same PR: the coverage gate enumerates units from `delvec schema --stage all`, and a unit is bound in the gallery or refusal-proven by a committed probe (the primary plus one declared edit) the engine rejects with a named code — no third state, no prose exemption. Vanilla registry values are data, never units. A bound element is accepted by perturbing the declaration and checking an emitted byte moves; an inert element is reported as a zero binding. The gallery owes legibility: a creator reading it sees what the engine builds and which checks fire. A campaign that stops building adopts or is deleted. **Every new mechanic owes a demo level** row in `docs/demo-levels.md`, queued when the mechanic lands; an engine capability is confirmed on a demo level, never on a campaign's renders.
- **Buildings are judged at playable scale**: does it read as the thing, and does the inside belong to it — never is the detail right. An oversized space with no answer to "what does the player do in here" is cut or filled. When the vanilla block that names a thing is too small for the weight the story gives it, the thing is built out of blocks.
- **A clean auto-merge is not evidence of semantic compatibility.** Enumerate what each branch claims to do and re-demonstrate every claim on the merged tree; re-read merged docs; grep the added lines of both sides for a second "one authority" of the same rule; name cross-feature pairs up front and land their test with the merge. A generated artifact is reset to one side wholesale and regenerated after the merge commit exists, never three-way merged. Before correcting a wrong git operation, establish what the correction changes (`merge-tree --write-tree`).
- Repeated workflows become skills once done manually twice.

## Conventions

- **English-first** for every repo artifact; i18n translates from English.
- Rust: workspace at `crates/`, edition 2024, `cargo fmt` + `clippy -D warnings` clean. `prefabs/*-generator` are their own workspaces.
- TypeScript (harness only): strict mode; assertions and navigation, never game logic.
- ADRs: sequential, status field, cite sources; never edit an Accepted decision — supersede it. Specs: `spec-NNNN-<slug>.md` with a machine-checkable "Acceptance criteria" section. Numbers (spec, ADR, DW code, `dsl_version`) are allocated by the planner across every remote ref, never picked by a round.
- Commits/PRs: conventional, small, one concern each. Commit messages and every `gh` text argument come from a file, never inline; a measurement belongs in the commit body.
- **Privacy in repo artifacts**: no personal information, no verbatim personal speech, no record of who decided what or when. Sanctioned identifiers are ADR numbers, spec numbers and DW codes; a task id, PR number or dated attribution is not one.
- **Attribution ledger**: any adopted library, ported algorithm or paper gets its entry (verified license) in `docs/ACKNOWLEDGEMENTS.md` in the same PR.
- **DW-diagnostic coverage**: every DW diagnostic is asserted by at least one test; a minimal, justified allowlist is the only exemption.

## Environments

- **Dev**: a developer workstation (macOS). Everything must run locally.
- **CI-equivalent**: the `validation/` docker compose profile — the same image CI uses.
- **Prod**: a delve-hosting single-board computer; release images are multi-arch (amd64 + arm64).
