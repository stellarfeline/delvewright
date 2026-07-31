//! `delve-harvest` core (spec-0006 M2): turn a playtest server log + the creator
//! overlay's `layout.json` into a versioned `playtest-report.json`.
//!
//! The orchestrator's first real job — thin glue (ADR-0012), no game logic. It
//! parses two kinds of line out of the server stdout log:
//!
//! - **stamp lines** — the overlay's `[DelveNote] pos=[x,y,z] area=… quests=…
//!   nearest_npc=…`, emitted via `say` (so they land in the server log; see
//!   `crates/compiler/src/creator.rs`);
//! - **creator chat lines** — normal player chat (`<name> text`), any language.
//!
//! It **pairs** each stamp with the creator's note, resolves DSL context via the
//! layout manifest, and emits the report.
//!
//! ## Pairing heuristic (spec-0006 "Open"; revised M2)
//!
//! Pair each stamp with the creator chat **nearest in time** within **±60s**;
//! ties prefer the line *before* the stamp; and **each chat line pairs to at most
//! one stamp** (greedy by proximity, assigned globally). A stamp with no chat left
//! in its window yields an empty note `text` (still reported — the context is the
//! point).
//!
//! The earlier rule ("prefer the closest line *after*, chats may pair to many
//! stamps") mis-paired the M2 dress rehearsal: with stamps at `t` and `t+53s` and
//! chats at `t-2s` and `t+51s`, it paired both stamps to the `t+51s` line and
//! **silently dropped** the `t-2s` note (the owner's most important one). The
//! nearest-in-time + one-chat-per-stamp rule pairs `t`→`t-2s` and `t+53s`→`t+51s`,
//! keeping both.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The report schema version (tracks the DSL/critical-path versioning).
pub const REPORT_VERSION: &str = "0.1.0";

/// Pairing window, in seconds, on either side of a stamp.
pub const PAIR_WINDOW_SECS: i64 = 60;

// --------------------------------------------------------------------------
// Layout manifest (input) — the harvester's only campaign knowledge.
// --------------------------------------------------------------------------

/// The creator overlay's `layout.json` (emitted by the compiler beside the
/// overlay). Extra fields are ignored so the manifest can grow.
#[derive(Debug, Clone, Deserialize)]
pub struct Layout {
    /// Campaign id (mirrored into the report).
    pub campaign_id: String,
    /// Areas, carrying the `area → prefab` binding.
    #[serde(default)]
    pub areas: Vec<AreaEntry>,
    /// Objectives, carrying the `objective → quest` binding.
    #[serde(default)]
    pub objectives: Vec<ObjectiveEntry>,
}

/// One area's layout entry.
#[derive(Debug, Clone, Deserialize)]
pub struct AreaEntry {
    /// Area id (`area/…`).
    pub id: String,
    /// The prefab bound to this area (`prefab/…`).
    pub prefab: String,
}

/// One objective's layout entry.
#[derive(Debug, Clone, Deserialize)]
pub struct ObjectiveEntry {
    /// Objective id (`obj/…`).
    pub id: String,
    /// The quest this objective belongs to (`quest/…`).
    pub quest: String,
}

impl Layout {
    /// Parse a `layout.json`.
    pub fn from_json(text: &str) -> Result<Self, String> {
        serde_json::from_str(text).map_err(|e| format!("invalid layout manifest: {e}"))
    }

    fn prefab_for(&self, area_id: &str) -> Option<String> {
        self.areas
            .iter()
            .find(|a| a.id == area_id)
            .map(|a| a.prefab.clone())
    }

    fn quest_for(&self, obj_id: &str) -> Option<&str> {
        self.objectives
            .iter()
            .find(|o| o.id == obj_id)
            .map(|o| o.quest.as_str())
    }
}

// --------------------------------------------------------------------------
// Report (output).
// --------------------------------------------------------------------------

/// The `playtest-report.json` document (spec-0006 §3 shape).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// Schema version.
    pub version: String,
    /// Campaign id (from the layout manifest).
    pub campaign_id: String,
    /// One entry per `[DelveNote]` stamp, in log order.
    pub notes: Vec<Note>,
}

/// One playtest note: a stamp paired with the creator's text, DSL-resolved.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Note {
    /// The stamp's log timestamp (`HH:MM:SS`), verbatim.
    pub at: String,
    /// The paired creator chat text (verbatim, any language; `""` if unpaired).
    pub text: String,
    /// Block position `[x, y, z]` from the stamp.
    pub pos: [i64; 3],
    /// Resolved area id (`null` when the stamp reported `none`).
    pub area: Option<String>,
    /// Resolved prefab id (`null` when the area is unknown/`none`).
    pub prefab: Option<String>,
    /// Completed objectives grouped by quest (only quests with ≥1 done objective).
    pub quest_state: BTreeMap<String, Vec<String>>,
    /// Nearest NPC id (`null` when the stamp reported `none`).
    pub nearest_npc: Option<String>,
}

