# Delvewright — Agent Constitution

Delvewright is an automated production line that outputs **self-contained Minecraft
adventure "delves" on demand** for a fixed group of 1–4 players (owner decision
2026-07-30, superseding the kickoff handoff's monthly cadence). A delve is a 2–3 hour
(10h ceiling), story-driven, box-garden (箱庭) adventure map: adventure mode, class
selection with pre-provided gear, zero grind. It ships as a versioned OCI image — one
`docker run` = a joinable dungeon — and must be **provably completable by machine**
before the owner spends their one QA hour on it.

Founding decisions live in `docs/adr/` and originate from the kickoff handoff
(`docs/handoff-2026-07-29.md`). Read the ADR index before proposing architecture.

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
  front-end — Claude Code is the agent runtime; the generated DSL documents are the
  artifact of record; building an agent runtime from scratch is permanently out of
  scope.

## Forbidden zones

- **No raw mcfunction authored by an LLM** — all commands come from the compiler.
- **No mods on the player-facing server** — validation-layer only.
- **No nondeterminism in the compiler**: no wall-clock time, no unseeded RNG, no
  hash-order iteration, no absolute paths in output.
- **No CC BY-NC / ND / unknown-license assets, ever.** Prefabs/content: original, CC0,
  CC BY, MIT, Apache-2.0, or GPL-3.0-compatible only (ADR-0013). Record provenance in
  prefab metadata.
- **No grind mechanics in delve design**: no mining/leveling loops, resource farming,
  or base building.
- **No runtime LLM in shipped delves** (current policy): all content — including
  dialogue — is authored at generation time; dialogue is pre-written branching
  options (spec-0001).
- **The owner's Raspberry Pi is prod-only** — never target it for dev or tests.
- **Generated campaigns/worlds do not live in this repo** — they ship via GitHub
  Releases / OCI registry (content licensed separately from GPL code; ADR-0007).
- **No feature without an owner-approved spec** in `docs/specs/`.

## Repository layout

```
CLAUDE.md            # this file
docs/adr/            # architecture decision records (numbered, immutable once Accepted)
docs/specs/          # owner-approved specs, one per feature
docs/reference/      # live behavior records: compiler.md, tools.md, i18n.md,
                     #   grammar.md + how a round is run: playtest-methodology.md
                     #   + how a delve is generated: skill-workflow.md
docs/ROADMAP.md      # milestones; M1 = hello-world delve
crates/              # Rust workspace: dsl / compiler / orchestrator / admit / schem / render
prefabs/             # .nbt library + metadata (git-lfs)
harness/             # mineflayer bot tests (TypeScript)
tools/               # auxiliary Python tooling (skins, i18n, CI checks) — never shipped in delves
packtest/            # PackTest templates
validation/          # docker compose: headless server + bot, same image as CI & prod
```

## Methodology

- **Spec-driven**: specs carry machine-verifiable acceptance criteria. Implementation
  sessions work against a spec; if none exists, write/propose the spec first.
- **No hacks at any layer** (owner, 2026-07-31): if vanilla/NBT provides an
  intended primitive that content needs, the DSL exposes it first-class — never
  leave it to downstream folklore or workarounds. If the only possible
  implementation of a feature is a lower-layer hack (e.g. raycast polling where
  vanilla has no primitive), the feature is excluded until vanilla provides one.
  Applies at every layer boundary: NBT→compiler, compiler→DSL, DSL→skill.
- **Debug doctrine** (owner, 2026-07-31): a red check is information, never an
  obstacle. Never weaken a check, test, or threshold — and never reroll a seed —
  to get green; fix the root cause or escalate. Escalating a toolchain bug is
  success, not failure. Preserve every debugging lesson in the strongest
  available form, strongest first: compiler diagnostic > tooling default
  (automate the pitfall out of existence) > generator invariant > docs.
  **An intermittent red is never re-run** (owner, 2026-08-05): it is a finding,
  and re-running discards it. An intermittent failure is an under-specified
  test — root-cause it. Evidence: a `grep -q` readiness probe under `pipefail`
  failed 28 times in 30 on a server that was up, because `grep` exiting at the
  match SIGPIPEs its producer; it read as flakiness for months, cost two owner
  playtest stagings, and the same idiom sat under both 25565 safety guards
  (task #173, PR #300).
- **CI is the sole arbiter** (ADR-0008). Nothing merges red. The owner reviews PR
  descriptions and architecture-level diffs, not lines. Write PR descriptions
  accordingly: what changed at the design level, what CI now proves.
  **Every CI job is a required status check** (owner, 2026-08-05). It used to be
  three of ten, so `tier 2` — datapack load plus the whole generated PackTest
  suite — never blocked a merge, and neither did the storybook engine-version
  marker or the prefab determinism gate. Because branch protection matches a
  context by its NAME STRING, a renamed job blocks every PR forever, including
  the one that would fix it: `.github/required-status-checks.txt` and
  `tools/check-required-contexts.py` hold the names in lockstep, in both
  directions, so a rename or a new advisory job is an ordinary red instead.
- **PR merge policy** (owner, 2026-07-30, refined same day): two classes of PR.
  *Owner-review PRs* — docs, specs, ADRs, README, product/design definitions —
  require owner approval of the **content, given in conversation**: the planning
  agent presents the key decisions as a concise chat summary, the owner confirms,
  and the agent then merges directly. The owner does NOT read long documents or
  full diffs — never block on that; if a decision wasn't surfaced in the summary,
  it isn't approved. *Mechanical PRs* — implementation whose correctness CI fully
  arbitrates — merge on green. When in doubt, surface it in chat first.
  Exception (owner, 2026-07-31): a PR that weakens, disables, or skips any
  check, test, or threshold is **never mechanical** — always owner-review,
  regardless of CI state.
  Amendment (owner, 2026-08-04, corrected same day): **CI green is admission
  to verification, not grounds to merge.** Two gates by what the change can
  touch:
  - **Validator-only fixes** — player-facing output byte-identical (harness,
    PackTest templates' test half, CI tooling): merge on a machine red→green
    demonstration of the motivating scenario; the owner does not personally
    review these.
  - **Everything the player can experience** — engine emission, DSL surface,
    campaign content: the owner's own playtest is the merge gate, **in
    batches, never per-PR**: fixes accumulate into the campaign; the planner
    hands the owner one round summary — what was fixed, what to look for per
    item; she tests once and reports. Items she does not flag are confirmed —
    their PRs merge; flagged items stay open and the fix continues on the
    same PR into the next round. Machine red→green on the motivating scenario
    is still REQUIRED first — it admits the fix into her batch, it never
    replaces her.
  - **New campaigns** merge to content-repo main only after the owner has
    played them; machine-ladder green alone is not a merge gate (it remains
    the prerequisite).
  Until its gate is passed a PR stays open. Unit/CI tests alone prove the
  change broke nothing, not that it fixed the target.
- **Write short documents.** Specs/ADRs are owner-consumed via chat summaries;
  their long form exists for agents. Keep them as terse as correctness allows.
- **Audience separation in docs** (owner, 2026-08-02): every document has ONE
  target reader. Agent-facing docs (CLAUDE.md, ADRs, specs, `docs/reference/`,
  skills) may be arbitrarily technical. User/player-facing docs (READMEs,
  release notes, tutorials, storybooks) contain only what that reader needs to
  act — never internal machinery such as model tiers, subagent dispatch,
  worker roles, or pipeline plumbing. Applies to both repos, including the
  content repo's play/hosting tutorials.
- **Version-adoption discipline** (owner, 2026-08-04): whenever a `dsl_version`
  introduces new obligations, adoption rounds for every ACTIVE campaign are
  scheduled within the same milestone — never left to accumulate (the island
  ran four rounds behind the branch-declaration obligations before anyone
  scheduled it). Dormant campaigns are marked upgrade-on-next-touch. A version
  upgrade is always its own explicit, proof-carrying round. Old versions keep
  compiling (per-stage fences); released delves reproduce via their pinned
  engine (`versions.toml` + OCI), not via eternal byte-stable emission.
- **A green gate that binds to nothing is VACUOUS, not a pass** (island rounds
  1–20). A check can be green three ways that mean nothing: *unbound* (it
  matched zero objects — the bot's combat floor gate examined zero enemies for
  nineteen rounds because no actor declared a tier), *unfenced* (the campaign's
  `dsl_version` never reached the surface the gate keys off, so the proof was
  inert), *unemitted* (declared, compiled green, never emitted). Every
  validation artifact states its binding count; a zero binding is a finding and
  is named in the round summary. Full derivation and the other playtest-round
  obligations: `docs/reference/playtest-methodology.md`.
- **A finding is not closed until its general form is a diagnostic** (island
  r7→r10 instance fix; the general rule became `DW0489` eleven rounds later and
  immediately found a second live instance the owner had by then hit herself).
  Every owner finding yields two deliverables — the instance fix, and the
  general form as a diagnostic **re-run against the current build** — or an
  explicit record that only the instance was fixed, which is then a risk item at
  the next staging review.
- **A capability-gap finding blocks staging, not just the backlog** (owner
  rebuke, island round 16). Every island finding that stayed open across more
  than one round was blocked on a missing first-class primitive, never on a
  forgotten task. Triage each finding as content / capability gap the day it is
  reported; a capability gap means the engine work lands before the next
  playtest, or the round summary tells the owner per item that it is still open
  and not to test it. Audit the findings ledger from round 1 — never from the
  last round — before staging any build.
- **Execute an owner ruling at the scope it was given.** Generalizing it is a
  design decision: propose it in one line and wait. (Round 16 turned a
  one-beat ruling into a campaign-wide ceiling and had to be corrected.)
  Unrequested change is a rejection cause on its own, independent of merit — a
  worker's entire island round was rejected wholesale for carrying extras.
- **Tiered testing**: unit + static analysis on every push; PackTest integration on PR;
  full bot playthrough on release candidates only.
- **PR-based flow even solo.** GitHub Actions; repo is private for now, public when
  the owner decides it's ready.
- **Docs are the only persistent memory.** End every session by writing lessons back:
  new constraints → this file; new decisions → an ADR; process learnings → the relevant
  spec. If you fought the codebase and won, record how.
- **Compiler behavior has one live reference.** `docs/reference/compiler.md` is the
  authoritative current-behavior record for `delvec` (DSL surface, emission,
  invariants, the full DW diagnostics catalog); specs stay historical decision
  records. Any PR that changes compiler behavior updates it in the same PR — CI
  enforces the DW-code subset bidirectionally (`tools/check-dw-codes.py`, docs job).
- **Tooling sync** (owner, 2026-08-02): a PR that adds or changes an authoring
  tool or loop updates `docs/reference/` AND every skill whose workflow it
  touches, in the same PR. LLM-facing tools enter skills as mandatory workflow
  steps; human-in-the-loop tools enter as advisory one-line mentions at the
  right step — never blocking, never waiting for a use/don't-use decision. A
  tool absent from docs and skills does not exist for future sessions. The
  inventory of the whole tool surface — every binary, script and flag, with its
  class — is `docs/reference/tools.md`.
- **Every dispatched worker runs in its own git worktree** (owner, 2026-08-05),
  named in the dispatch prompt, never the main checkout — plus the content
  symlink, or two `analyze` tests fail on a fresh tree. Workers **add** a commit;
  they never `--amend`, rebase or force-push a branch that has been pushed unless
  asked by name. Three workers dispatched without the worktree line put two of
  them in the main checkout editing one file at once, on a third party's branch;
  nothing was lost, but one `git add -A` would have swept three authors into one
  commit. Recovering from such a collision is **hunk-granular for every file** —
  the file that leaked was the one a worker had been told it owned — and the
  review asks for a full re-audit, never a targeted deletion: the planner named
  two leaked hunks and there were three. Code leaks fail CI; doc leaks merge
  green.
- Repeated workflows become skills/slash commands (`/new-campaign`, `/validate`,
  `/release`) — see ROADMAP; design them when the workflow has been done manually twice.

## Conventions

- **Language policy**: the owner may communicate in Chinese or English; all repo
  artifacts — docs, code comments, commit messages, PR descriptions, player-facing
  default strings — are **English-first**. English is the canonical source; any future
  i18n translates *from* the English version, never the reverse.
- Rust: workspace at `crates/`, edition 2024, `cargo fmt` + `clippy -D warnings` clean.
- TypeScript (harness only): strict mode; the harness never contains game logic, only
  assertions and navigation.
- ADRs: sequential numbers, status field (Proposed/Accepted/Superseded), cite sources.
  Never edit an Accepted ADR's decision — supersede it.
- Specs: numbered `spec-NNNN-<slug>.md`, each with an explicit "Acceptance criteria"
  section phrased as machine-checkable assertions.
- Commits/PRs: conventional, small, one concern each.
- **Privacy in repo artifacts** (owner, 2026-08-02): repo content never
  includes the owner's personal information or verbatim personal speech.
  Record decisions impersonally in English ("owner decision, date,
  rationale"); quotes and personal context stay in local agent memory or
  `docs/notes/private/` (gitignored). Applies to every repo, including
  campaign GENERATION logs in the content repo.
- **Attribution ledger** (owner, 2026-07-31): any PR that adopts a third-party
  library, ports an algorithm, or leans on a paper adds its entry (with verified
  license) to `docs/ACKNOWLEDGEMENTS.md` in the same PR. Unlicensed sources are
  ideas-only — never ported.
- **DW-diagnostic coverage** (owner, 2026-07-31): every DW diagnostic must be
  covered by at least one test asserting its code, CI-enforced
  (`tools/check-dw-codes.py`, docs job) — a minimal, justified allowlist is the
  only exemption.

## Environments

- **Dev**: the owner's workstation (macOS). Everything must run locally.
- **CI-equivalent**: `validation/` docker compose profile — the same image CI uses.
  "Works on my machine" means "the compose profile passes".
- **Prod**: owner's Raspberry Pi (delve hosting only) — implies multi-arch images
  (amd64 + arm64) at release time.
