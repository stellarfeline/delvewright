use super::*;

/// The name of the second player a cutscene-return template spawns.
fn second_player(n: usize) -> String {
    format!("dwcsr{n}")
}

/// **A cutscene moves no player, with two players present.** One template
/// `cutscene_return_<n>` per cutscene bracket whose `then` is empty (a `then`
/// lowers the author's own effects into the end, which a template must not
/// fire): two players stand four blocks apart facing opposite ways, the real
/// `cs_<bare>` starts the scene, its driver is cleared, each body is carried
/// somewhere else (what spectating the camera does to it), and the real
/// `cs_end_<bare>` must put each player back on the cell *they* stood on, facing
/// the way *they* faced. A return that gathers the party onto one place reds on
/// the second player; a return that moves nobody reds on both.
///
/// The start re-modes every player, so the build makes the template solo
/// (`crate::compiler::solo`): no other test's dummy is on the server.
pub(super) fn emit_cutscene_return_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut n = 0usize;
    for (eff, _) in crate::compiler::camera::cutscene_units(plan.campaign) {
        let Some(start) = super::super::cutscene::cutscene_effect_fn(eff) else {
            continue;
        };
        if !seen.insert(start.clone()) || !eff.cutscene_then().unwrap_or_default().is_empty() {
            continue;
        }
        let bare = start.strip_prefix("cs_").unwrap_or(&start).to_string();
        let other = second_player(n);
        let them = format!("@a[name={other},limit=1]");
        let mut b = packtest_header(&format!(
            "{title}: `{start}` puts each of two players back where that player stood, \
             facing where they faced — a cutscene moves nobody"
        ));
        b.push(format!("function {ns}:setup"));
        b.push(format!("scoreboard players set #run_{bare} dw.sys 0"));
        b.push(format!("dummy {other} spawn"));
        b.push("kill @e[type=minecraft:marker,tag=dw_csr]".to_string());
        b.push("execute at @s run tp @s ~ ~ ~ 90 0".to_string());
        b.push(format!("execute at @s run tp {them} ~4 ~ ~ -90 0"));
        b.push(
            "execute at @s run summon minecraft:marker ~ ~ ~ {Tags:[\"dw_csr\",\"dw_csr_a\"]}"
                .to_string(),
        );
        b.push(format!(
            "execute at {them} run summon minecraft:marker ~ ~ ~ {{Tags:[\"dw_csr\",\"dw_csr_b\"]}}"
        ));
        b.push(format!("function {ns}:{start}"));
        b.push(format!("schedule clear {ns}:cs_tick_{bare}"));
        b.push("# the camera carries each body away from where it stood".to_string());
        b.push("execute at @s run tp @s ~ ~12 ~3".to_string());
        b.push(format!("execute as {them} at @s run tp @s ~-6 ~12 ~"));
        b.push(format!("function {ns}:cs_end_{bare}"));
        let h = |k: &str| format!("#csr{n}_{k}");
        b.push(format!(
            "execute at @s store success score {} dw.sys if entity @e[type=minecraft:marker,tag=dw_csr_a,distance=..0.01]",
            h("a")
        ));
        b.push(format!("assert score {} dw.sys matches 1", h("a")));
        b.push(format!(
            "execute at {them} store success score {} dw.sys if entity @e[type=minecraft:marker,tag=dw_csr_b,distance=..0.01]",
            h("b")
        ));
        b.push(format!("assert score {} dw.sys matches 1", h("b")));
        b.push(format!(
            "execute store result score {} dw.sys run data get entity @s Rotation[0]",
            h("ya")
        ));
        b.push(format!("assert score {} dw.sys matches 90", h("ya")));
        b.push(format!(
            "execute store result score {} dw.sys run data get entity {them} Rotation[0]",
            h("yb")
        ));
        b.push(format!("assert score {} dw.sys matches -90", h("yb")));
        b.push("kill @e[type=minecraft:marker,tag=dw_csr]".to_string());
        b.push("execute at @s run tp @s ~ ~ ~ 90 0".to_string());
        b.push(format!("execute at @s run tp {them} ~4 ~ ~ -90 0"));
        b.push(
            "execute at @s run summon minecraft:marker ~ ~ ~ {Tags:[\"dw_csr\",\"dw_csr_a\"]}"
                .to_string(),
        );
        b.push(format!(
            "execute at {them} run summon minecraft:marker ~ ~ ~ {{Tags:[\"dw_csr\",\"dw_csr_b\"]}}"
        ));
        b.push(format!("function {ns}:{start}"));
        b.push(format!("schedule clear {ns}:cs_tick_{bare}"));
        b.push("# the camera carries each body away from where it stood".to_string());
        b.push("execute at @s run tp @s ~ ~12 ~3".to_string());
        b.push(format!("execute as {them} at @s run tp @s ~-6 ~12 ~"));
        b.push(format!("function {ns}:cs_end_{bare}"));
        b.push(
            "execute at @s store success score #csr_a dw.sys if entity @e[type=minecraft:marker,tag=dw_csr_a,distance=..0.01]"
                .to_string(),
        );
        b.push("assert score #csr_a dw.sys matches 1".to_string());
        b.push(format!(
            "execute at {them} store success score #csr_b dw.sys if entity @e[type=minecraft:marker,tag=dw_csr_b,distance=..0.01]"
        ));
        b.push("assert score #csr_b dw.sys matches 1".to_string());
        b.push(
            "execute store result score #csr_ya dw.sys run data get entity @s Rotation[0]"
                .to_string(),
        );
        b.push("assert score #csr_ya dw.sys matches 90".to_string());
        b.push(format!(
            "execute store result score #csr_yb dw.sys run data get entity {them} Rotation[0]"
        ));
        b.push("assert score #csr_yb dw.sys matches -90".to_string());
        b.push("kill @e[type=minecraft:marker,tag=dw_csr]".to_string());
        b.push(format!("dummy {other} leave"));
        out.insert(
            format!("packtest-datapack/data/{ns}/test/cutscene_return_{n}.mcfunction"),
            lines(&b).into_bytes(),
        );
        n += 1;
    }
}
