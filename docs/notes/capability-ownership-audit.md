# Capability-ownership audit

A one-time sweep of the DSL surface and compiler for capabilities bound to the
feature that first needed them. Filed in `docs/notes/` rather than
`docs/reference/`: it is a dated finding-set with a fix sequence, not a record of
current behaviour — the behaviour it describes is what we intend to change. The
part that must not rot is enforced instead by `tools/check-capability-ownership.py`.

Sources: owner ruling 2026-08-06 (`close-gate.sealed_hint`) and her restatement of
the principle the same day.

## The governing question

> **This is a general game engine.** Its primitives must be abstract, flexible and
> configurable, and must never be bound to one campaign's design. A creator must
> be able to build *any* content with it.

So for every primitive:

> **Does it encode a MECHANISM, or a DESIGN DECISION about what the mechanism is
> for?**

A mechanism: a thing a player can press, a region that can be sealed, a body that
walks a route, a hazard on a cycle. A design decision: *this* is a bonfire, *this*
is what a sealed door says. The genre we happen to be building is **content**, and
content belongs in campaigns.

The row test, answered explicitly per row: *could a creator making an entirely
different game want this primitive, and can they configure it to their own
fiction?*

## The four shapes

| # | Shape | The trap |
|---|---|---|
| S1 | capability keyed to the verb, not the object class | the second consumer has no surface, and the fix *looks like* a second bespoke field on the second parent |
| S2 | capability re-implemented privately inside a verb, when a general one exists | two mechanisms for one behaviour drift apart; the special case works perfectly so nothing ever looks; every proof, l10n pass and diagnostic written for the general path silently misses the private one |
| S3 | the general mechanism exists but binds too narrowly to reach the objects it should | looks exactly like a missing feature, and the "fix" adds a *fourth* mechanism |
| S4 | the primitive encodes a design decision rather than a mechanism | the engine can only build the game it was born from |

S3 is the one most often mis-diagnosed. For it the question is never "what surface
is missing" but **"what does the existing general mechanism fail to reach, and
why"**.

Precedent that the fix is possible: spec-0022 replaced a trap's private
consequence machinery (`traps[].effect`, a fixed dispenser payload) with the
general effect vocabulary (`traps[].payload`), and the verb became *more*
expressive. That is the template for every S2 row below.

## Ledger

Severity: **B** = blocks content today · **L** = latent.

### S3 — the general mechanism binds too narrowly (the root cause)

| # | Finding | Evidence | Sev |
|---|---|---|---|
| 1 | **`EnvTrigger` binds a body to a POINT, but scene objects are VOLUMES.** `EnvTrigger{at, on: use\|strike, effects}` is the general "player clicked a thing here, run effects" primitive and already carries the whole effect vocabulary, flag gating and `once`. It summons **one** interaction body at one cell. A seal, a door, a boulder occupy many cells. This single narrow binding is the root cause of most S2 rows: each volume object grew its own fleet. | general: `emit.rs:7119`. `close-gate` arms one body **per shell cell**: `emit.rs:4887`, `plan.rs:177` (`shell_cells`) | **B** |
| 2 | Because of #1, the general mechanism was taught to **ride** the private ones rather than being widened: a trigger on a gate summons nothing and borrows the seal's hitboxes; a `strike` trigger on an NPC borrows the NPC's. | `trigger_rides_seal` `emit.rs:4846`; `seal_rider_tags` `emit.rs:4821`; `npc_hitbox_trigger_tags` `emit.rs:5646`; `DW0422` `eclipse.rs:93` | **B** |
| 3 | `TriggerOn::StrikeNpc` exists **only** because a point-bound trigger could not reach a large NPC's body (`DW0359`, island round 7). It is shape #1 paid for once already, as a new enum variant instead of a widened binding. | `stages.rs:1766-1783` | L |

