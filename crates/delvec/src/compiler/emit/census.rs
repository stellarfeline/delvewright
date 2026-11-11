//! **The census** (spec-0134 §1): a counted datum is written by the engine —
//! the count of a set's bodies whose hitbox meets an anchor-centred box — on a
//! self-scheduling `census_tick` every [`CENSUS_PERIOD_TICKS`] ticks, and a
//! `body-enters` trigger fires when its datum rises from zero.
//!
//! The tick is started once in `setup`, in replace mode, and exists only when
//! the campaign declares a census: a campaign that declares none emits no
//! function and no setup line, byte-identical.

use super::*;

/// How often the census counts, in ticks (spec-0134 §1).
pub(super) const CENSUS_PERIOD_TICKS: u32 = 10;

/// The tick function's name.
pub(super) const CENSUS_TICK_FN: &str = "census_tick";

/// The selector terms that pick a census's bodies, without the box: a wave's
/// and an actor's by the tag the engine puts on them, an NPC's by its body
/// tag, a kind by type. The `minecraft:interaction` a body wears (an answer
/// box, a dialogue hitbox) carries the same tag and is no body, so it is
/// excluded by type. `None` for a vehicle's riders, which no selector picks:
/// a rider is a player whose vehicle carries the tag ([`census_store_lines`]).
pub(crate) fn census_terms(plan: &Plan, of: &delvewright_dsl::CensusOf) -> Option<String> {
    use delvewright_dsl::CensusOf;
    Some(match of {
        CensusOf::Riders(_) => return None,
        CensusOf::Wave(w) => format!("tag={}", plan::wave_tag(w.as_str())),
        CensusOf::Actor(a) => format!(
            "tag=dw_actor_{},type=!minecraft:interaction",
            plan::safe_local(a.as_str())
        ),
        CensusOf::Npc(n) => {
            let tag = plan
                .npcs
                .iter()
                .find(|p| p.npc_id == n.as_str())
                .map_or_else(
                    || format!("dw_npc_{}", plan::safe_local(n.as_str())),
                    |p| p.tag.clone(),
                );
            format!("tag={tag},type=!minecraft:interaction")
        }
        CensusOf::Kind(k) => format!("type={k}"),
    })
}

/// One counted datum, its census, and the box resolved.
pub(crate) type Census<'p> = (
    &'p delvewright_dsl::StateDecl,
    &'p delvewright_dsl::StateCounts,
    ([i32; 3], [i32; 3]),
);

/// Every counted datum with its resolved box, in declaration order.
pub(crate) fn censuses<'p>(plan: &'p Plan) -> Vec<Census<'p>> {
    plan.campaign
        .quests
        .content
        .state
        .iter()
        .filter_map(|s| {
            let k = s.counts.as_ref()?;
            let b = plan.zone_box(&k.region)?;
            Some((s, k, b))
        })
        .collect()
}

/// The lines that write one census: one `store` over the set's selector, or
/// — for a vehicle's riders (spec-0135 §5) — the datum set to nothing and one
/// added for every player in the box whose vehicle is the vehicle's body.
pub(crate) fn census_store_lines(
    plan: &Plan,
    s: &delvewright_dsl::StateDecl,
    k: &delvewright_dsl::StateCounts,
    b: ([i32; 3], [i32; 3]),
) -> Vec<String> {
    let score = format!("{} {}", plan::PARTY, plan::state_score(s.id.as_str()));
    let boxed = entity_box_selector(b.0, b.1);
    match (&k.of, census_terms(plan, &k.of)) {
        (delvewright_dsl::CensusOf::Riders(v), _) => vec![
            format!("scoreboard players set {score} 0"),
            format!(
                "execute as @a[{},tag=!dw_cutscene] on vehicle if entity @s[tag=dw_vehicle_{}] run \
                 scoreboard players add {score} 1",
                box_selector_args(b.0, b.1),
                plan::safe_local(v.as_str())
            ),
        ],
        (_, Some(terms)) => vec![format!(
            "execute store result score {score} if entity @e[{terms},{boxed}]"
        )],
        (_, None) => Vec::new(),
    }
}

/// The setup line that starts the census, or nothing.
pub(super) fn census_setup(plan: &Plan) -> Vec<String> {
    if censuses(plan).is_empty() {
        return Vec::new();
    }
    vec![format!(
        "schedule function {}:{CENSUS_TICK_FN} {CENSUS_PERIOD_TICKS}t replace",
        plan.namespace
    )]
}

/// The latch a `body-enters` trigger holds while its count is above zero.
pub(crate) fn body_enters_latch(id: &str) -> String {
    format!("#be_{id}")
}

/// `census_tick` and each `body-enters` trigger's dispatch `be_<id>`, or
/// nothing when the campaign declares no census.
///
/// The tick writes every census, then offers each `body-enters` trigger its
/// edge: the trigger fires when its datum is at least 1 and its latch is
/// down, with its own `once` and flag gates; the latch comes down when the
/// count returns to 0, which re-arms a trigger with `once: false`.
pub(super) fn census_fns(plan: &Plan, chrome: &delvewright_dsl::Chrome) -> Vec<(String, String)> {
    let all = censuses(plan);
    if all.is_empty() {
        return Vec::new();
    }
    let ns = &plan.namespace;
    let mut tick: Vec<String> = all
        .iter()
        .flat_map(|(s, k, b)| census_store_lines(plan, s, k, *b))
        .collect();
    let mut out = Vec::new();
    for t in &plan.emitted_triggers(chrome) {
        let delvewright_dsl::TriggerOn::BodyEnters { state } = &t.on else {
            continue;
        };
        let id = plan::safe_local(t.id.as_str());
        let latch = body_enters_latch(&id);
        let score = format!("{} {}", plan::PARTY, plan::state_score(state.as_str()));
        let (once_guard, forbid_guard, flag_guard) = trigger_poll_guards(plan, t);
        tick.push(format!(
            "execute {once_guard}{forbid_guard}unless score {latch} dw.sys matches 1 if score \
             {score} matches 1.. {flag_guard}run function {ns}:be_{id}"
        ));
        tick.push(format!(
            "execute if score {score} matches ..0 run scoreboard players set {latch} dw.sys 0"
        ));
        out.push((
            format!("be_{id}"),
            lines(&[
                format!("scoreboard players set {latch} dw.sys 1"),
                format!("function {ns}:trig_{id}"),
            ]),
        ));
    }
    tick.push(format!(
        "schedule function {ns}:{CENSUS_TICK_FN} {CENSUS_PERIOD_TICKS}t replace"
    ));
    out.insert(0, (CENSUS_TICK_FN.to_string(), lines(&tick)));
    out
}
