# spec-0093: Quiet guidance — an objective says whether it is marked and whether it is announced, a campaign states the default, and the proofs that stood on the marker and the announcement stand on the act instead

- **Status**: Approved
- **Ground**: written against engine `59de66925` (`origin/main`, delvec 1.8.2, dsl 0.35.1), read only — `Objective`, `QuestsContent`, `EnvTrigger`, `TriggerOn`, `Wave` in `crates/dsl/src/stages.rs`; `wave_area` in `crates/dsl/src/fight.rs`; `Placement` in `crates/dsl/src/placement.rs`; `check_objective_prompts` and `DW_FIGHT_UNSIGNED` in `crates/delvec/src/compiler/promise.rs`; `activation_commands`, `marker_name_fields`, the announce/complete emission, `option_display_conditions`, `pending_guard`, `check_wave_spawns` and `emit_one_dialogue_mask_packtest` in `crates/delvec/src/compiler/emit.rs`; `collect_v06_effects`, `CheckpointPlan`, `path_triggers` and `firing_of` in `crates/delvec/src/compiler/plan.rs`; `check_checkpoints`, `verify_checkpoints`, `critical_route_cells`, `region_state_at`, `reachable_walkable` and `aggro_sources` in `crates/delvec/src/compiler/nav.rs`; `Flow::skips`, `Flow::offered` and `DW_OPTIONAL_GATES_MAINLINE` in `crates/delvec/src/compiler/flow.rs`; row `bell-14` and row `isl-55` of `docs/playtest-findings.json`; `probe_is_self_measuring` in `tools/creator/staging-gate.py`. Each claim about the tree in §2 was reproduced by building the campaign that motivated this spec, pristine and under five perturbations, with the binary built from that revision.
- **What it is for**: a delve whose guidance is quiet. No glowing marker over the thing to press, no `New objective` line, objectives with no title, fights and checkpoints started by what the player does — a story that rewards exploring and reading. Today an `interact` with no `prop` always summons a glowing lantern, a titled objective always prints the chrome, a `kill` without both lines is refused, a wave a trigger fires in a site-plan campaign is refused as unplaceable, a checkpoint a trigger sets is proven as if it fired at the entry, and two talk beats in a row are refused because the dialogue button that completes the second ignores the `after` the author declared. A creator meets each of these as a wall and authors around it; the campaign that motivated this spec removed every title, every hint, every kill objective and every trigger-set checkpoint to get past them, and then read red at the staging gate for having fights with no kill objective.
- **Research**: §2 is this spec's record of what the tree does. Each statement is marked **cited** (a line of the tree at `59de66925`, a reading taken by running that tree, a published source already in `docs/reference/`, or a constitution rule) or **authored** (this spec chooses).
- **Numbers**: spec `0093`. Two new diagnostics: `DW0961` (an announcement asked for with nothing to announce, §7) and `DW0962` (a marker declared on an objective that places none, §7). `DW0862`, `DW0863`, `DW0310` and `DW0315` change what they judge under their own codes (§4–§6). `DW0205` is retired (§6.3). The surface change is stage 5 and takes `dsl_version` `0.36.0`, the unreleased minor other unreleased surface changes share.
- **Non-goals**: a marker drawn differently (another item, another colour — the marker is shown or not, and what it looks like stays the compiler's); a per-player announcement (the objective is the party's, spec-0018); an announcement on another channel (actionbar, title — a `narrate` in the objective's bundle does that today); hiding the bot's completion marker (`[dw:complete …]` is the proof's oracle and is never player-facing text); a quest-level or area-level override (an objective is the object the marker and the announcement act on, and the campaign default is the second level; a third would be a second authority for the same value); any change to what a `prop` is (an authored block is the affordance and never carried a marker).

## 1. The things, and the objects they belong to

**Cited** (`emit.rs`, `activation_commands`): when an objective activates the compiler places its world presence — a `collect`'s chest, an `interact`'s hitbox and, absent a `prop`, a glowing `minecraft:lantern` `item_display`; a `reach-anchor`'s glowing `minecraft:end_rod`. **Cited** (`emit.rs`, the announce function and the completion body): for a **titled** objective the compiler emits `announce_<obj>` (`New objective: <title>`, the hint's line, a sound, once per party) and the `Objective complete: <title>` line; for an untitled objective it emits none of these. **Cited** (`promise.rs`): `DW0862` refuses a `hint` with no `title`, because the hint's line is nested inside the title's guard; `DW0863` refuses a `kill` without both lines, because a fight is the one kind the compiler puts nothing in the world for.

**Authored — two properties of one object.** Whether an objective is **marked** and whether it is **announced** are properties of the objective, as its `title` and `hint` already are. Each is a value of a two-member closed set, `shown` or `hidden`, carried by the objective and defaulted by the campaign:

- `quests.content.guidance` — `{markers, announcements}`, each `shown` (the default when the block is absent) or `hidden`: the campaign's default for every objective.
- `objectives[].marker` — on `interact` and `reach-anchor`, the two kinds that summon a marker; absent means the campaign default.
- `objectives[].announcement` — on every kind; absent means the campaign default.

An objective **is marked** when its kind summons a marker and its resolved `marker` is `shown`; it **is announced** when it has a `title` and its resolved `announcement` is `shown`. An `interact` with a `prop` never summons a marker, so `marker` on it declares nothing (§7).

Placements considered and rejected. **On the quest**: a quest is where a beat is booked, not the object the lantern hangs over; two objectives of one quest can legitimately differ, and the campaign default already answers the "whole chapter is quiet" case. **Dropping `title` to silence the chrome** (what the motivating campaign did): the title is also the marker's nameplate and the name every report, render plan and ledger uses for the beat, so a creator who wants a named, unannounced objective has no way to say it, and a creator who wants a hint with no chrome has none either. **A `marker_item`** (choose the lantern's icon): a different question; the shop and the stake already carry one and nothing here changes it.

