//! spec-0096 — a span of text carries a style.
//!
//! The markup is accepted in every player-facing string, so the tests are about
//! the object class, not one verb: every string the inventory walks is styled at
//! once and the built tree is read for each; the language files, the refusals and
//! the `--lang` bake are driven through the shipped `delvec` binary.
//!
//! **Every coverage set states its binding count** (CLAUDE.md): a set that bound
//! zero objects fails rather than passes.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit;
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::{TextKind, each_string, key_kind, parse_campaign};
use serde_json::Value;

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A fresh copy of keep-trial (it declares zh-cn), so a test may patch it.
fn keep_trial_copy(name: &str) -> PathBuf {
    let dir = tmp(name);
    common::copy_dir_all(&common::keep_trial_dir(), &dir);
    dir
}

fn patch_json(path: &Path, f: impl FnOnce(&mut Value)) {
    let mut v: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    f(&mut v);
    std::fs::write(path, serde_json::to_string_pretty(&v).unwrap()).unwrap();
}

/// Set one English line and its zh-cn row (and the row's provenance) together.
fn set_line(dir: &Path, doc: &str, pointer: &str, key: &str, en: &str, zh: &str) {
    patch_json(&dir.join(doc), |v| {
        *v.pointer_mut(pointer)
            .unwrap_or_else(|| panic!("{doc}{pointer}")) = Value::from(en);
    });
    patch_json(&dir.join("l10n/zh-cn.json"), |v| {
        v["content"][key] = Value::from(zh);
        v["source"][key] = Value::from(en);
    });
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(BIN).args(args).output().expect("run delvec")
}

fn build(dir: &Path, out: &Path, extra: &[&str]) -> std::process::Output {
    let pf = common::prefabs_dir();
    let mut args = vec![
        "build",
        dir.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
    ];
    args.extend_from_slice(extra);
    run(&args)
}

fn ok(r: &std::process::Output) {
    assert!(
        r.status.success(),
        "{}{}",
        String::from_utf8_lossy(&r.stdout),
        String::from_utf8_lossy(&r.stderr)
    );
}

/// The DW codes `delvec validate --json` reports, with each diagnostic's path.
fn validate_codes(dir: &Path) -> Vec<(String, String)> {
    let pf = common::prefabs_dir();
    let r = run(&[
        "validate",
        dir.to_str().unwrap(),
        "--json",
        "--prefabs",
        pf.to_str().unwrap(),
    ]);
    String::from_utf8_lossy(&r.stdout)
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter_map(|v| {
            Some((
                v.get("code")?.as_str()?.to_string(),
                v.get("path")?.as_str().unwrap_or("").to_string(),
            ))
        })
        .collect()
}

/// Every file under `root`, keyed by its relative path.
fn read_tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut map = BTreeMap::new();
    fn walk(base: &Path, dir: &Path, map: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(base, &path, map);
            } else {
                let rel = path
                    .strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                map.insert(rel, std::fs::read(&path).unwrap());
            }
        }
    }
    walk(root, root, &mut map);
    map
}

/// The lang members of a STORE-method resource pack (see `i18n_v2.rs`).
fn lang_files(pack: &[u8]) -> BTreeMap<String, BTreeMap<String, String>> {
    let text = String::from_utf8_lossy(pack).to_string();
    let mut out = BTreeMap::new();
    let mut at = 0usize;
    while let Some(i) = text[at..].find("assets/delvewright/lang/") {
        let start = at + i;
        let name_end = text[start..].find(".json").expect("member name") + start + 5;
        let name = text[start..name_end].to_string();
        let Some(brace) = text[name_end..].find('{') else {
            break;
        };
        let body_start = name_end + brace;
        let body_end = text[body_start..]
            .find("\n}\n")
            .map(|e| body_start + e + 3)
            .expect("member body");
        if let Ok(map) =
            serde_json::from_str::<BTreeMap<String, String>>(&text[body_start..body_end])
        {
            out.insert(name, map);
        }
        at = body_end;
    }
    out
}

