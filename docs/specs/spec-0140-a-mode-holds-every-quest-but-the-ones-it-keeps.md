# spec-0140: A mode holds every quest but the ones it keeps — a state the story puts the map in, under which the rest of the story is neither told nor completable and its clocks stand still, left by the story or by itself when what it keeps is done

- **Status**: Proposed
- **Ground**: written against `feature/festival-day` at `d63627cec` (`delvec 1.17.0`, `dsl 0.79.0`), read for: `pending_guard`, `told_guard`, `serving_guard` and `quest_completion_fns` in `crates/delvec/src/compiler/emit/quest.rs`; the limit clock (`start_lines`, `quest_start_drivers`, `born_expired_drivers`, `expire_<obj>`) in `emit/limit.rs`; the guidance state in `emit/guidance.rs`; `PathFiring`, `dated` and `check_courses_started` in `plan/path/`; `branch_paths` in `emit/branch.rs`; `mainline_key` (`DW0868`) in `crates/dsl/src/quest_plan.rs`; `for_each_campaign_effect` and `EffectSite` in `crates/dsl/src/effects.rs`; the batch-state rule `compiler::batchstate` (`DW0807`) and the solo rule `compiler::solo`; spec-0018, spec-0039, spec-0051, spec-0104, spec-0133, spec-0139 (§4, §9).
- **What it is for**: a festival town holds a pig race beside its main story. While the race is run the main story must wait: its compass must not pull a racer off the course, its tasks must not complete under a racer's feet, and a task with a clock must not run out while the party is racing. spec-0139 built the race and left this out by name (§9). It is not a race's field: it acts on quests. This spec gives it a class of its own — a **mode** — says what a mode holds and what it does not, when a mode is left, and how the build proves a party is never left in one.
- **Research**: none commissioned. The mechanism is the engine's own guard with one more term; no craft question is decided here. The three facts of the pinned game it leans on are read from the pinned server jar and recorded with their read site in `docs/reference/delvec/compiler/mode.md` (*What the pinned game gives*). Every rule below is **[cited]** from the record named, or **[authored]** here.
- **Numbers**: spec `0140`; `dsl_version` `0.81.0`; `DW1122`, `DW1123` and `DW1124`, three used (§6). `DW1125` was handed and is not used. No `critical-path.json` format number was handed and none is used: no step changes (§5).
- **Non-goals**: a race mode (a primitive is a mechanism — a boss arena that shuts the story out until the boss falls, a dream the party walks through, a trial inside a trial configure the same class); what the party does inside a mode (that is the quests it keeps); a gate, a trigger or a body that reads the mode (§8); any campaign's modes.

## 1. The class, and why one **[authored, on cited facts]**

**[cited]** (`emit::quest`): every driver of an objective — its act's poll, the dialogue button, a `complete-objective`, the press that serves it — runs under one guard, `pending_guard`, and everything the party is told of it — its announcement, its journal page, the compass, its activation and so the start of its clock — under `told_guard`; a press that serves it opens under `serving_guard`. Each begins with one term, the quest's active score. **[cited]** (spec-0133 §3): an objective's clock is two `schedule`s and a bar, started by the function its start event runs.

**[authored]** So a mode is one class, **`modes[]`**, and what it does is one more term on those guards: a quest the mode does not keep is **held** while the mode is on. It belongs to the object class it acts on — quests — and not to the verb that first needed it: a race does not suspend a story, a mode does, and a race is run inside one by keeping the quest whose objective declares its `course`. It is not a flag every objective's `forbids_flags` names: the quantifier is *every quest but these*, which no gate on one objective says, and a clock reads no gate.

## 2. The surface **[authored]**

Stage 5 gains:

```json
"modes": [
  {"id": "mode/pig-race", "keeps": ["quest/pig-race"]}
]
```

- `id` is `mode/<kebab>` (`DW0110`/`DW0111`).
- `keeps` names the quests that go on while the mode is on; every other quest is held. One or more, each once (`DW1122`); each a quest this stage declares (`DW0112`). What a mode keeps is said, and what it holds is the rest: a quest added to the campaign later is held by every mode that does not name it, which is the safe default for a story that must wait.
- Two verbs: **`enter-mode {mode}`** and **`leave-mode {mode}`** (§3). Both are the party's — the map is in a mode for everyone — so neither takes an `audience` or an `in` (`DW0942`), and both take any bundle.

## 3. What a mode is, when it is on and when it is left **[authored]**

