//! The horizon model (spec-0026): one place where a campaign's resolved
//! stage-1 `horizon` becomes **declared physical facts** the rest of the
//! compiler reads — the placement datum, the flood level, the analytic ambient
//! and the emitted world generator. Nothing outside this module matches on
//! [`HorizonBase`] to decide world physics.
//!
//! ## The world model (spec-0026 §2)
//!
//! Each horizon declares:
//!
//! - **`walk_ref_y`** — the world y a placed piece's *walk plane* must land at.
//!   The per-area placement datum is `walk_ref_y − walk_y`, where `walk_y` is
//!   the tileset walk-plane convention declared in prefab metadata
//!   ([`crate::registry::PrefabMeta::walk_y`]; missing in a non-void horizon is
//!   `DW0367`, `crate::plan`). This supersedes spec-0013's single global
//!   `OCEAN_BASE_Y` datum — the datum that made the #149 flooded-interior class
//!   possible (an island-tileset constant applied to every tileset).
//! - **`flood_level`** — the world y at or below which a standable cell is
//!   under the ambient water. Proved *empirically* against the assembled
//!   geometry after placement (`DW0364`, [`crate::nav::check_flood_level`]) —
//!   declarations position, proofs read reality.
//! - **ambient** — what a column the compiler modelled nothing into actually
//!   contains ([`crate::nav::Ambient`]); the premise of the `DW0322`
//!   boundary-safety proof.
//! - the emitted `server.properties` world generator (`level-type` +
//!   `generator-settings`), the one channel the toolserver/delve images
//!   consume (task #84 parity).
//!
//! ## Plug-in contract for surround generators (W-B/W-C slices)
//!
//! A surround-bearing base (`valley`, `summit`, the flatland seam band, the
//! sky archipelago solver hooks) plugs in here, not across the codebase:
//!
//! 1. implement the generator in its **own module** (e.g. `crate::valley`),
//!    consuming the [`delvewright_dsl::ResolvedHorizon`] params and the
//!    campaign seed, and emitting placed prefab tiles like any other piece;
//! 2. flip that base's arm in [`base_implemented`] to `true` — the DSL-layer
//!    reserved-base rejection (`DW0141`, `delvewright_dsl::validate`) keys off
//!    the same slice status and is deleted in the same PR;
//! 3. extend [`generator_settings`] / [`walk_ref_y`] / `nav::Ambient` with the
//!    base's ambient facts (one `match` arm each — the compiler refuses to
//!    build an unimplemented base long before those arms are reachable).
//!
//! Everything else (per-area datum, DW0364/DW0367, server emission plumbing)
//! is already base-generic and needs no edits.

use delvewright_dsl::{Campaign, HorizonBase, ResolvedHorizon, resolved_horizon};

/// The resolved horizon of a campaign (defaults applied; `void` when absent).
pub fn of_campaign(campaign: &Campaign) -> ResolvedHorizon {
    resolved_horizon(&campaign.world.content.horizon)
}

/// The resolved base of a campaign's horizon.
pub fn base_of(campaign: &Campaign) -> HorizonBase {
    of_campaign(campaign).base
}

/// Whether this delvec slice implements the base end-to-end (spec-0026
/// foundation: `void`, `ocean`, `flatland`). The DSL validation layer rejects
/// the others (`DW0141` reserved), so compiler paths may treat an
/// unimplemented base as unreachable-after-validation.
pub fn base_implemented(base: HorizonBase) -> bool {
    match base {
        HorizonBase::Void | HorizonBase::Ocean | HorizonBase::Flatland | HorizonBase::Valley => {
            true
        }
        HorizonBase::Sky | HorizonBase::Summit => false,
    }
}

/// Sea level of the `ocean` horizon superflat (spec-0013): the pinned
/// bedrock/stone/water layer stack (1 + 118 + 8 from the -64 build floor) tops
/// the water at y=62. Emission pins the same stack in `generator-settings`.
pub const SEA_LEVEL: i32 = 62;

/// Height of the `ocean` horizon superflat's water layer (spec-0013) — the `8`
/// in the pinned `generator-settings` stack emission writes
/// (`emit::emit_server`). Ambient water occupies `SEA_LEVEL - 7 ..= SEA_LEVEL`.
pub const OCEAN_WATER_LAYERS: i32 = 8;

/// Y of the topmost ambient **solid** block of the `ocean` horizon superflat:
/// the sea floor (stone) directly under the water layers, at 54. The ambient
/// model boundary safety reasons about (`nav::Sea`) starts here — below it the
/// world is stone all the way to bedrock, which is why an ocean world has no
/// void column anywhere.
pub const SEA_FLOOR_TOP_Y: i32 = SEA_LEVEL - OCEAN_WATER_LAYERS;

/// Y of the `flatland` horizon's grass surface (spec-0026 §1): the pinned
/// bedrock/dirt/grass superflat (1 + 126 + 1 layers from the -64 build floor)
/// tops its grass at y=63 — **exactly one block under the scene walk plane**
/// ([`FLATLAND_WALK_REF_Y`]), the §3 zero-height-difference seam by datum
/// equation, never by blending.
pub const FLATLAND_SURFACE_Y: i32 = 63;

/// The `ocean` walk reference (spec-0026 §2): sea level + 1, the vanilla-normal
/// beach relationship — a walk plane one block above the top water block.
pub const OCEAN_WALK_REF_Y: i32 = SEA_LEVEL + 1;

