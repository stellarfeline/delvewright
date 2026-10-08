//! Checkpoints: bonfires, `set-checkpoint`, the death bundle, flasks, and the
//! v0.6 collector that plans checkpoints and stealth beats together.

use super::*;

/// A resolved `set-checkpoint` effect (DSL v0.6, spec-0012), collected in
/// deterministic content order so its `index` is a stable, byte-identical id used
/// both for the active-checkpoint marker (`#cp dw.sys`) and its `on_respawn`
/// dispatch function.
#[derive(Clone, Debug)]
pub struct CheckpointPlan {
    /// Stable content-ordered id (0-based).
    pub index: usize,
    /// The checkpoint anchor name.
    pub anchor: String,
    /// The resolved absolute anchor cell.
    pub pos: [i32; 3],
    /// Per-player `on_respawn` effects (may be empty). For a bonfire
    /// (`rest == true`) this is the `on_rest` bundle: the same effects run on a
    /// rest and on a respawn (spec-0016 §1).
    pub on_respawn: Vec<QuestEffect>,
    /// `critical_path` step index at which this checkpoint fires (roots DW0315).
    /// For a bonfire this is the step that **arms** the rest affordance — the
    /// earliest beat at which a rest (and therefore a respawn here) is possible,
    /// so the no-stranding proof stays conservative. For a checkpoint a
    /// **trigger** sets this is `0` — the party's own act, not a beat — and the
    /// no-stranding proof re-roots it at the earliest configuration in which the
    /// trigger is reachable (spec-0093 §6.2, `nav::check_checkpoints`).
    pub fire_step: usize,
    /// The `triggers[]` entry whose bundle sets this checkpoint, when a trigger
    /// does; `None` for a checkpoint a quest beat or a dialogue option sets.
    pub trigger: Option<String>,
    /// `true` for a `bonfire` (spec-0016 §1): the checkpoint moves only when the
    /// party rests at the affordance, not when the effect fires. `false` for a
    /// plain `set-checkpoint` (spec-0012), which is immediate.
    pub rest: bool,
    /// The bonfire rest dialog's three strings, already resolved against the
    /// compiler's canonical English. Meaningless for a
    /// plain `set-checkpoint`, which shows no dialog.
    pub prompt: String,
    /// The **rest and save** button label.
    pub rest_label: String,
    /// The **save only** button label.
    pub save_label: String,
    /// The **rest and save** button's hover tooltip, as authored (spec-0078);
    /// `None` emits no tooltip.
    pub rest_tooltip: Option<String>,
    /// The **save only** button's hover tooltip, as authored (spec-0078).
    pub save_tooltip: Option<String>,
}

impl<'a> Plan<'a> {
    /// Whether any collected checkpoint carries an `on_respawn` hook — gates the
    /// vanilla respawn-detection machinery so checkpoint-free / hook-free campaigns
    /// stay byte-identical (DSL v0.6, spec-0012).
    pub fn any_checkpoint_on_respawn(&self) -> bool {
        self.checkpoints.iter().any(|c| !c.on_respawn.is_empty())
            || !self.reseat_waves().is_empty()
            || !self.undefeated_reseat_waves().is_empty()
            || !self.reseat_actors().is_empty()
    }

    /// Whether the campaign declares **any** checkpoint at all (spec-0012 /
    /// spec-0016 §1). Gates the respawn **re-seat** machinery: the delve's own
    /// promise is "die and resume at the last checkpoint", and vanilla's
    /// `/spawnpoint` is only a hint — it silently falls back to the world spawn
    /// whenever the recorded cell is not a legal respawn position.
    /// A campaign with no checkpoint keeps the pre-0.6 emission byte-for-byte.
    pub fn any_checkpoint(&self) -> bool {
        !self.checkpoints.is_empty()
    }

