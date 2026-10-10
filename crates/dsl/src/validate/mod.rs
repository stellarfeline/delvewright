//! Campaign validation: all six rule groups from spec-0001.
//!
//! [`validate_campaign`] uses the vendored v0 registries; the compiler injects
//! full registries via [`validate_campaign_with`].

use crate::Verb;
use std::collections::{BTreeMap, BTreeSet};

use crate::QuestEffect;
use crate::diagnostic::{Diagnostic, codes};
use crate::envelope::Campaign;
use crate::metrics::Metrics;
use crate::registry::{
    AnchorRegistry, BlockRegistry, EntityRegistry, ItemBackedBlockRegistry, ItemRegistry,
    VendoredAnchorRegistry, VendoredEffectRegistry, VendoredEntityRegistry, VendoredItemRegistry,
};

/// Validate a campaign against all spec-0001 rules using the vendored v0
/// registries (subset item + entity registries + hello-world anchor metadata).
pub fn validate_campaign(c: &Campaign) -> Vec<Diagnostic> {
    let items = VendoredItemRegistry::v1_21_11();
    let entities = VendoredEntityRegistry::v1_21_11();
    let anchors = VendoredAnchorRegistry::hello_world();
    validate_campaign_with(c, &items, &anchors, &entities)
}

/// Validate a campaign with caller-supplied registries. The `entities` registry
/// (DSL v0.3) validates stage-5 wave mobs; the compiler injects the full
/// 1.21.11 item/entity registries and real prefab metadata.
pub fn validate_campaign_with(
    c: &Campaign,
    items: &dyn ItemRegistry,
    anchors: &dyn AnchorRegistry,
    entities: &dyn EntityRegistry,
) -> Vec<Diagnostic> {
    let mut d: Vec<Diagnostic> = Vec::new();

    crate::envelope::envelope_checks(c, &mut d);
    // Id syntax (`DW0110`), each collection in turn.
    crate::world::world_id_syntax(c, &mut d);
    crate::npc::npc_id_syntax(c, &mut d);
    crate::class::class_id_syntax(c, &mut d);
    crate::quest_plan::plan_id_syntax(c, &mut d);
    crate::quest::check::quest_id_syntax(c, &mut d);
    crate::actor::actor_id_syntax(c, &mut d);
    crate::assembly::assembly_id_syntax(c, &mut d);
    crate::dialogue::dialogue_id_syntax(c, &mut d);
    // Id uniqueness (`DW0111`), each namespace in turn.
    crate::world::world_id_uniqueness(c, &mut d);
    crate::npc::npc_id_uniqueness(c, &mut d);
    crate::class::class_id_uniqueness(c, &mut d);
    crate::quest_plan::plan_id_uniqueness(c, &mut d);
    crate::quest::check::quest_id_uniqueness(c, &mut d);
    crate::actor::actor_id_uniqueness(c, &mut d);
    crate::assembly::assembly_id_uniqueness(c, &mut d);
    crate::dialogue::dialogue_id_uniqueness(c, &mut d);
    // Dangling references (`DW0112`) outside the dialogue graph and the finale.
    crate::npc::npc_dangling_refs(c, &mut d);
    crate::world::atmosphere_dangling_refs(c, &mut d);
    crate::quest_plan::plan_dangling_refs(c, &mut d);
    crate::quest::check::quest_dangling_refs(c, &mut d);
    crate::dialogue::dialogue_graph_checks(c, &mut d);
    crate::quest_plan::plan_checks(c, &mut d);
    crate::quest::check::after_ordering(c, &mut d);
    crate::trigger::press_answer_checks(c, &mut d);
    crate::trigger::press_obligation_checks(c, &mut d);
    crate::world::horizon_param_checks(c, &mut d);
    crate::world::world_checks(c, &mut d);
    crate::world::lighting_range_checks(c, &mut d);
    // spec-0031: the runtime-state surface. Every loop inside is empty for a
    // campaign that declares no datum and no comparison.
    crate::state::state_checks(c, &mut d);
    // A gate that contradicts itself can never open (`DW0847`). Unconditional
    // and over the whole closed consumer set — an ungated site contributes no
    // terms and cannot contradict.
    crate::state::gate_contradiction_checks(c, &mut d);
    // `DW0527`: a gate read after a bundle's own conditional write. Over every
    // effect root of every campaign — not inside `economy_checks`, whose early
    // return on a campaign with no stakes and no shops would skip it.
    crate::state::read_after_write_checks(c, &mut d);
    crate::lethal::lethal_stage_checks(c, &mut d);
    // spec-0031: the status-effect verbs. Every walk inside is empty for a
    // campaign that declares neither verb. The status-effect registry is the
    // fixed vanilla list wave-mob effects are validated against, so no injected
    // registry is needed.
    crate::quest::check::status_effect_checks(c, &VendoredEffectRegistry::v1_21_11(), &mut d);
    // spec-0032: the trade and recovery-stake surface. Every loop inside is
    // empty for a campaign that declares neither.
    crate::economy::economy_checks(c, &mut d);
    // spec-0034: the per-body traversal declaration. The walk is empty for a
    // campaign that declares none.
    crate::body::body_traversal_checks(c, &mut d);
    // spec-0097: which overlay layers a skinned body hides. Empty for a campaign
    // that hides none.
    crate::npc::skin_layer_checks(c, &mut d);
    crate::world::prefab_binding(c, anchors, &mut d);
    // Anchors and items: an NPC's station, a quest's objective and effect
    // anchors (resolved against its planned area), a trigger's effect anchors
    // (against every area: a trigger is global), and the class kits. A site-plan
    // campaign's anchors are DERIVED from the graph and the plan, so the set is
    // knowable here exactly as a prefab's is (see [`AnchorProviders`]).
    let providers = AnchorProviders::build(c, anchors);
    crate::npc::npc_anchor_checks(c, &providers, &mut d);
    crate::quest::check::quest_anchor_checks(c, &providers, &mut d);
    crate::trigger::trigger_anchor_checks(c, &providers, &mut d);
    crate::class::kit_item_checks(c, items, &mut d);
    crate::quest::check::cross_stage(c, &mut d);
    crate::dialogue::npc_tree_checks(c, &mut d);
    // `DW0849`: an item gate no class can bring. The walk is empty for a
    // campaign with no `requires_item`; the rule judges a contradiction between
    // two authored documents (a quest's item gate against the class kits).
    // Bound HERE rather than to a step someone runs, because this is the
    // function every `delvec` subcommand's validation stage calls.
    crate::class::item_gate_class_checks(c, &mut d);
    // Waves, and the references a quest and a trigger's effects make to waves,
    // flags, items and anchors.
    crate::wave::wave_decl_checks(c, entities, &mut d);
    crate::quest::check::quest_reference_checks(c, items, anchors, &mut d);
    crate::trigger::trigger_effect_flag_checks(c, &mut d);
    // Skins, wave-mob effects, props, set-block, triggers, dialogue flags and the
    // NPC lifecycle. The status-effect registry is a fixed vanilla list; the block registry derives
    // from the item registry (see [`ItemBackedBlockRegistry`]), so no new
    // caller-supplied registry is needed.
    let blocks = ItemBackedBlockRegistry::new(items);
    let effects_reg = VendoredEffectRegistry::v1_21_11();
    crate::npc::npc_skin_checks(c, &mut d);
    crate::wave::mob_effect_checks(c, &effects_reg, &mut d);
    crate::trigger::trigger_prop_checks(c, &blocks, &mut d);
    crate::quest::check::quest_prop_checks(c, &blocks, &mut d);
    // Declared flags across quest / dialogue / trigger `set-flag` effects and
    // trap and timed-gate disarms (a disarm's `sets_flag` is a first-class
    // declared flag other objectives/triggers may gate on).
    let flags = collect_declared_flags(c);
    crate::trigger::trigger_decl_checks(c, anchors, &blocks, &flags, &mut d);
    crate::dialogue::dialogue_flag_checks(c, &flags, &mut d);
    let npc_ids: BTreeSet<&str> = c.npcs.content.npcs.iter().map(|n| n.id.as_str()).collect();
    crate::npc::despawned_ref_check(c, &npc_ids, &mut d);
    crate::npc::deferred_npc_checks(c, &npc_ids, &mut d);
    // Cutscene shot styles, scripted actors and their staging effects
    // (spec-0014), wave-mob and actor equipment, declared drops and the rest
    // re-seat. Actor entity ids validate against the injected entity registry
    // and equipment slots against the item registry.
    crate::cutscene::cutscene_style_checks(c, &mut d);
    crate::actor::actor_checks(c, anchors, entities, &mut d);
    crate::quest::check::carrier_one_checks(c, &mut d);
    crate::wave::wave_equipment_checks(c, items, &mut d);
    crate::actor::actor_equipment_checks(c, items, &mut d);
    // spec-0067: every piece is put where the pinned game shows it on the body
    // that wears it (`DW0898`).
    crate::equipment::fit_checks(c, items, &mut d);
    // Declared drops — the subset an elite/boss leaves behind.
    crate::wave::check_drops(c, &c.quests.content, items, &mut d);
    crate::wave::rest_reseat_checks(c, &mut d);
    crate::class::bonfire_flask_checks(c, declares_bonfire(c), &mut d);
    // spec-0082: assemblies, their rigs and the verbs and trigger that name
    // them. Every loop inside is empty for a campaign that declares none.
    crate::assembly::assembly_checks(c, anchors, &mut d);
    crate::trap::trap_checks(c, items, entities, anchors, &mut d);
    crate::shortcut::shortcut_checks(c, anchors, &mut d);
    crate::ambush::ambush_checks(c, &mut d);
    crate::timed_gate::timed_gate_checks(c, anchors, &mut d);
    crate::loot::loot_checks(c, items, anchors, &mut d);
    crate::quest::check::give_item_enchantment_checks(c, &mut d);
    crate::wave::lane_checks(c, anchors, &mut d);
    crate::wave::difficulty_checks(c, &mut d);
    crate::firework::firework_checks(c, &mut d);
    // spec-0085: the perception bundle — a particle's id, the envelope's audience
    // on a party fact, and a sight grant that ends under a camera.
    crate::perception::perception_checks(c, &mut d);
    // spec-0073: a fight's health bar. The walk is over every wave and actor;
    // a campaign that declares no bar and bills no fight `boss` gets nothing.
    crate::healthbar::health_bar_checks(c, &mut d);
    // Stage 7 (spec-0017): the map-editor edit script. Structural
    // checks only — frame/region *resolution* happens at build time against the
    // solved layout (the compiler's `DW0323`).
    if c.world_edits.is_some() {
        let blocks = ItemBackedBlockRegistry::new(items);
        crate::world_edits::world_edits_checks(c, &blocks, &mut d);
    }
    // spec-0049: the map-pipeline documents. Bound to the EVENT it guards
    // rather than to a step someone runs — a campaign directory holding a
    // `layout-graph.json` has no path to a verdict that does not come through
    // here, because this is the function every `delvec` subcommand's validation
    // stage calls. The reads ledger is what gives `DW0813` its document-side
    // binding: every building metric these checks rest a verdict on records
    // that it did.
    if c.geometry_brief.is_some() || c.layout_graph.is_some() || c.site_plan.is_some() {
        // ONE run-scoped ledger across stages 2, 3 and 4. That is what closes
        // the residual `dsl::metrics` names in its own module docs: a check
        // cannot rest a verdict on an uncalibrated standard without the notice
        // seeing it, because there is no second ledger for a read to go into.
        let mut reads = crate::metrics::Reads::new();
        crate::layout::check(c, &mut reads, &mut d);
        crate::siteplan::check(c, &mut reads, &mut d);
        // The notice names the newest document present, because that is the one
        // whose checks read the most of the provisional half.
        let stage = if c.site_plan.is_some() {
            "site-plan"
        } else if c.layout_graph.is_some() {
            "layout-graph"
        } else {
            "geometry-brief"
        };
        if let Some(notice) = Metrics::table().notice(&reads, stage) {
            d.push(notice);
        }
    }
    // spec-0025: the declared story forks and the per-node
    // `happening`. Structural only — the branch proofs themselves (`DW048x`) are
    // compiler-tier, because they need the branch/flag flow model.
    crate::quest_plan::branch_point_checks(c, &mut d);
    // A `collect` may adopt a prefab container, which puts a
    // second positional filler on the same anchor a `loot` entry can name.
    crate::loot::collect_container_claim_checks(c, &mut d);
    crate::quest_plan::happening_subject_checks(c, &mut d);
    // spec-0016 §1: what is really in the
    // flask. The status-effect registry is the same fixed vanilla list,
    // and the potion registry is complete in-crate, so no injected registry is
    // needed.
    let effects_reg = VendoredEffectRegistry::v1_21_11();
    crate::class::kit_potion_checks(c, &effects_reg, &mut d);
    // spec-0031: the stage-5 lethal volumes. Structural only — the
    // completability half (`DW0510` the forced route, `DW0511` the respawn seat)
    // is compiler-tier, because it needs the solved layout.
    crate::lethal::lethal_volume_checks(c, anchors, &mut d);
    crate::r#loop::loop_checks(c, anchors, &mut d);
    // spec-0032: a shop stands on a prefab anchor, and an anchor
    // no bound prefab provides is the same defect a lethal volume's is.
    crate::economy::shop_anchor_checks(c, anchors, &mut d);
    // spec-0071 §2: a price has no field, so the copies of it — the gate term,
    // the charge, the ceiling on the arm that answers below it — are compared
    // here. Quantified over every effect list that charges a datum, never over
    // shops.
    crate::economy::purchase_checks(c, &mut d);
    // spec-0074 §8.1: an `on_kill` bundle a credited kill can never reach
    // (`DW0913`), and an empty one (`DW0100`). The pair that needs the rest
    // points (`DW0914`/`DW0915`) is compiler-side.
    crate::onkill::on_kill_checks(c, &mut d);
    // spec-0061: the design record's own document-level refusals — an empty
    // `references`, a name that is not a path under `design/`, two rows for one
    // picture. The comparison against the world's reachable skies (`DW0890`) is
    // compiler-tier, because it reads the same reachable-state scan `DW0210`
    // and `DW0496` do and needs the campaign's `design/` directory beside it.
    // No-op for a campaign that ships no `design.json`.
    crate::design::check(c, &mut d);
    // spec-0081: the shape of every celestial time the documents state
    // (`DW0931`). Empty for a campaign of keywords.
    crate::celestial::check(c, &mut d);

    d
}

