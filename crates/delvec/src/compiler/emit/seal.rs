//! The seal answers.

use super::*;

/// How far a seal's answer hitbox protrudes past the sealed block, on every side.
///
/// **This margin is the whole mechanism.** A `minecraft:interaction` whose box
/// exactly coincides with the block it stands in loses the client's ray-pick:
/// vanilla takes the entity only when it is *strictly* nearer the eye than the
/// block hit, and a coincident box is hit at exactly the same distance. One
/// centimetre of protrusion makes the entity strictly nearer from every approach
/// angle, so pressing any face of the seal reaches the entity — while a hundredth
/// of a block never reaches into a neighbouring cell's own affordances.
pub const SEAL_MARGIN: f64 = 0.01;

/// The seal-answer entity's box size, as the `width`/`height` NBT floats: one
/// block plus [`SEAL_MARGIN`] on each side.
pub(super) const SEAL_BOX_SIZE: &str = "1.02f";

/// Render a signed count of hundredths as a decimal coordinate: `6899` →
/// `68.99`, `-4450` → `-44.5`, `700` → `7.0`. Integer-only, so the emitted text
/// is exactly what it reads as (no binary-float rounding in the datapack).
pub(super) fn fmt_centi(v: i64) -> String {
    let sign = if v < 0 { "-" } else { "" };
    let a = v.unsigned_abs();
    let (whole, frac) = (a / 100, a % 100);
    if frac == 0 {
        format!("{sign}{whole}.0")
    } else if frac.is_multiple_of(10) {
        format!("{sign}{whole}.{}", frac / 10)
    } else {
        format!("{sign}{whole}.{frac:02}")
    }
}

/// The seal plan for a gate anchor, if the campaign ever seals it.
pub(super) fn seal_hint_for<'a>(plan: &'a Plan, anchor: &str) -> Option<&'a plan::SealHintPlan> {
    plan.seal_hints.iter().find(|s| s.anchor == anchor)
}

/// The `seal_arm_<safe>` function name: what a `close-gate` calls to give the
/// stone a voice.
pub(super) fn seal_arm_fn(safe: &str) -> String {
    format!("seal_arm_{safe}")
}

/// The `dw_trig_<id>` tags every click trigger anchored **on this gate** rides,
/// in campaign declaration order (deterministic).
///
/// The round-6 rule, one layer out: one cell, one hitbox. A `strike`/`use`
/// trigger whose `at` is the gate anchor is asking the player to hit *the gate* —
/// and once the gate is sealed the gate's own hitboxes are what a click reaches.
/// Summoning the trigger a second, co-located entity is the exact ray-pick tie
/// that made the island's boulder unshippable, so the trigger's tag rides these
/// entities and [`env_trigger_setup`] summons nothing for it. The consequence is
/// also its meaning: such a trigger is live exactly while the gate is sealed.
pub(super) fn seal_rider_tags(
    plan: &Plan,
    chrome: &delvewright_dsl::Chrome,
    anchor: &str,
) -> Vec<String> {
    plan.emitted_triggers(chrome)
        .iter()
        .filter(|t| t.on.is_click())
        .filter(|t| t.at_anchor() == Some(anchor))
        .map(|t| format!("dw_trig_{}", plan::safe_local(t.id.as_str())))
        .collect()
}

/// The `seal_arm_<safe>` functions: one `minecraft:interaction` per
/// clickable cell of each sealed region, so the wall answers a press wherever the
/// party presses it.
///
/// Only the region's **shell** is armed ([`plan::SealHintPlan::shell_cells`]) —
/// a cell buried inside the seal has no face a crosshair can reach. Each entity
/// is one block plus [`SEAL_MARGIN`], positioned so its box brackets its cell on
/// every axis; see that constant for why the margin is not cosmetic.
///
/// Empty for a campaign that never seals a gate → byte-identical output.
pub(super) fn seal_fns(plan: &Plan, chrome: &delvewright_dsl::Chrome) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for s in &plan.seal_hints {
        let mut tags = vec![format!("dw_seal_{}", s.safe)];
        tags.extend(seal_rider_tags(plan, chrome, &s.anchor));
        let tag_list = tags
            .iter()
            .map(|t| format!("\"{t}\""))
            .collect::<Vec<_>>()
            .join(",");
        let body: Vec<String> = s
            .shell_cells()
            .into_iter()
            .map(|c| {
                // Positions are built from integer hundredths, never from f64
                // arithmetic: the datapack text is part of the byte-identity
                // contract (ADR-0006) and `y - 0.01` in binary floating point is
                // not the decimal `.99` a reader (or a diff) expects.
                //
                // x/z are the cell CENTRE (the box is width-symmetric about the
                // position); y is the box's FLOOR, dropped one margin so the box
                // brackets the cell below as well as above.
                let x = fmt_centi(c[0] as i64 * 100 + 50);
                let y = fmt_centi(c[1] as i64 * 100 - 1);
                let z = fmt_centi(c[2] as i64 * 100 + 50);
                format!(
                    "summon minecraft:interaction {x} {y} {z} \
                     {{width:{SEAL_BOX_SIZE},height:{SEAL_BOX_SIZE},response:1b,Invulnerable:1b,Tags:[{FIXTURE_NBT}{tag_list}]}}"
                )
            })
            .collect();
        out.push((seal_arm_fn(&s.safe), lines(&body)));
    }
    out
}
