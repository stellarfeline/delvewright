//! The prefab metadata document, as this crate's tools see it.
//!
//! The shape is **not** defined here. It is [`delvewright_dsl::prefab`], and
//! this module is a re-export so that the crate that writes the `.nbt` half of a
//! prefab names the `.json` half by the same path it always did.
//!
//! The definition sits in the DSL crate for one reason: `delvec` is published to
//! crates.io and may only depend on published crates, so that is the only crate
//! every reader of this document can reach. Anywhere else, the compiler would
//! need a copy — which is what it had.

pub use delvewright_dsl::prefab::{
    Anchor, AnchorEdit, Connector, ContractBar, ContractEdge, ContractFace, ContractNoBody,
    ContractSpace, ContractVolume, GeneratedBy, License, PrefabMeta, Region, SpatialContract,
    StructureMeta, UNMEASURED,
};

/// The `lighting` block, which the DSL owns outright: it is the same type the
/// compiler validates a campaign's lighting claims with, so a probe result that
/// this crate's tools write is refused here rather than three tools later.
pub use delvewright_dsl::registry::{Lighting, LightingProfile};

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::split::TileSet;

/// A zone whose blocks did not fit in one structure template.
///
/// Field for field this is [`PrefabMeta`] with `structure` replaced by
/// `structure_set`: same `prefab_id`, same zone-relative `anchors`, same
/// `connectors`, same `lighting`, same `license` — including the provenance row,
/// which names the program hash and seed that regenerate every tile at once.
/// What changed is how many files the blocks arrived in, and that is the only
/// thing that changed.
///
/// The key is deliberately a *different name*, not `structure` with an extra
/// field: a tool that has not learned about tile sets fails to parse this
/// document instead of reading it as a prefab with no blocks in it.
///
/// It lives here rather than beside [`PrefabMeta`] in the DSL crate for one
/// mechanical reason: `structure_set` is a [`TileSet`], which is this crate's
/// type, and `delvec` may only depend on published crates. Every other property
/// of the document — the totality rules below, the field order, the omit-never-
/// null discipline — is the same, and is the same because it is copied from a
/// definition that is one `use` away rather than from memory.
///
/// It is `Deserialize` as well as `Serialize`, which is the fix this type
/// carries. Written-only, it was a document nothing could edit: every admission
/// step is a read-modify-write, so a tiled zone had no reachable `lighting`
/// block, no reachable `anchors` map, and no way to be corrected — and the tools
/// handed one answered about a single tile instead, which is how a
/// `spdx: UNKNOWN` skeleton came to be written beside a correctly provenanced
/// zone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TileSetMeta {
    /// The DSL prefab id, `prefab/<id>`.
    pub prefab_id: String,
    /// The tiles and how they reassemble.
    pub structure_set: TileSet,
    /// Named anchors in **zone** coordinates. A cut never moves one, and no
    /// anchor is attributed to a tile: a mark is a fact about the building.
    #[serde(default)]
    pub anchors: BTreeMap<String, Anchor>,
    /// Jigsaw sockets — an empty list, for the same reason a single-template
    /// export writes one: "this zone declares no sockets" and "this manifest
    /// predates sockets" are different claims, and only the first is true.
    #[serde(default)]
    pub connectors: Vec<Connector>,
    /// The lighting declaration, `Option` for the reason [`PrefabMeta`]'s is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lighting: Option<Lighting>,
    /// Licence and provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<License>,
    /// The zone's spatial contract, in **zone** coordinates, for the reason
    /// `anchors` are: a cut is not part of the building.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spatial_contract: Option<SpatialContract>,
    /// Every top-level key this version does not model, kept verbatim — the
    /// same rule [`PrefabMeta::extra`] states, for the same reason: a tool that
    /// reads a document, edits one block and writes it back must not delete
    /// what it does not model.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl TileSetMeta {
    /// Parse from JSON text.
    pub fn from_json(text: &str) -> Result<TileSetMeta, String> {
        serde_json::from_str(text).map_err(|e| format!("invalid tile-set metadata: {e}"))
    }

    /// Serialize as canonical pretty JSON with a trailing newline.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("tile-set metadata serializes") + "\n"
    }
}

/// A prefab metadata document, in whichever of its two shapes it arrived.
///
/// Every tool that edits prefab metadata reads through this: a zone that ships
/// as a tile set is one prefab with one `lighting` block and one provenance
/// row, and the number of files its blocks came in is packaging. A tool that
/// handles only [`PrefabMeta`] is a tool that silently does the wrong thing to
/// half the library.
#[derive(Debug, Clone, PartialEq)]
pub enum PrefabDoc {
    /// One structure template.
    Single(PrefabMeta),
    /// A zone packaged as several templates plus this manifest.
    Zone(TileSetMeta),
}