fn functions_text(tree: &BTreeMap<String, Vec<u8>>) -> String {
    tree.iter()
        .filter(|(p, _)| p.ends_with(".mcfunction") || p.ends_with(".json"))
        .map(|(_, b)| String::from_utf8_lossy(b).to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The first value under an object key `name` anywhere in `v`.
fn find_key(v: &Value, name: &str) -> Option<Value> {
    match v {
        Value::Object(o) => o
            .get(name)
            .cloned()
            .or_else(|| o.values().find_map(|x| find_key(x, name))),
        Value::Array(a) => a.iter().find_map(|x| find_key(x, name)),
        _ => None,
    }
}

/// The first component in `v` asking for `key`.
fn find_component(v: &Value, key: &str) -> Option<Value> {
    match v {
        Value::Object(o) => {
            if o.get("translate").and_then(Value::as_str) == Some(key) {
                return Some(v.clone());
            }
            o.values().find_map(|x| find_component(x, key))
        }
        Value::Array(a) => a.iter().find_map(|x| find_component(x, key)),
        _ => None,
    }
}

/// Build `campaign` in memory, English, no languages (so no sidecar is owed).
fn build_in_memory(mut campaign: delvewright_dsl::Campaign) -> BTreeMap<String, Vec<u8>> {
    campaign.world.content.languages.clear();
    delvewright_dsl::tag_translatables(&mut campaign);
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let mut structures = BTreeMap::new();
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
    .expect("builds")
}

/// AC2 + AC3: every player-facing string the fixture emits, styled at once, is
/// emitted with its span — whatever class it is. The styled build must also pass
/// the compiler's own leak scan (`DW0185`) and the command-tree check, which
/// `emit::build` runs. Wrapping a whole line in `italic` keeps every width the
/// same, so no width check is perturbed.
#[test]
fn every_emitted_player_facing_string_carries_its_style() {
    let loaded = load_campaign_dir(&common::keep_trial_dir()).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("keep-trial parses");
    let ns = delvewright_dsl::pack_namespace(campaign.world.campaign_id.as_str());

    let plain_tree = functions_text(&build_in_memory(campaign.clone()));

    let mut styled = campaign.clone();
    each_string(&mut styled, &mut |_, v| *v = format!("[[italic|{v}]]"));
    assert!(
        delvewright_dsl::textstyle::validate_inline_styles(&styled, &BTreeMap::new()).is_empty()
    );
    let styled_tree = functions_text(&build_in_memory(styled));

    let inv = delvewright_dsl::l10n::inventory(&campaign);
    let mut emitted = 0usize;
    let mut kinds: BTreeSet<TextKind> = BTreeSet::new();
    for key in inv.keys() {
        let full = format!("{ns}{key}");
        let asked =
            |t: &str| t.contains(&format!("\"{full}\"")) || t.contains(&format!("\"{full}\\\""));
        if !asked(&plain_tree) {
            continue; // a string this fixture never draws (a quest goal with an outro, say)
        }
        emitted += 1;
        kinds.insert(key_kind(key).expect("every inventory key has a kind"));
        let span = delvewright_dsl::textstyle::span_key(&full, 0);
        assert!(
            styled_tree.contains(&format!("\"{span}\"")),
            "`{key}` is drawn but its styled span `{span}` never reaches the tree"
        );
    }
    assert!(emitted > 0, "binding: no inventory key reached the tree");
    assert!(
        !styled_tree.contains("[["),
        "markup leaked into the built tree"
    );
    println!(
        "inline-style emission binding: {emitted} of {} inventory key(s) drawn, all styled; \
         {} text kind(s): {kinds:?}",
        inv.len(),
        kinds.len()
    );
}

/// AC3 (SNBT): an NPC's name lives in `CustomName`, an NBT field, so a styled
/// name is an SNBT compound whose span sets `obfuscated:true`.
#[test]
fn a_styled_name_in_nbt_is_an_snbt_compound() {
    let loaded = load_campaign_dir(&common::keep_trial_dir()).unwrap();
    let mut campaign = parse_campaign(&loaded.raw).expect("keep-trial parses");
    campaign.npcs.content.npcs[0].name = "The [[obfuscated|Keeper]]".into();
    let text = functions_text(&build_in_memory(campaign));
    let line = text
        .lines()
        .find(|l| l.contains("CustomName:{") && l.contains("with:["))
        .expect("the styled name is summoned as an SNBT component");
    assert!(line.contains("fallback:\"The %1$s\""), "{line}");
    assert!(line.contains("obfuscated:true"), "{line}");
    assert!(line.contains(".span.0\""), "{line}");
    assert!(
        !line.contains("'{"),
        "never the stringified-JSON form: {line}"
    );
}

/// AC3 + AC4: the shipped build of one styled dialogue line, with a zh-cn row
/// that puts the two spans in the other order.
#[test]
fn a_styled_line_and_its_translation_reach_the_component_and_the_language_files() {
    let dir = keep_trial_copy("inline-styles-lang");
    let key = "dlg.keeper.lore.text";
    set_line(
        &dir,
        "dialogue.json",
        "/content/dialogues/0/nodes/1/text",
        key,
        "Half the [[obfuscated|captain]] is 100% [[bold,color=#8B0000|gone]].",
        "[[bold,color=#8B0000|没了]]的，是[[obfuscated|队长]]的一半。",
    );
    // The pointer must have hit the node the key names.
    let d: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("dialogue.json")).unwrap()).unwrap();
    assert_eq!(d["content"]["dialogues"][0]["nodes"][1]["id"], "dlg/lore");

    let out = tmp("inline-styles-lang-out");
    ok(&build(&dir, &out, &[]));
    let tree = read_tree(&out);
    let ns = delvewright_dsl::pack_namespace("keep-trial");
    let full = format!("{ns}{key}");

    // The component, read out of the node's dialog the way the client reads it.
    let dialog = |name: &str| -> Value {
        serde_json::from_slice(
            tree.get(&format!("datapack/data/keep-trial/dialog/{name}.json"))
                .unwrap_or_else(|| panic!("dialog {name}")),
        )
        .unwrap()
    };
    let comp = find_component(&dialog("keeper_lore"), &full).expect("the line is asked for");
    assert_eq!(comp["fallback"], "Half the %1$s is 100%% %2$s.");
    let with = comp["with"].as_array().expect("spans ride in `with`");
    assert_eq!(with.len(), 2);
    assert_eq!(with[0]["obfuscated"], true);
    assert_eq!(with[0]["translate"], format!("{full}.span.0"));
    assert_eq!(with[1]["bold"], true);
    assert_eq!(with[1]["color"], "#8b0000");
    assert_eq!(with[1]["fallback"], "gone");

    // An unstyled line keeps its old shape: no `with`.
    let greet = format!("{ns}dlg.keeper.greet.text");
    let g = find_component(&dialog("keeper_greet__m0"), &greet).expect("greet asked");
    assert!(g.get("with").is_none(), "{g}");

    // The language files.
    let langs = lang_files(tree.get("resourcepack.zip").expect("pack"));
    let en = &langs["assets/delvewright/lang/en_us.json"];
    let zh = &langs["assets/delvewright/lang/zh_cn.json"];
    assert_eq!(en[&full], "Half the %1$s is 100%% %2$s.");
    assert_eq!(en[&format!("{full}.span.0")], "captain");
    assert_eq!(en[&format!("{full}.span.1")], "gone");
    assert_eq!(zh[&full], "%2$s的，是%1$s的一半。");
    assert_eq!(zh[&format!("{full}.span.0")], "队长");
    assert_eq!(zh[&format!("{full}.span.1")], "没了");
    assert_eq!(
        en.keys().collect::<BTreeSet<_>>(),
        zh.keys().collect::<BTreeSet<_>>()
    );

    // AC7: the same inputs build byte-identically.
    let again = tmp("inline-styles-lang-again");
    ok(&build(&dir, &again, &[]));
    let a = read_tree(&out);
    let b = read_tree(&again);
    assert_eq!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>());
    for (p, bytes) in &a {
        assert_eq!(bytes, &b[p], "{p} differs between two builds");
    }

    // A `--lang zh-cn` bake ships no language file: the translated line is drawn
    // from its own spans, in its own order, as literal text.
    let baked = tmp("inline-styles-lang-zh");
    ok(&build(&dir, &baked, &["--lang", "zh-cn"]));
    let baked_tree = read_tree(&baked);
    let zt = functions_text(&baked_tree);
    let lore: Value =
        serde_json::from_slice(&baked_tree["datapack/data/keep-trial/dialog/keeper_lore.json"])
            .unwrap();
    let extra = find_key(&lore, "extra").expect("the baked line is drawn from its spans");
    assert_eq!(
        extra,
        serde_json::json!([
            {"bold": true, "color": "#8b0000", "text": "没了"},
            {"text": "的，是"},
            {"obfuscated": true, "text": "队长"},
            {"text": "的一半。"}
        ])
    );
    assert!(!zt.contains("[["), "markup leaked into the bake");
}

