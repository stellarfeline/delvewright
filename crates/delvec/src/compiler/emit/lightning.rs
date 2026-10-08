//! The `lightning` verb.

use super::*;

/// Emit a `lightning` effect (DSL v0.36, spec-0092): one `summon` of a
/// `minecraft:lightning_bolt` at the mark's cell centre, on the mark's plane, so
/// the block it strikes is the block under the mark — the cell
/// `compiler::lightning` reads for `DW0959`. Absolute coordinates, like every
/// point effect: the mark is a cell at build time.
pub(super) fn emit_lightning(plan: &Plan, at: &delvewright_dsl::Mark, body: &mut Vec<String>) {
    let Some(anchor) = anchor_point_any(plan, at.anchor.as_str()) else {
        return; // unresolved anchor (`DW0360` owns it)
    };
    let v = ent_xyz(at.cell(anchor));
    body.push(format!(
        "summon {entity} {x} {y} {z}",
        entity = delvewright_dsl::lightning::BOLT_ENTITY,
        x = v[0],
        y = v[1],
        z = v[2],
    ));
}