// ---------------------------------------------------------------------------
// What anchors does this campaign have? — ONE authority
// ---------------------------------------------------------------------------

/// Every anchor name this campaign's areas provide, and how much of that answer
/// is known at validation time.
///
/// **This is the only place the question is asked.** Eleven checks used to walk
/// `world.areas` and call [`AnchorRegistry::anchors_for`] themselves — the shape
/// `CLAUDE.md` names as *a hand-rolled walk enumerating three of five effect
/// roots: a defect of expressibility, not of care*. It cost exactly what that
/// shape always costs. When a site plan became a second placement authority
/// (spec-0049 §5.2), the derivation began synthesizing a campaign's whole anchor
/// vocabulary — a `node-` per place, a `seam-` per barred way, an `unlock-` on
/// the openable side of a one-sided one — and **one** of the eleven walks was
/// taught to ask [`crate::siteplan::synthesized_anchors`] about it. The other ten
/// went on enumerating prefabs a derived world does not have, so every stage-5
/// verb but the ones that walk went unauthorable on a derived map: a `shortcut`
/// naming the very `unlock-` anchor the derivation places for it was refused as
/// an invented name, and so were a trap, a shop, a lane, a loot chest, a lethal
/// volume, a timed gate, a cutscene camera and an actor.
///
/// Nothing was red, because a check that resolves against a smaller world than
/// the campaign has refuses *content*, not itself.
///
/// Two properties every consumer needs and neither should re-derive:
///
/// * **Leniency.** A pool area's prefab is chosen by the compiler, so an anchor
///   name it might provide cannot be refused here. [`Self::resolvable`] is
///   therefore true for every name once any pool area exists — the long-standing
///   policy, stated once instead of ten times.
/// * **Completeness.** [`Self::all_areas_known`] is true only when every area
///   contributed a set: no pool, no prefab the registry has never heard of. The
///   checks with no owning area to resolve against (a global trigger's effects, a
///   cutscene camera that legitimately flies between areas) need this rather than
///   leniency.
pub(crate) struct AnchorProviders {
    /// area id → the anchor names that area provides.
    per_area: BTreeMap<String, BTreeSet<String>>,
    /// **What SHAPE each derived name is**, for a site-plan campaign
    /// (spec-0052 §4) — taken from
    /// [`crate::siteplan::synthesized_anchor_kinds`], the same authority the
    /// names themselves come from.
    ///
    /// Empty for a prefab campaign, and that emptiness is load-bearing rather
    /// than a gap: a prefab's shape lives in its metadata, which
    /// [`AnchorRegistry`] deliberately does not carry — it answers names only.
    /// So [`Self::wrong_kind`] returns `None` there and the compiler keeps
    /// answering the question at placement time, exactly as it always has. This
    /// check is about names the ENGINE derives, which are knowable here.
    kinds: BTreeMap<String, crate::layout::StationKind>,
    /// The union of every known area's set.
    union: BTreeSet<String>,
    /// **This campaign's answer is not knowable at this tier**, so no name is
    /// refused for being outside the set. Two causes, one consequence: some area
    /// binds a pool whose draw the compiler makes later, or the campaign hands
    /// its space to a site plan whose `layout-graph.json` is absent — see
    /// [`crate::placement::anchor_vocabulary_unknowable`], which is where that
    /// second question is asked and the only place it is answered.
    deferred: bool,
    /// Every area contributed a set — the union is the whole truth.
    all_areas_known: bool,
    /// What this campaign hands its space to. It decides which names an author
    /// may write, and therefore what a refusal is allowed to prescribe.
    placement: crate::placement::Placement,
}

