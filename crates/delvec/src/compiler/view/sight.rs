//! **Whether an eye-level frame shows a room, and where to stand so it does.**
//!
//! An anchor declares a cell and the direction a body standing on it looks. The
//! pictures made from that — `delvec render piece`'s `eye-<anchor>` shot and
//! `delvec viewer`'s per-anchor point of view — put the camera on the anchor and
//! aim it along the facing, and whether a room is in the frame is then a matter
//! of what happens to be in front of the anchor. A guide who stands with her back
//! to a hall and faces its door is a correct anchor and a picture of a door.
//! Nothing said so: a pipeline aimed at a wall wrote its PNG and exited 0 exactly
//! as one aimed at a great hall did.
//!
//! This module answers the two halves of that, off the piece's own bytes.
//!
//! # `DW0893` — a frame is **blind**
//!
//! A frame is blind when **more than half of it is a surface across the view
//! within arm's reach**. It is measured by casting [`SIGHT_SAMPLES`]² rays through
//! the pixel centres of the frame (the draft renderer's own camera and voxel walk,
//! [`crate::compiler::snapshot`]) and counting the rays whose first hit is
//!
//! * a face turned **toward the camera** — its normal within 45° of the view
//!   axis — and
//! * no further than [`ARM_REACH_BLOCKS`] from the eye.
//!
//! A ray that starts inside a block counts as such a hit: there is nothing else
//! in its part of the frame.
//!
//! Each term is there because a simpler one is wrong on a shape that matters:
//!
//! * **Distance alone is not it.** A body's own floor is always near: at eye
//!   height 1.62 and a 70° field of view, about a quarter of the frame is floor
//!   within reach in an open field, and a three-high ceiling adds as much again.
//!   A corridor's side walls fill both edges of the frame at any depth. A
//!   distance-only count calls a vaulted passage blind. Floor, ceiling and the
//!   walls running alongside the view recede; only a surface across it closes
//!   the frame.
//! * **The centre ray alone is not it.** The per-shot `clearance_open_cells` the
//!   manifest has always carried is one ray at eye level. A parapet at the body's
//!   chest fills the lower frame while the centre ray runs fifteen blocks over it.
//! * **Arm's reach** is vanilla's, not this project's: a player's
//!   `minecraft:block_interaction_range` attribute defaults to 4.5 blocks — the
//!   distance at which the game itself says a surface is close enough to touch.
//!   *Cited.*
//! * **More than half** is the frame's majority, and it is the one number here
//!   that is *authored*: a picture whose majority is a surface you could touch is
//!   a picture of that surface.
//!
//! **It is a report, never a refusal**, for the reason `DW0895` is one: a
//! legitimate eye-level shot can have a surface as its subject — an altar in an
//! alcove, a hearth, a hanging — and the engine cannot tell *pressed against a
//! wall* from *looking at the thing on it*. A refusal that cannot draw that line is
//! one an author learns to route around. What the engine can do is say how much of
//! the frame, and hand over the picture of the room beside it.
//!
//! The draft walk counts every placed block as a full cube, so a torch, a pane
//! or a fence within reach reads as a surface. That error has one direction — it
//! can only call a frame blind that a person would call merely cluttered — and a
//! report is where that direction is affordable.
//!
//! # Standing back — the picture of the room
//!
//! [`stand_back`] keeps the anchor's facing, which is a creative statement and is
//! never overridden, and moves the body **backwards along it**: from the anchor's
//! standing cell, one column at a time directly behind, for as long as
//!
//! * a body can stand there ([`crate::schem::nav::standable`], the one standability rule,
//!   with a step of at most one course between neighbours),
//! * the column is under the same cover the anchor's is — roofed if it is
//!   roofed, open if it is open ([`crate::schem::nav::sheltered`]) — so the camera
//!   does not walk out of a room through a door on the axis, or in under an arch
//!   from a courtyard. A stretch under the other cover that the line comes back
//!   out of, and that is shorter than the anchor's own cover already crossed, is
//!   a feature of the space and is crossed: a hall's louvre over its hearth is a
//!   hole in a roof, not the edge of the hall, while a gatehouse passage between
//!   a bridge and a courtyard is as long as either, and

