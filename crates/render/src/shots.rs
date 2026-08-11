//! Per-piece shot planner for `delve-render piece`.
//!
//! Nucleation is an **orbit / turntable** renderer: `CameraConfig` fits the
//! camera to the model bounds and (optionally) aims at a `target` point — it does
//! not place a free camera inside a room. So the per-piece set is:
//!
//! - **4 exterior corner-isometric** shots (yaw 45/135/225/315, pitch 30) of the
//!   full schematic — Nucleation's strength.
//! - **3 section** shots, each a [`Cutaway`] read from the side the material was
//!   taken off: `top` (the dollhouse, `y-max:1`), `plan-mid` (the plan section
//!   at mid height, `y-max:50%`) and `sec-x` / `sec-z` (the elevation sections,
//!   `x-min:50%` / `z-min:50%`).
//! - **interior doorway** shots: one per socket, aimed (`target`) at a point
//!   just inside the socket, orbit-yaw set so the camera sits on the socket's
//!   outward side and looks through the opening.
//! - **anchor** shots (when metadata provides anchors): one per anchor, aimed at
//!   the anchor position.
//!
//! # Why three sections and not one stripped layer
//!
//! The plan used to be four exteriors and a single `top` on a one-layer
//! cutaway. For a small roofed room that *is* the floor plan. For a piece ten or
//! more courses tall carved out of solid mass it is a picture of the rock: the
//! layer under the roof is more rock, so a parameter that moves an interior wall
//! moved 0–0.3% of the frame on every planned shot (measured, PR #372) — the
//! sheet could show a zone's bounding box and never its massing.
//!
//! `top` is unchanged, because a small room's dollhouse is still the right
//! picture of a small room. What the plan adds is the two questions a one-layer
//! strip cannot ask of a tall body: cut it at half height and look down
//! (`plan-mid` — interior walls in plan), and halve it and look at the cut face
//! (`sec-x` / `sec-z` — storeys, floor levels, wall thickness). All three are
//! the *same* mechanism at different settings, and the camera for each is
//! derived by [`Cutaway::viewpoint`] rather than restated here.
//!
//! # Why the aimed shots cut down to what they aim at
//!
//! A doorway or anchor shot names a point it is about. Everything above that
//! point is, by construction, between the orbit camera and the subject, so the
//! shot's cutaway is "take the solid above the target away"
//! ([`aimed_cutaway`]). In a five-tall room that resolves to the one or two
//! layers the old boolean stripped; in a fourteen-tall tower it is the eleven
//! that were hiding the anchor. Nothing new was needed for this — it is the
//! existing mechanism, given the reach it was missing.
//!
//! True free-camera interior shots (camera literally standing at the doorway) are
//! the **Chunky scene path** (`delve-render scene`), which places cameras anywhere
//! in the built world. These per-piece cutaways are the fast per-prefab
//! approximation for authoring-time review of massing, floor plan, anchor
//! placement, and lighting. All are validation artifacts, never shipped.

use crate::cutaway::{Clip, Cutaway, Face};
use crate::diag::{DW_CUTAWAY_EMPTY, Diagnostic};
use crate::meta::PrefabMeta;

/// One planned per-piece shot.
#[derive(Debug, Clone, PartialEq)]
pub struct PieceShot {
    /// Output file stem (e.g. `ext-ne`, `top`, `sec-z`, `door-0`,
    /// `anchor-objective`).
    pub name: String,
    /// Orbit yaw / pitch (degrees), Nucleation `CameraConfig` convention:
    /// `dir = -[cos(pitch)·sin(yaw), sin(pitch), cos(pitch)·cos(yaw)]`.
    pub yaw_deg: f32,
    pub pitch_deg: f32,
    /// Zoom (>1 pushes the camera closer).
    pub zoom: f32,
    /// Explicit orbit target (interior/anchor shots); `None` uses the model
    /// centroid (exterior/section).
    pub target: Option<[f32; 3]>,
    /// Which solid the viewer is inside for this shot: the material removed
    /// before meshing. The shot owns this — not the piece, not the sheet.
    pub cutaway: Cutaway,
}

