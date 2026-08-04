//! `DW0331` — a dialogue option label wider than the button vanilla draws it on.
//!
//! Engine task #110. A dialogue option is a button caption: `build_node_dialog`
//! emits a `minecraft:multi_action` with no `width` override, so every button is
//! vanilla's default 150 GUI px and a label over the 146 usable px *scrolls*. The
//! budget is the widget's geometry, not the player's window — so unlike `DW0330`
//! this rejects rather than advises.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::textfit::{
    self, BUTTON_LABEL_BUDGET, DIALOG_BUTTON_WIDTH, DW_OPTION_LABEL_SCROLLS,
};
use delvewright_dsl::{
    Campaign, L10nDoc, RawCampaign, Severity, dialogue_option_labels, parse_campaign,
};

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

/// A one-NPC, one-node dialogue whose single option carries `label`. The node
/// completes `obj/talk`, so the campaign stays structurally valid.
fn dialogue_doc(label: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "0.2.0",
  "campaign_id": "hello-world",
  "stage": "dialogue",
  "content": {{
    "dialogues": [
      {{
        "npc": "npc/keeper",
        "root": "dlg/greeting",
        "nodes": [
          {{
            "id": "dlg/greeting",
            "text": "Halt, traveler. This keep is mine to guard, and the door stays shut.",
            "options": [
              {{
                "label": {},
                "effects": [ {{ "type": "complete-objective", "objective": "obj/talk" }} ]
              }}
            ]
          }}
        ]
      }}
    ]
  }}
}}"#,
        serde_json::Value::String(label.to_string())
    )
}

/// Parse hello-world with a custom `dialogue` doc (and optionally a custom `world`
/// doc, e.g. to declare l10n languages).
fn parse_hw(dialogue: &str, world: Option<String>) -> Campaign {
    let raw = RawCampaign {
        world: world.unwrap_or_else(|| read_hw("world.json")),
        npcs: read_hw("npcs.json"),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: read_hw("quests.json"),
        dialogue: dialogue.to_string(),
        world_edits: None,
    };
    parse_campaign(&raw).expect("campaign parses")
}

/// The budget is arithmetic on the widget, not a tunable: 150 GUI px of button less
/// 2 px of inset per side, at pose scale ×1 (so a font pixel is a GUI pixel). If
/// `build_node_dialog` ever emits a `width`, this is the test that must change with
/// it — the number may not drift on its own.
#[test]
fn the_button_budget_is_the_vanilla_widget_geometry() {
    assert_eq!(DIALOG_BUTTON_WIDTH, 150);
    assert_eq!(BUTTON_LABEL_BUDGET, 146);
    // ×1, not the ×4/×2 of a narrate title: the button budget is far larger in font
    // pixels than a title's, which is why a title lint cannot stand in for this one.
    assert!(BUTTON_LABEL_BUDGET > textfit::budget(delvewright_dsl::NarrateStyle::Title));
}

/// A caption-shaped label — what the authoring rule asks for — stays quiet.
#[test]
fn short_option_label_fits_clean() {
    let c = parse_hw(&dialogue_doc("Open the door."), None);
    assert!(
        textfit::check_option_labels(&c, &BTreeMap::new()).is_empty(),
        "a caption-length label must fit its button"
    );
}

/// The boundary is exact and inclusive: a label measuring exactly the budget fits,
/// one font pixel more scrolls. Pinned with real strings so the off-by-one cannot
/// be argued away.
#[test]
fn the_boundary_is_exact_and_inclusive() {
    // 24 `A`s + 1 `i` = 24*6 + 2 = 146 — exactly the budget.
    let at = format!("{}i", "A".repeat(24));
    assert_eq!(textfit::default_font_width(&at), BUTTON_LABEL_BUDGET);
    assert!(
        textfit::check_option_labels(&parse_hw(&dialogue_doc(&at), None), &BTreeMap::new())
            .is_empty(),
        "a label exactly at the budget fits — the check is `>`, not `>=`"
    );
    // One glyph wider: 147 px.
    let over = format!("{}l", "A".repeat(24));
    assert_eq!(textfit::default_font_width(&over), BUTTON_LABEL_BUDGET + 1);
    assert!(
        !textfit::check_option_labels(&parse_hw(&dialogue_doc(&over), None), &BTreeMap::new())
            .is_empty(),
        "one font pixel over the budget scrolls"
    );
}

