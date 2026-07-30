//! Deterministic emission of the `<out>/` build tree (spec-0002).
//!
//! All gameplay wiring is compiler-generated (ADR-0001): the LLM never writes
//! mcfunction. Output is a `BTreeMap<path, bytes>` so ordering is defined
//! (ADR-0006); `manifest.json` hashes make the double-build gate a one-line
//! comparison.
//!
//! JSON is serialized with `serde_json` (default `BTreeMap` maps → sorted keys)
//! plus a trailing newline; mcfunction bodies are built line-by-line. No
//! wall-clock, hostname, locale or absolute path enters any byte.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::commands::{CommandError, CommandTree};
use crate::plan::{
    self, Plan, ResolvedAnchor, Step, campaign_start_quests, obj_score, objective_effects,
    objective_quest, quest_active_score, quest_score,
};
use crate::{DELVEC_VERSION, DSL_VERSION, MC_VERSION, PACK_FORMAT};

use delvewright_dsl::{Objective, QuestEffect, Trigger};

/// The emitted build tree: relative path → file bytes.
pub type BuildOutput = BTreeMap<String, Vec<u8>>;

/// Build the full `<out>/` tree from a plan and the prefab structure bytes
/// (`structure_file` → raw `.nbt`). Runs the command-tree validator over every
/// emitted `.mcfunction`; a validation failure is a build error.
pub fn build(
    plan: &Plan,
    input_bytes: &BTreeMap<String, Vec<u8>>,
    structures: &BTreeMap<String, Vec<u8>>,
    tree: &CommandTree,
) -> Result<BuildOutput, Vec<CommandError>> {
    let ns = &plan.namespace;
    let mut out: BuildOutput = BTreeMap::new();

    // ---- datapack ----
    put_json(
        &mut out,
        "datapack/pack.mcmeta",
        &json!({
            "pack": {
                "description": format!("Delvewright delve: {ns}"),
                "min_format": PACK_FORMAT,
                "max_format": PACK_FORMAT,
            }
        }),
    );
    put_json(
        &mut out,
        "datapack/data/minecraft/tags/function/load.json",
        &json!({ "values": [format!("{ns}:load")] }),
    );
    put_json(
        &mut out,
        "datapack/data/minecraft/tags/function/tick.json",
        &json!({ "values": [format!("{ns}:tick")] }),
    );

    // structures
    for area in &plan.areas {
        if let Some(bytes) = structures.get(&area.structure_file) {
            out.insert(
                format!("datapack/data/{ns}/structure/{}.nbt", area.structure_id),
                bytes.clone(),
            );
        }
    }

    // functions
    let functions = emit_functions(plan);
    for (name, body) in &functions {
        out.insert(
            format!("datapack/data/{ns}/function/{name}.mcfunction"),
            body.clone().into_bytes(),
        );
    }

    // dialogs
    for (name, value) in emit_dialogs(plan) {
        put_json(
            &mut out,
            &format!("datapack/data/{ns}/dialog/{name}.json"),
            &value,
        );
    }

    // advancements
    for (name, value) in emit_advancements(plan) {
        put_json(
            &mut out,
            &format!("datapack/data/{ns}/advancement/{name}.json"),
            &value,
        );
    }

    // ---- packtest datapack ----
    emit_packtest(plan, &mut out);

    // ---- server ----
    emit_server(plan, &mut out);

    // ---- critical path ----
    put_json(&mut out, "critical-path.json", &emit_critical_path(plan));

    // ---- validate every emitted vanilla mcfunction ----
    let mut errors = Vec::new();
    for (path, bytes) in &out {
        if is_vanilla_function(path)
            && let Ok(body) = std::str::from_utf8(bytes)
        {
            errors.extend(tree.validate_function(body));
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }

    // ---- manifest (hashes of inputs + all other outputs) ----
    let manifest = emit_manifest(plan, input_bytes, &out);
    put_json(&mut out, "manifest.json", &manifest);

    Ok(out)
}

/// Re-validate every emitted vanilla `.mcfunction` in a built tree (used by
/// tests). PackTest functions are excluded — see [`is_vanilla_function`].
pub fn validate_emitted(out: &BuildOutput, tree: &CommandTree) -> Vec<CommandError> {
    let mut errors = Vec::new();
    for (path, bytes) in out {
        if is_vanilla_function(path)
            && let Ok(body) = std::str::from_utf8(bytes)
        {
            errors.extend(tree.validate_function(body));
        }
    }
    errors
}

/// A `.mcfunction` that must pass the vanilla 1.21.11 command-tree validator.
/// The `packtest-datapack/` suite uses PackTest-only commands (`assert`, …) and
/// runs on the modded validation server, so it is exempt (spec-0003/ADR-0003:
/// mods are tooling-only, never the player-facing datapack).
fn is_vanilla_function(path: &str) -> bool {
    path.ends_with(".mcfunction") && !path.starts_with("packtest-datapack/")
}

// ---------------------------------------------------------------------------
// mcfunction emission
// ---------------------------------------------------------------------------

/// Yaw for a facing keyword (MC: yaw 0 = +z/south).
fn facing_yaw(facing: Option<&str>) -> i32 {
    match facing {
        Some("north") => 180,
        Some("east") => 270,
        Some("west") => 90,
        _ => 0, // south / default
    }
}

fn emit_functions(plan: &Plan) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let mut fns: Vec<(String, String)> = Vec::new();

    // --- load ---
    fns.push((
        "load".to_string(),
        lines(&[
            "scoreboard objectives add dw.sys dummy".to_string(),
            format!("execute unless score #init dw.sys matches 1 run function {ns}:setup"),
        ]),
    ));

    // --- setup ---
    let mut setup: Vec<String> = Vec::new();
    setup.push("scoreboard objectives add dw.class trigger".to_string());
    setup.push("scoreboard objectives add dw.classed dummy".to_string());
    setup.push("scoreboard objectives add dw.dlg_shown dummy".to_string());
    for npc in &plan.npcs {
        setup.push(format!(
            "scoreboard objectives add {} trigger",
            npc.trigger_objective
        ));
    }
    for q in &c.quests.content.quests {
        for o in &q.objectives {
            setup.push(format!(
                "scoreboard objectives add {} dummy",
                obj_score(o.id().as_str())
            ));
        }
    }
    for q in &c.quests.content.quests {
        setup.push(format!(
            "scoreboard objectives add {} dummy",
            quest_active_score(q.id.as_str())
        ));
        setup.push(format!(
            "scoreboard objectives add {} dummy",
            quest_score(q.id.as_str())
        ));
    }
    setup.push("scoreboard objectives add dw.campaign dummy".to_string());
    setup.push("scoreboard objectives setdisplay sidebar dw.campaign".to_string());
    // Force-load the chunks covering each prefab BEFORE placing. `#minecraft:load`
    // runs during server boot when the target chunks are not guaranteed to be
    // loaded; without this, `place template` / `summon` / `fill` silently no-op
    // ("That position is not loaded") yet `#init` is still set, permanently
    // skipping setup. Verified live: `forceload add` loads the chunk synchronously
    // so the following commands in this same function succeed. The forceload is
    // kept (not removed) so the placed structure + NPCs stay simulated.
    for area in &plan.areas {
        let ox = area.origin[0];
        let oz = area.origin[2];
        setup.push(format!(
            "forceload add {} {} {} {}",
            ox,
            oz,
            ox + area.size[0] - 1,
            oz + area.size[2] - 1
        ));
    }
    // place each area's prefab
    for area in &plan.areas {
        setup.push(format!(
            "place template {ns}:{} {} {} {}",
            area.structure_id, area.origin[0], area.origin[1], area.origin[2]
        ));
    }
    // summon NPCs (villager body + interaction hitbox)
    for npc in &plan.npcs {
        let area = plan.npc_area(&npc.npc_id).unwrap_or("");
        let anchor = c
            .npcs
            .content
            .npcs
            .iter()
            .find(|n| n.id.as_str() == npc.npc_id)
            .map(|n| n.anchor.as_str())
            .unwrap_or("");
        let (pos, facing) = match plan.anchors.get(&(area.to_string(), anchor.to_string())) {
            Some(ResolvedAnchor::Point { pos, facing }) => (*pos, facing.as_deref()),
            _ => ([0, plan::BASE_Y, 0], None),
        };
        let dsl_npc = c
            .npcs
            .content
            .npcs
            .iter()
            .find(|n| n.id.as_str() == npc.npc_id);
        let name = dsl_npc.map(|n| n.name.as_str()).unwrap_or("NPC");
        let base = dsl_npc
            .map(|n| n.base_entity.as_str())
            .unwrap_or("minecraft:villager");
        let yaw = facing_yaw(facing);
        let cname = json!({ "text": name }).to_string().replace('\'', "\\'");
        setup.push(format!(
            "summon {base} {} {} {} {{NoAI:1b,Invulnerable:1b,Silent:1b,PersistenceRequired:1b,NoGravity:1b,Rotation:[{yaw}f,0f],Tags:[\"dw_npc\",\"{}\"],CustomName:'{}',CustomNameVisible:1b,VillagerData:{{profession:\"minecraft:none\",type:\"minecraft:plains\",level:1}}}}",
            pos[0], pos[1], pos[2], npc.tag, cname
        ));
        setup.push(format!(
            "summon minecraft:interaction {} {} {} {{width:1.0f,height:2.0f,response:1b,Invulnerable:1b,Tags:[\"{}\"]}}",
            pos[0], pos[1], pos[2], npc.tag
        ));
    }
    // Set world spawn to the first area's `spawn` anchor so joining players land
    // on the prefab floor instead of falling through the void world before class
    // selection teleports them.
    if let Some(pos) = campaign_spawn(plan) {
        setup.push(format!("setworldspawn {} {} {}", pos[0], pos[1], pos[2]));
    }
    setup.push("scoreboard players set #init dw.sys 1".to_string());
    fns.push(("setup".to_string(), lines(&setup)));

    // --- tick ---
    let mut tick: Vec<String> = Vec::new();
    tick.push("scoreboard players enable @a dw.class".to_string());
    for npc in &plan.npcs {
        tick.push(format!(
            "scoreboard players enable @a {}",
            npc.trigger_objective
        ));
    }
    tick.push(format!(
        "execute as @a unless score @s dw.classed matches 1 unless score @s dw.dlg_shown matches 1 run function {ns}:show_class"
    ));
    for class in &plan.classes {
        tick.push(format!(
            "execute as @a[scores={{dw.class={}}}] run function {ns}:class_apply_{}",
            class.n, class.safe
        ));
    }
    for npc in &plan.npcs {
        for opt in &npc.options {
            tick.push(format!(
                "execute as @a[scores={{{}={}}}] run function {ns}:dlg_{}_{}",
                npc.trigger_objective, opt.n, npc.safe, opt.n
            ));
        }
    }
    // reach-anchor proximity checks
    for q in &c.quests.content.quests {
        let area = plan.quest_area(q.id.as_str()).unwrap_or("");
        for o in &q.objectives {
            if let Objective::ReachAnchor {
                id,
                anchor,
                radius,
                after,
            } = o
            {
                let pos = match plan
                    .anchors
                    .get(&(area.to_string(), anchor.as_str().to_string()))
                {
                    Some(ResolvedAnchor::Point { pos, .. }) => *pos,
                    Some(ResolvedAnchor::Gate { from, .. }) => *from,
                    None => continue,
                };
                let mut guard = format!(
                    "execute as @a if score @s {} matches 1",
                    quest_active_score(q.id.as_str())
                );
                for a in after {
                    guard.push_str(&format!(" if score @s {} matches 1", obj_score(a.as_str())));
                }
                guard.push_str(&format!(
                    " unless score @s {} matches 1",
                    obj_score(id.as_str())
                ));
                guard.push_str(&format!(
                    " if entity @s[x={},y={},z={},distance=..{}] run function {ns}:complete_{}",
                    pos[0],
                    pos[1],
                    pos[2],
                    radius,
                    obj_score(id.as_str()).replace("dw.o_", "o_")
                ));
                tick.push(guard);
            }
        }
    }
    fns.push(("tick".to_string(), lines(&tick)));

    // --- show_class ---
    fns.push((
        "show_class".to_string(),
        lines(&[
            format!("dialog show @s {ns}:class_select"),
            "scoreboard players set @s dw.dlg_shown 1".to_string(),
        ]),
    ));

    // --- class apply ---
    let campaign_start = campaign_start_quests(c);
    for (i, class) in c.classes.content.classes.iter().enumerate() {
        let plan_class = &plan.classes[i];
        let mut body: Vec<String> = Vec::new();
        body.push("scoreboard players reset @s dw.class".to_string());
        for item in &class.kit {
            let comp = match &item.name {
                Some(n) => format!("[custom_name={}]", json!({ "text": n, "italic": false })),
                None => String::new(),
            };
            body.push(format!("give @s {}{} {}", item.item, comp, item.count));
        }
        body.push("scoreboard players set @s dw.classed 1".to_string());
        for qid in &campaign_start {
            body.push(format!(
                "scoreboard players set @s {} 1",
                quest_active_score(qid)
            ));
        }
        // teleport to the first area's spawn anchor
        if let Some(pos) = campaign_spawn(plan) {
            body.push(format!("teleport @s {} {} {}", pos[0], pos[1], pos[2]));
        }
        fns.push((format!("class_apply_{}", plan_class.safe), lines(&body)));
    }

    // --- dialog option handlers ---
    for npc in &plan.npcs {
        for opt in &npc.options {
            let mut body: Vec<String> = Vec::new();
            body.push(format!(
                "scoreboard players reset @s {}",
                npc.trigger_objective
            ));
            for obj in &opt.completes {
                if let Some((qid, _)) = objective_quest(c, obj) {
                    body.push(format!(
                        "execute if score @s {} matches 1 unless score @s {} matches 1 run function {ns}:complete_{}",
                        quest_active_score(qid),
                        obj_score(obj),
                        safe_obj_fn(obj)
                    ));
                }
            }
            if let Some(next) = &opt.next {
                body.push(format!(
                    "dialog show @s {ns}:{}_{}",
                    npc.safe,
                    plan::safe_local(next)
                ));
            }
            fns.push((format!("dlg_{}_{}", npc.safe, opt.n), lines(&body)));
        }
        // keeper interaction reward: (re)show the root dialog.
        fns.push((
            format!("talk_{}", npc.safe),
            lines(&[
                format!("advancement revoke @s only {ns}:{}_interact", npc.safe),
                format!(
                    "dialog show @s {ns}:{}_{}",
                    npc.safe,
                    plan::safe_local(&npc.root)
                ),
            ]),
        ));
    }

    // --- objective completion + quest checks ---
    for q in &c.quests.content.quests {
        for o in &q.objectives {
            let oid = o.id().as_str();
            let mut body: Vec<String> = Vec::new();
            body.push(format!("scoreboard players set @s {} 1", obj_score(oid)));
            for eff in objective_effects(c, oid) {
                emit_quest_effect(plan, eff, &mut body);
            }
            body.push(format!(
                "function {ns}:check_q_{}",
                plan::safe_local(q.id.as_str())
            ));
            fns.push((format!("complete_{}", safe_obj_fn(oid)), lines(&body)));
        }

        // check_q_<quest>
        let mut check: Vec<String> = Vec::new();
        let mut guard = "execute".to_string();
        for o in &q.objectives {
            guard.push_str(&format!(
                " if score @s {} matches 1",
                obj_score(o.id().as_str())
            ));
        }
        guard.push_str(&format!(
            " unless score @s {} matches 1 run function {ns}:complete_q_{}",
            quest_score(q.id.as_str()),
            plan::safe_local(q.id.as_str())
        ));
        check.push(guard);
        fns.push((
            format!("check_q_{}", plan::safe_local(q.id.as_str())),
            lines(&check),
        ));

        // complete_q_<quest>
        let mut done: Vec<String> = Vec::new();
        done.push(format!(
            "scoreboard players set @s {} 1",
            quest_score(q.id.as_str())
        ));
        for eff in &q.on_complete {
            emit_quest_effect(plan, eff, &mut done);
        }
        // activate quests triggered by this quest's completion
        for dep in &c.quests.content.quests {
            if let Trigger::QuestComplete { quest } = &dep.trigger
                && quest.as_str() == q.id.as_str()
            {
                done.push(format!(
                    "scoreboard players set @s {} 1",
                    quest_active_score(dep.id.as_str())
                ));
            }
        }
        fns.push((
            format!("complete_q_{}", plan::safe_local(q.id.as_str())),
            lines(&done),
        ));
    }

    // --- campaign_complete (shared by campaign-complete effect) ---
    let title = &c.world.content.title;
    fns.push((
        "campaign_complete".to_string(),
        lines(&[
            "scoreboard players set @s dw.campaign 1".to_string(),
            format!("advancement grant @s only {ns}:campaign_complete"),
            format!(
                "tellraw @s {}",
                json!([
                    { "text": format!("{title} — complete."), "color": "gold" },
                    { "text": "\n" },
                    { "text": "A Delvewright delve.", "color": "gray" }
                ])
            ),
            // Machine-readable completion marker for the validation bot. The bot
            // reads `dw.campaign` from the sidebar per the amended contract, BUT
            // mineflayer 4.37.x cannot parse 1.21.11 scoreboard score packets
            // (verified live: no score updates ever surface). Broadcasting a stable
            // token in chat — which mineflayer DOES parse reliably — lets the bot
            // observe completion. `<objective> <value>` mirror the assert-complete
            // step so the harness stays campaign-agnostic. `@a` so a bot filling a
            // seat in a future multiplayer delve still sees it.
            format!(
                "tellraw @a {}",
                json!({ "text": format!("[Delvewright] complete dw.campaign 1"), "color": "dark_gray" })
            ),
        ]),
    ));

    fns.sort_by(|a, b| a.0.cmp(&b.0));
    fns
}

