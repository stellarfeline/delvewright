//! spec-0136 — a body the story moves. A staged body (an NPC or an actor)
//! hops, takes a pose, turns, faces, holds an item and changes size; a
//! particle and a sound happen at one; and an effect of that class repeats
//! and staggers across a cast. Refusals: a move with no staged body
//! (`DW0942`), an NPC in the audience of an effect that harms (`DW0942`), an
//! undeclared body (`DW0112`), a number outside its bound (`DW0100`), an item
//! the game does not hold (`DW0143`) or draw there (`DW0898`), a mark nobody
//! placed (`DW0360`, at build), a pose on a
//! body that is not a mannequin and a watch handed back to a body with none
//! (`DW1101`), and a cadence the effect cannot keep (`DW1102`).

use crate::common;

use std::collections::BTreeMap;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{Campaign, Diagnostic, hop_arc, parse_campaign, validate_campaign_with};
use serde_json::{Value, json};

/// The fixture with a skinned NPC (`npc/keeper`, who watches by default), an
/// unskinned actor (`actor/moth`) and a skinned one (`actor/herald`), and
/// `effects` on the talk objective's completion.
fn campaign(effects: Vec<Value>) -> (Campaign, BTreeMap<String, Vec<u8>>) {
    let dir = common::compiler_fixtures_dir().join("souls-bonfire");
    let loaded = load_campaign_dir(&dir).unwrap();
    let mut c = parse_campaign(&loaded.raw).expect("fixture parses");
    let q = &mut c.quests.content;
    let anchor = q.waves[0].anchor.as_str().to_string();
    q.actors.push(
        serde_json::from_value(json!({"id": "actor/moth", "entity": "minecraft:bat",
            "anchor": anchor, "vulnerable": true}))
        .unwrap(),
    );
    q.actors.push(
        serde_json::from_value(json!({"id": "actor/herald", "entity": "minecraft:villager",
            "anchor": anchor, "offset": [1, 0, 0],
            "skin": {"model": "wide", "texture_id": "keeper"}}))
        .unwrap(),
    );
    let bundle = q.quests[0]
        .on_objective_complete
        .get_mut(&delvewright_dsl::ObjectiveId("obj/talk".to_string()))
        .unwrap();
    for e in effects {
        bundle.push(serde_json::from_value(e).expect("the effect parses"));
    }
    (c, loaded.inputs)
}

fn diags(c: &Campaign) -> Vec<Diagnostic> {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    validate_campaign_with(
        c,
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    )
}

fn errors(c: &Campaign) -> Vec<Diagnostic> {
    diags(c)
        .into_iter()
        .filter(|x| x.severity == delvewright_dsl::Severity::Error)
        .collect()
}

fn build(c: &Campaign, inputs: &BTreeMap<String, Vec<u8>>) -> BuildOutput {
    try_build(c, inputs).expect("emission succeeds")
}

/// The build, or the refusal it ends on as the build prints it.
fn try_build(c: &Campaign, inputs: &BTreeMap<String, Vec<u8>>) -> Result<BuildOutput, String> {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(c, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    emit::build(
        &plan,
        inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
    .map_err(|e| format!("{e:?}"))
}

/// Every function of the pack, by its name within the namespace.
fn functions(out: &BuildOutput) -> BTreeMap<String, Vec<String>> {
    out.iter()
        .filter(|(p, _)| p.starts_with("datapack/") && p.ends_with(".mcfunction"))
        .map(|(p, b)| {
            let name = p
                .rsplit_once("/function/")
                .map_or(p.as_str(), |(_, n)| n)
                .trim_end_matches(".mcfunction")
                .to_string();
            (
                name,
                String::from_utf8_lossy(b)
                    .lines()
                    .map(str::to_string)
                    .collect(),
            )
        })
        .collect()
}

fn all_lines(out: &BuildOutput) -> Vec<String> {
    functions(out).into_values().flatten().collect()
}

fn with<'a>(d: &'a [Diagnostic], code: &str) -> Vec<&'a Diagnostic> {
    d.iter().filter(|x| x.code == code).collect()
}

const KEEPER: &str = "@e[tag=dw_npc_keeper,tag=dw_npc,limit=1]";
const HERALD: &str = "@e[tag=dw_pup_herald,limit=1]";

fn npc(id: &str) -> Value {
    json!({"npc": id})
}