/// An over-long English label fires `DW0331` at **error** severity. `DW0330` warns
/// because the screen width is a guess about the player's window; the 150 px button
/// is the geometry of the dialog this compiler emits, so this is a fact about the
/// shipped datapack and rejects the build.
#[test]
fn overlong_english_label_is_dw0331_error() {
    let c = parse_hw(
        &dialogue_doc("I don't know — are you sure there isn't another way out of the cave?"),
        None,
    );
    let d = textfit::check_option_labels(&c, &BTreeMap::new());
    assert!(
        d.iter()
            .any(|x| x.code == "DW0331" && x.severity == Severity::Error),
        "an over-long option label must be a DW0331 error: {d:#?}"
    );
    let diag = d
        .iter()
        .find(|x| x.code == DW_OPTION_LABEL_SCROLLS)
        .unwrap();
    assert_eq!(diag.stage, "dialogue");
    assert!(
        diag.path
            .contains("/content/dialogues/0/nodes/0/options/0/label"),
        "the finding names the exact option: {}",
        diag.path
    );
}

/// Measured in pixels, never characters — the reason this reuses `DW0330`'s font
/// metrics. Two labels of the same character count fall on opposite sides of the
/// budget because `i` and `W` differ by 3×.
#[test]
fn the_budget_is_pixels_not_characters() {
    let narrow = "i".repeat(40); // 40 chars, 80 px
    let wide = "W".repeat(40); // 40 chars, 240 px
    assert_eq!(narrow.chars().count(), wide.chars().count());
    assert!(
        textfit::check_option_labels(&parse_hw(&dialogue_doc(&narrow), None), &BTreeMap::new())
            .is_empty(),
        "40 narrow glyphs fit — a character count would have rejected them"
    );
    assert!(
        !textfit::check_option_labels(&parse_hw(&dialogue_doc(&wide), None), &BTreeMap::new())
            .is_empty(),
        "40 wide glyphs do not fit"
    );
}

/// The l10n half, and the case that motivates per-language checking: the English
/// source fits its button, the `zh-cn` rendition does not — a Han glyph is 1.5× a
/// Latin letter, so a faithful translation of a label near the limit overruns it.
/// The finding must name the locale and the l10n key, not the English source.
#[test]
fn overlong_zh_label_translation_is_dw0331_naming_the_language() {
    let mut world: serde_json::Value = serde_json::from_str(&read_hw("world.json")).unwrap();
    world["content"]["languages"] = serde_json::json!(["zh-cn"]);
    let c = parse_hw(&dialogue_doc("Another way out?"), Some(world.to_string()));
    assert!(
        textfit::check_option_labels(&c, &BTreeMap::new()).is_empty(),
        "the English source must fit"
    );

    let key = dialogue_option_labels(&c)[0].key.clone();
    assert_eq!(key, "dlg.keeper.greeting.opt.0.label");
    let doc: L10nDoc = serde_json::from_value(serde_json::json!({
        "dsl_version": "0.6.0",
        "campaign_id": "hello-world",
        "kind": "l10n",
        "lang": "zh-cn",
        "content": { key.clone(): "我不太确定，你是说这座洞窟还有别的出口吗？" }
    }))
    .unwrap();
    let mut sidecars = BTreeMap::new();
    sidecars.insert("zh-cn".to_string(), doc);

    let d = textfit::check_option_labels(&c, &sidecars);
    assert!(
        d.iter().any(|x| x.code == "DW0331"
            && x.severity == Severity::Error
            && x.stage == "l10n"
            && x.path.contains("zh-cn")
            && x.path.contains(&key)),
        "an over-wide zh label must be DW0331 against the sidecar, naming the language \
         and the key: {d:#?}"
    );

    // …and a translation that respects the budget is clean, so the check is not
    // simply rejecting Chinese.
    let short: L10nDoc = serde_json::from_value(serde_json::json!({
        "dsl_version": "0.6.0",
        "campaign_id": "hello-world",
        "kind": "l10n",
        "lang": "zh-cn",
        "content": { key.clone(): "还有别的出口吗？" }
    }))
    .unwrap();
    let mut ok = BTreeMap::new();
    ok.insert("zh-cn".to_string(), short);
    assert!(
        textfit::check_option_labels(&c, &ok).is_empty(),
        "a caption-length zh translation fits"
    );
}

