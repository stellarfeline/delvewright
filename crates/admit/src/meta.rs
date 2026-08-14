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
    Anchor, AnchorEdit, Connector, GeneratedBy, License, Lighting, LightingProfile, PrefabMeta,
    Region, StructureMeta,
};

use crate::light::LightProbe;

/// Write a probe result into `meta`'s `lighting` block, marked as a static
/// estimate.
///
/// `measured` is present and empty on a measured profile: `Lighting` refuses a
/// `lit`/`dim`/`dark` profile that does not carry both `measured_min_light` and
/// `measured`, so omitting it would produce metadata the compiler then rejects.
/// The value is empty because a static estimate has no measurement date to
/// state, and `method` says in full that it is not a live probe.
///
/// A piece with no walkable floor has nothing to measure, and that is
/// `unmeasured` — a positive statement that the measurement is owed — carrying
/// no measurement fields, because a claim and its absence cannot both be true.
pub fn set_lighting_from_probe(meta: &mut PrefabMeta, p: &LightProbe) {
    let method = Some(format!(
        "static block-light BFS estimate (delve-admit): min over {} walkable floor cells; \
         doorways treated as sealed edge (sky-light=0). NOT a live-server probe; \
         dark_threshold={}. Re-probe live for borderline pieces.",
        p.floor_cells, p.dark_threshold
    ));
    meta.lighting = Some(match (p.profile, p.measured_min_light) {
        ("dark", Some(m)) => Lighting {
            profile: LightingProfile::Dark,
            measured_min_light: Some(m as i64),
            measured: Some(String::new()),
            rationale: None,
            method,
        },
        ("lit", Some(m)) => Lighting {
            profile: LightingProfile::Lit,
            measured_min_light: Some(m as i64),
            measured: Some(String::new()),
            rationale: None,
            method,
        },
        // No floor to stand on, so no minimum to report.
        _ => Lighting {
            method,
            ..Lighting::unmeasured()
        },
    });
}
