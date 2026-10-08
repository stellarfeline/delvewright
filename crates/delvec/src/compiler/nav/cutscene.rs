//! The cutscene proofs: a camera dolly never clips a solid, and its aim stays
//! inside the angular budget (`DW0308`, `DW0347`).

use super::*;
use crate::compiler::failure::Failure;
use crate::compiler::plan::{Plan, ResolvedAnchor};
use delvewright_dsl::Mark;

/// The camera dolly world points of a cutscene (anchor + offset, block centres) —
/// the exact points the emitter lerps between. Shared with the emitter so the
/// air-corridor check validates what actually ships.
pub fn camera_points(plan: &Plan, path: &[Mark]) -> Vec<[f64; 3]> {
    path.iter()
        .map(|w| anchor_offset_point(plan, w.anchor.as_str(), w.offset))
        .collect()
}

/// The world point a cutscene's `look_at` subject resolves to (DSL v0.6) — the
/// same anchor + offset block-centre convention as [`camera_points`], so a
/// waypoint and a look target at the same anchor/offset name the same point.
pub fn camera_look_point(plan: &Plan, target: &Mark) -> [f64; 3] {
    anchor_offset_point(plan, target.anchor.as_str(), target.offset)
}

/// Resolve `anchor + offset` to a block-centre world point (the shared cutscene
/// camera convention). An unresolved anchor falls back to the layout origin —
/// referential validation reports it separately.
pub(crate) fn anchor_offset_point(plan: &Plan, anchor: &str, offset: [i32; 3]) -> [f64; 3] {
    let base = plan
        .anchors
        .iter()
        .find(|((_, name), _)| name == anchor)
        .map(|(_, r)| match r {
            ResolvedAnchor::Point { pos, .. } => *pos,
            ResolvedAnchor::Gate { from, .. } => *from,
        })
        .unwrap_or([0, crate::compiler::plan::BASE_Y, 0]);
    [
        (base[0] + offset[0]) as f64 + 0.5,
        (base[1] + offset[1]) as f64 + 0.5,
        (base[2] + offset[2]) as f64 + 0.5,
    ]
}

