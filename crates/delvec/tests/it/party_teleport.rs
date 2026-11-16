//! A `teleport` whose subject is the party (`"from": "party"`, DSL v0.88).
//!
//! Every member present moves, wherever they stand, each onto a seat of their
//! own. These pin what the build emits (the numbering, the seats, the cutscene
//! viewer's kept place, the respawn wait), what it refuses (`DW1141` a mark
//! with no seat for every member, `DW1142` a party carry and a crossing on
//! one step), and what the path does with one (the landing is the step's
//! transport). The live half is the generated `party_teleport_<key>`
//! PackTest, whose shape is pinned here and which the gallery runs.

use crate::common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{Campaign, DSL_VERSION, RespawnWait, parse_campaign, validate_campaign_with};
use serde_json::json;

const NS: &str = "cast-ledger";

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The keeper's door: the latch, pressed, calls the party to `to`; `extra` is
/// spliced into the leaving quest's `on_complete`, and `ask` into the asking
/// objective's completion.
fn quests(to: &str, ask: &str, extra: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "{DSL_VERSION}",
  "campaign_id": "cast-ledger",
  "stage": "quests",
  "content": {{
    "triggers": [
      {{
        "id": "trigger/door-latch",
        "at": "anchor/latch",
        "on": {{ "on": "use" }},
        "prop": {{ "block": "minecraft:stone_button[face=floor,facing=north]" }},
        "answers": {{ "done": "The latch is already tried." }},
        "effects": [
          {{ "type": "narrate", "text": "The latch rings a bell somewhere." }},
          {{ "type": "teleport", "from": "party", "to": {to} }}
        ]
      }}
    ],
    "quests": [
      {{
        "id": "quest/ask",
        "trigger": {{ "type": "campaign-start" }},
        "objectives": [ {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }} ],
        "on_objective_complete": {{ "obj/talk": [ {ask} ] }},
        "on_complete": [
          {{ "type": "narrate", "text": "The bolt slides back." }},
          {{ "type": "open-gate", "anchor": "anchor/door" }}
        ]
      }},
      {{
        "id": "quest/leave",
        "trigger": {{ "type": "quest-complete", "quest": "quest/ask" }},
        "objectives": [
          {{ "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit", "radius": 2 }}
        ],
        "on_complete": [ {extra}{{ "type": "campaign-complete" }} ]
      }}
    ]
  }}
}}"#
    )
}

static WORLD: LazyLock<String> = LazyLock::new(|| {
    common::at_dsl_version(
        r#"{
  "dsl_version": "%dsl_version%",
  "campaign_id": "cast-ledger",
  "stage": "world",
  "content": {
    "title": "The Keeper's Door",
    "theme": "A lonely keep at the edge of the moor.",
    "premise": "One locked door stands between you and the road home.",
    "seed": 20260729,
    "target_minutes": 5,
    "time": "noon",
    "horizon": { "base": "void", "attributes": { "audio/background_music": {} } },
    "weather": "clear",
    "difficulty": "normal",
    "areas": [ { "id": "area/keep", "name": "The Keep", "prefab": "prefab/hello-room" } ]
  }
}"#,
    )
});

static DIALOGUE: LazyLock<String> = LazyLock::new(|| {
    common::at_dsl_version(
        r#"{
  "dsl_version": "%dsl_version%",
  "campaign_id": "cast-ledger",
  "stage": "dialogue",
  "content": {
    "dialogues": [
      {
        "npc": "npc/keeper",
        "root": "dlg/greeting",
        "nodes": [
          {
            "id": "dlg/greeting",
            "text": "Halt. The door stays shut until the toll is paid.",
            "options": [
              { "label": "Pay what he asks.",
                "effects": [ { "type": "complete-objective", "objective": "obj/talk" } ] }
            ]
          }
        ]
      },
      {
        "npc": "npc/sleeper",
        "root": "dlg/snore",
        "nodes": [ { "id": "dlg/snore", "text": "...mm. Not my watch.", "options": [] } ]
      }
    ]
  }
}"#,
    )
});