## 2. What the tree does today, measured

Five perturbations of one site-plan campaign at `59de66925`, each a copy of the pristine tree with one edit, built with `delvec` from that revision against the content repository's prefab library. The pristine build is green in 30 s.

| Edit | Reading |
|---|---|
| none | 16 `interact` objectives, all untitled, none with a `prop`: the datapack summons **16** glowing lantern `item_display`s and **0** objective chrome lines; `promise:` examines 24 objectives, 0 `kill` |
| a wave fired only by an `approach` trigger | `DW0310`: *its spawn anchor is not placed in any assembled area* |
| an `approach` trigger whose only effect is `set-checkpoint` on a late anchor | `DW0315`: *the next required anchor `[126, 96, 10]` is not walkable from it* — the entry, because the checkpoint is rooted at step 0 |
| the second of two talk beats given `after` the first | `DW0205`: *already offers the option that completes … at step #19 — before … has happened, and … declares `after`* |
| the same, gated on the flag the first beat sets | `DW0205`, naming the flag edge |
| an untitled `kill` on a wave the player's own act spawns | `DW0863`: *carries neither a `title` nor a `hint`* |
| the pristine build at the staging gate | `bell-14 UNBOUND — 0 × [type=kill] … but 6 × [mobs:declared] exist` |

**Cited** (`emit.rs`, `option_display_conditions` and the option body): a dialogue option carrying `complete-objective X` is displayed, and its click completes `X`, when `X`'s quest is active and `X` is not yet complete — and on nothing else. **Cited** (`emit.rs`, `pending_guard`): every other objective driver is guarded by quest active ∧ every `after` complete ∧ the objective's whole gate ∧ not yet complete. **Cited** (`flow.rs`, module doc): `DW0205` exists because of that one asymmetry — *the dialogue button does not [go through `pending_guard`], and that asymmetry is the whole defect class.*

**Cited** (`fight.rs`, `wave_area`): a wave fired by a trigger resolves an area only when `campaign.world.content.areas.len() == 1`. **Cited** (`placement.rs`, `Placement::SitePlan`): a site-plan campaign has `areas[]` empty and *exactly one area*, `SITE_AREA`. So the single-area test is false for the one kind of campaign that is always single-area.

**Cited** (`plan.rs`, `collect_v06_effects`): a `set-checkpoint` in a trigger's bundle is rooted at step 0, *conservative … so require the checkpoint to re-reach the whole remaining path*. **Cited** (`plan.rs`, `path_triggers`): the path performs a trigger only when its bundle opens a way or pays a flag debt; a trigger that only sets a checkpoint is never a path step, so `trigger_step` never holds it.

**Cited** (`nav.rs`, `aggro_sources` and `DEFAULT_FOLLOW_RANGE`): the radius inside which a wave's bodies acquire the party is the lane's `aggro_radius`, else the wave's declared `follow_range`, else 16 — vanilla's `generic.follow_range` default for the common hostiles. `DW0380` and `DW0478` already read the wave's reach this way.

