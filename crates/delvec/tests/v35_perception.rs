//! spec-0085, a perception bundle: what the envelope's `audience` and `in`
//! emit, how a timeline keeps its actor, what a `particle` and a listener-frame
//! sound emit, and the four refusals — read off real builds and real
//! validations, never recalled.
//!
//! What each test pins, by the spec's acceptance criterion:
//! * 1 — `the_surface_is_exported`;
//! * 2 — `the_default_audience_is_the_roots`;
//! * 3 — `a_timeline_keeps_its_actor`;
//! * 4 — `every_verb_is_classified_by_what_the_emitter_does`,
//!   `an_audience_on_a_party_fact_is_dw0942`, `an_actor_with_no_actor_is_dw0503`;
//! * 5 — `a_particle_is_forced_in_both_forms`, `an_unknown_or_optioned_particle_is_dw0941`,
//!   `the_particle_registry_states_options_per_entry`;
//! * 6 — `a_sound_stands_in_the_listeners_frame`;
//! * 7 — `a_sight_grant_that_ends_under_a_camera_is_dw0944`;
//! * 8, 9 — `a_blinding_at_the_rim_is_dw0943` and its perturbations,
//!   `the_reach_is_judged_over_the_lethality_free_world`,
//!   `the_binding_line_prints_zeroes`.

mod common;

use std::collections::BTreeMap;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign, validate_campaign_with};
use serde_json::{Value, json};

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

/// hello-world's quests document, edited by `f`.
fn quests(f: impl FnOnce(&mut Value)) -> String {
    common::patch_doc(&hw("quests.json"), f)
}