// --------------------------------------------------------------------------
// Log parsing.
// --------------------------------------------------------------------------

/// A parsed `[DelveNote]` stamp.
#[derive(Debug, Clone, PartialEq)]
struct Stamp {
    /// Timestamp string (`HH:MM:SS`), verbatim for the report.
    at: String,
    /// Seconds-of-day for pairing math (`None` if the line had no timestamp).
    secs: Option<i64>,
    pos: [i64; 3],
    /// Raw `area=` field (`"none"` or an id).
    area: String,
    /// Objective → value, from the `quests=` field.
    quests: Vec<(String, i64)>,
    /// Raw `nearest_npc=` field (`"none"` or an id).
    nearest_npc: String,
}

/// A parsed creator chat line.
#[derive(Debug, Clone, PartialEq)]
struct Chat {
    secs: Option<i64>,
    text: String,
}

/// Split a log line into its `HH:MM:SS` timestamp (if any) and the message body
/// (everything after the `]: ` that ends the log prefix).
fn split_log_line(line: &str) -> (Option<i64>, Option<&str>) {
    let secs = parse_timestamp(line);
    // The vanilla/itzg prefix is `[HH:MM:SS] [thread/LEVEL]: message`. The first
    // `]: ` terminates the thread bracket (the time bracket is followed by ` [`).
    let msg = line.find("]: ").map(|i| line[i + 3..].trim_end());
    (secs, msg)
}

/// Parse the leading `[HH:MM:SS]` of a log line into seconds-of-day.
fn parse_timestamp(line: &str) -> Option<i64> {
    let start = line.find('[')?;
    let end = line[start..].find(']')? + start;
    let inner = &line[start + 1..end];
    let mut it = inner.split(':');
    let h: i64 = it.next()?.trim().parse().ok()?;
    let m: i64 = it.next()?.trim().parse().ok()?;
    let s: i64 = it.next()?.trim().parse().ok()?;
    if it.next().is_some() {
        return None;
    }
    Some(h * 3600 + m * 60 + s)
}

/// Parse the `[DelveNote] …` payload out of a message body.
fn parse_stamp(at: String, secs: Option<i64>, msg: &str) -> Option<Stamp> {
    let idx = msg.find("[DelveNote] ")?;
    let payload = &msg[idx + "[DelveNote] ".len()..];

    let mut pos = None;
    let mut area = None;
    let mut quests = Vec::new();
    let mut nearest_npc = None;

    for field in payload.split_whitespace() {
        let (key, val) = field.split_once('=')?;
        match key {
            "pos" => pos = parse_pos(val),
            "area" => area = Some(val.to_string()),
            "quests" => quests = parse_quests(val),
            "nearest_npc" => nearest_npc = Some(val.to_string()),
            _ => {}
        }
    }

    Some(Stamp {
        at,
        secs,
        pos: pos?,
        area: area?,
        quests,
        nearest_npc: nearest_npc?,
    })
}

/// Parse `[x,y,z]` into three ints.
fn parse_pos(val: &str) -> Option<[i64; 3]> {
    let inner = val.strip_prefix('[')?.strip_suffix(']')?;
    let mut it = inner.split(',');
    let x = it.next()?.trim().parse().ok()?;
    let y = it.next()?.trim().parse().ok()?;
    let z = it.next()?.trim().parse().ok()?;
    if it.next().is_some() {
        return None;
    }
    Some([x, y, z])
}

/// Parse `obj/a:1,obj/b:0` into `[(obj/a, 1), (obj/b, 0)]`. An empty field list
/// (no objectives) yields an empty vec.
fn parse_quests(val: &str) -> Vec<(String, i64)> {
    if val.is_empty() {
        return Vec::new();
    }
    val.split(',')
        .filter_map(|pair| {
            let (id, v) = pair.rsplit_once(':')?;
            Some((id.to_string(), v.trim().parse().ok()?))
        })
        .collect()
}

/// Parse a normal `<name> text` chat message body. Returns the text only (the
/// speaker name is irrelevant — the overlay already stamped the context).
///
/// Offline / unsigned chat carries a leading `[Not Secure] ` marker (verified live
/// on the pinned 1.21.11 server in offline mode); it is stripped before matching so
/// the pairing works in both offline (CI/local) and online play.
fn parse_chat(msg: &str) -> Option<String> {
    let msg = msg.strip_prefix("[Not Secure] ").unwrap_or(msg);
    let rest = msg.strip_prefix('<')?;
    let close = rest.find('>')?;
    let text = rest[close + 1..].trim_start();
    Some(text.to_string())
}

