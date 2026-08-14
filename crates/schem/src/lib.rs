//! `delve-schem` library: Sponge schematic (`.schem`, v2/v3) -> vanilla structure
//! `.nbt` conversion for the spec-0007 asset pipeline.
//!
//! The pipeline is: [`schematic::parse_schematic`] -> optional [`split`] plan ->
//! [`convert::build_region`] per part (running the safety strip) -> gzip-framed
//! structure template(s) + a split manifest. All output is byte-deterministic
//! (ADR-0006): stable palette ordering, fixed block iteration, `BTreeMap`-backed
//! NBT, and gzip mtime pinned to 0.
//!
//! A prefab is the `.nbt` plus the sibling metadata JSON that describes it, so
//! this crate reaches both halves of that pair: [`convert`] writes the bytes and
//! [`prefab`] is the document, re-exported from
//! [`delvewright_dsl::prefab`] where it is defined. Every tool that produces,
//! reads or edits a prefab — the grammar back end, the hand-written generators,
//! `delve-admit`, `delve-render`, `delvec` — goes through that one definition
//! rather than a local copy of its shape, because a copy that models fewer
//! fields deletes the rest on read-modify-write and reports nothing, and a copy
//! that refuses what it does not model turns a forward addition into an outage.

pub mod blocks;
pub mod convert;
pub mod diag;
pub mod fixtures;
pub mod nbt;
pub mod prefab;
pub mod schematic;
pub mod split;

use diag::{DW_DATAVERSION, DW_SPLIT, Diagnostic};
use schematic::ParseError;
use split::{manifest_filename, manifest_json, part_filename, plan_split};

/// Where the converted structure(s) landed.
pub enum ConvertOutput {
    /// The schematic fit in one structure template.
    Single(Vec<u8>),
    /// The schematic was tiled; parts are `(filename, gzip bytes)` and the
    /// manifest is JSON text under `manifest_name`.
    Split {
        parts: Vec<(String, Vec<u8>)>,
        manifest_name: String,
        manifest_json: String,
    },
}

/// The result of a conversion, including audit outputs.
pub struct ConvertResult {
    pub output: ConvertOutput,
    pub diagnostics: Vec<Diagnostic>,
    /// Full input block-state palette (sorted), for `--palette-report`.
    pub palette: Vec<String>,
    pub size: [i32; 3],
    pub source_version: i32,
    pub grid: [i32; 3],
}

/// Convert a raw `.schem` byte stream. `base` names split parts/manifest.
pub fn convert(input: &[u8], base: &str, split_max: i32) -> Result<ConvertResult, ParseError> {
    let schem = schematic::parse_schematic(input)?;
    let palette = convert::palette_report(&schem);
    let mut diagnostics = Vec::new();

    // Note a source DataVersion that is not our pinned target — block states are
    // reinterpreted as 1.21.11 without migration (see README limitations).
    if let Some(dv) = schem.source_data_version
        && dv != convert::DATA_VERSION
    {
        diagnostics.push(Diagnostic::warning(
            DW_DATAVERSION,
            format!(
                "source DataVersion {dv} != target {} (1.21.11): block states are reinterpreted \
                 without migration and may be wrong — re-export the schematic from Minecraft \
                 1.21.11 to silence this and guarantee fidelity",
                convert::DATA_VERSION
            ),
        ));
    }

    let plan = plan_split(schem.size, split_max);

    let output = if plan.is_single() {
        let bytes = convert::build_region(&schem, [0, 0, 0], schem.size, &mut diagnostics);
        ConvertOutput::Single(bytes)
    } else {
        diagnostics.push(Diagnostic::warning(
            DW_SPLIT,
            format!(
                "schematic {}x{}x{} exceeds the {split_max}-cube cap; tiled into {}x{}x{} parts",
                schem.size[0],
                schem.size[1],
                schem.size[2],
                plan.grid[0],
                plan.grid[1],
                plan.grid[2]
            ),
        ));
        let mut parts = Vec::with_capacity(plan.parts.len());
        for part in &plan.parts {
            let bytes = convert::build_region(&schem, part.offset, part.size, &mut diagnostics);
            parts.push((part_filename(base, part.grid_index), bytes));
        }
        let manifest_json = manifest_json(
            base,
            convert::DATA_VERSION,
            schem.size,
            schem.offset,
            split_max,
            &plan,
        );
        ConvertOutput::Split {
            parts,
            manifest_name: manifest_filename(base),
            manifest_json,
        }
    };

    Ok(ConvertResult {
        output,
        diagnostics,
        palette,
        size: schem.size,
        source_version: schem.version,
        grid: plan.grid,
    })
}
