# spec-0143: A step's clue is presented before the step — an act may state what tells the player its answer, and a step that does is judged by that instead of by being in sight: on the main line the engine confirms the text is in the journal, or the thing was in sight, before the party reaches it

- **Status**: Proposed
- **Ground**: written against `feature/festival-day` at `4967f5bc8` (`dsl 0.81.0`), read for: `compiler::promise` (`DW0863`, `check_acts_found`, `judge_acts`, `act_unfound`), `compiler::acts` (`ActWalk`, `Act`, `Pass`, `derive_walks`), `Plan::strict_ancestor_steps` and `compute_strict_ancestor_steps` in `plan/path/ancestors.rs`, `Plan::branch_gate_model`, `RegionEvents::armed`, `nav::worlds_at`, `World::find_route`, `framing::in_sight`, `QuestPlanContent::spine` and `optional` in `crates/dsl/src/quest_plan.rs`, `for_each_effect_root` in `crates/dsl/src/effects.rs`, `compiler::telling` (`DW0982`, `DW1045`, `DW1060`); spec-0051, spec-0093 §5, spec-0104 §4, spec-0118, spec-0137.
- **What it is for**: a puzzle whose answer is told somewhere else. A castle's cellar opens when four watch fires are lit in an order a note gives; the fires stand on the walls, 77 blocks apart and hidden from each other by the turrets. `DW0863` refused it, because its rule is that every act is announced with a hint or in sight of where the party stands, and the content moved the fires into the yard to pass. That rule is right for an ordinary task and wrong for a puzzle's step, whose place is part of the puzzle. A step that states its clue is judged by another question, the one thing a machine can know about a puzzle: that the clue the creator says the answer rests on has reached the player before the step.
- **Research**: none commissioned. No craft question is decided here: which clue a step rests on is the creator's statement, and whether a player can reason from the clue to the answer is judged in play. Every rule below is **[authored]** on the cited engine facts.
- **Numbers**: spec `0143`; `dsl_version` `0.86.0`; `DW1136` (§3) and `DW1137` (§4), both used. `DW0863` keeps its number and its rule, and no longer judges a step that states a clue (§1).
- **Non-goals**: judging whether a clue is sufficient, legible or fair; guiding the party to an act (that stays `DW0863`'s); any clue for an optional step; a clue held by a chat line, a sign, a book in a chest or a dialogue line the journal does not keep (§7).

## 1. What a stated clue changes in `DW0863` **[authored]**

`DW0863` (spec-0093 §5, widened by spec-0104 §4) stands as it is for every act that states no clue: the act is announced with a `hint` or in sight from the cell the party stands on before it, a `kill`'s wave is also within reach of that cell, a place walked through is walked with an announced objective armed or the next act in sight, and a `kill` on no walk is judged from the acts that fire its wave. The findings it carries stay carried.

**An act that states a clue is not judged by `DW0863`.** An objective or a trigger with a non-empty `clues` is counted on `DW0863`'s binding line (`… C that state a clue and are judged by the clue rule (DW1137) instead`) and judged by §4 in its place: an act on a walk, a walked-through place whose trigger states one, and a `kill` on no walk alike.

**This is a loosening, in those words, and only for a step that states a clue.** Such a step loses the in-sight-or-hint requirement: with `clues` declared, an act out of sight of the last one with no hint, which `DW0863` refuses, builds — on the main line when its clue is presented before it, and off the main line with nothing asked. A creator can therefore silence `DW0863` on one act by stating a clue for it; what the act then owes on the main line is §4, and an optional act owes nothing. Whether the clue is one a player can use, and whether the step can be found, is judged in the playtest for those steps. Every act that states no clue is judged exactly as before.

## 2. The surface **[authored]**

`clues` is a list on every objective kind and on every environment trigger — the two things `compiler::acts` reads as the steps of a walked path. Each entry names one thing that tells the player the step's answer:

| Entry | The clue | Presented when |
|---|---|---|
| `{ "by": "hint", "objective": "obj/…" }` | a text: that objective's `hint`, on its open journal page | that objective has started |
| `{ "by": "record", "objective": "obj/…" }` | a text: that objective's `record`, on its finished journal page (spec-0137) | that objective is over |
| `{ "by": "sight", "of": "anchor/…" }` | a thing seen: the cell of that anchor | the party has stood where it is in sight |

The step states the clue; the engine never infers one from the step's kind, its place or its prose. A step with no `clues` states none, and `DW0863` judges it. A step with several owes each.

The two text entries are the journal's two pages, which is where the engine keeps text a player can read again. A note the party reads is an objective that is over, and its `record` is the line the journal keeps of it; what a talk told the party is the `record` of the `talk-to`. The castle reads:

```json
{ "id": "trigger/fire-kitchen", "at": "anchor/fire-kitchen", "on": { "on": "use" },
  "clues": [{ "by": "record", "objective": "obj/find-the-note" }] }
```

## 3. A clue names something that exists — `DW1136` **[authored]**

Judged at `delvec validate`, at the clue's own pointer (`…/clues/<i>`): a `hint` or `record` entry naming no objective; a `hint` entry naming an objective with no `hint`, or with no `title` to carry it (`DW0862`); a `record` entry naming an objective with no `record`. Judged at the build, where anchors have cells: a `sight` entry whose anchor resolves to no cell. The repair names the field to write. `delvec validate` prints `clues stated (DW1136): S of A act(s) state C clue(s); R name nothing that presents them`.

## 4. A main-line step's clue is presented before it — `DW1137` **[authored]**

**The main line is derived.** It is the stage-4 spine, `QuestPlanContent::spine` — the finale and every quest its `depends_on` chain demands, the one authority on which quests a body cannot reach the finale without (spec-0051). An objective is on the main line when its quest is in the spine. A trigger is on the main line of a walk when the objective it belongs to — the one it `serves`, else the one whose flag the walked path performs it to pay, else the next objective on that path — is. Where a path happens to put a press says nothing about the story's order, so a trigger is judged as a step of that objective. Nothing about the main line is written on the step.

**An optional step is owed nothing.** A step in a quest outside the spine, or a trigger that belongs to no spine objective on any walked path, is a side puzzle: not knowing how to solve it, and going to look for the clue, is play. Its clues are counted on the binding line and judged nowhere.

**The walks** are `compiler::acts`'s: the exported critical path and every realised branch's own path. A main-line step is judged on every walk that performs it.

**Before is every play order, never one walk's.** An objective `X` is *forced before* a step when no run reaches the step without `X` done:

1. `X` is a strict ancestor of the step (`Plan::strict_ancestor_steps`, and a branch's own from `Plan::branch_gate_model`): in the transitive `after` of the step's objective, or an objective of a quest its quest transitively `depends_on`; for a trigger, a strict ancestor of the objective it belongs to.
2. `X` is the one objective whose completion alone sets a flag that the step, the objective a trigger belongs to, or anything already forced before it, requires — every `set-flag` of that flag hangs in `X`'s `on_objective_complete` bundle and nowhere else (no other objective, no quest completion, no trigger, no dialogue option, no optional root). Taken to a fixed point.

So a chest that must be opened for the story to go on is forced before what it gates, whether it gates by `after` or by a flag; a chest the lineariser merely happened to put earlier is not.

**Presented:**

- a `record` clue, when its objective is forced before the step;
- a `hint` clue, when its objective is the step itself, or the one a trigger `serves` (the press is open only while that objective is pending), or is forced before the step, or is a main-line objective on the walk that the replay holds started (`RegionEvents::armed`) at or before the step;
- a `sight` clue, when its anchor's cell is in sight — `framing::in_sight`, one of nine lines clear, within 64 blocks — from the standing eye of a cell the main line stands on before the step: the walk's entry cell, the place of each step forced before this one, the step's own place, and the cells of the walked route into each of those (`World::find_route` between the cells the walk names, snapped to footing), each in the world the walk holds when it arrives there (`nav::worlds_at`).

A clue that is not presented is refused with `DW1137`, naming the walk, the step, the clue and its pointer, and why: its objective is not on the main line (a clue an optional detour shows does not count for a main-line step); or it comes earlier on this walk and nothing forces that order; or it comes later; or its page has not opened; or its cell is in sight from none of the counted cells, with the nearest reason. Each names its repair.

**What this is, against the engine before it: a loosening for a step that states a clue (§1), and a new refusal only for a declaration this spec adds.** No document that parsed before is judged differently: it states no clue.

## 5. What is proven, and what is not **[authored]**

- Proven: on every walked path, each stated text clue of a main-line step is on a journal page that every run reaching the step has opened; each stated sight clue has a clear line from a cell the model says the main line stands on.
- Not proven, and said so: that the player read the page or looked that way; that the clue is enough; that the route the model walks is the one a player takes — the route is one proven way between two forced places, and a thing seen from it is a thing a player walking that way sees.
- Conservative, in the direction that reds: a flag with a second setter, or one set from a dialogue option, forces nothing (rule 2); a `hint` clue's page is credited from the replay of the walks, not from every order. A creator whose order is real states it with `after`.

## 6. The binding, the ledger, the gallery **[authored, under spec-0039]**

The build prints `DW1137 binding: S of A act(s) state C clue(s) — M on the main line of W walk(s), J clue judgement(s): T text clue(s) and V sight clue(s) presented before their act, R refused; O optional act(s) owed nothing; N act(s) state no clue and are owed nothing`, zeroes included, and writes `validation/clues.json`: the counts (`examined` is the judgement count) and a row per stating act — its id, `main` or `optional`, the walks that judge it, and each clue's verdict with how it was presented.

The gallery binds `clues` on a trigger and on each of the five objective kinds, with every `by` entry. Three probes: `a-clue-in-an-optional-bay` (a main-line valve resting on the hint of an optional quest's objective: `DW1137`), `a-clue-read-after-the-step` (a step resting on a record the walk writes later: `DW1137`), `a-clue-that-names-no-text` (a `record` clue naming an objective with no `record`: `DW1136`).

## 7. What it leaves out, by name **[authored]**

- A clue held by text the journal does not keep: a `narrate` line, a sign, a written book, a dialogue line. The journal's pages are the texts a player can read again; a creator who wants a talk's line to count writes it as the `talk-to`'s `record`.
- A clue seen on a body (an NPC's skin, a mob's gear): `sight` names an anchor's cell.
- A clue for a trigger that belongs to no main-line objective on any walked path, whatever is written near it: it is optional by this rule.
- Guidance to a step that states a clue: `DW0863` does not judge it, and nothing else does.

## Acceptance criteria

1. **The surface.** `delvec schema --stage quests` exports `clues` on all five objective kinds and on a trigger, with the three `by` entries, at `dsl_version` 0.86.0; every document the repository holds parses.
2. **The castle passes.** `crates/delvec/tests/it/clue_presented.rs`: four `use` triggers 77 blocks apart with a wall between each pair, performed to pay the flags of a main-line objective whose `after` names the note, each stating the note's `record` — no refusal; the binding counts four main-line acts and four text clues presented.
3. **The castle reds.** The same shape with the note's `collect` in a quest outside the spine is `DW1137` on each fire, naming that the note is not on the main line; with the note on the main line and no `after` or flag forcing it, `DW1137` naming that nothing forces the order.
4. **A side puzzle is owed nothing.** The optional-note shape with the fires lit for an objective of an optional quest: no refusal, and the binding counts the acts as optional.
5. **A flag forces.** The note's completion as the sole setter of a flag the step requires passes with no `after`; a second setter of that flag reds.
6. **Sight.** A `sight` clue down a clear line from a forced step's place passes; behind a wall from every counted cell it is `DW1137` naming the first stopping cell; an anchor that resolves to no cell is `DW1136`.
7. **`DW1136`.** `clue_presented.rs`: each arm of §3 is raised at the clue's pointer, and the same clue with the text written is silent.
8. **The scope.** The castle's four fires with no `clues`, far apart and walled off, are refused by `DW0863` four times and the clue rule judges nothing; with `clues` stated `DW0863` counts four acts as the clue rule's and refuses none of them. The gallery probes `a-step-out-of-sight` and `a-fight-nobody-points-at` are refused with `DW0863` as before, and the findings rows `bell-14`, `str-04` and `str-18` are unchanged.
9. **One sight rule.** The clue rule asks `promise::unseen`, the function `DW0863` asks; `every_consumer_of_in_sight_answers_alike` holds it to the other consumers.
10. **The gallery and the pages.** The units of §2 are bound in the gallery primary; the three probes of §6 are refused with their codes; `docs/reference/compiler.md` and `docs/reference/delvec/compiler/clue.md` state the rule; `check-dw-codes.py`, `check-diagnostic-messages.py` and `check-gallery-coverage.py` are green.

## Measurements this spec states

Written into the commit bodies that land the implementation: the gallery's `DW0863` and `DW1137` binding lines; the tests of `clue_presented.rs` with their red side.