**Cited** (`staging-gate.py`, `probe_is_self_measuring`): a `dsl` predicate that selects by identity (`eq`/`in`/`prefix`) counts the class itself, and its zero is `INAPPLICABLE`; a declared `applies_when` that counts a different class turns that zero into `UNBOUND`. Row `bell-14` binds `{eq: {type: kill}}` and declares `applies_when: {has: [mobs]}`.

## 3. The surface

Stage 5, `dsl_version` `0.36.0`.

```json
{
  "guidance": { "markers": "hidden", "announcements": "hidden" },
  "quests": [{
    "objectives": [
      { "type": "interact", "id": "obj/the-slate", "anchor": "anchor/slate", "marker": "shown", "announcement": "hidden" },
      { "type": "reach-anchor", "id": "obj/the-rim", "anchor": "anchor/rim", "radius": 2, "marker": "hidden" },
      { "type": "kill", "id": "obj/the-watch", "wave": "wave/watch" }
    ]
  }]
}
```

`Visibility` is `shown | hidden`. `guidance` is omitted from the canonical form when both fields are `shown`; `marker` and `announcement` are omitted when absent. A pre-0.36 document that declares none of them is byte-identical in every emitted file.

**Resolution** (`Objective::marker_shown(&self, &Guidance)`, `Objective::announced(&self, &Guidance)`): the objective's own value when present, else the campaign's; `announced` is additionally false for an objective with no `title`.

## 4. Emission

