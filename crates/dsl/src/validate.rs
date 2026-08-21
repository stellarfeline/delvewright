//! Campaign validation: all six rule groups from spec-0001.
//!
//! [`validate_campaign`] uses the vendored v0 registries; the compiler injects
//! full registries via [`validate_campaign_with`].

use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostic::{Diagnostic, codes};
use crate::envelope::{
    Campaign, Stage, is_supported_version, is_v03, is_v04, is_v05, is_v06, is_v07, is_v08, is_v09,
    is_v10, is_v11, is_v12, is_v13, is_v14,
};
use crate::ids::is_kebab;
use crate::metrics::Metrics;
use crate::registry::{
    AnchorRegistry, BlockRegistry, EffectRegistry, EntityRegistry, ItemBackedBlockRegistry,
    ItemRegistry, VendoredAnchorRegistry, VendoredEffectRegistry, VendoredEntityRegistry,
    VendoredItemRegistry,
};
use crate::stages::{
    EditFrame, EncounterTier, Locomotion, MorphOp, NarrateStyle, Objective, QuestEffect,
    RegionShape, TriggerOn, WorldEdit, body_traversal_sites,
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

    envelope(c, &mut d);
    syntax(c, &mut d);
    uniqueness(c, &mut d);
    references(c, &mut d);
    dialogue(c, &mut d);
    plan(c, &mut d);
    after_ordering(c, &mut d);
    reserved(c, &mut d);
    // DSL v0.10 (spec-0031): the runtime-state surface. Unconditional — every
    // loop inside is empty for a campaign that declares no datum and no
    // comparison, and the version fence itself lives in `reserved_v10`.
    state_checks(c, &mut d);
    // DSL v0.10 (spec-0031): the status-effect verbs. Unconditional for the same
    // reason `state_checks` is — every walk inside is empty for a campaign that
    // declares neither verb, and the version fence lives in `reserved_v10`. The
    // status-effect registry is the fixed vanilla list v0.4 already validates
    // wave-mob effects against, so no injected registry is needed.
    status_effect_checks(c, &VendoredEffectRegistry::v1_21_11(), &mut d);
    // DSL v0.10 (spec-0032): the trade and recovery-stake surface. Unconditional
    // for the same reason — every loop inside is empty for a campaign that
    // declares neither, and the version fence lives in `reserved_v10`.
    economy_checks(c, &mut d);
    // DSL v0.11 (spec-0034): the per-body traversal declaration. Unconditional
    // for the same reason its neighbours are — the walk is empty for a campaign
    // that declares none, and the version fence lives in `reserved_v11`.
    body_traversal_checks(c, &mut d);
    prefab_binding(c, anchors, &mut d);
    anchors_and_items(c, items, anchors, &mut d);
    cross_stage(c, &mut d);
    // DSL v0.3: the new stage-5 verbs, waves and flags. Gated on the quests
    // stage's version so a v0.2 campaign is unaffected (its uses of these verbs
    // are still rejected as reserved by `reserved`, above).
    if is_v03(c.quests.dsl_version.as_str()) {
        v03_checks(c, items, anchors, entities, &mut d);
    }
    // DSL v0.4: props/set-block/narrate/triggers/skins/lifecycle/cutscene. The
    // status-effect registry is a fixed vanilla list; the block registry derives
    // from the item registry (see [`ItemBackedBlockRegistry`]), so no new
    // caller-supplied registry is needed. Gated on the quests stage version.
    if is_v04(c.quests.dsl_version.as_str()) {
        let blocks = ItemBackedBlockRegistry::new(items);
        let effects_reg = VendoredEffectRegistry::v1_21_11();
        v04_checks(c, anchors, &blocks, &effects_reg, &mut d);
    }
    // DSL v0.6: scripted actors + staging effects (spec-0014), traps (spec-0011)
    // and wave-mob equipment. Actor entity ids validate against the
    // injected entity registry and anchors against single-prefab area metadata;
    // traps' dispense payloads and wave-mob equipment slots validate against the
    // item registry (pool areas deferred to the compiler). Gated on the quests
    // stage version.
    if is_v06(c.quests.dsl_version.as_str()) {
        v06_checks(c, items, anchors, entities, &mut d);
        v06_trap_checks(c, items, entities, anchors, &mut d);
        shortcut_checks(c, anchors, &mut d);
        ambush_checks(c, &mut d);
        timed_gate_checks(c, anchors, &mut d);
        loot_checks(c, items, anchors, &mut d);
        lane_checks(c, anchors, &mut d);
        difficulty_checks(c, &mut d);
    }
    // DSL v0.6 stage 7 (spec-0017): the map-editor edit script. Structural
    // checks only — frame/region *resolution* happens at build time against the
    // solved layout (the compiler's `DW0323`).
    if c.world_edits.is_some() {
        let blocks = ItemBackedBlockRegistry::new(items);
        world_edits_checks(c, &blocks, &mut d);
    }
    // spec-0049 (DSL v0.13): the map-pipeline documents. Bound to the EVENT it
    // guards rather than to a step someone runs — a campaign directory holding a
    // `layout-graph.json` has no path to a verdict that does not come through
    // here, because this is the function every `delvec` subcommand's validation
    // stage calls. The version fence is `reserved_v13`, below, and the reads
    // ledger is what gives `DW0813` its document-side binding: every building
    // metric these checks rest a verdict on records that it did.
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
    // DSL v0.8 (spec-0025): the declared story forks and the per-node
    // `happening`. Structural only — the branch proofs themselves (`DW048x`) are
    // compiler-tier, because they need the branch/flag flow model.
    if is_v08(c.quest_plan.dsl_version.as_str()) {
        branch_point_checks(c, &mut d);
    }
    // DSL v0.8: a `collect` may adopt a prefab container, which puts a
    // second positional filler on the same anchor a `loot` entry can name.
    if is_v08(c.quests.dsl_version.as_str()) {
        collect_container_claim_checks(c, &mut d);
    }
    if is_v08(c.quests.dsl_version.as_str()) || is_v08(c.dialogue.dsl_version.as_str()) {
        happening_subject_checks(c, &mut d);
    }
    // DSL v0.8 (spec-0016 §1): what is really in the
    // flask. The status-effect registry is the same fixed vanilla list v0.4 uses,
    // and the potion registry is complete in-crate, so no injected registry is
    // needed.
    if is_v08(c.classes.dsl_version.as_str()) {
        let effects_reg = VendoredEffectRegistry::v1_21_11();
        kit_potion_checks(c, &effects_reg, &mut d);
    }
    // DSL v0.10 (spec-0031): the stage-5 lethal volumes. Structural only — the
    // completability half (`DW0510` the forced route, `DW0511` the respawn seat)
    // is compiler-tier, because it needs the solved layout.
    if is_v10(c.quests.dsl_version.as_str()) {
        lethal_volume_checks(c, anchors, &mut d);
        // DSL v0.10 (spec-0032): a shop stands on a prefab anchor, and an anchor
        // no bound prefab provides is the same defect a lethal volume's is.
        shop_anchor_checks(c, anchors, &mut d);
    }

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
    /// The union of every known area's set.
    union: BTreeSet<String>,
    /// Some area binds a pool, so the compiler resolves its anchors later.
    deferred: bool,
    /// Every area contributed a set — the union is the whole truth.
    all_areas_known: bool,
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
        if c.site_plan.is_some() {
            declared_areas += 1;
            per_area.insert(
                crate::siteplan::SITE_AREA.to_string(),
                crate::siteplan::synthesized_anchors(c),
            );
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
            union,
            deferred,
            all_areas_known,
        }
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
        if !providers.resolvable(v.region.anchor.as_str()) {
            d.push(Diagnostic::error(
                codes::ANCHOR_UNRESOLVED,
                "quests",
                format!("/content/lethal_volumes/{i}/region/anchor"),
                format!(
                    "lethal-volume anchor `{}` is not provided by any prefab bound in this \
                     campaign — use an anchor the prefab exposes (anchor names come from prefab \
                     metadata; do NOT invent one)",
                    v.region.anchor
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

/// `DW0527` — a gate that reads a datum an earlier effect in the same bundle has
/// already written.
///
/// Sibling effects in one bundle are consecutive commands in one generated
/// function, and vanilla evaluates each `execute` condition when it reaches it. So
/// a comparison placed after a write is a comparison against the post-write value —
/// which is exactly what the shop pattern spec-0032 recommends walks into if the
/// refusal is written after the purchase instead of before it.
///
/// Scope is deliberately one flat sibling list: a `sequence` step runs on a later
/// tick and a nested lifecycle bundle runs at a different moment entirely, so
/// "earlier in the same breath" is precisely a list index. Warning tier — an author
/// who means to write then compare is doing something legitimate, and the
/// diagnostic's job is to make sure they meant it.
fn read_after_write_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    crate::effects::for_each_effect_root(c, &mut |site, list| {
        // Only a **conditional** write counts, and that narrowing is the whole
        // precision of this rule. An UNCONDITIONAL write followed by a comparison
        // is the ordinary sequenced idiom — *pay the toll, then the door opens
        // because the toll is now zero* — where the author plainly means the value
        // the bundle just produced. A write that is itself gated on the SAME datum
        // is the other thing: the bundle asks about the datum, changes it across
        // the boundary it just asked about, and then asks again — which is the
        // shape that pays for something and apologises for it in the same breath.
        let mut written: BTreeMap<&str, usize> = BTreeMap::new();
        for (i, eff) in list.iter().enumerate() {
            for cmp in eff.requires_state() {
                let Some(at) = written.get(cmp.state.as_str()) else {
                    continue;
                };
                d.push(Diagnostic::warning(
                    codes::STATE_READ_AFTER_WRITE,
                    site.stage,
                    format!("{}/{i}/requires_state", site.path),
                    format!(
                        "this `{}` compares `{}`, and effect {at} of the same bundle already \
                         changes `{}` behind a gate on `{}` itself — so this comparison is made \
                         against the value THIS bundle just produced, on the far side of the \
                         boundary it just tested. The shape that bites is a purchase followed by \
                         its own refusal: buying the LAST coin debits it, and the `at-most` \
                         apology written after the debit then holds as well, so the player is \
                         charged AND told they cannot afford it. Move every reading effect ahead \
                         of the write. (An UNCONDITIONAL write followed by a comparison is not \
                         this: `set-state toll 0` and then a door gated on `toll at-most 0` \
                         plainly means the value the bundle just produced, and is not \
                         diagnosed.)",
                        eff.verb(),
                        cmp.state.as_str(),
                        cmp.state.as_str(),
                        cmp.state.as_str()
                    ),
                ));
            }
            if let Some((id, _)) = eff.writes_state()
                && eff
                    .requires_state()
                    .iter()
                    .any(|c| c.state.as_str() == id.as_str())
            {
                written.entry(id.as_str()).or_insert(i);
            }
        }
    });
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
        if providers.resolvable(sh.anchor.as_str()) {
            continue;
        }
        d.push(Diagnostic::error(
            codes::ANCHOR_UNRESOLVED,
            "quests",
            format!("/content/shops/{i}/anchor"),
            format!(
                "shop anchor `{}` is not provided by any prefab bound in this campaign — use an \
                 anchor the prefab exposes (anchor names come from prefab metadata; do NOT invent \
                 one)",
                sh.anchor
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
            Some(decl) if decl.scope != crate::stages::StateScope::Player => {
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
        if let Some(crate::stages::Forfeit::Proportion { percent }) = s.forfeit
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
    crate::stages::for_each_campaign_effect(c, &mut |path, _site, eff| {
        let crate::stages::QuestEffect::DropStake { stake, .. } = eff else {
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

    read_after_write_checks(c, d);

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
        if !is_supported_version(version) {
            d.push(Diagnostic::error(
                codes::DSL_VERSION,
                expected.name(),
                "/dsl_version",
                format!(
                    "unsupported dsl_version `{version}`, expected one of {:?}",
                    // The ledger minus its reservations: a reserved number is IN
                    // the ledger and refused, so listing it here would answer a
                    // refusal with advice to try something refused again.
                    crate::accepted_versions().collect::<Vec<_>>()
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
    macro_rules! chk {
        ($id:expr, $stage:expr, $path:expr) => {
            if !$id.is_valid_syntax() {
                d.push(Diagnostic::error(
                    codes::ID_SYNTAX,
                    $stage,
                    $path,
                    format!(
                        "malformed id `{}` — ids must be lowercase kebab-case with their type \
                         prefix (e.g. `area/keep`, `npc/keeper`, `quest/find-key`)",
                        $id
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
                "npc references unknown area `{}` — declare it in stage-1 `world.areas` or \
                 correct the reference",
                npc.area
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

    for (i, q) in c.quest_plan.content.quests.iter().enumerate() {
        dangling(
            d,
            area_ids.contains(q.area.as_str()),
            "quest-plan",
            format!("/content/quests/{i}/area"),
            format!(
                "quest references unknown area `{}` — declare it in stage-1 `world.areas` or \
                 correct the reference",
                q.area
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
        if let crate::stages::Trigger::QuestComplete { quest } = &q.trigger {
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
            if let crate::stages::Objective::TalkTo { npc, .. } = obj {
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

fn dialogue(c: &Campaign, d: &mut Vec<Diagnostic>) {
    use crate::stages::{DialogueEffect, Objective};

    // Stage-5 objective facts: which are `talk-to`, and (for those) their npc.
    let mut all_objectives: BTreeSet<&str> = BTreeSet::new();
    let mut talk_npc: BTreeMap<&str, &str> = BTreeMap::new();
    for q in &c.quests.content.quests {
        for o in &q.objectives {
            all_objectives.insert(o.id().as_str());
            if let Objective::TalkTo { id, npc, .. } = o {
                talk_npc.insert(id.as_str(), npc.as_str());
            }
        }
    }

    // npc id -> objective ids completed by an option reachable from that tree's
    // root (feeds the DW0123 coverage check).
    let mut reachable_completes: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();

    for (i, tree) in c.dialogue.content.dialogues.iter().enumerate() {
        let node_ids: BTreeSet<&str> = tree.nodes.iter().map(|n| n.id.as_str()).collect();

        // `next` / effect references.
        for (j, node) in tree.nodes.iter().enumerate() {
            for (k, opt) in node.options.iter().enumerate() {
                if let Some(next) = &opt.next
                    && !node_ids.contains(next.as_str())
                {
                    d.push(Diagnostic::error(
                        codes::DIALOGUE_BAD_REF,
                        "dialogue",
                        format!("/content/dialogues/{i}/nodes/{j}/options/{k}/next"),
                        format!(
                            "dialogue option `next` references unknown node `{next}` — add a node \
                             with that id to this tree or correct the reference"
                        ),
                    ));
                }
                for (m, eff) in opt.effects.iter().enumerate() {
                    let DialogueEffect::CompleteObjective { objective } = eff else {
                        continue;
                    };
                    let oid = objective.as_str();
                    let path = format!(
                        "/content/dialogues/{i}/nodes/{j}/options/{k}/effects/{m}/objective"
                    );
                    let msg = if !all_objectives.contains(oid) {
                        Some(format!(
                            "dialogue `complete-objective` effect references unknown objective \
                             `{objective}` — it must name a `talk-to` objective on this tree's \
                             npc; declare it or correct the reference"
                        ))
                    } else if let Some(owner) = talk_npc.get(oid) {
                        if *owner == tree.npc.as_str() {
                            None
                        } else {
                            Some(format!(
                                "dialogue effect completes `talk-to` objective `{objective}`, \
                                 which belongs to npc `{owner}`, not this tree's npc `{}` — a tree \
                                 may only complete its own npc's objectives; move the effect into \
                                 `{owner}`'s tree",
                                tree.npc
                            ))
                        }
                    } else {
                        Some(format!(
                            "dialogue `complete-objective` effect targets objective `{objective}`, \
                             which is not a `talk-to` objective — only `talk-to` objectives are \
                             completed through dialogue; retarget it or change the objective's type"
                        ))
                    };
                    if let Some(msg) = msg {
                        d.push(Diagnostic::error(
                            codes::DIALOGUE_BAD_OBJECTIVE,
                            "dialogue",
                            path,
                            msg,
                        ));
                    }
                }
            }
        }

        // Root existence.
        if !node_ids.contains(tree.root.as_str()) {
            d.push(Diagnostic::error(
                codes::DIALOGUE_BAD_REF,
                "dialogue",
                format!("/content/dialogues/{i}/root"),
                format!(
                    "dialogue tree `root` references unknown node `{}` — add a node with that id \
                     or point `root` at an existing node",
                    tree.root
                ),
            ));
            continue; // reachability is undefined without a root
        }

        // Reachability from root.
        let adj: BTreeMap<&str, Vec<&str>> = tree
            .nodes
            .iter()
            .map(|n| {
                let outs = n
                    .options
                    .iter()
                    .filter_map(|o| o.next.as_ref().map(|x| x.as_str()))
                    .collect();
                (n.id.as_str(), outs)
            })
            .collect();
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        // Entry points: the tree's own `root`, plus (DSL v0.7, spec-0020) every
        // node some quest's `cast` ledger declares as this NPC's root. A ledger
        // root IS an entry point — right-click opens it directly once that quest
        // begins — so a node reached only that way is reachable, not orphaned.
        // Without this, retiring a premise root by swapping to a later one would
        // make the later one `DW0120`, and the ledger would be unusable for the
        // exact thing it exists to do.
        let mut stack = vec![tree.root.as_str()];
        for q in &c.quests.content.quests {
            for (npc, entry) in &q.cast {
                if npc.as_str() != tree.npc.as_str() {
                    continue;
                }
                for p in entry.placements() {
                    if let Some(crate::stages::CastDialogue::Root(r)) = &p.dialogue {
                        stack.push(r.as_str());
                    }
                }
            }
        }
        while let Some(cur) = stack.pop() {
            if seen.insert(cur)
                && let Some(neis) = adj.get(cur)
            {
                stack.extend(neis.iter().copied());
            }
        }
        for (j, node) in tree.nodes.iter().enumerate() {
            if !seen.contains(node.id.as_str()) {
                d.push(Diagnostic::error(
                    codes::DIALOGUE_UNREACHABLE,
                    "dialogue",
                    format!("/content/dialogues/{i}/nodes/{j}"),
                    format!(
                        "dialogue node `{}` is unreachable from `root` — add an option whose \
                         `next` leads here from a reachable node, or remove this node",
                        node.id
                    ),
                ));
            }
        }

        // Objectives completed by reachable options (for the coverage check).
        let completes = reachable_completes.entry(tree.npc.as_str()).or_default();
        for node in &tree.nodes {
            if !seen.contains(node.id.as_str()) {
                continue;
            }
            for opt in &node.options {
                for eff in &opt.effects {
                    if let DialogueEffect::CompleteObjective { objective } = eff {
                        completes.insert(objective.as_str());
                    }
                }
            }
        }
    }

    // Every `talk-to` objective must have ≥ 1 reachable completing option in its
    // own npc's tree (the static half of the compiler's DW0203 guarantee).
    for (qi, q) in c.quests.content.quests.iter().enumerate() {
        for (oi, o) in q.objectives.iter().enumerate() {
            if let Objective::TalkTo { id, npc, .. } = o {
                let covered = reachable_completes
                    .get(npc.as_str())
                    .is_some_and(|s| s.contains(id.as_str()));
                if !covered {
                    d.push(Diagnostic::error(
                        codes::DIALOGUE_UNCOVERED,
                        "dialogue",
                        format!("/content/quests/{qi}/objectives/{oi}"),
                        format!(
                            "`talk-to` objective `{id}` has no reachable dialogue option in npc \
                             `{npc}`'s tree that completes it — add an option (reachable from \
                             `root`) with a `complete-objective` effect for `{id}`, else the \
                             objective can never finish"
                        ),
                    ));
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Rule group 4 — quest plan
// ---------------------------------------------------------------------------

fn plan(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let plan = &c.quest_plan.content;
    let planned_ids: BTreeSet<&str> = plan.quests.iter().map(|q| q.id.as_str()).collect();

    // Non-mandatory (reserved in v0).
    for (i, q) in plan.quests.iter().enumerate() {
        if !q.mandatory {
            d.push(Diagnostic::error(
                codes::NON_MANDATORY,
                "quest-plan",
                format!("/content/quests/{i}/mandatory"),
                format!(
                    "quest `{}` sets `mandatory: false`, which is reserved until M3 — set \
                     `mandatory: true` (every v0 quest is on the critical path)",
                    q.id
                ),
            ));
        }
    }

    // Dependency edges (only to existing quests; dangling handled elsewhere).
    let edges: BTreeMap<&str, Vec<&str>> = plan
        .quests
        .iter()
        .map(|q| {
            let deps = q
                .depends_on
                .iter()
                .map(|x| x.as_str())
                .filter(|x| planned_ids.contains(x))
                .collect();
            (q.id.as_str(), deps)
        })
        .collect();
    let nodes: Vec<&str> = plan.quests.iter().map(|q| q.id.as_str()).collect();

    if graph_has_cycle(&nodes, &edges) {
        d.push(Diagnostic::error(
            codes::PLAN_CYCLE,
            "quest-plan",
            "/content/quests",
            "stage-4 quest `depends_on` graph contains a cycle — the plan must be a DAG; remove a \
             `depends_on` edge so the quests form an acyclic order",
        ));
        return; // reachability is meaningless with a cycle
    }

    // Finale must be declared.
    if !planned_ids.contains(plan.finale.as_str()) {
        d.push(Diagnostic::error(
            codes::FINALE_UNKNOWN,
            "quest-plan",
            "/content/finale",
            format!(
                "stage-4 `finale` `{}` is not a declared quest — set `finale` to the id of an \
                 existing planned quest (the one that ends the delve)",
                plan.finale
            ),
        ));
        return;
    }

    // Finale convergence: every quest must be a transitive dependency of the
    // finale (the plan converges on the finale). See README (spec ambiguity).
    let mut reach: BTreeSet<&str> = BTreeSet::new();
    let mut stack = vec![plan.finale.as_str()];
    while let Some(cur) = stack.pop() {
        if reach.insert(cur)
            && let Some(deps) = edges.get(cur)
        {
            stack.extend(deps.iter().copied());
        }
    }
    for (i, q) in plan.quests.iter().enumerate() {
        if !reach.contains(q.id.as_str()) {
            d.push(Diagnostic::error(
                codes::FINALE_UNREACHABLE,
                "quest-plan",
                format!("/content/quests/{i}"),
                format!(
                    "quest `{}` is not a (transitive) dependency of finale `{}`, so the plan does \
                     not converge on the finale — add a `depends_on` chain so `{}` eventually \
                     depends on `{}` (or drop `{}` if it is not part of this delve)",
                    q.id, plan.finale, plan.finale, q.id, q.id
                ),
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// Rule group 5 — intra-quest objective ordering (`after` DAG)
// ---------------------------------------------------------------------------

fn after_ordering(c: &Campaign, d: &mut Vec<Diagnostic>) {
    for (i, q) in c.quests.content.quests.iter().enumerate() {
        let local: BTreeSet<&str> = q.objectives.iter().map(|o| o.id().as_str()).collect();
        let edges: BTreeMap<&str, Vec<&str>> = q
            .objectives
            .iter()
            .map(|o| {
                let deps = o
                    .after()
                    .iter()
                    .map(|x| x.as_str())
                    .filter(|x| local.contains(x))
                    .collect();
                (o.id().as_str(), deps)
            })
            .collect();
        let nodes: Vec<&str> = q.objectives.iter().map(|o| o.id().as_str()).collect();
        if graph_has_cycle(&nodes, &edges) {
            d.push(Diagnostic::error(
                codes::AFTER_CYCLE,
                "quests",
                format!("/content/quests/{i}/objectives"),
                format!(
                    "objective `after` ordering in quest `{}` contains a cycle — the `after` \
                     edges must form a DAG; remove one `after` entry to break the cycle",
                    q.id
                ),
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// Rule group 5 — reserved values / fields
// ---------------------------------------------------------------------------

fn reserved(c: &Campaign, d: &mut Vec<Diagnostic>) {
    // The v0.3 verbs (`kill`/`collect`/`interact`, `give-item`/`set-flag`/
    // `spawn-wave`) are implemented under dsl_version 0.3.0 and reserved under
    // 0.2.0. NPC roles `vendor`/`boss` remain reserved in both.
    let v03 = is_v03(c.quests.dsl_version.as_str());

    for (i, npc) in c.npcs.content.npcs.iter().enumerate() {
        if let Some(name) = npc.role.reserved() {
            d.push(Diagnostic::error(
                codes::RESERVED,
                "npcs",
                format!("/content/npcs/{i}/role"),
                format!(
                    "npc role `{name}` is reserved and not implemented in v0 — use role \
                     `quest-giver` or `flavor`. Do NOT raise `dsl_version` to try to enable it: \
                     `vendor`/`boss` are not implemented at any version yet"
                ),
            ));
        }
    }
    for (i, q) in c.quests.content.quests.iter().enumerate() {
        for (j, obj) in q.objectives.iter().enumerate() {
            if let Some(name) = obj.v03_verb()
                && !v03
            {
                d.push(Diagnostic::error(
                    codes::RESERVED,
                    "quests",
                    format!("/content/quests/{i}/objectives/{j}/type"),
                    format!(
                        "objective type `{name}` requires dsl_version 0.3.0 — raise this stage's \
                         `dsl_version` to at least 0.3.0, or remove the objective"
                    ),
                ));
            }
        }
        for_each_effect(q, |path, eff| {
            if let Some(name) = eff.v03_effect()
                && !v03
            {
                d.push(Diagnostic::error(
                    codes::RESERVED,
                    "quests",
                    format!("/content/quests/{i}/{path}/type"),
                    format!(
                        "effect `{name}` requires dsl_version 0.3.0 — raise this stage's \
                         `dsl_version` to at least 0.3.0, or remove the effect"
                    ),
                ));
            }
        });
    }

    reserved_v04(c, d);
    reserved_v05(c, d);
    reserved_v06(c, d);
    reserved_v07(c, d);
    reserved_v08(c, d);
    reserved_v09(c, d);
    reserved_v10(c, d);
    reserved_v11(c, d);
    reserved_v12(c, d);
    reserved_v13(c, d);
    reserved_v14(c, d);
    press_answer_checks(c, d);
    press_obligation_checks(c, d);
}

/// DSL v0.12 reserved-feature gating (spec-0042): the **`open-way`** effect.
///
/// One verb and one stage — the effect is written in stage-5 bundles at every
/// nesting depth, so the fence reads the `quests` document's own `dsl_version`
/// and rides [`crate::stages::for_each_campaign_effect`], the closed effect
/// enumeration. A hand-written walk over the roots this function's author
/// happened to remember is how a fence comes to be narrower than the surface it
/// fences; this one is exactly as total as the verb.
///
/// **No requirement half.** Nothing obliges a campaign to open a way, and a
/// campaign that opens none emits what it always emitted — no fill, no region
/// event, no ways ledger. The obligations this surface does carry belong to the
/// *piece* and are raised by the compiler at every version (`DW0547`,
/// `DW0548`, `DW0549`), because a campaign below 0.12.0 cannot reach them: it
/// cannot stage an `open-way` at all, and a piece whose way nothing opens is
/// content rather than a defect (spec-0042 §2.5).
/// The spec-0049 map-pipeline documents exist only at `dsl_version` 0.13.0.
///
/// A whole DOCUMENT is fenced here rather than a field, which is the only shape
/// the fence has for a stage that did not exist: an older campaign has no
/// `layout-graph.json`, so there is nothing on it to grandfather and nothing it
/// could have been judged against. The refusal is `DW0141` like every other
/// surface used below the version that introduced it, and it names the document
/// rather than a path inside it, because raising one field's version would not
/// make the file legal.
fn reserved_v13(c: &Campaign, d: &mut Vec<Diagnostic>) {
    for (stage, version) in [
        (
            Stage::GeometryBrief,
            c.geometry_brief.as_ref().map(|e| e.dsl_version.as_str()),
        ),
        (
            Stage::LayoutGraph,
            c.layout_graph.as_ref().map(|e| e.dsl_version.as_str()),
        ),
    ] {
        let Some(version) = version else { continue };
        if is_v13(version) {
            continue;
        }
        d.push(Diagnostic::error(
            codes::RESERVED,
            stage.name(),
            "/dsl_version",
            format!(
                "a `{name}` document requires dsl_version 0.13.0 and this one declares \
                 `{version}` — raise this document's own `dsl_version` to 0.13.0, or delete the \
                 file. It is a document of the map pipeline, which states a campaign's space \
                 before any coordinate exists; no earlier version has anywhere to put it.",
                name = stage.name(),
            ),
        ));
    }
}

/// The spec-0049 site plan exists only at `dsl_version` 0.14.0.
///
/// A whole DOCUMENT is fenced, on the same terms as its two siblings one version
/// below: an older campaign has no `site-plan.json`, so there is nothing on it
/// to grandfather and nothing it could have been judged against.
fn reserved_v14(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let Some(version) = c.site_plan.as_ref().map(|e| e.dsl_version.as_str()) else {
        return;
    };
    if is_v14(version) {
        return;
    }
    d.push(Diagnostic::error(
        codes::RESERVED,
        Stage::SitePlan.name(),
        "/dsl_version",
        format!(
            "a `site-plan` document requires dsl_version 0.14.0 and this one declares \
             `{version}` — raise this document's own `dsl_version` to 0.14.0, or delete the \
             file. It is the geometric embedding of the layout graph, and a campaign at 0.13.0 \
             states its space as a graph and has nowhere to put the embedding: that is the \
             ordering, and it is why the two are separate versions rather than one."
        ),
    ));
}

fn reserved_v12(c: &Campaign, d: &mut Vec<Diagnostic>) {
    if is_v12(c.quests.dsl_version.as_str()) {
        return;
    }
    crate::stages::for_each_campaign_effect(c, &mut |path, _site, eff| {
        let Some(verb) = eff.v12_effect() else {
            return;
        };
        d.push(Diagnostic::error(
            codes::RESERVED,
            "quests",
            path.to_string(),
            format!(
                "the `{verb}` effect (DSL v0.12, spec-0042: a campaign opening a placed piece's \
                 contingent way) requires dsl_version 0.12.0 — raise this stage's `dsl_version` \
                 to 0.12.0, or remove the effect"
            ),
        ));
    });
}

/// DSL v0.11 reserved-feature gating. **Two surfaces land in this version**, and
/// this is the one fence for both:
///
/// * spec-0034's per-body `traversal` declaration — what a body can do when it
///   moves — on the stage-2 NPC and the stage-5 actor;
/// * the **press-answer lift** — a `narrate` `actionbar` style and a trigger's
///   `audience: presser`.
///
/// **Fenced per stage, because the surface is per stage.** The stage-2 NPC's
/// traversal declaration is gated on the `npcs` document's own `dsl_version` and
/// the stage-5 actor's on the `quests` document's, so a campaign may adopt the
/// surface one stage at a time — which is what keeps every 0.2–0.10 campaign
/// compiling untouched. The press-answer lift is carried entirely by stage-5
/// documents, so its fence reads the `quests` version alone.
///
/// The press-answer additions are one capability and share one fence: without
/// the channel a press answer cannot be written where a player reads replies,
/// and without the addressee it is broadcast to a party three quarters of whom
/// are elsewhere. Together they are what makes `close-gate.sealed_hint`
/// expressible as an ordinary trigger.
///
/// Declaring any of it below 0.11.0 is `DW0141`. **The version does have a
/// requirement half**, and it is not this function's: at 0.11.0 and above a
/// sealed body nothing answers is `DW0429`, raised unconditionally by
/// [`press_obligation_checks`] and fenced by [`crate::fence`] off the code's own
/// [`crate::Binds::Since`] 11. Nothing obliges a body to declare traversal, and
/// nothing obliges a campaign to adopt either half of the lift — the obligation
/// binds a campaign that has ALREADY declared a sealed body, once its quests
/// stage reaches 0.11.0.
fn reserved_v11(c: &Campaign, d: &mut Vec<Diagnostic>) {
    // spec-0034: the per-body `traversal` declaration, stage 2.
    if !is_v11(c.npcs.dsl_version.as_str()) {
        for (i, npc) in c.npcs.content.npcs.iter().enumerate() {
            if npc.traversal.is_none() {
                continue;
            }
            d.push(Diagnostic::error(
                codes::RESERVED,
                "npcs",
                format!("/content/npcs/{i}/traversal"),
                format!(
                    "a per-body `traversal` declaration (what npc `{}` can do when it moves) \
                     requires dsl_version 0.11.0 — raise this stage's `dsl_version` to 0.11.0, or \
                     remove the field",
                    npc.id
                ),
            ));
        }
    }
    if !is_v11(c.quests.dsl_version.as_str()) {
        for (i, actor) in c.quests.content.actors.iter().enumerate() {
            if actor.traversal.is_none() {
                continue;
            }
            d.push(Diagnostic::error(
                codes::RESERVED,
                "quests",
                format!("/content/actors/{i}/traversal"),
                format!(
                    "a per-body `traversal` declaration (what actor `{}` can do when it moves) \
                     requires dsl_version 0.11.0 — raise this stage's `dsl_version` to 0.11.0, or \
                     remove the field",
                    actor.id
                ),
            ));
        }
    }
    // The press-answer lift, stage 5. Both halves ride the `quests` version, so
    // one guard carries them — and it is a SEPARATE guard from the actor
    // traversal one above rather than a shared `if`, because the two surfaces
    // are fenced independently and a future stage move must not silently take
    // the other with it.
    if !is_v11(c.quests.dsl_version.as_str()) {
        // The style rides `for_each_campaign_effect`, so a `narrate` nested
        // inside a `sequence` step or an `on_arrive` bundle is fenced exactly
        // like a top-level one — the fence is as total as the surface.
        crate::stages::for_each_campaign_effect(c, &mut |path, _site, eff| {
            if matches!(
                eff,
                QuestEffect::Narrate {
                    style: Some(NarrateStyle::Actionbar),
                    ..
                }
            ) {
                d.push(Diagnostic::error(
                    codes::RESERVED,
                    "quests",
                    format!("{path}/style"),
                    "the `actionbar` narrate style (the reply strip above the hotbar) requires \
                     dsl_version 0.11.0 — raise this stage's `dsl_version` to 0.11.0, or use \
                     `chat`/`title`/`subtitle`"
                        .to_string(),
                ));
            }
        });
        for (i, t) in c.quests.content.triggers.iter().enumerate() {
            if t.addresses_presser() {
                d.push(Diagnostic::error(
                    codes::RESERVED,
                    "quests",
                    format!("/content/triggers/{i}/audience"),
                    "a trigger `audience` of `presser` (the effects run as the one player who \
                     clicked, instead of addressing the party) requires dsl_version 0.11.0 — \
                     raise this stage's `dsl_version` to 0.11.0, or remove the field"
                        .to_string(),
                ));
            }
        }
    }
}

/// DSL v0.11 (spec-0034): a declared locomotion the engine cannot hold the body
/// to is refused at declaration time (`DW0455`).
///
/// Today that is exactly `aquatic`, and the reason is structural rather than a
/// taste call: `aquatic` carries no exemption and governs no rule, so declaring
/// it could never change a verdict — it would land in `DW0454` every time. A
/// value whose only outcome is another diagnostic is a trap, so it is refused
/// here with the gap named.
fn body_traversal_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    for site in body_traversal_sites(c) {
        if site.traversal.locomotion != Locomotion::Aquatic {
            continue;
        }
        let (stage, path, id) = (site.body.stage(), &site.path, site.body.id());
        d.push(Diagnostic::error(
            codes::TRAVERSAL_UNPROVABLE,
            stage,
            format!("{path}/locomotion"),
            format!(
                "`{id}` declares `locomotion: aquatic`, which the compiler cannot hold it to. \
                 `aquatic` is the one class that carries no exemption and governs no rule — it is \
                 a ledger label derived from vanilla's own `#minecraft:aquatic` tag — so the \
                 declaration could never change a verdict and would be reported inert (`DW0454`). \
                 The gap, stated rather than left to folklore: routing has ONE reachability model, \
                 standable ground, and water-flooded cells are impassable and never floor for \
                 EVERY body, so there is nothing for an aquatic claim to feed. Prescription: \
                 remove the declaration — a route that crosses water is already governed by the \
                 flooded-cell rules, and a body vanilla itself calls aquatic still reaches the \
                 traversal proof's binding ledger under its derived class."
            ),
        ));
    }
}

/// `DW0427`/`DW0428`: the two ways a trigger's **press answer** surface can be
/// declared wrong (DSL v0.11).
///
/// Both are about the trigger as an *object*, not about any effect inside it, so
/// they sit together and are checked over the one trigger authority.
fn press_answer_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    for (i, t) in c.quests.content.triggers.iter().enumerate() {
        if t.addresses_presser() && !matches!(t.on, TriggerOn::Use) {
            d.push(Diagnostic::error(
                codes::TRIGGER_AUDIENCE_UNATTRIBUTABLE,
                "quests",
                format!("/content/triggers/{i}/audience"),
                format!(
                    "trigger `{}` watches a `{}` and asks for `audience: presser`, but vanilla \
                     can only attribute a RIGHT-click to a player. \
                     `minecraft:player_interacted_with_entity` is the one criterion that runs a \
                     function as the clicker; a left-click is recorded in the interaction \
                     entity's `attack` NBT, which names a UUID no command can become, and an \
                     `approach` has no click at all. Guessing — polling the record and hoping the \
                     nearest player is the striker — is the kind of downstream folklore this \
                     engine refuses (CLAUDE.md: a capability with no vanilla primitive under it \
                     is excluded, not faked). Prescription: make it an `on: use` trigger, or drop \
                     `audience` and let the beat address the party",
                    t.id,
                    t.on.kind()
                ),
            ));
        }
        let local = crate::l10n::local_id(t.id.as_str());
        if local.starts_with(RESERVED_TRIGGER_PREFIX) {
            d.push(Diagnostic::error(
                codes::TRIGGER_ID_RESERVED,
                "quests",
                format!("/content/triggers/{i}/id"),
                format!(
                    "trigger id `{}` opens with `{RESERVED_TRIGGER_PREFIX}`, which the compiler \
                     reserves for the triggers it synthesizes itself — today the press answer \
                     every sealed gate and shortcut door gives (`trigger/dw-press-…`). Two \
                     triggers with one id would share one `dw_trig_…` tag and one emitted \
                     function, so one of them would silently vanish. Prescription: rename it; any \
                     kebab id not starting with `{RESERVED_TRIGGER_PREFIX}` is yours",
                    t.id
                ),
            ));
        }
    }
}

/// `DW0429`: **a sealed body the campaign never answers** (DSL v0.11),
/// uniformly over the pressable class.
///
/// A sealed thing is something the party walks up to and pushes on — a `shortcut`
/// door on the wrong side of the loop, a `close-gate`'s wall — and the press has
/// to say something. The compiler will not say it for them: a baked default is a
/// design statement (about tone, about what this thing is) made on the author's
/// behalf and never disclosed, so the obligation is stated instead of filled.
///
/// **One rule for the whole pressable class, not one per verb.** A shortcut door
/// and a sealed gate are two objects of the same class, and giving them two
/// defaulting policies would be exactly the "capability keyed to the verb" defect
/// CLAUDE.md's worked example is about — which this surface *is*. So above the
/// fence both are held to the same obligation, and `plan::press_answer_sites`
/// carries the single shared list they are read from.
///
/// **Two ways to discharge it**, and they are the same thing said at two layers:
///
/// * a `use` trigger anchored on the body — the general verb, available to every
///   pressable object (`QuestsContent::answers_press_at`);
/// * for a `close-gate`, an authored `sealed_hint` — the sugar, which *is* the
///   author defining the wording. The compiler lowering that onto the general
///   path is not the compiler putting words in a player's mouth.
///
/// A `strike` discharges neither: pressing a thing is a right-click, and a
/// left-click reply is a gesture the player may never make.
///
/// **Fenced on the quests stage's `dsl_version`.** This is a tightening, not new
/// surface, and the fence is what makes an already-approved design keep its
/// verdicts: the same declared version yields the same behaviour. Below
/// 0.11.0 a silent door still compiles and
/// still emits nothing, and an unauthored `close-gate` still takes the compiler's
/// canonical English — `plan::SilencePolicy`'s two grandfathered arms, which
/// differ from each other only because the two classes historically differed.
///
/// The fence is the general one: `DW0429` is declared [`Binds::Since`] 0.11.0 on
/// its own [`DwCode`](crate::DwCode), and [`crate::fence::Fenced`] grandfathers
/// it against a quests stage below that version. This check therefore raises the
/// diagnostic unconditionally and never tests a version itself — a private
/// `is_v11(...)` guard here would be a second, narrower copy of the mechanism
/// that already governs exactly this case.
fn press_obligation_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let quests = &c.quests.content;

    // Every gate anchor some `close-gate` seals, and whether any firing on it
    // authored a wording. Keyed by ANCHOR because the seal is a place, not an
    // event — the same reason `plan::collect_seal_hints` dedups by anchor and
    // `DW0423` refuses two firings that disagree.
    let mut sealed: BTreeMap<&str, (bool, String)> = BTreeMap::new();
    crate::stages::for_each_campaign_effect(c, &mut |path, _site, eff| {
        let Some(anchor) = eff.close_gate_anchor() else {
            return;
        };
        let entry = sealed
            .entry(anchor.as_str())
            .or_insert_with(|| (false, path.to_string()));
        entry.0 |= eff.close_gate_sealed_hint().is_some();
    });
    for (anchor, (authored, path)) in sealed {
        if authored || quests.answers_press_at(anchor) {
            continue;
        }
        d.push(Diagnostic::error(
            codes::SEALED_BODY_UNANSWERED,
            "quests",
            path,
            format!(
                "this `close-gate` seals `{anchor}`, and nothing says what the wall answers when \
                 the party presses it. A seal is a thing the party walks back to and pushes on, \
                 so the press has to say something — and the compiler will not word it for you: a \
                 baked default decides this wall's tone on your behalf and never tells you it \
                 did. Two ways to say it, and either is enough: add `\"sealed_hint\": \"<what the \
                 wall says>\"` to this effect, or anchor a trigger on the gate — \
                 `{{\"id\": \"trigger/<name>\", \"at\": \"{anchor}\", \"on\": {{\"on\": \"use\"}}, \
                 \"once\": false, \"audience\": \"presser\", \"effects\": [{{\"type\": \"narrate\", \
                 \"style\": \"actionbar\", \"text\": \"<what the wall says>\"}}]}}`. The trigger form \
                 is the general one and can carry a sound, a flag gate or any other effect"
            ),
        ));
    }

    for (i, sc) in quests.shortcuts.iter().enumerate() {
        let gate = sc.gate.as_str();
        if quests.answers_press_at(gate) {
            continue;
        }
        d.push(Diagnostic::error(
            codes::SEALED_BODY_UNANSWERED,
            "quests",
            format!("/content/shortcuts/{i}"),
            format!(
                "shortcut `{}` bars the gate `{gate}` from world-load, and nothing in the \
                 campaign answers a right-click on it — so a player who walks the long way \
                 round, arrives at the wrong side of the door and pushes on it is told nothing. \
                 That is the press a shortcut loop most invites. The compiler will not word it \
                 for you: a baked default would be the engine deciding this door's tone and \
                 never saying that it had. A `shortcut` carries no wording field, deliberately — \
                 the line is a trigger. Prescription: add a trigger anchored on the gate — \
                 `{{\"id\": \"trigger/<name>\", \"at\": \"{gate}\", \"on\": {{\"on\": \"use\"}}, \
                 \"once\": false, \"audience\": \"presser\", \"effects\": [{{\"type\": \"narrate\", \
                 \"style\": \"actionbar\", \"text\": \"<what the door says>\"}}]}}` — which rides \
                 the door's own hitboxes, fires only from the sealed side, and retires when the \
                 door opens. Any `use` trigger on `{gate}` discharges this, whatever it does",
                sc.id
            ),
        ));
    }
}

/// The id prefix the compiler reserves for triggers it synthesizes
/// (`plan::press_answer_trigger_id`). Stated here because the *reservation* is a
/// DSL-level fact even though today's only user is in the compiler.
const RESERVED_TRIGGER_PREFIX: &str = "dw-";

/// DSL v0.10 reserved-feature gating (spec-0031). **Two surfaces land in this
/// version**, and this is the one fence for both:
///
/// * runtime state — the stage-5 `state[]` declaration, the
///   `set-state`/`add-state`/`clear-state` verbs, and the `requires_state`
///   comparison on every gate consumer;
/// * the campaign-wide `on_death` bundle — effect root R7, the effects that run
///   at the moment a player dies;
/// * the stage-5 `lethal_volumes` declaration — a box that kills whatever enters
///   it;
/// * the status-effect pair `give-effect` / `clear-effect` and the region
///   `teleport` — three more verbs reported by `v10_effect`, so they ride the
///   same one walk rather than adding a fence of their own.
/// * spec-0032's trade and recovery surface — the stage-5 `shops` and `stakes`
///   declarations, the `drop-stake` verb, and a datum's player-visible `name`.
///
/// The same asymmetry every version ledger uses: *declaring* any of it below
/// 0.10.0 is `DW0141`. There is no requirement half — nothing obliges a campaign
/// to hold a datum, a death beat or a killing box, and one that declares none of
/// them emits exactly
/// what it emitted before (no scoreboard objective, no guard clause, no
/// function, the whole `dw.death_seen` half of the death edge absent, and no
/// lethal tick call), which is what keeps every 0.2–0.9 campaign's datapack
/// byte-for-byte unchanged.
///
/// The `requires_state` half reads the gates from
/// [`for_each_gate`](crate::gate::for_each_gate), the closed consumer
/// enumeration, rather than from a list of the sites this function's author
/// happened to remember: the fence is exactly as total as the surface.
fn reserved_v10(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let quests_ok = is_v10(c.quests.dsl_version.as_str());
    let dialogue_ok = is_v10(c.dialogue.dsl_version.as_str());
    if quests_ok && dialogue_ok {
        return;
    }
    if !quests_ok && !c.quests.content.on_death.is_empty() {
        d.push(Diagnostic::error(
            codes::RESERVED,
            "quests",
            "/content/on_death".to_string(),
            "the campaign-wide `on_death` bundle (the effects that run at the moment a player \
             dies) requires dsl_version 0.10.0 — raise this stage's `dsl_version` to 0.10.0, or \
             remove the section"
                .to_string(),
        ));
    }
    if !quests_ok && !c.quests.content.lethal_volumes.is_empty() {
        d.push(Diagnostic::error(
            codes::RESERVED,
            "quests",
            "/content/lethal_volumes".to_string(),
            "a `lethal_volumes` declaration (a box that kills whatever enters it) requires \
             dsl_version 0.10.0 — raise this stage's `dsl_version` to 0.10.0, or remove the field"
                .to_string(),
        ));
    }
    if !quests_ok && !c.quests.content.shops.is_empty() {
        d.push(Diagnostic::error(
            codes::RESERVED,
            "quests",
            "/content/shops".to_string(),
            "the stage-5 `shops` declaration (an interaction point that opens a list of gated \
             offers) requires dsl_version 0.10.0 — raise this stage's `dsl_version` to 0.10.0, or \
             remove the section"
                .to_string(),
        ));
    }
    if !quests_ok && !c.quests.content.stakes.is_empty() {
        d.push(Diagnostic::error(
            codes::RESERVED,
            "quests",
            "/content/stakes".to_string(),
            "the stage-5 `stakes` declaration (what a death forfeits and how it comes back) \
             requires dsl_version 0.10.0 — raise this stage's `dsl_version` to 0.10.0, or remove \
             the section"
                .to_string(),
        ));
    }
    if !quests_ok
        && let Some((i, s)) = c
            .quests
            .content
            .state
            .iter()
            .enumerate()
            .find(|(_, s)| s.name.is_some())
    {
        d.push(Diagnostic::error(
            codes::RESERVED,
            "quests",
            format!("/content/state/{i}/name"),
            format!(
                "a runtime datum's player-visible `name` (which makes `{}` a currency the player \
                 reads on the action bar) requires dsl_version 0.10.0 — raise this stage's \
                 `dsl_version` to 0.10.0, or remove the field",
                s.id.as_str()
            ),
        ));
    }
    if !quests_ok && !c.quests.content.state.is_empty() {
        d.push(Diagnostic::error(
            codes::RESERVED,
            "quests",
            "/content/state".to_string(),
            "the stage-5 `state` declaration (named, scoped, integer-valued runtime data) \
             requires dsl_version 0.10.0 — raise this stage's `dsl_version` to 0.10.0, or remove \
             the section"
                .to_string(),
        ));
    }
    crate::gate::for_each_gate(c, &mut |site, gate| {
        if gate.requires_state.is_empty() {
            return;
        }
        let stage = site.consumer.stage();
        let staged_ok = if stage == "dialogue" {
            dialogue_ok
        } else {
            quests_ok
        };
        if staged_ok {
            return;
        }
        d.push(Diagnostic::error(
            codes::RESERVED,
            stage,
            format!("{}/requires_state", site.path),
            format!(
                "a `requires_state` numeric gate on a {} requires dsl_version 0.10.0 — raise \
                 this stage's `dsl_version` to 0.10.0, or remove the comparison",
                site.consumer.label()
            ),
        ));
    });
    if quests_ok {
        return;
    }
    crate::stages::for_each_campaign_effect(c, &mut |path, _site, eff| {
        let Some(verb) = eff.v10_effect() else {
            return;
        };
        d.push(Diagnostic::error(
            codes::RESERVED,
            "quests",
            path.to_string(),
            format!(
                "the `{verb}` effect (DSL v0.10, spec-0031: runtime state, status effects and \
                 the region teleport) requires dsl_version 0.10.0 — raise this stage's \
                 `dsl_version` to 0.10.0, or remove the effect"
            ),
        ));
    });
}

/// DSL v0.10 runtime-state checks (spec-0031): every reference resolves, every
/// read has a writer, every datum has a reader, and a `player`-scoped datum is
/// only touched where emission has a player to touch it against.
///
/// Both halves of the read/write obligation are here on purpose. A datum a gate
/// reads and no verb writes is a comparison whose answer was fixed when the
/// campaign was written; a datum a verb writes and no gate reads is bookkeeping
/// no player can observe. Each is a **vacuous binding** in the CLAUDE.md sense,
/// and each is silent — the campaign compiles, the datapack loads, and the delve
/// plays as though the mechanism were live.
///
/// The reads come from [`for_each_gate`](crate::gate::for_each_gate) and the
/// writes from
/// [`for_each_campaign_effect`](crate::stages::for_each_campaign_effect) — the
/// two closed enumerations — so neither side of the ledger can drift narrower
/// than the surface it polices.
/// DSL v0.10 status-effect checks (spec-0031): a real effect id, a duration that
/// is actually a duration, and the one *pattern* that reintroduces the hazard the
/// mandatory duration exists to remove.
///
/// ## Why the duration is not enough on its own
///
/// `give-effect` has no infinite form: `seconds` is required and bounded, so a
/// grant always ends by itself. That is the whole design, and it can still be
/// defeated by two effects that are individually fine — grant blindness for an
/// hour, clear it four ticks later. The clear is then the real removal, and a
/// bundle that does not reach it (a logout, a crash, a death mid-chain, a
/// `sequence` whose remaining `schedule` never runs) leaves the player blind for
/// the rest of the hour. `DW0540` is that pattern, and it fires on exactly the
/// grants that are still LIVE when their clear arrives: where the duration
/// expires first, the duration is the removal and there is nothing to say.
///
/// ## What "the same sequence" means, mechanically
///
/// A bundle's own timeline: every effect it fires, at the tick offset it fires
/// on. A plain member runs at offset 0; a directly-nested `sequence`'s members
/// run at their step's `at_ticks` (nested sequences are `DW0329`, so the
/// expansion terminates). Conditional continuations — `on_arrive`, `on_caught`,
/// `on_respawn`, `on_rest` — are NOT folded in: they are separate bundles with
/// their own timelines, and each is walked as one. A clear that hangs off an
/// arrival is strictly more fragile than one on a fixed tick, and it is the
/// mandatory duration, not this rule, that keeps that case survivable.
fn status_effect_checks(c: &Campaign, effects: &dyn EffectRegistry, d: &mut Vec<Diagnostic>) {
    crate::effects::for_each_effect_root(c, &mut |site, list| {
        status_effect_bundle(site.stage, &site.path, list, effects, d);
    });
}

/// One effect bundle: its own timeline is checked for the grant/clear pattern,
/// then each member is checked individually and its nested bundles are walked.
fn status_effect_bundle(
    stage: &'static str,
    path: &str,
    list: &[QuestEffect],
    effects: &dyn EffectRegistry,
    d: &mut Vec<Diagnostic>,
) {
    let tl = effect_timeline(path, list);
    check_grant_removal(stage, &tl, d);
    for (_, p, eff) in &tl {
        check_one_status_effect(stage, p, eff, effects, d);
        for (pseg, _key, inner) in eff.nested_effect_lists_labeled() {
            status_effect_bundle(stage, &format!("{p}/{pseg}"), inner, effects, d);
        }
    }
}

/// A bundle's timeline: `(tick offset, json pointer, effect)` for every effect
/// that runs on it, in declaration order.
///
/// A `sequence` is REPLACED by its steps' effects at their `at_ticks`, so the
/// timeline is what actually happens rather than what is written — which is the
/// only view in which "the grant is still live when the clear fires" is a
/// question with an answer.
fn effect_timeline<'a>(path: &str, list: &'a [QuestEffect]) -> Vec<(u32, String, &'a QuestEffect)> {
    let mut out = Vec::new();
    for (i, eff) in list.iter().enumerate() {
        match eff {
            QuestEffect::Sequence { steps } => {
                for (s, step) in steps.iter().enumerate() {
                    for (k, inner) in step.effects.iter().enumerate() {
                        out.push((
                            step.at_ticks,
                            format!("{path}/{i}/steps/{s}/effects/{k}"),
                            inner,
                        ));
                    }
                }
            }
            _ => out.push((0, format!("{path}/{i}"), eff)),
        }
    }
    out
}

/// `DW0540`: a grant that is still live when a clear in the same bundle removes
/// it.
///
/// Each grant is paired with the EARLIEST clear that could remove it: the one
/// smallest in `(tick, declaration position)` among those strictly after the
/// grant, that either names the same effect or names none at all (vanilla's
/// `effect clear <targets>` with no id clears everything, and so does this verb).
///
/// **Ordered by the key, never by the loop index.** A bundle's steps are declared
/// in whatever order the author wrote them, not in tick order, so "the next clear
/// in the list" and "the clear that actually fires first" are different effects
/// the moment a `sequence` declares `at_ticks: 40` above `at_ticks: 5`. Taking
/// the first in declaration order made the rule miss exactly the live removal it
/// exists to find — and, symmetrically, miss a clear declared *above* a grant but
/// scheduled after it.
fn check_grant_removal(
    stage: &'static str,
    timeline: &[(u32, String, &QuestEffect)],
    d: &mut Vec<Diagnostic>,
) {
    for (gi, (g_tick, g_path, g_eff)) in timeline.iter().enumerate() {
        let Some((id, seconds, _, _, _)) = g_eff.give_effect() else {
            continue;
        };
        // `blindness` and `minecraft:blindness` are one effect to vanilla and to
        // the registry, so they are one effect here (`namespaced_effect_id`) —
        // an exact string compare would have made the rule silently miss the
        // pair whenever the two sites spelled the id differently.
        let granted = crate::registry::namespaced_effect_id(id);
        let removal = timeline
            .iter()
            .enumerate()
            .filter(|(ci, (t, _, _))| (*t, *ci) > (*g_tick, gi))
            .filter_map(|(ci, (t, p, e))| match e.clear_effect() {
                Some((None, _)) => Some(((*t, ci), t, p, "clears every effect")),
                Some((Some(c), _)) if crate::registry::namespaced_effect_id(c) == granted => {
                    Some(((*t, ci), t, p, "clears it"))
                }
                _ => None,
            })
            .min_by_key(|(key, _, _, _)| *key)
            .map(|(_, t, p, what)| (*t, p, what));
        let Some((c_tick, c_path, what)) = removal else {
            continue;
        };
        let live_ticks = seconds.saturating_mul(20);
        let elapsed = c_tick - g_tick;
        if elapsed >= live_ticks {
            // The duration ended first, so the duration IS the removal and the
            // clear is inert. That is a different (and much milder) finding than
            // this rule makes, so this rule says nothing about it.
            continue;
        }
        d.push(Diagnostic::error(
            codes::EFFECT_CLEARED_LIVE,
            stage,
            g_path.clone(),
            format!(
                "this `give-effect` grants `{id}` for {seconds}s ({live_ticks} ticks) and the \
                 `clear-effect` at `{c_path}`, {elapsed} tick(s) later in the same bundle, {what} \
                 while it is still live — so the CLEAR is what ends this grant, not its duration. \
                 Every path that does not reach that clear (a logout, a crash, a death mid-chain, \
                 a `sequence` whose remaining schedule never runs) leaves the player holding \
                 `{id}` for the rest of the {seconds}s. Set `seconds` to how long the effect \
                 should actually last — {elapsed} tick(s) here, so {} — and drop the \
                 `clear-effect`: a duration expires with no cooperation from anything. \
                 `clear-effect` is for effects this campaign did not grant.",
                if elapsed == 0 {
                    "the grant is doing nothing at all".to_string()
                } else {
                    format!("`seconds`: {}", elapsed.div_ceil(20).max(1))
                }
            ),
        ));
    }
}

/// The per-effect half: a real status-effect id (`DW0192`, the same registry
/// wave-mob effects and kit potions validate against) and a duration/amplifier
/// inside vanilla's own field widths (`DW0541`).
fn check_one_status_effect(
    stage: &'static str,
    path: &str,
    eff: &QuestEffect,
    effects: &dyn EffectRegistry,
    d: &mut Vec<Diagnostic>,
) {
    for (sub, id) in eff.status_effect_refs() {
        if !effects.contains(id) {
            d.push(Diagnostic::error(
                codes::EFFECT_UNKNOWN,
                stage,
                format!("{path}/{sub}"),
                format!(
                    "`{}` names status effect `{id}`, which is not in the pinned 1.21.11 \
                     `mob_effect` registry — use a valid namespaced effect id (e.g. \
                     `minecraft:blindness`)",
                    eff.verb()
                ),
            ));
        }
    }
    let Some((_, seconds, amplifier, _, _)) = eff.give_effect() else {
        return;
    };
    if seconds == 0 || seconds > crate::stages::MAX_EFFECT_SECONDS {
        d.push(Diagnostic::error(
            codes::EFFECT_GRANT_BOUNDS,
            stage,
            format!("{path}/seconds"),
            format!(
                "`give-effect` duration {seconds}s is out of range — it must be between 1 and {} \
                 seconds. {}",
                crate::stages::MAX_EFFECT_SECONDS,
                if seconds == 0 {
                    "Zero grants nothing at all: the effect is applied and gone before the next \
                     tick, so the beat reports green and the player sees nothing."
                } else {
                    "The ceiling is vanilla's own field width (past the 10-hour delve ceiling), \
                     so a value above it is a duration typed in ticks or milliseconds."
                }
            ),
        ));
    }
    if amplifier > crate::stages::MAX_POTION_AMPLIFIER {
        d.push(Diagnostic::error(
            codes::EFFECT_GRANT_BOUNDS,
            stage,
            format!("{path}/amplifier"),
            format!(
                "`give-effect` amplifier {amplifier} is out of range — vanilla stores it in an \
                 unsigned byte, so {} is the end of the field, not a policy",
                crate::stages::MAX_POTION_AMPLIFIER
            ),
        ));
    }
}

fn state_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let decls = &c.quests.content.state;
    // --- the declarations themselves ------------------------------------------
    let mut declared: BTreeMap<&str, &crate::stages::StateDecl> = BTreeMap::new();
    for (i, s) in decls.iter().enumerate() {
        if !s.id.is_valid_syntax() {
            d.push(Diagnostic::error(
                codes::ID_SYNTAX,
                "quests",
                format!("/content/state/{i}/id"),
                format!(
                    "runtime-state id `{}` is malformed — ids are `state/<kebab-case>`",
                    s.id.as_str()
                ),
            ));
            continue;
        }
        if declared.insert(s.id.as_str(), s).is_some() {
            d.push(Diagnostic::error(
                codes::ID_DUPLICATE,
                "quests",
                format!("/content/state/{i}/id"),
                format!(
                    "runtime-state id `{}` is declared more than once — one datum, one \
                     declaration (its scope and its initial value have to be a single fact)",
                    s.id.as_str()
                ),
            ));
        }
    }

    // --- the reads: every gate's `requires_state`, from the closed set --------
    let mut read: BTreeSet<String> = BTreeSet::new();
    crate::gate::for_each_gate(c, &mut |site, gate| {
        for (k, cmp) in gate.requires_state.iter().enumerate() {
            let path = format!("{}/requires_state/{k}", site.path);
            let stage = site.consumer.stage();
            match declared.get(cmp.state.as_str()) {
                None => d.push(Diagnostic::error(
                    codes::STATE_UNDECLARED,
                    stage,
                    path,
                    format!(
                        "`requires_state` on this {} compares `{}`, which the campaign never \
                         declares. Add it to the stage-5 `state` list (a datum's scope and its \
                         initial value are facts no use site can supply), or fix the id",
                        site.consumer.label(),
                        cmp.state.as_str()
                    ),
                )),
                Some(decl) => {
                    read.insert(cmp.state.as_str().to_string());
                    // An EFFECT's gate is evaluated wherever its bundle runs, and
                    // that is the root's fact, not the effect's — so
                    // `evaluates_per_player` answers `None` here and the scope
                    // check for effects happens in the root walk below, which
                    // knows both the root's audience and the seams inside it.
                    if decl.scope == crate::stages::StateScope::Player
                        && site.consumer.evaluates_per_player() == Some(false)
                    {
                        d.push(Diagnostic::error(
                            codes::STATE_SCOPE_UNREACHABLE,
                            stage,
                            path,
                            format!(
                                "`{}` is `player`-scoped, but emission evaluates a {}'s gate \
                                 against the party holder — there is no acting player to read it \
                                 from. Declare the datum `party`-scoped, or move the comparison \
                                 onto a site a player drives (a dialogue option, a cast \
                                 placement, or an effect on a beat a player completes)",
                                cmp.state.as_str(),
                                site.consumer.label()
                            ),
                        ));
                    }
                }
            }
        }
    });

    // --- the writes: every state verb, at every effect root, nesting included -
    let mut written: BTreeSet<String> = BTreeSet::new();
    crate::stages::for_each_campaign_effect(c, &mut |path, _site, eff| {
        let Some((id, _)) = eff.writes_state() else {
            return;
        };
        match declared.get(id.as_str()) {
            None => d.push(Diagnostic::error(
                codes::STATE_UNDECLARED,
                "quests",
                format!("{path}/state"),
                format!(
                    "`{}` writes `{}`, which the campaign never declares. Add it to the stage-5 \
                     `state` list, or fix the id",
                    eff.verb(),
                    id.as_str()
                ),
            )),
            Some(_) => {
                written.insert(id.as_str().to_string());
            }
        }
    });
    // A `player`-scoped datum read or written where there is no acting player
    // has no subject, exactly as a `carrier: "one"` give does (`DW0357`).
    //
    // **The latch starts from the ROOT**, not from `false`. Four of the seven
    // roots run with an acting player and three do not
    // (`EffectRootKind::runs_with_acting_player` — a trigger's effects, a trap's
    // payload and a shortcut's `on_unlock` are all polled on the tick with no
    // executor), and inside a bundle the `sequence` / `on_arrive` seams drop the
    // actor the same way. Seeding it `false` — as this walk first did — read
    // every root as player-bearing and let three of the seven through.
    crate::effects::for_each_effect_root(c, &mut |site, effs| {
        // Per SITE, not per kind (DSL v0.11): a trigger declaring
        // `audience: presser` is dispatched by the interaction advancement and so
        // DOES have an acting player, while every other trigger is polled with
        // none. Asking the kind would refuse a `player`-scoped read that the
        // emitter can serve — the mirror of the bug this seed was added to fix.
        let scheduled = !site.runs_with_acting_player();
        check_player_state_not_scheduled(effs, &declared, site.stage, &site.path, scheduled, d);
    });

    // --- the two halves of the vacuity ledger ---------------------------------
    for (i, s) in decls.iter().enumerate() {
        let id = s.id.as_str();
        // A malformed or duplicate id has already been reported; reporting it a
        // third time as "unread" would be noise about a datum that does not exist.
        if declared.get(id).is_none_or(|kept| !std::ptr::eq(*kept, s)) {
            continue;
        }
        if read.contains(id) && !written.contains(id) {
            d.push(Diagnostic::error(
                codes::STATE_NEVER_WRITTEN,
                "quests",
                format!("/content/state/{i}"),
                format!(
                    "`{id}` is read by a gate but no `set-state`/`add-state`/`clear-state` \
                     anywhere in the campaign ever writes it — it can only ever hold its declared \
                     initial ({}), so every comparison against it was decided when the campaign \
                     was written. Write it somewhere, or drop the comparison and say what you \
                     meant unconditionally",
                    s.initial
                ),
            ));
        }
        if !read.contains(id) {
            let tail = if written.contains(id) {
                "some verb writes it and nothing ever asks"
            } else {
                "nothing touches it at all"
            };
            d.push(Diagnostic::error(
                codes::STATE_NEVER_READ,
                "quests",
                format!("/content/state/{i}"),
                format!(
                    "`{id}` is declared but no gate's `requires_state` anywhere in the campaign \
                     ever reads it — {tail}. Runtime state exists to be compared against; gate \
                     something on it, or delete the declaration and its writes"
                ),
            ));
        }
    }
}

/// Reject a `player`-scoped datum **read or written** where emission has no
/// acting player (`DW0503`).
///
/// `scheduled` arrives already seeded from the ROOT
/// ([`EffectRootKind::runs_with_acting_player`](crate::EffectRootKind::runs_with_acting_player)),
/// and from there the seams and the latch semantics are
/// [`check_carrier_one_not_scheduled`]'s, deliberately: a `sequence` step and a
/// `move-npc`/`move-actor` `on_arrive` are re-invoked with the server command
/// source, while a `set-checkpoint`'s `on_respawn` and a `begin-stealth`'s
/// `on_caught` are dispatched per player and so reset the latch.
///
/// Reads and writes are checked together because they fail the same way: a
/// per-player score named from a sourceless function is `@s` with nothing to
/// resolve it to, whether the command is a `scoreboard players set` or an
/// `execute if score`.
fn check_player_state_not_scheduled(
    effs: &[QuestEffect],
    declared: &BTreeMap<&str, &crate::stages::StateDecl>,
    stage: &str,
    path: &str,
    scheduled: bool,
    d: &mut Vec<Diagnostic>,
) {
    let is_player = |id: &str| {
        declared
            .get(id)
            .is_some_and(|s| s.scope == crate::stages::StateScope::Player)
    };
    for e in effs {
        if scheduled {
            for cmp in e.requires_state() {
                if is_player(cmp.state.as_str()) {
                    d.push(Diagnostic::error(
                        codes::STATE_SCOPE_UNREACHABLE,
                        stage,
                        path.to_string(),
                        format!(
                            "a `{}` effect's `requires_state` compares `{}`, which is \
                             `player`-scoped, in a bundle that runs with no acting player (a \
                             trigger's effects, a trap's payload and a shortcut's `on_unlock` \
                             are polled on the tick from the server command source; so are a \
                             `sequence` step and a `move-npc`/`move-actor` `on_arrive`). There \
                             is no player to read the datum from. Declare it `party`-scoped, or \
                             move the comparison onto a beat a player completes",
                            e.verb(),
                            cmp.state.as_str()
                        ),
                    ));
                }
            }
        }
        if scheduled
            && let Some((id, _)) = e.writes_state()
            && is_player(id.as_str())
        {
            d.push(Diagnostic::error(
                codes::STATE_SCOPE_UNREACHABLE,
                stage,
                path.to_string(),
                format!(
                    "`{}` writes `{}`, which is `player`-scoped, from a bundle that runs with no \
                     acting player (a trigger's effects, a trap's payload and a shortcut's \
                     `on_unlock` are polled on the tick from the server command source; so are a \
                     `sequence` step and a `move-npc`/`move-actor` `on_arrive`). There is no \
                     acting player whose datum this would be, so the write would silently reach \
                     nobody. Declare the datum `party`-scoped, or move the write onto a beat a \
                     player completes",
                    e.verb(),
                    id.as_str()
                ),
            ));
        }
        match e {
            // Dispatched per player; they reset the latch.
            QuestEffect::SetCheckpoint { on_respawn, .. } => {
                check_player_state_not_scheduled(on_respawn, declared, stage, path, false, d);
            }
            QuestEffect::BeginStealth { on_caught, .. } => {
                check_player_state_not_scheduled(on_caught, declared, stage, path, false, d);
            }
            // The scheduler-only seams (see `emit::Audience::Scheduled`).
            QuestEffect::Sequence { steps } => {
                for st in steps {
                    check_player_state_not_scheduled(&st.effects, declared, stage, path, true, d);
                }
            }
            QuestEffect::MoveActor { on_arrive, .. } | QuestEffect::MoveNpc { on_arrive, .. } => {
                check_player_state_not_scheduled(on_arrive, declared, stage, path, true, d);
            }
            QuestEffect::Bonfire { on_rest, .. } => {
                check_player_state_not_scheduled(on_rest, declared, stage, path, scheduled, d);
            }
            _ => {}
        }
    }
}

/// DSL v0.9 reserved-feature gating: the
/// declared-drops surface — `drops[]` on a wave mob and on an actor, and the
/// `collect` `dropped_by` that sources a quest item off a body instead of out of
/// a box.
///
/// The same asymmetry every version ledger uses: *declaring* any of it below
/// 0.9.0 is `DW0141`. There is no requirement half — a campaign that declares no
/// drops keeps writing drop chance `0.0` on every slot, which is byte-for-byte
/// what pre-0.9 emission wrote.
fn reserved_v09(c: &Campaign, d: &mut Vec<Diagnostic>) {
    if is_v09(c.quests.dsl_version.as_str()) {
        return;
    }
    for (i, w) in c.quests.content.waves.iter().enumerate() {
        for (k, m) in w.mobs.iter().enumerate() {
            if m.drops.is_empty() {
                continue;
            }
            d.push(Diagnostic::error(
                codes::RESERVED,
                "quests",
                format!("/content/waves/{i}/mobs/{k}/drops"),
                "a wave mob's `drops` (the declared subset an elite or boss leaves behind) \
                 requires dsl_version 0.9.0 — raise this stage's `dsl_version` to 0.9.0, or \
                 remove the field"
                    .to_string(),
            ));
        }
    }
    for (i, a) in c.quests.content.actors.iter().enumerate() {
        if a.drops.is_empty() {
            continue;
        }
        d.push(Diagnostic::error(
            codes::RESERVED,
            "quests",
            format!("/content/actors/{i}/drops"),
            "an actor's `drops` (the declared subset an elite or boss leaves behind) requires \
             dsl_version 0.9.0 — raise this stage's `dsl_version` to 0.9.0, or remove the field"
                .to_string(),
        ));
    }
    for (i, q) in c.quests.content.quests.iter().enumerate() {
        for (j, o) in q.objectives.iter().enumerate() {
            if o.collect_dropped_by().is_none() {
                continue;
            }
            d.push(Diagnostic::error(
                codes::RESERVED,
                "quests",
                format!("/content/quests/{i}/objectives/{j}/dropped_by"),
                "a `collect` `dropped_by` (the wave whose declared drop provides this item, in \
                 place of a container) requires dsl_version 0.9.0 — raise this stage's \
                 `dsl_version` to 0.9.0, or remove the field"
                    .to_string(),
            ));
        }
    }
}

/// DSL v0.7 reserved-feature gating (spec-0020): the per-quest `cast` ledger,
/// plus the `interact.missing_item_hint` empty-hand narration.
///
/// Note the asymmetry with the deprecation window. *Declaring* a ledger below
/// v0.7 is `DW0141` like any other newer construct — the version contract stays
/// exact. What the window (`DW0465`, compiler tier) forgives is the **absence**
/// of a ledger in a pre-0.7 campaign: those keep building, with a warning, for
/// one version.
fn reserved_v07(c: &Campaign, d: &mut Vec<Diagnostic>) {
    if is_v07(c.quests.dsl_version.as_str()) {
        return;
    }
    for (i, w) in c.quests.content.waves.iter().enumerate() {
        if w.tier.is_none() {
            continue;
        }
        d.push(Diagnostic::error(
            codes::RESERVED,
            "quests",
            format!("/content/waves/{i}/tier"),
            "a wave `tier` (`elite`/`boss` — what the validation ladder's floor gate holds the \
             encounter to) requires dsl_version 0.7.0 — raise this stage's `dsl_version` to \
             0.7.0, or remove the field"
                .to_string(),
        ));
    }
    for (i, q) in c.quests.content.quests.iter().enumerate() {
        for (j, o) in q.objectives.iter().enumerate() {
            if let Objective::Interact {
                missing_item_hint: Some(_),
                ..
            } = o
            {
                d.push(Diagnostic::error(
                    codes::RESERVED,
                    "quests",
                    format!("/content/quests/{i}/objectives/{j}/missing_item_hint"),
                    "`interact.missing_item_hint` (the line narrated when a player clicks \
                     without the required item in hand) requires dsl_version 0.7.0 — raise this \
                     stage's `dsl_version` to 0.7.0, or remove the field"
                        .to_string(),
                ));
            }
        }
        if q.cast.is_empty() {
            continue;
        }
        d.push(Diagnostic::error(
            codes::RESERVED,
            "quests",
            format!("/content/quests/{i}/cast"),
            "the per-quest `cast` ledger (where each NPC is, what they are doing, and what their \
             right-click offers) requires dsl_version 0.7.0 — raise this stage's `dsl_version` to \
             0.7.0"
                .to_string(),
        ));
    }
}

/// DSL v0.6 reserved-feature gating + validation. Several independent,
/// stage-gated feature groups land under `dsl_version 0.6.0`, each rejected with
/// `DW0141` in a pre-0.6 campaign (all fields default to absent/empty, so a
/// v0.5-or-earlier campaign that uses none is byte-identical):
/// - **spec-0013 (stage 1)**: `horizon` + `boundary`. Under a v0.6 world,
///   `horizon: "ocean"` requires a `boundary` (`DW0320`) and `boundary.margin`
///   must lie in `0..=64` (`DW0321`).
/// - **spec-0012 / spec-0014 (stages 5/6)**: the `set-checkpoint` effect, the
///   `begin-stealth`/`end-stealth` verbs, the `play-sound` effect and the
///   `narrate` `art` style. (Sound-id `DW0326` and art-glyph `DW0328` are
///   compiler-side checks.)
/// - **spec-0014 (stage 5)**: scripted `actors` + the staging effects
///   (`spawn-actor`/`despawn-actor`/`move-actor`/`unleash-actor`, `sequence`).
/// - **stage 5**: the per-effect `requires_flags` flag gate on
///   quest/trigger effects (see [`reserved_v06_effect_flags`]).
fn reserved_v06(c: &Campaign, d: &mut Vec<Diagnostic>) {
    reserved_v06_world(c, d);
    reserved_v06_effect_flags(c, d);
}

/// Per-effect `requires_flags` is a v0.6 quests-stage surface: any
/// quest effect (`on_objective_complete` / `on_complete`) or environment-trigger
/// effect that carries a non-empty `requires_flags` under a pre-0.6 quests stage
/// is reserved (`DW0141`). `campaign-complete` cannot carry the field at all.
///
/// `forbids_flags` (the negative gate) is likewise a v0.6 surface **everywhere
/// it is accepted** — objectives, environment triggers, quest/trigger effects,
/// and dialogue options (the dialogue-stage form is gated on the dialogue
/// stage's version, mirroring how dialogue `requires_flags` was v0.4-gated).
fn reserved_v06_effect_flags(c: &Campaign, d: &mut Vec<Diagnostic>) {
    if !is_v06(c.dialogue.dsl_version.as_str()) {
        for (i, t) in c.dialogue.content.dialogues.iter().enumerate() {
            for (j, node) in t.nodes.iter().enumerate() {
                for (k, opt) in node.options.iter().enumerate() {
                    if !opt.forbids_flags.is_empty() {
                        d.push(Diagnostic::error(
                            codes::RESERVED,
                            "dialogue",
                            format!("/content/dialogues/{i}/nodes/{j}/options/{k}/forbids_flags"),
                            "dialogue option `forbids_flags` (negative flag gating) requires \
                             dsl_version 0.6.0 — raise this stage's `dsl_version` to 0.6.0, or \
                             remove the field"
                                .to_string(),
                        ));
                    }
                }
            }
        }
    }
    if is_v06(c.quests.dsl_version.as_str()) {
        return;
    }
    let req_msg = "effect `requires_flags` (per-effect flag gating) requires dsl_version 0.6.0 — \
                   raise this stage's `dsl_version` to 0.6.0, or remove the field";
    let fbd_msg = "`forbids_flags` (negative flag gating) requires dsl_version 0.6.0 — raise this \
                   stage's `dsl_version` to 0.6.0, or remove the field";
    for (i, q) in c.quests.content.quests.iter().enumerate() {
        for (j, obj) in q.objectives.iter().enumerate() {
            if !obj.forbids_flags().is_empty() {
                d.push(Diagnostic::error(
                    codes::RESERVED,
                    "quests",
                    format!("/content/quests/{i}/objectives/{j}/forbids_flags"),
                    fbd_msg.to_string(),
                ));
            }
        }
        for_each_effect(q, |path, eff| {
            if !eff.requires_flags().is_empty() {
                d.push(Diagnostic::error(
                    codes::RESERVED,
                    "quests",
                    format!("/content/quests/{i}/{path}/requires_flags"),
                    req_msg.to_string(),
                ));
            }
            if !eff.forbids_flags().is_empty() {
                d.push(Diagnostic::error(
                    codes::RESERVED,
                    "quests",
                    format!("/content/quests/{i}/{path}/forbids_flags"),
                    fbd_msg.to_string(),
                ));
            }
        });
    }
    for (i, t) in c.quests.content.triggers.iter().enumerate() {
        // `strike-npc` (the body-targeting trigger form) is a v0.6 surface.
        if matches!(t.on, crate::stages::TriggerOn::StrikeNpc { .. }) {
            d.push(Diagnostic::error(
                codes::RESERVED,
                "quests",
                format!("/content/triggers/{i}/on"),
                "trigger `on: strike-npc` (targeting an NPC's body rather than a cell) requires \
                 dsl_version 0.6.0 — raise this stage's `dsl_version` to 0.6.0, or use `strike` \
                 at an anchor"
                    .to_string(),
            ));
        }
        if !t.forbids_flags.is_empty() {
            d.push(Diagnostic::error(
                codes::RESERVED,
                "quests",
                format!("/content/triggers/{i}/forbids_flags"),
                fbd_msg.to_string(),
            ));
        }
        for (m, eff) in t.effects.iter().enumerate() {
            if !eff.requires_flags().is_empty() {
                d.push(Diagnostic::error(
                    codes::RESERVED,
                    "quests",
                    format!("/content/triggers/{i}/effects/{m}/requires_flags"),
                    req_msg.to_string(),
                ));
            }
            if !eff.forbids_flags().is_empty() {
                d.push(Diagnostic::error(
                    codes::RESERVED,
                    "quests",
                    format!("/content/triggers/{i}/effects/{m}/forbids_flags"),
                    fbd_msg.to_string(),
                ));
            }
        }
    }
}

/// Stage-1 `horizon`/`boundary` gating + validation (spec-0013), plus the
/// stage-5 v0.6 gating of `actors` and the effect verbs (spec-0012/0014:
/// `set-checkpoint`, `begin-stealth`/`end-stealth`, `play-sound`, `narrate art`,
/// and the actor staging verbs; gated on the quests stage). The per-effect
/// `requires_flags` gate is handled separately in [`reserved_v06_effect_flags`].
fn reserved_v06_world(c: &Campaign, d: &mut Vec<Diagnostic>) {
    use crate::stages::Horizon;

    // --- Stage 1: horizon / boundary (spec-0013), gated on the world stage ---
    if !is_v06(c.world.dsl_version.as_str()) {
        if c.world.content.horizon.is_some() {
            d.push(Diagnostic::error(
                codes::RESERVED,
                "world",
                "/content/horizon".to_string(),
                "world `horizon` requires dsl_version 0.6.0 — raise this stage's `dsl_version` to \
                 0.6.0, or remove the construct"
                    .to_string(),
            ));
        }
        if c.world.content.boundary.is_some() {
            d.push(Diagnostic::error(
                codes::RESERVED,
                "world",
                "/content/boundary".to_string(),
                "world `boundary` requires dsl_version 0.6.0 — raise this stage's `dsl_version` to \
                 0.6.0, or remove the construct"
                    .to_string(),
            ));
        }
        if c.world.content.min_players.is_some() {
            d.push(Diagnostic::error(
                codes::RESERVED,
                "world",
                "/content/min_players".to_string(),
                "world `min_players` requires dsl_version 0.6.0 — raise this stage's \
                 `dsl_version` to 0.6.0, or remove the construct"
                    .to_string(),
            ));
        }
        if c.world.content.difficulty.is_some() {
            d.push(Diagnostic::error(
                codes::RESERVED,
                "world",
                "/content/difficulty".to_string(),
                "world `difficulty` requires dsl_version 0.6.0 — raise this stage's `dsl_version` \
                 to 0.6.0, or remove the construct"
                    .to_string(),
            ));
        }
        for (i, a) in c.world.content.areas.iter().enumerate() {
            if a.mitigation.is_some() {
                d.push(Diagnostic::error(
                    codes::RESERVED,
                    "world",
                    format!("/content/areas/{i}/mitigation"),
                    "area `mitigation` requires dsl_version 0.6.0 — raise this stage's \
                     `dsl_version` to 0.6.0, or remove the construct"
                        .to_string(),
                ));
            }
        }
    } else {
        // spec-0018: a delve is played by ONE party of 1–4, so a declared
        // mandatory size outside that range can never be honoured.
        if let Some(n) = c.world.content.min_players
            && !(1..=4).contains(&n)
        {
            d.push(Diagnostic::error(
                codes::PARTY_SIZE,
                "world",
                "/content/min_players".to_string(),
                format!(
                    "`min_players` = {n} is out of range — a delve is played by one party of 1–4, \
                     so set it to a value in 1..=4 (absent = 1, a party of one)"
                ),
            ));
        }
        // Declared combat difficulty. `peaceful` is the
        // one keyword the compiler refuses: on peaceful the server discards every
        // hostile-category mob as it ticks it — summoned, `NoAI` and
        // `PersistenceRequired` are all irrelevant — so a peaceful delve is one in
        // which the entire cast of threats quietly does not exist.
        if matches!(
            c.world.content.difficulty,
            Some(crate::stages::WorldDifficulty::Peaceful)
        ) {
            d.push(Diagnostic::error(
                codes::DIFFICULTY_INVALID,
                "world",
                "/content/difficulty".to_string(),
                "`difficulty: \"peaceful\"` is refused: on peaceful the server discards every \
                 hostile-category mob as it ticks it — being `/summon`ed, `NoAI` or \
                 `PersistenceRequired` does not save one — so every wave, hostile actor and \
                 ambush in this campaign would silently cease to exist. Declare `easy`, `normal` \
                 or `hard`; for a delve that is genuinely combat-free, simply omit `difficulty` \
                 (a campaign with no waves already ships peaceful by derivation)"
                    .to_string(),
            ));
        }
        // `horizon: "ocean"` without a return rule strands wanderers in an infinite sea.
        if matches!(c.world.content.horizon, Some(Horizon::Ocean))
            && c.world.content.boundary.is_none()
        {
            d.push(Diagnostic::error(
                codes::OCEAN_NO_BOUNDARY,
                "world",
                "/content/horizon".to_string(),
                "`horizon: \"ocean\"` needs a `boundary` — an infinite swimmable sea with no \
                 return rule lets players wander off the map. Add a `boundary` (a bare `{}` uses \
                 the default margin), or set `horizon` to `void`"
                    .to_string(),
            ));
        }
        // `margin` range check (0..=64).
        if let Some(b) = &c.world.content.boundary
            && !(0..=64).contains(&b.margin)
        {
            d.push(Diagnostic::error(
                codes::BOUNDARY_MARGIN,
                "world",
                "/content/boundary/margin".to_string(),
                format!(
                    "`boundary.margin` = {} is out of range — set it to a value in 0..=64 (16 is \
                     the default)",
                    b.margin
                ),
            ));
        }
    }

    // --- Stage 3: kit-item `carrier` (spec-0018), gated on the classes stage ---
    if !is_v06(c.classes.dsl_version.as_str()) {
        for (i, class) in c.classes.content.classes.iter().enumerate() {
            for (k, item) in class.kit.iter().enumerate() {
                if item.carrier.is_some() {
                    d.push(Diagnostic::error(
                        codes::RESERVED,
                        "classes",
                        format!("/content/classes/{i}/kit/{k}/carrier"),
                        "kit item `carrier` requires dsl_version 0.6.0 — raise this stage's \
                         `dsl_version` to 0.6.0, or remove the construct"
                            .to_string(),
                    ));
                }
            }
        }
    }

    // --- Stage 5: scripted actors + quest / trigger effects (spec-0012 /
    //     spec-0014), gated on the quests stage. `check` covers every v0.6 effect
    //     verb (`set-checkpoint`, `begin-stealth`/`end-stealth`, `play-sound`,
    //     `spawn-actor`/`despawn-actor`/`move-actor`/`unleash-actor`, `sequence`
    //     — via `v06_effect`) and the `narrate` `art` style. ---
    if !is_v06(c.quests.dsl_version.as_str()) {
        if !c.quests.content.actors.is_empty() {
            d.push(Diagnostic::error(
                codes::RESERVED,
                "quests",
                "/content/actors".to_string(),
                "stage-5 `actors` require dsl_version 0.6.0 — raise this stage's `dsl_version` to \
                 0.6.0, or remove the actors"
                    .to_string(),
            ));
        }
        let res = |d: &mut Vec<Diagnostic>, path: String, what: &str| {
            d.push(Diagnostic::error(
                codes::RESERVED,
                "quests",
                path,
                format!(
                    "{what} requires dsl_version 0.6.0 — raise the quests stage's `dsl_version` \
                     to 0.6.0, or remove the construct"
                ),
            ));
        };
        let check = |d: &mut Vec<Diagnostic>, path_base: &str, eff: &QuestEffect| {
            if let Some(name) = eff.v06_effect() {
                res(d, format!("{path_base}/type"), &format!("effect `{name}`"));
            }
            if eff.narrate_art() {
                res(d, format!("{path_base}/style"), "narrate `style: art`");
            }
            // `cutscene.look_at` and the multi-shot `cutscene.shots` list are
            // additive v0.6 fields on the v0.4 `cutscene` verb — the verb itself
            // stays v0.4, only the new fields are gated.
            if eff.cutscene_look_at().is_some() {
                res(d, format!("{path_base}/look_at"), "cutscene `look_at`");
            }
            if eff.cutscene_multi_shot() {
                res(d, format!("{path_base}/shots"), "cutscene `shots`");
            }
            // `move-npc.on_arrive` is an additive v0.6 field on the v0.4
            // `move-npc` verb, exactly like `cutscene.look_at` above.
            if eff.move_npc_on_arrive().is_some() {
                res(d, format!("{path_base}/on_arrive"), "move-npc `on_arrive`");
            }
            // `give-item.carrier` (spec-0018) is likewise an additive v0.6 field
            // on the v0.3 `give-item` verb.
            if eff.give_carrier().is_some() {
                res(d, format!("{path_base}/carrier"), "give-item `carrier`");
            }
        };
        for (i, q) in c.quests.content.quests.iter().enumerate() {
            for (oid, effs) in &q.on_objective_complete {
                for (m, eff) in effs.iter().enumerate() {
                    check(
                        d,
                        &format!(
                            "/content/quests/{i}/on_objective_complete/{}/{m}",
                            oid.as_str()
                        ),
                        eff,
                    );
                }
            }
            for (m, eff) in q.on_complete.iter().enumerate() {
                check(d, &format!("/content/quests/{i}/on_complete/{m}"), eff);
            }
        }
        for (i, t) in c.quests.content.triggers.iter().enumerate() {
            for (m, eff) in t.effects.iter().enumerate() {
                check(d, &format!("/content/triggers/{i}/effects/{m}"), eff);
            }
        }
        // Traps (spec-0011) are a v0.6 stage-5 surface: reserved before 0.6.0.
        if !c.quests.content.traps.is_empty() {
            res(d, "/content/traps".to_string(), "the `traps` section");
        }
        // Shortcut doors (spec-0016 §2) are a v0.6 stage-5 surface too.
        if !c.quests.content.shortcuts.is_empty() {
            res(
                d,
                "/content/shortcuts".to_string(),
                "the `shortcuts` section",
            );
        }
        // Ambushes (spec-0016 §3) likewise.
        if !c.quests.content.ambushes.is_empty() {
            res(d, "/content/ambushes".to_string(), "the `ambushes` section");
        }
        // Timed gates (spec-0016 §4) likewise.
        if !c.quests.content.timed_gates.is_empty() {
            res(
                d,
                "/content/timed_gates".to_string(),
                "the `timed_gates` section",
            );
        }
        // Container fills (spec-0021) are a v0.6 stage-5 surface.
        if !c.quests.content.loot.is_empty() {
            res(d, "/content/loot".to_string(), "the `loot` section");
        }
        // Actor `equipment` (spec-0021) likewise, and actor `attributes` — the
        // same v0.4 shape a wave mob's takes, fenced on
        // the stage the actors themselves are fenced on.
        for (i, a) in c.quests.content.actors.iter().enumerate() {
            if a.equipment.is_some() {
                res(
                    d,
                    format!("/content/actors/{i}/equipment"),
                    "actor `equipment`",
                );
            }
            if a.attributes.is_some() {
                res(
                    d,
                    format!("/content/actors/{i}/attributes"),
                    "actor `attributes`",
                );
            }
        }
        // Wave-mob `equipment` is a v0.6 stage-5 surface: reserved
        // before 0.6.0 (the field defaults to absent, so an earlier campaign
        // that uses none is byte-identical).
        for (i, w) in c.quests.content.waves.iter().enumerate() {
            for (k, m) in w.mobs.iter().enumerate() {
                if m.equipment.is_some() {
                    res(
                        d,
                        format!("/content/waves/{i}/mobs/{k}/equipment"),
                        "wave-mob `equipment`",
                    );
                }
            }
            // Bonfire re-seating (spec-0016 §1) is a v0.6 stage-5 surface.
            if w.respawns_on_rest {
                res(
                    d,
                    format!("/content/waves/{i}/respawns_on_rest"),
                    "wave `respawns_on_rest`",
                );
            }
            // TD lanes + aggro-edge summoning (spec-0016 §6) are v0.6 too.
            if w.lane.is_some() {
                res(d, format!("/content/waves/{i}/lane"), "wave `lane`");
            }
            if w.summon.is_some() {
                res(d, format!("/content/waves/{i}/summon"), "wave `summon`");
            }
        }
    }

    // --- Stage 2: `deferred` NPC entrance, gated on the npcs stage. ---
    if !is_v06(c.npcs.dsl_version.as_str()) {
        for (i, n) in c.npcs.content.npcs.iter().enumerate() {
            if n.deferred {
                d.push(Diagnostic::error(
                    codes::RESERVED,
                    "npcs",
                    format!("/content/npcs/{i}/deferred"),
                    "npc `deferred` requires dsl_version 0.6.0 — raise this stage's \
                     `dsl_version` to 0.6.0, or remove the field"
                        .to_string(),
                ));
            }
        }
    }

    // --- Stage 6: dialogue effects (spec-0012 `set-checkpoint`; play-sound / art
    //     are quest/trigger-only), gated on the dialogue stage. ---
    if !is_v06(c.dialogue.dsl_version.as_str()) {
        for (i, t) in c.dialogue.content.dialogues.iter().enumerate() {
            for (j, node) in t.nodes.iter().enumerate() {
                for (k, opt) in node.options.iter().enumerate() {
                    for (m, eff) in opt.effects.iter().enumerate() {
                        if let Some(name) = eff.v06_effect() {
                            d.push(Diagnostic::error(
                                codes::RESERVED,
                                "dialogue",
                                format!(
                                    "/content/dialogues/{i}/nodes/{j}/options/{k}/effects/{m}/type"
                                ),
                                format!(
                                    "dialogue effect `{name}` requires dsl_version 0.6.0 — raise \
                                     this stage's `dsl_version` to 0.6.0, or remove the construct"
                                ),
                            ));
                        }
                    }
                }
            }
        }
    }
}

/// DSL v0.5 reserved-feature gating (spec-0010): declared world `time`/`weather`
/// and per-area `lighting` (stage 1), plus the `set-time`/`set-weather` effect
/// verbs (stage 5 quests, stage 6 dialogue), are rejected with `DW0141` in a
/// pre-0.5.0 campaign, gated on the stage that carries them. Additive fields
/// default to empty, so a v0.4 campaign that uses none is byte-identical. When
/// `lighting` is present under a v0.5 campaign, `min_light` is range-checked
/// (1..=14, `DW0196`).
fn reserved_v05(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let v05_world = is_v05(c.world.dsl_version.as_str());
    let v05_quests = is_v05(c.quests.dsl_version.as_str());
    let v05_dialogue = is_v05(c.dialogue.dsl_version.as_str());
    let res = |d: &mut Vec<Diagnostic>, stage, path: String, what: &str| {
        d.push(Diagnostic::error(
            codes::RESERVED,
            stage,
            path,
            format!(
                "{what} requires dsl_version 0.5.0 — raise this stage's `dsl_version` to 0.5.0, \
                 or remove the construct"
            ),
        ));
    };

    // Stage 1 — declared time / weather / per-area lighting.
    if !v05_world {
        if c.world.content.time.is_some() {
            res(d, "world", "/content/time".to_string(), "world `time`");
        }
        if c.world.content.weather.is_some() {
            res(
                d,
                "world",
                "/content/weather".to_string(),
                "world `weather`",
            );
        }
        for (i, area) in c.world.content.areas.iter().enumerate() {
            if area.lighting.is_some() {
                res(
                    d,
                    "world",
                    format!("/content/areas/{i}/lighting"),
                    "area `lighting`",
                );
            }
        }
    } else {
        // Range-check min_light (1..=14) where a lighting block is declared.
        for (i, area) in c.world.content.areas.iter().enumerate() {
            if let Some(lighting) = &area.lighting
                && !(1..=14).contains(&lighting.min_light)
            {
                d.push(Diagnostic::error(
                    codes::LIGHTING_RANGE,
                    "world",
                    format!("/content/areas/{i}/lighting/min_light"),
                    format!(
                        "area `{}` `lighting.min_light` = {} is out of range — set it to a value \
                         in 1..=14 (7 is the default)",
                        area.id, lighting.min_light
                    ),
                ));
            }
        }
    }

    // Stage 5 — set-time / set-weather quest effects.
    if !v05_quests {
        for (i, q) in c.quests.content.quests.iter().enumerate() {
            for_each_effect(q, |path, eff| {
                if let Some(name) = eff.v05_effect() {
                    res(
                        d,
                        "quests",
                        format!("/content/quests/{i}/{path}/type"),
                        &format!("effect `{name}`"),
                    );
                }
            });
        }
    }

    // Stage 6 — set-time / set-weather dialogue effects.
    if !v05_dialogue {
        for (i, t) in c.dialogue.content.dialogues.iter().enumerate() {
            for (j, node) in t.nodes.iter().enumerate() {
                for (k, opt) in node.options.iter().enumerate() {
                    for (m, eff) in opt.effects.iter().enumerate() {
                        if let Some(name) = eff.v05_effect() {
                            res(
                                d,
                                "dialogue",
                                format!(
                                    "/content/dialogues/{i}/nodes/{j}/options/{k}/effects/{m}/type"
                                ),
                                &format!("dialogue effect `{name}`"),
                            );
                        }
                    }
                }
            }
        }
    }
}

/// DSL v0.4 reserved-feature gating: every v0.4 construct is rejected with
/// `DW0141` in a pre-0.4.0 campaign, gated on the stage that carries it (mirroring
/// the v0.3 verb gate above). Additive fields default to empty, so a v0.3
/// campaign that uses none is byte-identical.
fn reserved_v04(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let v04_npcs = is_v04(c.npcs.dsl_version.as_str());
    let v04_quests = is_v04(c.quests.dsl_version.as_str());
    let v04_dialogue = is_v04(c.dialogue.dsl_version.as_str());
    let res = |d: &mut Vec<Diagnostic>, stage, path: String, what: &str| {
        d.push(Diagnostic::error(
            codes::RESERVED,
            stage,
            path,
            format!(
                "{what} requires dsl_version 0.4.0 — raise this stage's `dsl_version` to at least \
                 0.4.0, or remove the construct"
            ),
        ));
    };

    // Stage 2 — mannequin skins.
    if !v04_npcs {
        for (i, npc) in c.npcs.content.npcs.iter().enumerate() {
            if npc.skin.is_some() {
                res(d, "npcs", format!("/content/npcs/{i}/skin"), "npc `skin`");
            }
        }
    }

    // Stage 5 — objective stealth/prop, wave attributes/effects, v0.4 effects,
    // named give-item, environment triggers.
    if !v04_quests {
        for (i, q) in c.quests.content.quests.iter().enumerate() {
            for (j, o) in q.objectives.iter().enumerate() {
                if o.stealth() {
                    res(
                        d,
                        "quests",
                        format!("/content/quests/{i}/objectives/{j}/stealth"),
                        "objective `stealth`",
                    );
                }
                if o.prop().is_some() {
                    res(
                        d,
                        "quests",
                        format!("/content/quests/{i}/objectives/{j}/prop"),
                        "objective `prop`",
                    );
                }
            }
            for_each_effect(q, |path, eff| {
                if let Some(name) = eff.v04_effect() {
                    res(
                        d,
                        "quests",
                        format!("/content/quests/{i}/{path}/type"),
                        &format!("effect `{name}`"),
                    );
                }
                if eff.give_item_named() {
                    res(
                        d,
                        "quests",
                        format!("/content/quests/{i}/{path}/name"),
                        "give-item `name`",
                    );
                }
            });
        }
        for (i, w) in c.quests.content.waves.iter().enumerate() {
            for (k, m) in w.mobs.iter().enumerate() {
                if m.attributes.is_some() {
                    res(
                        d,
                        "quests",
                        format!("/content/waves/{i}/mobs/{k}/attributes"),
                        "wave-mob `attributes`",
                    );
                }
                if !m.effects.is_empty() {
                    res(
                        d,
                        "quests",
                        format!("/content/waves/{i}/mobs/{k}/effects"),
                        "wave-mob `effects`",
                    );
                }
            }
        }
        if !c.quests.content.triggers.is_empty() {
            res(
                d,
                "quests",
                "/content/triggers".to_string(),
                "environment `triggers`",
            );
        }
    }

    // Stage 6 — dialogue requires_flags + set-flag effect.
    if !v04_dialogue {
        for (i, t) in c.dialogue.content.dialogues.iter().enumerate() {
            for (j, node) in t.nodes.iter().enumerate() {
                for (k, opt) in node.options.iter().enumerate() {
                    if !opt.requires_flags.is_empty() {
                        res(
                            d,
                            "dialogue",
                            format!("/content/dialogues/{i}/nodes/{j}/options/{k}/requires_flags"),
                            "dialogue option `requires_flags`",
                        );
                    }
                    for (m, eff) in opt.effects.iter().enumerate() {
                        if let Some(name) = eff.v04_effect() {
                            res(
                                d,
                                "dialogue",
                                format!(
                                    "/content/dialogues/{i}/nodes/{j}/options/{k}/effects/{m}/type"
                                ),
                                &format!("dialogue effect `{name}`"),
                            );
                        }
                    }
                }
            }
        }
    }
}

/// Visit every quest effect with a relative path fragment.
fn for_each_effect(q: &crate::stages::Quest, mut f: impl FnMut(String, &QuestEffect)) {
    for (key, effs) in &q.on_objective_complete {
        for (m, eff) in effs.iter().enumerate() {
            f(format!("on_objective_complete/{key}/{m}"), eff);
        }
    }
    for (m, eff) in q.on_complete.iter().enumerate() {
        f(format!("on_complete/{m}"), eff);
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
fn for_each_effect_deep(q: &crate::stages::Quest, mut f: impl FnMut(String, &QuestEffect)) {
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
fn for_each_trigger_effect_deep(
    t: &crate::stages::EnvTrigger,
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
fn for_each_trap_payload_deep(t: &crate::stages::Trap, mut f: impl FnMut(String, &QuestEffect)) {
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
// DSL v0.6 — scripted actors + staging effects (spec-0014)
// ---------------------------------------------------------------------------

/// Recursively visit every effect in `effs`, descending into every nested effect
/// list ([`QuestEffect::nested_effect_lists`]: `sequence` steps, `set-checkpoint`
/// `on_respawn`, `begin-stealth` `on_caught`, `move-actor` / `move-npc`
/// `on_arrive`).
fn walk_effects_deep(effs: &[QuestEffect], f: &mut dyn FnMut(&QuestEffect)) {
    for e in effs {
        e.visit_deep(f);
    }
}

/// True if `e` is (or transitively reaches, via a `move-actor` / `move-npc`
/// `on_arrive`) a `sequence` — the recursion `DW0329` forbids inside another
/// sequence's steps.
fn reaches_sequence(e: &QuestEffect) -> bool {
    match e {
        QuestEffect::Sequence { .. } => true,
        QuestEffect::MoveActor { on_arrive, .. } | QuestEffect::MoveNpc { on_arrive, .. } => {
            on_arrive.iter().any(reaches_sequence)
        }
        _ => false,
    }
}

/// Reject a `sequence` nested inside another `sequence` (`DW0329`). Recurses into a
/// `move-actor` / `move-npc` `on_arrive` (a sequence there is legal — not yet
/// inside a sequence) but not into an already-flagged sequence's steps (avoids
/// double-reporting).
fn check_no_nested_sequence(effs: &[QuestEffect], path: &str, d: &mut Vec<Diagnostic>) {
    for e in effs {
        match e {
            QuestEffect::Sequence { steps } => {
                for s in steps {
                    for inner in &s.effects {
                        if reaches_sequence(inner) {
                            d.push(Diagnostic::error(
                                codes::NESTED_SEQUENCE,
                                "quests",
                                path.to_string(),
                                "a `sequence` effect is nested inside another `sequence` — \
                                 timelines do not recurse (spec-0014). Flatten the inner steps \
                                 into the outer timeline (shift their `at_ticks` by the inner \
                                 sequence's start), or drive the second beat from a flag/objective"
                                    .to_string(),
                            ));
                        }
                    }
                }
            }
            QuestEffect::MoveActor { on_arrive, .. } | QuestEffect::MoveNpc { on_arrive, .. } => {
                check_no_nested_sequence(on_arrive, path, d);
            }
            _ => {}
        }
    }
}

/// Reject a `carrier: "one"` `give-item` inside a **scheduler-only** bundle
/// (`DW0371`, spec-0018).
///
/// `carrier: "one"` means "hand this one quest prop to the player whose action
/// earned it". A `sequence` step and a `move-npc`/`move-actor` `on_arrive` are
/// re-invoked by the vanilla scheduler with the **server** command source: there
/// is no acting player there, so the effect has no defensible recipient. The
/// party-wide default (absent `carrier`) is always fine — it addresses `@a`.
///
/// `scheduled` latches on the way down and the walk deliberately **stops** at a
/// `set-checkpoint`'s `on_respawn` and a `begin-stealth`'s `on_caught`: those
/// bundles are dispatched per player (the respawning / spotted one), so they do
/// have an `@s` even when the effect that installed them was scheduled.
fn check_carrier_one_not_scheduled(
    effs: &[QuestEffect],
    path: &str,
    scheduled: bool,
    d: &mut Vec<Diagnostic>,
) {
    for e in effs {
        if scheduled && e.gives_to_one() {
            d.push(Diagnostic::error(
                codes::PARTY_CARRIER_SCHEDULED,
                "quests",
                path.to_string(),
                "a `give-item` with `carrier: \"one\"` sits in a bundle only the scheduler ever \
                 runs (a `sequence` step, or a `move-npc`/`move-actor` `on_arrive`). Those run \
                 with the server command source — there is no acting player to hand the prop to, \
                 so the give would silently reach nobody. Drop `carrier` to arm the whole party, \
                 or move the hand-off onto the beat a player completes"
                    .to_string(),
            ));
        }
        match e {
            // These bundles ARE dispatched per player; they reset the latch.
            QuestEffect::SetCheckpoint { on_respawn, .. } => {
                check_carrier_one_not_scheduled(on_respawn, path, false, d);
            }
            QuestEffect::BeginStealth { on_caught, .. } => {
                check_carrier_one_not_scheduled(on_caught, path, false, d);
            }
            // These are the scheduler-only seams (see `emit::Audience::Scheduled`).
            QuestEffect::Sequence { steps } => {
                for st in steps {
                    check_carrier_one_not_scheduled(&st.effects, path, true, d);
                }
            }
            QuestEffect::MoveActor { on_arrive, .. } | QuestEffect::MoveNpc { on_arrive, .. } => {
                check_carrier_one_not_scheduled(on_arrive, path, true, d);
            }
            _ => {}
        }
    }
}

/// DSL v0.6 checks (spec-0014): actor entity ids, skins, anchor resolution, actor
/// references from staging effects, the no-nested-`sequence` rule, and wave-mob
/// `equipment` item ids (`DW0143` — the give-item family). Gated on the
/// quests stage version by the caller.
fn v06_checks(
    c: &Campaign,
    items: &dyn ItemRegistry,
    anchors: &dyn AnchorRegistry,
    entities: &dyn EntityRegistry,
    d: &mut Vec<Diagnostic>,
) {
    let quests = &c.quests.content;
    let declared: BTreeSet<&str> = quests.actors.iter().map(|a| a.id.as_str()).collect();

    // Shot-style semantics (spec-0015): DW0348/DW0349 + subject refs.
    let npc_ids: BTreeSet<&str> = c.npcs.content.npcs.iter().map(|n| n.id.as_str()).collect();
    cutscene_style_checks(quests, &npc_ids, d);

    // Anchor names provided by single-prefab areas (pool areas resolve anchors in
    // the compiler, so their presence defers the check — mirroring `DW0142`'s
    // single-prefab-only scope; never a false positive).
    let providers = AnchorProviders::build(c, anchors);

    // Actor declarations: entity id, skin, spawn anchor.
    let mut seen_skins: BTreeSet<&str> = BTreeSet::new();
    for (i, a) in quests.actors.iter().enumerate() {
        if !entities.contains(&a.entity) {
            d.push(Diagnostic::error(
                codes::ENTITY_UNKNOWN,
                "quests",
                format!("/content/actors/{i}/entity"),
                format!(
                    "actor entity `{}` is not a known 1.21.11 entity id — use a valid namespaced \
                     entity id (e.g. `minecraft:warden`)",
                    a.entity
                ),
            ));
        }
        if let Some(skin) = &a.skin {
            if !is_kebab(&skin.texture_id) {
                d.push(Diagnostic::error(
                    codes::SKIN_INVALID,
                    "quests",
                    format!("/content/actors/{i}/skin/texture_id"),
                    format!(
                        "actor skin `texture_id` `{}` is malformed — it must be a bare kebab token \
                         (e.g. `giant-idle`), matching the `skins/<texture_id>.png` filename",
                        skin.texture_id
                    ),
                ));
            } else if !seen_skins.insert(skin.texture_id.as_str()) {
                d.push(Diagnostic::error(
                    codes::SKIN_INVALID,
                    "quests",
                    format!("/content/actors/{i}/skin/texture_id"),
                    format!(
                        "duplicate actor skin `texture_id` `{}` — each mannequin needs a distinct \
                         texture; rename one (and its `skins/<id>.png`)",
                        skin.texture_id
                    ),
                ));
            }
        }
        if !providers.resolvable(a.anchor.as_str()) {
            d.push(Diagnostic::error(
                codes::ANCHOR_UNRESOLVED,
                "quests",
                format!("/content/actors/{i}/anchor"),
                format!(
                    "actor anchor `{}` is not provided by any area's prefab — use an anchor a \
                     prefab exposes, or bind a prefab/pool that carries it",
                    a.anchor
                ),
            ));
        }
    }

    // Effect-level: actor references (DW0112), move-actor destination anchors
    // (DW0142), and the no-nested-sequence rule (DW0329). Deep-walk so effects
    // nested in a `sequence` / `move-actor` `on_arrive` are covered.
    let mut groups: Vec<(String, &[QuestEffect])> = Vec::new();
    for (i, q) in quests.quests.iter().enumerate() {
        for (key, effs) in &q.on_objective_complete {
            groups.push((
                format!("/content/quests/{i}/on_objective_complete/{key}"),
                effs.as_slice(),
            ));
        }
        groups.push((
            format!("/content/quests/{i}/on_complete"),
            q.on_complete.as_slice(),
        ));
    }
    for (i, t) in quests.triggers.iter().enumerate() {
        groups.push((
            format!("/content/triggers/{i}/effects"),
            t.effects.as_slice(),
        ));
    }
    for (path, effs) in &groups {
        let mut visit = |e: &QuestEffect| {
            if let Some(actor) = e.actor_ref()
                && !declared.contains(actor.as_str())
            {
                d.push(Diagnostic::error(
                    codes::DANGLING_REF,
                    "quests",
                    path.clone(),
                    format!(
                        "actor staging effect references unknown actor `{actor}` — declare it in \
                         the stage-5 `actors` list, or fix the reference"
                    ),
                ));
            }
            if let QuestEffect::MoveActor { to_anchor, .. } = e
                && !providers.resolvable(to_anchor.as_str())
            {
                d.push(Diagnostic::error(
                    codes::ANCHOR_UNRESOLVED,
                    "quests",
                    path.clone(),
                    format!(
                        "move-actor destination anchor `{to_anchor}` is not provided by any area's \
                         prefab — use an anchor a prefab exposes"
                    ),
                ));
            }
        };
        walk_effects_deep(effs, &mut visit);
        check_no_nested_sequence(effs, path, d);
        check_carrier_one_not_scheduled(effs, path, false, d);
    }

    // Wave-mob `equipment` item ids: every present slot must name a
    // pinned-1.21.11 item — the same registry and DW family as `give-item`
    // (`DW0143`).
    for (i, w) in quests.waves.iter().enumerate() {
        for (k, m) in w.mobs.iter().enumerate() {
            let Some(eq) = &m.equipment else { continue };
            check_equipment(
                eq,
                "wave-mob",
                &format!("/content/waves/{i}/mobs/{k}/equipment"),
                items,
                d,
            );
        }
    }

    // Actor `equipment` (spec-0021): the same shape, the same registries, the
    // same diagnostics as a wave mob's — one surface, one rule set.
    for (i, a) in quests.actors.iter().enumerate() {
        let Some(eq) = &a.equipment else { continue };
        check_equipment(
            eq,
            "actor",
            &format!("/content/actors/{i}/equipment"),
            items,
            d,
        );
    }

    // Declared drops — the subset an elite/boss leaves behind.
    check_drops(c, quests, items, d);

    // spec-0016 §1: `respawns_on_rest` is re-seating *by a bonfire*. With no
    // `bonfire` anywhere in the campaign nothing can ever fire the re-seat, so
    // the field is a silent no-op — the class of defect this compiler always
    // turns loud (`DW0370`).
    let mut has_bonfire = false;
    for q in &c.quests.content.quests {
        for_each_effect_deep(q, |_path, eff| {
            has_bonfire |= eff.bonfire().is_some();
        });
    }
    for t in &c.quests.content.triggers {
        for_each_trigger_effect_deep(t, |_path, eff| {
            has_bonfire |= eff.bonfire().is_some();
        });
    }
    if !has_bonfire {
        for (i, w) in quests.waves.iter().enumerate() {
            if w.respawns_on_rest {
                d.push(Diagnostic::error(
                    codes::REST_RESEAT_NO_BONFIRE,
                    "quests",
                    format!("/content/waves/{i}/respawns_on_rest"),
                    format!(
                        "wave `{}` declares `respawns_on_rest: true` but this campaign declares \
                         no `bonfire` — nothing can ever re-seat it, so the field is inert. Add \
                         the `bonfire` the re-seat hangs off (spec-0016 §1), or drop the field; \
                         do NOT leave a silently dead declaration in the DSL.",
                        w.id.as_str()
                    ),
                ));
            }
        }
    }

    // spec-0016 §1 + spec-0023, souls ruling 5/7 ("stage bosses never respawn
    // on rest"): `tier` and
    // `respawns_on_rest` are two fields on the SAME wave declaration — the only
    // place a "boss" billing and a "re-seat on rest" contract can land on one
    // another (an actor carries `tier` too, but has no `respawns_on_rest` field
    // at all, so it cannot express this violation). A rest-respawning boss
    // re-fight breaks the retry economy the ruling protects. Checked
    // unconditionally of `has_bonfire`: the combination is forbidden on its own
    // terms, not merely inert like `DW0370`.
    for (i, w) in quests.waves.iter().enumerate() {
        if w.respawns_on_rest && w.tier == Some(EncounterTier::Boss) {
            d.push(Diagnostic::error(
                codes::BOSS_RESPAWNS_ON_REST,
                "quests",
                format!("/content/waves/{i}/respawns_on_rest"),
                format!(
                    "wave `{}` declares `tier: boss` AND `respawns_on_rest: true` — souls \
                     ruling 5/7 is that stage bosses never respawn on rest, since a \
                     rest-respawning boss re-fight breaks the retry economy the ruling \
                     protects. Drop `respawns_on_rest` if this really is the boss, or drop \
                     `tier: boss` (bill it `elite` instead) if the encounter is meant to \
                     re-seat.",
                    w.id.as_str()
                ),
            ));
        }
    }

    // spec-0016 §1: a campaign that places a bonfire is
    // a souls campaign, and a souls campaign owes the party a flask. Resting
    // replenishes every `flask` kit entry to its declared count — with none
    // declared, "rest and save" and "save only" collapse into the same button and
    // the recovery economy the bonfire exists to serve does not exist (`DW0476`).
    // Campaign-global on purpose: the flask is per-class gear, and one class
    // without a flask is as broken as none, so the requirement is on EVERY class.
    if has_bonfire {
        let flaskless: Vec<&str> = c
            .classes
            .content
            .classes
            .iter()
            .filter(|cl| !cl.kit.iter().any(|k| k.flask))
            .map(|cl| cl.id.as_str())
            .collect();
        if !flaskless.is_empty() {
            d.push(Diagnostic::error(
                codes::BONFIRE_NO_FLASK,
                "classes",
                "/content/classes".to_string(),
                format!(
                    "this campaign places a `bonfire` but {} no `flask` kit item: {}. \
                     Resting at a bonfire replenishes every kit entry marked `\"flask\": true` to \
                     its declared `count` — with none, the rest option recovers nothing and the \
                     souls loop has no consumable to spend (spec-0016 §1). \
                     Add a recovery item to each class kit and mark it \
                     `\"flask\": true` (this needs `dsl_version` 0.8.0 on the classes stage). Do \
                     NOT drop the bonfire to silence this — the rest point is the design.",
                    if flaskless.len() == 1 {
                        "one class declares".to_string()
                    } else {
                        format!("{} classes declare", flaskless.len())
                    },
                    flaskless.join(", ")
                ),
            ));
        }
    }
}

/// spec-0016 §1: **what is actually in the flask.**
///
/// The kit `flask` marker landed with no way to say what the bottle pours, so
/// every flask shipped as `minecraft:potion` with no `minecraft:potion_contents`
/// component — the Uncraftable Potion, which a player can drink all day for
/// nothing. `contents` closes that, and these are the two halves of keeping it
/// honest: `DW0487` refuses the placeholder (a potion-bearing kit item that
/// declares no contents), `DW0486` refuses contents 1.21.11 cannot pour.
///
/// Runs only at `dsl_version` 0.8.0 on the classes stage — the same asymmetry the
/// version ledger uses everywhere: *declaring* `contents` earlier is `DW0141`,
/// and the *requirement* fires only at 0.8.0, so no pre-0.8 campaign's datapack
/// can move by a byte.
fn kit_potion_checks(c: &Campaign, effects: &dyn EffectRegistry, d: &mut Vec<Diagnostic>) {
    for (i, cl) in c.classes.content.classes.iter().enumerate() {
        for (k, item) in cl.kit.iter().enumerate() {
            let bearing = crate::stages::is_potion_bearing_item(&item.item);
            let path = format!("/content/classes/{i}/kit/{k}");
            let Some(contents) = &item.contents else {
                // The placeholder flask, as a build error.
                if bearing {
                    d.push(Diagnostic::error(
                        codes::KIT_POTION_MISSING,
                        "classes",
                        format!("{path}/contents"),
                        format!(
                            "kit item `{}` declares no `contents`, so it compiles to the \
                             *Uncraftable Potion* — a bottle with no `minecraft:potion_contents` \
                             component, which grants nothing when drunk however it is named. \
                             Declare what is in it: `\"contents\": {{\"potion\": \
                             \"minecraft:strong_healing\"}}`, or an `\"effects\"` list of \
                             `{{\"effect\", \"duration\", \"amplifier\"}}`. Do NOT rename the \
                             bottle instead — semantics never key on player-facing text \
                             (spec-0016 §1).",
                            item.item
                        ),
                    ));
                }
                continue;
            };
            // `contents` on an item with no such component: the data would be
            // dropped on the floor, silently.
            if !bearing {
                d.push(Diagnostic::error(
                    codes::KIT_POTION_INVALID,
                    "classes",
                    format!("{path}/contents"),
                    format!(
                        "kit item `{}` cannot carry potion `contents` — in 1.21.11 only \
                         `minecraft:potion`, `minecraft:splash_potion`, \
                         `minecraft:lingering_potion` and `minecraft:tipped_arrow` carry a \
                         `minecraft:potion_contents` component, and on anything else the game \
                         discards it. Put the contents on a potion item, or drop the field.",
                        item.item
                    ),
                ));
                continue;
            }
            if contents.potion.is_none() && contents.effects.is_empty() {
                d.push(Diagnostic::error(
                    codes::KIT_POTION_INVALID,
                    "classes",
                    format!("{path}/contents"),
                    "empty potion `contents` — it names no `potion` and lists no `effects`, so \
                     the bottle still pours nothing. Name a vanilla potion (e.g. \
                     `\"potion\": \"minecraft:strong_healing\"`) or list at least one effect."
                        .to_string(),
                ));
            }
            if let Some(p) = &contents.potion
                && !crate::registry::is_potion_id(p)
            {
                d.push(Diagnostic::error(
                    codes::KIT_POTION_INVALID,
                    "classes",
                    format!("{path}/contents/potion"),
                    format!(
                        "`{p}` is not in the pinned 1.21.11 `potion` registry — use a real potion \
                         id (`minecraft:healing`, `minecraft:strong_healing`, \
                         `minecraft:long_night_vision`, …). Note the 1.20.5+ spelling: strength \
                         and duration are part of the id (`strong_`/`long_` prefixes), not \
                         separate fields."
                    ),
                ));
            }
            if let Some(col) = &contents.color
                && !is_hex_color(col)
            {
                d.push(Diagnostic::error(
                    codes::KIT_POTION_INVALID,
                    "classes",
                    format!("{path}/contents/color"),
                    format!(
                        "potion `color` `{col}` is malformed — write the bottle colour as \
                         `#rrggbb` (e.g. `#ff9c30`), or omit it and take the colour vanilla \
                         derives from the effects."
                    ),
                ));
            }
            for (e, eff) in contents.effects.iter().enumerate() {
                let epath = format!("{path}/contents/effects/{e}");
                if !effects.contains(&eff.effect) {
                    d.push(Diagnostic::error(
                        codes::KIT_POTION_INVALID,
                        "classes",
                        format!("{epath}/effect"),
                        format!(
                            "potion effect `{}` is not a known 1.21.11 status-effect id — use a \
                             valid namespaced effect id (e.g. `minecraft:instant_health`).",
                            eff.effect
                        ),
                    ));
                }
                if let Some(amp) = eff.amplifier
                    && amp > crate::stages::MAX_POTION_AMPLIFIER
                {
                    d.push(Diagnostic::error(
                        codes::KIT_POTION_INVALID,
                        "classes",
                        format!("{epath}/amplifier"),
                        format!(
                            "potion effect `amplifier` {amp} is out of range — vanilla stores it \
                             in an unsigned byte, so it must be 0–{max} (0 = level I).",
                            max = crate::stages::MAX_POTION_AMPLIFIER
                        ),
                    ));
                }
                match (eff.is_instant(), eff.duration) {
                    // An instantaneous effect is applied once on drinking; a
                    // duration on it is a sentence the game never reads.
                    (true, Some(dur)) => d.push(Diagnostic::error(
                        codes::KIT_POTION_INVALID,
                        "classes",
                        format!("{epath}/duration"),
                        format!(
                            "`{}` is instantaneous — it lands once, on the tick the potion is \
                             drunk, so the `duration` of {dur} tick(s) here is never read. Drop \
                             the field; for healing that ticks over time use \
                             `minecraft:regeneration`, which does take a duration.",
                            eff.effect
                        ),
                    )),
                    (false, None) => d.push(Diagnostic::error(
                        codes::KIT_POTION_INVALID,
                        "classes",
                        format!("{epath}/duration"),
                        format!(
                            "potion effect `{}` lasts over time and declares no `duration` — \
                             vanilla would default it to zero ticks, i.e. nothing. Declare the \
                             duration in ticks (20 = one second).",
                            eff.effect
                        ),
                    )),
                    (false, Some(dur))
                        if dur == 0 || dur > crate::stages::MAX_POTION_DURATION_TICKS =>
                    {
                        d.push(Diagnostic::error(
                            codes::KIT_POTION_INVALID,
                            "classes",
                            format!("{epath}/duration"),
                            format!(
                                "potion effect `duration` {dur} is out of range — it is in \
                                 **ticks** (20 = one second) and must be 1–{max} \
                                 (≈13.9 hours, past the delve ceiling).",
                                max = crate::stages::MAX_POTION_DURATION_TICKS
                            ),
                        ));
                    }
                    _ => {}
                }
            }
        }
    }
}

/// True if `s` is a `#rrggbb` colour literal.
fn is_hex_color(s: &str) -> bool {
    let Some(hex) = s.strip_prefix('#') else {
        return false;
    };
    hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit())
}

// ---------------------------------------------------------------------------
// Rule group 5 — anchors and items
// ---------------------------------------------------------------------------

fn anchors_and_items(
    c: &Campaign,
    items: &dyn ItemRegistry,
    anchors: &dyn AnchorRegistry,
    d: &mut Vec<Diagnostic>,
) {
    // area id -> anchor set of its bound prefab (only for single-prefab areas
    // whose prefab is known; pool areas resolve their anchors elsewhere).
    //
    // A site-plan campaign has no prefab to ask, and it does not need one: its
    // anchors are DERIVED from the graph and the plan, so the set is knowable
    // here exactly as a prefab's is — and it is the same function the derivation
    // itself places them by, so a name that resolves here cannot fail to exist
    // in the built world.
    let providers = AnchorProviders::build(c, anchors);
    // Every anchor any known area provides, and whether that union is the whole
    // truth (no area binds a pool / an unknown prefab). Used for the anchor
    // references that have no owning area to resolve against: an environment
    // trigger's effects (triggers are global) and a cutscene's camera anchors (a
    // camera legitimately flies across areas).

    // planned quest id -> its area.
    let quest_area: BTreeMap<&str, &str> = c
        .quest_plan
        .content
        .quests
        .iter()
        .map(|q| (q.id.as_str(), q.area.as_str()))
        .collect();

    // NPC anchors.
    for (i, npc) in c.npcs.content.npcs.iter().enumerate() {
        if let Some(set) = providers.for_area(npc.area.as_str())
            && !set.contains(npc.anchor.as_str())
        {
            d.push(Diagnostic::error(
                codes::ANCHOR_UNRESOLVED,
                "npcs",
                format!("/content/npcs/{i}/anchor"),
                format!(
                    "npc anchor `{}` is not provided by the prefab bound to area `{}` — use an \
                     anchor the prefab exposes, or bind a prefab/pool that carries `{}`. Anchor \
                     names come from prefab metadata; do NOT invent one",
                    npc.anchor, npc.area, npc.anchor
                ),
            ));
        }
    }

    // Objective / effect anchors, resolved against the quest's planned area.
    for (i, q) in c.quests.content.quests.iter().enumerate() {
        let set = quest_area
            .get(q.id.as_str())
            .and_then(|area| providers.for_area(area));
        let Some(set) = set else { continue };

        for (j, obj) in q.objectives.iter().enumerate() {
            if let crate::stages::Objective::ReachAnchor { anchor, .. } = obj
                && !set.contains(anchor.as_str())
            {
                d.push(Diagnostic::error(
                    codes::ANCHOR_UNRESOLVED,
                    "quests",
                    format!("/content/quests/{i}/objectives/{j}/anchor"),
                    format!(
                        "objective anchor `{anchor}` is not provided by the prefab bound to this \
                         quest's area — use an anchor the prefab exposes (anchor names come from \
                         prefab metadata; do NOT invent one)"
                    ),
                ));
            }
        }
        // Effect anchors — **deep** and driven by the single anchor authority
        // ([`QuestEffect::anchor_refs`]), so every anchor-bearing effect at any
        // nesting depth resolves or is named. This scan used to be shallow
        // ([`for_each_effect`]) and enumerate variants by hand: a typo'd
        // `open-gate`/`set-block`/`set-checkpoint` anchor one level down (a
        // `sequence` step, an `on_respawn`/`on_caught`/`on_arrive` bundle)
        // validated clean and then emitted *nothing* — the silent-drop class of
        // bug the compiler's build-time seal (`DW0360`) now backstops.
        for_each_effect_deep(q, |path, eff| {
            // A `cutscene`'s camera anchors are legitimately cross-area (the
            // camera flies wherever the shot needs), so they resolve against the
            // union of every known area's anchors; every other effect anchor names
            // a world position the quest's own area must provide.
            let cross_area = matches!(eff, QuestEffect::Cutscene { .. });
            for (suffix, anchor) in eff.anchor_refs() {
                let resolves = if cross_area {
                    // An unknown/pool area makes the union incomplete — defer to
                    // the compiler's build-time seal rather than guess.
                    !providers.all_areas_known() || providers.union().contains(anchor.as_str())
                } else {
                    set.contains(anchor.as_str())
                };
                if resolves {
                    continue;
                }
                let scope = if cross_area {
                    "any area's prefab"
                } else {
                    "the prefab bound to this quest's area"
                };
                d.push(Diagnostic::error(
                    codes::ANCHOR_UNRESOLVED,
                    "quests",
                    format!("/content/quests/{i}/{path}/{suffix}"),
                    format!(
                        "`{verb}` anchor `{anchor}` is not provided by {scope} — use an anchor a \
                         prefab exposes (anchor names come from prefab metadata; do NOT invent \
                         one)",
                        verb = eff.verb(),
                    ),
                ));
            }
        });
    }

    // Environment triggers are global (no owning area), so their effect anchors
    // resolve against the union of every known area's anchors — the same
    // resolved-or-diagnostic rule as quest effects, applied at the only scope a
    // trigger has. Skipped entirely when some area binds a pool / an unknown
    // prefab, because then the union is not the whole truth.
    if providers.all_areas_known() {
        for (ti, t) in c.quests.content.triggers.iter().enumerate() {
            for_each_trigger_effect_deep(t, |path, eff| {
                for (suffix, anchor) in eff.anchor_refs() {
                    if providers.union().contains(anchor.as_str()) {
                        continue;
                    }
                    d.push(Diagnostic::error(
                        codes::ANCHOR_UNRESOLVED,
                        "quests",
                        format!("/content/triggers/{ti}/{path}/{suffix}"),
                        format!(
                            "`{verb}` anchor `{anchor}` in an environment trigger is not provided \
                             by any area's prefab — use an anchor a prefab exposes (anchor names \
                             come from prefab metadata; do NOT invent one)",
                            verb = eff.verb(),
                        ),
                    ));
                }
            });
        }
    }

    // Kit items.
    for (i, cl) in c.classes.content.classes.iter().enumerate() {
        for (j, it) in cl.kit.iter().enumerate() {
            if !items.contains(&it.item) {
                d.push(Diagnostic::error(
                    codes::ITEM_UNKNOWN,
                    "classes",
                    format!("/content/classes/{i}/kit/{j}/item"),
                    format!(
                        "kit item `{}` is not in the pinned 1.21.11 item registry — use a valid \
                         namespaced item id (e.g. `minecraft:iron_sword`)",
                        it.item
                    ),
                ));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Rule group 6 — prefab / prefab_pool binding (stage 1)
// ---------------------------------------------------------------------------

fn prefab_binding(c: &Campaign, anchors: &dyn AnchorRegistry, d: &mut Vec<Diagnostic>) {
    for (i, a) in c.world.content.areas.iter().enumerate() {
        // Exactly one of `prefab` / `prefab_pool`.
        match (&a.prefab, &a.prefab_pool) {
            (Some(_), Some(_)) => d.push(Diagnostic::error(
                codes::PREFAB_BINDING,
                "world",
                format!("/content/areas/{i}"),
                format!(
                    "area `{}` binds both `prefab` and `prefab_pool`; bind exactly one",
                    a.id
                ),
            )),
            (None, None) => d.push(Diagnostic::error(
                codes::PREFAB_BINDING,
                "world",
                format!("/content/areas/{i}"),
                format!(
                    "area `{}` binds neither `prefab` nor `prefab_pool`; bind exactly one",
                    a.id
                ),
            )),
            _ => {}
        }
        // A bound pool must resolve against the prefab-metadata surface.
        if let Some(pool) = &a.prefab_pool
            && pool.is_valid_syntax()
            && !anchors.has_pool(pool)
        {
            d.push(Diagnostic::error(
                codes::POOL_UNKNOWN,
                "world",
                format!("/content/areas/{i}/prefab_pool"),
                format!(
                    "area `prefab_pool` `{pool}` is not declared in the prefab metadata — bind a \
                     pool that exists in the prefabs dir, or add `{pool}` to the prefab library. \
                     This is a prefab-library/naming issue, not a quest-logic one"
                ),
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// Rule group 6 — cross-stage 1:1 (quest plan↔expansion, npc↔dialogue tree)
// ---------------------------------------------------------------------------

fn cross_stage(c: &Campaign, d: &mut Vec<Diagnostic>) {
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

    for (i, q) in c.quest_plan.content.quests.iter().enumerate() {
        if !expanded_ids.contains(q.id.as_str()) {
            d.push(Diagnostic::error(
                codes::QUEST_NOT_EXPANDED,
                "quest-plan",
                format!("/content/quests/{i}"),
                format!(
                    "planned quest `{}` has no stage-5 expansion — add a stage-5 quest with id \
                     `{}` (objectives/effects), or drop it from the stage-4 plan",
                    q.id, q.id
                ),
            ));
        }
    }
    for (i, q) in c.quests.content.quests.iter().enumerate() {
        if !planned_ids.contains(q.id.as_str()) {
            d.push(Diagnostic::error(
                codes::QUEST_NOT_PLANNED,
                "quests",
                format!("/content/quests/{i}"),
                format!(
                    "stage-5 quest `{}` is not planned in stage 4 — add a stage-4 plan entry with \
                     id `{}` (every quest must be planned), or remove this expansion",
                    q.id, q.id
                ),
            ));
        }
    }

    // Stage-2 NPC ↔ stage-6 dialogue tree, 1:1 both directions.
    let npc_ids: BTreeSet<&str> = c.npcs.content.npcs.iter().map(|n| n.id.as_str()).collect();
    let tree_npcs: BTreeSet<&str> = c
        .dialogue
        .content
        .dialogues
        .iter()
        .map(|t| t.npc.as_str())
        .collect();
    for (i, npc) in c.npcs.content.npcs.iter().enumerate() {
        if !tree_npcs.contains(npc.id.as_str()) {
            d.push(Diagnostic::error(
                codes::NPC_WITHOUT_TREE,
                "dialogue",
                format!("/content/npcs/{i}"),
                format!(
                    "npc `{}` has no stage-6 dialogue tree — every stage-2 npc needs exactly one \
                     tree; add a dialogue tree for `{}`, or remove the npc",
                    npc.id, npc.id
                ),
            ));
        }
    }
    for (i, tree) in c.dialogue.content.dialogues.iter().enumerate() {
        if !npc_ids.contains(tree.npc.as_str()) {
            d.push(Diagnostic::error(
                codes::TREE_WITHOUT_NPC,
                "dialogue",
                format!("/content/dialogues/{i}/npc"),
                format!(
                    "dialogue tree targets npc `{}`, which is not declared in stage 2 — declare \
                     that npc in stage 2, or point this tree at an existing npc",
                    tree.npc
                ),
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// DSL v0.3 — waves, flags, and the new stage-5 verbs
// ---------------------------------------------------------------------------

/// The v0.3 semantic checks (run only for `dsl_version` 0.3.0):
///
/// - wave declarations: id syntax (`DW0110`), uniqueness (`DW0111`), mob entity
///   ids (`DW0173`);
/// - `kill.wave` / `spawn-wave.wave` reference a declared wave (`DW0170`);
/// - a killed wave is spawned by some `spawn-wave` effect (`DW0171`);
/// - `requires_flags` reference a flag some `set-flag` effect produces
///   (`DW0172`);
/// - item ids on `collect` / `interact.requires_item` / `give-item` are in the
///   registry (`DW0143`, reused);
/// - `collect` / `interact` anchors resolve against the quest's single-prefab
///   area (`DW0142`, reused; pool-area anchors are resolved by the compiler,
///   as for `reach-anchor`).
fn v03_checks(
    c: &Campaign,
    items: &dyn ItemRegistry,
    anchors: &dyn AnchorRegistry,
    entities: &dyn EntityRegistry,
    d: &mut Vec<Diagnostic>,
) {
    let quests = &c.quests.content;

    // Wave declarations.
    let mut declared_waves: BTreeSet<&str> = BTreeSet::new();
    let mut seen_waves: BTreeSet<&str> = BTreeSet::new();
    for (i, w) in quests.waves.iter().enumerate() {
        if !w.id.is_valid_syntax() {
            d.push(Diagnostic::error(
                codes::ID_SYNTAX,
                "quests",
                format!("/content/waves/{i}/id"),
                format!(
                    "malformed wave id `{}` — wave ids must be lowercase kebab-case with the \
                     `wave/` prefix (e.g. `wave/ambush`)",
                    w.id
                ),
            ));
        }
        if !seen_waves.insert(w.id.as_str()) {
            d.push(Diagnostic::error(
                codes::ID_DUPLICATE,
                "quests",
                format!("/content/waves/{i}/id"),
                format!(
                    "duplicate wave id `{}` — rename one so every wave id is unique",
                    w.id
                ),
            ));
        }
        declared_waves.insert(w.id.as_str());
        for (k, m) in w.mobs.iter().enumerate() {
            if !entities.contains(&m.entity) {
                d.push(Diagnostic::error(
                    codes::ENTITY_UNKNOWN,
                    "quests",
                    format!("/content/waves/{i}/mobs/{k}/entity"),
                    format!(
                        "wave-mob entity `{}` is not a known 1.21.11 entity id — use a valid \
                         namespaced entity id (e.g. `minecraft:zombie`)",
                        m.entity
                    ),
                ));
            }
        }
    }

    // Flags declared by `set-flag`, from the ONE producer inventory
    // ([`produced_flags`]); waves spawned by `spawn-wave`.
    let declared_flags: BTreeSet<String> = produced_flags(c);
    let mut spawned_waves: BTreeSet<&str> = BTreeSet::new();
    crate::stages::for_each_campaign_effect(c, &mut |_path, _site, e| {
        if let Some(w) = e.spawn_wave() {
            spawned_waves.insert(w.as_str());
        }
    });

    // area id -> its single-prefab anchor set (pool areas deferred to compiler).
    let providers = AnchorProviders::build(c, anchors);
    let quest_area: BTreeMap<&str, &str> = c
        .quest_plan
        .content
        .quests
        .iter()
        .map(|q| (q.id.as_str(), q.area.as_str()))
        .collect();

    // Reference checks.
    for (i, q) in quests.quests.iter().enumerate() {
        let set = quest_area
            .get(q.id.as_str())
            .and_then(|area| providers.for_area(area));

        for (j, obj) in q.objectives.iter().enumerate() {
            match obj {
                Objective::Kill { wave, .. } => {
                    if !declared_waves.contains(wave.as_str()) {
                        d.push(Diagnostic::error(
                            codes::WAVE_UNKNOWN,
                            "quests",
                            format!("/content/quests/{i}/objectives/{j}/wave"),
                            format!(
                                "`kill` objective references unknown wave `{wave}` — declare it \
                                 in the stage-5 `waves` section or correct the reference"
                            ),
                        ));
                    } else if !spawned_waves.contains(wave.as_str()) {
                        d.push(Diagnostic::error(
                            codes::WAVE_NEVER_SPAWNED,
                            "quests",
                            format!("/content/quests/{i}/objectives/{j}/wave"),
                            format!(
                                "wave `{wave}` is killed but never spawned by any `spawn-wave` \
                                 effect — a wave must be spawned before its `kill` objective is \
                                 reachable; add a `spawn-wave` effect for `{wave}` on an earlier \
                                 objective/quest"
                            ),
                        ));
                    }
                }
                Objective::Collect {
                    id: oid,
                    item,
                    count,
                    anchor,
                    container,
                    dropped_by,
                    fill_count,
                    ..
                } => {
                    // v0.9: the wave a drop-gated collect names must
                    // exist — the same dangling-reference rule a `kill` follows.
                    if let Some(wave) = dropped_by
                        && !declared_waves.contains(wave.as_str())
                    {
                        d.push(Diagnostic::error(
                            codes::WAVE_UNKNOWN,
                            "quests",
                            format!("/content/quests/{i}/objectives/{j}/dropped_by"),
                            format!(
                                "`collect` `dropped_by` references unknown wave `{wave}` — \
                                 declare it in the stage-5 `waves` section or correct the \
                                 reference"
                            ),
                        ));
                    }
                    if !items.contains(item) {
                        d.push(Diagnostic::error(
                            codes::ITEM_UNKNOWN,
                            "quests",
                            format!("/content/quests/{i}/objectives/{j}/item"),
                            format!(
                                "collect item `{item}` is not in the pinned 1.21.11 item \
                                 registry — use a valid namespaced item id (e.g. \
                                 `minecraft:emerald`)"
                            ),
                        ));
                    }
                    // The objective's props are placed with a single-slot `item
                    // replace … container.0` fill, so an over-cap count leaves the
                    // chest empty and the objective uncompletable (`DW0436`).
                    check_stack_count(
                        item,
                        *count,
                        &format!("collect objective `{oid}`"),
                        format!("/content/quests/{i}/objectives/{j}/count"),
                        items,
                        d,
                    );
                    anchor_resolves(set, anchor, i, j, "anchor", d);
                    // v0.8: the adopted container's anchor must exist
                    // too. Whether its CELL really holds a chest/barrel needs the
                    // assembled world and is the build-tier `DW0438`.
                    if let Some(cont) = container {
                        anchor_resolves(set, cont, i, j, "container", d);
                    }
                    // v0.8: the fill is positional — the required stack
                    // in `container.0` plus one padding stack per slot after it —
                    // so it obeys the same 27-slot ceiling a `loot` declaration
                    // does, and for the same reason: every stack past the last slot
                    // is dropped without a word.
                    let slots = 1usize + *fill_count as usize;
                    if slots > MIN_CONTAINER_SLOTS {
                        d.push(Diagnostic::error(
                            codes::LOOT_TOO_MANY_ITEMS,
                            "quests",
                            format!("/content/quests/{i}/objectives/{j}/fill_count"),
                            format!(
                                "collect objective `{oid}` fills {slots} slots (its own stack \
                                 plus `fill_count` {fill_count}), more than the \
                                 {MIN_CONTAINER_SLOTS} slots a vanilla chest or barrel has. \
                                 Slots are assigned positionally, so every stack past the \
                                 {MIN_CONTAINER_SLOTS}th would be dropped silently. Lower \
                                 `fill_count` to at most {max} — a container that reads full \
                                 does not need to overflow.",
                                max = MIN_CONTAINER_SLOTS - 1
                            ),
                        ));
                    }
                }
                Objective::Interact {
                    anchor,
                    requires_item,
                    missing_item_hint,
                    ..
                } => {
                    // v0.7: the empty-hand narration answers the held-item gate;
                    // without a gate there is no missing hand to narrate to, and
                    // the authored line would be dead content that never fires.
                    if missing_item_hint.is_some() && requires_item.is_none() {
                        d.push(Diagnostic::error(
                            codes::MISSING_ITEM_HINT_WITHOUT_ITEM,
                            "quests",
                            format!("/content/quests/{i}/objectives/{j}/missing_item_hint"),
                            "`interact.missing_item_hint` narrates the click that arrives \
                             without the required item in hand, but this objective declares no \
                             `requires_item` — add the `requires_item` this hint is about, or \
                             drop the hint"
                                .to_string(),
                        ));
                    }
                    if let Some(it) = requires_item
                        && !items.contains(it)
                    {
                        d.push(Diagnostic::error(
                            codes::ITEM_UNKNOWN,
                            "quests",
                            format!("/content/quests/{i}/objectives/{j}/requires_item"),
                            format!(
                                "`interact.requires_item` `{it}` is not in the pinned 1.21.11 \
                                 item registry — use a valid namespaced item id (e.g. \
                                 `minecraft:tripwire_hook`)"
                            ),
                        ));
                    }
                    anchor_resolves(set, anchor, i, j, "anchor", d);
                }
                Objective::TalkTo { .. } | Objective::ReachAnchor { .. } => {}
            }

            for (m, f) in obj.requires_flags().iter().enumerate() {
                if !declared_flags.contains(f.as_str()) {
                    d.push(Diagnostic::error(
                        codes::FLAG_UNKNOWN,
                        "quests",
                        format!("/content/quests/{i}/objectives/{j}/requires_flags/{m}"),
                        format!(
                            "objective `requires_flags` references flag `{f}`, which no \
                             `set-flag` effect ever produces — add a `set-flag {{ flag: \"{f}\" }}` \
                             effect on an earlier objective/quest, or correct the flag name"
                        ),
                    ));
                }
            }
            // v0.6: `forbids_flags` gets the same unknown-flag treatment as
            // `requires_flags` — a never-produced flag can never suppress anything,
            // so the reference is dead (a typo until proven otherwise).
            for (m, f) in obj.forbids_flags().iter().enumerate() {
                if !declared_flags.contains(f.as_str()) {
                    d.push(Diagnostic::error(
                        codes::FLAG_UNKNOWN,
                        "quests",
                        format!("/content/quests/{i}/objectives/{j}/forbids_flags/{m}"),
                        format!(
                            "objective `forbids_flags` references flag `{f}`, which no `set-flag` \
                             effect ever produces — the gate can never suppress anything; add the \
                             producing `set-flag {{ flag: \"{f}\" }}` effect, or correct the flag \
                             name"
                        ),
                    ));
                }
            }
        }

        for_each_effect_deep(q, |path, eff| {
            if let Some(w) = eff.spawn_wave()
                && !declared_waves.contains(w.as_str())
            {
                d.push(Diagnostic::error(
                    codes::WAVE_UNKNOWN,
                    "quests",
                    format!("/content/quests/{i}/{path}/wave"),
                    format!(
                        "`spawn-wave` effect references unknown wave `{w}` — declare it in the \
                         stage-5 `waves` section or correct the reference"
                    ),
                ));
            }
            if let Some(it) = eff.give_item()
                && !items.contains(it)
            {
                d.push(Diagnostic::error(
                    codes::ITEM_UNKNOWN,
                    "quests",
                    format!("/content/quests/{i}/{path}/item"),
                    format!(
                        "`give-item` item `{it}` is not in the pinned 1.21.11 item registry — use \
                         a valid namespaced item id (e.g. `minecraft:golden_apple`)"
                    ),
                ));
            }
            // v0.6: per-effect `requires_flags` must resolve to a produced flag,
            // mirroring the objective/trigger `requires_flags` check (DW0172).
            for (n, f) in eff.requires_flags().iter().enumerate() {
                if !declared_flags.contains(f.as_str()) {
                    d.push(Diagnostic::error(
                        codes::FLAG_UNKNOWN,
                        "quests",
                        format!("/content/quests/{i}/{path}/requires_flags/{n}"),
                        format!(
                            "effect `requires_flags` references flag `{f}`, which no `set-flag` \
                             effect ever produces — add a `set-flag {{ flag: \"{f}\" }}` effect \
                             earlier, or correct the flag name"
                        ),
                    ));
                }
            }
            // v0.6: per-effect `forbids_flags` — same unknown-flag treatment.
            for (n, f) in eff.forbids_flags().iter().enumerate() {
                if !declared_flags.contains(f.as_str()) {
                    d.push(Diagnostic::error(
                        codes::FLAG_UNKNOWN,
                        "quests",
                        format!("/content/quests/{i}/{path}/forbids_flags/{n}"),
                        format!(
                            "effect `forbids_flags` references flag `{f}`, which no `set-flag` \
                             effect ever produces — the gate can never suppress anything; add the \
                             producing `set-flag {{ flag: \"{f}\" }}` effect, or correct the flag \
                             name"
                        ),
                    ));
                }
            }
        });
    }

    // v0.6: environment-trigger effect `requires_flags` / `forbids_flags`
    // resolution (DW0172).
    for (i, t) in quests.triggers.iter().enumerate() {
        for_each_trigger_effect_deep(t, |path, eff| {
            for (n, f) in eff.requires_flags().iter().enumerate() {
                if !declared_flags.contains(f.as_str()) {
                    d.push(Diagnostic::error(
                        codes::FLAG_UNKNOWN,
                        "quests",
                        format!("/content/triggers/{i}/{path}/requires_flags/{n}"),
                        format!(
                            "effect `requires_flags` references flag `{f}`, which no `set-flag` \
                             effect ever produces — add a `set-flag {{ flag: \"{f}\" }}` effect \
                             earlier, or correct the flag name"
                        ),
                    ));
                }
            }
            for (n, f) in eff.forbids_flags().iter().enumerate() {
                if !declared_flags.contains(f.as_str()) {
                    d.push(Diagnostic::error(
                        codes::FLAG_UNKNOWN,
                        "quests",
                        format!("/content/triggers/{i}/{path}/forbids_flags/{n}"),
                        format!(
                            "effect `forbids_flags` references flag `{f}`, which no `set-flag` \
                             effect ever produces — the gate can never suppress anything; add the \
                             producing `set-flag {{ flag: \"{f}\" }}` effect, or correct the flag \
                             name"
                        ),
                    ));
                }
            }
        });
    }
}

// ---------------------------------------------------------------------------
// DSL v0.4 — props, set-block, narrate, wave tuning, lifecycle, skins, triggers
// ---------------------------------------------------------------------------

/// The v0.4 semantic checks (run only for `dsl_version` 0.4.0):
///
/// - mannequin skins: `texture_id` syntax + uniqueness (`DW0190`);
/// - wave-mob `effects[].effect` ids in the effect registry (`DW0192`);
/// - `set-block` / `interact.prop` block ids in the block registry (`DW0193`);
/// - environment triggers: id syntax + uniqueness (`DW0194`), `at` anchor
///   resolves against some area (`DW0142`), `requires_flags` resolve (`DW0172`),
///   effect refs (item/block/wave) validated;
/// - lifecycle: `despawn-npc` / `move-npc` npc refs resolve (`DW0112`);
/// - dialogue `requires_flags` resolve (`DW0172`), and a `talk-to` with only
///   flag-gated completing options is a potential deadlock (`DW0191`);
/// - a `talk-to` targeting an NPC despawned earlier on a dependency path
///   (`DW0195`).
fn v04_checks(
    c: &Campaign,
    anchors: &dyn AnchorRegistry,
    blocks: &dyn BlockRegistry,
    effects: &dyn EffectRegistry,
    d: &mut Vec<Diagnostic>,
) {
    let quests = &c.quests.content;
    let npc_ids: BTreeSet<&str> = c.npcs.content.npcs.iter().map(|n| n.id.as_str()).collect();

    // area anchor sets (single-prefab areas only) + whether any pool area exists.
    let providers = AnchorProviders::build(c, anchors);

    // Declared flags across quest / dialogue / trigger `set-flag` effects and v0.6
    // trap disarms (a disarm's `sets_flag` is a first-class declared flag other
    // objectives/triggers may gate on).
    let flags = collect_declared_flags(c);
    let declared_waves: BTreeSet<&str> = quests.waves.iter().map(|w| w.id.as_str()).collect();

    // --- skins (spec-0009) ---
    let mut seen_skins: BTreeSet<&str> = BTreeSet::new();
    for (i, npc) in c.npcs.content.npcs.iter().enumerate() {
        if let Some(skin) = &npc.skin {
            if !is_kebab(&skin.texture_id) {
                d.push(Diagnostic::error(
                    codes::SKIN_INVALID,
                    "npcs",
                    format!("/content/npcs/{i}/skin/texture_id"),
                    format!(
                        "skin `texture_id` `{}` is malformed — it must be a bare kebab token \
                         (e.g. `keeper-armor`), matching the `skins/<texture_id>.png` filename",
                        skin.texture_id
                    ),
                ));
            }
            if !seen_skins.insert(skin.texture_id.as_str()) {
                d.push(Diagnostic::error(
                    codes::SKIN_INVALID,
                    "npcs",
                    format!("/content/npcs/{i}/skin/texture_id"),
                    format!(
                        "duplicate skin `texture_id` `{}` — each mannequin needs a distinct \
                         texture; rename one (and its `skins/<id>.png`)",
                        skin.texture_id
                    ),
                ));
            }
        }
    }

    // --- wave-mob effects + attributes ---
    for (i, w) in quests.waves.iter().enumerate() {
        for (k, m) in w.mobs.iter().enumerate() {
            for (e, eff) in m.effects.iter().enumerate() {
                if !effects.contains(&eff.effect) {
                    d.push(Diagnostic::error(
                        codes::EFFECT_UNKNOWN,
                        "quests",
                        format!("/content/waves/{i}/mobs/{k}/effects/{e}/effect"),
                        format!(
                            "wave-mob effect `{}` is not a known 1.21.11 status-effect id — use a \
                             valid namespaced effect id (e.g. `minecraft:strength`)",
                            eff.effect
                        ),
                    ));
                }
            }
        }
    }

    // --- block ids: interact props + set-block effects (quest + trigger) ---
    for (i, q) in quests.quests.iter().enumerate() {
        for (j, o) in q.objectives.iter().enumerate() {
            if let Some(prop) = o.prop() {
                check_block_field(
                    blocks,
                    &prop.block,
                    format!("/content/quests/{i}/objectives/{j}/prop/block"),
                    "interact.prop",
                    "minecraft:lever",
                    d,
                );
            }
        }
        for_each_effect_deep(q, |path, eff| {
            check_effect_v04(
                eff,
                blocks,
                &declared_waves,
                &format!("/content/quests/{i}/{path}"),
                &npc_ids,
                d,
            );
        });
    }

    // --- environment triggers ---
    let mut seen_triggers: BTreeSet<&str> = BTreeSet::new();
    for (i, t) in quests.triggers.iter().enumerate() {
        if !t.id.is_valid_syntax() {
            d.push(Diagnostic::error(
                codes::TRIGGER_INVALID,
                "quests",
                format!("/content/triggers/{i}/id"),
                format!(
                    "malformed trigger id `{}` — trigger ids must be lowercase kebab-case with \
                     the `trigger/` prefix (e.g. `trigger/pressure-plate`)",
                    t.id
                ),
            ));
        }
        if !seen_triggers.insert(t.id.as_str()) {
            d.push(Diagnostic::error(
                codes::TRIGGER_INVALID,
                "quests",
                format!("/content/triggers/{i}/id"),
                format!(
                    "duplicate trigger id `{}` — rename one so every trigger id is unique",
                    t.id
                ),
            ));
        }
        // `at` names a place; `strike-npc` names a character. Exactly one of the
        // two must be supplied, so neither form can be authored half-way (an
        // ignored anchor would read as meaningful and silently do nothing).
        match (t.on.needs_anchor(), t.at_anchor()) {
            (true, None) => d.push(Diagnostic::error(
                codes::TRIGGER_INVALID,
                "quests",
                format!("/content/triggers/{i}/at"),
                format!(
                    "trigger `{}` fires on `{}`, which watches a place, but declares no `at` \
                     anchor — add one (anchor names come from prefab metadata; do NOT invent \
                     one), or switch to `strike-npc` if the target is an NPC's body",
                    t.id,
                    t.on.kind()
                ),
            )),
            (false, Some(at)) => d.push(Diagnostic::error(
                codes::TRIGGER_INVALID,
                "quests",
                format!("/content/triggers/{i}/at"),
                format!(
                    "trigger `{}` fires on `strike-npc`, whose target is NPC `{}`'s body — it \
                     watches no cell, so the `at` anchor `{at}` names nothing and would be \
                     silently ignored. Remove `at`.",
                    t.id,
                    t.on.npc_target().map(|n| n.as_str()).unwrap_or("?")
                ),
            )),
            (true, Some(at)) if !providers.resolvable(at) => d.push(Diagnostic::error(
                codes::ANCHOR_UNRESOLVED,
                "quests",
                format!("/content/triggers/{i}/at"),
                format!(
                    "trigger `at` anchor `{at}` is not provided by any area's prefab — set `at` to \
                     an anchor some area's prefab exposes (anchor names come from prefab metadata; \
                     do NOT invent one)"
                ),
            )),
            _ => {}
        }
        // A `strike-npc` target must be a real stage-2 NPC: the trigger's tag
        // rides that NPC's hitbox, so an unknown id would emit a tag on nothing
        // and the trigger could never fire.
        if let Some(npc) = t.on.npc_target()
            && !c.npcs.content.npcs.iter().any(|n| n.id == *npc)
        {
            d.push(Diagnostic::error(
                codes::DANGLING_REF,
                "quests",
                format!("/content/triggers/{i}/on/npc"),
                format!(
                    "`strike-npc` trigger `{}` targets NPC `{npc}`, which stage 2 does not \
                     declare — use a declared npc id",
                    t.id
                ),
            ));
        }
        if let TriggerOn::Approach { range } = &t.on
            && *range == 0
        {
            d.push(Diagnostic::error(
                codes::TRIGGER_INVALID,
                "quests",
                format!("/content/triggers/{i}/on/range"),
                "`approach` trigger `range` must be > 0 — set a positive block radius (e.g. 3)"
                    .to_string(),
            ));
        }
        if matches!(t.on, TriggerOn::Use)
            && let Some(at) = t.at_anchor()
            && let Some(npc) = c.npcs.content.npcs.iter().find(|n| n.anchor.as_str() == at)
        {
            d.push(Diagnostic::error(
                codes::USE_TRIGGER_ON_NPC,
                "quests",
                format!("/content/triggers/{i}/at"),
                format!(
                    "`use` trigger `{}` is anchored at `{}`, where NPC `{}` stands — a \
                     right-click there already belongs to the NPC's dialogue, and two \
                     interaction hitboxes in one cell race for the same click (the loser is \
                     silently dead, which can soft-lock the delve). Move the trigger to its \
                     own anchor, or express the interaction as a dialogue option on the NPC. \
                     (To make an NPC's body itself the target, use `strike-npc`.)",
                    t.id, at, npc.id
                ),
            ));
        }
        for (m, f) in t.requires_flags.iter().enumerate() {
            if !flags.contains(f.as_str()) {
                d.push(Diagnostic::error(
                    codes::FLAG_UNKNOWN,
                    "quests",
                    format!("/content/triggers/{i}/requires_flags/{m}"),
                    format!(
                        "trigger `requires_flags` references flag `{f}`, which no `set-flag` \
                         effect ever produces — add a `set-flag {{ flag: \"{f}\" }}` effect \
                         somewhere, or correct the flag name"
                    ),
                ));
            }
        }
        // v0.6: trigger-level `forbids_flags` — same unknown-flag treatment as
        // `requires_flags` (DW0172).
        for (m, f) in t.forbids_flags.iter().enumerate() {
            if !flags.contains(f.as_str()) {
                d.push(Diagnostic::error(
                    codes::FLAG_UNKNOWN,
                    "quests",
                    format!("/content/triggers/{i}/forbids_flags/{m}"),
                    format!(
                        "trigger `forbids_flags` references flag `{f}`, which no `set-flag` \
                         effect ever produces — the gate can never suppress anything; add the \
                         producing `set-flag {{ flag: \"{f}\" }}` effect, or correct the flag name"
                    ),
                ));
            }
        }
        for_each_trigger_effect_deep(t, |path, eff| {
            check_effect_v04(
                eff,
                blocks,
                &declared_waves,
                &format!("/content/triggers/{i}/{path}"),
                &npc_ids,
                d,
            );
        });
    }

    // --- dialogue requires_flags resolution + flag-deadlock guard ---
    dialogue_v04(c, &flags, d);

    // --- despawned-npc references (DW0195) ---
    despawned_ref_check(c, &npc_ids, d);
    deferred_npc_checks(c, &npc_ids, d);
}

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
fn check_block_field(
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
    crate::stages::for_each_campaign_effect(c, &mut |_path, _site, e| {
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

/// DSL v0.6 trap validation (spec-0011). Each trap binds to an `anchor/trap`
/// marker and gives the mute prefab hardware meaning. Structural failures are
/// `DW0340` (a malformed/duplicate id, an `at`/`disarm.via` no area's prefab
/// provides, or a `disarm.via` colliding with the trap's own trigger anchor); a
/// dispense payload item unknown to the pinned registry is `DW0341`. A trap's
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
        if !providers.resolvable(t.at.as_str()) {
            d.push(Diagnostic::error(
                codes::TRAP_INVALID,
                "quests",
                format!("/content/traps/{i}/at"),
                format!(
                    "trap `at` anchor `{}` is not provided by any area's prefab — bind the trap to \
                     an `anchor/trap` marker some area's prefab exposes (anchor names come from \
                     prefab metadata; do NOT invent one)",
                    t.at
                ),
            ));
        }
        if let Some(dis) = &t.disarm {
            if !providers.resolvable(dis.via.as_str()) {
                d.push(Diagnostic::error(
                    codes::TRAP_INVALID,
                    "quests",
                    format!("/content/traps/{i}/disarm/via"),
                    format!(
                        "trap `disarm.via` anchor `{}` is not provided by any area's prefab — use \
                         an anchor some area's prefab exposes for the disarm affordance",
                        dis.via
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
            match eff {
                QuestEffect::Volley {
                    projectile,
                    salvos,
                    interval,
                    ..
                } => {
                    let proj = projectile
                        .as_deref()
                        .unwrap_or(crate::stages::DEFAULT_VOLLEY_PROJECTILE);
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
                    let n = salvos.unwrap_or(crate::stages::DEFAULT_VOLLEY_SALVOS);
                    if n == 0 || n > crate::stages::MAX_VOLLEY_SALVOS {
                        d.push(Diagnostic::error(
                            codes::VOLLEY_CADENCE,
                            "quests",
                            format!("{base}/salvos"),
                            format!(
                                "volley `salvos` is {n} — must be 1..={}. A volley fires \
                                 its whole kill zone every salvo, so the entity count is \
                                 `salvos x standable cells`; beyond the cap that is a \
                                 server hazard, not a trap",
                                crate::stages::MAX_VOLLEY_SALVOS
                            ),
                        ));
                    }
                    let iv = interval.unwrap_or(crate::stages::DEFAULT_VOLLEY_INTERVAL);
                    if iv == 0 || iv > crate::stages::MAX_VOLLEY_INTERVAL {
                        d.push(Diagnostic::error(
                            codes::VOLLEY_CADENCE,
                            "quests",
                            format!("{base}/interval"),
                            format!(
                                "volley `interval` is {iv} ticks — must be 1..={}. Salvos \
                                 spaced wider than that stop reading as one trap event",
                                crate::stages::MAX_VOLLEY_INTERVAL
                            ),
                        ));
                    }
                }
                QuestEffect::Collapse {
                    falling_block,
                    then_floor,
                    ..
                } => {
                    let blocks = ItemBackedBlockRegistry::new(items);
                    let fb = falling_block
                        .as_deref()
                        .unwrap_or(crate::stages::DEFAULT_COLLAPSE_FALLING_BLOCK);
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

/// Validate a v0.4-relevant [`QuestEffect`]'s refs: `set-block` block id
/// (`DW0193`), `despawn-npc`/`move-npc` npc ids (`DW0112`), `move-npc` speed
/// positivity, `cutscene` shape (`DW0199`). Item/wave/flag refs are covered by
/// the shared v0.3 checks.
fn check_effect_v04(
    eff: &QuestEffect,
    blocks: &dyn BlockRegistry,
    _declared_waves: &BTreeSet<&str>,
    base_path: &str,
    npc_ids: &BTreeSet<&str>,
    d: &mut Vec<Diagnostic>,
) {
    check_cutscene_shape(eff, base_path, d);
    match eff {
        QuestEffect::SetBlock { block, .. } => {
            check_block_field(
                blocks,
                block,
                format!("{base_path}/block"),
                "set-block",
                "minecraft:air",
                d,
            );
        }
        // The v0.10 region write (spec-0031) places the same kind of block over a
        // box instead of a cell, so it is the same `DW0193` check — the registry
        // belongs to "a block id an author wrote", not to `set-block`.
        QuestEffect::FillRegion { block, .. } => {
            check_block_field(
                blocks,
                block,
                format!("{base_path}/block"),
                "fill-region",
                "minecraft:stone",
                d,
            );
        }
        // NPC-lifecycle refs. `spawn-npc` (v0.6) joins the v0.4 `despawn-npc`/
        // `move-npc` family: an unknown npc id is the same `DW0112` dangling ref.
        QuestEffect::DespawnNpc { npc, .. }
        | QuestEffect::MoveNpc { npc, .. }
        | QuestEffect::SpawnNpc { npc, .. }
            if !npc_ids.contains(npc.as_str()) =>
        {
            let verb = eff
                .v04_effect()
                .or_else(|| eff.v06_effect())
                .unwrap_or("lifecycle");
            d.push(Diagnostic::error(
                codes::DANGLING_REF,
                "quests",
                format!("{base_path}/npc"),
                format!(
                    "`{verb}` references unknown npc `{npc}` — declare it in stage 2 or correct \
                     the reference"
                ),
            ));
        }
        _ => {}
    }
}

/// `cutscene` shape (`DW0199`): a cutscene is written either multi-shot
/// (`shots: [...]`, DSL v0.6) or single-shot (`path` + `seconds`, DSL v0.4) —
/// never both, never neither — and every resolved shot needs at least one camera
/// waypoint. The two spellings normalize to the same shot list
/// ([`QuestEffect::cutscene_shots`]), so this is the one place the shape is
/// policed; emission may then assume a non-empty, well-formed list.
fn check_cutscene_shape(eff: &QuestEffect, base_path: &str, d: &mut Vec<Diagnostic>) {
    let QuestEffect::Cutscene {
        shots,
        path,
        seconds,
        ..
    } = eff
    else {
        return;
    };
    let single = !path.is_empty() || seconds.is_some();
    let err = |d: &mut Vec<Diagnostic>, field: &str, msg: String| {
        d.push(Diagnostic::error(
            codes::CUTSCENE_SHAPE,
            "quests",
            format!("{base_path}/{field}"),
            msg,
        ));
    };
    match (!shots.is_empty(), single) {
        (true, true) => err(
            d,
            "shots",
            "`cutscene` mixes the multi-shot `shots` list with the single-shot \
             `path`/`seconds` fields — use one form: move the single-shot fields into a `shots` \
             entry, or drop `shots`"
                .to_string(),
        ),
        (false, false) => err(
            d,
            "shots",
            "`cutscene` declares no shot — give a `shots` list of \
             `{path, seconds, look_at?}` (multi-shot), or a single-shot `path` + `seconds`"
                .to_string(),
        ),
        (false, true) if seconds.is_none() => err(
            d,
            "seconds",
            "single-shot `cutscene` is missing `seconds` — every shot needs a duration".to_string(),
        ),
        _ => {}
    }
    for (i, shot) in shots.iter().enumerate() {
        // A `shot_style` supplies both a default dolly and a default duration
        // (spec-0015), so `path`/`seconds` become optional overrides on a
        // styled shot; the style's own shape is policed by `DW0348`/`DW0349`.
        if shot.shot_style.is_some() {
            continue;
        }
        if shot.path.is_empty() {
            err(
                d,
                &format!("shots/{i}/path"),
                "`cutscene` shot has an empty camera `path` — give at least one waypoint (one \
                 waypoint is a static shot, two or more is a dolly), or use a `shot_style`"
                    .to_string(),
            );
        }
        if shot.seconds.is_none() {
            err(
                d,
                &format!("shots/{i}/seconds"),
                "`cutscene` shot is missing `seconds` — every unstyled shot needs an explicit \
                 duration (a `shot_style` would supply a default)"
                    .to_string(),
            );
        }
    }
    if shots.is_empty() && single && path.is_empty() {
        err(
            d,
            "path",
            "single-shot `cutscene` has an empty camera `path` — give at least one waypoint (one \
             waypoint is a static shot, two or more is a dolly)"
                .to_string(),
        );
    }
}

/// Shot-style semantics (DSL v0.6, spec-0015 shot grammar): `DW0348` for
/// invalid style/param combinations, `DW0112` for a subject referencing an
/// unknown npc/actor, and `DW0349` for a `side-track`/`low-follow` whose
/// subject provably cannot move.
///
/// The moving-subject scope mirrors the compiler's expansion resolution
/// exactly: a subject "moves" if a matching `move-npc`/`move-actor` runs in the
/// **same effect list**, or anywhere in the **same `sequence` timeline**
/// (including the list that launched the sequence). Nested reaction lists
/// (`on_arrive`/`on_caught`/`on_respawn`) start a fresh scope — their firing
/// time is unknowable statically, so motion outside them is never assumed.
fn cutscene_style_checks(
    quests: &crate::stages::QuestsContent,
    npc_ids: &BTreeSet<&str>,
    d: &mut Vec<Diagnostic>,
) {
    use crate::stages::{CameraSubject, ShotStyle};
    let actor_ids: BTreeSet<&str> = quests.actors.iter().map(|a| a.id.as_str()).collect();

    /// The sibling moves visible to a cutscene: `(is_actor, id)`.
    fn moves_in(list: &[QuestEffect], scope: &mut Vec<(bool, String)>) {
        for e in list {
            match e {
                QuestEffect::MoveNpc { npc, .. } => scope.push((false, npc.to_string())),
                QuestEffect::MoveActor { actor, .. } => scope.push((true, actor.to_string())),
                QuestEffect::Sequence { steps } => {
                    // A sequence launched from this list shares its timeline.
                    for st in steps {
                        moves_in(&st.effects, scope);
                    }
                }
                _ => {}
            }
        }
    }

    fn subject_check(
        sub: &CameraSubject,
        field: &str,
        path: &str,
        npc_ids: &BTreeSet<&str>,
        actor_ids: &BTreeSet<&str>,
        d: &mut Vec<Diagnostic>,
    ) {
        let (unknown, kind, id) = match sub {
            CameraSubject::Anchor(_) => return,
            CameraSubject::Npc(s) => (!npc_ids.contains(s.npc.as_str()), "npc", s.npc.as_str()),
            CameraSubject::Actor(s) => (
                !actor_ids.contains(s.actor.as_str()),
                "actor",
                s.actor.as_str(),
            ),
        };
        if unknown {
            d.push(Diagnostic::error(
                codes::DANGLING_REF,
                "quests",
                format!("{path}/{field}"),
                format!(
                    "shot `{field}` references unknown {kind} `{id}` — declare it (stage 2 npcs / \
                     stage-5 `actors`) or correct the reference"
                ),
            ));
        }
    }

    fn check_shot_list(
        eff: &QuestEffect,
        scope: &[(bool, String)],
        path: &str,
        npc_ids: &BTreeSet<&str>,
        actor_ids: &BTreeSet<&str>,
        d: &mut Vec<Diagnostic>,
    ) {
        let QuestEffect::Cutscene { shots, .. } = eff else {
            return;
        };
        for (i, shot) in shots.iter().enumerate() {
            let spath = format!("{path}/shots/{i}");
            let err = |d: &mut Vec<Diagnostic>, field: &str, msg: String| {
                d.push(Diagnostic::error(
                    codes::SHOT_STYLE_INVALID,
                    "quests",
                    format!("{spath}/{field}"),
                    msg,
                ));
            };
            let Some(style) = shot.shot_style else {
                for (field, present) in [
                    ("subject", shot.subject.is_some()),
                    ("subject_b", shot.subject_b.is_some()),
                    ("dist", shot.dist.is_some()),
                    ("degrees", shot.degrees.is_some()),
                    ("bearing", shot.bearing.is_some()),
                ] {
                    if present {
                        err(
                            d,
                            field,
                            format!(
                                "`{field}` is a `shot_style` parameter but this shot declares no \
                                 `shot_style` — add one, or drop the field"
                            ),
                        );
                    }
                }
                continue;
            };
            let token = style.token();
            match &shot.subject {
                None => err(
                    d,
                    "subject",
                    format!(
                        "`shot_style: {token}` needs a `subject` — the anchor, npc, or actor the \
                         shot frames"
                    ),
                ),
                Some(sub) => subject_check(sub, "subject", &spath, npc_ids, actor_ids, d),
            }
            match (&shot.subject_b, style == ShotStyle::TwoShot) {
                (None, true) => err(
                    d,
                    "subject_b",
                    "`shot_style: two-shot` frames two subjects — give `subject_b`".to_string(),
                ),
                (Some(_), false) => err(
                    d,
                    "subject_b",
                    format!("`subject_b` is only meaningful on `two-shot`, not `{token}`"),
                ),
                (Some(sub), true) => subject_check(sub, "subject_b", &spath, npc_ids, actor_ids, d),
                (None, false) => {}
            }
            if let Some(g) = shot.degrees {
                if style != ShotStyle::OrbitArc {
                    err(
                        d,
                        "degrees",
                        format!("`degrees` is only meaningful on `orbit-arc`, not `{token}`"),
                    );
                } else if !(45.0..=120.0).contains(&g) {
                    err(
                        d,
                        "degrees",
                        format!(
                            "`orbit-arc` sweep `{g}` is outside `45..=120` degrees (the dossier's \
                             readable-orbit range)"
                        ),
                    );
                }
            }
            if let Some(dist) = shot.dist
                && !(1.0..=48.0).contains(&dist)
            {
                err(
                    d,
                    "dist",
                    format!("`dist` `{dist}` is outside the sane `1..=48` block range"),
                );
            }
            if let Some(b) = shot.bearing
                && !(-360.0..=360.0).contains(&b)
            {
                err(
                    d,
                    "bearing",
                    format!("`bearing` `{b}` is outside `-360..=360` degrees"),
                );
            }
            if style.needs_moving_subject() {
                let moved = match &shot.subject {
                    Some(CameraSubject::Npc(s)) => {
                        scope.iter().any(|(a, id)| !a && id == s.npc.as_str())
                    }
                    Some(CameraSubject::Actor(s)) => {
                        scope.iter().any(|(a, id)| *a && id == s.actor.as_str())
                    }
                    // An anchor can never move; a missing subject already got DW0348.
                    Some(CameraSubject::Anchor(_)) => false,
                    None => true,
                };
                if !moved {
                    d.push(Diagnostic::error(
                        codes::SHOT_SUBJECT_UNMOVED,
                        "quests",
                        format!("{spath}/subject"),
                        format!(
                            "`shot_style: {token}` dollies with a MOVING subject, but this \
                             subject has no matching `move-npc`/`move-actor` in the same effect \
                             group or sequence — add the move alongside the cutscene, or use a \
                             static style (`locked-off`, `push-in`)"
                        ),
                    ));
                }
            }
        }
    }

    /// Walk an effect list with its move scope; recurse into nested lists.
    fn walk_list(
        list: &[QuestEffect],
        outer_scope: &[(bool, String)],
        path: &str,
        npc_ids: &BTreeSet<&str>,
        actor_ids: &BTreeSet<&str>,
        d: &mut Vec<Diagnostic>,
    ) {
        let mut scope = outer_scope.to_vec();
        moves_in(list, &mut scope);
        for (j, e) in list.iter().enumerate() {
            let epath = format!("{path}/{j}");
            check_shot_list(e, &scope, &epath, npc_ids, actor_ids, d);
            for (pseg, _kseg, inner) in e.nested_effect_lists_labeled() {
                // A sequence step shares this timeline's scope; reaction lists
                // (`on_arrive`/`on_caught`/`on_respawn`) fire at an unknowable
                // time and start fresh.
                let inherited: &[(bool, String)] = if matches!(e, QuestEffect::Sequence { .. }) {
                    &scope
                } else {
                    &[]
                };
                walk_list(
                    inner,
                    inherited,
                    &format!("{epath}/{pseg}"),
                    npc_ids,
                    actor_ids,
                    d,
                );
            }
        }
    }

    for (i, q) in quests.quests.iter().enumerate() {
        for (key, effs) in &q.on_objective_complete {
            walk_list(
                effs,
                &[],
                &format!("/content/quests/{i}/on_objective_complete/{key}"),
                npc_ids,
                &actor_ids,
                d,
            );
        }
        walk_list(
            &q.on_complete,
            &[],
            &format!("/content/quests/{i}/on_complete"),
            npc_ids,
            &actor_ids,
            d,
        );
    }
    for (i, t) in quests.triggers.iter().enumerate() {
        walk_list(
            &t.effects,
            &[],
            &format!("/content/triggers/{i}/effects"),
            npc_ids,
            &actor_ids,
            d,
        );
    }
}

/// Dialogue v0.4: option `requires_flags` resolve against declared flags
/// (`DW0172`); a `talk-to` whose completing options are all flag-gated is a
/// potential deadlock (`DW0191`, spec-0008 §1).
fn dialogue_v04(c: &Campaign, flags: &BTreeSet<&str>, d: &mut Vec<Diagnostic>) {
    use crate::stages::DialogueEffect;
    // Option requires_flags resolution.
    for (i, tree) in c.dialogue.content.dialogues.iter().enumerate() {
        for (j, node) in tree.nodes.iter().enumerate() {
            for (k, opt) in node.options.iter().enumerate() {
                for (m, f) in opt.requires_flags.iter().enumerate() {
                    if !flags.contains(f.as_str()) {
                        d.push(Diagnostic::error(
                            codes::FLAG_UNKNOWN,
                            "dialogue",
                            format!(
                                "/content/dialogues/{i}/nodes/{j}/options/{k}/requires_flags/{m}"
                            ),
                            format!(
                                "dialogue option `requires_flags` references flag `{f}`, which no \
                                 `set-flag` effect ever produces — add a `set-flag {{ flag: \
                                 \"{f}\" }}` effect somewhere, or correct the flag name"
                            ),
                        ));
                    }
                }
            }
        }
    }
    // v0.6: dialogue option `forbids_flags` — same unknown-flag treatment as
    // `requires_flags` (DW0172).
    for (i, tree) in c.dialogue.content.dialogues.iter().enumerate() {
        for (j, node) in tree.nodes.iter().enumerate() {
            for (k, opt) in node.options.iter().enumerate() {
                for (m, f) in opt.forbids_flags.iter().enumerate() {
                    if !flags.contains(f.as_str()) {
                        d.push(Diagnostic::error(
                            codes::FLAG_UNKNOWN,
                            "dialogue",
                            format!(
                                "/content/dialogues/{i}/nodes/{j}/options/{k}/forbids_flags/{m}"
                            ),
                            format!(
                                "dialogue option `forbids_flags` references flag `{f}`, which no \
                                 `set-flag` effect ever produces — the gate can never suppress \
                                 anything; add the producing `set-flag {{ flag: \"{f}\" }}` \
                                 effect, or correct the flag name"
                            ),
                        ));
                    }
                }
            }
        }
    }
    // Per-NPC: objectives completed by an UNGATED option in that npc's tree. An
    // option gated either way — `requires_flags` (hidden until set) or, v0.6,
    // `forbids_flags` (hidden once set) — counts as gated: the static analysis
    // does no temporal reasoning about which flags end up set, so any
    // conditionally-visible option may be unavailable exactly when needed.
    let mut ungated_completes: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    let mut any_completes: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for tree in &c.dialogue.content.dialogues {
        let npc = tree.npc.as_str();
        for node in &tree.nodes {
            for opt in &node.options {
                for eff in &opt.effects {
                    if let DialogueEffect::CompleteObjective { objective } = eff {
                        any_completes
                            .entry(npc)
                            .or_default()
                            .insert(objective.as_str());
                        if opt.requires_flags.is_empty() && opt.forbids_flags.is_empty() {
                            ungated_completes
                                .entry(npc)
                                .or_default()
                                .insert(objective.as_str());
                        }
                    }
                }
            }
        }
    }
    for (qi, q) in c.quests.content.quests.iter().enumerate() {
        for (oi, o) in q.objectives.iter().enumerate() {
            if let Objective::TalkTo { id, npc, .. } = o {
                let npc = npc.as_str();
                let oid = id.as_str();
                let completed = any_completes.get(npc).is_some_and(|s| s.contains(oid));
                let ungated = ungated_completes.get(npc).is_some_and(|s| s.contains(oid));
                // Only when it IS completed somewhere (else DW0123 fires) but every
                // completing option is flag-gated.
                if completed && !ungated {
                    d.push(Diagnostic::error(
                        codes::DIALOGUE_FLAG_DEADLOCK,
                        "quests",
                        format!("/content/quests/{qi}/objectives/{oi}"),
                        format!(
                            "`talk-to` objective `{id}` has no ungated completing dialogue option \
                             in npc `{npc}`'s tree — every completing option is `requires_flags`- \
                             or `forbids_flags`-gated, so it can be unavailable the moment it is \
                             needed; keep at least one completing option with no flag gate"
                        ),
                    ));
                }
            }
        }
    }
}

/// Collect every NPC `e` (or an effect nested inside it) despawns **on every
/// playthrough that runs `e` at all** — the only despawns `DW0195` may reason
/// about, because its model is quest-DAG order with no branch semantics.
///
/// Two things stop the descent, and each is a real branch rather than a
/// convenience:
///
/// - **A flag gate.** An effect carrying `requires_flags`/`forbids_flags` fires
///   only when campaign state says so. The island's Perimedes walks out through
///   the cave mouth and despawns *only* on the flee branch (`flag/flee`); the
///   `talk-to`s that follow live on the sealed-in branch. Counting that despawn
///   would reject a perfectly playable delve. Branch-conditional reachability is
///   the branch-coherent completability proof's job (`DW0204`), not this rule's.
/// - **A lifecycle reaction bundle.** `set-checkpoint`'s `on_respawn` runs only if
///   a player dies and `begin-stealth`'s `on_caught` only if one is caught, so
///   neither is guaranteed. A `sequence` step and a `move-*` `on_arrive` *are*
///   guaranteed once their parent runs, so the descent continues through them.
fn unconditional_despawns<'a>(e: &'a QuestEffect, out: &mut Vec<&'a crate::ids::NpcId>) {
    if !e.requires_flags().is_empty() || !e.forbids_flags().is_empty() {
        return;
    }
    if let Some(npc) = e.despawn_npc() {
        out.push(npc);
    }
    for (_pseg, kseg, list) in e.nested_effect_lists_labeled() {
        if kseg == "respawn" || kseg == "caught" {
            continue;
        }
        for inner in list {
            unconditional_despawns(inner, out);
        }
    }
}

/// DW0195: a `talk-to` targeting an NPC despawned by an effect that runs strictly
/// before it on the quest dependency graph. Conservative: quest-ancestor despawn
/// (via `on_complete`) or same-quest earlier-objective despawn (via
/// `on_objective_complete` on a prerequisite `after` objective).
fn despawned_ref_check(c: &Campaign, _npc_ids: &BTreeSet<&str>, d: &mut Vec<Diagnostic>) {
    // Quest transitive ancestors (a quest completes before its dependents start).
    let deps: BTreeMap<&str, &Vec<crate::ids::QuestId>> = c
        .quest_plan
        .content
        .quests
        .iter()
        .map(|q| (q.id.as_str(), &q.depends_on))
        .collect();
    let ancestors = |q: &str| -> BTreeSet<&str> {
        let mut out = BTreeSet::new();
        let mut stack = vec![q];
        while let Some(cur) = stack.pop() {
            if let Some(ds) = deps.get(cur) {
                for dep in ds.iter() {
                    if out.insert(dep.as_str()) {
                        stack.push(dep.as_str());
                    }
                }
            }
        }
        out
    };

    // Where each npc is despawned: quests that despawn it on completion. Deep, but
    // only through effects that are **certain to run** (see
    // [`unconditional_despawns`]) — a `despawn-npc` nested one level down in a
    // `sequence` step removes the NPC exactly as thoroughly as a top-level one, and
    // the shallow scan this replaces walked straight past it.
    let mut despawn_quest: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for q in &c.quests.content.quests {
        for e in q
            .on_objective_complete
            .values()
            .flatten()
            .chain(&q.on_complete)
        {
            let mut npcs = Vec::new();
            unconditional_despawns(e, &mut npcs);
            for npc in npcs {
                despawn_quest
                    .entry(npc.as_str())
                    .or_default()
                    .insert(q.id.as_str());
            }
        }
    }
    if despawn_quest.is_empty() {
        return;
    }
    for (qi, q) in c.quests.content.quests.iter().enumerate() {
        let anc = ancestors(q.id.as_str());
        for (oi, o) in q.objectives.iter().enumerate() {
            if let Objective::TalkTo { npc, .. } = o
                && let Some(dq) = despawn_quest.get(npc.as_str())
                && dq.iter().any(|dqid| anc.contains(dqid))
            {
                d.push(Diagnostic::error(
                    codes::NPC_DESPAWNED_REF,
                    "quests",
                    format!("/content/quests/{qi}/objectives/{oi}/npc"),
                    format!(
                        "`talk-to` targets npc `{npc}`, which a prerequisite quest despawns — the \
                         npc is gone by the time this objective activates; talk to `{npc}` before \
                         the quest that despawns it, or drop the `despawn-npc`"
                    ),
                ));
            }
        }
    }
}

/// Transitive stage-4 quest ancestors: `q -> {every quest that must complete before
/// q starts}` (the `depends_on` closure). Acyclicity is guaranteed by `DW0130`.
fn quest_ancestors(c: &Campaign) -> BTreeMap<&str, BTreeSet<&str>> {
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

/// `deferred` NPC staging proofs (DSL v0.6), the dual of `despawned_ref_check`:
///
/// * `DW0112` — a dialogue `spawn-npc` naming an unknown NPC (the quest-effect form
///   is covered by `check_effect_v04`).
/// * `DW0197` — a `deferred: true` NPC that **no** `spawn-npc` anywhere summons: it
///   never enters the world, so its tree and any `talk-to` on it are dead content.
/// * `DW0198` — a `talk-to` on a deferred NPC that provably activates before the
///   NPC exists: every `spawn-npc` for it lives in a quest that is a strict DAG
///   *descendant* of the objective's quest. Conservative by construction — a spawn
///   from a trigger, from dialogue, or from the objective's own quest is not
///   DAG-ordered, so it suppresses the proof rather than risking a false positive.
fn deferred_npc_checks(c: &Campaign, npc_ids: &BTreeSet<&str>, d: &mut Vec<Diagnostic>) {
    use crate::stages::DialogueEffect;
    let deferred: BTreeSet<&str> = c
        .npcs
        .content
        .npcs
        .iter()
        .filter(|n| n.deferred)
        .map(|n| n.id.as_str())
        .collect();

    // Spawn sites. `quest_spawns`: npc -> quests whose effects spawn it (DAG-ordered).
    // `loose_spawns`: npcs spawned from a trigger or a dialogue option — sources with
    // no position on the quest DAG.
    let mut quest_spawns: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut loose_spawns: BTreeSet<String> = BTreeSet::new();
    for q in &c.quests.content.quests {
        let qid = q.id.as_str().to_string();
        for_each_effect_deep(q, |_path, eff| {
            if let Some(npc) = eff.spawn_npc() {
                quest_spawns
                    .entry(npc.as_str().to_string())
                    .or_default()
                    .insert(qid.clone());
            }
        });
    }
    for t in &c.quests.content.triggers {
        for_each_trigger_effect_deep(t, |_path, eff| {
            if let Some(npc) = eff.spawn_npc() {
                loose_spawns.insert(npc.as_str().to_string());
            }
        });
    }
    for (i, tree) in c.dialogue.content.dialogues.iter().enumerate() {
        for (j, node) in tree.nodes.iter().enumerate() {
            for (k, opt) in node.options.iter().enumerate() {
                for (m, eff) in opt.effects.iter().enumerate() {
                    let DialogueEffect::SpawnNpc { npc } = eff else {
                        continue;
                    };
                    if !npc_ids.contains(npc.as_str()) {
                        d.push(Diagnostic::error(
                            codes::DANGLING_REF,
                            "dialogue",
                            format!("/content/dialogues/{i}/nodes/{j}/options/{k}/effects/{m}/npc"),
                            format!(
                                "dialogue `spawn-npc` references unknown npc `{npc}` — declare it \
                                 in stage 2 or correct the reference"
                            ),
                        ));
                        continue;
                    }
                    loose_spawns.insert(npc.as_str().to_string());
                }
            }
        }
    }

    // DW0197: deferred but never spawned anywhere.
    for (i, n) in c.npcs.content.npcs.iter().enumerate() {
        if !n.deferred {
            continue;
        }
        let id = n.id.as_str();
        if quest_spawns.contains_key(id) || loose_spawns.contains(id) {
            continue;
        }
        d.push(Diagnostic::error(
            codes::NPC_NEVER_SPAWNED,
            "npcs",
            format!("/content/npcs/{i}/deferred"),
            format!(
                "npc `{id}` is `deferred: true` but no `spawn-npc` effect anywhere in the \
                 campaign summons it — it never enters the world, so its dialogue tree (and any \
                 `talk-to` on it) is unreachable content. Add a `spawn-npc {{ npc: \"{id}\" }}` \
                 effect at the beat where the character should walk in, or drop `deferred` so it \
                 stands at its anchor from world init. Do NOT delete the dialogue tree to silence \
                 this — every stage-2 npc needs one (`DW0152`)"
            ),
        ));
    }
    if deferred.is_empty() {
        return;
    }

    // DW0198: a `talk-to` on a deferred npc whose every spawn site is a strict DAG
    // descendant of the objective's quest.
    let ancestors = quest_ancestors(c);
    for (qi, q) in c.quests.content.quests.iter().enumerate() {
        for (oi, o) in q.objectives.iter().enumerate() {
            let Objective::TalkTo { npc, .. } = o else {
                continue;
            };
            let npc = npc.as_str();
            if !deferred.contains(npc) || loose_spawns.contains(npc) {
                continue;
            }
            let Some(sqs) = quest_spawns.get(npc) else {
                continue; // never spawned at all — already DW0197
            };
            let all_later = sqs.iter().all(|sq| {
                sq.as_str() != q.id.as_str()
                    && ancestors
                        .get(sq.as_str())
                        .is_some_and(|anc| anc.contains(q.id.as_str()))
            });
            if !all_later {
                continue;
            }
            let names: Vec<&str> = sqs.iter().map(|s| s.as_str()).collect();
            d.push(Diagnostic::error(
                codes::NPC_SPAWNED_LATE,
                "quests",
                format!("/content/quests/{qi}/objectives/{oi}/npc"),
                format!(
                    "`talk-to` targets deferred npc `{npc}`, but every `spawn-npc` for it fires \
                     in a quest that depends on this one (`{}`) — the objective activates on an \
                     empty anchor and can never complete. Move the `spawn-npc` to this quest or \
                     one of its prerequisites, or move the `talk-to` after the entrance. Do NOT \
                     drop `deferred` just to pass this — that puts the character back on stage \
                     from minute one",
                    names.join("`, `")
                ),
            ));
        }
    }
}

/// Push `DW0142` if `anchor` is not provided by the quest's (known single-prefab)
/// area. `None` set = pool area or unknown prefab → deferred to the compiler.
fn anchor_resolves(
    set: Option<&BTreeSet<String>>,
    anchor: &crate::ids::AnchorId,
    qi: usize,
    oi: usize,
    field: &str,
    d: &mut Vec<Diagnostic>,
) {
    if let Some(set) = set
        && !set.contains(anchor.as_str())
    {
        d.push(Diagnostic::error(
            codes::ANCHOR_UNRESOLVED,
            "quests",
            format!("/content/quests/{qi}/objectives/{oi}/{field}"),
            format!(
                "objective `{field}` anchor `{anchor}` is not provided by the prefab bound to \
                 this quest's area — use an anchor the prefab exposes (anchor names come from \
                 prefab metadata; do NOT invent one)"
            ),
        ));
    }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// Directed-graph cycle detection (three-color DFS).
fn graph_has_cycle<'a>(nodes: &[&'a str], edges: &BTreeMap<&'a str, Vec<&'a str>>) -> bool {
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
/// shape/recipe well-formedness (`DW0162`), block ids (`DW0193`), and the v0.6
/// stage gate (`DW0141`). Frame/region *resolution* against the solved layout
/// is the compiler's job (`DW0323`) — validation never needs prefabs.
fn world_edits_checks(c: &Campaign, blocks: &dyn BlockRegistry, d: &mut Vec<Diagnostic>) {
    let Some(env) = &c.world_edits else {
        return;
    };
    let stage = Stage::WorldEdits.name();
    if is_supported_version(env.dsl_version.as_str()) && !is_v06(env.dsl_version.as_str()) {
        d.push(Diagnostic::error(
            codes::RESERVED,
            stage,
            "/dsl_version",
            format!(
                "the `world-edits` stage is DSL v0.6 surface (spec-0017) but this document \
                 declares dsl_version `{}` — set it to `0.6.0` (the stage did not exist before \
                 v0.6, so an earlier version cannot carry it)",
                env.dsl_version
            ),
        ));
    }

    let areas: BTreeSet<&str> = c
        .world
        .content
        .areas
        .iter()
        .map(|a| a.id.as_str())
        .collect();

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
        recipe: &crate::stages::PaletteRecipe,
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
                    "batch `{}` targets area `{}` which stage 1 does not declare — use one of \
                     the world stage's area ids",
                    batch.id, batch.area
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
        for (field, anchor) in [("gate", &sc.gate), ("unlock", &sc.unlock)] {
            if !providers.resolvable(anchor.as_str()) {
                d.push(Diagnostic::error(
                    codes::SHORTCUT_INVALID,
                    "quests",
                    format!("/content/shortcuts/{i}/{field}"),
                    format!(
                        "shortcut `{field}` anchor `{anchor}` is not provided by any area's \
                         prefab — use an anchor a prefab exposes (anchor names come from prefab \
                         metadata; do NOT invent one)"
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
            if !providers.resolvable(dis.via.as_str()) {
                err(
                    format!("/content/timed_gates/{i}/disarm/via"),
                    format!(
                        "timed-gate `disarm.via` anchor `{}` is not provided by any area's prefab \
                         — use an anchor some area's prefab exposes for the jam affordance \
                         (anchor names come from prefab metadata; do NOT invent one)",
                        dis.via
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

/// The raider family: the species whose `Patrolling` / `patrol_target` NBT
/// vanilla actually honours, all live-verified marching a compiler-driven lane on
/// 1.21.11 (`docs/notes/td-routing-spike.md`). On anything else the keys are
/// inert — the mob stands where it spawned — which is the silent no-op class
/// `DW0382` exists to make loud.
const LANE_RAIDERS: [&str; 5] = ["pillager", "vindicator", "evoker", "ravager", "witch"];

/// Species whose ONLY attack goal is gated on holding a specific weapon: they
/// acquire a target, find no runnable attack goal, and freeze — while the patrol
/// goal stays blocked by the very target they cannot hit (`DW0384`). A pillager
/// is a crossbow mob and nothing else; every other raider melees or casts
/// bare-handed, so this table has exactly one row on purpose.
const LANE_WEAPON_GATED: [(&str, &str); 1] = [("pillager", "minecraft:crossbow")];

/// The bare entity id (`minecraft:pillager` → `pillager`).
fn bare_entity(id: &str) -> &str {
    id.strip_prefix("minecraft:").unwrap_or(id)
}

/// The advisory half of the difficulty surface
/// (`DW0469`): a campaign that stages a **fighting** actor but declares no
/// `waves[]` and no `world.difficulty` ships the compiler's derived
/// `difficulty=peaceful` — under which the server discards every
/// hostile-category mob as it ticks it (`/summon`ed, `NoAI` and
/// `PersistenceRequired` all irrelevant), so that fighter is gone on the tick it
/// spawns and the beat that summoned it plays to an empty room.
///
/// "Fighting" is read off the campaign's own declarations, never guessed from
/// the species: an `unleash-actor` (the author asking for a real-AI twin) or
/// `vulnerable: true` (the author declaring a damageable target). Both are
/// statements of combat intent the compiler can see. The species question — is
/// `minecraft:sheep` a monster? — is exactly what it cannot answer, because the
/// pinned entity registry is a membership set with no mob-category data, which
/// is also why this is advisory rather than an error.
///
/// Gated with the rest of the v0.6 quests surface, where actors live —
/// deliberately NOT on the world stage's version, so a campaign whose world
/// stage is older still hears about it.
fn difficulty_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    if c.world.content.difficulty.is_some() || !c.quests.content.waves.is_empty() {
        return;
    }
    let mut fighters: BTreeSet<String> = c
        .quests
        .content
        .actors
        .iter()
        .filter(|a| a.vulnerable)
        .map(|a| a.id.as_str().to_string())
        .collect();
    for q in &c.quests.content.quests {
        for_each_effect_deep(q, |_, eff| {
            if let QuestEffect::UnleashActor { actor, .. } = eff {
                fighters.insert(actor.as_str().to_string());
            }
        });
    }
    for t in &c.quests.content.triggers {
        for_each_trigger_effect_deep(t, |_, eff| {
            if let QuestEffect::UnleashActor { actor, .. } = eff {
                fighters.insert(actor.as_str().to_string());
            }
        });
    }
    for t in &c.quests.content.traps {
        for_each_trap_payload_deep(t, |_, eff| {
            if let QuestEffect::UnleashActor { actor, .. } = eff {
                fighters.insert(actor.as_str().to_string());
            }
        });
    }
    if fighters.is_empty() {
        return;
    }
    d.push(Diagnostic::warning(
        codes::DIFFICULTY_UNDECLARED_ACTORS,
        "world",
        "/content/difficulty".to_string(),
        format!(
            "this campaign stages {} actor(s) meant to FIGHT ({}) — unleashed into a real-AI twin, \
             or declared `vulnerable` — but declares no `waves[]` and no `world.difficulty`, so it \
             ships the compiler's derived `difficulty=peaceful`. On peaceful the server discards \
             every hostile-category mob as it ticks it, so a monster among these is gone on the \
             tick it spawns and the beat that summoned it plays to an empty room. Declare \
             `world.difficulty` on the world stage: `easy` reproduces the halved-damage world \
             existing combat numbers were tuned in, `normal` is the vanilla baseline. (If every \
             one of them is a passive species, there is nothing to fix.)",
            fighters.len(),
            fighters
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    ));
}

/// Validate the spec-0016 §6 wave `lane` / `summon` surface.
///
/// Five rules, five codes, each pinned to a live-verified 1.21.11 failure mode:
/// * `DW0381` — the declaration does not resolve or contradicts itself;
/// * `DW0382` — a lane species outside the raider family (the NBT is inert);
/// * `DW0383` — a lane squad below 2 (a lone patroller self-cancels);
/// * `DW0384` — a lane pillager without its crossbow (target-acquisition deadlock);
/// * `DW0385` — an aggro-edge mob with no authored `follow_range` (no ring radius).
///
/// Anchor resolution stays lenient for pool areas the compiler resolves later —
/// the same policy as the trap, trigger and shortcut checks. Waypoint *geometry*
/// (standable, reachable, spaced > 10) is a build-tier proof over the assembled
/// world (`DW0386`), not a validation-tier one.
fn lane_checks(c: &Campaign, anchors: &dyn AnchorRegistry, d: &mut Vec<Diagnostic>) {
    let quests = &c.quests.content;
    if quests
        .waves
        .iter()
        .all(|w| w.lane.is_none() && w.summon.is_none())
    {
        return;
    }
    let providers = AnchorProviders::build(c, anchors);

    for (i, w) in quests.waves.iter().enumerate() {
        let aggro_edge = w.summon == Some(crate::stages::WaveSummon::AggroEdge);
        if aggro_edge {
            if w.lane.is_some() {
                d.push(Diagnostic::error(
                    codes::LANE_INVALID,
                    "quests",
                    format!("/content/waves/{i}/summon"),
                    format!(
                        "wave `{}` declares BOTH a `lane` and `summon: aggro-edge` (spec-0016 §6) \
                         — a lane IS the routing (march while distant, native AI once aggroed), \
                         and aggro-edge is its opposite (materialize already at the edge of \
                         perception, no routing at all). Pick one.",
                        w.id
                    ),
                ));
            }
            for (k, m) in w.mobs.iter().enumerate() {
                if m.attributes.and_then(|a| a.follow_range).is_none() {
                    d.push(Diagnostic::error(
                        codes::AGGRO_EDGE_NO_RANGE,
                        "quests",
                        format!("/content/waves/{i}/mobs/{k}/attributes"),
                        format!(
                            "`summon: aggro-edge` mob `{}` in wave `{}` declares no \
                             `attributes.follow_range` (spec-0016 §6). That radius IS the summon \
                             ring — the distance at which this mob perceives the party — so it is \
                             authored, never guessed: the compiler will not fabricate a vanilla \
                             default it cannot verify against the pinned server.",
                            m.entity, w.id
                        ),
                    ));
                }
            }
        }
        let Some(lane) = &w.lane else { continue };

        if lane.waypoints.is_empty() {
            d.push(Diagnostic::error(
                codes::LANE_INVALID,
                "quests",
                format!("/content/waves/{i}/lane/waypoints"),
                format!(
                    "wave `{}` declares a `lane` with no waypoints (spec-0016 §6) — a lane is a \
                     polyline the squad marches; give it at least one waypoint anchor",
                    w.id
                ),
            ));
        }
        for (k, wp) in lane.waypoints.iter().enumerate() {
            if !providers.resolvable(wp.as_str()) {
                d.push(Diagnostic::error(
                    codes::LANE_INVALID,
                    "quests",
                    format!("/content/waves/{i}/lane/waypoints/{k}"),
                    format!(
                        "lane waypoint anchor `{wp}` is not provided by any area's prefab — use \
                         an anchor a prefab exposes (anchor names come from prefab metadata; do \
                         NOT invent one)"
                    ),
                ));
            }
            if k > 0 && lane.waypoints[k - 1] == *wp {
                d.push(Diagnostic::error(
                    codes::LANE_INVALID,
                    "quests",
                    format!("/content/waves/{i}/lane/waypoints/{k}"),
                    format!(
                        "lane waypoint `{wp}` repeats the one before it — the squad would be told \
                         to march to where it already stands, and vanilla re-rolls a patrol \
                         target on arrival. Remove the repeat."
                    ),
                ));
            }
        }
        if !(4..=64).contains(&lane.aggro_radius) {
            d.push(Diagnostic::error(
                codes::LANE_INVALID,
                "quests",
                format!("/content/waves/{i}/lane/aggro_radius"),
                format!(
                    "lane `aggro_radius` {} on wave `{}` is outside 4..=64 (spec-0016 §6). It is \
                     emitted verbatim as the mobs' `follow_range` attribute AND as the release \
                     radius; below 4 the squad walks into contact before it can see anyone, and \
                     past 64 it aggroes across the whole delve.",
                    lane.aggro_radius, w.id
                ),
            ));
        }
        if w.mobs.iter().map(|m| m.count).sum::<u32>() < 2 {
            d.push(Diagnostic::error(
                codes::LANE_SQUAD_TOO_SMALL,
                "quests",
                format!("/content/waves/{i}/mobs"),
                format!(
                    "lane wave `{}` fields fewer than 2 mobs (spec-0016 §6). A lone patroller \
                     sets `Patrolling:0b` on ITSELF when it finds no companion within its follow \
                     range — vanilla behaviour, live-verified — so a one-mob lane cancels its own \
                     routing and just stands there. Field a squad of at least 2.",
                    w.id
                ),
            ));
        }
        for (k, m) in w.mobs.iter().enumerate() {
            let bare = bare_entity(&m.entity);
            if !LANE_RAIDERS.contains(&bare) {
                d.push(Diagnostic::error(
                    codes::LANE_NOT_RAIDER,
                    "quests",
                    format!("/content/waves/{i}/mobs/{k}/entity"),
                    format!(
                        "lane wave `{}` fields `{}`, which is not raider-family (spec-0016 §6). \
                         `Patrolling`/`patrol_target` are Raider NBT: on any other species they \
                         are simply dropped and the mob stands where it spawned. Lane species: \
                         {}. For anything else use `summon: aggro-edge`, which needs no patrol AI.",
                        w.id,
                        m.entity,
                        LANE_RAIDERS.join(" / ")
                    ),
                ));
            }
            if let Some((_, weapon)) = LANE_WEAPON_GATED.iter().find(|(s, _)| *s == bare) {
                let held = m
                    .equipment
                    .as_ref()
                    .and_then(|e| e.main_hand.as_ref())
                    .map_or(*weapon, |p| p.item());
                if held != *weapon {
                    d.push(Diagnostic::error(
                        codes::LANE_UNARMED,
                        "quests",
                        format!("/content/waves/{i}/mobs/{k}/equipment/main_hand"),
                        format!(
                            "lane `{bare}` in wave `{}` holds `{held}` instead of `{weapon}` \
                             (spec-0016 §6). Its ONLY attack goal is the crossbow goal, so on \
                             acquiring a target it has nothing runnable to do — and the patrol \
                             goal is meanwhile blocked BY that target. The mob freezes in place \
                             indefinitely (live-verified deadlock). Give it the crossbow, or drop \
                             the `main_hand` override and take the compiler's default.",
                            w.id
                        ),
                    ));
                }
            }
        }
        if let Some(bad) = w.mobs.iter().enumerate().find(|(_, m)| {
            m.attributes
                .and_then(|a| a.follow_range)
                .is_some_and(|r| r != f64::from(lane.aggro_radius))
        }) {
            let (k, m) = bad;
            d.push(Diagnostic::error(
                codes::LANE_INVALID,
                "quests",
                format!("/content/waves/{i}/mobs/{k}/attributes/follow_range"),
                format!(
                    "lane mob `{}` in wave `{}` declares `follow_range` {} but the lane's \
                     `aggro_radius` is {} (spec-0016 §6). They MUST be equal: the release radius \
                     is where routing hands over to native AI, and a patrolling raider that \
                     targets a player outside its engagement range HOLDS GROUND instead of \
                     marching — the squad stalls mid-lane. Drop the override (the compiler sets \
                     `follow_range` from `aggro_radius`) or make the two agree.",
                    m.entity,
                    w.id,
                    m.attributes
                        .and_then(|a| a.follow_range)
                        .unwrap_or_default(),
                    lane.aggro_radius
                ),
            ));
        }
    }
}

/// Declared drops: what an elite or boss
/// leaves behind is a **declared subset**, never automatically everything.
///
/// Four rules, all of them about the gap between what a campaign says and what
/// the world can actually produce:
///
/// * `DW0491` — only an `elite`/`boss` encounter may declare drops. Rank-and-file
///   gear stays unfarmable by construction (no-grind constitution).
/// * `DW0490` — a `slot` entry must name a **distinct** slot the same entity's
///   own `equipment` fills. A body cannot leave behind a piece it never wore.
/// * `DW0492` — a `dropped_by` collect must be backed by the wave it names: the
///   wave really declares that item, in at least the count the objective asks
///   for, and the objective does not also adopt a container.
/// * `DW0493` — that collect must be **ordered after** the fight, so the chain
///   "kill the boss → take its key → open the door" is a proof rather than an
///   intention.
fn check_drops(
    c: &Campaign,
    quests: &crate::stages::QuestsContent,
    items: &dyn ItemRegistry,
    d: &mut Vec<Diagnostic>,
) {
    use crate::stages::{EncounterTier, MobDrop};

    // --- the declaration side: waves and actors ---------------------------
    let tiered =
        |t: Option<EncounterTier>| matches!(t, Some(EncounterTier::Elite | EncounterTier::Boss));
    for (i, w) in quests.waves.iter().enumerate() {
        for (k, m) in w.mobs.iter().enumerate() {
            if m.drops.is_empty() {
                continue;
            }
            if !tiered(w.tier) {
                d.push(Diagnostic::error(
                    codes::DROP_NOT_TIERED,
                    "quests",
                    format!("/content/waves/{i}/mobs/{k}/drops"),
                    format!(
                        "wave `{}` declares drops but is not billed `elite` or `boss` — only a \
                         named fight leaves anything behind; an ordinary mob's kit is never \
                         farmable. Declare the wave's `tier`, or remove the `drops`",
                        w.id
                    ),
                ));
            }
            check_drop_list(
                &m.drops,
                m.equipment.as_ref(),
                &format!("wave `{}` mob {k}", w.id),
                &format!("/content/waves/{i}/mobs/{k}/drops"),
                items,
                d,
            );
        }
    }
    for (i, a) in quests.actors.iter().enumerate() {
        if a.drops.is_empty() {
            continue;
        }
        if !tiered(a.tier) {
            d.push(Diagnostic::error(
                codes::DROP_NOT_TIERED,
                "quests",
                format!("/content/actors/{i}/drops"),
                format!(
                    "actor `{}` declares drops but is not billed `elite` or `boss` — only a named \
                     fight leaves anything behind; a staged puppet's kit is never farmable. \
                     Declare the actor's `tier`, or remove the `drops`",
                    a.id
                ),
            ));
        }
        check_drop_list(
            &a.drops,
            a.equipment.as_ref(),
            &format!("actor `{}`", a.id),
            &format!("/content/actors/{i}/drops"),
            items,
            d,
        );
    }

    // --- the consumption side: `collect.dropped_by` ------------------------
    // How many copies of each item every wave can yield: one per declaring mob
    // in the stack, so a pair of elites each dropping a sword yields two.
    let mut yielded: BTreeMap<&str, BTreeMap<&str, u32>> = BTreeMap::new();
    for w in &quests.waves {
        let per = yielded.entry(w.id.as_str()).or_default();
        for m in &w.mobs {
            for dr in &m.drops {
                if let MobDrop::Item(it) = dr {
                    *per.entry(it.item.as_str()).or_default() += m.count;
                }
            }
        }
    }
    let anc = quest_ancestors(c);
    // Which quests hold a `kill` objective for each wave, and which objective ids.
    let mut kills: BTreeMap<&str, Vec<(&str, &str)>> = BTreeMap::new();
    for q in &quests.quests {
        for o in &q.objectives {
            if let Objective::Kill { wave, id, .. } = o {
                kills
                    .entry(wave.as_str())
                    .or_default()
                    .push((q.id.as_str(), id.as_str()));
            }
        }
    }
    for (i, q) in quests.quests.iter().enumerate() {
        let after_anc = objective_ancestors(q);
        for (j, o) in q.objectives.iter().enumerate() {
            let Objective::Collect {
                id,
                item,
                count,
                container,
                dropped_by: Some(wave),
                ..
            } = o
            else {
                continue;
            };
            let path = format!("/content/quests/{i}/objectives/{j}/dropped_by");
            if container.is_some() {
                d.push(Diagnostic::error(
                    codes::DROP_COLLECT_UNSOURCED,
                    "quests",
                    path.clone(),
                    format!(
                        "`collect` `{id}` declares both `dropped_by` (wave `{wave}`) and a \
                         `container` — the item comes off a body or out of a box, not both; drop \
                         whichever provisioning this beat does not use"
                    ),
                ));
            }
            let Some(per) = yielded.get(wave.as_str()) else {
                // Unknown wave: the ordinary dangling-reference diagnostic
                // (`DW0170`) already names it; nothing to add here.
                continue;
            };
            match per.get(item.as_str()).copied() {
                None => d.push(Diagnostic::error(
                    codes::DROP_COLLECT_UNSOURCED,
                    "quests",
                    path.clone(),
                    format!(
                        "`collect` `{id}` takes `{item}` off wave `{wave}`, but no mob of that \
                         wave declares a `{{\"item\": \"{item}\"}}` drop — {}. Declare the drop on \
                         the wave's mob, or point `dropped_by` at the wave that really carries it",
                        if per.is_empty() {
                            "the wave declares no item drops at all".to_string()
                        } else {
                            format!(
                                "it declares {}",
                                per.keys().cloned().collect::<Vec<_>>().join(", ")
                            )
                        }
                    ),
                )),
                Some(have) if have < *count => d.push(Diagnostic::error(
                    codes::DROP_COLLECT_UNSOURCED,
                    "quests",
                    path.clone(),
                    format!(
                        "`collect` `{id}` asks for {count} × `{item}`, but wave `{wave}` yields \
                         only {have} — a body drops its declared item once. Lower the `count`, or \
                         raise the declaring mob's `count`"
                    ),
                )),
                Some(_) => {}
            }
            // The ordering proof: some `kill` objective for this wave must
            // strictly precede this collect — in the same quest through the
            // `after` graph, or in a quest this one transitively depends on.
            let ordered = kills.get(wave.as_str()).is_some_and(|ks| {
                ks.iter().any(|(kq, ko)| {
                    if *kq == q.id.as_str() {
                        after_anc
                            .get(id.as_str())
                            .is_some_and(|set| set.contains(ko))
                    } else {
                        anc.get(q.id.as_str()).is_some_and(|set| set.contains(kq))
                    }
                })
            });
            if !ordered {
                d.push(Diagnostic::error(
                    codes::DROP_COLLECT_UNORDERED,
                    "quests",
                    path,
                    format!(
                        "`collect` `{id}` takes `{item}` off wave `{wave}`, but no `kill` \
                         objective for `{wave}` is proven to run first — the item would be \
                         unreachable while the objective reads as active from the campaign's \
                         first tick. Add a `kill` objective for `{wave}` and list it in this \
                         objective's `after`, or put the kill in a quest this one `depends_on`"
                    ),
                ));
            }
        }
    }
}

/// One entity's `drops[]` list: distinct, really-worn slots (`DW0490`) and
/// registry-valid quest items (`DW0143`). Shared by wave mobs and actors so the
/// two surfaces cannot drift.
fn check_drop_list(
    drops: &[crate::stages::MobDrop],
    equipment: Option<&crate::stages::MobEquipment>,
    what: &str,
    base_path: &str,
    items: &dyn ItemRegistry,
    d: &mut Vec<Diagnostic>,
) {
    use crate::stages::MobDrop;

    let mut seen_slots: BTreeSet<&'static str> = BTreeSet::new();
    for (n, dr) in drops.iter().enumerate() {
        match dr {
            MobDrop::Slot(s) => {
                let field = s.slot.field();
                let filled: Vec<&str> = equipment
                    .map(|eq| {
                        eq.slots()
                            .into_iter()
                            .filter(|(_, p)| p.is_some())
                            .map(|(name, _)| name)
                            .collect()
                    })
                    .unwrap_or_default();
                if equipment.is_none_or(|eq| eq.filled(s.slot).is_none()) {
                    d.push(Diagnostic::error(
                        codes::DROP_SLOT_UNFILLED,
                        "quests",
                        format!("{base_path}/{n}/slot"),
                        format!(
                            "{what} declares a `{field}` drop, but its `equipment` puts nothing \
                             in `{field}` — {}. A body can only leave behind a piece it wears: \
                             equip the slot, or drop a slot it fills",
                            if filled.is_empty() {
                                "it declares no equipment at all".to_string()
                            } else {
                                format!("it fills {}", filled.join(", "))
                            }
                        ),
                    ));
                } else if !seen_slots.insert(field) {
                    d.push(Diagnostic::error(
                        codes::DROP_SLOT_UNFILLED,
                        "quests",
                        format!("{base_path}/{n}/slot"),
                        format!(
                            "{what} declares the `{field}` drop twice — a body leaves each piece \
                             behind once; remove the duplicate entry"
                        ),
                    ));
                }
            }
            MobDrop::Item(it) => {
                if !items.contains(&it.item) {
                    d.push(Diagnostic::error(
                        codes::ITEM_UNKNOWN,
                        "quests",
                        format!("{base_path}/{n}/item"),
                        format!(
                            "{what} declares a drop of `{}`, which is not in the pinned 1.21.11 \
                             item registry — use a valid namespaced item id (e.g. \
                             `minecraft:tripwire_hook`)",
                            it.item
                        ),
                    ));
                }
            }
        }
    }
}

/// Per-objective transitive `after` ancestors within one quest: `obj -> {every
/// objective that must complete before it}`. Acyclicity is guaranteed by
/// `DW0140`; a cyclic quest simply yields a partial set and the cycle's own
/// diagnostic fires.
fn objective_ancestors(q: &crate::stages::Quest) -> BTreeMap<&str, BTreeSet<&str>> {
    let direct: BTreeMap<&str, Vec<&str>> = q
        .objectives
        .iter()
        .map(|o| {
            (
                o.id().as_str(),
                o.after().iter().map(|a| a.as_str()).collect::<Vec<_>>(),
            )
        })
        .collect();
    let mut out = BTreeMap::new();
    for o in &q.objectives {
        let mut anc: BTreeSet<&str> = BTreeSet::new();
        let mut stack = vec![o.id().as_str()];
        while let Some(cur) = stack.pop() {
            if let Some(ds) = direct.get(cur) {
                for dep in ds {
                    if anc.insert(dep) {
                        stack.push(dep);
                    }
                }
            }
        }
        out.insert(o.id().as_str(), anc);
    }
    out
}

/// Validate one [`MobEquipment`] block — item ids against the pinned registry
/// (`DW0143`) and every piece's enchantments against the pinned enchantment
/// registry (`DW0433`) and level range (`DW0434`).
///
/// Shared verbatim by wave mobs and actors so the two surfaces cannot drift:
/// they are the same schema type and therefore must be the same rules.
fn check_equipment(
    eq: &crate::stages::MobEquipment,
    what: &str,
    base_path: &str,
    items: &dyn ItemRegistry,
    d: &mut Vec<Diagnostic>,
) {
    let ench_reg = crate::registry::VendoredEnchantmentRegistry::v1_21_11();
    for (slot, piece) in eq.slots() {
        let Some(piece) = piece else { continue };
        let it = piece.item();
        if !items.contains(it) {
            d.push(Diagnostic::error(
                codes::ITEM_UNKNOWN,
                "quests",
                format!("{base_path}/{slot}"),
                format!(
                    "{what} equipment `{slot}` item `{it}` is not in the pinned 1.21.11 \
                     item registry — use a valid namespaced item id (e.g. \
                     `minecraft:iron_helmet`)"
                ),
            ));
        }
        check_enchantments(
            piece.enchantments(),
            &format!("{what} equipment `{slot}`"),
            &format!("{base_path}/{slot}/enchantments"),
            &ench_reg,
            d,
        );
    }
}

/// Validate an enchantment map: known ids (`DW0433`), legal levels (`DW0434`).
///
/// Levels are checked against what the `minecraft:enchantments` **component**
/// can carry (1..=255), not against each enchantment's survival max. Exceeding
/// the survival max from a command is legal vanilla and is a legitimate way to
/// build a set-piece elite, so refusing it would be the compiler overruling a
/// design decision it cannot second-guess; 0 and >255 are simply not
/// representable and would be silently dropped by the game.
fn check_enchantments(
    ench: &std::collections::BTreeMap<String, u32>,
    what: &str,
    path: &str,
    reg: &dyn crate::registry::EnchantmentRegistry,
    d: &mut Vec<Diagnostic>,
) {
    for (id, level) in ench {
        if !reg.contains(id) {
            d.push(Diagnostic::error(
                codes::ENCHANTMENT_UNKNOWN,
                "quests",
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
                "quests",
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
/// (`crates/compiler/data/item-stack-sizes-1.21.11.json`), never a hand table.
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
const MIN_CONTAINER_SLOTS: usize = 27;

fn check_stack_count(
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
        if !providers.resolvable(l.anchor.as_str()) {
            d.push(Diagnostic::error(
                codes::ANCHOR_UNRESOLVED,
                "quests",
                format!("/content/loot/{i}/anchor"),
                format!(
                    "loot anchor `{}` is not provided by any prefab bound in this campaign — \
                     use an anchor the prefab exposes (anchor names come from prefab metadata; \
                     do NOT invent one)",
                    l.anchor
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

/// DSL v0.8 reserved-feature gating: spec-0025's stage-4 `branch_points`
/// declaration, per-node `happening` and `campaign-complete` `ending`, plus
/// spec-0016 §1's bonfire rest-dialog labels (stage 5), the class-kit `flask`
/// (stage 3), spec-0023's actor `tier` (stage 5) and the `collect`
/// container adoption (`container` / `item_name` / `fill_count`, stage 5).
///
/// Same asymmetry the v0.7 ledger established: *declaring* any of it below 0.8.0
/// is `DW0141`, so the version contract stays exact and a 0.6/0.7 campaign's
/// datapack cannot move by a byte. What fires only **at** 0.8.0 is the
/// requirement side — `DW0481` (a story node with no `happening`) and `DW0480`
/// (a fork nobody declared).
fn reserved_v08(c: &Campaign, d: &mut Vec<Diagnostic>) {
    if !is_v08(c.quest_plan.dsl_version.as_str()) && !c.quest_plan.content.branch_points.is_empty()
    {
        d.push(Diagnostic::error(
            codes::RESERVED,
            "quest-plan",
            "/content/branch_points".to_string(),
            "the stage-4 `branch_points` declaration (which flags fork the story, where the fork \
             opens, and what each branch runs to) requires dsl_version 0.8.0 — raise this stage's \
             `dsl_version` to 0.8.0, or remove the section"
                .to_string(),
        ));
    }
    if !is_v08(c.quests.dsl_version.as_str()) {
        // spec-0023: an actor's `tier` — the same `elite`/`boss`
        // billing a wave declares, on the OTHER shape an elite takes.
        for (i, a) in c.quests.content.actors.iter().enumerate() {
            if a.tier.is_none() {
                continue;
            }
            d.push(Diagnostic::error(
                codes::RESERVED,
                "quests",
                format!("/content/actors/{i}/tier"),
                "an actor `tier` (`elite`/`boss` — what the validation ladder's floor gate holds \
                 this fight to) requires dsl_version 0.8.0 — raise this stage's `dsl_version` to \
                 0.8.0, or remove the field"
                    .to_string(),
            ));
        }
        for (i, q) in c.quests.content.quests.iter().enumerate() {
            if q.happening.is_some() {
                d.push(reserved_happening(
                    "quests",
                    format!("/content/quests/{i}/happening"),
                ));
            }
            for (j, o) in q.objectives.iter().enumerate() {
                if o.happening().is_some() {
                    d.push(reserved_happening(
                        "quests",
                        format!("/content/quests/{i}/objectives/{j}/happening"),
                    ));
                }
                // The `collect` container-adoption trio. Declaring any of it below
                // 0.8.0 is `DW0141`, so a 0.6/0.7 campaign's chest, its unnamed
                // item and its single stack cannot move by a byte.
                if let Objective::Collect {
                    container,
                    item_name,
                    fill_count,
                    ..
                } = o
                {
                    for (present, field, what) in [
                        (
                            container.is_some(),
                            "container",
                            "a `collect` `container` (the prefab-placed chest or barrel the \
                             objective adopts instead of conjuring its own chest)",
                        ),
                        (
                            item_name.is_some(),
                            "item_name",
                            "a `collect` `item_name` (the display name the collected item \
                             carries as a `custom_name` component)",
                        ),
                        (
                            *fill_count != 0,
                            "fill_count",
                            "a `collect` `fill_count` (the padding stacks that make the \
                             container read full)",
                        ),
                    ] {
                        if !present {
                            continue;
                        }
                        d.push(Diagnostic::error(
                            codes::RESERVED,
                            "quests",
                            format!("/content/quests/{i}/objectives/{j}/{field}"),
                            format!(
                                "{what} requires dsl_version 0.8.0 — raise this stage's \
                                 `dsl_version` to 0.8.0, or remove the field"
                            ),
                        ));
                    }
                }
            }
        }
        crate::stages::for_each_campaign_effect(c, &mut |path, _site, eff| {
            if effect_happening(eff).is_some() {
                d.push(reserved_happening("quests", format!("{path}/happening")));
            }
            if let QuestEffect::CampaignComplete {
                ending: Some(_), ..
            } = eff
            {
                d.push(Diagnostic::error(
                    codes::RESERVED,
                    "quests",
                    format!("{path}/ending"),
                    "a named `campaign-complete` `ending` requires dsl_version 0.8.0 — raise this \
                     stage's `dsl_version` to 0.8.0, or remove the field"
                        .to_string(),
                ));
            }
        });
    }
    // spec-0016 §1: the class-kit `flask` a bonfire
    // rest replenishes, and the bonfire's authorable rest-dialog labels.
    if !is_v08(c.classes.dsl_version.as_str()) {
        for (i, cl) in c.classes.content.classes.iter().enumerate() {
            for (k, item) in cl.kit.iter().enumerate() {
                if item.flask {
                    d.push(Diagnostic::error(
                        codes::RESERVED,
                        "classes",
                        format!("/content/classes/{i}/kit/{k}/flask"),
                        "a class kit `flask` (the recovery item a bonfire rest replenishes) \
                         requires dsl_version 0.8.0 — raise this stage's `dsl_version` to 0.8.0, \
                         or remove the field"
                            .to_string(),
                    ));
                }
                // What the bottle actually pours.
                if item.contents.is_some() {
                    d.push(Diagnostic::error(
                        codes::RESERVED,
                        "classes",
                        format!("/content/classes/{i}/kit/{k}/contents"),
                        "kit item potion `contents` (the vanilla `minecraft:potion_contents` \
                         component — a named potion or a custom-effect list) requires \
                         dsl_version 0.8.0 — raise this stage's `dsl_version` to 0.8.0, or \
                         remove the field"
                            .to_string(),
                    ));
                }
            }
        }
    }
    if !is_v08(c.quests.dsl_version.as_str()) {
        // The line a sealed gate answers a
        // right-click with. A pre-0.8 campaign still gets the answer — the
        // compiler's canonical English — it just cannot author the wording.
        crate::stages::for_each_campaign_effect(c, &mut |path, _site, eff| {
            if eff.close_gate_sealed_hint().is_some() {
                d.push(Diagnostic::error(
                    codes::RESERVED,
                    "quests",
                    format!("{path}/sealed_hint"),
                    "a `close-gate` `sealed_hint` (the line the sealed gate answers a right-click \
                     with) requires dsl_version 0.8.0 — raise this stage's `dsl_version` to 0.8.0, \
                     or remove the field and take the compiler's canonical English"
                        .to_string(),
                ));
            }
        });
        crate::stages::for_each_campaign_effect(c, &mut |path, _site, eff| {
            let Some(l) = eff.bonfire_labels() else {
                return;
            };
            for (present, field) in [
                (l.prompt.is_some(), "prompt"),
                (l.rest_label.is_some(), "rest_label"),
                (l.save_label.is_some(), "save_label"),
            ] {
                if !present {
                    continue;
                }
                d.push(Diagnostic::error(
                    codes::RESERVED,
                    "quests",
                    format!("{path}/{field}"),
                    format!(
                        "a `bonfire` `{field}` (an authored label on the two-option rest dialog) \
                         requires dsl_version 0.8.0 — raise this stage's `dsl_version` to 0.8.0, \
                         or remove the field and take the compiler's canonical English"
                    ),
                ));
            }
        });
    }
    if !is_v08(c.dialogue.dsl_version.as_str()) {
        for (i, t) in c.dialogue.content.dialogues.iter().enumerate() {
            for (j, n) in t.nodes.iter().enumerate() {
                for (k, o) in n.options.iter().enumerate() {
                    if o.happening.is_some() {
                        d.push(reserved_happening(
                            "dialogue",
                            format!("/content/dialogues/{i}/nodes/{j}/options/{k}/happening"),
                        ));
                    }
                    // The button's hover tooltip — the full line the caption
                    // stands for.
                    if o.tooltip.is_some() {
                        d.push(Diagnostic::error(
                            codes::RESERVED,
                            "dialogue",
                            format!("/content/dialogues/{i}/nodes/{j}/options/{k}/tooltip"),
                            "a dialogue option `tooltip` (the full line vanilla shows in a hover \
                             box while the button keeps the caption) requires dsl_version 0.8.0 \
                             — raise this stage's `dsl_version` to 0.8.0, or remove the field"
                                .to_string(),
                        ));
                    }
                }
            }
        }
    }
}

fn reserved_happening(stage: &str, path: String) -> Diagnostic {
    Diagnostic::error(
        codes::RESERVED,
        stage,
        path,
        "a `happening` declaration (what this node does to the story, as a structured event verb \
         plus one line of prose) requires dsl_version 0.8.0 — raise this stage's `dsl_version` to \
         0.8.0, or remove the field"
            .to_string(),
    )
}

/// The `happening` of an effect, for the eleven story-node verbs that carry one.
pub(crate) fn effect_happening(eff: &QuestEffect) -> Option<&crate::stages::Happening> {
    match eff {
        QuestEffect::OpenGate { happening, .. }
        | QuestEffect::CloseGate { happening, .. }
        | QuestEffect::CampaignComplete { happening, .. }
        | QuestEffect::SpawnWave { happening, .. }
        | QuestEffect::DespawnNpc { happening, .. }
        | QuestEffect::MoveNpc { happening, .. }
        | QuestEffect::SpawnNpc { happening, .. }
        | QuestEffect::SpawnActor { happening, .. }
        | QuestEffect::DespawnActor { happening, .. }
        | QuestEffect::MoveActor { happening, .. }
        | QuestEffect::UnleashActor { happening, .. } => happening.as_ref(),
        _ => None,
    }
}

/// Structural validation of the stage-4 `branch_points` declaration (spec-0025).
///
/// Everything here reuses the DSL's existing structural codes on purpose — a
/// branch point is an ordinary declaration with ordinary ids, so a malformed id
/// is `DW0110`, a repeated one `DW0111`, and a reference to something that does
/// not exist `DW0112`. The `DW048x` block is reserved for what is genuinely new:
/// proofs *about* branches.
fn branch_point_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let quests: BTreeSet<&str> = c
        .quest_plan
        .content
        .quests
        .iter()
        .map(|q| q.id.as_str())
        .collect();
    let endings: BTreeSet<String> = declared_endings(c);
    let flags: BTreeSet<String> = produced_flags(c);
    let mut seen_points: BTreeSet<&str> = BTreeSet::new();
    let mut seen_branches: BTreeSet<&str> = BTreeSet::new();

    for (i, bp) in c.quest_plan.content.branch_points.iter().enumerate() {
        let base = format!("/content/branch_points/{i}");
        if !bp.id.is_valid_syntax() {
            d.push(Diagnostic::error(
                codes::ID_SYNTAX,
                "quest-plan",
                format!("{base}/id"),
                format!(
                    "`{}` is not a valid branch-point id — use `branch-point/<kebab-case>`",
                    bp.id.as_str()
                ),
            ));
        } else if !seen_points.insert(bp.id.as_str()) {
            d.push(Diagnostic::error(
                codes::ID_DUPLICATE,
                "quest-plan",
                format!("{base}/id"),
                format!("duplicate branch-point id `{}`", bp.id.as_str()),
            ));
        }
        if !quests.contains(bp.opens_at.as_str()) {
            d.push(Diagnostic::error(
                codes::DANGLING_REF,
                "quest-plan",
                format!("{base}/opens_at"),
                format!(
                    "branch point `{}` opens at `{}`, which is not a planned quest — name the \
                     quest at which the story actually forks",
                    bp.id.as_str(),
                    bp.opens_at.as_str()
                ),
            ));
        }
        for (j, f) in bp.forks_on.iter().enumerate() {
            if !flags.contains(f.as_str()) {
                d.push(Diagnostic::error(
                    codes::FLAG_UNKNOWN,
                    "quest-plan",
                    format!("{base}/forks_on/{j}"),
                    format!(
                        "branch point `{}` forks on `{}`, which no `set-flag` effect produces — a \
                         fork nothing can set is not a fork",
                        bp.id.as_str(),
                        f.as_str()
                    ),
                ));
            }
        }
        let fork_set: BTreeSet<&str> = bp.forks_on.iter().map(|f| f.as_str()).collect();
        for (j, b) in bp.branches.iter().enumerate() {
            let bpath = format!("{base}/branches/{j}");
            if !b.id.is_valid_syntax() {
                d.push(Diagnostic::error(
                    codes::ID_SYNTAX,
                    "quest-plan",
                    format!("{bpath}/id"),
                    format!(
                        "`{}` is not a valid branch id — use `branch/<kebab-case>`",
                        b.id.as_str()
                    ),
                ));
            } else if !seen_branches.insert(b.id.as_str()) {
                d.push(Diagnostic::error(
                    codes::ID_DUPLICATE,
                    "quest-plan",
                    format!("{bpath}/id"),
                    format!(
                        "duplicate branch id `{}` — branch ids are campaign-wide unique because \
                         each one names an emitted `validation/branch-chronicle-<id>.md`",
                        b.id.as_str()
                    ),
                ));
            }
            for (k, f) in b.flags.iter().enumerate() {
                if !fork_set.contains(f.as_str()) {
                    d.push(Diagnostic::error(
                        codes::DANGLING_REF,
                        "quest-plan",
                        format!("{bpath}/flags/{k}"),
                        format!(
                            "branch `{}` holds `{}`, which its branch point does not list in \
                             `forks_on` — a branch may only pin flags its own fork owns",
                            b.id.as_str(),
                            f.as_str()
                        ),
                    ));
                }
            }
            match (b.converges_at(), b.ending()) {
                (Some(q), _) => {
                    if !quests.contains(q.as_str()) {
                        d.push(Diagnostic::error(
                            codes::DANGLING_REF,
                            "quest-plan",
                            format!("{bpath}/leads_to"),
                            format!(
                                "branch `{}` converges at `{}`, which is not a planned quest",
                                b.id.as_str(),
                                q.as_str()
                            ),
                        ));
                    }
                }
                (None, Some(e)) => {
                    if !endings.contains(e.as_str()) {
                        d.push(Diagnostic::error(
                            codes::DANGLING_REF,
                            "quest-plan",
                            format!("{bpath}/leads_to"),
                            format!(
                                "branch `{}` runs to `{}`, which no `campaign-complete` effect \
                                 declares — name the ending on the `campaign-complete` that ends \
                                 this branch",
                                b.id.as_str(),
                                e.as_str()
                            ),
                        ));
                    }
                }
                (None, None) => d.push(Diagnostic::error(
                    codes::ID_SYNTAX,
                    "quest-plan",
                    format!("{bpath}/leads_to"),
                    format!(
                        "`{}` is neither a `quest/<kebab>` (the branches converge there) nor an \
                         `ending/<kebab>` (this branch runs to it) — the prefix is what says which \
                         one a branch leads to",
                        b.leads_to
                    ),
                )),
            }
        }
    }
}

/// Dangling-subject check for every `happening` (spec-0025). A subject naming an
/// `npc/`, `actor/`, `wave/` or `anchor/` id must resolve; an `item/<kebab>`
/// label is a free namespace for a story token the campaign tracks by hand, and
/// anything else is a malformed id.
fn happening_subject_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let npcs: BTreeSet<&str> = c.npcs.content.npcs.iter().map(|n| n.id.as_str()).collect();
    let actors: BTreeSet<&str> = c
        .quests
        .content
        .actors
        .iter()
        .map(|a| a.id.as_str())
        .collect();
    let waves: BTreeSet<&str> = c
        .quests
        .content
        .waves
        .iter()
        .map(|w| w.id.as_str())
        .collect();
    let check = |subject: &str, stage: &str, path: String, d: &mut Vec<Diagnostic>| {
        let known = match subject.split_once('/') {
            Some(("npc", _)) => npcs.contains(subject),
            Some(("actor", _)) => actors.contains(subject),
            Some(("wave", _)) => waves.contains(subject),
            // Anchors resolve against prefab metadata far downstream (pool areas
            // are drawn at build time), so the DSL only polices the namespace.
            Some(("anchor", _)) | Some(("item", _)) => true,
            _ => false,
        };
        if !known {
            d.push(Diagnostic::error(
                codes::DANGLING_REF,
                stage,
                path,
                format!(
                    "`happening.subject` names `{subject}`, which is not a declared `npc/`, \
                     `actor/` or `wave/` id (`anchor/` and `item/` labels are also accepted). A \
                     subject the compiler cannot resolve cannot be reasoned about, so the \
                     contradiction proof would silently skip this beat"
                ),
            ));
        }
    };
    for (i, q) in c.quests.content.quests.iter().enumerate() {
        if let Some(h) = &q.happening
            && let Some(s) = &h.subject
        {
            check(
                s,
                "quests",
                format!("/content/quests/{i}/happening/subject"),
                d,
            );
        }
        for (j, o) in q.objectives.iter().enumerate() {
            if let Some(h) = o.happening()
                && let Some(s) = &h.subject
            {
                check(
                    s,
                    "quests",
                    format!("/content/quests/{i}/objectives/{j}/happening/subject"),
                    d,
                );
            }
        }
    }
    let mut effect_subjects: Vec<(String, String)> = Vec::new();
    crate::stages::for_each_campaign_effect(c, &mut |path, _site, eff| {
        if let Some(h) = effect_happening(eff)
            && let Some(s) = &h.subject
        {
            effect_subjects.push((format!("{path}/happening/subject"), s.clone()));
        }
    });
    for (path, s) in effect_subjects {
        check(&s, "quests", path, d);
    }
    for (i, t) in c.dialogue.content.dialogues.iter().enumerate() {
        for (j, n) in t.nodes.iter().enumerate() {
            for (k, o) in n.options.iter().enumerate() {
                if let Some(h) = &o.happening
                    && let Some(s) = &h.subject
                {
                    check(
                        s,
                        "dialogue",
                        format!("/content/dialogues/{i}/nodes/{j}/options/{k}/happening/subject"),
                        d,
                    );
                }
            }
        }
    }
}

/// Every ending id some `campaign-complete` declares. There is no separate
/// declaration list — the same rule flags follow.
pub fn declared_endings(c: &Campaign) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    crate::stages::for_each_campaign_effect(c, &mut |_p, _site, eff| {
        if let QuestEffect::CampaignComplete {
            ending: Some(e), ..
        } = eff
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
/// walk is [`crate::stages::for_each_campaign_effect`], which inherits both axes
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
    crate::stages::for_each_campaign_effect(c, &mut |_p, _site, eff| {
        if let QuestEffect::SetFlag { flag, .. } = eff {
            out.insert(flag.as_str().to_string());
        }
    });
    for t in &c.dialogue.content.dialogues {
        for n in &t.nodes {
            for o in &n.options {
                for e in &o.effects {
                    if let crate::stages::DialogueEffect::SetFlag { flag } = e {
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
