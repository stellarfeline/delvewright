# What `DW0379` measures on shipping content

`DW0379` is the retry-cost lint: the proven walk from a rest point to the beat it can respawn the party into, warned when it exceeds `RETRY_BUDGET_TICKS` (1200 ticks = 60 s = 300 blocks at the 4 t/block sprint model `DW0355` uses). Its catalogue row in `compiler.md` says the budget is a design decision rather than a compiler one, and that no box-garden delve approaches it. This page is the measurement that decision is made against. It recommends no threshold.

## Instrument

| | |
|---|---|
| Engine | `d52b6061cab173b215f5dae5fd19eba0e4c2bfc1`, `crates/delvec/src/compiler/nav.rs` blob `8fbe4259d65337e2658fe600a7937e97873d5648` |
| Content | `92bbe6576eb84304c8a0bbb95d4413551b4f036c`, `campaigns/vesperhold` tree `6b9a416455b30e943e9990a9e17ba77ff25b5652`, `prefabs` tree `7b0770f3e22eca338118aacf4e5b3051269579d7` |
| Gallery | `gallery` tree `ba1e5a59645c7b482a9194a896fc21ee2e6e6cf4` at the engine revision above |
| Command | `delvec build <campaign> --prefabs <content>/prefabs --json -o <out>` |
| Nav model | the assembled `World` the completability proofs use — `world.snap_standable`, `world.snap_endpoint`, `world.find_path`, the same three calls `verify_retry_cost` makes |
| Reading | `blocks = path.len() - 1`; `ticks = blocks * 4`; `s = ticks / 20`. No distance is derived from coordinates |

Every walk is read out of `verify_retry_cost` itself, by an observation-only probe that prints each rest point's own `blocks` before the threshold comparison and changes no control flow. The probe is not committed: it exists only to take this reading, and the reading is cross-checked below against the lint's own emitted text.

The measured build is deterministic across the two runs the reading was taken from: the emitted tree hashes to `fd63eab0e7ed4e3436b95f16a04941b0372d59595a531a7d1b48eb696350bd4a` both times, and the diagnostic stream is identical with the probe on and off.

## Cross-check

The probe's readings share the nav model with the lint, so they are cross-checked against the one other reader of the same numbers: the lint's own diagnostic text, with `RETRY_BUDGET_TICKS` lowered to 1 tick and nothing else varied. All eight walks reproduce exactly — `106, 13, 12, 53, 23` on vesperhold and `20, 10, 10` on the gallery, in blocks, in the lint's own words. What the two methods share is the path; what they do not share is the reporting site, which is what was in doubt.

## Vesperhold: what `DW0379` computes

Five rest points, all five bonfires, in critical-path order. Denominator: 5 rest points declared in the plan, 5 measured, 0 skipped.

| # | rest point | fires after step | target beat | blocks | ticks | s |
|---|---|---|---|---|---|---|
| 1 | `anchor/causeway-fire` | 1 `obj/hear-tamsin` | `obj/cross-the-causeway` (step 2) | 106 | 424 | 21.2 |
| 2 | `anchor/cloister-fire` | 11 `obj/find-the-cloister` | `obj/hear-halvard` (step 12) | 13 | 52 | 2.6 |
| 3 | `anchor/watch-fire` | 24 `obj/clear-the-rampart` | `obj/meet-halvard-again` (step 25) | 12 | 48 | 2.4 |
| 4 | `anchor/tower-fire` | 29 `obj/wake-the-second-shard` | `obj/climb-to-the-belfry` (step 30) | 53 | 212 | 10.6 |
| 5 | `anchor/throne-fire` | 35 `obj/reach-the-antechamber` | `obj/approach-the-throne` (step 36) | 23 | 92 | 4.6 |

Maximum 106 blocks, 21.2 s. The budget is 300 blocks, 60 s. Nothing fires.

## Vesperhold: the deepest beat each rest point covers

