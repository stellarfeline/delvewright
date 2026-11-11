//! Vehicles (spec-0135 §2, §3): the seat, the re-seat, the removal, and what
//! boarding one does.

use super::*;
use crate::compiler::nav::vehicle::VehiclePlan;
use delvewright_dsl::{EquipSlot, VehicleKind};

/// Ticks between two re-seat checks: a replace-mode `schedule`, so a vehicle
/// costs nothing per tick (spec-0135 §3).
pub(crate) const RESEAT_TICKS: u32 = 100;

/// The tag a re-seat marks a riderless body away from its mark with, for the
/// one function call that follows.
const RESEAT_TAG: &str = "dw_vehicle_reseat";

/// How far (blocks) a body may stand from its mark and still be at it.
const AT_MARK: &str = "1.5";

/// The advancement a boarding answers through, and its reward function.
pub(super) const BOARD: &str = "vehicle_board";

/// The id of the modifier that holds a riderless mount still.
fn wait_modifier(ns: &str) -> String {
    format!("{ns}:vehicle_wait")
}

/// Where a vehicle's body is summoned: the mark's cell centre, a boat on the
/// water's top face, where it floats.
fn summon_point(v: &VehiclePlan) -> [f64; 3] {
    let p = crate::compiler::nav::cell_center(v.seat);
    if v.kind == VehicleKind::Boat {
        [p[0], p[1] + 1.0, p[2]]
    } else {
        p
    }
}

/// **A riderless mount waits** (the record §11): its `movement_speed` is
/// multiplied to nothing, so its own AI walks it nowhere; boarding takes the
/// modifier off ([`emit_boarding`]).
fn wait_line(ns: &str, v: &VehiclePlan) -> Option<String> {
    (v.kind == VehicleKind::Mount).then(|| {
        format!(
            "attribute @e[tag={},limit=1] minecraft:movement_speed modifier add {} -1 add_multiplied_total",
            v.tag(),
            wait_modifier(ns)
        )
    })
}

/// The summon of one vehicle's body.
pub(super) fn vehicle_summon(plan: &Plan, v: &VehiclePlan) -> String {
    let decl = &plan.campaign.quests.content.vehicles[v.index];
    let p = summon_point(v);
    let y = p[1];
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

/// What a `seat-vehicle` runs: the seat function, and the re-seat chain
/// armed — the one `setup_finish` runs, so a seat after a despawn brings the
/// re-seat back with the body.
pub(super) fn vehicle_seat_lines(ns: &str, v: &VehiclePlan) -> Vec<String> {
    vec![
        format!("function {ns}:vehicle_seat_{}", v.safe),
        format!(
            "schedule function {ns}:vehicle_reseat_{} {RESEAT_TICKS}t replace",
            v.safe
        ),
    ]
}

/// What a `despawn-vehicle` runs: the removal, and the re-seat chain cleared —
/// a re-seat that found the body missing would put back what the story took
/// away.
pub(super) fn vehicle_despawn_lines(ns: &str, v: &VehiclePlan) -> Vec<String> {
    let mut out = vehicle_removal_lines(ns, v);
    out.push(format!("schedule clear {ns}:vehicle_reseat_{}", v.safe));
    out
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
        .flat_map(|v| vehicle_seat_lines(ns, v))
        .collect()
}

/// Two functions per vehicle: `vehicle_seat_<s>` (any standing body removed
/// unseen, then the summon, a mount held still) and `vehicle_reseat_<s>` (a
/// body missing, or a riderless body away from its mark, seated again; the
/// chain re-armed) — so a vehicle nobody rides is at its mark, which is where
/// every proof of a ride starts.
pub(super) fn emit_vehicle_functions(plan: &Plan) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut fns = Vec::new();
    for v in &plan.vehicles {
        let tag = v.tag();
        let mut seat = vehicle_removal_lines(ns, v);
        seat.push(vehicle_summon(plan, v));
        seat.extend(wait_line(ns, v));
        fns.push((format!("vehicle_seat_{}", v.safe), lines(&seat)));
        let p = summon_point(v);
        let reseat = vec![
            format!(
                "execute as @e[tag={tag}] unless entity @s[x={},y={},z={},distance=..{AT_MARK}] run tag @s add {RESEAT_TAG}",
                fmt_f64(p[0]),
                fmt_f64(p[1]),
                fmt_f64(p[2]),
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
    if let Some(board) = boarding_lines(plan) {
        fns.push((BOARD.to_string(), lines(&board)));
    }
    fns
}

/// **What boarding a vehicle does** (the record §11), run as the player who
/// boarded, by the `minecraft:started_riding` advancement: a cart is pushed
/// the way it faces — the rails take it from there — and a mount is given its
/// speed back. `None` for a campaign with no cart and no mount, which emits
/// neither the function nor the advancement.
fn boarding_lines(plan: &Plan) -> Option<Vec<String>> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for v in &plan.vehicles {
        match v.kind {
            VehicleKind::Minecart => {
                let push = crate::compiler::rail::CART_PUSH;
                out.push(format!(
                    "execute on vehicle run data merge entity @s[tag={}] {{Motion:[{}d,0.0d,{}d]}}",
                    v.tag(),
                    fmt_f64(f64::from(v.facing[0]) * push),
                    fmt_f64(f64::from(v.facing[1]) * push),
                ));
            }
            VehicleKind::Mount => out.push(format!(
                "execute on vehicle run attribute @s[tag={}] minecraft:movement_speed modifier remove {}",
                v.tag(),
                wait_modifier(ns)
            )),
            VehicleKind::Boat => {}
        }
    }
    if out.is_empty() {
        return None;
    }
    out.push(format!("advancement revoke @s only {ns}:{BOARD}"));
    Some(out)
}

/// The advancement [`boarding_lines`] answers through, when the campaign has
/// a cart or a mount.
pub(super) fn boarding_advancement(plan: &Plan) -> Option<(String, Value)> {
    boarding_lines(plan)?;
    Some((
        BOARD.to_string(),
        json!({
            "criteria": { "board": { "trigger": "minecraft:started_riding" } },
            "rewards": { "function": format!("{}:{BOARD}", plan.namespace) }
        }),
    ))
}