impl AnchorProviders {
    /// Ask the campaign, once.
    pub(crate) fn build(c: &Campaign, anchors: &dyn AnchorRegistry) -> Self {
        let mut per_area: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut deferred = false;
        // A site-plan campaign has no prefab to ask, and it does not need one:
        // its anchors are DERIVED from the graph and the plan, so the set is
        // knowable here exactly as a prefab's is — and it is the same function
        // the derivation itself places them by, so a name that resolves here
        // cannot fail to exist in the built world.
        let mut declared_areas = c.world.content.areas.len();
        let mut kinds: BTreeMap<String, crate::layout::StationKind> = BTreeMap::new();
        if c.site_plan.is_some() {
            declared_areas += 1;
            // A derivation with no graph to read names NOTHING, and an empty set
            // here would make every anchor reference in the campaign a refusal —
            // of names that are correct, with a remedy that cannot be taken. The
            // set is unknown, not empty, and that is exactly the pool area's
            // situation, so it takes the pool area's path: contribute no set and
            // let the whole campaign defer.
            if crate::placement::anchor_vocabulary_unknowable(c) {
                deferred = true;
            } else {
                kinds = crate::siteplan::synthesized_anchor_kinds(c);
                per_area.insert(
                    crate::siteplan::SITE_AREA.to_string(),
                    kinds.keys().cloned().collect(),
                );
            }
        }
        for a in &c.world.content.areas {
            if let Some(prefab) = &a.prefab {
                if let Some(set) = anchors.anchors_for(prefab) {
                    per_area.insert(a.id.as_str().to_string(), set.clone());
                }
            } else if a.prefab_pool.is_some() {
                deferred = true;
            }
        }
        let union: BTreeSet<String> = per_area.values().flatten().cloned().collect();
        let all_areas_known = declared_areas == per_area.len();
        Self {
            per_area,
            kinds,
            union,
            deferred,
            all_areas_known,
            placement: crate::placement::Placement::of(c),
        }
    }