/// Nucleation orbit yaw (degrees) that puts the camera on the outward side of a
/// socket with the given `facing`, looking through it into the room. Derived from
/// Nucleation's view-direction formula (see [`PieceShot`]): at pitch 0,
/// `[sin(yaw), cos(yaw)] == facing_xz`.
fn facing_to_orbit_yaw(facing: &str) -> f32 {
    match facing {
        "south" => 0.0,
        "east" => 90.0,
        "north" => 180.0,
        "west" => 270.0,
        _ => 0.0,
    }
}

fn centre_of(pos: [i32; 3]) -> [f32; 3] {
    [
        pos[0] as f32 + 0.5,
        pos[1] as f32 + 0.5,
        pos[2] as f32 + 0.5,
    ]
}

/// The cutaway for a shot aimed at `target` in a model of `size`: take away
/// everything above the aimed point, which is exactly the material an orbit
/// camera would have to see through.
///
/// Never zero — a shot that asked for an interior view and got none would be a
/// silently exterior picture — and never the whole model, so an anchor sitting
/// on the top course still leaves its own layer standing.
pub fn aimed_cutaway(target: [f32; 3], size: [i32; 3]) -> Cutaway {
    let y = target[1].floor() as i32;
    // Layers strictly above the target cell, clamped into `1..=size_y-1`.
    let above = (size[1] - y - 1).clamp(1, (size[1] - 1).max(1));
    Cutaway::of(Clip::layers(Face::YMax, above as u32))
}

/// A section shot: the cut, framed from where the material was removed.
fn section(name: &str, spec: &str, size: [i32; 3]) -> PieceShot {
    let cutaway: Cutaway = spec.parse().expect("built-in section spec parses");
    let (yaw, pitch) = cutaway
        .viewpoint(size)
        .expect("a built-in section removes something");
    PieceShot {
        name: name.to_string(),
        yaw_deg: yaw,
        pitch_deg: pitch,
        zoom: 1.0,
        target: None,
        cutaway,
    }
}

/// Plan the deterministic shot set for a piece of `size`, using `meta` when
/// present (sockets → doorway shots, anchors → anchor shots).
pub fn plan_piece(size: [i32; 3], meta: Option<&PrefabMeta>) -> Vec<PieceShot> {
    let mut shots = Vec::new();

    // 4 exterior corner-isometric — the whole body, nothing removed.
    for (name, yaw) in [
        ("ext-ne", 45.0),
        ("ext-se", 135.0),
        ("ext-sw", 225.0),
        ("ext-nw", 315.0),
    ] {
        shots.push(PieceShot {
            name: name.to_string(),
            yaw_deg: yaw,
            pitch_deg: 30.0,
            zoom: 1.0,
            target: None,
            cutaway: Cutaway::none(),
        });
    }

    // The three sections: the dollhouse, the plan cut, and the two elevations.
    shots.push(section("top", "y-max:1", size));
    shots.push(section("plan-mid", "y-max:50%", size));
    shots.push(section("sec-x", "x-min:50%", size));
    shots.push(section("sec-z", "z-min:50%", size));

    if let Some(meta) = meta {
        // Interior doorway shots, one per socket (connector order).
        for (i, c) in meta.connectors.iter().enumerate() {
            // Aim a couple of blocks inside the socket (opposite its outward facing).
            let inward = match c.facing.as_str() {
                "north" => [0.0, 0.0, 2.0],
                "south" => [0.0, 0.0, -2.0],
                "east" => [-2.0, 0.0, 0.0],
                "west" => [2.0, 0.0, 0.0],
                _ => [0.0, 0.0, 0.0],
            };
            let base = centre_of(c.local_pos);
            let target = [base[0] + inward[0], base[1] + 1.0, base[2] + inward[2]];
            shots.push(PieceShot {
                name: format!("door-{i}"),
                yaw_deg: facing_to_orbit_yaw(&c.facing),
                pitch_deg: 20.0,
                zoom: 1.6,
                target: Some(target),
                cutaway: aimed_cutaway(target, size),
            });
        }

        // Anchor shots (sorted by name for determinism). Gate anchors use the
        // region centre; point anchors use their position.
        for (name, a) in &meta.anchors {
            let target = if let Some(p) = a.pos {
                Some(centre_of(p))
            } else {
                a.region.as_ref().map(|r| {
                    [
                        (r.from[0] + r.to[0]) as f32 / 2.0 + 0.5,
                        (r.from[1] + r.to[1]) as f32 / 2.0 + 1.0,
                        (r.from[2] + r.to[2]) as f32 / 2.0 + 0.5,
                    ]
                })
            };
            let Some(target) = target else { continue };
            shots.push(PieceShot {
                name: format!(
                    "anchor-{}",
                    name.rsplit('/')
                        .next()
                        .unwrap_or(name)
                        .replace(['-', '.'], "_")
                ),
                yaw_deg: 45.0,
                pitch_deg: 55.0,
                zoom: 1.4,
                target: Some(target),
                cutaway: aimed_cutaway(target, size),
            });
        }
    }

    shots
}

