//! spec-0128 §8: the stage-5 `signs` section — the words of a sign the piece
//! placed.
//!
//! This tier checks what the document alone decides: the id (`DW0110`/
//! `DW0111`), the anchor (`DW0142`) and the lines a side holds, restated at the
//! schema tier (`DW0100`). Whether the anchor's cell holds a sign, and whether
//! each line fits its board, need the assembled world: the build-tier proof
//! `DW1075` is covered in the compiler crate (`compiler::sign`).

mod common;

use delvewright_dsl::{
    DSL_VERSION, RawCampaign, TextKind, check_campaign, key_kind, key_situations, parse_campaign,
};

const QUESTS_TMPL: &str = r#"{
  "dsl_version": "{VER}",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {
    "quests": [
      {
        "id": "quest/open-the-door",
        "trigger": { "type": "campaign-start" },
        "objectives": [
          { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" },
          { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit", "radius": 2, "after": ["obj/talk"] }
        ],
        "on_objective_complete": { "obj/talk": [ { "type": "open-gate", "anchor": "anchor/door" } ] },
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ],
    "signs": [ {SIGNS} ]
  }
}"#;

const VALID: &str = r#"{
  "id": "sign/exit",
  "anchor": "anchor/exit",
  "front": ["", "Way out"],
  "back": ["Keeper's hall"]
}"#;

fn with_signs(signs: &str) -> RawCampaign {
    let quests = QUESTS_TMPL
        .replacen("{VER}", DSL_VERSION, 1)
        .replacen("{SIGNS}", signs, 1);
    RawCampaign {
        world: common::read_valid("world.json"),
        npcs: common::read_valid("npcs.json"),
        classes: common::read_valid("classes.json"),
        quest_plan: common::read_valid("quest-plan.json"),
        quests,
        dialogue: common::read_valid("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    }
}

fn codes(c: &RawCampaign) -> Vec<String> {
    check_campaign(c).iter().map(|d| d.code.clone()).collect()
}

#[test]
fn a_sign_text_validates_clean() {
    let d = check_campaign(&with_signs(VALID));
    assert!(d.is_empty(), "{d:#?}");
}

/// Every lettered line is inventoried under its side and index, a blank
/// line is not, and each key is of kind `sign` with a situation naming the
/// sign's anchor and its whole side.
#[test]
fn every_lettered_line_is_inventoried_as_a_sign_line() {
    let c = parse_campaign(&with_signs(VALID)).unwrap();
    let inv = delvewright_dsl::l10n::inventory(&c);
    let keys: Vec<&str> = inv
        .keys()
        .map(String::as_str)
        .filter(|k| k.starts_with("sign."))
        .collect();
    assert_eq!(keys, ["sign.exit.back.0", "sign.exit.front.1"]);
    for k in &keys {
        assert_eq!(key_kind(k), Some(TextKind::Sign), "{k}");
    }
    let sit = key_situations(&c);
    let front = sit["sign.exit.front.1"].join("\n");
    assert!(front.contains("Line 2 of 2 on the front"), "{front}");
    assert!(front.contains("`anchor/exit`"), "{front}");
    assert!(front.contains("The whole front: Way out"), "{front}");
}

#[test]
fn a_sign_on_an_anchor_no_prefab_provides_is_dw0142() {
    let s = VALID.replacen("anchor/exit", "anchor/nowhere", 1);
    assert!(codes(&with_signs(&s)).contains(&"DW0142".to_string()));
}

#[test]
fn a_malformed_or_duplicate_sign_id_is_refused() {
    let s = VALID.replacen("sign/exit", "sign/Exit", 1);
    assert!(codes(&with_signs(&s)).contains(&"DW0110".to_string()));
    assert!(codes(&with_signs(&format!("{VALID}, {VALID}"))).contains(&"DW0111".to_string()));
}

/// A side holds four lines; a fifth, or a front with none, is `DW0100`.
#[test]
fn a_side_holds_one_to_four_lines() {
    let five = VALID.replacen(
        r#""front": ["", "Way out"]"#,
        r#""front": ["a", "b", "c", "d", "e"]"#,
        1,
    );
    let d = check_campaign(&with_signs(&five));
    assert!(
        d.iter()
            .any(|x| x.code == "DW0100" && x.path == "/content/signs/0/front"),
        "{d:#?}"
    );
    let back = VALID.replacen(
        r#""back": ["Keeper's hall"]"#,
        r#""back": ["a", "b", "c", "d", "e"]"#,
        1,
    );
    assert!(codes(&with_signs(&back)).contains(&"DW0100".to_string()));
    let empty = VALID.replacen(r#""front": ["", "Way out"]"#, r#""front": []"#, 1);
    assert!(codes(&with_signs(&empty)).contains(&"DW0100".to_string()));
    let four = VALID.replacen(
        r#""front": ["", "Way out"]"#,
        r#""front": ["a", "b", "c", "d"]"#,
        1,
    );
    assert!(check_campaign(&with_signs(&four)).is_empty());
}