The target in the table above is the **first** beat after the rest point, which is what the lint selects. A player does not die at the first beat; they die anywhere in the stretch the rest point is the checkpoint for. So the same nav model, over the same rest points, measured to every beat from the rest point up to and including the next rest point's own firing step. This second measure is **authored**, not cited: no spec names it, and it is here because the first measure cannot answer the question a human asks after a death.

Denominator: 5 rest points, 37 beat-walks computed, 0 refused for want of a path.

| # | rest point | beats covered | deepest beat | blocks | ticks | s |
|---|---|---|---|---|---|---|
| 1 | `anchor/causeway-fire` | 10 (steps 2–11) | `obj/find-the-cloister` (step 11) | 266 | 1064 | 53.2 |
| 2 | `anchor/cloister-fire` | 13 (steps 12–24) | `obj/clear-the-rampart` (step 24) | 266 | 1064 | 53.2 |
| 3 | `anchor/watch-fire` | 5 (steps 25–29) | `obj/wake-the-second-shard` (step 29) | 146 | 584 | 29.2 |
| 4 | `anchor/tower-fire` | 6 (steps 30–35) | `obj/reach-the-antechamber` (step 35) | 211 | 844 | 42.2 |
| 5 | `anchor/throne-fire` | 3 (steps 36–38) | `obj/approach-the-throne` (step 36) | 23 | 92 | 4.6 |

The full distribution, in blocks, in step order:

| rest point | walk to each beat it covers |
|---|---|
| `anchor/causeway-fire` | 106, 125, 125, 122, 144, 144, 199, 204, 144, 266 |
| `anchor/cloister-fire` | 13, 35, 48, 64, 84, 90, 82, 82, 27, 71, 71, 241, 266 |
| `anchor/watch-fire` | 12, 78, 78, 132, 146 |
| `anchor/tower-fire` | 53, 47, 52, 178, 192, 211 |
| `anchor/throne-fire` | 23, 23, 21 |

The two measures disagree most where the complaint is. At the Watch Fire the lint sees 12 blocks and the party walks up to 146; at the Tower Fire the lint sees 53 and the party walks up to 211. A rest point whose next beat is one room away and whose last beat is across the castle reads as cheap to the lint and expensive to the player.

## The sweep

Container: `campaigns/` in the content repository at the content revision above — 4 campaign directories, `README.md` excluded because it is not one. Plus `gallery/` in the engine repository, the engine's own campaign, included because a threshold read off one campaign is a threshold fitted to one campaign. Plus `demos/` in the content repository, 3 directories, enumerated and refused below.

The other three content campaigns are byte-identical at this content revision to content `main` (`git rev-parse <rev>:campaigns/<name>` equal on both sides for all three), so one revision covers the whole container.

| container/campaign | built | rest points | reason if not measured |
|---|---|---|---|
| `campaigns/vesperhold` | yes | 5 of 5 measured | — |
| `campaigns/doune-castle-tour` | no | 0 declared | `DW0100`: `quests` document carries `to_anchor`, a field the current `move-npc` schema does not have. Refused at stage validation, so no nav model is assembled |
| `campaigns/hollow-vigil` | no | 0 declared | `DW0100`: `world` document is missing `time`. Refused at stage validation |
| `campaigns/nobodys-cave-island` | no | 3 `set-checkpoint` declared | `DW0100`: `quests` document carries `to_anchor`. Refused at stage validation — the three rest points it declares are unmeasurable until it builds |
| `gallery` (engine repo) | yes | 3 of 4 measured | 1 skipped, below |
| `demos/gatehouse-elevations` | no | — | `DW0874`: not a campaign directory — it holds a piece, its program and its report, and none of the 6 stage documents |
| `demos/guard-exhaustion` | no | — | `DW0874`: same shape |
| `demos/mill-race` | no | — | `DW0874`: same shape |

The rest-point counts for the three campaigns that do not build are counts of `bonfire` and `set-checkpoint` verbs in their authored `quests.json`, not plan counts — a campaign refused at stage validation never produces a plan, so a declaration is all there is to count and a walk is not derivable from it at all.

