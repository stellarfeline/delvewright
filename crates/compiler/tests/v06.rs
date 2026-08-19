//! DSL v0.6 (spec-0014) compiler-side surface: sound-event validation (`DW0326`),
//! the deferred `play-sound at: actor` gate (`DW0335`), art-title glyph coverage
//! over source + l10n translations (`DW0328`), and the `delve:art` resource-pack
//! font — baked deterministically into the pack, byte-identical across builds.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::atmos;
use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_compiler::textfit;
use delvewright_dsl::{
    Campaign, L10nDoc, RawCampaign, Severity, art_narrates, on_screen_narrates, parse_campaign,
};

/// A v0.6 `quests` document whose single quest fires the given `on_complete`
/// effects (a raw JSON array body, without the surrounding brackets).
fn quests_doc(on_complete: &str) -> String {
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

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

/// Parse a hello-world campaign with a custom `quests` doc (and optionally a
/// custom `world` doc, e.g. to declare l10n languages).
fn parse_hw(quests: &str, world: Option<String>) -> Campaign {
    let raw = RawCampaign {
        world: world.unwrap_or_else(|| read_hw("world.json")),
        npcs: read_hw("npcs.json"),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: quests.to_string(),
        dialogue: read_hw("dialogue.json"),
        world_edits: None,
    };
    parse_campaign(&raw).expect("campaign parses")
}

// --- DW0326: sound-event validation --------------------------------------

#[test]
fn known_sounds_validate_clean() {
    let c = parse_hw(
        &quests_doc(
            r#"{ "type": "play-sound", "sound": "minecraft:ui.toast.challenge_complete" },
               { "type": "play-sound", "sound": "entity.experience_orb.pickup",
                 "at": { "at": "anchor", "anchor": "anchor/door" }, "volume": 0.8, "pitch": 1.5 },
               { "type": "narrate", "text": "hi", "sound": "block.note_block.pling" },
               { "type": "campaign-complete" }"#,
        ),
        None,
    );
    assert!(
        atmos::check_sounds(&c).is_empty(),
        "known sounds must validate clean"
    );
}

#[test]
fn unknown_play_sound_is_dw0326() {
    let c = parse_hw(
        &quests_doc(
            r#"{ "type": "play-sound", "sound": "minecraft:not.a.real.sound" },
               { "type": "campaign-complete" }"#,
        ),
        None,
    );
    let d = atmos::check_sounds(&c);
    assert!(
        d.iter().any(|x| x.code == atmos::DW_SOUND_UNKNOWN),
        "unknown play-sound must be DW0326: {d:#?}"
    );
}

#[test]
fn unknown_narrate_sound_is_dw0326() {
    let c = parse_hw(
        &quests_doc(
            r#"{ "type": "narrate", "text": "hi", "sound": "made.up.sound" },
               { "type": "campaign-complete" }"#,
        ),
        None,
    );
    let d = atmos::check_sounds(&c);
    assert!(
        d.iter().any(|x| x.code == "DW0326"),
        "unknown narrate.sound must be DW0326: {d:#?}"
    );
}

// --- DW0335: deferred play-sound at: actor -------------------------------

#[test]
fn play_sound_at_actor_is_dw0335() {
    let c = parse_hw(
        &quests_doc(
            r#"{ "type": "play-sound", "sound": "minecraft:ui.toast.challenge_complete",
                 "at": { "at": "actor", "actor": "actor/giant" } },
               { "type": "campaign-complete" }"#,
        ),
        None,
    );
    let d = atmos::check_sounds(&c);
    assert!(
        d.iter()
            .any(|x| x.code == atmos::DW_PLAYSOUND_ACTOR_DEFERRED),
        "play-sound at: actor must be DW0335 until actors land: {d:#?}"
    );
}

// --- DW0328: art-title glyph coverage ------------------------------------

#[test]
fn ascii_art_title_validates_clean() {
    let c = parse_hw(
        &quests_doc(
            r#"{ "type": "narrate", "text": "YOU WIN! (100)", "style": "art" },
               { "type": "campaign-complete" }"#,
        ),
        None,
    );
    assert!(
        atmos::check_art(&c, &BTreeMap::new()).is_empty(),
        "an ASCII art title must validate clean"
    );
}