// --------------------------------------------------------------------------
// Harvest.
// --------------------------------------------------------------------------

/// Parse a whole server log into stamps and creator chats.
fn scan(log: &str) -> (Vec<Stamp>, Vec<Chat>) {
    let mut stamps = Vec::new();
    let mut chats = Vec::new();
    for line in log.lines() {
        let (secs, Some(msg)) = split_log_line(line) else {
            continue;
        };
        if msg.contains("[DelveNote] ") {
            let at = timestamp_string(line);
            if let Some(stamp) = parse_stamp(at, secs, msg) {
                stamps.push(stamp);
            }
        } else if let Some(text) = parse_chat(msg) {
            chats.push(Chat { secs, text });
        }
    }
    (stamps, chats)
}

/// The verbatim `HH:MM:SS` timestamp string of a line (empty if none).
fn timestamp_string(line: &str) -> String {
    let Some(start) = line.find('[') else {
        return String::new();
    };
    let Some(rel_end) = line[start..].find(']') else {
        return String::new();
    };
    let inner = &line[start + 1..start + rel_end];
    if parse_timestamp(line).is_some() {
        inner.to_string()
    } else {
        String::new()
    }
}

/// Pair every stamp with a creator chat: nearest in time within ±60s, ties prefer
/// the line *before*, each chat used by at most one stamp (greedy by proximity).
/// Returns one text per stamp (index-aligned), empty when nothing pairs.
fn pair_all(stamps: &[Stamp], chats: &[Chat]) -> Vec<String> {
    // Candidate (stamp, chat) pairs within the window, keyed for greedy ordering:
    // (|delta|, before-first, stamp idx, chat idx) — `after` is `1` so a strictly
    // earlier line wins an equal-distance tie; the index fields make ordering
    // total and deterministic (ADR-0006). Lexicographic tuple sort does the rest.
    let mut cands: Vec<(i64, u8, usize, usize)> = Vec::new();
    for (si, s) in stamps.iter().enumerate() {
        let Some(ss) = s.secs else { continue };
        for (ci, ch) in chats.iter().enumerate() {
            let Some(cs) = ch.secs else { continue };
            let delta = cs - ss;
            if delta.abs() > PAIR_WINDOW_SECS {
                continue;
            }
            let after = u8::from(delta >= 0);
            cands.push((delta.abs(), after, si, ci));
        }
    }
    cands.sort_unstable();

    let mut texts = vec![String::new(); stamps.len()];
    let mut stamp_used = vec![false; stamps.len()];
    let mut chat_used = vec![false; chats.len()];
    for (_, _, si, ci) in cands {
        if stamp_used[si] || chat_used[ci] {
            continue;
        }
        texts[si].clone_from(&chats[ci].text);
        stamp_used[si] = true;
        chat_used[ci] = true;
    }
    texts
}

/// Build one report note from a stamp + its paired text, resolved via the layout.
fn resolve(stamp: &Stamp, text: String, layout: &Layout) -> Note {
    let area = id_or_none(&stamp.area);
    let prefab = area.as_deref().and_then(|a| layout.prefab_for(a));

    // Completed objectives (value >= 1) grouped by quest.
    let mut quest_state: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (obj, val) in &stamp.quests {
        if *val >= 1 {
            let quest = layout.quest_for(obj).unwrap_or("quest/unknown").to_string();
            quest_state.entry(quest).or_default().push(obj.clone());
        }
    }

    Note {
        at: stamp.at.clone(),
        text,
        pos: stamp.pos,
        area,
        prefab,
        quest_state,
        nearest_npc: id_or_none(&stamp.nearest_npc),
    }
}

/// Map the overlay's `none` sentinel to `null`; anything else is a DSL id.
fn id_or_none(raw: &str) -> Option<String> {
    if raw == "none" {
        None
    } else {
        Some(raw.to_string())
    }
}

/// Harvest a server log + layout manifest into a [`Report`].
pub fn harvest(log: &str, layout: &Layout) -> Report {
    let (stamps, chats) = scan(log);
    let texts = pair_all(&stamps, &chats);
    let notes = stamps
        .iter()
        .zip(texts)
        .map(|(s, text)| resolve(s, text, layout))
        .collect();
    Report {
        version: REPORT_VERSION.to_string(),
        campaign_id: layout.campaign_id.clone(),
        notes,
    }
}