    /// **The shape this campaign declared for `name`, when it disagrees with what
    /// the reference site demands** (spec-0052 §7.3); `None` when they agree,
    /// when the name has no declared shape, or when this campaign has no site
    /// plan.
    ///
    /// Judged from the DECLARATION, at validation, with zero pieces bound —
    /// which is the whole point of putting the kind on the station rather than
    /// discovering it when a piece arrives. When a piece does arrive, the same
    /// kind is demanded of the piece anchor it binds to (spec-0052 §6), so the
    /// two readings cannot drift.
    pub(crate) fn wrong_kind(
        &self,
        name: &str,
        demands: crate::layout::StationKind,
    ) -> Option<crate::layout::StationKind> {
        let declared = *self.kinds.get(name)?;
        (declared != demands).then_some(declared)
    }

    /// The anchor set `area` provides, or `None` when that area's provider is not
    /// known here (a pool the compiler resolves later, or an unknown prefab).
    pub(crate) fn for_area(&self, area: &str) -> Option<&BTreeSet<String>> {
        self.per_area.get(area)
    }

    /// Whether `name` may be referenced. Lenient by design: a pool area defers the
    /// answer to the compiler, so nothing is refused while one exists.
    pub(crate) fn resolvable(&self, name: &str) -> bool {
        self.deferred || self.union.contains(name)
    }

