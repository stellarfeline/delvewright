use super::*;

/// The vehicle templates (spec-0135 §7), synchronous, per vehicle.
///
/// `vehicle_<s>_seated` runs the seat function `setup_finish` and
/// `seat-vehicle` run, and asserts exactly one body carries the vehicle's tag
/// and stands within two blocks of the mark — a stripped summon or a removal
/// that missed the old body reds it. `vehicle_<s>_reseats` seats it, moves the
/// body outside `bounds`, runs the re-seat once, and asserts the one body is
/// back at the mark and the re-seat re-armed itself (one schedule cleared) —
/// a stripped bounds test or a stripped `schedule` reds it. Calling the re-seat
/// stands for the 100 ticks its schedule steps, so the template asserts the
/// function and not the clock.
///
/// A vehicle that stays where its rider left it (`left: where-left`) has
/// `vehicle_<s>_stays` in place of `_reseats`: seated, its body moved off its
/// mark and — for a mount — given its AI back as boarding gives it, the
/// re-seat run once leaves the one body where it was moved, a mount without
/// its AI again, and re-arms itself. A re-seat that brings it home, or one
/// that leaves a mount with its AI, reds it.
///
/// That `ride … mount` seats a body and `ride … dismount` stands it beside the
/// vehicle is vanilla, held by `docs/reference/vehicles-animals-and-targets.md`
/// §4–§6, and has no template.
pub(super) fn emit_vehicle_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    for v in &plan.vehicles {
        let s = &v.safe;
        let tag = v.tag();
        let c = crate::compiler::nav::cell_center(v.seat);
        let at = format!(
            "x={},y={},z={},distance=..2",
            fmt_f64(c[0]),
            fmt_f64(c[1]),
            fmt_f64(c[2])
        );
        let count = format!("#vh_{s} dw.sys");
        let one_at_mark = [
            format!("execute store result score {count} if entity @e[tag={tag}]"),
            format!("assert score {count} matches 1"),
            format!("assert entity @e[tag={tag},{at}]"),
        ];
        let mut t = packtest_header(&format!(
            "{title}: vehicle `{}` is seated at its mark, one body (spec-0135)",
            v.id
        ));
        t.push(format!("function {ns}:vehicle_seat_{s}"));
        t.extend(one_at_mark.iter().cloned());
        t.push(format!("scoreboard players reset {count}"));
        out.insert(
            format!("packtest-datapack/data/{ns}/test/vehicle_{s}_seated.mcfunction"),
            lines(&t).into_bytes(),
        );
        let (lo, _) = v.bounds;
        let cleared = format!("#vhc_{s} dw.sys");
        if v.left == delvewright_dsl::VehicleLeft::WhereLeft {
            let left = [lo[0] - 3, v.seat[1], v.seat[2]];
            let lc = crate::compiler::nav::cell_center(left);
            let there = format!(
                "x={},y={},z={},distance=..2",
                fmt_f64(lc[0]),
                fmt_f64(lc[1]),
                fmt_f64(lc[2])
            );
            let mut r = packtest_header(&format!(
                "{title}: vehicle `{}` left off its mark stays where it was left (spec-0135)",
                v.id
            ));
            r.push(format!("function {ns}:vehicle_seat_{s}"));
            r.push(format!(
                "tp @e[tag={tag}] {} {} {}",
                fmt_f64(lc[0]),
                left[1],
                fmt_f64(lc[2])
            ));
            let mount = v.kind == delvewright_dsl::VehicleKind::Mount;
            if mount {
                r.push(format!(
                    "data merge entity @e[tag={tag},limit=1] {{NoAI:0b}}"
                ));
            }
            r.push(format!("schedule clear {ns}:vehicle_reseat_{s}"));
            r.push(format!("function {ns}:vehicle_reseat_{s}"));
            r.push(format!(
                "execute store result score {count} if entity @e[tag={tag}]"
            ));
            r.push(format!("assert score {count} matches 1"));
            r.push(format!("assert entity @e[tag={tag},{there}]"));
            r.push(format!("assert not entity @e[tag={tag},{at}]"));
            if mount {
                r.push(format!("assert entity @e[tag={tag},nbt={{NoAI:1b}}]"));
            }
            r.push(format!(
                "execute store result score {cleared} run schedule clear {ns}:vehicle_reseat_{s}"
            ));
            r.push(format!("assert score {cleared} matches 1"));
            r.push(format!(
                "schedule function {ns}:vehicle_reseat_{s} 100t replace"
            ));
            r.push(format!("scoreboard players reset {count}"));
            r.push(format!("scoreboard players reset {cleared}"));
            out.insert(
                format!("packtest-datapack/data/{ns}/test/vehicle_{s}_stays.mcfunction"),
                lines(&r).into_bytes(),
            );
            continue;
        }
        let mut r = packtest_header(&format!(
            "{title}: vehicle `{}` left outside its bounds is seated again (spec-0135)",
            v.id
        ));
        r.push(format!("function {ns}:vehicle_seat_{s}"));
        r.push(format!(
            "tp @e[tag={tag}] {} {} {}",
            lo[0] - 3,
            v.seat[1],
            v.seat[2]
        ));
        r.push(format!("assert not entity @e[tag={tag},{at}]"));
        r.push(format!("schedule clear {ns}:vehicle_reseat_{s}"));
        r.push(format!("function {ns}:vehicle_reseat_{s}"));
        r.extend(one_at_mark.iter().cloned());
        r.push(format!(
            "execute store result score {cleared} run schedule clear {ns}:vehicle_reseat_{s}"
        ));
        r.push(format!("assert score {cleared} matches 1"));
        r.push(format!(
            "schedule function {ns}:vehicle_reseat_{s} 100t replace"
        ));
        r.push(format!("scoreboard players reset {count}"));
        r.push(format!("scoreboard players reset {cleared}"));
        out.insert(
            format!("packtest-datapack/data/{ns}/test/vehicle_{s}_reseats.mcfunction"),
            lines(&r).into_bytes(),
        );
    }
}
