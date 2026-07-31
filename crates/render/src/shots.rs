//! Per-piece shot planner for `delve-render piece`.
//!
//! Nucleation is an **orbit / turntable** renderer: `CameraConfig` fits the
//! camera to the model bounds and (optionally) aims at a `target` point — it does
//! not place a free camera inside a room. So the per-piece set is:
//!
//! - **4 exterior corner-isometric** shots (yaw 45/135/225/315, pitch 30) of the
//!   full schematic — Nucleation's strength.
//! - **1 top-down** floor-plan shot (pitch 90) on a **ceiling-stripped** cutaway
//!   so the interior floor is visible instead of the roof.
//! - **interior doorway** shots: one per socket, ceiling-stripped, aimed
//!   (`target`) at a point just inside the socket, orbit-yaw set so the camera
//!   sits on the socket's outward side and looks through the opening.
//! - **anchor** shots (when metadata provides anchors): one per anchor,
//!   ceiling-stripped, aimed at the anchor position.
//!
//! True free-camera interior shots (camera literally standing at the doorway) are
//! the **Chunky scene path** (`delve-render scene`), which places cameras anywhere
//! in the built world. These per-piece cutaways are the fast per-prefab
//! approximation for authoring-time review of floor plan, anchor placement, and
//! lighting. All are validation artifacts, never shipped.

use crate::meta::PrefabMeta;

/// One planned per-piece shot.
#[derive(Debug, Clone, PartialEq)]
pub struct PieceShot {
    /// Output file stem (e.g. `ext-ne`, `top`, `door-0`, `anchor-objective`).
    pub name: String,
    /// Orbit yaw / pitch (degrees), Nucleation `CameraConfig` convention:
    /// `dir = -[cos(pitch)·sin(yaw), sin(pitch), cos(pitch)·cos(yaw)]`.
    pub yaw_deg: f32,
    pub pitch_deg: f32,
    /// Zoom (>1 pushes the camera closer).
    pub zoom: f32,
    /// Explicit orbit target (interior/anchor shots); `None` uses the model
    /// centroid (exterior/top-down).
    pub target: Option<[f32; 3]>,
    /// Strip the top Y layer before meshing (dollhouse cutaway) so an orbit
    /// camera can see the roofed interior.
    pub cutaway: bool,
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

/// Plan the deterministic shot set for a piece of `size`, using `meta` when
/// present (sockets → doorway shots, anchors → anchor shots).
pub fn plan_piece(size: [i32; 3], meta: Option<&PrefabMeta>) -> Vec<PieceShot> {
    let mut shots = Vec::new();

    // 4 exterior corner-isometric.
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
            cutaway: false,
        });
    }

    // Top-down floor plan (ceiling stripped).
    shots.push(PieceShot {
        name: "top".to_string(),
        yaw_deg: 0.0,
        pitch_deg: 90.0,
        zoom: 1.0,
        target: None,
        cutaway: true,
    });

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
                cutaway: true,
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
                cutaway: true,
            });
        }
    }

    let _ = size; // reserved for future zoom auto-scaling by piece extent
    shots
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exterior_and_topdown_without_metadata() {
        let shots = plan_piece([7, 5, 9], None);
        assert_eq!(shots.len(), 5);
        let names: Vec<&str> = shots.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["ext-ne", "ext-se", "ext-sw", "ext-nw", "top"]);
        assert!(shots.iter().find(|s| s.name == "top").unwrap().cutaway);
        assert!(!shots.iter().find(|s| s.name == "ext-ne").unwrap().cutaway);
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
        // Every interior shot is a cutaway with an explicit target.
        for s in shots
            .iter()
            .filter(|s| s.name.starts_with("door") || s.name.starts_with("anchor"))
        {
            assert!(s.cutaway && s.target.is_some());
        }
        // north socket → orbit yaw 180 (camera on the outward/−Z side, looking in).
        assert_eq!(
            shots.iter().find(|s| s.name == "door-0").unwrap().yaw_deg,
            180.0
        );
    }

    #[test]
    fn plan_is_deterministic() {
        let a = plan_piece([9, 5, 9], None);
        let b = plan_piece([9, 5, 9], None);
        assert_eq!(a, b);
    }
}
