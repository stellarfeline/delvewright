//! The prefab metadata document (`<id>.json`, beside the structure `.nbt`) — the
//! **one** definition of its shape.
//!
//! A prefab is a *pair* of files: the gzip-framed structure template that
//! [`crate::convert::build_region`] writes, and this sibling JSON that says what
//! the template is, where its anchors and sockets are, how lit it is, and what
//! regenerates it. Both halves are produced by more than one tool — the grammar
//! back end and the five hand-written generators write the pair from scratch,
//! `delve-admit` reads it and writes it back after every admission step — so the
//! document's shape lives here, in the crate both sides already depend on for the
//! `.nbt` half, and neither owns a copy of it.
//!
//! **That is the invariant, not a tidiness preference.** A private copy of a
//! document shape is a lossy filter the moment the two copies disagree: a writer
//! that models fewer fields than the document has silently deletes the rest on
//! read-modify-write, and it does so while every test it has passes. The field
//! this cost was `license.generated_by` — the four inputs ADR-0006 promises
//! regenerate the `.nbt` byte for byte — dropped by the admission step that runs
//! immediately after export.
//!
//! # Reading is total, writing preserves
//!
//! Every field a producer may legitimately omit is `Option`/`default` and is
//! omitted (never `null`) on write, so a legacy prefab that predates a field
//! still loads and a piece that has never been probed does not have to invent a
//! measurement. Field order is the emission order, and it is the order the
//! library's checked-in prefabs already use, so a reviewer diffing a generated
//! piece against a hand-built one sees only values change.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::split::TileSet;

/// A prefab's sibling metadata file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrefabMeta {
    /// The DSL prefab id, `prefab/<id>`.
    pub prefab_id: String,
    /// The structure-template reference.
    pub structure: StructureMeta,
    /// Named anchors, keyed by DSL anchor name. `{}` for a piece that declares
    /// none.
    #[serde(default)]
    pub anchors: BTreeMap<String, Anchor>,
    /// Jigsaw sockets. `[]` for a piece that is placed directly rather than
    /// drawn from a pool.
    #[serde(default)]
    pub connectors: Vec<Connector>,
    /// The lighting declaration.
    pub lighting: Lighting,
    /// Licence, provenance prose, and the machine-readable provenance row.
    pub license: License,
}

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
/// It is `Deserialize` as well as `Serialize` for the reason the module header
/// gives. Written-only, it is a document nothing can edit: every admission step
/// is a read-modify-write, so a tiled zone had no reachable `lighting` block,
/// no reachable `anchors` map, and no way to be corrected — and the tools that
/// were handed one answered about a single tile instead, which is how a
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
    /// The lighting declaration.
    pub lighting: Lighting,
    /// Licence and provenance.
    pub license: License,
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

    /// The `lighting` block, for the one step that owns it.
    pub fn lighting_mut(&mut self) -> &mut Lighting {
        match self {
            PrefabDoc::Single(m) => &mut m.lighting,
            PrefabDoc::Zone(m) => &mut m.lighting,
        }
    }

    /// The licence block — what this document can say about where it came from.
    pub fn license(&self) -> &License {
        match self {
            PrefabDoc::Single(m) => &m.license,
            PrefabDoc::Zone(m) => &m.license,
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

/// The `structure` block: which file, how big, for which MC version.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructureMeta {
    /// The `.nbt` filename, relative to this metadata file.
    pub file: String,
    /// The datapack structure id (a path segment).
    pub id: String,
    /// Structure extent `[x, y, z]`.
    pub size: [i32; 3],
    /// The MC data version the structure targets (ADR-0009).
    pub data_version: i32,
    /// Provenance breadcrumb: what wrote the `.nbt`.
    pub generator: String,
}

/// One entry of the `anchors` map.
///
/// A point anchor carries `pos` (+ optionally `facing`); a gate anchor carries a
/// `region` (+ optionally `block`). Both shapes are the same object class, so
/// both live in one type and each writes only the keys it means.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Anchor {
    /// Local cell `[x, y, z]`, relative to the structure origin.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pos: Option<[i32; 3]>,
    /// Cardinal facing keyword.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facing: Option<String>,
    /// Local cell range, for a gate anchor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<Region>,
    /// Block id, for a gate anchor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block: Option<String>,
}

impl Anchor {
    /// The point-anchor shape: a cell and a facing.
    pub fn point(pos: [i32; 3], facing: impl Into<String>) -> Anchor {
        Anchor {
            pos: Some(pos),
            facing: Some(facing.into()),
            region: None,
            block: None,
        }
    }
}

