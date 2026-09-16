//! Camera harvest (spec-0069): turn the `[DelveCamera]` stamps a creator's
//! `/trigger dw.cam set <n>` writes into a playtest server log into a versioned
//! `camera-report.json`, beside `playtest-report.json`.
//!
//! ## The stamp line
//!
//! ```text
//! [DelveCamera] slot=<n> eye=<x_mb>,<y_mb>,<z_mb> yaw=<centi-degrees> pitch=<centi-degrees> in=<air|block>
//! ```
//!
//! Emitted through `say`, the one vanilla command whose output reaches the
//! server log (`crates/delvec/src/compiler/creator.rs`). Every number is a
//! **fixed-point integer**: the eye in milli-blocks, the rotation in
//! centi-degrees — the one NBT type a function macro substitutes without a type
//! suffix. The harvester divides, so no float ever crosses a macro. The eye is
//! the player's own eye point in whatever pose they were in (`execute anchored
//! eyes`), and the rotation is the entity's `Rotation`, which is the camera
//! record's own convention (`compiler::view::camera`): a harvested camera is
//! written into `design/cameras.json` with no conversion.
//!
//! `in` says whether the eye's own cell held anything but air when the stamp
//! was taken. It refuses nothing: the build's clear-eye proof (`DW0724`) is the
//! verdict, and names the block.
//!
//! ## One slot, last word wins
//!
//! A creator re-fires `dw.cam` on one slot until the frame is right. The report
//! keeps the **last** stamp per slot and counts every stamp, so a harvest never
//! mixes an early and a late pose.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::orchestrator::split_log_line;

/// The camera report schema version.
pub const CAMERA_REPORT_VERSION: &str = "0.1.0";

/// The stamp prefix the overlay writes.
const STAMP: &str = "[DelveCamera] ";

/// One harvested camera pose.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapturedCamera {
    /// The slot fired (`/trigger dw.cam set <slot>`; `/trigger dw.cam` is 1).
    pub slot: u32,
    /// The eye point, world blocks.
    pub eye: [f64; 3],
    /// Minecraft yaw, degrees.
    pub yaw: f64,
    /// Minecraft pitch, degrees: positive looks down.
    pub pitch: f64,
    /// `air` when the eye's cell held only air, `block` otherwise.
    #[serde(rename = "in")]
    pub eye_in: String,
    /// Log timestamp (`HH:MM:SS`) of the stamp this entry came from.
    pub at: String,
    /// How many times this slot was stamped in the session.
    pub stamps: u32,
}

/// The `camera-report.json` document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CameraReport {
    /// Schema version.
    pub version: String,
    /// The campaign the poses were captured in.
    pub campaign_id: String,
    /// Every stamped slot, ordered by slot.
    pub cameras: Vec<CapturedCamera>,
}

/// Harvest every `[DelveCamera]` stamp out of a server log.
pub fn harvest_cameras(log: &str, campaign_id: &str) -> CameraReport {
    let mut by_slot: BTreeMap<u32, CapturedCamera> = BTreeMap::new();
    for line in log.lines() {
        let (secs, Some(msg)) = split_log_line(line) else {
            continue;
        };
        let Some(mut cam) = parse_camera_stamp(msg) else {
            continue;
        };
        cam.at = secs.map(fmt_hms).unwrap_or_default();
        cam.stamps = by_slot.get(&cam.slot).map(|c| c.stamps).unwrap_or(0) + 1;
        by_slot.insert(cam.slot, cam);
    }
    CameraReport {
        version: CAMERA_REPORT_VERSION.to_string(),
        campaign_id: campaign_id.to_string(),
        cameras: by_slot.into_values().collect(),
    }
}

/// Canonical pretty JSON with a trailing newline.
pub fn camera_json(report: &CameraReport) -> String {
    let mut s = serde_json::to_string_pretty(report).expect("report serializes");
    s.push('\n');
    s
}

