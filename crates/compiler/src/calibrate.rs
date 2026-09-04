//! `delvec calibrate` (spec-0019 §4): turn a harvested `rehearsal-report.json`
//! back into the DSL's own vocabulary — `anchor + integer offset` — as a
//! ready-to-apply JSON patch per shot.
//!
//! ## Why this exists
//!
//! The game hands back **world cells**; the DSL only ever speaks `anchor +
//! offset` (spec-0019 §5: no free-floating world coordinates, ever). This
//! converter is the only place that translation happens, and it is deliberately
//! *not* allowed to invent an anchor: a proposal with no declared anchor within
//! [`SNAP_RADIUS`] is **reported, never snapped** (`DW0390`) — the fix is a
//! prefab-metadata anchor, which is a content decision, not a compiler one.
//!
//! ## Why the snap is lossless
//!
//! A proposal is authored at block-cell granularity end to end (see
//! [`crate::rehearsal`]), and a resolved anchor is a block cell too, so
//! `offset = cell − anchor` is exact integer arithmetic: the patched DSL
//! resolves back to the very cell the creator marked. The `distance` this
//! converter prints is therefore *how far the offset reaches*, not an error
//! term — the error is identically zero, and that is a property worth stating
//! rather than a number worth printing.
//!
//! The patch is never applied here: nothing writes to a stage document from the
//! game (spec-0019 §4). The agent applies it, reruns `delvec build`, and the
//! normal proofs (`DW0308` air corridors, `DW0347` angular budget) gate it
//! exactly as they gate a hand-written shot.

use delvewright_dsl::{DwCode, ExitTier};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// A harvested proposal names no declared anchor within [`SNAP_RADIUS`], so it
/// cannot be expressed in the DSL at all.
pub const DW_SHOT_UNSNAPPABLE: DwCode = DwCode::every_version("DW0390", ExitTier::Build);
/// The rehearsal report and the layout manifest describe different campaigns —
/// calibrating one build's proposals against another build's anchors.
pub const DW_SHOT_CAMPAIGN_MISMATCH: DwCode = DwCode::every_version("DW0391", ExitTier::Build);
/// The rehearsal report is unreadable, is not a rehearsal report, or carries a
/// schema version this `delvec` does not understand.
pub const DW_SHOT_REPORT_INVALID: DwCode = DwCode::every_version("DW0392", ExitTier::Build);

/// How far a proposal may sit from an anchor and still be expressed as an
/// offset from it (blocks, spec-0019 §5). Beyond this the offset stops being a
/// readable authoring statement — "12 blocks north-east of the fire pit" is a
/// place, "41 blocks from the only anchor in the campaign" is a coordinate in
/// disguise.
pub const SNAP_RADIUS: f64 = 16.0;

/// The patch document's schema version.
pub const PATCH_VERSION: &str = "0.1.0";

// ---------------------------------------------------------------------------
// Inputs
// ---------------------------------------------------------------------------

/// One resolved anchor from the overlay's `layout.json`.
#[derive(Debug, Clone, Deserialize)]
pub struct Anchor {
    /// Anchor id (`anchor/…`).
    pub id: String,
    /// The area it belongs to (`area/…`).
    pub area: String,
    /// The absolute cell it resolved to in the assembled world.
    pub pos: [i64; 3],
}

/// The subset of `creator-datapack/layout.json` this converter reads. Unknown
/// fields are ignored so the manifest can keep growing.
#[derive(Debug, Clone, Deserialize)]
pub struct LayoutAnchors {
    /// Campaign id, checked against the report's (`DW0391`).
    pub campaign_id: String,
    /// The resolved-anchor vocabulary.
    #[serde(default)]
    pub anchors: Vec<Anchor>,
}

