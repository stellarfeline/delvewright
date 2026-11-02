//! A click box fits its block (`DW1013`, spec-0117 criterion 17), on a real
//! build.
//!
//! A boxed pressable is a real block with a `minecraft:interaction` fitted over
//! its outline, so the crosshair's pick takes the box and never the block. These
//! tests build the `souls-shortcut` fixture and prove, over the shipped bytes:
//! every click box the build places is judged against the block it stands over
//! (`compiler::clickbox::check` green, its binding equal to a count taken here by
//! a second reading); a gated lever trigger's box is the fitted one; the same
//! build with that box written one margin short on its top face is refused
//! naming the face; and a box moved over a cell `setup_finish` leaves as air is
//! refused naming the cell.

mod common;

use std::collections::BTreeMap;

use delvec::compiler::clickbox::{DW_CLICK_BOX_FLUSH, check};
use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput, gate_blocks};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, EnvTrigger, parse_campaign};

const NS: &str = "souls-shortcut";
const LEVER_TAG: &str = "\"dw_trig_pull_the_bar\"";

/// The fixture as it stands: a shortcut's unlock button and its sealed door,
/// answered by the door's own bars.
fn fixture() -> Campaign {
    let dir = common::compiler_fixtures_dir().join(NS);
    let loaded = load_campaign_dir(&dir).unwrap();
    parse_campaign(&loaded.raw).expect("souls-shortcut parses")
}

/// The fixture with a gated lever trigger beside the door: boxed while its gate
/// is shut (spec-0117 §3a).
fn campaign() -> Campaign {
    let mut c = fixture();
    c.quests.content.triggers.push(
        serde_json::from_str::<EnvTrigger>(
            r#"{ "id": "trigger/pull-the-bar", "at": "anchor/door-knock", "on": { "on": "use" },
                 "requires_flags": ["flag/bar-free"],
                 "prop": { "block": "minecraft:lever[face=floor,facing=north]" },
                 "answers": { "not_yet": "The bar is chained." },
                 "effects": [ { "type": "narrate", "text": "Pulled.", "style": "chat" } ] }"#,
        )
        .expect("trigger parses"),
    );
    c
}

fn plan_and_build(c: &Campaign) -> (BTreeMap<[i32; 3], String>, BuildOutput) {
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
    let out = emit::build(
        &plan,
        &BTreeMap::new(),
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
    .expect("the fixture builds, DW1013 included");
    (gate_blocks(&plan), out)
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

/// Rewrite every summon carrying `tag` through `f`; how many were rewritten.
fn plant(out: &mut BuildOutput, tag: &str, f: impl Fn(&str) -> String) -> usize {
    let mut planted = 0;
    for bytes in out.values_mut() {
        let Ok(text) = std::str::from_utf8(bytes) else {
            continue;
        };
        if !text.contains(tag) {
            continue;
        }
        let new: Vec<String> = text
            .lines()
            .map(|l| {
                if l.contains("summon minecraft:interaction ") && l.contains(tag) {
                    planted += 1;
                    f(l)
                } else {
                    l.to_string()
                }
            })
            .collect();
        *bytes = format!("{}\n", new.join("\n")).into_bytes();
    }
    planted
}

#[test]
fn every_click_box_the_build_places_is_judged_against_its_block() {
    for c in [fixture(), campaign()] {
        let (gates, out) = plan_and_build(&c);
        let ledger: serde_json::Value = serde_json::from_slice(
            out.get("validation/click-box.json")
                .expect("the build writes the DW1013 ledger"),
        )
        .unwrap();
        // The second reading: a placed box is a fixture-class summon at
        // absolute coordinates; counted here by plain text, not by the check's
        // parser.
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
        // The sealed door's own answer box is judged against the door's bars.
        assert!(
            ledger["placed"]["dw_gate"].as_u64().unwrap_or(0) > 0,
            "the gate's own answer box is judged: {ledger}"
        );
        let b = check(&out, &gates).expect("every shipped box fits its block");
        assert_eq!(b.judged(), placed);
    }
}

#[test]
fn a_gated_lever_gets_the_fitted_box() {
    let (_, out) = plan_and_build(&campaign());
    let lines: Vec<String> = summons(&out)
        .into_iter()
        .map(|(_, l)| l)
        .filter(|l| l.contains(LEVER_TAG))
        .collect();
    assert!(!lines.is_empty(), "the lever is boxed");
    for l in &lines {
        assert!(
            l.contains("{width:0.52f,height:0.395f,") && l.contains(".99 "),
            "a floor lever's outline one margin out on every face: {l}"
        );
    }
}

/// The perturbation: the lever's box written one margin short on its top face,
/// so its top lies on the lever's own outline. Only `DW1013` reads a box's faces
/// against the block it stands over.
#[test]
fn a_box_one_margin_short_is_refused_naming_the_face() {
    let (gates, mut out) = plan_and_build(&campaign());
    let n = plant(&mut out, LEVER_TAG, |l| {
        l.replace("height:0.395f", "height:0.385f")
    });
    assert!(n > 0, "the lever's box is planted");
    let e = check(&out, &gates).expect_err("a box flush with the lever loses its top");
    assert_eq!(e.code, DW_CLICK_BOX_FLUSH);
    assert!(
        e.message.contains("top face") && e.message.contains("minecraft:lever"),
        "{}",
        e.message
    );
}

/// A box moved two cells up stands over a cell `setup_finish` leaves as air.
#[test]
fn a_box_over_air_is_refused_naming_the_cell() {
    let (gates, mut out) = plan_and_build(&campaign());
    let n = plant(&mut out, LEVER_TAG, |l| {
        let mut f: Vec<String> = l.split(' ').map(str::to_string).collect();
        let i = f
            .iter()
            .position(|w| w == "minecraft:interaction")
            .expect("a summon line");
        let y: f64 = f[i + 2].parse().unwrap();
        f[i + 2] = format!("{:.2}", y + 2.0);
        f.join(" ")
    });
    assert!(n > 0);
    let e = check(&out, &gates).expect_err("a box in the air");
    assert_eq!(e.code, DW_CLICK_BOX_FLUSH);
    assert!(e.message.contains("stands in the air"), "{}", e.message);
}