    /// The campaign's `on_death` bundle (DSL v0.10, spec-0031) — effect root R7,
    /// the effects that run at the moment a player dies. Empty for every campaign
    /// below 0.10.0 and for any that declares no death beat, which is what keeps
    /// the whole corpse-side half of the death edge out of their emission.
    ///
    /// Read straight off the campaign rather than planned into a field: unlike a
    /// checkpoint or a shortcut this bundle resolves no geometry, so a planning
    /// step would only be a second place for it to go stale.
    pub fn on_death(&self) -> &[QuestEffect] {
        &self.campaign.quests.content.on_death
    }

    /// The collected checkpoint matching a `set-checkpoint` effect (by anchor +
    /// `on_respawn` list), giving the emitter its stable content-ordered index.
    pub fn checkpoint_for(
        &self,
        anchor: &str,
        on_respawn: &[QuestEffect],
    ) -> Option<&CheckpointPlan> {
        self.checkpoints
            .iter()
            .find(|c| !c.rest && c.anchor == anchor && c.on_respawn.as_slice() == on_respawn)
    }

    /// The collected **bonfire** matching a `bonfire` effect (by anchor +
    /// `on_rest` list), giving the emitter its stable content-ordered index
    /// (spec-0016 §1). Disjoint from [`Self::checkpoint_for`]: a bonfire and a
    /// plain `set-checkpoint` may share an anchor and a hook list and still be
    /// two distinct rest points.
    pub fn bonfire_for(&self, anchor: &str, on_rest: &[QuestEffect]) -> Option<&CheckpointPlan> {
        self.checkpoints
            .iter()
            .find(|c| c.rest && c.anchor == anchor && c.on_respawn.as_slice() == on_rest)
    }

    /// Every collected bonfire (spec-0016 §1), content-ordered.
    pub fn bonfires(&self) -> impl Iterator<Item = &CheckpointPlan> {
        self.checkpoints.iter().filter(|c| c.rest)
    }

    /// For each [`Self::checkpoints`] entry, in the same order: the step at which
    /// it stops being where a dead player lands, or `None` for "never".
    ///
    /// A plain `set-checkpoint` is **monotonic** (spec-0012): the next one to fire
    /// replaces it outright, so its reign is `[fire_step, next_set_checkpoint)`.
    /// A later **bonfire** does not end it — the checkpoint moves only when the
    /// party actually rests, and nobody is forced to (the same "an unguaranteed
    /// firing may be assumed only where assuming so is conservative" rule
    /// [`collect_region_events`] states).
    ///
    /// A **bonfire**'s reign is `None`: the party can return to the last fire they
    /// rested at for the rest of the campaign, and a proof over it must not narrow
    /// on the strength of a rest that might never happen. That is also exactly
    /// today's behaviour, so every bonfire proof is unchanged by this existing.
    pub fn respawn_reign_ends(&self) -> Vec<Option<usize>> {
        let later_plain: Vec<usize> = self
            .checkpoints
            .iter()
            .filter(|c| !c.rest)
            .map(|c| c.fire_step)
            .collect();
        self.checkpoints
            .iter()
            .map(|c| {
                if c.rest {
                    return None;
                }
                later_plain
                    .iter()
                    .copied()
                    .filter(|s| *s > c.fire_step)
                    .min()
            })
            .collect()
    }

    /// Every class-kit **flask** (DSL v0.8, spec-0016 §1): `(class index, kit
    /// index)` pairs in declaration order — the recovery stacks a bonfire rest
    /// replenishes to their declared `count`. Empty for a campaign that declares
    /// none, which is exactly the campaigns whose emission stays byte-identical
    /// (`DW0476` guarantees a bonfire campaign is never in that set).
    pub fn flasks(&self) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for (i, class) in self.campaign.classes.content.classes.iter().enumerate() {
            for (k, item) in class.kit.iter().enumerate() {
                if item.flask {
                    out.push((i, k));
                }
            }
        }
        out
    }
}

