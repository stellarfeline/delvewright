//! Areas: the origin datum, the area and piece placements, and the templates a
//! piece stamps.

use super::*;

/// The Y of every area origin under `horizon: void` (structures carry their own
/// floor at local y=0). Also the fallback Y for an unresolvable position.
pub const BASE_Y: i32 = 64;
/// Sea level of the `ocean` horizon superflat (spec-0013): the pinned
/// bedrock/stone/water layer stack (1 + 118 + 8 from the -64 build floor) tops the
/// water at y=62. Emission pins the same stack in `generator-settings`.
pub const SEA_LEVEL: i32 = 62;
/// Height of the `ocean` horizon superflat's water layer (spec-0013) — the `8`
/// in the pinned `generator-settings` stack emission writes
/// (`emit::emit_server`). Ambient water occupies `SEA_LEVEL - 7 ..= SEA_LEVEL`.
pub const OCEAN_WATER_LAYERS: i32 = 8;
/// Y of the topmost ambient **solid** block of the `ocean` horizon superflat: the
/// sea floor (stone) directly under the water layers, at 54. The ambient model
/// boundary safety reasons about (`nav::Sea`) starts here — below it the world is
/// stone all the way to bedrock, which is why an ocean world has no void column
/// anywhere.
pub const SEA_FLOOR_TOP_Y: i32 = SEA_LEVEL - OCEAN_WATER_LAYERS;
/// The lowest y of the pinned overworld (`min_y`): the first layer of a
/// superflat stack stands here.
pub const BUILD_FLOOR_Y: i32 = -64;
/// **The `ocean` horizon superflat, bottom up**: each layer's block and height,
/// from [`BUILD_FLOOR_Y`]. The one statement of the stack: emission writes it as
/// `generator-settings` (`emit::emit_server`), and the world the engine writes
/// lays it in every cell no build write reaches ([`ocean_layer_at`]), because
/// that is what the server holds there.
pub const OCEAN_LAYERS: [(&str, i32); 3] = [
    ("minecraft:bedrock", 1),
    ("minecraft:stone", 118),
    ("minecraft:water", OCEAN_WATER_LAYERS),
];
const _: () = assert!(
    BUILD_FLOOR_Y + OCEAN_LAYERS[0].1 + OCEAN_LAYERS[1].1 + OCEAN_LAYERS[2].1 - 1 == SEA_LEVEL,
    "the ocean stack tops its water at SEA_LEVEL"
);
/// The block the `ocean` superflat generates at height `y`, or `None` above
/// the stack (air) and below the build floor.
pub fn ocean_layer_at(y: i32) -> Option<&'static str> {
    let mut floor = BUILD_FLOOR_Y;
    for (block, height) in OCEAN_LAYERS {
        if (floor..floor + height).contains(&y) {
            return Some(block);
        }
        floor += height;
    }
    None
}

/// A placed area: one or more pieces plus their socket seals.
pub struct AreaPlacement {
    /// Area id (`area/…`).
    pub area_id: String,
    /// The placed pieces: the blockout's records of every volume and box, then
    /// the detail plan's pieces in `details[]` order.
    pub pieces: Vec<PiecePlacement>,
    /// **The region writes the site's own blocks arrive as**, in the order the
    /// world applies them — before the templates
    /// (`crate::compiler::assembled::placed_blocks`). The blockout
    /// (`crate::compiler::blockout`) is the one producer — a shell of uniform
    /// boxes, whose natural packaging is a fill and whose `.nbt` packaging would
    /// be tens of thousands of mostly-air cells split across tiles.
    pub mass: Vec<SealFill>,
}

impl AreaPlacement {
    /// The union world AABB `(min, max)` covering every placed piece. For a
    /// single-prefab area this is exactly `origin .. origin+size-1`.
    pub fn bounds(&self) -> ([i32; 3], [i32; 3]) {
        let mut min = [i32::MAX; 3];
        let mut max = [i32::MIN; 3];
        for piece in &self.pieces {
            let (pmin, pmax) = piece.bbox();
            for a in 0..3 {
                min[a] = min[a].min(pmin[a]);
                max[a] = max[a].max(pmax[a]);
            }
        }
        (min, max)
    }
}