//! * the anchor's own eye point stays in sight.
//!
//! A door on the axis between two rooms under the same cover is not a boundary
//! this walk can see, so the camera can stand in the next room looking at the
//! anchor through the door; the manifest's standing cell and stop reason are how
//! a reader tells.
//! * the anchor's own eye point stays in sight.
//!
//! The camera stands at the **furthest** such column: the far side of the space
//! the anchor stands in, looking at it. That is the framing an interior is shot
//! from — the screens end of a hall looking at its dais — and it is decided by
//! the space, not by a tuned distance. Why it stopped is recorded ([`Stop`]),
//! because a camera that moved is invisible in its own frame.

use delvewright_dsl::metrics::PLAYER_EYE_HEIGHT;

use crate::compiler::snapshot::{Camera, VoxelGrid};
use crate::compiler::view::blockcolor;
use crate::compiler::view::diag::Diagnostic;
use crate::compiler::view::nbt::Structure;
use crate::compiler::view::showing::Cells;
use crate::schem::nav;

/// **`DW0893`: an eye-level frame is blind** — more than half of it is a surface
/// across the view within arm's reach. A report, never a refusal (module note).
pub const DW_BLIND_FRAME: &str = "DW0893";

/// A player's reach, in blocks: the vanilla default of the
/// `minecraft:block_interaction_range` attribute (Java Edition 1.20.5 onward,
/// unchanged at the pinned 1.21.11). *Cited.*
pub const ARM_REACH_BLOCKS: f64 = 4.5;

/// Rays per side of the sampled frame: `SIGHT_SAMPLES²` rays, one through each
/// pixel centre of a `SIGHT_SAMPLES`-square frame.
pub const SIGHT_SAMPLES: u32 = 32;

/// A face counts as turned toward the camera when its normal is within 45° of
/// the view axis.
const ACROSS_COS: f64 = std::f64::consts::FRAC_1_SQRT_2;

/// The field of view the eye-level frames here are taken at: Minecraft's own
/// default first-person FOV — the eye shots' and the viewer's.
pub const EYE_FOV_DEG: f64 = 70.0;

/// The piece as the draft renderer's voxel walk sees it: every placed block with
/// an appearance, and nothing where a structure template places nothing.
pub fn grid_of(st: &Structure) -> VoxelGrid {
    let mut blocks = std::collections::BTreeMap::new();
    for (pos, state) in &st.blocks {
        if let Some(name) = st.palette.get(*state)
            && !blockcolor::is_air(name)
        {
            blocks.insert(*pos, name.clone());
        }
    }
    VoxelGrid::build(&blocks)
}

/// A horizontal facing keyword → its unit step (`north` → `[0, 0, -1]`). `up`,
/// `down` and anything else → `None`: an eye-level frame looks along the ground.
pub fn horizontal_step(facing: &str) -> Option<[i32; 3]> {
    crate::compiler::faces::dir_vector(facing).filter(|s| s[1] == 0)
}

/// The eye point of a body standing in `cell`.
pub fn eye_of(cell: [i32; 3]) -> [f64; 3] {
    [
        f64::from(cell[0]) + 0.5,
        f64::from(cell[1]) + PLAYER_EYE_HEIGHT,
        f64::from(cell[2]) + 0.5,
    ]
}

/// A level camera at `eye` looking along a horizontal `step`, at [`EYE_FOV_DEG`].
pub fn level_camera(eye: [f64; 3], step: [i32; 3]) -> Camera {
    // Minecraft yaw: 0 looks +Z (south), 90 −X (west), 180 −Z, 270 +X.
    let yaw = match step {
        [0, 0, 1] => 0.0,
        [-1, 0, 0] => 90.0,
        [0, 0, -1] => 180.0,
        [1, 0, 0] => 270.0,
        [dx, _, dz] => f64::from(-dx).atan2(f64::from(dz)).to_degrees(),
    };
    Camera {
        pos: eye,
        yaw,
        pitch: 0.0,
        fov: EYE_FOV_DEG,
    }
}

/// What one frame's sampled rays met.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sight {
    /// Rays cast — the denominator.
    pub rays: u32,
    /// Rays whose first hit is a face turned toward the camera within
    /// [`ARM_REACH_BLOCKS`], or that start inside a block.
    pub near: u32,
}

