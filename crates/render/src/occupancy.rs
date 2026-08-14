//! Where a body fits inside a prefab, and where its eye sits — the geometry the
//! eye-level shots stand on.
//!
//! A prefab is mostly rock. The motivating piece (`z4-chapel-ward`, 16×9×26) is
//! **81% solid**, and one of its six anchors sits inside a bank of iron bars: an
//! eye point derived from an anchor position alone lands inside a block roughly
//! as often as not, and a camera inside a block renders the inside of a block.
//! So the eye cell is *resolved*, never assumed, and the resolution is reported
//! ([`Placement`]) rather than applied silently.
//!
//! ## What "open" means here, and why it is strict
//!
//! [`Occupancy::is_open`] admits only true void: `air`, `cave_air`, `void_air`,
//! `structure_void`, and cells the template does not place at all (a vanilla
//! structure template may be sparse; an unplaced cell paints nothing and the
//! renderer draws nothing there). Everything else — glass, bars, a carpet — is
//! treated as occupied.
//!
//! The asymmetry is deliberate: being too strict costs a one-block step and a
//! recorded [`Placement`], while being too lenient costs a picture of the inside
//! of a block that looks like a picture of a room. The error is one-directional
//! on purpose.

use std::collections::HashMap;

use crate::nbt::Structure;

/// Player eye height above the standing cell's floor, in blocks — the same
/// 1.62 the compiler's player-POV cameras stand at, so a per-prefab eye shot and
/// a whole-scene POV shot of the same spot agree.
pub const EYE_HEIGHT: f32 = 1.62;

/// A cardinal facing keyword, and the two things a facing is for: the direction
/// a body walks/looks, and the yaw that points a camera along it.
///
/// One conversion, one home. The doorway shots want the yaw that puts a camera
/// *outside* a socket looking in ([`Facing::behind_yaw_deg`]); the eye shots want
/// the yaw that looks *along* the facing ([`Facing::view_yaw_deg`]). They are the
/// same conversion 180° apart, and deriving the second from the first is what
/// keeps a later change to Nucleation's camera convention a one-line change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Facing {
    North,
    South,
    East,
    West,
}

impl Facing {
    /// Parse a cardinal keyword. Non-cardinal (`up`/`down`/garbage) → `None`.
    pub fn parse(s: &str) -> Option<Facing> {
        match s {
            "north" => Some(Facing::North),
            "south" => Some(Facing::South),
            "east" => Some(Facing::East),
            "west" => Some(Facing::West),
            _ => None,
        }
    }

    /// The keyword, for metadata round-tripping.
    pub fn as_str(self) -> &'static str {
        match self {
            Facing::North => "north",
            Facing::South => "south",
            Facing::East => "east",
            Facing::West => "west",
        }
    }

    /// Unit step in Minecraft axes: north is `−Z`, east is `+X`.
    pub fn unit(self) -> [i32; 3] {
        match self {
            Facing::North => [0, 0, -1],
            Facing::South => [0, 0, 1],
            Facing::East => [1, 0, 0],
            Facing::West => [-1, 0, 0],
        }
    }

    /// Nucleation orbit yaw (degrees) whose **view direction is this facing**.
    ///
    /// Nucleation's view direction for `(yaw, pitch)` is
    /// `−[cos p·sin y, sin p, cos p·cos y]`, so at pitch 0 a yaw of 0 looks
    /// toward `−Z` (north), 90 toward `−X` (west).
    pub fn view_yaw_deg(self) -> f32 {
        match self {
            Facing::North => 0.0,
            Facing::West => 90.0,
            Facing::South => 180.0,
            Facing::East => 270.0,
        }
    }

    /// Nucleation orbit yaw that puts the camera on the far side of an object
    /// with this facing, looking back *along* it — the socket/doorway framing.
    pub fn behind_yaw_deg(self) -> f32 {
        (self.view_yaw_deg() + 180.0).rem_euclid(360.0)
    }
}

