//! spec-0019 rehearsal overlay emission: the `dw:rehearsal` proposal, the
//! calibration triggers, and the `[DelveShot]` harvest stamp.
//!
//! The overlay is playtest-only, so these tests are the every-push proof that
//! its emission is well-formed (the vanilla command-tree validator runs inside
//! `emit::build`, so a syntax error here is a build failure), that the proposal
//! seeds from the compiled DSL values, and that nothing leaks into the shipped
//! datapack.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::plan::{Plan, ResolvedAnchor};
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};

const NS: &str = "hello-world";

/// A v0.6 quests document whose exit beat plays `cutscene`.
fn quests_doc(cutscene: &str) -> String {
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
          "obj/talk": [ {{ "type": "open-gate", "anchor": "anchor/door" }} ]
        }},
        "on_complete": [ {cutscene}, {{ "type": "campaign-complete" }} ]
      }}
    ]
  }}
}}"#
    )
}

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

fn parse_hw(quests: &str) -> Campaign {
    let raw = RawCampaign {
        world: read_hw("world.json"),
        npcs: read_hw("npcs.json"),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: quests.to_string(),
        dialogue: read_hw("dialogue.json"),
        world_edits: None,
    };
    parse_campaign(&raw).expect("campaign parses")
}

/// Build hello-world with `cutscene` on its exit beat. Campaign and registry are
/// leaked so the returned `Plan` can outlive the call (test-only convenience).
fn build(cutscene: &str) -> (Plan<'static>, BuildOutput) {
    let prefabs: &'static PrefabRegistry = Box::leak(Box::new(
        PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap(),
    ));
    let campaign: &'static Campaign = Box::leak(Box::new(parse_hw(&quests_doc(cutscene))));
    let plan = Plan::build(campaign, prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let bytes = std::fs::read(common::prefabs_dir().join(&piece.structure_file)).unwrap();
            structures.insert(piece.structure_file.clone(), bytes);
        }
    }
    let out = emit::build(
        &plan,
        &BTreeMap::new(),
        &structures,
        &CommandTree::v1_21_11(),
        prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
    .expect("every emitted command validates");
    (plan, out)
}

/// A two-shot cutscene: an explicit-path dolly plus a static insert.
const TWO_SHOT: &str = r#"{ "type": "cutscene", "shots": [
    { "path": [ { "anchor": "anchor/exit", "offset": [-2, 2, 0] },
                { "anchor": "anchor/exit", "offset": [2, 2, 0] } ],
      "seconds": 6,
      "look_at": { "anchor": "anchor/keeper-stand", "offset": [0, 2, 0] } },
    { "path": [ { "anchor": "anchor/keeper-stand", "offset": [0, 2, 1] } ],
      "seconds": 4 } ] }"#;

fn overlay(out: &BuildOutput, name: &str) -> String {
    let key = format!("creator-datapack/data/{NS}/function/creator/{name}.mcfunction");
    String::from_utf8(
        out.get(&key)
            .unwrap_or_else(|| panic!("overlay function {name} is emitted"))
            .clone(),
    )
    .unwrap()
}

fn layout(out: &BuildOutput) -> serde_json::Value {
    serde_json::from_slice(&out["creator-datapack/layout.json"]).unwrap()
}

/// The resolved cell of `anchor + offset` — the compiler's own convention,
/// which the proposal stores verbatim.
fn cell(plan: &Plan, anchor: &str, offset: [i32; 3]) -> [i32; 3] {
    let base = plan
        .anchors
        .iter()
        .find(|((_, name), _)| name == anchor)
        .map(|(_, r)| match r {
            ResolvedAnchor::Point { pos, .. } => *pos,
            ResolvedAnchor::Gate { from, .. } => *from,
        })
        .expect("anchor resolves");
    [
        base[0] + offset[0],
        base[1] + offset[1],
        base[2] + offset[2],
    ]
}

