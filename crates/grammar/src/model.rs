//! The expanded voxel model — what a grammar program derives to.
//!
//! Upstream has no model: `MCScope.set_material` writes straight into an Amulet
//! world, so nothing downstream can inspect, compare or hash the result. The
//! model is the artifact the rest of spec-0027 works over: the craft diagnostics
//! of §4 read it, the `.nbt` export of §2 writes it out, and the determinism
//! gate hashes it.

use std::collections::BTreeMap;
use std::fmt;

use crate::block::BlockState;
use crate::geom::Box3;

/// One of the eight ways a finished building can be **set down**: the four yaw
/// rotations about the vertical, each with and without a horizontal mirror.
///
/// Written as three independent flags because that is exactly the dihedral
/// group of the square — swap the two horizontal axes, reverse either of them —
/// which makes it obvious by inspection that the eight are closed under
/// composition, i.e. that "same building" below is an equivalence and not
/// merely a pairwise test.
///
/// The vertical is deliberately absent. Gravity is not a symmetry of a
/// building: a barrow turned upside down is a different barrow, and nothing
/// downstream can set it down that way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Placement {
    /// Swap the two horizontal axes.
    swap: bool,
    /// Reverse the resulting X.
    flip_x: bool,
    /// Reverse the resulting Z.
    flip_z: bool,
}

impl Placement {
    /// The eight, in a fixed order — the canonical form is a minimum over this
    /// list, so the order is part of the determinism contract (ADR-0006).
    const ALL: [Placement; 8] = [
        Placement::new(false, false, false),
        Placement::new(false, false, true),
        Placement::new(false, true, false),
        Placement::new(false, true, true),
        Placement::new(true, false, false),
        Placement::new(true, false, true),
        Placement::new(true, true, false),
        Placement::new(true, true, true),
    ];

    const fn new(swap: bool, flip_x: bool, flip_z: bool) -> Placement {
        Placement {
            swap,
            flip_x,
            flip_z,
        }
    }

    /// True when this placement is a proper rotation — an even number of axis
    /// reversals once the swap (itself one reversal's worth of handedness) is
    /// counted. A caller that needs to say *which kind* of sameness it found
    /// asks this; the metric itself does not care.
    const fn is_rotation(self) -> bool {
        (self.swap as u8 + self.flip_x as u8 + self.flip_z as u8).is_multiple_of(2)
    }
}

/// How many distinct block states one model can hold: cells are `u16` palette
/// indices, so index `u16::MAX` is the last usable one.
pub const MAX_PALETTE: usize = u16::MAX as usize + 1;

/// A write needed a palette slot the `u16` cell encoding cannot address.
///
/// Reaching this is an authoring accident (a per-cell mix over tens of thousands
/// of distinct states), not a physical impossibility — so it is an error value
/// the expander turns into [`crate::ExpandError::PaletteFull`], never a panic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaletteFull {
    /// The palette size that cannot be exceeded.
    pub limit: usize,
}

impl fmt::Display for PaletteFull {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the model already holds {} distinct block states, the most a u16 cell can index",
            self.limit
        )
    }
}

impl std::error::Error for PaletteFull {}

/// A dense grid of block states over an integer box.
///
/// Cells are palette indices; index 0 is always air, so a freshly allocated
/// model is empty. The palette grows in first-write order, which is
/// deterministic because expansion is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelModel {
    region: Box3,
    palette: Vec<BlockState>,
    index_of: BTreeMap<BlockState, u16>,
    cells: Vec<u16>,
}

impl VoxelModel {
    /// An all-air model covering `region`.
    pub fn new(region: Box3) -> VoxelModel {
        let air = BlockState::air();
        VoxelModel {
            region,
            palette: vec![air.clone()],
            index_of: BTreeMap::from([(air, 0)]),
            cells: vec![0; region.volume() as usize],
        }
    }

    /// The box the model covers.
    pub fn region(&self) -> Box3 {
        self.region
    }

    /// The palette, in index order; entry 0 is air.
    pub fn palette(&self) -> &[BlockState] {
        &self.palette
    }