#[test]
fn each_move_emits_its_one_measured_command() {
    let (c, inputs) = campaign(vec![
        json!({"type": "pose", "pose": "fall-flying", "audience": npc("npc/keeper")}),
        json!({"type": "turn", "by": 20, "audience": {"actor": "actor/herald"}}),
        json!({"type": "face", "to": -90, "audience": {"actor": "actor/herald"}}),
        json!({"type": "face", "to": {"anchor": "anchor/gate", "offset": [0, 4, 0]},
               "audience": {"actor": "actor/moth"}}),
        json!({"type": "hold", "slot": "main_hand", "item": "minecraft:poppy",
               "audience": {"bodies": [npc("npc/keeper"), {"actor": "actor/herald"}]}}),
        json!({"type": "hold", "slot": "head", "audience": npc("npc/keeper")}),
        json!({"type": "resize", "scale": 1.25, "audience": {"actor": "actor/moth"}}),
        json!({"type": "particle", "particle": "minecraft:heart", "count": 5,
               "at": {"bodies": [npc("npc/keeper")], "up": 2}}),
        json!({"type": "play-sound", "sound": "minecraft:entity.villager.celebrate",
               "at": {"at": "bodies", "bodies": [{"actor": "actor/herald"}]}, "volume": 1,
               "pitch": 1.2}),
    ]);
    let e = errors(&c);
    assert!(e.is_empty(), "{e:#?}");
    let lines = all_lines(&build(&c, &inputs));
    let has = |needle: &str| {
        assert!(
            lines.iter().any(|l| l == needle),
            "missing `{needle}` among {:#?}",
            lines
                .iter()
                .filter(|l| l.contains("dw_npc_keeper,")
                    || l.contains("herald")
                    || l.contains("moth"))
                .collect::<Vec<_>>()
        );
    };
    has(&format!(
        "data merge entity {KEEPER} {{pose:\"fall_flying\"}}"
    ));
    has(&format!("execute as {HERALD} at @s run rotate @s ~20 ~"));
    has(&format!("execute as {HERALD} run rotate @s -90 0"));
    assert!(
        lines.iter().any(|l| l.starts_with(
            "execute as @e[tag=dw_pup_moth,limit=1] at @s anchored eyes run rotate @s facing "
        )),
        "a mark is faced from the body's eyes"
    );
    has(&format!(
        "item replace entity {KEEPER} weapon.mainhand with minecraft:poppy"
    ));
    // An item or a size is the body's whoever wears it: the puppet or its twin.
    has("item replace entity @e[tag=dw_actor_herald,limit=1] weapon.mainhand with minecraft:poppy");
    has(&format!(
        "item replace entity {KEEPER} armor.head with minecraft:air"
    ));
    has("execute as @e[tag=dw_actor_moth,limit=1] run attribute @s minecraft:scale base set 1.25");
    has(&format!(
        "execute at {KEEPER} run particle minecraft:heart ~ ~2 ~ 0 0 0 0 5 force @a"
    ));
    has(
        "execute at @e[tag=dw_actor_herald,limit=1] run playsound minecraft:entity.villager.celebrate master @a ~ ~ ~ 1 1.2",
    );
}

#[test]
fn a_turn_takes_a_watching_bodys_facing_and_watch_hands_it_back() {
    let (c, inputs) = campaign(vec![
        json!({"type": "turn", "by": -45, "audience": npc("npc/keeper")}),
        json!({"type": "face", "to": "watch", "audience": npc("npc/keeper")}),
    ]);
    let e = errors(&c);
    assert!(e.is_empty(), "{e:#?}");
    let lines = all_lines(&build(&c, &inputs));
    let yield_at = lines
        .iter()
        .position(|l| l == "tag @e[tag=dw_npc_keeper,tag=dw_npc] remove dw_watch")
        .expect("the turn takes the watch's tag");
    assert_eq!(
        lines[yield_at + 1],
        format!("execute as {KEEPER} at @s run rotate @s ~-45 ~"),
        "the watch yields before the turn is written"
    );
    assert!(
        lines
            .iter()
            .any(|l| l == "tag @e[tag=dw_npc_keeper,tag=dw_npc] add dw_watch"),
        "`to: watch` hands the facing back"
    );
}