**The map is in one mode, or in none.** `enter-mode` puts it in the named mode. Entering a mode while another is on leaves that one first: two modes on at once would each hold what the other keeps, and nothing could go on. Entering the mode the map is already in does nothing.

**A mode is left three ways.** By a `leave-mode` naming it, which does nothing when the map is not in that mode. By another mode's entry. And **by itself, the moment none of the quests it keeps is in progress** — started and not complete. The engine asks at the entry and at the completion of every quest the mode keeps, after the quests that completion starts have started. So a mode entered with nothing of its own running is left the tick it is entered, a mode whose last kept quest completes is left in that completion, and no mode outlasts what it keeps. `leave-mode` is the story's early way out; the common shape needs none.

**Leaving gives everything back.** Every held quest is told and completable again from that tick: an objective announced before the mode is not announced twice, one that became open while held is announced now, the compass and the journal follow the guard as they always do.

## 4. What a mode holds, and what it does not **[authored, on cited primitives]**

While a mode is on, of every quest it does not keep:

- **No objective completes** — by its act, by a dialogue option (the option is not drawn), by a `complete-objective`, or by any other driver: all run under the one guard.
- **No objective is told.** Nothing is announced, the compass follows none, no journal page is open for one, and none activates — so no prop of one is placed and no clock of one starts.
- **No press that serves one opens** (spec-0117): the presses are part of the task.
- **Its clocks stand still.** A clock running at the entry is stopped and its bar hidden; one that would start (a `quest-start` clock of a quest that starts while held) waits. At the leaving each stopped clock goes on **from the seconds its bar showed** — the part of a second that had run is given back, so a clock is never shorter for having been held. An objective is never expired while held, whenever its clock started.

A mode holds **the story's tasks, not the world**:

- **A held quest still starts.** A quest armed by a completion while held becomes active — a fact, with nothing of it told or done until the mode is left.
- **`fail-objective` still settles** a held objective failed: the story losing a task is the story's line, fired by something that is not held.
- **What stands stays.** A prop or a highlight an objective placed before the mode, an NPC and what it says, a wave, an actor, a vehicle, a race, a board, a trigger that serves no held objective, a trap, a shop, a checkpoint, the weather: none is a quest, and none is touched. A creator who wants one of these to wait gates it on a flag the entry sets.
- **A kept quest is untouched**: it is told, completed and clocked exactly as with no mode.

## 5. A party is never left in a mode **[authored]**

Three facts, each with its own proof.

**No mode outlasts what it keeps — by construction.** §3's settlement is emitted, not authored: a mode is on only while a quest it keeps is in progress, so *the mode is on* always means *the party has a quest of the mode's to work at*. There is no declaration that turns this off and so nothing to check.

**What a mode keeps is not locked by what it holds** (`DW1123`, document tier). An objective of a kept quest that may be open while the mode is on, and that `requires_flags` a flag whose only setters are bundles of quests the mode holds, is refused: once the mode is on nothing can set the flag, the kept quest never completes and the mode is never left. *May be open while the mode is on* is every objective of a kept quest, less the ones certainly settled before every `enter-mode` of the mode fires — an entry in a kept quest's own `on_objective_complete` bundle has that objective and its plain `after` ancestors behind it. A flag a trigger, a trap, a shop, a disarm or a dialogue option also sets is free: no mode holds those. So is a flag set by a quest above the kept one in its chain of `quest-complete` triggers: that quest is complete before the kept one starts. A mode no line enters is not judged. The shape is `DW0868`'s (a mainline objective keyed on a flag only optional quests set), asked of a mode's two halves.

**Every path the build proves walks through its modes** (`DW1124`, plan tier). The map's mode is replayed along the exported path and along every story world's path, moved as the emitted functions move it: an `enter-mode` the path certainly fires at a step enters from that step's completion; the mode is left where §3 leaves it — at the entry, at a kept quest's completion, at another entry, at a `leave-mode` the path certainly fires. A step that performs an objective of a quest the mode then on does not keep is refused, naming the objective, its step, the mode and the step it was entered at. Dated as a race's lines are (spec-0139 §4): an `enter-mode` the path cannot date — a trigger it does not perform, a reaction bundle — enters nothing on that path, and a `leave-mode` it cannot date leaves nothing. A mode that interrupts a line is therefore ordered by the quest graph, as everything a path walks is: the quest a mode keeps takes a `quest-complete` trigger on what comes before it, and what follows waits on its completion.

**The bot's verdict is the objective's own marker.** The contract gains no step and its format does not move: under a mode the path performs only what the mode keeps, each step passing on its completion marker as always; a mode the server did not leave withholds the marker of the first step after it.

