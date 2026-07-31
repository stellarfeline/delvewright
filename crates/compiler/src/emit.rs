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
use crate::{DELVEC_VERSION, MC_VERSION, PACK_FORMAT};

use delvewright_dsl::{Objective, QuestEffect, Trigger, is_v03};

/// The emitted build tree: relative path → file bytes.
pub type BuildOutput = BTreeMap<String, Vec<u8>>;

/// Build the full `<out>/` tree from a plan and the prefab structure bytes
/// (`structure_file` → raw `.nbt`). Runs the command-tree validator over every
/// emitted `.mcfunction`; a validation failure is a build error.
///
/// `language` is the target build language (i18n): `None` or `Some("en")` is the
/// canonical English build (the manifest records no `language`, so an English
/// build stays byte-identical to a pre-i18n one); `Some("<code>")` records the
/// language in the manifest. The `plan`'s campaign must already be localized to
/// that language by the caller ([`delvewright_dsl::localize`]).
pub fn build(
    plan: &Plan,
    input_bytes: &BTreeMap<String, Vec<u8>>,
    structures: &BTreeMap<String, Vec<u8>>,
    tree: &CommandTree,
    language: Option<&str>,
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

    // structures (one `.nbt` per distinct structure id, even if reused across
    // several placed pieces — the insert is idempotent, same bytes)
    for area in &plan.areas {
        for piece in &area.pieces {
            if let Some(bytes) = structures.get(&piece.structure_file) {
                out.insert(
                    format!("datapack/data/{ns}/structure/{}.nbt", piece.structure_id),
                    bytes.clone(),
                );
            }
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

    // ---- creator overlay (playtest-only; spec-0006) ----
    // A self-contained module (crate::creator). Its `.mcfunction`s are plain
    // vanilla, so they flow through the command-tree validator below and the
    // determinism gate like the main datapack; the shipped delve image excludes
    // this directory (CI-checked, same as packtest-datapack/).
    crate::creator::emit_creator(plan, &mut out);

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
    let manifest = emit_manifest(plan, input_bytes, &out, language);
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

/// The environment-sealing baseline (spec-0002 "Environment sealing").
///
/// **1.21.11 gamerule syntax — verified live against a pinned 1.21.11 server
/// (delvewright-base / itzg VANILLA), 2026-07-30.** 1.21.11 replaced the legacy
/// camelCase gamerule identifiers with a registry of snake_case names, several of
/// them renamed outright; the old spellings are rejected with "Incorrect argument
/// for command". The confirmed successors used here:
///
/// | Legacy (spec text)   | 1.21.11 accepted                       |
/// |----------------------|----------------------------------------|
/// | `doMobSpawning`      | `spawn_mobs` (umbrella natural-spawn)   |
/// | `doDaylightCycle`    | `advance_time`                          |
/// | `doWeatherCycle`     | `advance_weather`                       |
/// | `doFireTick`         | `fire_spread_radius_around_player` (int)|
/// | `mobGriefing`        | `mob_griefing`                          |
///
/// `doFireTick` has **no boolean successor**; 1.21.11 models fire spread as an
/// integer radius around players, so `0` disables it (the sealing intent: no
/// spreading fire). Time is pinned to noon (`time set noon` = daytime 6000, a
/// tree literal) — the v0 default; a stage-1 field may override it later. Names
/// may optionally be `minecraft:`-prefixed on the server, but the bare form is
/// accepted and matches the vendored command tree (`data/commands-1.21.11.json`),
/// so it is what we emit and validate.
fn sealing_commands() -> Vec<String> {
    vec![
        "gamerule spawn_mobs false".to_string(),
        "gamerule advance_time false".to_string(),
        "gamerule advance_weather false".to_string(),
        "gamerule fire_spread_radius_around_player 0".to_string(),
        "gamerule mob_griefing false".to_string(),
        "time set noon".to_string(),
    ]
}

/// Yaw for a facing keyword (MC: yaw 0 = +z/south).
fn facing_yaw(facing: Option<&str>) -> i32 {
    match facing {
        Some("north") => 180,
        Some("east") => 270,
        Some("west") => 90,
        _ => 0, // south / default
    }
}

/// Whether this campaign compiles under DSL v0.3 (gate for every M2
/// presentation fix). The gate is the quests-stage version (all v0.3 surface
/// lives in stage 5) — matching [`crate::registry`]/validation. v0.2 campaigns
/// (hello-world / keep-crawl) take the untouched pre-v0.3 emission path, keeping
/// their output byte-identical.
fn campaign_is_v03(plan: &Plan) -> bool {
    is_v03(plan.campaign.quests.dsl_version.as_str())
}

/// Escape a player-facing string as a double-quoted SNBT string. On 1.21.11
/// `CustomName` is a **text component**, so a bare quoted SNBT string is read as
/// literal text (the JSON-string form `'{"text":"…"}'` renders verbatim, incl. in
/// death messages — the M2 defect). Only `\` and `"` need escaping inside SNBT.
fn snbt_string(s: &str) -> String {
    let esc = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{esc}\"")
}

/// Default hand equipment for a summoned mob whose natural spawns are armed
/// (M2 fix 5). `/summon` gives no equipment, so a wither-skeleton boss spawned
/// unarmed was trivial. Returns an NBT fragment (no leading comma) setting
/// `HandItems` with drop chance 0, or `None` for mobs that spawn unarmed. Small
/// static table (documented in the compiler README); mobs not listed (zombie,
/// drowned — a wild trident is not a default) get nothing.
fn default_equipment(entity: &str) -> Option<&'static str> {
    match entity.strip_prefix("minecraft:").unwrap_or(entity) {
        "wither_skeleton" => {
            Some("HandItems:[{id:\"minecraft:stone_sword\",count:1},{}],HandDropChances:[0f,0f]")
        }
        "skeleton" | "stray" => {
            Some("HandItems:[{id:\"minecraft:bow\",count:1},{}],HandDropChances:[0f,0f]")
        }
        _ => None,
    }
}

fn emit_functions(plan: &Plan) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let v03 = campaign_is_v03(plan);
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
    // Environment sealing (spec-0002): a delve is a box garden — every dynamic is
    // authored, nothing is left to vanilla chance. Emitted first, once, guarded by
    // the same `#init` flag as the rest of setup.
    setup.push(
        "# Environment sealing (spec-0002): box garden — nothing left to vanilla chance."
            .to_string(),
    );
    setup.extend(sealing_commands());
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
    // v0.3: the shared wave countdown, per-flag scores, and interact triggers.
    // Each loop is empty for a v0.2 campaign, so hello-world / keep-crawl setup is
    // byte-identical.
    if !c.quests.content.waves.is_empty() {
        setup.push(format!(
            "scoreboard objectives add {} dummy",
            plan::WAVE_OBJECTIVE
        ));
    }
    for flag in declared_flags(c) {
        setup.push(format!(
            "scoreboard objectives add {} dummy",
            plan::flag_score(&flag)
        ));
    }
    for (oid, _) in interact_objectives(c) {
        setup.push(format!(
            "scoreboard objectives add {} trigger",
            plan::interact_trigger(&oid)
        ));
    }
    // v0.3 objective-activation feedback (M2 fix 4): one "announced" flag per
    // titled objective. Empty for a v0.2 campaign, so hello-world / keep-crawl
    // setup stays byte-identical.
    if v03 {
        for q in &c.quests.content.quests {
            for o in &q.objectives {
                if o.title().is_some() {
                    setup.push(format!(
                        "scoreboard objectives add {} dummy",
                        announce_score(o.id().as_str())
                    ));
                }
            }
        }
    }
    // Force-load the chunks covering each prefab BEFORE placing. `#minecraft:load`
    // runs during server boot when the target chunks are not guaranteed to be
    // loaded; without this, `place template` / `summon` / `fill` silently no-op
    // ("That position is not loaded") yet `#init` is still set, permanently
    // skipping setup. Verified live: `forceload add` loads the chunk synchronously
    // so the following commands in this same function succeed. The forceload is
    // kept (not removed) so the placed structure + NPCs stay simulated.
    for area in &plan.areas {
        for piece in &area.pieces {
            let (min, max) = piece.bbox();
            setup.push(format!(
                "forceload add {} {} {} {}",
                min[0], min[2], max[0], max[2]
            ));
        }
    }
    // place each piece (single-prefab areas: one piece, rotation none → no
    // rotation token, keeping the M1 output byte-identical; pool areas: the
    // solver's per-piece pos + rotation)
    for area in &plan.areas {
        for piece in &area.pieces {
            let rot = match piece.rotation.token() {
                Some(t) => format!(" {t}"),
                None => String::new(),
            };
            setup.push(format!(
                "place template {ns}:{} {} {} {}{rot}",
                piece.structure_id, piece.pos[0], piece.pos[1], piece.pos[2]
            ));
        }
    }
    // seal/clear sockets: open sockets get a wall fill; mated sockets get their
    // jigsaw block cleared to air, leaving a clean 3×3 passage (keep-socket-v1).
    // Runs after placement so it overwrites the raw structure blocks. Empty for
    // single-prefab areas.
    for area in &plan.areas {
        for seal in &area.seals {
            setup.push(format!(
                "fill {} {} {} {} {} {} {}",
                seal.from[0],
                seal.from[1],
                seal.from[2],
                seal.to[0],
                seal.to[1],
                seal.to[2],
                seal.block
            ));
        }
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
        // CustomName is a 1.21.11 text component. v0.3 emits a plain SNBT string
        // (renders correctly, incl. death messages — M2 fix 1); v0.2 keeps the
        // legacy `'{"text":…}'` form so hello-world / keep-crawl stay byte-identical.
        let cname_field = if v03 {
            snbt_string(name)
        } else {
            let cname = json!({ "text": name }).to_string().replace('\'', "\\'");
            format!("'{cname}'")
        };
        setup.push(format!(
            "summon {base} {} {} {} {{NoAI:1b,Invulnerable:1b,Silent:1b,PersistenceRequired:1b,NoGravity:1b,Rotation:[{yaw}f,0f],Tags:[\"dw_npc\",\"{}\"],CustomName:{},CustomNameVisible:1b,VillagerData:{{profession:\"minecraft:none\",type:\"minecraft:plains\",level:1}}}}",
            pos[0], pos[1], pos[2], npc.tag, cname_field
        ));
        setup.push(format!(
            "summon minecraft:interaction {} {} {} {{width:1.0f,height:2.0f,response:1b,Invulnerable:1b,Tags:[\"{}\"]}}",
            pos[0], pos[1], pos[2], npc.tag
        ));
    }
    // v0.3: place a loaded chest at each `collect` anchor and an interaction
    // entity at each `interact` anchor. After the seal fills so they overwrite the
    // structure floor cell. Empty for v0.2 campaigns (byte-identity preserved).
    for q in &c.quests.content.quests {
        let area = plan.quest_area(q.id.as_str()).unwrap_or("");
        for o in &q.objectives {
            match o {
                Objective::Collect {
                    item,
                    count,
                    anchor,
                    ..
                } => {
                    if let Some(pos) = plan.point(area, anchor.as_str()) {
                        setup.push(format!(
                            "setblock {} {} {} minecraft:chest",
                            pos[0], pos[1], pos[2]
                        ));
                        setup.push(format!(
                            "item replace block {} {} {} container.0 with {} {}",
                            pos[0], pos[1], pos[2], item, count
                        ));
                    }
                }
                Objective::Interact { id, anchor, .. } => {
                    if let Some(pos) = plan.point(area, anchor.as_str()) {
                        setup.push(format!(
                            "summon minecraft:interaction {} {} {} {{width:1.0f,height:2.0f,response:1b,Invulnerable:1b,Tags:[\"{}\"]}}",
                            pos[0], pos[1], pos[2], interact_entity_tag(id.as_str())
                        ));
                        // Visible, glowing, adventure-safe marker so a human can
                        // find the interact target (M2 fix 3): an `item_display`
                        // has no collision, so it obstructs neither movement nor
                        // the interaction hitbox. Its name derives from the
                        // objective `title` (fallback: the objective id).
                        let marker_name = snbt_string(o.title().unwrap_or(id.as_str()));
                        setup.push(format!(
                            "summon minecraft:item_display {} {} {} {{Glowing:1b,Tags:[\"dw_marker\",\"{}\"],CustomName:{},CustomNameVisible:1b,billboard:\"center\",item:{{id:\"minecraft:lantern\",count:1}}}}",
                            pos[0], pos[1], pos[2], interact_entity_tag(id.as_str()), marker_name
                        ));
                    }
                }
                _ => {}
            }
        }
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
    // v0.3: interact triggers are enabled so the bot's `/trigger` (and re-tries)
    // work, matching the dialog trigger pattern. Empty for v0.2 campaigns.
    for (oid, _) in interact_objectives(c) {
        tick.push(format!(
            "scoreboard players enable @a {}",
            plan::interact_trigger(&oid)
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
    // v0.3 objective-activation feedback (M2 fix 4): announce a titled objective
    // the tick it becomes active (quest active, `after`/flags satisfied, not yet
    // complete) and has not been announced. Runs before the completion checks so
    // "new objective" precedes any same-tick "complete". Empty for v0.2.
    if v03 {
        for q in &c.quests.content.quests {
            let qa = quest_active_score(q.id.as_str());
            for o in &q.objectives {
                if o.title().is_some() {
                    tick.push(format!(
                        "execute as @a{} unless score @s {} matches 1 run function {ns}:announce_{}",
                        pending_guard(o, &qa),
                        announce_score(o.id().as_str()),
                        safe_obj_fn(o.id().as_str())
                    ));
                }
            }
        }
    }
    // Per-tick objective completion checks. `reach-anchor` (proximity) is
    // unchanged for v0.2; `kill` (wave countdown reached zero) and `interact`
    // (trigger fired + optional item) are v0.3 additions. `collect` is
    // advancement-driven, not polled here.
    for q in &c.quests.content.quests {
        let area = plan.quest_area(q.id.as_str()).unwrap_or("");
        let qa = quest_active_score(q.id.as_str());
        for o in &q.objectives {
            match o {
                Objective::ReachAnchor {
                    id, anchor, radius, ..
                } => {
                    let pos = match plan
                        .anchors
                        .get(&(area.to_string(), anchor.as_str().to_string()))
                    {
                        Some(ResolvedAnchor::Point { pos, .. }) => *pos,
                        Some(ResolvedAnchor::Gate { from, .. }) => *from,
                        None => continue,
                    };
                    // v0.3 (M2 fix 8): a point-radius `distance=..R` sphere was too
                    // tight for a human standing on the altar cell. Test a block
                    // region instead — the anchor cell with ±1 generosity on every
                    // axis (a 3×3×3 box centred on the anchor). v0.2 keeps the
                    // sphere so hello-world / keep-crawl stay byte-identical.
                    if v03 {
                        tick.push(format!(
                            "execute as @a{} if entity @s[x={},dx=2,y={},dy=2,z={},dz=2] run function {ns}:complete_{}",
                            pending_guard(o, &qa),
                            pos[0] - 1, pos[1] - 1, pos[2] - 1,
                            safe_obj_fn(id.as_str())
                        ));
                    } else {
                        tick.push(format!(
                            "execute as @a{} if entity @s[x={},y={},z={},distance=..{}] run function {ns}:complete_{}",
                            pending_guard(o, &qa),
                            pos[0], pos[1], pos[2], radius,
                            safe_obj_fn(id.as_str())
                        ));
                    }
                }
                Objective::Kill { id, wave, .. } => {
                    tick.push(format!(
                        "execute as @a{} if score {} {} matches ..0 run function {ns}:complete_{}",
                        pending_guard(o, &qa),
                        plan::wave_counter(wave.as_str()),
                        plan::WAVE_OBJECTIVE,
                        safe_obj_fn(id.as_str())
                    ));
                }
                Objective::Interact {
                    id, requires_item, ..
                } => {
                    let trigger = plan::interact_trigger(id.as_str());
                    let item_guard = match requires_item {
                        Some(it) => format!(" if items entity @s container.* {it}"),
                        None => String::new(),
                    };
                    // The trigger is set by the bot's chat command or the
                    // interaction advancement's reward; the guard applies uniformly.
                    tick.push(format!(
                        "execute as @a[scores={{{trigger}=1..}}]{}{item_guard} run function {ns}:complete_{}",
                        pending_guard(o, &qa),
                        safe_obj_fn(id.as_str())
                    ));
                    // Reset the trigger every tick so a gated attempt can be retried
                    // (e.g. clicked the door before holding the key).
                    tick.push(format!(
                        "execute as @a[scores={{{trigger}=1..}}] run scoreboard players reset @s {trigger}"
                    ));
                }
                Objective::TalkTo { .. } | Objective::Collect { .. } => {}
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
            // v0.3 objective-activation feedback (M2 fix 4): the announce function
            // shows the title + hint once and plays a subtle sound. Emitted only
            // for titled objectives (v0.3); nothing for v0.2.
            if v03 && let Some(title) = o.title() {
                let mut ann: Vec<String> = Vec::new();
                ann.push(format!(
                    "tellraw @s {}",
                    json!([
                        { "text": "New objective: ", "color": "yellow", "bold": true },
                        { "text": title, "color": "gold" }
                    ])
                ));
                if let Some(hint) = o.hint() {
                    ann.push(format!(
                        "tellraw @s {}",
                        json!({ "text": hint, "color": "gray", "italic": true })
                    ));
                }
                ann.push("playsound minecraft:block.note_block.pling player @s".to_string());
                ann.push(format!(
                    "scoreboard players set @s {} 1",
                    announce_score(oid)
                ));
                fns.push((format!("announce_{}", safe_obj_fn(oid)), lines(&ann)));
            }

            let mut body: Vec<String> = Vec::new();
            body.push(format!("scoreboard players set @s {} 1", obj_score(oid)));
            // v0.3 objective-completion feedback (M2 fix 4): a confirmation line +
            // sound so progress is legible. Titled objectives only; v0.2 unchanged.
            if v03 && let Some(title) = o.title() {
                body.push(format!(
                    "tellraw @s {}",
                    json!([
                        { "text": "Objective complete: ", "color": "green" },
                        { "text": title, "color": "white" }
                    ])
                ));
                body.push("playsound minecraft:entity.experience_orb.pickup player @s".to_string());
            }
            for eff in objective_effects(c, oid) {
                emit_quest_effect(plan, eff, &mut body);
            }
            // Inter-area transport: if completing this objective moves the player
            // into a different area on the critical path, teleport them to that
            // area's entry spawn (areas are AREA_SPACING apart across void). Runs
            // after gate effects so the destination area is already unlocked.
            if let Some(pos) = plan.transport.get(oid) {
                body.push(format!("teleport @s {} {} {}", pos[0], pos[1], pos[2]));
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
    let mut cc: Vec<String> = Vec::new();
    cc.push("scoreboard players set @s dw.campaign 1".to_string());
    cc.push(format!("advancement grant @s only {ns}:campaign_complete"));
    cc.push(format!(
        "tellraw @s {}",
        json!([
            { "text": format!("{title} — complete."), "color": "gold" },
            { "text": "\n" },
            { "text": "A Delvewright delve.", "color": "gray" }
        ])
    ));
    // v0.3 finale fanfare (M2 fix 4): the owner finished the finale and got no
    // feedback. Show a proper title banner + play a fanfare. Gated on v0.3 so the
    // shared `campaign_complete` stays byte-identical for hello-world / keep-crawl.
    if v03 {
        cc.push(format!(
            "title @s title {}",
            json!({ "text": "Delve Complete", "color": "gold", "bold": true })
        ));
        cc.push(format!(
            "title @s subtitle {}",
            json!({ "text": title, "color": "yellow" })
        ));
        cc.push("playsound minecraft:ui.toast.challenge_complete player @s".to_string());
    }
    // Machine-readable completion marker for the validation bot. The bot reads
    // `dw.campaign` from the sidebar per the amended contract, BUT mineflayer
    // 4.37.x cannot parse 1.21.11 scoreboard score packets (verified live: no
    // score updates ever surface). Broadcasting a stable token in chat — which
    // mineflayer DOES parse reliably — lets the bot observe completion.
    // `<objective> <value>` mirror the assert-complete step so the harness stays
    // campaign-agnostic. `@a` so a bot filling a seat in a future multiplayer
    // delve still sees it.
    cc.push(format!(
        "tellraw @a {}",
        json!({ "text": format!("[Delvewright] complete dw.campaign 1"), "color": "dark_gray" })
    ));
    fns.push(("campaign_complete".to_string(), lines(&cc)));

    // --- v0.3: wave spawn functions + verb reward functions ---
    for w in &c.quests.content.waves {
        let Some(pos) = wave_spawn_pos(plan, w.id.as_str()) else {
            continue;
        };
        let mut body: Vec<String> = Vec::new();
        body.push(format!(
            "scoreboard players set {} {} {}",
            plan::wave_counter(w.id.as_str()),
            plan::WAVE_OBJECTIVE,
            plan::wave_total(w)
        ));
        let mut idx = 0i32;
        for mob in &w.mobs {
            // CustomName as a plain SNBT text component (M2 fix 1). Waves are
            // v0.3-only, so no v0.2 byte-identity concern.
            let name = match &mob.name {
                Some(n) => format!(",CustomName:{},CustomNameVisible:1b", snbt_string(n)),
                None => String::new(),
            };
            // Default hand equipment for mobs whose natural spawns are armed (M2
            // fix 5): a summoned wither_skeleton/skeleton otherwise had no weapon
            // and was trivial. Drop chance 0.
            let equip = default_equipment(&mob.entity)
                .map(|e| format!(",{e}"))
                .unwrap_or_default();
            for _ in 0..mob.count {
                // Spread stacks by a small deterministic offset; AI is left enabled
                // (no NoAI) so the mobs fight.
                body.push(format!(
                    "summon {} {} {} {} {{Tags:[\"{}\"],PersistenceRequired:1b{name}{equip}}}",
                    mob.entity,
                    pos[0] + idx,
                    pos[1],
                    pos[2],
                    plan::wave_tag(w.id.as_str())
                ));
                idx += 1;
            }
        }
        fns.push((
            format!("spawn_{}", plan::safe_local(w.id.as_str())),
            lines(&body),
        ));
        // kill reward: each slain wave mob decrements the countdown, then re-arms.
        fns.push((
            format!("k_reward_{}", plan::safe_local(w.id.as_str())),
            lines(&[
                format!(
                    "scoreboard players remove {} {} 1",
                    plan::wave_counter(w.id.as_str()),
                    plan::WAVE_OBJECTIVE
                ),
                format!(
                    "advancement revoke @s only {ns}:k_{}",
                    plan::safe_local(w.id.as_str())
                ),
            ]),
        ));
    }
    for q in &c.quests.content.quests {
        let qa = quest_active_score(q.id.as_str());
        for o in &q.objectives {
            match o {
                Objective::Interact { id, .. } => {
                    // Human click path: the interaction advancement sets the same
                    // trigger the bot chats; the per-tick handler applies guards.
                    fns.push((
                        format!("i_reward_{}", plan::safe_local(id.as_str())),
                        lines(&[
                            format!(
                                "advancement revoke @s only {ns}:i_{}",
                                plan::safe_local(id.as_str())
                            ),
                            format!(
                                "scoreboard players set @s {} 1",
                                plan::interact_trigger(id.as_str())
                            ),
                        ]),
                    ));
                }
                Objective::Collect { id, .. } => {
                    // inventory_changed reward: complete (if the quest/after/flags
                    // guards hold), then re-arm.
                    fns.push((
                        format!("c_reward_{}", plan::safe_local(id.as_str())),
                        lines(&[
                            format!(
                                "execute{} run function {ns}:complete_{}",
                                pending_guard(o, &qa),
                                safe_obj_fn(id.as_str())
                            ),
                            format!(
                                "advancement revoke @s only {ns}:c_{}",
                                plan::safe_local(id.as_str())
                            ),
                        ]),
                    ));
                }
                _ => {}
            }
        }
    }

    fns.sort_by(|a, b| a.0.cmp(&b.0));
    fns
}

/// The absolute spawn position of a wave: the world coords of its anchor, resolved
/// in the area of the (first) quest whose `kill` objective references it.
fn wave_spawn_pos(plan: &Plan, wave_id: &str) -> Option<[i32; 3]> {
    let c = plan.campaign;
    let w = plan::wave_of(c, wave_id)?;
    for q in &c.quests.content.quests {
        let Some(area) = plan.quest_area(q.id.as_str()) else {
            continue;
        };
        for o in &q.objectives {
            if let Objective::Kill { wave, .. } = o
                && wave.as_str() == wave_id
            {
                return plan.point(area, w.anchor.as_str());
            }
        }
    }
    None
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
        QuestEffect::GiveItem { item, count } => {
            body.push(format!("give @s {item} {count}"));
        }
        QuestEffect::SetFlag { flag } => {
            body.push(format!(
                "scoreboard players set @s {} 1",
                plan::flag_score(flag.as_str())
            ));
        }
        QuestEffect::SpawnWave { wave } => {
            body.push(format!(
                "function {ns}:spawn_{}",
                plan::safe_local(wave.as_str())
            ));
        }
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

/// Per-objective "already announced" scoreboard (v0.3 objective-activation
/// feedback, M2 fix 4). Set once the objective's title/hint has been shown so the
/// announce fires exactly once per player.
fn announce_score(obj_id: &str) -> String {
    format!("dw.ann_{}", plan::safe_local(obj_id))
}

/// The entity tag on an `interact` objective's interaction hitbox.
fn interact_entity_tag(obj_id: &str) -> String {
    format!("dw_i_{}", plan::safe_local(obj_id))
}

/// The flags any `set-flag` effect produces (sorted, deduped).
fn declared_flags(c: &delvewright_dsl::Campaign) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    for q in &c.quests.content.quests {
        let effs = q
            .on_objective_complete
            .values()
            .flatten()
            .chain(q.on_complete.iter());
        for eff in effs {
            if let Some(f) = eff.set_flag() {
                out.insert(f.as_str().to_string());
            }
        }
    }
    out
}

/// `(objective id, quest id)` for every `interact` objective, in declared order.
fn interact_objectives(c: &delvewright_dsl::Campaign) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for q in &c.quests.content.quests {
        for o in &q.objectives {
            if matches!(o, Objective::Interact { .. }) {
                out.push((o.id().as_str().to_string(), q.id.as_str().to_string()));
            }
        }
    }
    out
}

/// The intra-quest activation + pending guard for an objective (v0.3): the quest
/// must be active, every `after` prerequisite and `requires_flags` flag set, and
/// the objective itself not yet complete. Returns the ` if …`/` unless …` fragment
/// (leading space); callers prepend `execute as @a` and append the type-specific
/// condition + `run`. For a v0.2 objective (no `after`/flags) this is exactly the
/// pre-v0.3 reach guard, keeping keep-crawl byte-identical.
fn pending_guard(o: &Objective, quest_active: &str) -> String {
    let mut g = format!(" if score @s {quest_active} matches 1");
    for a in o.after() {
        g.push_str(&format!(" if score @s {} matches 1", obj_score(a.as_str())));
    }
    for f in o.requires_flags() {
        g.push_str(&format!(
            " if score @s {} matches 1",
            plan::flag_score(f.as_str())
        ));
    }
    g.push_str(&format!(
        " unless score @s {} matches 1",
        obj_score(o.id().as_str())
    ));
    g
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

    // per-npc dialogue nodes (stage 6) → one dialog each
    for npc in &plan.npcs {
        let dsl_npc = c
            .npcs
            .content
            .npcs
            .iter()
            .find(|n| n.id.as_str() == npc.npc_id);
        let Some(dsl_npc) = dsl_npc else { continue };
        let Some(tree) = c.dialogue.content.tree_for(&npc.npc_id) else {
            continue;
        };
        for node in &tree.nodes {
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

    // v0.3: one advancement per interact objective, collect objective and wave.
    for q in &c.quests.content.quests {
        for o in &q.objectives {
            match o {
                Objective::Interact { id, .. } => {
                    let tag = interact_entity_tag(id.as_str());
                    advs.push((
                        format!("i_{}", plan::safe_local(id.as_str())),
                        json!({
                            "criteria": {
                                "interact": {
                                    "trigger": "minecraft:player_interacted_with_entity",
                                    "conditions": {
                                        "entity": {
                                            "type": "minecraft:interaction",
                                            "nbt": format!("{{Tags:[\"{tag}\"]}}")
                                        }
                                    }
                                }
                            },
                            "rewards": { "function": format!("{ns}:i_reward_{}", plan::safe_local(id.as_str())) }
                        }),
                    ));
                }
                Objective::Collect {
                    id, item, count, ..
                } => {
                    advs.push((
                        format!("c_{}", plan::safe_local(id.as_str())),
                        json!({
                            "criteria": {
                                "got": {
                                    "trigger": "minecraft:inventory_changed",
                                    "conditions": {
                                        "items": [ { "items": item, "count": { "min": count } } ]
                                    }
                                }
                            },
                            "rewards": { "function": format!("{ns}:c_reward_{}", plan::safe_local(id.as_str())) }
                        }),
                    ));
                }
                _ => {}
            }
        }
    }
    for w in &c.quests.content.waves {
        let tag = plan::wave_tag(w.id.as_str());
        advs.push((
            format!("k_{}", plan::safe_local(w.id.as_str())),
            json!({
                "criteria": {
                    "slain": {
                        "trigger": "minecraft:player_killed_entity",
                        "conditions": {
                            "entity": { "nbt": format!("{{Tags:[\"{tag}\"]}}") }
                        }
                    }
                },
                "rewards": { "function": format!("{ns}:k_reward_{}", plan::safe_local(w.id.as_str())) }
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

    // Sealed-state test: prove the environment-sealing baseline (spec-0002) is
    // applied on boot. What PackTest / vanilla 1.21.11 lets us assert in-test:
    //   * `time set noon` — the world time has a read-back path
    //     (`time query daytime` -> 6000), so it is asserted directly here.
    //   * the five gamerules — 1.21.11 gamerule *values* have NO `execute
    //     if`/predicate read-back in vanilla, so they cannot be asserted in-game.
    //     Their presence and exact 1.21.11 form is a compile-time regression
    //     instead (crates/compiler/tests/emit.rs::environment_sealing_emitted),
    //     which is the authoritative sealing assertion.
    // Verified live: `function <ns>:setup` sets daytime to 6000 and this assert
    // passes on Fabric + PackTest 2.4.0.
    let mut sealed: Vec<String> = Vec::new();
    sealed.push(format!(
        "#> {}: environment sealed on boot (spec-0002)",
        c.world.content.title
    ));
    sealed.push("# @dummy".to_string());
    sealed.push("# @timeout 100".to_string());
    sealed.push(String::new());
    sealed.push(format!("function {ns}:setup"));
    sealed.push("# time set noon -> daytime 6000 (the sole sealing command with a".to_string());
    sealed.push("# vanilla read-back path; gamerules are asserted at compile time).".to_string());
    sealed.push("execute store result score #sealtime dw.sys run time query daytime".to_string());
    sealed.push("assert score #sealtime dw.sys matches 6000".to_string());

    out.insert(
        format!("packtest-datapack/data/{ns}/test/sealed_state.mcfunction"),
        lines(&sealed).into_bytes(),
    );

    // v0.3: one focused mechanism test per gameplay verb present in the campaign,
    // plus a flag-gate test. Each drives the compiler-generated mechanic functions
    // on a dummy player (no real combat / advancement events needed) and asserts
    // the objective scoreboard. Emits nothing for a v0.2 campaign.
    emit_verb_packtests(plan, out);
}

/// The header lines shared by every generated PackTest (`# @dummy` + timeout).
fn packtest_header(title: &str) -> Vec<String> {
    vec![
        format!("#> {title}"),
        "# @dummy".to_string(),
        "# @timeout 100".to_string(),
        String::new(),
    ]
}

/// Lines that satisfy an objective's activation guard on `@a` (quest active, all
/// `after` prerequisites set, all `requires_flags` set, and any required item
/// given). Optionally omit the flags to test the gate.
fn packtest_preamble(quest_id: &str, o: &Objective, with_flags: bool) -> Vec<String> {
    let mut p = vec![format!(
        "scoreboard players set @a {} 1",
        quest_active_score(quest_id)
    )];
    for a in o.after() {
        p.push(format!(
            "scoreboard players set @a {} 1",
            obj_score(a.as_str())
        ));
    }
    if with_flags {
        for f in o.requires_flags() {
            p.push(format!(
                "scoreboard players set @a {} 1",
                plan::flag_score(f.as_str())
            ));
        }
    }
    match o {
        Objective::Collect { item, count, .. } => {
            p.push(format!("give @a {item} {count}"));
        }
        Objective::Interact {
            requires_item: Some(it),
            ..
        } => {
            p.push(format!("give @a {it} 1"));
        }
        _ => {}
    }
    p
}

/// Emit a per-verb mechanism PackTest for the first `kill` / `collect` /
/// `interact` objective, plus a flag-gate test for the first flag-gated
/// collect/interact objective.
fn emit_verb_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let c = plan.campaign;

    let mut write = |name: &str, body: Vec<String>| {
        out.insert(
            format!("packtest-datapack/data/{ns}/test/{name}.mcfunction"),
            lines(&body).into_bytes(),
        );
    };

    // Collect (quest, objective) pairs by verb, in declared order.
    let mut first_kill = None;
    let mut first_collect = None;
    let mut first_interact = None;
    let mut first_flag_gated = None;
    for q in &c.quests.content.quests {
        for o in &q.objectives {
            let qid = q.id.as_str();
            match o {
                Objective::Kill { .. } if first_kill.is_none() => first_kill = Some((qid, o)),
                Objective::Collect { .. } if first_collect.is_none() => {
                    first_collect = Some((qid, o))
                }
                Objective::Interact { .. } if first_interact.is_none() => {
                    first_interact = Some((qid, o))
                }
                _ => {}
            }
            if first_flag_gated.is_none()
                && !o.requires_flags().is_empty()
                && matches!(o, Objective::Collect { .. } | Objective::Interact { .. })
            {
                first_flag_gated = Some((qid, o));
            }
        }
    }

    // kill: spawn the wave, drain the countdown via the kill reward, tick,
    // assert the objective completed.
    if let Some((qid, o)) = first_kill
        && let Objective::Kill { id, wave, .. } = o
        && let Some(w) = plan::wave_of(c, wave.as_str())
    {
        let total = plan::wave_total(w);
        let ws = plan::safe_local(wave.as_str());
        let mut b = packtest_header(&format!(
            "{}: kill wave `{wave}` -> countdown -> complete",
            c.world.content.title
        ));
        b.push(format!("function {ns}:setup"));
        b.extend(packtest_preamble(qid, o, true));
        b.push(format!("function {ns}:spawn_{ws}"));
        b.push(format!(
            "assert score {} {} matches {total}",
            plan::wave_counter(wave.as_str()),
            plan::WAVE_OBJECTIVE
        ));
        b.push(format!("kill @e[tag={}]", plan::wave_tag(wave.as_str())));
        for _ in 0..total {
            b.push(format!("execute as @a run function {ns}:k_reward_{ws}"));
        }
        b.push(format!("function {ns}:tick"));
        b.push(format!(
            "assert score @p {} matches 1",
            obj_score(id.as_str())
        ));
        write("verb_kill", b);
    }

    // collect: satisfy guards + hold the item, run the collect reward, assert.
    if let Some((qid, o)) = first_collect
        && let Objective::Collect { id, .. } = o
    {
        let mut b = packtest_header(&format!(
            "{}: collect -> reward completes objective",
            c.world.content.title
        ));
        b.push(format!("function {ns}:setup"));
        b.extend(packtest_preamble(qid, o, true));
        b.push(format!(
            "execute as @a run function {ns}:c_reward_{}",
            plan::safe_local(id.as_str())
        ));
        b.push(format!(
            "assert score @p {} matches 1",
            obj_score(id.as_str())
        ));
        write("verb_collect", b);
    }

    // interact: hold the required item, fire the trigger, tick, assert.
    if let Some((qid, o)) = first_interact
        && let Objective::Interact { id, .. } = o
    {
        let mut b = packtest_header(&format!(
            "{}: interact trigger + item -> complete",
            c.world.content.title
        ));
        b.push(format!("function {ns}:setup"));
        b.extend(packtest_preamble(qid, o, true));
        b.push(format!(
            "scoreboard players set @a {} 1",
            plan::interact_trigger(id.as_str())
        ));
        b.push(format!("function {ns}:tick"));
        b.push(format!(
            "assert score @p {} matches 1",
            obj_score(id.as_str())
        ));
        write("verb_interact", b);
    }

    // flag gate: without the flag the objective must NOT complete; with it, it does.
    if let Some((qid, o)) = first_flag_gated {
        let id = o.id().as_str();
        let driver = |b: &mut Vec<String>| match o {
            Objective::Collect { .. } => b.push(format!(
                "execute as @a run function {ns}:c_reward_{}",
                plan::safe_local(id)
            )),
            Objective::Interact { .. } => {
                b.push(format!(
                    "scoreboard players set @a {} 1",
                    plan::interact_trigger(id)
                ));
                b.push(format!("function {ns}:tick"));
            }
            _ => {}
        };
        let mut b = packtest_header(&format!(
            "{}: requires_flags gates objective `{id}`",
            c.world.content.title
        ));
        b.push(format!("function {ns}:setup"));
        b.push(format!("scoreboard players set @a {} 0", obj_score(id)));
        b.extend(packtest_preamble(qid, o, false)); // flags withheld
        driver(&mut b);
        b.push(format!("assert score @p {} matches 0", obj_score(id)));
        for f in o.requires_flags() {
            b.push(format!(
                "scoreboard players set @a {} 1",
                plan::flag_score(f.as_str())
            ));
        }
        driver(&mut b);
        b.push(format!("assert score @p {} matches 1", obj_score(id)));
        write("verb_flag_gate", b);
    }
}

fn emit_server(plan: &Plan, out: &mut BuildOutput) {
    // Combat waves (v0.3) require a non-peaceful difficulty: peaceful *removes*
    // hostile mobs even when summoned. Wave-free campaigns stay `peaceful`
    // (hello-world / keep-crawl byte-identical). Natural spawning is still off
    // (`spawn-monsters=false` + gamerule `spawn_mobs false`); only the compiler's
    // summoned wave mobs exist.
    let difficulty = if plan.campaign.quests.content.waves.is_empty() {
        "peaceful"
    } else {
        "easy"
    };
    // server.properties (keys sorted for determinism).
    let props: BTreeMap<&str, String> = BTreeMap::from([
        ("allow-nether", "false".to_string()),
        ("difficulty", difficulty.to_string()),
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
            Step::Kill { wave_id, pos, tag, count } => json!({
                "action": "kill", "wave": wave_id, "pos": pos, "tag": tag, "count": count
            }),
            Step::Collect { item, count, pos } => json!({
                "action": "collect", "item": item, "count": count, "pos": pos
            }),
            Step::Interact { anchor_id, pos, command, requires_item } => json!({
                "action": "interact", "anchor": anchor_id, "pos": pos,
                "command": command, "requires_item": requires_item
            }),
            Step::AssertComplete { objective, value } => json!({
                "action": "assert-complete", "scoreboard": { "objective": objective, "value": value }
            }),
        })
        .collect();
    json!({
        // Campaign-derived (not the compiler's max supported version): a v0.2
        // campaign emits a v0.2 critical path, a v0.3 campaign a v0.3 one.
        "version": plan.campaign.world.dsl_version,
        "campaign_id": plan.namespace,
        "steps": steps
    })
}

fn emit_manifest(
    plan: &Plan,
    input_bytes: &BTreeMap<String, Vec<u8>>,
    out: &BuildOutput,
    language: Option<&str>,
) -> Value {
    let inputs: BTreeMap<String, String> = input_bytes
        .iter()
        .map(|(k, v)| (k.clone(), sha256_hex(v)))
        .collect();
    let outputs: BTreeMap<String, String> = out
        .iter()
        .map(|(k, v)| (k.clone(), sha256_hex(v)))
        .collect();
    let mut manifest = json!({
        "campaign_id": plan.namespace,
        "delvec_version": DELVEC_VERSION,
        "dsl_version": plan.campaign.world.dsl_version,
        "mc_version": MC_VERSION,
        "inputs": inputs,
        "outputs": outputs
    });
    // Record the build language ONLY for a non-canonical build. English is the
    // implicit canonical language, so an `en` build's manifest is byte-identical to
    // a pre-i18n one (preserving the determinism regression for all campaigns that
    // do not localize).
    if let Some(lang) = language
        && lang != delvewright_dsl::CANONICAL_LANG
    {
        manifest
            .as_object_mut()
            .expect("manifest is a JSON object")
            .insert("language".to_string(), Value::String(lang.to_string()));
    }
    manifest
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

// ---------------------------------------------------------------------------
// Unit tests (helpers)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facing_yaw_matches_mc_convention() {
        // MC yaw: south=0, west=90, north=180, east=270.
        assert_eq!(facing_yaw(Some("south")), 0);
        assert_eq!(facing_yaw(Some("west")), 90);
        assert_eq!(facing_yaw(Some("north")), 180);
        assert_eq!(facing_yaw(Some("east")), 270);
        assert_eq!(facing_yaw(None), 0);
    }

    #[test]
    fn snbt_string_is_a_plain_quoted_component() {
        // A bare quoted SNBT string is a valid text component (renders literally),
        // unlike the old `'{"text":…}'` JSON-string form.
        assert_eq!(
            snbt_string("Hedric of the Watch"),
            "\"Hedric of the Watch\""
        );
        // Backslash and double-quote are escaped.
        assert_eq!(snbt_string("a\"b\\c"), "\"a\\\"b\\\\c\"");
    }

    #[test]
    fn default_equipment_arms_only_naturally_armed_mobs() {
        // wither_skeleton → stone sword, drop chance 0.
        let ws = default_equipment("minecraft:wither_skeleton").unwrap();
        assert!(ws.contains("minecraft:stone_sword"));
        assert!(ws.contains("HandDropChances:[0f,0f]"));
        // skeleton/stray → bow.
        assert!(
            default_equipment("skeleton")
                .unwrap()
                .contains("minecraft:bow")
        );
        assert!(
            default_equipment("minecraft:stray")
                .unwrap()
                .contains("minecraft:bow")
        );
        // zombie stays unarmed; drowned's trident is not a default.
        assert!(default_equipment("minecraft:zombie").is_none());
        assert!(default_equipment("minecraft:drowned").is_none());
    }
}