/// Emit a quest effect's commands into `body`.
fn emit_quest_effect(plan: &Plan, eff: &QuestEffect, body: &mut Vec<String>) {
    let ns = &plan.namespace;
    match eff {
        QuestEffect::OpenGate { anchor } => {
            // Find the gate anchor across areas (first match).
            for ((_, name), resolved) in &plan.anchors {
                if name == anchor.as_str()
                    && let ResolvedAnchor::Gate { from, to, block } = resolved
                {
                    body.push(format!(
                        "fill {} {} {} {} {} {} minecraft:air replace {}",
                        from[0], from[1], from[2], to[0], to[1], to[2], block
                    ));
                    return;
                }
            }
        }
        QuestEffect::CampaignComplete => {
            body.push(format!("function {ns}:campaign_complete"));
        }
        // Reserved effects are rejected by validation before emission.
        _ => {}
    }
}

/// The first area's `spawn` anchor absolute position.
fn campaign_spawn(plan: &Plan) -> Option<[i32; 3]> {
    for area in &plan.areas {
        if let Some(ResolvedAnchor::Point { pos, .. }) = plan
            .anchors
            .get(&(area.area_id.clone(), "spawn".to_string()))
        {
            return Some(*pos);
        }
    }
    None
}

/// Objective id → function-name-safe token (`obj/talk` → `o_talk`).
fn safe_obj_fn(obj_id: &str) -> String {
    format!("o_{}", plan::safe_local(obj_id))
}