/// The whole of a hop, read from the pack: the body is never released, it
/// rises and falls by exact steps that sum to nothing, and it cannot be
/// started twice.
#[test]
fn a_hop_never_releases_the_body_and_ends_where_it_began() {
    let (c, inputs) = campaign(vec![
        // The keeper's hall is three blocks high: the game's own jump would
        // put his head in its ceiling (`DW1103`), so he hops one block.
        json!({"type": "hop", "height": 1, "audience": npc("npc/keeper")}),
        json!({"type": "hop", "height": 0.5, "audience": {"actor": "actor/herald"}}),
    ]);
    let e = errors(&c);
    assert!(e.is_empty(), "{e:#?}");
    let out = build(&c, &inputs);
    let fns = functions(&out);
    let lines = all_lines(&out);

    // The start: only a body on the ground takes the hop (the fixture walks
    // the keeper once, so his start also asks that walk's latch).
    assert!(
        lines.iter().any(|l| l.starts_with("execute unless score #mrun_keeper_")
            && l.ends_with(&format!(
                " dw.sys matches 1 as {KEEPER} unless score @s dw.hop matches 1.. run function souls-bonfire:hop_start_1"
            ))),
        "{:#?}",
        lines.iter().filter(|l| l.contains("hop")).collect::<Vec<_>>()
    );
    assert!(lines.iter().any(|l| l
        == &format!(
            "execute as {HERALD} unless score @s dw.hop matches 1.. run function souls-bonfire:hop_start_2"
        )));
    assert_eq!(
        fns["hop_start_1"],
        vec![
            format!(
                "scoreboard players set @s dw.hop {}",
                hop_arc(Some(1.0)).len()
            ),
            "scoreboard players set @s dw.hop_arc 1".to_string()
        ]
    );
    assert!(
        fns["load"].contains(&"scoreboard objectives add dw.hop dummy".to_string())
            && fns["load"].contains(&"scoreboard objectives add dw.hop_arc dummy".to_string())
    );
    assert!(fns["tick"].contains(&"function souls-bonfire:hop_tick".to_string()));
    assert_eq!(
        fns["hop_tick"],
        vec![
            "execute as @e[scores={dw.hop=1..}] at @s run function souls-bonfire:hop_step"
                .to_string()
        ]
    );
    assert_eq!(
        fns["hop_step"],
        vec![
            "execute if score @s dw.hop_arc matches 1 run function souls-bonfire:hop_arc_1"
                .to_string(),
            "execute if score @s dw.hop_arc matches 2 run function souls-bonfire:hop_arc_2"
                .to_string(),
            "scoreboard players remove @s dw.hop 1".to_string(),
        ]
    );

    // The arc: one relative teleport per remaining-tick value, counting down
    // from the arc's length to 1, whose steps are the arc's own differences
    // and sum to exactly zero.
    for (i, height) in [(1usize, Some(1.0)), (2, Some(0.5))] {
        let arc = hop_arc(height);
        let body = &fns[&format!("hop_arc_{i}")];
        let mut y = 0.0_f64;
        let mut expect_remaining = arc.len();
        let mut seen = 0usize;
        for l in body {
            let rest = l
                .strip_prefix("execute if score @s dw.hop matches ")
                .unwrap_or_else(|| panic!("an arc line is a guarded step: {l}"));
            let (remaining, step) = rest
                .split_once(" run tp @s ~ ~")
                .unwrap_or_else(|| panic!("an arc line is a relative teleport: {l}"));
            let remaining: usize = remaining.parse().unwrap();
            assert!(remaining <= expect_remaining && remaining >= 1, "{l}");
            expect_remaining = remaining - 1;
            let step: f64 = step.strip_suffix(" ~").unwrap().parse().unwrap();
            y += step;
            assert!(y >= 0.0, "the body never goes under where it stood: {l}");
            assert_eq!(
                y * 1024.0,
                f64::from(arc[arc.len() - remaining]),
                "on that tick the body is at the arc's height: {l}"
            );
            seen += 1;
        }
        assert!(seen >= arc.len() - 2, "every tick that moves has its step");
        assert_eq!(y, 0.0, "the steps of arc {i} sum to exactly zero");
    }

    // Never released: no line the build emits for these bodies writes
    // `immovable` apart from the summon that sets it, and no line of the hop
    // writes `Motion`.
    let releases: Vec<&String> = lines
        .iter()
        .filter(|l| l.contains("immovable") && !l.contains("summon minecraft:mannequin"))
        .collect();
    assert!(releases.is_empty(), "{releases:#?}");
    assert!(
        fns.iter()
            .filter(|(n, _)| n.starts_with("hop_"))
            .all(|(_, b)| b
                .iter()
                .all(|l| !l.contains("Motion") && !l.contains("data "))),
        "the hop is teleports and scores, nothing else"
    );
}