impl Sight {
    /// Measure the frame `cam` takes of `grid`, square, over
    /// [`SIGHT_SAMPLES`]² rays.
    pub fn measure(grid: &VoxelGrid, cam: &Camera) -> Sight {
        let (forward, _, _) = cam.basis();
        let n = SIGHT_SAMPLES;
        let mut near = 0u32;
        for py in 0..n {
            for px in 0..n {
                let dir = cam.ray(n, n, px, py);
                let Some(hit) = grid.cast(cam.pos, dir, ARM_REACH_BLOCKS) else {
                    continue;
                };
                // A ray that starts inside a block is all block.
                if hit.t <= 0.0 || forward[hit.axis].abs() >= ACROSS_COS {
                    near += 1;
                }
            }
        }
        Sight { rays: n * n, near }
    }

    /// More than half the frame is a surface within reach.
    pub fn is_blind(&self) -> bool {
        2 * u64::from(self.near) > u64::from(self.rays)
    }

    /// `near` as a percentage of the frame, one decimal.
    pub fn percent(&self) -> f64 {
        if self.rays == 0 {
            return 0.0;
        }
        (f64::from(self.near) * 1000.0 / f64::from(self.rays)).round() / 10.0
    }
}

/// Why [`stand_back`] went no further.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    /// The next column back has no cell a body can stand in within one course —
    /// a wall, a drop, or the edge of the piece.
    FloorEnds,
    /// The next column back is under different cover from the anchor: out of a
    /// roofed room into the open, or in under a roof from the open.
    CoverChanges,
    /// A body fits in the next column back, but its eye cell holds a block the
    /// camera would render from inside.
    EyeCellOccupied,
    /// From the next column back, the anchor's own eye point is out of sight.
    AnchorOutOfSight,
}

impl Stop {
    /// A short machine tag for manifests.
    pub fn tag(self) -> &'static str {
        match self {
            Stop::FloorEnds => "floor-ends",
            Stop::CoverChanges => "cover-changes",
            Stop::EyeCellOccupied => "eye-cell-occupied",
            Stop::AnchorOutOfSight => "anchor-out-of-sight",
        }
    }

    /// The reason, in words.
    pub fn words(self) -> &'static str {
        match self {
            Stop::FloorEnds => "the floor ends behind it",
            Stop::CoverChanges => "the cover changes behind it (a roof begins or ends)",
            Stop::EyeCellOccupied => "the next eye cell back holds a block",
            Stop::AnchorOutOfSight => "the anchor drops out of sight behind it",
        }
    }
}

/// Where a room camera stands for one anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stand {
    /// The cell a body on the anchor stands in: the anchor's own, or the one
    /// course above or below it where the anchor cell is the floor itself.
    pub start: [i32; 3],
    /// The cell the camera's body stands in.
    pub cell: [i32; 3],
    /// Columns behind `start` along the facing.
    pub back: u32,
    /// Why it went no further.
    pub stop: Stop,
}

impl Stand {
    /// The camera point.
    pub fn eye(&self) -> [f64; 3] {
        eye_of(self.cell)
    }
}

/// A body can stand here and the camera would not start inside a block.
fn stands(cells: &Cells<'_>, grid: &VoxelGrid, c: [i32; 3]) -> bool {
    nav::standable(cells, c) && !grid.solid([c[0], c[1] + 1, c[2]])
}