// ---------------------------------------------------------------------------
// dialogs / advancements
// ---------------------------------------------------------------------------

fn emit_dialogs(plan: &Plan) -> Vec<(String, Value)> {
    let c = plan.campaign;
    let mut dialogs = Vec::new();

    // class selection
    let actions: Vec<Value> = plan
        .classes
        .iter()
        .zip(&c.classes.content.classes)
        .map(|(cp, class)| {
            json!({
                "label": class.name,
                "tooltip": class.blurb,
                "action": { "type": "minecraft:run_command", "command": format!("/trigger dw.class set {}", cp.n) }
            })
        })
        .collect();
    dialogs.push((
        "class_select".to_string(),
        json!({
            "type": "minecraft:multi_action",
            "title": "Choose your class",
            "body": [{ "type": "minecraft:plain_message", "contents": "Pick the kit you will carry." }],
            "columns": 1,
            "can_close_with_escape": false,
            "after_action": "close",
            "actions": actions
        }),
    ));

    // per-npc dialogue nodes → one dialog each
    for npc in &plan.npcs {
        let dsl_npc = c
            .npcs
            .content
            .npcs
            .iter()
            .find(|n| n.id.as_str() == npc.npc_id);
        let Some(dsl_npc) = dsl_npc else { continue };
        for node in &dsl_npc.dialogue.nodes {
            let node_opts: Vec<&plan::OptionPlan> = npc
                .options
                .iter()
                .filter(|o| o.node_id == node.id.as_str())
                .collect();
            let actions: Vec<Value> = node_opts
                .iter()
                .map(|o| {
                    json!({
                        "label": o.label,
                        "action": { "type": "minecraft:run_command", "command": format!("/trigger {} set {}", npc.trigger_objective, o.n) }
                    })
                })
                .collect();
            dialogs.push((
                format!("{}_{}", npc.safe, plan::safe_local(node.id.as_str())),
                json!({
                    "type": "minecraft:multi_action",
                    "title": dsl_npc.name,
                    "body": [{ "type": "minecraft:plain_message", "contents": node.text }],
                    "columns": 1,
                    "can_close_with_escape": true,
                    "after_action": "close",
                    "actions": actions
                }),
            ));
        }
    }
    dialogs
}