#[test]
fn a_walk_and_a_hop_never_hold_one_body_together() {
    let (c, inputs) = campaign(vec![
        json!({"type": "hop", "height": 1, "audience": npc("npc/keeper")}),
        json!({"type": "move-npc", "npc": "npc/keeper", "to": {"anchor": "anchor/gate"}}),
    ]);
    let e = errors(&c);
    assert!(e.is_empty(), "{e:#?}");
    let out = build(&c, &inputs);
    let fns = functions(&out);
    // Every walk of the keeper: the fixture's own and the one added here.
    let starts: Vec<(&String, &Vec<String>)> = fns
        .iter()
        .filter(|(n, b)| {
            n.starts_with("mv_keeper") && b.iter().any(|l| l.starts_with("schedule function "))
        })
        .collect();
    assert_eq!(starts.len(), 2, "{:?}", fns.keys().collect::<Vec<_>>());
    let hop = all_lines(&out)
        .into_iter()
        .find(|l| l.ends_with("run function souls-bonfire:hop_start_1"))
        .expect("the hop's start line");
    for (start_name, start) in starts {
        let bare = start_name.strip_prefix("mv_").unwrap();
        let reset = start
            .iter()
            .position(|l| l == "scoreboard players reset @e[tag=dw_npc_keeper,tag=dw_npc] dw.hop")
            .expect("the walk's start ends a hop in flight");
        let schedule = start
            .iter()
            .position(|l| l.starts_with("schedule function "))
            .unwrap();
        assert!(reset < schedule, "before the driver's first tick");
        assert!(
            hop.contains(&format!("unless score #mrun_{bare} dw.sys matches 1 ")),
            "a body a walk carries does not hop: {hop}"
        );
    }
    assert!(hop.ends_with(&format!(
        " as {KEEPER} unless score @s dw.hop matches 1.. run function souls-bonfire:hop_start_1"
    )));
    // Perturbation: with no `hop` on the body, the walk's start is as it was.
    let (c, inputs) = campaign(vec![
        json!({"type": "move-npc", "npc": "npc/keeper", "to": {"anchor": "anchor/gate"}}),
    ]);
    let fns = functions(&build(&c, &inputs));
    assert!(
        fns.values().flatten().all(|l| !l.contains("dw.hop")),
        "a campaign with no hop carries none of its machinery"
    );
    assert!(!fns.contains_key("hop_step"));
}

#[test]
fn a_cadence_repeats_and_staggers_from_the_scheduler() {
    let cast = json!({"bodies": [npc("npc/keeper"), {"actor": "actor/herald"},
                                 {"actor": "actor/moth"}]});
    let (c, inputs) = campaign(vec![
        json!({"type": "hop", "height": 1, "audience": cast, "stagger_ticks": 3,
               "repeat": {"times": 5, "every_ticks": 14}}),
        json!({"type": "particle", "particle": "minecraft:note",
               "at": {"bodies": [npc("npc/keeper")], "up": 2.25},
               "repeat": {"times": 4, "every_ticks": 10}}),
    ]);
    let e = errors(&c);
    assert!(e.is_empty(), "{e:#?}");
    let out = build(&c, &inputs);
    let fns = functions(&out);
    let base = "cad_greet_oc_talk_0";
    let start = fns.get(base).unwrap_or_else(|| {
        panic!(
            "{:?}",
            fns.keys()
                .filter(|k| k.starts_with("cad_"))
                .collect::<Vec<_>>()
        )
    });
    assert_eq!(
        start,
        &vec![
            format!("scoreboard players set #{base}_0 dw.sys 5"),
            format!("function souls-bonfire:{base}_0"),
            format!("scoreboard players set #{base}_1 dw.sys 5"),
            format!("schedule function souls-bonfire:{base}_1 3t"),
            format!("scoreboard players set #{base}_2 dw.sys 5"),
            format!("schedule function souls-bonfire:{base}_2 6t"),
        ]
    );
    assert_eq!(
        fns[&format!("{base}_1")],
        vec![
            format!(
                "execute as {HERALD} unless score @s dw.hop matches 1.. run function souls-bonfire:hop_start_1"
            ),
            format!("scoreboard players remove #{base}_1 dw.sys 1"),
            format!(
                "execute if score #{base}_1 dw.sys matches 1.. run schedule function souls-bonfire:{base}_1 14t"
            ),
        ]
    );
    // No stagger: one group, the whole of the effect.
    let notes = "cad_greet_oc_talk_1";
    assert_eq!(
        fns[notes],
        vec![
            format!("scoreboard players set #{notes}_0 dw.sys 4"),
            format!("function souls-bonfire:{notes}_0"),
        ]
    );
    assert_eq!(
        fns[&format!("{notes}_0")][0],
        format!("execute at {KEEPER} run particle minecraft:note ~ ~2.25 ~ 0 0 0 0 1 force @a")
    );
    // The effect itself is one call.
    let calls: Vec<String> = all_lines(&out)
        .into_iter()
        .filter(|l| l.ends_with(&format!("function souls-bonfire:{base}")))
        .collect();
    assert_eq!(calls.len(), 1, "{calls:#?}");
}

