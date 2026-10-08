//! Coordinates and selectors: the cell-to-entity conversion, facing yaw, box selectors, anchor points.

use super::*;

/// Yaw for a facing keyword (MC: yaw 0 = +z/south).
pub(crate) fn facing_yaw(facing: Option<&str>) -> i32 {
    match facing {
        Some("north") => 180,
        Some("east") => 270,
        Some("west") => 90,
        _ => 0, // south / default
    }
}

/// The world position an entity is summoned at, formatted per axis: the horizontal
/// **centre** of the cell, on its floor ([`crate::compiler::nav::cell_center`]).
///
/// A block cell `(x, y, z)` spans `[x, x+1)`, and an entity's position is the centre
/// of its AABB — so summoning at the bare integer cell parks the body on the corner
/// where four columns meet, with most of it inside the neighbouring columns (a
/// 0.6-wide villager at `x = 7.0` occupies `[6.7, 7.3]`). Against a wall that reads
/// as an NPC standing inside the wall; along a walked path it is the owner's
/// "visibly passes through blocks" defect. Every entity the compiler places or
/// moves goes through this conversion.
///
/// Block-targeting commands (`setblock`, `fill`, `place`, `spawnpoint`) keep the
/// integer cell — that is the coordinate space they take.
pub(super) fn ent_xyz(c: [i32; 3]) -> [String; 3] {
    let p = crate::compiler::nav::cell_center(c);
    [fmt_f64(p[0]), fmt_f64(p[1]), fmt_f64(p[2])]
}

/// The centre of a block cell on a horizontal axis, as the compiler writes it into
/// a `tp`. Vanilla's own respawn lands a player at `cell + 0.5` on X/Z, so the
/// re-seat has to agree with it or a correct respawn would visibly twitch. Written
/// through `f64` (not string concatenation) because `-16` centres on `-15.5`, not
/// `-16.5`; the value is exactly representable, so the text is deterministic.
pub(super) fn center(cell: i32) -> String {
    format!("{:.1}", cell as f64 + 0.5)
}

/// A `@a`-selector volume covering an inclusive block region — the same box
/// model `damage-players`' `within` filter uses. `dx/dy/dz` are spans, so a
/// one-block region is `dx=0` and still selects the whole block.
///
/// Corners are normalised because a gate region's stored corners are whatever
/// the prefab metadata declared; a selector with a negative span selects
/// nothing, which would make the crush silently no-op.
pub(super) fn region_selector(from: [i32; 3], to: [i32; 3]) -> String {
    let lo = [from[0].min(to[0]), from[1].min(to[1]), from[2].min(to[2])];
    let hi = [from[0].max(to[0]), from[1].max(to[1]), from[2].max(to[2])];
    format!(
        "x={},dx={},y={},dy={},z={},dz={},tag=!{CUTSCENE_TAG}",
        lo[0],
        hi[0] - lo[0],
        lo[1],
        hi[1] - lo[1],
        lo[2],
        hi[2] - lo[2]
    )
}

/// A box as an entity-selector volume (`x=…,dx=…`) — the test the corpse is put to
/// so the placement table's death-region axis is a comparison rather than a search.
///
/// `dx` is the *span*, and a selector volume is half-open on the far side, so an
/// inclusive box `[lo, hi]` spans `hi − lo + 1` cells.
pub(super) fn selector_box(region: ([i32; 3], [i32; 3])) -> String {
    let (lo, hi) = region;
    format!(
        "x={},y={},z={},dx={},dy={},dz={}",
        lo[0],
        lo[1],
        lo[2],
        hi[0] - lo[0] + 1,
        hi[1] - lo[1] + 1,
        hi[2] - lo[2] + 1
    )
}

/// An inclusive block AABB as vanilla selector arguments — `x=…,dx=…,…`, with the
/// fixture-class exclusion every box-narrowed **entity** selector carries.
///
/// One spelling for one fact. Vanilla's `dx` is a *span*, not a count, so the box
/// `lo..=hi` is `dx = hi - lo`; every anchor-centred volume in the engine
/// (`lethal_volumes[]`, a `teleport`'s `from`, a status effect's `in`) resolves
/// through [`crate::compiler::plan::Plan::zone_box`] to exactly this pair and formats it
/// here, so no two verbs can disagree by one block about what "inside" means.
///
/// The exclusion is NOT added here, because the *player* selectors
/// (`damage-players`, `give-effect`, the volume's `@a` half) share this
/// formatter and no player is a fixture. [`entity_box_selector`] is the entity
/// spelling, and `DW0545` proves nothing else reaches a box.
pub(crate) fn box_selector_args(lo: [i32; 3], hi: [i32; 3]) -> String {
    format!(
        "x={},dx={},y={},dy={},z={},dz={}",
        lo[0],
        hi[0] - lo[0],
        lo[1],
        hi[1] - lo[1],
        lo[2],
        hi[2] - lo[2]
    )
}

/// A box as the arguments of an `@e[…]` selector: the volume, then the
/// fixture-class exclusion.
///
/// **Every** entity selector narrowed by a box in this engine goes through here,
/// and `DW0545` reads the shipped datapack to prove it. One term, negating one
/// class tag — never a `type=!…` roster, which grows with the engine and, on a
/// verb that moves rather than deletes, would strip an NPC's dialogue hitbox off
/// its body.
pub(super) fn entity_box_selector(lo: [i32; 3], hi: [i32; 3]) -> String {
    format!(
        "{},{}",
        box_selector_args(lo, hi),
        crate::compiler::affordance::FIXTURE_EXCLUDE
    )
}

/// Resolve an anchor name to a world point by scanning every area (first match),
/// mirroring how `open-gate` resolves its anchor. `None` if unresolved.
pub(super) fn anchor_point_any(plan: &Plan, anchor: &str) -> Option<[i32; 3]> {
    plan.point_any(anchor)
}
