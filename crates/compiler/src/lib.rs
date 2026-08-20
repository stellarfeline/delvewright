//! Delvewright compiler (`delvec`): staged DSL in, deterministic datapack +
//! server assets out (spec-0002, ADR-0001/0006/0011).
//!
//! Modules:
//! - [`registry`]: the full pinned-MC item registry and prefab/anchor metadata.
//! - [`commands`]: the vendored 1.21.11 Brigadier command-tree validator.
//! - [`analyze`]: deep quest/objective reachability (`DW02xx`, exit 2).
//! - [`flow`]: the branch-coherent flag/quest flow model — XOR dialogue branches,
//!   gate-conditional flag producers, the single-branch critical-path extraction
//!   and its step-by-step replay proof (`DW0204`).
//! - [`plan`]: resolve the campaign into a placement/naming model.
//! - [`nav`]: compile-time pathfinding over the solved voxel grid — collision-safe
//!   `move-npc` walked paths (`DW0307`) + cutscene air-corridor checks (`DW0308`).
//! - [`light`]: assembled-world lighting model + deterministic relight pass
//!   (`DW0210`/`DW0211`, exit 2) + declared time/weather sky attenuation (spec-0010).
//! - [`eclipse`]: the body-vs-affordance occlusion proof (`DW0359`) — an NPC or
//!   actor body may not stand on, or immediately in front of, an interaction
//!   affordance the party has to click.
//! - [`clearance`]: the body-vs-block proof (`DW0450`/`DW0451`) — no NPC or actor
//!   body may occupy the same space as block geometry, at its spawn anchor or at
//!   any tick of any walked leg.
//! - [`traversal`]: the route-vs-capability proof (`DW0452`/`DW0453`) — a walked
//!   leg may only contain moves the BODY walking it can make, derived from its
//!   entity (a spider climbs, a ghast flies, nothing opens a fence gate).
//! - [`emit`]: build the `<out>/` output tree (bytes), deterministically.
//! - [`integrity`]: the emitted call graph is closed (`DW0497`) — a
//!   `function <ns>:<name>` the compiler writes must point at a function the
//!   compiler wrote, whatever feature emitted either half.
//! - [`seeding`]: the emitted score reads are closed (`DW0495`) — no comparison
//!   may read a scoreboard entry the pack never creates, because on the pinned
//!   server a missing entry is not zero, it is false to every question.
//! - [`waypoints`]: export the DW0311-proven critical-path routes as validation
//!   metadata (`validation/critical-path-waypoints.json`) for leg-by-leg bot nav.
//! - [`creator`]: the playtest-only creator overlay (`creator-datapack/`, spec-0006).
//! - [`png`]: the deterministic hand-rolled PNG writer shared by the `delve:art`
//!   font atlas and the visual-authoring-loop renders.
//! - [`blocking`]: `delvec blocking-chart` — per-elevation cutaway floor plans
//!   (spec-0015 pillar 3).
//! - [`raster`]: the shared RGBA canvas + bitmap-text primitives both
//!   visual-authoring-loop renderers draw on.
//! - [`snapshot`]: `delvec snapshot` — the voxel raycaster + scene manifest that
//!   let an authoring agent look at its own build (spec-0015).
//! - [`edit`]: the map editor's stage-7 edit-script replay (spec-0017) — seeded
//!   L3 verbs over the assembled world, per-batch invariant re-proofs, runtime
//!   `fill`/`setblock` materialization.
//! - [`timeline`]: per-effect-timeline gate state — which gates an earlier effect
//!   in the *same* bundle / `sequence` provably sealed, feeding the `DW0410`
//!   staged-walk proof in [`nav`].

pub mod affordance;
pub mod analyze;
pub mod assembled;
pub mod atmos;
pub mod blocking;
pub mod branch;
pub mod calibrate;
pub mod camera;
pub mod cast;
pub mod clearance;
pub mod combat;
pub mod commands;
pub mod continuity;
pub mod creator;
pub mod crosshair;
pub mod daylight;
pub mod deathplan;
pub mod eclipse;
pub mod edit;
pub mod emit;
pub mod faces;
pub mod flow;
pub mod gates;
pub mod integrity;
pub mod lethal;
pub mod light;
pub mod load;
pub mod loot;
pub mod massing;
pub mod nav;
pub mod plan;
pub mod png;
pub mod pool;
pub mod pressable;
pub mod raster;
pub mod registry;
pub mod rehearsal;
pub mod render_plan;
pub mod resourcepack;
pub mod respawn;
pub mod seeding;
pub mod snapshot;
pub mod solver;
pub mod stairs;
pub mod stake;
pub mod teleport;
pub mod textfit;
pub mod timeline;
pub mod traversal;
pub mod view;
pub mod waypoints;
pub mod wrongside;

/// This compiler's version (reported by `--version`, stamped in `manifest.json`).
///
/// Derived from `crates/compiler/Cargo.toml`'s `[package] version` at compile
/// time — the crate manifest is the one source of truth, so this can never
/// drift from the release identity the way a hand-typed literal can: a version
/// bump in `Cargo.toml` beside a hard-coded constant is a release identity that
/// never reaches a single emitted artifact.
pub const DELVEC_VERSION: &str = env!("CARGO_PKG_VERSION");

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