#[test]
fn a_move_names_a_staged_body_the_campaign_declares() {
    for (effect, pointer) in [
        (json!({"type": "hop"}), "/audience"),
        (json!({"type": "hop", "audience": "party"}), "/audience"),
        (
            json!({"type": "pose", "pose": "sleeping", "audience": {"wave": "wave/guards"}}),
            "/audience",
        ),
    ] {
        let (c, _) = campaign(vec![effect.clone()]);
        let d = diags(&c);
        let hit = with(&d, "DW0942");
        assert_eq!(hit.len(), 1, "{effect}: {d:#?}");
        assert!(hit[0].path.ends_with(pointer), "{}", hit[0].path);
        assert!(
            hit[0].message.contains("This one names none"),
            "{}",
            hit[0].message
        );
    }
    for (effect, pointer) in [
        (
            json!({"type": "hop", "audience": npc("npc/nobody")}),
            "/audience/npc",
        ),
        (
            json!({"type": "hop", "audience": {"bodies": [npc("npc/keeper"),
                {"actor": "actor/nobody"}]}}),
            "/audience/bodies/1/actor",
        ),
        (
            json!({"type": "particle", "particle": "minecraft:heart",
                   "at": {"bodies": [npc("npc/nobody")]}}),
            "/at/bodies/0/npc",
        ),
        (
            json!({"type": "play-sound", "sound": "minecraft:entity.villager.celebrate",
                   "at": {"at": "bodies", "bodies": [{"actor": "actor/nobody"}]}}),
            "/at/bodies/0/actor",
        ),
    ] {
        let (c, _) = campaign(vec![effect.clone()]);
        let d = diags(&c);
        let hit = with(&d, "DW0112");
        assert_eq!(hit.len(), 1, "{effect}: {d:#?}");
        assert!(hit[0].path.ends_with(pointer), "{}", hit[0].path);
    }
    // An NPC is moved, never harmed.
    let (c, _) = campaign(vec![json!({"type": "give-effect",
        "effect": "minecraft:glowing", "seconds": 5, "audience": npc("npc/keeper")})]);
    let d = diags(&c);
    assert!(
        with(&d, "DW0942")
            .iter()
            .any(|x| x.message.contains("never harmed")),
        "{d:#?}"
    );
}

#[test]
fn a_number_outside_its_bound_and_a_misfit_item_are_refused() {
    for (effect, code, pointer) in [
        (json!({"type": "hop", "height": 9}), "DW0100", "/height"),
        (json!({"type": "turn", "by": 0}), "DW0100", "/by"),
        (json!({"type": "turn", "by": 270}), "DW0100", "/by"),
        (json!({"type": "face", "to": 400}), "DW0100", "/to"),
        (json!({"type": "resize", "scale": 17}), "DW0100", "/scale"),
        (
            json!({"type": "hold", "slot": "main_hand", "item": "minecraft:not_an_item"}),
            "DW0143",
            "/item",
        ),
        // A mannequin draws no saddle.
        (
            json!({"type": "hold", "slot": "saddle", "item": "minecraft:saddle"}),
            "DW0898",
            "/slot",
        ),
    ] {
        let mut effect = effect;
        effect["audience"] = npc("npc/keeper");
        let (c, _) = campaign(vec![effect.clone()]);
        let d = diags(&c);
        let hit = with(&d, code);
        assert_eq!(hit.len(), 1, "{effect}: {d:#?}");
        assert!(hit[0].path.ends_with(pointer), "{}", hit[0].path);
    }
}