    /// The block at a world position, or `None` outside the region.
    pub fn get(&self, pos: [i32; 3]) -> Option<&BlockState> {
        let index = self.offset(pos)?;
        Some(&self.palette[self.cells[index] as usize])
    }

    /// Write a block at a world position.
    ///
    /// The model is the boundary that guarantees expansion stays inside its box,
    /// so a position outside the region is a **defect in the caller**, not an
    /// input: a debug build asserts, and a release build drops the write rather
    /// than wrapping it into an unrelated cell. [`VoxelModel::try_set`] is the
    /// same write without the assertion, for the boundary's own tests.
    pub fn set(&mut self, pos: [i32; 3], block: &BlockState) -> Result<(), PaletteFull> {
        let landed = self.try_set(pos, block)?;
        debug_assert!(
            landed,
            "wrote {pos:?} outside the model region {:?} — expansion must never leave its box",
            self.region
        );
        Ok(())
    }

    /// [`VoxelModel::set`] without the in-region assertion, reporting whether the
    /// write landed. Private: the only caller that may legitimately aim outside
    /// the region is the test that pins what happens when something does.
    fn try_set(&mut self, pos: [i32; 3], block: &BlockState) -> Result<bool, PaletteFull> {
        let Some(index) = self.offset(pos) else {
            return Ok(false);
        };
        let id = match self.index_of.get(block) {
            Some(id) => *id,
            None => {
                let id = u16::try_from(self.palette.len())
                    .map_err(|_| PaletteFull { limit: MAX_PALETTE })?;
                self.palette.push(block.clone());
                self.index_of.insert(block.clone(), id);
                id
            }
        };
        self.cells[index] = id;
        Ok(true)
    }

    /// How many cells hold something other than air.
    pub fn filled_cells(&self) -> usize {
        self.cells
            .iter()
            .filter(|&&c| !self.palette[c as usize].is_air())
            .count()
    }