/// hello-world with `quests` as its quests document.
fn campaign(quests: &str) -> Campaign {
    let raw = RawCampaign {
        world: hw("world.json"),
        npcs: hw("npcs.json"),
        classes: hw("classes.json"),
        quest_plan: hw("quest-plan.json"),
        quests: quests.to_string(),
        dialogue: hw("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    };
    let mut c = parse_campaign(&raw).expect("campaign parses");
    delvewright_dsl::tag_translatables(&mut c);
    c
}

fn prefabs() -> PrefabRegistry {
    PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap()
}

fn codes(c: &Campaign) -> Vec<(String, String)> {
    validate_campaign_with(
        c,
        &FullItemRegistry::v1_21_11(),
        &prefabs(),
        &FullEntityRegistry::v1_21_11(),
    )
    .into_iter()
    .filter(|d| d.severity == delvewright_dsl::Severity::Error)
    .map(|d| (d.code.to_string(), d.message))
    .collect()
}

fn has(c: &Campaign, code: &str) -> bool {
    codes(c).iter().any(|(k, _)| k == code)
}

fn structures(plan: &Plan) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                out.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    out
}

/// Validate clean, then build.
fn build(c: &Campaign) -> BuildOutput {
    let errs = codes(c);
    assert!(errs.is_empty(), "the campaign validates clean: {errs:#?}");
    let prefabs = prefabs();
    let plan = Plan::build(c, &prefabs).expect("plan builds");
    emit::build(
        &plan,
        &BTreeMap::new(),
        &structures(&plan),
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
    .expect("builds")
}

fn function(out: &BuildOutput, name: &str) -> String {
    let path = format!("datapack/data/hello-world/function/{name}.mcfunction");
    String::from_utf8(
        out.get(&path)
            .unwrap_or_else(|| panic!("{path} is emitted"))
            .clone(),
    )
    .unwrap()
}

/// hello-world with `effects` appended to `obj/talk`'s completion — a `Party`
/// root (the completing player is `@s`, the party is `@a`).
fn at_talk(effects: Vec<Value>) -> Campaign {
    campaign(&quests(|d| {
        common::objective_effects(d, 0, "obj/talk").extend(effects);
    }))
}

/// hello-world with one trigger on `anchor/exit` carrying `effects`.
fn with_trigger(on: Value, audience: Option<&str>, effects: Vec<Value>) -> Campaign {
    campaign(&quests(|d| {
        let mut t = json!({
            "id": "trigger/by-the-door",
            "at": "anchor/exit",
            "on": on,
            "effects": effects,
        });
        if let Some(a) = audience {
            t["audience"] = json!(a);
        }
        d["content"]["triggers"] = json!([t]);
    }))
}

fn narrate(audience: Option<&str>) -> Value {
    let mut v = json!({"type": "narrate", "text": "The hall answers."});
    if let Some(a) = audience {
        v["audience"] = json!(a);
    }
    v
}

// ---------------------------------------------------------------------------
// 1 — the surface
// ---------------------------------------------------------------------------

#[test]
fn the_surface_is_exported() {
    let schema = delvewright_dsl::stage_schema(delvewright_dsl::Stage::Quests);
    let defs = &schema["$defs"];
    let q = &defs["QuestEffect"];
    assert_eq!(
        q["properties"]["in"]["anyOf"][0]["$ref"], "#/$defs/StealthZone",
        "`in` is an envelope field"
    );
    assert!(
        q["properties"]["audience"].is_object(),
        "`audience` is an envelope field"
    );
    let audience: Vec<&str> = defs["EffectAudience"]["oneOf"]
        .as_array()
        .or(defs["EffectAudience"]["enum"].as_array())
        .map(|a| {
            a.iter()
                .filter_map(|b| b["const"].as_str().or(b.as_str()))
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(audience, ["party", "actor"]);
    let branches = q["oneOf"].as_array().unwrap();
    // The union's size is stated once, by `v29_firework`'s
    // `the_effect_union_names_forty_four_verbs`.
    let branch = |t: &str| {
        branches
            .iter()
            .find(|b| b["properties"]["type"]["const"] == t)
            .unwrap_or_else(|| panic!("`{t}` is a verb"))
    };
    for t in ["give-effect", "clear-effect", "damage-players"] {
        assert!(
            branch(t)["properties"].get("in").is_none(),
            "`{t}` carries no `in` of its own"
        );
    }
    let p = &branch("particle")["properties"];
    for f in ["particle", "at", "count", "spread", "speed"] {
        assert!(p.get(f).is_some(), "`particle` has `{f}`");
    }
    assert_eq!(p["count"]["minimum"], 1);
    let at = serde_json::to_string(&defs["ParticleAt"]).unwrap();
    assert!(at.contains("#/$defs/Mark") && at.contains("PlayersKeyword"));
    assert_eq!(defs["PlayersKeyword"]["oneOf"][0]["const"], "players");
    let players = defs["SoundAt"]["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["properties"]["at"]["const"] == "players")
        .expect("SoundAt has players");
    assert!(
        players["properties"].get("offset").is_some(),
        "`players` has `offset`"
    );
}

// ---------------------------------------------------------------------------
// 2 — the default audience is the root's
// ---------------------------------------------------------------------------

/// The default at each kind of site this fixture can stand up — a quest
/// completion (`Party`, `@a`), a `presser` trigger (`@s`) and a polled `party`
/// trigger (`@a`, no actor) — and that stating the root's own answer moves no
/// byte. Two builds are byte-identical.
#[test]
fn the_default_audience_is_the_roots() {
    let plain = build(&at_talk(vec![narrate(None)]));
    let talk = function(&plain, "complete_o_talk");
    assert!(
        talk.contains("tellraw @a "),
        "a quest completion addresses the party: {talk}"
    );
    let stated = build(&at_talk(vec![narrate(Some("party"))]));
    assert_eq!(
        plain, stated,
        "stating `party` where it is the default moves no byte"
    );
    assert_eq!(
        plain,
        build(&at_talk(vec![narrate(None)])),
        "two builds are byte-identical"
    );
    let actor = build(&at_talk(vec![narrate(Some("actor"))]));
    assert!(function(&actor, "complete_o_talk").contains("tellraw @s "));

    let presser = build(&with_trigger(
        json!({"on": "use"}),
        Some("presser"),
        vec![narrate(None)],
    ));
    let t = function(&presser, "trig_by_the_door");
    assert!(
        t.contains("tellraw @s "),
        "a presser trigger addresses its presser: {t}"
    );
    let presser_party = build(&with_trigger(
        json!({"on": "use"}),
        Some("presser"),
        vec![narrate(Some("party"))],
    ));
    assert!(function(&presser_party, "trig_by_the_door").contains("tellraw @a "));
    let polled = build(&with_trigger(
        json!({"on": "use"}),
        None,
        vec![narrate(None)],
    ));
    assert!(function(&polled, "trig_by_the_door").contains("tellraw @a "));
}

// ---------------------------------------------------------------------------
// 3 — a timeline keeps its actor
// ---------------------------------------------------------------------------

fn timeline(audience: Option<&str>) -> Value {
    json!({"type": "sequence", "steps": [
        {"at_ticks": 0, "effects": [narrate(audience)]},
        {"at_ticks": 20, "effects": [narrate(audience)]}
    ]})
}

#[test]
fn a_timeline_keeps_its_actor() {
    let out = build(&at_talk(vec![timeline(Some("actor"))]));
    let base = "seq_open_the_door_oc_talk_0";
    let tag = format!("dw_{base}");
    let start = function(&out, base);
    assert!(start.starts_with(&format!("tag @s add {tag}\n")), "{start}");
    assert!(start.contains(&format!("schedule function hello-world:{base}_1_as 20t")));
    for i in 0..2 {
        let d = function(&out, &format!("{base}_{i}_as"));
        assert_eq!(
            d.trim(),
            format!("execute as @a[tag={tag}] at @s run function hello-world:{base}_{i}")
        );
        assert!(function(&out, &format!("{base}_{i}")).contains("tellraw @s "));
    }
    assert!(function(&out, &format!("{base}_1")).ends_with(&format!("tag @s remove {tag}\n")));
    assert!(!function(&out, &format!("{base}_0")).contains("tag @s remove"));

    // The same timeline under a polled `party` trigger: nobody to carry.
    let polled = build(&with_trigger(
        json!({"on": "use"}),
        None,
        vec![timeline(None)],
    ));
    let names: Vec<&String> = polled
        .keys()
        .filter(|k| k.contains("/function/seq_"))
        .collect();
    assert!(!names.is_empty(), "the timeline is emitted");
    for k in names {
        let body = String::from_utf8(polled[k].clone()).unwrap();
        assert!(
            !body.contains("dw_seq_"),
            "no tag under a polled root: {k}: {body}"
        );
        assert!(
            !k.ends_with("_as.mcfunction"),
            "no dispatch under a polled root: {k}"
        );
        assert!(!body.contains("tellraw @s"), "{k}: {body}");
    }
}

// ---------------------------------------------------------------------------
// 4 — the classification, DW0942, DW0503's fourth shape
// ---------------------------------------------------------------------------

/// One minimal instance per verb, held to the variant set of the exported
/// schema — a fortieth verb reds this test until it is answered here and by
/// `Verb::addresses_players`.
fn samples() -> Vec<(&'static str, Value)> {
    vec![
        (
            "open-gate",
            json!({"type":"open-gate","anchor":"anchor/door"}),
        ),
        (
            "close-gate",
            json!({"type":"close-gate","anchor":"anchor/door"}),
        ),
        ("campaign-complete", json!({"type":"campaign-complete"})),
        (
            "give-item",
            json!({"type":"give-item","item":"minecraft:bread","count":1}),
        ),
        ("set-flag", json!({"type":"set-flag","flag":"flag/heard"})),
        (
            "set-state",
            json!({"type":"set-state","state":"state/purse","value":1}),
        ),
        (
            "add-state",
            json!({"type":"add-state","state":"state/purse","amount":1}),
        ),
        (
            "drop-stake",
            json!({"type":"drop-stake","stake":"stake/purse"}),
        ),
        (
            "clear-state",
            json!({"type":"clear-state","state":"state/purse"}),
        ),
        (
            "spawn-wave",
            json!({"type":"spawn-wave","wave":"wave/muster"}),
        ),
        (
            "narrate",
            json!({"type":"narrate","text":"The hall answers."}),
        ),
        (
            "set-block",
            json!({"type":"set-block","anchor":"anchor/exit","block":"minecraft:stone"}),
        ),
        (
            "fill-region",
            json!({"type":"fill-region","region":{"anchor":"anchor/exit","extent":[1,0,1]},"block":"minecraft:stone"}),
        ),
        (
            "clear-region",
            json!({"type":"clear-region","region":{"anchor":"anchor/exit","extent":[1,0,1]}}),
        ),
        (
            "open-way",
            json!({"type":"open-way","piece":"prefab/span","way":"gap"}),
        ),
        (
            "despawn-npc",
            json!({"type":"despawn-npc","npc":"npc/keeper"}),
        ),
        (
            "move-npc",
            json!({"type":"move-npc","npc":"npc/keeper","to":{"anchor":"anchor/exit"}}),
        ),
        (
            "cutscene",
            json!({"type":"cutscene","seconds":2,"path":[{"anchor":"anchor/exit","offset":[0,2,0]}]}),
        ),
        ("set-time", json!({"type":"set-time","time":"dusk"})),
        (
            "set-weather",
            json!({"type":"set-weather","weather":"rain"}),
        ),
        (
            "play-sound",
            json!({"type":"play-sound","sound":"minecraft:block.bell.use"}),
        ),
        (
            "damage-players",
            json!({"type":"damage-players","amount":2}),
        ),
        (
            "set-checkpoint",
            json!({"type":"set-checkpoint","anchor":"anchor/keeper-stand"}),
        ),
        (
            "bonfire",
            json!({"type":"bonfire","anchor":"anchor/keeper-stand"}),
        ),
        (
            "begin-stealth",
            json!({"type":"begin-stealth","zones":[{"anchor":"anchor/exit","extent":[1,1,1]}]}),
        ),
        ("end-stealth", json!({"type":"end-stealth"})),
        (
            "spawn-actor",
            json!({"type":"spawn-actor","actor":"actor/shade"}),
        ),
        (
            "despawn-actor",
            json!({"type":"despawn-actor","actor":"actor/shade","style":"vanish"}),
        ),
        (
            "move-actor",
            json!({"type":"move-actor","actor":"actor/shade","to":{"anchor":"anchor/exit"}}),
        ),
        (
            "unleash-actor",
            json!({"type":"unleash-actor","actor":"actor/shade"}),
        ),
        ("spawn-npc", json!({"type":"spawn-npc","npc":"npc/keeper"})),
        (
            "sequence",
            json!({"type":"sequence","steps":[{"at_ticks":0,"effects":[{"type":"set-flag","flag":"flag/heard"}]}]}),
        ),
        (
            "volley",
            json!({"type":"volley","from_anchor":"anchor/exit","kill_zone":{"anchor":"anchor/exit","extent":[1,0,1]}}),
        ),
        (
            "collapse",
            json!({"type":"collapse","region_anchor":{"anchor":"anchor/exit","extent":[1,0,1]}}),
        ),
        (
            "give-effect",
            json!({"type":"give-effect","effect":"minecraft:glowing","seconds":5}),
        ),
        (
            "clear-effect",
            json!({"type":"clear-effect","effect":"minecraft:glowing"}),
        ),
        (
            "teleport",
            json!({"type":"teleport","from":{"anchor":"anchor/exit","extent":[1,1,1]},"to":{"anchor":"anchor/keeper-stand"}}),
        ),
        (
            "firework",
            json!({"type":"firework","at":{"anchor":"anchor/exit"},"explosions":[{"shape":"star","colors":["#ffd700"]}]}),
        ),
        // spec-0080: a biome repaint is a world fact.
        (
            "set-atmosphere",
            json!({"type":"set-atmosphere","atmosphere":null,"region":{"anchor":"anchor/exit","extent":[1,1,1]}}),
        ),
        // spec-0082: an assembly is a world object.
        (
            "spawn-assembly",
            json!({"type":"spawn-assembly","assembly":"assembly/limb"}),
        ),
        (
            "despawn-assembly",
            json!({"type":"despawn-assembly","assembly":"assembly/limb"}),
        ),
        (
            "play-clip",
            json!({"type":"play-clip","assembly":"assembly/limb","clip":"idle"}),
        ),
        (
            "arm-strikes",
            json!({"type":"arm-strikes","assembly":"assembly/limb"}),
        ),
        (
            "particle",
            json!({"type":"particle","particle":"minecraft:soul","at":"players"}),
        ),
        // spec-0092: a bolt is a world fact.
        (
            "lightning",
            json!({"type":"lightning","at":{"anchor":"anchor/exit","offset":[0,5,0]}}),
        ),
    ]
}

/// **The classification is the emitter's** (criterion 4): every verb, emitted
/// under a party bundle and under a solo one, differs exactly when
/// `Verb::addresses_players` says it addresses players. The sample table is
/// held to the schema's verb set in both directions, so a new verb reds here
/// until both sides answer it.
#[test]
fn every_verb_is_classified_by_what_the_emitter_does() {
    let schema = delvewright_dsl::stage_schema(delvewright_dsl::Stage::Quests);
    let declared: std::collections::BTreeSet<String> = schema["$defs"]["QuestEffect"]["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| {
            b["properties"]["type"]["const"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect();
    let table = samples();
    let answered: std::collections::BTreeSet<String> =
        table.iter().map(|(t, _)| (*t).to_string()).collect();
    assert_eq!(
        answered, declared,
        "the table answers for exactly the declared verbs"
    );

    // The plan of a campaign that declares the sample timeline, so the emitter
    // can name its function under both audiences.
    let seq = table
        .iter()
        .find(|(t, _)| *t == "sequence")
        .unwrap()
        .1
        .clone();
    let c = campaign(&quests(|d| {
        common::objective_effects(d, 0, "obj/talk").push(seq.clone());
        d["content"]["on_death"] = json!([seq.clone()]);
    }));
    let prefabs = prefabs();
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let (mut facing, mut facts) = (0, 0);
    for (tag, v) in &table {
        let eff: delvewright_dsl::QuestEffect = serde_json::from_value(v.clone()).unwrap();
        // A timeline's call names the function of the timeline started under
        // that audience (spec-0085 §3.2): the name is the timeline's key, not an
        // address, so it is read as one token.
        let norm = |v: Vec<String>| -> Vec<String> {
            v.into_iter()
                .map(|l| {
                    if l.starts_with("function hello-world:seq_") {
                        "function <timeline>".to_string()
                    } else {
                        l
                    }
                })
                .collect()
        };
        let party = norm(emit::effect_commands(&plan, &eff, false));
        let solo = norm(emit::effect_commands(&plan, &eff, true));
        let moves = party != solo;
        assert_eq!(
            moves,
            eff.addresses_players(),
            "`{tag}`: emitted under @a and @s the commands {} differ, and \
             `addresses_players` says {}: {party:?} / {solo:?}",
            if moves { "do" } else { "do not" },
            eff.addresses_players()
        );
        if moves {
            facing += 1;
        } else {
            facts += 1;
        }
    }
    assert_eq!(facing + facts, declared.len());
    assert!(
        facing > 0 && facts > 0,
        "both answers occur: {facing} / {facts}"
    );
    println!(
        "classification binding: {facing} player-facing verb(s), {facts} party fact(s), of {}",
        declared.len()
    );
}

#[test]
fn an_audience_on_a_party_fact_is_dw0942() {
    let mut refused = 0;
    for (tag, v) in samples() {
        let eff: delvewright_dsl::QuestEffect = serde_json::from_value(v.clone()).unwrap();
        for field in ["audience", "in"] {
            let mut w = v.clone();
            w[field] = if field == "audience" {
                json!("party")
            } else {
                json!({"anchor": "anchor/exit", "extent": [2, 1, 2]})
            };
            let c = at_talk(vec![w]);
            let got = codes(&c)
                .into_iter()
                .any(|(k, m)| k == "DW0942" && m.contains(&format!("declares `{field}`")));
            assert_eq!(
                got,
                !eff.addresses_players(),
                "`{tag}` with `{field}`: DW0942 {}",
                if got { "raised" } else { "not raised" }
            );
            refused += usize::from(got);
        }
    }
    assert!(refused > 0);
}

#[test]
fn an_actor_with_no_actor_is_dw0503() {
    let polled = with_trigger(json!({"on": "use"}), None, vec![narrate(Some("actor"))]);
    assert!(has(&polled, "DW0503"), "{:#?}", codes(&polled));
    let in_a_timeline = with_trigger(json!({"on": "use"}), None, vec![timeline(Some("actor"))]);
    assert!(has(&in_a_timeline, "DW0503"));
    // Where there is an actor, the same beat is admitted.
    assert!(!has(&at_talk(vec![narrate(Some("actor"))]), "DW0503"));
    assert!(!has(&at_talk(vec![timeline(Some("actor"))]), "DW0503"));
    let presser = with_trigger(
        json!({"on": "use"}),
        Some("presser"),
        vec![timeline(Some("actor"))],
    );
    assert!(!has(&presser, "DW0503"));
}

// ---------------------------------------------------------------------------
// 5 — the particle
// ---------------------------------------------------------------------------

#[test]
fn a_particle_is_forced_in_both_forms() {
    let out = build(&at_talk(vec![
        json!({"type":"particle","particle":"minecraft:elder_guardian","at":"players"}),
        json!({"type":"particle","particle":"soul","count":40,"spread":[1.0,0.5,1.0],"speed":0.02,
               "at":{"anchor":"anchor/exit","offset":[0,1,0]}}),
    ]));
    let f = function(&out, "complete_o_talk");
    assert!(
        f.contains(
            "execute as @a at @s run particle minecraft:elder_guardian ~ ~ ~ 0 0 0 0 1 force @s\n"
        ),
        "{f}"
    );
    let mark = f
        .lines()
        .find(|l| l.starts_with("particle minecraft:soul "))
        .expect("the mark form is one bare `particle` line");
    let t: Vec<&str> = mark.split(' ').collect();
    assert!(
        t[2].ends_with(".5") && t[4].ends_with(".5"),
        "the cell's centre: {mark}"
    );
    assert!(!t[3].contains('.'), "on the mark's plane: {mark}");
    assert_eq!(
        &t[5..],
        ["1", "0.5", "1", "0.02", "40", "force", "@a"],
        "{mark}"
    );
}

#[test]
fn an_unknown_or_optioned_particle_is_dw0941() {
    for id in ["minecraft:ghost_face", "minecraft:dust", "flash"] {
        let c = at_talk(vec![
            json!({"type":"particle","particle":id,"at":"players"}),
        ]);
        assert!(has(&c, "DW0941"), "`{id}` is refused: {:#?}", codes(&c));
    }
    let ok = at_talk(vec![
        json!({"type":"particle","particle":"elder_guardian","at":"players"}),
    ]);
    assert!(!has(&ok, "DW0941"));
    let zero = at_talk(vec![
        json!({"type":"particle","particle":"soul","at":"players","count":0}),
    ]);
    assert!(has(&zero, "DW0100"), "count 0 is the schema's refusal");
}

/// The registry is the jar's, and every entry states whether it takes options.
/// The spec's from-memory list of fourteen options-taking types is held inside
/// what the jar measured; the jar names four more (`dragon_breath`, `effect`,
/// `flash`, `instant_effect`).
#[test]
fn the_particle_registry_states_options_per_entry() {
    let reg = delvewright_dsl::perception::particle_registry();
    assert_eq!(
        reg.len(),
        115,
        "the pinned game registers 115 particle types"
    );
    let optioned: Vec<&str> = reg
        .iter()
        .filter(|(_, o)| **o)
        .map(|(k, _)| k.as_str())
        .collect();
    let from_memory = [
        "block",
        "block_marker",
        "dust",
        "dust_color_transition",
        "dust_pillar",
        "entity_effect",
        "falling_dust",
        "item",
        "sculk_charge",
        "shriek",
        "vibration",
        "trail",
        "tinted_leaves",
        "block_crumble",
    ];
    for id in from_memory {
        assert!(
            optioned.contains(&format!("minecraft:{id}").as_str()),
            "{id}"
        );
    }
    assert_eq!(optioned.len(), 18, "{optioned:?}");
    assert_eq!(reg.get("minecraft:elder_guardian"), Some(&false));
    let raw: Value = serde_json::from_str(
        &std::fs::read_to_string(
            common::repo_root().join("crates/dsl/data/particles-1.21.11.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(
        raw.as_object()
            .unwrap()
            .values()
            .all(|v| v["options"].is_boolean())
    );
    let prov =
        std::fs::read_to_string(common::repo_root().join("crates/delvec/data/PROVENANCE.md"))
            .unwrap();
    assert!(
        prov.contains("`particles-1.21.11.json`") && prov.contains("extract-particle-registry.py")
    );
}

// ---------------------------------------------------------------------------
// 6 — the listener's frame
// ---------------------------------------------------------------------------

#[test]
fn a_sound_stands_in_the_listeners_frame() {
    let sound = |at: Value| {
        json!({"type":"play-sound","sound":"minecraft:entity.warden.nearby_closest",
               "volume":1.0,"pitch":0.5,"at":at})
    };
    let out = build(&at_talk(vec![sound(
        json!({"at":"players","offset":[0,0,-3]}),
    )]));
    assert!(function(&out, "complete_o_talk").contains(
        "execute as @a at @s rotated ~ 0 positioned ^0 ^0 ^-3 run playsound \
         minecraft:entity.warden.nearby_closest master @s ~ ~ ~ 1 0.5\n"
    ));
    let zero = build(&at_talk(vec![sound(
        json!({"at":"players","offset":[0,0,0]}),
    )]));
    let bare = build(&at_talk(vec![sound(json!({"at":"players"}))]));
    assert_eq!(zero, bare, "a zero offset is today's line");
    assert!(function(&bare, "complete_o_talk").contains(
        "execute as @a at @s run playsound minecraft:entity.warden.nearby_closest master @s ~ ~ ~ 1 0.5\n"
    ));
}

// ---------------------------------------------------------------------------
// 7 — a sight effect outlasts the camera
// ---------------------------------------------------------------------------

fn sight_timeline(seconds: u32) -> Value {
    json!({"type":"sequence","steps":[{"at_ticks":0,"effects":[
        {"type":"give-effect","effect":"minecraft:night_vision","seconds":seconds,"hide_particles":true},
        {"type":"cutscene","seconds":10,"path":[{"anchor":"anchor/exit","offset":[0,2,0]}]}
    ]}]})
}

/// Night vision winds down over its last 200 ticks, so against a ten-second
/// shot (ticks 0..200) a grant starting at tick 0 must run to tick 400: 3 s and
/// 19 s are refused, 20 s is green. (The spec's example put the green at 15 s;
/// a 15 s grant ends at tick 300 and is visibly flickering from tick 100, under
/// the shot — see the report.)
#[test]
fn a_sight_grant_that_ends_under_a_camera_is_dw0944() {
    for (s, red) in [(3, true), (15, true), (19, true), (20, false), (25, false)] {
        let c = at_talk(vec![sight_timeline(s)]);
        assert_eq!(has(&c, "DW0944"), red, "{s} s: {:#?}", codes(&c));
    }
    // A bundle fires all at once: a timeline of one step at tick 0.
    let bundle = at_talk(vec![
        json!({"type":"give-effect","effect":"minecraft:night_vision","seconds":3}),
        json!({"type":"cutscene","seconds":10,"path":[{"anchor":"anchor/exit","offset":[0,2,0]}]}),
    ]);
    assert!(has(&bundle, "DW0944"), "{:#?}", codes(&bundle));
    // No cutscene in the timeline: not examined.
    let alone = at_talk(vec![
        json!({"type":"sequence","steps":[{"at_ticks":0,"effects":[
        {"type":"give-effect","effect":"minecraft:night_vision","seconds":3}]}]}),
    ]);
    assert!(!has(&alone, "DW0944"));
    for (id, _ticks, page) in delvewright_dsl::perception::SIGHT {
        assert!(id.starts_with("minecraft:") && !page.is_empty());
    }
    assert_eq!(
        delvewright_dsl::perception::sight_wind_down_ticks("night_vision"),
        Some(200)
    );
}

// ---------------------------------------------------------------------------
// 8, 9 — the blind reach
// ---------------------------------------------------------------------------

/// The `lethal-volume` fixture (a road ending in a magma floor the volume
/// catches), with `extra` appended to `obj/talk`'s completion, planned over its
/// edited world.
fn with_lethal_fixture<R>(
    extra: Vec<Value>,
    f: impl FnOnce(&Plan, &delvec::compiler::nav::World) -> R,
) -> R {
    let dir = common::compiler_fixtures_dir().join("lethal-volume");
    let read = |n: &str| std::fs::read_to_string(dir.join(n)).unwrap();
    let parse = |quests: String| {
        let raw = RawCampaign {
            world: read("world.json"),
            npcs: read("npcs.json"),
            classes: read("classes.json"),
            quest_plan: read("quest-plan.json"),
            quests,
            dialogue: read("dialogue.json"),
            world_edits: Some(read("world-edits.json")),
            geometry_brief: None,
            layout_graph: None,
            site_plan: None,
            detail_plan: None,
            design: None,
        };
        let mut c = parse_campaign(&raw).expect("the fixture parses");
        delvewright_dsl::tag_translatables(&mut c);
        c
    };
    // The world is the fixture's own (the replay asks `DW0943` itself, so it is
    // replayed without the grant); the plan the check reads carries the grant.
    let base = parse(read("quests.json"));
    let c = parse(common::patch_doc(&read("quests.json"), |d| {
        common::objective_effects(d, 0, "obj/talk").extend(extra);
    }));
    let prefabs = prefabs();
    let base_plan = Plan::build(&base, &prefabs).expect("plan builds");
    let s = structures(&base_plan);
    let replay = delvec::compiler::edit::replay(&base_plan, &prefabs, &s)
        .expect("the edit script replays")
        .expect("the fixture declares world-edits");
    let occ = delvec::compiler::assembled::occupancy_over(
        &replay.assembled.blocks,
        &replay.assembled.open_gates,
    );
    let world = delvec::compiler::nav::World::from_occupancy(
        occ,
        delvec::compiler::nav::Premises::of_plan(&base_plan, replay.assembled.gate_seals.clone()),
    );
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    f(&plan, &world)
}

fn grant(effect: &str, seconds: u32, anchor: &str, extent: [u32; 3]) -> Value {
    json!({"type":"give-effect","effect":effect,"seconds":seconds,"hide_particles":true,
           "in":{"anchor":anchor,"extent":extent}})
}

#[test]
fn a_blinding_at_the_rim_is_dw0943() {
    with_lethal_fixture(
        vec![grant("minecraft:blindness", 2, "anchor/exit", [2, 1, 2])],
        |plan, world| {
            let entry = plan.campaign_start().map(|(_, p)| p);
            let (b, v) = delvec::compiler::blind::check(plan, world, entry);
            let err = v.expect_err("a blinding at the rim is refused");
            assert_eq!(err.code.to_string(), "DW0943");
            assert!(
                err.message.contains("lethal/the-burn"),
                "names the volume: {}",
                err.message
            );
            assert_eq!(b.grants.len(), 1);
            let g = &b.grants[0];
            assert!(g.standing > 0 && !g.caught.is_empty(), "{g:?}");
            assert_eq!(g.reach_moves, 9, "ceil(2 × 4.317)");
            println!("{}", b.line());
        },
    );
}

#[test]
fn the_same_grant_as_nausea_is_green() {
    with_lethal_fixture(
        vec![grant("minecraft:nausea", 2, "anchor/exit", [2, 1, 2])],
        |plan, world| {
            let (b, v) =
                delvec::compiler::blind::check(plan, world, plan.campaign_start().map(|(_, p)| p));
            assert!(v.is_ok());
            assert!(b.grants.is_empty(), "nausea is not a blinding: {b:?}");
        },
    );
}

/// Away from every hazard the grant is examined and green: hello-world's keep
/// has no killing volume and no drop a body does not survive, so a blinding
/// there binds a non-zero standing set and catches nothing. (The fixture with
/// the burn is too small to hold a place nine moves clear of it; the gallery's
/// `a-blinding-at-the-rim` probe, moved to `anchor/lane-west`, is the far case
/// over a world with volumes.)
#[test]
fn a_blinding_far_from_any_hazard_is_green_and_bound() {
    let c = at_talk(vec![grant(
        "minecraft:blindness",
        2,
        "anchor/keeper-stand",
        [1, 1, 1],
    )]);
    let prefabs = prefabs();
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let world = delvec::compiler::nav::World::from_plan(&plan, &structures(&plan));
    let (b, v) =
        delvec::compiler::blind::check(&plan, &world, plan.campaign_start().map(|(_, p)| p));
    assert!(v.is_ok(), "{:?}", v.err());
    assert_eq!(b.grants.len(), 1);
    assert!(b.grants[0].standing > 0, "a non-zero standing set: {b:?}");
    assert!(b.grants[0].caught.is_empty());
    let _ = build(&c);
    println!("{}", b.line());
}

/// An `in` box over no standable cell is a declaration the bytes do not bear
/// out, refused with a count of zero. Every anchor of the fixture is a seat, so
/// the box's one cell is filled in the world handed to the judgement — the
/// geometry a box over a wall would have.
#[test]
fn a_box_over_no_standable_cell_is_refused_with_zero() {
    with_lethal_fixture(
        vec![grant("minecraft:blindness", 1, "anchor/door", [0, 0, 0])],
        |plan, world| {
            let cell = plan.point_any("anchor/door").unwrap();
            let wall: std::collections::BTreeSet<[i32; 3]> =
                [cell, [cell[0], cell[1] + 1, cell[2]]]
                    .into_iter()
                    .collect();
            let open = world.without_exclusions().with_extra_solid(&wall);
            assert!(
                !open.is_standable(cell),
                "the box's one cell is no place to stand"
            );
            let (b, v) =
                delvec::compiler::blind::judge(plan, &open, plan.campaign_start().map(|(_, p)| p));
            let err = v.expect_err("an empty standing set is refused");
            assert_eq!(err.code.to_string(), "DW0943");
            assert!(
                err.message.contains("0 standable cell(s)"),
                "{}",
                err.message
            );
            assert_eq!(b.grants[0].standing, 0);
        },
    );
}

/// **The reach is judged over the lethality-free world, and that is its
/// binding** (spec-0085 §10.8): handed the world the router walks, the rim
/// grant goes green — the vacuous shape — because the keep-out is already
/// impassable there; handed the counterfactual `check` builds, it is caught.
#[test]
fn the_reach_is_judged_over_the_lethality_free_world() {
    with_lethal_fixture(
        vec![grant("minecraft:blindness", 2, "anchor/exit", [2, 1, 2])],
        |plan, world| {
            let entry = plan.campaign_start().map(|(_, p)| p);
            let (vacuous, v) = delvec::compiler::blind::judge(plan, world, entry);
            assert!(v.is_ok(), "over the lethal-applied world nothing is caught");
            assert!(vacuous.grants[0].caught.is_empty());
            let (bound, v) = delvec::compiler::blind::check(plan, world, entry);
            assert!(v.is_err());
            assert!(!bound.grants[0].caught.is_empty());
        },
    );
    assert!((delvewright_dsl::metrics::SPRINT_SPEED_BLOCKS_PER_SECOND - 5.612).abs() < 1e-9);
    assert_eq!(delvewright_dsl::metrics::SPRINT_SPEED_PAGE, "Sprinting");
    assert_eq!(delvewright_dsl::perception::reach_moves(8, false), 45);
}

#[test]
fn the_binding_line_prints_zeroes() {
    let b = delvec::compiler::blind::BlindReach::default();
    assert_eq!(
        b.line(),
        "blind-reach binding: 0 blinding grant(s) examined; standing sets of 0 cell(s); \
         reaches of 0 move(s); 0 caught."
    );
    // …and a built campaign with a volume carries the per-grant fields.
    let dir = common::compiler_fixtures_dir().join("lethal-volume");
    let loaded = load_campaign_dir(&dir).unwrap();
    let mut c = parse_campaign(&loaded.raw).unwrap();
    delvewright_dsl::tag_translatables(&mut c);
    let out = build(&c);
    let gate: Value =
        serde_json::from_slice(&out["validation/lethal-gate.json"]).expect("the ledger is JSON");
    assert_eq!(gate["blind_reach"]["grants_examined"], 0);
}

/// **A granted sight effect outlasts every authored camera it can overlap, plus
/// its wind-down** — the general form of the findings-ledger row `isl-52`, both
/// halves in one test so the row's one carrier carries both.
///
/// * The `mitigation` half is structural: the compiler derives the area grant's
///   lease from the campaign's longest camera, so a fifteen-second cutscene
///   lifts the lease to at least 15 + 10 (night vision's wind-down) seconds.
/// * The `give-effect` half is `DW0944`: an author-chosen `seconds` that ends
///   under a camera in its own timeline is refused.
#[test]
fn a_granted_sight_effect_outlasts_every_camera_it_overlaps() {
    // The mitigation half.
    let q = quests(|d| {
        common::objective_effects(d, 0, "obj/talk").push(
            json!({"type":"cutscene","seconds":15,"path":[{"anchor":"anchor/exit","offset":[0,2,0]}]}),
        );
    });
    let mut c = campaign(&q);
    c.world.content.areas[0].mitigation = Some(delvewright_dsl::AreaMitigation::NightVision);
    let prefabs = prefabs();
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let out = emit::build(
        &plan,
        &BTreeMap::new(),
        &structures(&plan),
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
    .expect("builds");
    let tick = function(&out, "night_vision_tick");
    let lease: u32 = tick
        .split("minecraft:night_vision ")
        .nth(1)
        .and_then(|t| t.split(' ').next())
        .and_then(|n| n.parse().ok())
        .expect("the clock grants night vision with a lease");
    let wind = delvewright_dsl::perception::sight_wind_down_ticks("night_vision").unwrap() / 20;
    assert!(
        lease >= 15 + wind,
        "the lease {lease} s outlasts a 15 s camera plus {wind} s"
    );
    // The give-effect half.
    assert!(has(&at_talk(vec![sight_timeline(3)]), "DW0944"));
    assert!(!has(&at_talk(vec![sight_timeline(20)]), "DW0944"));
}
