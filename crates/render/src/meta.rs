//! Prefab metadata (`<basename>.json` beside the `.nbt`) — the socket/anchor
//! subset `delve-render piece` needs to aim interior shots. Degrades gracefully:
//! a missing/partial file just yields fewer shots (exterior + top-down always
//! render from the `.nbt` alone).
//!
//! # A narrow view, not a second definition
//!
//! [`PrefabMeta`] here reads three of the document's blocks and is a genuinely
//! narrower type than the document — because it must also read a **tile-set
//! manifest**, which carries the same `anchors`, `connectors` and `lighting`
//! under the same keys but names its blocks `structure_set` rather than
//! `structure`. The full document requires `structure`, so it cannot parse a
//! manifest, and one reader has to serve both shapes.
//!
//! What it does not do is re-declare the fields. Every leaf type below is the
//! document's own ([`delvewright_schem::prefab`]), so an anchor here is an
//! anchor there — same keys, same optionality, same tolerance of a key this
//! version has never heard of. The only local decision is *which* blocks are
//! read.

use std::path::Path;

use serde::Deserialize;

/// The document's own leaf types. A projection selects blocks; it does not get
/// to have its own opinion about what an anchor is.
pub use delvewright_schem::prefab::{Anchor as AnchorMeta, Connector, Lighting, Region};

/// The subset of prefab metadata the renderer reads, from either shape of the
/// document (single template or tile-set manifest).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PrefabMeta {
    /// Named anchors (`spawn`, `anchor/…`), keyed by name.
    #[serde(default)]
    pub anchors: std::collections::BTreeMap<String, AnchorMeta>,
    /// Jigsaw sockets (doorways). Empty for single-piece prefabs.
    #[serde(default)]
    pub connectors: Vec<Connector>,
    /// Declared lighting profile, if measured.
    #[serde(default)]
    pub lighting: Option<Lighting>,
}

impl PrefabMeta {
    /// Load `<nbt_path with .json>` if present; `Ok(None)` when absent, `Err` only
    /// on a malformed file.
    pub fn beside_nbt(nbt_path: &Path) -> Result<Option<PrefabMeta>, String> {
        Self::at_path(&nbt_path.with_extension("json"))
    }

    /// Load a metadata file by path; `Ok(None)` when absent, `Err` only on a
    /// malformed file.
    ///
    /// A tiled zone's metadata is not beside any one `.nbt` — it is the manifest
    /// the whole set was reassembled from — so the caller says which file to
    /// read. The anchors and the lighting profile live under the same keys in
    /// both shapes, which is why one reader serves them.
    pub fn at_path(json_path: &Path) -> Result<Option<PrefabMeta>, String> {
        if !json_path.exists() {
            return Ok(None);
        }
        let bytes =
            std::fs::read(json_path).map_err(|e| format!("read {}: {e}", json_path.display()))?;
        let meta: PrefabMeta = serde_json::from_slice(&bytes)
            .map_err(|e| format!("parse {}: {e}", json_path.display()))?;
        Ok(Some(meta))
    }

    /// `true` when the declared lighting profile is `lit`.
    pub fn is_lit(&self) -> Option<bool> {
        self.lighting
            .as_ref()
            .map(|l| l.profile == delvewright_schem::prefab::LightingProfile::Lit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_connectors_and_anchors() {
        let json = br#"{
            "anchors": {
                "anchor/gate": { "region": { "from": [2,1,4], "to": [4,3,4] }, "block": "minecraft:iron_bars" },
                "anchor/keeper-stand": { "pos": [3,1,2], "facing": "north" }
            },
            "connectors": [
                { "name": "keep:socket", "target": "keep:socket", "local_pos": [3,1,0],
                  "facing": "north", "opening": [3,3], "joint": "aligned" }
            ],
            "lighting": { "profile": "lit", "measured_min_light": 9, "measured": "2026-08-01" }
        }"#;
        let meta: PrefabMeta = serde_json::from_slice(json).unwrap();
        assert_eq!(meta.connectors.len(), 1);
        assert_eq!(meta.connectors[0].facing, "north");
        assert!(meta.anchors.contains_key("anchor/keeper-stand"));
        assert_eq!(meta.is_lit(), Some(true));
    }
}