/// Validate every cutscene camera dolly (per shot: a multi-shot cutscene
/// hard-cuts between shots, so only the within-shot dolly is a corridor the
/// camera actually flies):
///
/// - **`DW0308` (authored polyline)**: the waypoint polyline passes only
///   through non-solid blocks (cameras fly but must not clip a solid). Names
///   the offending shot, segment and clipping block.
/// - **`DW0308` (rendered chords)**: the client draws straight chords between
///   the emitted keyframes ([`crate::compiler::camera::plan_shot`] — the tween is
///   client-side and linear, spike-measured), which can cut a corner of the
///   authored polyline by up to [`crate::compiler::camera::CHORD_POS_TOLERANCE`]. The
///   chord polyline is what actually ships, so it is ray-checked too.
/// - **`DW0347` (angular budget)**: the shot's peak aim rate must stay within
///   [`crate::compiler::camera::MAX_AIM_DEG_PER_TICK`]. An over-budget pan is a
///   provably nauseating shot — an error, not a warning: the fix (more camera
///   distance, a longer shot, or a hard cut between two shots) is always
///   available, and a red check is information (CLAUDE.md debug doctrine).
pub fn check_cutscenes(
    plan: &Plan,
    world: &World,
    moves: &[MovePlan],
    actor_moves: &[ActorMovePlan],
) -> Result<usize, Failure> {
    let mut judged = 0usize;
    for (eff, ctx) in crate::compiler::camera::cutscene_units(plan.campaign) {
        let Some(shots) = eff.cutscene_shots() else {
            continue;
        };
        let mut offset: i32 = 0;
        for (si, shot) in shots.iter().enumerate() {
            judged += 1;
            let ex =
                crate::compiler::camera::expand_shot(plan, moves, actor_moves, shot, &ctx, offset);
            offset += ex.ticks + 1;
            let pts = ex.clip_polyline();
            if let Some((seg, cell)) = first_clip(world, pts) {
                return Err(Failure {
                    code: DW_CUTSCENE_CLIP,
                    message: format!(
                        "cutscene: shot {si} camera dolly segment {seg} (from {:?} to {:?}) clips \
                         a solid block at {cell:?} — cameras must fly through open air; move the \
                         segment's waypoint `anchor`/`offset` (or the shot's `bearing`/`dist` for \
                         a styled shot) so the whole path clears solid blocks",
                        round3(pts[seg]),
                        round3(pts[seg + 1]),
                    ),
                });
            }
            let frames = ex.frames();
            let chord: Vec<[f64; 3]> = frames.frames.iter().map(|f| f.pos).collect();
            if let Some((seg, cell)) = first_clip(world, &chord) {
                return Err(Failure {
                    code: DW_CUTSCENE_CLIP,
                    message: format!(
                        "cutscene: shot {si} client-rendered dolly chord {seg} (keyframe {:?} to \
                         {:?}) clips a solid block at {cell:?} — the client tweens straight \
                         between keyframes, cutting inside the authored waypoint corner; move the \
                         nearby waypoint `anchor`/`offset` a block outward so the smoothed path \
                         also clears",
                        round3(chord[seg]),
                        round3(chord[seg + 1]),
                    ),
                });
            }
            // spec-0091 (`DW0956`): what the shot looks at is served to the
            // player watching it. Every keyframe is a body's eye for a tick, and
            // the aim at that tick is what the picture is of.
            let radius = delvewright_dsl::viewdistance::served_radius_blocks(
                delvewright_dsl::viewdistance::chunks(plan.campaign),
            );
            let mut farthest: Option<(f64, [f64; 3], [f64; 3], i32)> = None;
            for f in &frames.frames {
                let aim = match &ex.aim {
                    crate::compiler::camera::AimTrack::Travel => continue,
                    crate::compiler::camera::AimTrack::Static(p) => *p,
                    crate::compiler::camera::AimTrack::Moving(track) => {
                        let i = (f.tick.max(0) as usize).min(track.len().saturating_sub(1));
                        track[i]
                    }
                };
                let d = [aim[0] - f.pos[0], aim[1] - f.pos[1], aim[2] - f.pos[2]];
                let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                if farthest.is_none_or(|(l, ..)| len > l) {
                    farthest = Some((len, f.pos, aim, f.tick));
                }
            }
            if let Some((len, pos, aim, tick)) = farthest
                && len > radius
            {
                return Err(Failure {
                    code: delvewright_dsl::codes::VIEW_BEYOND_SERVED,
                    message: format!(
                        "cutscene: shot {si} at tick {tick} stands at {:?} looking at {:?}, \
                         {len:.1} blocks away, and the served view distance reaches {radius:.0} \
                         blocks ({} chunks): what the shot looks at is never sent to the player \
                         watching it. Declare `world.view_distance: {}` (the fewest chunks that \
                         serve it), or bring the camera path nearer its `look_at`/subject",
                        round3(pos),
                        round3(aim),
                        delvewright_dsl::viewdistance::chunks_for(radius),
                        delvewright_dsl::viewdistance::chunks_for(len),
                    ),
                });
            }
            let rate = ex.max_aim_deg_per_tick();
            if rate > crate::compiler::camera::MAX_AIM_DEG_PER_TICK {
                return Err(Failure {
                    code: DW_CAMERA_SPIN,
                    message: format!(
                        "cutscene: shot {si} pans at {rate} deg/tick, over the {} deg/tick \
                         (120 deg/s) budget — at 20 Hz that reads as a spin, not a shot \
                         (comfortable is <= 2 deg/tick). Move the camera path farther from its \
                         `look_at` subject, lengthen `seconds`, or split the move into two shots \
                         (the hard cut between shots is the idiomatic fast reframe)",
                        crate::compiler::camera::MAX_AIM_DEG_PER_TICK,
                    ),
                });
            }
        }
    }
    Ok(judged)
}

