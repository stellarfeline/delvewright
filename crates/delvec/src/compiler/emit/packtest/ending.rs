use super::*;

use crate::compiler::ending::{InputKey, Scenes};

/// spec-0108 §6.2: **the world after the scene holds the change.** One template
/// per (ending, witness world) — `ending_<slug>_<k>` — that pins the world's
/// solved readings of every input the final scene's gates read, runs the REAL
/// function the scene is lowered into (`complete_q_<quest>`, `complete_<obj>`,
/// `trig_<trigger>`), drives every part of the scene the emitter scheduled —
/// each cutscene's `cs_end_<bare>` (and with it the cutscene's `then`), each
/// later `sequence` step — and asserts each aftermath effect the world fires
/// left its observable behind.
///
/// **Driven, not awaited.** A template that `await`s must be the sole owner of
/// every `#party` score it touches (`tests/packtest_batch.rs`), and a world's
/// readings are party flags every sibling shares; so the scheduled parts are
/// driven inside the one atomic function instead — each schedule cleared, then
/// its function called, in tick order — which runs the same functions the
/// scheduler would have run, in the same order, with nothing between them.
///
/// Every observable is made falsifiable before the scene runs: a written block's
/// cells are set to something else first, a carried body is summoned in the
/// volume (and must leave it), a spawned tag is cleared (and must come back).
/// Every global the scene changes that a sibling reads — the clock, the
/// weather — is restored before the function returns.
pub(super) fn emit_ending_packtests(plan: &Plan, out: &mut BuildOutput) {
    for t in ending_templates(plan) {
        out.insert(
            format!(
                "packtest-datapack/data/{}/test/{}.mcfunction",
                plan.namespace, t.name
            ),
            lines(&t.lines).into_bytes(),
        );
    }
}

/// One generated ending template.
pub(crate) struct EndingTemplate {
    /// `ending_<slug>_<k>`.
    pub(crate) name: String,
    /// The ending's id.
    pub(crate) ending: String,
    /// The world it drives.
    pub(crate) world: usize,
    /// The template body.
    pub(crate) lines: Vec<String>,
    /// How many observables it asserts.
    pub(crate) asserted: usize,
}

/// The function a final scene is lowered into, as the line that runs it.
fn scene_driver(plan: &Plan, scene: &str, me: &str) -> Option<String> {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let segs: Vec<&str> = scene.trim_start_matches('/').split('/').collect();
    match segs.as_slice() {
        ["content", "quests", i, "on_complete"] => {
            let q = c.quests.content.quests.get(i.parse::<usize>().ok()?)?;
            Some(format!(
                "function {ns}:complete_q_{}",
                plan::safe_local(q.id.as_str())
            ))
        }
        ["content", "quests", _, "on_objective_complete", "obj", o] => Some(format!(
            "execute as {me} at {me} run function {ns}:complete_{}",
            safe_obj_fn(&format!("obj/{o}"))
        )),
        ["content", "triggers", i, "effects"] => {
            let t = c.quests.content.triggers.get(i.parse::<usize>().ok()?)?;
            Some(format!(
                "function {ns}:trig_{}",
                plan::safe_local(t.id.as_str())
            ))
        }
        _ => None,
    }
}

/// The audience a scene's root bundle is lowered under.
fn scene_audience(scene: &str) -> Audience {
    if scene.contains("/on_complete") || scene.contains("/on_objective_complete") {
        root_audience(delvewright_dsl::EffectRootKind::QuestComplete)
    } else {
        Audience::Scheduled
    }
}

/// Every scheduled part of `list`, as `(tick, line)` pairs: a cutscene's end
/// (its schedule cleared, then `cs_end_<bare>` called — which runs its `then`)
/// and a `sequence`'s later steps, each followed by whatever they schedule in
/// turn. Plain effects are not here: the functions that hold them run them.
fn drive(plan: &Plan, list: &[QuestEffect], t0: u32, aud: Audience, out: &mut Vec<(u32, String)>) {
    let ns = &plan.namespace;
    for e in list {
        match &e.verb {
            Verb::Cutscene { then, .. } => {
                let (Some(name), Some(end)) = (
                    cutscene_effect_fn(e),
                    crate::compiler::link::cutscene_end_offset(e),
                ) else {
                    continue;
                };
                let bare = name.trim_start_matches("cs_").to_string();
                out.push((t0 + end, format!("schedule clear {ns}:cs_tick_{bare}")));
                out.push((t0 + end, format!("function {ns}:cs_end_{bare}")));
                drive(plan, then, t0 + end + 1, Audience::Scheduled, out);
            }
            Verb::Sequence { steps } => {
                let name = sequence_fn(plan, e, aud);
                for (i, st) in steps.iter().enumerate() {
                    if st.at_ticks > 0 {
                        out.push((t0 + st.at_ticks, format!("schedule clear {ns}:{name}_{i}")));
                        out.push((t0 + st.at_ticks, format!("function {ns}:{name}_{i}")));
                    }
                    drive(plan, &st.effects, t0 + st.at_ticks, aud, out);
                }
            }
            _ => {}
        }
    }
}

