//! Boards: a list of places the story shows in the sidebar (spec-0135 §13).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::BoardId;

/// **A board of places** (spec-0135 §13): the pinned game's scoreboard
/// objective in the sidebar display slot, headed by `title`, whose rows are
/// score holders — a player by name, a named NPC or actor by a holder the
/// engine names and shows under the body's own name. A row is written by a
/// `place-on-board` the story fires at the moment a body finishes (a player
/// crossing a line, a racer's walk arriving), and its place is the order the
/// writes came in; `show-board` and `hide-board` put the board in the slot and
/// take it out.
///
/// A class of its own and not a `state[]` datum: a datum is a value the
/// story reads and gates on, one holder per player; a board is a record of an
/// order that nothing gates on, its rows include bodies that are not players,
/// and its slot is shown and hidden by the story rather than standing from
/// load.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Board {
    /// Unique board id (`board/<kebab>`).
    pub id: BoardId,
    /// The heading over the rows.
    pub title: String,
}

/// The most rows a board can be given: a party of four and every named body
/// a `place-on-board` of that board can name. Rows past this are not
/// written; the engine sizes it from the campaign.
pub fn board_rows(c: &crate::envelope::Campaign, board: &str) -> u32 {
    let mut bodies: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut players = false;
    crate::for_each_campaign_effect(c, &mut |_, _, e| {
        if let crate::Verb::PlaceOnBoard { board: b } = &e.verb
            && b.as_str() == board
        {
            match place_rows(e) {
                PlaceRows::Players => players = true,
                PlaceRows::Bodies(keys) => {
                    bodies.extend(keys.iter().map(|k| k.id().to_string()));
                }
                PlaceRows::Crowd => {}
            }
        }
    });
    // spec-0139: a race that places its finishers here places every player
    // and every actor it lists.
    for r in &c.quests.content.races {
        if r.board.as_ref().is_some_and(|b| b.as_str() == board) {
            players = true;
            bodies.extend(r.racers.iter().map(|a| a.as_str().to_string()));
        }
    }
    let party = if players { crate::wave::MAX_PARTY } else { 0 };
    party + bodies.len() as u32
}

/// Who a `place-on-board` places, read off its envelope's `audience`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlaceRows {
    /// Players: the acting player, or the party.
    Players,
    /// Named staged bodies, in the order the audience lists them.
    Bodies(Vec<crate::BodyKey>),
    /// A wave's bodies: a crowd, never a row (`DW1116`).
    Crowd,
}

/// [`PlaceRows`] of an effect.
pub fn place_rows(e: &crate::QuestEffect) -> PlaceRows {
    use crate::EffectAudience;
    match &e.audience {
        None | Some(EffectAudience::Actor | EffectAudience::Party) => PlaceRows::Players,
        Some(EffectAudience::Wave(_)) => PlaceRows::Crowd,
        Some(EffectAudience::Body(a)) => PlaceRows::Bodies(vec![crate::BodyKey::actor(a.clone())]),
        Some(EffectAudience::Npc(n)) => PlaceRows::Bodies(vec![crate::BodyKey::npc(n.clone())]),
        Some(EffectAudience::Bodies(keys)) => PlaceRows::Bodies(keys.clone()),
    }
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

use crate::diagnostic::{Diagnostic, DwCode, ExitTier, codes};
use crate::envelope::Campaign;

crate::dw_code! {
    /// (spec-0135 §13) **A row a board cannot show**: a `place-on-board`
    /// whose audience is a wave's bodies — a crowd, which a row of places
    /// does not name — or names a body that carries no name, whose row would
    /// read as an id. Validation-tier (exit 1). Prescription: place the
    /// acting player, the party or a named NPC or actor; give the body a
    /// `name`.
    pub const BOARD_ROW_UNSHOWN: DwCode = DwCode::new("DW1116", ExitTier::Build);
}

/// Stage-5 board checks (spec-0135 §13): id syntax and uniqueness, every
/// board verb naming a declared board (`DW0112`), and every row one a board
/// can show (`DW1116`).
pub(crate) fn board_checks(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let boards = &c.quests.content.boards;
    for (i, b) in boards.iter().enumerate() {
        crate::ids::id_syntax!(d, b.id, "quests", format!("/content/boards/{i}/id"));
    }
    crate::ids::dup_check(
        boards
            .iter()
            .enumerate()
            .map(|(i, b)| (b.id.as_str(), format!("/content/boards/{i}/id"))),
        "quests",
        "board",
        d,
    );
    let declared: std::collections::BTreeSet<&str> = boards.iter().map(|b| b.id.as_str()).collect();
    crate::for_each_campaign_effect(c, &mut |path, _site, e| {
        let board = match &e.verb {
            crate::Verb::ShowBoard { board }
            | crate::Verb::HideBoard { board }
            | crate::Verb::PlaceOnBoard { board } => board,
            _ => return,
        };
        if !declared.contains(board.as_str()) {
            d.push(Diagnostic::error(
                codes::DANGLING_REF,
                "quests",
                path.to_string(),
                format!(
                    "`{}` names unknown board `{board}` — declare it in the stage-5 `boards` \
                     list, or fix the reference",
                    e.verb.tag()
                ),
            ));
        }
        if !matches!(e.verb, crate::Verb::PlaceOnBoard { .. }) {
            return;
        }
        match place_rows(e) {
            PlaceRows::Players => {}
            PlaceRows::Crowd => d.push(Diagnostic::error(
                BOARD_ROW_UNSHOWN,
                "quests",
                path.to_string(),
                format!(
                    "`place-on-board` places a wave's bodies on `{board}`: a wave is a crowd, \
                     and a row of places names one body. Place the acting player, the party, \
                     or a named NPC or actor"
                ),
            )),
            PlaceRows::Bodies(keys) => {
                for k in keys {
                    let named = match k.resolve(c) {
                        Some(crate::BodyRef::Npc(_)) => true,
                        Some(crate::BodyRef::Actor(a)) => a.name.is_some(),
                        None => true, // `DW0112` names a dangling body.
                    };
                    if !named {
                        d.push(Diagnostic::error(
                            BOARD_ROW_UNSHOWN,
                            "quests",
                            path.to_string(),
                            format!(
                                "`place-on-board` places actor `{}` on `{board}`, and the actor \
                                 carries no `name`: its row would read as an id. Give the actor \
                                 a `name`",
                                k.id()
                            ),
                        ));
                    }
                }
            }
        }
    });
}
