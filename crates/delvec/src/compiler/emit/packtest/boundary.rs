use super::*;

/// v0.6 night-vision PackTest: a dummy standing inside a `mitigation:
/// "night-vision"` area actually holds `minecraft:night_vision` after one clock
/// tick, and a dummy far outside the area does not.
///
/// This is the gametest that makes the mitigation un-fakeable end-to-end: the
/// `DW0210` gate keys on the declaration, and this asserts the declaration really
/// puts the effect on a player in the world. Emits nothing for a campaign that
/// declares no mitigation.
pub(super) fn emit_night_vision_packtest(plan: &Plan, out: &mut BuildOutput) {
    let Some(area) = plan.areas.iter().find(|a| {
        plan.campaign
            .world
            .content
            .areas
            .iter()
            .find(|d| d.id.as_str() == a.area_id)
            .is_some_and(crate::compiler::light::area_night_vision)
    }) else {
        return;
    };
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let (min, max) = area.bounds();
    let mid = [
        (min[0] + max[0]) / 2,
        (min[1] + max[1]) / 2,
        (min[2] + max[2]) / 2,
    ];
    let mut b = packtest_header(&format!(
        "{title}: the declared night-vision mitigation really reaches a player in the area"
    ));
    b.push("effect clear @s minecraft:night_vision".to_string());
    // Inside the declared bounds: one tick of the real clock must grant the effect.
    b.push(format!("tp @s {} {} {}", mid[0], mid[1], mid[2]));
    b.push(format!("function {ns}:night_vision_tick"));
    b.push(
        "execute store success score #nv_nvis dw.sys run effect clear @s minecraft:night_vision"
            .to_string(),
    );
    b.push("assert score #nv_nvis dw.sys matches 1".to_string());
    // Far outside: the same clock tick must NOT grant it (the selector is scoped).
    b.push(format!("tp @s {} {} {}", max[0] + 1000, mid[1], mid[2]));
    b.push(format!("function {ns}:night_vision_tick"));
    b.push(
        "execute store success score #nv_nvis dw.sys run effect clear @s minecraft:night_vision"
            .to_string(),
    );
    b.push("assert score #nv_nvis dw.sys matches 0".to_string());
    out.insert(
        format!("packtest-datapack/data/{ns}/test/v06_night_vision.mcfunction"),
        lines(&b).into_bytes(),
    );
}

pub(super) fn emit_boundary_packtest(plan: &Plan, out: &mut BuildOutput) {
    let Some(region) = playable_region(plan).filter(|_| boundary_returns(plan)) else {
        return;
    };
    let Some(spawn) = campaign_spawn(plan) else {
        return;
    };
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    // setup_finish (which writes `dw:cp`) is placement-gated and cannot run in a
    // bare PackTest, so seed the same spawn-cell value the real init would write.
    let seed_cp = format!(
        "data modify storage dw:cp pos set value [{}, {}, {}]",
        spawn[0], spawn[1], spawn[2]
    );

    // Return: a dummy far outside the region (x well past the inflated max) is
    // teleported back to the checkpoint's x within one clock tick.
    let out_x = region.max[0] + 1000;
    let mut b = packtest_header(&format!(
        "{title}: a player outside the playable region returns to the last checkpoint"
    ));
    b.push(seed_cp.clone());
    b.push(format!("tp @s {out_x} {} {}", spawn[1], spawn[2]));
    b.push(format!("function {ns}:boundary_tick"));
    b.push(
        "execute store result score #bx_bret dw.sys run data get entity @s Pos[0] 1".to_string(),
    );
    b.push(format!("assert score #bx_bret dw.sys matches {}", spawn[0]));
    out.insert(
        format!("packtest-datapack/data/{ns}/test/v06_boundary_return.mcfunction"),
        lines(&b).into_bytes(),
    );

    // Inside: a dummy at an interior cell distinct from the checkpoint is untouched.
    let in_x = spawn[0] + 5;
    let mut b = packtest_header(&format!(
        "{title}: a player inside the playable region is never moved"
    ));
    b.push(seed_cp.clone());
    b.push(format!("tp @s {in_x} {} {}", spawn[1], spawn[2]));
    // Precondition: the interior cell really is inside the region (else the geometry
    // is too small — fail informatively rather than silently pass).
    b.push(
        "execute store result score #px_bins dw.sys run data get entity @s Pos[0] 1".to_string(),
    );
    b.push(format!(
        "assert score #px_bins dw.sys matches {}..{}",
        region.min[0], region.max[0]
    ));
    b.push(format!("function {ns}:boundary_tick"));
    b.push(
        "execute store result score #bx_bins dw.sys run data get entity @s Pos[0] 1".to_string(),
    );
    b.push(format!("assert score #bx_bins dw.sys matches {in_x}"));
    out.insert(
        format!("packtest-datapack/data/{ns}/test/v06_boundary_inside.mcfunction"),
        lines(&b).into_bytes(),
    );

    // Exempt (spec-0092 §10): a player outside the region who is watching a
    // cutscene (`dw_cutscene`) or flying out of the body with the creator's free
    // camera (`dw_free`) is never moved — the camera is not the party, and a
    // creator tool is not fought by the player bound. Each tag is asserted on
    // its own, so a selector that forgot either reds here.
    for (n, tag) in ["dw_cutscene", crate::compiler::creator::FREE_TAG]
        .iter()
        .enumerate()
    {
        let mut b = packtest_header(&format!(
            "{title}: a player outside the region carrying `{tag}` is never returned"
        ));
        b.push(seed_cp.clone());
        b.push(format!("tp @s {out_x} {} {}", spawn[1], spawn[2]));
        b.push(format!("tag @s add {tag}"));
        b.push(format!("function {ns}:boundary_tick"));
        b.push(format!("tag @s remove {tag}"));
        b.push(format!(
            "execute store result score #bx_bex{n} dw.sys run data get entity @s Pos[0] 1"
        ));
        b.push(format!("assert score #bx_bex{n} dw.sys matches {out_x}"));
        out.insert(
            format!(
                "packtest-datapack/data/{ns}/test/boundary_exempt_{}.mcfunction",
                tag.trim_start_matches("dw_")
            ),
            lines(&b).into_bytes(),
        );
    }
}
