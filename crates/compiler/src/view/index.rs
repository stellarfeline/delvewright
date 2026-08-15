//! Shot index emission (spec-0003 visual tier, ladder integration).
//!
//! Pairs every `render-plan.json` shot with its `expect` checklist and the image
//! filename a renderer produces, so a reviewing agent / vision model is handed
//! **(image ↔ expect) pairs** — the deliverable of the visual tier. The review step
//! itself stays agent-driven (no vision-model call is wired into CI); this module
//! only produces the contract the reviewer consumes.
//!
//! Order and content mirror `render-plan.json` exactly (shots in plan order), and
//! the emitter is a pure byte-deterministic function (fixed field order, 2-space
//! pretty, trailing newline), so the index rides the determinism gate like the plan
//! it derives from.
//!
//! Shots stamped dark-with-night-vision (see [`crate::view::scene`]'s REVIEW POLICY)
//! additionally carry `review_policy` = [`crate::view::scene::REVIEW_POLICY`] and their
//! `lighting` stamp, so the reviewer knows those images are night-vision
//! **emulations** (legibility ground truth, not lighting ground truth). Both
//! fields are absent — not null — everywhere else, keeping indexes for
//! declaration-free campaigns byte-identical.

use serde::{Deserialize, Serialize};

use crate::view::diag::{DW_INPUT, Diagnostic};
use crate::view::scene::{LightingStamp, REVIEW_POLICY, needs_emulation, scene_file_stem};

#[derive(Debug, Deserialize)]
struct RenderPlan {
    campaign_id: String,
    shots: Vec<Shot>,
}

#[derive(Debug, Deserialize)]
struct Shot {
    id: String,
    kind: String,
    /// Critical-path leg index (player-POV shots only).
    #[serde(default)]
    leg: Option<u32>,
    /// The objective the shot serves (player-POV shots; may be null).
    #[serde(default)]
    objective: Option<String>,
    /// The machine-generated expect checklist; the first entry of a POV shot is the
    /// one-sentence first-person description.
    #[serde(default)]
    expect: Vec<String>,
    /// The compiler's declaration-derived lighting stamp, if any (raw
    /// passthrough for the entry, plus the emulation predicate).
    #[serde(default)]
    lighting: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct IndexEntry {
    id: String,
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    leg: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    objective: Option<String>,
    /// The image filename a renderer produces for this shot: the scene's own
    /// name (`crate::view::scene::scene_file_stem`) with a `.png` extension, so the
    /// scene JSON, its Chunky caches and this image all share one stem.
    image: String,
    expect: Vec<String>,
    /// The shot's `lighting` stamp, passed through verbatim from the plan
    /// (absent for shots of undeclared areas — index bytes unchanged for them).
    #[serde(skip_serializing_if = "Option::is_none")]
    lighting: Option<serde_json::Value>,
    /// [`REVIEW_POLICY`] for shots whose scene `delvec scene` emulates
    /// (dark-with-night-vision stamp): tells the reviewing agent/vision model
    /// the image approximates the night-vision player view — judge layout and
    /// readability from it, never the world's real lighting.
    #[serde(skip_serializing_if = "Option::is_none")]
    review_policy: Option<&'static str>,
}

#[derive(Debug, Serialize)]
struct ShotIndex {
    campaign_id: String,
    count: usize,
    shots: Vec<IndexEntry>,
}

/// Build the shot index bytes from a `render-plan.json`. `DW0721` on malformed
/// input.
pub fn index_from_plan(plan_json: &[u8]) -> Result<Vec<u8>, Diagnostic> {
    let plan: RenderPlan = serde_json::from_slice(plan_json)
        .map_err(|e| Diagnostic::error(DW_INPUT, format!("parse render-plan.json: {e}")))?;
    let shots: Vec<IndexEntry> = plan
        .shots
        .iter()
        .map(|s| {
            // The same predicate `delvec scene` applies, over the same
            // stamp — index and scene can never disagree about which shots are
            // emulated.
            let stamp: Option<LightingStamp> = s
                .lighting
                .clone()
                .and_then(|v| serde_json::from_value(v).ok());
            let emulated = needs_emulation(stamp.as_ref());
            IndexEntry {
                id: s.id.clone(),
                kind: s.kind.clone(),
                leg: s.leg,
                objective: s.objective.clone(),
                image: format!("{}.png", scene_file_stem(&plan.campaign_id, &s.id)),
                expect: s.expect.clone(),
                lighting: s.lighting.clone(),
                review_policy: emulated.then_some(REVIEW_POLICY),
            }
        })
        .collect();
    let idx = ShotIndex {
        campaign_id: plan.campaign_id,
        count: shots.len(),
        shots,
    };
    let mut bytes = serde_json::to_vec_pretty(&idx)
        .map_err(|e| Diagnostic::error(DW_INPUT, format!("serialize shot index: {e}")))?;
    bytes.push(b'\n');
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::diag::DW_INPUT;

    const MINI: &[u8] = include_bytes!("../../tests/fixtures/view/render-plan-mini.json");
    const POV: &[u8] = include_bytes!("../../tests/fixtures/view/render-plan-pov.json");

    #[test]
    fn malformed_plan_is_dw0721() {
        let err = index_from_plan(b"nope").unwrap_err();
        assert_eq!(err.code, DW_INPUT);
    }

    #[test]
    fn pairs_every_shot_with_an_image_and_expect() {
        let out = index_from_plan(MINI).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["campaign_id"], "mini");
        let shots = v["shots"].as_array().unwrap();
        assert_eq!(v["count"].as_u64().unwrap() as usize, shots.len());
        // Order mirrors the plan; every entry has a `.png` image and its expects.
        assert_eq!(shots[0]["id"], "spawn");
        for s in shots {
            assert!(
                s["image"].as_str().unwrap().ends_with(".png"),
                "image is a png filename"
            );
            assert!(
                !s["image"].as_str().unwrap().contains('/'),
                "path-safe name"
            );
            assert!(!s["expect"].as_array().unwrap().is_empty());
        }
    }

