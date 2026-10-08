//! The naming scheme: the scoreboard, tag and marker names every id lowers to.

use super::*;

/// The **party holder** (spec-0018): the single fake player that carries every
/// shared progression score.
///
/// Progress is a fact about the party, not about a player. Objective completion,
/// quest activation/completion, story flags, the announce-once latches and
/// campaign completion all live on `#party` — so any player's completing action
/// advances everyone, and two players clearing two arms of an `after` AND-join in
/// two different rooms unlock the successor together.
///
/// A fake player needs no entity and survives every join/leave, which is exactly
/// the lifetime party state needs. Everything that is genuinely per-player —
/// class + kit, `dw.dlg_shown`, the interact/dialogue triggers, `dw.dmask`, the
/// `deathCount` respawn edge, the stealth grace clocks, `dw.hold` — stays on the
/// player and is deliberately NOT routed here.
pub const PARTY: &str = "#party";

/// The declared mandatory party size (spec-0018 `world.min_players`), defaulting
/// to 1 — a party of one is always legal, and every pre-0.6 campaign reads as 1.
pub fn min_players(campaign: &Campaign) -> u8 {
    campaign.world.content.min_players.unwrap_or(1)
}

/// Sanitize an id's local part (after its `/`) to `[a-z0-9_]`.
pub fn safe_local(id: &str) -> String {
    let local = id.split_once('/').map(|(_, r)| r).unwrap_or(id);
    local.replace(['-', '/', '.'], "_")
}

/// The machine completion-marker token for campaign completion. An objective's
/// token is simply its own id (`obj/<kebab>`).
pub const MARKER_TOKEN_CAMPAIGN: &str = "campaign";

/// One line of the machine completion-marker channel:
/// `[dw:complete <campaign_id> <token>]`.
///
/// The format is **anchored and exact**: the harness matches the whole chat line
/// against this grammar (campaign id = the running campaign's, token = `campaign`
/// or an `obj/<kebab>` id), never a substring anywhere in chat. Three properties
/// make it a real oracle:
/// * player chat reaches the client as `<name> …`, so no player can utter a line
///   that starts with the sigil;
/// * the campaign id is part of the match, so a marker from other content cannot
///   satisfy this campaign's step;
/// * the sigil is reserved in every player-visible string by `DW0182`
///   ([`delvewright_dsl::validate_marker_channel`]), so authored — or
///   LLM-translated — text cannot forge one.
pub fn marker_line(campaign_id: &str, token: &str) -> String {
    format!("[dw:complete {campaign_id} {token}]")
}

/// Scoreboard objective for a DSL objective id.
pub fn obj_score(objective_id: &str) -> String {
    format!("dw.o_{}", safe_local(objective_id))
}
/// Scoreboard objective marking a quest complete.
pub fn quest_score(quest_id: &str) -> String {
    format!("dw.q_{}", safe_local(quest_id))
}
/// Scoreboard objective marking a quest active (its trigger fired).
pub fn quest_active_score(quest_id: &str) -> String {
    format!("dw.qa_{}", safe_local(quest_id))
}
/// Trigger objective for an NPC's dialogue.
pub fn dlg_trigger(npc_id: &str) -> String {
    format!("dw.dlg_{}", safe_local(npc_id))
}
/// Per-player scoreboard for a campaign flag (`set-flag` / `requires_flags`, v0.3).
pub fn flag_score(flag_id: &str) -> String {
    format!("dw.f_{}", safe_local(flag_id))
}
/// Scoreboard objective holding a declared runtime datum (`state/<kebab>`, DSL
/// v0.10, spec-0031).
///
/// One objective per datum, holding an ordinary integer. **Who** holds the value
/// is the datum's declared scope, not a property of the objective: a `party`
/// datum lives on the [`PARTY`] fake player (where every story flag already
/// lives, spec-0018) and a `player` datum on each real player.
pub fn state_score(state_id: &str) -> String {
    format!("dw.s_{}", safe_local(state_id))
}

/// The per-player tag marking "this player's `player`-scoped data are seeded to
/// their declared initials" (DSL v0.10). Player tags live in player data, so the
/// seed runs exactly once per player per world — on their first tick, never
/// again on a relog, which is what makes a datum survive a disconnect the way a
/// scoreboard score does.
pub const STATE_SEEDED_TAG: &str = "dw_state";

/// Trigger objective the bot chats / an interaction advancement sets to drive an
/// `interact` objective (v0.3).
pub fn interact_trigger(obj_id: &str) -> String {
    format!("dw.i_{}", safe_local(obj_id))
}
/// The shared scoreboard objective holding every wave's remaining-mob countdown
/// (fake players `#<wave>`, v0.3).
pub const WAVE_OBJECTIVE: &str = "dw.wave";
/// The fake-player key holding a wave's remaining-mob count.
pub fn wave_counter(wave_id: &str) -> String {
    format!("#{}", safe_local(wave_id))
}
/// The entity tag stamped on a wave's spawned mobs (v0.3).
pub fn wave_tag(wave_id: &str) -> String {
    format!("dw_wave_{}", safe_local(wave_id))
}

/// The entity tag a **census brand** stamps on a wave's currently-living mobs
/// The harness applies it before a scripted death and reads it back
/// after the re-seat: a mob still wearing it is, by identity and not by
/// silhouette, one the previous life already fought.
///
/// Per wave rather than one shared brand, so branding one encounter can never
/// colour a neighbouring wave's census.
pub fn wave_brand_tag(wave_id: &str) -> String {
    format!("dw_brand_{}", safe_local(wave_id))
}

/// Marker token for the per-wave census SUMMARY line.
pub const MARKER_TOKEN_CENSUS: &str = "census";
/// Marker token for one mob's line inside a census.
pub const MARKER_TOKEN_CENSUS_MOB: &str = "censusmob";
/// Marker token for the per-wave MUSTER summary line (`compiler::muster`).
pub const MARKER_TOKEN_MUSTER: &str = "muster";
/// Marker token for one live body's reading inside a muster.
pub const MARKER_TOKEN_MUSTER_BODY: &str = "musterbody";
