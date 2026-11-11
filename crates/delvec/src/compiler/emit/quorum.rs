//! A quorum of objectives (spec-0134 §7): `after: [{any, at_least}]` holds when
//! `at_least` of its members are done, and when it first holds every member
//! not yet done is **lapsed** — settled, its completion bundle never fired, its
//! act refused after, its announcement closed with the lapse line, counted
//! toward its quest's completion.
//!
//! The emission is the one place the `after` gate is spelled for the datapack:
//! [`after_cond`] writes every guard's `after` terms and [`after_drive`] the
//! lines that make them hold (the PackTests' driver), both through the same
//! per-term reading, so a guard and its driver cannot disagree.

use super::*;

/// The `dw.sys` holder a quorum's done-count is summed into: quorum `i` of
/// objective `holder`.
pub(super) fn quorum_count(holder: &str, i: usize) -> String {
    format!("#q_{}_{i}", plan::safe_local(holder))
}

/// The function a quorum runs when it first holds: it lapses every member
/// not yet settled.
fn lapse_fn(holder: &str, i: usize) -> String {
    format!("lapse_q_{}_{i}", safe_obj_fn(holder))
}

/// The function that settles `oid` lapsed.
pub(super) fn settle_lapsed_fn(oid: &str) -> String {
    format!("settle_lapsed_{}", safe_obj_fn(oid))
}

/// Every quorum, with the objective that waits on it and its index among that
/// objective's quorums, in document order.
fn quorums_of(c: &delvewright_dsl::Campaign) -> Vec<(&Objective, usize, &delvewright_dsl::Quorum)> {
    c.quests
        .content
        .quests
        .iter()
        .flat_map(|q| &q.objectives)
        .flat_map(|o| {
            o.after_quorums()
                .into_iter()
                .enumerate()
                .map(move |(i, q)| (o, i, q))
        })
        .collect()
}

/// Whether `oid` is a member of some quorum: it settles done or lapsed, and
/// carries a settled score as a limited objective does.
pub(super) fn is_member(c: &delvewright_dsl::Campaign, oid: &str) -> bool {
    quorums_of(c)
        .iter()
        .any(|(_, _, q)| q.any.iter().any(|m| m.as_str() == oid))
}

/// **The guard terms of `o`'s `after`**, in declaration order: a plain
/// prerequisite's settled (or done) score at 1, a quorum's summed done-count at
/// `at_least` or more.
pub(super) fn after_cond(c: &delvewright_dsl::Campaign, o: &Objective) -> String {
    let p = plan::PARTY;
    let mut g = String::new();
    let mut qi = 0;
    for t in o.after_terms() {
        match t {
            delvewright_dsl::AfterTerm::Objective(a) => {
                g.push_str(&format!(
                    " if score {p} {} matches 1",
                    after_score(c, a.as_str())
                ));
            }
            delvewright_dsl::AfterTerm::Quorum(q) => {
                g.push_str(&format!(
                    " if score {} dw.sys matches {}..",
                    quorum_count(o.id().as_str(), qi),
                    q.at_least
                ));
                qi += 1;
            }
        }
    }
    g
}

/// **Each of `o`'s `after` terms written false and written true**, in
/// [`after_cond`]'s order: a plain prerequisite's settled score at 0 or 1, a
/// quorum's count one short of its `at_least` or at it. The PackTests drive a
/// gate open, or shut by one term, through these.
pub(super) fn after_term_lines(
    c: &delvewright_dsl::Campaign,
    o: &Objective,
) -> Vec<(String, String)> {
    let p = plan::PARTY;
    let mut out = Vec::new();
    let mut qi = 0;
    for t in o.after_terms() {
        match t {
            delvewright_dsl::AfterTerm::Objective(a) => {
                let sc = after_score(c, a.as_str());
                out.push((
                    format!("scoreboard players set {p} {sc} 0"),
                    format!("scoreboard players set {p} {sc} 1"),
                ));
            }
            delvewright_dsl::AfterTerm::Quorum(q) => {
                let h = quorum_count(o.id().as_str(), qi);
                out.push((
                    format!("scoreboard players set {h} dw.sys {}", q.at_least - 1),
                    format!("scoreboard players set {h} dw.sys {}", q.at_least),
                ));
                qi += 1;
            }
        }
    }
    out
}

/// **The lines that make `o`'s `after` hold**: every term of
/// [`after_term_lines`], written true.
pub(super) fn after_drive(c: &delvewright_dsl::Campaign, o: &Objective) -> Vec<String> {
    after_term_lines(c, o).into_iter().map(|(_, t)| t).collect()
}