/// A mark's anchor is resolved at build, with every other effect anchor.
#[test]
fn a_face_at_a_mark_nobody_placed_is_dw0360() {
    let (c, inputs) = campaign(vec![json!({"type": "face",
        "to": {"anchor": "anchor/nowhere"}, "audience": npc("npc/keeper")})]);
    let refusal = try_build(&c, &inputs).expect_err("the build refuses the mark");
    assert!(
        refusal.contains("DW0360") && refusal.contains("anchor/nowhere"),
        "{refusal}"
    );
    // Perturbation: the same face at a placed anchor builds.
    let (c, inputs) = campaign(vec![json!({"type": "face",
        "to": {"anchor": "anchor/gate"}, "audience": npc("npc/keeper")})]);
    assert!(try_build(&c, &inputs).is_ok());
}

#[test]
fn dw1101_a_move_its_body_cannot_make() {
    // `actor/moth` is a bat: no mannequin, no pose.
    let (c, _) = campaign(vec![json!({"type": "pose", "pose": "crouching",
        "audience": {"bodies": [npc("npc/keeper"), {"actor": "actor/moth"}]}})]);
    let d = diags(&c);
    let hit = with(&d, "DW1101");
    assert_eq!(hit.len(), 1, "{d:#?}");
    assert!(hit[0].message.contains("actor/moth") && hit[0].message.contains("minecraft:bat"));
    // Perturbation: the skinned bodies alone pass.
    let (c, _) = campaign(vec![json!({"type": "pose", "pose": "crouching",
        "audience": {"bodies": [npc("npc/keeper"), {"actor": "actor/herald"}]}})]);
    assert!(with(&diags(&c), "DW1101").is_empty());

    // `actor/herald` declares no watch.
    let (c, _) = campaign(vec![json!({"type": "face", "to": "watch",
        "audience": {"actor": "actor/herald"}})]);
    let d = diags(&c);
    let hit = with(&d, "DW1101");
    assert_eq!(hit.len(), 1, "{d:#?}");
    assert!(hit[0].message.contains("watches nobody"));
}

#[test]
fn dw1102_a_cadence_the_effect_cannot_keep() {
    let cast = json!({"bodies": [npc("npc/keeper"), {"actor": "actor/herald"}]});
    for (effect, pointer, says) in [
        // A verb that happens once.
        (
            json!({"type": "set-flag", "flag": "flag/summoned",
                   "repeat": {"times": 3, "every_ticks": 5}}),
            "/repeat",
            "happens once",
        ),
        (
            json!({"type": "firework", "at": {"anchor": "anchor/gate"},
                   "explosions": [{"shape": "small_ball", "colors": ["#ffd700"]}],
                   "stagger_ticks": 2}),
            "/stagger_ticks",
            "happens once",
        ),
        // A stagger with nobody to stagger.
        (
            json!({"type": "hop", "audience": npc("npc/keeper"), "stagger_ticks": 2}),
            "/stagger_ticks",
            "at least two",
        ),
        // A hop asked again before it has landed.
        (
            json!({"type": "hop", "audience": cast,
                   "repeat": {"times": 3, "every_ticks": 11}}),
            "/repeat/every_ticks",
            "in the air for 12",
        ),
        // A size no client is told.
        (
            json!({"type": "resize", "scale": 1.2, "audience": cast,
                   "repeat": {"times": 3, "every_ticks": 1}}),
            "/repeat/every_ticks",
            "every second tick",
        ),
        // No acting player in the scheduler.
        (
            json!({"type": "particle", "particle": "minecraft:heart", "at": "players",
                   "audience": "actor", "repeat": {"times": 3, "every_ticks": 5}}),
            "/audience",
            "no acting player",
        ),
    ] {
        let (c, _) = campaign(vec![effect.clone()]);
        let d = diags(&c);
        let hit = with(&d, "DW1102");
        assert_eq!(hit.len(), 1, "{effect}: {d:#?}");
        assert!(hit[0].path.ends_with(pointer), "{}", hit[0].path);
        assert!(hit[0].message.contains(says), "{}", hit[0].message);
    }
    // Perturbation: the same hop at its own length, and a size held two
    // ticks, are kept.
    let (c, _) = campaign(vec![
        json!({"type": "hop", "audience": cast, "stagger_ticks": 2,
               "repeat": {"times": 3, "every_ticks": 12}}),
        json!({"type": "resize", "scale": 1.2, "audience": cast,
               "repeat": {"times": 3, "every_ticks": 2}}),
    ]);
    let d = diags(&c);
    assert!(with(&d, "DW1102").is_empty(), "{d:#?}");
    // A bound of the cadence itself is the schema's.
    let (c, _) = campaign(vec![json!({"type": "hop", "audience": cast,
        "repeat": {"times": 0, "every_ticks": 12}})]);
    assert_eq!(with(&diags(&c), "DW0100").len(), 1);
}

