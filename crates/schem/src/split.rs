//! Oversize splitting.
//!
//! Vanilla structure templates cap each axis at 48. Larger schematics tile into
//! a deterministic grid of parts plus a `<base>.split.json` manifest recording
//! grid dimensions, per-part sizes, and source-local offsets so the prefab
//! admission pipeline can reassemble losslessly.

use serde::Serialize;

/// One grid cell of a split.
#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    pub grid_index: [i32; 3],
    /// Source-local origin of this part.
    pub offset: [i32; 3],
    pub size: [i32; 3],
}

/// The plan for tiling a `source_size` volume with a `part_max`-cube cap.
#[derive(Debug, Clone)]
pub struct SplitPlan {
    pub grid: [i32; 3],
    pub parts: Vec<Part>,
}

impl SplitPlan {
    /// True when the volume fits in a single part.
    pub fn is_single(&self) -> bool {
        self.grid == [1, 1, 1]
    }
}

fn ceil_div(a: i32, b: i32) -> i32 {
    (a + b - 1) / b
}

/// Compute a split plan. Parts are emitted in x -> y -> z grid order.
pub fn plan_split(size: [i32; 3], part_max: i32) -> SplitPlan {
    let max = part_max.max(1);
    let grid = [
        ceil_div(size[0], max).max(1),
        ceil_div(size[1], max).max(1),
        ceil_div(size[2], max).max(1),
    ];
    let mut parts = Vec::with_capacity((grid[0] * grid[1] * grid[2]) as usize);
    for i in 0..grid[0] {
        for j in 0..grid[1] {
            for k in 0..grid[2] {
                let offset = [i * max, j * max, k * max];
                let part_size = [
                    (size[0] - offset[0]).min(max),
                    (size[1] - offset[1]).min(max),
                    (size[2] - offset[2]).min(max),
                ];
                parts.push(Part {
                    grid_index: [i, j, k],
                    offset,
                    size: part_size,
                });
            }
        }
    }
    SplitPlan { grid, parts }
}

/// Part filename for a base name: `<base>.x<i>y<j>z<k>.nbt`.
pub fn part_filename(base: &str, grid_index: [i32; 3]) -> String {
    format!(
        "{base}.x{}y{}z{}.nbt",
        grid_index[0], grid_index[1], grid_index[2]
    )
}

/// Manifest filename: `<base>.split.json`.
pub fn manifest_filename(base: &str) -> String {
    format!("{base}.split.json")
}

#[derive(Serialize)]
struct PartManifest {
    file: String,
    grid_index: [i32; 3],
    offset: [i32; 3],
    size: [i32; 3],
}

#[derive(Serialize)]
struct SplitManifest {
    base: String,
    data_version: i32,
    source_size: [i32; 3],
    source_offset: [i32; 3],
    part_max: i32,
    grid: [i32; 3],
    parts: Vec<PartManifest>,
}

/// Render the split manifest as pretty JSON (deterministic — fixed field order,
/// parts in grid order).
pub fn manifest_json(
    base: &str,
    data_version: i32,
    source_size: [i32; 3],
    source_offset: [i32; 3],
    part_max: i32,
    plan: &SplitPlan,
) -> String {
    let manifest = SplitManifest {
        base: base.to_string(),
        data_version,
        source_size,
        source_offset,
        part_max,
        grid: plan.grid,
        parts: plan
            .parts
            .iter()
            .map(|p| PartManifest {
                file: part_filename(base, p.grid_index),
                grid_index: p.grid_index,
                offset: p.offset,
                size: p.size,
            })
            .collect(),
    };
    serde_json::to_string_pretty(&manifest).expect("manifest serializes")
}