/// One shot proposal from `rehearsal-report.json`.
#[derive(Debug, Clone, Deserialize)]
pub struct ShotProposal {
    /// 1-based shot id.
    pub shot: u32,
    /// 1-based beat id.
    #[serde(default)]
    pub beat: u32,
    /// JSON pointer to the `cutscene` effect in the `quests` stage document.
    pub pointer: String,
    /// The shot's 0-based index within that effect.
    #[serde(default)]
    pub shot_index: u32,
    /// Camera waypoints as absolute cells.
    pub path: Vec<[i64; 3]>,
    /// Look target cell, if the proposal names one.
    #[serde(default)]
    pub look_at: Option<[i64; 3]>,
    /// Shot duration in seconds.
    pub seconds: u32,
}

/// The `rehearsal-report.json` document.
#[derive(Debug, Clone, Deserialize)]
pub struct RehearsalReport {
    /// Schema version.
    pub version: String,
    /// Campaign the proposals belong to.
    pub campaign_id: String,
    /// Every stamped shot.
    pub shots: Vec<ShotProposal>,
}

// ---------------------------------------------------------------------------
// Outputs
// ---------------------------------------------------------------------------

/// One `anchor + offset` snap.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Snap {
    /// The anchor the offset is measured from.
    pub anchor: String,
    /// Integer block offset. Exact: `cell − anchor`.
    pub offset: [i64; 3],
    /// Distance from the anchor to the cell, in blocks (how far the offset
    /// reaches — NOT an error term; the snap is lossless).
    pub distance: f64,
}

/// A proposal cell that no anchor can express.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Unsnappable {
    /// Shot id.
    pub shot: u32,
    /// `path` or `look_at`.
    pub kind: String,
    /// Waypoint index within `path` (`0` for a `look_at`).
    pub index: usize,
    /// The cell that could not be expressed.
    pub cell: [i64; 3],
    /// The closest declared anchor and how far away it is.
    pub nearest: Option<Snap>,
}

/// The calibration result: patches for what snapped, a report for what did not.
#[derive(Debug, Clone, PartialEq)]
pub struct Calibration {
    /// One patch per fully-snappable shot, in shot order.
    pub patches: Vec<serde_json::Value>,
    /// Every cell that no anchor can express.
    pub unsnappable: Vec<Unsnappable>,
    /// Campaign id, carried through.
    pub campaign_id: String,
}

impl Calibration {
    /// The patch document, canonical pretty JSON with a trailing newline.
    pub fn to_json(&self) -> Vec<u8> {
        let doc = json!({
            "version": PATCH_VERSION,
            "campaign_id": self.campaign_id,
            "snap_radius": SNAP_RADIUS,
            // Stated, not measured: a proposal and an anchor are both block
            // cells, so `offset = cell - anchor` is exact.
            "snap_error": 0,
            "patches": self.patches,
            "unsnappable": self.unsnappable,
        });
        let mut bytes = serde_json::to_vec_pretty(&doc).expect("patch serializes");
        bytes.push(b'\n');
        bytes
    }
}

/// Snap `cell` to the nearest declared anchor. Ties break on anchor id, so the
/// result is a pure function of the inputs (ADR-0006). `None` only when there
/// are no anchors at all.
pub fn nearest(cell: [i64; 3], anchors: &[Anchor]) -> Option<Snap> {
    anchors
        .iter()
        .map(|a| {
            let offset = [cell[0] - a.pos[0], cell[1] - a.pos[1], cell[2] - a.pos[2]];
            let d2 = (offset[0] * offset[0] + offset[1] * offset[1] + offset[2] * offset[2]) as f64;
            Snap {
                anchor: a.id.clone(),
                offset,
                distance: round3(d2.sqrt()),
            }
        })
        .min_by(|a, b| {
            a.distance
                .partial_cmp(&b.distance)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.anchor.cmp(&b.anchor))
        })
}

/// Snap `cell` only if a declared anchor is within [`SNAP_RADIUS`].
fn snap_within(cell: [i64; 3], anchors: &[Anchor]) -> Result<Snap, Option<Snap>> {
    match nearest(cell, anchors) {
        Some(s) if s.distance <= SNAP_RADIUS => Ok(s),
        other => Err(other),
    }
}