    /// Every area contributed a set, so [`Self::union`] is the whole truth.
    pub(crate) fn all_areas_known(&self) -> bool {
        self.all_areas_known
    }

    /// Every anchor name any known area provides.
    pub(crate) fn union(&self) -> &BTreeSet<String> {
        &self.union
    }

    /// **What to write instead**, in the vocabulary THIS campaign has.
    ///
    /// One authority for every anchor refusal in this file. `prefab` is the
    /// sentence the site has always printed, returned unchanged where prefabs
    /// place the world and dropped where they do not: a derived map has no
    /// prefab metadata for an author to go and read, so a refusal sending them
    /// to read some was prescribing an act the campaign cannot perform. See
    /// [`crate::placement`].
    pub(crate) fn anchor_remedy<'a>(&self, prefab: &'a str) -> &'a str {
        self.placement.anchor_remedy(prefab)
    }
}

/// **`DW0871` for one reference site** (spec-0052 §7.3).
///
/// One builder rather than a message per call site, for the reason the live
/// command rule gives: a correct rule living inside one caller gives the next
/// two nothing to reuse, and the wording an author meets must not depend on
/// which verb happened to catch them.
///
/// The remedy it names is **reachable**, and that is checked rather than
/// assumed: it tells the author to change the station's `kind` in the layout
/// graph, or to name a station of the demanded kind. Both are things the graph
/// can say — `kind` is a required field with two writable values, and nothing
/// refuses a node for declaring either.
pub(crate) fn station_kind_diag(
    providers: &AnchorProviders,
    name: &str,
    demands: impl Into<Option<crate::layout::StationKind>>,
    what: &str,
    stage: &'static str,
    path: String,
) -> Option<Diagnostic> {
    // `None` is a site that names a LOCATION rather than demanding a shape, and
    // it is the majority: see `QuestEffect::anchor_refs` for where the line is
    // drawn and for the gallery content an earlier, stricter draft refused.
    let demands = demands.into()?;
    let declared = providers.wrong_kind(name, demands)?;
    Some(Diagnostic::error(
        crate::layout::DW_STATION_KIND,
        stage,
        path,
        format!(
            "`{name}` is declared as a {got} station, and {what} demands a {want}. A station's \
             `kind` is its SHAPE: a `point` is a cell a body is put at — a seat, a subject, an \
             affordance, the centre of an anchor-centred volume — and a `gate` is a region that \
             seals and clears, which is what `open-gate`, `close-gate`, a `shortcut` and a \
             `timed-gate` address. This is read from the layout graph's declaration, so it is \
             answered before any piece is bound and the same shape is demanded of the piece \
             anchor when one is. Either change this station's `kind` to `{want}` in the layout \
             graph, or name a station that is already one.",
            got = declared.word(),
            want = demands.word(),
        ),
    ))
}

// ---------------------------------------------------------------------------
// Shared helpers: what more than one object module asks of the campaign
// ---------------------------------------------------------------------------

/// Does the campaign declare a `bonfire` — a rest point that re-seats fights
/// (spec-0016 §1)? Read at the roots the compiler collects rest points from: a
/// quest's bundles and an environment trigger's effects, at any nesting depth.
/// The match over the site is exhaustive, so a new root answers here. `DW0370`
/// asks it, and so does the compiler's `fight_comes_back` where no plan exists
/// yet (`DW0914`/`DW0915`).
pub fn declares_bonfire(c: &Campaign) -> bool {
    collected_effect_any(c, |eff| eff.bonfire().is_some())
}