/// Collect every `set-checkpoint` and `begin-stealth` effect (DSL v0.6) in a
/// deterministic content order, resolving each anchor to a cell and rooting it at
/// its firing step. An effect whose anchor does not resolve to a point is skipped
/// here (validation guarantees the anchor exists; a pool anchor that fails to
/// resolve at plan time simply carries no proof/emission).
pub(super) fn collect_v06_effects(
    campaign: &Campaign,
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
    obj_step: &BTreeMap<String, usize>,
) -> (Vec<CheckpointPlan>, Vec<StealthBeat>) {
    let mut c = V06Collector {
        anchors,
        trigger: None,
        checkpoints: Vec::new(),
        stealth: Vec::new(),
        stealth_ends: Vec::new(),
    };

    // Stage 5 — quest effects (on_objective_complete, then on_complete).
    for q in &campaign.quests.content.quests {
        for (obj_id, effs) in &q.on_objective_complete {
            let step = obj_step.get(obj_id.as_str()).copied().unwrap_or(0);
            for eff in effs {
                c.handle(eff, step);
            }
        }
        let done_step = quest_complete_step(q, obj_step);
        for eff in &q.on_complete {
            c.handle(eff, done_step);
        }
    }

    // Stage 5 — environment triggers. Fire step 0 here — a trigger fires on the
    // party's own act, not at a beat — and the checkpoint remembers which
    // trigger set it, so the no-stranding proof can re-root it at the earliest
    // configuration in which the party can reach the trigger (spec-0093 §6.2).
    for t in &campaign.quests.content.triggers {
        c.trigger = Some(t.id.as_str().to_string());
        for eff in &t.effects {
            c.handle(eff, 0);
        }
        c.trigger = None;
    }

    // Stage 6 — dialogue `set-checkpoint` (rooted at the NPC's talk-to beat).
    for tree in &campaign.dialogue.content.dialogues {
        let step = dialogue_fire_step(campaign, tree.npc.as_str(), obj_step);
        for node in &tree.nodes {
            for opt in &node.options {
                for eff in &opt.effects {
                    if let Some((anchor, on_respawn)) = eff.set_checkpoint() {
                        c.push_checkpoint(anchor.as_str(), on_respawn, step, false, None);
                    }
                }
            }
        }
    }

    close_stealth_windows(&mut c.stealth, &c.stealth_ends);
    (c.checkpoints, c.stealth)
}

/// Accumulates v0.6 checkpoints / stealth beats in content order while resolving
/// their anchors (a struct so the collection borrows stay simple).
struct V06Collector<'a> {
    anchors: &'a BTreeMap<(String, String), ResolvedAnchor>,
    /// The trigger whose bundle is being walked, so a checkpoint it sets
    /// records its source (spec-0093 §6.2); `None` outside a trigger's bundle.
    trigger: Option<String>,
    checkpoints: Vec<CheckpointPlan>,
    stealth: Vec<StealthBeat>,
    /// Firing steps of every `end-stealth`, in content order — closes each beat's
    /// active window ([`StealthBeat::end_step`]).
    stealth_ends: Vec<usize>,
}