/// How the eye cell was arrived at. Never inferred by a reader from the image:
/// it rides the shot manifest, and anything but [`Placement::AnchorCell`] also
/// raises a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// The anchor's own cell holds a body. The image is taken exactly there.
    AnchorCell,
    /// The anchor cell is occupied, so the body stands `blocks` behind it along
    /// its facing — the anchor's object stays in frame, in the foreground.
    SteppedBack { blocks: i32 },
    /// Neither; the nearest open body cell that still has the anchor in front of
    /// it. `offset` is that cell minus the anchor cell.
    Nearest { offset: [i32; 3] },
}

impl Placement {
    /// A short machine tag for the shot manifest.
    pub fn tag(self) -> &'static str {
        match self {
            Placement::AnchorCell => "anchor-cell",
            Placement::SteppedBack { .. } => "stepped-back",
            Placement::Nearest { .. } => "nearest",
        }
    }
}

/// A resolved standing position for one anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Standing {
    /// The cell the body's feet occupy.
    pub cell: [i32; 3],
    /// How that cell was chosen.
    pub placement: Placement,
    /// Whether the cell directly below is solid. A body needs a floor; a shot
    /// without one is still worth taking (a ledge brink, a shaft) and says so.
    pub supported: bool,
}

impl Standing {
    /// The camera point: the cell's horizontal centre at [`EYE_HEIGHT`].
    pub fn eye(&self) -> [f32; 3] {
        [
            self.cell[0] as f32 + 0.5,
            self.cell[1] as f32 + EYE_HEIGHT,
            self.cell[2] as f32 + 0.5,
        ]
    }
}

/// How far the resolver will look for a body cell when the anchor's own is
/// occupied, in blocks. Three is a doorway's depth plus one: far enough to step
/// out of a gate or a barrel, near enough that the frame is still *about* the
/// anchor. Beyond it the shot is refused rather than taken somewhere else.
pub const SEARCH_RADIUS: i32 = 3;

/// What an eye ray meets, and after how many open cells.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Clearance {
    /// A block stops the ray after `open` clear cells.
    Blocked { open: i32, state: String },
    /// The ray reaches the edge of the template after `open` clear cells —
    /// beyond it is whatever the layout puts there, which a per-piece render
    /// draws as empty background.
    LeavesThePiece { open: i32 },
}

impl Clearance {
    /// Open cells ahead of the eye.
    pub fn open(&self) -> i32 {
        match self {
            Clearance::Blocked { open, .. } | Clearance::LeavesThePiece { open } => *open,
        }
    }

    /// What stops the ray, as one machine-readable string.
    pub fn stopped_by(&self) -> &str {
        match self {
            Clearance::Blocked { state, .. } => state,
            Clearance::LeavesThePiece { .. } => "<edge of the piece>",
        }
    }
}

/// A prefab's cells, answering "can a body be here".
pub struct Occupancy<'a> {
    size: [i32; 3],
    cells: HashMap<[i32; 3], &'a str>,
}

impl<'a> Occupancy<'a> {
    /// Index a parsed structure. Borrows the palette strings; no copy of the
    /// block data beyond the position map.
    pub fn new(st: &'a Structure) -> Self {
        let mut cells = HashMap::with_capacity(st.blocks.len());
        for (pos, idx) in &st.blocks {
            cells.insert(*pos, st.palette[*idx].as_str());
        }
        Occupancy {
            size: st.size,
            cells,
        }
    }

    /// The structure's `[sx, sy, sz]`.
    pub fn size(&self) -> [i32; 3] {
        self.size
    }

    fn in_bounds(&self, c: [i32; 3]) -> bool {
        (0..self.size[0]).contains(&c[0])
            && (0..self.size[1]).contains(&c[1])
            && (0..self.size[2]).contains(&c[2])
    }

    /// The block state at `c`, or `None` when the template places nothing there.
    pub fn state(&self, c: [i32; 3]) -> Option<&str> {
        self.cells.get(&c).copied()
    }