/// Stand a camera back along `step` from `anchor` — see the module note.
///
/// `None` when no cell within one course of the anchor holds a standing body:
/// there is nowhere on the anchor to start from.
pub fn stand_back(
    cells: &Cells<'_>,
    grid: &VoxelGrid,
    anchor: [i32; 3],
    step: [i32; 3],
) -> Option<Stand> {
    let [x, y, z] = anchor;
    let start = [[x, y, z], [x, y + 1, z], [x, y - 1, z]]
        .into_iter()
        .find(|&c| stands(cells, grid, c))?;
    let anchor_eye = eye_of(start);

    // Every column a body could stand the camera on, in order, and why the line
    // of them ends.
    let mut line = vec![start];
    let end = loop {
        let cur = line[line.len() - 1];
        let column = [cur[0] - step[0], cur[1], cur[2] - step[2]];
        let found = [0, 1, -1]
            .into_iter()
            .map(|dy| [column[0], column[1] + dy, column[2]])
            .find(|&c| nav::standable(cells, c));
        let Some(next) = found else {
            break Stop::FloorEnds;
        };
        if grid.solid([next[0], next[1] + 1, next[2]]) {
            break Stop::EyeCellOccupied;
        }
        if grid.blocked(eye_of(next), anchor_eye) {
            break Stop::AnchorOutOfSight;
        }
        line.push(next);
    };

    // Cover, a run at a time. A run under different cover from the anchor's
    // that the line comes back out of, and that is shorter than the stretch of
    // the anchor's own cover already crossed, is a feature of the space — a
    // louvre, a light well — and is crossed. Any other is another space, and the
    // camera stops before it.
    let roofed: Vec<bool> = line.iter().map(|&c| nav::sheltered(cells, c)).collect();
    let mut i = 1;
    while i < line.len() {
        if roofed[i] == roofed[0] {
            i += 1;
            continue;
        }
        let back_under = (i..line.len()).find(|&j| roofed[j] == roofed[0]);
        match back_under {
            Some(j) if j - i < i => i = j,
            _ => {
                return Some(Stand {
                    start,
                    cell: line[i - 1],
                    back: (i - 1) as u32,
                    stop: Stop::CoverChanges,
                });
            }
        }
    }
    Some(Stand {
        start,
        cell: line[line.len() - 1],
        back: (line.len() - 1) as u32,
        stop: end,
    })
}

/// One anchor's two eye-level frames as a door that offers the anchor's own
/// point of view takes them: a body on the anchor's cell exactly, and the room
/// camera stood back along the same facing.
#[derive(Debug, Clone, PartialEq)]
pub struct AnchorFrames {
    /// The anchor's full name.
    pub anchor: String,
    /// Its declared facing keyword.
    pub facing: String,
    /// Its declared cell — where the point-of-view body's feet are.
    pub cell: [i32; 3],
    /// The frame from the anchor's own cell.
    pub pov: Sight,
    /// The room camera, when a body can stand on the anchor at all.
    pub room: Option<(Stand, Sight)>,
}

/// Every anchor that declares a position and a horizontal facing, in name
/// order, with both of its frames measured. Anchors without both have no eye
/// frame to measure and are not in the list; [`frames_line`] counts them.
pub fn anchor_frames(
    st: &Structure,
    meta: Option<&crate::compiler::view::meta::PrefabMeta>,
) -> Vec<AnchorFrames> {
    let Some(meta) = meta else {
        return Vec::new();
    };
    let grid = grid_of(st);
    let cells = Cells::of(st);
    let mut out = Vec::new();
    for (name, a) in &meta.anchors {
        let (Some(cell), Some(facing)) = (a.pos, a.facing.as_deref()) else {
            continue;
        };
        let Some(step) = horizontal_step(facing) else {
            continue;
        };
        let pov = Sight::measure(&grid, &level_camera(eye_of(cell), step));
        let room = stand_back(&cells, &grid, cell, step).map(|stand| {
            (
                stand,
                Sight::measure(&grid, &level_camera(stand.eye(), step)),
            )
        });
        out.push(AnchorFrames {
            anchor: name.clone(),
            facing: facing.to_string(),
            cell,
            pov,
            room,
        });
    }
    out
}

/// The binding line for [`anchor_frames`], printed on every run with its
/// zeroes.
pub fn frames_line(id: &str, declared: usize, frames: &[AnchorFrames]) -> String {
    let blind_pov = frames.iter().filter(|f| f.pov.is_blind()).count();
    let rooms: Vec<&(Stand, Sight)> = frames.iter().filter_map(|f| f.room.as_ref()).collect();
    let blind_room = rooms.iter().filter(|(_, s)| s.is_blind()).count();
    let max_back = rooms.iter().map(|(st, _)| st.back).max().unwrap_or(0);
    format!(
        "sight: `{id}` — {} of {declared} anchor(s) declare a position and a horizontal facing; \
         blind (more than half the frame a surface within {ARM_REACH_BLOCKS} blocks): {blind_pov} \
         of {} point-of-view frame(s), {blind_room} of {} room frame(s); room cameras stood \
         0..{max_back} block(s) back",
        frames.len(),
        frames.len(),
        rooms.len(),
    )
}