fn emit_advancements(plan: &Plan) -> Vec<(String, Value)> {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let mut advs = Vec::new();

    // one interaction advancement per NPC
    for npc in &plan.npcs {
        advs.push((
            format!("{}_interact", npc.safe),
            json!({
                "criteria": {
                    "interact": {
                        "trigger": "minecraft:player_interacted_with_entity",
                        // 1.21.11's `player_interacted_with_entity` `entity` field is
                        // an Either<single entity sub-predicate, list of loot
                        // conditions>. The list form requires each entity_properties
                        // condition to carry its own `entity: "this"` key; the single
                        // sub-predicate object form is simpler and is what loads
                        // cleanly on a live server (verified in the load shakeout —
                        // the list form failed with "No key entity in MapLike").
                        "conditions": {
                            "entity": {
                                "type": "minecraft:interaction",
                                "nbt": format!("{{Tags:[\"{}\"]}}", npc.tag)
                            }
                        }
                    }
                },
                "rewards": { "function": format!("{ns}:talk_{}", npc.safe) }
            }),
        ));
    }

    // campaign-complete advancement (granted by command)
    advs.push((
        "campaign_complete".to_string(),
        json!({
            "criteria": { "granted": { "trigger": "minecraft:impossible" } },
            "display": {
                "icon": { "id": "minecraft:iron_door" },
                "title": c.world.content.title,
                "description": "You left the keep.",
                "frame": "goal",
                "show_toast": true,
                "announce_to_chat": false,
                "hidden": false
            }
        }),
    ));
    advs
}

