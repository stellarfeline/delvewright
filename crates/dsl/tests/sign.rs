//! spec-0115: a sign says its words — the `signs[]` surface, `letter-sign`,
//! the inventory rows a sign line enters, and the refusals the DSL tier
//! raises (`DW0100`, `DW0110`, `DW0111`, `DW0112`, `DW0193`, `DW0435`).
//!
//! Built on the hello-world fixture campaign, like `v04.rs`: one literal
//! quests document, perturbed one construct per test.

mod common;

use delvewright_dsl::{RawCampaign, check_campaign};

fn campaign(quests: &str) -> RawCampaign {
    RawCampaign {
        quests: quests.to_string(),
        ..common::valid_raw()
    }
}

/// The hello-world quests document with a sign on `anchor/exit`, a trigger
/// prop placing it, and a beat re-lettering it. `%SIGNS%`, `%EFFECT%` and
/// `%PROP%` are the slots the tests perturb.
const BASE: &str = r#"{
  "dsl_version": "%dsl_version%",
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
        "on_objective_complete": {
          "obj/talk": [ { "type": "open-gate", "anchor": "anchor/door" }%EFFECT% ]
        },
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ],
    "triggers": [
      { "id": "trigger/read", "at": "anchor/exit", "on": { "on": "use" },
        "prop": { "block": "%PROP%" },
        "effects": [ { "type": "narrate", "text": "It reads." } ] }
    ],
    "signs": [ %SIGNS% ]
  }
}"#;

const SIGN: &str = r#"{ "id": "sign/board", "anchor": "anchor/exit",
  "front": { "lines": ["THE DOOR", "", "is open"] },
  "back": { "lines": ["", "[[bold|go on]]"], "color": "red", "glowing": true } }"#;

const LETTER: &str = r#", { "type": "letter-sign", "sign": "sign/board",
  "front": { "lines": ["", "SHUT"], "color": "blue" } }"#;

fn quests(signs: &str, effect: &str, prop: &str) -> String {
    common::at_dsl_version(BASE)
        .replace("%SIGNS%", signs)
        .replace("%EFFECT%", effect)
        .replace("%PROP%", prop)
}

fn codes(q: &str) -> Vec<String> {
    check_campaign(&campaign(q))
        .into_iter()
        .map(|d| d.code.to_string())
        .collect()
}

const PROP: &str = "minecraft:oak_sign[rotation=8]";

#[test]
fn a_declared_sign_and_its_beat_validate_clean() {
    let diags = check_campaign(&campaign(&quests(SIGN, LETTER, PROP)));
    assert!(diags.is_empty(), "{diags:#?}");
}

#[test]
fn the_id_is_checked_for_shape_and_uniqueness() {
    let bad = SIGN.replace("sign/board", "board");
    assert!(codes(&quests(&bad, "", PROP)).contains(&"DW0110".to_string()));
    let twice = format!("{SIGN}, {}", SIGN.replace("anchor/exit", "anchor/door"));
    assert!(codes(&quests(&twice, "", PROP)).contains(&"DW0111".to_string()));
}

#[test]
fn two_rows_on_one_anchor_are_dw0435() {
    let twice = format!("{SIGN}, {}", SIGN.replace("sign/board", "sign/again"));
    assert!(codes(&quests(&twice, "", PROP)).contains(&"DW0435".to_string()));
}

#[test]
fn a_beat_naming_no_sign_is_dw0112_and_one_stating_no_face_is_dw0100() {
    let nobody = LETTER.replace("sign/board", "sign/nobody");
    assert!(codes(&quests(SIGN, &nobody, PROP)).contains(&"DW0112".to_string()));
    let empty = r#", { "type": "letter-sign", "sign": "sign/board" }"#;
    assert!(codes(&quests(SIGN, empty, PROP)).contains(&"DW0100".to_string()));
}

#[test]
fn a_fifth_line_an_empty_face_and_a_colour_no_dye_has_are_dw0100() {
    let five = SIGN.replace(
        r#"["THE DOOR", "", "is open"]"#,
        r#"["a", "b", "c", "d", "e"]"#,
    );
    assert!(codes(&quests(&five, "", PROP)).contains(&"DW0100".to_string()));
    let none = SIGN.replace(r#"["THE DOOR", "", "is open"]"#, "[]");
    assert!(codes(&quests(&none, "", PROP)).contains(&"DW0100".to_string()));
    let purple = SIGN.replace(r#""red""#, r#""dark_purple""#);
    assert!(codes(&quests(&purple, "", PROP)).contains(&"DW0100".to_string()));
}

/// spec-0115 §3.5: a `{` in a block string is refused at every block field
/// through the one parser — a trigger's `prop`, a `set-block`, a
/// `fill-region` — and the message names `signs[]`.
#[test]
fn words_typed_into_a_block_are_dw0193_at_prop_set_block_and_fill_region() {
    let nbt = "minecraft:oak_sign[rotation=8]{front_text:{}}";
    let set_block =
        format!(r#", {{ "type": "set-block", "anchor": "anchor/exit", "block": "{nbt}" }}"#);
    let fill = format!(
        r#", {{ "type": "fill-region", "region": {{ "anchor": "anchor/exit", "extent": [0, 0, 0] }}, "block": "{nbt}" }}"#
    );
    for (what, q) in [
        ("prop", quests("", "", nbt)),
        ("set-block", quests("", &set_block, PROP)),
        ("fill-region", quests("", &fill, PROP)),
    ] {
        let diags = check_campaign(&campaign(&q));
        let hit = diags
            .iter()
            .find(|d| d.code == "DW0193" && d.message.contains("signs[]"));
        assert!(hit.is_some(), "{what}: {diags:#?}");
    }
}

/// Every non-blank line is an inventory row of kind `sign-line`, a blank one
/// is none, and the situation carries the face's other lines and its budget
/// from the kind of sign the document writes at the anchor.
#[test]
fn every_non_blank_line_is_a_sign_line_row_with_its_budget() {
    let c =
        delvewright_dsl::parse_campaign(&campaign(&quests(SIGN, LETTER, PROP))).expect("parses");
    let inv = delvewright_dsl::inventory(&c);
    let rows: Vec<&String> = inv
        .keys()
        .filter(|k| delvewright_dsl::key_kind(k) == Some(delvewright_dsl::TextKind::SignLine))
        .collect();
    assert_eq!(
        rows,
        [
            "fx.open-the-door.oc.talk.1.front.1",
            "sign.board.back.1",
            "sign.board.front.0",
            "sign.board.front.2",
        ],
        "{inv:#?}"
    );
    assert!(
        !inv.contains_key("sign.board.front.1"),
        "a blank line is no row"
    );
    let sit = delvewright_dsl::key_situations(&c);
    let front = &sit["sign.board.front.0"];
    assert!(
        front.iter().any(|l| l.contains("line 3: is open")),
        "{front:?}"
    );
    assert!(
        front
            .iter()
            .any(|l| l.contains("90 font pixels: 15 Latin or 10 Han")),
        "{front:?}"
    );
}