/// The proposal seeds from the COMPILED shot values: every waypoint is the exact
/// `anchor + offset` cell the shipped dolly flies through, the duration is the
/// resolved `seconds`, and the look target is the resolved `look_at` — so a
/// replay before any adjustment shows exactly what ships.
#[test]
fn defaults_seed_from_the_compiled_shot_values() {
    let (plan, out) = build(TWO_SHOT);
    let body = overlay(&out, "rehearsal/defaults");
    let a = cell(&plan, "anchor/exit", [-2, 2, 0]);
    let b = cell(&plan, "anchor/exit", [2, 2, 0]);
    let look = cell(&plan, "anchor/keeper-stand", [0, 2, 0]);
    let c = cell(&plan, "anchor/keeper-stand", [0, 2, 1]);
    assert!(
        body.contains(&format!(
            "path:[{{x:{},y:{},z:{}}},{{x:{},y:{},z:{}}}],pstr:\"{},{},{};{},{},{}\"",
            a[0], a[1], a[2], b[0], b[1], b[2], a[0], a[1], a[2], b[0], b[1], b[2]
        )),
        "shot 1 path seeds from the compiled waypoints:\n{body}"
    );
    assert!(
        body.contains(&format!("lstr:\"{},{},{}\"", look[0], look[1], look[2])),
        "shot 1 look_at seeds from the compiled subject:\n{body}"
    );
    assert!(
        body.contains(&format!("pstr:\"{},{},{}\"", c[0], c[1], c[2])),
        "shot 2 path seeds from its single waypoint:\n{body}"
    );
    assert!(body.contains("seconds:6"), "shot 1 duration:\n{body}");
    assert!(body.contains("seconds:4"), "shot 2 duration:\n{body}");
    // The travel-aimed shot has no fixed look target until `dw.aim` gives it one.
    assert!(body.contains("lstr:\"none\""), "shot 2 aim:\n{body}");
    // The live proposal starts as a copy of the immutable compiled baseline.
    assert!(
        body.contains("data modify storage dw:rehearsal shots set from storage dw:rehearsal base"),
        "the proposal copies the baseline:\n{body}"
    );
}

/// A `/reload` re-runs `#minecraft:load`; the seed is guarded so it does NOT
/// discard a proposal the creator is midway through — the whole spec-0019 claim
/// is that the adjust/replay loop survives inside one session.
#[test]
fn reload_does_not_discard_an_in_progress_proposal() {
    let (_, out) = build(TWO_SHOT);
    let init = overlay(&out, "init");
    assert!(
        init.contains(
            "execute unless data storage dw:rehearsal shots run function \
             hello-world:creator/rehearsal/defaults"
        ),
        "the seed is guarded on absence:\n{init}"
    );
}

/// Every calibration trigger is registered, armed each tick for everyone, and
/// dispatched — the full spec-0019 §3 surface.
#[test]
fn calibration_triggers_are_registered_and_dispatched() {
    let (_, out) = build(TWO_SHOT);
    let init = overlay(&out, "init");
    let tick = overlay(&out, "tick");
    for t in ["dw.mark", "dw.aim", "dw.faster", "dw.slower", "dw.done"] {
        assert!(
            init.contains(&format!("scoreboard objectives add {t} trigger")),
            "{t} is registered:\n{init}"
        );
        assert!(
            tick.contains(&format!("scoreboard players enable @a {t}")),
            "{t} is armed each tick:\n{tick}"
        );
    }
    assert!(
        tick.contains("scores={dw.mark=1..}") && tick.contains("scores={dw.mark=..-1}"),
        "`dw.mark set <s>` marks and `set -<s>` resets:\n{tick}"
    );
}

