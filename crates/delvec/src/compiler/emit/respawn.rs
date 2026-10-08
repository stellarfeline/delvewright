//! The respawn wait and party wipe clocks.

use super::*;

/// The `dw.sys` latch a party wipe sets and the first respawn after it spends.
pub(super) const WIPE: &str = "#wipe";

/// The `dw.sys` scratch counting the players alive this tick.
pub(super) const ALIVE: &str = "#alive";

/// The tag every player dead at a party wipe carries until they respawn.
pub(super) const WIPED: &str = "dw_wiped";

/// spec-0077: each waiting player's wait clock, in ticks. It runs from 1, and a
/// score is held exactly while the player waits, so `matches 1..` is "is
/// waiting" and a player who never waited (no entry) reads the same as one who
/// was released (entry reset) — `DW0495`'s rule. The tick only reaches online
/// players, so the clock pauses while its player is away.
pub(super) const RW_CLOCK: &str = "dw.rwait";

/// The `dw.sys` scratch counting the players present this tick.
pub(super) const RW_PRESENT: &str = "#present";

/// The `dw.sys` scratch the respawn edge reads: players in play other than the
/// one coming back.
pub(super) const RW_OTHERS: &str = "#rw_others";

/// The `dw.sys` scratch holding a waiting player's whole seconds left.
pub(super) const RW_LEFT: &str = "#rw_left";

/// The `dw.sys` constant the seconds-left division divides by.
pub(super) const RW_TPS: &str = "#rw_tps";

/// The campaign's declared respawn wait (spec-0077), if any.
pub(super) fn respawn_wait(plan: &Plan) -> Option<delvewright_dsl::RespawnWait> {
    plan.campaign.world.content.respawn_wait
}

/// Does this campaign latch party wipes? A bonfire re-seats on one (spec-0016
/// §1) and a respawn wait ends on one (spec-0077 §4).
pub(super) fn wipes(plan: &Plan) -> bool {
    plan.bonfires().next().is_some() || respawn_wait(plan).is_some()
}

/// The selector argument that keeps a player who is only watching out of a
/// positional or health rule ([`CUTSCENE_TAG`]'s staging invariant). Spliced
/// into the selectors that do not already carry it. Unconditional: a cutscene
/// viewer carries the tag in every campaign, and a declared respawn wait only
/// adds a second state that does.
pub(super) fn observer_guard(_plan: &Plan) -> String {
    format!(",tag=!{CUTSCENE_TAG}")
}

/// The selector of the living teammate in play a waiting player watches
/// (spec-0077 §4.2): the nearest player not in the observation state and not on
/// a death screen. The watcher stands where its target stands, so the nearest
/// is the one it already watches and the binding is stable.
pub(super) fn rw_watch_target() -> String {
    format!("@p[tag=!{CUTSCENE_TAG},nbt=!{{Health:0.0f}}]")
}

/// **The respawn wait's tick** (spec-0077 §4): the lines that end every wait at
/// a party wipe and drive each waiting player's own clock. Placed after
/// [`party_wipe_tick`], whose counts they read, and before the respawn edge.
/// Empty without a declared wait.
///
/// A wipe is no player in play. With `alone: true` a player alone in the delve
/// waits with nobody in play by construction, so there the wipe that ends a wait
/// is one with a second player present. While a cutscene plays the wait is held
/// whole: the cutscene owns every camera and its end restores every mode, so the
/// clock resumes, and the state is re-applied, when it is over.
///
/// The lock line is NOT held by a cutscene: a waiting player answers nothing
/// (spec-0077 §5) for the whole wait, and the per-tick `enable @a` lines near
/// the top of the tick re-arm a dialog channel every tick. The lock runs after
/// every one of them, so each tick ends with the waiting player's channels shut.
pub(super) fn respawn_wait_tick(plan: &Plan) -> Vec<String> {
    let Some(w) = respawn_wait(plan) else {
        return Vec::new();
    };
    let ns = &plan.namespace;
    let party = if w.alone {
        format!("if score {RW_PRESENT} dw.sys matches 2.. ")
    } else {
        String::new()
    };
    let held = if campaign_has_cutscene(plan.campaign) {
        format!("unless score {CS_LIVE} dw.sys matches 1.. ")
    } else {
        String::new()
    };
    vec![
        format!(
            "execute if score {ALIVE} dw.sys matches 0 {party}as @a if score @s {RW_CLOCK} \
             matches 1.. run function {ns}:rw_release"
        ),
        format!("execute as @a if score @s {RW_CLOCK} matches 1.. run function {ns}:rw_lock"),
        format!("execute {held}as @a if score @s {RW_CLOCK} matches 1.. run function {ns}:rw_tick"),
    ]
}

