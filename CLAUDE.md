# Delvewright — Agent Constitution

Delvewright is an automated production line that outputs **self-contained Minecraft
adventure "delves" on demand** for a fixed group of 1–4 players. A delve is a 2–3 hour
(10h ceiling), story-driven, box-garden (箱庭) adventure map: adventure mode, class
selection with pre-provided gear, zero grind. It ships as a versioned OCI image — one
`docker run` = a joinable dungeon — and must be **provably completable by machine**
before a human spends their one QA hour on it.

Founding decisions live in `docs/adr/` and originate from the kickoff handoff
(`docs/handoff-2026-07-29.md`). Read the ADR index before proposing architecture.

## This file is half of the constitution

This file holds what anyone building Delvewright must obey to produce a correct
artifact: the architecture, the forbidden zones, the layout, the conventions, and
the engineering doctrine. The other half is **operating practice** — how a
particular deployment of this project is run: how work is dispatched, how a change
is reviewed and merged, how a playtest round is staged, how a decision session is
conducted. That half is deployment-specific, so it is not checked in.

That half lives in **`CLAUDE.local.md`**, which is gitignored. It is loaded by the
same memory loader as this file, so it carries the same force: instructions, not a
page an agent is shown and may skim. Delivering it any other way weakens it —
emitting it from a hook makes it a tool result, which is the standing of a doc line,
and this file's own doctrine says a doc line is not an invocation.

A missing memory file loads silently, so `tools/planner-state.sh` — bound to
`SessionStart` and `UserPromptSubmit --if-stale 12` — **refuses by name when it is
absent**, and states its size when present. A fresh clone is exactly the case where
it is missing, and a silent no-op there would be the UNRUN vacuity mode wearing the
fix's clothes.

So: if you have not been given that page, **you have half a constitution**. Say so
and ask before improvising anything about dispatch, review, merge or staging. Nothing
below is conditional on it — the two halves are disjoint, not layered.

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
docs/reference/      # live behavior records: compiler.md, tools.md, i18n.md,
                     #   grammar.md + how a round is run: playtest-methodology.md
                     #   + how a delve is generated: skill-workflow.md
                     #   + how a prefab is admitted: prefab-procedure.md
                     #   + what a fresh checkout still needs: worktree-bootstrap.md
                     #   + distribution-size.md
docs/ROADMAP.md      # milestones; M1 = hello-world delve
crates/              # Rust workspace: dsl / compiler / grammar / orchestrator /
                     #   admit / schem / render
prefabs/             # tileset GENERATORS + shared invariants. The .nbt library and
                     #   its metadata live in the CONTENT repo, reached through the
                     #   `campaigns/` dev symlink — see prefabs/README.md
harness/             # mineflayer bot tests (TypeScript)
tools/               # auxiliary Python/shell tooling (skins, i18n, CI checks) —
                     #   never shipped in delves
