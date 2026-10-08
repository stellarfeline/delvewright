# `delvec::compiler::seeding`

The reference page for `crates/delvec/src/compiler/seeding.rs`: the diagnostics-catalog row of every DW code
this module declares. The catalog's shared rules are in [`compiler.md` §5](../../compiler.md#5-diagnostics-catalog).

## Diagnostics

### DW0495 — emitted score-read integrity (`compiler::seeding`; error; exit 3)

**The runtime fact, measured on the pinned 1.21.11 server** (rcon plus a real
client joined): a scoreboard entry
does not exist until something **writes** it — joining creates nothing, a
`deathCount` objective has no entry until the player dies, a statistic objective
none until the statistic moves, a `trigger` objective none until it is enabled —
and **every** comparison against a holder with no entry is FALSE. `if score X O
matches 0` does not fire; nor does `matches 0..`; nor does `if score A oA > B
oB` with either side missing; `unless` is correspondingly always true, and
`scores={O=…}` matches no entity lacking an entry in `O`. `set`/`add`/`remove`/
`enable` create the entry, `operation` creates **both** its target's and its
source's, `execute store … score` creates it, and `reset` destroys it.

So an unwritten score is not zero. It is *false to every question*, including the
questions whose honest answer at zero is yes — which is why this is a defect
class and not one bug.

| Code | Meaning |
|------|---------|
| `DW0495` | **The compiler emitted a comparison against a scoreboard entry it never creates.** Build-tier (exit 3), `compiler::seeding::check_tree`, run last over the finished output tree, beside `DW0497` and the affordance self-check, on the same principle: judge the commands that ship. **The worked instance**: `dw.death_ack` and `dw.death_seen` are `dummy` objectives, so a player who has never died has no entry in either, and an unseeded `execute if score @s dw.deaths > @s dw.death_ack` does not fire. `on_death` and the checkpoint respawn dispatch would then never run on a player's **first** death — no forfeit, no recovery stake, no `on_respawn`, no engine re-seat — while both edges work from the second death onward, which is exactly why the class survives every shape proof and every manual test: anyone testing a death loop dies more than once. **Relation to `DW0501`.** That rule is the same insight one layer up and binds to a different object: a campaign-**declared** datum whose `requires_state` gate no verb ever writes, decided from the campaign JSON. Engine-internal objectives — `dw.death_ack`, `dw.cast`, `dw.dmask` — are declarable by nobody, so no campaign-layer rule could ever have reached them. This is the emitted-layer sibling, not a widening. **Evidence: a comparison is admitted on any one of four forms of one demand — the entry exists by the time this runs.** (1) *A write*: an unconditional write of that `(holder, objective)` earlier in the same body, or in a function the body calls unconditionally before it. (2) *A spelling*: the answer on a missing entry equals the answer at the baseline 0, i.e. any `matches` range that **excludes 0**, in either sense — the flag idiom §`set-flag` already documents, stated as a property instead of left to folklore; a range spanning the whole of `i32` is admitted separately, as the deliberate *does an entry exist* probe the generated PackTest suite is built on. (3) *A guard*: a conjunctive `if score <h> <O> matches <R>` with `0 ∉ R` — earlier in the same `execute` chain, or a sibling clause of the same `scores={…}` block — proves the entity has an entry in `O`, and therefore in every objective the pack always writes **alongside** `O`; that co-write group is computed from the tree (the bodies that can leave `O` holding a value `R` admits, intersected), never declared, which is what makes a stake ledger's `kx/ky/kz` provable behind its own `kl matches 1` and a shop's `shop_at` behind its `shop=1`. (4) *A driver*: the objective is written unconditionally, for an entity, by a function the `minecraft:tick`/`minecraft:load` chain reaches **without crossing a single condition** — the once-per-player seeding hooks (`state_seed`, `class_arm`), which land on a player's first tick before any player-driven site can ask. **Named limits, both of which admit rather than accuse:** ordering *within* one tick is not modelled; and a `#`-prefixed holder is not an entity (vanilla's own convention for a compiler-owned singleton, whose whole lifecycle is one emitter's arm/read pair), so for those the demand is only that something in the pack writes it. **Binding.** The check reports a census — total comparisons, entity reads, and which evidence admitted each — so a walker that stopped matching cannot read as a pass; `crates/delvec/tests/score_seeding.rs` floors both the totals and the entity reads per fixture, and separately floors the guard and driver rules on `economy`, the one fixture that exercises both. The message lists every unbacked read with its artifact path, line number, the whole command and the `<holder> <objective>` it reads. **Prescription: fix the emitter** — seed the entry on a path that reaches the comparison (`scoreboard players add @s <obj> 0` is idempotent and, on a `deathCount` objective, does not disturb the criterion), or write the comparison so a missing entry cannot change its answer. Never silence it by deleting the comparison. |
