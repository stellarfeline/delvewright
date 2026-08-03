//! Rehearsal harvest (spec-0019 §4): turn the `[DelveShot]` stamps a creator's
//! `/trigger dw.done` writes into a playtest server log into a versioned
//! `rehearsal-report.json`, beside `playtest-report.json`.
//!
//! ## The stamp line
//!
//! ```text
//! [DelveShot] shot=<n> beat=<n> ptr=<json-pointer> idx=<n> seconds=<n> look_at=<x,y,z|none> path=<x,y,z;…>
//! ```
//!
//! Emitted through `say` — the only vanilla command whose output reaches the
//! server stdout log the harvester reads (`tellraw` does not; see
//! `crates/compiler/src/creator.rs`). Every value is an **integer block cell**:
//! that is the DSL's own granularity for a camera waypoint (`anchor + integer
//! offset`), so the proposal → report → patch round trip is lossless, and it is
//! the only NBT numeric type a function macro substitutes without a type suffix.
//!
//! `shot`, `beat`, `ptr` and `idx` are compile-time constants baked into the
//! overlay: `ptr` is the JSON pointer of the **`cutscene` effect** inside the
//! `quests` stage document and `idx` the shot's 0-based index within it, so a
//! harvested proposal always knows which DSL node a patch belongs on — under
//! either cutscene spelling (`…/shots/<idx>` for the multi-shot form, the effect
//! itself for the single-shot one, whose index is always 0).
//!
//! ## One session, last word wins
//!
//! A creator may fire `dw.done` several times in a session (it is cheap and
//! non-destructive). The report keeps the **last** stamp per shot id — the final
//! state of the proposal — and records how many times each shot was stamped, so
//! a harvest can never silently mix an early and a late reading of one loop.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::split_log_line;

/// The rehearsal report schema version. Independent of `playtest-report.json`:
/// the two artifacts describe different things and version separately.
pub const REHEARSAL_VERSION: &str = "0.1.0";

/// The stamp prefix the overlay writes.
const STAMP: &str = "[DelveShot] ";

/// One harvested shot proposal.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ShotProposal {
    /// 1-based shot id (the `<s>` of `/trigger dw.mark set <s>`).
    pub shot: u32,
    /// 1-based id of the beat this shot belongs to.
    pub beat: u32,
    /// JSON pointer to the `cutscene` effect in the `quests` stage document.
    pub pointer: String,
    /// The shot's 0-based index within that effect.
    pub shot_index: u32,
    /// Camera waypoints, as absolute block cells, in path order.
    pub path: Vec<[i64; 3]>,
    /// The look target cell, when the proposal names one.
    pub look_at: Option<[i64; 3]>,
    /// Shot duration in seconds.
    pub seconds: u32,
    /// Log timestamp (`HH:MM:SS`) of the stamp this entry came from.
    pub at: String,
    /// How many times this shot was stamped in the session (`dw.done` may be
    /// fired repeatedly; the reported values are from the last one).
    pub stamps: u32,
}

/// The `rehearsal-report.json` document.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RehearsalReport {
    /// Schema version.
    pub version: String,
    /// The campaign the proposals belong to.
    pub campaign_id: String,
    /// Every stamped shot, ordered by shot id.
    pub shots: Vec<ShotProposal>,
}

/// Harvest every `[DelveShot]` stamp out of a server log.
pub fn harvest_rehearsal(log: &str, campaign_id: &str) -> RehearsalReport {
    let mut by_shot: BTreeMap<u32, ShotProposal> = BTreeMap::new();
    for line in log.lines() {
        let (secs, Some(msg)) = split_log_line(line) else {
            continue;
        };
        let Some(mut stamp) = parse_shot_stamp(msg) else {
            continue;
        };
        stamp.at = secs.map(fmt_hms).unwrap_or_default();
        let seen = by_shot.get(&stamp.shot).map(|p| p.stamps).unwrap_or(0);
        stamp.stamps = seen + 1;
        by_shot.insert(stamp.shot, stamp);
    }
    RehearsalReport {
        version: REHEARSAL_VERSION.to_string(),
        campaign_id: campaign_id.to_string(),
        shots: by_shot.into_values().collect(),
    }
}

/// Serialize a report the way every Delvewright JSON artifact is written:
/// canonical pretty JSON with a trailing newline.
pub fn rehearsal_json(report: &RehearsalReport) -> String {
    let mut s = serde_json::to_string_pretty(report).expect("report serializes");
    s.push('\n');
    s
}

/// Parse one `[DelveShot] …` payload. Returns `None` for any line that is not a
/// well-formed stamp — a truncated or hand-typed lookalike is skipped, never
/// half-applied (the same tolerance `[DelveNote]` parsing has).
fn parse_shot_stamp(msg: &str) -> Option<ShotProposal> {
    let idx = msg.find(STAMP)?;
    let payload = &msg[idx + STAMP.len()..];
    let mut shot = None;
    let mut beat = None;
    let mut pointer = None;
    let mut shot_index = 0;
    let mut seconds = None;
    let mut look_at = None;
    let mut path = None;
    for field in payload.split_whitespace() {
        let (key, val) = field.split_once('=')?;
        match key {
            "shot" => shot = val.parse().ok(),
            "beat" => beat = val.parse().ok(),
            "ptr" => pointer = Some(val.to_string()),
            "idx" => shot_index = val.parse().unwrap_or(0),
            "seconds" => seconds = val.parse().ok(),
            "look_at" => look_at = if val == "none" { None } else { parse_cell(val) },
            "path" => path = parse_path(val),
            // Unknown keys are ignored so the stamp can grow a field without
            // breaking an older harvester.
            _ => {}
        }
    }
    Some(ShotProposal {
        shot: shot?,
        beat: beat.unwrap_or(0),
        pointer: pointer?,
        shot_index,
        path: path?,
        look_at,
        seconds: seconds?,
        at: String::new(),
        stamps: 0,
    })
}