/// Does the campaign declare any checkpoint a fallen player respawns at — a
/// `set-checkpoint` or a `bonfire` — at a root the compiler collects checkpoints
/// from: the roots [`declares_bonfire`] reads, plus a dialogue option's
/// `set-checkpoint`. The reading `DW0925` needs.
pub fn declares_checkpoint(c: &Campaign) -> bool {
    collected_effect_any(c, |eff| {
        eff.bonfire().is_some() || eff.set_checkpoint().is_some()
    }) || c.dialogue.content.dialogues.iter().any(|tree| {
        tree.nodes.iter().any(|node| {
            node.options
                .iter()
                .any(|opt| opt.effects.iter().any(|e| e.set_checkpoint().is_some()))
        })
    })
}

/// Does any quest-effect at a root the compiler collects checkpoints and rest
/// points from (a quest's bundles and an environment trigger's effects, at any
/// nesting depth) satisfy `pred`? The match over the site is exhaustive, so a
/// new root answers here.
fn collected_effect_any(c: &Campaign, pred: impl Fn(&QuestEffect) -> bool) -> bool {
    use crate::EffectSite;
    let mut found = false;
    crate::for_each_campaign_effect(c, &mut |_, site, eff| {
        let collected = match site {
            EffectSite::Objective { .. }
            | EffectSite::QuestComplete { .. }
            | EffectSite::Trigger { .. } => true,
            EffectSite::Trap { .. }
            | EffectSite::DialogueRespawn { .. }
            | EffectSite::ShortcutUnlock { .. }
            | EffectSite::ShopOffer { .. }
            | EffectSite::OnDeath
            | EffectSite::OnKill { .. }
            | EffectSite::AssemblyLand { .. }
            | EffectSite::LoopCross { .. } => false,
        };
        found |= collected && pred(eff);
    });
    found
}

/// Visit every quest effect **and every transitively-nested effect** (a `sequence`
/// step, an `on_respawn`/`on_caught`/`on_arrive` bundle) with its relative path
/// fragment, threading the JSON-pointer path through
/// [`QuestEffect::nested_effect_lists_labeled`] (`steps/<step>/effects`,
/// `on_respawn`, …). The deep counterpart of [`for_each_effect`] — the effect-ref
/// consumer checks (unknown wave / item / block / npc references) use this so a bad
/// ref nested in a timeline is caught, not shipped unvalidated (mirroring how the
/// flag/wave *producer* scans and emission already descend). Top-level paths are
/// unchanged, so a nesting-free campaign is validated identically.
pub(crate) fn for_each_effect_deep(q: &crate::Quest, mut f: impl FnMut(String, &QuestEffect)) {
    fn descend(path: String, eff: &QuestEffect, f: &mut dyn FnMut(String, &QuestEffect)) {
        f(path.clone(), eff);
        for (pseg, _kseg, list) in eff.nested_effect_lists_labeled() {
            for (j, inner) in list.iter().enumerate() {
                descend(format!("{path}/{pseg}/{j}"), inner, f);
            }
        }
    }
    for (key, effs) in &q.on_objective_complete {
        for (m, eff) in effs.iter().enumerate() {
            descend(format!("on_objective_complete/{key}/{m}"), eff, &mut f);
        }
    }
    for (m, eff) in q.on_complete.iter().enumerate() {
        descend(format!("on_complete/{m}"), eff, &mut f);
    }
}

/// Visit an environment trigger's effects **and every transitively-nested effect**
/// with a relative path fragment (`effects/<m>`, then nested segments) — the
/// trigger analogue of [`for_each_effect_deep`].
pub(crate) fn for_each_trigger_effect_deep(
    t: &crate::EnvTrigger,
    mut f: impl FnMut(String, &QuestEffect),
) {
    fn descend(path: String, eff: &QuestEffect, f: &mut dyn FnMut(String, &QuestEffect)) {
        f(path.clone(), eff);
        for (pseg, _kseg, list) in eff.nested_effect_lists_labeled() {
            for (j, inner) in list.iter().enumerate() {
                descend(format!("{path}/{pseg}/{j}"), inner, f);
            }
        }
    }
    for (m, eff) in t.effects.iter().enumerate() {
        descend(format!("effects/{m}"), eff, &mut f);
    }
}

