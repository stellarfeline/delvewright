//! Seals: the gates a `close-gate` seals, and the shell of a region.

use super::*;

/// A gate anchor that some `close-gate` seals. One entry per **anchor**: the
/// seal is a place, not an event, so two `close-gate`s on one anchor share its
/// state and its answer (spec-0117: the answer is a trigger that `serves` the
/// gate, `DW0429`).
#[derive(Clone, Debug)]
pub struct SealHintPlan {
    /// The gate anchor name (`anchor/boulder`).
    pub anchor: String,
    /// The function/tag-safe local id, used for `dw_gate_<safe>` and the seal
    /// state `#gseal_<safe>`.
    pub safe: String,
    /// The gate region's inclusive corners (absolute world coords).
    pub region: ([i32; 3], [i32; 3]),
    /// The block the region is filled with while sealed (the generated PackTest
    /// stages and un-stages the seal with it).
    pub block: String,
}

impl SealHintPlan {
    /// The **shell** cells of the seal: every region cell with at least one
    /// axis-neighbour outside the region, in ascending `(x, y, z)` order.
    ///
    /// A cell buried inside the region has six sealed neighbours, so no face of
    /// it can ever be in a player's crosshair — giving it a hitbox would ship an
    /// entity nothing can reach. The shell is exactly the clickable surface, and
    /// for the thin slab a gate anchor usually is (a doorway one block deep) it
    /// is the whole region.
    pub fn shell_cells(&self) -> Vec<[i32; 3]> {
        shell_cells_of(self.region)
    }
}

/// The **shell** cells of an inclusive region: every cell with at least one
/// axis-neighbour outside it, in ascending `(x, y, z)` order.
///
/// Extracted verbatim from [`SealHintPlan::shell_cells`] when the shortcut door's
/// own answer needed the identical surface. One definition, because
/// two copies of "which cells of a sealed slab can be clicked" would be free to
/// drift apart, and the whole point of the geometry is that it is the same
/// question in both places.
pub(super) fn shell_cells_of(region: ([i32; 3], [i32; 3])) -> Vec<[i32; 3]> {
    let (a, b) = region;
    let lo = [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])];
    let hi = [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])];
    let mut out = Vec::new();
    for x in lo[0]..=hi[0] {
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                let interior = (lo[0] < x && x < hi[0])
                    && (lo[1] < y && y < hi[1])
                    && (lo[2] < z && z < hi[2]);
                if !interior {
                    out.push([x, y, z]);
                }
            }
        }
    }
    out
}

/// Collect one [`SealHintPlan`] per gate anchor that any `close-gate` seals (DSL
/// v0.8), in first-firing order.
///
/// A repeat of an anchor already collected is dropped: the seal is a **place**,
/// so its state and its answer belong to the anchor, not to each firing.
///
/// A `close-gate` whose anchor is not a resolvable gate region carries no entry
/// (`DW0343` owns that).
pub(super) fn collect_seal_hints(
    campaign: &Campaign,
    anchors: &BTreeMap<(String, String), ResolvedAnchor>,
) -> Vec<SealHintPlan> {
    let mut out: Vec<SealHintPlan> = Vec::new();
    for_each_gate_effect(campaign, &mut |_site, e| {
        let Some(anchor) = e.close_gate_anchor() else {
            return;
        };
        let name = anchor.as_str();
        if out.iter().any(|s| s.anchor == name) {
            return;
        }
        let Some((from, to, block)) = gate_region_block_any(anchors, name) else {
            return;
        };
        out.push(SealHintPlan {
            anchor: name.to_string(),
            safe: safe_local(name),
            region: (from, to),
            block,
        });
    });
    out
}