    /// Whether a body's volume can occupy this cell (see the module header for
    /// why the set is this small). Out of bounds is **not** open: a camera
    /// outside the piece is an exterior shot, and this resolves interior ones.
    pub fn is_open(&self, c: [i32; 3]) -> bool {
        if !self.in_bounds(c) {
            return false;
        }
        match self.state(c) {
            None => true,
            Some(s) => matches!(
                strip_state(s),
                "air" | "cave_air" | "void_air" | "structure_void"
            ),
        }
    }

    /// Whether a standing body fits: its feet cell and the cell above it.
    pub fn fits_body(&self, c: [i32; 3]) -> bool {
        self.is_open(c) && self.is_open([c[0], c[1] + 1, c[2]])
    }

    /// Whether the cell below `c` is solid — a floor to stand on.
    pub fn has_floor(&self, c: [i32; 3]) -> bool {
        let below = [c[0], c[1] - 1, c[2]];
        self.in_bounds(below) && !self.is_open(below)
    }

    /// What the eye meets looking along `facing` from the eye cell of a body
    /// standing at `cell` — a **measurement, not a verdict** (the grammar
    /// report's convention): how many open cells lie ahead, and what stops the
    /// ray, or that it leaves the piece.
    ///
    /// It exists because a frame cannot say how far away the thing filling it
    /// is. "Mostly wall" and "a corridor" are the same picture at different
    /// scales, and the number resolves them without asking the reviewer to
    /// count blocks in a texture.
    pub fn forward_clearance(&self, cell: [i32; 3], facing: Facing) -> Clearance {
        let f = facing.unit();
        let eye = [cell[0], cell[1] + 1, cell[2]];
        let mut open = 0;
        loop {
            let c = [
                eye[0] + f[0] * (open + 1),
                eye[1] + f[1] * (open + 1),
                eye[2] + f[2] * (open + 1),
            ];
            if !self.in_bounds(c) {
                return Clearance::LeavesThePiece { open };
            }
            if !self.is_open(c) {
                return Clearance::Blocked {
                    open,
                    state: self.state(c).unwrap_or("minecraft:air").to_string(),
                };
            }
            open += 1;
        }
    }

    /// Resolve where a body would stand to look along `facing` from `anchor`.
    ///
    /// In order: the anchor's own cell; then up to [`SEARCH_RADIUS`] blocks back
    /// along the facing, so the thing the anchor names stays in front of the
    /// camera; then the nearest open body cell within that radius which still
    /// has the anchor at or ahead of it (`(anchor − cell) · facing ≥ 0`) — a
    /// camera that has to look away from the anchor to be in open air answers a
    /// different question, so it is refused instead.
    ///
    /// Deterministic: the fallback scan is ordered by
    /// `(Chebyshev distance, |dy|, dy, dz, dx)` with no reliance on hash order.
    pub fn resolve_standing(&self, anchor: [i32; 3], facing: Facing) -> Option<Standing> {
        let finish = |cell: [i32; 3], placement: Placement| Standing {
            cell,
            placement,
            supported: self.has_floor(cell),
        };

        if self.fits_body(anchor) {
            return Some(finish(anchor, Placement::AnchorCell));
        }

        let f = facing.unit();
        for k in 1..=SEARCH_RADIUS {
            let c = [
                anchor[0] - f[0] * k,
                anchor[1] - f[1] * k,
                anchor[2] - f[2] * k,
            ];
            if self.fits_body(c) {
                return Some(finish(c, Placement::SteppedBack { blocks: k }));
            }
        }

        let mut best: Option<([i32; 5], [i32; 3])> = None;
        for dy in -SEARCH_RADIUS..=SEARCH_RADIUS {
            for dz in -SEARCH_RADIUS..=SEARCH_RADIUS {
                for dx in -SEARCH_RADIUS..=SEARCH_RADIUS {
                    let cheb = dx.abs().max(dy.abs()).max(dz.abs());
                    if cheb == 0 || cheb > SEARCH_RADIUS {
                        continue;
                    }
                    let c = [anchor[0] + dx, anchor[1] + dy, anchor[2] + dz];
                    // The anchor must not end up behind the camera.
                    if (-dx) * f[0] + (-dy) * f[1] + (-dz) * f[2] < 0 {
                        continue;
                    }
                    if !self.fits_body(c) {
                        continue;
                    }
                    let key = [cheb, dy.abs(), dy, dz, dx];
                    if best.as_ref().is_none_or(|(k, _)| key < *k) {
                        best = Some((key, c));
                    }
                }
            }
        }
        best.map(|(_, c)| {
            finish(
                c,
                Placement::Nearest {
                    offset: [c[0] - anchor[0], c[1] - anchor[1], c[2] - anchor[2]],
                },
            )
        })
    }
}