/// The effect list at a scene pointer.
fn scene_list<'a>(c: &'a delvewright_dsl::Campaign, scene: &str) -> Option<&'a [QuestEffect]> {
    let mut found: Option<&'a [QuestEffect]> = None;
    delvewright_dsl::for_each_effect_root(c, &mut |site, list| {
        if site.path == scene {
            found = Some(list);
        }
    });
    found
}

/// Every ending template the build owes, in ending then world order.
pub(crate) fn ending_templates(plan: &Plan) -> Vec<EndingTemplate> {
    templates_and_unproven(plan).0
}

/// Every (ending, world) pair whose world fires an observable aftermath effect
/// of the ending's scene and for which no template could be generated, in
/// words. Empty on every campaign the generator covers; the build refuses a
/// non-empty list as an internal invariant, so an ending that owes a proof
/// never ships without one, wherever its `campaign-complete` stands.
pub(crate) fn unproven_endings(plan: &Plan) -> Vec<String> {
    templates_and_unproven(plan).1
}

fn templates_and_unproven(plan: &Plan) -> (Vec<EndingTemplate>, Vec<String>) {
    let c = plan.campaign;
    let ns = &plan.namespace;
    let title = artifact_title(c);
    let flow = crate::compiler::flow::Flow::new(c);
    let story = flow.story();
    let a = crate::compiler::ending::analyse(c, story);
    let scenes: &Scenes = &a.scenes;
    let mut by_path: BTreeMap<String, &QuestEffect> = BTreeMap::new();
    delvewright_dsl::for_each_campaign_effect(c, &mut |p, _, e| {
        by_path.insert(p.to_string(), e);
    });
    let world_time = c.world.content.time;
    let world_weather = c.world.content.weather;
    let mut out = Vec::new();
    let mut unproven = Vec::new();
    for v in &a.endings {
        let scene = &scenes.scenes[v.ending.scene];
        let slug =
            plan::safe_local(v.ending.id.trim_start_matches("ending/")).replace(['<', '>'], "");
        for &k in &v.worlds {
            let run = &story.runs[k];
            // What this world owes: an observable change its scene fires.
            let owes = scene.aftermath.iter().any(|x| {
                run.fired.contains(&x.path)
                    && by_path
                        .get(&x.path)
                        .and_then(|e| crate::compiler::emit::ending::observation(plan, e))
                        .is_some()
            });
            let name = format!("ending_{slug}_{k}");
            let (pin, me) = pin_dummy(&format!("dw_endtest_{slug}_{k}"));
            let (Some(list), Some(snap), Some(driver)) = (
                scene_list(c, &scene.path),
                run.scene_at.get(&scene.path),
                scene_driver(plan, &scene.path, &me),
            ) else {
                if owes {
                    unproven.push(format!(
                        "`{}` at `{}` in world/{k} (scene `{}`)",
                        v.ending.id, v.ending.path, scene.path
                    ));
                }
                continue;
            };
            let sc = format!("{slug}_{k}");
            let mut t = packtest_header(&format!(
                "{title}: `{}` in world/{k} leaves the world it says it left — every aftermath \
                 effect of its final scene observed after the scene (spec-0108 §6.2)",
                v.ending.id
            ));
            t.push(format!("function {ns}:setup"));
            t.push(pin);
            // The world's readings, pinned: every input the scene's gates read.
            for key in &scene.inputs {
                match key {
                    InputKey::Flag(f) => t.push(format!(
                        "scoreboard players set {} {} {}",
                        plan::PARTY,
                        plan::flag_score(f),
                        u8::from(snap.flags.contains(f))
                    )),
                    InputKey::Datum(d) => {
                        let Some(val) = snap.data.get(d).copied().flatten() else {
                            continue;
                        };
                        let id = delvewright_dsl::StateId(d.clone());
                        let holder = state_holder(plan, &id);
                        let holder = if holder == "@s" { me.clone() } else { holder };
                        t.push(format!(
                            "scoreboard players set {holder} {} {val}",
                            plan::state_score(d)
                        ));
                    }
                }
            }
            // Each aftermath effect this world fires, made falsifiable first.
            let mut asserts: Vec<String> = Vec::new();
            let mut cleanup: Vec<String> = Vec::new();
            let mut restore_time = false;
            let mut restore_weather = false;
            let mut n = 0usize;
            for x in &scene.aftermath {
                if !run.fired.contains(&x.path) {
                    continue;
                }
                let Some(eff) = by_path.get(&x.path) else {
                    continue;
                };
                let Some(obs) = crate::compiler::emit::ending::observation(plan, eff) else {
                    continue;
                };
                let h = format!("#end_{sc}_{n}");
                match obs["kind"].as_str() {
                    Some("block") => {
                        let block = obs["block"].as_str().unwrap_or(AIR).to_string();
                        let sentinel = if block == "minecraft:bedrock" {
                            "minecraft:obsidian"
                        } else {
                            "minecraft:bedrock"
                        };
                        let mut cells: Vec<Vec<i64>> = Vec::new();
                        for at in ["first", "last"] {
                            let p: Vec<i64> = obs[at]
                                .as_array()
                                .map(|a| a.iter().filter_map(Value::as_i64).collect())
                                .unwrap_or_default();
                            if p.len() != 3 || cells.contains(&p) {
                                continue;
                            }
                            cells.push(p.clone());
                            t.push(format!("setblock {} {} {} {sentinel}", p[0], p[1], p[2]));
                            asserts
                                .push(format!("assert block {} {} {} {block}", p[0], p[1], p[2]));
                        }
                    }
                    Some("position") => {
                        let lo: Vec<i64> = obs["from"]["min"]
                            .as_array()
                            .map(|a| a.iter().filter_map(Value::as_i64).collect())
                            .unwrap_or_default();
                        let hi: Vec<i64> = obs["from"]["max"]
                            .as_array()
                            .map(|a| a.iter().filter_map(Value::as_i64).collect())
                            .unwrap_or_default();
                        let to: Vec<i64> = obs["to"]
                            .as_array()
                            .map(|a| a.iter().filter_map(Value::as_i64).collect())
                            .unwrap_or_default();
                        if lo.len() != 3 || hi.len() != 3 || to.len() != 3 {
                            continue;
                        }
                        let (lo, hi) = (
                            [lo[0] as i32, lo[1] as i32, lo[2] as i32],
                            [hi[0] as i32, hi[1] as i32, hi[2] as i32],
                        );
                        let bx = box_selector_args(lo, hi);
                        let body = format!("dw_endbody_{sc}_{n}");
                        let mid = [
                            (lo[0] + hi[0]) / 2,
                            (lo[1] + hi[1]) / 2,
                            (lo[2] + hi[2]) / 2,
                        ];
                        t.push(format!("kill @e[tag={body}]"));
                        t.push(format!(
                            "summon minecraft:zombie {} {} {} {{Tags:[\"{body}\"],NoAI:1b,Silent:1b,\
                             PersistenceRequired:1b,Invulnerable:1b}}",
                            f64::from(mid[0]) + 0.5,
                            mid[1],
                            f64::from(mid[2]) + 0.5
                        ));
                        t.push(format!(
                            "execute store result score {h}_in dw.sys if entity @e[tag={body},{bx}]"
                        ));
                        t.push(format!("assert score {h}_in dw.sys matches 1"));
                        asserts.push(format!(
                            "execute store result score {h}_left dw.sys if entity @e[tag={body},{bx}]"
                        ));
                        asserts.push(format!("assert score {h}_left dw.sys matches 0"));
                        asserts.push(format!(
                            "execute store result score {h}_at dw.sys if entity @e[tag={body},x={},y={},z={},distance=..1.5]",
                            f64::from(to[0] as i32) + 0.5,
                            to[1],
                            f64::from(to[2] as i32) + 0.5
                        ));
                        asserts.push(format!("assert score {h}_at dw.sys matches 1"));
                        cleanup.push(format!("kill @e[tag={body}]"));
                    }
                    Some("present") => {
                        let sel = match (obs["tag"].as_str(), obs["type"].as_str()) {
                            (Some(tag), _) => format!("@e[tag={tag}]"),
                            (None, Some(ty)) => format!("@e[type={ty}]"),
                            _ => continue,
                        };
                        if obs["tag"].is_string() {
                            t.push(format!("kill {sel}"));
                            cleanup.push(format!("kill {sel}"));
                        }
                        asserts.push(format!(
                            "execute store result score {h} dw.sys if entity {sel}"
                        ));
                        asserts.push(format!("assert score {h} dw.sys matches 1.."));
                    }
                    Some("absent") => {
                        let Some(tag) = obs["tag"].as_str() else {
                            continue;
                        };
                        asserts.push(format!(
                            "execute store result score {h} dw.sys if entity @e[tag={tag}]"
                        ));
                        asserts.push(format!("assert score {h} dw.sys matches 0"));
                    }
                    Some("time") => {
                        let Some(daytime) = obs["daytime"].as_i64() else {
                            continue;
                        };
                        asserts.push(format!(
                            "execute store result score {h} dw.sys run time query daytime"
                        ));
                        asserts.push(format!("assert score {h} dw.sys matches {daytime}"));
                        restore_time = true;
                    }
                    Some("weather") => {
                        let pred = match obs["weather"].as_str() {
                            Some("clear") => {
                                "{condition:\"minecraft:weather_check\",raining:false}"
                            }
                            Some("rain") => {
                                "{condition:\"minecraft:weather_check\",raining:true,thundering:false}"
                            }
                            Some("thunder") => {
                                "{condition:\"minecraft:weather_check\",thundering:true}"
                            }
                            _ => continue,
                        };
                        asserts.push(format!(
                            "execute store success score {h} dw.sys if predicate {pred}"
                        ));
                        asserts.push(format!("assert score {h} dw.sys matches 1"));
                        restore_weather = true;
                    }
                    Some("score") => {
                        if let Some(p) = obs["checkpoint"].as_array() {
                            let p: Vec<i64> = p.iter().filter_map(Value::as_i64).collect();
                            if p.len() != 3 {
                                continue;
                            }
                            asserts.push(format!(
                                "execute store success score {h} dw.sys if data storage dw:cp {{pos:[{},{},{}]}}",
                                p[0], p[1], p[2]
                            ));
                            asserts.push(format!("assert score {h} dw.sys matches 1"));
                        } else if let (Some(holder), Some(obj)) =
                            (obs["holder"].as_str(), obs["objective"].as_str())
                        {
                            let holder = if holder == "@s" { me.as_str() } else { holder };
                            let value = match (obs["value"].as_i64(), obs["added"].as_i64()) {
                                (Some(v), _) => Some(v),
                                (None, Some(add)) => {
                                    let d = obj.trim_start_matches("dw.s_");
                                    scene
                                        .inputs
                                        .iter()
                                        .find_map(|k| match k {
                                            InputKey::Datum(id) if plan::state_score(id) == obj => {
                                                snap.data.get(id).copied().flatten()
                                            }
                                            _ => None,
                                        })
                                        .or_else(|| {
                                            let _ = d;
                                            None
                                        })
                                        .map(|base| base + add)
                                }
                                _ => None,
                            };
                            let Some(value) = value else {
                                continue;
                            };
                            asserts.push(format!("assert score {holder} {obj} matches {value}"));
                        } else {
                            continue;
                        }
                    }
                    _ => continue,
                }
                n += 1;
            }
            if n == 0 {
                if owes {
                    unproven.push(format!(
                        "`{}` at `{}` in world/{k}: no observable of its scene could be staged",
                        v.ending.id, v.ending.path
                    ));
                }
                continue;
            }
            // The scene, through its real function, and every part of it the
            // emitter scheduled, in tick order.
            t.push(driver);
            let mut scheduled: Vec<(u32, String)> = Vec::new();
            drive(plan, list, 0, scene_audience(&scene.path), &mut scheduled);
            scheduled.sort_by_key(|(tick, _)| *tick);
            t.extend(scheduled.into_iter().map(|(_, l)| l));
            t.extend(asserts);
            t.extend(cleanup);
            if restore_time {
                t.push(format!(
                    "time set {}",
                    world_time.token(world_time.world_clock())
                ));
            }
            if restore_weather {
                t.push(format!("weather {}", world_weather.token()));
            }
            out.push(EndingTemplate {
                name,
                ending: v.ending.id.clone(),
                world: k,
                lines: t,
                asserted: n,
            });
        }
    }
    (out, unproven)
}
