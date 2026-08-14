//! **The spatial contract's second door** (spec-0036 §1c): a piece nobody
//! generated, judged by the same checker over the same two inputs.
//!
//! A grammar expansion resolves its scope-bound declarations and hands the
//! checker a block grid and a resolved contract. A hand-built or ingested piece
//! has no declarations to resolve — its boxes are literal from the start — so it
//! carries the identical resolved block in its metadata and hands the checker
//! the identical pair. There is one implementation of the obligations, and this
//! module's whole job is to build its two arguments out of a `.nbt` and a
//! `.json`.
//!
//! That is not tidiness. Two checkers over one contract agree right up until
//! they do not, and the disagreement surfaces as a piece that passed admission
//! and fails at expansion — or worse, the other way round.

use std::collections::BTreeMap;

use delvewright_grammar::block::BlockState;
use delvewright_grammar::contract::{ContractReport, check};
use delvewright_grammar::geom::Box3;
use delvewright_grammar::model::VoxelModel;
use delvewright_schem::prefab::PrefabMeta;

use crate::structure::Structure;

/// Turn a parsed structure template into the block grid the checker reads.
///
/// A cell the template does not name is air, which is what `/place template`
/// does with it.
pub fn grid(s: &Structure) -> VoxelModel {
    let size = [
        s.size[0].max(0) as u32,
        s.size[1].max(0) as u32,
        s.size[2].max(0) as u32,
    ];
    let mut model = VoxelModel::new(Box3::at_origin(size));
    for x in 0..s.size[0] {
        for y in 0..s.size[1] {
            for z in 0..s.size[2] {
                let Some(entry) = s.entry_at([x, y, z]) else {
                    continue;
                };
                let mut block = BlockState::simple(&entry.name);
                block.properties = entry.properties.clone();
                let _ = model.set([x, y, z], &block);
            }
        }
    }
    model
}

/// The anchors a metadata document declares, as the point the checker needs.
///
/// A gate anchor names a region rather than a cell; its low corner is the point
/// used, because a `posted` region's demand is that something is placed *in* it
/// and the low corner is inside it by construction.
fn anchor_points(meta: &PrefabMeta) -> BTreeMap<String, [i32; 3]> {
    meta.anchors
        .iter()
        .filter_map(|(name, a)| {
            let pos = a.pos.or_else(|| a.region.as_ref().map(|r| r.from))?;
            Some((name.clone(), pos))
        })
        .collect()
}

/// Judge a piece's declared contract against its own bytes, or `None` when it
/// declares none.
pub fn audit(s: &Structure, meta: &PrefabMeta) -> Option<ContractReport> {
    let contract = meta.spatial_contract.as_ref()?;
    Some(check(&grid(s), contract, &anchor_points(meta)))
}
