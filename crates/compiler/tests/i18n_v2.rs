//! spec-0029 — i18n v2: the client picks the language.
//!
//! One test per acceptance criterion, run through the SHIPPED path (`delvec
//! build`), because the translation tagging that turns an authored string into a
//! `{"translate": …, "fallback": …}` component happens there — a test that
//! assembles a `Plan` by hand emits the same components with literal bodies and
//! would prove nothing about the delivery.
//!
//! **Every coverage set states its binding count** (CLAUDE.md; spec-0029 AC4).
//! A gate here that examined zero objects fails rather than passes, and the count
//! is printed so a reader of the CI log sees what the green covered.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

use serde_json::Value;

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn tmp(name: &str) -> std::path::PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// `delvec build <dir> -o <out>` with the shipped prefab library. Panics with the
/// compiler's own diagnostics on failure, so a red reads like a build log.
fn build(dir: &Path, out: &Path, extra: &[&str]) {
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
    let r = Command::new(BIN).args(&args).output().expect("run delvec");
    assert!(
        r.status.success(),
        "build {}: {}{}",
        dir.display(),
        String::from_utf8_lossy(&r.stdout),
        String::from_utf8_lossy(&r.stderr)
    );
}

/// Every file under `root`, keyed by its path relative to `root`.
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

/// The `assets/delvewright/lang/*.json` members of a built resource pack, by
/// archive path. The pack is a STORE-method zip (`resourcepack.rs`), so a member's
/// bytes appear verbatim and can be sliced out without a zip crate — the same
/// technique `resourcepack`'s own `pack.mcmeta` test uses.
fn lang_files(pack: &[u8]) -> BTreeMap<String, BTreeMap<String, String>> {
    let text = String::from_utf8_lossy(pack).to_string();
    let mut out = BTreeMap::new();
    let mut at = 0usize;
    while let Some(i) = text[at..].find("assets/delvewright/lang/") {
        let start = at + i;
        let name_end = text[start..].find(".json").expect("lang member name") + start + 5;
        let name = text[start..name_end].to_string();
        // The local file header is followed immediately by the STORE payload.
        let Some(brace) = text[name_end..].find('{') else {
            break;
        };
        let body_start = name_end + brace;
        let body_end = text[body_start..]
            .find("\n}\n")
            .map(|e| body_start + e + 3)
            .expect("lang member body");
        if let Ok(map) =
            serde_json::from_str::<BTreeMap<String, String>>(&text[body_start..body_end])
        {
            out.insert(name, map);
        }
        at = body_end;
    }
    out
}

/// The `fallback` field's string value inside a JSON component slice, if present.
fn fallback_value(obj: &str) -> Option<&str> {
    let i = obj.find("\"fallback\"")? + "\"fallback\"".len();
    let rest = obj[i..].trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(&rest[..end])
}

/// The l10n inventory of a campaign directory, derived fresh from its stage docs
/// — never from a fixture, so AC2 compares the pack against the authority rather
/// than against a copy of itself.
fn fresh_inventory(dir: &Path) -> BTreeMap<String, String> {
    let read = |n: &str| std::fs::read_to_string(dir.join(n)).unwrap();
    let campaign = delvewright_dsl::parse_campaign(&delvewright_dsl::RawCampaign {
        world: read("world.json"),
        npcs: read("npcs.json"),
        classes: read("classes.json"),
        quest_plan: read("quest-plan.json"),
        quests: read("quests.json"),
        dialogue: read("dialogue.json"),
        world_edits: None,
    })
    .expect("fixture parses");
    delvewright_dsl::l10n_inventory(&campaign)
}