#[test]
fn non_latin_art_source_is_dw0328() {
    let c = parse_hw(
        &quests_doc(
            r#"{ "type": "narrate", "text": "胜利", "style": "art" },
               { "type": "campaign-complete" }"#,
        ),
        None,
    );
    let d = atmos::check_art(&c, &BTreeMap::new());
    assert!(
        d.iter().any(|x| x.code == atmos::DW_ART_GLYPH_UNCOVERED),
        "a non-Latin art source must be DW0328: {d:#?}"
    );
}

/// The l10n interaction: an `art`-styled narrate goes through the l10n key
/// inventory, so a `zh-cn` sidecar translation that cannot render in the Latin art
/// font fails `DW0328` — forcing per-language art titles to stay ASCII/Latin.
#[test]
fn zh_art_translation_is_dw0328() {
    let mut world: serde_json::Value = serde_json::from_str(&read_hw("world.json")).unwrap();
    world["content"]["languages"] = serde_json::json!(["zh-cn"]);
    let c = parse_hw(
        &quests_doc(
            r#"{ "type": "narrate", "text": "VICTORY", "style": "art" },
               { "type": "campaign-complete" }"#,
        ),
        Some(world.to_string()),
    );
    // The art narrate's l10n key, translated to non-renderable Han in the sidecar.
    let key = art_narrates(&c)[0].key.clone();
    let sidecar_json = serde_json::json!({
        "dsl_version": "0.6.0",
        "campaign_id": "hello-world",
        "kind": "l10n",
        "lang": "zh-cn",
        "content": { key: "\u{80dc}\u{5229}" }
    });
    let doc: L10nDoc = serde_json::from_value(sidecar_json).unwrap();
    let mut sidecars = BTreeMap::new();
    sidecars.insert("zh-cn".to_string(), doc);

    let d = atmos::check_art(&c, &sidecars);
    assert!(
        d.iter()
            .any(|x| x.code == "DW0328" && x.stage == "l10n" && x.path.contains("zh-cn")),
        "a Han art translation must be DW0328 against the l10n sidecar: {d:#?}"
    );
}

// --- Emission + deterministic art font in the resource pack --------------

fn build_v06(quests: &str) -> BuildOutput {
    let campaign = parse_hw(&quests_doc(quests), None);
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
        &load_campaign_dir(&common::hello_world_dir())
            .unwrap()
            .inputs,
        &structures,
        &tree,
        &prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
    .expect("v0.6 campaign builds")
}

const SHOWCASE: &str = r#"{ "type": "play-sound", "sound": "minecraft:ui.toast.challenge_complete",
     "at": { "at": "anchor", "anchor": "anchor/door" }, "volume": 0.9, "pitch": 1.2 },
   { "type": "play-sound", "sound": "entity.experience_orb.pickup", "at": { "at": "players" } },
   { "type": "narrate", "text": "You Win", "style": "art" },
   { "type": "campaign-complete" }"#;

#[test]
fn play_sound_and_art_emitted() {
    let out = build_v06(SHOWCASE);
    let all: String = out
        .iter()
        .filter(|(p, _)| p.ends_with(".mcfunction"))
        .map(|(_, b)| String::from_utf8_lossy(b).into_owned())
        .collect::<Vec<_>>()
        .join("\n");
    // spec-0018: a quest beat's sound reaches the whole party. An `at: anchor`
    // sound carries absolute coordinates, so `@a` hears it in one place.
    assert!(
        all.contains("playsound minecraft:ui.toast.challenge_complete master @a"),
        "anchor play-sound emitted with positioned targets"
    );
    assert!(
        all.contains("playsound minecraft:entity.experience_orb.pickup master @a"),
        "players play-sound emitted"
    );
    assert!(
        all.contains("delve:art"),
        "art title emitted with the delve:art font component"
    );
}

/// Build a v0.6 campaign after localizing its strings with `translations`, the way
/// `delvec build --lang <code>` does (localize in place, then plan + emit).
fn build_localized_v06(quests: &str, translations: &BTreeMap<String, String>) -> BuildOutput {
    let mut campaign = parse_hw(&quests_doc(quests), None);
    delvewright_dsl::localize(&mut campaign, translations);
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
        &load_campaign_dir(&common::hello_world_dir())
            .unwrap()
            .inputs,
        &structures,
        &tree,
        &prefabs,
        Some("zh-cn"),
        "unpinned",
        &BTreeMap::new(),
    )
    .expect("localized v0.6 campaign builds")
}

