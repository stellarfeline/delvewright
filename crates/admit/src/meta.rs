//! Prefab metadata (`<basename>.json` beside the `.nbt`) — the **full** shape the
//! generator writes (`prefab_id`, `structure`, `anchors`, `connectors`,
//! `lighting`, `license`), so an admitted external piece is **indistinguishable
//! consumer-side** from a generated one (the compiler, `delve-render`, and the
//! solver all read this exact file).
//!
//! Admission edits it in place: `anchor` adds to `anchors`, `socket` appends to
//! `connectors`, `lighting` sets `lighting`. Field order matches the generator so
//! diffs stay clean; optional fields are omitted (not emitted as `null`).

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::light::LightProbe;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefabMeta {
    pub prefab_id: String,
    pub structure: StructureMeta,
    #[serde(default)]
    pub anchors: BTreeMap<String, Anchor>,
    #[serde(default)]
    pub connectors: Vec<Connector>,
    pub lighting: Lighting,
    pub license: License,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureMeta {
    pub file: String,
    pub id: String,
    pub size: [i32; 3],
    pub data_version: i32,
    pub generator: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Anchor {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pos: Option<[i32; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facing: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<Region>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Region {
    pub from: [i32; 3],
    pub to: [i32; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connector {
    pub name: String,
    pub target: String,
    pub local_pos: [i32; 3],
    pub facing: String,
    pub opening: [i32; 2],
    pub joint: String,
}

/// The `lighting` block.
///
/// **Every field but `profile` is optional, and that is a correctness fix, not a
/// relaxation.** A prefab that has never been probed declares
/// `{"profile": "unmeasured"}` and carries no measurement, because a claim and
/// its absence cannot both be true — that is what `delvewright_dsl`'s own
/// `Lighting` requires, and what the grammar back end (spec-0027 §2) emits. This
/// copy demanded all four fields, so `delve-admit` **refused to read a prefab
/// the compiler accepts**: `socket`, `anchor`, `lighting` and `catalog` all fail
/// at `DW0732` on a grammar-exported piece, which is the entire admission half
/// of the pipeline closed to the entire generated half. Measured 2026-08-11 by
/// running the chain on a grammar export.
///
/// The two shapes should not both exist: the DSL's type is the authority and
/// this one should become a re-export. That is a larger change than the defect
/// warrants — `delve-admit`'s static probe reports a `"unknown"` profile the DSL
/// has no variant for — and is a named follow-up, not a silence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lighting {
    pub profile: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measured_min_light: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measured: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct License {
    pub source: String,
    pub spdx: String,
    pub note: String,
    pub provenance: String,
}

impl PrefabMeta {
    /// Load metadata from JSON text.
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

    /// Serialize as canonical pretty JSON with a trailing newline (generator style).
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("prefab metadata serializes") + "\n"
    }

    /// A minimal skeleton for a freshly admitted external piece.
    pub fn skeleton(id: &str, size: [i32; 3], data_version: i32, license: License) -> PrefabMeta {
        PrefabMeta {
            prefab_id: format!("prefab/{id}"),
            structure: StructureMeta {
                file: format!("{id}.nbt"),
                id: id.to_string(),
                size,
                data_version,
                generator: "delve-admit (external admission)".to_string(),
            },
            anchors: BTreeMap::new(),
            connectors: Vec::new(),
            lighting: Lighting {
                profile: "unmeasured".to_string(),
                measured_min_light: None,
                measured: None,
                rationale: None,
                method: Some("not yet probed".to_string()),
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

    /// Write a probe result into the `lighting` block, marked as a static estimate.
    pub fn set_lighting_from_probe(&mut self, p: &LightProbe) {
        self.lighting = Lighting {
            profile: p.profile.to_string(),
            measured_min_light: p.measured_min_light,
            // Present, and deliberately unchanged from before this struct's
            // fields became optional: `delvewright_dsl::registry::Lighting`
            // refuses a `lit`/`dim`/`dark` profile that does not carry BOTH
            // `measured_min_light` and `measured`, so dropping this would make
            // `delve-admit lighting --write` produce metadata the compiler then
            // rejects. (That the date is empty is a pre-existing oddity of the
            // static estimate — `method` says it is not a live probe — and is
            // not this change's to invent a value for.)
            measured: Some(String::new()),
            rationale: None,
            method: Some(format!(
                "static block-light BFS estimate (delve-admit): min over {} walkable floor cells; \
                 doorways treated as sealed edge (sky-light=0). NOT a live-server probe; \
                 dark_threshold={}. Re-probe live for borderline pieces.",
                p.floor_cells, p.dark_threshold
            )),
        };
    }
}
