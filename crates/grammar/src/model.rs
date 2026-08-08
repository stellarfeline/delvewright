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