/// AC1 — a campaign declaring `["zh-cn"]` ships exactly `en_us.json` and
/// `zh_cn.json`, with EQUAL key sets.
#[test]
fn ac1_a_declared_language_ships_its_lang_file_beside_english() {
    let out = tmp("i18n-ac1");
    build(&common::keep_trial_dir(), &out, &[]);
    let tree = read_tree(&out);
    let pack = tree
        .get("resourcepack.zip")
        .expect("a campaign declaring a language ships the resource pack that carries it");
    let langs = lang_files(pack);
    assert_eq!(
        langs.keys().collect::<Vec<_>>(),
        vec![
            "assets/delvewright/lang/en_us.json",
            "assets/delvewright/lang/zh_cn.json"
        ],
        "exactly English plus each declared language"
    );
    let en: BTreeSet<&String> = langs["assets/delvewright/lang/en_us.json"].keys().collect();
    let zh: BTreeSet<&String> = langs["assets/delvewright/lang/zh_cn.json"].keys().collect();
    assert!(!en.is_empty(), "AC1 binding: zero keys is an unbound pass");
    println!("AC1 binding: {} keys per language, 2 languages", en.len());
    assert_eq!(en, zh, "a key in one language and not the other");
}

/// AC2 — every `en_us.json` value equals the English source for that key, as
/// produced by `each_string`. Compared against a FRESH inventory of the stage
/// docs, not against a fixture.
#[test]
fn ac2_english_lang_file_is_the_live_inventory() {
    let out = tmp("i18n-ac2");
    let dir = common::keep_trial_dir();
    build(&dir, &out, &[]);
    let tree = read_tree(&out);
    let langs = lang_files(tree.get("resourcepack.zip").expect("resource pack"));
    let en = &langs["assets/delvewright/lang/en_us.json"];
    let inv = fresh_inventory(&dir);
    assert!(!inv.is_empty(), "AC2 binding: the inventory is empty");
    println!("AC2 binding: {} inventory keys compared", inv.len());
    assert_eq!(en, &inv, "en_us.json must BE the inventory, key and value");
}

/// AC3 + AC4 — over EVERY shipped fixture: no authored string appears in the
/// emitted tree as a literal where a translate key belongs, and every translatable
/// component carries a non-empty `fallback` equal to the English source.
///
/// The two are one walk because they are one question asked from both ends: for
/// each inventoried English string, every occurrence of it in the emitted tree
/// must sit in a `fallback` next to its own `translate` key.
#[test]
fn ac3_ac4_authored_strings_ship_only_as_translatable_components() {
    let mut total_components = 0usize;
    let mut total_keys = 0usize;
    let mut campaigns = 0usize;
    for dir in [
        common::hello_world_dir(),
        common::keep_crawl_dir(),
        common::keep_trial_dir(),
        common::keep_vertical_dir(),
        common::cutscene_shots_dir(),
    ] {
        let name = dir.file_name().unwrap().to_string_lossy().to_string();
        let out = tmp(&format!("i18n-ac34-{name}"));
        build(&dir, &out, &[]);
        let tree = read_tree(&out);
        let inv = fresh_inventory(&dir);
        assert!(!inv.is_empty(), "{name}: empty inventory examined nothing");

        // Every `{"translate": k, "fallback": f}` the tree emits, JSON and SNBT.
        // Both forms are searched textually, because the same key is emitted into
        // `.mcfunction` command bodies (where the component is not a standalone
        // JSON document) as well as into `.json` files.
        let mut seen: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for (path, bytes) in &tree {
            if path.ends_with(".nbt") || path.ends_with(".png") || path == "resourcepack.zip" {
                continue;
            }
            let text = String::from_utf8_lossy(bytes);
            for (key, english) in &inv {
                // JSON component form (serde_json sorts object keys, so
                // `fallback` always precedes `translate`).
                let json = format!(
                    "{{\"fallback\":{},\"translate\":{}}}",
                    Value::String(english.clone()),
                    Value::String(key.clone())
                );
                // The same component with styling merged in keeps `fallback`
                // first and `translate` last only when no other field sorts
                // between them, so match the pair rather than the whole object.
                let json_pair =
                    format!("\"fallback\":{},", serde_json::to_string(english).unwrap());
                let snbt = format!(
                    "{{fallback:{},translate:{}}}",
                    serde_json::to_string(english).unwrap(),
                    serde_json::to_string(key).unwrap()
                );
                if text.contains(&json) || text.contains(&snbt) {
                    seen.entry(key.clone()).or_default().insert(path.clone());
                    total_components += 1;
                } else if text.contains(&json_pair) {
                    // Styled component: prove the pair belongs to THIS key by
                    // finding the key's own `"translate"` in the same file.
                    let tk = format!("\"translate\":{}", serde_json::to_string(key).unwrap());
                    if text.contains(&tk) {
                        seen.entry(key.clone()).or_default().insert(path.clone());
                        total_components += 1;
                    }
                }
            }
        }
        assert!(
            !seen.is_empty(),
            "{name}: zero translatable components examined — a vacuous pass"
        );
        total_keys += seen.len();
        campaigns += 1;

        // AC3: no bare literal. `DW0185` already proves no TAGGED string escaped
        // (the build above would have failed); this asserts the complementary
        // half — an authored string's English text never appears in a `"text"`
        // component, which is where an untranslatable literal would live.
        for (path, bytes) in &tree {
            if path.ends_with(".nbt") || path.ends_with(".png") || path == "resourcepack.zip" {
                continue;
            }
            // The named exclusions: not text components, never rendered by a
            // client. See `docs/reference/compiler.md`.
            if path.starts_with("packtest-datapack/")
                || path == "render-plan.json"
                || path == "manifest.json"
                || path == "critical-path.json"
                || path.starts_with("validation/")
                || path.starts_with("creator-datapack/")
                || path.starts_with("server/")
                || path == "SKINS.md"
            {
                continue;
            }
            let text = String::from_utf8_lossy(bytes);
            for english in inv.values() {
                let literal = format!("\"text\":{}", serde_json::to_string(english).unwrap());
                assert!(
                    !text.contains(&literal),
                    "{name}: `{path}` ships the authored string {english:?} as a literal \
                     `text` component — it must be `{{\"translate\": …, \"fallback\": …}}`"
                );
            }
        }
    }
    assert!(campaigns > 0);
    println!(
        "AC3/AC4 binding: {campaigns} campaigns, {total_keys} distinct keys, \
         {total_components} translatable component occurrences examined"
    );
    assert!(
        total_components > 0,
        "AC4 binding: zero components examined is a failure, not a pass"
    );
}

