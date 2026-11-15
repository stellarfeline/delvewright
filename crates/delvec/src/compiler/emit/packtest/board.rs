use super::*;

/// The board template (spec-0135 §13), synchronous, per board that places a
/// named body: `board_<b>_places` clears the board, places each named body
/// in the campaign's order, asserts the first holds the top place and each
/// next one the place below, then places the first again and asserts its
/// place and the board's count did not move — a placing that is not once, or
/// a place written the wrong way up for the sidebar's order, reds it. A
/// board that places players has `board_<b>_players` beside it
/// ([`players_template`]).
pub(super) fn emit_board_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    for b in &plan.campaign.quests.content.boards {
        let id = b.id.as_str();
        if super::super::places_players(plan.campaign, id) {
            out.insert(
                format!(
                    "packtest-datapack/data/{ns}/test/board_{}_players.mcfunction",
                    plan::safe_local(id)
                ),
                lines(&players_template(plan, &title, id)).into_bytes(),
            );
        }
        let bodies = super::super::placed_bodies(plan.campaign, id);
        if bodies.is_empty() {
            continue;
        }
        let obj = super::super::board_objective(id);
        let n = super::super::board_counter(id);
        let rows = delvewright_dsl::board_rows(plan.campaign, id).max(1);
        let holders: Vec<String> = bodies.iter().map(super::super::board_holder).collect();
        let mut t = packtest_header(&format!(
            "{title}: board `{id}` gives each named body the next place, once (spec-0135 §13)"
        ));
        t.push(format!("scoreboard players reset {n} dw.sys"));
        for h in &holders {
            t.push(format!("scoreboard players reset {h} {obj}"));
        }
        for k in &bodies {
            t.push(format!(
                "function {ns}:{}",
                super::super::place_body_fn(id, k)
            ));
        }
        for (i, h) in holders.iter().enumerate() {
            t.push(format!(
                "assert score {h} {obj} matches {}",
                rows - i as u32
            ));
        }
        t.push(format!(
            "function {ns}:{}",
            super::super::place_body_fn(id, &bodies[0])
        ));
        t.push(format!("assert score {} {obj} matches {rows}", holders[0]));
        t.push(format!("assert score {n} dw.sys matches {}", bodies.len()));
        t.push(format!("scoreboard players reset {n} dw.sys"));
        for h in &holders {
            t.push(format!("scoreboard players reset {h} {obj}"));
        }
        out.insert(
            format!(
                "packtest-datapack/data/{ns}/test/board_{}_places.mcfunction",
                plan::safe_local(id)
            ),
            lines(&t).into_bytes(),
        );
    }
}

/// The board's players template (spec-0135 §13): two placings run as two
/// executors in turn — the template's dummy, then a second body standing in
/// for a second player, since a player's row is the executor's own (`@s`) —
/// each take the next place, the first the top one, and the first placed
/// again keeps its place: the order a party crossing a line one by one is
/// placed in, each by the act that names that player (a `step` with
/// `audience: presser` runs the placing as each player who steps on).
fn players_template(plan: &Plan, title: &str, id: &str) -> Vec<String> {
    let ns = &plan.namespace;
    let safe = plan::safe_local(id);
    let obj = super::super::board_objective(id);
    let n = super::super::board_counter(id);
    let rows = delvewright_dsl::board_rows(plan.campaign, id).max(1);
    let place = format!("function {ns}:{}", super::super::place_player_fn(id));
    let (pin, first) = pin_dummy(&format!("dw_pt_bp_{safe}"));
    let second_tag = format!("dw_pt_bp2_{safe}");
    let second = format!("@e[tag={second_tag},limit=1]");
    let mut t = packtest_header(&format!(
        "{title}: board `{id}` places two players in the order they cross, once each (spec-0135 §13)"
    ));
    t.extend([
        pin,
        format!("kill @e[tag={second_tag}]"),
        format!("summon minecraft:marker ~ ~ ~ {{Tags:[\"{second_tag}\"]}}"),
        format!("scoreboard players reset {n} dw.sys"),
        format!("scoreboard players reset {first} {obj}"),
        format!("scoreboard players reset {second} {obj}"),
        format!("execute as {first} run {place}"),
        format!("execute as {second} run {place}"),
        format!("execute as {first} run {place}"),
        format!("assert score {first} {obj} matches {rows}"),
        format!("assert score {second} {obj} matches {}", rows - 1),
        format!("assert score {n} dw.sys matches 2"),
        format!("scoreboard players reset {n} dw.sys"),
        format!("scoreboard players reset {first} {obj}"),
        format!("kill @e[tag={second_tag}]"),
        format!("tag {first} remove dw_pt_bp_{safe}"),
    ]);
    t
}