const AT_EXIT: &str = r#"{ "anchor": "anchor/exit" }"#;
const NARRATE: &str = r#"{ "type": "narrate", "text": "Paid." }"#;
const A_CUTSCENE: &str = r#"{ "type": "cutscene", "subject": "landscape", "seconds": 2,
  "path": [ { "anchor": "anchor/exit", "offset": [-2, 2, 0] },
            { "anchor": "anchor/exit", "offset": [2, 2, 0] } ] },"#;

/// The fixture directory with `quests` written in.
fn fixture(who: &str, quests: &str) -> PathBuf {
    let dir = tmp(&format!("party-teleport-{who}"));
    for f in common::STAGE_FILES {
        std::fs::copy(
            common::compiler_fixtures_dir().join("cast-ledger").join(f),
            dir.join(f),
        )
        .unwrap();
    }
    std::fs::write(dir.join("world.json"), WORLD.as_str()).unwrap();
    std::fs::write(dir.join("dialogue.json"), DIALOGUE.as_str()).unwrap();
    std::fs::write(dir.join("quests.json"), quests).unwrap();
    dir
}

fn campaign(dir: &Path) -> Campaign {
    let loaded = load_campaign_dir(dir).unwrap();
    parse_campaign(&loaded.raw).expect("the party-teleport fixture parses")
}