impl PrefabDoc {
    /// Parse either shape, told apart by which structure key the document has.
    ///
    /// A document with neither is an error naming both: "which shape is this"
    /// has exactly two answers and no third, so a shrug here would put a
    /// half-read document into an editing step.
    pub fn from_json(text: &str) -> Result<PrefabDoc, String> {
        let value: serde_json::Value =
            serde_json::from_str(text).map_err(|e| format!("invalid prefab metadata: {e}"))?;
        let object = value
            .as_object()
            .ok_or_else(|| "prefab metadata is not a JSON object".to_string())?;
        match (
            object.contains_key("structure_set"),
            object.contains_key("structure"),
        ) {
            (true, _) => TileSetMeta::from_json(text).map(PrefabDoc::Zone),
            (false, true) => PrefabMeta::from_json(text).map(PrefabDoc::Single),
            (false, false) => Err(
                "prefab metadata has neither a `structure` block nor a `structure_set` block — \
                 it does not say what blocks it describes"
                    .to_string(),
            ),
        }
    }

    /// Read the document at `path`, or `Ok(None)` when there is no file there.
    pub fn read(path: &Path) -> Result<Option<PrefabDoc>, String> {
        if !path.exists() {
            return Ok(None);
        }
        let text =
            std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
        PrefabDoc::from_json(&text).map(Some)
    }

    /// The `lighting` block, when the document carries one.
    pub fn lighting(&self) -> Option<&Lighting> {
        match self {
            PrefabDoc::Single(m) => m.lighting.as_ref(),
            PrefabDoc::Zone(m) => m.lighting.as_ref(),
        }
    }

    /// Declare the piece's lighting — the one block an admission step owns.
    pub fn set_lighting(&mut self, lighting: Lighting) {
        match self {
            PrefabDoc::Single(m) => m.lighting = Some(lighting),
            PrefabDoc::Zone(m) => m.lighting = Some(lighting),
        }
    }

    /// The licence block — what this document can say about where it came from.
    pub fn license(&self) -> Option<&License> {
        match self {
            PrefabDoc::Single(m) => m.license.as_ref(),
            PrefabDoc::Zone(m) => m.license.as_ref(),
        }
    }

    /// Canonical pretty JSON with a trailing newline, in the shape it arrived.
    pub fn to_json(&self) -> String {
        match self {
            PrefabDoc::Single(m) => m.to_json(),
            PrefabDoc::Zone(m) => m.to_json(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiled zone's manifest is a prefab document like any other: it is read,
    /// one block of it is edited, and it is written back whole.
    ///
    /// It was `Serialize`-only, which is the same defect one shape over: nothing
    /// could edit it, so every admission step handed a tiled zone was handed a
    /// single tile instead — and wrote a `spdx: UNKNOWN` skeleton beside a
    /// document whose provenance row was sitting right there.
    #[test]
    fn a_tile_set_manifest_round_trips_through_an_edit() {
        let text = r#"{
  "prefab_id": "prefab/notre-dame",
  "structure_set": {
    "base": "notre-dame",
    "size": [31, 64, 93],
    "part_max": 48,
    "grid": [1, 1, 2],
    "data_version": 4671,
    "generator": "crates/grammar",
    "parts": [
      { "file": "notre-dame.x0y0z0.nbt", "id": "a", "grid_index": [0,0,0], "offset": [0,0,0], "size": [31,64,48] },
      { "file": "notre-dame.x0y0z1.nbt", "id": "b", "grid_index": [0,0,1], "offset": [0,0,48], "size": [31,64,45] }
    ]
  },
  "anchors": { "anchor/crossing": { "pos": [15, 1, 56], "facing": "south" } },
  "connectors": [],
  "lighting": { "profile": "unmeasured" },
  "license": {
    "source": "original",
    "spdx": "GPL-3.0-or-later",
    "note": "n",
    "provenance": "p",
    "generated_by": { "generator": "grammar", "program": "nd", "program_hash": "sha256:00", "seed": 1 }
  },
  "a_key_no_engine_models": { "kept": true }
}
"#;
        let mut doc = PrefabDoc::from_json(text).unwrap();
        assert!(matches!(doc, PrefabDoc::Zone(_)));
        assert_eq!(doc.license().unwrap().spdx, "GPL-3.0-or-later");
        doc.set_lighting(Lighting {
            profile: LightingProfile::Lit,
            measured_min_light: Some(6),
            measured: Some(String::new()),
            rationale: None,
            method: Some("static estimate".to_string()),
        });
        let after: serde_json::Value = serde_json::from_str(&doc.to_json()).unwrap();
        let before: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(after["license"], before["license"]);
        assert_eq!(after["structure_set"], before["structure_set"]);
        assert_eq!(after["anchors"], before["anchors"]);
        assert_eq!(after["lighting"]["profile"], "lit");
        // Reading is total here too: a key this version has never heard of
        // survives an edit rather than being deleted by the tool that made it.
        assert_eq!(
            after["a_key_no_engine_models"], before["a_key_no_engine_models"],
            "an unmodelled key must survive a read-modify-write"
        );
    }

    /// The two shapes are told apart by which structure key is present, and a
    /// document with neither is refused rather than half-read.
    #[test]
    fn a_document_that_names_no_blocks_is_refused_naming_both_keys() {
        let err = PrefabDoc::from_json(r#"{"prefab_id":"prefab/x"}"#).unwrap_err();
        assert!(err.contains("structure_set"), "{err}");
        assert!(err.contains("structure"), "{err}");
    }
}
