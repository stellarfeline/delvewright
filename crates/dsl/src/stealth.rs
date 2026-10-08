//! Stealth zones: the shadow regions a stealth section is judged by.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::AnchorId;

/// A stealth "shadow" region (DSL v0.6, spec-0014): an axis-aligned box centred
/// on `anchor`, extending `extent` blocks along each axis (so the box spans
/// `anchor ± extent`). Presented in-world via dark cells but judged purely by
/// region membership, so the check is deterministic and provable.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StealthZone {
    /// The prefab anchor at the centre of the zone box.
    pub anchor: AnchorId,
    /// Half-extents `[x, y, z]` in blocks from the anchor (each component ≥ 0);
    /// the zone AABB is `[anchor - extent, anchor + extent]`.
    pub extent: [u32; 3],
}
