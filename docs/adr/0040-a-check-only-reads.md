# ADR-0040: A check only reads — emission never takes a check's result, one entry point answers where a body can be, and a look build runs no check

- **Status**: Proposed
- **Date**: 2026-11-17
- **Source**: a reading of `compiler::emit` as it stands (`emit_all`, `prove_world`, the proof helpers) against three lines of the constitution: *one question, one implementation*; *a check confirms intent, it never restricts*; *a look is short, a merge is thorough*.
- **Supersedes**: nothing.

## Context

`delvec build` is one function, `emit::emit_all`, in which the judgements of a build and the writing of its bytes are interleaved, and the bytes are written from what the judgements return. `prove_world` hands the emitters the NPC and actor walks, the wave seats, the lane routes, the volley and collapse plans and the stake table out of the same calls that refuse a build; the assemblies' strike plans, each pulse's range and each assembly's `view_range` are fields of a proof's binding. Three things follow.

- A check cannot be removed, skipped or made faster without asking whether the datapack still gets written the same: a judgement and a derivation share a function, so the answer is found by reading it.
- A build made to be looked at pays for every proof of a build made to be shipped. On the town that is minutes of proofs before the first byte of a world a person only wants to walk through.
- "Can a body stand, cross or reach here" has been answered by private copies, each repaired separately when it disagreed with the reach graph.

## Decision

### 1. Three kinds of work, and what each may read

- A **derivation** computes a fact the bytes are written from: a seat, a route, a range, a placement. It reads the campaign, the pieces and other derivations. It fails only where there is nothing to write — a wave with no cell to stand on has no `summon` line — and that failure is the derivation's own, raised in every kind of build.
- **Emission** writes the player tree — `datapack/`, `server/`, `creator-datapack/`, `resourcepack.zip`, `SKINS.md` — from the campaign and the derivations.
- A **check** judges a declared intent against the derivations and the emitted bytes. What it yields is a refusal, advisories, and files of the validation tree: `validation/`, `packtest-datapack/`, `critical-path.json`, `render-plan.json`. Nothing it yields is read by a derivation or by emission.

`manifest.json` indexes both trees.

### 2. The separation is held by types

`compiler::check` owns the three types a check is written with.

- `Checks` is the bench a build runs its checks on. `Checks::run` takes an `Fn(&mut Sink) -> Result<(), Failure>` — a closure that cannot assign to anything it captured — and returns only pass or refusal.
- `Sink` is where a check writes: a validation file, an advisory. It has no reader, and its contents join the output tree only when the bench is closed, after the last emitter has returned.
- `Held<T>` is what one check keeps for a later one — a binding the route proof made that the ledger writer finishes. It is opened only through a `Sink`, which exists only inside a running check.

A value emission needs is therefore produced outside `Checks::run`, by a derivation that runs whether or not any check does; a function that both derived and judged is split in two, the judgement reading the derivation's result.

### 3. A look build

`delvec build --look` runs parsing, the derivations and emission, and no check: `Checks::run` does not call its closure, and `Held<T>` is never filled. It validates nothing, analyses nothing, writes no validation tree, and its `manifest.json` carries `look_only: true`.

- `tools/creator/playtest-server.sh … --stage-anyway` builds this way.
- `tools/creator/staging-gate.py` refuses a look build before any row is read; `--stage-anyway` overrides that refusal as it overrides every other, printing it.
- The delve image (`validation/Dockerfile.delve`) refuses a tree whose manifest carries `look_only`; the playtest image, which is built for looking, says so when it builds over one.

### 4. Where a body can be

`nav::World` and its reach graph are the one answer to whether a body stands in, crosses or reaches a cell. Every other reader asks it, and `nav::world::reach::agreement` holds each of them to it: the same generated cases — dry floor, a step, a water surface, a swim, a body afloat, a flush ledge — asked of every consumer, red on any difference, and red when a case shape is missing from the run.

### 5. Two latent items

- The critical path credits an ambient producer that is not a trigger — a purchase, a trap payload — by its flag gate alone.
- A choice option's `happening` is not a dated line of the chronicle.

Each is settled in this round or recorded where it belongs; the finalised text says which.

## Consequences

- A check can be deleted, reordered behind another check, or made lazy, and the player tree cannot move: the compiler refuses the program in which it could.
- A look costs the derivations and the emitters. What it serves is unproven and says so in its manifest; nothing that reads a manifest to admit a build admits it.
- A refusal that used to come from inside a derivation is raised by a check that reads the derivation, at the same point of the build, with the same code and words.
- `emit::build` keeps its signature for callers that want the whole verdict; the kind of build is one argument.

## Revisit triggers

- A check needs a value so costly that deriving it twice — once for emission, once to judge — shows in the build's wall time: the derivation is then shared as a fact, never taken back from the check.
- A validation file turns out to be read by a served world.
