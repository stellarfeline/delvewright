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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lighting {
    pub profile: String,
    pub measured_min_light: i32,
    pub measured: String,
    pub method: String,
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
                profile: "unknown".to_string(),
                measured_min_light: 0,
                measured: "".to_string(),
                method: "not yet probed".to_string(),
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
            measured_min_light: p.measured_min_light.unwrap_or(0),
            measured: "".to_string(),
            method: format!(
                "static block-light BFS estimate (delve-admit): min over {} walkable floor cells; \
                 doorways treated as sealed edge (sky-light=0). NOT a live-server probe; \
                 dark_threshold={}. Re-probe live for borderline pieces.",
                p.floor_cells, p.dark_threshold
            ),
        };
    }
}