    /// A canonical byte encoding: the bytes the determinism gate hashes and
    /// compares. Self-describing, endian-fixed, and independent of anything but
    /// the model's content.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(64 + self.cells.len() * 2);
        out.extend_from_slice(b"DWGRAMMARv1\n");
        for v in self.region.origin {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for v in self.region.size {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(&(self.palette.len() as u32).to_le_bytes());
        for block in &self.palette {
            let s = block.to_string();
            out.extend_from_slice(&(s.len() as u32).to_le_bytes());
            out.extend_from_slice(s.as_bytes());
        }
        for cell in &self.cells {
            out.extend_from_slice(&cell.to_le_bytes());
        }
        out
    }

    /// The tight box around every cell that is not air, or `None` when the
    /// model is empty.
    ///
    /// The building, as opposed to the box someone expanded it in: the padding
    /// of air around it is a fact about the request, not about what was built.
    pub fn occupied_box(&self) -> Option<Box3> {
        let mut min = [i32::MAX; 3];
        let mut max = [i32::MIN; 3];
        for pos in self.region.positions() {
            let index = self.offset(pos).expect("a position of the model's own box");
            if self.palette[self.cells[index] as usize].is_air() {
                continue;
            }
            for a in 0..3 {
                min[a] = min[a].min(pos[a]);
                max[a] = max[a].max(pos[a]);
            }
        }
        if min[0] == i32::MAX {
            return None;
        }
        Some(Box3::new(
            min,
            [
                (max[0] - min[0] + 1) as u32,
                (max[1] - min[1] + 1) as u32,
                (max[2] - min[2] + 1) as u32,
            ],
        ))
    }

    /// Bytes identifying this model **up to placement**: the building, not the
    /// pose it happens to have been expanded in.
    ///
    /// Two models share these bytes exactly when one can be carried onto the
    /// other by a whole-body move a placement could undo — a translation, a
    /// yaw rotation, a horizontal mirror — and their blocks then agree cell for
    /// cell. It is [`VoxelModel::canonical_bytes`] quotiented by that group,
    /// and nothing else: paint, shape, height, footprint and every internal
    /// arrangement still separate two models here.
    ///
    /// This is what a *count of distinct buildings* has to hash. The pose is
    /// not the author's decision — the frame assigns it, so `z(Largest)` alone
    /// gives one building two identities the moment a sweep transposes its
    /// region — and the pose is not the reviewer's decision either, because a
    /// zone is set down wherever the campaign wants it.
    ///
    /// Not to be confused with [`VoxelModel::canonical_bytes`], which is the
    /// determinism gate's encoding and must stay pose-sensitive: "the same
    /// expansion twice" is a different question from "the same building twice",
    /// and an expansion that started answering the first with the second would
    /// hide a real drift.
    pub fn placement_canonical_bytes(&self) -> Vec<u8> {
        self.placement_canonical(true)
    }

    /// The same identity over the solid/air bitmap alone — **massing** up to
    /// placement.
    ///
    /// Blind to which block is where, exactly as
    /// [`VoxelModel::placement_canonical_bytes`] is not: that is the difference
    /// between "a different building" and "the same building in a different
    /// stone".
    pub fn placement_canonical_massing(&self) -> Vec<u8> {
        self.placement_canonical(false)
    }

    /// The lexicographic minimum over the eight placements — a canonical
    /// representative, hence a well-defined identity for the whole class.
    fn placement_canonical(&self, paint: bool) -> Vec<u8> {
        Placement::ALL
            .iter()
            .map(|p| self.placed_bytes(*p, paint))
            .min()
            .expect("eight placements")
    }

    /// This model cropped to its building, set down under `placement`, encoded.
    fn placed_bytes(&self, placement: Placement, paint: bool) -> Vec<u8> {
        let tag: &[u8] = if paint {
            b"DWBUILDINGv1\n"
        } else {
            b"DWMASSINGv1\n"
        };
        let Some(crop) = self.occupied_box() else {
            // An empty model is one building — the absent one — however it is
            // turned, so every placement encodes the same way.
            let mut out = tag.to_vec();
            out.extend_from_slice(&[0u8; 12]);
            return out;
        };
        let [cx, cy, cz] = crop.size;
        let (nx, nz) = if placement.swap { (cz, cx) } else { (cx, cz) };

        // Written into rather than read from: the forward map is the one the
        // placement states, and inverting it here would be a second, silently
        // divergent statement of the same thing.
        let mut cells = vec![0u16; (nx as usize) * (cy as usize) * (nz as usize)];
        for pos in crop.positions() {
            let src = self.offset(pos).expect("crop lies inside the region");
            let d = [
                (pos[0] - crop.origin[0]) as u32,
                (pos[1] - crop.origin[1]) as u32,
                (pos[2] - crop.origin[2]) as u32,
            ];
            let (mut a, mut b) = if placement.swap {
                (d[2], d[0])
            } else {
                (d[0], d[2])
            };
            if placement.flip_x {
                a = nx - 1 - a;
            }
            if placement.flip_z {
                b = nz - 1 - b;
            }
            cells[((a * cy + d[1]) * nz + b) as usize] = self.cells[src];
        }

        let mut out = Vec::with_capacity(tag.len() + 32 + cells.len() * 2);
        out.extend_from_slice(tag);
        for v in [nx, cy, nz] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        if !paint {
            for cell in &cells {
                out.push(u8::from(!self.palette[*cell as usize].is_air()));
            }
            return out;
        }
        // Palette indices are first-write order, which is a fact about how the
        // expansion ran and not about the building — so they are re-numbered in
        // the canonical scan order before they are hashed.
        let mut renumbered: BTreeMap<u16, u16> = BTreeMap::new();
        let mut order: Vec<u16> = Vec::new();
        let mut indices = Vec::with_capacity(cells.len());
        for cell in &cells {
            let next = renumbered.len() as u16;
            let id = *renumbered.entry(*cell).or_insert_with(|| {
                order.push(*cell);
                next
            });
            indices.push(id);
        }
        out.extend_from_slice(&(order.len() as u32).to_le_bytes());
        for old in &order {
            let s = self.palette[*old as usize].to_string();
            out.extend_from_slice(&(s.len() as u32).to_le_bytes());
            out.extend_from_slice(s.as_bytes());
        }
        for id in &indices {
            out.extend_from_slice(&id.to_le_bytes());
        }
        out
    }

    /// Which placements carry this model's **massing** onto `other`'s, if any.
    ///
    /// The metric answers "same or not"; this answers "same **how**", which is
    /// what a reviewer needs when a count drops and what a test needs to state
    /// that a merge was a rotation rather than a coincidence. Returns the
    /// placements as `(is_rotation, index)` pairs in [`Placement::ALL`] order.
    pub fn placements_onto(&self, other: &VoxelModel) -> Vec<(bool, usize)> {
        let mine: Vec<Vec<u8>> = Placement::ALL
            .iter()
            .map(|p| self.placed_bytes(*p, false))
            .collect();
        let theirs = other.placed_bytes(Placement::new(false, false, false), false);
        Placement::ALL
            .iter()
            .enumerate()
            .filter(|(i, _)| mine[*i] == theirs)
            .map(|(i, p)| (p.is_rotation(), i))
            .collect()
    }

    fn offset(&self, pos: [i32; 3]) -> Option<usize> {
        let max = self.region.maximum();
        for axis in 0..3 {
            if pos[axis] < self.region.origin[axis] || pos[axis] >= max[axis] {
                return None;
            }
        }
        let [_, sy, sz] = self.region.size;
        let dx = (pos[0] - self.region.origin[0]) as usize;
        let dy = (pos[1] - self.region.origin[1]) as usize;
        let dz = (pos[2] - self.region.origin[2]) as usize;
        Some((dx * sy as usize + dy) * sz as usize + dz)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_empty_and_records_writes() {
        let mut m = VoxelModel::new(Box3::new([-2, 0, 5], [3, 4, 2]));
        assert_eq!(m.filled_cells(), 0);
        assert_eq!(m.get([-2, 0, 5]), Some(&BlockState::air()));
        let stone = BlockState::simple("stone");
        m.set([-1, 2, 6], &stone).unwrap();
        assert_eq!(m.get([-1, 2, 6]), Some(&stone));
        assert_eq!(m.filled_cells(), 1);
        assert_eq!(m.palette(), &[BlockState::air(), stone]);
    }

    #[test]
    fn writes_outside_the_region_are_dropped_not_wrapped() {
        let mut m = VoxelModel::new(Box3::at_origin([2, 2, 2]));
        assert_eq!(
            m.try_set([9, 9, 9], &BlockState::simple("stone")),
            Ok(false)
        );
        assert_eq!(
            m.try_set([-1, 0, 0], &BlockState::simple("stone")),
            Ok(false)
        );
        assert_eq!(m.filled_cells(), 0);
        assert_eq!(m.get([9, 9, 9]), None);
    }

    /// ...and in a debug build the same write is a *bug*, not a no-op: nothing
    /// in expansion may aim outside its box, so the boundary says so loudly
    /// where a developer will see it (PR #266 review).
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "outside the model region")]
    fn a_write_outside_the_region_trips_a_debug_assert() {
        let mut m = VoxelModel::new(Box3::at_origin([2, 2, 2]));
        let _ = m.set([9, 9, 9], &BlockState::simple("stone"));
    }