// ---------------------------------------------------------------------------
// DW1103 — a hop passes through no block
// ---------------------------------------------------------------------------

use delvec::compiler::blockstate::{BlockMap, BlockState};
use delvec::compiler::hop::{self, Hop};

/// A mannequin placed at `[10, 64, 10]`, hopping to `peak`.
fn mannequin_hop(peak: f64) -> Hop {
    Hop {
        path: "/content/quests/0/on_complete/0".to_string(),
        body: "npc `npc/keeper`".to_string(),
        cell: [10, 64, 10],
        width: 0.6,
        height: 1.8,
        peak,
    }
}

fn stone_at(cells: &[[i32; 3]]) -> BlockMap {
    let mut m = BlockMap::new();
    for c in cells {
        m.insert(*c, BlockState::new("minecraft:stone"));
    }
    m
}

/// The rule, asked of a room built for it: the game's jump carries a
/// 1.8-block body's top to y 67.05, so the cells at y 65, 66 and 67 over its
/// feet are the arc's, and the cell at y 68 is not.
#[test]
fn dw1103_a_hop_through_a_block_is_refused() {
    let peak = f64::from(hop_arc(None).into_iter().max().unwrap()) / 1024.0;
    let jump = mannequin_hop(peak);
    assert_eq!(jump.top_cell(), 67);
    assert_eq!(
        jump.swept(),
        vec![[10, 65, 10], [10, 66, 10], [10, 67, 10]],
        "one column, from over the feet's cell to the top's"
    );

    // A beam where the head arrives.
    let (gate, findings) = hop::judge(&[jump.clone()], &stone_at(&[[10, 67, 10]]));
    assert_eq!((gate.bodies, gate.cells, gate.refused), (1, 3, 1));
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].code, "DW1103");
    assert!(
        findings[0].message.contains("npc `npc/keeper`")
            && findings[0].message.contains("[10, 67, 10]")
            && findings[0].message.contains("3 cell(s)"),
        "{}",
        findings[0].message
    );

    // Perturbations, each of which only this rule could tell apart: the same
    // beam one cell higher, one column over, or under the feet is clear; so
    // is a lower hop under the same beam.
    for clear in [[10, 68, 10], [11, 66, 10], [10, 64, 10], [10, 63, 10]] {
        let (gate, findings) = hop::judge(&[jump.clone()], &stone_at(&[clear]));
        assert!(findings.is_empty(), "{clear:?}: {findings:?}");
        assert_eq!(gate.cells, 3, "the sweep still looked");
    }
    let low = mannequin_hop(0.25);
    assert_eq!(low.top_cell(), 66);
    assert!(hop::judge(&[low], &stone_at(&[[10, 67, 10]])).1.is_empty());

    // A block a body passes — a torch — is no ceiling.
    let mut torch = BlockMap::new();
    torch.insert([10, 66, 10], BlockState::new("minecraft:torch"));
    assert!(hop::judge(&[jump.clone()], &torch).1.is_empty());

    // A body wider than a block sweeps the columns its width covers.
    let wide = Hop { width: 1.4, ..jump };
    assert_eq!(
        wide.swept().len(),
        27,
        "three columns by three, three cells each"
    );
    assert_eq!(
        hop::judge(&[wide], &stone_at(&[[11, 66, 10]])).1.len(),
        1,
        "the neighbouring column is the wide body's"
    );
}

/// The rule through the build, at the body's placed cell. The keeper stands
/// at `[11, 65, 12]` in a hall three blocks high: the game's own jump carries
/// his top to y 68.05, into the ceiling at `[11, 68, 12]`, and the build
/// says so; a hop of one block, whose top stays at y 67.8, builds.
#[test]
fn dw1103_is_asked_at_the_bodys_placed_cell_by_the_build() {
    let (c, inputs) = campaign(vec![json!({"type": "hop", "audience": npc("npc/keeper")})]);
    let e = errors(&c);
    assert!(e.is_empty(), "validation has no world to sweep: {e:#?}");
    let refusal = try_build(&c, &inputs).expect_err("the build refuses the hop");
    assert!(
        refusal.contains("DW1103")
            && refusal.contains("npc `npc/keeper`")
            && refusal.contains("[11, 68, 12]"),
        "{refusal}"
    );
    // Perturbation: the same hop lowered under the ceiling.
    let (c, inputs) = campaign(vec![
        json!({"type": "hop", "height": 1, "audience": npc("npc/keeper")}),
    ]);
    assert!(
        try_build(&c, &inputs).is_ok(),
        "a one-block hop clears the hall"
    );
}

