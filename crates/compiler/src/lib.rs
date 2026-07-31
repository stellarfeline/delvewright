//! Delvewright compiler (`delvec`): staged DSL in, deterministic datapack +
//! server assets out (spec-0002, ADR-0001/0006/0011).
//!
//! Modules:
//! - [`registry`]: the full pinned-MC item registry and prefab/anchor metadata.
//! - [`commands`]: the vendored 1.21.11 Brigadier command-tree validator.
//! - [`analyze`]: deep quest/objective reachability (`DW02xx`, exit 2).
//! - [`plan`]: resolve the campaign into a placement/naming model.
//! - [`emit`]: build the `<out>/` output tree (bytes), deterministically.
//! - [`creator`]: the playtest-only creator overlay (`creator-datapack/`, spec-0006).

pub mod analyze;
pub mod commands;
pub mod creator;
pub mod emit;
pub mod load;
pub mod plan;
pub mod registry;
pub mod render_plan;
pub mod resourcepack;
pub mod solver;

/// This compiler's version (reported by `--version`, stamped in `manifest.json`).
pub const DELVEC_VERSION: &str = "0.1.0";

/// The pinned Minecraft version (ADR-0009).
pub const MC_VERSION: &str = "1.21.11";

/// The MC 1.21.11 data pack format (`pack.mcmeta`) as `[major, minor]` = 94.1.
///
/// 1.21.11's `version.json` reports `data_major: 94, data_minor: 1`. Packs whose
/// format is newer than 81 MUST declare `min_format`/`max_format` (verified live
/// on a 1.21.11 server: a bare `pack_format` is rejected with "Pack declares
/// support for version newer than 81, but is missing mandatory fields min_format
/// and max_format"). Both are emitted as `[major, minor]` arrays.
pub const PACK_FORMAT: [u32; 2] = [94, 1];

/// The MC 1.21.11 structure `DataVersion` (see `data/PROVENANCE.md`).
pub const DATA_VERSION: i32 = 4671;

/// The DSL version this compiler implements (re-exported from the DSL crate).
pub const DSL_VERSION: &str = delvewright_dsl::SUPPORTED_DSL_VERSION;