| 3b | **`NarrateStyle` has no `actionbar` channel** (`chat`/`title`/`subtitle`/`art` only). This is the *mechanical* reason `sealed_hint` and `boundary.message` could not have routed through `narrate` even had someone tried — the general effect cannot reach the channel both wanted. A second narrow binding, independent of #1, on the other half of the same lift. | `NarrateStyle` `stages.rs`; `emit_narrate` `emit.rs:5523-5541` | **B** |
| 3c | `QuestEffect` has **27 verbs and no apply-status-effect verb at all**, so `AreaMitigation::NightVision` emits a private clocked `effect give` because there was nothing to call. A genuine capability **gap**, not a duplication — recorded here so the lift is not mis-scoped as a de-duplication. | `stages.rs:4523-4552`; `emit.rs:8218-8241` | L |

**The lift for #1–#3 is one thing**: let a trigger's `at` bind to an anchor's
*shape* — the region the DSL already knows, and the seal already computes. Then
`sealed_hint` is an ordinary trigger with a `narrate` effect, and every other
volume object gets the capability free. It is emphatically **not** a new stage-5
section keyed by anchor: that would be a third mechanism, strictly weaker than
`EnvTrigger` (text only, one click, no flag gating, no `once`).

### S2 — private re-implementations of a general construct

Nine `minecraft:interaction` bodies are emitted; **exactly one is `EnvTrigger`**.
Twelve interaction *handlers* exist (nine bodies plus three riders); **exactly one
can say anything back** — the seal.

| # | Feature | Private emission | Duplicates | Sev |
|---|---|---|---|---|
| 4 | **`close-gate.sealed_hint`** — the live instance. Own body fleet, own actionbar reply, own baked default. | `emit.rs:4887`, `emit.rs:4909`, `plan.rs:147` | `EnvTrigger{on:use}` + `narrate` | **B** |
| 5 | `traps[].disarm{via, sets_flag}` — own interaction + own flag-set. | `emit.rs:7560` | `EnvTrigger{on:use, effects:[set-flag]}` | **B** |
| 6 | `timed_gates[].disarm{via, sets_flag}` — a **second copy** of #5's private machinery. | `emit.rs:4478` | same | **B** |
| 7 | `shortcuts[].unlock` — interaction half private; `on_unlock` already uses the general effect vocabulary. | `emit.rs:4749` | `EnvTrigger{on:use}` | L |
| 8 | `bonfire` — private body **and** a private two-option dialog with its own strings, instead of the stage-6 dialogue machinery. | `emit.rs:4040`, `emit.rs:5382` | `DialogueNode`/`DialogueOption` | L |
| 9 | `Objective::Interact` — own affordance and own `missing_item_hint` refusal reply. The activation-gated body is legitimate; the *reply* need not be private. | `emit.rs:8479`, `emit.rs:2234` | `narrate` | L |
| 10 | Trapped-chest fire body. | `emit.rs:7548` | `EnvTrigger` | L |

**Consequence of S2, measured.** Ten of the twelve handlers answer a press with
**silence**, and no author-facing field exists in which to say otherwise
(`emit.rs:7179`, `4490`, `7576`, `4742`). The owner's island finding #34 — a
sealed boulder answering silence — was fixed for one verb; the other nine sites
still have it.

### S1 — capability keyed to the verb, not the object class