/// Convert a harvested report into per-shot DSL patches.
///
/// A shot is patched only when **every** cell it names snaps; a shot with any
/// un-snappable cell is reported whole (a half-patched dolly would fly a path
/// no one authored).
pub fn calibrate(report: &RehearsalReport, layout: &LayoutAnchors) -> Calibration {
    let mut patches = Vec::new();
    let mut unsnappable = Vec::new();
    for shot in &report.shots {
        let mut path = Vec::new();
        let mut failed = false;
        for (i, cell) in shot.path.iter().enumerate() {
            match snap_within(*cell, &layout.anchors) {
                Ok(s) => path.push(s),
                Err(nearest) => {
                    failed = true;
                    unsnappable.push(Unsnappable {
                        shot: shot.shot,
                        kind: "path".to_string(),
                        index: i,
                        cell: *cell,
                        nearest,
                    });
                }
            }
        }
        let mut look = None;
        if let Some(cell) = shot.look_at {
            match snap_within(cell, &layout.anchors) {
                Ok(s) => look = Some(s),
                Err(nearest) => {
                    failed = true;
                    unsnappable.push(Unsnappable {
                        shot: shot.shot,
                        kind: "look_at".to_string(),
                        index: 0,
                        cell,
                        nearest,
                    });
                }
            }
        }
        if failed {
            continue;
        }
        let mut patch = json!({
            "path": path.iter().map(waypoint).collect::<Vec<_>>(),
            "seconds": shot.seconds,
        });
        if let Some(l) = &look {
            patch["look_at"] = waypoint(l);
        }
        patches.push(json!({
            "shot": shot.shot,
            "beat": shot.beat,
            "pointer": shot.pointer,
            "shot_index": shot.shot_index,
            "patch": patch,
            "snaps": path,
            "look_at_snap": look,
        }));
    }
    Calibration {
        patches,
        unsnappable,
        campaign_id: report.campaign_id.clone(),
    }
}

/// A `Snap` as the DSL spells a camera waypoint / look target. A zero offset is
/// omitted, matching the DSL's own `skip_serializing_if` default.
fn waypoint(s: &Snap) -> serde_json::Value {
    if s.offset == [0, 0, 0] {
        json!({ "anchor": s.anchor })
    } else {
        json!({ "anchor": s.anchor, "offset": s.offset })
    }
}