/// A sidecar for a language the campaign never declared is not this check's
/// business (nor anything the build ships), so it must not raise a finding.
#[test]
fn undeclared_language_sidecar_is_ignored() {
    let c = parse_hw(&dialogue_doc("Another way out?"), None);
    let key = dialogue_option_labels(&c)[0].key.clone();
    let doc: L10nDoc = serde_json::from_value(serde_json::json!({
        "dsl_version": "0.6.0",
        "campaign_id": "hello-world",
        "kind": "l10n",
        "lang": "zh-cn",
        "content": { key: "我不太确定，你是说这座洞窟还有别的出口吗？" }
    }))
    .unwrap();
    let mut sidecars = BTreeMap::new();
    sidecars.insert("zh-cn".to_string(), doc);
    assert!(
        textfit::check_option_labels(&c, &sidecars).is_empty(),
        "a sidecar for an undeclared language is not checked"
    );
}

/// **Every** campaign fixture in this repo — the DSL's valid corpus and the
/// compiler's own test campaigns — must satisfy the lint it now ships, sidecars
/// included. Fixtures are what future authoring sessions copy from, so a fixture
/// that violates the rule teaches the violation. Enumerated from disk rather than
/// listed, so a fixture added later cannot quietly skip the gate.
#[test]
fn every_engine_fixture_fits_its_buttons() {
    let roots = [
        common::repo_root().join("crates/dsl/fixtures/valid"),
        common::repo_root().join("crates/compiler/tests/fixtures"),
    ];
    let mut checked = 0;
    for root in roots {
        let mut dirs: Vec<_> = std::fs::read_dir(&root)
            .unwrap_or_else(|e| panic!("{} readable: {e}", root.display()))
            .map(|e| e.unwrap().path())
            .filter(|p| p.join("dialogue.json").is_file())
            .collect();
        dirs.sort();
        for dir in dirs {
            let loaded = delvewright_compiler::load::load_campaign_dir(&dir)
                .unwrap_or_else(|e| panic!("{} loads: {e:?}", dir.display()));
            let Ok(c) = parse_campaign(&loaded.raw) else {
                continue; // patch-style / deliberately-invalid fixtures are not ours
            };
            let mut sidecars = BTreeMap::new();
            for (lang, bytes) in &loaded.l10n {
                if let Ok(doc) = serde_json::from_slice::<L10nDoc>(bytes) {
                    sidecars.insert(lang.clone(), doc);
                }
            }
            let d = textfit::check_option_labels(&c, &sidecars);
            assert!(
                d.is_empty(),
                "{} has option labels that scroll: {d:#?}",
                dir.display()
            );
            checked += 1;
        }
    }
    assert!(checked >= 10, "the fixture corpus must actually be walked");
}

// ---------------------------------------------------------------------------
// The v0.8 `tooltip` is deliberately OUT of scope
// ---------------------------------------------------------------------------

/// The same node, at 0.8.0, with a caption-length `label` and a sentence-length
/// `tooltip` — the wine-beat pattern.
fn dialogue_doc_with_tooltip(label: &str, tooltip: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "0.8.0",
  "campaign_id": "hello-world",
  "stage": "dialogue",
  "content": {{
    "dialogues": [
      {{
        "npc": "npc/keeper",
        "root": "dlg/greeting",
        "nodes": [
          {{
            "id": "dlg/greeting",
            "text": "Halt, traveler. This keep is mine to guard, and the door stays shut.",
            "options": [
              {{
                "label": {},
                "tooltip": {},
                "effects": [ {{ "type": "complete-objective", "objective": "obj/talk" }} ]
              }}
            ]
          }}
        ]
      }}
    ]
  }}
}}"#,
        serde_json::Value::String(label.to_string()),
        serde_json::Value::String(tooltip.to_string())
    )
}

