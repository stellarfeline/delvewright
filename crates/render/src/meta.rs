//! Prefab metadata (`<basename>.json` beside the `.nbt`) — what the renderer
//! reads about a piece that its blocks do not say.
//!
//! Two producers write this file and both are read here, because the reader is
//! the same question either way: a hand-built prefab's sibling metadata
//! (sockets, anchors, a measured lighting profile), and a sweep candidate's
//! semantics sidecar (`delvewright.snapshot-semantics/1` — anchors, the walkable
//! floor, the boundary openings). Every field is optional and defaulted, so
//! either producer's file loads, and a piece with no file at all still renders
//! from the `.nbt` alone — with fewer shots and a plan key that says which parts
//! of itself nobody supplied.
//!
//! Nothing here derives anything. The renderer draws what it is told, so there
//! is exactly one authority on which cells a body can stand in
//! (`delvewright-grammar`'s `floor`, the same one the generator's gates assert
//! with) and a picture can never disagree with a gate.

use std::path::Path;

use serde::Deserialize;

/// The subset of prefab metadata the renderer reads.
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
    /// The walkable floor, when the producer computed one. `None` means "nobody
    /// supplied it", which the plan key states rather than filling in.
    #[serde(default)]
    pub floor: Option<Floor>,
    /// Standable cells on each boundary face — every place a body could cross
    /// into or out of the piece.
    #[serde(default)]
    pub openings: Option<Openings>,
    /// Anchors the producer identified as the party's way in.
    #[serde(default)]
    pub declared_entries: Vec<String>,
    /// Anchors the producer identified as the party's way onward.
    #[serde(default)]
    pub declared_exits: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnchorMeta {
    /// Point-anchor local position.
    #[serde(default)]
    pub pos: Option<[i32; 3]>,
    /// Facing keyword for point anchors.
    #[serde(default)]
    pub facing: Option<String>,
    /// Region (two inclusive corners) for gate anchors.
    #[serde(default)]
    pub region: Option<Region>,
    /// The rule that declared it, when the producer recorded one.
    #[serde(default)]
    pub declared_by: Option<String>,
}

/// The walkable floor as a plan: one entry per `(x, z)` column, `x`-major.
#[derive(Debug, Clone, Deserialize)]
pub struct Floor {
    /// Region extent `[x, y, z]` the columns are indexed against.
    pub size: [u32; 3],
    /// `null` where nothing in the column can be stood in.
    pub columns: Vec<Option<FloorColumn>>,
    /// Standable cells in total.
    #[serde(default)]
    pub standable_cells: usize,
    /// Columns carrying more than one standable level — the storeys a
    /// single-height plan cannot show, counted so the page can say so.
    #[serde(default)]
    pub multi_level_columns: usize,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct FloorColumn {
    /// Local `y` of the lowest standable cell.
    pub y: i32,
    /// Standable levels in this column.
    #[serde(default)]
    pub levels: u32,
}

/// Standable cells on the boundary, by face name (`x-min`, `z-max`, …).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Openings {
    #[serde(default)]
    pub by_face: std::collections::BTreeMap<String, Vec<[i32; 3]>>,
}

impl Openings {
    /// Cells across every face.
    pub fn total(&self) -> usize {
        self.by_face.values().map(Vec::len).sum()
    }
}

impl Floor {
    /// The column at `(x, z)`, or `None` outside the plan / with no floor.
    pub fn at(&self, x: u32, z: u32) -> Option<FloorColumn> {
        if x >= self.size[0] || z >= self.size[2] {
            return None;
        }
        *self
            .columns
            .get((x as usize) * (self.size[2] as usize) + z as usize)?
    }

    /// `(lowest, highest)` floor level across the plan.
    pub fn level_span(&self) -> Option<(i32, i32)> {
        let mut span: Option<(i32, i32)> = None;
        for c in self.columns.iter().flatten() {
            span = Some(match span {
                None => (c.y, c.y),
                Some((lo, hi)) => (lo.min(c.y), hi.max(c.y)),
            });
        }
        span
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Region {
    pub from: [i32; 3],
    pub to: [i32; 3],
}

#[derive(Debug, Clone, Deserialize)]
pub struct Connector {
    /// Socket local position (the doorway centre at floor level).
    pub local_pos: [i32; 3],
    /// Outward facing keyword (the socket's normal, pointing away from the room).
    pub facing: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Lighting {
    pub profile: String,
}

impl PrefabMeta {
    /// Load `<nbt_path with .json>` if present; `Ok(None)` when absent, `Err` only
    /// on a malformed file.
    pub fn beside_nbt(nbt_path: &Path) -> Result<Option<PrefabMeta>, String> {
        let json_path = nbt_path.with_extension("json");
        if !json_path.exists() {
            return Ok(None);
        }
        let bytes =
            std::fs::read(&json_path).map_err(|e| format!("read {}: {e}", json_path.display()))?;
        let meta: PrefabMeta = serde_json::from_slice(&bytes)
            .map_err(|e| format!("parse {}: {e}", json_path.display()))?;
        Ok(Some(meta))
    }

    /// `true` when the declared lighting profile is `lit`.
    pub fn is_lit(&self) -> Option<bool> {
        self.lighting.as_ref().map(|l| l.profile == "lit")
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
                { "name": "keep:socket", "local_pos": [3,1,0], "facing": "north", "opening": [3,3] }
            ],
            "lighting": { "profile": "lit", "measured_min_light": 9 }
        }"#;
        let meta: PrefabMeta = serde_json::from_slice(json).unwrap();
        assert_eq!(meta.connectors.len(), 1);
        assert_eq!(meta.connectors[0].facing, "north");
        assert!(meta.anchors.contains_key("anchor/keeper-stand"));
        assert_eq!(meta.is_lit(), Some(true));
    }
}
