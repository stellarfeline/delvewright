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
gallery/             # the ENGINE's own campaign: one instance of every surface
                     #   the DSL declares, built on every PR, never released or
                     #   staged. Its piece is generated, never committed.
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
- **No hacks at any layer**: if vanilla/NBT provides an intended primitive that
  content needs, the DSL exposes it first-class — never leave it to downstream
  folklore or workarounds. If the only possible implementation of a feature is a
  lower-layer hack (e.g. raycast polling where vanilla has no primitive), the
  feature is excluded until vanilla provides one. Applies at every layer boundary:
  NBT→compiler, compiler→DSL, DSL→skill.
- **When the record does not answer a craft question, the answer is RESEARCHED
  against established practice — never invented.** This is a research-grade
  integration project: the default is that a mature answer exists in some
  discipline and has not been looked up. Not knowing how to do something is
  evidence that the research is incomplete, not licence to think a scheme up.
  It binds at every layer — level-design craft, algorithms, tooling, review
  method — and it binds hardest where the question *feels* like a matter of
  taste, because that is where an invented answer is least likely to be
  challenged. Worked example, and the shape to recognise: asked to light an
  interior, an invented scheme distributes emitters by a periodic rule or paves
  a floor with a glowing block; the researched answer is that light is
  motivated, that pools of light and dark are a navigation grammar, and that
  the eye reads contrast rather than brightness. Four obligations follow.
  **State per rule whether it is cited or authored** — an invented scheme and a
  researched one read identically in a report unless the report says which, and
  that indistinguishability is the actual danger. **Name the weak spots**: a
  claim no source supports is written down as unsupported, not smoothed over.
  **Land the research where the next session finds it** — a record under
  `docs/reference/`, not in a session's scratch, or the same question is
  invented again. And **record the gap against the line that should have
  covered it**, so an incomplete research record is a finding rather than a
  permanent hole. The bound: research answers the question that was asked and
  stops. Unlicensed sources are ideas-only (ADR-0013, `ACKNOWLEDGEMENTS.md`).
- **This is a general engine. Primitives are abstract, flexible and
  configurable, and never bound to one campaign's design.** A creator must be able
  to build **any** content with it. A primitive encodes a *mechanism* — a thing a
  player can press, a region that can be sealed, a body that can walk a route —
  never a *design decision* about what the mechanism is for; the genre being built
  this month is content, and content lives in campaigns. Test before adding any
  surface: **could a creator making an entirely different game want this, and can
  they configure it to their own fiction?** If it only makes sense inside this
  month's delve, it is authored content wearing a primitive's clothes.
  Corollary: **a capability belongs to the object class it acts on, not to the
  verb that first needed it.** Built onto the verb, the second object that needs
  it has no surface, and the fix looks like a second bespoke field — **a second
  bespoke field is the defect, not the fix.** Generality is decided at the FIRST
  site: retrofitting at the second means rewriting every call site and every
  proof written against the narrow one — and the second object arrives long
  after anyone remembers why the first was shaped that way.
  **Three shapes to look for in review**, hardest last:
  1. *Keyed to the verb, not the object class.* Tell: `"X, mirroring Y"` in a doc
     comment; a hook on one variant of a sum type but not its siblings.
  2. *A general mechanism privately re-implemented inside a verb.* The worst kind:
     the special case works perfectly, nothing ever looks, and every proof, l10n
     pass and diagnostic written for the general path silently misses the private
     copy. Before adding a "missing" hook, ask whether a general mechanism already
     IS that hook.
  3. *The general mechanism exists but its binding is too narrow to reach the
     objects it should.* Reads as a missing feature, and the "fix" adds a new
     mechanism strictly weaker than the one it duplicates. Ask **"what does the
     existing general mechanism fail to reach, and why"** before ever asking
     "what surface is missing".
  Same shape one layer down: a hand-rolled walk enumerating 3 of 5 effect roots is
  a defect of expressibility, not of care.
