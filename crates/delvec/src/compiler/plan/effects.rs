//! Effect roots: every place an effect bundle hangs, the area it belongs to, and
//! the walks over them.

use super::*;

/// Which of the five effect roots a visited effect hangs off — the part of a
/// site that decides **when** the firing happens, **whether the player is
/// forced to cause it**, and **what gates the firing as a whole**, which is all
/// the completability model needs on top of the effect itself.
///
/// The three roots that have an owner carry it: a consumer that must gate or
/// date a firing reads the owner off the site instead of re-deriving it from a
/// second, drift-prone walk (that is the whole point of the enumeration).
#[derive(Clone, Copy)]
pub(crate) enum EffectRoot<'a> {
    /// A quest's `on_objective_complete[<objective>]` — fires at that objective's
    /// `critical_path` step. Forced when the owning quest is mandatory:
    /// completing the objective is then the mainline.
    ///
    /// **Carries the owning quest id, and that is load-bearing** (spec-0051
    /// §8.6). The DSL side always carried it
    /// (`EffectRootOwner::ObjectiveComplete { quest, objective }`) and this
    /// adapter used to drop it, so the only reading available here was "an
    /// objective completed, therefore forced". Once a quest may be optional
    /// that reading is unsound in the direction that ships: it credits an
    /// `open-gate` and lays footing from a bundle the party may never fire.
    ObjectiveComplete {
        /// The quest whose `on_objective_complete` map this bundle sits in.
        quest: &'a str,
        /// The objective whose completion fires it.
        objective: &'a str,
    },
    /// A quest's `on_complete` — fires at the quest's completion step. Forced
    /// when the owning quest is mandatory.
    QuestComplete(&'a Quest),
    /// An environment `triggers[].effects` — proximity/interaction-fired, so it has
    /// no step of its own; conservatively rooted at step 0. Carries the trigger,
    /// whose `requires_flags` gate the whole bundle.
    Trigger(&'a EnvTrigger),
    /// A `traps[].payload` (spec-0022) — proximity/interaction-fired exactly like a
    /// trigger, and **optional**: the party may never trip it. Carries the trap,
    /// whose `requires_flags` gate the whole payload.
    TrapPayload(&'a Trap),
    /// A dialogue option's `set-checkpoint` `on_respawn` bundle — re-run on death
    /// while that checkpoint is active, so it is optional too (nobody is forced to
    /// die).
    DialogueRespawn,
    /// A `shortcuts[].on_unlock` (spec-0016 §2) — fired by the far-side
    /// interaction, so it has no step of its own, and **optional**: `Plan::build`
    /// registers every shortcut gate as sealed at step 0 so the delve is proven
    /// completable with no shortcut ever taken, which is exactly the statement
    /// "the party may never fire this bundle".
    ///
    /// Carries nothing, unlike its trigger/trap siblings, because a shortcut
    /// declares no flag gate — there is no `requires_flags` for a consumer to read
    /// off it. The owning object is still available on the DSL side
    /// (`EffectRootOwner::ShortcutUnlock`) for a consumer that needs to name it,
    /// and the site's `path` already does.
    ShortcutUnlock,
    /// The campaign's `on_death` (spec-0031) — fired at the moment a player dies,
    /// so it has no step and is optional in the strongest sense the model has:
    /// nobody is forced to die.
    OnDeath,
    /// A `shops[].offers[].effects` (spec-0032) — fired by a player pressing a
    /// button, so it has no step and is optional: nobody is forced to buy
    /// anything. Carries nothing for the same reason `ShortcutUnlock` does — the
    /// gate that decides whether the button exists is the offer's own, and the
    /// site's `path` already names it.
    ShopOffer,
    /// A wave's or an actor's `on_kill` (spec-0074) — fired by a player being
    /// credited with one of the fight's bodies, so it has no step and is
    /// optional: nobody is forced to be credited with a kill. Carries the fight.
    OnKill(delvewright_dsl::Fight<'a>),
    /// An assembly strike step's `on_land` (spec-0082) — fired by the strike
    /// machine while a player stands in the arming region, so it has no step
    /// and is optional: nobody is forced to stand where a blow lands.
    AssemblyLand(&'a delvewright_dsl::Assembly),
    /// A loop's `on_cross` (spec-0086) — fired by a body crossing the holding
    /// slab. Unforced from step 0 on its own (a mob crossing fires it too); the
    /// crossings the path's exercise step performs are credited as forced at
    /// that step by `Plan::build`, effect by effect, from the loop replay.
    LoopCross(&'a delvewright_dsl::Loop),
}

/// **The area an [`EffectRoot`]'s bundle plays in, when it has one.**
///
/// A quest's own area for [`EffectRoot::ObjectiveComplete`] and
/// [`EffectRoot::QuestComplete`] — the two roots [`EffectRoot`]'s own doc names
/// as carrying an owner; `None` for every other root (a trigger, a trap
/// payload, a dialogue respawn, a shortcut unlock, `on_death`, a shop offer),
/// which are global by construction and have no area to be scoped to.
///
/// This is the SAME area [`crate::compiler::continuity::replay`]'s `here_area` binds a
/// `move-npc`'s [`crate::compiler::continuity::Staged::area`] to — read once, from the
/// quest plan, so a `move-npc`'s own destination and the continuity model's
/// idea of where that move puts the body cannot disagree about which building
/// "here" names. `nav::plan_moves` asks it to give [`body_station`]'s
/// [`BodyScope::Beat`] the same `beat` the cast ledger's per-beat station
/// already asks for.
pub(crate) fn effect_root_area<'a>(
    campaign: &'a Campaign,
    root: &EffectRoot<'a>,
) -> Option<&'a str> {
    match root {
        EffectRoot::ObjectiveComplete { quest, .. } => quest_area_of(campaign, quest),
        EffectRoot::QuestComplete(quest) => quest_area_of(campaign, quest.id.as_str()),
        _ => None,
    }
}

/// Where an effect was declared: which stage document, the JSON pointer inside it,
/// and which root it hangs off. Carried so a diagnostic can name the exact firing
/// site and so a consumer can reason about *when* the firing happens.
pub(crate) struct GateSite<'a> {
    /// The stage document the effect lives in (`quests` or `dialogue`).
    pub stage: &'static str,
    /// JSON pointer to the effect within that document.
    pub path: String,
    /// The effect root this firing hangs off.
    pub root: EffectRoot<'a>,
}

/// Where a top-level effect **list** was declared: which stage document, the JSON
/// pointer to the list itself, and which root it is.
pub(crate) struct EffectRootSite<'a> {
    /// The stage document the list lives in (`quests` or `dialogue`).
    pub stage: &'static str,
    /// JSON pointer to the **list** within that document (an element's pointer is
    /// this plus `/<index>`).
    pub path: String,
    /// The list's key prefix — the same stable, readable, position-derived
    /// identifier the l10n inventory keys off, so anything that has to NAME a
    /// root (a generated function, a translation key) uses one name for it.
    pub key: String,
    /// Which root this list is.
    pub root: EffectRoot<'a>,
}

/// Visit **every top-level effect list the compiler can lower**, in one fixed
/// deterministic order.
///
/// A thin adapter over [`delvewright_dsl::for_each_effect_root`], which is the
/// single enumeration of effect roots in the workspace. It exists to re-present
/// the DSL's [`delvewright_dsl::EffectRootOwner`] as this crate's [`EffectRoot`],
/// which carries the same owners plus the completability model's reading of them
/// (see [`collect_region_events`]); it enumerates nothing itself.
///
/// A list is a root if `emit::emit_quest_effect` can reach it, not if the quests
/// stage happens to own it. Five lists are. Their order, and the reasoning, live
/// with the enumeration in `delvewright_dsl::effects`.
///
/// Consumers: [`for_each_gate_effect`] (→ the seal planner, `gates::check_seal_hints`
/// and the completability model), [`crate::compiler::timeline::walk_campaign`] (→ the
/// `DW0410` staged-walk model and, defined as it, `nav::all_effects`),
/// `emit::all_campaign_effects` (→ the generated functions themselves),
/// `emit::check_effect_anchors` (→ `DW0360`), `emit::declared_flags` (→ the
/// `dw.f_<flag>` scoreboard objectives), `rehearsal::bundles` (→ the
/// `dw:rehearsal` inventory) and both halves of [`crate::compiler::flow`].
pub(crate) fn for_each_effect_root<'a>(
    campaign: &'a Campaign,
    f: &mut dyn FnMut(&EffectRootSite<'a>, &'a [QuestEffect]),
) -> delvewright_dsl::RootBinding {
    delvewright_dsl::for_each_effect_root(campaign, &mut |site, list| {
        let root = match site.owner {
            delvewright_dsl::EffectRootOwner::ObjectiveComplete { quest, objective } => {
                EffectRoot::ObjectiveComplete {
                    quest: quest.id.as_str(),
                    objective,
                }
            }
            delvewright_dsl::EffectRootOwner::QuestComplete { quest } => {
                EffectRoot::QuestComplete(quest)
            }
            delvewright_dsl::EffectRootOwner::Trigger(t) => EffectRoot::Trigger(t),
            delvewright_dsl::EffectRootOwner::TrapPayload(t) => EffectRoot::TrapPayload(t),
            delvewright_dsl::EffectRootOwner::DialogueRespawn => EffectRoot::DialogueRespawn,
            delvewright_dsl::EffectRootOwner::ShortcutUnlock(_) => EffectRoot::ShortcutUnlock,
            delvewright_dsl::EffectRootOwner::OnDeath => EffectRoot::OnDeath,
            delvewright_dsl::EffectRootOwner::ShopOffer(_) => EffectRoot::ShopOffer,
            delvewright_dsl::EffectRootOwner::OnKill(f) => EffectRoot::OnKill(f),
            delvewright_dsl::EffectRootOwner::AssemblyLand(m) => EffectRoot::AssemblyLand(m),
            delvewright_dsl::EffectRootOwner::LoopCross(l) => EffectRoot::LoopCross(l),
        };
        f(
            &EffectRootSite {
                stage: site.stage,
                path: site.path.clone(),
                key: site.key.clone(),
                root,
            },
            list,
        );
    })
}

