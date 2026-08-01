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

use serde::{Deserialize, Serialize};

use crate::diag::{DW_INPUT, Diagnostic};
use crate::scene::scene_name;

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
}

#[derive(Debug, Serialize)]
struct IndexEntry {
    id: String,
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    leg: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    objective: Option<String>,
    /// The image filename a renderer produces for this shot (matches the Chunky
    /// scene name; `delve-render scene` writes `<image>` beside the scene JSON).
    image: String,
    expect: Vec<String>,
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
        .map(|s| IndexEntry {
            id: s.id.clone(),
            kind: s.kind.clone(),
            leg: s.leg,
            objective: s.objective.clone(),
            image: format!("{}.png", scene_name(&s.id)),
            expect: s.expect.clone(),
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
    use crate::diag::DW_INPUT;

    const MINI: &[u8] = include_bytes!("../tests/fixtures/render-plan-mini.json");
    const POV: &[u8] = include_bytes!("../tests/fixtures/render-plan-pov.json");

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
        assert_eq!(pov["image"], "pov_leg0_wp0.png");
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
}