/// `DW0331` measures the **button**, and a tooltip is not drawn on one.
///
/// Ground truth from the pinned 1.21.11 client jar: the dialog control set turns a
/// present `tooltip` into `Tooltip.create(component)`, and `Tooltip` splits its
/// text with `Font.split(message, 170)` — it *wraps* into a hover box. The failure
/// `DW0331` exists to reject is `renderScrollingString` sliding a caption back and
/// forth across a fixed 150 px button; nothing about a wrapped hover box does that,
/// so there is no budget here to enforce and inventing one would forbid exactly the
/// pattern the field was added for.
#[test]
fn a_long_tooltip_is_not_dw0331() {
    // Far past the button budget — as a label this exact string is rejected below.
    let line = "And who are you, to come knocking at a door shut for thirty winters?";
    assert!(textfit::default_font_width(line) > BUTTON_LABEL_BUDGET);
    let c = parse_hw(&dialogue_doc_with_tooltip("Who are you?", line), None);
    assert!(
        textfit::check_option_labels(&c, &BTreeMap::new()).is_empty(),
        "a tooltip wraps at 170 px in its own box — it has no button budget to overrun"
    );
    // The control: the same string as the *caption* is still an error, so the
    // exemption is the tooltip's, not this string's.
    assert!(
        textfit::check_option_labels(&parse_hw(&dialogue_doc(line), None), &BTreeMap::new())
            .iter()
            .any(|x| x.code == DW_OPTION_LABEL_SCROLLS),
        "the same line on the button still scrolls"
    );
    // And the caption on the same option is measured as it always was.
    let over = format!("{}l", "A".repeat(24));
    assert!(
        textfit::check_option_labels(
            &parse_hw(&dialogue_doc_with_tooltip(&over, line), None),
            &BTreeMap::new()
        )
        .iter()
        .any(|x| x.code == DW_OPTION_LABEL_SCROLLS),
        "an over-wide caption is still rejected when the option also has a tooltip"
    );
}

/// The l10n half of the same scope rule: a translated tooltip is not a caption
/// either, so a `zh-cn` rendition of one raises nothing however long it is.
#[test]
fn a_long_tooltip_translation_is_not_dw0331() {
    let mut world: serde_json::Value = serde_json::from_str(&read_hw("world.json")).unwrap();
    world["content"]["languages"] = serde_json::json!(["zh-cn"]);
    let c = parse_hw(
        &dialogue_doc_with_tooltip("Who are you?", "And who are you, exactly?"),
        Some(world.to_string()),
    );
    // The width check's inventory is the *label* inventory — a tooltip never
    // enters it, which is what keeps the two surfaces from drifting.
    let keys: Vec<String> = dialogue_option_labels(&c)
        .into_iter()
        .map(|l| l.key)
        .collect();
    assert_eq!(keys, vec!["dlg.keeper.greeting.opt.0.label".to_string()]);

    let doc: L10nDoc = serde_json::from_value(serde_json::json!({
        "dsl_version": "0.6.0",
        "campaign_id": "hello-world",
        "kind": "l10n",
        "lang": "zh-cn",
        "content": {
            "dlg.keeper.greeting.opt.0.label": "你是谁？",
            "dlg.keeper.greeting.opt.0.tooltip":
                "你又是谁，敢来敲这扇三十个寒冬都没有开过的大门？"
        }
    }))
    .unwrap();
    let mut sidecars = BTreeMap::new();
    sidecars.insert("zh-cn".to_string(), doc);
    assert!(
        textfit::check_option_labels(&c, &sidecars).is_empty(),
        "a long zh tooltip is not a scrolling caption"
    );
}