/// A `narrate` nested inside a `sequence` step (the Q4/Q7 cinematic shape).
const SEQUENCE_NARRATE: &str = r#"{ "type": "sequence", "steps": [
       { "at_ticks": 20, "effects": [ { "type": "narrate", "text": "The seal cracks open." } ] }
     ] },
   { "type": "campaign-complete" }"#;

/// A `narrate` nested in a `sequence` is localized on the **emission** path: a
/// translated build emits the translated line, not the English source. Regression
/// for the cinematic narration that shipped English-only in `zh-cn`.
#[test]
fn sequence_narrate_is_localized_on_emission() {
    // The nested key for on_complete effect 0 (sequence), step 0, nested effect 0.
    let key = "fx.open-the-door.done.0.seq.0.0.narrate";
    let mut tr = BTreeMap::new();
    tr.insert(key.to_string(), "封印裂开了。".to_string());

    let out = build_localized_v06(SEQUENCE_NARRATE, &tr);
    let all: String = out
        .iter()
        .filter(|(p, _)| p.ends_with(".mcfunction"))
        .map(|(_, b)| String::from_utf8_lossy(b).into_owned())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        all.contains("封印裂开了。"),
        "the translated sequence narrate must be emitted:\n{all}"
    );
    assert!(
        !all.contains("The seal cracks open."),
        "the English source of a translated nested narrate must not ship:\n{all}"
    );

    // Determinism (ADR-0006): the localized build is byte-identical twice.
    let again = build_localized_v06(SEQUENCE_NARRATE, &tr);
    assert_eq!(out, again, "localized build is byte-identical across runs");
}

#[test]
fn art_font_baked_into_resource_pack() {
    let out = build_v06(SHOWCASE);
    assert!(
        out.contains_key("resourcepack.zip"),
        "resource pack emitted"
    );
    let zip = &out["resourcepack.zip"];
    let hay = zip.as_slice();
    let has = |needle: &str| hay.windows(needle.len()).any(|w| w == needle.as_bytes());
    assert!(has("assets/delve/font/art.json"), "font json in pack");
    assert!(
        has("assets/delve/textures/font/art.png"),
        "font png in pack"
    );
    let manifest = std::str::from_utf8(&out["manifest.json"]).unwrap();
    assert!(
        manifest.contains("\"resource_pack_sha1\""),
        "manifest records the pack SHA-1"
    );
}

#[test]
fn double_build_is_byte_identical_incl_resource_pack() {
    let a = build_v06(SHOWCASE);
    let b = build_v06(SHOWCASE);
    assert_eq!(
        a.keys().collect::<Vec<_>>(),
        b.keys().collect::<Vec<_>>(),
        "output file set differs"
    );
    for (path, bytes) in &a {
        assert_eq!(bytes, &b[path], "byte mismatch in {path}");
    }
    assert!(
        a.contains_key("resourcepack.zip"),
        "resource pack present (art font baked)"
    );
}

// --- nested-effect consumer recursion ------------------------------------------

/// A `play-sound` with a bogus id **nested in a `sequence` step** fires `DW0326`
/// — the sound-event scan (`sound_refs`) now descends nested effects, so a bad ref
/// buried in a timeline is caught, not shipped unvalidated.
#[test]
fn nested_play_sound_bad_id_is_dw0326() {
    let c = parse_hw(
        &quests_doc(
            r#"{ "type": "sequence", "steps": [ { "at_ticks": 0, "effects": [
                 { "type": "play-sound", "sound": "minecraft:not.a.real.sound" } ] } ] },
               { "type": "campaign-complete" }"#,
        ),
        None,
    );
    let d = atmos::check_sounds(&c);
    assert!(
        d.iter().any(|x| x.code == atmos::DW_SOUND_UNKNOWN),
        "a sequence-nested bogus play-sound must be DW0326: {d:#?}"
    );
}