/// The respawn wait's functions (spec-0077 §4). Empty without a declared wait.
///
/// * `rw_begin` (as the player coming back, on the respawn edge) decides: in a
///   party (a second player present) the player waits when somebody else is in
///   play and they were not part of a wipe; alone, they wait only with `alone:
///   true`. Otherwise the respawn fires at once, exactly as without a wait.
/// * `rw_start` enters the state: clock at 0, counted out of play for the rest
///   of this tick, the observation tag, spectator, and every answer channel
///   locked.
/// * `rw_lock` locks every trigger objective the delve declares (`answer_channels`,
///   read off the finished `setup`) for `@s`: `scoreboard players reset` clears
///   the score and revokes the permission, so `/trigger` is refused and no
///   dispatch reads a stale answer. Run on entry and every tick of the wait.
///   Release enables nothing: the normal flow re-arms each channel exactly as it
///   would for any player — the per-tick `enable @a`, `class_arm`, or the
///   bonfire / shop opening its own dialog.
/// * `rw_tick` holds the state (a relog comes back in adventure and a cutscene's
///   end restores adventure, so mode and tag are re-applied), runs the clock,
///   shows the seconds left, and binds the view to a teammate in play unless the
///   player holds sneak — the cutscene bounce's rule. Alone, with nobody to
///   watch, the player watches from the active checkpoint.
/// * `rw_release` leaves the state and fires the respawn the edge held back:
///   the seat on the checkpoint cell, the fire's per-player half, the wipe.
pub(super) fn emit_respawn_wait_functions(
    plan: &Plan,
    chrome: &delvewright_dsl::Chrome,
    answer_channels: &[String],
) -> Vec<(String, String)> {
    let Some(w) = respawn_wait(plan) else {
        return Vec::new();
    };
    let ns = &plan.namespace;
    let ticks = u32::from(w.seconds) * 20;
    let target = rw_watch_target();
    let mut begin = vec![
        format!("scoreboard players operation {RW_OTHERS} dw.sys = {ALIVE} dw.sys"),
        format!("scoreboard players remove {RW_OTHERS} dw.sys 1"),
        format!(
            "execute if score {RW_PRESENT} dw.sys matches 2.. if entity @s[tag=!{WIPED}] if score \
             {RW_OTHERS} dw.sys matches 1.. run return run function {ns}:rw_start"
        ),
    ];
    if w.alone {
        begin.push(format!(
            "execute if score {RW_PRESENT} dw.sys matches ..1 run return run function {ns}:rw_start"
        ));
    }
    begin.push(format!("function {ns}:cp_respawn_fire"));
    let start = vec![
        format!("scoreboard players set @s {RW_CLOCK} 1"),
        format!("scoreboard players remove {ALIVE} dw.sys 1"),
        format!("tag @s add {CUTSCENE_TAG}"),
        "gamemode spectator @s".to_string(),
        format!("function {ns}:rw_lock"),
    ];
    let lock: Vec<String> = answer_channels
        .iter()
        .map(|t| format!("scoreboard players reset @s {t}"))
        .collect();
    let countdown = tr_with(
        &chrome.get(delvewright_dsl::chrome::RESPAWN_WAIT),
        &[
            ("color", json!("gray")),
            (
                "with",
                json!([{ "score": { "name": RW_LEFT, "objective": "dw.sys" }, "color": "white" }]),
            ),
        ],
    )
    .to_string();
    let mut tick = vec![
        "gamemode spectator @s[gamemode=!spectator]".to_string(),
        format!("tag @s add {CUTSCENE_TAG}"),
        format!(
            "execute if score @s {RW_CLOCK} matches {ticks}.. run return run function {ns}:rw_release"
        ),
        format!("scoreboard players set {RW_LEFT} dw.sys {}", ticks + 19),
        format!("scoreboard players operation {RW_LEFT} dw.sys -= @s {RW_CLOCK}"),
        format!("scoreboard players set {RW_TPS} dw.sys 20"),
        format!("scoreboard players operation {RW_LEFT} dw.sys /= {RW_TPS} dw.sys"),
        format!("title @s actionbar {countdown}"),
        format!("scoreboard players add @s {RW_CLOCK} 1"),
        format!(
            "execute at @s unless predicate {ns}:{SNEAK_HELD_PREDICATE} run spectate {target} @s"
        ),
    ];
    if w.alone {
        tick.push(format!(
            "execute unless entity {target} run function {ns}:rw_watch_fire"
        ));
    }
    let release = vec![
        format!("scoreboard players reset @s {RW_CLOCK}"),
        format!("tag @s remove {CUTSCENE_TAG}"),
        "gamemode adventure @s".to_string(),
        format!("function {ns}:cp_respawn_fire"),
    ];
    let mut fns = vec![
        ("rw_begin".to_string(), lines(&begin)),
        ("rw_start".to_string(), lines(&start)),
        ("rw_lock".to_string(), lines(&lock)),
        ("rw_tick".to_string(), lines(&tick)),
        ("rw_release".to_string(), lines(&release)),
    ];
    if w.alone {
        fns.push(("rw_watch_fire".to_string(), lines(&cp_seat_dispatch(plan))));
    }
    fns
}