/// The calibration verbs mutate `dw:rehearsal` storage and NOTHING else — no
/// datapack write, no world edit, no scoreboard the campaign reads. This is the
/// property that makes adjust-and-replay work with no reload (spec-0019 §3).
#[test]
fn calibration_verbs_touch_only_the_rehearsal_storage() {
    let (_, out) = build(TWO_SHOT);
    let prefix = format!("creator-datapack/data/{NS}/function/creator/rehearsal/");
    let mut seen = 0;
    for (path, bytes) in &out {
        let Some(name) = path.strip_prefix(&prefix) else {
            continue;
        };
        // `defaults` seeds, the stamps only `say`; the ray probe is a marker
        // entity the cast summons and kills within its own command chain.
        if name.starts_with("stamp_") || name.starts_with("roster") {
            continue;
        }
        seen += 1;
        let body = String::from_utf8(bytes.clone()).unwrap();
        for line in body.lines() {
            let writes_storage = line.contains("storage dw:rehearsal");
            let writes_nothing_else = !line.contains("data modify storage ") || writes_storage;
            assert!(
                writes_nothing_else,
                "{name}: writes storage outside dw:rehearsal: {line}"
            );
            for forbidden in [
                "setblock ",
                "fill ",
                "give ",
                "tp ",
                "teleport ",
                "gamemode ",
            ] {
                assert!(
                    !line.trim_start_matches('$').starts_with(forbidden),
                    "{name}: a calibration verb must not touch the world: {line}"
                );
            }
            // Campaign scoreboards are off limits; the overlay owns `dw.rh` and
            // the trigger objectives it registered itself.
            if let Some(rest) = line.split(" dw.").nth(1) {
                let obj = rest.split_whitespace().next().unwrap_or("");
                assert!(
                    ["rh", "mark", "aim", "faster", "slower", "done"].contains(&obj),
                    "{name}: touches campaign scoreboard dw.{obj}: {line}"
                );
            }
        }
    }
    assert!(seen >= 10, "the calibration surface is in scope ({seen})");
}

/// `dw.done` stamps ONE parseable `[DelveShot]` line per shot, carrying the
/// current proposal (macro-substituted) and the compile-time identity a patch
/// needs — shot id, beat id and the JSON pointer into the quests document.
#[test]
fn done_stamps_one_delveshot_line_per_shot() {
    let (_, out) = build(TWO_SHOT);
    let done = overlay(&out, "rehearsal/done");
    assert_eq!(
        done.lines()
            .filter(|l| l.contains("creator/rehearsal/stamp_"))
            .count(),
        2,
        "one stamp per shot:\n{done}"
    );
    assert!(
        done.contains(
            "function hello-world:creator/rehearsal/stamp_1 with storage dw:rehearsal shots[0]"
        ),
        "the stamp reads the LIVE proposal, not the baseline:\n{done}"
    );
    let stamp = overlay(&out, "rehearsal/stamp_1");
    assert!(
        stamp.starts_with(
            "$say [DelveShot] shot=1 beat=1 ptr=/content/quests/0/on_complete/0 idx=0 \
             seconds=$(seconds) look_at=$(lstr) path=$(pstr)"
        ),
        "stamp line shape:\n{stamp}"
    );
    let stamp2 = overlay(&out, "rehearsal/stamp_2");
    assert!(
        stamp2.contains("ptr=/content/quests/0/on_complete/0 idx=1"),
        "shot 2 names its own DSL location:\n{stamp2}"
    );
}

/// A shot is identified by its `cutscene` EFFECT pointer plus an index, never by
/// a `…/shots/<k>` pointer — because the single-shot spelling and its one-entry
/// `shots` equivalent are the same cutscene and must emit byte-identical output
/// (`v06_cutscene::single_shot_spellings_are_byte_identical`). A pointer that
/// spelled the shot path would have made the overlay differ between two
/// documents that mean exactly the same thing.
#[test]
fn both_cutscene_spellings_identify_a_shot_the_same_way() {
    let path = r#""path": [ { "anchor": "anchor/exit", "offset": [-2, 2, 0] },
                            { "anchor": "anchor/exit", "offset": [2, 2, 0] } ]"#;
    let (_, legacy) = build(&format!(
        r#"{{ "type": "cutscene", "seconds": 2, {path} }}"#
    ));
    let (_, shots) = build(&format!(
        r#"{{ "type": "cutscene", "shots": [ {{ "seconds": 2, {path} }} ] }}"#
    ));
    let stamp = overlay(&legacy, "rehearsal/stamp_1");
    assert!(
        stamp.contains("ptr=/content/quests/0/on_complete/0 idx=0 "),
        "the pointer names the effect, the index the shot:\n{stamp}"
    );
    assert_eq!(
        stamp,
        overlay(&shots, "rehearsal/stamp_1"),
        "both spellings stamp the same identity"
    );
}