/// The trap-payload analogue of [`for_each_trigger_effect_deep`] (spec-0022):
/// visit every effect of `t.payload`, descending into nested effect lists, with
/// the JSON pointer relative to the trap. A trap payload is an effect ROOT — the
/// same standing as a quest bundle or a trigger bundle — so every consumer scan
/// that walks the other two walks this one too. Empty for a pure spec-0011
/// redstone trap.
pub(crate) fn for_each_trap_payload_deep(t: &crate::Trap, mut f: impl FnMut(String, &QuestEffect)) {
    fn descend(path: String, eff: &QuestEffect, f: &mut dyn FnMut(String, &QuestEffect)) {
        f(path.clone(), eff);
        for (pseg, _kseg, list) in eff.nested_effect_lists_labeled() {
            for (j, inner) in list.iter().enumerate() {
                descend(format!("{path}/{pseg}/{j}"), inner, f);
            }
        }
    }
    for (m, eff) in t.payload.iter().enumerate() {
        descend(format!("payload/{m}"), eff, &mut f);
    }
}

/// Split a block field into its base id and (optional) blockstate suffix,
/// validating the suffix's syntax (DSL v0.6). Returns the base id to
/// check against the block registry; `Err(reason)` when a `[...]` suffix is
/// present but malformed (unbalanced brackets, empty, or a token that is not a
/// lowercase `key=value`). A well-formed state string is passed through verbatim
/// to `setblock` — vanilla validates the property names/values against the
/// block's own state definition, so the compiler only guards the surface syntax.
pub(crate) fn split_blockstate(block: &str) -> Result<&str, String> {
    let Some(open) = block.find('[') else {
        return Ok(block);
    };
    let rest = &block[open..];
    if !rest.ends_with(']') {
        return Err(format!(
            "malformed blockstate in `{block}` — the `[...]` suffix must close with `]` \
             (e.g. `minecraft:grindstone[face=floor]`)"
        ));
    }
    let inner = &rest[1..rest.len() - 1];
    if inner.trim().is_empty() {
        return Err(format!(
            "malformed blockstate in `{block}` — the `[...]` suffix is empty; drop the brackets or \
             add a `key=value` property"
        ));
    }
    let is_token = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
    };
    for prop in inner.split(',') {
        let mut kv = prop.splitn(2, '=');
        let key = kv.next().unwrap_or("").trim();
        match kv.next().map(str::trim) {
            Some(val) if is_token(key) && is_token(val) => {}
            _ => {
                return Err(format!(
                    "malformed blockstate in `{block}` — each property must be `key=value` with \
                     lowercase `[a-z0-9_]` tokens (e.g. `face=floor`)"
                ));
            }
        }
    }
    Ok(&block[..open])
}

/// **The one rest rule at the entry points** (spec-0100 §4.1): a block state an
/// author places — a `set-block`/`fill-region` block, an `interact`/trigger
/// prop, a world-edit recipe or scatter item — is judged by
/// [`crate::blocks::sculk_rest`] where it is typed, so the refusal names the
/// document path (`DW0998`, `DW0999`). The build judges the assembled world
/// again with the same function, for the pieces no document typed.
pub(crate) fn check_sculk_rest(block: &str, stage: &str, path: String, d: &mut Vec<Diagnostic>) {
    if let Err(fault) = crate::blocks::sculk_rest(block, None) {
        d.push(Diagnostic::error(
            fault.code(),
            stage,
            path.clone(),
            format!("{fault} (entered at `{path}`)"),
        ));
    }
}

/// Validate a block field (interact prop / set-block) allowing an optional
/// verbatim blockstate suffix (DSL v0.6). The base id must be in the block
/// registry (`DW0193`); a malformed `[...]` suffix reuses `DW0193` with a clear
/// message. Always a quests-stage diagnostic.
pub(crate) fn check_block_field(
    blocks: &dyn BlockRegistry,
    block: &str,
    path: String,
    kind: &str,
    example: &str,
    d: &mut Vec<Diagnostic>,
) {
    match split_blockstate(block) {
        Ok(base) => {
            if !blocks.contains(base) {
                d.push(Diagnostic::error(
                    codes::BLOCK_UNKNOWN,
                    "quests",
                    path.clone(),
                    format!(
                        "`{kind}` block `{block}` is not a known 1.21.11 block id — use a valid \
                         namespaced block id (e.g. `{example}`)"
                    ),
                ));
            }
            check_sculk_rest(block, "quests", path, d);
        }
        Err(reason) => {
            d.push(Diagnostic::error(
                codes::BLOCK_UNKNOWN,
                "quests",
                path,
                reason,
            ));
        }
    }
}

