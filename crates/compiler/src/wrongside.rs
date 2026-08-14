//! Which side of a sealed shortcut door a player is standing on (task #50).
//!
//! **No `shortcuts[]` field names this.** Earlier drafts of this module's docs
//! promised one — `on_wrong_side` in one paragraph, `sealed_hint` in another, and
//! neither ever existed in the schema — which is the second bespoke field
//! CLAUDE.md names as the defect rather than the fix, twice, under two names.
//! Those drafts also claimed the wording "defaults" while no code baked one, so
//! the door in fact said nothing: the docs, the code and the player each believed
//! something different. A shortcut door's wrong-side answer is an ordinary
//! `EnvTrigger{on: use, audience: presser}` carrying a `narrate{style: actionbar}`,
//! anchored on the `gate`, exactly like every other pressable object's — and from
//! `dsl_version` 0.11.0 the campaign must write it or the build refuses
//! (`DW0429`). What lives here is only the geometry: WHERE that trigger's body
//! stands, and therefore from which side it can be pressed.
//!
//! ## The gap this closes
//!
//! `shortcuts[]` is the souls loop-back: sealed from world-load, opened
//! permanently from the far side. It is therefore the surface a party presses
//! most — walking up to a barred door you have not yet earned and pushing on it
//! *is* the idiom — and until this module it was the one gate in the engine that
//! could not answer at all. Seal answers came exclusively from `close-gate`
//! (`plan::collect_seal_hints`), and `DW0372` structurally forbids a `close-gate`
//! on a shortcut gate.
//!
//! The workaround an author would reach for — the island boulder's shape, a
//! repeatable click trigger at the gate anchor — compiles with **zero
//! diagnostics** and ships something worse than silence. Measured on the
//! `souls-shortcut` fixture (gate slab `[4,65,6]..[5,67,6]`, unlock `[5,65,8]`):
//!
//! ```text
//! summon minecraft:interaction 4.5 65.0 6.5 {width:1.0f,height:2.0f,…,Tags:["dw_trig_door_wont_open"]}
//! ```
//!
//! One entity for a six-cell door, and its box spans `z 6..7` — coincident with
//! the solid block on the near side (so it loses the client's ray-pick, the exact
//! defect [`crate::emit::SEAL_MARGIN`] exists to fix) and protruding into the air
//! on the **far** side. The only authored answer available today is pressable
//! only from the side the door opens from.
//!
//! ## Two layers, the same two the island's boulder answers with
//!
//! The boulder answers a **right-click** with the compiler's press answer (it is
//! a `close-gate` target the campaign never answers itself, so the wording is the
//! `delvewright.ui.gate.sealed` chrome), and a **left-click** with
//! `trigger/boulder-wont-move` — the author's own thirty words plus
//! `minecraft:block.deepslate.hit`. A shortcut door needs both, and had neither,
//! because it had no interaction body for either click to reach.
//!
//! This module supplies the body, and with it both layers:
//!
//! * the right-click half is a press answer — the campaign's own `use` trigger at
//!   the `gate`, on the presser's actionbar, re-armed every press. From
//!   `dsl_version` 0.11.0 the campaign **must** write it: the compiler does not
//!   word a sealed thing for its author, and one with nothing to say is `DW0429`
//!   — the same rule a `close-gate` wall is held to at that version, because they
//!   are two objects of one class ([`crate::plan::SilencePolicy`]);
//! * the left-click half is the author's: an ordinary `strike` trigger anchored
//!   on the `gate` now **rides these hitboxes** instead of summoning its own dead
//!   co-located box, so it can carry whatever prose and sound the campaign wants.
//!
//! Neither half is machinery this module owns. Both are the general click verb,
//! reaching a body it could not reach before.
//!
//! `minecraft:player_interacted_with_entity` runs its reward function **as the
//! player who right-clicked** — the same primitive every NPC dialogue, `interact`
//! objective and bonfire rest already runs on. That is what makes the side
//! knowable at all: a trigger is dispatched from the tick under the server
//! command source (`trig_<id>` is [`crate::emit::Audience`]`::Party`) and never
//! knows who pressed it.
//!
//! ## Why the side is a position test and not a face test
//!
//! Vanilla carries no face data on that criterion, and the answer hitboxes are
//! deliberately symmetric (one block plus [`crate::emit::SEAL_MARGIN`] on every
//! side, so every face of the door is pressable). Placing the hitbox on the near
//! face alone and relying on the door to occlude the far side does **not** hold:
//! a gate block is whatever the prefab metadata declares, and `minecraft:iron_bars`
//! — the `souls-shortcut` fixture's own gate block — is not a full cube. A far-side
//! player standing at the door can raycast between the bars.
//!
//! So the side is asked of the **player**, not of the click: the gate slab has a
//! thin axis, the `unlock` cell lies on one side of it, and the sealed side is the
//! other one. That is a fact about the assembled world the compiler already has,
//! resolved deterministically at plan time (ADR-0006) — never author folklore.
//! When it is not decidable the compiler withholds rather than invents
//! (`DW0425`), because an answer placed on a guessed side would fire exactly where
//! the door DOES open.

use delvewright_dsl::DwCode;

/// `DW0425`: the compiler cannot decide which side of a shortcut's gate is the
/// sealed one.
///
/// Every shortcut door is given a clickable body, and every body has to stand on
/// a side, so this binds to **every** shortcut in the campaign — there is no
/// declaration to opt into and none to forget.
pub const DW_SHORTCUT_SIDE_UNDECIDABLE: DwCode = DwCode::every_version("DW0425");