**Classed by what each judges** (CLAUDE.md, *A check guards only what our toolchain owns*): that a held objective is refused and a clock stands still is the compiler's output, proven by Rust tests and the PackTests of §7; that this campaign's kept quest can be completed inside its mode is the content's, judged by the bot on the paths that walk it; that `schedule clear` answers how many it cleared and that a failed command stores 0 is vanilla's, held by the module page and no recurring check.

The engine states, on every build that declares a mode, `mode binding (DW1122–DW1124): <m> mode(s), <e> entered by a line; <k> quest(s) kept and <h> held over them, <c> clock(s) stood still; on the exported path <s> span(s) of a mode, <l> left before its end, <j> objective step(s) inside judged for a quest the mode keeps`.

## 6. The rules **[authored]**

| code | module | refuses |
|---|---|---|
| `DW1122` | `dsl::mode` | a mode that cannot be run as declared: it keeps no quest (it would be left the tick it is entered); it keeps one quest twice |
| `DW1123` | `dsl::mode` | a kept objective, open while the mode is on, that requires a flag only bundles of held quests set |
| `DW1124` | `compiler::mode` | a path the build proves that performs an objective of a quest the mode then on does not keep |

Reused: `DW0110`/`DW0111` (id), `DW0112` (a mode or a kept quest nobody declared), `DW0942` (an `audience` on a mode verb).

## 7. Emission, PackTest and the gallery **[authored, under spec-0039]**

**The mode is one party score**, `#party dw.mode`: `0` in no mode, else the mode's place in `modes[]` from one. Every guard of a quest some mode holds gains, beside its quest's active score, one `unless score #party dw.mode matches <run>` per maximal run of the numbers of the modes holding it; a quest every mode keeps, and every quest of a campaign with no mode, gains nothing and is byte-identical. It is `#party`'s because it is the party's progression, and so that the batch-state rule (`DW0807`) holds every generated PackTest that opens a guard to owning it.

`mode_<m>_enter` returns for a mode already on, runs `mode_off`, sets the score, stops each running clock of a quest the mode holds — `execute store result score #mh_<obj> dw.sys run schedule clear <ns>:expire_<obj>`, then the countdown's step cleared and the bar hidden where that stored one or more — and runs `mode_<m>_settle`. `mode_<m>_settle` returns unless the map is in the mode, returns for each kept quest that is active and not complete, and else runs `mode_off`; `complete_q_<k>` runs it, last, for every mode keeping `k`. `mode_off` sets the score to 0 and, for each clock a mode stopped, shows the bar and schedules its step and — through a one-line macro, `$schedule function <ns>:expire_<obj> $(t)t replace` — its expiry at twenty ticks a second left. `leave-mode` is `execute if score #party dw.mode matches <n> run function <ns>:mode_off`. `setup` creates the party's entry without moving it (`scoreboard players add #party dw.mode 0`), so a reload leaves the map in the mode it was in.

**PackTest.** Per mode the build emits `mode_<m>_holds` — run alone, from the whole party ledger at the campaign's start (the entry and the leaving reach every clock and quest the mode touches, and the batch-state rule holds the template to owning each term on that way); with the kept quests in progress, the pending guard and the told guard of the first objective of a held quest, and the pending guard of the first objective of a kept quest, each the very guard the tick runs, read before the entry, under the mode and after a `leave-mode`: the held one open, shut, open; the kept one open throughout; then the mode entered again and left by the settlement once what it keeps is done, and left at once when entered with nothing in progress — and, for a mode that holds a limited objective, `mode_<m>_clock`, run alone on the server's own clock for the held clock of the shortest window: the clock started, the mode entered, more than the whole window waited with the objective not expired and its seconds unmoved, the mode left, the seconds left waited, the objective expired.

**The gallery** binds the class on the long hall, where its races are run: `mode/the-long-hall` keeps `quest/long-hall` and is entered from the bundle of that quest's first objective, the arrival on the porch. While the hall and its yard are walked the near hall, the reliquary — the optional strand, in progress beside it — and the far hall, whose loft carries the clock the mode's entry would stop, are held; the mode is left by its settlement when the cart's line is ridden and the quest completes, in the same completion that starts the far hall. A `leave-mode` stands in the far hall's first beat, where the map is already in no mode and it does nothing. The exported path walks the mode whole: one span, seven objective steps inside. Probes: `a-mode-that-keeps-nothing` (`DW1122`), `a-kept-task-keyed-from-a-held-hall` (`DW1123`) and `a-mode-that-holds-the-next-step` (`DW1124`).