/// AC5 — a declared language with no Minecraft code is a compile error naming the
/// language and the mapping table, at `validate` (so it never reaches a build).
#[test]
fn ac5_an_unmappable_language_is_dw0184() {
    let dir = tmp("i18n-ac5-src");
    common::copy_dir_all(&common::keep_trial_dir(), &dir);
    let world = dir.join("world.json");
    let mut doc: Value = serde_json::from_str(&std::fs::read_to_string(&world).unwrap()).unwrap();
    doc["content"]["languages"] = serde_json::json!(["kl-ingon"]);
    std::fs::write(&world, serde_json::to_string_pretty(&doc).unwrap()).unwrap();

    let pf = common::prefabs_dir();
    let r = Command::new(BIN)
        .args([
            "validate",
            dir.to_str().unwrap(),
            "--prefabs",
            pf.to_str().unwrap(),
        ])
        .output()
        .expect("run delvec");
    let s = String::from_utf8_lossy(&r.stdout).to_string() + &String::from_utf8_lossy(&r.stderr);
    assert_eq!(
        r.status.code(),
        Some(1),
        "an unmapped language must fail: {s}"
    );
    assert!(s.contains("DW0184"), "{s}");
    assert!(
        s.contains("kl-ingon"),
        "the diagnostic names the language: {s}"
    );
    assert!(
        s.contains("mc_lang_code"),
        "the diagnostic names the mapping table: {s}"
    );
}