/// The sealed side of a shortcut gate — expressed as the cells a body must stand
/// in for the door to be pressable from that side and no other.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SealedSide {
    /// The gate slab's own inclusive cell bounds.
    pub gate_lo: [i32; 3],
    /// The gate slab's own inclusive cell bounds.
    pub gate_hi: [i32; 3],
    /// Which axis the door is thin on (0 = x, 1 = y, 2 = z).
    pub axis: usize,
    /// `-1` if the sealed side is the low side of `axis`, `+1` if the high side.
    pub dir: i32,
}

impl SealedSide {
    /// The cells a press must come through: the gate's face projected one cell
    /// **out of the door, into the open air on the sealed side**, in ascending
    /// `(x, y, z)` order.
    ///
    /// This is the whole side mechanism, and it needs no player test at all.
    /// Sidedness is reachability: a body standing in the open air in front of the
    /// bars is hit by a near-side ray before the block behind it, while a far-side
    /// ray hits the door first and stops — vanilla bounds its entity raycast by
    /// the block hit distance. A trigger anchored on the gate rides these, so an
    /// author's `use`/`strike` answer fires only where it is true.
    ///
    /// A body inside the slab — which is where a point-shaped trigger body lands
    /// today — is flush with or interior to the block on every face and therefore
    /// reachable from nowhere at all (see the module docs).
    pub fn approach_cells(&self) -> Vec<[i32; 3]> {
        let (lo, hi) = (self.gate_lo, self.gate_hi);
        let face = if self.dir < 0 {
            lo[self.axis] - 1
        } else {
            hi[self.axis] + 1
        };
        let mut out = Vec::new();
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    let mut c = [x, y, z];
                    c[self.axis] = face;
                    out.push(c);
                }
            }
        }
        out.sort();
        out.dedup();
        out
    }
}

/// Derive the sealed side of `gate` given where the shortcut's `unlock` stands.
///
/// `None` — `DW0425` — when the derivation is not honest:
///
/// * the gate region has no **unique** thinnest axis (a cube-shaped region is not
///   a doorway, and has no "sides" to be on);
/// * the `unlock` cell does not lie clear of the gate's span on that axis (it is
///   around a corner, or level with the doorway), so which side it is on is not a
///   fact the geometry states.
pub fn derive(gate: ([i32; 3], [i32; 3]), unlock: [i32; 3]) -> Option<SealedSide> {
    let (a, b) = gate;
    let lo = [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])];
    let hi = [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])];
    let extent = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];

    // The thin axis must be a strict minimum: a tie means the region is not a
    // slab and "which side" is not a question its shape answers.
    let axis = (0..3).min_by_key(|&i| extent[i])?;
    if (0..3).any(|i| i != axis && extent[i] == extent[axis]) {
        return None;
    }

    // Which side does the unlock stand on? It must be clear of the slab's own
    // span, else it is level with the doorway and names no side.
    let near_is_below = if unlock[axis] > hi[axis] {
        true // unlock is above ⇒ the sealed side is below
    } else if unlock[axis] < lo[axis] {
        false
    } else {
        return None;
    };

    Some(SealedSide {
        gate_lo: lo,
        gate_hi: hi,
        axis,
        dir: if near_is_below { -1 } else { 1 },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `souls-shortcut` fixture's own geometry: a 2x3x1 doorway slab thin on
    /// **z**, with the unlock two blocks beyond it. The sealed side is low-z, so
    /// the door is pressable from the open air at z = 5 and from nowhere else.
    #[test]
    fn the_fixture_doorway_resolves_its_sealed_side() {
        let s = derive(([4, 65, 6], [5, 67, 6]), [5, 65, 8]).expect("a doorway has sides");
        assert_eq!(s.axis, 2, "the slab is thin on z");
        assert_eq!(
            s.dir, -1,
            "the unlock is at z=8, so the sealed side is low-z"
        );
        let cells = s.approach_cells();
        assert_eq!(cells.len(), 6, "one per doorway cell: {cells:?}");
        assert!(
            cells.iter().all(|c| c[2] == 5),
            "every body stands in the open air in front of the bars: {cells:?}"
        );
        // …and never inside the slab, which is the unreachable case.
        assert!(
            cells.iter().all(|c| c[2] != 6),
            "no body inside the door: {cells:?}"
        );
    }

    /// The mirror case: an unlock on the LOW side puts the approach high. Nothing
    /// in the derivation privileges a direction.
    #[test]
    fn the_sealed_side_follows_the_unlock() {
        let s = derive(([4, 65, 6], [5, 67, 6]), [5, 65, 3]).expect("a doorway has sides");
        assert_eq!(s.dir, 1);
        assert!(s.approach_cells().iter().all(|c| c[2] == 7));
    }

    /// `DW0425`, cause 1: a cube has no thin axis, so it has no sides.
    #[test]
    fn a_cube_shaped_gate_has_no_sides() {
        assert_eq!(derive(([4, 65, 6], [5, 66, 7]), [5, 65, 12]), None);
    }

    /// `DW0425`, cause 2: an unlock level with the doorway names no side.
    #[test]
    fn an_unlock_level_with_the_doorway_names_no_side() {
        assert_eq!(derive(([4, 65, 6], [5, 67, 6]), [9, 65, 6]), None);
    }

    /// Pure function of its two inputs — no wall clock, no iteration order,
    /// no ambient state (ADR-0006).
    #[test]
    fn the_derivation_is_deterministic() {
        let once = derive(([4, 65, 6], [5, 67, 6]), [5, 65, 8]);
        for _ in 0..64 {
            assert_eq!(derive(([4, 65, 6], [5, 67, 6]), [5, 65, 8]), once);
        }
    }
}