/// `x,y,z;x,y,z;…` → waypoint cells. An empty path is legal (a shot whose
/// proposal was cleared) and parses to an empty list.
fn parse_path(val: &str) -> Option<Vec<[i64; 3]>> {
    if val.is_empty() {
        return Some(Vec::new());
    }
    val.split(';').map(parse_cell).collect()
}

/// `x,y,z` → a block cell.
fn parse_cell(val: &str) -> Option<[i64; 3]> {
    let mut it = val.split(',');
    let x = it.next()?.trim().parse().ok()?;
    let y = it.next()?.trim().parse().ok()?;
    let z = it.next()?.trim().parse().ok()?;
    if it.next().is_some() {
        return None;
    }
    Some([x, y, z])
}

/// Seconds-of-day → `HH:MM:SS`.
fn fmt_hms(secs: i64) -> String {
    format!(
        "{:02}:{:02}:{:02}",
        secs / 3600,
        (secs / 60) % 60,
        secs % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = "\
[06:12:01] [Server thread/INFO]: Done (12.345s)! For help, type \"help\"
[06:12:44] [Server thread/INFO]: [Server] [DelveShotRoster] shots=2 1=/content/quests/0/on_complete/0/shots/0 2=/content/quests/0/on_complete/0/shots/1
[06:13:02] [Server thread/INFO]: [Server] [DelveShot] shot=1 beat=1 ptr=/content/quests/0/on_complete/0 idx=0 seconds=6 look_at=5,67,4 path=3,67,8;7,67,8
[06:13:02] [Server thread/INFO]: [Server] [DelveShot] shot=2 beat=1 ptr=/content/quests/0/on_complete/0 idx=1 seconds=4 look_at=none path=5,67,5
";

    /// The happy path: a fixture log yields one entry per stamped shot with
    /// every field resolved, ordered by shot id.
    #[test]
    fn parses_a_fixture_log_into_a_versioned_report() {
        let r = harvest_rehearsal(LOG, "hello-world");
        assert_eq!(r.version, "0.1.0");
        assert_eq!(r.campaign_id, "hello-world");
        assert_eq!(r.shots.len(), 2);
        let one = &r.shots[0];
        assert_eq!(one.shot, 1);
        assert_eq!(one.beat, 1);
        assert_eq!(one.pointer, "/content/quests/0/on_complete/0");
        assert_eq!(one.shot_index, 0);
        assert_eq!(r.shots[1].shot_index, 1);
        assert_eq!(one.path, vec![[3, 67, 8], [7, 67, 8]]);
        assert_eq!(one.look_at, Some([5, 67, 4]));
        assert_eq!(one.seconds, 6);
        assert_eq!(one.at, "06:13:02");
        assert_eq!(one.stamps, 1);
        // A travel-aimed shot reports no look target rather than a sentinel.
        assert_eq!(r.shots[1].look_at, None);
    }

    /// The roster line shares the `[DelveShot`-ish prefix space; it must not be
    /// mistaken for a proposal.
    #[test]
    fn the_roster_line_is_not_a_proposal() {
        let r = harvest_rehearsal(
            "[06:12:44] [Server thread/INFO]: [Server] [DelveShotRoster] shots=2 1=/a 2=/b\n",
            "hello-world",
        );
        assert!(r.shots.is_empty());
    }

    /// `dw.done` fired twice keeps the LAST reading — the creator's final word —
    /// and counts the stamps, so a mixed early/late harvest is impossible.
    #[test]
    fn a_second_dw_done_supersedes_the_first() {
        let log = format!(
            "{LOG}[06:20:00] [Server thread/INFO]: [Server] [DelveShot] shot=1 beat=1 \
             ptr=/content/quests/0/on_complete/0 idx=0 seconds=9 look_at=1,2,3 path=0,64,0\n"
        );
        let r = harvest_rehearsal(&log, "hello-world");
        assert_eq!(r.shots.len(), 2);
        assert_eq!(r.shots[0].seconds, 9);
        assert_eq!(r.shots[0].path, vec![[0, 64, 0]]);
        assert_eq!(r.shots[0].at, "06:20:00");
        assert_eq!(r.shots[0].stamps, 2);
    }

    /// Negative coordinates survive: the island's shots live at negative Z, and
    /// a sign-eating parser would silently mirror every camera.
    #[test]
    fn negative_coordinates_round_trip() {
        let log = "[00:00:01] [Server thread/INFO]: [Server] [DelveShot] shot=1 beat=1 ptr=/p \
                   seconds=5 look_at=-9,69,-56 path=-13,70,-41;-9,71,-56\n";
        let r = harvest_rehearsal(log, "island");
        assert_eq!(r.shots[0].path, vec![[-13, 70, -41], [-9, 71, -56]]);
        assert_eq!(r.shots[0].look_at, Some([-9, 69, -56]));
    }

    /// A malformed stamp is skipped whole, never half-applied.
    #[test]
    fn a_malformed_stamp_is_skipped() {
        let log = "[00:00:01] [Server thread/INFO]: [Server] [DelveShot] shot=1 beat=1 \
                   seconds=5 path=nonsense\n";
        assert!(harvest_rehearsal(log, "x").shots.is_empty());
    }

    /// Canonical serialization, like every other Delvewright JSON artifact.
    #[test]
    fn report_json_is_pretty_with_trailing_newline() {
        let json = rehearsal_json(&harvest_rehearsal(LOG, "hello-world"));
        assert!(json.starts_with("{\n"));
        assert!(json.ends_with("}\n"));
        assert!(json.contains("\"version\": \"0.1.0\""));
    }
}
