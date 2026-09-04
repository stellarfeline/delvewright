//! `world.difficulty` emission (v0.6) + actor
//! `attributes`.
//!
//! Difficulty was a compiler constant — `easy` with waves, `peaceful` without —
//! and easy halves incoming player damage (`min(dmg / 2 + 1, dmg)`), so the
//! setting behind "the enemies are too weak" was one no campaign could state.
//! Declaring it must land in **both** places a difficulty can come from:
//!
//! * `server/server.properties`, which is what the shipped image and every
//!   compose profile derive their world from (`validation/check-world-settings.sh`);
//! * a `/difficulty` in the sealing baseline, so the declaration also holds when
//!   the datapack alone is dropped into somebody else's world.
//!
//! ...and it must be *provable on a live server*, hence the generated
//! `declared_difficulty` PackTest. A campaign that declares nothing keeps the
//! derivation and emits none of it (byte-identity, ADR-0006).

mod common;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::parse_campaign;

const NS: &str = "hello-world";

fn scratch_dir(kind: &str) -> std::path::PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "dw-diff-{kind}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ))
}

fn build_dir(dir: &Path) -> BuildOutput {
    let loaded = load_campaign_dir(dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("valid campaign parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    let tree = CommandTree::v1_21_11();
    emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &tree,
        &prefabs,
        None,
        &BTreeMap::new(),
    )
    .expect("every emitted command validates")
}

/// hello-world with an optional `world.difficulty` (world stage raised to 0.6.0
/// when one is declared) and an optional quests-stage replacement.
fn build_hw(difficulty: Option<&str>, quests: Option<&str>) -> BuildOutput {
    let src = common::hello_world_dir();
    let dst = scratch_dir("hw");
    let _ = std::fs::remove_dir_all(&dst);
    std::fs::create_dir_all(&dst).unwrap();
    for f in common::STAGE_FILES {
        std::fs::copy(src.join(f), dst.join(f)).unwrap();
    }
    if let Some(value) = difficulty {
        let wp = dst.join("world.json");
        let w = std::fs::read_to_string(&wp)
            .unwrap()
            .replacen(
                "\"dsl_version\": \"0.2.0\"",
                "\"dsl_version\": \"0.6.0\"",
                1,
            )
            .replacen(
                "\"target_minutes\": 5,",
                &format!("\"target_minutes\": 5,\n    \"difficulty\": \"{value}\","),
                1,
            );
        assert!(w.contains("\"difficulty\""), "world.json patch applied");
        std::fs::write(&wp, w).unwrap();
    }
    if let Some(q) = quests {
        std::fs::write(dst.join("quests.json"), q).unwrap();
    }
    let out = build_dir(&dst);
    let _ = std::fs::remove_dir_all(&dst);
    out
}

fn text<'a>(out: &'a BuildOutput, path: &str) -> &'a str {
    std::str::from_utf8(out.get(path).unwrap_or_else(|| panic!("{path} emitted"))).unwrap()
}

fn properties(out: &BuildOutput) -> &str {
    text(out, "server/server.properties")
}

fn setup(out: &BuildOutput) -> &str {
    text(
        out,
        &format!("datapack/data/{NS}/function/setup.mcfunction"),
    )
}

/// A v0.6 quests doc that spawns a wave — the case whose derived difficulty is
/// `easy`, so a declaration must be seen to *override* it, not merely fill a gap.
const WAVE_QUESTS: &str = r#"{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {
    "quests": [
      {
        "id": "quest/open-the-door",
        "trigger": { "type": "campaign-start" },
        "objectives": [
          { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" },
          { "type": "kill", "id": "obj/clear", "wave": "wave/ambush", "after": ["obj/talk"] },
          { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit",
            "radius": 2, "after": ["obj/clear"] }
        ],
        "on_objective_complete": {
          "obj/talk": [ { "type": "spawn-wave", "wave": "wave/ambush" },
                        { "type": "open-gate", "anchor": "anchor/door" } ]
        },
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ],
    "waves": [
      { "id": "wave/ambush", "anchor": "anchor/keeper-stand",
        "mobs": [ { "entity": "minecraft:zombie", "count": 2 } ] }
    ]
  }
}"#;

