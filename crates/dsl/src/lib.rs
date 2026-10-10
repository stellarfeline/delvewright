//! Delvewright DSL: serde types, validation, canonical serialization and JSON
//! Schema export for the staged campaign DSL (spec-0001).
//!
//! Entry points:
//! - [`parse_campaign`] / [`check_campaign`]: parse the six raw stage documents.
//! - [`validate_campaign`]: run all spec-0001 v0.2 rule groups on a parsed
//!   [`Campaign`], returning [`Diagnostic`]s (spec-0002 `--json` shape).
//! - [`to_canonical_string`]: the single canonical writer — pretty JSON put
//!   through [`fmt`], so what the compiler WRITES is what `delvec fmt --check`
//!   accepts. One canonical form, one implementation.
//! - [`fmt`]: that canonical form as a grammar-level formatter over any
//!   authored JSON (stage documents, l10n sidecars, prefab metadata).
//! - [`stage_schema`]: export a stage's JSON Schema.
//!
//! Each stage type lives in the module of the object it declares (ADR-0031), and
//! the root re-exports every module's public items through one glob per module.
//! Every stage struct is `deny_unknown_fields`; reserved enum values a campaign's
//! `dsl_version` is too low for parse and are refused by [`validate`].
//!
//! Determinism (ADR-0006): all iteration is over `BTreeMap`/`BTreeSet` or slices;
//! nothing depends on hash order, wall-clock, or absolute paths.

pub mod actor;
pub mod ambush;
pub mod assembly;
pub mod blocks;
pub mod blockshape;
pub mod body;
pub mod canonical;
pub mod cast;
pub mod celestial;
pub mod chrome;
pub mod class;
pub mod color;
pub mod cutscene;
pub mod design;
pub mod detailplan;
pub mod diagnostic;
pub mod dialogue;
pub mod economy;
pub mod effects;
pub mod envelope;
pub mod equipment;
pub mod fight;
pub mod firework;
/// **What a cell does when there is fluid beside it** — block knowledge, so it
/// lives beside [`blocks`] and [`blockshape`] rather than beside any one reader
/// of it. `delvewright_schem::fluid` re-exports it, and the prefab generators
/// reach it through their dependency on this crate: they may not depend on
/// `delvec`, and this is the crate every reader of a block fact can reach.
pub mod fluid;
pub mod fmt;
pub mod gate;
pub mod healthbar;
pub mod ids;
pub mod l10n;
pub mod layout;
pub mod lethal;
pub mod license;
pub mod lightning;
pub mod r#loop;
pub mod loot;
pub mod mark;
pub mod mclang;
pub mod metrics;
pub mod npc;
pub mod onkill;
pub mod perception;
pub mod placement;
pub mod pulse;
pub mod prefab;
pub mod quest;
pub mod quest_plan;
pub mod registry;
pub mod rig;
pub mod schema;
mod serde_fields;
pub mod shortcut;
pub mod siteplan;
pub mod split;
pub mod state;
pub mod stealth;
pub mod textstyle;
pub mod timed_gate;
pub mod trap;
pub mod trigger;
pub mod validate;
pub mod viewdistance;
pub mod wave;
pub mod world;
pub mod world_edits;

pub use actor::*;
pub use ambush::*;
pub use assembly::*;
pub use body::*;
pub use canonical::*;
pub use cast::*;
pub use celestial::*;
pub use chrome::*;
pub use class::*;
pub use cutscene::*;
pub use design::*;
pub use detailplan::*;
pub use diagnostic::*;
pub use dialogue::*;
pub use economy::*;
pub use effects::*;
pub use envelope::*;
pub use equipment::*;
pub use fight::*;
pub use firework::*;
pub use gate::*;
pub use healthbar::*;
pub use ids::*;
pub use l10n::*;
pub use layout::*;
pub use lethal::*;
pub use r#loop::*;
pub use loot::*;
pub use mark::*;
pub use mclang::*;
pub use npc::*;
pub use onkill::*;
pub use placement::*;
pub use pulse::*;
pub use prefab::*;
pub use quest::*;
pub use quest_plan::*;
pub use registry::*;
pub use schema::*;
pub use shortcut::*;
pub use siteplan::*;
pub use state::*;
pub use stealth::*;
pub use timed_gate::*;
pub use trap::*;
pub use trigger::*;
pub use validate::*;
pub use wave::*;
pub use world::*;
pub use world_edits::*;