/// The `flatland` walk reference: the grass surface + 1. Numerically equal to
/// `plan::BASE_Y` (64), so a walk_y=0 tileset sits exactly where `void` puts
/// every area — flatland relocates nothing, it just fills the world in.
pub const FLATLAND_WALK_REF_Y: i32 = FLATLAND_SURFACE_Y + 1;

/// The `valley` gap floor's top solid block (spec-0026 §1: the flat floor
/// between the scene edge and the inner slopes). The valley ambient is void,
/// so the floor y is a free convention — pinned to the flatland relationship
/// (one block under `plan::BASE_Y`) so a valley, like flatland, relocates
/// nothing relative to a `void` build; only the surround tiles differ.
pub const VALLEY_GAP_FLOOR_TOP_Y: i32 = 63;

/// The `valley` walk reference (spec-0026 §2): gap floor + 1.
pub const VALLEY_WALK_REF_Y: i32 = VALLEY_GAP_FLOOR_TOP_Y + 1;

/// The world y a placed piece's walk plane must land at under this horizon
/// (spec-0026 §2), or `None` for `void` — the one base with no physical
/// reference plane, where areas keep the historical `plan::BASE_Y` origin.
///
/// `sky` reads its `float_y` param; `valley`/`summit` derive from their
/// surround geometry and land with their generator slices (unreachable behind
/// [`base_implemented`] until then).
pub fn walk_ref_y(h: &ResolvedHorizon) -> Option<i32> {
    match h.base {
        HorizonBase::Void => None,
        HorizonBase::Ocean => Some(OCEAN_WALK_REF_Y),
        HorizonBase::Flatland => Some(FLATLAND_WALK_REF_Y),
        HorizonBase::Sky => Some(h.float_y),
        HorizonBase::Valley => Some(VALLEY_WALK_REF_Y),
        // Summit: plateau top + 1 — lands with its surround generator (W-C);
        // until then validation refuses the base (`base_implemented`).
        HorizonBase::Summit => None,
    }
}

/// The world y at or below which a standable cell is flooded by the ambient
/// (spec-0026 §2 hazard facts): `ocean` 62, every other base none. Proved
/// empirically by `DW0364` (`crate::nav::check_flood_level`) with **no
/// waterline exemption** — the check reads assembled geometry, never metadata.
pub fn flood_level(h: &ResolvedHorizon) -> Option<i32> {
    match h.base {
        HorizonBase::Ocean => Some(SEA_LEVEL),
        _ => None,
    }
}

/// The pinned `generator-settings` JSON for the horizon's ambient superflat
/// (fixed literals — deterministic emission, ADR-0006). The `void` literal is
/// byte-identical to every pre-0.6 campaign's; `ocean` to every v0.6 one.
pub fn generator_settings(h: &ResolvedHorizon) -> &'static str {
    match h.base {
        HorizonBase::Ocean => {
            "{\"biome\":\"minecraft:ocean\",\"layers\":[{\"block\":\"minecraft:bedrock\",\"height\":1},{\"block\":\"minecraft:stone\",\"height\":118},{\"block\":\"minecraft:water\",\"height\":8}]}"
        }
        HorizonBase::Flatland => {
            "{\"biome\":\"minecraft:plains\",\"layers\":[{\"block\":\"minecraft:bedrock\",\"height\":1},{\"block\":\"minecraft:dirt\",\"height\":126},{\"block\":\"minecraft:grass_block\",\"height\":1}]}"
        }
        // `sky` will emit its declared backdrop's generator (spec-0026 §4, the
        // sky slice); until then it is unreachable behind `base_implemented`.
        // `valley`/`summit` ambients are void below their tile skirts.
        HorizonBase::Void | HorizonBase::Sky | HorizonBase::Valley | HorizonBase::Summit => {
            "{\"biome\":\"minecraft:the_void\",\"layers\":[]}"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use delvewright_dsl::horizon_defaults;

    /// The flatland datum equation (spec-0026 §3): grass top is exactly one
    /// block under the walk reference, and the layer arithmetic agrees with
    /// the emitted literal (1 + 126 + 1 layers from the −64 floor → top 63).
    #[test]
    fn flatland_datum_equation() {
        assert_eq!(FLATLAND_SURFACE_Y, -64 + 1 + 126 + 1 - 1);
        assert_eq!(FLATLAND_WALK_REF_Y, FLATLAND_SURFACE_Y + 1);
        assert_eq!(FLATLAND_WALK_REF_Y, crate::plan::BASE_Y);
    }

    /// The ocean facts are the spec-0013 pins, unchanged by the datum rework.
    #[test]
    fn ocean_facts_unchanged() {
        let h = ResolvedHorizon::of_base(HorizonBase::Ocean);
        assert_eq!(walk_ref_y(&h), Some(63));
        assert_eq!(flood_level(&h), Some(62));
        assert_eq!(SEA_FLOOR_TOP_Y, 54);
    }

    /// Sky's walk reference is its `float_y` param (default 160).
    #[test]
    fn sky_walk_ref_is_float_y() {
        let h = ResolvedHorizon::of_base(HorizonBase::Sky);
        assert_eq!(walk_ref_y(&h), Some(horizon_defaults::FLOAT_Y));
    }
}