/// The lines `complete_<m>` gains for a quorum member: settled, and every
/// quorum naming it re-summed — run on the tick it first holds.
pub(super) fn member_completion_lines(plan: &Plan, oid: &str) -> Vec<String> {
    let c = plan.campaign;
    let ns = &plan.namespace;
    let p = plan::PARTY;
    let mut out = Vec::new();
    if !is_member(c, oid) {
        return out;
    }
    // A limited member's own completion already writes it (spec-0133 §3).
    if limited(c, oid).is_none() {
        out.push(format!(
            "scoreboard players set {p} {} 1",
            settled_score(oid)
        ));
    }
    for (holder, i, q) in quorums_of(c) {
        if !q.any.iter().any(|m| m.as_str() == oid) {
            continue;
        }
        let h = holder.id().as_str();
        let count = quorum_count(h, i);
        out.push(format!("scoreboard players set {count} dw.sys 0"));
        for m in &q.any {
            out.push(format!(
                "execute if score {p} {} matches 1 run scoreboard players add {count} dw.sys 1",
                obj_score(m.as_str())
            ));
        }
        out.push(format!(
            "execute if score {count} dw.sys matches {}.. run function {ns}:{}",
            q.at_least,
            lapse_fn(h, i)
        ));
    }
    out
}

/// The guard term a quorum member gains (spec-0134 §7): once lapsed, nothing
/// completes it. A limited member's settled score is its expiry's and its
/// lapse's at once, so the one term covers both.
pub(super) fn lapsed_term(c: &delvewright_dsl::Campaign, o: &Objective) -> String {
    if is_member(c, o.id().as_str()) && o.limit().is_none() {
        format!(
            " unless score {} {} matches 1",
            plan::PARTY,
            settled_score(o.id().as_str())
        )
    } else {
        String::new()
    }
}

/// `setup` lines: the settled score every quorum member owns (a limited one
/// owns it already).
pub(super) fn quorum_setup_lines(plan: &Plan) -> Vec<String> {
    let c = plan.campaign;
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for (_, _, q) in quorums_of(c) {
        for m in &q.any {
            let oid = m.as_str();
            if limited(c, oid).is_none() && seen.insert(oid.to_string()) {
                out.push(format!(
                    "scoreboard objectives add {} dummy",
                    settled_score(oid)
                ));
            }
        }
    }
    out
}

/// Every function a quorum owns: its lapse, and each member's settlement as
/// lapsed.
pub(super) fn quorum_fns(
    plan: &Plan,
    chrome: &delvewright_dsl::Chrome,
    holders: &[crate::compiler::holders::OwnedHolder],
    guide: &guidance::Guide,
) -> Vec<(String, String)> {
    let c = plan.campaign;
    let ns = &plan.namespace;
    let p = plan::PARTY;
    let mut fns = Vec::new();
    let mut settled: BTreeSet<String> = BTreeSet::new();
    for (holder, i, q) in quorums_of(c) {
        let h = holder.id().as_str();
        let body: Vec<String> = q
            .any
            .iter()
            .map(|m| {
                format!(
                    "execute unless score {p} {} matches 1 run function {ns}:{}",
                    settled_score(m.as_str()),
                    settle_lapsed_fn(m.as_str())
                )
            })
            .collect();
        fns.push((lapse_fn(h, i), lines(&body)));
        for m in &q.any {
            let oid = m.as_str();
            if !settled.insert(oid.to_string()) {
                continue;
            }
            let Some((qu, o)) = c.quests.content.quests.iter().find_map(|qq| {
                qq.objectives
                    .iter()
                    .find(|o| o.id().as_str() == oid)
                    .map(|o| (qq, o))
            }) else {
                continue;
            };
            let q_area = plan.quest_area(qu.id.as_str()).unwrap_or("");
            let mut s = vec![format!(
                "scoreboard players set {p} {} 1",
                settled_score(oid)
            )];
            if o.announced(&c.quests.content.guidance)
                && let Some(title) = o.title()
            {
                s.push(format!(
                    "tellraw @a {}",
                    tr_with(
                        &chrome.get(delvewright_dsl::chrome::LIMIT_LAPSED),
                        &[
                            ("color", json!("gray")),
                            (
                                "with",
                                json!([tr_with(title, &[("color", json!("white"))])])
                            ),
                        ],
                    )
                ));
            }
            // A limited member's clock stops with it.
            if o.limit().is_some() {
                s.extend(super::limit::stop_settlement_lines(ns, oid));
            }
            // The objective's affordance goes as completion's does.
            if !activation_commands(plan, holders, q_area, o).is_empty() {
                s.extend(completion_cleanup(plan, o));
            }
            s.extend(guidance::completion_lines(guide, ns, oid));
            s.push(format!(
                "function {ns}:check_q_{}",
                plan::safe_local(qu.id.as_str())
            ));
            fns.push((settle_lapsed_fn(oid), lines(&s)));
        }
    }
    fns
}