// ---------------------------------------------------------------------------
// The running-server witness: a declared hop moves its body and returns it
// ---------------------------------------------------------------------------

/// The generated PackTest of a hop starts it by the very line the pack
/// carries, walks it by the pack's own driver once per tick of the arc, and
/// reads the body's height three times: counting after the start, the arc's
/// peak above where it stood on the peak tick, and back on the height it
/// left, counting 0, after the last.
#[test]
fn every_declared_hop_has_its_running_server_witness() {
    let (c, inputs) = campaign(vec![
        json!({"type": "hop", "height": 1, "audience": {"bodies": [npc("npc/keeper"),
            {"actor": "actor/herald"}]}, "stagger_ticks": 2}),
        json!({"type": "hop", "height": 0.5, "audience": {"actor": "actor/moth"}}),
    ]);
    let e = errors(&c);
    assert!(e.is_empty(), "{e:#?}");
    let out = build(&c, &inputs);
    let template = |n: usize| -> Vec<String> {
        let path = format!("packtest-datapack/data/souls-bonfire/test/hop_{n}.mcfunction");
        let (_, bytes) = out
            .iter()
            .find(|(p, _)| **p == path)
            .unwrap_or_else(|| panic!("no {path}"));
        String::from_utf8_lossy(bytes)
            .lines()
            .map(str::to_string)
            .collect()
    };
    assert!(
        !out.iter()
            .any(|(p, _)| p.ends_with("/test/hop_2.mcfunction")),
        "one template per declared hop"
    );
    let pack = all_lines(&out);
    for (n, height, body) in [
        (0usize, 1.0, KEEPER.to_string()),
        (1, 0.5, "@e[tag=dw_pup_moth,limit=1]".to_string()),
    ] {
        let arc = hop_arc(Some(height));
        let peak = *arc.iter().max().unwrap();
        let peak_tick = arc.iter().position(|h| *h == peak).unwrap() + 1;
        let t = template(n);
        // The start is the pack's own line for that body, cadence and all
        // other bodies set aside.
        let start = t
            .iter()
            .position(|l| l.contains(&format!("as {body} unless score @s dw.hop matches 1..")))
            .unwrap_or_else(|| panic!("hop_{n} starts its body: {t:#?}"));
        assert!(
            pack.contains(&t[start]),
            "the start line is one the pack itself carries: {}",
            t[start]
        );
        let ticks: Vec<usize> = t
            .iter()
            .enumerate()
            .filter(|(_, l)| *l == "function souls-bonfire:hop_tick")
            .map(|(i, _)| i)
            .collect();
        assert_eq!(
            ticks.len(),
            arc.len(),
            "the driver, once per tick of the arc"
        );
        let asserts: Vec<(usize, &String)> = t
            .iter()
            .enumerate()
            .filter(|(_, l)| l.starts_with("assert score "))
            .collect();
        assert_eq!(
            asserts.iter().map(|(_, l)| l.as_str()).collect::<Vec<_>>(),
            vec![
                format!("assert score #hopt{n} dw.sys matches 1"),
                format!("assert score #hopy{n} dw.sys matches {peak}"),
                format!("assert score #hopy{n} dw.sys matches 0"),
                format!("assert score #hopl{n} dw.sys matches 1"),
            ]
        );
        // Taken before any tick; at its peak after exactly the peak tick's
        // call; home after the last.
        assert!(asserts[0].0 > start && asserts[0].0 < ticks[0]);
        assert!(asserts[1].0 > ticks[peak_tick - 1] && asserts[1].0 < ticks[peak_tick]);
        assert!(asserts[2].0 > *ticks.last().unwrap());
        assert!(
            t.contains(&format!(
                "execute as {body} if score @s dw.hop matches {} run scoreboard players set #hopt{n} dw.sys 1",
                arc.len()
            )) && t.contains(&format!(
                "execute as {body} if score @s dw.hop matches 0 run scoreboard players set #hopl{n} dw.sys 1"
            ))
        );
    }
    // The keeper's start asks his walk's latch, so the template clears it.
    assert!(
        template(0)
            .iter()
            .any(|l| l.starts_with("scoreboard players set #mrun_keeper_")
                && l.ends_with(" dw.sys 0"))
    );
}