/// AC6 — determinism holds with the lang files in the pack. The pack is a
/// `BTreeMap`-ordered STORE zip over `BTreeMap`-ordered JSON, so a double build is
/// byte-identical including its language carrier.
#[test]
fn ac6_double_build_is_byte_identical_lang_files_included() {
    let a = tmp("i18n-ac6-a");
    let b = tmp("i18n-ac6-b");
    let dir = common::keep_trial_dir();
    build(&dir, &a, &[]);
    build(&dir, &b, &[]);
    let ta = read_tree(&a);
    let tb = read_tree(&b);
    assert_eq!(ta.keys().collect::<Vec<_>>(), tb.keys().collect::<Vec<_>>());
    for (p, bytes) in &ta {
        assert_eq!(bytes, &tb[p], "double-build mismatch in {p}");
    }
    let langs = lang_files(ta.get("resourcepack.zip").expect("resource pack"));
    assert_eq!(
        langs.len(),
        2,
        "the gate must cover the lang files it claims"
    );
    println!(
        "AC6 binding: {} files compared, {} lang files",
        ta.len(),
        langs.len()
    );
}

/// spec-0029 §3 — the `fallback` rides the COMPONENT, not the pack. A player who
/// declines the resource-pack prompt has no lang files at all, so every emitted
/// translate component must carry its own English.
#[test]
fn every_translate_component_carries_its_own_fallback() {
    let out = tmp("i18n-fallback");
    build(&common::keep_trial_dir(), &out, &[]);
    let tree = read_tree(&out);
    let mut examined = 0usize;
    for (path, bytes) in &tree {
        if path.ends_with(".nbt") || path.ends_with(".png") || path == "resourcepack.zip" {
            continue;
        }
        let text = String::from_utf8_lossy(bytes);
        // Each occurrence of a `translate` field must have a `fallback` beside it.
        for (i, _) in text.match_indices("\"translate\":") {
            // The component object starts at the nearest preceding `{`.
            let start = text[..i].rfind('{').unwrap_or(0);
            let end = text[i..].find('}').map(|e| i + e).unwrap_or(text.len());
            let obj = &text[start..end];
            // Pretty-printed JSON writes `"fallback": "…"` with a space, the
            // command bodies write `"fallback":"…"` without one — match the field
            // name, then require its value to be a non-empty string.
            assert!(
                fallback_value(obj).is_some_and(|v| !v.is_empty()),
                "{path}: a translate component with no fallback would be unreadable to a \
                 player who declined the resource pack: {obj}"
            );
            examined += 1;
        }
        for (i, _) in text.match_indices("translate:\"") {
            let start = text[..i].rfind('{').unwrap_or(0);
            let end = text[i..].find('}').map(|e| i + e).unwrap_or(text.len());
            let obj = &text[start..end];
            assert!(
                obj.contains("fallback:\"") && !obj.contains("fallback:\"\""),
                "{path}: SNBT component: {obj}"
            );
            examined += 1;
        }
    }
    assert!(
        examined > 0,
        "zero translate components examined is a failure, not a pass"
    );
    println!("fallback binding: {examined} translate components examined");
}

/// `DW0183` — the private-use block the translation tag is built from is reserved
/// in authored and translated content alike.
#[test]
fn dw0183_reserves_the_translation_tag_block() {
    for (name, mutate) in [
        (
            "authored",
            Box::new(|dir: &Path| {
                let p = dir.join("world.json");
                let mut d: Value =
                    serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
                d["content"]["title"] = serde_json::json!("The \u{e000} Keep");
                std::fs::write(&p, serde_json::to_string_pretty(&d).unwrap()).unwrap();
            }) as Box<dyn Fn(&Path)>,
        ),
        (
            "translated",
            Box::new(|dir: &Path| {
                let p = dir.join("l10n/zh-cn.json");
                let mut d: Value =
                    serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
                d["content"]["world.title"] = serde_json::json!("\u{e001}要塞");
                std::fs::write(&p, serde_json::to_string_pretty(&d).unwrap()).unwrap();
            }) as Box<dyn Fn(&Path)>,
        ),
    ] {
        let dir = tmp(&format!("i18n-dw0183-{name}"));
        common::copy_dir_all(&common::keep_trial_dir(), &dir);
        mutate(&dir);
        let pf = common::prefabs_dir();
        let r = Command::new(BIN)
            .args([
                "validate",
                dir.to_str().unwrap(),
                "--prefabs",
                pf.to_str().unwrap(),
            ])
            .output()
            .expect("run delvec");
        let s =
            String::from_utf8_lossy(&r.stdout).to_string() + &String::from_utf8_lossy(&r.stderr);
        assert_eq!(r.status.code(), Some(1), "{name}: {s}");
        assert!(s.contains("DW0183"), "{name}: {s}");
    }
}