## 8. What it leaves out, by name **[authored]**

A **gate term that reads the mode** — a trigger, an effect's `when` or a dialogue option open only under a mode: a creator gates those on a flag the entry sets, and a term is added when a quantifier a flag cannot say is needed. An **effect root at a mode's leaving** (`on_leave`): the completion bundle of the quest the mode keeps is where the story says how it ended. A mode that **holds bodies** — waves, actors, traps — or the world's clock. A mode **per player**: the map is in a mode for the party. **Nested modes** — a mode entered inside another and returned from: one mode at a time, the second replacing the first. `DW1123` over **`requires_state`** and over an item only a held quest gives: both are real shapes, both end with the mode still on and a told task that cannot be done, and both are caught by the bot on a path that walks the mode. A **race's own end as a line** (spec-0139 §9): the race is over, the objective that declares its `course` completes on its act, its quest completes, and the mode is left by itself.

## 9. A worked example: the town's pig race **[authored, on spec-0139 §2]**

The steward's invitation is the first objective of an optional quest; accepting takes the party to the start, puts the map in the race's mode and shows the course; the race starts when the first racer mounts; the second objective is run on the course and completes once the race is over — every racer in, or thirty seconds after the first — and with it the quest completes and the mode is left. No `leave-mode` is written.

```json
"modes": [
  {"id": "mode/pig-race", "keeps": ["quest/pig-race"]}
],
"races": [
  {"id": "race/round-the-town",
   "checkpoints": [
     {"at": {"anchor": "anchor/east-wood", "offset": [0, 0, -40]}, "extent": [4, 3, 1]},
     {"at": {"anchor": "anchor/garth"}, "extent": [3, 3, 3]},
     {"at": {"anchor": "anchor/fairground", "offset": [6, 0, 0]}, "extent": [1, 3, 4]}
   ],
   "racers": ["actor/racer-ulla"],
   "board": "board/pig-race",
   "grace_ticks": 600,
   "missed": "You missed a checkpoint — go back.",
   "passed": "Checkpoint"}
],
"state": [
  {"id": "state/racers-mounted", "scope": "party",
   "counts": {"of": {"riders": "vehicle/race-pigs"},
              "region": {"anchor": "anchor/fairground", "extent": [12, 3, 12]}}}
],
"triggers": [
  {"id": "trigger/first-in-the-saddle",
   "on": {"on": "body-enters", "state": "state/racers-mounted"},
   "requires_flags": ["flag/race-called"],
   "effects": [
     {"type": "start-race", "race": "race/round-the-town"},
     {"type": "move-actor", "actor": "actor/racer-ulla", "along": "race/round-the-town",
      "to": {"anchor": "anchor/fairground", "offset": [8, 0, 0]}}
   ]}
],
"quests": [
  {"id": "quest/pig-race",
   "trigger": {"type": "quest-complete", "quest": "quest/open-the-fair"},
   "objectives": [
     {"type": "talk-to", "id": "obj/take-the-stewards-dare", "npc": "npc/steward",
      "title": "Hear the steward's dare"},
     {"type": "reach-anchor", "id": "obj/run-the-pig-race", "after": ["obj/take-the-stewards-dare"],
      "anchor": "anchor/fairground", "radius": 4,
      "ride": "vehicle/race-pigs", "course": "race/round-the-town",
      "title": "Ride the course and cross the line"}
   ],
   "on_objective_complete": {
     "obj/take-the-stewards-dare": [
       {"type": "teleport", "from": {"anchor": "anchor/steward", "extent": [6, 3, 6]},
        "to": {"anchor": "anchor/fairground", "offset": [-4, 0, 0]}},
       {"type": "enter-mode", "mode": "mode/pig-race"},
       {"type": "set-flag", "flag": "flag/race-called"},
       {"type": "seat-vehicle", "vehicle": "vehicle/race-pigs"},
       {"type": "show-board", "board": "board/pig-race"},
       {"type": "cutscene", "seconds": 8,
        "path": [{"anchor": "anchor/east-wood", "offset": [0, 12, -40]},
                 {"anchor": "anchor/garth", "offset": [0, 12, 0]},
                 {"anchor": "anchor/fairground", "offset": [6, 8, 0]}]}
     ],
     "obj/run-the-pig-race": [
       {"type": "narrate", "style": "chat", "text": "The steward chalks the places on the board."}
     ]
   },
   "on_complete": [
     {"type": "hide-board", "board": "board/pig-race"}
   ]}
]
```