/// A v0.6 quests doc with one scripted actor, spawned then unleashed.
/// `{extra}` is spliced into the actor object.
fn actor_quests(extra: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {{
    "quests": [
      {{
        "id": "quest/open-the-door",
        "trigger": {{ "type": "campaign-start" }},
        "objectives": [
          {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }},
          {{ "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit",
             "radius": 2, "after": ["obj/talk"] }}
        ],
        "on_objective_complete": {{
          "obj/talk": [
            {{ "type": "open-gate", "anchor": "anchor/door" }},
            {{ "type": "spawn-actor", "actor": "actor/giant" }},
            {{ "type": "unleash-actor", "actor": "actor/giant" }}
          ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ],
    "actors": [
      {{ "id": "actor/giant", "entity": "minecraft:vindicator", "name": "The Sleeper",
         "anchor": "anchor/keeper-stand"{extra} }}
    ]
  }}
}}"#
    )
}

// --- the derivation is preserved ---------------------------------------------

/// No declaration ⇒ exactly the pre-0.6 behavior: `peaceful` for a wave-free
/// campaign, no `/difficulty` in setup, no generated difficulty PackTest. This is
/// the byte-identity contract for every campaign written before the field.
#[test]
fn absent_difficulty_keeps_the_derivation_and_emits_nothing() {
    let out = build_hw(None, None);
    assert!(properties(&out).contains("difficulty=peaceful"));
    assert!(
        !setup(&out).contains("difficulty "),
        "setup must carry no /difficulty when none is declared:\n{}",
        setup(&out)
    );
    assert!(
        !out.contains_key(&format!(
            "packtest-datapack/data/{NS}/test/declared_difficulty.mcfunction"
        )),
        "no declaration, no generated difficulty test"
    );

    let waves = build_hw(None, Some(WAVE_QUESTS));
    assert!(
        properties(&waves).contains("difficulty=easy"),
        "a wave campaign still derives `easy`"
    );
    assert!(!setup(&waves).contains("difficulty "));
}

// --- the declaration wins, in both places ------------------------------------

/// A declaration lands in `server.properties` (what the shipped image and the
/// compose profiles boot from) AND in the sealing baseline (what a
/// datapack-only install gets).
#[test]
fn declared_difficulty_drives_properties_and_the_sealing_baseline() {
    for (value, id) in [("easy", 1), ("normal", 2), ("hard", 3)] {
        let out = build_hw(Some(value), None);
        assert!(
            properties(&out).contains(&format!("difficulty={value}")),
            "server.properties must carry the declared difficulty `{value}`"
        );
        assert!(
            setup(&out).contains(&format!("difficulty {value}")),
            "setup must re-assert the declared difficulty `{value}`"
        );
        // ...and the live-server proof, keyed to vanilla's Difficulty#getId().
        let test = text(
            &out,
            &format!("packtest-datapack/data/{NS}/test/declared_difficulty.mcfunction"),
        );
        assert!(test.contains("execute store result score #difficulty dw.sys run difficulty"));
        assert!(test.contains(&format!("assert score #difficulty dw.sys matches {id}")));
    }
}

/// The declaration overrides the wave derivation rather than merely filling a
/// gap: a wave campaign that says `hard` ships `hard`, not the historical `easy`.
#[test]
fn declared_difficulty_overrides_the_wave_derivation() {
    let out = build_hw(Some("hard"), Some(WAVE_QUESTS));
    assert!(properties(&out).contains("difficulty=hard"));
    assert!(!properties(&out).contains("difficulty=easy"));
    assert!(setup(&out).contains("difficulty hard"));
}

/// Same DSL ⇒ same bytes, with the field declared (ADR-0006).
#[test]
fn declared_difficulty_is_deterministic() {
    let a = build_hw(Some("normal"), Some(WAVE_QUESTS));
    let b = build_hw(Some("normal"), Some(WAVE_QUESTS));
    assert_eq!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>());
    for (path, bytes) in &a {
        assert_eq!(bytes, &b[path], "byte mismatch in {path}");
    }
}

// --- actor attributes ---------------------------------------------------------

fn actor_fns(out: &BuildOutput) -> (String, String) {
    (
        text(
            out,
            &format!("datapack/data/{NS}/function/spawn_actor_giant.mcfunction"),
        )
        .to_string(),
        text(
            out,
            &format!("datapack/data/{NS}/function/unleash_giant.mcfunction"),
        )
        .to_string(),
    )
}

/// An actor's `attributes` ride BOTH bodies. The puppet is what the party
/// circles; the twin is what actually fights, and unleashing replaces the body —
/// so tuning that stopped at the puppet would tune nothing that matters.
#[test]
fn actor_attributes_ride_the_puppet_and_the_twin() {
    let extra = r#", "attributes": { "max_health": 200.0, "attack_damage": 12.0 }"#;
    let out = build_hw(Some("hard"), Some(&actor_quests(extra)));
    let (puppet, twin) = actor_fns(&out);
    let expect = "attributes:[{id:\"minecraft:max_health\",base:200.0},\
                  {id:\"minecraft:attack_damage\",base:12.0}]";
    assert!(puppet.contains(expect), "puppet summon:\n{puppet}");
    assert!(twin.contains(expect), "twin summon:\n{twin}");
}

/// An actor with no `attributes` emits none — the pre-`attributes` string, which
/// is what keeps every earlier campaign byte-identical.
#[test]
fn actor_without_attributes_emits_none() {
    let out = build_hw(Some("hard"), Some(&actor_quests("")));
    let (puppet, twin) = actor_fns(&out);
    assert!(!puppet.contains("attributes:["), "puppet:\n{puppet}");
    assert!(!twin.contains("attributes:["), "twin:\n{twin}");
}

/// A `vulnerable` puppet's compiler-owned knockback-immunity comes FIRST and the
/// authored overrides follow it — one attribute list, not two, and the
/// no-authored-attributes rendering is unchanged.
#[test]
fn vulnerable_knockback_immunity_precedes_authored_attributes() {
    let extra = r#", "vulnerable": true, "attributes": { "max_health": 40.0 }"#;
    let out = build_hw(Some("normal"), Some(&actor_quests(extra)));
    let (puppet, _) = actor_fns(&out);
    assert!(
        puppet.contains(
            "attributes:[{id:\"minecraft:knockback_resistance\",base:1.0},\
             {id:\"minecraft:max_health\",base:40.0}]"
        ),
        "puppet summon:\n{puppet}"
    );

    // The twin is a freed elite, not a caged creep: it keeps the tuning but not
    // the knockback immunity.
    let (_, twin) = actor_fns(&out);
    assert!(twin.contains("{id:\"minecraft:max_health\",base:40.0}"));
    assert!(!twin.contains("knockback_resistance"), "twin:\n{twin}");
}
