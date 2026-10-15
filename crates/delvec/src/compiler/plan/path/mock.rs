//! The acts a path performs by mock (spec-0108 §6.5): an entry deed no harness
//! step kind drives, run as the engine's own bundle function and asserted by
//! the scoreboard terms it changes.

use super::*;

/// The function the vanilla interaction calls for elective act `act` of kind
/// `kind` — the one its bundle is lowered into, bare (no namespace) — or `None`
/// for a kind the bot drives, or an act whose root the campaign does not hold.
///
/// The names are the emitters' own: `emit::economy` (`shop_pick_<i>_<j>`),
/// `emit::trap` (`trap_fire_<id>`, `trap_disarm_<id>`), `emit::shortcut`
/// (`shortcut_open_<id>`), `emit::onkill` (`on_kill_w_<wave>`,
/// `on_kill_a_<actor>`) and `emit::timed_gate` (`tgate_disarm_<id>`). The build
/// refuses a mock whose function the datapack does not ship
/// (`emit::branch::branch_paths`), so a rename on either side is a red build.
pub fn mock_function(campaign: &Campaign, kind: &str, act: &str) -> Option<String> {
    let content = &campaign.quests.content;
    match kind {
        "shop-offer" => {
            let (shop, offer) = act.rsplit_once("/offer-")?;
            let j: usize = offer.parse().ok()?;
            let i = content.shops.iter().position(|s| s.id.as_str() == shop)?;
            Some(format!("shop_pick_{i}_{j}"))
        }
        "trap" => content
            .traps
            .iter()
            .find(|t| t.id.as_str() == act)
            .map(|t| format!("trap_fire_{}", safe_local(t.id.as_str()))),
        "shortcut" => content
            .shortcuts
            .iter()
            .find(|s| s.id.as_str() == act)
            .map(|s| format!("shortcut_open_{}", safe_local(s.id.as_str()))),
        "on-kill" => {
            if content.waves.iter().any(|w| w.id.as_str() == act) {
                Some(format!("on_kill_w_{}", safe_local(act)))
            } else if content.actors.iter().any(|a| a.id.as_str() == act) {
                Some(format!("on_kill_a_{}", safe_local(act)))
            } else {
                None
            }
        }
        "trap-disarm" => {
            let id = act.strip_suffix("/disarm")?;
            content
                .traps
                .iter()
                .any(|t| t.id.as_str() == id && t.disarm.is_some())
                .then(|| format!("trap_disarm_{}", safe_local(id)))
        }
        "timed-gate-disarm" => {
            let id = act.strip_suffix("/disarm")?;
            content
                .timed_gates
                .iter()
                .any(|g| g.id.as_str() == id && g.disarm.is_some())
                .then(|| format!("tgate_disarm_{}", safe_local(id)))
        }
        _ => None,
    }
}

/// The scoreboard terms that tell walk state `a` from walk state `b`: one term
/// per flag or datum whose value differs, read in `a` (`(terms of a, terms of
/// b)`), flags first then data, each in id order. A datum either state cannot
/// date is not a term — the bot is never handed a value the replay cannot name.
pub(super) fn change_terms(
    campaign: &Campaign,
    a: &crate::compiler::flow::Walk<'_, '_>,
    b: &crate::compiler::flow::Walk<'_, '_>,
) -> (Vec<GateTerm>, Vec<GateTerm>) {
    let flag = |f: &str, held: bool| GateTerm {
        objective: flag_score(f),
        party: true,
        min: Some(1),
        max: Some(1),
        negate: !held,
    };
    let (mut before, mut after) = (Vec::new(), Vec::new());
    let fa = a.flags();
    let fb = b.flags();
    for f in fa.symmetric_difference(fb) {
        before.push(flag(f, fa.contains(f)));
        after.push(flag(f, fb.contains(f)));
    }
    let da: BTreeMap<&str, Option<i64>> = a.data().into_iter().collect();
    for (id, vb) in b.data() {
        let (Some(Some(va)), Some(vb)) = (da.get(id).copied(), vb) else {
            continue;
        };
        if va == vb {
            continue;
        }
        let party = !matches!(
            campaign.quests.content.state_decl(id).map(|s| s.scope),
            Some(delvewright_dsl::StateScope::Player)
        );
        let term = |v: i64| GateTerm {
            objective: state_score(id),
            party,
            min: i32::try_from(v).ok(),
            max: i32::try_from(v).ok(),
            negate: false,
        };
        before.push(term(va));
        after.push(term(vb));
    }
    (before, after)
}

