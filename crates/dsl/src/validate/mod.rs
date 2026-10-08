//! Campaign validation: all six rule groups from spec-0001.
//!
//! [`validate_campaign`] uses the vendored v0 registries; the compiler injects
//! full registries via [`validate_campaign_with`].

use crate::Verb;
use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostic::{Diagnostic, codes};
use crate::envelope::{Campaign, DSL_VERSION, Stage};
use crate::metrics::Metrics;
use crate::registry::{
    AnchorRegistry, BlockRegistry, EntityRegistry, ItemBackedBlockRegistry, ItemRegistry,
    VendoredAnchorRegistry, VendoredEffectRegistry, VendoredEntityRegistry, VendoredItemRegistry,
};
use crate::{EditFrame, MorphOp, QuestEffect, RegionShape, WorldEdit};

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

    envelope(c, &mut d);
    syntax(c, &mut d);
    uniqueness(c, &mut d);
    references(c, &mut d);
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
    lethal_stage_checks(c, &mut d);
    // spec-0031: the status-effect verbs. Every walk inside is empty for a
    // campaign that declares neither verb. The status-effect registry is the
    // fixed vanilla list wave-mob effects are validated against, so no injected
    // registry is needed.
    crate::quest::check::status_effect_checks(c, &VendoredEffectRegistry::v1_21_11(), &mut d);
    // spec-0032: the trade and recovery-stake surface. Every loop inside is
    // empty for a campaign that declares neither.
    economy_checks(c, &mut d);
    // spec-0034: the per-body traversal declaration. The walk is empty for a
    // campaign that declares none.
    crate::body::body_traversal_checks(c, &mut d);
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
    assembly_checks(c, anchors, &mut d);
    v06_trap_checks(c, items, entities, anchors, &mut d);
    shortcut_checks(c, anchors, &mut d);
    ambush_checks(c, &mut d);
    timed_gate_checks(c, anchors, &mut d);
    loot_checks(c, items, anchors, &mut d);
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
        world_edits_checks(c, &blocks, &mut d);
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
    collect_container_claim_checks(c, &mut d);
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
    lethal_volume_checks(c, anchors, &mut d);
    loop_checks(c, anchors, &mut d);
    // spec-0032: a shop stands on a prefab anchor, and an anchor
    // no bound prefab provides is the same defect a lethal volume's is.
    shop_anchor_checks(c, anchors, &mut d);
    // spec-0071 §2: a price has no field, so the copies of it — the gate term,
    // the charge, the ceiling on the arm that answers below it — are compared
    // here. Quantified over every effect list that charges a datum, never over
    // shops.
    crate::purchase::purchase_checks(c, &mut d);
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

