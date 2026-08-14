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
    /// The piece's spatial contract, when it declares one.
    ///
    /// Absent means legacy metadata — the piece makes no spatial claim — exactly
    /// as an absent `lighting` block differs from `unmeasured`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spatial_contract: Option<SpatialContract>,
}

/// A piece's declared spaces, out-of-walk regions and edges, **already
/// resolved**: every box is a local cell range of these exact bytes.
///
/// Resolved rather than parametric on purpose. A grammar program's declarations
/// are scope-bound and mean different boxes at different parameters, so the only
/// contract that can describe *this* `.nbt` is the one its own expansion
/// produced. That is also what lets a hand-built piece carry the same block: it
/// has no parameters to resolve, so the two routes write the same shape and one
/// reader serves both.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpatialContract {
    /// The space a body enters at.
    pub entry: String,
    /// Named spaces.
    #[serde(default)]
    pub spaces: BTreeMap<String, ContractSpace>,
    /// Named standable-but-out-of-walk regions.
    #[serde(default)]
    pub no_body: BTreeMap<String, ContractNoBody>,
    /// The graph, in declaration order.
    #[serde(default)]
    pub edges: Vec<ContractEdge>,
    /// **The piece's face contract**: every `exterior` edge, as the side of the
    /// piece it is on and the opening it leaves there.
    ///
    /// Derived from the edges and the blocks at export time and written out, so
    /// that assembly can ask whether two pieces fit without opening either
    /// `.nbt`. It is the thing an `exterior` edge IS from the outside: an edge
    /// with no cells is a claim nothing can mate with, and one whose opening
    /// does not answer its neighbour's is two pieces that were each approved
    /// alone and do not assemble.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub faces: Vec<ContractFace>,
    /// The author's acknowledgement that this piece is mostly out-of-walk.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub no_body_majority_ack: Option<String>,
}

/// One face of the piece's face contract.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContractFace {
    /// The space the way in or out belongs to.
    pub space: String,
    /// The edge's class: `walk` | `stair` | `drop` | `barred` | `vision`.
    pub class: String,
    /// Which side of the piece: `east` | `west` | `up` | `down` | `south` |
    /// `north`.
    pub dir: String,
    /// The opening, as an inclusive local cell range flat in the face's own
    /// axis.
    pub opening: Region,
}

/// One entry of `spatial_contract.spaces`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContractSpace {
    /// `enclosed` | `open_top` | `open`.
    pub envelope: String,
    /// The cells it covers.
    pub boxes: Vec<Region>,
}

/// One entry of `spatial_contract.no_body`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContractNoBody {
    /// Why these cells are out of play, in the author's words. Which exemption
    /// the region qualifies for is a fact about the blocks and is not recorded
    /// here.
    pub reason: String,
    /// The cells it covers.
    pub boxes: Vec<Region>,
}

/// One entry of `spatial_contract.edges`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContractEdge {
    /// A declared space name, or `exterior`.
    pub a: String,
    /// A declared space name, or `exterior`.
    pub b: String,
    /// `walk` | `stair` | `drop` | `barred` | `vision`.
    pub class: String,
    /// The declared level change, on the classes that carry one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rise: Option<i64>,
    /// The opening or transit volume, when the edge declares one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub via: Option<ContractVolume>,
    /// The bar, on a `barred` edge.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bar: Option<ContractBar>,
}

/// An edge's own volume — an opening, a stair's treads, a fall column.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContractVolume {
    /// The region's name, which is what content binds to.
    pub region: String,
    /// The cells it covers.
    pub boxes: Vec<Region>,
}

/// A `barred` edge's bar: the region that stands in the way, and its block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContractBar {
    /// The region's name.
    pub region: String,
    /// The cells it covers.
    pub boxes: Vec<Region>,
    /// The block state the bar is built from.
    pub block: String,
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
    /// **Which element of the piece's spatial contract this anchor lands in** —
    /// `space:<name>`, `no_body:<name>`, `via:<name>` or `bar:<name>`.
    ///
    /// A campaign binds content to an anchor by name; what says whether that
    /// place is play space, a door or exterior dressing is the contract, and a
    /// reader who has only the anchor list cannot tell. Absent on a piece that
    /// declares no contract, and on an anchor that lands in nothing the contract
    /// accounts for — which is a finding the checker raises rather than a
    /// silence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolves_to: Option<String>,
}

impl Anchor {
    /// The point-anchor shape: a cell and a facing.
    pub fn point(pos: [i32; 3], facing: impl Into<String>) -> Anchor {
        Anchor {
            pos: Some(pos),
            facing: Some(facing.into()),
            region: None,
            block: None,
            resolves_to: None,
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
            spatial_contract: None,
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
