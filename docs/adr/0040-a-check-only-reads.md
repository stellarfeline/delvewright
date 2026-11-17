# ADR-0040: A check only reads — emission never takes a check's result, one entry point answers where a body can be, and a look build runs no check

- **Status**: Proposed
- **Date**: 2026-11-17
- **Source**: a reading of `compiler::emit` (`emit_all`, `prove_world`, the proof helpers) against three lines of the constitution — *one question, one implementation*; *a check confirms intent, it never restricts*; *a look is short, a merge is thorough* — and the build that followed it, measured on the gallery and on the town campaign.
- **Supersedes**: nothing.

## Context

`delvec build` was one function in which the judgements of a build and the writing of its bytes were interleaved, and the bytes were written from what the judgements returned. The world block handed the emitters the NPC and actor walks, the wave seats, the lane routes, the volley and collapse plans and the stake table out of the same calls that refused a build; the assemblies' strike plans, each pulse's range and each assembly's `view_range` were fields of a proof's binding. Three things followed.

- A check could not be removed, skipped or made faster without asking whether the datapack still got written the same: a judgement and a derivation shared a function, so the answer was found by reading it.
- A build made to be looked at paid for every proof of a build made to be shipped.
- "Can a body stand, cross or reach here" had been answered by private copies, each repaired separately when it disagreed with the reach graph.

## Decision

### 1. Three kinds of work, and what each may read

- A **derivation** computes a fact the bytes are written from: a seat, a route, a range, a placement. It reads the campaign, the pieces and other derivations, and it runs in every build. Where it cannot derive part of what was declared — a wave with fewer cells than mobs, a walk with no route — it derives the rest and records the **shortfall**.
- **Emission** writes the player tree — `datapack/`, `server/`, `creator-datapack/`, `resourcepack.zip`, `SKINS.md` — from the campaign and the derivations.
- A **check** judges a declared intent against the derivations and the emitted bytes. What it yields is a refusal, advisories, and files of the validation tree: `validation/`, `packtest-datapack/`, `critical-path.json`, `render-plan.json`. A shortfall is refused by a check, at the point of the build the derivation used to refuse at, in the same code and words. Nothing a check yields is read by a derivation or by emission.

`manifest.json` indexes both trees and is written once the last check has returned.

### 2. The separation is held by types

`compiler::check` owns the types a build's work is written with.

- `Checks` is the bench a build runs its checks on. `Checks::run` takes an `Fn(&mut Sink) -> Result<(), BuildFailure>` — a closure that cannot assign to anything it captured — and returns only pass or refusal.
- `Sink` is where a check writes: a validation file, an advisory. Emission is handed the player tree (`Checks::player`), a map no validation file is in until the bench is closed. A pass over the finished tree that must read both as one (`Sink::whole`) is stopped if it adds, removes or changes a player file.
- `Held<T>` is what one check keeps for a later one — a binding the route proof made that the ledger writer finishes. It is opened only through a `Sink`, which exists only inside a running check. `Held::sealed` takes the judged half of a function that derives and judges in one pass, in the statement that calls it.
- `Derived<T>` is a derivation's result: the value emission writes from, and the shortfalls, which only a check can take out.

A value emission needs is therefore produced outside `Checks::run`, by a derivation that runs whether or not any check does.

Three things the types do not hold, and what holds each instead:

- **A cell of the plan filled late** (`Plan`'s `OnceLock`s) can be written through a shared reference. The ones a check fills — the walks' acts, the shot stances — are read by checks alone; the ones emission reads — the respawn and party seats — are filled by derivations.
- **A derivation that takes its judge from the bench.** The edit replay proves each batch as it lays it, and the relight pass measures the darkness gate over the survey it places fixtures from; each takes one argument saying whether it judges (`edit::replay_taking`'s `enforce`, `light::relight_judging`), and the bench supplies it. A test holds each judging form to its unjudging one over everything the build writes from.
- **The whole.** `tests/it/look.rs` builds each fixture campaign both ways and holds the look build's tree to the proven build's player tree, file for file: a check that came to write a player byte, or an emitter that came to run only when checks do, makes the two differ. The one line that may differ is the quota of a campaign whose PackTest pack's functions need more than its own (*not separated*, below).

### 3. A look build

`delvec build --look` runs parsing, the derivations and emission, and no check: no validation, no analysis, `Checks::run` does not call its closure, `Held<T>` is never filled, a shortfall is refused by nothing, and no validation file is written. Its `manifest.json` carries `look_only: true`.

- `tools/creator/playtest-server.sh … --stage-anyway` builds this way, and `look` is a component of its kept-image key.
- `tools/creator/staging-gate.py` refuses a look build before any row is read; `--stage-anyway` overrides that refusal as it overrides every other, printing it.
- The delve image (`validation/Dockerfile.delve`) refuses a tree whose manifest carries `look_only`, unless the playtest path tells it the image is for looking.

A document `delvec validate` would refuse can stop a look build part-way with an internal error; `delvec build` is what says why.

### 4. Where a body can be

`nav::World` and its reach graph are the one answer to whether a body stands in, crosses or reaches a cell: the rest relation (`World::rest_kind`), the edges out of a state (`World::body_edge_cells`) and the closure (`World::body_reach`). The readers that had each been repaired once already ask it — the seam profile (`DW0877`) through `World::rests_with_body_in`, the contract walk's ends through `World::swum_at`, the clear width and the shut model through `shutmodel::can_be`, the commons flood (`DW0838`) through `World::body_flood` — and `nav::world::reach::agreement` holds them to it over generated courses of six shapes — dry floor, a step, a water surface, a swim, a body afloat, a flush ledge — against the blocks each course was built from. It is red on any difference and red when a shape's run never produces the state it is named for.

### 5. Two latent items

- **A choice option's `happening` is a dated line of the chronicle.** The option a `talk-to` beat is completed by always was; an option a story world's path takes in front of a step, completing no objective, now is too (`flow::JournalStep::choices`).
- **The critical path credits an ambient producer that is not a trigger — a purchase, a trap payload, a disarm — by its flag gate alone** (`Flow::trigger_debts` withholds only trigger-rooted producers). Settling it means the exported path performs the act, which is a new kind of step in the bot's contract, or the build refuses a path that owes a flag to one, which is a new diagnostic. Either changes what a proof asserts and can turn a campaign red; neither is a refactor, and it is a change of its own.

## What was found, and where it stands

Every argument of every writer of the player tree was traced to the statement that produces it. Of the 33 producers, 21 shared a function or a refusal with a judgement.

| How it is held apart now | Sites |
|---|---|
| A shortfall a check refuses (`Derived`) | taken links, respawn seats, branch crossings, party seats, wave seats, NPC walks, actor walks, the stake table, lanes, volleys and collapses, trap hardware, language files |
| The judged half sealed at the call (`Held::sealed`) | holder stances, the assemblies' strike plans, each assembly's `view_range`, each pulse's range |
| The judge taken from the bench | the edit replay, the relight pass |
| Separated outright | the texture rows |
| Not separated | the plan (`Plan::build`): its refusals — an anchor that resolves nowhere, a layout that cannot be solved — stand inside the derivation, and stop a look build as they stop any other. The command quota the delve's `load` declares: a function of the PackTest pack runs under it, so a build that runs its checks states it again over the PackTest pack, and that one line of the player tree can read the validation tree (`Sink::whole_restating`); a look build states the player tree's own need. It is separated when the PackTest pack states a quota of its own. |

A sealed judgement is still computed in a look build; nothing reads it there.

## Consequences

- A check can be deleted, reordered behind another check, or made lazy, and the player tree cannot move: the compiler refuses the program in which it could, and the look test reds on what the compiler cannot see.
- A look costs the derivations and the emitters. What it serves is unproven and says so in its manifest; nothing that reads a manifest to admit a build admits it.
- A proven build writes what it wrote. Over the gallery primary the output tree is byte-identical to the tree built before this decision; over the town the player tree is, and the validation tree differs in the world chronicles, which gain the choice lines of §5.
- A look build of a campaign with a shortfall writes less than was declared — a wave short of its mobs, a walk not taken — and says nothing about it. That is what unproven means.

## Revisit triggers

- A judgement sealed at its call shows in a look build's wall time: the function is split, so the look does not compute it.
- A plan-stage refusal stops a look someone needed: the plan's refusals are classified, and the ones that judge become shortfalls.
- A validation file turns out to be read by a served world.