// ---------------------------------------------------------------------------
// packtest / server / critical-path / manifest
// ---------------------------------------------------------------------------

/// Emit the compiler-generated PackTest suite (spec-0003). PackTest (misode,
/// 2.4.0 for MC 1.21.11) auto-discovers `*.mcfunction` files under
/// `data/<ns>/test/`; each is one game test driven by `# @…` directive comments,
/// with `assert`/`await`/`succeed`/`fail` commands the mod adds. Run headlessly
/// with `-Dpacktest.auto` (exit code = failed tests). These functions use
/// PackTest-only commands and run on the modded validation server, so they are
/// exempt from the vanilla command-tree validator (see `is_vanilla_function`).
fn emit_packtest(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let c = plan.campaign;
    put_json(
        out,
        "packtest-datapack/pack.mcmeta",
        &json!({
            "pack": {
                "description": format!("Delvewright PackTest suite: {ns}"),
                "min_format": PACK_FORMAT,
                "max_format": PACK_FORMAT,
            }
        }),
    );

    // The completion objective + value the critical path asserts on.
    let (comp_obj, comp_val) = plan
        .critical_path
        .iter()
        .find_map(|s| match s {
            Step::AssertComplete { objective, value } => Some((objective.clone(), *value)),
            _ => None,
        })
        .unwrap_or_else(|| ("dw.campaign".to_string(), 1));

    // Mechanism test: on a dummy player, run the real generated init, activate the
    // campaign-start quests (as class selection does), drive each objective's
    // generated completion function (as the dialog `/trigger` and the reach
    // proximity check do), then assert the completion objective is set. This
    // proves the compiler's objective -> quest -> campaign chain end to end
    // without needing dialog-UI clicks or bot movement (verified live: passes on
    // Fabric + PackTest 2.4.0).
    let mut body: Vec<String> = Vec::new();
    body.push(format!(
        "#> {}: objective completions set {comp_obj} (Delvewright mechanism test)",
        c.world.content.title
    ));
    body.push("# @dummy".to_string());
    body.push("# @timeout 100".to_string());
    body.push(String::new());
    body.push(format!("function {ns}:setup"));
    for qid in campaign_start_quests(c) {
        body.push(format!(
            "scoreboard players set @a {} 1",
            quest_active_score(qid)
        ));
    }
    for q in &c.quests.content.quests {
        for o in &q.objectives {
            body.push(format!(
                "execute as @a run function {ns}:complete_{}",
                safe_obj_fn(o.id().as_str())
            ));
        }
    }
    // `assert score` requires a single-entity selector (@p = the dummy player).
    body.push(format!("assert score @p {comp_obj} matches {comp_val}"));

    out.insert(
        format!("packtest-datapack/data/{ns}/test/campaign.mcfunction"),
        lines(&body).into_bytes(),
    );
}