/// Serialize a report as canonical pretty JSON with a trailing newline.
pub fn report_json(report: &Report) -> String {
    let mut s = serde_json::to_string_pretty(report).expect("report serializes");
    s.push('\n');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout() -> Layout {
        Layout::from_json(
            r#"{
              "version": "0.1.0",
              "campaign_id": "hello-world",
              "areas": [{ "id": "area/keep", "prefab": "prefab/hello-room" }],
              "objectives": [
                { "id": "obj/talk", "quest": "quest/open-the-door" },
                { "id": "obj/exit", "quest": "quest/open-the-door" }
              ]
            }"#,
        )
        .unwrap()
    }

    // A realistic itzg/vanilla server log: a stamp (`say`, square-bracket sender)
    // then the creator's Chinese note (`<name>` chat), then a second round.
    const LOG: &str = "\
[12:00:01] [Server thread/INFO]: Done (8.123s)! For help, type \"help\"
[12:00:10] [Server thread/INFO]: delve-creator joined the game
[12:00:20] [Server thread/INFO]: [delve-creator] [DelveNote] pos=[5,65,4] area=area/keep quests=obj/talk:1,obj/exit:0 nearest_npc=npc/keeper
[12:00:23] [Server thread/INFO]: <delve-creator> 这个房间太暗了
[12:01:40] [Server thread/INFO]: <delve-creator> unrelated chatter far from any stamp
[12:05:00] [Server thread/INFO]: [delve-creator] [DelveNote] pos=[261,65,3] area=none quests=obj/talk:1,obj/exit:1 nearest_npc=none
[12:05:02] [Server thread/INFO]: <delve-creator> the exit gate never opened here
";

    #[test]
    fn pairs_stamp_with_following_chinese_note() {
        let report = harvest(LOG, &layout());
        assert_eq!(report.version, "0.1.0");
        assert_eq!(report.campaign_id, "hello-world");
        assert_eq!(report.notes.len(), 2);

        let n0 = &report.notes[0];
        assert_eq!(n0.at, "12:00:20");
        assert_eq!(n0.text, "这个房间太暗了");
        assert_eq!(n0.pos, [5, 65, 4]);
        assert_eq!(n0.area.as_deref(), Some("area/keep"));
        assert_eq!(n0.prefab.as_deref(), Some("prefab/hello-room"));
        assert_eq!(n0.nearest_npc.as_deref(), Some("npc/keeper"));
        // only obj/talk is done (value 1); grouped under its quest.
        assert_eq!(
            n0.quest_state.get("quest/open-the-door"),
            Some(&vec!["obj/talk".to_string()])
        );
    }

    #[test]
    fn resolves_none_sentinels_to_null_and_keeps_all_done_objectives() {
        let report = harvest(LOG, &layout());
        let n1 = &report.notes[1];
        assert_eq!(n1.text, "the exit gate never opened here");
        assert_eq!(n1.area, None);
        assert_eq!(n1.prefab, None);
        assert_eq!(n1.nearest_npc, None);
        // both objectives done → both listed under the quest.
        assert_eq!(
            n1.quest_state.get("quest/open-the-door"),
            Some(&vec!["obj/talk".to_string(), "obj/exit".to_string()])
        );
    }

    #[test]
    fn pairs_nearest_in_time_not_a_later_line() {
        // A chat 5s BEFORE and a chat 8s AFTER the stamp: the nearer (before) wins.
        // (The old rule preferred the later line — the M2 mispairing defect.)
        let log = "\
[09:00:00] [Server thread/INFO]: <c> five seconds before
[09:00:05] [Server thread/INFO]: [c] [DelveNote] pos=[1,2,3] area=area/keep quests= nearest_npc=none
[09:00:13] [Server thread/INFO]: <c> eight seconds after
";
        let report = harvest(log, &layout());
        assert_eq!(report.notes.len(), 1);
        assert_eq!(report.notes[0].text, "five seconds before");
        // empty quests field → no quest_state entries.
        assert!(report.notes[0].quest_state.is_empty());
    }

    #[test]
    fn equal_distance_tie_prefers_the_line_before() {
        // A chat exactly 4s before and 4s after: the before line wins the tie.
        let log = "\
[08:00:00] [Server thread/INFO]: <c> the before line
[08:00:04] [Server thread/INFO]: [c] [DelveNote] pos=[1,2,3] area=area/keep quests= nearest_npc=none
[08:00:08] [Server thread/INFO]: <c> the after line
";
        let report = harvest(log, &layout());
        assert_eq!(report.notes[0].text, "the before line");
    }

    #[test]
    fn nearest_in_time_one_chat_per_stamp_reproduces_m2_mispairing() {
        // The real dress-rehearsal transcript: stamps at t and t+53s, chats at
        // t-2s and t+51s. The old rule paired BOTH stamps to the +51s line and
        // dropped the -2s note. The new rule pairs each stamp to its nearer line,
        // and no chat is reused.
        let log = "\
[10:00:58] [Server thread/INFO]: <c> early important note
[10:01:00] [Server thread/INFO]: [c] [DelveNote] pos=[1,2,3] area=area/keep quests=obj/talk:1 nearest_npc=npc/keeper
[10:01:51] [Server thread/INFO]: <c> later note
[10:01:53] [Server thread/INFO]: [c] [DelveNote] pos=[4,5,6] area=none quests=obj/exit:1 nearest_npc=none
";
        let report = harvest(log, &layout());
        assert_eq!(report.notes.len(), 2);
        assert_eq!(report.notes[0].text, "early important note"); // t   -> t-2s
        assert_eq!(report.notes[1].text, "later note"); // t+53 -> t+51s (chat not reused)
    }

    #[test]
    fn known_objectives_resolve_to_their_quest_never_unknown() {
        // Every stamped-done objective declared in the layout resolves to its
        // quest; `quest/unknown` is only ever a fallback for an objective the
        // layout does not declare (regression lock for the reported bug).
        let log = "[07:00:00] [Server thread/INFO]: [c] [DelveNote] pos=[1,2,3] area=area/keep quests=obj/talk:1,obj/exit:1 nearest_npc=none\n";
        let report = harvest(log, &layout());
        let qs = &report.notes[0].quest_state;
        assert_eq!(
            qs.get("quest/open-the-door"),
            Some(&vec!["obj/talk".to_string(), "obj/exit".to_string()])
        );
        assert!(
            !qs.contains_key("quest/unknown"),
            "known objectives never fall back to quest/unknown"
        );
    }

    #[test]
    fn falls_back_to_line_before_when_none_after_in_window() {
        let log = "\
[09:00:00] [Server thread/INFO]: <c> shortly before
[09:00:04] [Server thread/INFO]: [c] [DelveNote] pos=[1,2,3] area=area/keep quests=obj/talk:0 nearest_npc=npc/keeper
[09:02:00] [Server thread/INFO]: <c> way too late to pair
";
        let report = harvest(log, &layout());
        assert_eq!(report.notes[0].text, "shortly before");
        // obj/talk not done (0) → empty quest_state.
        assert!(report.notes[0].quest_state.is_empty());
    }

    #[test]
    fn stamp_with_no_chat_in_window_yields_empty_text() {
        let log = "[09:00:04] [Server thread/INFO]: [c] [DelveNote] pos=[1,2,3] area=none quests= nearest_npc=none\n";
        let report = harvest(log, &layout());
        assert_eq!(report.notes.len(), 1);
        assert_eq!(report.notes[0].text, "");
    }

    // Verbatim lines captured from a live pinned-1.21.11 offline server (the
    // note-bot flow, 2026-07-30): offline chat is `[Not Secure] <name> …` and the
    // `say`-emitted stamp is `[Not Secure] [name] [DelveNote] …`. Locks in the
    // real-world prefix handling.
    #[test]
    fn parses_real_offline_server_log_lines() {
        let log = "\
[08:57:05] [Server thread/INFO]: [Not Secure] [delve-creator] [DelveNote] pos=[5,65,2] area=area/keep quests=obj/talk:0,obj/exit:0 nearest_npc=npc/keeper
[08:57:07] [Server thread/INFO]: [Not Secure] <delve-creator> 这个房间太暗了
";
        let report = harvest(log, &layout());
        assert_eq!(report.notes.len(), 1);
        let n = &report.notes[0];
        assert_eq!(n.at, "08:57:05");
        assert_eq!(n.text, "这个房间太暗了");
        assert_eq!(n.pos, [5, 65, 2]);
        assert_eq!(n.area.as_deref(), Some("area/keep"));
        assert_eq!(n.prefab.as_deref(), Some("prefab/hello-room"));
        assert_eq!(n.nearest_npc.as_deref(), Some("npc/keeper"));
        // fresh player: no objective done → quest_state resolves to empty.
        assert!(n.quest_state.is_empty());
    }

    #[test]
    fn report_json_is_pretty_with_trailing_newline() {
        let report = harvest(LOG, &layout());
        let json = report_json(&report);
        assert!(json.ends_with("}\n"));
        // round-trips as valid JSON and preserves the multibyte note.
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["notes"][0]["text"], "这个房间太暗了");
    }
}