/// One placed structure piece.
pub struct PiecePlacement {
    /// Bound prefab id (`prefab/…`).
    pub prefab_id: String,
    /// The structure templates this piece's blocks arrive in, each already
    /// placed in world space — one for a single-template prefab, one per tile
    /// for a zone past the vanilla 48-per-axis cap.
    ///
    /// **A piece is one piece however many files it ships as.** Tiling is a
    /// packaging fact about a file format, so it is absorbed here, at the one
    /// place a `.nbt` filename is reachable from: everything above this — the
    /// area's anchors, its seals, massing — sees the piece the author bound, at its size, with its
    /// rotation. Everything below it emits one `/place template` per entry and
    /// never asks how many there were.
    pub templates: Vec<PlacedTemplate>,
    /// World-space `/place template` position `[x, y, z]` of the PIECE (where
    /// piece-local `(0,0,0)` lands). Each template's own position is derived
    /// from it and is on [`PlacedTemplate::pos`].
    pub pos: [i32; 3],
    /// Unrotated prefab size `[sx, sy, sz]` — the WHOLE piece, from prefab
    /// metadata, never one tile's extent.
    pub size: [i32; 3],
    /// Placement rotation (identity for single-prefab areas).
    pub rotation: Rotation,
    /// **Which of this piece's jigsaw sockets the layout mated**, index-aligned
    /// with the prefab's `connectors` — the solver's own claim about where this
    /// piece joins another, carried out of the solver rather than recomputed.
    ///
    /// It is what makes a mated socket falsifiable at all. Every other consumer
    /// reads it to decide whether to clear the doorway to air or seal it with
    /// wall material (`solver::seal_layout`), and until it reached here nothing
    /// ever asked the complementary question: **is the piece it says it mated to
    /// actually there?** A layout that moved a piece off its seam still emits an
    /// opened doorway, into whatever the world has at that plane.
    ///
    /// Empty for a piece nothing mated — a single-prefab area's lone piece, a
    /// derived blockout box, a detail piece, the surround — which reads as *no
    /// socket is mated*, and is the honest answer for each of them.
    pub mated: Vec<bool>,
}

/// One structure template of a placed piece, in world space.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedTemplate {
    /// Datapack structure id path segment (e.g. `hello-room`, or
    /// `z0-barrow-shore.x0y0z1` for a tile).
    pub structure_id: String,
    /// Structure `.nbt` filename (relative to `prefabs/`).
    pub structure_file: String,
    /// World-space `/place template` position `[x, y, z]` — where THIS
    /// template's local `(0,0,0)` lands, already carrying the piece rotation
    /// applied to the template's piece-local offset. Equal to the piece's own
    /// `pos` for a single-template prefab.
    pub pos: [i32; 3],
    /// The template's extent `[x, y, z]`, unrotated.
    pub size: [i32; 3],
}

impl PiecePlacement {
    /// The world AABB `(min, max)` of this placed piece, for chunk `forceload`.
    pub fn bbox(&self) -> ([i32; 3], [i32; 3]) {
        self.rotation.bbox(self.pos, self.size)
    }
}

/// The templates a piece's metadata declares, placed in world space at `pos`
/// under `rotation`.
///
/// Vanilla `/place template … <rotation>` rotates about the placement position,
/// so a tile at piece-local `offset` lands at `pos + rotation(offset)` and its
/// own cells then rotate about that — which composes to exactly
/// `pos + rotation(offset + local)`, the whole zone rotated about the piece
/// origin. That identity is what lets a tiled zone be rotated at all, and it is
/// the same arithmetic [`Rotation::bbox`] already uses.
pub(crate) fn placed_templates(
    meta: &delvewright_dsl::prefab::PrefabMeta,
    pos: [i32; 3],
    rotation: Rotation,
) -> Vec<PlacedTemplate> {
    meta.templates()
        .into_iter()
        .map(|t| {
            let o = rotation.transform(t.offset);
            PlacedTemplate {
                structure_id: t.id.to_string(),
                structure_file: t.file.to_string(),
                pos: [pos[0] + o[0], pos[1] + o[1], pos[2] + o[2]],
                size: t.size,
            }
        })
        .collect()
}

impl<'a> Plan<'a> {
    /// **Every placed piece in this build**, area pieces and the horizon
    /// surround alike.
    ///
    /// The one iterator every PLACEMENT site reads — the structure files that
    /// ship, the chunks that forceload, the `/place template` lines, the
    /// placement sentinels, and the voxel model the proofs run over. It exists
    /// so that "the surround is placed like any other piece" is one fact in one
    /// place rather than five parallel additions, four of which the next
    /// placement site would forget.
    ///
    /// Deliberately NOT the iterator for anything that reasons about CONTENT:
    /// the boundary region, the relight scope, the anchor table and analysis
    /// read `areas` and must go on reading `areas`, because a mountain is not
    /// somewhere the campaign happens.
    pub fn placed_pieces(&self) -> impl Iterator<Item = &PiecePlacement> {
        self.areas
            .iter()
            .flat_map(|a| a.pieces.iter())
            .chain(self.surround.iter().map(|s| &s.piece))
    }

    /// The AABB of the assembled piece carrying `cell` in `area_id` — "the room
    /// this cell was authored inside". Falls back to the whole area's bounds when
    /// the cell sits in no single piece box (defensive; a single-prefab area has
    /// exactly one piece == the area), and to the degenerate `(cell, cell)` when
    /// the area is not placed at all.
    ///
    /// The confinement boundary for anything that must not silently leave the
    /// piece it was declared in: wave seating
    /// ([`crate::compiler::nav::World::confined_standable_cells`]) and anchor seating
    /// ([`crate::compiler::nav::AnchorRoot`]).
    pub fn piece_bounds(&self, area_id: &str, cell: [i32; 3]) -> ([i32; 3], [i32; 3]) {
        let Some(area) = self.areas.iter().find(|a| a.area_id == area_id) else {
            return (cell, cell);
        };
        for piece in &area.pieces {
            let (lo, hi) = piece.bbox();
            if (0..3).all(|i| lo[i] <= cell[i] && cell[i] <= hi[i]) {
                return (lo, hi);
            }
        }
        area.bounds()
    }
}
