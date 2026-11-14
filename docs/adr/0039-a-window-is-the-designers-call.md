# ADR-0039: A timed window is the designer's call — the machine proves both endings of a clock and never judges its length

- **Status**: Proposed
- **Date**: 2026-11-14
- **Source**: the ruling that the length of a timed task's window is a creative judgement of the same kind as a fight's difficulty; the engine tree at `2e580c80c`, read for the limit's surface (`crates/dsl/src/quest/objective.rs`), its emission (`crates/delvec/src/compiler/emit/limit.rs`), its measurement (`crates/delvec/src/compiler/nav/limit.rs`), its story worlds (`crates/delvec/src/compiler/flow/story.rs`, `docs/reference/delvec/compiler/flow.md`) and its harness steps (`harness/src/executor/expire.ts`, `harness/src/executor/objective.ts`, `harness/src/journal.ts`); spec-0133, spec-0134, spec-0137; `docs/reference/playtest-methodology.md` *What the ladder asserts at a combat encounter*; CLAUDE.md *A check confirms intent; it never restricts* and *A check guards only what our toolchain owns*.
- **Supersedes**: nothing. Loosens spec-0133 (§6, §7's `DW1084`, §9's third probe, acceptance criteria 6 and 11) and amends its §8; each is named under Decision.

## Context

An objective with a `limit` settles one of two ways: **done**, by its act, or **expired**, when its clock runs out while it is pending (spec-0133 §1). Both are story branches. The engine enumerates one expiry world per limited objective (`WorldOrigin::Expiry`, `crates/delvec/src/compiler/flow/story.rs:99-101`; `docs/reference/delvec/compiler/flow.md:19`), the journal ledger states per world how each recorded objective `settles` — `done`, `expired` or `open` (spec-0137 §5; `harness/src/journal.ts:304`) — and a task that runs out of time is "a branch like any other" (spec-0137, its amendment line).

Two things in the tree let a window's length decide an outcome that is not the story's.

**The build refuses a short window.** `nav::limit::judge_limits` prices the critical path's proven legs from the clock's start step to the objective's step at the 4 t/block sprint model and refuses a window with fewer ticks as `DW1084` (`crates/delvec/src/compiler/nav/limit.rs:181-253`, the refusal at `:225-239`; raised at `crates/delvec/src/compiler/emit/mod.rs:1806-1809`). Its message says "no party can do the task in time". That sentence is a claim about players, and the measurement does not support it: the walk is one body's route in the one order the critical path takes. A party of several stands one member at the goal before another starts the clock; a designer writes a window nobody is meant to beat, so that the loss is the story. spec-0133 §6 already says "a window is the author's call, as a monster's difficulty is, and the playtest judges it", and then keeps the walk as a floor. The floor is the same judgement at a smaller size.

**The bot races the clock on every done run.** The clock is the server's own `schedule`, started at the start event (`crates/delvec/src/compiler/emit/limit.rs:156-161`) and cleared only by completion (`limit_completion_lines`, `:183-200`). Nothing in `harness/src` holds it: on a world that settles the objective done, the bot must arrive and act inside the real window, and when it does not, the objective expires and the step fails for want of a completion marker (`harness/src/executor/objective.ts:115-139`). The bot's pace is not a player's — it walks proven legs at its own speed, stages a clear, reads a muster, runs a die-retry stage — so that red measures the harness against the window, and reads as a mechanism defect when it is a statement about length.

The project already answers this question for every other difficulty.

- **A fight.** "The machine verifies mechanism. It does not fight, and it makes no claim about whether a fight can be won" (`docs/reference/playtest-methodology.md:46-50`). The bodies are read, then removed by the engine's own `wave_strike_<wave>` (`crates/delvec/src/compiler/emit/wave.rs:733-735`), and the wiring the kill drives is judged. spec-0023 declines "to model human dodge/aim skill" (`docs/specs/spec-0023-combat-verification-semantics.md:87`).
- **A herd.** "A mechanic that rests on a mob's own AI is judged like combat: the process is skipped and the result judged" (`docs/specs/spec-0134-the-small-surfaces-of-a-festival-day.md:25`). The harness teleports the counted bodies into their region and holds them still; "the herding is the playtest's, not the bot's" (`harness/src/census.ts:35-36`).
- **A dodge.** Whether a body can dodge a hazard is the designer's call and is never checked by the engine. The rule lives in the deployment's operating practice; `docs/reference/maze-craft.md:76` applies it to a maze loop.

