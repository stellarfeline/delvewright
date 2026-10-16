//! spec-0108 §6.1: `validation/endings.json` — per ending, what it reads, where
//! each input is written, what its scene leaves changed and how a proof can see
//! it, and which story worlds reach it.

use super::*;

use crate::compiler::ending::{Analysis, InputKey, observable};

/// Every effect the campaign can lower, by JSON pointer.
fn effects_by_path(c: &delvewright_dsl::Campaign) -> BTreeMap<String, &QuestEffect> {
    let mut out = BTreeMap::new();
    delvewright_dsl::for_each_campaign_effect(c, &mut |path, _site, e| {
        out.insert(path.to_string(), e);
    });
    out
}

/// What a proof looks at after the scene for one aftermath effect (§6.2): the
/// cell and block, the volume and destination, the entity tag, the clock, or
/// the score — `None` for an emission-only verb, and for an observable verb
/// whose operand does not resolve (refused at validation, never guessed here).
pub(super) fn observation(plan: &Plan, eff: &QuestEffect) -> Option<Value> {
    let kind = observable(eff.verb.tag())?;
    let block = |first: [i32; 3], last: [i32; 3], block: &str| json!({"kind": "block", "first": first, "last": last, "block": block});
    let gate = |anchor: &str| {
        plan.anchors.iter().find_map(|((_, name), r)| match r {
            ResolvedAnchor::Gate { from, to, block } if name == anchor => {
                Some((*from, *to, block.clone()))
            }
            _ => None,
        })
    };
    let v = match &eff.verb {
        Verb::SetBlock {
            anchor, block: b, ..
        } => {
            let p = anchor_point_any(plan, anchor.as_str())?;
            block(p, p, b)
        }
        Verb::FillRegion { .. } | Verb::ClearRegion { .. } => {
            let (zone, b) = eff.region_write()?;
            let (lo, hi) = plan.zone_box(zone)?;
            block(lo, hi, b.unwrap_or(AIR))
        }
        Verb::OpenGate { anchor, .. } => {
            let (lo, hi, _) = gate(anchor.as_str())?;
            block(lo, hi, AIR)
        }
        Verb::CloseGate { anchor, .. } => {
            let (lo, hi, b) = gate(anchor.as_str())?;
            block(lo, hi, &b)
        }
        Verb::OpenWay { .. } => {
            let (piece, name) = eff.way_write()?;
            let way = plan.ways.resolve(piece.as_str(), name).ok()?;
            let b = match way.sign {
                crate::compiler::ways::Sign::Laid => way.block.as_str(),
                crate::compiler::ways::Sign::Cleared => AIR,
            };
            let first = way.boxes.first()?.0;
            let last = way.boxes.last()?.1;
            block(first, last, b)
        }
        Verb::Collapse { .. } => {
            // The region's top layer is emptied by the drop, whatever the
            // falling blocks settle into below it.
            let (zone, _) = eff.region_write()?;
            let (lo, hi) = plan.zone_box(zone)?;
            block([lo[0], hi[1], lo[2]], [hi[0], hi[1], hi[2]], AIR)
        }
        Verb::Teleport { from, to } => {
            let (lo, hi) = plan.zone_box(from)?;
            let cell = to.cell(anchor_point_any(plan, to.anchor.as_str())?);
            json!({"kind": "position", "from": {"min": lo, "max": hi}, "to": cell})
        }
        Verb::SpawnNpc { npc, .. } => {
            json!({"kind": "present", "tag": format!("dw_npc_{}", plan::safe_local(npc.as_str()))})
        }
        Verb::DespawnNpc { npc, .. } => {
            json!({"kind": "absent", "tag": format!("dw_npc_{}", plan::safe_local(npc.as_str()))})
        }
        Verb::SpawnActor { actor, .. } => json!({"kind": "present",
            "tag": format!("dw_actor_{}", plan::safe_local(actor.as_str()))}),
        Verb::DespawnActor { actor, .. } => json!({"kind": "absent",
            "tag": format!("dw_actor_{}", plan::safe_local(actor.as_str()))}),
        Verb::SpawnWave { wave, .. } => {
            json!({"kind": "present", "tag": plan::wave_tag(wave.as_str())})
        }
        Verb::SpawnAssembly { assembly } => json!({"kind": "present",
            "tag": crate::compiler::assembly::tag(&plan::safe_local(assembly.as_str()))}),
        Verb::DespawnAssembly { assembly, .. } => json!({"kind": "absent",
            "tag": crate::compiler::assembly::tag(&plan::safe_local(assembly.as_str()))}),
        Verb::Lightning { .. } => json!({"kind": "present", "type": "minecraft:lightning_bolt"}),
        Verb::MoveNpc { npc, to, .. } => {
            let cell = to.cell(anchor_point_any(plan, to.anchor.as_str())?);
            json!({"kind": "destination",
                "tag": format!("dw_npc_{}", plan::safe_local(npc.as_str())), "to": cell})
        }
        Verb::MoveActor { actor, to, .. } => {
            let cell = to.cell(anchor_point_any(plan, to.anchor.as_str())?);
            json!({"kind": "destination",
                "tag": format!("dw_actor_{}", plan::safe_local(actor.as_str())), "to": cell})
        }
        Verb::UnleashActor { actor, .. } => json!({"kind": "destination",
            "tag": format!("dw_actor_{}", plan::safe_local(actor.as_str()))}),
        Verb::SetTime { time, .. } => {
            let clock = time.clock(
                delvewright_dsl::TimeSite::Cut,
                plan.campaign.world.content.time,
            );
            json!({"kind": "time", "daytime": clock.daytime})
        }
        Verb::SetWeather { weather, .. } => json!({"kind": "weather", "weather": weather.token()}),
        Verb::SetFlag { flag, .. } => json!({"kind": "score",
            "holder": plan::PARTY, "objective": plan::flag_score(flag.as_str()), "value": 1}),
        Verb::SetState { state, value, .. } => json!({"kind": "score",
            "holder": state_holder(plan, state), "objective": plan::state_score(state.as_str()),
            "value": value}),
        Verb::ClearState { state, .. } => json!({"kind": "score",
            "holder": state_holder(plan, state), "objective": plan::state_score(state.as_str()),
            "value": state_initial(plan, state)}),
        Verb::AddState { state, amount, .. } => json!({"kind": "score",
            "holder": state_holder(plan, state), "objective": plan::state_score(state.as_str()),
            "added": amount}),
        Verb::SetCheckpoint { anchor, .. } => {
            let p = anchor_point_any(plan, anchor.as_str())?;
            json!({"kind": "score", "checkpoint": p})
        }
        _ => json!({"kind": kind}),
    };
    Some(v)
}

