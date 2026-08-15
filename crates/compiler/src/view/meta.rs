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
//! document's own ([`delvewright_dsl::prefab`]), so an anchor here is an
//! anchor there — same keys, same optionality, same tolerance of a key this
//! version has never heard of. The only local decision is *which* blocks are
//! read.

use std::path::Path;

use serde::Deserialize;

/// The document's own leaf types. A projection selects blocks; it does not get
/// to have its own opinion about what an anchor is.
pub use delvewright_dsl::prefab::{Anchor as AnchorMeta, Connector, Region};
pub use delvewright_dsl::registry::Lighting;

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
            .map(|l| l.profile == delvewright_dsl::registry::LightingProfile::Lit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // The document itself, so the projection can be checked against what it
    // projects rather than against a second copy of this file's beliefs.
    use delvewright_dsl::prefab::{Anchor as DocAnchor, License, PrefabMeta as Document};
    use delvewright_dsl::registry::LightingProfile;

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

    /// A document with every block a producer can write, both anchor shapes and
    /// a full socket. Built through the document's own type, so it is whatever
    /// the document currently is rather than whatever this file remembers.
    fn a_full_document() -> Document {
        let mut doc = Document::skeleton(
            "chapel-ward",
            [16, 9, 26],
            4671,
            "crates/grammar",
            License {
                source: "original".to_string(),
                spdx: "GPL-3.0-or-later".to_string(),
                note: "n".to_string(),
                provenance: "p".to_string(),
                generated_by: None,
            },
        );
        doc.lighting = Some(Lighting {
            profile: LightingProfile::Lit,
            measured_min_light: Some(9),
            measured: Some("2026-08-11".to_string()),
            rationale: None,
            method: Some("static estimate".to_string()),
        });
        doc.anchors.insert(
            "anchor/bell".to_string(),
            DocAnchor::point([3, 1, 4], "north"),
        );
        doc.anchors.insert(
            "anchor/ward".to_string(),
            DocAnchor {
                region: Some(Region {
                    from: [0, 0, 0],
                    to: [2, 2, 2],
                }),
                block: Some("minecraft:iron_bars".to_string()),
                resolves_to: Some("via:ward-door".to_string()),
                ..DocAnchor::default()
            },
        );
        doc.add_connector(Connector {
            name: "keep:socket".to_string(),
            target: "keep:socket".to_string(),
            local_pos: [3, 1, 0],
            facing: "north".to_string(),
            opening: [3, 3],
            joint: "aligned".to_string(),
        });
        doc
    }

    /// **The projection agrees with the document it projects.**
    ///
    /// Selecting three blocks out of a larger type is only safe while the two
    /// sides still mean the same thing by them, and nothing about a `serde`
    /// struct says so: rename a key on the document and this reader goes on
    /// parsing, quietly reading its `default` for every block it can no longer
    /// find. Writing the document with its own writer and reading it back with
    /// this reader is what turns that from an assertion into a check.
    #[test]
    fn the_reader_is_a_projection_of_the_document_not_a_copy_of_it() {
        let doc = a_full_document();
        let view: PrefabMeta = serde_json::from_str(&doc.to_json()).unwrap();

        assert_eq!(view.anchors, doc.anchors);
        assert_eq!(view.connectors, doc.connectors);
        assert_eq!(view.lighting, doc.lighting);

        // Bound, not vacuous: two empty documents satisfy every equality above.
        assert_eq!(view.anchors.len(), 2, "the fixture declares two anchors");
        assert_eq!(view.connectors.len(), 1, "the fixture declares one socket");
        assert_eq!(view.is_lit(), Some(true));
        // Both anchor shapes, and fields the renderer's own copy of the document
        // used to lack — they arrive because the leaf type is the document's.
        assert_eq!(view.anchors["anchor/bell"].pos, Some([3, 1, 4]));
        assert_eq!(
            view.anchors["anchor/ward"].block.as_deref(),
            Some("minecraft:iron_bars")
        );
        assert_eq!(
            view.anchors["anchor/ward"].resolves_to.as_deref(),
            Some("via:ward-door"),
            "a field added to the document's Anchor must reach this reader unedited"
        );
    }

    /// Why the projection is a type at all: a tiled zone's manifest carries the
    /// same three blocks under `structure_set` rather than `structure`, and the
    /// full document refuses it — deliberately, so a tool that has never heard
    /// of tile sets fails loudly instead of reviewing a building with no blocks
    /// in it. The renderer has to open both.
    #[test]
    fn the_reader_opens_the_manifest_the_document_type_refuses() {
        let doc = a_full_document();
        let mut json: serde_json::Value = serde_json::from_str(&doc.to_json()).unwrap();
        let obj = json.as_object_mut().unwrap();
        obj.remove("structure");
        obj.insert(
            "structure_set".to_string(),
            serde_json::json!({
                "base": "chapel-ward", "size": [16, 9, 26], "part_max": 48,
                "grid": [1, 1, 1], "data_version": 4671, "generator": "crates/grammar",
                "parts": []
            }),
        );
        let text = serde_json::to_string(&json).unwrap();

        assert!(
            Document::from_json(&text).is_err(),
            "the document type must keep refusing a manifest it cannot represent"
        );
        let view: PrefabMeta = serde_json::from_str(&text).unwrap();
        assert_eq!(view.anchors, doc.anchors);
        assert_eq!(view.connectors, doc.connectors);
        assert_eq!(view.lighting, doc.lighting);
    }
}