fn emit_server(plan: &Plan, out: &mut BuildOutput) {
    // server.properties (keys sorted for determinism).
    let props: BTreeMap<&str, String> = BTreeMap::from([
        ("allow-nether", "false".to_string()),
        ("difficulty", "peaceful".to_string()),
        ("force-gamemode", "true".to_string()),
        ("gamemode", "adventure".to_string()),
        ("generate-structures", "false".to_string()),
        (
            "generator-settings",
            "{\"biome\":\"minecraft:the_void\",\"layers\":[]}".to_string(),
        ),
        ("level-name", "world".to_string()),
        ("level-seed", plan.seed.to_string()),
        ("level-type", "minecraft:flat".to_string()),
        ("online-mode", "false".to_string()),
        ("pvp", "false".to_string()),
        ("spawn-monsters", "false".to_string()),
        ("spawn-protection", "0".to_string()),
    ]);
    let mut text = String::new();
    text.push_str(&format!(
        "# Generated by delvec for campaign {} (spec-0002 world strategy).\n",
        plan.namespace
    ));
    text.push_str("# Void/superflat + fixed seed; the world is created on first boot.\n");
    for (k, v) in &props {
        text.push_str(&format!("{k}={v}\n"));
    }
    out.insert("server/server.properties".to_string(), text.into_bytes());

    out.insert(
        "server/eula-note.txt".to_string(),
        b"Accepting Mojang's EULA is the operator's action, never the compiler's.\n\
Set EULA=TRUE in the environment (or eula.txt) before running a server here.\n\
The server jar is NOT shipped (ADR-0010); it is fetched by version at run time.\n"
            .to_vec(),
    );

    out.insert(
        "server/README.md".to_string(),
        format!(
            "# server/\n\n\
Level config for campaign `{}`. The world is generated on first server boot\n\
from `server.properties` (no region files shipped, spec-0002):\n\n\
- `level-type=minecraft:flat` + `generator-settings` with an empty layer list and\n\
  the `minecraft:the_void` biome ⇒ a void world.\n\
- `level-seed={}` pins world generation (ADR-0006); v0 uses no other randomness.\n\
- `gamemode=adventure`, `difficulty=peaceful`, no structures/monsters.\n\n\
The compiler-emitted `#minecraft:load` bootstrap (`datapack/`) places each area's\n\
prefab with `/place template` and summons NPCs; nothing is baked into region\n\
bytes, so byte-identity (ADR-0006) covers the whole `<out>/` tree.\n",
            plan.namespace, plan.seed
        )
        .into_bytes(),
    );
}

