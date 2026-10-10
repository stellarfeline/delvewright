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

/// **Where something reaches, as one of two spellings** (spec-0102 §3.4): an
/// anchor-centred box, or a whole place.
///
/// The pair `set-atmosphere` repaints and a pulse is heard in. Declared once
/// and flattened into each carrier, so the JSON keeps the two flat keys every
/// carrier writes in the same words, and a carrier that names neither or both
/// is refused by the one rule that reads it (`DW0929`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PlaceRef {
    /// The volume, as an anchor-centred box (`anchor ± extent`) — resolved
    /// through the one `Plan::zone_box` every box verb takes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<StealthZone>,
    /// A whole place: an `area/…` id, or a site-plan box's `node/…`, whose
    /// bounds the compiler reads from the placement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
}

impl PlaceRef {
    /// `Some(words)` when the reference names neither or both spellings — the
    /// shape `DW0929` refuses wherever a `PlaceRef` is read.
    pub fn ambiguity(&self) -> Option<&'static str> {
        match (&self.region, &self.place) {
            (Some(_), Some(_)) => Some("names both a `region` and a `place`"),
            (None, None) => Some("names neither a `region` nor a `place`"),
            _ => None,
        }
    }
}