/// Stage-5 lethal-volume structural checks (DSL v0.10, spec-0031): id syntax and
/// uniqueness, a resolvable region anchor, and a wording the player can actually
/// read (`DW0512`).
///
/// Everything geometric is deliberately absent here and lives in the compiler:
/// where the box lands, what it overlaps and whether the party can still finish
/// are questions about the *solved layout*, which the DSL crate does not have.
fn lethal_volume_checks(c: &Campaign, anchors: &dyn AnchorRegistry, d: &mut Vec<Diagnostic>) {
    let volumes = &c.quests.content.lethal_volumes;
    if volumes.is_empty() {
        return;
    }
    // The same "is this anchor provided by some bound prefab" rule every stage-5
    // anchor reference uses; a pool area defers the answer to the compiler.
    let providers = AnchorProviders::build(c, anchors);
    let mut seen_id: BTreeSet<&str> = BTreeSet::new();
    for (i, v) in volumes.iter().enumerate() {
        if !v.id.is_valid_syntax() {
            d.push(Diagnostic::error(
                codes::ID_SYNTAX,
                "quests",
                format!("/content/lethal_volumes/{i}/id"),
                format!(
                    "malformed lethal-volume id `{}` — lethal-volume ids must be lowercase \
                     kebab-case with the `lethal/` prefix (e.g. `lethal/cliff-fall`)",
                    v.id
                ),
            ));
        }
        if !seen_id.insert(v.id.as_str()) {
            d.push(Diagnostic::error(
                codes::ID_DUPLICATE,
                "quests",
                format!("/content/lethal_volumes/{i}/id"),
                format!("duplicate lethal-volume id `{}`", v.id),
            ));
        }
        if let Some(f) = station_kind_diag(
            &providers,
            v.region.anchor.as_str(),
            crate::layout::StationKind::Point,
            "a lethal volume's region centre",
            "quests",
            format!("/content/lethal_volumes/{i}/region/anchor"),
        ) {
            d.push(f);
        }
        if !providers.resolvable(v.region.anchor.as_str()) {
            d.push(Diagnostic::error(
                codes::ANCHOR_UNRESOLVED,
                "quests",
                format!("/content/lethal_volumes/{i}/region/anchor"),
                format!(
                    "lethal-volume anchor `{}` is not provided by any prefab bound in this \
                     campaign — {}",
                    v.region.anchor,
                    providers.anchor_remedy(
                        "use an anchor the prefab exposes (anchor names come from prefab \
                         metadata; do NOT invent one)"
                    ),
                ),
            ));
        }
        // `DW0891`, document arm (spec-0062 §4): a `shown_by` id that is a known
        // block and not one vanilla hurts a body with. Nothing has to be placed
        // to know it, so it is refused here rather than three passes later, and
        // the message prints the set the author may choose from — a remedy that
        // named no candidates would be a remedy an author has to guess at.
        //
        // An id the pinned version does not have at all is `DW0193`, the code
        // every block id in the DSL validates under, and deliberately not this
        // rule's: a typo is a typo wherever it is written.
        for (j, block) in v.shown_by.iter().enumerate() {
            if crate::blocks::BlockRegistry::v1_21_11()
                .validate_state_string(block)
                .is_err()
            {
                d.push(Diagnostic::error(
                    codes::BLOCK_UNKNOWN,
                    "quests",
                    format!("/content/lethal_volumes/{i}/shown_by/{j}"),
                    format!(
                        "lethal volume `{}` declares `shown_by` block `{block}`, which is not a \
                         block state of Minecraft Java 1.21.11",
                        v.id
                    ),
                ));
                continue;
            }
            if crate::blockshape::hurts_body(block) {
                continue;
            }
            d.push(Diagnostic::error(
                codes::LETHAL_INVISIBLE,
                "quests",
                format!("/content/lethal_volumes/{i}/shown_by/{j}"),
                format!(
                    "lethal volume `{}` declares `shown_by` block `{block}`, which vanilla does \
                     not hurt a body with — so it shows a player nothing, and floor made of it \
                     reads as safe however this volume is declared. `shown_by` says what the \
                     player SEES; it is not a word that switches the rule off. Name the block \
                     that shows the danger, from the set vanilla hurts a body with: {}.",
                    v.id,
                    crate::blockshape::HURTING_BLOCKS_1_21_11
                        .iter()
                        .map(|b| format!("`{b}`"))
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
            ));
        }
        if v.message.trim().is_empty() {
            d.push(Diagnostic::error(
                codes::LETHAL_MESSAGE_BLANK,
                "quests",
                format!("/content/lethal_volumes/{i}/message"),
                format!(
                    "lethal volume `{}` declares a blank `message`, so it would kill in silence \
                     — the one thing this declaration exists to prevent. Write the line the \
                     player reads as they die (`The undertow takes you.`); there is no compiler \
                     default that could be right for a cliff, a lava pit and an acid pool at \
                     once.",
                    v.id
                ),
            ));
        }
    }
}

/// Stage-5 loop structural checks (spec-0086): id syntax and uniqueness, the two
/// anchors resolvable, and the release a fact about the party (`DW0949`).
///
/// Everything geometric — the slab's shape, the move clearing it, the closed and
/// identical view — is about the solved layout and lives in the compiler
/// (`compiler::loop`).
fn loop_checks(c: &Campaign, anchors: &dyn AnchorRegistry, d: &mut Vec<Diagnostic>) {
    let loops = &c.quests.content.loops;
    if loops.is_empty() {
        return;
    }
    let providers = AnchorProviders::build(c, anchors);
    let scope_of: BTreeMap<&str, crate::StateScope> = c
        .quests
        .content
        .state
        .iter()
        .map(|s| (s.id.as_str(), s.scope))
        .collect();
    let mut seen_id: BTreeSet<&str> = BTreeSet::new();
    for (i, l) in loops.iter().enumerate() {
        let at = |tail: &str| format!("/content/loops/{i}{tail}");
        if !l.id.is_valid_syntax() {
            d.push(Diagnostic::error(
                codes::ID_SYNTAX,
                "quests",
                at("/id"),
                format!(
                    "malformed loop id `{}` — loop ids must be lowercase kebab-case with the \
                     `loop/` prefix (e.g. `loop/long-gallery`)",
                    l.id
                ),
            ));
        }
        if !seen_id.insert(l.id.as_str()) {
            d.push(Diagnostic::error(
                codes::ID_DUPLICATE,
                "quests",
                at("/id"),
                format!("duplicate loop id `{}`", l.id),
            ));
        }
        for (field, anchor, what) in [
            (
                "/region/anchor",
                l.region.anchor.as_str(),
                "a loop's slab centre",
            ),
            ("/to/anchor", l.to.anchor.as_str(), "a loop's landing"),
        ] {
            if let Some(f) = station_kind_diag(
                &providers,
                anchor,
                crate::layout::StationKind::Point,
                what,
                "quests",
                at(field),
            ) {
                d.push(f);
            }
            if !providers.resolvable(anchor) {
                d.push(Diagnostic::error(
                    codes::ANCHOR_UNRESOLVED,
                    "quests",
                    at(field),
                    format!(
                        "loop `{}` names anchor `{anchor}` ({what}), which no prefab bound in \
                         this campaign provides — {}",
                        l.id,
                        providers.anchor_remedy(
                            "use an anchor the prefab exposes (anchor names come from prefab \
                             metadata; do NOT invent one)"
                        ),
                    ),
                ));
            }
        }
        // `DW0949`: the gate is the release, and a loop with none holds forever.
        if l.gate().is_empty() {
            d.push(Diagnostic::error(
                codes::LOOP_GATE,
                "quests",
                at(""),
                format!(
                    "loop `{}` declares no gate term — no `requires_flags`, no `forbids_flags`, \
                     no `requires_state` — so it holds forever and a party that walks into it \
                     can never leave: that is a soft-lock spelled out, not a mechanism. Give it \
                     a release the party reaches: `forbids_flags: [flag/<found>]` ends it when \
                     a flag is set, and `requires_state: [{{\"state\": <counts>, \"op\": \
                     \"at-most\", \"value\": n}}]` on its own `counts` datum ends it after a \
                     number of crossings",
                    l.id
                ),
            ));
        }
        // …and the release is a fact about the party.
        for (k, cmp) in l.requires_state.iter().enumerate() {
            if scope_of.get(cmp.state.as_str()) == Some(&crate::StateScope::Player) {
                d.push(Diagnostic::error(
                    codes::LOOP_GATE,
                    "quests",
                    at(&format!("/requires_state/{k}")),
                    format!(
                        "loop `{}` reads `{}` in its gate term `requires_state/{k}`, and that \
                         datum is `player`-scoped: a release one player holds and another does \
                         not splits the party into a looped half and a free half. The release \
                         is a fact about the party — declare the datum `party`-scoped, or \
                         release on a flag",
                        l.id,
                        cmp.state.as_str()
                    ),
                ));
            }
        }
        if let Some(counts) = &l.counts {
            match scope_of.get(counts.as_str()) {
                None => d.push(Diagnostic::error(
                    codes::STATE_UNDECLARED,
                    "quests",
                    at("/counts"),
                    format!(
                        "loop `{}` counts its crossings into `{}`, which the campaign never \
                         declares. Add it to the stage-5 `state` list as a `party` datum, or \
                         fix the id",
                        l.id,
                        counts.as_str()
                    ),
                )),
                Some(crate::StateScope::Player) => d.push(Diagnostic::error(
                    codes::LOOP_GATE,
                    "quests",
                    at("/counts"),
                    format!(
                        "loop `{}` counts its crossings into `{}`, which is `player`-scoped: \
                         the count a release reads is a fact about the party, and a count each \
                         player keeps for themselves is a release one of them holds and \
                         another does not. Declare the datum `party`-scoped",
                        l.id,
                        counts.as_str()
                    ),
                )),
                Some(crate::StateScope::Party) => {}
            }
        }
        // A `teleport` inside `on_cross`, at any nesting depth.
        fn teleports(effs: &[QuestEffect], path: &str, out: &mut Vec<String>) {
            for (j, e) in effs.iter().enumerate() {
                let here = format!("{path}/{j}");
                if matches!(e.verb, crate::Verb::Teleport { .. }) {
                    out.push(here.clone());
                }
                for (pseg, _k, list) in e.nested_effect_lists_labeled() {
                    teleports(list, &format!("{here}/{pseg}"), out);
                }
            }
        }
        let mut found = Vec::new();
        teleports(&l.on_cross, &at("/on_cross"), &mut found);
        for path in found {
            d.push(Diagnostic::error(
                codes::LOOP_GATE,
                "quests",
                path.clone(),
                format!(
                    "loop `{}` runs a `teleport` in its `on_cross` (term `{path}`): the body was \
                     just moved by the loop, and a second move in the same tick is two carries \
                     with one position. A place that looks different is a `teleport` of its \
                     own, fired from a trigger the party reaches — take it out of the loop",
                    l.id
                ),
            ));
        }
    }
}

/// A shop's anchor must be provided by some prefab bound in this campaign
/// (DSL v0.10, spec-0032) — the same rule, and the same message shape, every
/// other stage-5 anchor reference follows.
fn shop_anchor_checks(c: &Campaign, anchors: &dyn AnchorRegistry, d: &mut Vec<Diagnostic>) {
    if c.quests.content.shops.is_empty() {
        return;
    }
    let providers = AnchorProviders::build(c, anchors);
    for (i, sh) in c.quests.content.shops.iter().enumerate() {
        if let Some(f) = station_kind_diag(
            &providers,
            sh.anchor.as_str(),
            crate::layout::StationKind::Point,
            "a shop counter",
            "quests",
            format!("/content/shops/{i}/anchor"),
        ) {
            d.push(f);
        }
        if providers.resolvable(sh.anchor.as_str()) {
            continue;
        }
        d.push(Diagnostic::error(
            codes::ANCHOR_UNRESOLVED,
            "quests",
            format!("/content/shops/{i}/anchor"),
            format!(
                "shop anchor `{}` is not provided by any prefab bound in this campaign — {}",
                sh.anchor,
                providers.anchor_remedy(
                    "use an anchor the prefab exposes (anchor names come from prefab metadata; do \
                     NOT invent one)"
                ),
            ),
        ));
    }
}

/// Stage-5 economy checks (DSL v0.10, spec-0032): the shop's structural
/// obligations and the stake's declaration obligations.
///
/// **What is deliberately NOT here.** Everything about *where* a stake lands —
/// the placement table, its walkability, its reachability from the respawn point
/// in force, and whether the block under it is one runtime removes — is a
/// question about the solved layout and the navigation world, which the DSL crate
/// does not have. Those live in `crate::stake` on the compiler side, exactly as a
/// lethal volume's geometry does.
///
/// **No price check appears here either**, and that is the design showing
/// through: a price is a [`crate::gate::Gate`] term, so it is already covered by
/// `DW0500`–`DW0503` — the datum must be declared, must be written somewhere,
/// must be read somewhere, and must be reachable at the scope the site evaluates
/// at. A shop that added a comparison field of its own would have needed all four
/// rules written a second time.
fn economy_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let stakes = &c.quests.content.stakes;
    let shops = &c.quests.content.shops;
    if stakes.is_empty() && shops.is_empty() {
        return;
    }

    // --- stakes: id hygiene, the datum, the scope, the forfeit range ----------
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for (i, s) in stakes.iter().enumerate() {
        if !s.id.is_valid_syntax() {
            d.push(Diagnostic::error(
                codes::ID_SYNTAX,
                "quests",
                format!("/content/stakes/{i}/id"),
                format!(
                    "malformed stake id `{}` — stake ids are `stake/<kebab-case>`",
                    s.id
                ),
            ));
        }
        if !seen.insert(s.id.as_str()) {
            d.push(Diagnostic::error(
                codes::ID_DUPLICATE,
                "quests",
                format!("/content/stakes/{i}/id"),
                format!("duplicate stake id `{}`", s.id),
            ));
        }
        match c.quests.content.state_decl(s.state.as_str()) {
            None => d.push(Diagnostic::error(
                codes::STAKE_STATE_SCOPE,
                "quests",
                format!("/content/stakes/{i}/state"),
                format!(
                    "stake `{}` forfeits `{}`, which the campaign never declares. Add it to the \
                     stage-5 `state` list with `\"scope\": \"player\"` — a stake is one player's \
                     wager, and a datum's scope is a fact no use site can supply",
                    s.id,
                    s.state.as_str()
                ),
            )),
            Some(decl) if decl.scope != crate::StateScope::Player => {
                d.push(Diagnostic::error(
                    codes::STAKE_STATE_SCOPE,
                    "quests",
                    format!("/content/stakes/{i}/state"),
                    format!(
                        "stake `{}` forfeits `{}`, which is declared `party`-scoped. A stake is a \
                         PERSONAL wager: one shared purse turns a teammate's death into a penalty \
                         on everyone, and nothing in the JSON would say so. Declare the datum \
                         `player`-scoped, or point the stake at one that is.",
                        s.id,
                        s.state.as_str()
                    ),
                ));
            }
            Some(_) => {}
        }
        if let Some(crate::Forfeit::Proportion { percent }) = s.forfeit
            && percent > 100
        {
            d.push(Diagnostic::error(
                codes::STAKE_FORFEIT_RANGE,
                "quests",
                format!("/content/stakes/{i}/forfeit/percent"),
                format!(
                    "stake `{}` forfeits {percent}% of `{}` — more than the whole purse. Use \
                     0–100, or `{{\"kind\": \"all\"}}`.",
                    s.id,
                    s.state.as_str()
                ),
            ));
        }
    }

    // --- `drop-stake`: every reference resolves, every declaration is dropped --
    // Both halves, for the reason `DW0501`/`DW0502` state for a datum: a
    // reference with no declaration is a runtime no-op, and a declaration no beat
    // fires is a whole mechanism that binds to nothing.
    let mut dropped: BTreeSet<String> = BTreeSet::new();
    crate::for_each_campaign_effect(c, &mut |path, _site, eff| {
        let crate::Verb::DropStake { stake, .. } = &eff.verb else {
            return;
        };
        if c.quests.content.stake_decl(stake.as_str()).is_none() {
            d.push(Diagnostic::error(
                codes::STAKE_UNDECLARED,
                "quests",
                path.to_string(),
                format!(
                    "`drop-stake` leaves `{}`, which the campaign never declares. Add it to the \
                     stage-5 `stakes` list, or fix the id",
                    stake.as_str()
                ),
            ));
            return;
        }
        dropped.insert(stake.as_str().to_string());
    });
    for (i, s) in stakes.iter().enumerate() {
        if dropped.contains(s.id.as_str()) {
            continue;
        }
        d.push(Diagnostic::error(
            codes::STAKE_NEVER_DROPPED,
            "quests",
            format!("/content/stakes/{i}/id"),
            format!(
                "stake `{}` is declared and no `drop-stake` effect anywhere in the campaign ever \
                 leaves one. Its forfeit rule, its retention policy and its whole compile-time \
                 placement table describe a mechanism no beat can fire. Drop it from a beat — \
                 `on_death` is the usual one — or delete the declaration.",
                s.id
            ),
        ));
    }

    crate::state::read_after_write_checks(c, d);

    // --- shops: id hygiene, and no button that cannot answer ------------------
    let mut seen_shop: BTreeSet<&str> = BTreeSet::new();
    for (i, sh) in shops.iter().enumerate() {
        if !sh.id.is_valid_syntax() {
            d.push(Diagnostic::error(
                codes::ID_SYNTAX,
                "quests",
                format!("/content/shops/{i}/id"),
                format!(
                    "malformed shop id `{}` — shop ids are `shop/<kebab-case>`",
                    sh.id
                ),
            ));
        }
        if !seen_shop.insert(sh.id.as_str()) {
            d.push(Diagnostic::error(
                codes::ID_DUPLICATE,
                "quests",
                format!("/content/shops/{i}/id"),
                format!("duplicate shop id `{}`", sh.id),
            ));
        }
        if sh.offers.is_empty() {
            d.push(Diagnostic::error(
                codes::SHOP_OFFER_INERT,
                "quests",
                format!("/content/shops/{i}/offers"),
                format!(
                    "shop `{}` declares no offers. Vanilla's dialog codec rejects an empty action \
                     list outright, so this is not merely an empty shop — it is a dialog that \
                     fails to load. Give it at least one offer, or delete the shop.",
                    sh.id
                ),
            ));
        }
        for (j, off) in sh.offers.iter().enumerate() {
            if off.effects.is_empty() {
                d.push(Diagnostic::error(
                    codes::SHOP_OFFER_INERT,
                    "quests",
                    format!("/content/shops/{i}/offers/{j}/effects"),
                    format!(
                        "offer `{}` in shop `{}` declares no effects: the button is drawn, is \
                         pressable, and does nothing. A control the player can operate must have \
                         an answer — a refusal counts, so a single `narrate` gated on \
                         `requires_state` (`at-most <price − 1>`) is enough.",
                        off.label, sh.id
                    ),
                ));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Rule group 1 — envelope
// ---------------------------------------------------------------------------

fn envelope(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let stages = [
        (Stage::World, c.world.stage, c.world.dsl_version.as_str()),
        (Stage::Npcs, c.npcs.stage, c.npcs.dsl_version.as_str()),
        (
            Stage::Classes,
            c.classes.stage,
            c.classes.dsl_version.as_str(),
        ),
        (
            Stage::QuestPlan,
            c.quest_plan.stage,
            c.quest_plan.dsl_version.as_str(),
        ),
        (Stage::Quests, c.quests.stage, c.quests.dsl_version.as_str()),
        (
            Stage::Dialogue,
            c.dialogue.stage,
            c.dialogue.dsl_version.as_str(),
        ),
    ];
    let stages: Vec<(Stage, Stage, &str)> = stages
        .into_iter()
        .chain(
            c.world_edits
                .iter()
                .map(|e| (Stage::WorldEdits, e.stage, e.dsl_version.as_str())),
        )
        .chain(
            c.geometry_brief
                .iter()
                .map(|e| (Stage::GeometryBrief, e.stage, e.dsl_version.as_str())),
        )
        .chain(
            c.layout_graph
                .iter()
                .map(|e| (Stage::LayoutGraph, e.stage, e.dsl_version.as_str())),
        )
        .chain(
            c.site_plan
                .iter()
                .map(|e| (Stage::SitePlan, e.stage, e.dsl_version.as_str())),
        )
        .chain(
            c.detail_plan
                .iter()
                .map(|e| (Stage::DetailPlan, e.stage, e.dsl_version.as_str())),
        )
        .chain(
            c.design
                .iter()
                .map(|e| (Stage::Design, e.stage, e.dsl_version.as_str())),
        )
        .collect();
    for (expected, actual, version) in stages {
        if actual != expected {
            d.push(Diagnostic::error(
                codes::STAGE_MISMATCH,
                expected.name(),
                "/stage",
                format!(
                    "`stage` is `{}` but this is the `{}` stage document — set `stage` to `{}` (or \
                     move this content into the `{}` document it belongs to)",
                    actual.name(),
                    expected.name(),
                    expected.name(),
                    actual.name(),
                ),
            ));
        }
        if version != DSL_VERSION {
            d.push(Diagnostic::error(
                codes::DSL_VERSION,
                expected.name(),
                "/dsl_version",
                format!(
                    "dsl_version `{version}` is not the one this engine accepts — set it to \
                     `{DSL_VERSION}` and revise the document against that surface. An engine \
                     accepts exactly the number it implements (ADR-0024); a document written \
                     for another number is built by the engine that implements that number."
                ),
            ));
        }
    }

    let ids: Vec<(Stage, &crate::ids::CampaignId)> = [
        (Stage::World, &c.world.campaign_id),
        (Stage::Npcs, &c.npcs.campaign_id),
        (Stage::Classes, &c.classes.campaign_id),
        (Stage::QuestPlan, &c.quest_plan.campaign_id),
        (Stage::Quests, &c.quests.campaign_id),
        (Stage::Dialogue, &c.dialogue.campaign_id),
    ]
    .into_iter()
    .chain(
        c.world_edits
            .iter()
            .map(|e| (Stage::WorldEdits, &e.campaign_id)),
    )
    .chain(
        c.geometry_brief
            .iter()
            .map(|e| (Stage::GeometryBrief, &e.campaign_id)),
    )
    .chain(
        c.layout_graph
            .iter()
            .map(|e| (Stage::LayoutGraph, &e.campaign_id)),
    )
    .chain(
        c.site_plan
            .iter()
            .map(|e| (Stage::SitePlan, &e.campaign_id)),
    )
    .chain(
        c.detail_plan
            .iter()
            .map(|e| (Stage::DetailPlan, &e.campaign_id)),
    )
    .chain(c.design.iter().map(|e| (Stage::Design, &e.campaign_id)))
    .collect();
    let canonical = c.world.campaign_id.as_str();
    for (stage, id) in ids {
        if !id.is_valid_syntax() {
            d.push(Diagnostic::error(
                codes::ID_SYNTAX,
                stage.name(),
                "/campaign_id",
                format!("malformed campaign_id `{id}` (expected kebab-case)"),
            ));
        }
        if id.as_str() != canonical {
            d.push(Diagnostic::error(
                codes::CAMPAIGN_ID_MISMATCH,
                stage.name(),
                "/campaign_id",
                format!(
                    "campaign_id `{id}` differs from `{canonical}` (the world stage's id) — set \
                     every stage's `campaign_id` to `{canonical}` so all six documents name one \
                     campaign"
                ),
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// Rule group 2 — id syntax
// ---------------------------------------------------------------------------

fn syntax(c: &Campaign, d: &mut Vec<Diagnostic>) {
    // The form is taken from the id's own type (`syntax_form`), never written
    // into this message. This macro is the ONE path every id type's syntax
    // refusal goes through, and it used to answer all of them with the same
    // three examples — `area/keep`, `npc/keeper`, `quest/find-key` — so a
    // rejected dialogue node id was refused by a sentence that never spelled
    // `dlg/<kebab>`, and the rule it needed lived only in the schema
    // description. The prefix belongs to the id type, so every site gets it
    // from the type: the general mechanism was here all along, and only its
    // message was too narrow to reach what it was rejecting.
    macro_rules! chk {
        ($id:expr, $stage:expr, $path:expr) => {
            if !$id.is_valid_syntax() {
                d.push(Diagnostic::error(
                    codes::ID_SYNTAX,
                    $stage,
                    $path,
                    format!(
                        "malformed id `{}` — this field takes {}: the type prefix, a `/`, and \
                         one lowercase kebab-case segment after it ([a-z0-9] and `-`, no second \
                         `/`, no capitals, no underscores)",
                        $id,
                        $id.syntax_form()
                    ),
                ));
            }
        };
    }

    for (i, a) in c.world.content.areas.iter().enumerate() {
        chk!(a.id, "world", format!("/content/areas/{i}/id"));
        if let Some(prefab) = &a.prefab {
            chk!(prefab, "world", format!("/content/areas/{i}/prefab"));
        }
        if let Some(pool) = &a.prefab_pool {
            chk!(pool, "world", format!("/content/areas/{i}/prefab_pool"));
        }
    }
    for (i, a) in c.world.content.atmospheres.iter().enumerate() {
        chk!(a.id, "world", format!("/content/atmospheres/{i}/id"));
    }
    for (i, npc) in c.npcs.content.npcs.iter().enumerate() {
        chk!(npc.id, "npcs", format!("/content/npcs/{i}/id"));
    }
    for (i, cl) in c.classes.content.classes.iter().enumerate() {
        chk!(cl.id, "classes", format!("/content/classes/{i}/id"));
    }
    for (i, q) in c.quest_plan.content.quests.iter().enumerate() {
        chk!(q.id, "quest-plan", format!("/content/quests/{i}/id"));
    }
    for (i, q) in c.quests.content.quests.iter().enumerate() {
        chk!(q.id, "quests", format!("/content/quests/{i}/id"));
        for (j, obj) in q.objectives.iter().enumerate() {
            chk!(
                obj.id(),
                "quests",
                format!("/content/quests/{i}/objectives/{j}/id")
            );
        }
    }
    for (i, a) in c.quests.content.actors.iter().enumerate() {
        chk!(a.id, "quests", format!("/content/actors/{i}/id"));
    }
    for (i, a) in c.quests.content.assemblies.iter().enumerate() {
        chk!(a.id, "quests", format!("/content/assemblies/{i}/id"));
        chk!(a.rig, "quests", format!("/content/assemblies/{i}/rig"));
    }
    for (i, tree) in c.dialogue.content.dialogues.iter().enumerate() {
        for (j, node) in tree.nodes.iter().enumerate() {
            chk!(
                node.id,
                "dialogue",
                format!("/content/dialogues/{i}/nodes/{j}/id")
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Rule group 2 — id uniqueness
// ---------------------------------------------------------------------------

fn dup_check<'a>(
    ids: impl Iterator<Item = (&'a str, String)>,
    stage: &'static str,
    what: &str,
    d: &mut Vec<Diagnostic>,
) {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for (id, path) in ids {
        if !seen.insert(id) {
            d.push(Diagnostic::error(
                codes::ID_DUPLICATE,
                stage,
                path,
                format!("duplicate {what} id `{id}` — rename one so every {what} id is unique"),
            ));
        }
    }
}

fn uniqueness(c: &Campaign, d: &mut Vec<Diagnostic>) {
    dup_check(
        c.world
            .content
            .areas
            .iter()
            .enumerate()
            .map(|(i, a)| (a.id.as_str(), format!("/content/areas/{i}/id"))),
        "world",
        "area",
        d,
    );
    dup_check(
        c.npcs
            .content
            .npcs
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.as_str(), format!("/content/npcs/{i}/id"))),
        "npcs",
        "npc",
        d,
    );
    dup_check(
        c.classes
            .content
            .classes
            .iter()
            .enumerate()
            .map(|(i, cl)| (cl.id.as_str(), format!("/content/classes/{i}/id"))),
        "classes",
        "class",
        d,
    );
    dup_check(
        c.quest_plan
            .content
            .quests
            .iter()
            .enumerate()
            .map(|(i, q)| (q.id.as_str(), format!("/content/quests/{i}/id"))),
        "quest-plan",
        "quest",
        d,
    );
    dup_check(
        c.quests
            .content
            .quests
            .iter()
            .enumerate()
            .map(|(i, q)| (q.id.as_str(), format!("/content/quests/{i}/id"))),
        "quests",
        "quest",
        d,
    );
    // Objective ids: unique across all of stage 5 (so cross-stage dialogue refs
    // resolve unambiguously).
    dup_check(
        c.quests
            .content
            .quests
            .iter()
            .enumerate()
            .flat_map(|(i, q)| {
                q.objectives.iter().enumerate().map(move |(j, o)| {
                    (
                        o.id().as_str(),
                        format!("/content/quests/{i}/objectives/{j}/id"),
                    )
                })
            }),
        "quests",
        "objective",
        d,
    );
    // Scripted-actor ids: unique within the stage-5 actors namespace (DSL v0.6).
    dup_check(
        c.quests
            .content
            .actors
            .iter()
            .enumerate()
            .map(|(i, a)| (a.id.as_str(), format!("/content/actors/{i}/id"))),
        "quests",
        "actor",
        d,
    );
    // Assembly ids: unique within the stage-5 assemblies namespace (spec-0082).
    dup_check(
        c.quests
            .content
            .assemblies
            .iter()
            .enumerate()
            .map(|(i, a)| (a.id.as_str(), format!("/content/assemblies/{i}/id"))),
        "quests",
        "assembly",
        d,
    );
    // Dialogue trees: at most one per NPC (a duplicate tree is a duplicate npc
    // binding within the stage-6 dialogue namespace).
    dup_check(
        c.dialogue
            .content
            .dialogues
            .iter()
            .enumerate()
            .map(|(i, t)| (t.npc.as_str(), format!("/content/dialogues/{i}/npc"))),
        "dialogue",
        "dialogue tree for npc",
        d,
    );
    // Dialogue node ids: unique within each tree.
    for (i, tree) in c.dialogue.content.dialogues.iter().enumerate() {
        dup_check(
            tree.nodes.iter().enumerate().map(|(j, node)| {
                (
                    node.id.as_str(),
                    format!("/content/dialogues/{i}/nodes/{j}/id"),
                )
            }),
            "dialogue",
            "dialogue node",
            d,
        );
    }
}

// ---------------------------------------------------------------------------
// Rule group 2 — dangling references (non-dialogue, non-finale)
// ---------------------------------------------------------------------------

fn references(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let mut area_ids: BTreeSet<&str> = c
        .world
        .content
        .areas
        .iter()
        .map(|a| a.id.as_str())
        .collect();
    // A site-plan campaign has no `areas[]` — `DW0839` refuses one that does —
    // and exactly one place instead: the site the plan lays out. NPCs and
    // planned quests name it like any other area, so it is a declared area id
    // here for the same reason `areas[]` entries are.
    if c.site_plan.is_some() {
        area_ids.insert(crate::siteplan::SITE_AREA);
    }
    let npc_ids: BTreeSet<&str> = c.npcs.content.npcs.iter().map(|n| n.id.as_str()).collect();
    let planned_ids: BTreeSet<&str> = c
        .quest_plan
        .content
        .quests
        .iter()
        .map(|q| q.id.as_str())
        .collect();
    let expanded_ids: BTreeSet<&str> = c
        .quests
        .content
        .quests
        .iter()
        .map(|q| q.id.as_str())
        .collect();

    let dangling = |d: &mut Vec<Diagnostic>, ok: bool, stage, path: String, msg: String| {
        if !ok {
            d.push(Diagnostic::error(codes::DANGLING_REF, stage, path, msg));
        }
    };

    for (i, npc) in c.npcs.content.npcs.iter().enumerate() {
        dangling(
            d,
            area_ids.contains(npc.area.as_str()),
            "npcs",
            format!("/content/npcs/{i}/area"),
            format!(
                "npc references unknown area `{}` — {}",
                npc.area,
                crate::placement::Placement::of(c).area_remedy(),
            ),
        );
        // Persona relationships are same-stage NPC refs (validated within stage 2).
        for (k, rel) in npc.persona.relationships.iter().enumerate() {
            dangling(
                d,
                npc_ids.contains(rel.npc.as_str()),
                "npcs",
                format!("/content/npcs/{i}/persona/relationships/{k}/npc"),
                format!(
                    "persona relationship references unknown npc `{}` — declare that npc in \
                     stage 2 or correct the reference",
                    rel.npc
                ),
            );
        }
    }

    // spec-0080: an atmosphere is named by a place for its first tick and by a
    // `set-atmosphere` for a repaint, and a repaint's `place` names an area or
    // a site-plan box. Each is the plain unresolved-reference shape.
    let atmosphere_ids: BTreeSet<&str> = c
        .world
        .content
        .atmospheres
        .iter()
        .map(|a| a.id.as_str())
        .collect();
    let atmosphere_remedy = |id: &str| {
        format!(
            "unknown atmosphere `{id}` — declare it in `world.atmospheres[]` or correct the \
             reference"
        )
    };
    for (i, a) in c.world.content.areas.iter().enumerate() {
        if let Some(id) = &a.atmosphere {
            dangling(
                d,
                atmosphere_ids.contains(id.as_str()),
                "world",
                format!("/content/areas/{i}/atmosphere"),
                atmosphere_remedy(id.as_str()),
            );
        }
    }
    let mut place_ids: BTreeSet<&str> = c
        .world
        .content
        .areas
        .iter()
        .map(|a| a.id.as_str())
        .collect();
    if let Some(sp) = &c.site_plan {
        for (i, b) in sp.content.boxes.iter().enumerate() {
            place_ids.insert(b.node.as_str());
            if let Some(id) = &b.atmosphere {
                dangling(
                    d,
                    atmosphere_ids.contains(id.as_str()),
                    "site-plan",
                    format!("/content/boxes/{i}/atmosphere"),
                    atmosphere_remedy(id.as_str()),
                );
            }
        }
    }
    crate::for_each_campaign_effect(c, &mut |path, site, e| {
        let crate::Verb::SetAtmosphere {
            atmosphere, place, ..
        } = &e.verb
        else {
            return;
        };
        let stage = match site {
            crate::EffectSite::DialogueRespawn { .. } => "dialogue",
            _ => "quests",
        };
        if let Some(id) = atmosphere {
            dangling(
                d,
                atmosphere_ids.contains(id.as_str()),
                stage,
                format!("{path}/atmosphere"),
                atmosphere_remedy(id.as_str()),
            );
        }
        if let Some(place) = place {
            dangling(
                d,
                place_ids.contains(place.as_str()),
                stage,
                format!("{path}/place"),
                format!(
                    "`set-atmosphere` repaints unknown place `{place}` — name an `area/…` from \
                     `world.areas[]` or a site-plan box's `node/…`"
                ),
            );
        }
    });

    for (i, q) in c.quest_plan.content.quests.iter().enumerate() {
        dangling(
            d,
            area_ids.contains(q.area.as_str()),
            "quest-plan",
            format!("/content/quests/{i}/area"),
            format!(
                "quest references unknown area `{}` — {}",
                q.area,
                crate::placement::Placement::of(c).area_remedy(),
            ),
        );
        for (k, npc) in q.npcs.iter().enumerate() {
            dangling(
                d,
                npc_ids.contains(npc.as_str()),
                "quest-plan",
                format!("/content/quests/{i}/npcs/{k}"),
                format!(
                    "quest references unknown npc `{npc}` — declare it in stage 2 or correct the \
                     reference"
                ),
            );
        }
        for (k, dep) in q.depends_on.iter().enumerate() {
            dangling(
                d,
                planned_ids.contains(dep.as_str()),
                "quest-plan",
                format!("/content/quests/{i}/depends_on/{k}"),
                format!(
                    "quest depends on unknown quest `{dep}` — declare it in the stage-4 quest \
                     plan or correct the `depends_on` entry"
                ),
            );
        }
    }

    for (i, q) in c.quests.content.quests.iter().enumerate() {
        if let crate::Trigger::QuestComplete { quest } = &q.trigger {
            dangling(
                d,
                expanded_ids.contains(quest.as_str()),
                "quests",
                format!("/content/quests/{i}/trigger/quest"),
                format!(
                    "quest trigger `quest-complete` references unknown quest `{quest}` — declare \
                     that quest in stage 5 or correct the reference"
                ),
            );
        }
        let local_objs: BTreeSet<&str> = q.objectives.iter().map(|o| o.id().as_str()).collect();
        for (j, obj) in q.objectives.iter().enumerate() {
            if let crate::Objective::TalkTo { npc, .. } = obj {
                dangling(
                    d,
                    npc_ids.contains(npc.as_str()),
                    "quests",
                    format!("/content/quests/{i}/objectives/{j}/npc"),
                    format!(
                        "`talk-to` objective references unknown npc `{npc}` — declare it in \
                         stage 2 or correct the reference"
                    ),
                );
            }
            for (m, aft) in obj.after().iter().enumerate() {
                dangling(
                    d,
                    local_objs.contains(aft.as_str()),
                    "quests",
                    format!("/content/quests/{i}/objectives/{j}/after/{m}"),
                    format!(
                        "objective `after` references unknown objective `{aft}` — `after` may \
                         only name another objective in the same quest; declare it or correct \
                         the reference"
                    ),
                );
            }
        }
        for key in q.on_objective_complete.keys() {
            dangling(
                d,
                local_objs.contains(key.as_str()),
                "quests",
                format!("/content/quests/{i}/on_objective_complete/{key}"),
                format!(
                    "`on_objective_complete` is keyed by unknown objective `{key}` — the key must \
                     name an objective declared in this quest; declare it or correct the key"
                ),
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Rule group 3 — stage-6 dialogue graph
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Rule group 4 — quest plan
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Rule group 5 — intra-quest objective ordering (`after` DAG)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Rule group 5 — reserved values / fields
// ---------------------------------------------------------------------------

/// `DW0953`'s empty-gate shape (spec-0088 §3.2): a `when` with no term is not a
/// stage. The player-scoped shape is raised beside `DW0503` in
/// [`state_checks`], where every gate's `requires_state` is already read.
fn lethal_stage_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    for (i, v) in c.quests.content.lethal_volumes.iter().enumerate() {
        if v.when.is_some() && v.gate().is_empty() {
            d.push(Diagnostic::error(
                codes::LETHAL_STAGE_GATE,
                "quests",
                format!("/content/lethal_volumes/{i}/when"),
                format!(
                    "lethal volume `{}` declares `when: {{}}` — a stage with no term. An \
                     always-live volume is spelled by leaving `when` out; to stage it, name a \
                     flag (`requires_flags` / `forbids_flags`) or a `party`-scoped datum \
                     (`requires_state`)",
                    v.id.as_str()
                ),
            ));
        }
    }
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

// ---------------------------------------------------------------------------
// Rule group 6 — prefab / prefab_pool binding (stage 1)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Rule group 6 — cross-stage 1:1 (quest plan↔expansion, npc↔dialogue tree)
// ---------------------------------------------------------------------------

/// Split a block field into its base id and (optional) blockstate suffix,
/// validating the suffix's syntax (DSL v0.6). Returns the base id to
/// check against the block registry; `Err(reason)` when a `[...]` suffix is
/// present but malformed (unbalanced brackets, empty, or a token that is not a
/// lowercase `key=value`). A well-formed state string is passed through verbatim
/// to `setblock` — vanilla validates the property names/values against the
/// block's own state definition, so the compiler only guards the surface syntax.
fn split_blockstate(block: &str) -> Result<&str, String> {
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
                    path,
                    format!(
                        "`{kind}` block `{block}` is not a known 1.21.11 block id — use a valid \
                         namespaced block id (e.g. `{example}`)"
                    ),
                ));
            }
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
fn collect_declared_flags(c: &Campaign) -> BTreeSet<&str> {
    // Every root, every depth — inherited, not listed. This was the shallowest and
    // narrowest of the three answers this file used to give to "what flags does
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

/// spec-0082: **assemblies, their rigs, and every reference to one.**
///
/// * Each assembly's `rig` resolves in the library and passes the rig's
///   structural rules ([`crate::rig::check`]); its `initial` and every strike
///   step's `windup`/`strike` name clips the rig declares (`DW0935`). A
///   registry that is not the whole library answers
///   [`crate::rig::RigLookup::Unknown`] and nothing is refused on its word.
/// * Its mark's anchor, and its arming region's, are provided by some area
///   (`DW0142`), and the mark is a point station.
/// * Every `spawn-assembly` / `despawn-assembly` / `play-clip`, at every depth
///   of every effect root, names a declared assembly (`DW0112`), and a
///   `play-clip` names a clip its rig declares (`DW0935`).
/// * Every `strike-assembly` trigger names a declared assembly (`DW0112`).
///
/// The hitbox's bounds, its reach and where a blow lands are judged at build
/// time, where cells exist (`DW0936`–`DW0938`, `compiler::assembly`).
fn assembly_checks(c: &Campaign, anchors: &dyn AnchorRegistry, d: &mut Vec<Diagnostic>) {
    use crate::rig::RigLookup;
    let quests = &c.quests.content;
    if quests.assemblies.is_empty()
        && !quests
            .triggers
            .iter()
            .any(|t| t.on.assembly_target().is_some())
    {
        // Still walk the effects: a verb naming an assembly in a campaign that
        // declares none is a dangling reference.
        let mut any = false;
        crate::for_each_campaign_effect(c, &mut |_, _, e| {
            any |= assembly_verb(e).is_some();
        });
        if !any {
            return;
        }
    }
    let providers = AnchorProviders::build(c, anchors);
    // The rig each declared assembly resolved to, for the clip checks below.
    let mut rigs: BTreeMap<&str, Option<&crate::rig::Rig>> = BTreeMap::new();
    let clip_list = |r: &crate::rig::Rig| -> String {
        let names = r.clip_names();
        if names.is_empty() {
            "none".to_string()
        } else {
            names
                .iter()
                .map(|n| format!("`{n}`"))
                .collect::<Vec<_>>()
                .join(", ")
        }
    };
    for (i, a) in quests.assemblies.iter().enumerate() {
        let at = format!("/content/assemblies/{i}");
        let resolved = match anchors.rig(&a.rig) {
            RigLookup::Unknown => None,
            RigLookup::Missing => {
                d.push(Diagnostic::error(
                    codes::ASSEMBLY_RIG,
                    "quests",
                    format!("{at}/rig"),
                    format!(
                        "assembly `{}` names rig `{}`, and the library holds no `{}/{}/{}` — a \
                         rig is a file a generator writes beside the prefab library, never \
                         campaign JSON. Run the generator that writes it, or name a rig the \
                         library holds",
                        a.id,
                        a.rig,
                        crate::rig::RIGS_DIR,
                        crate::l10n::local_id(a.rig.as_str()),
                        crate::rig::RIG_FILE,
                    ),
                ));
                None
            }
            RigLookup::Malformed(e) => {
                d.push(Diagnostic::error(
                    codes::ASSEMBLY_RIG,
                    "quests",
                    format!("{at}/rig"),
                    format!(
                        "assembly `{}` names rig `{}`, whose `{}` does not parse as a rig \
                         document: {e}. Regenerate it with the generator that wrote it",
                        a.id,
                        a.rig,
                        crate::rig::RIG_FILE,
                    ),
                ));
                None
            }
            RigLookup::Found(r) => {
                let issues = crate::rig::check(r);
                for issue in &issues {
                    d.push(Diagnostic::error(
                        codes::ASSEMBLY_RIG,
                        "quests",
                        format!("{at}/rig"),
                        format!(
                            "assembly `{}` names rig `{}`, which breaks a rig rule at `{}`: {}. \
                             Regenerate the rig with its generator",
                            a.id, a.rig, issue.field, issue.message
                        ),
                    ));
                }
                if issues.is_empty() { Some(r) } else { None }
            }
        };
        rigs.insert(a.id.as_str(), resolved);
        if let Some(r) = resolved {
            let mut need = |clip: &str, path: String, role: &str| {
                if r.clips.contains_key(clip) {
                    return;
                }
                d.push(Diagnostic::error(
                    codes::ASSEMBLY_RIG,
                    "quests",
                    path,
                    format!(
                        "assembly `{}` asks for clip `{clip}` as its {role}, and rig `{}` declares \
                         no such clip. Its clips are: {}",
                        a.id,
                        a.rig,
                        clip_list(r)
                    ),
                ));
            };
            if let Some(initial) = &a.initial {
                need(initial, format!("{at}/initial"), "`initial`");
            }
            let mut paced: Vec<Diagnostic> = Vec::new();
            if let Some(s) = &a.strikes {
                for (j, step) in s.pattern.iter().enumerate() {
                    need(
                        &step.windup,
                        format!("{at}/strikes/pattern/{j}/windup"),
                        "strike step's `windup`",
                    );
                    need(
                        &step.strike,
                        format!("{at}/strikes/pattern/{j}/strike"),
                        "strike step's `strike`",
                    );
                    if let Some(lock) = &step.lock {
                        for (k, reach) in lock.reaches.iter().enumerate() {
                            need(
                                reach,
                                format!("{at}/strikes/pattern/{j}/lock/reaches/{k}"),
                                "locked strike step's `reaches`",
                            );
                        }
                    }
                    if let Some(t) = step.ticks_per_frame
                        && !(crate::rig::MIN_TICKS_PER_FRAME..=crate::rig::MAX_TICKS_PER_FRAME)
                            .contains(&t)
                    {
                        paced.push(Diagnostic::error(
                            codes::ASSEMBLY_RIG,
                            "quests",
                            format!("{at}/strikes/pattern/{j}/ticks_per_frame"),
                            format!(
                                "assembly `{}`'s strike step {j} plays its clips at {t} tick(s) per \
                                 frame. A keyframe cadence is {} to {} — the bounds every rig clip \
                                 is held to. Choose a cadence in that range, or drop \
                                 `ticks_per_frame` to play each clip at its own",
                                a.id,
                                crate::rig::MIN_TICKS_PER_FRAME,
                                crate::rig::MAX_TICKS_PER_FRAME
                            ),
                        ));
                    }
                }
            }
            d.extend(paced);
        }
        if let Some(f) = station_kind_diag(
            &providers,
            a.at.anchor.as_str(),
            crate::layout::StationKind::Point,
            "an assembly's mark",
            "quests",
            format!("{at}/at/anchor"),
        ) {
            d.push(f);
        } else if !providers.resolvable(a.at.anchor.as_str()) {
            d.push(Diagnostic::error(
                codes::ANCHOR_UNRESOLVED,
                "quests",
                format!("{at}/at/anchor"),
                format!(
                    "assembly `{}` stands at anchor `{}`, which no area's prefab provides — {}",
                    a.id,
                    a.at.anchor,
                    providers.anchor_remedy(
                        "use an anchor a prefab exposes, or bind a prefab/pool that carries it"
                    ),
                ),
            ));
        }
        if let Some(s) = &a.strikes {
            lock_shape_checks(a, i, s, d);
            for (j, step) in s.pattern.iter().enumerate() {
                let Some(lock) = &step.lock else { continue };
                if providers.resolvable(lock.within.anchor.as_str()) {
                    continue;
                }
                d.push(Diagnostic::error(
                    codes::ANCHOR_UNRESOLVED,
                    "quests",
                    format!("{at}/strikes/pattern/{j}/lock/within/anchor"),
                    format!(
                        "assembly `{}`'s strike step {j} locks onto a player in a region centred \
                         on anchor `{}`, which no area's prefab provides — {}",
                        a.id,
                        lock.within.anchor,
                        providers.anchor_remedy(
                            "use an anchor a prefab exposes, or bind a prefab/pool that carries it"
                        ),
                    ),
                ));
            }
        }
        if let Some(s) = &a.strikes
            && !providers.resolvable(s.while_in.anchor.as_str())
        {
            d.push(Diagnostic::error(
                codes::ANCHOR_UNRESOLVED,
                "quests",
                format!("{at}/strikes/while_in/anchor"),
                format!(
                    "assembly `{}`'s arming region is centred on anchor `{}`, which no area's \
                     prefab provides — {}",
                    a.id,
                    s.while_in.anchor,
                    providers.anchor_remedy(
                        "use an anchor a prefab exposes, or bind a prefab/pool that carries it"
                    ),
                ),
            ));
        }
    }
    // Which declared assemblies strike, for `arm-strikes` (`DW0970`).
    let strikes: BTreeMap<&str, bool> = quests
        .assemblies
        .iter()
        .map(|a| (a.id.as_str(), a.strikes.is_some()))
        .collect();
    // Every verb that names an assembly, at every depth of every root.
    crate::for_each_campaign_effect(c, &mut |path, _site, e| {
        let Some((assembly, clip)) = assembly_verb(e) else {
            return;
        };
        if matches!(e.verb, Verb::ArmStrikes { .. }) && strikes.get(assembly) == Some(&false) {
            d.push(Diagnostic::error(
                codes::ASSEMBLY_ARM_NOTHING,
                "quests",
                format!("{path}/assembly"),
                format!(
                    "`arm-strikes` re-arms assembly `{assembly}`'s strike pattern, and the \
                     assembly declares no `strikes` — there is no pattern to re-arm, so the beat \
                     does nothing. Give the assembly a `strikes` pattern, or drop the effect"
                ),
            ));
        }
        let Some(resolved) = rigs.get(assembly) else {
            d.push(Diagnostic::error(
                codes::DANGLING_REF,
                "quests",
                path.to_string(),
                format!(
                    "`{}` names assembly `{assembly}`, which the stage-5 `assemblies` list does \
                     not declare — declare it, or fix the reference",
                    e.verb.tag()
                ),
            ));
            return;
        };
        if let (Some(clip), Some(r)) = (clip, resolved)
            && !r.clips.contains_key(clip)
        {
            d.push(Diagnostic::error(
                codes::ASSEMBLY_RIG,
                "quests",
                format!("{path}/clip"),
                format!(
                    "`play-clip` asks assembly `{assembly}` for clip `{clip}`, and its rig \
                     declares no such clip. Its clips are: {}",
                    clip_list(r)
                ),
            ));
        }
    });
    for (i, t) in quests.triggers.iter().enumerate() {
        if let Some(m) = t.on.assembly_target()
            && !rigs.contains_key(m.as_str())
        {
            d.push(Diagnostic::error(
                codes::DANGLING_REF,
                "quests",
                format!("/content/triggers/{i}/on/assembly"),
                format!(
                    "`strike-assembly` trigger `{}` targets assembly `{m}`, which the stage-5 \
                     `assemblies` list does not declare — use a declared assembly id",
                    t.id
                ),
            ));
        }
    }
}

/// The assembly a verb names, with the clip a `play-clip` asks for.
fn assembly_verb(e: &QuestEffect) -> Option<(&str, Option<&str>)> {
    match &e.verb {
        Verb::SpawnAssembly { assembly } | Verb::DespawnAssembly { assembly } => {
            Some((assembly.as_str(), None))
        }
        Verb::PlayClip { assembly, clip } => Some((assembly.as_str(), Some(clip.as_str()))),
        Verb::ArmStrikes { assembly } => Some((assembly.as_str(), None)),
        _ => None,
    }
}

/// spec-0094 §5.2 (`DW0969`): **a locked step's blow is the lock's to place.**
/// A locked step lands on the cells its chosen clip comes down on at the turn
/// it locked to, so a box an author writes cannot be where the blow lands: a
/// top-level `damage-players` in its `on_land` declares no `in`, no
/// `damage-players` stands inside another effect's list there, and a pattern
/// that turns by `aim` has no locked step.
fn lock_shape_checks(
    a: &crate::Assembly,
    i: usize,
    s: &crate::AssemblyStrikes,
    d: &mut Vec<Diagnostic>,
) {
    fn nested_damage(effs: &[QuestEffect], path: &str, out: &mut Vec<String>) {
        for (k, e) in effs.iter().enumerate() {
            let here = format!("{path}/{k}");
            if matches!(e.verb, Verb::DamagePlayers { .. }) {
                out.push(here.clone());
            }
            for (seg, _, list) in e.nested_effect_lists_labeled() {
                nested_damage(list, &format!("{here}/{seg}"), out);
            }
        }
    }
    let at = format!("/content/assemblies/{i}/strikes");
    for (j, step) in s.pattern.iter().enumerate() {
        if step.lock.is_none() {
            continue;
        }
        let here = format!("{at}/pattern/{j}");
        if s.aim.is_some() {
            d.push(Diagnostic::error(
                codes::ASSEMBLY_LOCK_SHAPE,
                "quests",
                format!("{here}/lock"),
                format!(
                    "assembly `{}`'s strike pattern turns by `aim`, and its step {j} declares a \
                     `lock` — two rules choosing the one turn the assembly strikes from. A locked \
                     step turns to the cell it locks onto; an aimed pattern turns to one of its \
                     facings. Drop `aim` from the pattern, or drop `lock` from the step",
                    a.id
                ),
            ));
        }
        for (k, e) in step.on_land.iter().enumerate() {
            let p = format!("{here}/on_land/{k}");
            if matches!(e.verb, Verb::DamagePlayers { .. }) && e.damage_within().is_some() {
                d.push(Diagnostic::error(
                    codes::ASSEMBLY_LOCK_SHAPE,
                    "quests",
                    format!("{p}/in"),
                    format!(
                        "assembly `{}`'s strike step {j} locks onto a player, and its \
                         `damage-players` ({p}) declares an `in` box. A locked blow lands on the \
                         cells its clip comes down on at the turn it locked to — the compiler \
                         derives that area for every cell it can lock, so a written box is a \
                         second, fixed answer that is wrong at every other cell. Drop the `in`",
                        a.id
                    ),
                ));
            }
            let mut deep = Vec::new();
            for (seg, _, list) in e.nested_effect_lists_labeled() {
                nested_damage(list, &format!("{p}/{seg}"), &mut deep);
            }
            for q in deep {
                d.push(Diagnostic::error(
                    codes::ASSEMBLY_LOCK_SHAPE,
                    "quests",
                    q.clone(),
                    format!(
                        "assembly `{}`'s strike step {j} locks onto a player, and a \
                         `damage-players` ({q}) stands inside another effect's list. Only a blow \
                         at the top of a locked step's `on_land` is moved to the cells the clip \
                         comes down on; this one would land nowhere the lock chose. Move it to the \
                         top of `on_land` (a `when` on it is kept)",
                        a.id
                    ),
                ));
            }
        }
    }
}

/// DSL v0.6 trap validation (spec-0011). Each trap binds to a **point anchor**
/// an area's prefab provides — any anchor, whatever it is called; a spec-0022
/// command `payload` needs that cell and nothing else of the piece, because the
/// compiler emits the detection. Structural failures are `DW0340` (a
/// malformed/duplicate id, an `at`/`disarm.via` no area's prefab provides, or a
/// `disarm.via` colliding with the trap's own trigger anchor); a dispense
/// payload item unknown to the pinned registry is `DW0341`. A trap's
/// `requires_flags` resolves against the declared-flag set like a trigger's
/// (`DW0172`). The completability obligation for a *lethal* trap is discharged
/// later by the compiler nav proof (`DW0342`).
fn v06_trap_checks(
    c: &Campaign,
    items: &dyn ItemRegistry,
    entities: &dyn EntityRegistry,
    anchors: &dyn AnchorRegistry,
    d: &mut Vec<Diagnostic>,
) {
    let quests = &c.quests.content;
    if quests.traps.is_empty() {
        return;
    }

    // Area anchor sets (single-prefab areas) + whether any pool area exists, so
    // resolution stays lenient for pool areas the compiler resolves later — the
    // same policy as the v0.4 trigger check.
    let providers = AnchorProviders::build(c, anchors);

    let flags = collect_declared_flags(c);

    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for (i, t) in quests.traps.iter().enumerate() {
        if !t.id.is_valid_syntax() {
            d.push(Diagnostic::error(
                codes::TRAP_INVALID,
                "quests",
                format!("/content/traps/{i}/id"),
                format!(
                    "malformed trap id `{}` — trap ids must be lowercase kebab-case with the \
                     `trap/` prefix (e.g. `trap/dart-hall`)",
                    t.id
                ),
            ));
        }
        if !seen.insert(t.id.as_str()) {
            d.push(Diagnostic::error(
                codes::TRAP_INVALID,
                "quests",
                format!("/content/traps/{i}/id"),
                format!(
                    "duplicate trap id `{}` — rename one so every trap id is unique",
                    t.id
                ),
            ));
        }
        if let Some(f) = station_kind_diag(
            &providers,
            t.at.as_str(),
            crate::layout::StationKind::Point,
            "a trap's `at`",
            "quests",
            format!("/content/traps/{i}/at"),
        ) {
            d.push(f);
        }
        if !providers.resolvable(t.at.as_str()) {
            d.push(Diagnostic::error(
                codes::TRAP_INVALID,
                "quests",
                format!("/content/traps/{i}/at"),
                format!(
                    "trap `at` anchor `{}` is not provided by any area's prefab — {}",
                    t.at,
                    providers.anchor_remedy(
                        "bind the trap to a point anchor some area's prefab exposes, whatever \
                         that anchor is called (names come from prefab metadata; do NOT invent \
                         one). A `payload` trap needs nothing of the piece but that one cell and \
                         the trigger block standing in it (`DW0917`) — the compiler emits the \
                         detection; only a legacy `dispense` effect \
                         needs the anchor's `dispenser` socket, and only a flag-gated trap \
                         needs its `trigger_block`"
                    ),
                ),
            ));
        }
        if let Some(dis) = &t.disarm {
            if let Some(f) = station_kind_diag(
                &providers,
                dis.via.as_str(),
                crate::layout::StationKind::Point,
                "a trap's disarm affordance",
                "quests",
                format!("/content/traps/{i}/disarm/via"),
            ) {
                d.push(f);
            }
            if !providers.resolvable(dis.via.as_str()) {
                d.push(Diagnostic::error(
                    codes::TRAP_INVALID,
                    "quests",
                    format!("/content/traps/{i}/disarm/via"),
                    format!(
                        "trap `disarm.via` anchor `{}` is not provided by any area's prefab — \
                         {}",
                        dis.via,
                        providers.anchor_remedy(
                            "use an anchor some area's prefab exposes for the disarm affordance"
                        ),
                    ),
                ));
            }
            if dis.via == t.at {
                d.push(Diagnostic::error(
                    codes::TRAP_INVALID,
                    "quests",
                    format!("/content/traps/{i}/disarm/via"),
                    format!(
                        "trap `disarm.via` anchor `{}` is the trap's own trigger anchor — the \
                         disarm must be a distinct, separately-reachable affordance, not the trap \
                         cell itself",
                        dis.via
                    ),
                ));
            }
        }
        // spec-0022: a trap must actually DO something. Neither the legacy
        // redstone `effect` nor a command `payload` means mute hardware that
        // the completability proofs would still reason about — a content
        // mistake, never a deliberate no-op.
        if t.effect.is_none() && t.payload.is_empty() {
            d.push(Diagnostic::error(
                codes::TRAP_NO_CONSEQUENCE,
                "quests",
                format!("/content/traps/{i}"),
                format!(
                    "trap `{}` declares no consequence — give it a `payload` (an ordered \
                     effect list: `volley`, `collapse`, `damage-players`, `play-sound`, \
                     `narrate`, `set-flag`, `spawn-wave`, …). A trigger with nothing \
                     downstream of it is scenery, not a trap",
                    t.id
                ),
            ));
        }
        // spec-0022 payload validation: the trap-payload verbs' own ids and
        // cadence, plus the standard flag/wave/item consumer resolution every
        // other effect root gets.
        for_each_trap_payload_deep(t, |path, eff| {
            let base = format!("/content/traps/{i}/{path}");
            match &eff.verb {
                Verb::Volley {
                    projectile,
                    salvos,
                    interval,
                    ..
                } => {
                    let proj = projectile
                        .as_deref()
                        .unwrap_or(crate::DEFAULT_VOLLEY_PROJECTILE);
                    if !entities.contains(proj) {
                        d.push(Diagnostic::error(
                            codes::TRAP_VERB_ID_UNKNOWN,
                            "quests",
                            format!("{base}/projectile"),
                            format!(
                                "volley `projectile` `{proj}` is not in the pinned 1.21.11 \
                                 entity registry — use a projectile entity id (e.g. \
                                 `minecraft:arrow`, `minecraft:spectral_arrow`)"
                            ),
                        ));
                    }
                    let n = salvos.unwrap_or(crate::DEFAULT_VOLLEY_SALVOS);
                    if n == 0 || n > crate::MAX_VOLLEY_SALVOS {
                        d.push(Diagnostic::error(
                            codes::VOLLEY_CADENCE,
                            "quests",
                            format!("{base}/salvos"),
                            format!(
                                "volley `salvos` is {n} — must be 1..={}. A volley fires \
                                 its whole kill zone every salvo, so the entity count is \
                                 `salvos x standable cells`; beyond the cap that is a \
                                 server hazard, not a trap",
                                crate::MAX_VOLLEY_SALVOS
                            ),
                        ));
                    }
                    let iv = interval.unwrap_or(crate::DEFAULT_VOLLEY_INTERVAL);
                    if iv == 0 || iv > crate::MAX_VOLLEY_INTERVAL {
                        d.push(Diagnostic::error(
                            codes::VOLLEY_CADENCE,
                            "quests",
                            format!("{base}/interval"),
                            format!(
                                "volley `interval` is {iv} ticks — must be 1..={}. Salvos \
                                 spaced wider than that stop reading as one trap event",
                                crate::MAX_VOLLEY_INTERVAL
                            ),
                        ));
                    }
                }
                Verb::Collapse {
                    falling_block,
                    then_floor,
                    ..
                } => {
                    let blocks = ItemBackedBlockRegistry::new(items);
                    let fb = falling_block
                        .as_deref()
                        .unwrap_or(crate::DEFAULT_COLLAPSE_FALLING_BLOCK);
                    for (field, id) in [
                        ("falling_block", Some(fb)),
                        ("then_floor", then_floor.as_deref()),
                    ] {
                        let Some(id) = id else { continue };
                        if !blocks.contains(id) {
                            d.push(Diagnostic::error(
                                codes::TRAP_VERB_ID_UNKNOWN,
                                "quests",
                                format!("{base}/{field}"),
                                format!(
                                    "collapse `{field}` `{id}` is not in the pinned 1.21.11 \
                                     block registry — use a placeable block id (e.g. \
                                     `minecraft:gravel`, `minecraft:sand`)"
                                ),
                            ));
                        }
                    }
                }
                _ => {}
            }
            for (kind, list) in [
                ("requires_flags", eff.requires_flags()),
                ("forbids_flags", eff.forbids_flags()),
            ] {
                for (n, f) in list.iter().enumerate() {
                    if !flags.contains(f.as_str()) {
                        d.push(Diagnostic::error(
                            codes::FLAG_UNKNOWN,
                            "quests",
                            format!("{base}/{kind}/{n}"),
                            format!(
                                "trap payload effect `{kind}` references flag `{f}`, which no \
                                 `set-flag` effect ever produces — add the producing \
                                 `set-flag {{ flag: \"{f}\" }}`, or correct the flag name"
                            ),
                        ));
                    }
                }
            }
            if let Some(w) = eff.spawn_wave()
                && !c.quests.content.waves.iter().any(|x| x.id == *w)
            {
                d.push(Diagnostic::error(
                    codes::WAVE_UNKNOWN,
                    "quests",
                    format!("{base}/wave"),
                    format!("trap payload `spawn-wave` references unknown wave `{w}`"),
                ));
            }
            if let Some(item) = eff.give_item()
                && !items.contains(item)
            {
                d.push(Diagnostic::error(
                    codes::ITEM_UNKNOWN,
                    "quests",
                    format!("{base}/item"),
                    format!(
                        "trap payload `give-item` item `{item}` is not in the pinned \
                         1.21.11 item registry"
                    ),
                ));
            }
        });
        if let Some((item, count)) = t.dispense() {
            if !items.contains(item) {
                d.push(Diagnostic::error(
                    codes::TRAP_PAYLOAD_UNKNOWN,
                    "quests",
                    format!("/content/traps/{i}/effect/dispense/item"),
                    format!(
                        "trap dispense payload item `{item}` is not in the pinned 1.21.11 item \
                         registry — use a valid namespaced item id (e.g. `minecraft:arrow`)"
                    ),
                ));
            }
            // The dispenser payload is the same single-slot `item replace …
            // container.0` fill a `loot` entry is, so it carries the same silent
            // over-cap failure (`DW0436`) — a splash potion caps at 1.
            check_stack_count(
                item,
                count,
                &format!("trap `{}` dispense payload", t.id),
                format!("/content/traps/{i}/effect/dispense/count"),
                items,
                d,
            );
        }
        for (m, f) in t.requires_flags.iter().enumerate() {
            if !flags.contains(f.as_str()) {
                d.push(Diagnostic::error(
                    codes::FLAG_UNKNOWN,
                    "quests",
                    format!("/content/traps/{i}/requires_flags/{m}"),
                    format!(
                        "trap `requires_flags` references flag `{f}`, which no `set-flag` effect or \
                         trap disarm ever produces — add a producer or correct the flag name"
                    ),
                ));
            }
        }
        // Trap `forbids_flags` — same unknown-flag treatment (DW0172).
        for (m, f) in t.forbids_flags.iter().enumerate() {
            if !flags.contains(f.as_str()) {
                d.push(Diagnostic::error(
                    codes::FLAG_UNKNOWN,
                    "quests",
                    format!("/content/traps/{i}/forbids_flags/{m}"),
                    format!(
                        "trap `forbids_flags` references flag `{f}`, which no `set-flag` effect or \
                         trap disarm ever produces — the gate can never suppress anything; add a \
                         producer or correct the flag name"
                    ),
                ));
            }
        }
    }
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

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Stage 7 — world-edits (the map-editor edit script, DSL v0.6, spec-0017)
// ---------------------------------------------------------------------------

/// Structural validation of the stage-7 edit script: id syntax/uniqueness
/// (`DW0110`/`DW0111`), area refs (`DW0112`), strictly-backward region refs and
/// shape/recipe well-formedness (`DW0162`) and block ids (`DW0193`).
/// Frame/region *resolution* against the solved layout
/// is the compiler's job (`DW0323`) — validation never needs prefabs.
fn world_edits_checks(c: &Campaign, blocks: &dyn BlockRegistry, d: &mut Vec<Diagnostic>) {
    let Some(env) = &c.world_edits else {
        return;
    };
    let stage = Stage::WorldEdits.name();

    let mut areas: BTreeSet<&str> = c
        .world
        .content
        .areas
        .iter()
        .map(|a| a.id.as_str())
        .collect();
    // A site-plan campaign has no `areas[]` — `DW0839` refuses one that does —
    // and exactly one place instead: the site the plan lays out. A batch names
    // it like any other area, so it is a declared area id here for the same
    // reason `areas[]` entries are.
    //
    // The third of three area sets in this file, and the only one that used to
    // omit this. The pair it made was unsatisfiable: `DW0839` REQUIRES a
    // site-plan campaign to declare no `areas[]`, and every batch of a stage-7
    // edit script was then checked against a set that could only be empty. So no
    // site-plan campaign could carry an edit script at all, and the repair the
    // message prescribes — use one of the world stage's area ids — names a set
    // the other rule guarantees is empty. Each gate was right on its own terms;
    // the union had no green state. What it cost is every build-tier check a
    // stage-7 script is the only route to: content could not reach them from a
    // site-plan campaign at all.
    if c.site_plan.is_some() {
        areas.insert(crate::siteplan::SITE_AREA);
    }

    // Small helpers, each pushing at most one diagnostic.
    fn bad_syntax(d: &mut Vec<Diagnostic>, stage: &str, path: String, what: &str, id: &str) {
        d.push(Diagnostic::error(
            codes::ID_SYNTAX,
            stage,
            path,
            format!(
                "malformed {what} id `{id}` (expected `{}`)",
                what_pattern(what)
            ),
        ));
    }
    fn what_pattern(what: &str) -> String {
        format!("{what}/<kebab>")
    }
    fn check_region_ref(
        d: &mut Vec<Diagnostic>,
        stage: &str,
        regions: &BTreeSet<&str>,
        path: String,
        r: &crate::ids::RegionId,
    ) {
        if !r.is_valid_syntax() {
            bad_syntax(d, stage, path, "region", r.as_str());
        } else if !regions.contains(r.as_str()) {
            d.push(Diagnostic::error(
                codes::EDIT_INVALID,
                stage,
                path,
                format!(
                    "region `{r}` is not defined by an earlier `select` in this batch — every \
                     region reference is strictly backward within its batch; add a `select` verb \
                     naming `{r}` above this edit (or fix the name)"
                ),
            ));
        }
    }
    fn check_recipe(
        d: &mut Vec<Diagnostic>,
        stage: &str,
        blocks: &dyn BlockRegistry,
        path: &str,
        recipe: &crate::PaletteRecipe,
    ) {
        if recipe.blocks.is_empty() {
            d.push(Diagnostic::error(
                codes::EDIT_INVALID,
                stage,
                format!("{path}/blocks"),
                "palette recipe has no entries — give it at least one weighted block (and \
                 prefer ≥ 2 so the seeded noise reads as natural variation, never a uniform \
                 fill)"
                    .to_string(),
            ));
        }
        for (i, b) in recipe.blocks.iter().enumerate() {
            if !(b.weight.is_finite() && b.weight > 0.0) {
                d.push(Diagnostic::error(
                    codes::EDIT_INVALID,
                    stage,
                    format!("{path}/blocks/{i}/weight"),
                    format!(
                        "palette weight `{}` for `{}` must be a finite number > 0",
                        b.weight, b.block
                    ),
                ));
            }
            check_edit_block(
                d,
                stage,
                blocks,
                format!("{path}/blocks/{i}/block"),
                &b.block,
            );
        }
        if let Some(scale) = recipe.scale
            && !(scale.is_finite() && scale > 0.0)
        {
            d.push(Diagnostic::error(
                codes::EDIT_INVALID,
                stage,
                format!("{path}/scale"),
                format!("recipe `scale` `{scale}` must be a finite number > 0 (blocks⁻¹)"),
            ));
        }
    }
    fn check_edit_block(
        d: &mut Vec<Diagnostic>,
        stage: &str,
        blocks: &dyn BlockRegistry,
        path: String,
        block: &str,
    ) {
        match split_blockstate(block) {
            Ok(base) => {
                if !blocks.contains(base) {
                    d.push(Diagnostic::error(
                        codes::BLOCK_UNKNOWN,
                        stage,
                        path,
                        format!(
                            "block `{block}` is not a known 1.21.11 block id — use a valid \
                             namespaced block id (e.g. `minecraft:mossy_stone_bricks`)"
                        ),
                    ));
                }
            }
            Err(reason) => {
                d.push(Diagnostic::error(codes::BLOCK_UNKNOWN, stage, path, reason));
            }
        }
    }

    // A verb's phase: L2 massing (applied at plan time, over the jigsaw
    // layout) vs L3 detailing (applied at replay time, over the assembled
    // blocks). A batch never mixes phases, and every massing batch precedes
    // every detailing batch — the replay applies all massing first by
    // construction, so an interleaved script would misrepresent its own order.
    fn is_massing(edit: &WorldEdit) -> bool {
        matches!(
            edit,
            WorldEdit::SwapPiece { .. }
                | WorldEdit::InsertPiece { .. }
                | WorldEdit::RemovePiece { .. }
                | WorldEdit::RewireSocket { .. }
                | WorldEdit::ReseedPiece { .. }
        )
    }

    let mut batch_ids: BTreeSet<&str> = BTreeSet::new();
    let mut seen_detailing = false;
    for (bi, batch) in env.content.batches.iter().enumerate() {
        let bpath = format!("/batches/{bi}");
        let massing_count = batch.edits.iter().filter(|e| is_massing(e)).count();
        if massing_count > 0 && massing_count < batch.edits.len() {
            d.push(Diagnostic::error(
                codes::EDIT_INVALID,
                stage,
                format!("{bpath}/edits"),
                format!(
                    "batch `{}` mixes L2 massing and L3 detailing verbs — massing applies at \
                     plan time (before assembly), detailing at replay time, so a mixed batch \
                     cannot execute in its written order. Split it into a massing batch and a \
                     detailing batch",
                    batch.id
                ),
            ));
        }
        if massing_count > 0 && seen_detailing {
            d.push(Diagnostic::error(
                codes::EDIT_INVALID,
                stage,
                bpath.to_string(),
                format!(
                    "massing batch `{}` follows a detailing batch — every massing batch must \
                     precede every detailing batch (massing reshapes the layout the detailing \
                     verbs' frames resolve against). Move it up the script",
                    batch.id
                ),
            ));
        }
        if massing_count == 0 && !batch.edits.is_empty() {
            seen_detailing = true;
        }
        if !batch.id.is_valid_syntax() {
            bad_syntax(d, stage, format!("{bpath}/id"), "batch", batch.id.as_str());
        } else if !batch_ids.insert(batch.id.as_str()) {
            d.push(Diagnostic::error(
                codes::ID_DUPLICATE,
                stage,
                format!("{bpath}/id"),
                format!(
                    "duplicate batch id `{}` — batch ids are unique across the edit script \
                     (they name snapshots and seed streams)",
                    batch.id
                ),
            ));
        }
        if !areas.contains(batch.area.as_str()) {
            d.push(Diagnostic::error(
                codes::DANGLING_REF,
                stage,
                format!("{bpath}/area"),
                format!(
                    "batch `{}` targets area `{}` which this campaign does not declare — {}",
                    batch.id,
                    batch.area,
                    crate::placement::Placement::of(c).area_remedy(),
                ),
            ));
        }

        // Regions defined so far in THIS batch (strictly backward references).
        let mut regions: BTreeSet<&str> = BTreeSet::new();
        for (ei, edit) in batch.edits.iter().enumerate() {
            let epath = format!("{bpath}/edits/{ei}");
            match edit {
                WorldEdit::Select { name, shape } => {
                    match shape {
                        RegionShape::Box { frame, min, max } => {
                            if min.iter().zip(max).any(|(lo, hi)| lo > hi) {
                                d.push(Diagnostic::error(
                                    codes::EDIT_INVALID,
                                    stage,
                                    format!("{epath}/shape"),
                                    format!(
                                        "box region `{name}` has min {min:?} > max {max:?} on \
                                         an axis — corners are inclusive with min ≤ max per axis"
                                    ),
                                ));
                            }
                            match frame {
                                EditFrame::PieceLocal { prefab, .. } => {
                                    if !prefab.is_valid_syntax() {
                                        bad_syntax(
                                            d,
                                            stage,
                                            format!("{epath}/shape/frame/prefab"),
                                            "prefab",
                                            prefab.as_str(),
                                        );
                                    }
                                }
                                EditFrame::AnchorRelative { anchor } => {
                                    if !anchor.is_valid_syntax() {
                                        bad_syntax(
                                            d,
                                            stage,
                                            format!("{epath}/shape/frame/anchor"),
                                            "anchor",
                                            anchor.as_str(),
                                        );
                                    }
                                }
                            }
                        }
                        RegionShape::SurfaceBand { over, from, to } => {
                            check_region_ref(
                                d,
                                stage,
                                &regions,
                                format!("{epath}/shape/over"),
                                over,
                            );
                            if from > to {
                                d.push(Diagnostic::error(
                                    codes::EDIT_INVALID,
                                    stage,
                                    format!("{epath}/shape"),
                                    format!(
                                        "surface band `{name}` has from {from} > to {to} — the \
                                         band is inclusive with from ≤ to (offsets relative to \
                                         each column's surface)"
                                    ),
                                ));
                            }
                        }
                        RegionShape::PaletteMatch { within, blocks: bl } => {
                            check_region_ref(
                                d,
                                stage,
                                &regions,
                                format!("{epath}/shape/within"),
                                within,
                            );
                            if bl.is_empty() {
                                d.push(Diagnostic::error(
                                    codes::EDIT_INVALID,
                                    stage,
                                    format!("{epath}/shape/blocks"),
                                    format!(
                                        "palette-match region `{name}` lists no blocks — name \
                                         at least one base block id to match"
                                    ),
                                ));
                            }
                            for (i, b) in bl.iter().enumerate() {
                                check_edit_block(
                                    d,
                                    stage,
                                    blocks,
                                    format!("{epath}/shape/blocks/{i}"),
                                    b,
                                );
                            }
                        }
                        RegionShape::Union { of } | RegionShape::Intersect { of } => {
                            if of.len() < 2 {
                                d.push(Diagnostic::error(
                                    codes::EDIT_INVALID,
                                    stage,
                                    format!("{epath}/shape/of"),
                                    format!(
                                        "composition region `{name}` lists {} region(s) — a \
                                         union/intersection needs at least 2 (a single-region \
                                         composition is just the region; use it directly)",
                                        of.len()
                                    ),
                                ));
                            }
                            for (i, r) in of.iter().enumerate() {
                                check_region_ref(
                                    d,
                                    stage,
                                    &regions,
                                    format!("{epath}/shape/of/{i}"),
                                    r,
                                );
                            }
                        }
                        RegionShape::Subtract { base, remove } => {
                            check_region_ref(
                                d,
                                stage,
                                &regions,
                                format!("{epath}/shape/base"),
                                base,
                            );
                            if remove.is_empty() {
                                d.push(Diagnostic::error(
                                    codes::EDIT_INVALID,
                                    stage,
                                    format!("{epath}/shape/remove"),
                                    format!(
                                        "subtract region `{name}` removes nothing — list at \
                                         least one region to subtract (or use `base` directly)"
                                    ),
                                ));
                            }
                            for (i, r) in remove.iter().enumerate() {
                                check_region_ref(
                                    d,
                                    stage,
                                    &regions,
                                    format!("{epath}/shape/remove/{i}"),
                                    r,
                                );
                            }
                        }
                    }
                    if !name.is_valid_syntax() {
                        bad_syntax(d, stage, format!("{epath}/name"), "region", name.as_str());
                    } else if !regions.insert(name.as_str()) {
                        d.push(Diagnostic::error(
                            codes::ID_DUPLICATE,
                            stage,
                            format!("{epath}/name"),
                            format!(
                                "duplicate region name `{name}` in batch `{}` — region names \
                                 are unique within their batch",
                                batch.id
                            ),
                        ));
                    }
                }
                WorldEdit::Fill { region, recipe } => {
                    check_region_ref(d, stage, &regions, format!("{epath}/region"), region);
                    check_recipe(d, stage, blocks, &format!("{epath}/recipe"), recipe);
                }
                WorldEdit::Replace {
                    region,
                    matching,
                    recipe,
                } => {
                    check_region_ref(d, stage, &regions, format!("{epath}/region"), region);
                    if matching.is_empty() {
                        d.push(Diagnostic::error(
                            codes::EDIT_INVALID,
                            stage,
                            format!("{epath}/matching"),
                            "replace matches no blocks — list at least one base block id to \
                             rewrite (an unconditional rewrite is `fill`)"
                                .to_string(),
                        ));
                    }
                    for (i, b) in matching.iter().enumerate() {
                        check_edit_block(d, stage, blocks, format!("{epath}/matching/{i}"), b);
                    }
                    check_recipe(d, stage, blocks, &format!("{epath}/recipe"), recipe);
                }
                WorldEdit::Carve { region } => {
                    check_region_ref(d, stage, &regions, format!("{epath}/region"), region);
                }
                WorldEdit::Morph { region, op } => {
                    check_region_ref(d, stage, &regions, format!("{epath}/region"), region);
                    match op {
                        MorphOp::Raise { by, recipe } => {
                            if *by == 0 {
                                d.push(Diagnostic::error(
                                    codes::EDIT_INVALID,
                                    stage,
                                    format!("{epath}/op/by"),
                                    "morph raise `by` is 0 — a zero raise is a no-op; give a \
                                     positive height (or drop the edit)"
                                        .to_string(),
                                ));
                            }
                            check_recipe(d, stage, blocks, &format!("{epath}/op/recipe"), recipe);
                        }
                        MorphOp::Lower { by } => {
                            if *by == 0 {
                                d.push(Diagnostic::error(
                                    codes::EDIT_INVALID,
                                    stage,
                                    format!("{epath}/op/by"),
                                    "morph lower `by` is 0 — a zero lower is a no-op; give a \
                                     positive depth (or drop the edit)"
                                        .to_string(),
                                ));
                            }
                        }
                        MorphOp::Smooth { passes, recipe } => {
                            if *passes == 0 {
                                d.push(Diagnostic::error(
                                    codes::EDIT_INVALID,
                                    stage,
                                    format!("{epath}/op/passes"),
                                    "morph smooth `passes` is 0 — a zero-pass smooth is a \
                                     no-op; give a positive pass count (or drop the edit)"
                                        .to_string(),
                                ));
                            }
                            check_recipe(d, stage, blocks, &format!("{epath}/op/recipe"), recipe);
                        }
                    }
                }
                WorldEdit::Scatter {
                    region,
                    items,
                    density,
                    avoid,
                    spacing: _,
                    limit,
                } => {
                    check_region_ref(d, stage, &regions, format!("{epath}/region"), region);
                    for (i, r) in avoid.iter().enumerate() {
                        check_region_ref(d, stage, &regions, format!("{epath}/avoid/{i}"), r);
                    }
                    if items.is_empty() {
                        d.push(Diagnostic::error(
                            codes::EDIT_INVALID,
                            stage,
                            format!("{epath}/items"),
                            "scatter has no items — give it at least one weighted dressing \
                             block"
                                .to_string(),
                        ));
                    }
                    for (i, b) in items.iter().enumerate() {
                        if !(b.weight.is_finite() && b.weight > 0.0) {
                            d.push(Diagnostic::error(
                                codes::EDIT_INVALID,
                                stage,
                                format!("{epath}/items/{i}/weight"),
                                format!(
                                    "scatter item weight `{}` for `{}` must be a finite \
                                     number > 0",
                                    b.weight, b.block
                                ),
                            ));
                        }
                        check_edit_block(
                            d,
                            stage,
                            blocks,
                            format!("{epath}/items/{i}/block"),
                            &b.block,
                        );
                    }
                    if !(density.is_finite() && *density > 0.0 && *density <= 1.0) {
                        d.push(Diagnostic::error(
                            codes::EDIT_INVALID,
                            stage,
                            format!("{epath}/density"),
                            format!(
                                "scatter `density` `{density}` must be in (0, 1] — it is the \
                                 per-candidate placement probability"
                            ),
                        ));
                    }
                    if let Some(limit) = limit
                        && *limit == 0
                    {
                        d.push(Diagnostic::error(
                            codes::EDIT_INVALID,
                            stage,
                            format!("{epath}/limit"),
                            "scatter `limit` is 0 — a zero-item scatter is a no-op; give a \
                             positive cap (or drop the field for no cap)"
                                .to_string(),
                        ));
                    }
                }
                WorldEdit::Plant {
                    region,
                    tree: _,
                    count,
                    avoid,
                    spacing: _,
                } => {
                    check_region_ref(d, stage, &regions, format!("{epath}/region"), region);
                    for (i, r) in avoid.iter().enumerate() {
                        check_region_ref(d, stage, &regions, format!("{epath}/avoid/{i}"), r);
                    }
                    if *count == 0 {
                        d.push(Diagnostic::error(
                            codes::EDIT_INVALID,
                            stage,
                            format!("{epath}/count"),
                            "plant `count` is 0 — a zero-tree plant is a no-op; give a \
                             positive count (or drop the edit)"
                                .to_string(),
                        ));
                    }
                }
                WorldEdit::Fragment {
                    prefab,
                    frame,
                    at: _,
                    rotation: _,
                } => {
                    if !prefab.is_valid_syntax() {
                        bad_syntax(
                            d,
                            stage,
                            format!("{epath}/prefab"),
                            "prefab",
                            prefab.as_str(),
                        );
                    }
                    match frame {
                        EditFrame::PieceLocal { prefab, .. } => {
                            if !prefab.is_valid_syntax() {
                                bad_syntax(
                                    d,
                                    stage,
                                    format!("{epath}/frame/prefab"),
                                    "prefab",
                                    prefab.as_str(),
                                );
                            }
                        }
                        EditFrame::AnchorRelative { anchor } => {
                            if !anchor.is_valid_syntax() {
                                bad_syntax(
                                    d,
                                    stage,
                                    format!("{epath}/frame/anchor"),
                                    "anchor",
                                    anchor.as_str(),
                                );
                            }
                        }
                    }
                }
                WorldEdit::Relight {
                    region,
                    fixture,
                    min_light,
                } => {
                    check_region_ref(d, stage, &regions, format!("{epath}/region"), region);
                    if let Some(ml) = min_light
                        && !(1..=14).contains(ml)
                    {
                        d.push(Diagnostic::error(
                            codes::EDIT_INVALID,
                            stage,
                            format!("{epath}/min_light"),
                            format!(
                                "relight `min_light` {ml} out of range — vanilla block light \
                                 is 1..=14 (15 is only at the emitter itself)"
                            ),
                        ));
                    }
                    // Without an area `lighting` declaration the verb has no
                    // fixture/target to fall back on — both overrides required.
                    let area_lighting = c
                        .world
                        .content
                        .areas
                        .iter()
                        .find(|a| a.id.as_str() == batch.area.as_str())
                        .and_then(|a| a.lighting);
                    if area_lighting.is_none() && (fixture.is_none() || min_light.is_none()) {
                        d.push(Diagnostic::error(
                            codes::EDIT_INVALID,
                            stage,
                            epath.to_string(),
                            format!(
                                "relight in batch `{}`: area `{}` declares no `lighting`, so \
                                 the verb must carry BOTH `fixture` and `min_light` (there is \
                                 nothing to default to). Declare area lighting or add the \
                                 overrides",
                                batch.id, batch.area
                            ),
                        ));
                    }
                }
                WorldEdit::SwapPiece {
                    piece: _,
                    prefab,
                    with,
                } => {
                    for (what, id) in [("prefab", prefab.as_str()), ("prefab", with.as_str())] {
                        if !crate::ids::is_prefixed(id, "prefab") {
                            bad_syntax(d, stage, epath.to_string(), what, id);
                        }
                    }
                }
                WorldEdit::InsertPiece {
                    at_piece: _,
                    prefab,
                    socket: _,
                    insert,
                } => {
                    for id in [prefab.as_str(), insert.as_str()] {
                        if !crate::ids::is_prefixed(id, "prefab") {
                            bad_syntax(d, stage, epath.to_string(), "prefab", id);
                        }
                    }
                }
                WorldEdit::RemovePiece { piece: _, prefab }
                | WorldEdit::ReseedPiece { piece: _, prefab }
                | WorldEdit::RewireSocket { prefab, .. } => {
                    if !prefab.is_valid_syntax() {
                        bad_syntax(
                            d,
                            stage,
                            format!("{epath}/prefab"),
                            "prefab",
                            prefab.as_str(),
                        );
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// spec-0016 §2 — shortcut doors
// ---------------------------------------------------------------------------

/// Validate the stage-5 `shortcuts` section (spec-0016 §2).
///
/// Two rules, two codes:
/// * `DW0371` — the declaration must resolve: a well-formed, unique
///   `shortcut/<id>`, a `gate` and an `unlock` some area's prefab provides, and
///   the two must be different anchors (the mechanism sits on the FAR side, not
///   in the doorway it opens).
/// * `DW0372` — no `close-gate` anywhere may target a gate a shortcut owns. A
///   shortcut opens permanently; making that structural is cheaper and safer than
///   trusting every author to never reach for the re-seal verb. `close-gate` on
///   any other gate (the point-of-no-return beat) is untouched.
///
/// Anchor resolution stays lenient for pool areas the compiler resolves later —
/// the same policy as the trap and trigger checks.
fn shortcut_checks(c: &Campaign, anchors: &dyn AnchorRegistry, d: &mut Vec<Diagnostic>) {
    let quests = &c.quests.content;
    if quests.shortcuts.is_empty() {
        return;
    }
    let providers = AnchorProviders::build(c, anchors);

    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for (i, sc) in quests.shortcuts.iter().enumerate() {
        if !sc.id.is_valid_syntax() {
            d.push(Diagnostic::error(
                codes::SHORTCUT_INVALID,
                "quests",
                format!("/content/shortcuts/{i}/id"),
                format!(
                    "malformed shortcut id `{}` — shortcut ids must be lowercase kebab-case with \
                     the `shortcut/` prefix (e.g. `shortcut/keep-lift`)",
                    sc.id
                ),
            ));
        }
        if !seen.insert(sc.id.as_str()) {
            d.push(Diagnostic::error(
                codes::SHORTCUT_INVALID,
                "quests",
                format!("/content/shortcuts/{i}/id"),
                format!(
                    "duplicate shortcut id `{}` — rename one so every shortcut id is unique",
                    sc.id
                ),
            ));
        }
        // A shortcut's two anchors are two SHAPES: the `gate` is the region it
        // clears on unlock, and the `unlock` is the cell its far-side affordance
        // stands on. They travelled together through one resolvable() call
        // because a name was all either needed.
        for (field, anchor, demands) in [
            ("gate", &sc.gate, crate::layout::StationKind::Gate),
            ("unlock", &sc.unlock, crate::layout::StationKind::Point),
        ] {
            if let Some(f) = station_kind_diag(
                &providers,
                anchor.as_str(),
                demands,
                &format!("a shortcut's `{field}`"),
                "quests",
                format!("/content/shortcuts/{i}/{field}"),
            ) {
                d.push(f);
                continue;
            }
            if !providers.resolvable(anchor.as_str()) {
                d.push(Diagnostic::error(
                    codes::SHORTCUT_INVALID,
                    "quests",
                    format!("/content/shortcuts/{i}/{field}"),
                    format!(
                        "shortcut `{field}` anchor `{anchor}` is not provided by any area's \
                         prefab — {}",
                        providers.anchor_remedy(
                            "use an anchor a prefab exposes (anchor names come from prefab \
                             metadata; do NOT invent one)"
                        ),
                    ),
                ));
            }
        }
        if sc.gate == sc.unlock {
            d.push(Diagnostic::error(
                codes::SHORTCUT_INVALID,
                "quests",
                format!("/content/shortcuts/{i}/unlock"),
                format!(
                    "shortcut `{}` unlocks at its own gate anchor `{}` — the mechanism belongs on \
                     the FAR side of the door you have not opened yet, which is the entire point \
                     of the pattern (spec-0016 §2)",
                    sc.id, sc.gate
                ),
            ));
        }
    }

    // `close-gate` may never target a shortcut gate: permanence is structural.
    let owned: BTreeSet<&str> = quests.shortcuts.iter().map(|s| s.gate.as_str()).collect();
    let report = |path: String, anchor: &str, d: &mut Vec<Diagnostic>| {
        d.push(Diagnostic::error(
            codes::SHORTCUT_RESEALED,
            "quests",
            path,
            format!(
                "`close-gate` targets `{anchor}`, a gate a `shortcut` owns — a shortcut opens \
                 PERMANENTLY (spec-0016 §2), so nothing may re-seal it. Use a different gate for \
                 the point-of-no-return beat, or drop the shortcut declaration."
            ),
        ));
    };
    for (qi, q) in quests.quests.iter().enumerate() {
        for_each_effect_deep(q, |path, eff| {
            if let Some(a) = eff.close_gate_anchor()
                && owned.contains(a.as_str())
            {
                report(format!("/content/quests/{qi}/{path}/anchor"), a.as_str(), d);
            }
        });
    }
    for (ti, t) in quests.triggers.iter().enumerate() {
        for_each_trigger_effect_deep(t, |path, eff| {
            if let Some(a) = eff.close_gate_anchor()
                && owned.contains(a.as_str())
            {
                report(
                    format!("/content/triggers/{ti}/{path}/anchor"),
                    a.as_str(),
                    d,
                );
            }
        });
    }
}

// ---------------------------------------------------------------------------
// spec-0016 §3 — ambushes
// ---------------------------------------------------------------------------

/// Validate the stage-5 `ambushes` section (spec-0016 §3), `DW0375`.
///
/// An ambush desugars to an ordinary environment trigger at parse time, so it
/// inherits every trigger diagnostic already in the compiler — id/range checks
/// (`DW0194`), anchor resolution, unknown actor refs, the `use`-on-an-NPC rule
/// (`DW0350`). This function only owns what the sugar itself can get wrong:
/// its own id, and an actor list that does not actually stage an ambush.
///
/// It deliberately does **not** require a `telegraph`. The un-telegraphed
/// ambush is core souls vocabulary — 初见杀 is how a
/// level teaches. What the engine owes the player is counterplay on the retry,
/// which is a geometric question and is proven in `compiler::nav` (`DW0376`).
fn ambush_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for (i, a) in c.quests.content.ambushes.iter().enumerate() {
        if !a.id.is_valid_syntax() {
            d.push(Diagnostic::error(
                codes::AMBUSH_INVALID,
                "quests",
                format!("/content/ambushes/{i}/id"),
                format!(
                    "malformed ambush id `{}` — ambush ids must be lowercase kebab-case with the \
                     `ambush/` prefix (e.g. `ambush/stair-turn`)",
                    a.id
                ),
            ));
        }
        if !seen.insert(a.id.as_str()) {
            d.push(Diagnostic::error(
                codes::AMBUSH_INVALID,
                "quests",
                format!("/content/ambushes/{i}/id"),
                format!(
                    "duplicate ambush id `{}` — rename one so every ambush id is unique (each \
                     desugars to a trigger named after it)",
                    a.id
                ),
            ));
        }
        if a.actors.is_empty() {
            d.push(Diagnostic::error(
                codes::AMBUSH_INVALID,
                "quests",
                format!("/content/ambushes/{i}/actors"),
                format!(
                    "ambush `{}` lists no actors — it would spring nothing. List the actors that \
                     ambush the player, or delete the declaration; a beat that fires and does \
                     nothing is never what was meant.",
                    a.id
                ),
            ));
        }
        let mut dup: BTreeSet<&str> = BTreeSet::new();
        for (j, actor) in a.actors.iter().enumerate() {
            if !dup.insert(actor.as_str()) {
                d.push(Diagnostic::error(
                    codes::AMBUSH_INVALID,
                    "quests",
                    format!("/content/ambushes/{i}/actors/{j}"),
                    format!(
                        "ambush `{}` lists actor `{actor}` twice — `spawn-actor` is idempotent, so \
                         the second one is a silent no-op and the ambush is half the size it \
                         reads as. Declare a second actor instead.",
                        a.id
                    ),
                ));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// spec-0016 §4 — timed gates
// ---------------------------------------------------------------------------

/// Validate the stage-5 `timed_gates` section (spec-0016 §4), `DW0377` /
/// `DW0389`.
///
/// The structural half only: ids, a cycle that actually cycles, a phase inside
/// the cycle, one owner per gate region, and a `disarm.via` that
/// resolves to a real anchor outside the span it jams. The *design* half — that
/// the gate is a timing read and not a coin flip — needs the nav model's crossing
/// time and lives in `compiler::nav` (`DW0378`). The fill-block requirement is
/// `DW0343`, the same rule `close-gate` and `shortcut` obey.
///
/// `DW0389` is the permanence rule, and it is the exact mirror of a shortcut's
/// `DW0372`: a disarmed gate rests OPEN forever, so no `close-gate` anywhere may
/// name it. Making that structural is cheaper and safer than trusting every
/// author never to reach for the re-seal verb.
///
/// Anchor resolution stays lenient for pool areas the compiler resolves later —
/// the same policy as the trap and shortcut checks.
fn timed_gate_checks(c: &Campaign, anchors: &dyn AnchorRegistry, d: &mut Vec<Diagnostic>) {
    let quests = &c.quests.content;
    if quests.timed_gates.is_empty() {
        return;
    }
    let providers = AnchorProviders::build(c, anchors);
    let shortcut_gates: BTreeSet<&str> = quests.shortcuts.iter().map(|s| s.gate.as_str()).collect();
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut driven: BTreeSet<&str> = BTreeSet::new();
    for (i, g) in quests.timed_gates.iter().enumerate() {
        let err = |path: String, msg: String, d: &mut Vec<Diagnostic>| {
            d.push(Diagnostic::error(
                codes::TIMED_GATE_INVALID,
                "quests",
                path,
                msg,
            ));
        };
        if !g.id.is_valid_syntax() {
            err(
                format!("/content/timed_gates/{i}/id"),
                format!(
                    "malformed timed-gate id `{}` — ids must be lowercase kebab-case with the \
                     `timed-gate/` prefix (e.g. `timed-gate/piston-hall`)",
                    g.id
                ),
                d,
            );
        }
        if !seen.insert(g.id.as_str()) {
            err(
                format!("/content/timed_gates/{i}/id"),
                format!("duplicate timed-gate id `{}` — rename one", g.id),
                d,
            );
        }
        for (field, ticks) in [
            ("open_ticks", g.open_ticks),
            ("closed_ticks", g.closed_ticks),
        ] {
            if ticks == 0 {
                err(
                    format!("/content/timed_gates/{i}/{field}"),
                    format!(
                        "timed gate `{}` declares `{field}: 0` — a gate that never {} is not a \
                         timing gate. Use `open-gate`/`close-gate` for a one-way state change, or \
                         give both halves of the cycle a real duration.",
                        g.id,
                        if field == "open_ticks" {
                            "opens"
                        } else {
                            "closes"
                        }
                    ),
                    d,
                );
            }
        }
        let cycle = g.open_ticks.saturating_add(g.closed_ticks);
        if cycle > 0 && g.phase >= cycle {
            err(
                format!("/content/timed_gates/{i}/phase"),
                format!(
                    "timed gate `{}` declares `phase: {}` at or beyond its own {cycle}-tick cycle \
                     — a phase is an offset INTO the cycle, so it must be less than it (use \
                     `phase % cycle`).",
                    g.id, g.phase
                ),
                d,
            );
        }
        // The clock fills and clears a REGION twice a cycle, so `gate` demands a
        // gate station. This is the first shape question asked of
        // `timed_gates[].gate` at this tier at all: the name itself is resolved
        // only by the compiler, so a point named here used to travel all the way
        // to `DW0343`.
        if let Some(f) = station_kind_diag(
            &providers,
            g.gate.as_str(),
            crate::layout::StationKind::Gate,
            "a timed gate's `gate`",
            "quests",
            format!("/content/timed_gates/{i}/gate"),
        ) {
            d.push(f);
        }
        if !driven.insert(g.gate.as_str()) {
            err(
                format!("/content/timed_gates/{i}/gate"),
                format!(
                    "gate `{}` is driven by two timed gates — two clocks filling and clearing the \
                     same region race every tick and the region's state becomes emission order, \
                     not design. One clock per gate.",
                    g.gate
                ),
                d,
            );
        }
        if shortcut_gates.contains(g.gate.as_str()) {
            err(
                format!("/content/timed_gates/{i}/gate"),
                format!(
                    "gate `{}` is both a `shortcut` gate and a `timed-gate` — a shortcut opens \
                     PERMANENTLY (spec-0016 §2) and a clock would re-seal it every cycle, which \
                     is exactly the re-seal `DW0358` exists to forbid. Use two different gates.",
                    g.gate
                ),
                d,
            );
        }
        // The disarm affordance, the same two rules a trap's obeys.
        if let Some(dis) = &g.disarm {
            if let Some(f) = station_kind_diag(
                &providers,
                dis.via.as_str(),
                crate::layout::StationKind::Point,
                "a timed gate's disarm affordance",
                "quests",
                format!("/content/timed_gates/{i}/disarm/via"),
            ) {
                d.push(f);
            }
            if !providers.resolvable(dis.via.as_str()) {
                err(
                    format!("/content/timed_gates/{i}/disarm/via"),
                    format!(
                        "timed-gate `disarm.via` anchor `{}` is not provided by any area's \
                         prefab — {}",
                        dis.via,
                        providers.anchor_remedy(
                            "use an anchor some area's prefab exposes for the jam affordance \
                             (anchor names come from prefab metadata; do NOT invent one)"
                        ),
                    ),
                    d,
                );
            }
            if dis.via == g.gate {
                err(
                    format!("/content/timed_gates/{i}/disarm/via"),
                    format!(
                        "timed gate `{}` puts its `disarm.via` on its own gate anchor `{}` — the \
                         jam lever would stand inside the span the portcullis closes on (and, \
                         with `crush`, kills in). The affordance belongs on ground the player \
                         can reach and hold WITHOUT gambling on the clock, which is the entire \
                         point of the third rung.",
                        g.id, g.gate
                    ),
                    d,
                );
            }
        }
    }

    // `close-gate` may never target a disarmable timed gate: a disarm leaves the
    // portcullis jammed OPEN forever, so permanence is structural (`DW0389`, the
    // mirror of a shortcut's `DW0372`).
    let disarmed: BTreeSet<&str> = quests
        .timed_gates
        .iter()
        .filter(|g| g.disarm.is_some())
        .map(|g| g.gate.as_str())
        .collect();
    if disarmed.is_empty() {
        return;
    }
    let report = |path: String, anchor: &str, d: &mut Vec<Diagnostic>| {
        d.push(Diagnostic::error(
            codes::TIMED_GATE_REARMED,
            "quests",
            path,
            format!(
                "`close-gate` targets `{anchor}`, the gate of a `timed-gate` that declares a \
                 `disarm` — a disarmed gate rests OPEN permanently (souls dossier \
                 §5.2: a hazard the party has switched off stays off), so nothing may re-arm \
                 its clock. Use a different gate for the beat that must re-seal, or drop the \
                 `disarm` and keep the clock running."
            ),
        ));
    };
    for (qi, q) in quests.quests.iter().enumerate() {
        for_each_effect_deep(q, |path, eff| {
            if let Some(a) = eff.close_gate_anchor()
                && disarmed.contains(a.as_str())
            {
                report(format!("/content/quests/{qi}/{path}/anchor"), a.as_str(), d);
            }
        });
    }
    for (ti, t) in quests.triggers.iter().enumerate() {
        for_each_trigger_effect_deep(t, |path, eff| {
            if let Some(a) = eff.close_gate_anchor()
                && disarmed.contains(a.as_str())
            {
                report(
                    format!("/content/triggers/{ti}/{path}/anchor"),
                    a.as_str(),
                    d,
                );
            }
        });
    }
}

// ---------------------------------------------------------------------------
// spec-0016 §6 — TD lanes + aggro-edge summoning
// ---------------------------------------------------------------------------

/// Validate an enchantment map: known ids (`DW0433`), legal levels (`DW0434`).
///
/// Levels are checked against what the `minecraft:enchantments` **component**
/// can carry (1..=255), not against each enchantment's survival max. Exceeding
/// the survival max from a command is legal vanilla and is a legitimate way to
/// build a set-piece elite, so refusing it would be the compiler overruling a
/// design decision it cannot second-guess; 0 and >255 are simply not
/// representable and would be silently dropped by the game.
pub(crate) fn check_enchantments(
    ench: &std::collections::BTreeMap<String, u32>,
    what: &str,
    stage: &'static str,
    path: &str,
    reg: &dyn crate::registry::EnchantmentRegistry,
    d: &mut Vec<Diagnostic>,
) {
    for (id, level) in ench {
        if !reg.contains(id) {
            d.push(Diagnostic::error(
                codes::ENCHANTMENT_UNKNOWN,
                stage,
                format!("{path}/{id}"),
                format!(
                    "{what} enchantment `{id}` is not in the pinned 1.21.11 enchantment \
                     registry — use a valid namespaced enchantment id (e.g. \
                     `minecraft:protection`, `minecraft:sharpness`). Note the vanilla \
                     ids for curses are `minecraft:binding_curse` and \
                     `minecraft:vanishing_curse`, NOT `curse_of_binding`."
                ),
            ));
        }
        if *level == 0 || *level > 255 {
            d.push(Diagnostic::error(
                codes::ENCHANTMENT_LEVEL,
                stage,
                format!("{path}/{id}"),
                format!(
                    "{what} enchantment `{id}` has level {level}, outside the 1..=255 range \
                     the `minecraft:enchantments` component stores. Levels above an \
                     enchantment's survival maximum ARE allowed (that is how a set-piece \
                     elite is built) — but 0 means \"not enchanted\" and is silently \
                     dropped by the game, so declare the level you want or remove the entry."
                ),
            ));
        }
    }
}

/// `DW0436`: a **single-slot fill** whose `count` exceeds the item's
/// `minecraft:max_stack_size` in the pinned 1.21.11 registry.
///
/// Every one of these compiles to `item replace … container.<n> with <item>
/// <count>`, and that command fails **silently** above the cap: the slot simply
/// stays empty and the server logs nothing. A `count: 2` of `minecraft:rabbit_stew`
/// (cap 1) shipped an empty chest slot in the-drowned-bell round 2 — exactly the
/// silent-failure class `DW0431` exists for, one tier too late. The cap comes from
/// Mojang's own item-components data, vendored per MC pin
/// (`crates/delvec/data/item-stack-sizes-1.21.11.json`), never a hand table.
///
/// Skipped when the registry does not carry stack sizes (the small vendored DSL-side
/// subset) or the item id is unknown — the latter is already `DW0143`, and stacking
/// a second diagnostic on one typo is noise.
/// Exclusive ownership of an adopted container (DSL v0.8, `DW0435`).
///
/// Both container-fill surfaces write **positionally** from `container.0`: a
/// `loot` entry and a `collect`'s adopted container filling one cell overwrite
/// each other slot-for-slot, and the loser vanishes without a word — the same
/// silent-overwrite defect `DW0435` already names for two `loot` entries, reached
/// through a second door. Two `collect` objectives sharing a container is the
/// same collision (and worse: whichever activates second replaces the first
/// objective's items with its own).
///
/// Only claims involving at least one `collect` are reported here; `loot`-vs-
/// `loot` stays in [`loot_checks`], so nothing is diagnosed twice.
fn collect_container_claim_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    // (anchor -> what claims it), in declaration order. `loot` first: a loot
    // entry is the surface that exists to fill a container, so it reads as the
    // incumbent in the message.
    let mut claimed: BTreeMap<&str, String> = BTreeMap::new();
    for l in &c.quests.content.loot {
        claimed
            .entry(l.anchor.as_str())
            .or_insert_with(|| format!("loot `{}`", l.id));
    }
    for (i, q) in c.quests.content.quests.iter().enumerate() {
        for (j, o) in q.objectives.iter().enumerate() {
            let Some(cont) = o.collect_container() else {
                continue;
            };
            let mine = format!("collect objective `{}`", o.id());
            if let Some(prev) = claimed.get(cont.as_str()) {
                d.push(Diagnostic::error(
                    codes::LOOT_DUPLICATE_ANCHOR,
                    "quests",
                    format!("/content/quests/{i}/objectives/{j}/container"),
                    format!(
                        "{prev} and {mine} both fill the container at anchor `{cont}`. Slots \
                         are assigned positionally from `container.0`, so one fill overwrites \
                         the other slot-for-slot and its items never reach the player. Give the \
                         collect its own container anchor (prefabs may expose several), or \
                         fold the other fill's items into it — do NOT rely on declaration \
                         order to combine them."
                    ),
                ));
            } else {
                claimed.insert(cont.as_str(), mine);
            }
        }
    }
}

/// The smallest vanilla container the container-fill surfaces admit. A barrel and
/// a single chest both hold 27; refusing >27 up front keeps the overflow from
/// being discovered as a silently dropped stack on a live server. Shared by the
/// `loot` stack ceiling and the v0.8 `collect` `fill_count` ceiling, which are the
/// same positional-fill rule (`DW0432`) on two surfaces.
pub(crate) const MIN_CONTAINER_SLOTS: usize = 27;

pub(crate) fn check_stack_count(
    item: &str,
    count: u32,
    what: &str,
    path: String,
    items: &dyn ItemRegistry,
    d: &mut Vec<Diagnostic>,
) {
    let Some(cap) = items.max_stack_size(item) else {
        return;
    };
    if count <= cap {
        return;
    }
    d.push(Diagnostic::error(
        codes::ITEM_COUNT_OVER_STACK,
        "quests",
        path,
        format!(
            "{what} declares `{item}` × {count}, but `{item}` stacks to at most {cap} in \
             1.21.11. This is filled with `item replace … container.<n>`, which fails \
             SILENTLY above the cap — the slot ships empty and nothing is logged. Lower \
             the count to {cap} or fewer, or declare additional entries/containers."
        ),
    ));
}

/// Stage-5 `loot` declarations (spec-0021): id syntax/uniqueness (`DW0110`/
/// `DW0111`), anchor resolution (`DW0142`), item ids (`DW0143`), enchantments
/// (`DW0433`/`DW0434`), duplicate anchors (`DW0435`) and slot overflow
/// (`DW0432`).
///
/// The *container-ness* of the anchor's cell is deliberately NOT checked here:
/// it needs the assembled world, so it is a build-tier proof (`DW0431`) in the
/// compiler. This tier checks everything decidable from the DSL alone.
fn loot_checks(
    c: &Campaign,
    items: &dyn ItemRegistry,
    anchors: &dyn AnchorRegistry,
    d: &mut Vec<Diagnostic>,
) {
    let quests = &c.quests.content;
    if quests.loot.is_empty() {
        return;
    }
    let ench_reg = crate::registry::VendoredEnchantmentRegistry::v1_21_11();

    let providers = AnchorProviders::build(c, anchors);

    let mut seen_id: BTreeSet<&str> = BTreeSet::new();
    let mut seen_anchor: BTreeMap<&str, usize> = BTreeMap::new();
    for (i, l) in quests.loot.iter().enumerate() {
        if !l.id.is_valid_syntax() {
            d.push(Diagnostic::error(
                codes::ID_SYNTAX,
                "quests",
                format!("/content/loot/{i}/id"),
                format!(
                    "malformed loot id `{}` — loot ids must be lowercase kebab-case with the \
                     `loot/` prefix (e.g. `loot/galley-stores`)",
                    l.id
                ),
            ));
        }
        if !seen_id.insert(l.id.as_str()) {
            d.push(Diagnostic::error(
                codes::ID_DUPLICATE,
                "quests",
                format!("/content/loot/{i}/id"),
                format!("duplicate loot id `{}`", l.id),
            ));
        }
        if let Some(f) = station_kind_diag(
            &providers,
            l.anchor.as_str(),
            crate::layout::StationKind::Point,
            "a loot chest",
            "quests",
            format!("/content/loot/{i}/anchor"),
        ) {
            d.push(f);
        }
        if !providers.resolvable(l.anchor.as_str()) {
            d.push(Diagnostic::error(
                codes::ANCHOR_UNRESOLVED,
                "quests",
                format!("/content/loot/{i}/anchor"),
                format!(
                    "loot anchor `{}` is not provided by any prefab bound in this campaign — {}",
                    l.anchor,
                    providers.anchor_remedy(
                        "use an anchor the prefab exposes (anchor names come from prefab \
                         metadata; do NOT invent one)"
                    ),
                ),
            ));
        }
        // Two fills on one container: the second `item replace block` overwrites
        // the first slot-for-slot, so one declaration silently loses.
        if let Some(prev) = seen_anchor.insert(l.anchor.as_str(), i) {
            d.push(Diagnostic::error(
                codes::LOOT_DUPLICATE_ANCHOR,
                "quests",
                format!("/content/loot/{i}/anchor"),
                format!(
                    "loot `{}` and loot `{}` both fill anchor `{}`. Slots are assigned \
                     positionally from `container.0`, so the later declaration overwrites the \
                     earlier one and its items never appear. Merge the two `items` lists into \
                     ONE `loot` entry — do NOT rely on declaration order to combine them.",
                    quests.loot[prev].id, l.id, l.anchor
                ),
            ));
        }
        if l.items.len() > MIN_CONTAINER_SLOTS {
            d.push(Diagnostic::error(
                codes::LOOT_TOO_MANY_ITEMS,
                "quests",
                format!("/content/loot/{i}/items"),
                format!(
                    "loot `{}` declares {} stacks, more than the {MIN_CONTAINER_SLOTS} slots a \
                     vanilla chest or barrel has. Slots are assigned positionally, so every \
                     stack past the {MIN_CONTAINER_SLOTS}th would be dropped silently. Split \
                     the contents across more than one container.",
                    l.id,
                    l.items.len()
                ),
            ));
        }
        for (k, it) in l.items.iter().enumerate() {
            if !items.contains(&it.item) {
                d.push(Diagnostic::error(
                    codes::ITEM_UNKNOWN,
                    "quests",
                    format!("/content/loot/{i}/items/{k}/item"),
                    format!(
                        "loot item `{}` is not in the pinned 1.21.11 item registry — use a \
                         valid namespaced item id (e.g. `minecraft:cooked_cod`)",
                        it.item
                    ),
                ));
            }
            check_stack_count(
                &it.item,
                it.count,
                &format!("loot `{}`", l.id),
                format!("/content/loot/{i}/items/{k}/count"),
                items,
                d,
            );
            check_enchantments(
                &it.enchantments,
                &format!("loot `{}` item `{}`", l.id, it.item),
                "quests",
                &format!("/content/loot/{i}/items/{k}/enchantments"),
                &ench_reg,
                d,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// DSL v0.8 — branch points, happenings, named endings (spec-0025);
// the bonfire rest interaction + the class-kit flask (spec-0016 §1)
// ---------------------------------------------------------------------------

/// Every ending id some `campaign-complete` declares. There is no separate
/// declaration list — the same rule flags follow.
pub fn declared_endings(c: &Campaign) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    crate::for_each_campaign_effect(c, &mut |_p, _site, eff| {
        if let Verb::CampaignComplete {
            ending: Some(e), ..
        } = &eff.verb
        {
            out.insert(e.as_str().to_string());
        }
    });
    out
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