    #[test]
    fn pov_shots_carry_leg_and_objective() {
        let out = index_from_plan(POV).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        let pov = v["shots"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["kind"] == "pov")
            .expect("a pov shot");
        assert!(pov["leg"].is_number(), "POV entry keeps its leg index");
        assert_eq!(pov["objective"], "obj/exit");
        // image stem == the Chunky scene name (campaign-qualified).
        assert_eq!(pov["image"], "hello-world_pov_leg0_wp0.png");
        assert!(
            pov["expect"][0]
                .as_str()
                .unwrap()
                .starts_with("First-person view")
        );
    }

    #[test]
    fn index_is_deterministic() {
        assert_eq!(index_from_plan(POV).unwrap(), index_from_plan(POV).unwrap());
    }

    #[test]
    fn emulated_shots_are_marked_and_others_untouched() {
        let plan = br#"{"campaign_id":"cave","shots":[
          {"id":"pov/leg0/wp0","kind":"pov","leg":0,
           "lighting":{"profile":"dark","mitigation":"night-vision"},
           "expect":["First-person view walking east."]},
          {"id":"interior/cave/0","kind":"interior",
           "lighting":{"profile":"lit"},
           "expect":["room interior assembled"]},
          {"id":"spawn","kind":"spawn","expect":["spawn point clear"]}
        ]}"#;
        let v: serde_json::Value = serde_json::from_slice(&index_from_plan(plan).unwrap()).unwrap();
        let shots = v["shots"].as_array().unwrap();
        // Dark + night-vision → marked, stamp passed through.
        assert_eq!(
            shots[0]["review_policy"],
            crate::view::scene::REVIEW_POLICY,
            "emulated shot carries the review marker"
        );
        assert_eq!(shots[0]["lighting"]["profile"], "dark");
        assert_eq!(shots[0]["lighting"]["mitigation"], "night-vision");
        // Lit → stamp passed through, but never the emulation marker.
        assert_eq!(shots[1]["lighting"]["profile"], "lit");
        assert!(shots[1].get("review_policy").is_none());
        // Unstamped → neither key exists (byte-identical for old campaigns —
        // the POV fixture proves the same across the whole index).
        assert!(shots[2].get("lighting").is_none());
        assert!(shots[2].get("review_policy").is_none());
        let unstamped: serde_json::Value =
            serde_json::from_slice(&index_from_plan(POV).unwrap()).unwrap();
        for s in unstamped["shots"].as_array().unwrap() {
            assert!(s.get("lighting").is_none() && s.get("review_policy").is_none());
        }
    }
}