What each line of the lifecycle is: *the invite teleports the party to the start* — the `teleport` in the dare's bundle; *on confirming, the map switches to race mode* — the dialogue option that completes `obj/take-the-stewards-dare`, whose bundle carries `enter-mode`; *a cutscene shows the course* — the `cutscene` after it; *the race starts when the first player mounts* — `trigger/first-in-the-saddle`, a `body-enters` over the riders' census, gated on the flag the entry set; *it ends thirty seconds after the first finisher or when all have finished* — `grace_ticks: 600` and the race's own end; *then the map returns to the main quest* — `obj/run-the-pig-race` completes for the party once the race is over and a racer stands at the line, `quest/pig-race` completes, and `mode/pig-race`, keeping nothing in progress, is left. `DW1123` reads `obj/run-the-pig-race` (the dare is behind the entry) and finds no flag on it; `DW1124` judges the mode on any path that walks the quest. A party that will not race is not held: the mode is entered only by the dare's acceptance.

## 10. Acceptance criteria

1. **The surface.** `delvec schema --stage quests` exports `modes[]` with `id` and `keeps`, and the verbs `enter-mode {mode}` and `leave-mode {mode}`, at `dsl_version` 0.81.0; every document the repository holds parses. `crates/dsl/tests/it/v29_firework.rs` holds the sixty-six verbs.
2. **The refusals of the document.** `crates/dsl/tests/it/mode.rs`: each arm of `DW1122` is raised at the field that says it and the same mode without the fault is silent; a mode verb naming no mode and a `keeps` naming no quest are `DW0112`; `DW1123` is raised at the flag for a kept objective keyed from a held quest, and is silent when the mode keeps the setter, when a trigger also sets the flag, when no line enters the mode, when the setter is complete before the kept quest starts, and for an objective behind every entry — and raised again when a second entry stands in a held quest's bundle.
3. **The guard.** `crates/delvec/tests/it/mode.rs`: every tick line that announces or completes an objective of a held quest carries the term beside its quest's active score, and no line of a kept quest's does; a quest held by modes 1 and 3 reads two terms and one held by 1 and 2 reads `1..2` (the perturbation only this proof catches: the same three modes in another order); a campaign with no mode emits no `dw.mode` and no `mode_` function.
4. **Entering and leaving.** `mode.rs`: `mode_<m>_enter`, `mode_<m>_settle` and `mode_off` are the lines of §7; `enter-mode` calls the entry and `leave-mode` the guarded `mode_off`; the completion of a kept quest calls the settlement last, after the quests it arms, and a held quest's completion calls none.
5. **A held clock.** `mode.rs`: the entry stops a held limited objective's clock by the four lines of §7; `mode_off` resumes it through `mode_resume_<obj>`, which reschedules the step and, through the macro, the expiry from the seconds left, and returns for an objective settled meanwhile; a held quest's `quest-start` driver carries the term; the expiry's own guard is held.
6. **The path.** `plan::path::pursuers::mode_tests`: a path performing a held quest's objective between a mode's entry and its leaving is `DW1124` naming the objective, its quest, the two steps and the mode; the same steps with the kept quest completed first pass, with the span left at that completion; a mode keeping both quests is left at the last completion; a `leave-mode` the path fires leaves; an entry the path does not certainly fire enters nothing; a mode whose kept quest has not started is left at its entry. `mode.rs`: `compiler::mode::gate` counts the modes, the entered, the kept, the held, the clocks and the path's spans.
7. **PackTest.** The gallery build emits `mode_<m>_holds` for every mode and `mode_<m>_clock` for every mode that holds a limited objective, and they run green in tier 2. `mode.rs` holds the nine guard readings of `mode_<m>_holds` in order (open, shut, open for the held pair; open throughout for the kept one) and the two waits of `mode_<m>_clock`.
8. **The gallery.** Every unit of §2 is bound in the gallery primary, the coverage gate reports none in neither state, the three probes of §7 are refused with their codes, and the gallery bot's run completes every objective.
9. **Codes and pages.** Every code of §6 is asserted by a test and sits on its module's page; `check-dw-codes.py`, `check-diagnostic-messages.py`, `check-capability-ownership.py`, `check-effect-roots.py`, `check-gallery-coverage.py`, `check-live-commands.py` and `check-skill-page.py` are green.

## 11. Measurements this spec states

Written into the commit bodies that land the implementation: the mode binding line over the gallery; the two templates' names per gallery mode; the count of generated PackTests the batch-state rule held to owning the mode score.
