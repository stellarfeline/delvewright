//! `render-plan.json` emission (spec-0003 visual tier / spec-0007 rendering infra).
//!
//! The compiler knows every world coordinate, so the visual-tier camera plan is
//! **computed, not guessed**: one deterministic shot list derived from the layout,
//! each shot carrying a camera (pos + orientation) and a machine-generated
//! `expect` checklist derived from the DSL. `delve-render scene` turns this into
//! Chunky scenes; the generation-time vision agent reviews each render against its
//! `expect` strings (findings are DSL-addressable, same shape as playtest notes).
//!
//! ## Camera convention (shared with `delve-render`)
//!
//! `pos`/`look_at` are world coordinates (block-centre floats). `yaw`/`pitch` are
//! **degrees**, aimed from `pos` at `look_at`:
//! - `yaw = atan2(-dz, dx)`: `0` faces +X (east), `90` faces −Z (north), `180`
//!   faces −X (west), `-90`/`270` faces +Z (south).
//! - `pitch = atan2(-dy, hypot(dx, dz))`: positive looks **down**, `0` is level.
//!
//! This matches the camera axes the spike-render-fidelity spike verified for
//! Chunky (`yaw ≈ π/2` faces −Z, positive pitch = down); `delve-render scene`
//! converts these degrees to Chunky's radians directly.
//!
//! ## Determinism
//!
//! Shots are emitted in a fixed order (spawn → per-area interiors+seams → NPCs →
//! interacts → gates), every list is plan-ordered or sorted, and there is no RNG
//! or wall-clock input — so `render-plan.json` rides the ADR-0006 double-build
//! byte-identity gate like every other output.

use delvewright_dsl::{Campaign, LightingProfile, Objective};
use serde_json::{Value, json};

use crate::plan::{Plan, ResolvedAnchor};
use crate::registry::PrefabRegistry;

/// Unit facing vector for a Minecraft facing keyword (north = −Z).
fn facing_vec(facing: Option<&str>) -> [f64; 3] {
    match facing {
        Some("north") => [0.0, 0.0, -1.0],
        Some("south") => [0.0, 0.0, 1.0],
        Some("east") => [1.0, 0.0, 0.0],
        Some("west") => [-1.0, 0.0, 0.0],
        _ => [0.0, 0.0, 1.0], // default south, matching the summon yaw default
    }
}

/// Aim a camera at `look_at` from `pos`; returns `(yaw_deg, pitch_deg)` in the
/// documented convention.
fn aim(pos: [f64; 3], look_at: [f64; 3]) -> (f64, f64) {
    let dx = look_at[0] - pos[0];
    let dy = look_at[1] - pos[1];
    let dz = look_at[2] - pos[2];
    let yaw = (-dz).atan2(dx).to_degrees();
    let horiz = (dx * dx + dz * dz).sqrt();
    let pitch = (-dy).atan2(horiz).to_degrees();
    (round3(yaw), round3(pitch))
}

/// Round to 3 decimals so float formatting is stable across platforms (the
/// determinism gate compares bytes; a repeatable rounding avoids libm ulp drift).
fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

fn centre(pos: [i32; 3]) -> [f64; 3] {
    [pos[0] as f64 + 0.5, pos[1] as f64, pos[2] as f64 + 0.5]
}

fn camera(pos: [f64; 3], look_at: [f64; 3]) -> Value {
    let (yaw, pitch) = aim(pos, look_at);
    json!({
        "pos": [round3(pos[0]), round3(pos[1]), round3(pos[2])],
        "yaw": yaw,
        "pitch": pitch,
        "look_at": [round3(look_at[0]), round3(look_at[1]), round3(look_at[2])],
    })
}

/// The thin (wall-normal) axis of an inclusive `[from,to]` region, or `None` when
/// it is not planar (no axis with `from == to`); prefers the first such axis.
fn thin_axis(from: [i32; 3], to: [i32; 3]) -> Option<usize> {
    (0..3).find(|&a| from[a] == to[a])
}

fn region_centre(from: [i32; 3], to: [i32; 3]) -> [f64; 3] {
    [
        (from[0] + to[0]) as f64 / 2.0 + 0.5,
        (from[1] + to[1]) as f64 / 2.0 + 1.0,
        (from[2] + to[2]) as f64 / 2.0 + 0.5,
    ]
}

/// Resolve the campaign spawn as `(area, pos, facing)` — the first area's `spawn`
/// point anchor.
fn spawn_of(plan: &Plan) -> Option<(String, [i32; 3], Option<String>)> {
    for area in &plan.areas {
        if let Some(ResolvedAnchor::Point { pos, facing }) = plan
            .anchors
            .get(&(area.area_id.clone(), "spawn".to_string()))
        {
            return Some((area.area_id.clone(), *pos, facing.clone()));
        }
    }
    None
}