- **Debug doctrine**: a red check is information, never an obstacle. Never weaken
  a check, test, or threshold — and never reroll a seed — to get green; fix the
  root cause or escalate. Escalating a toolchain bug is success. Preserve every
  debugging lesson in the strongest available form, strongest first: compiler
  diagnostic > tooling default (automate the pitfall out of existence) > generator
  invariant > docs.
  **An intermittent red is never re-run** — it is a finding, and re-running
  discards it; an intermittent failure is an under-specified test — root-cause it.
  (Recorded trap: a `grep -q` readiness probe under `pipefail` SIGPIPEs its
  producer at the match and reads as flakiness.)
  **Non-trivial ad-hoc shell is written for bash, not the interactive shell.** The
  tool layer runs zsh, where an unquoted parameter does not word-split (a `for`
  over a 33-item list runs once) and assigning to `path` destroys `PATH` —
  both vanish under `bash -c`, so anything with a loop, an array or a variable
  holding a list goes through bash. Repo scripts are already
  `#!/usr/bin/env bash`, so no repo check can catch this, and a check would
  red correct scripts. That rule removes only those two traps — the rest of
  this doctrine stands.
  **The dangerous shell idiom is the one that returns a plausible wrong number
  instead of an error**, and an agent's own measurements are where it bites,
  because nothing downstream re-checks them. Recorded forms: hashing `shasum`
  output hashes the file PATHS too; `cargo test --test X` rebuilds the binary
  under `CARGO_BIN_EXE`, resurrecting a reverted perturbation; a `cd` in the
  first clause of a compound command persists through the rest; `git merge-tree`'s
  three-argument form under-reports conflicts against `--write-tree`. **A zero
  from a measurement that disagrees with an independent observer is the
  measurement failing, not the fact being absent.**
  **A computed key is itself a measurement and needs its own confirmation** —
  a lookup can ask the right question about the wrong key and get an honest
  answer (hashing paths instead of contents; an environment probe that resolved a
  variable's NAME from a grep matching two lines). Before trusting an answer,
  check what the question resolved to.
  The costliest computed key names an INSTRUMENT: **a frozen measurement names
  its instrument literally, by exact revision, never through an indirection**
  (a pin variable, "the pinned engine") — otherwise moving the pin silently
  re-reads every recorded figure against a different instrument.
  **A `grep` for a phrase the file wraps across a line break returns zero**, and
  zero reads as absent — ask what the pattern could match before believing what
  it did not.
  **A count equal to its own fetch limit is not a measurement — it is the
  limit.** The repair is not a bigger number but a **refusal when the page comes
  back full**, the one condition under which the answer cannot be trusted.
  Truncation fakes coverage, in the direction that reads as a clean pass.
  **A scripted string replacement that matches nothing is a silent no-op** — an
  edit script asserts its match count before it writes; a replace whose count is
  not exactly what was intended is a failure, not a no-op. (Stated binding
  counts in one line are what catch this from outside.)
  **`git checkout -- <file>` cannot tell "revert my perturbation" from "discard
  my work"** — commit before demonstrating; where the work cannot be committed
  yet, restore the perturbation from a scratch copy, never from git.
  **When a measurement is the deliverable, cross-check the number by a second
  method before reporting it.** Precondition: **a second method that shares the
  first's calibration is not a second method** — the shared premise is rarely
  the arithmetic, it is the configuration underneath (one pin, one symlink, one
  `target/`, one checkout); ask of a cross-check *what does this share with the
  first one* — the strongest second method has an unrelated failure mode: a
  different instrument, a different tree, or an observer outside the machine.
  Mirror image: **a second method must differ where the suspicion is and agree
  everywhere else** — isolate the one variable; re-deriving the rest is a second
  measurement to get wrong.
- **CI is the sole arbiter** (ADR-0008). Nothing merges red. **Every CI job is a
  required status check** — an advisory job is a job that does not gate. Branch
  protection matches a context by its NAME STRING, so a renamed job blocks every
  PR forever, including the one that would fix it:
  `.github/required-status-checks.txt` and `tools/check-required-contexts.py`
  hold the names in lockstep, in both directions, so a rename or a new advisory
  job is an ordinary red. **CI green is admission to verification, not grounds
  to merge**: unit and CI tests prove the change broke nothing, never that it
  fixed the target. What else a change must pass, and who decides, is operating
  practice.
- **Every validation authoring needs must be runnable on the creator's own
  machine, and this is not negotiable.** The floor is always available: clone the
  repo and build from source. Completeness is guaranteed there, never by the
  convenience layer. **Binary distribution is an optimisation, not the
  guarantee**: where a prebuilt binary cannot carry a capability, never contort
  the binary, drop the capability, or ship a diminished tool — the skill states
  how to build locally and the first run builds from source. Every skill owns an
  explicit **`Init` section that establishes a complete toolchain before any
  work begins**; a tool that cannot be bundled is acquired at the step that needs
  it. A distribution question never decides a capability question.
- **Write short documents.** A spec or ADR is read in full by agents and in
  summary by humans; keep them as terse as correctness allows.
- **Audience separation in docs**: every document has ONE target reader.
  Agent-facing docs (CLAUDE.md, ADRs, specs, `docs/reference/`, skills) may be
  arbitrarily technical. User/player-facing docs (READMEs, release notes,
  tutorials, storybooks) contain only what that reader needs to act — never
  internal machinery such as model tiers, subagent dispatch, worker roles, or
  pipeline plumbing. Applies to both repos, including the content repo's
  play/hosting tutorials.
- **A reader-facing document is written in the present tense of the current
  version.** It says what the thing IS, as if it had always been that way. No
  "used to be X", "originally", "as of vN", no parenthetical citing the internal
  decision a behaviour came from. Two leaks, and the second is the one to watch:
  an internal reference number a stranger cannot resolve; and the *repair* —
  stripping the reference while narrating what it used to assert trades a
  citation for a changelog. Keep the BEHAVIOUR as a plain present-tense fact, or
  delete it. Relocating a historically-worded sentence into `docs/reference/` is
  not a fix — that is a current-behaviour record too. **ADRs are the one place
  history legitimately lives.**
- **Nothing here owes compatibility to anything already built.** This is a
  research-grade integration project: there is no production environment and no
  user on the other side of a compatibility promise. Only the final result
  counts. A change that stops an existing campaign document compiling is not a
  defect — the document is changed or deleted, and that needs no justification.
  Time spent on backward compatibility is wasted, and so is the argument for
  discarding an old artifact: discard it. No compatibility shim, opt-in flag,
  migration path or gradual adoption is added for the benefit of existing
  content. `dsl_version` numbers a surface so a document can say which surface
  it was written against; it is not a promise that the old surface survives.
  What this does NOT relax, because none of it is about history: determinism
  (ADR-0006), the refusal to weaken a check to get green, and a diagnostic
  owing a test. Those are how "the result is good" is measured.
- **A green gate that binds to nothing is VACUOUS, not a pass.** Three empty
  greens: *unbound* (matched zero objects), *unfenced* (the campaign's
  `dsl_version` never reached the surface the gate keys off), *unemitted*
  (declared, compiled green, never emitted). Every validation artifact states
  its binding count; a zero binding is a finding and is named in the round
  summary. Full derivation: `docs/reference/playtest-methodology.md`.
- **A gate nothing INVOKES is not a gate — it is UNRUN**, the fourth vacuity
  mode. A check can be correct in every reviewable way and protect nothing,
  because the obligation to run it lives in a doc line. **A doc line is not an
  invocation.** A gate is done only when the event it guards cannot happen
  without it; the review question is always *what calls this, and what happens
  if someone does the guarded thing without calling it?* Bind it to the event
  (a script step, a compose `depends_on`, a required token), never to a
  checklist. Where the event has several entry points, enumerate them — an
  existence check that only looks where someone pointed is how the shape
  survives review. Where a gate must be skippable, the override is explicit,
  prints what is being overridden, and is shaped so it cannot become habit — a
  convenient override is the same defect one layer out.
- **An opt-out must be secured by a property the defect cannot supply** — the
  sixth vacuity mode: a gate can be bound, invoked, honestly counted and
  falsifiable, yet logically incapable of separating pass from fail, because
  the escape hatch's proof obligation is entailed by the failure it exists to
  catch (an "unreachable, so sealed" opt-out proven by unreachability succeeds
  on exactly the cells that failed). The repair is a **different** demand, one
  the defect cannot supply (a sealed region must itself be closed). Two review
  questions, the second decisive: *what does this opt-out demand* — and *could
  the defect itself produce it?* Applies to every escape hatch,
  acknowledgement and override; a second hatch on the same gate is the defect.
  Where an opt-out is a choice among kinds, the effective obligation is their
  disjunction and is only as strong as the weakest — the kind must be
  determined by the object, never picked by the author.
- **When one gate's prescription is another gate's refusal, the defect belongs to
  the PAIR.** Each half can be correct and the union unsatisfiable, reachable by
  an ordinary merge. The review question is never only *is this check right* but
  ***what does its remedy oblige, and does anything refuse that***. A gate that
  names a remedy owes a check that the remedy is **reachable**; where two gates
  guard one artifact, they are read together or not at all. Tell: a guard that
  carefully qualifies two of the three things its artifact holds was written
  against the cases its author had met.
- **A checker reads a document the way its CONSUMER reads it.** A gate over a
  repository document is only as true as its parse: where its reading differs
  from the reading the document actually gets, the gate passes on something no
  reader can see (a markdown renderer ends a table at a blank line; a checker
  that does not is counting rows no reader sees). Two obligations: the reading
  is **cross-checked against a real implementation of the format** and that
  comparison is committed; and the parse rule is **one shared authority**,
  never a private copy per gate.
- **A command whose response nobody reads cannot fail.** A site that issues a
  command to a server and discards the reply asserts an effect it has not
  established, and one bad line costs the whole function silently. A live
  command goes through the shared rejection rule (`tools/lib/rcon.{sh,mjs}`);
  an EMITTED command is checked against the pinned command tree by the emitter,
  not by a test, because the operator running the tool does not run
  `cargo test`. Both are bound in CI by `tools/check-live-commands.py`. The
  generalisable half: a correct rule living inside ONE call site's `ok()` gives
  the next two callers nothing to reuse — extract it.
- **A finding is not closed until its general form is a diagnostic.** Every
  playtest finding yields two deliverables — the instance fix, and the general
  form as a diagnostic **re-run against the current build** — or an explicit
  record that only the instance was fixed, which is then a risk item at the
  next staging review.
- **A capability-gap finding blocks staging, not just the backlog.** Triage each
  finding as content / capability gap the day it is reported; a capability gap
  means the engine work lands before the next playtest, or the round summary
  says per item that it is still open and not to test it. Audit the findings
  ledger from round 1 — never from the last round — before staging any build.
- **A release is built from a frozen approved tree, never from a moving branch.**
  A release names the exact tree that was accepted; only files that cannot reach
  the shipped artifact (release plumbing) may be added on top, each named in the
  release request. A release refuses when the campaign tree differs from the
  approved baseline by anything unnamed.
- **Tiered testing**: unit + static analysis on every push; PackTest integration on PR;
  full bot playthrough on release candidates only.
- **PR-based flow even solo.** GitHub Actions. **Both repos are PUBLIC** —
  `stellarfeline/delvewright` and `stellarfeline/delvewright-campaigns` — so
  public distribution channels (GitHub Releases, crates.io, GHCR) are open to us
  by default (ADR-0017).
- **Docs are the only persistent memory.** End every session by writing lessons back:
  new constraints → this file; new decisions → an ADR; process learnings → the relevant
  spec. If you fought the codebase and won, record how. A stale premise in THIS
  file is not a stale comment — it is a wrong conclusion, repeated every session.
- **Compiler behavior has one live reference.** `docs/reference/compiler.md` is the
  authoritative current-behavior record for `delvec` (DSL surface, emission,
  invariants, the full DW diagnostics catalog); specs stay historical decision
  records. Any PR that changes compiler behavior updates it in the same PR — CI
  enforces the DW-code subset bidirectionally (`tools/check-dw-codes.py`, docs job).
- **Tooling sync**: a PR that adds or changes an authoring tool or loop updates
  `docs/reference/` AND every skill whose workflow it touches, in the same PR.
  LLM-facing tools enter skills as mandatory workflow steps; human-in-the-loop
  tools enter as advisory one-line mentions at the right step — never blocking.
  A tool absent from docs and skills does not exist for future sessions. The
  inventory of the whole tool surface — every binary, script and flag, with its
  class — is `docs/reference/tools.md`.
- **A campaign is never the engine's test surface. Engine surfaces are
  exercised against the gallery** (spec-0039) — a real campaign's content and
  its engine use cannot be separated, which is exactly what disqualifies it as
  a test surface. A campaign that stops building under a new engine is not a
  finding about the engine: **the campaign adopts, or it is deleted.** There is
  no released-versus-in-development distinction to triage, because nothing is
  released and nothing is owed compatibility.
- **Every engine surface owes a gallery element, in the same PR.** The coverage
  gate enumerates its unit set from the compiler's own `schema --stage all`
  export — the single authority, never a parser of the source — so a new schema
  property or enum variant is an unbound unit the moment it lands. A unit is
  either **bound** in the gallery domain or **refusal-proven** by a committed
  probe the engine actually rejects with a named code. **No third state and no
  prose exemption**: the hatch demands a machine-produced refusal, which
  "nobody authored it" cannot supply. Distinct from the demo-level rule: a demo
  teaches ONE mechanic to a human and is queued; a gallery element is coverage
  and lands with the surface. Vanilla registry values (block, sound, potion
  ids) are data, never units. The gallery also owes **legibility**: a creator
  reading it sees what the engine builds and which checks fire.
- **Every new mechanic owes a demo level**: the PR that lands a mechanic adds
  its row to `docs/demo-levels.md` — the first-party showcase queue of small
  levels that verify one mechanic and document it by example. Not necessarily
  built when the mechanic lands, but always queued.
- **Buildings are judged at playable scale**: a structure reads as what it
  depicts, and its interior belongs to the same theme. Fine detail is
  deliberately dropped — build-art convention scales the referent up so detail
  survives; a delve is walked at player scale, so a cathedral is a
  cathedral-sized cathedral and the **silhouette carries the recognition the
  detail cannot**. The review question is always "does this read as the thing,
  and does the inside belong to it", never "is the detail right" — a piece is
  not rejected for lacking detail it was never going to have.
- **Grandeur is playable content, not volume.** A structure is grand because
  there is a lot in it to play. **A big empty room is a small building that
  costs more to walk across.** The silhouette earns recognition from outside,
  the density from inside: ask of an oversized space *what does the player do
  in here*, and a space with no answer is cut or filled. Applied to objects:
  **when the vanilla block that names a thing is too small to carry the weight
  the story gives it, the thing is built out of blocks** — placing the block
  that shares the name is labelling the object, not depicting it.
- **A clean auto-merge is not evidence of semantic compatibility.** When two
  branches change one subsystem's *intent*, the dangerous hunk is the one git
  resolves **without a conflict marker** — it compiles, passes clippy and every
  existing test, and can undo the other branch's whole point. **Enumerate what
  each branch claims to DO, and re-demonstrate every claim on the merged
  tree**; a textual conflict count measures nothing. Corollaries: docs merge as
  text and are never re-read — re-read them; and an integration is the first
  place a **cross-feature interaction** exists at all — name such pairs up
  front, and the test that covers one goes in with the merge.
- Repeated workflows become skills/slash commands (`/new-campaign`, `/validate`,
  `/release`) — see ROADMAP; design them when the workflow has been done manually twice.

## Conventions

- **Language policy**: all repo artifacts — docs, code comments, commit messages,
  PR descriptions, player-facing default strings — are **English-first**. English
  is the canonical source; any future i18n translates *from* the English version,
  never the reverse.
- Rust: workspace at `crates/`, edition 2024, `cargo fmt` + `clippy -D warnings` clean.
- TypeScript (harness only): strict mode; the harness never contains game logic, only
  assertions and navigation.
- ADRs: sequential numbers, status field (Proposed/Accepted/Superseded), cite sources.
  Never edit an Accepted ADR's decision — supersede it.
- Specs: numbered `spec-NNNN-<slug>.md`, each with an explicit "Acceptance criteria"
  section phrased as machine-checkable assertions.
- Commits/PRs: conventional, small, one concern each.
- **Privacy in repo artifacts**: repo content never includes personal information
  or verbatim personal speech, and no repository artifact records who decided
  something or when they said it. State a rule impersonally, as a fact about the
  software; personal context and the record of who decided what stay in local
  agent memory or `docs/notes/private/` (gitignored). Applies to every repo,
  including campaign GENERATION logs in the content repo. The sanctioned
  repository identifiers are ADR numbers, spec numbers and DW codes — a task id,
  a PR number or a dated attribution is not one.
- **Attribution ledger**: any PR that adopts a third-party library, ports an
  algorithm, or leans on a paper adds its entry (with verified license) to
  `docs/ACKNOWLEDGEMENTS.md` in the same PR. Unlicensed sources are ideas-only —
  never ported.
- **DW-diagnostic coverage**: every DW diagnostic must be covered by at least one
  test asserting its code, CI-enforced (`tools/check-dw-codes.py`, docs job) — a
  minimal, justified allowlist is the only exemption.

## Environments

- **Dev**: a developer workstation (macOS). Everything must run locally.
- **CI-equivalent**: `validation/` docker compose profile — the same image CI uses.
  "Works on my machine" means "the compose profile passes".
- **Prod**: a delve-hosting single-board computer — which is why release images are
  multi-arch (amd64 + arm64).