/// Parse one `[DelveCamera] …` payload. A line missing a field or carrying a
/// non-integer is skipped whole, never half-applied.
fn parse_camera_stamp(msg: &str) -> Option<CapturedCamera> {
    let idx = msg.find(STAMP)?;
    let payload = &msg[idx + STAMP.len()..];
    let (mut slot, mut eye, mut yaw, mut pitch, mut eye_in) = (None, None, None, None, None);
    for field in payload.split_whitespace() {
        let (key, val) = field.split_once('=')?;
        match key {
            "slot" => slot = Some(val.parse::<u32>().ok()?),
            "eye" => {
                let mut it = val.split(',');
                let mut axis = || it.next()?.parse::<i64>().ok();
                let (x, y, z) = (axis()?, axis()?, axis()?);
                if it.next().is_some() {
                    return None;
                }
                eye = Some([milli(x), milli(y), milli(z)]);
            }
            "yaw" => yaw = Some(centi(val.parse::<i64>().ok()?)),
            "pitch" => pitch = Some(centi(val.parse::<i64>().ok()?)),
            "in" => eye_in = Some(val.to_string()),
            _ => {}
        }
    }
    Some(CapturedCamera {
        slot: slot.filter(|s| *s >= 1)?,
        eye: eye?,
        yaw: yaw?,
        pitch: pitch?,
        eye_in: eye_in?,
        at: String::new(),
        stamps: 0,
    })
}

/// Milli-blocks → blocks. Exact to the decimal the stamp carried: the quotient
/// is the double nearest `v / 1000`, which prints as that decimal.
fn milli(v: i64) -> f64 {
    v as f64 / 1000.0
}

/// Centi-degrees → degrees.
fn centi(v: i64) -> f64 {
    v as f64 / 100.0
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
[06:13:02] [Server thread/INFO]: [Not Secure] [delve-creator] [DelveCamera] slot=1 eye=44500,83620,-12250 yaw=13550 pitch=2000 in=air
[06:13:09] [Server thread/INFO]: [Not Secure] [delve-creator] [DelveCamera] slot=2 eye=-3005,71270,2500 yaw=-4575 pitch=-1210 in=block
";

    #[test]
    fn a_stamp_is_the_pose_divided_out() {
        let r = harvest_cameras(LOG, "gallery");
        assert_eq!(r.version, "0.1.0");
        assert_eq!(r.campaign_id, "gallery");
        assert_eq!(r.cameras.len(), 2);
        let one = &r.cameras[0];
        assert_eq!(one.slot, 1);
        assert_eq!(one.eye, [44.5, 83.62, -12.25]);
        assert_eq!(one.yaw, 135.5);
        assert_eq!(one.pitch, 20.0);
        assert_eq!(one.eye_in, "air");
        assert_eq!(one.at, "06:13:02");
        assert_eq!(one.stamps, 1);
        // Negative values keep their sign on every axis and on both angles.
        let two = &r.cameras[1];
        assert_eq!(two.eye, [-3.005, 71.27, 2.5]);
        assert_eq!((two.yaw, two.pitch), (-45.75, -12.1));
        assert_eq!(two.eye_in, "block");
    }

    #[test]
    fn two_stamps_for_one_slot_keep_the_last_and_count_two() {
        let log = format!(
            "{LOG}[06:20:00] [Server thread/INFO]: [Not Secure] [delve-creator] [DelveCamera] \
             slot=1 eye=1000,64000,1000 yaw=0 pitch=0 in=air\n"
        );
        let r = harvest_cameras(&log, "gallery");
        assert_eq!(r.cameras.len(), 2);
        assert_eq!(r.cameras[0].eye, [1.0, 64.0, 1.0]);
        assert_eq!(r.cameras[0].at, "06:20:00");
        assert_eq!(r.cameras[0].stamps, 2);
        assert_eq!(r.cameras[1].stamps, 1);
    }

    #[test]
    fn a_malformed_stamp_is_skipped_whole() {
        for bad in [
            "slot=1 eye=1,2 yaw=0 pitch=0 in=air",
            "slot=1 eye=1,2,3,4 yaw=0 pitch=0 in=air",
            "slot=1 eye=1.5,2,3 yaw=0 pitch=0 in=air",
            "slot=0 eye=1,2,3 yaw=0 pitch=0 in=air",
            "slot=1 eye=1,2,3 pitch=0 in=air",
            "slot=1 eye=1,2,3 yaw=0 pitch=0",
        ] {
            let log = format!("[00:00:01] [Server thread/INFO]: [c] [DelveCamera] {bad}\n");
            assert!(harvest_cameras(&log, "x").cameras.is_empty(), "{bad}");
        }
    }

    #[test]
    fn the_report_reads_back_as_itself() {
        let r = harvest_cameras(LOG, "gallery");
        let json = camera_json(&r);
        assert!(json.starts_with("{\n") && json.ends_with("}\n"));
        assert!(json.contains("\"in\": \"air\""));
        let back: CameraReport = serde_json::from_str(&json).unwrap();
        assert_eq!(back, r);
    }
}