/// Build the `render-plan.json` value for a compiled plan.
pub fn render_plan(plan: &Plan, prefabs: &PrefabRegistry) -> Value {
    let c = plan.campaign;
    let mut shots: Vec<Value> = Vec::new();

    // --- spawn -------------------------------------------------------------
    if let Some((area, pos, facing)) = spawn_of(plan) {
        let f = facing_vec(facing.as_deref());
        let base = centre(pos);
        // Camera behind the player (opposite the spawn facing), raised, looking
        // the way the player faces.
        let eye = [base[0] - f[0] * 5.0, base[1] + 3.0, base[2] - f[2] * 5.0];
        let look = [base[0] + f[0] * 2.0, base[1] + 1.0, base[2] + f[2] * 2.0];
        shots.push(json!({
            "id": "spawn",
            "kind": "spawn",
            "area": area,
            "camera": camera(eye, look),
            "expect": [
                "spawn point clear — player can stand, not suffocating or falling",
                "area entry framed — no void gap at the floor",
            ],
        }));
    }

    // --- per-area interiors + seams ---------------------------------------
    for area in &plan.areas {
        for (pi, piece) in area.pieces.iter().enumerate() {
            let (min, max) = piece.bbox();
            let cx = (min[0] + max[0]) as f64 / 2.0 + 0.5;
            let cy = min[1] as f64 + 1.0;
            let cz = (min[2] + max[2]) as f64 / 2.0 + 0.5;
            // Dollhouse overview: eye above a corner, aimed down at the interior
            // centre. (Renderer strips the ceiling for the matching per-piece
            // interior shot; the Chunky path places a true in-room camera.)
            let eye = [
                min[0] as f64 - 1.5,
                max[1] as f64 + 3.0,
                min[2] as f64 - 1.5,
            ];
            let look = [cx, cy, cz];
            let lit = piece_is_lit(prefabs, &piece.prefab_id);
            let mut expect = vec![
                Value::String("room interior assembled — walls, floor and ceiling intact".into()),
                Value::String("no floating or clipped blocks at piece bounds".into()),
            ];
            expect.push(Value::String(match lit {
                Some(true) => "room declared lit — no dark frame".into(),
                Some(false) => {
                    "room declared dark — mitigation expected (night-vision/placed light)".into()
                }
                None => "room lighting unmeasured — verify readability".into(),
            }));
            shots.push(json!({
                "id": format!("interior/{}/{pi}", short(&area.area_id)),
                "kind": "interior",
                "area": area.area_id,
                "prefab": piece.prefab_id,
                "camera": camera(eye, look),
                "expect": expect,
            }));
        }
        // Seam (doorway) shots: air-clear seals are cut openings between mated
        // pieces; wall-fill seals seal dead-end sockets (skipped — nothing to see).
        for (si, seal) in area.seals.iter().enumerate() {
            if seal.block != "minecraft:air" {
                continue;
            }
            let ctr = region_centre(seal.from, seal.to);
            let axis = thin_axis(seal.from, seal.to).unwrap_or(2);
            let mut n = [0.0, 0.0, 0.0];
            n[axis] = 1.0;
            let eye = [ctr[0] + n[0] * 4.0, ctr[1] + 1.5, ctr[2] + n[2] * 4.0];
            shots.push(json!({
                "id": format!("seam/{}/{si}", short(&area.area_id)),
                "kind": "seam",
                "area": area.area_id,
                "camera": camera(eye, ctr),
                "expect": [
                    "seam between pieces shows no floating or clipped blocks",
                    "doorway opening is clear — passage traversable",
                ],
            }));
        }
    }

    // --- NPCs --------------------------------------------------------------
    for npc in &plan.npcs {
        let area = plan.npc_area(&npc.npc_id).unwrap_or("").to_string();
        let anchor = c
            .npcs
            .content
            .npcs
            .iter()
            .find(|n| n.id.as_str() == npc.npc_id)
            .map(|n| n.anchor.as_str())
            .unwrap_or("");
        let name = c
            .npcs
            .content
            .npcs
            .iter()
            .find(|n| n.id.as_str() == npc.npc_id)
            .map(|n| n.name.as_str())
            .unwrap_or("NPC");
        let Some(ResolvedAnchor::Point { pos, facing }) =
            plan.anchors.get(&(area.clone(), anchor.to_string()))
        else {
            continue;
        };
        let f = facing_vec(facing.as_deref());
        let base = centre(*pos);
        // The player approaches from the direction the NPC faces (NPCs are summoned
        // facing the player), so the camera stands there and looks back at the NPC.
        let eye = [base[0] + f[0] * 4.0, base[1] + 1.6, base[2] + f[2] * 4.0];
        let look = [base[0], base[1] + 1.0, base[2]];
        shots.push(json!({
            "id": format!("npc/{}", short(&npc.npc_id)),
            "kind": "npc",
            "area": area,
            "npc": npc.npc_id,
            "camera": camera(eye, look),
            "expect": [
                format!("NPC named \"{name}\" faces the camera"),
                "NPC name tag renders as readable text — not literal JSON/SNBT",
                "NPC stands on the floor — not floating, sunk, or clipping a wall",
            ],
        }));
    }

    // --- interact anchors --------------------------------------------------
    for (obj_id, anchor, area, hint) in interact_anchors(c) {
        let Some(pos) = plan.point(&area, &anchor) else {
            continue;
        };
        let base = centre(pos);
        let eye = [base[0] + 2.5, base[1] + 3.0, base[2] + 2.5];
        let look = [base[0], base[1] + 1.0, base[2]];
        let mut expect = vec![
            Value::String("glowing lantern marker visible at the interact anchor".into()),
            Value::String("interaction hitbox present — objective is completable here".into()),
        ];
        if let Some(h) = hint {
            expect.push(Value::String(format!("matches objective hint: {h}")));
        }
        shots.push(json!({
            "id": format!("interact/{}", short(&obj_id)),
            "kind": "interact",
            "area": area,
            "objective": obj_id,
            "camera": camera(eye, look),
            "expect": expect,
        }));
    }

    // --- gates (both sides) -----------------------------------------------
    for ((area, anchor), resolved) in &plan.anchors {
        let ResolvedAnchor::Gate { from, to, block } = resolved else {
            continue;
        };
        let ctr = region_centre(*from, *to);
        let axis = thin_axis(*from, *to).unwrap_or(2);
        for (side, sign) in [("a", 1.0f64), ("b", -1.0f64)] {
            let mut off = [0.0, 0.0, 0.0];
            off[axis] = sign * 4.0;
            let eye = [ctr[0] + off[0], ctr[1] + 1.0, ctr[2] + off[2]];
            shots.push(json!({
                "id": format!("gate/{}/{}/{side}", short(area), short(anchor)),
                "kind": "gate",
                "area": area,
                "camera": camera(eye, ctr),
                "expect": [
                    format!("gate of {block} spans the opening — no gaps when closed"),
                    "gate approach clear from this side — passage blocked until opened",
                ],
            }));
        }
    }

    let (amin, amax) = layout_aabb(plan);
    json!({
        "version": c.world.dsl_version,
        "campaign_id": plan.namespace,
        "layout_aabb": { "min": amin, "max": amax },
        "camera_convention": "yaw/pitch degrees; yaw=atan2(-dz,dx) (0=+X,90=-Z); pitch=atan2(-dy,horiz) (+down)",
        "shots": shots,
    })
}