/// An inclusive local cell range.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Region {
    /// Low corner `[x, y, z]`.
    pub from: [i32; 3],
    /// High corner `[x, y, z]`.
    pub to: [i32; 3],
}

/// One jigsaw socket declared by a prefab.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Connector {
    /// Jigsaw `name`.
    pub name: String,
    /// Jigsaw `target`.
    pub target: String,
    /// The socket's wall cell, local coords `[x, y, z]`.
    pub local_pos: [i32; 3],
    /// Cardinal direction the opening faces outward.
    pub facing: String,
    /// Opening extent `[width, height]`.
    pub opening: [i32; 2],
    /// Jigsaw joint.
    pub joint: String,
}

/// The `lighting` block.
///
/// Only `profile` is mandatory. A prefab that has never been probed declares
/// `{"profile": "unmeasured"}` and carries no measurement, because a claim and
/// its absence cannot both be true — which is what the engine's own `Lighting`
/// requires and what the grammar back end emits. A `lit`/`dim`/`dark` profile
/// does carry both `measured_min_light` and `measured`; the engine refuses one
/// that does not.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Lighting {
    /// `unmeasured` | `lit` | `dim` | `dark`.
    pub profile: String,
    /// Minimum block light over the walkable floor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measured_min_light: Option<i32>,
    /// When the measurement was taken.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measured: Option<String>,
    /// Why the profile is what it is, when that needs saying.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
    /// How the measurement was taken.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
}

impl Lighting {
    /// The declaration of a piece nothing has probed.
    pub fn unmeasured() -> Lighting {
        Lighting {
            profile: UNMEASURED.to_string(),
            measured_min_light: None,
            measured: None,
            rationale: None,
            method: None,
        }
    }
}

/// The profile of a prefab whose light nothing has measured.
pub const UNMEASURED: &str = "unmeasured";

/// The `license` block: the human half and the machine half of provenance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct License {
    /// Where the asset came from (`original`, or a named upstream).
    pub source: String,
    /// SPDX id (ADR-0013).
    pub spdx: String,
    /// Human note.
    pub note: String,
    /// Human-readable provenance sentence.
    pub provenance: String,
    /// The machine-readable provenance row: what regenerates these exact bytes.
    ///
    /// Absent for a piece nothing can regenerate — an ingested community build,
    /// or a hand-edited one. Present, it is the ADR-0006 claim in a form a tool
    /// can act on rather than a sentence a human can read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_by: Option<GeneratedBy>,
}

/// Everything needed to reproduce the `.nbt` byte for byte (ADR-0006).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeneratedBy {
    /// The back end that produced the bytes.
    pub generator: String,
    /// The source program's name.
    pub program: String,
    /// `sha256:<64 hex>` over the program's canonical JSON.
    pub program_hash: String,
    /// The expansion seed.
    pub seed: u64,
}

impl PrefabMeta {
    /// Parse metadata from JSON text.
    pub fn from_json(text: &str) -> Result<PrefabMeta, String> {
        serde_json::from_str(text).map_err(|e| format!("invalid prefab metadata: {e}"))
    }

    /// Load `<nbt_path>.json` (the sibling metadata), or `Ok(None)` when absent.
    pub fn beside_nbt(nbt_path: &Path) -> Result<Option<PrefabMeta>, String> {
        let json_path = nbt_path.with_extension("json");
        if !json_path.exists() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(&json_path)
            .map_err(|e| format!("read {}: {e}", json_path.display()))?;
        Ok(Some(PrefabMeta::from_json(&text)?))
    }

