//! Loops: an endless corridor (spec-0086).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{FlagId, LoopId, Mark, QuestEffect, StateCompare, StateId, StealthZone};

#[cfg(doc)]
use crate::LethalVolume;

/// A stage-5 **loop** (spec-0086): a slab a body crosses and is returned from,
/// by a whole-block offset, to an earlier section that looks exactly the same —
/// its position inside the cell, its facing and its velocity all kept.
///
/// # It is a region with a standing property
///
/// A loop acts on whatever body enters a volume, every tick, the way a
/// [`LethalVolume`] does; nothing completes and nobody is addressed. So it is
/// declared beside the lethal volume, with the same region type
/// ([`StealthZone`], resolved through the one `Plan::zone_box`), and not as a
/// trigger with a relative teleport in it: the seamlessness proof is a property
/// of the region-plus-offset pair, which a compiler would otherwise have to
/// recognise by pattern-matching a trigger's effect list.
///
/// # The offset is derived, never typed
///
/// `to` is a [`Mark`] naming where the slab's own anchor cell lands, so the
/// offset is `cell(to) − cell(region.anchor)` — a whole-block vector by
/// construction, and a judgement about the world (*the fourth bay's anchor lands
/// on the second's*) rather than a vector the compiler could compute.
///
/// # The gate is the release
///
/// The loop **holds** while its gate is open and stands down while it is shut,
/// read against the party: flags are campaign state, and a `requires_state` term
/// must name a `party` datum (`DW0949`). A loop with no gate term holds forever
/// and is refused at the document (`DW0949`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Loop {
    /// Unique loop id (`loop/<kebab>`).
    pub id: LoopId,
    /// The slab a body crosses: an anchor-centred box (`anchor ± extent`), one
    /// axis of which is the crossing axis. The existing zone type, for the reason
    /// [`LethalVolume::region`] gives.
    pub region: StealthZone,
    /// Where the slab's own anchor cell lands: the loop's offset is
    /// `cell(to) − cell(region.anchor)`. Lies inside its anchor's piece
    /// (`DW0897`).
    pub to: Mark,
    /// Flags that must all be set for the loop to hold.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_flags: Vec<FlagId>,
    /// Flags any one of which stands the loop down — `forbids_flags:
    /// [flag/the-bell-found]` is a loop that ends the moment the bell is found.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forbids_flags: Vec<FlagId>,
    /// Numeric gate terms: every comparison must hold for the loop to hold. Each
    /// names a `party`-scoped datum (`DW0949`). The third field of the one gate,
    /// carried by every gate consumer.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires_state: Vec<StateCompare>,
    /// A `party`-scoped datum the loop raises by one on every move, before
    /// `on_cross` runs — the counter a crossing-counted release reads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counts: Option<StateId>,
    /// The dungeon's answer to a move: effects run on every move, after the body
    /// is moved and the count raised, from the server command source (no acting
    /// player). Each effect's own `when` keys a write to a count. A `teleport`
    /// here is refused (`DW0949`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on_cross: Vec<QuestEffect>,
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostic::{Diagnostic, codes};
use crate::envelope::Campaign;
use crate::registry::AnchorRegistry;
use crate::validate::{AnchorProviders, station_kind_diag};

/// Stage-5 loop structural checks (spec-0086): id syntax and uniqueness, the two
/// anchors resolvable, and the release a fact about the party (`DW0949`).
///
/// Everything geometric — the slab's shape, the move clearing it, the closed and
/// identical view — is about the solved layout and lives in the compiler
/// (`compiler::loop`).
pub(crate) fn loop_checks(c: &Campaign, anchors: &dyn AnchorRegistry, d: &mut Vec<Diagnostic>) {
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
