//! Boards (spec-0135 §13): a scoreboard objective in the sidebar whose rows
//! are places, written by the story as bodies finish.
//!
//! The pinned game's primitives, each a command of the pinned tree: an
//! objective (`scoreboard objectives add … dummy <title>`) shown in the
//! sidebar slot (`setdisplay sidebar`), whose entries are score holders — a
//! player by name, any other name a holder of its own — each drawn with a
//! display name (`scoreboard players display name`) and a number format
//! (`display numberformat … fixed`). The sidebar sorts its rows by score,
//! highest first, so a place is written as a score that falls with the
//! place, and the place itself is the number the row shows.

use super::*;
use delvewright_dsl::{BodyKey, PlaceRows, Verb};

/// The objective board `id` is kept in.
pub(crate) fn board_objective(id: &str) -> String {
    format!("dw.bd.{}", plan::safe_local(id))
}

/// The `dw.sys` holder that counts the places board `id` has given.
pub(super) fn board_counter(id: &str) -> String {
    format!("#bp_{}", plan::safe_local(id))
}

/// The score holder a named body's row is kept under: never a player's name
/// (no player's name holds a `.`), never hidden (no `#`).
pub(crate) fn board_holder(key: &BodyKey) -> String {
    format!("dw.r.{}", plan::safe_local(key.id()))
}

/// The function that places the player it runs as on board `id`.
pub(super) fn place_player_fn(id: &str) -> String {
    format!("board_place_{}", plan::safe_local(id))
}

/// The function that places a named body on board `id`.
pub(crate) fn place_body_fn(id: &str, key: &BodyKey) -> String {
    format!(
        "board_row_{}_{}",
        plan::safe_local(id),
        plan::safe_local(key.id())
    )
}

/// `setup` lines: each board's objective, headed by its title. Empty for a
/// campaign that declares none.
pub(super) fn board_setup(plan: &Plan) -> Vec<String> {
    plan.campaign
        .quests
        .content
        .boards
        .iter()
        .map(|b| {
            format!(
                "scoreboard objectives add {} dummy {}",
                board_objective(b.id.as_str()),
                tr(&b.title)
            )
        })
        .collect()
}

/// The lines that give holder `h` the next place on board `id`, once: a
/// holder already placed keeps its place.
fn place_lines(plan: &Plan, id: &str, h: &str) -> Vec<String> {
    let obj = board_objective(id);
    let n = board_counter(id);
    let rows = delvewright_dsl::board_rows(plan.campaign, id).max(1);
    let mut out = vec![
        format!("execute if score {h} {obj} matches -2147483648.. run return 0"),
        format!("scoreboard players add {n} dw.sys 1"),
        format!(
            "execute if score {n} dw.sys matches {}.. run return 0",
            rows + 1
        ),
        format!("scoreboard players set {h} {obj} {}", rows + 1),
        format!("scoreboard players operation {h} {obj} -= {n} dw.sys"),
    ];
    for k in 1..=rows {
        out.push(format!(
            "execute if score {n} dw.sys matches {k} run scoreboard players display numberformat \
             {h} {obj} fixed {}",
            json!({ "text": k.to_string(), "color": "gold" })
        ));
    }
    out
}

/// Every named body some `place-on-board` of board `id` places, in the
/// campaign's effect order, once each.
pub(super) fn placed_bodies(c: &delvewright_dsl::Campaign, id: &str) -> Vec<BodyKey> {
    let mut out: Vec<BodyKey> = Vec::new();
    for e in all_campaign_effects(c) {
        if let Verb::PlaceOnBoard { board } = &e.verb
            && board.as_str() == id
            && let PlaceRows::Bodies(keys) = delvewright_dsl::place_rows(e)
        {
            for k in keys {
                if !out.contains(&k) {
                    out.push(k);
                }
            }
        }
    }
    out
}

/// Whether some `place-on-board` of board `id` places players.
pub(super) fn places_players(c: &delvewright_dsl::Campaign, id: &str) -> bool {
    all_campaign_effects(c).into_iter().any(|e| {
        matches!(&e.verb, Verb::PlaceOnBoard { board } if board.as_str() == id)
            && delvewright_dsl::place_rows(e) == PlaceRows::Players
    })
}

/// The name a placed body's row shows: the NPC's or the actor's own.
fn body_name<'c>(c: &'c delvewright_dsl::Campaign, key: &BodyKey) -> Option<&'c str> {
    match key.resolve(c)? {
        delvewright_dsl::BodyRef::Npc(n) => Some(n.name.as_str()),
        delvewright_dsl::BodyRef::Actor(a) => a.name.as_deref(),
    }
}

/// The board functions: per board, the player's placing function (run as
/// the player) when a player is placed on it, and one per named body.
pub(super) fn board_fns(plan: &Plan) -> Vec<(String, String)> {
    let c = plan.campaign;
    let mut out = Vec::new();
    for b in &c.quests.content.boards {
        let id = b.id.as_str();
        if places_players(c, id) {
            out.push((place_player_fn(id), lines(&place_lines(plan, id, "@s"))));
        }
        for key in placed_bodies(c, id) {
            let h = board_holder(&key);
            let mut body = place_lines(plan, id, &h);
            if let Some(name) = body_name(c, &key) {
                // Written after the first line, so a body already placed is
                // not renamed.
                body.insert(
                    1,
                    format!(
                        "scoreboard players display name {h} {} {}",
                        board_objective(id),
                        tr(name)
                    ),
                );
            }
            out.push((place_body_fn(id, &key), lines(&body)));
        }
    }
    out
}

/// What a board verb runs. `who` is the effect's player selector.
pub(super) fn emit_board_verb(plan: &Plan, eff: &QuestEffect, who: &str, body: &mut Vec<String>) {
    let ns = &plan.namespace;
    match &eff.verb {
        Verb::ShowBoard { board } => body.push(format!(
            "scoreboard objectives setdisplay sidebar {}",
            board_objective(board.as_str())
        )),
        Verb::HideBoard { .. } => {
            body.push("scoreboard objectives setdisplay sidebar".to_string());
            // The datum that stands in the sidebar (spec-0076) stands again.
            for (st, display) in standing_states(plan) {
                body.push(format!(
                    "scoreboard objectives setdisplay {} {}",
                    display.slot(),
                    plan::state_score(st.id.as_str())
                ));
            }
        }
        Verb::PlaceOnBoard { board } => match delvewright_dsl::place_rows(eff) {
            PlaceRows::Players => body.push(format!(
                "execute as {who} run function {ns}:{}",
                place_player_fn(board.as_str())
            )),
            PlaceRows::Bodies(keys) => {
                for k in keys {
                    body.push(format!(
                        "function {ns}:{}",
                        place_body_fn(board.as_str(), &k)
                    ));
                }
            }
            // `DW1116` refuses a crowd.
            PlaceRows::Crowd => {}
        },
        _ => {}
    }
}

/// Whether `eff` is a board verb [`emit_board_verb`] lowers.
pub(super) fn is_board_verb(eff: &QuestEffect) -> bool {
    matches!(
        eff.verb,
        Verb::ShowBoard { .. } | Verb::HideBoard { .. } | Verb::PlaceOnBoard { .. }
    )
}
