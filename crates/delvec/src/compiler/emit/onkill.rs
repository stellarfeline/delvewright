//! On-kill rewards (spec-0074).

use super::*;

/// The `on_kill` **payment ledger** of one fight (spec-0074 §4): `#kf_w_<wave>`
/// or `#kf_a_<actor>` on `dw.sys`, counting how many times the fight's bundle
/// has fired over the whole delve.
///
/// Seeded `0` by `setup` and incremented as the first line of the bundle
/// function, so it counts firings and nothing else; no spawn, re-seat or rest
/// resets it. It is what a `first-kill` guard reads, and it is not the census's
/// `credited` ([`wave_credited_holder`]): that one counts credited kills per
/// seating, this one counts payments per delve — two facts, two holders.
pub(super) fn kill_ledger(fight: delvewright_dsl::Fight<'_>) -> String {
    match fight {
        delvewright_dsl::Fight::Wave(w) => format!("#kf_w_{}", plan::safe_local(w.id.as_str())),
        delvewright_dsl::Fight::Actor(a) => format!("#kf_a_{}", plan::safe_local(a.id.as_str())),
    }
}

/// The function a fight's `on_kill` bundle is lowered into (spec-0074 §6):
/// `on_kill_w_<wave>` or `on_kill_a_<actor>`.
pub(super) fn on_kill_function(fight: delvewright_dsl::Fight<'_>) -> String {
    match fight {
        delvewright_dsl::Fight::Wave(w) => {
            format!("on_kill_w_{}", plan::safe_local(w.id.as_str()))
        }
        delvewright_dsl::Fight::Actor(a) => {
            format!("on_kill_a_{}", plan::safe_local(a.id.as_str()))
        }
    }
}

/// How many bodies a fight seats: [`plan::wave_total`] for a wave, one for an
/// actor. A `first-kill` bundle pays at most this many times over the delve.
pub(super) fn fight_bodies(fight: delvewright_dsl::Fight<'_>) -> i32 {
    match fight {
        delvewright_dsl::Fight::Wave(w) => plan::wave_total(w),
        delvewright_dsl::Fight::Actor(_) => 1,
    }
}

/// The line a kill reward runs to pay a fight's bundle (spec-0074 §6):
/// unguarded for `every-kill`; for `first-kill` — and for an absent `fires`,
/// which validation admits only on a fight that never comes back, where the two
/// coincide — guarded on the ledger so the fight pays at most once per body it
/// seats.
pub(super) fn on_kill_call(
    ns: &str,
    fight: delvewright_dsl::Fight<'_>,
    ok: &delvewright_dsl::OnKill,
) -> String {
    let call = format!("function {ns}:{}", on_kill_function(fight));
    match ok.fires {
        Some(delvewright_dsl::KillFires::EveryKill) => call,
        Some(delvewright_dsl::KillFires::FirstKill) | None => format!(
            "execute if score {} dw.sys matches ..{} run {call}",
            kill_ledger(fight),
            fight_bodies(fight) - 1
        ),
    }
}

/// The body of a fight's bundle function (spec-0074 §6): the ledger increment
/// first, then the effects under the root's audience — the credited killer's
/// own (`Audience::Solo`).
pub(super) fn on_kill_body(
    plan: &Plan,
    fight: delvewright_dsl::Fight<'_>,
    ok: &delvewright_dsl::OnKill,
) -> String {
    let mut body = vec![format!(
        "scoreboard players add {} dw.sys 1",
        kill_ledger(fight)
    )];
    body.extend(emit_effect_bundle(
        plan,
        &ok.effects,
        root_audience(delvewright_dsl::EffectRootKind::OnKill),
    ));
    lines(&body)
}

/// The actors whose `on_kill` has machinery (spec-0074 §6): a declared bundle on
/// an actor whose body resolves, in declaration order — the one walk the
/// function emitter, the advancement emitter, `setup` and the PackTests read.
pub(super) fn on_kill_actors<'a>(
    plan: &Plan<'a>,
) -> Vec<(&'a delvewright_dsl::Actor, &'a delvewright_dsl::OnKill)> {
    plan.campaign
        .quests
        .content
        .actors
        .iter()
        .filter(|a| {
            plan.body_point(delvewright_dsl::BodyRef::Actor(a))
                .is_some()
        })
        .filter_map(|a| a.on_kill.as_ref().map(|ok| (a, ok)))
        .collect()
}