/// Refuse any shot whose cutaway keeps nothing of a model of `size`.
///
/// An empty frame is indistinguishable from an empty body: the reviewer would
/// read "there is nothing here" where the truth is "you asked for nothing".
/// Exact arithmetic ([`Cutaway::kept_box`]), so this cannot be a sampling miss,
/// and it runs before a pixel — or a GPU — is touched.
pub fn refuse_empty_cuts(plan: &[PieceShot], size: [i32; 3]) -> Option<Diagnostic> {
    for shot in plan {
        if !shot.cutaway.is_empty() && shot.cutaway.kept_box(size).is_none() {
            return Some(Diagnostic::error(
                DW_CUTAWAY_EMPTY,
                format!(
                    "shot `{}`: cutaway `{}` removes the whole {}×{}×{} model — the frame \
                     would be empty. Cut less, or cut from another face",
                    shot.name, shot.cutaway, size[0], size[1], size[2]
                ),
            ));
        }
    }
    None
}

/// The extra shot `--cutaway <spec>` asks for: an arbitrary cut, framed by the
/// same derivation the planned sections use, so a hand-asked section is shot
/// exactly like a planned one.
pub fn custom_shot(cutaway: Cutaway, size: [i32; 3]) -> PieceShot {
    let (yaw, pitch) = cutaway.viewpoint(size).unwrap_or((45.0, 30.0));
    PieceShot {
        name: "cut".to_string(),
        yaw_deg: yaw,
        pitch_deg: pitch,
        zoom: 1.0,
        target: None,
        cutaway,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exterior_and_sections_without_metadata() {
        let shots = plan_piece([7, 5, 9], None);
        let names: Vec<&str> = shots.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "ext-ne", "ext-se", "ext-sw", "ext-nw", "top", "plan-mid", "sec-x", "sec-z"
            ]
        );
        // The exteriors are the whole body; every section removes something.
        for s in shots.iter().filter(|s| s.name.starts_with("ext-")) {
            assert!(s.cutaway.is_empty(), "{} must show the whole body", s.name);
        }
        for s in shots.iter().filter(|s| !s.name.starts_with("ext-")) {
            assert!(!s.cutaway.is_empty(), "{} must cut", s.name);
        }
    }

    #[test]
    fn the_historical_top_shot_is_unchanged() {
        // `top` was yaw 0 / pitch 90 on a one-layer strip before the cutaway
        // became a parameter, and it still is — the general form reproduces the
        // special case rather than replacing its picture.
        let top = plan_piece([7, 5, 9], None)
            .into_iter()
            .find(|s| s.name == "top")
            .unwrap();
        assert_eq!(top.yaw_deg, 0.0);
        assert_eq!(top.pitch_deg, 90.0);
        assert_eq!(top.cutaway.to_string(), "y-max:1");
    }

    #[test]
    fn the_sections_are_the_cuts_a_tall_solid_needs() {
        let shots = plan_piece([20, 10, 84], None);
        let by = |n: &str| shots.iter().find(|s| s.name == n).unwrap().clone();
        // The plan cut goes half way down, not one layer: in a ten-course body
        // one layer is the roof and nine are still rock.
        let plan = by("plan-mid");
        assert_eq!(plan.cutaway.to_string(), "y-max:50%");
        assert_eq!(plan.cutaway.kept_box([20, 10, 84]).unwrap().1[1], 5);
        // The elevations halve the body and stand where the half went.
        let sx = by("sec-x");
        assert_eq!(sx.cutaway.to_string(), "x-min:50%");
        assert_eq!(sx.yaw_deg, 270.0);
        let sz = by("sec-z");
        assert_eq!(sz.cutaway.to_string(), "z-min:50%");
        assert_eq!(sz.yaw_deg, 180.0);
    }

    #[test]
    fn sockets_and_anchors_add_interior_shots() {
        let meta: PrefabMeta = serde_json::from_slice(
            br#"{
                "anchors": { "anchor/gate": { "region": { "from": [2,1,4], "to": [4,3,4] } } },
                "connectors": [
                    { "name": "s", "local_pos": [3,1,0], "facing": "north", "opening": [3,3] },
                    { "name": "s", "local_pos": [3,1,8], "facing": "south", "opening": [3,3] }
                ]
            }"#,
        )
        .unwrap();
        let shots = plan_piece([7, 5, 9], Some(&meta));
        let names: Vec<String> = shots.iter().map(|s| s.name.clone()).collect();
        assert!(names.contains(&"door-0".to_string()));
        assert!(names.contains(&"door-1".to_string()));
        assert!(names.contains(&"anchor-gate".to_string()));
        // Every aimed shot cuts, and cuts to what it aims at.
        for s in shots
            .iter()
            .filter(|s| s.name.starts_with("door") || s.name.starts_with("anchor"))
        {
            assert!(!s.cutaway.is_empty() && s.target.is_some());
        }
        // north socket → orbit yaw 180 (camera on the outward/−Z side, looking in).
        assert_eq!(
            shots.iter().find(|s| s.name == "door-0").unwrap().yaw_deg,
            180.0
        );
    }

    #[test]
    fn an_aimed_shot_cuts_down_to_what_it_aims_at() {
        // Five tall, aimed at y≈3 → the one layer above it: the old behaviour,
        // arrived at rather than hard-coded.
        assert_eq!(
            aimed_cutaway([3.5, 3.0, 4.5], [7, 5, 9]).to_string(),
            "y-max:1"
        );
        // Fourteen tall, an anchor on the third course → eleven layers of rock
        // come off. This is the case the boolean could not express.
        assert_eq!(
            aimed_cutaway([20.5, 2.0, 60.5], [41, 14, 125]).to_string(),
            "y-max:11"
        );
        // An anchor on the top course still leaves its own layer standing.
        assert_eq!(
            aimed_cutaway([3.5, 4.0, 4.5], [7, 5, 9]).to_string(),
            "y-max:1"
        );
        // …and one below the floor cannot ask for more than the body has.
        let c = aimed_cutaway([3.5, -20.0, 4.5], [7, 5, 9]);
        assert!(c.kept_box([7, 5, 9]).is_some(), "never empties the model");
    }

    #[test]
    fn a_custom_cut_is_framed_like_a_planned_one() {
        let size = [20, 10, 84];
        let custom = custom_shot("z-min:50%".parse().unwrap(), size);
        let planned = plan_piece(size, None)
            .into_iter()
            .find(|s| s.name == "sec-z")
            .unwrap();
        assert_eq!(
            (custom.yaw_deg, custom.pitch_deg),
            (planned.yaw_deg, planned.pitch_deg)
        );
    }

    #[test]
    fn a_cut_that_keeps_nothing_is_refused_not_rendered() {
        let size = [20, 10, 84];
        // The planned set never does this — that is the point of asserting it.
        assert!(refuse_empty_cuts(&plan_piece(size, None), size).is_none());
        let doomed = custom_shot("y-max:100%".parse().unwrap(), size);
        let d = refuse_empty_cuts(&[doomed], size).expect("refused");
        assert_eq!(d.code, "DW0727");
        assert!(d.is_error());
        assert!(d.message.contains("y-max:100%"), "{}", d.message);
        // Two clips that only empty the body together are caught just the same —
        // the check is on the kept box, not on any one clip.
        let pincer = custom_shot("x-min:50%+x-max:50%".parse().unwrap(), size);
        assert_eq!(
            refuse_empty_cuts(&[pincer], size).map(|d| d.code),
            Some("DW0727")
        );
    }

    #[test]
    fn plan_is_deterministic() {
        let a = plan_piece([9, 5, 9], None);
        let b = plan_piece([9, 5, 9], None);
        assert_eq!(a, b);
    }
}
