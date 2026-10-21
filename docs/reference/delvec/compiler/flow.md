# `delvec::compiler::flow`

The reference page for `crates/delvec/src/compiler/flow.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Story worlds (spec-0108)

`Flow` holds two enumerations, each for its own proofs.

**Today's worlds** — one per dialogue selection (one alternative per flag-reading choice group), every elective act performed — with their API (`world_count`, `world_flags`, `world_completed`, `playthrough_in`) decide `DW0201`, `DW0202`, `DW0203`, `DW0879` and the critical-path `DW0204`. A world that performs less can only complete less, so these reachability codes need nothing finer.

**Story worlds** (`Flow::story`, `flow::story`) serve the branch proofs (`compiler::branch`: `realize`, the branch plan, `DW0480`, `DW0482`–`DW0485`, `DW0927`, the branch-path `DW0204`) and the ending proofs (`compiler::ending`: `DW1029`, `DW1030`, `validation/endings.json`, the `ending_<slug>_<k>` PackTests). A story world is a dialogue selection **and a participation**: the set of elective acts performed.

- **Acts.** An elective act is an optional quest (`mandatory: false`, performed whole) or an ambient root fired by a player — a trigger, a trap's payload, a trap's or timed gate's disarm, a shortcut's unlock, a shop offer, an `on_kill` — performed once or not at all. A root the mainline performs is **forced**: a trigger that pays a debt of the exported path (`trigger_debts`), whose bundle opens a way, that writes a datum a mandatory objective's `requires_state` reads, or that is a repeatable carry (a link); an `on_kill` of a wave a mandatory `kill` slays. A forced act fires in every world; its flags are credited wherever their gate holds and its datum writes stay undatable. A loop's `on_cross` is not an act.
- **Solving.** A world is solved with the producer table restricted to its participation (`solve_with`): an optional quest it skips is never activated, an elective producer it skips credits nothing, an `on_kill` producer credits only where the world performs the kill (today's model credits none).
- **Dating.** A datum a final scene reads whose every undated writer is an elective ambient act is dated in a story world: a performed act writes once, at its **entry deed** — the first step in front of which its own gate holds — and a skipped act never writes. Every other undatable datum stays undatable.
- **Runs.** Each world is run: ordered (`dag_order` over what completes, an ending quest last), replayed with its entry deeds performed (`Flow::run_story`, `Walk` with the world's `StoryCtx`), and read at the moment each final scene begins to fire (`ReplayState::scene_at`). A run whose step is illegal where it is taken is **incoherent** and reaches nothing.
- **Worlds.** Per dialogue selection, in today's order: the *all* world (today's world), the *none* world, then one **witness** per target a candidate participation realizes — a target being one value class per input of a final scene (a flag's two values; a datum's intervals, cut by every `requires_state` term reading it in any final scene). Candidates are the subsets of the elective acts that write some input, by size then act id, at most `MAX_CANDIDATES` (256) per selection; targets past `MAX_TARGETS` (4096) per scene are counted at the bound. Worlds are deduplicated on (selection, participation) and capped at `MAX_WORLDS`; a test drives the cap with `Flow::story_within`. A branch is realized by the first world in that order whose flags satisfy its assignment, so a branch every world of its selection realizes keeps today's world and path.
- **Paths.** `Flow::story_playthrough(k)` carries the world's participation and deeds (`Playthrough::story`); `journal`, `replay` and `walk_for` play it the way the world plays it, and `plan::path_triggers` performs each trigger deed once, in front of the step its run performed it at.

## Diagnostics

### DW02xx — analysis (`compiler::analyze` reachability + `compiler::light` lighting; error; exit 2)

This module's rows of a section whose prose is on the [`delvec::compiler::analyze` page](analyze.md#dw02xx--analysis-compileranalyze-reachability--compilerlight-lighting-error-exit-2).

| Code | Meaning |
|------|---------|
| `DW0204` | An exported path is not a playthrough any player can walk: some step is not activatable/completable at its position, or `campaign-complete` fires before the final step (the signature of two mutually exclusive endings sharing one path, or of a quest ordered after the ending). Quantifies over the exported critical path and every reachable branch's exported path — the path of the story world that realizes it (`validation/world-path-<k>.json`, spec-0108), replayed under that world's participation and entry deeds; a branch finding names the branch and that file. Names the first incoherent step. |

### DW0879 — a numeric gate the path has already cleared (`compiler::statepath` over `compiler::flow`; error; exit 2)

| Code | Meaning |
|------|---------|
| `DW0879` | **A forced-path numeric gate the path itself has made unsatisfiable.** An objective's `requires_state` term evaluated at that objective's own position on a walked path, against the value every state write the path performed has left the datum holding. Analysis tier (exit 2); it judges what the document says, and a campaign that declares no `state[]` reads no gate this rule can see. Names the objective, the datum, the comparison, the value held, the beat whose write left it there, and the two remedies: move the write past the beat that reads the datum, or move the gate. |

**The quantifier is the whole rule.** `DW0501` asks whether a datum is written
anywhere, `DW0502` whether it is read anywhere, `DW0847` whether the gate's own
terms are jointly satisfiable, and `DW0527` whether a comparison sits after a
write **in the same bundle's effect list**. None of them asks what the datum
holds at the moment a *later* beat reads it, and that question needs an order —
which the monotone fixpoint does not have and the path replay does. So this is
the replay's binding widened from flags to the whole gate, not a mechanism
beside it: `ReplayState` carries every declared datum's value, `Flow::fire`
honours a write's own `requires_state` where it stands (which is where vanilla
evaluates it) and applies the write, and `Flow::state_gates` reads each
objective's gate at its position.

**What is walked**: the exported critical path (`Flow::playthrough` — the
participation-minimal order `DW0204` already proves is a playthrough), and every
enumerated branch world's own whole path (`Flow::playthrough_in`, a `dag_order`
over everything that completes in that world), which reaches optional strands
the finale-rooted path never visits and branches it is not on. A finding already
named on the critical path is not named again; a branch finding names the branch
by the flags that distinguish it.

**Two refusals to over-claim, both counted rather than silent.** A term is
refused only where the failure is one **no play order avoids**: the emitter's
`pending_guard` lets a player complete any activatable objective at any moment,
so two beats with no `after` between them can be played either way round, and a
gate that fails under one of those orders and holds under the other is a path
this walk picked rather than a defect. The writes the walk applied must
therefore be chained into one order by the campaign's own `after` and
`quest-complete` relations, ending before the gate. And a **datum no ordered
walk can date** is never refused at all: an ambient producer (an environment
trigger, a trap payload, a shortcut's `on_unlock`, a shop offer) may be fired any
number of times at any moment, a reaction bundle (`on_death`, `on_respawn`,
`on_rest`, `on_caught`) fires at a moment nothing names, a `stakes[]` forfeit
moves the purse on a death, and a `player`-scoped datum under `min_players >= 2`
depends on which agent acts. A flag is monotone and so is credited
unconditionally in all four cases; a number is not. Undatable **absorbs**,
`set-state` and `clear-state` included — a write pins a value only until the next
undated write, which can land the beat after.

**Ordering.** It withholds itself entirely where `DW0201` (no branch completes
the finale) or `DW0204` (the exported path is not walkable) already names a
cause, and says which, rather than adding a second refusal about one break.

**Binding**: paths walked, steps, numeric gate terms read — split into the ones
read against an undatable datum and the ones withheld for an unforced order —
state writes replayed, and declared data with how many of them are undatable.
Stated on every validate, analyze and build of a campaign, including the zeroes.
The split is the load-bearing half: a run that read twenty terms and decided all
twenty and a run that read twenty and could decide none print the same
`gate term(s) read`, and only the second is a green that means nothing.