/// The `DW0893`s [`anchor_frames`] owes, one per blind frame. `pov_frame` and
/// `room_frame` name the two frames for an anchor, in the words of the door.
pub fn frames_findings(
    frames: &[AnchorFrames],
    pov_frame: impl Fn(&str) -> String,
    room_frame: impl Fn(&str) -> String,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for f in frames {
        let room_name = room_frame(&f.anchor);
        let room = f.room.as_ref().map(|(st, s)| (room_name.as_str(), st, s));
        if f.pov.is_blind() {
            out.push(blind_diagnostic(
                &pov_frame(&f.anchor),
                &f.anchor,
                &f.facing,
                f.cell,
                &f.pov,
                room,
            ));
        }
        if let Some((st, s)) = &f.room
            && s.is_blind()
        {
            out.push(blind_diagnostic(
                &room_name, &f.anchor, &f.facing, st.cell, s, room,
            ));
        }
    }
    out
}

/// The `DW0893` a blind frame owes its reader.
///
/// `frame` names the image (`<stem>/eye-gate`, or a viewer preset), `anchor`
/// the anchor it serves, `cell` where the body stands. `room` is the room
/// camera for the same anchor, when there is one and this frame is not it — the
/// picture to open instead, or the news that there is none.
pub fn blind_diagnostic(
    frame: &str,
    anchor: &str,
    facing: &str,
    cell: [i32; 3],
    sight: &Sight,
    room: Option<(&str, &Stand, &Sight)>,
) -> Diagnostic {
    let instead = match room {
        Some((name, stand, rs)) if !rs.is_blind() => format!(
            "The room it stands in is in `{name}`: the same facing, {} block(s) back at {:?}, \
             where {}% of the frame is that near",
            stand.back,
            stand.cell,
            rs.percent()
        ),
        Some((name, stand, rs)) => format!(
            "Standing back does not clear it: `{name}` is {} block(s) back at {:?} ({}), and {}% \
             of that frame is still that near. Wherever a body can stand behind this anchor, what \
             it faces is closer than arm's reach — if the room is the subject, the anchor's \
             position or facing is the thing to change",
            stand.back,
            stand.cell,
            stand.stop.words(),
            rs.percent()
        ),
        None => "No room camera stands for this anchor: no cell within one course of it holds \
                 a standing body"
            .to_string(),
    };
    Diagnostic::warning(
        DW_BLIND_FRAME,
        format!(
            "{frame}: BLIND — {}% of the frame ({} of {} sampled rays) is a surface turned toward \
             the camera within arm's reach ({ARM_REACH_BLOCKS} blocks), so a body at {cell:?} on \
             `{anchor}` looking {facing} sees that surface and not the room. A report, not a \
             refusal: a surface can be the subject of a shot. {instead}",
            sight.percent(),
            sight.near,
            sight.rays,
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A box of stone: floor at `y=0`, ceiling at `y=h-1`, walls around, open
    /// inside — `size` cells in all.
    fn room(size: [i32; 3]) -> Structure {
        let mut blocks = Vec::new();
        for x in 0..size[0] {
            for y in 0..size[1] {
                for z in 0..size[2] {
                    let shell = x == 0
                        || x == size[0] - 1
                        || z == 0
                        || z == size[2] - 1
                        || y == 0
                        || y == size[1] - 1;
                    if shell {
                        blocks.push(([x, y, z], 0usize));
                    }
                }
            }
        }
        Structure {
            size,
            palette: vec!["minecraft:stone".to_string()],
            blocks,
        }
    }

    fn frame(st: &Structure, cell: [i32; 3], facing: &str) -> Sight {
        let grid = grid_of(st);
        let step = horizontal_step(facing).unwrap();
        Sight::measure(&grid, &level_camera(eye_of(cell), step))
    }

    /// **The defect, and the perturbation only this check catches.** A body
    /// standing one block from a wall and facing it sees the wall; the same body
    /// in the same room, facing the room's length, does not. Nothing but the
    /// distance to the surface across the view moves between the two.
    #[test]
    fn a_body_facing_a_wall_at_arms_length_is_blind_and_facing_the_room_is_not() {
        // 9 wide, 6 high, 24 long; interior x 1..=7, z 1..=22.
        let st = room([9, 6, 24]);
        let against = frame(&st, [4, 1, 2], "north");
        assert!(against.is_blind(), "{against:?}");
        assert_eq!(against.rays, SIGHT_SAMPLES * SIGHT_SAMPLES);
        let along = frame(&st, [4, 1, 2], "south");
        assert!(!along.is_blind(), "{along:?}");
        assert_eq!(along.near, 0, "the far wall is 20 blocks off: {along:?}");
    }

    /// Floor, ceiling and side walls are near in every low corridor, and none
    /// of them closes the frame: a three-wide, three-high passage with a long
    /// view is not blind, although a distance-only count would call it so.
    #[test]
    fn a_low_narrow_passage_with_a_long_view_is_not_blind() {
        // Interior x 1..=3, y 1..=3, z 1..=38.
        let st = room([5, 5, 40]);
        let s = frame(&st, [2, 1, 38], "north");
        assert_eq!(s.near, 0, "{s:?}");
        // …while the same passage, faced at its end wall, is.
        let end = frame(&st, [2, 1, 2], "north");
        assert!(end.is_blind(), "{end:?}");
    }

    /// The threshold is arm's reach, a bound on the surface's distance: the same
    /// wall faced from past reach contributes nothing, however much of the frame
    /// it fills.
    #[test]
    fn the_same_wall_past_arms_reach_is_not_blind() {
        // Interior z 1..=22; a body at z=6 facing north has the wall face at z=1,
        // 5.5 blocks from the eye.
        let st = room([9, 6, 24]);
        assert_eq!(frame(&st, [4, 1, 6], "north").near, 0);
        // At z=2 the face is 1.5 blocks off.
        assert!(frame(&st, [4, 1, 2], "north").is_blind());
    }

    /// A camera inside a block is all block.
    #[test]
    fn an_eye_inside_a_block_is_blind() {
        let st = room([9, 6, 24]);
        let s = frame(&st, [0, 1, 5], "south");
        assert_eq!(s.near, s.rays, "{s:?}");
    }

    /// The `DW0893` message is a warning and names what to open instead.
    #[test]
    fn a_blind_frame_is_reported_as_dw0893_never_refused() {
        let st = room([9, 6, 24]);
        let grid = grid_of(&st);
        let cells = Cells::of(&st);
        let sight = frame(&st, [4, 1, 22], "south");
        assert!(sight.is_blind());
        let stand = stand_back(&cells, &grid, [4, 1, 22], [0, 0, 1]).unwrap();
        let rs = Sight::measure(&grid, &level_camera(stand.eye(), [0, 0, 1]));
        let d = blind_diagnostic(
            "piece/eye-k",
            "anchor/k",
            "south",
            [4, 1, 22],
            &sight,
            Some(("piece/room-k", &stand, &rs)),
        );
        assert_eq!(d.code, "DW0893");
        assert!(!d.is_error(), "a report, never a refusal");
        assert!(d.message.contains("BLIND"), "{}", d.message);
        assert!(d.message.contains("`piece/room-k`"), "{}", d.message);
    }

    /// **Standing back keeps the facing and walks to the far side of the room.**
    #[test]
    fn standing_back_walks_to_the_far_wall_behind_the_anchor() {
        let st = room([9, 6, 24]);
        let grid = grid_of(&st);
        let cells = Cells::of(&st);
        // The anchor stands at the south wall facing it; the room is behind it.
        let s = stand_back(&cells, &grid, [4, 1, 22], [0, 0, 1]).unwrap();
        assert_eq!(s.start, [4, 1, 22]);
        assert_eq!(s.cell, [4, 1, 1], "the far wall's first standing cell");
        assert_eq!(s.back, 21);
        assert_eq!(s.stop, Stop::FloorEnds);
        let rs = Sight::measure(&grid, &level_camera(s.eye(), [0, 0, 1]));
        assert!(!rs.is_blind(), "{rs:?}");
    }

    /// A roofed room with a doorway on the anchor's axis out into the open: the
    /// camera stops at the threshold rather than walking out of the room and
    /// photographing it through its own door.
    #[test]
    fn standing_back_does_not_leave_the_room_it_starts_in() {
        // The south half (z 12..=23) is a roofed room; the north half (z 0..=11)
        // is open floor with no walls or roof. A wall at z=12 has a two-high
        // doorway at x=4.
        let mut st = room([9, 6, 24]);
        st.blocks.retain(|(p, _)| p[2] >= 12 || p[1] == 0);
        for x in 0..9 {
            for y in 1..5 {
                if !(x == 4 && y <= 2) {
                    st.blocks.push(([x, y, 12], 0));
                }
            }
        }
        let grid = grid_of(&st);
        let cells = Cells::of(&st);
        // Anchor inside, at the south wall, facing south; the door is behind it.
        let s = stand_back(&cells, &grid, [4, 1, 22], [0, 0, 1]).unwrap();
        assert_eq!(s.cell, [4, 1, 12], "the camera stops in the doorway: {s:?}");
        assert_eq!(s.stop, Stop::CoverChanges, "{s:?}");
        // Without the roof test it would have walked on to the open floor's far
        // edge: the same walk over a piece with the whole north half roofed does.
        let mut roofed = st.clone();
        for x in 0..9 {
            for z in 0..12 {
                roofed.blocks.push(([x, 5, z], 0));
            }
        }
        let g2 = grid_of(&roofed);
        let c2 = Cells::of(&roofed);
        let s2 = stand_back(&c2, &g2, [4, 1, 22], [0, 0, 1]).unwrap();
        assert_eq!(s2.cell, [4, 1, 0], "{s2:?}");
        assert_eq!(s2.stop, Stop::FloorEnds, "{s2:?}");
    }

    /// **A hole in a roof is not the edge of a room.** A roofed hall with a
    /// three-column louvre across the walk: the camera walks under it to the far
    /// wall, because the louvre is shorter than the hall already crossed.
    #[test]
    fn a_louvre_over_the_walk_does_not_end_the_room() {
        let mut st = room([9, 8, 24]);
        st.blocks
            .retain(|(p, _)| !(p[1] == 7 && (3..=5).contains(&p[0]) && (10..=12).contains(&p[2])));
        let grid = grid_of(&st);
        let cells = Cells::of(&st);
        // The louvre's own columns are open to the sky, one cell at a time.
        assert!(!nav::sheltered(&cells, [4, 1, 11]));
        let s = stand_back(&cells, &grid, [4, 1, 22], [0, 0, 1]).unwrap();
        assert_eq!(s.cell, [4, 1, 1], "{s:?}");
        assert_eq!(s.stop, Stop::FloorEnds, "{s:?}");
    }

    /// **A passage as long as the open ground before it is another space.** Open
    /// ground, then a roofed passage at least as long, then open ground again:
    /// the camera stops at the passage mouth rather than crossing it to the far
    /// side, which is the perturbation the louvre test cannot see.
    #[test]
    fn a_roofed_passage_between_open_ground_ends_the_walk() {
        let mut st = room([9, 8, 24]);
        // Unroof z 16..=22 and z 0..=7; the roof stays over z 8..=15.
        st.blocks
            .retain(|(p, _)| !(p[1] == 7 && !(8..=15).contains(&p[2])));
        let grid = grid_of(&st);
        let cells = Cells::of(&st);
        let s = stand_back(&cells, &grid, [4, 1, 22], [0, 0, 1]).unwrap();
        assert_eq!(s.cell, [4, 1, 16], "{s:?}");
        assert_eq!(s.stop, Stop::CoverChanges, "{s:?}");
    }

    /// Nowhere to stand on the anchor is no room camera at all.
    #[test]
    fn an_anchor_with_no_standing_cell_has_no_room_camera() {
        let st = room([9, 6, 24]);
        let grid = grid_of(&st);
        let cells = Cells::of(&st);
        assert!(stand_back(&cells, &grid, [0, 3, 5], [0, 0, 1]).is_none());
    }

    #[test]
    fn measurement_and_standing_are_deterministic() {
        let st = room([9, 6, 24]);
        let grid = grid_of(&st);
        let cells = Cells::of(&st);
        let a = stand_back(&cells, &grid, [4, 1, 22], [0, 0, 1]);
        let b = stand_back(&cells, &grid, [4, 1, 22], [0, 0, 1]);
        assert_eq!(a, b);
        assert_eq!(
            frame(&st, [4, 1, 5], "north"),
            frame(&st, [4, 1, 5], "north")
        );
    }
}