    /// A `u16` cell cannot index past 65536 states. Upstream of this the code
    /// `expect`ed, i.e. a grammar could crash the tool; now the boundary reports
    /// and the expander turns it into an `ExpandError`.
    #[test]
    fn a_palette_past_the_u16_ceiling_is_an_error_not_a_panic() {
        // 65536 cells: air + 65535 distinct states fills the palette exactly.
        let mut m = VoxelModel::new(Box3::at_origin([256, 1, 257]));
        for i in 0..MAX_PALETTE - 1 {
            let block = BlockState::with("stone", [("n", &*i.to_string())]);
            let pos = [(i / 257) as i32, 0, (i % 257) as i32];
            m.set(pos, &block).expect("still inside the u16 ceiling");
        }
        assert_eq!(m.palette().len(), MAX_PALETTE);
        // One more distinct state has nowhere to go.
        let overflow = BlockState::with("stone", [("n", "overflow")]);
        assert_eq!(
            m.set([255, 0, 256], &overflow),
            Err(PaletteFull { limit: MAX_PALETTE })
        );
        // ...but a state already in the palette still writes.
        let known = BlockState::with("stone", [("n", "0")]);
        assert_eq!(m.set([255, 0, 256], &known), Ok(()));
    }

    #[test]
    fn canonical_bytes_track_content_only() {
        let mut a = VoxelModel::new(Box3::at_origin([2, 2, 2]));
        let mut b = VoxelModel::new(Box3::at_origin([2, 2, 2]));
        assert_eq!(a.canonical_bytes(), b.canonical_bytes());
        a.set([0, 0, 0], &BlockState::simple("stone")).unwrap();
        assert_ne!(a.canonical_bytes(), b.canonical_bytes());
        b.set([0, 0, 0], &BlockState::simple("stone")).unwrap();
        assert_eq!(a.canonical_bytes(), b.canonical_bytes());
    }

