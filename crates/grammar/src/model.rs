//! The expanded voxel model — what a grammar program derives to.
//!
//! Upstream has no model: `MCScope.set_material` writes straight into an Amulet
//! world, so nothing downstream can inspect, compare or hash the result. The
//! model is the artifact the rest of spec-0027 works over: the craft diagnostics
//! of §4 read it, the `.nbt` export of §2 writes it out, and the determinism
//! gate hashes it.

use std::collections::BTreeMap;

use crate::block::BlockState;
use crate::geom::Box3;

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

    /// Write a block at a world position. Positions outside the region are
    /// ignored — expansion never produces one, and the model is the boundary
    /// that guarantees it.
    pub fn set(&mut self, pos: [i32; 3], block: &BlockState) {
        let Some(index) = self.offset(pos) else {
            return;
        };
        let id = match self.index_of.get(block) {
            Some(id) => *id,
            None => {
                let id = u16::try_from(self.palette.len())
                    .expect("a grammar model cannot need more than 65536 distinct block states");
                self.palette.push(block.clone());
                self.index_of.insert(block.clone(), id);
                id
            }
        };
        self.cells[index] = id;
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
        m.set([-1, 2, 6], &stone);
        assert_eq!(m.get([-1, 2, 6]), Some(&stone));
        assert_eq!(m.filled_cells(), 1);
        assert_eq!(m.palette(), &[BlockState::air(), stone]);
    }

    #[test]
    fn writes_outside_the_region_are_dropped_not_wrapped() {
        let mut m = VoxelModel::new(Box3::at_origin([2, 2, 2]));
        m.set([9, 9, 9], &BlockState::simple("stone"));
        m.set([-1, 0, 0], &BlockState::simple("stone"));
        assert_eq!(m.filled_cells(), 0);
        assert_eq!(m.get([9, 9, 9]), None);
    }

    #[test]
    fn canonical_bytes_track_content_only() {
        let mut a = VoxelModel::new(Box3::at_origin([2, 2, 2]));
        let mut b = VoxelModel::new(Box3::at_origin([2, 2, 2]));
        assert_eq!(a.canonical_bytes(), b.canonical_bytes());
        a.set([0, 0, 0], &BlockState::simple("stone"));
        assert_ne!(a.canonical_bytes(), b.canonical_bytes());
        b.set([0, 0, 0], &BlockState::simple("stone"));
        assert_eq!(a.canonical_bytes(), b.canonical_bytes());
    }

    #[test]
    fn distinct_block_states_are_distinct_cells() {
        let mut m = VoxelModel::new(Box3::at_origin([2, 1, 1]));
        let east = BlockState::with("oak_stairs", [("facing", "east")]);
        let west = BlockState::with("oak_stairs", [("facing", "west")]);
        m.set([0, 0, 0], &east);
        m.set([1, 0, 0], &west);
        assert_eq!(m.palette().len(), 3, "air + two states of the same block");
        assert_ne!(m.get([0, 0, 0]), m.get([1, 0, 0]));
    }
}
