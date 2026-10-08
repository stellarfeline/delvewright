use super::*;

/// spec-0095 PackTest: the stand-in a `present` cutscene places for a player is
/// that player — the real [`crate::compiler::standin::STANDIN_FN`], run as the
/// template's own dummy, leaves a mannequin on the dummy's position wearing the
/// dummy's own profile (its `id` is the dummy's UUID), facing the dummy's yaw,
/// dressed in a copy of what the dummy wears and holds, and with no head left
/// over from the profile step where the dummy wears none. Emits nothing for a
/// campaign with no `present` cutscene.
pub(super) fn emit_standin_packtest(plan: &Plan, out: &mut BuildOutput) {
    use crate::compiler::standin::{STANDIN_ENTITY, STANDIN_FN};
    if !cutscene_parties(plan)
        .iter()
        .any(|(_, p)| *p == delvewright_dsl::CutsceneParty::Present)
    {
        return;
    }
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let mine = format!("@e[type={STANDIN_ENTITY},tag=dw_standin_new,distance=..0.01]");
    let one = format!("@n[type={STANDIN_ENTITY},tag=dw_standin_new,distance=..0.01]");
    let mut b = packtest_header(&format!(
        "{title}: a cutscene's stand-in wears its player's own profile, facing and gear"
    ));
    b.push(format!("function {ns}:setup"));
    b.push("execute at @s run tp @s ~ ~ ~ 90 0".to_string());
    b.push("item replace entity @s armor.chest with minecraft:iron_chestplate".to_string());
    b.push("item replace entity @s weapon.mainhand with minecraft:stick".to_string());
    b.push("item replace entity @s armor.head with minecraft:air".to_string());
    b.push(format!("execute at @s run function {ns}:{STANDIN_FN}"));
    // 1. one body, where the dummy stands
    b.push(format!(
        "execute at @s store success score #sti_body dw.sys if entity {mine}"
    ));
    b.push("assert score #sti_body dw.sys matches 1".to_string());
    // 2. the dummy's own profile: the stand-in's profile id IS the dummy's UUID
    b.push("data remove storage dw:sti id".to_string());
    b.push(format!(
        "execute at @s run data modify storage dw:sti id set from entity {one} profile.id"
    ));
    b.push(
        "execute store success score #sti_other dw.sys run data modify storage dw:sti id set from entity @s UUID"
            .to_string(),
    );
    b.push("assert score #sti_other dw.sys matches 0".to_string());
    // 3. the dummy's facing
    b.push(format!(
        "execute at @s store result score #sti_yaw dw.sys run data get entity {one} Rotation[0] 100"
    ));
    b.push("assert score #sti_yaw dw.sys matches 9000".to_string());
    // 4. a copy of what the dummy wears and holds
    b.push(format!(
        "execute at @s store success score #sti_gear dw.sys if entity @e[type={STANDIN_ENTITY},tag=dw_standin_new,distance=..0.01,nbt={{equipment:{{chest:{{id:\"minecraft:iron_chestplate\"}},mainhand:{{id:\"minecraft:stick\"}}}}}}]"
    ));
    b.push("assert score #sti_gear dw.sys matches 1".to_string());
    // 5. the profile head is gone where the dummy wears no helmet
    b.push(format!(
        "execute at @s store success score #sti_head dw.sys if entity @e[type={STANDIN_ENTITY},tag=dw_standin_new,distance=..0.01,nbt={{equipment:{{head:{{}}}}}}]"
    ));
    b.push("assert score #sti_head dw.sys matches 0".to_string());
    b.push(format!("execute at @s run kill {mine}"));
    b.push("item replace entity @s armor.chest with minecraft:air".to_string());
    b.push("item replace entity @s weapon.mainhand with minecraft:air".to_string());
    out.insert(
        format!("packtest-datapack/data/{ns}/test/standin.mcfunction"),
        lines(&b).into_bytes(),
    );
}
