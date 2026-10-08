# `delvec::compiler::branch`

The reference page for `crates/delvec/src/compiler/branch.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW048x — branch-complete narrative verification (`compiler::branch`; spec-0025)

"Provably completable by machine" quantifies over **branches**, not paths. A
fork that decides who lives is declared in the DSL, and proving one critical
path never plays it. The blind class in one shape: a branch's cast ledger says a
character lives while the staging still belongs to the branch where he dies — an
NPC despawns himself, another holds a cave the party has left, a third mourns a
man standing beside him. **A fork that moves the ledger must also move the
bodies**, and these proofs own that gap.

**The model.** Stage 4 declares its `branch_points`: the flag set a fork owns
(`forks_on`), the quest it `opens_at`, and the branches it offers. An
**enumerated branch** is one point of the product over the declared points, so
the branch set is authored and small — never a combinatorial sweep of every flag.
Each branch carries a **flag assignment**: the flags it lists are pinned SET and
every other flag of its points' `forks_on` is pinned UNSET. That second half is
what makes leakage decidable rather than hopeful. An assignment is realized
against `compiler::flow`'s enumerated worlds — a world realizes a branch when its
solved flag set holds every pinned-set flag and no pinned-unset one — and the
branch's own playthrough is rooted at **the branch**, not at the stage-4
`finale` (a branch running to its own ending never completes the finale, so
rooting there would say the branch plays nothing).

Validation-tier (exit 1), like the `DW046x` ledger it extends.

| Code | Meaning |
|------|---------|
| `DW0480` | **Undeclared story fork.** A flag that gates casts, staging, quest structure or a staging trigger, is set on some enumerated playthroughs and not others, and belongs to no declared branch point. "Forks" is decided, never guessed: a flag EVERY playthrough sets is ordinary sequencing and is silent. An undeclared fork is a branch nothing verifies — exactly how a campaign ships with the ledger on one branch and the bodies on the other. Prescription: declare the branch point (`forks_on`, `opens_at`, and each branch's `leads_to`). Do NOT silence it by ungating the content — the gate is the story. |
| `DW0481` | **A story node declares no `happening`.** The forcing function, generalizing spec-0020's `doing` from NPC presence to event flow: a design that never got written down node by node cannot compile. Required on every quest, every objective, every one of the **eleven story-node effects** (`spawn-npc`, `despawn-npc`, `move-npc`, `spawn-actor`, `despawn-actor`, `move-actor`, `unleash-actor`, `spawn-wave`, `open-gate`, `close-gate`, `campaign-complete`) at any nesting depth, and every **story-weight dialogue option** — one carrying a `set-flag`, which is how a player's choice forks the world. An option that only walks the tree or completes an objective needs none (the objective already declares one). Prescription: state the beat with one of the ten verbs plus a line of prose. Do NOT fill it with a placeholder: the per-branch chronicle the narrative review reads is assembled from exactly these lines. |
| `DW0482` | **Branch terminality.** A declared branch reaches no ending: either **no playthrough realizes its flag assignment** (a branch nobody can take — commonly a branch declaring two mutually exclusive flags), or its playthrough fires no `campaign-complete`, or it fires an ending other than the one the branch declares, or the quest it declares it `converges_at` never completes. The message names the branch, the assignment, and the ending that really fires. |
| `DW0483` | **Cast continuity** — spec-0020 proof 4 (`DW0462`) extended from "the declaration exists" to "the selector resolves to THIS branch's cast at every quest after the fork". For each enumerated branch, at each quest **strictly after** its fork, an NPC declaring per-branch casts must have exactly one placement selected under the branch's flag state when that quest opens. Zero selecting means the NPC has no declared position on this branch; two or more means emission dispatches the last clause, which is how a placement left UNGATED (or gated on the other branch's flag) keeps governing long past the beat that wrote it. The fork quest itself is excluded on purpose: during it the flag state is by construction pre-fork, so a per-branch cast there could never select. Prescription: gate each placement on the flags of the branch it belongs to, every branch, every post-fork quest. Do NOT leave one ungated as a fallback. |
| `DW0484` | **Exclusive-content leakage.** Every playthrough that realizes a branch's set flags also produces a flag the branch pins UNSET — so content gated on a sibling's flag is reachable HERE. The mourning scene on the branch where nobody died, as a build error rather than a review note. The message names the leaked flag and where it is produced (an ambient environment trigger or trap disarm is called out explicitly, since those fire on every branch by construction). Prescription: make the producer exclusive to the branch that owns it. Do NOT relax the branch declaration to admit the leak. |
| `DW0485` | **Hard event contradiction**, per branch, over **every play order the branch admits**, with **both chronicle lines shown**. Four rules, each decidable from the structured verbs alone: `dies(S)` then any later act by `S`; `departs(S)` then an act by `S` with no `arrives(S)` between; `seals(S)` then any later beat about `S` that is not `opens(S)`; `loses(S)` then a second `loses(S)` with no `gains(S)` between. `learns`/`believes` are **epistemic** and never contradict — their subject is what the beat is *about*, and a living character may perfectly well believe something about a dead one; "Elpenor mourns a man standing beside him" is precisely the class spec-0025 leaves to the chronicle's human reader, because no verb makes it decidable. Ambient beats (environment triggers, trap payloads) are excluded: `flow` refuses to date them, so ordering them against the dated account would invent a sequence. **The quantifier** (`branch::check_every_order`): a *legal order* of a branch is a sequence of distinct steps of the branch's own path — every objective of every quest its world completes, each `talk-to` with the option the world takes — in which every step passes the replay's per-step test where it stands (`Flow::walk`, the same test `DW0204` replays: quest active, `after` done, `requires_flags` held, `forbids_flags` clear, completing option reachable) and nothing follows the step that fires `campaign-complete`; its chronicle is written by the rule the exported chronicle is. The branch is refused when the four rules refuse the chronicle of any legal order or any prefix of one — so an optional strand is judged at every point between the step that opens it and the end of the delve, not only where the exported path happens to walk it. The exported order is read first and its refusal reads as before. Beyond it, the proof searches every legal order, depth first, merging two orders that reach the same **play state** — the objectives done, the flags held, the value of every datum an effect gate compares, and, per subject some step can leave in a carried state (`dies`/`departs`/`seals`/`loses`), the last carrying line about it. Everything a later step's legality, its fired effects and its lines' verdict depend on is in that state, so the search is exact over every legal order, whatever `forbids_flags`, effect gates or numeric gates the campaign carries. Which effects a step fires, and so which lines it adds, is the replay's own answer at the point it reaches each effect (`JournalStep::fired`), never a second reading of the gate. One reduction: where a **quiet** step is legal — it fires the same effects whenever taken (no gated effect in its bundle or its quest's `on_complete`), sets no flag a `forbids_flags` or effect gate reads (directly or through an ambient producer), writes no datum an effect gate compares, does not end the delve, and has no acting line about a tracked subject — only it is walked, since taking it first preserves every clash any order from there shows. Cost: `O(states × steps)` legality tests. The search stops at `branch::MAX_ORDER_STATES` (50 000) distinct play states per branch; a branch that reaches it is refused as UNPROVEN under `DW0927`, never called clean. Every order a refusal prints is one the search walked, so it is a play order a player can walk. Binding line: `contradiction binding:` — branches read, dated lines, steps searched (quiet among them), distinct play states walked, refusals beyond the exported order, branches unproven at the bound (`DW0927`). Prescription: in the exported order, fix whichever beat is on the wrong branch; beyond it, make the later beat unable to follow the earlier one (an `after` edge, a quest-complete trigger, a `forbids_flags` on a flag the earlier beat's bundle sets), or fix whichever beat is on the wrong branch. Do NOT reword the `happening` to hide the clash — the verbs are the only part of the chronicle a machine can check. |
| `DW0927` | **A branch's hard-event-contradiction question is unproven.** `DW0485`'s every-order search (`branch::check_every_order`) walked `branch::MAX_ORDER_STATES` (50 000) distinct play states on one branch before it had walked every legal order, so an order it never reached may carry a clash. The branch is refused, never called clean: a bound hit is not a pass. A different statement from `DW0485`, which says a clash exists; this says whether one exists is not known. The message names the branch, the bound, the steps on the branch's path and how many of them are quiet. Counted on `DW0485`'s binding line (`contradiction binding:` — branches unproven at the bound). Build tier (exit 3). Prescription: cut the orders a player can interleave — chain strands that need not run side by side (an `after` edge, a quest-complete trigger), or drop a gate that makes a beat's effects depend on the order (a `forbids_flags`, or an effect gate on a flag several strands set). Do NOT raise the bound to get green. |

**A beat's subject is derived where the effect supplies one** (spec-0071 §3).
`happening.subject` is optional, and `DW0485` reasons only over beats that name
one, so a careful creator names it on every effect — and every time it is the id
two keys to the left, typed a second time. Where a `happening` hangs on an effect naming **exactly
one** object of a subject kind (an `npc/`, `actor/`, `wave/` or `anchor/` id), an
absent `subject` resolves to that object, for the proof and for the chronicle
alike. The rule is over the objects an effect names, not over a list of verbs:
`open-gate` and `close-gate` resolve their anchor, `spawn-actor`,
`despawn-actor` and `unleash-actor` their actor, `spawn-wave` its wave,
`spawn-npc` and `despawn-npc` their npc, and so do `set-block`, `fill-region`,
`clear-region`, `collapse`, `set-checkpoint`, `bonfire`, a `play-sound`,
`firework` or `lightning` at an anchor, and a `begin-stealth` over one zone. An effect naming
several — `move-npc` and `move-actor` name a body and a destination, `teleport`
and `volley` two anchors, a multi-shot `cutscene` a whole path — resolves
nothing, because naming one of them would be the compiler guessing which; so does
an effect naming none. A stated `subject` always wins: the caller knows more.
`open-way` is the one beat whose object the subject namespace cannot hold — a
way is named by its piece (`prefab/<name>`), which is not a subject kind — and it
therefore states its subject or resolves nothing. The derivation is one function
(`QuestEffect::happening_subject`), read by the namespace check in `dsl::validate`
and by the chronicle writer, so a beat cannot be about one thing for the proof and
another for the account a reviewer is handed. It is a reading rule and not a
field, and it adds no surface for a `dsl_version` step to number.

Every code of the `DW048x` block is assigned: `DW0486`/`DW0487` (the flask's
contents) just above this section, `DW0488` in the table above, and `DW0489`
under crosshair disambiguation.
