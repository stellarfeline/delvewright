//! The prefab metadata document (`<id>.json`, beside the structure `.nbt`) —
//! the **one** definition of its shape.
//!
//! A prefab is a *pair* of files: a gzip-framed structure template and this
//! sibling JSON that says what the template is, where its anchors and sockets
//! are, how lit it is, what it claims about the space inside it, and what
//! regenerates it. Both halves are produced and consumed by several tools of
//! several ages — the grammar back end and the hand-written generators write the
//! pair from scratch, `delve-admit` reads it and writes it back after every
//! admission step, `delvec` reads it to plan a world, `delve-render` reads it to
//! aim a camera — so the document's shape is defined once, here, and every one
//! of them reads that definition instead of a copy of it.
//!
//! # Why the definition lives in the DSL crate
//!
//! Not because prefab metadata is DSL surface — it is a library-asset document —
//! but because this is the only crate every reader can depend on. `delvec` is
//! published to crates.io and may only depend on published crates, and this crate
//! is the one it already depends on. The alternative was a copy inside `delvec`,
//! which is what existed and what this module replaces. The crate already owns
//! the document's `lighting` block ([`crate::registry::Lighting`], whose field
//! names are this file's field names) and the anchor surface DSL validation
//! resolves refs against ([`crate::registry::AnchorRegistry`]), so the document's
//! remaining blocks join a shape that was already half here.
//!
//! # Reading is total, writing preserves
//!
//! Every field a producer may legitimately omit is `Option`/`default` and is
//! omitted (never `null`) on write, so a legacy prefab that predates a field
//! still loads and a piece that has never been probed does not have to invent a
//! measurement. Field order is the emission order, and it is the order the
//! library's checked-in prefabs already use, so a reviewer diffing a generated
//! piece against a hand-built one sees only values change.
//!
//! Keys this version has never heard of are **kept**, in [`PrefabMeta::extra`]
//! and [`Anchor::extra`], and written back out. That is not politeness to the
//! future; it is the only behaviour that is neither an outage nor silent data
//! loss. See the `deny_unknown_fields` note below.
//!
//! # `deny_unknown_fields`, decided rather than inherited
//!
//! The attribute is right on a document whose reader is also its **owner**: a
//! campaign stage document is authored against a versioned schema, a typo there
//! is the bug the attribute exists to catch, and forward compatibility is
//! handled by the `dsl_version` fence instead. Every stage struct in
//! [`crate::stages`] keeps it for exactly that reason.
//!
//! It is wrong on a **consumer that is not the owner**, which is what every
//! reader of this document is. Here a new key is not a typo — it is a newer
//! producer meeting an older reader, which happens on every mixed-version pair
//! of engine and content library. Refusing turns a forward addition into a hard
//! failure at the layer with the least context; the compiler's private copy of
//! this shape did exactly that, and the first grammar-exported prefab carrying a
//! new key would have failed every campaign build.
//!
//! Tolerating alone is not the fix either, because a tool that reads this
//! document, edits one block and writes it back deletes everything it does not
//! model — and does so while every test it has passes. That is not
//! hypothetical: `license.generated_by` was dropped that way once, and
//! `waterline_y` — a field five shipped island prefabs carry and the
//! ocean-horizon placement check keys off — was being dropped that way at the
//! time this module was written.
//!
//! So the rule is: **this document's structs neither refuse an unknown key nor
//! discard it.** They keep it, and the reader that wants to say something about
//! it says it as a diagnostic (`DW0543`) rather than as a parse failure. The
//! blocks whose own definition lives elsewhere are the exception and say why at
//! their field.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::registry::Lighting;

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
    ///
    /// Absent means legacy metadata that predates the field, which is a
    /// different claim from `{"profile": "unmeasured"}` — the positive statement
    /// that a measurement is owed.
    ///
    /// The block's own shape is [`Lighting`], and it is **the one part of this
    /// document that still refuses a key it does not know**. Its job is a rule
    /// about values — a measured profile must carry its measurement, an
    /// `unmeasured` one must not — so a misspelled measurement key there is a
    /// claim quietly becoming its own absence, which the profile/measurement
    /// agreement alone does not catch for `rationale` or `method`. The cost is
    /// real and is stated where an author will meet it
    /// (`docs/reference/prefab-procedure.md` §9): a key added inside `lighting`
    /// is a hard parse failure for an older engine, so adding one is a
    /// `dsl_version` matter rather than a metadata edit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lighting: Option<Lighting>,
    /// Licence, provenance prose, and the machine-readable provenance row.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<License>,
    /// The local y of this piece's **top authored water block** — its waterline
    /// — for open-air pieces built to a tileset convention that authors a sea.
    /// Consumed by the ocean-horizon placement invariant (`DW0344`): in a
    /// `horizon: ocean` world the declared waterline must land at world sea
    /// level. Absent for pieces that author no sea, which are then not checked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waterline_y: Option<i32>,
    /// The piece's spatial contract, when it declares one.
    ///
    /// Absent means legacy metadata — the piece makes no spatial claim — exactly
    /// as an absent `lighting` block differs from `unmeasured`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spatial_contract: Option<SpatialContract>,
    /// Every top-level key this version does not model, kept verbatim so that
    /// reading and writing the document is not the same as editing it.
    ///
    /// A reader that wants to report one has it in hand; a reader that does not
    /// care carries it through. Emitted after the modelled keys, in key order.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generator: Option<String>,
}