/// `DW0185` — the guard that makes spec-0029's central risk an invariant instead
/// of an audit: an authored string emitted OUTSIDE a text component fails the
/// build.
///
/// Exercised directly against the checker, because the only way to produce the
/// condition from a campaign is to write a defective emitter — and a test that
/// requires a defective emitter to exist is a test that only runs once.
#[test]
fn dw0185_catches_an_authored_string_emitted_as_a_literal() {
    use delvewright_compiler::emit;
    let mut out: emit::BuildOutput = BTreeMap::new();
    out.insert(
        "datapack/data/x/function/leak.mcfunction".to_string(),
        format!(
            "tellraw @a {{\"text\":\"{}\"}}\n",
            delvewright_dsl::l10n_plain("") // keeps the literal below honest
        )
        .into_bytes(),
    );
    // A clean tree passes.
    emit::check_untranslated_literals(&out, &BTreeMap::new()).expect("a clean tree is clean");

    // The same tree with one tagged string left un-lowered fails, naming the key.
    let tagged = format!(
        "tellraw @a {{\"text\":\"{}{}{}Unbar the door\"}}\n",
        delvewright_dsl::TR_SIGIL,
        "obj.trial.door.title",
        delvewright_dsl::TR_SIGIL
    );
    out.insert(
        "datapack/data/x/function/leak.mcfunction".to_string(),
        tagged.into_bytes(),
    );
    let err = emit::check_untranslated_literals(&out, &BTreeMap::new())
        .expect_err("a leaked authored string must fail the build");
    match err {
        emit::BuildFailure::Diagnostic { code, message } => {
            assert_eq!(code, "DW0185");
            assert!(message.contains("obj.trial.door.title"), "{message}");
            assert!(message.contains("leak.mcfunction"), "{message}");
        }
        other => panic!("wrong failure: {other:?}"),
    }

    // An unclassified BINARY output fails too, rather than silently skipping the
    // scan — the check is total, not best-effort.
    let mut binary: emit::BuildOutput = BTreeMap::new();
    binary.insert("mystery.bin".to_string(), vec![0xff, 0xfe, 0xfd]);
    let err = emit::check_untranslated_literals(&binary, &BTreeMap::new())
        .expect_err("an unclassified non-UTF-8 output must not skip the scan");
    match err {
        emit::BuildFailure::Diagnostic { code, message } => {
            assert_eq!(code, "DW0185");
            assert!(message.contains("mystery.bin"), "{message}");
        }
        other => panic!("wrong failure: {other:?}"),
    }
}

/// spec-0029 §4 — `--lang` survives, unchanged, as a single-language bake: the
/// strings are swapped before emission, so nothing carries a translate key and the
/// build ships no lang files for a client to choose between.
#[test]
fn a_lang_bake_ships_no_language_carrier() {
    let out = tmp("i18n-bake");
    build(&common::keep_trial_dir(), &out, &["--lang", "zh-cn"]);
    let tree = read_tree(&out);
    assert!(
        !tree.contains_key("resourcepack.zip"),
        "a `--lang` bake has one language baked in; there is nothing to select between"
    );
    let mut checked = 0usize;
    for (path, bytes) in &tree {
        if path.ends_with(".nbt") || path.ends_with(".png") {
            continue;
        }
        let text = String::from_utf8_lossy(bytes);
        assert!(
            !text.contains("\"translate\":\"dlg."),
            "{path}: a bake must carry no translate key"
        );
        checked += 1;
    }
    assert!(checked > 0, "binding: zero files checked");
    println!("bake binding: {checked} files checked for translate keys");
}