/// AC5: `DW0975` — malformed markup, in the English and in a sidecar row.
#[test]
fn malformed_markup_is_refused_at_validate() {
    let dir = keep_trial_copy("inline-styles-dw0975-en");
    set_line(
        &dir,
        "dialogue.json",
        "/content/dialogues/0/nodes/1/text",
        "dlg.keeper.lore.text",
        "Half the [[obfuscated|captain is gone.",
        "队长的一半没了。",
    );
    let codes = validate_codes(&dir);
    assert!(
        codes
            .iter()
            .any(|(c, p)| c == "DW0975" && p.contains("dlg.keeper.lore.text")),
        "{codes:?}"
    );

    let dir = keep_trial_copy("inline-styles-dw0975-zh");
    patch_json(&dir.join("l10n/zh-cn.json"), |v| {
        v["content"]["dlg.keeper.lore.text"] = Value::from("队长的[[shiny|一半]]没了。");
    });
    let codes = validate_codes(&dir);
    assert!(
        codes
            .iter()
            .any(|(c, p)| c == "DW0975" && p.starts_with("l10n/zh-cn.json")),
        "{codes:?}"
    );
}

/// AC5: `DW0976` — a translation that drops a span, and one that restyles it.
#[test]
fn a_translation_whose_spans_differ_is_refused() {
    for (name, zh) in [
        ("drop", "队长的一半没了。"),
        ("restyle", "[[bold|队长]]的一半没了。"),
    ] {
        let dir = keep_trial_copy(&format!("inline-styles-dw0976-{name}"));
        set_line(
            &dir,
            "dialogue.json",
            "/content/dialogues/0/nodes/1/text",
            "dlg.keeper.lore.text",
            "Half the [[obfuscated|captain]] is gone.",
            zh,
        );
        let codes = validate_codes(&dir);
        assert!(
            codes
                .iter()
                .any(|(c, p)| c == "DW0976" && p.contains("dlg.keeper.lore.text")),
            "{name}: {codes:?}"
        );
        assert!(
            !codes.iter().any(|(c, _)| c == "DW0975"),
            "{name}: the row parses; only its spans differ: {codes:?}"
        );
    }
}