/// An `art`-styled `narrate` with non-Latin text **nested in a `sequence` step**
/// fires `DW0328` — the art-glyph scan (`art_narrates`) descends nested effects.
#[test]
fn nested_art_narrate_non_latin_is_dw0328() {
    let c = parse_hw(
        &quests_doc(
            r#"{ "type": "sequence", "steps": [ { "at_ticks": 0, "effects": [
                 { "type": "narrate", "text": "胜利", "style": "art" } ] } ] },
               { "type": "campaign-complete" }"#,
        ),
        None,
    );
    let d = atmos::check_art(&c, &BTreeMap::new());
    assert!(
        d.iter().any(|x| x.code == atmos::DW_ART_GLYPH_UNCOVERED),
        "a sequence-nested non-Latin art narrate must be DW0328: {d:#?}"
    );
}

/// A valid `play-sound`/art `narrate` nested in a sequence validates clean — the
/// deep scan must not spuriously reject a good nested ref (island uses these).
#[test]
fn nested_valid_sound_and_art_are_clean() {
    let c = parse_hw(
        &quests_doc(
            r#"{ "type": "sequence", "steps": [ { "at_ticks": 0, "effects": [
                 { "type": "play-sound", "sound": "minecraft:block.note_block.pling" },
                 { "type": "narrate", "text": "YOU WIN", "style": "art" } ] } ] },
               { "type": "campaign-complete" }"#,
        ),
        None,
    );
    assert!(
        atmos::check_sounds(&c).is_empty(),
        "valid nested sound is clean"
    );
    assert!(
        atmos::check_art(&c, &BTreeMap::new()).is_empty(),
        "valid nested art is clean"
    );
}

// --- DW0330: on-screen text fit ------------------------------------------

/// A short title fits the width budget and stays quiet.
#[test]
fn short_title_fits_clean() {
    let c = parse_hw(
        &quests_doc(
            r#"{ "type": "narrate", "text": "He Wakes", "style": "title" },
               { "type": "campaign-complete" }"#,
        ),
        None,
    );
    assert!(
        textfit::check_text_fits(&c, &BTreeMap::new()).is_empty(),
        "a short title must fit the screen"
    );
}

/// A `chat` narrate has no width budget at all — chat wraps and scrolls — so even a
/// very long line stays quiet. Guards against the check over-reaching.
#[test]
fn long_chat_line_is_not_dw0330() {
    let c = parse_hw(
        &quests_doc(
            r#"{ "type": "narrate", "text": "The surf gives up its dead: three drowned wade out of the shallows toward the fire, and not one of them says a word.", "style": "chat" },
               { "type": "campaign-complete" }"#,
        ),
        None,
    );
    assert!(
        textfit::check_text_fits(&c, &BTreeMap::new()).is_empty(),
        "chat wraps and scrolls — it has no width budget"
    );
}

/// An over-long title fires `DW0330`, at **warning** severity: over-long text is a
/// legibility defect, not a build failure (the true limit depends on the player's
/// window and GUI scale), so it must not reject the campaign.
#[test]
fn overlong_title_is_dw0330_warning() {
    let c = parse_hw(
        &quests_doc(
            r#"{ "type": "narrate", "text": "He Rises Blind And Very Angry Indeed", "style": "title" },
               { "type": "campaign-complete" }"#,
        ),
        None,
    );
    let d = textfit::check_text_fits(&c, &BTreeMap::new());
    assert!(
        d.iter()
            .any(|x| x.code == textfit::DW_TEXT_OVERRUNS_SCREEN && x.severity == Severity::Warning),
        "an over-long title must be a DW0330 warning: {d:#?}"
    );
}

/// A subtitle gets twice a title's budget (it renders at ×2, not ×4): the same
/// string that overruns as a title fits as a subtitle.
#[test]
fn subtitle_budget_is_twice_the_title_budget() {
    let text = "He Rises Blind And Angry";
    let as_title = parse_hw(
        &quests_doc(&format!(
            r#"{{ "type": "narrate", "text": "{text}", "style": "title" }},
               {{ "type": "campaign-complete" }}"#
        )),
        None,
    );
    let as_subtitle = parse_hw(
        &quests_doc(&format!(
            r#"{{ "type": "narrate", "text": "{text}", "style": "subtitle" }},
               {{ "type": "campaign-complete" }}"#
        )),
        None,
    );
    assert!(
        !textfit::check_text_fits(&as_title, &BTreeMap::new()).is_empty(),
        "the string must overrun the ×4 title budget"
    );
    assert!(
        textfit::check_text_fits(&as_subtitle, &BTreeMap::new()).is_empty(),
        "the same string must fit the ×2 subtitle budget"
    );
}