/// Visit **every effect the compiler can lower to a gate command**, at every
/// nesting depth: [`for_each_effect_root`] flattened, each root's list walked in
/// declaration order and each effect yielded ahead of its own nested lists.
///
/// Every consumer that reasons about emitted gate commands walks THIS: the seal
/// planner ([`collect_seal_hints`]), the wording check (`gates::check_seal_hints`,
/// `DW0423`) and the completability model ([`collect_region_events`], which feeds
/// `DW0311`/`DW0315`/`DW0342`/`DW0410`). Sharing the traversal is what makes the
/// checks and the emission unable to disagree about which firings exist.
pub(crate) fn for_each_gate_effect<'a>(
    campaign: &'a Campaign,
    f: &mut dyn FnMut(&GateSite<'a>, &'a QuestEffect),
) {
    fn deep<'a>(
        eff: &'a QuestEffect,
        stage: &'static str,
        path: &str,
        root: EffectRoot<'a>,
        f: &mut dyn FnMut(&GateSite<'a>, &'a QuestEffect),
    ) {
        f(
            &GateSite {
                stage,
                path: path.to_string(),
                root,
            },
            eff,
        );
        for (pseg, _kseg, list) in eff.nested_effect_lists_labeled() {
            for (j, inner) in list.iter().enumerate() {
                deep(inner, stage, &format!("{path}/{pseg}/{j}"), root, f);
            }
        }
    }
    for_each_effect_root(campaign, &mut |site, effs| {
        for (i, eff) in effs.iter().enumerate() {
            deep(eff, site.stage, &format!("{}/{i}", site.path), site.root, f);
        }
    });
}