A window is the fourth instance and the only one the machine still judges.

## Decision

### 1. The engine never refuses a build over a window's length

No diagnostic compares a window to a walk, a fight, an activity price or any other measured duration. **`DW1084` is removed**: its declaration and `judge_limits`' verdict (`crates/delvec/src/compiler/nav/limit.rs`), its reference row (`docs/reference/delvec/compiler/nav/limit.md`), its gallery probe (`gallery/probes/a-window-shorter-than-the-walk/`), its tests (`crates/delvec/tests/it/limit.rs:443-517`), and the skill line that teaches it (`.claude/skills/delvewright/skills/new-delve/references/quest-capabilities.md:39`). The number is retired, never reissued.

**This is a loosening of spec-0133.** What it stops asserting: §6's refusal and §7's `DW1084`; acceptance criterion 6's red half and its perturbation; §9's third probe and criterion 11's count of three probes. Direction of the loosening: a build whose window no party can meet is now admitted and can ship. It cannot ship a dead end, because the expired ending of that same objective is a world the engine proves reaches an ending (item 2).

What the engine still owes about a limit, all of it the compiler's own output and proven by Rust tests:

- **The schema range**: `ticks` in `20..=720000`, and a non-empty `on_expire` when present — `DW0100` (`crates/dsl/src/quest/objective.rs:21-24`).
- **`from` names an event that exists and precedes what it closes** — `DW1086`; and the player is told a clock runs — `DW1085` (`crates/dsl/src/quest/check.rs:112-120`).
- **The clock is emitted as declared**: scheduled at its start event for exactly `ticks`, cleared by completion, settling only a pending objective, firing `on_expire` and nothing of the completion bundle (spec-0133 acceptance criteria 2–4; `crates/delvec/tests/it/limit.rs`), with the two PackTest templates per limit that step the declared ticks on a server (`crates/delvec/src/compiler/emit/packtest/limit.rs:199-284`).
- **The figure, as a number.** The walk the critical path proves inside each window stays a measurement the build prints and writes to `validation/limit.json` — walk ticks, window, the walk's share — on a line that names no code and carries no verdict. A share above 100% is printed like any other. The author declared the window; the engine states what it measured beside it and stops, as `delvec analyze` shows a way with no journal line "as a number, never refused" (spec-0137 §9).

### 2. Each run takes one ending of each clock, and the clock never picks it

A story world fixes, per limited objective, whether it is done or expired (`flow.md:19`). The run of that world follows the world, not the stopwatch.

- **Done: the clock is held.** For every limited objective its world settles done, the harness holds the clock for the whole run. The mechanism is a **hold latch**: the compiler emits, beside each clock, a function `limit_hold_<obj>` that sets a party latch the clock's own `expire_<obj>` reads and returns on, so a held clock that runs out settles nothing and marks nothing run out. The path file lists the objectives to hold (`held_limits`, derived by the compiler from the world: every limited objective the path carries no `expire` step for); the harness calls each function on its opped channel at the run's open, before the first step, and reads each reply through the shared rejection rule. The harness infers nothing and sends no raw clock command: it runs the engine's own function, as it does to strike a wave or to mock an act's bundle. The done step then judges what it judged before — the objective can be reached and its act completes it — and an `[dw:expired …]` marker for a held objective fails the run by name, as a completion fails an `expire` step today (`harness/src/executor/expire.ts:33-37`).
- **Expired: the clock is run out on purpose.** The world's `expire` step is unchanged (spec-0133 §8; `harness/src/executor/expire.ts:55-99`): the bot stands outside the objective's volume, performs nothing, waits the real window out, and passes on the expiry marker alone. Every step after it proves the story continues from the expiry to an ending; the journal judgement reads that world's `settles: expired` row. Only the expired objective's clock runs; every other limited objective of that world is held.

spec-0133 §8 is amended accordingly: its refusal of a mocked clock stands for the expiry run, whose wait stays real, and no longer describes the done run. That the schedule fires at the declared tick, and that a completion clears it, is proven where it belongs — the emission tests and the two PackTest templates — and is not re-proven by racing a bot.

