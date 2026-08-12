//! Prefab metadata (`<basename>.json` beside the `.nbt`), as `delve-admit` uses
//! it.
//!
//! The document's shape is not defined here. It is
//! [`delvewright_schem::prefab`], the crate that also writes the `.nbt` half of
//! the pair, so an admitted external piece and a generated one are the same
//! document produced by two tools — and, more to the point, so that an admission
//! step that reads a prefab, edits one block of it and writes it back cannot
//! silently drop the blocks it does not itself model. `anchor` adds to
//! `anchors`, `socket` appends to `connectors`, `lighting` sets `lighting`;
//! everything else is carried through untouched because it is parsed, not
//! ignored.
//!
//! What lives here is only what is specific to this tool: turning
//! [`crate::light::LightProbe`] — a type the shared crate has no reason to know —
//! into a `lighting` block.

pub use delvewright_schem::prefab::{
    Anchor, Connector, GeneratedBy, License, Lighting, PrefabMeta, Region, StructureMeta,
};

use crate::light::LightProbe;

/// Write a probe result into `meta`'s `lighting` block, marked as a static
/// estimate.
///
/// `measured` is present and empty: `delvewright_dsl`'s `Lighting` refuses a
/// `lit`/`dim`/`dark` profile that does not carry both `measured_min_light` and
/// `measured`, so omitting it would produce metadata the compiler then rejects.
/// The value is empty because a static estimate has no measurement date to
/// state, and `method` says in full that it is not a live probe.
pub fn set_lighting_from_probe(meta: &mut PrefabMeta, p: &LightProbe) {
    meta.lighting = Lighting {
        profile: p.profile.to_string(),
        measured_min_light: p.measured_min_light,
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