fn emit_critical_path(plan: &Plan) -> Value {
    let steps: Vec<Value> = plan
        .critical_path
        .iter()
        .map(|s| match s {
            Step::SelectClass { class_id, command } => json!({
                "action": "select-class", "class": class_id, "command": command
            }),
            Step::TalkTo { npc_id, pos, command } => json!({
                "action": "talk-to", "npc": npc_id, "pos": pos, "command": command
            }),
            Step::Reach { anchor_id, pos, radius } => json!({
                "action": "reach", "anchor": anchor_id, "pos": pos, "radius": radius
            }),
            Step::AssertComplete { objective, value } => json!({
                "action": "assert-complete", "scoreboard": { "objective": objective, "value": value }
            }),
        })
        .collect();
    json!({
        "version": DSL_VERSION,
        "campaign_id": plan.namespace,
        "steps": steps
    })
}

fn emit_manifest(plan: &Plan, input_bytes: &BTreeMap<String, Vec<u8>>, out: &BuildOutput) -> Value {
    let inputs: BTreeMap<String, String> = input_bytes
        .iter()
        .map(|(k, v)| (k.clone(), sha256_hex(v)))
        .collect();
    let outputs: BTreeMap<String, String> = out
        .iter()
        .map(|(k, v)| (k.clone(), sha256_hex(v)))
        .collect();
    json!({
        "campaign_id": plan.namespace,
        "delvec_version": DELVEC_VERSION,
        "dsl_version": DSL_VERSION,
        "mc_version": MC_VERSION,
        "inputs": inputs,
        "outputs": outputs
    })
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// Join lines with `\n` and a trailing newline.
fn lines(v: &[String]) -> String {
    let mut s = v.join("\n");
    s.push('\n');
    s
}

/// Serialize a JSON value canonically (sorted keys via serde_json default map,
/// 2-space pretty, trailing newline) into `out` at `path`.
fn put_json(out: &mut BuildOutput, path: &str, value: &Value) {
    let mut bytes = serde_json::to_vec_pretty(value).expect("json serializes");
    bytes.push(b'\n');
    out.insert(path.to_string(), bytes);
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut s = String::with_capacity(64);
    for b in digest {
        s.push_str(&format!("{b:02x}"));
    }
    s
}