No latch is set on a player-facing server: nothing but the harness calls `limit_hold_<obj>`, and the datapack's bytes are the same in both.

### 3. Each check sits at the layer that owns what it judges

| What | Whose | Judged by |
|---|---|---|
| How long a window is; whether a party makes it | a creative judgement | the playtest; no machine check exists |
| The clock as emitted: the schedule, its tick count, the two settlements, the guard, the countdown, the hold latch | the compiler's output | Rust tests over emission; the PackTest templates |
| That `schedule` fires at its tick and survives a restart | vanilla behaviour | established once, recorded (`docs/reference/compiler.md:732`, the `timed_gates[]` row spec-0133 §3 cites); never a recurring check |
| Which ending a world takes, that the objective is reachable and doable, that the story continues from either ending | the campaign's content | the bot, on a running server: the done run under a held clock, the expiry run under a real one |

### 4. The general form

**A check that would judge a difficulty is never built.** A fight's danger, a dodge, a herd, a window's length: each is material the designer shapes and the playtest judges. The machine proves two things about a challenge and nothing else — that it can be **reached**, and that **every ending the author declared for it is reachable and the story continues from it**. Where the challenge's process stands between the bot and that proof, the process is skipped by the engine's own function and the artifact says so; the result is what is judged. A proposed check is tested against this before it is specified: if a more skilled or a larger party would change its verdict, it is a difficulty check.

## Consequences

- A designer writes any window the schema admits: a sprint, a window meant to be lost, a window only a split party meets. The build states the walk beside it and refuses nothing.
- A bot run no longer reds because the harness was slower than a window. A done-run red on a limited objective means the objective could not be reached or its act did not complete it.
- A window too short for any party is found at the playtest, not at the build. The party that meets it sees the expired branch, which is proven to reach an ending; what is lost is the done branch's content for that party, and that is a design defect the playtest is for.
- The done run's wall time stops depending on windows; the expiry runs' cost is unchanged and still stated as summed `wait_ticks`.
- The implementation owes, in one change: the removal of item 1; the latch, `limit_hold_<obj>`, `held_limits` and the path contract's format number; the harness's hold at open and its named failure; the gallery element whose perturbation moves an emitted byte (the hold function) and a harness test that reds when the hold is not called on a window the bot cannot meet; the pages (`compiler.md`'s limit rows at `:688` and `:1758`, `nav/limit.md`, `flow.md`, the skill's *A task that can be lost* and its ladder page). The gallery's mezzanine window needs no lengthening for the bot's sake afterwards.
- `validation/limit.json` and the binding line change shape (no code in the line); nothing owes compatibility to the old one.

## Alternatives considered

- **Keep the walk as a floor (spec-0133 §6 as written).** Refused: the floor's claim, "no party can do the task", is about parties, and the measurement is one body in one order. It also forbids the window that is meant to be lost.
- **Demote `DW1084` to a warning.** Refused: a warning is a verdict the author must answer or learn to ignore. The number on the binding line carries the same information with no verdict.
- **Leave the done run under the real clock and lengthen the harness's patience.** Not available: the clock is the server's, and no harness timeout moves it. The bot would still fail whenever its own pace exceeded a window a player meets easily.
- **Hold the clock by raw commands from the harness** (`schedule clear` on the expiry function). Refused: the harness would have to know the engine's function names and re-issue the clear after every start event, a private copy of emission knowledge; a clock started before its objective opens would need the same care a second time. The latch is one function per clock, emitted by the code that emits the clock.
- **`/tick freeze` around timed steps.** Refused: it stops every body and every other schedule, so the step it protects could not run.
- **Run the expired ending out by calling `expire_<obj>` instead of waiting.** Not decided here. It is consistent with item 3 — the tick arithmetic is the compiler's output and is proven by PackTest — and it would remove the expiry runs' wait; it is also a second loosening of spec-0133 §8 that the ruling did not ask for. It stays as the first revisit trigger.

## Revisit triggers

- The summed `wait_ticks` of a campaign's expiry worlds makes the branch tier impractical: decide whether the expiry run calls the engine's own expiry function.
- A playtest finds a window no party could meet and the printed share would have shown it: decide whether the figure needs a clearer place in the build's output — never a refusal.
- A new timed mechanism (a clock on something other than an objective) is specified: it takes item 4 as written, and its done and lapsed endings are worlds.