/// Build `c` (from `dir`'s inputs). `Err` carries the build diagnostic.
fn try_build(dir: &Path, c: &Campaign) -> Result<BuildOutput, (String, String)> {
    let loaded = load_campaign_dir(dir).unwrap();
    let prefab_dir = common::prefabs_dir();
    let prefabs = PrefabRegistry::load_dir(&prefab_dir).unwrap();
    let diags = validate_campaign_with(
        c,
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    );
    assert!(
        diags.is_empty(),
        "the fixture must validate clean: {diags:#?}"
    );
    let plan = Plan::build(c, &prefabs)
        .map_err(|e| (e.failure.code.id().to_string(), e.failure.message.clone()))?;
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(prefab_dir.join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
    .map_err(|e| match e {
        emit::BuildFailure::Diagnostic { code, message } => (code.to_string(), message),
        other => panic!("expected a diagnostic, got {other:?}"),
    })
}

fn build(who: &str, quests: &str) -> BuildOutput {
    let dir = fixture(who, quests);
    try_build(&dir, &campaign(&dir)).expect("the fixture builds")
}

fn text(out: &BuildOutput, path: &str) -> String {
    String::from_utf8_lossy(out.get(path).unwrap_or_else(|| panic!("{path} is emitted")))
        .into_owned()
}

/// The one party teleport's generated function: `(name, body)`.
fn party_fn(out: &BuildOutput) -> (String, String) {
    let prefix = format!("datapack/data/{NS}/function/teleport_");
    let hits: Vec<(String, String)> = out
        .iter()
        .filter_map(|(p, b)| {
            let rest = p.strip_prefix(&prefix)?.strip_suffix(".mcfunction")?;
            (!rest.ends_with("_keep")).then(|| {
                (
                    format!("teleport_{rest}"),
                    String::from_utf8_lossy(b).into_owned(),
                )
            })
        })
        .collect();
    assert_eq!(hits.len(), 1, "one party teleport is declared: {hits:?}");
    hits.into_iter().next().unwrap()
}

/// The `[x, y, z]` of each `tp @a[…]` line, in order.
fn seats(body: &str) -> Vec<[f64; 3]> {
    body.lines()
        .filter(|l| l.starts_with("tp @a[tag=!dw_cutscene,scores={dw.tpp="))
        .map(|l| {
            let v: Vec<f64> = l.rsplit(' ').take(3).map(|t| t.parse().unwrap()).collect();
            [v[2], v[1], v[0]]
        })
        .collect()
}

#[test]
fn every_member_is_numbered_and_put_on_a_seat_of_their_own() {
    let out = build("seats", &quests(AT_EXIT, NARRATE, ""));
    let (name, body) = party_fn(&out);
    // Numbered: every member who has taken a class, alive, in `@a`'s order.
    assert!(
        body.contains(
            "execute as @a[scores={dw.classed=1}] unless data entity @s {Health:0.0f} store \
             result score @s dw.tpp run scoreboard players add #tpp dw.sys 1"
        ),
        "{body}"
    );
    // Four seats, the last taking every number past it, each its own cell.
    let s = seats(&body);
    assert_eq!(s.len(), 4, "{body}");
    for k in 1..=3 {
        assert!(body.contains(&format!("scores={{dw.tpp={k}}}]")), "{body}");
    }
    assert!(body.contains("scores={dw.tpp=4..}]"), "{body}");
    let cells: std::collections::BTreeSet<[i64; 3]> = s
        .iter()
        .map(|p| [p[0].floor() as i64, p[1] as i64, p[2].floor() as i64])
        .collect();
    assert_eq!(cells.len(), 4, "no two members share a cell: {s:?}");
    // The first seat is the mark's own cell, which the box teleport's
    // destination spelling names.
    let plan_dir = fixture("seats-plan", &quests(AT_EXIT, NARRATE, ""));
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let c = campaign(&plan_dir);
    let plan = Plan::build(&c, &prefabs).unwrap();
    let mark = plan.party_carries[0].to;
    assert_eq!(
        [s[0][0] - 0.5, s[0][1], s[0][2] - 0.5],
        [f64::from(mark[0]), f64::from(mark[1]), f64::from(mark[2])],
        "{body}"
    );
    // Taken off again in the same function, and declared in setup.
    assert_eq!(
        body.lines().last(),
        Some("scoreboard players reset @a dw.tpp"),
        "{body}"
    );
    assert!(
        text(
            &out,
            &format!("datapack/data/{NS}/function/setup.mcfunction")
        )
        .contains("scoreboard objectives add dw.tpp dummy")
    );
    // The press calls it.
    let all: String = out
        .iter()
        .filter(|(p, _)| p.ends_with(".mcfunction") && p.starts_with("datapack/"))
        .map(|(_, b)| String::from_utf8_lossy(b).into_owned())
        .collect();
    assert!(all.contains(&format!("function {NS}:{name}")));
    // No cutscene in this campaign: nothing is kept for a viewer.
    assert!(!body.contains("_keep"), "{body}");
    assert!(!out.contains_key(&format!("datapack/data/{NS}/function/tpp_keep.mcfunction")));
    // The ledger counts it.
    let gate: serde_json::Value =
        serde_json::from_str(&text(&out, "validation/teleport-gate.json")).unwrap();
    assert_eq!(gate["teleports"]["party"], 1);
    assert_eq!(gate["party_seats"], 4);
    assert_eq!(gate["unbound"], false);
}

#[test]
fn the_packtest_carries_two_members_far_apart_onto_two_seats_alone() {
    let out = build("packtest", &quests(AT_EXIT, NARRATE, ""));
    let (name, body) = party_fn(&out);
    let t = text(
        &out,
        &format!("packtest-datapack/data/{NS}/test/party_{name}.mcfunction"),
    );
    assert!(
        t.lines()
            .nth(1)
            .unwrap()
            .starts_with(&format!("# @environment {NS}:solo/party_{name}")),
        "the move takes every player on the server, so the template runs alone:\n{t}"
    );
    assert!(out.contains_key(&format!(
        "packtest-datapack/data/{NS}/test_environment/solo/party_{name}.json"
    )));
    assert!(t.contains("dummy dwptp0p1 spawn"), "{t}");
    assert!(
        t.contains("assert score #ptp_0_near dw.sys matches 0"),
        "{t}"
    );
    assert!(t.contains(&format!("function {NS}:{name}")), "{t}");
    // One player on each of the first two seats, read at the seats the
    // function writes.
    let s = seat_words(&body);
    for (i, p) in s.iter().take(2).enumerate() {
        let at = format!("execute positioned {p} store result score #ptp_0_s{i} dw.sys");
        assert!(t.contains(&at), "{at}\n{t}");
        assert!(t.contains(&format!("assert score #ptp_0_s{i} dw.sys matches 1")));
    }
}

/// The `x y z` words of each `tp @a[…]` line, as written.
fn seat_words(body: &str) -> Vec<String> {
    body.lines()
        .filter(|l| l.starts_with("tp @a[tag=!dw_cutscene,scores={dw.tpp="))
        .map(|l| l.splitn(3, ' ').nth(2).unwrap().to_string())
        .collect()
}

#[test]
fn a_member_watching_a_cutscene_is_seated_where_the_camera_returns_them() {
    let out = build("cutscene", &quests(AT_EXIT, NARRATE, A_CUTSCENE));
    let (name, body) = party_fn(&out);
    assert!(
        body.contains(&format!(
            "execute as @a[tag=dw_cutscene,scores={{dw.tpp=1..}}] if score @s dw.cs_saved \
             matches 1 run function {NS}:{name}_keep"
        )),
        "{body}"
    );
    // The camera is not fought: no `tp` reaches a viewer.
    assert!(
        body.lines()
            .filter(|l| l.starts_with("tp "))
            .all(|l| l.contains("tag=!dw_cutscene")),
        "{body}"
    );
    let keep = text(
        &out,
        &format!("datapack/data/{NS}/function/{name}_keep.mcfunction"),
    );
    assert_eq!(
        keep.lines()
            .filter(|l| l.contains("data modify storage dw:cs tpp.pos set value ["))
            .count(),
        4,
        "{keep}"
    );
    assert!(keep.contains(&format!("function {NS}:tpp_keep with storage dw:cs tpp")));
    assert_eq!(
        text(
            &out,
            &format!("datapack/data/{NS}/function/tpp_keep.mcfunction")
        )
        .trim(),
        "$data modify storage dw:cs places.p$(id).pos set from storage dw:cs tpp.pos"
    );
}

#[test]
fn a_member_waiting_out_a_respawn_is_not_moved() {
    let dir = fixture(
        "wait",
        &quests(
            AT_EXIT,
            r#"{ "type": "set-checkpoint", "anchor": "anchor/keeper-stand" }"#,
            "",
        ),
    );
    let mut c = campaign(&dir);
    c.world.content.respawn_wait = Some(RespawnWait {
        seconds: 5,
        alone: false,
    });
    let out = try_build(&dir, &c).expect("builds");
    let (_, body) = party_fn(&out);
    assert!(
        body.contains(
            "unless data entity @s {Health:0.0f} unless score @s dw.rwait matches 1.. store \
             result score @s dw.tpp"
        ),
        "{body}"
    );
}

#[test]
fn a_mark_with_no_room_for_every_member_is_dw1141() {
    // The mark sunk into the floor under the exit.
    let into_floor = r#"{ "anchor": "anchor/exit", "offset": [0, -1, 0] }"#;
    let dir = fixture("dw1141", &quests(into_floor, NARRATE, ""));
    let (code, message) = try_build(&dir, &campaign(&dir)).expect_err("refused");
    assert_eq!(code, "DW1141", "{message}");
    assert!(
        message.contains("trigger/door-latch") || message.contains("/triggers/0"),
        "{message}"
    );
    assert!(message.contains("anchor/exit"), "{message}");
    // The same fixture with the mark on the floor builds: one variable.
    let dir = fixture("dw1141-green", &quests(AT_EXIT, NARRATE, ""));
    try_build(&dir, &campaign(&dir)).expect("the mark on the floor seats four");
}

#[test]
fn a_box_teleport_emits_no_party_machinery() {
    let q = quests(AT_EXIT, NARRATE, "").replace(
        r#""from": "party""#,
        r#""from": { "anchor": "anchor/keeper-stand", "extent": [1, 2, 1] }"#,
    );
    let out = build("box", &q);
    let setup = text(
        &out,
        &format!("datapack/data/{NS}/function/setup.mcfunction"),
    );
    assert!(!setup.contains("dw.tpp"), "{setup}");
    assert!(
        !out.keys().any(|p| p.contains("/test/party_teleport_")),
        "no party template for a box teleport"
    );
}

// ---------------------------------------------------------------------------
// the path: the landing is the step's transport; a crossing too is DW1142
// ---------------------------------------------------------------------------

fn first_leg(tag: &str, mark: serde_json::Value) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dw-party-carry-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    common::copy_dir_all(&common::compiler_fixtures_dir().join("first-leg"), &dir);
    // The vault binds a piece that can be arrived in and whose anchor names no
    // other area holds (`anchor/exit` and `spawn` are the hall's too, and a
    // name resolves to its first holder).
    common::patch_file(&dir.join("world.json"), |w| {
        for a in w["content"]["areas"].as_array_mut().unwrap() {
            if a["id"] == "area/vault" {
                a["prefab"] = json!("prefab/island-galley");
            }
        }
    });
    common::patch_file(&dir.join("quests.json"), |q| {
        for x in q["content"]["quests"].as_array_mut().unwrap() {
            if x["id"] == "quest/vault" {
                x["objectives"][0]["anchor"] = json!("anchor/deck");
            }
            if x["id"] == "quest/hall" && !mark.is_null() {
                x["on_objective_complete"]["obj/bell"] =
                    json!([{ "type": "teleport", "from": "party", "to": mark }]);
            }
        }
    });
    dir
}

