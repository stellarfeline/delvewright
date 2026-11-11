//! Vehicles (spec-0135 §2, §3): the seat, the re-seat and the removal.

use super::*;
use crate::compiler::nav::vehicle::VehiclePlan;
use delvewright_dsl::{EquipSlot, VehicleKind};

/// Ticks between two re-seat checks: a replace-mode `schedule`, so a vehicle
/// costs nothing per tick (spec-0135 §3).
pub(crate) const RESEAT_TICKS: u32 = 100;

/// The tag a re-seat marks a riderless body outside its bounds with, for the
/// one function call that follows.
const RESEAT_TAG: &str = "dw_vehicle_reseat";

/// The summon of one vehicle's body.
pub(super) fn vehicle_summon(plan: &Plan, v: &VehiclePlan) -> String {
    let decl = &plan.campaign.quests.content.vehicles[v.index];
    let p = crate::compiler::nav::cell_center(v.seat);
    // A boat is put on the water's top face and floats there.
    let y = if v.kind == VehicleKind::Boat {
        p[1] + 1.0
    } else {
        p[1]
    };
    let mut nbt: Vec<String> = vec!["Invulnerable:1b".to_string()];
    if v.kind == VehicleKind::Mount {
        nbt.push("PersistenceRequired:1b".to_string());
        nbt.push("Silent:1b".to_string());
        if v.tame {
            nbt.push("Tame:1b".to_string());
        }
    }
    nbt.push(format!("Rotation:[{}f,0f]", v.yaw));
    nbt.push(format!("Tags:[\"dw_vehicle\",\"{}\"]", v.tag()));
    if let Some(n) = &decl.name {
        nbt.push(format!(
            "CustomName:{},CustomNameVisible:1b",
            snbt_component(n)
        ));
    }
    if v.kind == VehicleKind::Mount {
        // The saddle is the engine's unless a piece is declared there; every
        // drop chance is zero (a ride is never loot).
        let mut items: Vec<String> = Vec::new();
        let mut chances: Vec<String> = Vec::new();
        for slot in EquipSlot::ALL {
            let piece = decl.equipment.as_ref().and_then(|e| e.filled(slot));
            let item = match (piece, slot) {
                (Some(p), _) => format!(
                    "{}:{{id:\"{}\",count:1{}}}",
                    slot.nbt(),
                    p.item(),
                    enchantment_components(p)
                ),
                (None, EquipSlot::Saddle) => {
                    format!("{}:{{id:\"minecraft:saddle\",count:1}}", slot.nbt())
                }
                (None, _) => continue,
            };
            items.push(item);
            chances.push(format!("{}:0.0f", slot.nbt()));
        }
        nbt.push(format!(
            "equipment:{{{}}},drop_chances:{{{}}}",
            items.join(","),
            chances.join(",")
        ));
        let attrs = wrap_attribute_entries(attribute_entries(decl.attributes.as_ref()));
        if let Some(a) = attrs.strip_prefix(',') {
            nbt.push(a.to_string());
        } else if !attrs.is_empty() {
            nbt.push(attrs);
        }
    }
    format!(
        "summon {} {} {} {} {{{}}}",
        v.entity,
        fmt_f64(p[0]),
        fmt_f64(y),
        fmt_f64(p[2]),
        nbt.join(",")
    )
}

/// The removal of one vehicle's bodies: a rider set down, the body leaving
/// unseen — the removal a wave's re-seat performs.
pub(super) fn vehicle_removal_lines(ns: &str, v: &VehiclePlan) -> Vec<String> {
    removal_lines(ns, &v.tag(), false, Exit::Unseen)
}

/// The vehicle a `seat-vehicle` / `despawn-vehicle` names, as planned.
pub(super) fn vehicle_named<'p>(plan: &'p Plan, id: &str) -> Option<&'p VehiclePlan> {
    plan.vehicles.iter().find(|v| v.id == id)
}

/// `setup_finish` lines: each vehicle seated, and its re-seat chain started.
/// Empty for a campaign that declares none, so its `setup_finish` is
/// byte-identical.
pub(super) fn vehicle_setup(plan: &Plan) -> Vec<String> {
    let ns = &plan.namespace;
    plan.vehicles
        .iter()
        .flat_map(|v| {
            [
                format!("function {ns}:vehicle_seat_{}", v.safe),
                format!(
                    "schedule function {ns}:vehicle_reseat_{} {RESEAT_TICKS}t replace",
                    v.safe
                ),
            ]
        })
        .collect()
}

/// Two functions per vehicle: `vehicle_seat_<s>` (any standing body removed
/// unseen, then the summon) and `vehicle_reseat_<s>` (a body missing, or a
/// riderless body outside `bounds`, seated again; the chain re-armed).
pub(super) fn emit_vehicle_functions(plan: &Plan) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut fns = Vec::new();
    for v in &plan.vehicles {
        let tag = v.tag();
        let mut seat = vehicle_removal_lines(ns, v);
        seat.push(vehicle_summon(plan, v));
        fns.push((format!("vehicle_seat_{}", v.safe), lines(&seat)));
        let (lo, hi) = v.bounds;
        let reseat = vec![
            format!(
                "execute as @e[tag={tag}] unless entity @s[{}] run tag @s add {RESEAT_TAG}",
                box_selector_args(lo, hi)
            ),
            format!(
                "execute as @e[tag={tag},tag={RESEAT_TAG}] on passengers on vehicle run tag @s remove {RESEAT_TAG}"
            ),
            format!(
                "execute if entity @e[tag={tag},tag={RESEAT_TAG}] run function {ns}:vehicle_seat_{}",
                v.safe
            ),
            format!(
                "execute unless entity @e[tag={tag}] run function {ns}:vehicle_seat_{}",
                v.safe
            ),
            format!("tag @e[tag={RESEAT_TAG}] remove {RESEAT_TAG}"),
            format!(
                "schedule function {ns}:vehicle_reseat_{} {RESEAT_TICKS}t replace",
                v.safe
            ),
        ];
        fns.push((format!("vehicle_reseat_{}", v.safe), lines(&reseat)));
    }
    fns
}