/// The layout manifest carries the resolved-anchor vocabulary `delvec calibrate`
/// snaps onto, with the same cell convention the compiler resolves waypoints
/// with — so a calibrated `anchor + offset` means the same thing to both.
#[test]
fn layout_carries_the_resolved_anchor_manifest() {
    let (plan, out) = build(TWO_SHOT);
    let l = layout(&out);
    let anchors = l["anchors"].as_array().expect("anchors array");
    assert!(!anchors.is_empty());
    let door = anchors
        .iter()
        .find(|a| a["id"] == "anchor/door")
        .expect("the gate anchor is listed");
    let expect = cell(&plan, "anchor/door", [0, 0, 0]);
    assert_eq!(door["kind"], "gate");
    assert_eq!(
        door["pos"],
        serde_json::json!([expect[0], expect[1], expect[2]])
    );
    let shots = l["shots"].as_array().expect("shot roster");
    assert_eq!(shots.len(), 2);
    assert_eq!(shots[0]["pointer"], "/content/quests/0/on_complete/0");
    assert_eq!(shots[0]["shot_index"], 0);
    assert_eq!(shots[1]["shot_index"], 1);
}

/// A cutscene-less campaign emits NO rehearsal artifacts: nothing to rehearse,
/// no dead triggers, and the overlay's function set is unchanged from its
/// pre-spec-0019 form.
#[test]
fn a_cutscene_less_campaign_emits_no_rehearsal_artifacts() {
    let (_, out) = build(r#"{ "type": "set-flag", "flag": "flag/seen" }"#);
    for path in out.keys() {
        assert!(
            !path.contains("creator/rehearsal/"),
            "unexpected rehearsal artifact: {path}"
        );
    }
    let init = overlay(&out, "init");
    assert_eq!(
        init.trim(),
        "scoreboard objectives add dw.note trigger",
        "the overlay init is unchanged for a cutscene-less campaign"
    );
    assert!(layout(&out)["shots"].as_array().unwrap().is_empty());
}

/// The rehearsal overlay is playtest-only: not one byte of it may reach the
/// shipped datapack (the CI image-exclusion grep is the runtime backstop).
#[test]
fn rehearsal_overlay_absent_from_the_shipped_datapack() {
    let (_, out) = build(TWO_SHOT);
    for (path, bytes) in &out {
        if !path.starts_with("datapack/") {
            continue;
        }
        let body = std::str::from_utf8(bytes).unwrap_or("");
        for marker in ["dw:rehearsal", "DelveShot", "dw.mark", "dw.aim", "dw.done"] {
            assert!(
                !body.contains(marker) && !path.contains("rehearsal"),
                "rehearsal artifact `{marker}` leaked into the shipped datapack at {path}"
            );
        }
    }
}

/// ADR-0006: the overlay is a pure function of the campaign — two builds of the
/// same input are byte-identical, `creator-datapack/` included.
#[test]
fn overlay_emission_is_deterministic() {
    let (_, a) = build(TWO_SHOT);
    let (_, b) = build(TWO_SHOT);
    let keys: Vec<&String> = a
        .keys()
        .filter(|k| k.starts_with("creator-datapack/"))
        .collect();
    assert!(!keys.is_empty());
    for k in keys {
        assert_eq!(a[k], b[k], "creator overlay is not deterministic at {k}");
    }
}

/// The `cutscene-shots` fixture is the campaign `validation/rehearsal-flow.sh`
/// plays. Tier 3 boots a server to assert the harvested proposal; this asserts
/// the *seed* that flow starts from, so a fixture edit fails on every push
/// instead of on a release candidate.
#[test]
fn the_tier3_fixture_seeds_the_proposal_the_flow_asserts() {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let loaded = delvewright_compiler::load::load_campaign_dir(&common::cutscene_shots_dir())
        .expect("the fixture campaign loads");
    let campaign = parse_campaign(&loaded.raw).expect("the fixture campaign parses");
    let plan = Plan::build(&campaign, &prefabs).expect("the fixture plans");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let bytes = std::fs::read(common::prefabs_dir().join(&piece.structure_file)).unwrap();
            structures.insert(piece.structure_file.clone(), bytes);
        }
    }
    let out = emit::build(
        &plan,
        &BTreeMap::new(),
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
    .expect("the fixture builds");
    let key = "creator-datapack/data/cutscene-shots/function/creator/rehearsal/defaults.mcfunction";
    let body = String::from_utf8(out[key].clone()).unwrap();
    // The exact values `validation/rehearsal-flow.sh` computes its expectations
    // from: shot 1 is 6 s (so `dw.faster` lands on 5), shot 2 is an untouched
    // 4 s single-waypoint insert with no look target.
    assert!(body.contains("seconds:6"), "{body}");
    assert!(
        body.contains("pstr:\"5,67,5\",look:{},lstr:\"none\""),
        "shot 2 is the untouched control the flow asserts:\n{body}"
    );
    assert!(
        body.contains("lstr:\"5,67,4\""),
        "shot 1 compiled aim:\n{body}"
    );
}