    /// An L of stone, so the shape has a handedness and no accidental symmetry.
    fn ell(region: Box3, at: [i32; 3], turn: usize, mirror: bool) -> VoxelModel {
        let mut m = VoxelModel::new(region);
        let stone = BlockState::simple("stone");
        // (dx, dz) offsets of the L's arms: three along X, two more along Z.
        let cells = [(0, 0), (1, 0), (2, 0), (0, 1), (0, 2)];
        for (dx, dz) in cells {
            let (mut x, mut z) = (dx, dz);
            for _ in 0..turn {
                // A quarter turn about the vertical.
                let (nx, nz) = (z, -x);
                x = nx;
                z = nz;
            }
            if mirror {
                x = -x;
            }
            m.set([at[0] + x, at[1], at[2] + z], &stone).unwrap();
        }
        m
    }

    /// The group the identity quotients, one member at a time.
    ///
    /// A building is the same building wherever it is set down and whichever
    /// way it is turned, because none of that is the author's decision — the
    /// frame assigns the pose and the campaign assigns the spot. This is the
    /// property the sweep's "distinct massings" count needs, and it is a
    /// property of the model, not of the sweep.
    #[test]
    fn a_building_is_the_same_building_turned_mirrored_or_moved() {
        let base = ell(Box3::at_origin([9, 1, 9]), [3, 0, 3], 0, false);
        let want = base.placement_canonical_massing();
        for turn in 0..4 {
            for mirror in [false, true] {
                // ...and in a differently-sized box, at a different corner of
                // it, so translation and the box's own padding go too.
                let other = ell(
                    Box3::new([-4, 7, 100], [13, 1, 11]),
                    [0, 7, 105],
                    turn,
                    mirror,
                );
                assert_eq!(
                    other.placement_canonical_massing(),
                    want,
                    "turn {turn}, mirror {mirror} came out a different building"
                );
                assert_eq!(
                    other.placement_canonical_bytes(),
                    base.placement_canonical_bytes(),
                    "turn {turn}, mirror {mirror}: same blocks, different identity"
                );
            }
        }
    }

