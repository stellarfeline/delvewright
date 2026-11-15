use super::*;

/// The board template (spec-0135 §13), synchronous, per board that places a
/// named body: `board_<b>_places` clears the board, places each named body
/// in the campaign's order, asserts the first holds the top place and each
/// next one the place below, then places the first again and asserts its
/// place and the board's count did not move — a placing that is not once, or
/// a place written the wrong way up for the sidebar's order, reds it. A
/// player's row runs the same lines as the player (`board_place_<b>`), which a
/// template's dummy does not stand in for; the body rows bind the lines.
pub(super) fn emit_board_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    for b in &plan.campaign.quests.content.boards {
        let id = b.id.as_str();
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