/// **A `trigger` objective is armed by its score entry, so `scoreboard players
/// reset` disarms it.** Vanilla keeps "this player may `/trigger` this
/// objective" as a lock flag on the score entry; deleting the entry deletes the
/// permission, and `scoreboard players enable` re-creates it at 0. A tick that
/// both `enable`s an objective and `reset`s it leaves it permanently unusable —
/// `/trigger` answers "You cannot trigger this objective yet" and **nothing
/// reaches the server log**, so no report, no PackTest assertion and no amount
/// of reading the emitted commands makes it visible.
///
/// That is exactly what shipped in the first live run of this feature: a
/// per-tick hygiene clause resetting the no-op value (`scores={dw.mark=0}`)
/// matched the entry `enable` had just created, so every adjust verb was
/// silently refused while `dw.done` — which had no such clause — worked. The
/// invariant below is the strongest form the lesson can take short of a
/// compiler diagnostic: it reads the emitted overlay and fails the build's
/// tests if any function ever again arms and disarms the same objective.
#[test]
fn the_tick_never_resets_a_trigger_it_arms() {
    let (_, out) = build(TWO_SHOT);
    for name in ["init", "tick"] {
        let body = overlay(&out, name);
        let armed: Vec<&str> = body
            .lines()
            .filter_map(|l| l.strip_prefix("scoreboard players enable @a "))
            .collect();
        assert!(
            name == "init" || !armed.is_empty(),
            "the tick must arm the calibration triggers"
        );
        for obj in &armed {
            assert!(
                !body.contains(&format!("scoreboard players reset @s {obj}")),
                "creator/{name} both arms and resets `{obj}` — `reset` deletes the score \
                 entry that carries the trigger permission, so `/trigger {obj}` would be \
                 refused forever with nothing in the server log. Clear a fired trigger \
                 inside its handler (the next tick's `enable` re-arms it), never here.\n{body}"
            );
        }
    }
    // The handlers DO reset — that is the correct place, and it is what makes the
    // trigger one-shot per fire.
    assert!(
        overlay(&out, "rehearsal/mark").contains("scoreboard players reset @s dw.mark"),
        "a handler clears the trigger it consumed"
    );
}