    /// ...and the group stops there, deliberately. Every one of these is a
    /// genuinely different building, and a metric that merged them would hide a
    /// choice from the reviewer instead of inventing one — the same defect
    /// pointing the other way.
    #[test]
    fn shape_height_scale_and_the_vertical_still_separate_two_buildings() {
        let base = ell(Box3::at_origin([9, 1, 9]), [3, 0, 3], 0, false);
        let flat = base.placement_canonical_massing();
        let stone = BlockState::simple("stone");

        // Upside down. Gravity is not a symmetry: a two-storey L standing on
        // its short arm is not the same building inverted.
        let mut upright = VoxelModel::new(Box3::at_origin([9, 4, 9]));
        let mut inverted = VoxelModel::new(Box3::at_origin([9, 4, 9]));
        for (dx, dz) in [(0, 0), (1, 0), (2, 0), (0, 1), (0, 2)] {
            upright.set([dx, 0, dz], &stone).unwrap();
            inverted.set([dx, 3, dz], &stone).unwrap();
        }
        upright.set([0, 1, 0], &stone).unwrap();
        inverted.set([0, 2, 0], &stone).unwrap();
        assert_ne!(
            upright.placement_canonical_massing(),
            inverted.placement_canonical_massing(),
            "a building and the same building upside down are two buildings"
        );

        // One cell more.
        let mut bigger = ell(Box3::at_origin([9, 1, 9]), [3, 0, 3], 0, false);
        bigger.set([6, 0, 3], &stone).unwrap();
        assert_ne!(bigger.placement_canonical_massing(), flat);

        // Twice the size. Nothing in the group scales.
        let mut doubled = VoxelModel::new(Box3::at_origin([9, 1, 9]));
        for (dx, dz) in [(0, 0), (1, 0), (2, 0), (0, 1), (0, 2)] {
            for (ex, ez) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                doubled.set([dx * 2 + ex, 0, dz * 2 + ez], &stone).unwrap();
            }
        }
        assert_ne!(doubled.placement_canonical_massing(), flat);

        // Paint: one identity sees it, the other does not — the whole reason
        // there are two.
        let mut painted = ell(Box3::at_origin([9, 1, 9]), [3, 0, 3], 0, false);
        painted
            .set([3, 0, 3], &BlockState::simple("deepslate"))
            .unwrap();
        assert_eq!(painted.placement_canonical_massing(), flat);
        assert_ne!(
            painted.placement_canonical_bytes(),
            base.placement_canonical_bytes()
        );
    }

    /// The determinism gate asks a different question and must keep asking it.
    #[test]
    fn the_determinism_encoding_stays_pose_sensitive() {
        let base = ell(Box3::at_origin([9, 1, 9]), [3, 0, 3], 0, false);
        let turned = ell(Box3::at_origin([9, 1, 9]), [3, 0, 3], 1, false);
        assert_eq!(
            base.placement_canonical_massing(),
            turned.placement_canonical_massing()
        );
        assert_ne!(
            base.canonical_bytes(),
            turned.canonical_bytes(),
            "`canonical_bytes` answers `the same expansion twice`, not `the same \
             building twice`; blurring the two would hide a real drift"
        );
    }

    #[test]
    fn an_empty_model_has_no_building_and_one_identity() {
        let a = VoxelModel::new(Box3::at_origin([4, 4, 4]));
        let b = VoxelModel::new(Box3::new([9, 9, 9], [2, 7, 3]));
        assert_eq!(a.occupied_box(), None);
        assert_eq!(
            a.placement_canonical_massing(),
            b.placement_canonical_massing()
        );
        assert_eq!(a.placement_canonical_bytes(), b.placement_canonical_bytes());

        let mut one = VoxelModel::new(Box3::at_origin([4, 4, 4]));
        one.set([1, 2, 3], &BlockState::simple("stone")).unwrap();
        assert_eq!(one.occupied_box(), Some(Box3::new([1, 2, 3], [1, 1, 1])));
        assert_ne!(
            one.placement_canonical_massing(),
            a.placement_canonical_massing()
        );
    }

    #[test]
    fn distinct_block_states_are_distinct_cells() {
        let mut m = VoxelModel::new(Box3::at_origin([2, 1, 1]));
        let east = BlockState::with("oak_stairs", [("facing", "east")]);
        let west = BlockState::with("oak_stairs", [("facing", "west")]);
        m.set([0, 0, 0], &east).unwrap();
        m.set([1, 0, 0], &west).unwrap();
        assert_eq!(m.palette().len(), 3, "air + two states of the same block");
        assert_ne!(m.get([0, 0, 0]), m.get([1, 0, 0]));
    }
}