/// The mock steps a story world's path performs in front of path step `si`:
/// every entry deed due there whose act no step kind drives and whose bundle has
/// a function, in the order its world performs them, each with the terms it
/// changes. A deed the bot drives is performed first, as its `trigger` step is.
///
/// The terms are read off the world WITHOUT the act — its participation and its
/// deed removed, walked up to `si` — against that state once the act's own
/// bundle has fired there ([`crate::compiler::flow::Walk::performed_alone`]). The
/// world's own walk credits a performed act's flags from the start (spec-0108
/// §4.1), so it holds no state in front of the act to tell the change by.
pub(super) fn mocks_before(
    campaign: &Campaign,
    flow: &crate::compiler::flow::Flow<'_>,
    path: &crate::compiler::flow::Playthrough,
    si: usize,
) -> Vec<Step> {
    let Some(ctx) = &path.story else {
        return Vec::new();
    };
    let due: Vec<&str> = ctx
        .deeds
        .iter()
        .filter(|(i, _)| *i == si)
        .map(|(_, a)| a.as_str())
        .collect();
    if due.is_empty() {
        return Vec::new();
    }
    let story = flow.story();
    let undrivable = |act: &str| {
        story
            .electives
            .iter()
            .find(|e| e.id == act && e.undrivable.is_some())
            .map(|e| e.kind)
    };
    let driven: Vec<&str> = due
        .iter()
        .copied()
        .filter(|a| undrivable(a).is_none())
        .collect();
    let mocked: Vec<(&str, &'static str)> = due
        .iter()
        .filter_map(|a| undrivable(a).map(|k| (*a, k)))
        .collect();
    let mut out = Vec::new();
    for (n, (act, kind)) in mocked.iter().enumerate() {
        let Some(function) = mock_function(campaign, kind, act) else {
            continue;
        };
        // The world without this act: its participation and its deed gone.
        let mut without = path.clone();
        without.story = Some(std::sync::Arc::new(crate::compiler::flow::StoryCtx {
            performs: ctx.performs.iter().filter(|a| a != act).cloned().collect(),
            deeds: ctx
                .deeds
                .iter()
                .filter(|(_, a)| a != act)
                .cloned()
                .collect(),
        }));
        let mut walk = flow.walk_for(&without);
        for st in &path.steps[..si] {
            walk.take(st);
        }
        // What is performed in front of it here first: the driven deeds, then
        // the mocks before it.
        for a in driven
            .iter()
            .copied()
            .chain(mocked[..n].iter().map(|(a, _)| *a))
        {
            walk = walk.performing(a);
        }
        let alone = walk.performed_alone(act);
        let (before, after) = change_terms(campaign, &walk, &alone);
        out.push(Step::Mock {
            act: act.to_string(),
            kind: kind.to_string(),
            function,
            before,
            after,
        });
    }
    out
}

/// Every mock step story-world playthrough `path` performs, with the index of
/// the path step it is performed in front of. Empty outside a story world.
pub(crate) fn path_mocks(
    campaign: &Campaign,
    flow: &crate::compiler::flow::Flow<'_>,
    path: &crate::compiler::flow::Playthrough,
) -> Vec<(usize, Step)> {
    if path.story.is_none() {
        return Vec::new();
    }
    (0..path.steps.len())
        .flat_map(|si| {
            mocks_before(campaign, flow, path, si)
                .into_iter()
                .map(move |m| (si, m))
        })
        .collect()
}