/// The negative half of the `ART_SCALE` shrink: shrinking the provider must not
/// disarm the check. An art title still renders in the ×4 title slot on a 90 px
/// budget, so a full sentence of a banner overruns it and must still be `DW0330`.
#[test]
fn overlong_art_title_is_dw0330() {
    const LONG: &str = "THE QUIET SAIL HOMEWARD";
    let c = parse_hw(
        &quests_doc(&format!(
            r#"{{ "type": "narrate", "text": "{LONG}", "style": "art" }},
               {{ "type": "campaign-complete" }}"#
        )),
        None,
    );
    let d = textfit::check_text_fits(&c, &BTreeMap::new());
    assert!(
        d.iter().any(|x| x.code == textfit::DW_TEXT_OVERRUNS_SCREEN),
        "an over-long art title must be DW0330: {d:#?}"
    );
    // It is the ×4 title slot that overruns, not the string being unusable on screen:
    // the same line fits the ×2 subtitle budget.
    let sub = parse_hw(
        &quests_doc(&format!(
            r#"{{ "type": "narrate", "text": "{LONG}", "style": "subtitle" }},
               {{ "type": "campaign-complete" }}"#
        )),
        None,
    );
    assert!(
        textfit::check_text_fits(&sub, &BTreeMap::new()).is_empty(),
        "the same string fits the ×2 subtitle budget"
    );
}

/// The regression this PR exists for: the island's ending banners are 6 and 8 glyphs
/// and **must** be quiet. At the old ×4 provider scale they measured 126 and 168 px
/// against a 90 px budget and physically could not fit on screen.
#[test]
fn island_ending_banners_fit_the_art_budget() {
    for text in ["NOBODY", "HOMEWARD"] {
        let c = parse_hw(
            &quests_doc(&format!(
                r#"{{ "type": "narrate", "text": "{text}", "style": "art" }},
                   {{ "type": "campaign-complete" }}"#
            )),
            None,
        );
        assert!(
            textfit::check_text_fits(&c, &BTreeMap::new()).is_empty(),
            "the `{text}` ending banner must fit the art budget"
        );
    }
}

/// The l10n half — the case the owner actually hit. The English source fits, but the
/// `zh-cn` sidecar rendition does not, and the finding must name the locale and the
/// l10n key so the string to shorten is unambiguous.
#[test]
fn overlong_zh_subtitle_translation_is_dw0330() {
    let mut world: serde_json::Value = serde_json::from_str(&read_hw("world.json")).unwrap();
    world["content"]["languages"] = serde_json::json!(["zh-cn"]);
    let c = parse_hw(
        &quests_doc(
            r#"{ "type": "narrate", "text": "Stay down.", "style": "subtitle" },
               { "type": "campaign-complete" }"#,
        ),
        Some(world.to_string()),
    );
    assert!(
        textfit::check_text_fits(&c, &BTreeMap::new()).is_empty(),
        "the English source must fit"
    );
    let key = on_screen_narrates(&c)[0].key.clone();
    let sidecar_json = serde_json::json!({
        "dsl_version": "0.6.0",
        "campaign_id": "hello-world",
        "kind": "l10n",
        "lang": "zh-cn",
        "content": {
            key.clone(): "待在阴影里。伏低。别弄出一点他能循着找来的声音,他会听见的。"
        }
    });
    let doc: L10nDoc = serde_json::from_value(sidecar_json).unwrap();
    let mut sidecars = BTreeMap::new();
    sidecars.insert("zh-cn".to_string(), doc);

    let d = textfit::check_text_fits(&c, &sidecars);
    assert!(
        d.iter().any(|x| x.code == "DW0330"
            && x.stage == "l10n"
            && x.path.contains("zh-cn")
            && x.path.contains(&key)),
        "an over-long zh subtitle must be DW0330 against the l10n sidecar, named by key: {d:#?}"
    );
}
