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

/// **Each body a census places stands on a cell of its own** (spec-0134
/// §1): every census row the harness performs by placement (`select`) gains
/// `cells`, taken in order from its datum's cells (`nav::CensusProof::cells`)
/// and never one a body was already put on along the same path. A body the
/// census moved stays where it was put once it is let go: two data counted
/// over one box (three goats' pen) share it, and a body put onto another, in
/// the same step or a later one, is pushed out of the box or into a wall.
///
/// A row that moves bodies of a declared set takes exactly `count` cells. A
/// census of a kind (`stand_in`) takes every cell left once the step's other
/// rows have taken theirs: the bodies of the kind already in the box (a
/// penned goat an earlier step's story put there) count toward it, so the
/// harness summons only the shortfall, each onto a cell no body stands in
/// when it acts, and removes them when the act ends — its cells are free
/// again for the next step. A datum whose box holds fewer such cells than
/// the row places hands over what it holds; the harness refuses a row short
/// of its count by name.
pub(super) fn annotate_census_cells(cp: &mut Value, cells: &BTreeMap<String, Vec<[i32; 3]>>) {
    let Some(steps) = cp.get_mut("steps").and_then(Value::as_array_mut) else {
        return;
    };
    // Cells a moved body still stands on, along the whole path.
    let mut kept: BTreeSet<[i32; 3]> = BTreeSet::new();
    for step in steps {
        let Some(rows) = step.get_mut("census").and_then(Value::as_array_mut) else {
            continue;
        };
        let mut taken: BTreeSet<[i32; 3]> = kept.clone();
        // The rows that move a declared set's bodies first, then the kinds'.
        for stand_ins in [false, true] {
            for row in rows.iter_mut() {
                if row.get("select").is_none() || row.get("stand_in").is_some() != stand_ins {
                    continue;
                }
                let count = row["count"].as_u64().unwrap_or(0) as usize;
                let state = row["state"].as_str().unwrap_or("");
                let free = cells
                    .get(state)
                    .into_iter()
                    .flatten()
                    .filter(|c| !taken.contains(*c));
                let mine: Vec<[i32; 3]> = if stand_ins {
                    free.copied().collect()
                } else {
                    free.take(count).copied().collect()
                };
                if !stand_ins {
                    taken.extend(mine.iter().copied());
                    kept.extend(mine.iter().copied());
                }
                if let Some(o) = row.as_object_mut() {
                    o.insert("cells".to_string(), json!(mine));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Three goats counted into one pen by three data, one body each: three
    /// cells, no two alike. The rows carried none before, and every body was
    /// put on the box's centre.
    #[test]
    fn three_data_over_one_box_take_three_cells() {
        let row = |s: &str| json!({"state": s, "objective": "o", "select": "tag=t", "count": 1});
        let mut cp = json!({"steps": [{"action": "interact", "census": [
            row("state/a"), row("state/b"), row("state/c"),
            {"state": "state/r", "objective": "o", "riders": "dw_vehicle_v", "count": 1}
        ]}]});
        let pen = vec![[1, 65, 1], [2, 65, 1], [1, 65, 2]];
        let cells: BTreeMap<String, Vec<[i32; 3]>> = ["state/a", "state/b", "state/c"]
            .iter()
            .map(|s| (s.to_string(), pen.clone()))
            .collect();
        annotate_census_cells(&mut cp, &cells);
        let rows = cp["steps"][0]["census"].as_array().unwrap();
        let got: Vec<Value> = rows[..3].iter().map(|r| r["cells"].clone()).collect();
        assert_eq!(
            got,
            vec![
                json!([[1, 65, 1]]),
                json!([[2, 65, 1]]),
                json!([[1, 65, 2]])
            ]
        );
        assert!(
            rows[3].get("cells").is_none(),
            "a riders row is boarded, not placed"
        );
    }

    /// A body moved by one step's census still stands on its cell when a
    /// later step places another: the later one takes the next cell. A kind's
    /// census is handed every cell still free, and its bodies are removed
    /// when its act ends, so its cells are free again. Red before: every step
    /// started from the first cell, and the three goats homed one step at a
    /// time were put on one cell.
    #[test]
    fn a_later_step_never_takes_a_cell_a_moved_body_still_stands_on() {
        let row = |s: &str| json!({"state": s, "objective": "o", "select": "tag=t", "count": 1});
        let stand_in = json!({"state": "state/k", "objective": "o", "select": "type=g",
            "stand_in": "g", "count": 2});
        let mut cp = json!({"steps": [
            {"action": "trigger", "census": [row("state/a")]},
            {"action": "trigger", "census": [stand_in.clone()]},
            {"action": "trigger", "census": [row("state/b")]},
            {"action": "interact", "census": [stand_in]},
        ]});
        let pen = vec![[1, 65, 1], [2, 65, 1], [1, 65, 2], [2, 65, 2]];
        let cells: BTreeMap<String, Vec<[i32; 3]>> = ["state/a", "state/b", "state/k"]
            .iter()
            .map(|s| (s.to_string(), pen.clone()))
            .collect();
        annotate_census_cells(&mut cp, &cells);
        let got: Vec<Value> = cp["steps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["census"][0]["cells"].clone())
            .collect();
        assert_eq!(
            got,
            vec![
                json!([[1, 65, 1]]),
                json!([[2, 65, 1], [1, 65, 2], [2, 65, 2]]),
                json!([[2, 65, 1]]),
                json!([[1, 65, 2], [2, 65, 2]]),
            ]
        );
    }

    /// In one step a kind's census listed first still takes only what the
    /// rows moving declared bodies leave.
    #[test]
    fn a_kinds_census_takes_what_the_steps_other_rows_leave() {
        let mut cp = json!({"steps": [{"action": "interact", "census": [
            {"state": "state/k", "objective": "o", "select": "type=g", "stand_in": "g", "count": 1},
            {"state": "state/a", "objective": "o", "select": "tag=t", "count": 2},
        ]}]});
        let pen = vec![[1, 65, 1], [2, 65, 1], [1, 65, 2]];
        let cells: BTreeMap<String, Vec<[i32; 3]>> = ["state/a", "state/k"]
            .iter()
            .map(|s| (s.to_string(), pen.clone()))
            .collect();
        annotate_census_cells(&mut cp, &cells);
        let rows = &cp["steps"][0]["census"];
        assert_eq!(rows[1]["cells"], json!([[1, 65, 1], [2, 65, 1]]));
        assert_eq!(rows[0]["cells"], json!([[1, 65, 2]]));
    }
}
