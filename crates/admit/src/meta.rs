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
    Anchor, AnchorEdit, Connector, GeneratedBy, License, Lighting, LightingProfile, PieceTemplate,
    PrefabMeta, Region, StructureMeta,
};

use crate::light::LightProbe;

/// Write a probe result into a document's `lighting` block, marked as a static
/// estimate.
///
/// `measured` is present and empty on a measured profile: `Lighting` refuses a
/// `lit`/`dim`/`dark` profile that does not carry both `measured_min_light` and
/// `measured`, so omitting it would produce metadata the compiler then rejects.
/// The value is empty because a static estimate has no measurement date to
/// state, and `method` says in full that it is not a live probe.
///
/// The `method` sentence states the **binding**: how many cells the minimum was
/// taken over, and out of what. A measurement whose binding is not written down
/// beside it cannot be read afterwards — a `lit` taken over four cells and a
/// `lit` taken over four thousand are the same word.
///
/// A probe that bound to nothing is never written; the caller refuses first
/// (`DW0752`), because "unbound" is a finding about the piece and not a
/// lighting profile.
pub fn set_lighting_from_probe(doc: &mut PrefabMeta, p: &LightProbe) {
    debug_assert!(
        !p.is_unbound(),
        "an unbound probe is a finding, not a profile"
    );
    let method = Some(format!(
        "static block-light BFS estimate (delve-admit): min over {} roofed floor cell(s) \
         reachable on foot from {} ground-level entry cell(s), of {} standable in the region \
         box; openings treated as sealed edge (sky-light=0). NOT a live-server probe; \
         dark_threshold={}. Re-probe live for borderline pieces.",
        p.measured_cells, p.entry_cells, p.standable_cells, p.dark_threshold
    ));
    doc.lighting = Some(match (p.profile, p.measured_min_light) {
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
        // Unreachable behind the caller's `DW0752` refusal, and stated rather
        // than assumed: with no binding there is no minimum, and a profile
        // without its measurement is the claim this type refuses.
        _ => Lighting {
            method,
            ..Lighting::unmeasured()
        },
    });
}