/// `validation/endings.json` (§6.1).
pub(super) fn endings_ledger(plan: &Plan) -> Vec<u8> {
    let c = plan.campaign;
    let flow = crate::compiler::flow::Flow::new(c);
    let story = flow.story();
    let a: Analysis = crate::compiler::ending::analyse(c, story);
    let by_path = effects_by_path(c);
    let decl = c.quest_plan.content.endings_read;
    let templates = super::packtest::ending::ending_templates(plan);
    let endings: Vec<Value> = a
        .endings
        .iter()
        .map(|v| {
            let scene = &a.scenes.scenes[v.ending.scene];
            let inputs: Vec<Value> = v
                .inputs
                .iter()
                .map(|i| {
                    let kinds: BTreeSet<&str> = i.writers.iter().map(|w| w.kind.token()).collect();
                    json!({
                        "key": i.key.id(),
                        "of": match i.key { InputKey::Flag(_) => "flag", InputKey::Datum(_) => "datum" },
                        "kind": kinds.into_iter().collect::<Vec<_>>(),
                        "seeded": i.seeded,
                        "deed": i.deed,
                        "writers": i.writers.iter().map(|w| json!({
                            "path": w.path,
                            "stage": w.stage,
                            "kind": w.kind.token(),
                            "quest": w.quest,
                            "act": w.act,
                        })).collect::<Vec<_>>(),
                    })
                })
                .collect();
            let aftermath: Vec<Value> = scene
                .aftermath
                .iter()
                .map(|x| {
                    let obs = by_path
                        .get(&x.path)
                        .and_then(|e| observation(plan, e))
                        .unwrap_or(Value::String("emission-only".to_string()));
                    json!({"path": x.path, "verb": x.verb, "observable": obs})
                })
                .collect();
            let verdict = decl.map(|d| {
                json!({
                    "seeded": v.seeded(),
                    "seeded_with_a_deed": v.seeded_deeds(),
                    "at_least": d.at_least,
                    "deeds_at_least": d.deeds_at_least,
                    "holds": d.at_least.is_none_or(|n| v.seeded() >= n as usize)
                        && d.deeds_at_least.is_none_or(|n| v.seeded_deeds() >= n as usize),
                })
            });
            let mine: Vec<Value> = templates
                .iter()
                .filter(|t| t.ending == v.ending.id)
                .map(|t| json!({
                    "template": t.name,
                    "world": format!("world/{}", t.world),
                    "observables_asserted": t.asserted,
                }))
                .collect();
            json!({
                "id": v.ending.id,
                "path": v.ending.path,
                "packtests": mine,
                "scene": scene.path,
                "host_quest": v.host_quest,
                "inputs": inputs,
                "aftermath": aftermath,
                "worlds": v.worlds.iter().map(|k| format!("world/{k}")).collect::<Vec<_>>(),
                "undecided_worlds": v.undecided.iter().map(|k| format!("world/{k}")).collect::<Vec<_>>(),
                "endings_read": verdict,
            })
        })
        .collect();
    let worlds: Vec<Value> = story
        .worlds
        .iter()
        .enumerate()
        .map(|(k, w)| {
            json!({
                "id": format!("world/{k}"),
                "selection": w.selection,
                "origin": w.origin.token(),
                "performs": w.performs.iter().collect::<Vec<_>>(),
            })
        })
        .collect();
    let targets: Vec<Value> = story
        .targets
        .iter()
        .map(|t| {
            json!({
                "scene": t.scene,
                "selection": t.selection,
                "classes": t.classes.iter().map(|(k, c)| json!({"key": k.id(), "class": c.words()})).collect::<Vec<_>>(),
                "witness": t.witness.map(|k| format!("world/{k}")),
                "reason": t.reason,
            })
        })
        .collect();
    let b = crate::compiler::ending::binding(&a, story);
    let doc = json!({
        "version": c.quest_plan.dsl_version,
        "campaign_id": c.quest_plan.campaign_id.as_str(),
        "binding": b.line(),
        "endings_read": decl.map(|d| json!({"at_least": d.at_least, "deeds_at_least": d.deeds_at_least})),
        "endings": endings,
        "electives": story.electives.iter().map(|e| json!({
            "id": e.id, "kind": e.kind, "undrivable": e.undrivable,
        })).collect::<Vec<_>>(),
        "forced": story.forced.iter().collect::<Vec<_>>(),
        "worlds": worlds,
        "world_bound": story.bound,
        "at_bound": story.at_bound,
        "targets": targets,
    });
    let mut bytes = serde_json::to_vec_pretty(&doc).expect("the endings ledger serializes");
    bytes.push(b'\n');
    bytes
}