/// One entry of the `anchors` map.
///
/// A point anchor carries `pos` (+ optionally `facing`); a gate anchor carries a
/// `region` (+ optionally `block`); a trap anchor also carries the hardware the
/// prefab pre-wired for it. All of those are the same object class — a named
/// place in a piece — so they live in one type and each writes only the keys it
/// means.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Anchor {
    /// Local cell `[x, y, z]`, relative to the structure origin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pos: Option<[i32; 3]>,
    /// Cardinal facing keyword.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facing: Option<String>,
    /// Local cell range, for a gate anchor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<Region>,
    /// Block id filling a gate region (e.g. `minecraft:iron_bars`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    /// The pre-wired dispenser socket cell (local coords) for an `anchor/trap`
    /// marker. `pos` is the trap's trigger/hazard cell (the plate, tripwire or
    /// chest modelled as the hazard); `dispenser` is the separate cell holding
    /// the empty dispenser whose payload is filled at compile time. Absent for
    /// every non-trap anchor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dispenser: Option<[i32; 3]>,
    /// The block the prefab wired as this `anchor/trap`'s **trigger** — the
    /// plate or tripwire sitting on `pos` — with its full blockstate exactly as
    /// authored (`minecraft:oak_pressure_plate[powered=false]`), because
    /// flag-gating a trap physically removes and restores this block and must
    /// put back what was there. The gate-anchor `block` above is the same
    /// contract for a sealed gate. Absent for every non-trap anchor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_block: Option<String>,
    /// Every anchor key this version does not model, kept verbatim. The anchor
    /// block is where this document has grown most often — `resolves_to`,
    /// `dispenser` and `trigger_block` were each a new key on a shipped
    /// document — so it captures for the same reason [`PrefabMeta::extra`]
    /// does.
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

impl Anchor {
    /// The point-anchor shape: a cell and a facing.
    pub fn point(pos: [i32; 3], facing: impl Into<String>) -> Anchor {
        Anchor {
            pos: Some(pos),
            facing: Some(facing.into()),
            ..Anchor::default()
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
///
/// `local_pos` is the socket's wall cell (bottom-centre of the opening) in the
/// prefab's local coordinates; `facing` is the cardinal direction the opening
/// faces outward. Two sockets mate by placing the child so its socket sits one
/// block beyond the parent's, facing the opposite way.
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

    /// Every key of this document — top level and per anchor — that this version
    /// does not model, as `(where, key)` pairs in a stable order.
    ///
    /// `where` is `""` for a top-level key and the anchor's name for an anchor
    /// key. A reader that wants to say something about a key it kept asks here;
    /// nothing has to re-open the file to find out.
    pub fn unknown_keys(&self) -> Vec<(&str, &str)> {
        let mut out: Vec<(&str, &str)> = self
            .extra
            .keys()
            .map(|k| ("", k.as_str()))
            .collect::<Vec<_>>();
        for (name, anchor) in &self.anchors {
            for key in anchor.extra.keys() {
                out.push((name.as_str(), key.as_str()));
            }
        }
        out
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
                generator: Some(generator.to_string()),
            },
            anchors: BTreeMap::new(),
            connectors: Vec::new(),
            lighting: Some(Lighting {
                method: Some("not yet probed".to_string()),
                ..Lighting::unmeasured()
            }),
            license: Some(license),
            waterline_y: None,
            spatial_contract: None,
            extra: BTreeMap::new(),
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
  },
  "waterline_y": 2
}
"#;
        let mut meta = PrefabMeta::from_json(text).unwrap();
        meta.lighting = Some(Lighting {
            profile: crate::registry::LightingProfile::Dark,
            measured_min_light: Some(0),
            measured: Some("2026-08-11".to_string()),
            rationale: None,
            method: Some("static estimate".to_string()),
        });
        let after: serde_json::Value = serde_json::from_str(&meta.to_json()).unwrap();
        let before: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(
            after["license"]["generated_by"], before["license"]["generated_by"],
            "the provenance row must survive an edit to an unrelated block"
        );
        assert_eq!(after["anchors"], before["anchors"]);
        assert_eq!(after["structure"], before["structure"]);
        assert_eq!(
            after["waterline_y"], before["waterline_y"],
            "a declared waterline must survive an edit to an unrelated block"
        );
    }

    /// The same guarantee for a key no version of this type has ever heard of.
    /// This is the general form of the `waterline_y` and `generated_by` losses:
    /// the type cannot enumerate what has not been invented, so it keeps it.
    #[test]
    fn a_key_this_version_does_not_model_survives_the_round_trip() {
        let text = r#"{
  "prefab_id": "prefab/x",
  "structure": { "file": "x.nbt", "id": "x", "size": [3, 3, 3], "data_version": 4671 },
  "anchors": { "anchor/a": { "pos": [1, 1, 1], "acoustics": "reverberant" } },
  "connectors": [],
  "lighting": { "profile": "unmeasured" },
  "from_the_future": { "nested": [1, 2, 3] }
}
"#;
        let mut meta = PrefabMeta::from_json(text).unwrap();
        assert_eq!(
            meta.unknown_keys(),
            vec![("", "from_the_future"), ("anchor/a", "acoustics")],
            "both unknown keys must be reportable, top level and per anchor"
        );
        meta.connectors.push(Connector {
            name: "keep:socket".to_string(),
            target: "keep:socket".to_string(),
            local_pos: [0, 0, 0],
            facing: "north".to_string(),
            opening: [3, 3],
            joint: "aligned".to_string(),
        });
        let after: serde_json::Value = serde_json::from_str(&meta.to_json()).unwrap();
        let before: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(after["from_the_future"], before["from_the_future"]);
        assert_eq!(
            after["anchors"]["anchor/a"]["acoustics"],
            before["anchors"]["anchor/a"]["acoustics"]
        );
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