/// `minecraft:foo[a=b]` → `foo`.
fn strip_state(state: &str) -> &str {
    let name = state.split('[').next().unwrap_or(state);
    name.split_once(':').map(|(_, p)| p).unwrap_or(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 5×4×5 room: solid floor at y=0, solid shell, open interior at y=1..2,
    /// plus a bar of `iron_bars` filling the column x=2 at y=1..2 (the gate).
    fn room() -> Structure {
        let mut blocks = Vec::new();
        let mut palette = vec![
            "minecraft:air".to_string(),
            "minecraft:stone".to_string(),
            "minecraft:iron_bars".to_string(),
        ];
        palette.shrink_to_fit();
        for x in 0..5 {
            for y in 0..4 {
                for z in 0..5 {
                    let shell = x == 0 || x == 4 || z == 0 || z == 4 || y == 0 || y == 3;
                    let gate = x == 2 && (1..=2).contains(&y) && z == 2;
                    let idx = if shell {
                        1
                    } else if gate {
                        2
                    } else {
                        0
                    };
                    blocks.push(([x, y, z], idx));
                }
            }
        }
        Structure {
            size: [5, 4, 5],
            palette,
            blocks,
        }
    }

    #[test]
    fn facing_yaws_are_opposites_and_view_yaw_points_along_the_facing() {
        for f in [Facing::North, Facing::South, Facing::East, Facing::West] {
            let (y, p) = (f.view_yaw_deg().to_radians(), 0.0f32);
            // Nucleation view direction at pitch 0.
            let dir = [-(p.cos() * y.sin()), -(p.sin()), -(p.cos() * y.cos())];
            let u = f.unit();
            for i in 0..3 {
                assert!(
                    (dir[i] - u[i] as f32).abs() < 1e-6,
                    "{f:?}: view dir {dir:?} != facing {u:?}"
                );
            }
            assert_eq!(
                f.behind_yaw_deg(),
                (f.view_yaw_deg() + 180.0).rem_euclid(360.0)
            );
        }
        // The doorway framing this replaced, pinned: a north socket orbits at 180.
        assert_eq!(Facing::North.behind_yaw_deg(), 180.0);
        assert_eq!(Facing::South.behind_yaw_deg(), 0.0);
        assert_eq!(Facing::East.behind_yaw_deg(), 90.0);
        assert_eq!(Facing::West.behind_yaw_deg(), 270.0);
    }

    #[test]
    fn open_is_air_and_void_only() {
        let st = room();
        let occ = Occupancy::new(&st);
        assert!(occ.is_open([1, 1, 1]));
        assert!(!occ.is_open([0, 1, 1]), "shell is solid");
        assert!(!occ.is_open([2, 1, 2]), "iron bars occupy the cell");
        assert!(!occ.is_open([-1, 1, 1]), "outside the piece is not open");
        assert!(!occ.is_open([1, 9, 1]), "above the piece is not open");
    }

    #[test]
    fn an_open_anchor_cell_is_used_as_is() {
        let st = room();
        let occ = Occupancy::new(&st);
        let s = occ.resolve_standing([1, 1, 1], Facing::East).unwrap();
        assert_eq!(s.cell, [1, 1, 1]);
        assert_eq!(s.placement, Placement::AnchorCell);
        assert!(s.supported);
        assert_eq!(s.eye(), [1.5, 1.0 + EYE_HEIGHT, 1.5]);
    }

    #[test]
    fn an_occupied_anchor_steps_back_along_its_facing() {
        let st = room();
        let occ = Occupancy::new(&st);
        // The gate cell faces west; the body steps back to +X, so the bars stay
        // in frame between the camera and what is beyond them.
        let s = occ.resolve_standing([2, 1, 2], Facing::West).unwrap();
        assert_eq!(s.cell, [3, 1, 2]);
        assert_eq!(s.placement, Placement::SteppedBack { blocks: 1 });
        // Facing east from the same cell steps the other way, to −X.
        let s = occ.resolve_standing([2, 1, 2], Facing::East).unwrap();
        assert_eq!(s.cell, [1, 1, 2]);
    }

    #[test]
    fn a_body_needs_head_room_too() {
        let st = room();
        let occ = Occupancy::new(&st);
        // y=2 is open but y=3 is the ceiling, so no body fits at y=2.
        assert!(occ.is_open([1, 2, 1]));
        assert!(!occ.fits_body([1, 2, 1]));
    }

    #[test]
    fn an_anchor_with_no_reachable_body_cell_is_refused() {
        // A solid block of stone: nothing anywhere is open.
        let mut blocks = Vec::new();
        for x in 0..5 {
            for y in 0..5 {
                for z in 0..5 {
                    blocks.push(([x, y, z], 0usize));
                }
            }
        }
        let st = Structure {
            size: [5, 5, 5],
            palette: vec!["minecraft:deepslate".to_string()],
            blocks,
        };
        let occ = Occupancy::new(&st);
        assert!(occ.resolve_standing([2, 2, 2], Facing::North).is_none());
    }

    #[test]
    fn forward_clearance_measures_the_view_ahead() {
        let st = room();
        let occ = Occupancy::new(&st);
        // Standing at x=1 looking east: one open cell (x=2 at eye level is the
        // gate's own column, but the gate is at z=2 only) …
        assert_eq!(
            occ.forward_clearance([1, 1, 2], Facing::East),
            Clearance::Blocked {
                open: 0,
                state: "minecraft:iron_bars".to_string()
            }
        );
        // …and at z=1 the way is clear to the far wall.
        assert_eq!(
            occ.forward_clearance([1, 1, 1], Facing::East),
            Clearance::Blocked {
                open: 2,
                state: "minecraft:stone".to_string()
            }
        );
    }

    #[test]
    fn a_ray_that_reaches_the_template_edge_says_so() {
        // An open-ended box: no shell on the −X face.
        let mut blocks = Vec::new();
        for x in 0..4 {
            for y in 0..4 {
                for z in 0..4 {
                    let solid = x == 3 || z == 0 || z == 3 || y == 0 || y == 3;
                    blocks.push(([x, y, z], usize::from(solid)));
                }
            }
        }
        let st = Structure {
            size: [4, 4, 4],
            palette: vec!["minecraft:air".to_string(), "minecraft:stone".to_string()],
            blocks,
        };
        let occ = Occupancy::new(&st);
        assert_eq!(
            occ.forward_clearance([2, 1, 1], Facing::West),
            Clearance::LeavesThePiece { open: 2 }
        );
    }

    #[test]
    fn resolution_is_deterministic() {
        let st = room();
        let occ = Occupancy::new(&st);
        for _ in 0..8 {
            assert_eq!(
                occ.resolve_standing([2, 1, 2], Facing::North),
                occ.resolve_standing([2, 1, 2], Facing::North)
            );
        }
    }
}
