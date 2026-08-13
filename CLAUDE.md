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
- **This is a general engine. Primitives are abstract, flexible and
  configurable, and never bound to one campaign's design** (owner decision,
  2026-08-06). A creator must be able to build **any** content with it, not a
  second delve shaped like the first. So a primitive encodes a *mechanism* — a
  thing a player can press, a
  region that can be sealed, a body that can walk a route — and never a
  *design decision* about what that mechanism is for. The genre we happen to be
  building (souls-like, box-garden) is content, and content lives in campaigns.
  The test, applied before any surface is added: **could a creator making an
  entirely different game want this, and can they configure it to their own
  fiction?** If it only makes sense inside the delve we are writing this month,
  it is authored content wearing a primitive's clothes.

  The corollary that keeps it honest: **a capability belongs to the object class
  it acts on, not to the verb that first needed it.** `close-gate` owns
  `sealed_hint`, which encodes *answering a player who presses this thing* — a
  property of anything right-clickable, and nothing to do with closing a gate.
  Built onto the verb, it leaves the second object that needs it with no surface,
  so the fix looks like a second bespoke field on `shortcuts[]`. **A second
  bespoke field is the defect, not the fix.** Generality is decided at the FIRST
  site: retrofitting at the second costs a `dsl_version` bump, per-stage fences,
  and an adoption round on every active campaign.

  **Three shapes to look for in review**, hardest last:
  1. *Keyed to the verb, not the object class.* The second consumer has no
     surface, so the fix looks like another field. Tell: `"X, mirroring Y"` in a
     doc comment; a hook on one variant of a sum type but not its siblings.
  2. *A general mechanism privately re-implemented inside a verb.* `EnvTrigger`
     (`at` + `on: strike|use` + `effects[]`) already **is** "give any scene
     object a custom left- or right-click response, and the response is any
     effect — prose, a sound, a sprung trap, a command". So `sealed_hint` is not
     a missing feature; it is a private copy of a general one. Worst kind,
     because the special case works perfectly and nothing ever looks — and every
     proof, l10n pass and diagnostic written for the general path silently does
     not cover the private one.
  3. *The general mechanism exists but its binding is too narrow to reach the
     objects it should.* A trigger's interaction body is a **point at a cell**,
     not the clickable **shape** of the object at that anchor — so authoring the
     island boulder's own pattern on a shortcut door compiles clean and ships a
     box pressable only from the side the door opens from. This reads as a
     missing feature, and the "fix" adds a fourth mechanism — typically a new
     authoring section strictly weaker than the mechanism it duplicates. Ask
     **"what does the existing general mechanism fail to reach, and why"** before
     ever asking "what surface is missing".

  Same shape one layer down as a hand-rolled walk enumerating 3 of 5 effect roots
  (#301/#302/#321): a defect of expressibility, not of care.
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
  **Non-trivial ad-hoc shell is written for bash, not for the interactive
  shell** (owner, 2026-08-12). The tool layer runs the user's login shell, which
  is zsh, and two of this doctrine's recorded traps exist only there: zsh does
  not word-split an unquoted parameter, so `for x in $var` over a 33-item list
  runs ONCE; and assigning to a variable named `path` destroys `PATH`, because
  zsh ties them. Both vanish under `bash -c`. So anything with a loop, an array
  or a variable holding a list goes through bash — and the repo's own scripts
  already do, being `#!/usr/bin/env bash`, which is why no repo check can catch
  this and why a check would red two correct scripts.
  **That rule removes two of the six recorded instances and no more**, which is
  the reason the next paragraph is not replaced by it.
  **The dangerous shell idiom is the one that returns a plausible wrong number
  instead of an error**, and an agent's own measurements are where it bites,
  because nothing downstream re-checks them. Six in one session, each of which
  first read as a real finding — and only two were about the shell at all.
  Hashing `shasum` output hashes the FILE PATHS too, so comparing two
  differently-named output dirs called all 62 expansions different when 0 were.
  `cargo test --test X` rebuilds the binary under `CARGO_BIN_EXE`, so a reverted
  perturbation stayed live in the binary then used to build campaigns, and a
  real binding read as zero. A `cd` in the first clause of a compound command
  persists through the rest of it, so `gh` and `git worktree` ran against the
  wrong repository and reported ten live PRs as nonexistent. And `git
  merge-tree`'s three-argument form reports no conflicts where the modern
  `--write-tree` form reports five files — **a zero from a measurement that
  disagrees with an independent observer is the measurement failing, not the
  fact being absent.**
  Hence the obligation, stated where it can bind: **when a measurement is the
  deliverable, cross-check the number by a second method before reporting it.**
  Three of the six were caught only after being reported.
- **CI is the sole arbiter** (ADR-0008). Nothing merges red. The owner reviews PR
  descriptions and architecture-level diffs, not lines. Write PR descriptions
  accordingly: what changed at the design level, what CI now proves.
  **Every CI job is a required status check** (owner, 2026-08-05): an advisory
  job is a job that does not gate — at three of ten required, `tier 2` (datapack
  load plus the whole generated PackTest suite) did not block a merge. Because
  branch protection matches a context by its NAME STRING, a renamed job blocks
  every PR forever, including
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
    the prerequisite). **An in-progress campaign lives on its own development
    branch, and EVERYTHING of it lands there** — design of record, prefabs,
    stage JSON, l10n sidecars, generation logs. However many sub-pieces the
    work is split across, each merges into that branch; the branch reaches
    main once, after acceptance. **Sort a file by which artifact it belongs
    to, never by what kind of file it is**: if abandoning the campaign would
    delete it, it is the campaign, and a design document is no exception. A
    general-purpose engine primitive is not campaign content and is untouched
    by this.
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
- **A reader-facing document is written in the present tense of the current
  version** (owner, 2026-08-11). It says what the thing IS and how to use it,
  as if it had always been that way. No "this used to be X and is now Y", no
  "originally", "formerly", "as of vN", no parenthetical citing the internal
  decision a behaviour came from. An outside reader does not care how the
  software arrived at its present shape, and **a page that keeps telling them
  reads as a half-finished project** — which is the cost, and it is paid on the
  page a stranger lands on first.
  Two ways this leaks in, and the second is the one to watch. The obvious one
  is an internal reference number (`spec-0001`, a task id, a PR link) on a
  crates.io front page: a stranger cannot resolve it and gains nothing.
  The subtle one is the *repair*: stripping the reference while narrating what
  it used to assert just trades an unresolvable citation for a changelog. Keep
  the BEHAVIOUR as a plain present-tense fact, or delete it — a shorter true
  page beats a page that explains itself. Relocating a historically-worded
  sentence into `docs/reference/` is not a fix either; that file is a
  current-behaviour record too. **ADRs are the one place history legitimately
  lives**, because superseding is their mechanism.
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
- **A gate nothing INVOKES is not a gate — it is UNRUN**, the fourth vacuity
  mode, and this project has shipped it five times. A check can be correct in
  every way that is reviewable — right verdicts, honest red list, fails in the
  direction that actually drifts — and still protect nothing, because the
  obligation to run it lives in a doc line. `bin/lab-audit.py` is the worked
  example: its own commit message promised staleness would be "measured not
  remembered", and it shipped a script that had to be remembered; the record
  went stale twice more and needed four backfills. **A doc line is not an
  invocation.** So a new gate is not done when it is correct — it is done when
  the event it guards cannot happen without it, and the review question is
  always *what calls this, and what happens if someone does the guarded thing
  without calling it?* Bind it to the event (a script step, a compose
  `depends_on`, a required token), never to a checklist. Where the event has
  several entry points, enumerate them — an existence check that only looks
  where someone pointed is how the shape survives review. Where a gate must be
  skippable, the override is explicit, prints what is being overridden, and is
  shaped so it cannot become habit; a convenient override is the same defect one
  layer out. (Staging gate, task #341: the enumeration found a third path —
  the release workflow — that neither reviewer had named.)