/// What the path made of the first-leg fixture: the per-step transports,
/// the crossings, and the party carry's mark.
struct Carried {
    transport: Vec<Option<[i32; 3]>>,
    crossings: usize,
    to: Option<[i32; 3]>,
}

fn plan_first_leg(dir: &Path) -> Result<Carried, (String, String)> {
    let loaded = load_campaign_dir(dir).unwrap();
    let c = parse_campaign(&loaded.raw).unwrap();
    let prefabs = PrefabRegistry::load_dir(&common::shown_prefabs_dir("first-leg")).unwrap();
    let plan = Plan::build(&c, &prefabs)
        .map_err(|e| (e.failure.code.id().to_string(), e.failure.message))?;
    Ok(Carried {
        transport: plan.critical_path_transport.clone(),
        crossings: plan.transport.len(),
        to: plan.party_carries.first().map(|p| p.to),
    })
}

#[test]
fn a_party_carry_a_step_fires_is_its_transport_and_replaces_the_crossing() {
    // Without it, the bell's completion carries the party across by the
    // compiler's crossing.
    let none = plan_first_leg(&first_leg("none", serde_json::Value::Null)).unwrap();
    assert_eq!(none.crossings, 1);
    // With it, onto a mark in the vault: the party is already where the next
    // beat stands, so no crossing; the landing is the bell step's transport.
    let Carried {
        transport,
        crossings,
        to,
    } = plan_first_leg(&first_leg("vault", json!({ "anchor": "anchor/deck-aft" }))).unwrap();
    assert_eq!(crossings, 0, "the party carry is the move");
    let to = to.expect("one party carry");
    assert_eq!(
        transport.iter().filter(|t| **t == Some(to)).count(),
        1,
        "{transport:?}"
    );
}

#[test]
fn a_party_carry_and_a_crossing_on_one_step_is_dw1142() {
    let (code, message) = plan_first_leg(&first_leg(
        "hall",
        json!({ "anchor": "anchor/keeper-stand" }),
    ))
    .expect_err("one step cannot put the party in two places");
    assert_eq!(code, "DW1142", "{message}");
    for expected in ["obj/bell", "obj/take", "area/hall", "area/vault"] {
        assert!(message.contains(expected), "{expected}: {message}");
    }
}
