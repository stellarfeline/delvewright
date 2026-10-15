//! A click box wins the pick (`DW1013`), on a real build.
//!
//! The finding: an `interact` prop took many right-clicks before one registered,
//! because the `minecraft:interaction` box the compiler summoned over it was
//! `1.0 × 2.0` at the cell centre — its side faces and floor exactly the faces of
//! the full-cube prop standing in the same cell — and the client takes an entity
//! only when it is strictly nearer than the block the same ray hits.
//!
//! These tests build the `souls-shortcut` fixture and prove three things over the
//! shipped bytes: every click box the build places is judged and wins
//! (`compiler::clickbox::check` green, its binding equal to a count taken here by
//! a second reading); a `use` trigger whose prop is a full cube gets the column
//! the one rule builds; and that build, with the one box planted back in its
//! flush shape, is refused under `DW1013`.

mod common;

use std::collections::BTreeMap;

use delvec::compiler::clickbox::{DW_CLICK_BOX_FLUSH, check};
use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, EnvTrigger, parse_campaign};

const NS: &str = "souls-shortcut";

/// The fixture as it stands: a shortcut's unlock and its sealed door.
fn fixture() -> Campaign {
    let dir = common::compiler_fixtures_dir().join(NS);
    let loaded = load_campaign_dir(&dir).unwrap();
    parse_campaign(&loaded.raw).expect("souls-shortcut parses")
}

/// The fixture with its shortcut and triggers cleared and one `use` trigger on
/// `anchor/exit` instead, its prop a full cube a `1.0`-wide box would be flush with.
fn campaign() -> Campaign {
    let mut c = fixture();
    c.quests.content.shortcuts.clear();
    c.quests.content.triggers = vec![
        serde_json::from_str::<EnvTrigger>(
            r#"{ "id": "trigger/open-the-barrel", "at": "anchor/exit", "on": { "on": "use" },
                 "prop": { "block": "minecraft:barrel[facing=up]" },
                 "effects": [ { "type": "narrate", "text": "Empty.", "style": "chat" } ] }"#,
        )
        .expect("trigger parses"),
    ];
    c
}

fn build(c: &Campaign) -> BuildOutput {
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
        &BTreeMap::new(),
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
    .expect("the fixture builds, DW1013 included")
}

/// Every shipped `summon minecraft:interaction` line, as `(path, line)`.
fn summons(out: &BuildOutput) -> Vec<(String, String)> {
    let mut v = Vec::new();
    for (path, bytes) in out {
        if !path.starts_with("datapack/") || !path.ends_with(".mcfunction") {
            continue;
        }
        for line in std::str::from_utf8(bytes).unwrap().lines() {
            if line.contains("summon minecraft:interaction ") {
                v.push((path.clone(), line.to_string()));
            }
        }
    }
    v
}

#[test]
fn every_click_box_the_build_places_is_judged_and_wins() {
    let out = build(&fixture());
    let ledger: serde_json::Value = serde_json::from_slice(
        out.get("validation/click-box.json")
            .expect("the build writes the DW1013 ledger"),
    )
    .unwrap();
    // The second reading: a placed box is a fixture-class summon at absolute
    // coordinates; counted here by plain text, not by the check's parser.
    let all = summons(&out);
    let placed = all
        .iter()
        .filter(|(_, l)| l.contains("\"dw_fixture\"") && !l.contains(" ~"))
        .count();
    let borne = all
        .iter()
        .filter(|(_, l)| l.contains("\"dw_borne\""))
        .count();
    assert!(placed > 0, "the fixture places click boxes: {all:#?}");
    assert_eq!(ledger["summons"], all.len(), "{ledger}");
    assert_eq!(ledger["judged"], placed, "{ledger}");
    assert_eq!(ledger["borne"], borne, "{ledger}");
    // The families the fixture carries are each bound: the shortcut's unlock and
    // its sealed door.
    for family in ["dw_sc", "dw_ws"] {
        assert!(
            ledger["placed"][family].as_u64().unwrap_or(0) > 0,
            "`{family}` boxes are judged: {ledger}"
        );
    }
    let b = check(&out).expect("the shipped boxes all win the pick");
    assert_eq!(b.judged(), placed);
}

#[test]
fn the_full_prop_trigger_box_is_the_column_the_rule_builds() {
    let out = build(&campaign());
    let lines: Vec<String> = summons(&out)
        .into_iter()
        .map(|(_, l)| l)
        .filter(|l| l.contains("\"dw_trig_open_the_barrel\""))
        .collect();
    assert_eq!(lines.len(), 1, "{lines:#?}");
    assert!(
        lines[0].contains(" 64.99 ") && lines[0].contains("{width:1.02f,height:2.02f,"),
        "one margin past the barrel's faces and the head cell's: {}",
        lines[0]
    );
}

/// The perturbation, toward the shape the finding shipped with: the same build,
/// the barrel trigger's box planted back as the flush `1.0 × 2.0` at the cell
/// floor. Only `DW1013` reads a box's faces against the grid.
#[test]
fn the_same_build_with_a_flush_box_planted_is_refused() {
    let mut out = build(&campaign());
    let mut planted = 0;
    for bytes in out.values_mut() {
        let Ok(text) = std::str::from_utf8(bytes) else {
            continue;
        };
        if !text.contains("\"dw_trig_open_the_barrel\"") {
            continue;
        }
        let new: Vec<String> = text
            .lines()
            .map(|l| {
                if l.contains("summon minecraft:interaction ")
                    && l.contains("\"dw_trig_open_the_barrel\"")
                {
                    planted += 1;
                    l.replace(" 64.99 ", " 65.0 ")
                        .replace("width:1.02f,height:2.02f", "width:1.0f,height:2.0f")
                } else {
                    l.to_string()
                }
            })
            .collect();
        *bytes = format!("{}\n", new.join("\n")).into_bytes();
    }
    assert_eq!(planted, 1, "exactly the one box is planted");
    let e = check(&out).expect_err("a flush box ties the barrel and loses the click");
    assert_eq!(e.code, DW_CLICK_BOX_FLUSH);
    assert!(
        e.message.contains("dw_trig_open_the_barrel"),
        "{}",
        e.message
    );
}