/// Every flag id declared anywhere in the campaign — the union of `set-flag`
/// effects (quest / dialogue / trigger) and v0.6 trap disarm `sets_flag`
/// (spec-0011). The authoritative declared-flag set every `requires_flags`
/// resolves against.
pub(crate) fn collect_declared_flags(c: &Campaign) -> BTreeSet<&str> {
    // Every root, every depth — inherited, not listed. This was the shallowest and
    // narrowest of the three answers `dsl::validate` used to give to "what flags does
    // this campaign produce": three roots and no descent at all, so a `set-flag`
    // in a `sequence` step was invisible to it while the main pass saw it.
    let mut flags: BTreeSet<&str> = BTreeSet::new();
    crate::for_each_campaign_effect(c, &mut |_path, _site, e| {
        if let Some(f) = e.set_flag() {
            flags.insert(f.as_str());
        }
    });
    for t in &c.quests.content.traps {
        if let Some(dis) = &t.disarm {
            flags.insert(dis.sets_flag.as_str());
        }
    }
    // A timed gate's disarm produces a first-class flag exactly as a
    // trap's does — the party jammed the portcullis, and the rest of the campaign
    // may read that fact.
    for g in &c.quests.content.timed_gates {
        if let Some(dis) = &g.disarm {
            flags.insert(dis.sets_flag.as_str());
        }
    }
    for tree in &c.dialogue.content.dialogues {
        for node in &tree.nodes {
            for opt in &node.options {
                for e in &opt.effects {
                    if let Some(f) = e.set_flag() {
                        flags.insert(f.as_str());
                    }
                }
            }
        }
    }
    flags
}

/// Transitive stage-4 quest ancestors: `q -> {every quest that must complete before
/// q starts}` (the `depends_on` closure). Acyclicity is guaranteed by `DW0130`.
pub(crate) fn quest_ancestors(c: &Campaign) -> BTreeMap<&str, BTreeSet<&str>> {
    let deps: BTreeMap<&str, &Vec<crate::ids::QuestId>> = c
        .quest_plan
        .content
        .quests
        .iter()
        .map(|q| (q.id.as_str(), &q.depends_on))
        .collect();
    let mut out = BTreeMap::new();
    for q in c.quest_plan.content.quests.iter() {
        let mut anc: BTreeSet<&str> = BTreeSet::new();
        let mut stack = vec![q.id.as_str()];
        while let Some(cur) = stack.pop() {
            if let Some(ds) = deps.get(cur) {
                for dep in ds.iter() {
                    if anc.insert(dep.as_str()) {
                        stack.push(dep.as_str());
                    }
                }
            }
        }
        out.insert(q.id.as_str(), anc);
    }
    out
}

/// Directed-graph cycle detection (three-color DFS).
pub(crate) fn graph_has_cycle<'a>(
    nodes: &[&'a str],
    edges: &BTreeMap<&'a str, Vec<&'a str>>,
) -> bool {
    let mut color: BTreeMap<&'a str, u8> = BTreeMap::new();
    for &n in nodes {
        if color.get(n).copied().unwrap_or(0) == 0 && dfs_cycle(n, edges, &mut color) {
            return true;
        }
    }
    false
}

fn dfs_cycle<'a>(
    node: &'a str,
    edges: &BTreeMap<&'a str, Vec<&'a str>>,
    color: &mut BTreeMap<&'a str, u8>,
) -> bool {
    color.insert(node, 1);
    if let Some(neis) = edges.get(node) {
        for &n in neis {
            match color.get(n).copied().unwrap_or(0) {
                0 => {
                    if dfs_cycle(n, edges, color) {
                        return true;
                    }
                }
                1 => return true,
                _ => {}
            }
        }
    }
    color.insert(node, 2);
    false
}

/// **The one inventory of flags this campaign produces.**
///
/// A `FlagId` has no declaration list — the set of flags is exactly those the
/// campaign produces — so every rule that asks *does this flag exist?* has to
/// reconstruct that set, and a rule that reconstructs it differently is asking a
/// different question under the same name. This is the answer all of them read:
/// `DW0172`'s unknown-flag refusals over objectives, effects, triggers, traps,
/// dialogue options and cast placements; `DW0480`'s over a declared branch's
/// `forks_on`; and `DW0818`'s over a layout-graph edge's gating.
///
/// Three producers, and the third is the one a second inventory forgets. A
/// `set-flag` fires from any effect root at any nesting depth, so the quest-side
/// walk is [`crate::for_each_campaign_effect`], which inherits both axes
/// rather than listing either. A dialogue option's `set-flag` is a flat outcome
/// of a conversation in the dialogue vocabulary, which that walk neither reaches
/// nor should. And a **trap's `disarm.sets_flag`** is a flag no effect anywhere
/// sets: the field's own documentation says other objectives and triggers may
/// read it through `requires_flags`, and until this became the single authority
/// `DW0172` was computing its own inventory that did not include it — so a
/// campaign gating on a disarm was refused for naming a flag "no `set-flag`
/// effect ever produces", which it never claimed to be.
pub fn produced_flags(c: &Campaign) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    crate::for_each_campaign_effect(c, &mut |_p, _site, eff| {
        if let Verb::SetFlag { flag, .. } = &eff.verb {
            out.insert(flag.as_str().to_string());
        }
    });
    for t in &c.dialogue.content.dialogues {
        for n in &t.nodes {
            for o in &n.options {
                for e in &o.effects {
                    if let crate::DialogueEffect::SetFlag { flag } = e {
                        out.insert(flag.as_str().to_string());
                    }
                }
            }
        }
    }
    for trap in &c.quests.content.traps {
        if let Some(dis) = &trap.disarm {
            out.insert(dis.sets_flag.as_str().to_string());
        }
    }
    out
}
