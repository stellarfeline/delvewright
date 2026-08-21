//! Rejoin-after-cutscene repair.
//!
//! The cutscene save/restore bracket is entirely `@a`-scoped: `cs_end_<bare>`
//! restores gamemode, teleports and untags *the players online when it ends*. A
//! player who disconnects during the shot is not one of them, so they came back
//! tagged `dw_cutscene`, in spectator, with the marker they would have been
//! teleported to already killed — a ghost with no way out. `join_place` could not
//! help: it is gated on `dw_joined`, which a relog keeps exactly like the cutscene
//! tag does.
//!
//! The repair is a tick clause keyed on the stuck state itself — tagged while no
//! cutscene is playing — plus a per-player (`@s`) restore to the live checkpoint.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};

const NS: &str = "hello-world";

/// A v0.6 `quests` doc whose exit beat plays `cutscene` (a raw JSON object body,
/// or an empty string for a cutscene-less campaign).
fn quests_doc(cutscene: &str) -> String {
    let on_complete = if cutscene.is_empty() {
        r#"{ "type": "campaign-complete" }"#.to_string()
    } else {
        format!(r#"{cutscene}, {{ "type": "campaign-complete" }}"#)
    };
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
        "on_complete": [ {on_complete} ]
      }}
    ]
  }}
}}"#
    )
}

const A_CUTSCENE: &str = r#"{ "type": "cutscene", "seconds": 2,
  "path": [ { "anchor": "anchor/exit", "offset": [-2, 2, 0] },
            { "anchor": "anchor/exit", "offset": [2, 2, 0] } ] }"#;

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

fn build(cutscene: &str) -> BuildOutput {
    let campaign: Campaign = parse_campaign(&RawCampaign {
        world: read_hw("world.json"),
        npcs: read_hw("npcs.json"),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: quests_doc(cutscene),
        dialogue: read_hw("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
    })
    .expect("campaign parses");
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
    emit::build(
        &plan,
        &BTreeMap::new(),
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
    .expect("every emitted command validates")
}

fn body(out: &BuildOutput, name: &str) -> String {
    let path = format!("datapack/data/{NS}/function/{name}.mcfunction");
    String::from_utf8(
        out.get(&path)
            .unwrap_or_else(|| panic!("no `{name}` function; keys: {:?}", out.keys().len()))
            .clone(),
    )
    .unwrap()
}

fn cutscene_fn_body(out: &BuildOutput, kind: &str) -> String {
    let prefix = format!("datapack/data/{NS}/function/cs_");
    let (_, bytes) = out
        .iter()
        .find(|(p, _)| {
            p.starts_with(&prefix)
                && match kind {
                    "start" => !p.contains("cs_tick_") && !p.contains("cs_end_"),
                    other => p.contains(other),
                }
        })
        .unwrap_or_else(|| panic!("no cutscene `{kind}` function"));
    String::from_utf8(bytes.clone()).unwrap()
}

/// The stuck state is detected on the world tick: tagged `dw_cutscene` while the
/// live-cutscene refcount is zero. Asserted as an exact line so the selector and
/// the `unless` direction cannot silently drift.
#[test]
fn tick_drives_the_rejoin_repair() {
    let out = build(A_CUTSCENE);
    let tick = body(&out, "tick");
    let driver = "execute unless score #cs_live dw.sys matches 1.. \
                  as @a[tag=dw_cutscene] run function hello-world:cs_repair";
    assert!(
        tick.lines().any(|l| l.trim() == driver),
        "the tick must drive the cutscene rejoin repair:\n{tick}"
    );
}

/// The bracket maintains the refcount: `+1` on start (after the re-entry guard, so
/// a re-entrant start cannot inflate it) and `-1` on end.
#[test]
fn the_cutscene_bracket_refcounts_itself() {
    let out = build(A_CUTSCENE);
    let start = cutscene_fn_body(&out, "start");
    let end = cutscene_fn_body(&out, "cs_end_");
    let start_lines: Vec<&str> = start.lines().collect();
    let guard = start_lines
        .iter()
        .position(|l| l.contains("run return fail"))
        .expect("the re-entry guard is line 0 of a cutscene start");
    let bump = start_lines
        .iter()
        .position(|l| l.trim() == "scoreboard players add #cs_live dw.sys 1")
        .unwrap_or_else(|| panic!("no #cs_live bump in the cutscene start:\n{start}"));
    assert!(
        bump > guard,
        "the refcount must be bumped after the re-entry guard:\n{start}"
    );
    assert!(
        end.lines()
            .any(|l| l.trim() == "scoreboard players remove #cs_live dw.sys 1"),
        "the cutscene end must drop the refcount:\n{end}"
    );
}

/// The repair itself is per-player (`@s`, never `@a` — a repair must not disturb
/// players who are mid-cutscene right now) and returns the player to the live
/// checkpoint mirror, because the cutscene's own position marker has been killed
/// by the time this can run.
#[test]
fn the_repair_is_per_player_and_returns_to_the_checkpoint() {
    let out = build(A_CUTSCENE);
    let repair = body(&out, "cs_repair");
    for expected in [
        "gamemode adventure @s",
        "tag @s remove dw_cutscene",
        "data modify storage dw:cs at.x set from storage dw:cp pos[0]",
        "data modify storage dw:cs at.y set from storage dw:cp pos[1]",
        "data modify storage dw:cs at.z set from storage dw:cp pos[2]",
        "function hello-world:cs_repair_tp with storage dw:cs at",
    ] {
        assert!(
            repair.lines().any(|l| l.trim() == expected),
            "cs_repair must contain `{expected}`:\n{repair}"
        );
    }
    assert!(
        !repair.contains("@a"),
        "the repair must never touch the whole party:\n{repair}"
    );
    assert_eq!(
        body(&out, "cs_repair_tp").trim(),
        "$tp @s $(x) $(y) $(z)",
        "the checkpoint mirror is an [x, y, z] list, so the return is a macro tp"
    );
}

/// A cutscene-less campaign emits none of this — no tick clause, no functions —
/// so its pack stays byte-identical (ADR-0006).
#[test]
fn a_cutscene_less_campaign_emits_no_repair() {
    let out = build("");
    assert!(
        !body(&out, "tick").contains("cs_repair"),
        "a cutscene-less campaign must not gain a repair clause"
    );
    for name in ["cs_repair", "cs_repair_tp"] {
        assert!(
            !out.keys()
                .any(|p| p.ends_with(&format!("{name}.mcfunction"))),
            "a cutscene-less campaign must not emit `{name}`"
        );
    }
}