- **An opt-out must be secured by a property the defect cannot supply** — the
  sixth vacuity mode, and the only one that survives every check the previous
  five ask for. Such a gate is bound, invoked, reports an honest binding count
  and is falsifiable in principle; it is nonetheless **logically incapable** of
  separating pass from fail, because the escape hatch's own proof obligation is
  entailed by the failure it exists to catch. Worked example: a prefab contract
  let an author mark unreachable floor as "sealed — no body goes here", proved
  by *showing those cells are unreachable*. That is the identical property that
  made them a finding, so sealing was guaranteed to succeed on exactly the cells
  that had failed, and a 90-line script that read the checker's own red list and
  sealed everything in it turned a broken building green. The repair is not a
  stronger threshold — it is a **different** demand: a sealed region must itself
  be closed, which is what stranding cannot supply.
  Two review questions, and the second is the one that catches this: *what does
  this opt-out demand* — and *could the defect itself produce it?* Applies to
  every escape hatch, acknowledgement and override, and a second hatch on the
  same gate is the defect rather than the fix. Where an opt-out is a choice
  among several kinds, the effective obligation is their **disjunction** and is
  only as strong as the weakest, so the kind must be determined by the object
  rather than picked by the author.
- **A command whose response nobody reads cannot fail** (task #70). A site that
  issues a command to a server and discards the reply is asserting an effect it
  has not established, and it stays green forever: `delve-admit`'s gallery
  emitted four legacy camelCase gamerules and a `text_opacity:255b`, 1.21.11
  refused to load `admit:load` and `admit:finish` **in their entirety** — one bad
  line costs the whole function — and the tool shipped a world with no
  objectives, nothing forceloaded and nothing placed, for as long as it existed.
  So: a live command goes through the shared rejection rule
  (`tools/lib/rcon.{sh,mjs}`); an EMITTED command is checked against the pinned
  command tree by the emitter, not by a test, because the operator running the
  tool does not run `cargo test`. Both are bound in CI by
  `tools/check-live-commands.py`. The generalisable half is not the identifier
  list — it is that **the rule lived, correct, inside ONE spike's `ok()`**, so
  the next two callers had nothing to reuse and wrote the unchecked version.
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
- **Execute an owner ruling at the scope it was given.** Widening a one-beat
  ruling into a campaign-wide rule is a design decision: propose it in one line
  and wait. Unrequested change is a rejection cause on its own, independent of
  merit — a worker's entire island round was rejected wholesale for carrying
  extras.
- **A settled ruling is never re-asked. Search the record first** (owner rebuke,
  2026-08-08). Re-asking spends the scarcest resource in the project on something
  a grep would have answered: one such question — "are traps redstone or
  commands" — was the TITLE of `spec-0022-traps-v2-command-driven.md`, her own
  directive of 2026-08-03 sitting in the repo. Before any question: the specs and
  ADRs, `docs/reference/`, the private handoff notes, then prior session
  transcripts. Ask only what none of them contain.
- **A release is built from a frozen approved tree, never from a moving branch**
  (owner ruling, 2026-08-08). A release names the exact tree the owner accepted;
  only files that cannot reach the shipped artifact (release plumbing) may be
  added on top, and each is named in the release request. Tagging `main` at
  release time is what shipped a package the owner had not approved and could
  not have detected: the pipeline had no place to ask "is this the thing that
  was accepted", so nothing asked. A release refuses when the campaign tree
  differs from the approved baseline by anything unnamed.
- **Tiered testing**: unit + static analysis on every push; PackTest integration on PR;
  full bot playthrough on release candidates only.
- **PR-based flow even solo.** GitHub Actions. **Both repos are PUBLIC** —
  `stellarfeline/delvewright` and `stellarfeline/delvewright-campaigns` — so
  public distribution channels (GitHub Releases, crates.io, GHCR) are open to us
  by default (ADR-0017).
- **Docs are the only persistent memory.** End every session by writing lessons back:
  new constraints → this file; new decisions → an ADR; process learnings → the relevant
  spec. If you fought the codebase and won, record how. A stale premise in THIS
  file is not a stale comment — it is a wrong conclusion, repeated every session
  (a "private for now" line outlived the fact and cost a planning session hours).
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
- **Every new mechanic owes a demo level** (owner, 2026-08-03): the PR that lands
  a mechanic adds its row to `docs/demo-levels.md` — the first-party showcase
  queue of small levels that verify one mechanic and document it by example. Not
  necessarily built when the mechanic lands, but always queued; building the next
  one is the planning agent's standing idle work.
- **Buildings are judged at playable scale** (owner, 2026-08-12): a structure
  reads as what it depicts, and its interior belongs to the same theme. Fine
  detail is deliberately dropped. Minecraft build art conventionally scales a
  detailed referent up — several blocks per real metre — so that tracery,
  mullions and mouldings survive; that is a different craft with a different
  goal. A delve is walked at player scale, so a cathedral is a
  cathedral-sized cathedral, and the **silhouette carries the recognition the
  detail cannot**. The review question is therefore always "does this read as
  the thing, and does the inside belong to it", never "is the detail right" —
  and a piece is not rejected for lacking detail it was never going to have.
- **Every dispatched worker runs in its own git worktree** (owner, 2026-08-05),
  named in the dispatch prompt, never the main checkout — plus the content
  symlink, or two `analyze` tests fail on a fresh tree. Workers **add** a commit;
  they never `--amend`, rebase or force-push a branch that has been pushed unless
  asked by name. Two workers editing one file in the main checkout is one
  `git add -A` away from sweeping three authors into one commit, and being told
  you own a file does not stop it leaking. Recovering from such a collision is
  **hunk-granular for every file**, and the review asks for a full re-audit,
  never a targeted deletion (one targeted pass named two leaked hunks; there were
  three). Code leaks fail CI; doc leaks merge green.
- **A clean auto-merge is not evidence of semantic compatibility** (integration
  of #395+#400+#402+#403, 2026-08-12). When two branches change the same
  subsystem's *intent*, the dangerous hunk is the one git resolves **without a
  conflict marker**. One branch made an oversize region tile automatically; the
  other had added an early refusal of oversize regions. Git merged the refusal
  in silently. It compiled, passed `clippy`, and passed every test that existed
  on either branch — and it undid the entire point of the other PR. What caught
  it was reading for intent, not any tool. So: **enumerate what each branch
  claims to DO, and re-demonstrate every claim on the merged tree**; a textual
  conflict count measures nothing. Two corollaries from the same round. Docs
  merge as text and are never re-read: three sentences across three reference
  files still asserted the refused behaviour afterwards, and
  `check-doc-dupes`, `check-dw-codes` and `check-reference-versions` all stayed
  green. And an integration is the first place a **cross-feature interaction**
  exists at all — the eye camera over a tiled zone belonged to neither branch,
  so neither could test it; naming such pairs up front is part of the merge,
  and the test that covers one goes in with the merge.
- **A worktree is created by the dispatch and destroyed by the MERGE** (owner,
  2026-08-11). Reclaim it — `git worktree remove` plus the local branch — as the
  last step of merging its work, in the same breath as the evidence entry, and
  reclaim a stopped worker's the moment its work is pushed. Not as a chore to
  notice later: an unbounded set nobody owns is only ever noticed when it takes
  the machine down. It did. 36 worktrees, each carrying a full `cargo target/`
  at 8–15 GB, filled the disk to the point where `Bash` could not open its own
  output file — `df` itself was unrunnable. **The trigger is not the cause**: the
  first diagnosis blamed the three workers running at that moment, which were
  ~25 GB of 200; the cause was every worker since the beginning, none reclaimed.
  Reaching for the most recent change is how an accumulation gets misdiagnosed.
  All 36 trees were clean and pushed, so 21 of them had been pure garbage for
  days. Sweep with `git worktree list`, removing anything whose PR is merged plus
  every detached verification tree (spent once its measurement is reported), and
  `git branch --merged origin/main | grep worktree-agent-` for the harness's own
  throwaway branches. When space is already tight the cheap first move is
  `rm -rf <wt>/target` on every tree but the live one — pure rebuildable output,
  zero risk. Before deleting a tree check BOTH `git status --porcelain` and
  `git log @{u}..HEAD`: dirty is obvious, an unpushed commit is the one that
  cannot be recovered.
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
