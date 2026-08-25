//! **What a cell of a piece does when there is fluid beside it.**
//!
//! A body of fluid is the one thing an author places that does not stay placed.
//! Every other block is where it was written; water and lava run — down first,
//! then sideways — until something holds them, and they run on the server's own
//! clock, before any player arrives. So a piece carrying a pond, a channel or a
//! flooded cellar is asserting something about the cells around it, and nothing
//! upstream of the server had been checking that assertion.
//!
//! Three facts decide it. All three are block knowledge rather than grid
//! knowledge, which is why they live here beside the registry every emitter and
//! auditor already depends on — and all three were **measured on the pinned
//! 1.21.11 server** (`tools/spike-block-settling/`, its `observations.json`),
//! because two of them are the opposite of what this module was first written
//! with:
//!
//! - **A fluid block runs.** `minecraft:water` / `minecraft:lava` in a cell
//!   spreads into any open cell beside or below it — measured: a source with
//!   one open neighbour and no other way out put 8 flowing cells into it, and
//!   the same source in a sealed box stayed 1 source, 0 flowing.
//! - **A block written `waterlogged=true` does NOT run.** It holds its water,
//!   spreads nothing, and stays waterlogged — measured beside an open cell, and
//!   again after a block update was forced next to it (the update schedules a
//!   fluid tick, so "it has not moved yet" and "it will not move" are different
//!   claims and this rig tells them apart). It is fluid the author placed, and
//!   it is not a body that leaks.
//! - **A block written `waterlogged=false` is a WALL.** Spreading water does not
//!   fill it. Measured five ways — a grate and a stair in a wall, a stair turned
//!   so its open face meets the water, a stair with a source on each side, and a
//!   source directly above one — and every rig left the block dry and the water
//!   where it was put. The plausible-sounding opposite ("a grate is a hole,
//!   because iron bars are waterloggable") is a claim about *placing* water in
//!   that cell, not about a body beside it.
//!
//! So the rule is short: **a fluid block must be a source, and must have
//! something — anything — in each of the five cells it would run into.**
//!
//! # The documented residue
//!
//! Any written non-fluid block is read here as holding fluid. Vanilla is
//! stricter in one direction this cannot see: it also flows into, and destroys,
//! blocks that do not block movement — a torch, a carpet, a plant. Separating
//! those from stone needs collision shapes, which are not in the pinned data
//! tables this repo derives from. That is the honest under-approximation: this
//! rule can miss such a leak and cannot invent one.

use std::collections::BTreeMap;

/// A body of fluid in a piece is not still, or not where it was authored.
pub const DW_FLUID_ESCAPES: &str = "DW0800";

/// Minecraft's two fluids, as this module's callers spell them. The membership
/// question is [`delvewright_dsl::blockshape::is_fluid`]; this is the list a
/// diagnostic prints.
pub const FLUIDS: [&str; 2] = ["minecraft:water", "minecraft:lava"];

/// How wet one cell is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wetness {
    /// No fluid in this cell.
    Dry,
    /// A fluid source: a fluid block at `level=0`. **This is the fluid that
    /// runs**, and the only cell kind the containment rule binds.
    Source,
    /// A fluid block mid-flow. `level` is a value vanilla derives from the
    /// cell's neighbours and re-derives on its own clock; a piece cannot pin
    /// one, so an authored flow is a defect rather than a state.
    Flowing(u8),
    /// A block holding water in the same cell (`waterlogged=true`). Wet, and
    /// measured not to spread.
    Held,
}

fn qualify(name: &str) -> String {
    if name.contains(':') {
        name.to_string()
    } else {
        format!("minecraft:{name}")
    }
}

/// True when this id is one of the game's fluid blocks.
///
/// **Not decided here** (spec-0056). This module and `delvec` each used to carry
/// a fluid list, tied together by a cross-crate test whose own header said the
/// duplication was unavoidable because `delvec` may not depend on this crate.
/// That premise was true and is no longer the whole truth: both already depend on
/// `delvewright-dsl`, which is where the block-shape table now lives, so the two
/// lists collapse into one and the test tying them becomes a check that the
/// delegation is real.
pub fn is_fluid(name: &str) -> bool {
    super::blockshape::is_fluid(name)
}

/// True for the three air blocks — the cells a fluid runs into. Also the
/// block-shape authority's, for the same reason.
pub fn is_air(name: &str) -> bool {
    super::blockshape::is_air(name)
}

/// True for the one block that means "whatever was already here": a cell the
/// piece deliberately does not decide, so nothing about it can be judged.
pub fn is_structure_void(name: &str) -> bool {
    qualify(name) == "minecraft:structure_void"
}

/// How wet a written block is.
pub fn wetness(name: &str, properties: &BTreeMap<String, String>) -> Wetness {
    if is_fluid(name) {
        return match properties.get("level").map(String::as_str) {
            None | Some("0") => Wetness::Source,
            Some(other) => Wetness::Flowing(other.parse().unwrap_or(u8::MAX)),
        };
    }
    if properties.get("waterlogged").map(String::as_str) == Some("true") {
        return Wetness::Held;
    }
    Wetness::Dry
}

/// **True when this block would hold a fluid back.**
///
/// Everything does except air and fluid itself — including a block written
/// `waterlogged=false`, which is the measured fact this rule turns on, and a
/// block written `waterlogged=true`, which is already full and spreads nothing.
pub fn holds_fluid(name: &str, properties: &BTreeMap<String, String>) -> bool {
    let _ = properties;
    !is_air(name) && !is_fluid(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn a_source_is_wet_and_a_flow_is_named_by_its_level() {
        assert_eq!(
            wetness("minecraft:water", &props(&[("level", "0")])),
            Wetness::Source
        );
        assert_eq!(
            wetness("minecraft:water", &props(&[("level", "3")])),
            Wetness::Flowing(3)
        );
        assert_eq!(
            wetness("minecraft:lava", &props(&[("level", "0")])),
            Wetness::Source
        );
    }

    #[test]
    fn a_waterlogged_block_is_wet_but_is_not_a_body_that_runs() {
        assert_eq!(
            wetness(
                "minecraft:oak_stairs",
                &props(&[("waterlogged", "true"), ("facing", "north")])
            ),
            Wetness::Held
        );
        assert_eq!(
            wetness("minecraft:oak_stairs", &props(&[("waterlogged", "false")])),
            Wetness::Dry
        );
    }

    #[test]
    fn everything_but_air_and_fluid_holds_a_body_back() {
        // The measured rule (`tools/spike-block-settling`): spreading water does
        // not fill a block written dry, whatever the block could hold.
        assert!(holds_fluid(
            "minecraft:iron_bars",
            &props(&[("waterlogged", "false")])
        ));
        assert!(holds_fluid(
            "minecraft:oak_stairs",
            &props(&[("waterlogged", "false")])
        ));
        assert!(holds_fluid(
            "minecraft:iron_bars",
            &props(&[("waterlogged", "true")])
        ));
        assert!(holds_fluid("minecraft:stone", &props(&[])));
        assert!(!holds_fluid("minecraft:air", &props(&[])));
        assert!(!holds_fluid("minecraft:cave_air", &props(&[])));
        assert!(!holds_fluid("minecraft:water", &props(&[("level", "0")])));
        assert!(!holds_fluid("minecraft:lava", &props(&[("level", "0")])));
    }
}