/// The last `/`-segment of an id, sanitized to `[a-z0-9_]`, for stable shot ids.
fn short(id: &str) -> String {
    let local = id.rsplit('/').next().unwrap_or(id);
    local.replace(['-', '.', ':'], "_")
}

/// Whether a placed prefab's declared lighting profile is `lit` (`Some(true)`),
/// `dark` (`Some(false)`), or undeclared (`None`).
fn piece_is_lit(prefabs: &PrefabRegistry, prefab_id: &str) -> Option<bool> {
    prefabs.get(prefab_id).and_then(|m| {
        m.lighting
            .as_ref()
            .map(|l| matches!(l.profile, LightingProfile::Lit))
    })
}

/// `(objective_id, anchor, area, hint)` for every `interact` objective, in
/// declared order.
fn interact_anchors(c: &Campaign) -> Vec<(String, String, String, Option<String>)> {
    let quest_area: std::collections::BTreeMap<&str, &str> = c
        .quest_plan
        .content
        .quests
        .iter()
        .map(|q| (q.id.as_str(), q.area.as_str()))
        .collect();
    let mut out = Vec::new();
    for q in &c.quests.content.quests {
        let area = quest_area.get(q.id.as_str()).copied().unwrap_or("");
        for o in &q.objectives {
            if let Objective::Interact {
                id, anchor, hint, ..
            } = o
            {
                out.push((
                    id.as_str().to_string(),
                    anchor.as_str().to_string(),
                    area.to_string(),
                    hint.clone(),
                ));
            }
        }
    }
    out
}

/// Union world AABB across every placed area (for the Chunky `chunkList`).
fn layout_aabb(plan: &Plan) -> ([i32; 3], [i32; 3]) {
    let mut min = [i32::MAX; 3];
    let mut max = [i32::MIN; 3];
    for area in &plan.areas {
        let (amin, amax) = area.bounds();
        for a in 0..3 {
            min[a] = min[a].min(amin[a]);
            max[a] = max[a].max(amax[a]);
        }
    }
    if min[0] == i32::MAX {
        // No pieces (should not happen for a valid campaign); degrade to origin.
        return ([0, 0, 0], [0, 0, 0]);
    }
    (min, max)
}
