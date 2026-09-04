//! Delvewright compiler (`delvec`): staged DSL in, deterministic datapack +
//! server assets out (spec-0002, ADR-0001/0006/0011).
//!
//! Modules — **all of them**, one line each. A partial list is worse than
//! none: it reads as an index, so a module missing from it reads as a module
//! that does not exist, and the twenty-four names this list used to carry
//! were the ones a reader happened to add. `the_module_list_names_every_module`
//! (`tests/module_list.rs`) is what keeps it whole.
//!
//! - [`affordance`]: affordance hardware — the visible half of every right-click target the compiler owns (`DW0420`/`DW0421`).
//! - [`analyze`]: deep quest/objective reachability (`DW02xx`, exit 2).
//! - [`assembled`]: the shared assembled-world block model every geometric proof reads.
//! - [`atmos`]: sound-event validation (`DW0326`), the refused `play-sound at: actor` (`DW0335`), and the `delve:art` banner font (`DW0328`).
//! - [`batchstate`]: a generated PackTest that drives an outcome owns every `#party` term the gates on it read (`DW0807`).
//! - [`blocking`]: `delvec blocking-chart` — per-elevation cutaway floor plans.
//! - [`blockout`]: the derived blockout — the map pipeline's stage nobody authors.
//! - [`branch`]: branch-complete narrative verification (`DW0480`–`DW0485`).
//! - [`calibrate`]: `delvec calibrate` — a harvested rehearsal report turned back into anchor + offset patches.
//! - [`camera`]: cutscene camera geometry: the eased dolly and the `shot_style` expansion, shared by emission and validation.
//! - [`cast`]: the NPC scene ledger (`DW0460`–`DW0467`).
//! - [`clearance`]: the body-vs-block proof — no body occupies the same space as block geometry (`DW0450`/`DW0451`).
//! - [`combat`]: compile-time combat winnability — the arithmetic half of the combat proofs.
//! - [`commands`]: the vendored 1.21.11 Brigadier command-tree validator.
//! - [`continuity`]: the NPC location-continuity lint (`DW0351`).
//! - [`creator`]: the playtest-only creator overlay (`creator-datapack/`).
//! - [`crosshair`]: two things the party must click may not stand where the crosshair cannot tell them apart (`DW0489`).
//! - [`daylight`]: a body the sun kills may not be staged where the sun reaches it (`DW0496`).
//! - [`deathplan`]: `validation/death-plan.json` — the bot tier's contract for dying.
//! - [`detail`]: a place is detailed inside the box the whole gave it.
//! - [`eclipse`]: no body stands in front of an affordance the party clicks (`DW0359`), no affordance shares a cell with a sealed gate's hitboxes (`DW0422`), and no two affordances share a cell with each other (`DW0878`).
//! - [`edit`]: the map editor's stage-7 edit-script replay.
//! - [`emit`]: build the `<out>/` output tree (bytes), deterministically.
//! - [`faces`]: does the piece next to this one answer the way out it declares?
//! - [`failure`]: the one type a compiler pass fails with — a DW code and the message that goes with it.
//! - [`flow`]: the branch-coherent flag/quest flow model and the critical-path extraction (`DW0204`).
//! - [`gates`]: `close-gate` gate-block validation — the physical dual of `open-gate`.
//! - [`gym`]: the metrics gym — a site-plan campaign generated from the metrics table.
//! - [`horizon`]: the one place a resolved `horizon` becomes the physical facts the rest of the compiler reads.
//! - [`integrity`]: the emitted call graph is closed — every `function` call points at a function the compiler wrote (`DW0497`).
//! - [`lethal`]: lethal volumes — the proofs a box that kills owes the completability model.
//! - [`light`]: the assembled-world lighting model and the deterministic relight pass (`DW0210`/`DW0211`).
//! - [`load`]: read a campaign directory into the DSL's `RawCampaign`, keeping the raw bytes for input hashing.
//! - [`loot`]: container-fill proofs over the assembled world (`DW0431`/`DW0438`).
//! - [`massing`]: the L2 massing verbs — declarative control of a pool area's solved jigsaw layout.
//! - [`nav`]: compile-time navigation over the solved voxel grid.
//! - [`plan`]: resolve a validated campaign into the placement + naming model emission reads.
//! - [`png`]: the deterministic hand-rolled PNG writer.
//! - [`pool`]: a pool draw that seats the same anchored prefab twice (`DW0498`).
//! - [`pressable`]: what a player's click reaches at an anchor — the one authority every `strike`/`use` trigger dispatches from.
//! - [`promise`]: an objective keeps the promise its prompt makes (`DW0860`–`DW0863`).
//! - [`raster`]: the shared RGBA canvas and bitmap-text primitives both renderers draw on.
//! - [`reach`]: what completes a `reach` — the one authority for the volume a body has to be in.
//! - [`registry`]: the full pinned-MC item registry and the prefab/anchor metadata.
//! - [`rehearsal`]: the compile-time inventory of every rehearsable cutscene beat and every shot inside it.
//! - [`render_plan`]: `render-plan.json` emission.
//! - [`resourcepack`]: the per-delve skin resource pack.
//! - [`respawn`]: what separates a retry from a soft-lock — the evidence `DW0478` accepts.
//! - [`seeding`]: no emitted comparison reads a score entry the emitted pack never creates (`DW0495`).
//! - [`snapshot`]: `delvec snapshot` — the voxel raycaster and scene manifest an authoring agent looks at its own build through.
//! - [`solver`]: the jigsaw layout solver.
//! - [`stairs`]: the stair-orientation proof over the assembled world (`DW0430`).
//! - [`stake`]: the recovery stake's compile-time placement table and the proofs it owes.
//! - [`statepath`]: a numeric gate judged against the writes the path performs before it (`DW0879`).
//! - [`surround`]: horizon surround generation — the tiles that dress the world outside the placed pieces.
//! - [`teleport`]: the `teleport` verb's one compile-time obligation, and the ledger saying what it looked at.
//! - [`textfit`]: on-screen text that does not fit what draws it (`DW0330`/`DW0331`).
//! - [`timeline`]: per-effect-timeline gate state — the static half of the `close-gate` model (`DW0410`).
//! - [`traversal`]: a walked leg may only contain moves the body walking it can make (`DW0452`/`DW0453`).
//! - [`view`]: the CPU render surface — the visual channel that ships in the one binary a creator installs.
//! - [`watch`]: runtime-watch coverage of per-object bodies, in two tiers.
//! - [`waypoints`]: the compiler-proven critical-path waypoint polyline, as validation metadata.
//! - [`ways`]: what a campaign does with a piece's contingent ways.
//! - [`wrongside`]: which side of a sealed shortcut door a player is standing on.

pub mod affordance;
pub mod analyze;
pub mod assembled;
pub mod atmos;
pub mod batchstate;
pub mod blocking;
pub mod blockout;
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
pub mod detail;
pub mod eclipse;
pub mod edit;
pub mod emit;
pub mod faces;
pub mod failure;
pub mod flow;
pub mod gates;
pub mod gym;
pub mod horizon;
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
pub mod promise;
pub mod raster;
pub mod reach;
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
pub mod statepath;
pub mod surround;
pub mod teleport;
pub mod textfit;
pub mod timeline;
pub mod traversal;
pub mod view;
pub mod watch;
pub mod waypoints;
pub mod ways;
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
