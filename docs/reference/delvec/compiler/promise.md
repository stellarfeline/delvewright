# `delvec::compiler::promise`

The reference page for `crates/delvec/src/compiler/promise.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW0860–DW0862, DW0961, DW0964, DW1016, DW1017 — an objective keeps the promise its prompt makes (`compiler::promise`; error; exit 1)

An objective carries two player-facing strings — `title` and `hint` — and two
visibilities, `announcement` and `compass`, each defaulted by the campaign's
`guidance` block (spec-0093, spec-0104), and may declare a `highlight`. These rules are the whole of what the compiler
asserts about them. They are one defect class: what the game tells the party and
what the machine requires are not the same thing.

One emitter fact is load-bearing under most of them and is not visible in the
schema: the activation announcement is emitted **only for an announced
objective** — a `title` whose resolved `announcement` is `shown`
(`Objective::announced`) — and the hint's line is nested inside that guard. An
objective that is not announced prints no activation line and no cue; with a
`title` and a `hint` it is a hidden task whose hint is on its journal page once
it has started, and its completion is told as any titled objective's is
(spec-0137 §3). An objective with a `hint` and no `title` shows *neither*.

`DW0862`, `DW0961`, `DW1016` and `DW1017` judge what the document says — a `hint`
that asks to be shown where nothing will show it, an announcement asked for with
no title to print, a highlight on an act with nothing to light, a compass asked
for on an objective nobody is told about — each a declaration that binds to
nothing, refused rather than ignored. `DW0860` and `DW0861`
require the campaign to have something: they are the forcing function `DW0481`
applies to the story, turned to face the player.

Every run prints a binding line naming what it examined, zeroes included.

| Code | Meaning |
|------|---------|
| `DW0860` | **A failure clock nothing explained.** A `begin-stealth` with a non-empty `on_caught` is a clock: a player outside every zone for `grace_ticks` is punished by that bundle. It is refused when no `narrate` fires at or before the arming in the same effect bundle, or when the interval between the last such line and the clock's first bite is shorter than that line takes to read. The interval is `(arming offset − prompt offset) + grace_ticks` on the arming's own timeline, so a `sequence` step's `at_ticks` counts toward it; the reading floor is `20 + 2 × characters` ticks — one second to appear, then ten characters a second, which at five characters a word is 120 wpm. **The prompt the clock races is the LAST one before it**, not the sum of the bundle and not the longest: three mutually exclusive branch retellings of one beat are ordinary authoring and must not accumulate into a refusal, and the line still being read when the clock starts is the instruction. Prose in `on_caught` does not satisfy it — that line is read after the punishment. **Binding: `begin-stealth` effects with a non-empty `on_caught`**, over every effect root (the population is enumerated from `dsl::for_each_effect_root`, which asserts it reached all of them, never from a remembered list of places effects live). A `timed-gate` is deliberately out of scope: it arms at world load, not at a beat, so no prompt can precede it — what the party is owed there is the chance to watch it (`DW0388`) and a window worth reading (`DW0378`). Prescription: put a `narrate` before the arming saying what is now being asked, and give the clock time for it. Do NOT shorten the line to fit the clock. |
| `DW0861` | **An adopted container nothing distinguishes.** A `collect` that sets `container` adopts a chest or barrel the prefab already placed, and is refused without both a `title` and an `item_name`. Adoption is what creates the ambiguity: the compiler's own chest at `anchor` is a new object that appears the tick the objective activates, whereas an adopted container is scenery the party has been walking past since the beat began, identical to every other barrel the piece put down, and the compiler adds nothing to it. The announcement and the name on the stack inside are then the only two things that can tell one box from its neighbours. **Binding: `collect` objectives with `container` set** — a `collect` that conjures its own chest is not this rule's subject. Prescription: give the objective a `title` and an `item_name`. Do NOT reach for `fill_count`: padding makes the right box read full, it does not say which box is right. |
| `DW0862` | **A prompt the emitter will never show.** An objective authors a `hint` and has no `title`. An announced objective's hint is the second line of its announcement; a hidden task's hint (no announcement, a `title`) is shown on its journal page under its title once it has started (spec-0137 §3); an objective with no title has neither, so the hint is inventoried for translation, rendered into every language sidecar, and never put on a screen. **Binding: every objective** (the binding line counts the announced ones). An objective with no `hint` is not refused: this is a rule about a prompt that was written and is not shown, not about one nobody wrote. Prescription: give the objective a `title`. Do NOT delete the hint to clear it — that silences the beat rather than fixing it. |
| `DW1016` | **A highlight with nothing to light** (spec-0104 §7.5; validation tier, `compiler::promise::check_highlights`). A `highlight` on a `collect` whose items fall from a wave (`dropped_by` — no container stands) or that the world supplies (`gathered` — the engine places nothing), or on an `approach`, `step` or `strike-assembly` trigger (a place, a plate's cell, or an assembly lit as no one body), is refused. On an `interact`, a `collect` with a container or the compiler's chest, a `talk-to`, a `kill`, a `use` or `strike` trigger and a `strike-npc` trigger it lights the block or the body. **Binding line**: `highlight binding: H highlight(s) over A act(s) — B on a block, Y on a body; R refused`. Prescription: remove `highlight`, or give the act a thing to light. |
| `DW1017` | **A compass on an objective nobody is told about** (spec-0104 §6.6; validation tier, `compiler::promise::check_compass`). `compass: shown` on an objective that is not announced — no `title`, or a resolved `announcement` of `hidden` (the message says which) — is refused: the compass would point at a thing the player has no name for, and the line naming what is followed would have nothing to say. **Binding line**: `compass binding: T tracked objective(s) of N — F by their own field, D by the campaign default; R refused`. Prescription: announce the objective, or remove the field. |
| `DW0961` | **An announcement asked for with nothing to announce** (spec-0093 §7). An objective states `announcement: shown` and has no `title`. The announcement's first line is the title; without one the emitter prints no line, plays no cue and marks nothing complete on screen, so the declaration binds to nothing — refused rather than ignored, because an inert opt-in is the vacuous shape: the author believes the party is told, and the party is not. `hidden` with no title is quiet on purpose and is not refused. Prescription: give the objective a `title`, or remove the field. |
| `DW0964` | **A prop on a trigger with no cell of its own** (spec-0093 §6.5, §7; validation tier, `compiler::promise`). A prop is the block an event at its own cell acts on — the block a `use` or a `strike` sits on, the target a `shot` lands on (one table, `TriggerOn::places_prop`, which `DW1090`'s prop rule answers to as well); an approach or a step is a body's position, a `strike-npc` a character and a `strike-assembly` an assembly, so the block would be placed and nothing would read it — refused rather than ignored. Prescription: remove `prop`, or make the trigger a `use`, a `strike` or a `shot`. |

**What these deliberately do not claim.** The ledger's general form behind `DW0862`
is *an objective's prompt names the place and the act that actually complete it*,
and that is not what is proven: a machine cannot read prose for whether it names
the right place. What is proven is the necessary condition — that the prompt
reaches a player at all. The stronger reading was built and measured: keying "the
place" to the objective's quest's declared `area` finds three objectives on a live
campaign, and all three are correct — a quest booked in one area whose objective is
*travelling to* the next names the destination on purpose. A quest's `area` is
where the beat is booked, not where each objective completes, so it is not a sound
proxy for the place. Likewise `DW0861` proves that the party is told what to look
for and what they have found, not that the target is visually distinguishable from
its neighbours; and the ledger's second disjunct — *or every copy satisfies it* —
is not expressible at all, because `container` names one anchor.