impl V06Collector<'_> {
    fn push_checkpoint(
        &mut self,
        anchor: &str,
        on_respawn: &[QuestEffect],
        fire_step: usize,
        rest: bool,
        labels: Option<delvewright_dsl::BonfireLabels<'_>>,
    ) {
        if let Some(pos) = point_any(self.anchors, anchor) {
            let labels = labels.unwrap_or(delvewright_dsl::BonfireLabels {
                prompt: None,
                rest_label: None,
                save_label: None,
                rest_tooltip: None,
                save_tooltip: None,
            });
            self.checkpoints.push(CheckpointPlan {
                index: self.checkpoints.len(),
                anchor: anchor.to_string(),
                pos,
                on_respawn: on_respawn.to_vec(),
                fire_step,
                trigger: self.trigger.clone(),
                rest,
                // Authored strings are ordinary inventoried campaign text; an
                // unauthored one takes the compiler's chrome default in its tagged
                // form, which `emit` rebinds to the build's language.
                prompt: labels
                    .prompt
                    .map(str::to_string)
                    .unwrap_or_else(|| delvewright_dsl::chrome::BONFIRE_TITLE.tagged()),
                rest_label: labels
                    .rest_label
                    .map(str::to_string)
                    .unwrap_or_else(|| delvewright_dsl::chrome::BONFIRE_REST.tagged()),
                save_label: labels
                    .save_label
                    .map(str::to_string)
                    .unwrap_or_else(|| delvewright_dsl::chrome::BONFIRE_SAVE.tagged()),
                rest_tooltip: labels.rest_tooltip.map(str::to_string),
                save_tooltip: labels.save_tooltip.map(str::to_string),
            });
        }
    }

    fn push_stealth(
        &mut self,
        zones: &[delvewright_dsl::StealthZone],
        on_caught: &[QuestEffect],
        grace_ticks: u32,
        fire_step: usize,
    ) {
        let resolved: Vec<(String, [i32; 3], [u32; 3])> = zones
            .iter()
            .filter_map(|z| {
                point_any(self.anchors, z.anchor.as_str())
                    .map(|p| (z.anchor.as_str().to_string(), p, z.extent))
            })
            .collect();
        if resolved.len() == zones.len() {
            self.stealth.push(StealthBeat {
                index: self.stealth.len() + 1,
                zones: resolved,
                on_caught: on_caught.to_vec(),
                grace_ticks,
                fire_step,
                end_step: None, // filled in by `close_stealth_windows`
            });
        }
    }

    fn handle(&mut self, eff: &QuestEffect, fire_step: usize) {
        if let Some((anchor, on_respawn)) = eff.set_checkpoint() {
            self.push_checkpoint(anchor.as_str(), on_respawn, fire_step, false, None);
        } else if let Some((anchor, on_rest)) = eff.bonfire() {
            // A bonfire IS a checkpoint (spec-0016 §1) — it inherits DW0315 /
            // DW0316 by being collected here. It is rooted at the arming step,
            // the earliest beat a rest can happen.
            self.push_checkpoint(
                anchor.as_str(),
                on_rest,
                fire_step,
                true,
                eff.bonfire_labels(),
            );
        } else if let Some((zones, on_caught, grace)) = eff.begin_stealth() {
            self.push_stealth(zones, on_caught, grace, fire_step);
        } else if matches!(&eff.verb, Verb::EndStealth) {
            self.stealth_ends.push(fire_step);
        }
        // Descend into every nested effect list (`sequence` steps, `on_respawn`,
        // `on_caught`, `on_arrive`): a `set-checkpoint`/`begin-stealth` nested in a
        // `sequence` step is a real checkpoint/beat, fired at the same critical-path
        // step, and must be collected — else its content-ordered index is never
        // registered and `emit_set_checkpoint` silently mis-binds `#cp` to 0.
        for list in eff.nested_effect_lists() {
            for inner in list {
                self.handle(inner, fire_step);
            }
        }
    }
}

pub(super) fn point_of(
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
    area: &str,
    anchor: &str,
) -> Result<[i32; 3], PlanError> {
    match anchors.get(&(area.to_string(), anchor.to_string())) {
        Some(ResolvedAnchor::Point { pos, .. }) => Ok(*pos),
        Some(ResolvedAnchor::Gate { from, .. }) => Ok(*from),
        None => Err(PlanError::new(
            DW_BUILD,
            format!(
                "anchor `{anchor}` in area `{area}` did not resolve to a world position at build \
                 time — if the campaign references an anchor no bound prefab/pool provides, \
                 `DW0142`/`DW0302` should have named it; reaching here means the resolver and \
                 validator disagree, a compiler bug — stop and escalate"
            ),
        )),
    }
}