/// Round to 3 decimals — the emission-wide float policy, so a printed distance
/// is byte-stable across platforms (ADR-0006).
fn round3(v: f64) -> f64 {
    let r = (v * 1000.0).round() / 1000.0;
    if r == 0.0 { 0.0 } else { r }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn anchors() -> Vec<Anchor> {
        vec![
            Anchor {
                id: "anchor/fire-pit".to_string(),
                area: "area/island".to_string(),
                pos: [9, 69, -56],
            },
            Anchor {
                id: "anchor/exit".to_string(),
                area: "area/island".to_string(),
                pos: [5, 67, 8],
            },
        ]
    }

    fn layout() -> LayoutAnchors {
        LayoutAnchors {
            campaign_id: "island".to_string(),
            anchors: anchors(),
        }
    }

    fn report(shots: Vec<ShotProposal>) -> RehearsalReport {
        RehearsalReport {
            version: "0.1.0".to_string(),
            campaign_id: "island".to_string(),
            shots,
        }
    }

    /// The round-trip property (spec-0019 acceptance): a proposal at a known
    /// world cell yields `anchor + offset` that resolves back to that exact
    /// cell — the snap is lossless, not approximate.
    #[test]
    fn snapping_round_trips_to_the_same_cell() {
        for cell in [[9, 69, -56], [12, 70, -50], [5, 67, 8], [-1, 64, 12]] {
            let s = snap_within(cell, &anchors()).expect("within radius");
            let a = anchors()
                .into_iter()
                .find(|a| a.id == s.anchor)
                .expect("named anchor exists");
            assert_eq!(
                [
                    a.pos[0] + s.offset[0],
                    a.pos[1] + s.offset[1],
                    a.pos[2] + s.offset[2]
                ],
                cell,
                "anchor + offset must resolve back to the marked cell"
            );
        }
    }

    /// The nearest anchor wins, and the reported distance is the real one.
    #[test]
    fn nearest_anchor_wins() {
        let s = snap_within([6, 67, 7], &anchors()).expect("within radius");
        assert_eq!(s.anchor, "anchor/exit");
        assert_eq!(s.offset, [1, 0, -1]);
        assert_eq!(s.distance, round3(2.0_f64.sqrt()));
    }

    /// `seconds` survives the round trip untouched — the creator converged it by
    /// watching, and nothing downstream may re-derive it.
    #[test]
    fn seconds_survive_the_round_trip() {
        let c = calibrate(
            &report(vec![ShotProposal {
                shot: 1,
                beat: 1,
                pointer: "/p".to_string(),
                shot_index: 0,
                path: vec![[6, 67, 7]],
                look_at: Some([9, 69, -56]),
                seconds: 11,
            }]),
            &layout(),
        );
        assert_eq!(c.patches.len(), 1);
        assert_eq!(c.patches[0]["patch"]["seconds"], 11);
        // A zero offset is spelled as a bare anchor, like the DSL does.
        assert_eq!(
            c.patches[0]["patch"]["look_at"],
            json!({ "anchor": "anchor/fire-pit" })
        );
    }

    /// Beyond the snap radius nothing is invented: the cell is REPORTED with the
    /// anchor it was closest to, and the shot produces no patch at all (spec-0019
    /// §5 — the fix is a prefab-metadata anchor, not a raw coordinate).
    #[test]
    fn a_far_proposal_is_reported_never_snapped() {
        let c = calibrate(
            &report(vec![ShotProposal {
                shot: 2,
                beat: 1,
                pointer: "/p".to_string(),
                shot_index: 0,
                path: vec![[6, 67, 7], [400, 90, 400]],
                look_at: None,
                seconds: 5,
            }]),
            &layout(),
        );
        assert!(c.patches.is_empty(), "no half-patched dolly");
        assert_eq!(c.unsnappable.len(), 1);
        let u = &c.unsnappable[0];
        assert_eq!(u.shot, 2);
        assert_eq!(u.kind, "path");
        assert_eq!(u.index, 1);
        assert_eq!(u.cell, [400, 90, 400]);
        assert!(u.nearest.as_ref().unwrap().distance > SNAP_RADIUS);
    }

    /// An un-snappable `look_at` is reported in its own right — an aim that
    /// cannot be expressed is exactly the defect this loop exists to surface.
    #[test]
    fn an_unsnappable_look_at_is_reported() {
        let c = calibrate(
            &report(vec![ShotProposal {
                shot: 1,
                beat: 1,
                pointer: "/p".to_string(),
                shot_index: 0,
                path: vec![[6, 67, 7]],
                look_at: Some([-900, 64, 900]),
                seconds: 5,
            }]),
            &layout(),
        );
        assert!(c.patches.is_empty());
        assert_eq!(c.unsnappable[0].kind, "look_at");
    }

    /// Ties break on anchor id, so the same report always yields the same patch.
    #[test]
    fn ties_break_deterministically() {
        let a = vec![
            Anchor {
                id: "anchor/b".to_string(),
                area: "a".to_string(),
                pos: [0, 64, 0],
            },
            Anchor {
                id: "anchor/a".to_string(),
                area: "a".to_string(),
                pos: [4, 64, 0],
            },
        ];
        let s = nearest([2, 64, 0], &a).unwrap();
        assert_eq!(s.anchor, "anchor/a");
    }

    /// The patch document is canonical pretty JSON with a trailing newline.
    #[test]
    fn patch_json_is_canonical() {
        let c = calibrate(&report(Vec::new()), &layout());
        let bytes = c.to_json();
        assert!(bytes.ends_with(b"}\n"));
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["version"], PATCH_VERSION);
        assert_eq!(v["snap_radius"], SNAP_RADIUS);
        assert_eq!(v["snap_error"], 0);
    }
}
