//! Grants (spec-0141 §3.2): a stack handed to the whole party is owed to every
//! member, present at the beat or not.
//!
//! `give @a` reaches the players online on the tick it runs. A member who was
//! disconnected on that tick, or who joins the delve after it, holds nothing —
//! and a later objective that needs the stack in a hand leaves that member
//! with nothing to do. So a party give is a **grant**: one function that
//! gives every present player the stack and counts the issue, on the party
//! (`#all`) and on each player it reached. A player whose count is behind the
//! party's is handed the difference, one stack a tick, by the tick's one owed
//! line.
//!
//! A grant is keyed by the stack it gives, so two beats that hand out the same
//! stack share one function and one count, and a member is owed one for each
//! time either fired.

use super::*;

/// The per-player (and `#all`) count of every grant issued: equal on a player
/// and on [`GRANT_PARTY`] exactly when that player is owed nothing.
pub(crate) const GRANT_TOTAL: &str = "dw.gr";

/// The holder the party's issue counts are kept under.
pub(crate) const GRANT_PARTY: &str = "#all";

/// The function a player behind the party's count runs.
pub(crate) const GRANT_OWED_FN: &str = "grant_owed";

/// The count of grant `i`.
pub(crate) fn grant_objective(i: usize) -> String {
    format!("{GRANT_TOTAL}.{i}")
}

/// The function that issues grant `i` to the party.
pub(crate) fn grant_fn(i: usize) -> String {
    format!("grant_{i}")
}

/// The function that hands grant `i` to the one player it runs as.
pub(crate) fn grant_owed_fn(i: usize) -> String {
    format!("grant_owed_{i}")
}

/// The stack a `give-item` gives, as `give`'s item argument with its count:
/// `None` for any other verb and for a source that does not resolve
/// (`DW0112`/`DW0490`).
pub(super) fn given_stack(plan: &Plan, eff: &QuestEffect) -> Option<String> {
    let Verb::GiveItem { source, .. } = &eff.verb else {
        return None;
    };
    let stack = source.resolve(plan.campaign)?;
    let comp = super::stack::command_suffix(&stack, vec![]);
    Some(format!("{}{comp} {}", stack.item, stack.count))
}

/// Whether `eff`, lowered under `aud`, hands its stack to the whole party: a
/// `give-item` that addresses players, is not the one carrier's, and draws no
/// `in` box (a box is the bodies standing in it, which is a fact about the
/// beat and owed to nobody else).
pub(super) fn is_party_grant(eff: &QuestEffect, aud: Audience) -> bool {
    if !matches!(eff.verb, Verb::GiveItem { .. }) || eff.within.is_some() {
        return false;
    }
    if matches!(
        eff.audience,
        Some(
            delvewright_dsl::EffectAudience::Wave(_)
                | delvewright_dsl::EffectAudience::Body(_)
                | delvewright_dsl::EffectAudience::Npc(_)
                | delvewright_dsl::EffectAudience::Bodies(_)
        )
    ) {
        return false;
    }
    let aud = effect_audience(eff, aud);
    if eff.gives_to_one() && aud.has_actor() {
        return false;
    }
    !matches!(aud, Audience::Solo)
}

/// **Every grant the campaign issues**: the stack of each `give-item` some
/// effect root lowers for the whole party, sorted and deduplicated. A grant's
/// index here is its name everywhere ([`grant_fn`], [`grant_objective`]).
///
/// Built from `plan::for_each_effect_root` and the audience each nesting site
/// is lowered under ([`push_effect_deep_audience`]), the walk the timelines are
/// named by, so the call a bundle emits and the function it calls are one set.
pub(crate) fn grants(plan: &Plan) -> Vec<String> {
    let mut out: BTreeSet<String> = BTreeSet::new();
    plan::for_each_effect_root(plan.campaign, &mut |site, effs| {
        let aud = site_audience(&site.root);
        let mut here: Vec<(&QuestEffect, Audience)> = Vec::new();
        for e in effs {
            push_effect_deep_audience(e, aud, &mut here);
        }
        for (e, a) in here {
            if is_party_grant(e, a)
                && let Some(s) = given_stack(plan, e)
            {
                out.insert(s);
            }
        }
    });
    out.into_iter().collect()
}

/// The index of the grant that gives `stack`, when the campaign issues one.
pub(super) fn grant_index(plan: &Plan, stack: &str) -> Option<usize> {
    grants(plan).iter().position(|s| s == stack)
}

/// `setup` lines: the counts, and the party's at zero so a player with no
/// entry reads as behind only once something was issued. Empty for a campaign
/// that issues no grant.
pub(super) fn grant_setup(plan: &Plan) -> Vec<String> {
    let all = grants(plan);
    if all.is_empty() {
        return Vec::new();
    }
    let mut out = vec![
        format!("scoreboard objectives add {GRANT_TOTAL} dummy"),
        format!("scoreboard players add {GRANT_PARTY} {GRANT_TOTAL} 0"),
    ];
    for i in 0..all.len() {
        out.push(format!(
            "scoreboard objectives add {} dummy",
            grant_objective(i)
        ));
        out.push(format!(
            "scoreboard players add {GRANT_PARTY} {} 0",
            grant_objective(i)
        ));
    }
    out
}

/// The selector of the players a grant is owed to: every player who has taken
/// a class, where the campaign has classes (a player still choosing is handed
/// theirs once they have chosen), else every player.
pub(crate) fn owed_players(plan: &Plan) -> &'static str {
    if plan.classes.is_empty() {
        "@a"
    } else {
        "@a[scores={dw.classed=1}]"
    }
}

/// The tick's one owed line: a player whose total is not the party's runs
/// [`GRANT_OWED_FN`]. Empty for a campaign that issues no grant.
pub(super) fn grant_tick(plan: &Plan) -> Vec<String> {
    if grants(plan).is_empty() {
        return Vec::new();
    }
    vec![format!(
        "execute as {} unless score @s {GRANT_TOTAL} = {GRANT_PARTY} {GRANT_TOTAL} run function {}:{GRANT_OWED_FN}",
        owed_players(plan),
        plan.namespace
    )]
}

/// The grant functions: per grant the issue and the hand-over, and the owed
/// dispatch. Empty for a campaign that issues no grant.
pub(super) fn grant_fns(plan: &Plan) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let all = grants(plan);
    if all.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut owed = vec![format!("scoreboard players add @s {GRANT_TOTAL} 0")];
    for (i, stack) in all.iter().enumerate() {
        let obj = grant_objective(i);
        out.push((
            grant_fn(i),
            lines(&[
                format!("give @a {stack}"),
                format!("scoreboard players add @a {obj} 1"),
                format!("scoreboard players add @a {GRANT_TOTAL} 1"),
                format!("scoreboard players add {GRANT_PARTY} {obj} 1"),
                format!("scoreboard players add {GRANT_PARTY} {GRANT_TOTAL} 1"),
            ]),
        ));
        out.push((
            grant_owed_fn(i),
            lines(&[
                format!("give @s {stack}"),
                format!("scoreboard players add @s {obj} 1"),
                format!("scoreboard players add @s {GRANT_TOTAL} 1"),
            ]),
        ));
        owed.push(format!("scoreboard players add @s {obj} 0"));
        owed.push(format!(
            "execute if score @s {obj} < {GRANT_PARTY} {obj} run function {ns}:{}",
            grant_owed_fn(i)
        ));
    }
    out.push((GRANT_OWED_FN.to_string(), lines(&owed)));
    out
}