    /// Serialize as canonical pretty JSON with a trailing newline.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("prefab metadata serializes") + "\n"
    }

    /// A minimal skeleton for a freshly admitted external piece.
    pub fn skeleton(
        id: &str,
        size: [i32; 3],
        data_version: i32,
        generator: &str,
        license: License,
    ) -> PrefabMeta {
        PrefabMeta {
            prefab_id: format!("prefab/{id}"),
            structure: StructureMeta {
                file: format!("{id}.nbt"),
                id: id.to_string(),
                size,
                data_version,
                generator: generator.to_string(),
            },
            anchors: BTreeMap::new(),
            connectors: Vec::new(),
            lighting: Lighting {
                method: Some("not yet probed".to_string()),
                ..Lighting::unmeasured()
            },
            license,
        }
    }

    /// Add or replace a named anchor.
    pub fn set_anchor(&mut self, name: &str, anchor: Anchor) {
        self.anchors.insert(name.to_string(), anchor);
    }

    /// Append a socket connector (idempotent by `local_pos` + `facing`).
    pub fn add_connector(&mut self, c: Connector) {
        if !self
            .connectors
            .iter()
            .any(|x| x.local_pos == c.local_pos && x.facing == c.facing)
        {
            self.connectors.push(c);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole reason this type is not two types: an editing tool reads a
    /// document, changes one block of it, and writes it back. Anything it does
    /// not model is deleted, and nothing says so.
    #[test]
    fn a_read_modify_write_round_trip_keeps_every_field() {
        let text = r#"{
  "prefab_id": "prefab/chapel-ward",
  "structure": {
    "file": "chapel-ward.nbt",
    "id": "chapel-ward",
    "size": [16, 9, 26],
    "data_version": 4671,
    "generator": "crates/grammar"
  },
  "anchors": {
    "anchor/bell": { "pos": [3, 1, 4], "facing": "north" },
    "anchor/ward": { "region": { "from": [0, 0, 0], "to": [2, 2, 2] }, "block": "minecraft:stone" }
  },
  "connectors": [],
  "lighting": { "profile": "unmeasured" },
  "license": {
    "source": "original",
    "spdx": "GPL-3.0-or-later",
    "note": "n",
    "provenance": "p",
    "generated_by": {
      "generator": "grammar",
      "program": "bell_chapel_ward",
      "program_hash": "sha256:00",
      "seed": 1
    }
  }
}
"#;
        let mut meta = PrefabMeta::from_json(text).unwrap();
        meta.lighting = Lighting {
            profile: "dark".to_string(),
            measured_min_light: Some(0),
            measured: Some("2026-08-11".to_string()),
            rationale: None,
            method: Some("static estimate".to_string()),
        };
        let after: serde_json::Value = serde_json::from_str(&meta.to_json()).unwrap();
        let before: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(
            after["license"]["generated_by"], before["license"]["generated_by"],
            "the provenance row must survive an edit to an unrelated block"
        );
        assert_eq!(after["anchors"], before["anchors"]);
        assert_eq!(after["structure"], before["structure"]);
    }

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
  }
}
"#;
        let mut doc = PrefabDoc::from_json(text).unwrap();
        assert!(matches!(doc, PrefabDoc::Zone(_)));
        assert_eq!(doc.license().spdx, "GPL-3.0-or-later");
        *doc.lighting_mut() = Lighting {
            profile: "lit".to_string(),
            measured_min_light: Some(6),
            measured: Some(String::new()),
            rationale: None,
            method: Some("static estimate".to_string()),
        };
        let after: serde_json::Value = serde_json::from_str(&doc.to_json()).unwrap();
        let before: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(after["license"], before["license"]);
        assert_eq!(after["structure_set"], before["structure_set"]);
        assert_eq!(after["anchors"], before["anchors"]);
        assert_eq!(after["lighting"]["profile"], "lit");

        // The two shapes are told apart by which structure key is present, and
        // a document with neither is refused rather than half-read.
        assert!(matches!(
            PrefabDoc::from_json(
                &PrefabMeta::skeleton(
                    "x",
                    [1, 1, 1],
                    4671,
                    "t",
                    License {
                        source: "original".into(),
                        spdx: "CC0-1.0".into(),
                        note: String::new(),
                        provenance: String::new(),
                        generated_by: None,
                    },
                )
                .to_json()
            )
            .unwrap(),
            PrefabDoc::Single(_)
        ));
        let err = PrefabDoc::from_json(r#"{"prefab_id":"prefab/x"}"#).unwrap_err();
        assert!(err.contains("structure_set"), "{err}");
    }

    /// A piece nothing has regenerated has no row, and the key is absent rather
    /// than `null` — `null` reads as "measured, and the answer is nothing".
    #[test]
    fn absent_optional_fields_are_omitted_not_nulled() {
        let meta = PrefabMeta::skeleton(
            "ingested",
            [3, 3, 3],
            4671,
            "delve-admit (external admission)",
            License {
                source: "unknown".to_string(),
                spdx: "UNKNOWN".to_string(),
                note: String::new(),
                provenance: String::new(),
                generated_by: None,
            },
        );
        let json = meta.to_json();
        assert!(!json.contains("generated_by"), "{json}");
        assert!(!json.contains("null"), "{json}");
        assert!(json.contains("\"connectors\": []"), "{json}");
        assert_eq!(PrefabMeta::from_json(&json).unwrap(), meta);
    }
}