Three of four content campaigns not building is not a defect: nothing owes compatibility to anything already built, and those documents were authored against a DSL surface that has moved. What it does mean is that **the distribution below rests on one campaign plus the gallery**, and a threshold chosen from it is chosen from that.

### Gallery

Denominator: 4 rest points declared in the plan, 3 measured, 1 skipped. Anchor names are not unique here — three rest points resolve to `anchor/hearth` — so rows are keyed by `(anchor, firing step)`, and the two rows keyed `(hearth, 1)` are distinguishable only by plan order.

| # | rest point | kind | target beat | blocks | s | deepest in segment |
|---|---|---|---|---|---|---|
| 1 | `anchor/hearth` @ step 1 | checkpoint | `obj/press-the-case` (step 2) | 10 | 2.0 | 20 blocks (step 4) |
| 2 | `anchor/hearth` @ step 1 | checkpoint | `obj/press-the-case` (step 2) | 10 | 2.0 | 20 blocks (step 4) |
| 3 | `anchor/hearth` @ step 4 | bonfire | `obj/answer-the-marshal` (step 5) | 20 | 4.0 | 22 blocks (step 9) |
| 4 | `anchor/lectern` @ step 9 | — | — | — | — | skipped: no beat follows its firing step, so there is nothing to respawn into |

One beat inside the third row's segment, `obj/climb-the-loft` (step 8), has no path in the nav model and is excluded from its maximum; a climb that the walking model cannot route is `DW0315`'s question, not this one.

## What `DW0379` emits

Nothing, on every campaign that builds. Read from the diagnostic stream, not assumed:

| campaign | diagnostics emitted | of which `DW0379` |
|---|---|---|
| `campaigns/vesperhold` | 5 warnings (`DW0351` ×3, `DW0781`, `DW0810`) | 0 of 5 |
| `gallery` | 37 warnings (`DW0527` ×24, `DW0477` ×3, and one each of `DW0330`, `DW0351`, `DW0353`, `DW0453`, `DW0467`, `DW0475`, `DW0498`, `DW0813`, `DW0822`, `DW0889`) | 0 of 37 |

The catalogue's "effectively inert in practice" is confirmed as stated, on the two campaigns that can test it. The claim next to it — that no box-garden delve approaches 300 blocks — holds for what the lint measures (peak 106) and is close to false for what a player walks (peak 266).

## Where a threshold would have to sit

Arithmetic over the tables above, not a recommendation. Denominator: 8 measured rest points — 5 vesperhold, 3 gallery. Column **A** is the quantity `DW0379` computes today; column **B** is the deepest-beat quantity.

| threshold | blocks | A fires | which | B fires | which |
|---|---|---|---|---|---|
| 15 s | 75 | 1 of 8 | causeway | 4 of 8 | causeway, cloister, watch, tower |
| 20 s | 100 | 1 of 8 | causeway | 4 of 8 | causeway, cloister, watch, tower |
| 25 s | 125 | 0 of 8 | — | 4 of 8 | causeway, cloister, watch, tower |
| 30 s | 150 | 0 of 8 | — | 3 of 8 | causeway, cloister, tower |
| 45 s | 225 | 0 of 8 | — | 2 of 8 | causeway, cloister |
| 60 s | 300 | 0 of 8 | — | 0 of 8 | — |

Read against the reported complaint — the stretch from the third fire onward — the two columns answer differently:

- On **A**, no threshold catches the Watch Fire before it catches nearly everything. Its walk is 12 blocks, third shortest of the eight; the highest threshold that warns on it is 2.3 s, and at 2.3 s six of the eight warn, the gallery's own bonfire among them. The quantity cannot separate the complaint from the campaign.
- On **B**, the Watch Fire's 146 blocks is the fourth largest of the eight, and every threshold in the band 4.6 s ≤ t < 29.2 s catches exactly the four vesperhold fires with a long stretch behind them and neither gallery hearth. The separation the complaint asks for exists in this quantity and not in the other.

So the question in front of a threshold decision is not only *what number* but *of what*: no value of `RETRY_BUDGET_TICKS` makes the current measure fire on the stretch a human reported without firing on rest points nobody has complained about.