| # | Finding | Evidence | Sev |
|---|---|---|---|
| 11 | **`requires_flags`/`forbids_flags` are documented as a "per-effect flag gate" but ride only 16 of 26 `QuestEffect` variants.** `spawn-actor`, `move-actor`, `unleash-actor`, `despawn-actor`, `spawn-npc`, `set-checkpoint`, `bonfire`, `begin-stealth`, `sequence` **cannot be flag-gated at all** — every one of them staging or souls vocabulary, which is exactly what per-branch content (spec-0025) must gate. (`campaign-complete`'s absence is deliberate and documented.) | `stages.rs:3153+` | **B** |
| 12 | `happening` rides 11 of **34** (was 11 of 26). The vocabulary anticipates consumers it was never given: `HappeningVerb` has `gains`/`loses` — "the party gains a thing" — while **`give-item` carries no `happening`**. The contradiction proof `DW0485` reasons only over beats that declare one. **No longer machine-enforced, and NOT because it improved:** spec-0031 added five verbs (`fill-region`, `clear-region`, `give-effect`, `clear-effect`, `teleport`), none of which declares a `happening`, so the share fell from 0.423 to 0.324 and crossed below check D's `MODIFIER_MIN_SHARE` (1/3). The checker then demanded its `MODIFIER_HOLES` entry be dropped — the stale-exemption guard working exactly as designed. The count moved; the finding did not. Reading this row as closed is the error the note exists to prevent. | `stages.rs:1146`, `3234` | L |
| 13 | **`TrapDisarm` and `TimedGateDisarm` are field-for-field twins** (`{via, sets_flag}`) declared separately. The second's own doc states the general intent — "one affordance grammar for every mechanism the party can switch off" — and then copies the type. | `stages.rs:1501`, `1579` | L |
| 14 | **"An item stack" is an object class the DSL never named**; six parents inline a near-copy with a *different* capability subset each. Holes are real: a boss's dropped weapon **cannot be enchanted** while a chest's can; `give-item` cannot grant an enchanted or potion-bearing item; `ItemDrop` cannot even carry a `count`. `Collect.item_name` is spelled differently from the other five only because `title` already occupies `name` on an objective — the rename is the tell. | `KitItem` 856, `LootItem` 1315, `ItemDrop` 2140, `Collect.item_name` 2710, `GiveItem.name` 3241, `EnchantedItem` 2230 | **B** |
| 15 | Seven fields (`title`, `hint`, `after`, `requires_flags`, `forbids_flags`, `stealth`, `happening`) are copy-pasted verbatim across all five `Objective` variants, with 10 hand-written accessor `match`es to read them. Complete today; the next variant is one omission from a hole. | `stages.rs:2531-2810`, accessors `2966-3138` | L |
| 16 | `DialogueEffect` is a hand-maintained 5-variant subset of `QuestEffect`'s 26, each variant documented "mirroring `QuestEffect::X`". A dialogue choice cannot fire `narrate`, `play-sound`, `give-item`, `open-gate` or any actor verb. | `stages.rs:684-721` | **B** |

### S2/S1 — proofs and walks that enumerate sites instead of deriving them

This is the shape PRs #301/#302/#321 fixed thirteen times. It is **not closed**.

| # | Finding | Evidence | Sev |
|---|---|---|---|
| 16b | **`shortcuts[].on_unlock` is a SIXTH effect root that no enumeration knows about.** It is a `Vec<QuestEffect>` hanging off a stage-5 struct — structurally identical in kind to `traps[].payload`, which *is* root R4 — and emission really lowers it. But it is not an `EffectRootKind` variant and not in `nested_effect_lists`. So every walk that inherits the five roots skips it: a `narrate` inside it is never l10n-inventoried, a `set-flag` inside it is invisible to the flag model and to `emit::declared_flags`. This is precisely the defect PRs #301/#302/#321 claimed to close, still live, in the one root the enumeration does not contain — and `check-effect-roots.py` cannot see it, because it greps for the five roots it knows. **Zero live campaign usage is the only reason it has not shipped as a bug.** **CLOSED (spec-0031): it is `EffectRootKind::ShortcutUnlock`, root R6** — see fix-sequence item 0 for why a root rather than the desugar this audit recommended. | field `stages.rs:1703`; lowered `emit.rs:5043`; roots `effects.rs:63-88`; nesting `stages.rs:4883` | **B** |
| 17 | **`DW0473` (unavoidable lethal damage) walks 2 of 5 effect roots** — `on_complete` and `on_objective_complete` only. A `damage-players` inside a `traps[].payload` is invisible to it, and spec-0022 made trap payloads the *intended* home for exactly that. A safety proof with a hole in the place the hazard verb lives. | `combat.rs:1084-1115` | **B** |
| 18 | Twelve further hand-rolled effect walks still miss roots after #321: `has_any_sustain`/`DW0474` (3/5), `collect_open_gate_anchors` (3/5), `gate_open_indices` (3/5, and its comment says it "mirrors" the one above — it mirrors the gap), `collect_v06_effects` (3/5 + a private dialogue walk), `wave_area` (4/5), `required_anchors_for_area` (2-3/5), `continuity` NPC-lifecycle (2/5 — the same module walks both ways), `quests_ending_tail` (2/5), the kill-less-wave PackTest picker (1/5), `branch` beat-account (2/5), `flow` advance replay (2/5), `actor_beats` (4/5, self-documented). | as listed | **B** |
| 19 | **Two affordance registries with divergent membership.** `emit::affordances` (feeding `DW0420`/`DW0421` — "the affordance has visible hardware") is a hand-enumerated list of four kinds, and its own doc claims "the list is the definition of the class … which is what makes the proof total rather than a spot check". It is a spot check: it never sees `interact` objectives, env triggers, NPC hitboxes, seals or trapfire bodies. `eclipse::affordances` enumerates a *different* subset. | `emit.rs:4676-4724`, `eclipse.rs:308-395` | **B** |

`tools/check-effect-roots.py` did not catch #17–#18: it is a 40-line proximity
heuristic, and these walks either spread their roots wider or reach them through
helpers. Its own `ALLOWED` already records one such open finding
(`required_anchors_for_area`).

### S4 — design decisions wearing a primitive's clothes

Recorded with the mechanism/decision test answered. **Distinguish a naming finding
(cheap) from a structural one (not).**

| # | Primitive | Mechanism or decision? | Verdict |
|---|---|---|---|
| 20 | `bonfire` | The **mechanism** is "a save-and-restore point with configurable side effects": it moves a checkpoint, runs an `on_rest` bundle, re-seats declared waves. All of that is general. The *name*, and the baked English `Bonfire`/`Rest and save`/`Save only`, are the genre decision. | **Naming + defaults, not structure.** Cheap. Its private dialog (#8) is the structural half. |
| 21 | `shortcuts[]` | The mechanism is "a barrier openable once, permanently, from one side, with a reachability obligation". General and genuinely useful to any game. The souls framing is in the doc comment, not the shape. | **Correctly general, genre-flavoured name.** Cheap or leave. |
| 22 | `Wave.respawns_on_rest`, `EncounterTier::{Elite,Boss}` | `respawns_on_rest` is a mechanism (re-seat on restore). `EncounterTier` is a *billing declaration* consumed by the validation ladder — but `boss`/`elite` are genre words for what is really "the tier the content claims, which the bot must測 against". | Naming. Low priority. |
| 23 | Baked English defaults — ~~`SEAL_HINT_DEFAULT`~~, ~~`BOUNDARY_DEFAULT_MESSAGE`~~ (**both closed by spec-0029: they now read from `dsl::chrome`, which is inventoried**), `BONFIRE_PROMPT_EN`, `BONFIRE_REST_LABEL_EN`, `BONFIRE_SAVE_LABEL_EN`, plus unnamed literals (`"Waiting for the party — "`, `"New objective: "`, `"Objective complete: "`, `"Delve Complete"`, the `"NPC"` name fallback). | A creator cannot replace these, and a **baked default is not l10n-inventoried** — so a non-English delve ships English sentences unless every site is authored by hand. | **Structural, and a real S4 instance.** The generality test fails: another game cannot re-word them. |
| 24 | `crates/grammar/src/library/bell/` — `barrow_shore.rs`, `chapel_ward.rs`, `cliff_road.rs`, `gate_ward.rs`, `hall_keep.rs`, `cistern_deep.rs` (1131 lines): **the zone programs of one specific campaign, inside the engine crate.** | By the owner's principle this is the largest open instance: one campaign's design compiled into the engine. | **Recorded, not judged.** This may well be deliberate — the grammar back end's first production workload. **A call for the owner. No move proposed.** |

## Process root cause, and why adoption is cheaper than it looks

**ADR-0015 already governs this.** It names the mechanism — *"the demo-level phase
drives DSL growth: each new mechanic forces a design decision under the no-hacks
rule, usually with exactly one motivating campaign (N=1)"* — and sets two
promotion gates: a **second-campaign gate** and a **machine-proof gate**. It even
names `crush` and `branch_points` as proof-motivated rather than
expressiveness-motivated.

The finding is that **four of the fields audited here were introduced by a task,
not a spec, and so recorded no ADR-0015 gate at all**: `sealed_hint` (task #142),
`missing_item_hint` (v0.7, no spec, and missing from the `DW0141` reserved list
that claims to be exhaustive), timed-gate `disarm` (task #184), and
`shortcuts[].on_unlock` (a single parenthetical in spec-0016 §2, never described
in the reference). The gate exists; the task route bypasses it.

**Adoption cost is near zero for most of this.** Measured across all tracked
campaign content:

- `on_unlock`, `sets_flag`, `via`, `payload`, `disarm`, `sealed_hint`,
  `respawns_on_rest`, `crush`, `telegraph`, `flask`, `bonfire`, `shortcuts`,
  `timed_gates`, `traps`, `ambushes`, `lane`, `drops`, `dropped_by` — **zero live
  usages**. Their only consumers are Rust fixtures. Reshaping any of them costs
  the released campaign nothing.
- Real cost, in order: `requires_flags`/`forbids_flags` (138 uses),
  `happening` (108), `label`/`title`/`hint`/`tooltip` (~180 across both
  campaigns), then a long tail of 1–3 uses each.

**One risk to record.** Only two campaigns can be migrated at all: `nobodys-cave`
(0.3.0) and `the-drowned-bell` (0.6.0) exist **only as untracked build outputs
with no source in git**, so any DSL change orphans them permanently.
`the-drowned-bell` is the campaign whose playtest produced `DW0420`/`DW0421`, and
it cannot be recompiled.

## Recommended fix sequence

Ordered by what is blocked today, not by size. Each names its adoption cost.

0. **Bring `shortcuts[].on_unlock` inside an enumeration** (#16b). Smallest change
   here and the only one that is a latent *correctness* bug rather than an
   expressiveness limit. Cheapest correct form is the `Ambush::to_trigger`
   pattern — desugar it into an existing root — not a sixth `EffectRootKind`.
   No DSL change, no version bump, **no adoption**, zero live usage. *Do this
   first and alone; the shortcut-door worker is inside this surface right now.*

   **DONE (spec-0031), and NOT by the desugar this item recommended.** It is
   `EffectRootKind::ShortcutUnlock`, root R6. The recommendation above was
   right about the shape of the bug and wrong about the cheapest correct fix,
   for a reason worth keeping: `Ambush::to_trigger` works because an ambush
   **is** a trigger — a one-shot `EnvTrigger` at an anchor is the entirety of
   what an ambush emits, so the sugar has nothing left over. A shortcut's
   unlock is not a trigger. Its detection is a once-only `#sc_<id>` sentinel
   poll that in the same function clears the gate region, retires the unlock
   affordance (`DW0421`) and kills the wrong-side bodies, and its permanence is
   structural (`DW0372`). Desugaring `on_unlock` into an `EnvTrigger` at the
   unlock anchor would have put **two independent detectors on one event** —
   the sentinel poll and the trigger's own — free to fire in different ticks or
   for one to fire when the other did not. Cheapness is not worth a second
   detector. The general rule the two cases share: *desugar when the sugar's
   whole meaning is the general construct; add a root when the bundle hangs off
   an object with runtime machinery of its own* (which is also why
   `traps[].payload` is R4 and not a desugared trigger).
1. **Widen `EnvTrigger.at` from a point to an anchor's shape** (#1–#3, #3b).
   #3b (a `narrate` `actionbar` style) is part of the same lift — without it the
   general effect still cannot reach the channel `sealed_hint` uses. Unblocks
   the `sealed_hint` lift and every future volume object at once, and is the only
   change that prevents rows 4–10 recurring. `dsl_version` **minor bump**; purely
   additive (a point anchor keeps its meaning), so **no campaign adoption** is
   forced. *Do this before, or with, the `sealed_hint`/shortcut-door lift — that
   lift is the first consumer, and doing it without this produces the third
   mechanism the owner already rejected once.*
2. **Lift `requires_flags`/`forbids_flags` to every `QuestEffect`** (#11). Blocks
   per-branch staging today. Additive, `skip_serializing_if` empty ⇒
   byte-identical for every existing campaign; minor bump, **no adoption round**.
3. **Close the effect-root walks** (#17–#19). #17 is a safety proof with a hole and
   should go first alone. No DSL change, no version bump, **no adoption** — pure
   compiler correctness. Strengthen `check-effect-roots.py` (drop the 40-line
   window; require every walk to go through `for_each_effect_root`) rather than
   fixing thirteen call sites by hand.
4. **Name the item-stack object class** (#14). Blocks souls drop content today.
   `dsl_version` minor; the six inlined shapes stay valid, so **no forced
   adoption**, but new capabilities only appear on the shared type.
5. **Route the private interaction bodies through the widened trigger** (#5–#10),
   in the order 5+6 (twins, one lift), then 7, 8, 9, 10. Emission moves ⇒ **owner
   playtest gate**, batched per CLAUDE.md, never per-PR.
6. **Make baked defaults authorable and inventoried** (#23). Player-visible ⇒
   playtest gate. Do it with #5–#10, not separately.
7. **Naming-only rows** (#20–#22) — one PR, no behaviour change, whenever
   convenient. `DialogueEffect`→`QuestEffect` unification (#16) belongs with 2.
8. **#24 is the owner's call.** Not scheduled here.

**Honest scope.** This is materially more than one round: steps 1–4 are each a
round of their own, and step 5 is a multi-PR sequence behind a playtest batch. The
cut I would take: **1 → 2 → 3 as one milestone** (they are the three that block
content today and none forces campaign adoption), and hold 4–7 until that lands,
because step 5's shape depends entirely on step 1's outcome.

## The machine form

`tools/check-capability-ownership.py`, wired into the `docs (local link check)`
job (an existing required context — no new job name, so no branch-protection
deadlock). Four ledger checks; each prints its binding count and **fails on a zero
binding**, because a gate that matched nothing is vacuous, not a pass.

> **These are the numbers the tool prints, re-measured 2026-08-10** by running
> `python3 tools/check-capability-ownership.py` on `main` and reading its own
> `N matched` column. Three of the five rows had drifted below the build (A
> 11→13, D 7→6, E 10→11) — a hand-copied binding count is exactly the unbound
> fact this table exists to prevent, so read it as a snapshot and re-run the
> tool rather than quoting the row.

| Check | Binds today |
|---|---|
| A — every `summon minecraft:interaction`, keyed by enclosing fn | **13** sites (was 9, then 11; the shortcut wrong-side lift added `ws_arm_fns`) |
| B — every compiler-baked player-facing English string | 3 constants (was 5; spec-0029 closed two) |
| C — DSL structs declared separately with an identical field set | 2 groups |
| D — cross-cutting modifier absent from some variants of a tagged enum | **6** (enum, field) pairs (was 6, then 8; `QuestEffect.happening` left the ledger at spec-0031 — see finding 12, which is still open) |
| E — every `Vec<QuestEffect>` bundle is reachable by some enumeration | **11** `Vec<QuestEffect>` fields |

Demonstrated firing on the live instances: with `seal_fns` and
`SEAL_HINT_DEFAULT` removed from the ledger — i.e. simulating `sealed_hint` being
introduced today — A and B both go red and name it; with `on_unlock` removed, E
goes red and names it. See the PR body for the transcripts.

That transcript is now **historical for B**: spec-0029 moved `SEAL_HINT_DEFAULT`
and `BOUNDARY_DEFAULT_MESSAGE` behind `dsl::chrome`, so neither is a baked literal
any more and the ledger entries were dropped — the checker demanded it by name, which
is the stale-exemption guard working on its first real occasion. The same demo
reproduces today against any surviving entry (`BONFIRE_PROMPT_EN`).

Check E is the one that would have caught #16b, and is the answer to "why did
`check-effect-roots.py` not catch this": that gate greps for the five roots it
knows, so a *sixth* root is invisible to it by construction. E asks the inverse
question — every bundle in the DSL must be claimed by some enumeration — which
does not depend on knowing the roots in advance.

**What it is not.** A/B are text scans; a private body built through a helper that
hides the `summon`, or a default assembled from fragments, is invisible. C/D parse
`stages.rs` structurally but see only what `pub` fields and variant blocks look
like textually. This makes the known shapes un-addable **in silence**. It does not
certify that none remain.