/// The first `(segment index, block cell)` where a camera dolly polyline passes
/// through a solid block, or `None` if the whole path is air.
///
/// **Exact, not sampled**. This used to step each segment at ≤ 0.25
/// blocks and floor the sample point, which misses any cell the segment only
/// grazes: a shot can cut a block corner, enter and leave the cell entirely
/// between two samples, and ship as "provably clear". The clip test is now a
/// 3-D grid walk (Amanatides–Woo digital differential analyser) that visits
/// **every** cell the segment intersects, in order, with no error term at all —
/// so `DW0308` can no longer be dodged by geometry that happens to fall between
/// two sample points.
///
/// Deterministic (ADR-0006): integer cell stepping driven by exact ratios; ties
/// (a segment passing exactly through a cell corner) resolve on the fixed axis
/// order x, y, z.
fn first_clip(world: &World, pts: &[[f64; 3]]) -> Option<(usize, [i32; 3])> {
    for (seg, w) in pts.windows(2).enumerate() {
        if let Some(cell) = walk_cells(w[0], w[1], |c| world.blocks_camera(c)) {
            return Some((seg, cell));
        }
    }
    None
}

fn round3(p: [f64; 3]) -> [f64; 3] {
    [
        (p[0] * 1000.0).round() / 1000.0,
        (p[1] * 1000.0).round() / 1000.0,
        (p[2] * 1000.0).round() / 1000.0,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::nav::testkit::*;

    #[test]
    fn cutscene_clip_detects_a_solid_on_the_dolly_and_passes_clean_air() {
        // A solid pillar at [2,66,1]; a dolly through it clips, one beside it does
        // not.
        let world = floored(5, 4, 65, &[[2, 66, 1]]);
        let through = [[0.5, 66.5, 1.5], [4.5, 66.5, 1.5]];
        assert_eq!(first_clip(&world, &through), Some((0, [2, 66, 1])));
        let clear = [[0.5, 66.5, 3.5], [4.5, 66.5, 3.5]];
        assert_eq!(first_clip(&world, &clear), None);
    }

    #[test]
    fn camera_dolly_clips_fences_and_closed_gates() {
        // A fence contains visible geometry: a cutscene camera flying through its
        // cell is a DW0308 clip, exactly like a full solid — and so is a closed
        // fence gate (the camera would fly through the gate leaves).
        let world = classified(5, 3, 65, &[[2, 65, 1]], &[[2, 65, 2]]);
        let through_fence = [[0.5, 65.5, 1.5], [4.5, 65.5, 1.5]];
        assert_eq!(first_clip(&world, &through_fence), Some((0, [2, 65, 1])));
        let through_gate = [[0.5, 65.5, 2.5], [4.5, 65.5, 2.5]];
        assert_eq!(first_clip(&world, &through_gate), Some((0, [2, 65, 2])));
    }

    // --- exact camera clip test -----------------------------------

    #[test]
    fn first_clip_catches_a_corner_graze_the_old_sampler_missed() {
        // A single solid cell the dolly cuts diagonally through the corner of. The
        // old 0.25-sampler stepped over it (the segment is inside the cell for far
        // less than one sample); the DDA walk visits every cell the segment
        // touches, so the clip is caught.
        let world = World::from_solid_cells([[1, 0, 1]].into_iter().collect());
        let pts = [[0.9, 0.5, 0.9], [1.2, 0.5, 1.2]];
        assert_eq!(
            first_clip(&world, &pts).map(|(_, c)| c),
            Some([1, 0, 1]),
            "the grazed cell must be reported"
        );
        // A parallel path that never enters the cell stays clean.
        let clear = [[0.9, 0.5, 0.9], [0.9, 0.5, 2.5]];
        assert_eq!(first_clip(&world, &clear), None);
    }
}