packtest/            # PackTest templates
validation/          # docker compose: headless server + bot, same image as CI & prod
```

## Methodology

- **Spec-driven**: specs carry machine-verifiable acceptance criteria. Implementation
  sessions work against a spec; if none exists, write/propose the spec first.
- **No hacks at any layer**: if vanilla/NBT provides an
  intended primitive that content needs, the DSL exposes it first-class — never
  leave it to downstream folklore or workarounds. If the only possible
  implementation of a feature is a lower-layer hack (e.g. raycast polling where
  vanilla has no primitive), the feature is excluded until vanilla provides one.
  Applies at every layer boundary: NBT→compiler, compiler→DSL, DSL→skill.
- **This is a general engine. Primitives are abstract, flexible and
  configurable, and never bound to one campaign's design.** A creator must be able
  to build **any** content with it, not a
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
     not the clickable **shape** of the object at that anchor — so authoring a
     boulder's own pattern on a shortcut door compiles clean and ships a
     box pressable only from the side the door opens from. This reads as a
     missing feature, and the "fix" adds a fourth mechanism — typically a new
     authoring section strictly weaker than the mechanism it duplicates. Ask
     **"what does the existing general mechanism fail to reach, and why"** before
     ever asking "what surface is missing".

  Same shape one layer down as a hand-rolled walk enumerating 3 of 5 effect roots:
  a defect of expressibility, not of care.
- **Debug doctrine**: a red check is information, never an
  obstacle. Never weaken a check, test, or threshold — and never reroll a seed —
  to get green; fix the root cause or escalate. Escalating a toolchain bug is
  success, not failure. Preserve every debugging lesson in the strongest
  available form, strongest first: compiler diagnostic > tooling default
  (automate the pitfall out of existence) > generator invariant > docs.
  **An intermittent red is never re-run**: it is a finding,
  and re-running discards it. An intermittent failure is an under-specified
  test — root-cause it. Evidence: a `grep -q` readiness probe under `pipefail`
  failed 28 times in 30 on a server that was up, because `grep` exiting at the
  match SIGPIPEs its producer; it read as flakiness for months, cost two
  playtest stagings, and the same idiom sat under both 25565 safety guards.
  **Non-trivial ad-hoc shell is written for bash, not for the interactive
  shell.** The tool layer runs the user's login shell, which
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
  Two of the six share a deeper shape, and naming it catches cases the list
  cannot enumerate: **the lookup asked the right question about the wrong key,
  and the answer came back honest.** Hashing paths instead of contents is one.
  The other is an environment probe that read the variable's NAME out of a config
  file with a `grep` that matched two lines, so the indirection resolved a
  two-line string, found nothing, and reported a live credential as absent —
  which cost a message asking the owner to supply something she had already
  supplied. Nothing errored, because nothing was wrong with the question. So
  before trusting an answer, check what the question resolved to: **a computed
  key is itself a measurement and needs its own confirmation.**
  The costliest such key is the one that names an INSTRUMENT. A pin held in a
  variable, or a document's phrase "the pinned engine", is a computed key whose
  value moves — so **a frozen measurement names its instrument literally**, by
  the exact revision, never through the indirection. Otherwise moving the pin
  silently re-reads every recorded figure against a different instrument, and
  nothing anywhere says so. Of four occurrences in one session of something
  long-lived being judged by a version of a tool it does not have, three named
  that tool by indirection rather than by revision.
  The same trap has a cheap textual form worth naming because it is committed so
  easily: **a `grep` for a phrase the file wraps across a line break returns
  zero**, and zero reads as absent. Ask what the pattern could match before
  believing what it did not.
  And the form that hides best, because the wrong number is printed beside the
  thing that produced it: **a count equal to its own fetch limit is not a
  measurement — it is the limit.** A ledger audit ran `--limit 200` and applied
  the date filter afterwards, so once the window held more than 200 rows the
  oldest were never fetched; it printed `200 merged, 200 logged, 0 MISSING` for
  weeks, and `200` was the cap. The window actually held 239. The repair is not a
  bigger number — that is a fix with an expiry date — but a **refusal when the
  page comes back full**, since that is the one condition under which the answer
  cannot be trusted. Same family as UNTRAVERSED: truncation fakes coverage, and it
  fakes it in the direction that reads as a clean pass.
  The editing counterpart, and it is the cheapest of the family to commit: **a
  scripted string replacement that matches nothing is a silent no-op** — it
  returns the file unchanged and reports success, so a version that was supposed
  to join a ledger simply is not in it. Nothing errors, the diff looks small
  because it IS small, and review reads the intent rather than the absence. The
  live instance was caught only because an unrelated gate **states its binding
  count** and printed eight where nine was due, which is the argument for stated
  bindings in one line. So an edit script asserts its match count before it
  writes; a replace whose count is not exactly what was intended is a failure, not
  a no-op.
  The same family reaches the act of demonstrating, which is when a tree is most
  often perturbed and restored: **`git checkout -- <file>` cannot tell "revert my
  perturbation" from "discard my work".** A round reverting a perturbation that
  way destroyed its own uncommitted edits, and every demonstration that ran
  afterwards passed — against the committed version, which was a different
  instrument. Nothing errored and the output was plausible; it was caught only by
  reading `git diff` before committing. So: **commit before demonstrating**, and
  where the work cannot be committed yet, restore the perturbation from a scratch
  copy rather than from git.
  Hence the obligation, stated where it can bind: **when a measurement is the
  deliverable, cross-check the number by a second method before reporting it.**
  Three of the six were caught only after being reported.
  And the obligation has a precondition that is easy to satisfy by accident and
  fatal when missed: **a second method that shares the first's calibration is not
  a second method** — it measures the same error twice, and the agreement reads
  as confirmation. Two red tests were "confirmed pre-existing" by reproducing
  them on a baseline tree carrying the SAME wrong content symlink; both
  instruments were miscalibrated identically, so of course they agreed. The
  shared premise is rarely the arithmetic — it is the configuration underneath:
  one pin, one symlink, one `target/`, one checkout the shell happened to be
  sitting in. So the question to ask of a cross-check is not *is this a different
  command* but ***what does this share with the first one***, and the strongest
  second method is the one whose failure mode is unrelated: a different
  instrument, a different tree, or an observer outside the machine entirely.
  The mirror image is just as costly and is easier to walk into while obeying the
  rule: **a second method must differ where the suspicion is and agree everywhere
  else.** Re-checking that audit meant varying the fetch limit — instead the
  re-check also re-implemented the ledger reader, missed a legacy row shape the
  shipped reader accepts, and reported twelve missing entries that did not exist.
  It disagreed with the first method for a reason that had nothing to do with the
  question. Isolate the one variable; re-deriving the rest is not extra rigour, it
  is a second measurement to get wrong.
- **CI is the sole arbiter** (ADR-0008). Nothing merges red.
  **Every CI job is a required status check**: an advisory
  job is a job that does not gate — at three of ten required, `tier 2` (datapack
  load plus the whole generated PackTest suite) did not block a merge. Because
  branch protection matches a context by its NAME STRING, a renamed job blocks
  every PR forever, including
  the one that would fix it: `.github/required-status-checks.txt` and
  `tools/check-required-contexts.py` hold the names in lockstep, in both
  directions, so a rename or a new advisory job is an ordinary red instead.
  **CI green is admission to verification, not grounds to merge**: unit and CI
  tests prove the change broke nothing, never that it fixed the target. What
  else a change must pass, and who decides, is operating practice.
- **Every validation authoring needs must be runnable on the creator's own
  machine, and this is not negotiable.** The toolchain is
  *for creators*, so a check that only we can run is not a check the product
  has. The floor is deliberately low and always available: **clone the repo and
  build from source**. Completeness is guaranteed there, never by the
  convenience layer.
  The consequence, and it is what makes the rule cheap to hold: **binary
  distribution is an optimisation, not the guarantee.** Where a platform's
  prebuilt binary cannot carry a capability, that is not a reason to contort the
  binary, drop the capability, or ship a creator a diminished tool — the
  **skill** states how to build locally, and the first run builds from source. A
  skill is a prompt: state it clearly and the agent does it. So every skill owns
  an explicit **`Init` section that establishes a complete toolchain before any
  work begins**, and a tool that cannot be bundled is acquired at the step that
  needs it rather than assumed present.
  This is why a distribution question is never allowed to decide a capability
  question. The two are separate, and only the second is about what the engine
  can do.
- **Write short documents.** A spec or ADR is read in full by agents and in
  summary by humans; keep them as terse as correctness allows.
- **Audience separation in docs**: every document has ONE
  target reader. Agent-facing docs (CLAUDE.md, ADRs, specs, `docs/reference/`,
  skills) may be arbitrarily technical. User/player-facing docs (READMEs,
  release notes, tutorials, storybooks) contain only what that reader needs to
  act — never internal machinery such as model tiers, subagent dispatch,
  worker roles, or pipeline plumbing. Applies to both repos, including the
  content repo's play/hosting tutorials.
- **A reader-facing document is written in the present tense of the current
  version.** It says what the thing IS and how to use it,
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
- **Version-adoption discipline**: whenever a `dsl_version`
  introduces new obligations, adoption rounds for every ACTIVE campaign are
  scheduled within the same milestone — never left to accumulate (one campaign
  ran four rounds behind the branch-declaration obligations before anyone
  scheduled it). Dormant campaigns are marked upgrade-on-next-touch. A version
  upgrade is always its own explicit, proof-carrying round. Old versions keep
  compiling (per-stage fences); released delves reproduce via their pinned
  engine (`versions.toml` + OCI), not via eternal byte-stable emission.
- **A green gate that binds to nothing is VACUOUS, not a pass.** A check can be
  green three ways that mean nothing: *unbound* (it
  matched zero objects — a bot's combat floor gate examined zero enemies for
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
  layer out. On the staging gate the enumeration found a third path — the
  release workflow — that neither reviewer had named.
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
- **A command whose response nobody reads cannot fail.** A site that
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
- **A finding is not closed until its general form is a diagnostic.** An instance
  fix landed eleven rounds before its general rule became `DW0489`, which then
  immediately found a second live instance a playtester had by that point hit
  herself.
  Every playtest finding yields two deliverables — the instance fix, and the
  general form as a diagnostic **re-run against the current build** — or an
  explicit record that only the instance was fixed, which is then a risk item at
  the next staging review.
- **A capability-gap finding blocks staging, not just the backlog.** Every
  playtest finding that stayed open across more
  than one round was blocked on a missing first-class primitive, never on a
  forgotten task. Triage each finding as content / capability gap the day it is
  reported; a capability gap means the engine work lands before the next
  playtest, or the round summary says per item that it is still open
  and not to test it. Audit the findings ledger from round 1 — never from the
  last round — before staging any build.
- **A release is built from a frozen approved tree, never from a moving branch.**
  A release names the exact tree that was accepted;
  only files that cannot reach the shipped artifact (release plumbing) may be
  added on top, and each is named in the release request. Tagging `main` at
  release time is what shipped a package nobody had approved and nobody
  could have detected: the pipeline had no place to ask "is this the thing that
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
  file is not a stale comment — it is a wrong conclusion, repeated every session:
  a "private for now" line outlived the fact and cost a planning session hours.
- **Compiler behavior has one live reference.** `docs/reference/compiler.md` is the
  authoritative current-behavior record for `delvec` (DSL surface, emission,
  invariants, the full DW diagnostics catalog); specs stay historical decision
  records. Any PR that changes compiler behavior updates it in the same PR — CI
  enforces the DW-code subset bidirectionally (`tools/check-dw-codes.py`, docs job).
- **Tooling sync**: a PR that adds or changes an authoring
  tool or loop updates `docs/reference/` AND every skill whose workflow it
  touches, in the same PR. LLM-facing tools enter skills as mandatory workflow
  steps; human-in-the-loop tools enter as advisory one-line mentions at the
  right step — never blocking, never waiting for a use/don't-use decision. A
  tool absent from docs and skills does not exist for future sessions. The
  inventory of the whole tool surface — every binary, script and flag, with its
  class — is `docs/reference/tools.md`.
- **A released campaign is never the engine's test surface, and a campaign that
  stops building on a new engine is a FENCE defect.** A shipped
  campaign declares an old `dsl_version`; per-stage fences exist precisely so that
  document keeps compiling unchanged. When it stops, the finding is in the fence,
  not in the content, and a campaign already accepted is never edited to satisfy a
  new engine. Engine surfaces are exercised against **the gallery** (spec-0039) —
  which exists because a real campaign's content and its engine use cannot be
  separated, and that entanglement is exactly what disqualifies it as a test
  surface. The cost of learning this the other way: three diagnostics reddened an
  accepted campaign and a day went into repairing the campaign, when every one of
  them was a new obligation reaching a document declaring an older version.
  **The complement is an obligation, not an exception: a campaign that has not
  been released adopts.** Anything still in development on its own branch tracks
  the current engine, so its red under a new obligation is an **adoption item on
  the campaign** — scheduled by the version-adoption rule above — and is not a
  fence finding. The fence exists so an *accepted* document keeps compiling
  unchanged forever; it was never licence for live content to fall behind. So the
  triage question is one fact about the artifact, asked before the diagnostic is
  even read: **has this campaign been released or accepted?** Both halves of the
  answer are load-bearing, and reading only the first is how every red starts
  looking like a fence defect.
- **Every engine surface owes a gallery element, in the same PR.** The
  coverage gate enumerates its unit set from the compiler's own
  `schema --stage all` export — the single authority, never a parser of the source
  — so a new schema property or enum variant becomes an **unbound unit the moment
  it lands**, and the gate reds naming it. A unit is either **bound** in the
  gallery domain or **refusal-proven** by a committed probe the engine actually
  rejects with a named code. **There is no third state and no prose exemption**:
  the hatch demands a machine-produced refusal, which "nobody authored it" — the
  defect the gate exists to catch — cannot supply. Distinct from the demo-level
  rule and not a substitute for it: a demo teaches ONE mechanic to a human and is
  queued rather than built, while a gallery element is coverage and lands with the
  surface it covers. Vanilla registry values (block, sound, potion ids) are data,
  never units — exhausting them would make the gallery a registry dump. The
  gallery also owes **legibility**: a creator reading it sees what the engine
  builds and which checks fire, so an element that satisfies the tool and cannot
  be read has met half its obligation.
- **Every new mechanic owes a demo level**: the PR that lands
  a mechanic adds its row to `docs/demo-levels.md` — the first-party showcase
  queue of small levels that verify one mechanic and document it by example. Not
  necessarily built when the mechanic lands, but always queued.
- **Buildings are judged at playable scale**: a structure
  reads as what it depicts, and its interior belongs to the same theme. Fine
  detail is deliberately dropped. Minecraft build art conventionally scales a
  detailed referent up — several blocks per real metre — so that tracery,
  mullions and mouldings survive; that is a different craft with a different
  goal. A delve is walked at player scale, so a cathedral is a
  cathedral-sized cathedral, and the **silhouette carries the recognition the
  detail cannot**. The review question is therefore always "does this read as
  the thing, and does the inside belong to it", never "is the detail right" —
  and a piece is not rejected for lacking detail it was never going to have.
- **Grandeur is playable content, not volume.** Buildings
  should be built grand — but a structure is grand because there is a lot in it
  to play, not because it encloses a lot of air. **A big empty room is not a big
  building; it is a small building that costs more to walk across.** The two
  rules compose: the silhouette earns the recognition from outside, and the
  density earns it from inside. So the question asked of an oversized space is
  always *what does the player do in here*, and a space with no answer is cut or
  filled — never kept because it looks impressive in an elevation.
  The same rule applied to objects, as design guidance: **when the vanilla block
  that names a thing is too small to carry the weight the story gives it, the
  thing is built out of blocks instead.** A vanilla bell is a fitting on a fence
  post; a bell a campaign is named after is a structure a player stands under.
  Placing the block that shares the name is not depicting the object — it is
  labelling it.
- **A clean auto-merge is not evidence of semantic compatibility.** When two
  branches change the same
  subsystem's *intent*, the dangerous hunk is the one git resolves **without a
  conflict marker**. One branch made an oversize region tile automatically; the
  other had added an early refusal of oversize regions. Git merged the refusal
  in silently. It compiled, passed `clippy`, and passed every test that existed
  on either branch — and it undid the entire point of the other change. What
  caught it was reading for intent, not any tool. So: **enumerate what each
  branch claims to DO, and re-demonstrate every claim on the merged tree**; a
  textual conflict count measures nothing. Two corollaries from the same round.
  Docs merge as text and are never re-read: three sentences across three
  reference files still asserted the refused behaviour afterwards, and
  `check-doc-dupes`, `check-dw-codes` and `check-reference-versions` all stayed
  green. And an integration is the first place a **cross-feature interaction**
  exists at all — the eye camera over a tiled zone belonged to neither branch,
  so neither could test it; naming such pairs up front is part of the merge,
  and the test that covers one goes in with the merge.
- Repeated workflows become skills/slash commands (`/new-campaign`, `/validate`,
  `/release`) — see ROADMAP; design them when the workflow has been done manually twice.

## Conventions

- **Language policy**: all repo
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
- **Privacy in repo artifacts**: repo content never
  includes personal information or verbatim personal speech, and no repository
  artifact records who decided something or when they said it.
  State a rule impersonally, as a fact about the software; personal context and
  the record of who decided what stay in local agent memory or
  `docs/notes/private/` (gitignored). Applies to every repo, including
  campaign GENERATION logs in the content repo. The sanctioned repository
  identifiers are ADR numbers, spec numbers and DW codes — a task id, a PR
  number or a dated attribution is not one.
- **Attribution ledger**: any PR that adopts a third-party
  library, ports an algorithm, or leans on a paper adds its entry (with verified
  license) to `docs/ACKNOWLEDGEMENTS.md` in the same PR. Unlicensed sources are
  ideas-only — never ported.
- **DW-diagnostic coverage**: every DW diagnostic must be
  covered by at least one test asserting its code, CI-enforced
  (`tools/check-dw-codes.py`, docs job) — a minimal, justified allowlist is the
  only exemption.

## Environments

- **Dev**: a developer workstation (macOS). Everything must run locally.
- **CI-equivalent**: `validation/` docker compose profile — the same image CI uses.
  "Works on my machine" means "the compose profile passes".
- **Prod**: a delve-hosting single-board computer — which is why release images are
  multi-arch (amd64 + arm64).