- An `interact` with no `prop` summons its lantern only when it is marked; the `minecraft:interaction` hitbox is summoned either way, because the hitbox is what the player presses. A `reach-anchor` summons its end rod only when it is marked. The completion cleanup is unchanged (`kill @e[tag=…]` over the objective's tag matches nothing extra).
- `announce_<obj>`, its `tick` line, its `dw.ann_<obj>` scoreboard and the `Objective complete` line and sound are emitted only for an announced objective. An objective with a title and `announcement: hidden` keeps its title on the marker's nameplate (when marked), in `critical-path.json`, in the render plan and in the l10n inventory.
- Nothing about the bot's channel moves: `[dw:complete <ns> <obj>]` is broadcast on every completion as before.

## 5. The rules that stood on the announcement

**`DW0862` — a prompt nobody sees** (widened, same code). An objective authors a `hint` and is **not announced** — no `title`, or its resolved `announcement` is `hidden`. The message names which. A hint is the second line of an announcement; under hidden announcements it is prose inventoried, translated and never shown. Binding: every objective.

**`DW0863` — a fight the party cannot find** (re-grounded, same code, moved from the validation funnel to the build, where positions exist). A `kill` objective is **discoverable** when either holds:

1. it is announced and carries a `hint` (the standing rule: the two lines say where the wave arrives); or
2. **the fight finds the party**: every site that fires `spawn-wave` for its wave is a **placed act** — a place the party stands at when the bundle fires — and the wave's spawn anchor lies within the wave's **reach** of that place. Reach is the radius `aggro_sources` already reads: the lane's `aggro_radius`, else the largest declared `follow_range` among the wave's mobs, else `DEFAULT_FOLLOW_RANGE` (16). Distance is Euclidean, anchor cell to act cell.

A placed act, per effect root: an objective's completion bundle fires where that objective completes (the critical path's step position for the objective, else its anchor resolved in its quest's area; a `talk-to` at its NPC's step position; a `kill` at its wave's anchor); a quest's `on_complete` fires where each of the quest's last objectives completes (every objective no other objective of the quest declares `after`; all must be within reach); a trigger fires at its `at` anchor, or at the struck NPC's or assembly's anchor; a trap payload at the trap's anchor; a shop offer at the shop's anchor; a fight's `on_kill` at that fight's own anchor; an assembly's `on_land` at the assembly's anchor. The remaining roots — a checkpoint's `on_respawn`, a shortcut's `on_unlock`, `on_death`, a loop's `on_cross` — place the party nowhere this rule can name, and a wave they fire is not found by the party by this rule. A `sequence` step fires from the same place as its bundle, whatever its offset: the party may walk away during a delay, and that is the author's choice, stated here as authored.

A `kill` that is neither is refused, naming the wave, every firing site with its distance and the reach it was held to, and both remedies: announce it with a `hint`, or spawn it where the act is. Binding line: `kill` objectives examined, of which announced, of which found by the party, firing sites measured.

The gallery's probe `a-fight-nobody-points-at` stays a refusal: `wave/muster` is fired by `quest/near-hall`'s completion at the counter and arrives at the muster line, farther than its reach (§10 measures the distance).

## 6. The rules that were wrong

### 6.1 `DW0310` — a trigger has a place

`wave_area` asks `Placement::of(c)`: a site-plan campaign is single-area and its sole area is `SITE_AREA`; a prefab campaign is single-area when `areas.len() == 1`. The rest of the resolution order is unchanged. A trigger-fired wave in a multi-area prefab campaign stays unresolvable at this layer — the trigger is global and its `at` anchor's area is not knowable before placement — and the message says so in those words, naming the two remedies: fire it from a quest booked in the wave's area, or make the campaign single-area. The refusal is right there; it was wrong for a site plan.

### 6.2 `DW0315` — a trigger fires where the party can reach it

A checkpoint set from a trigger's bundle is rooted at the **earliest configuration in which a body on the route can reach a cell the trigger fires from**, computed over the assembled world in `nav::check_checkpoints`:

- the trigger's **firing cells**: for `approach {range}`, every standable cell within `range` of the `at` anchor; for `use`/`strike`, every standable cell within `crosshair::INTERACTION_REACH` (3.0) of it; for `strike-npc` and `strike-assembly`, the same around the NPC's or assembly's anchor;
- the **configurations**: the ones `DW0921` already judges — the critical route's cells grouped by `region_state_at` of their arrival step, each with the first step arriving under it;
- the root is `first − 1` for the earliest configuration whose walkable flood (`reachable_walkable` from that configuration's route cells, over `with_region_state`) meets a firing cell; a trigger the path performs (`trigger_step`) is rooted at that step instead, as the region model already roots its writes; a trigger no configuration reaches keeps step 0.

Flooding from the union of a configuration's route cells rather than from each step's own cells can only root earlier (a leg carried by a crossing is credited to the whole configuration), which is the conservative direction. The checkpoint's reign for the respawn proofs (`CheckpointPlan::fire_step`) is unchanged at 0: a reign that starts early can only refuse more. The message names the root it derived (*the party can first reach `trigger/x` while `obj/y` is next*) beside the unreachable anchor, so a creator reads why the proof started where it did.

### 6.3 `DW0205` — the button obeys the objective's gate, and the rule has nothing left to say

The dialogue option that completes an objective is displayed, and its click completes the objective, under the objective's **whole** `pending_guard` — quest active ∧ every `after` complete ∧ `requires_flags` ∧ `forbids_flags` ∧ `requires_state` ∧ not yet complete — exactly as every other driver. The generated `dialogue_mask` PackTest drives each of those terms, breaking one at a time, from the objective's own declaration.

With that, the class `DW0205` named cannot exist: a button on screen before the beat it follows is a button whose objective is not yet pending, and the compiler no longer draws it. `DW0205`, `Flow::skips`, the per-branch skip pass and the ledger carrier of `isl-55` are retired; `isl-55`'s carrier becomes the emission invariant that proves the button carries its objective's `after` and gate terms. `DW0191` is unchanged: an author-written gate on the one completing option could still deadlock a beat, and the implicit pending gate opens exactly when the objective activates, so it cannot. The remedy `DW0205` used to prescribe (gate the way in) remains legal authoring; it is no longer required.

The owner-hit softlock this rule was built from ("Lead on." beside "We climb." from the first tick) is dissolved by construction: "Lead on." is not drawn until `obj/climb-out`'s `after` is complete.

### 6.4 `bell-14` — the gate's precondition counts the class the carrier judges

The row's binding is `kill` objectives — the class its general form names (*a mandatory encounter is discoverable from the objective that requires it*). Its `applies_when` counted waves, a different class, so a campaign with waves and no kill objective read `UNBOUND`. The precondition is removed; the identity-shaped binding is self-measuring and a campaign that declares no `kill` objective reads `INAPPLICABLE`. **This is a loosening, in those words**: the gate no longer refuses a campaign whose fights are not required by any objective. What bounds it: a wave nothing requires the party to win is not an encounter the party must find, and the fights such a campaign does have are judged by `DW0380` (an optional fight must have a bypass) and `DW0376`.

## 7. Refusals

| Code | Shape | Tier |
|---|---|---|
| `DW0961` | `announcement: shown` on an objective with no `title` — an announcement asked for with nothing to announce. | validation (`promise`) |
| `DW0962` | `marker` on an `interact` that declares a `prop` — the prop is the affordance, no marker is ever summoned, so the field declares nothing. | validation (`promise`) |
| `DW0862` | a `hint` on an objective that is not announced (§5). | validation (`promise`) |
| `DW0863` | a `kill` neither announced with a hint nor found by the party (§5). | build (`promise`, from `emit::build`) |

Both new codes refuse an inert declaration rather than ignore it (CLAUDE.md: a declaration that binds to nothing is a zero binding, reported, never a quiet pass).

## 8. What the owner's campaign can now say

A `guidance` block of `hidden`/`hidden`; interacts with no marker; titles where the creator wants a name without a line; fights started by a use or an approach, each wave within reach of the act; checkpoints set by approach triggers, proven from where the party can first reach them; two talk beats in a row ordered by `after`. The staging gate reads `bell-14` as `INAPPLICABLE` on a campaign with no kill objective and `BOUND` on one with them.

## 9. Acceptance criteria

Each is a test or a gate run, named.

1. **Surface.** `delvec schema --stage quests` exports `QuestsContent.guidance`, `Guidance.markers`, `Guidance.announcements`, `Visibility::shown`, `Visibility::hidden`, `marker` on `interact` and `reach-anchor`, and `announcement` on all five objective kinds; `tools/ci/check-gallery-coverage.py` reports every one of them bound or refusal-proven. A document carrying none of them canonicalises byte-identically (`crates/dsl` round-trip test).
2. **Markers.** An `interact` with no `prop` and `marker: hidden` emits a `minecraft:interaction` and no `item_display`; with `marker: shown` under a campaign default of `hidden` it emits both; a `reach-anchor` under `guidance.markers: hidden` emits no `end_rod`. Perturbing the declaration moves an emitted byte (gallery baseline).
3. **Announcements.** A titled objective with `announcement: hidden` emits no `announce_<obj>`, no `dw.ann_<obj>` objective, no `tick` announce line and no `Objective complete` line, and still broadcasts `[dw:complete …]`; the title is still on the marker when marked. Under `guidance.announcements: hidden`, an objective with `announcement: shown` and a title is announced.
4. **`DW0961` / `DW0962`.** Each is raised on its shape and silent one edit away; a gallery probe per code.
5. **`DW0862`.** Raised for a hint under `announcement: hidden` and under `guidance.announcements: hidden`; silent for the same objective once announced; silent for an objective with neither string.
6. **`DW0863`.** (a) Silent for an announced kill with a hint. (b) Silent for an untitled kill whose wave is fired by an `interact` completion at a place within the wave's reach of its anchor. (c) Raised when that wave's anchor is moved beyond reach, naming the site and the distance. (d) Raised when the wave is fired from a root with no place. (e) The gallery probe `a-fight-nobody-points-at` is still refused with `DW0863`, and the run's binding line states the distance it measured.
7. **`DW0310`.** A site-plan fixture with a wave fired only by a trigger builds; the same wave in a two-area prefab fixture is refused with the multi-area message.
8. **`DW0315`.** Over a synthetic world: a trigger-set checkpoint on the far side of a one-way drop, whose trigger is reachable only from the far side, passes; the same checkpoint with the trigger reachable from the entry is refused; rooting from a performed trigger uses `trigger_step`. On the `v06-checkpoints` fixture an `approach` trigger setting a checkpoint at the shrine builds green and the message of a red names the derived root.
9. **The button.** On the `beach` fixture that motivated `DW0205`, the ungated variant builds with no `DW0205` and the dmask condition for "Lead on." carries `obj/surf`'s completion term; the generated `dialogue_mask` PackTest breaks the `after` term and the `requires_flags` term and asserts the bit gone. `DW0205` is declared nowhere in `crates/` and listed nowhere in `docs/reference/compiler.md` (`check-dw-codes` bidirectional).
10. **`bell-14`.** `tools/tests/test_staging_gate.py`: a campaign with waves and no `kill` objective reads `INAPPLICABLE` for `bell-14`; one with a `kill` objective reads `BOUND`.
11. **Demo level.** `docs/demo-levels.md` carries the row *Quiet guidance*; the level on content branch `demo/quiet-guidance` builds, passes PackTest, the bot walk and the staging gate, and holds: an unmarked interact, no chrome, an untitled kill started by a use, a checkpoint set by approach, and two talk beats in a row.
12. **Pages.** `references/quest-capabilities.md` teaches the `guidance` block, both per-objective fields, the two ways a fight is discoverable, that `after` on a talk beat is honoured by its button, and that a trigger-set checkpoint is proven from where the trigger is first reachable; `tools/ci/check-skill-page.py` is green.

## 10. Measurements this spec states

Taken on the tree this spec lands with, and written into the commit body that lands them: the distance from the gallery's counter to `anchor/muster` against the reach of `wave/muster`; the `DW0921`-style configuration count and the derived root for the `v06-checkpoints` trigger checkpoint; the gallery coverage line (units enumerated, bound, refusal-proven, 0 in neither state).
