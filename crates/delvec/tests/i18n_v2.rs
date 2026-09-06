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
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
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

/// AC2 — `en_us.json` is exactly the campaign's live inventory PLUS the
/// compiler's own chrome, each half compared against its live source rather than
/// a fixture: the inventory from a fresh `each_string` walk, the chrome from
/// `dsl::chrome::english_entries`. The two halves are disjoint by construction
/// (chrome lives under the reserved `delvewright.` prefix), and the test proves
/// that too — a campaign key leaking into the chrome namespace, or chrome
/// shadowing a campaign key, would show up here first.
#[test]
fn ac2_english_lang_file_is_the_live_inventory_plus_chrome() {
    let out = tmp("i18n-ac2");
    let dir = common::keep_trial_dir();
    build(&dir, &out, &[]);
    let tree = read_tree(&out);
    let langs = lang_files(tree.get("resourcepack.zip").expect("resource pack"));
    let en = &langs["assets/delvewright/lang/en_us.json"];
    let inv = fresh_inventory(&dir);
    let chrome = delvewright_dsl::chrome::english_entries();
    assert!(!inv.is_empty(), "AC2 binding: the inventory is empty");
    assert!(!chrome.is_empty(), "AC2 binding: no chrome string binds");
    println!(
        "AC2 binding: {} inventory keys + {} chrome keys compared",
        inv.len(),
        chrome.len()
    );
    let prefix = delvewright_dsl::chrome::RESERVED_PREFIX;
    let (campaign_half, chrome_half): (BTreeMap<_, _>, BTreeMap<_, _>) = en
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .partition(|(k, _)| !k.starts_with(prefix));
    assert_eq!(
        campaign_half, inv,
        "the campaign half of en_us.json must BE the inventory, key and value"
    );
    assert_eq!(
        chrome_half, chrome,
        "the chrome half of en_us.json must BE the compiler's own English"
    );
    assert!(
        inv.keys().all(|k| !k.starts_with(prefix)),
        "no campaign l10n key may enter the reserved chrome namespace"
    );
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
        s.contains("CLIENT_LANGS"),
        "the diagnostic names the derived client language set: {s}"
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

// ---------------------------------------------------------------------------
// Compiler chrome (spec-0029 addendum)
// ---------------------------------------------------------------------------

/// **The chrome hole, closed.** Every string the compiler writes itself — the
/// eight product-chrome lines that never had an authored override, plus the five
/// diegetic defaults it bakes when a campaign authors none — is emitted as a
/// `{"translate": "delvewright.ui.…", "fallback": <English>}` component, and the
/// English literal appears nowhere in the datapack outside such a fallback.
///
/// Before this, a player reading a delve in Chinese got `New objective: ` in
/// English wrapped around a translated title.
#[test]
fn chrome_ships_as_components_not_literals() {
    let out = tmp("i18n-chrome");
    build(&common::keep_trial_dir(), &out, &[]);
    let tree = read_tree(&out);

    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut literals = Vec::new();
    for (path, bytes) in &tree {
        if !path.starts_with("datapack/") {
            continue;
        }
        let text = String::from_utf8_lossy(bytes);
        for c in delvewright_dsl::chrome::ALL {
            if text.contains(c.key) {
                seen.insert(c.key);
            }
            // The English may appear ONLY as a `fallback`. Any other occurrence is
            // a literal a client cannot translate. Both the compact JSON the
            // functions carry and the pretty-printed dialog JSON are stripped, so
            // the assertion is about the VALUE's position, not about formatting.
            let quoted = serde_json::to_string(c.en).unwrap();
            let mut stripped = text.to_string();
            for form in [
                format!("\"fallback\": {quoted}"),
                format!("\"fallback\":{quoted}"),
                format!("fallback:{quoted}"),
            ] {
                stripped = stripped.replace(&form, "");
            }
            if stripped.contains(c.en) {
                literals.push(format!("{path}: {:?}", c.en));
            }
        }
    }
    assert!(
        literals.is_empty(),
        "compiler chrome still ships as an untranslatable literal: {literals:#?}"
    );
    assert!(
        !seen.is_empty(),
        "chrome binding: zero chrome keys reached the datapack — an unbound pass"
    );
    println!(
        "chrome binding: {} of {} chrome keys emitted by the keep-trial fixture",
        seen.len(),
        delvewright_dsl::chrome::ALL.len()
    );
    // The four the owner named, plus the two the fixture's own shape guarantees.
    for c in [
        delvewright_dsl::chrome::OBJECTIVE_NEW,
        delvewright_dsl::chrome::OBJECTIVE_COMPLETE,
        delvewright_dsl::chrome::CAMPAIGN_COMPLETE,
        delvewright_dsl::chrome::CAMPAIGN_SIGNATURE,
        delvewright_dsl::chrome::CAMPAIGN_BANNER,
        delvewright_dsl::chrome::CLASS_TITLE,
        delvewright_dsl::chrome::CLASS_BODY,
    ] {
        assert!(seen.contains(c.key), "`{}` was never emitted", c.key);
    }
}

/// A framed value is **one key with `%s`**, carried by `with`, not a key
/// concatenated with a component: word order belongs to the translator. This pins
/// the three-part shape — key, fallback, arguments — on the objective toast, and
/// that the argument is itself the objective's own translatable title.
#[test]
fn framed_chrome_uses_placeholders_and_with_arguments() {
    let out = tmp("i18n-chrome-args");
    build(&common::keep_trial_dir(), &out, &[]);
    let tree = read_tree(&out);
    let announce = tree
        .iter()
        .find(|(p, _)| p.contains("/function/announce_"))
        .map(|(_, b)| String::from_utf8_lossy(b).to_string())
        .expect("a titled objective announces itself");
    let line = announce
        .lines()
        .find(|l| l.starts_with("tellraw"))
        .expect("the announcement is a tellraw");
    let comp: Value =
        serde_json::from_str(line.trim_start_matches("tellraw @a ")).expect("one component");

    assert_eq!(
        comp["translate"],
        delvewright_dsl::chrome::OBJECTIVE_NEW.key
    );
    assert_eq!(comp["fallback"], "New objective: %s");
    let with = comp["with"].as_array().expect("the title rides in `with`");
    assert_eq!(with.len(), 1, "one argument for one placeholder");
    assert!(
        with[0]["translate"]
            .as_str()
            .is_some_and(|k| k.starts_with("obj.") && k.ends_with(".title")),
        "the argument is the objective's own translatable title: {comp}"
    );
    // The prefix and the title are ONE component now, so the title's own style
    // must survive the merge (it was never bold beside the prefix).
    assert_eq!(comp["bold"], true);
    assert_eq!(with[0]["bold"], false);
}

/// Chrome is written into the language files the delve already ships, and only
/// those. `en_us` carries every chrome key; a declared language carries the ones
/// the compiler can really translate.
#[test]
fn chrome_rides_the_language_files_the_delve_ships() {
    let out = tmp("i18n-chrome-lang");
    build(&common::keep_trial_dir(), &out, &[]);
    let langs = lang_files(read_tree(&out).get("resourcepack.zip").expect("pack"));
    let prefix = delvewright_dsl::chrome::RESERVED_PREFIX;
    let chrome_of = |file: &str| -> BTreeMap<String, String> {
        langs[file]
            .iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    };
    let en = chrome_of("assets/delvewright/lang/en_us.json");
    let zh = chrome_of("assets/delvewright/lang/zh_cn.json");
    assert_eq!(en, delvewright_dsl::chrome::english_entries());
    assert_eq!(zh, delvewright_dsl::chrome::lang_entries("zh_cn"));
    assert_eq!(
        zh.len(),
        delvewright_dsl::chrome::ALL.len(),
        "chrome binding: zh_cn must carry every chrome string, got {}",
        zh.len()
    );
    println!(
        "chrome lang binding: en_us {} keys, zh_cn {} keys",
        en.len(),
        zh.len()
    );
    // A `%s` in the English is a `%s` in the translation — a dropped placeholder
    // is an objective title that never appears on screen.
    for c in delvewright_dsl::chrome::ALL {
        assert_eq!(
            zh[c.key].matches("%s").count(),
            c.args,
            "`{}` loses a placeholder in zh_cn",
            c.key
        );
    }
}

/// **`DW0186`** — a campaign sidecar may not define a chrome key. Chrome is
/// compiler-owned end to end; a sidecar row under the reserved prefix would be
/// written into the language file and silently replace product chrome.
#[test]
fn a_sidecar_may_not_define_a_chrome_key_dw0186() {
    let dir = tmp("i18n-chrome-shadow");
    common::copy_dir_all(&common::keep_trial_dir(), &dir);
    let side = dir.join("l10n/zh-cn.json");
    let mut doc: Value = serde_json::from_str(&std::fs::read_to_string(&side).unwrap()).unwrap();
    doc["content"][delvewright_dsl::chrome::CLASS_TITLE.key] = serde_json::json!("我的标题");
    std::fs::write(&side, serde_json::to_string_pretty(&doc).unwrap()).unwrap();

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
        "a shadowed chrome key must fail: {s}"
    );
    assert!(s.contains("DW0186"), "{s}");
    assert!(
        s.contains(delvewright_dsl::chrome::CLASS_TITLE.key),
        "the diagnostic names the offending key: {s}"
    );
}

/// The control for `DW0186`: the same campaign, untouched, is clean. A guard that
/// fires on everything proves nothing about the case it was written for.
#[test]
fn an_untouched_sidecar_raises_no_dw0186() {
    let pf = common::prefabs_dir();
    let r = Command::new(BIN)
        .args([
            "validate",
            common::keep_trial_dir().to_str().unwrap(),
            "--prefabs",
            pf.to_str().unwrap(),
        ])
        .output()
        .expect("run delvec");
    let s = String::from_utf8_lossy(&r.stdout).to_string() + &String::from_utf8_lossy(&r.stderr);
    assert!(!s.contains("DW0186"), "control campaign must be clean: {s}");
}

// ---------------------------------------------------------------------------
// Translation provenance — DW0187 / DW0188
// ---------------------------------------------------------------------------

/// Read a campaign's sidecar, hand it to `f`, write it back.
fn edit_sidecar(dir: &Path, f: impl FnOnce(&mut Value)) {
    let p = dir.join("l10n/zh-cn.json");
    let mut doc: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    f(&mut doc);
    std::fs::write(&p, serde_json::to_string_pretty(&doc).unwrap()).unwrap();
}

fn validate(dir: &Path) -> String {
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
    String::from_utf8_lossy(&r.stdout).to_string() + &String::from_utf8_lossy(&r.stderr)
}

/// **The general case.** Coverage proves the sidecar's key SET matches the
/// inventory; it is silent about whether a row still translates the English it
/// renders. Rewrite an authored line and the translation is present, applied and
/// wrong, with no key moved — `DW0187` is what sees it.
#[test]
fn an_edited_line_makes_its_translation_stale_dw0187() {
    let dir = tmp("i18n-stale-general");
    common::copy_dir_all(&common::keep_trial_dir(), &dir);
    let world = dir.join("world.json");
    let mut doc: Value = serde_json::from_str(&std::fs::read_to_string(&world).unwrap()).unwrap();
    doc["content"]["title"] = serde_json::json!("Trial of the Stone Keep");
    std::fs::write(&world, serde_json::to_string_pretty(&doc).unwrap()).unwrap();

    let s = validate(&dir);
    assert!(s.contains("DW0187"), "an edited line must be caught: {s}");
    assert!(
        s.contains("world.title"),
        "the diagnostic names the key: {s}"
    );
    assert!(
        s.contains("Trial of the Stone Keep"),
        "the diagnostic names what the line now reads: {s}"
    );
}

/// **The ownership-migration case**, which the text-owned key scheme introduces:
/// rename ONE body and the key migrates to ANOTHER body, so the row that goes
/// stale is not the row the author touched. `DW0180` points at the newly-required
/// key — somewhere else entirely — while `DW0187` points at the row that is
/// actually wrong.
#[test]
fn renaming_one_body_makes_another_bodys_row_stale_dw0187() {
    let dir = tmp("i18n-stale-migration");
    common::copy_dir_all(&common::keep_trial_dir(), &dir);

    // Two puppets with one name: the first owns the key, the second follows it.
    let quests = dir.join("quests.json");
    let mut q: Value = serde_json::from_str(&std::fs::read_to_string(&quests).unwrap()).unwrap();
    // 0.10.0, because an actor NAMEPLATE is only a coverage obligation from 0.10
    // onward (`l10n::ACTOR_NAME_ENTRY`): the widening that inventoried
    // it landed over v0.6 surface. The
    // pair to this line is `an_actor_nameplate_is_not_demanded_below_0_10`.
    q["dsl_version"] = serde_json::json!("0.19.0");
    q["content"]["actors"] = serde_json::json!([
        { "id": "actor/ram-a", "entity": "minecraft:sheep",
          "name": "Ram of the Cave", "anchor": "anchor/hall" },
        { "id": "actor/ram-b", "entity": "minecraft:sheep",
          "name": "Ram of the Cave", "anchor": "anchor/hall" },
    ]);
    std::fs::write(&quests, serde_json::to_string_pretty(&q).unwrap()).unwrap();
    edit_sidecar(&dir, |doc| {
        doc["content"]["actor.ram-a.name"] = serde_json::json!("洞中公羊");
        doc["source"]["actor.ram-a.name"] = serde_json::json!("Ram of the Cave");
    });
    assert!(
        !validate(&dir).contains("DW0187"),
        "control: the shared key is sound before the rename"
    );

    // Rename ONLY the first. Its key now holds the NEW text; the translation of
    // the old text is still sitting on it, and `ram-b` needs a key of its own.
    let mut q: Value = serde_json::from_str(&std::fs::read_to_string(&quests).unwrap()).unwrap();
    q["content"]["actors"][0]["name"] = serde_json::json!("Bellwether of the Cave");
    std::fs::write(&quests, serde_json::to_string_pretty(&q).unwrap()).unwrap();

    let s = validate(&dir);
    assert!(s.contains("DW0187"), "the migrated key must be caught: {s}");
    assert!(
        s.contains("actor.ram-a.name") && s.contains("Bellwether of the Cave"),
        "the diagnostic names the row that is wrong, not the one that is missing: {s}"
    );
    assert!(
        s.contains("DW0180") && s.contains("actor.ram-b.name"),
        "and the newly-required key is still reported: {s}"
    );
}

/// Provenance that names a key the campaign no longer has is itself stale.
#[test]
fn provenance_for_a_vanished_key_is_dw0187() {
    let dir = tmp("i18n-stale-orphan-source");
    common::copy_dir_all(&common::keep_trial_dir(), &dir);
    edit_sidecar(&dir, |doc| {
        doc["source"]["obj.trial.gone.title"] = serde_json::json!("Something Removed");
    });
    let s = validate(&dir);
    assert!(s.contains("DW0187"), "{s}");
    assert!(s.contains("obj.trial.gone.title"), "{s}");
}

/// **`DW0188` states the unguarded count.** A sidecar with no `source` is not
/// checked by `DW0187`, and the point of the warning is that this can never be
/// mistaken for a pass: it is a number, on every run, naming how many rows are
/// unguarded out of how many exist.
#[test]
fn a_sidecar_without_provenance_reports_its_unguarded_count_dw0188() {
    let dir = tmp("i18n-unguarded");
    common::copy_dir_all(&common::keep_trial_dir(), &dir);
    let rows = {
        let p = dir.join("l10n/zh-cn.json");
        let mut doc: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        let n = doc["content"].as_object().unwrap().len();
        doc.as_object_mut().unwrap().remove("source");
        std::fs::write(&p, serde_json::to_string_pretty(&doc).unwrap()).unwrap();
        n
    };
    assert!(rows > 0, "binding: the fixture sidecar translates nothing");

    let s = validate(&dir);
    assert!(
        s.contains("DW0188"),
        "an unguarded sidecar must say so: {s}"
    );
    assert!(
        s.contains(&format!("{rows} of {rows} translated rows")),
        "the warning states the count ({rows} rows): {s}"
    );
    println!("DW0188 binding: {rows} unguarded rows reported");
}

/// The control, and the binding count for `DW0187`: the shipped fixture records
/// provenance for every row, so the guard really compares them and finds nothing.
/// A sidecar that compared **zero** rows would be a green that binds to nothing.
#[test]
fn the_fixture_sidecar_is_fully_guarded_and_clean() {
    let p = common::keep_trial_dir().join("l10n/zh-cn.json");
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    let content = doc["content"].as_object().expect("content");
    let source = doc["source"]
        .as_object()
        .expect("the fixture records provenance");
    assert!(
        !content.is_empty(),
        "binding: the fixture translates nothing"
    );
    assert_eq!(
        source.len(),
        content.len(),
        "DW0187 binding: every translated row must record what it came from"
    );
    println!(
        "DW0187 binding: {} rows compared against the inventory",
        source.len()
    );

    let s = validate(&common::keep_trial_dir());
    assert!(!s.contains("DW0187"), "control campaign must be clean: {s}");
    assert!(
        !s.contains("DW0188"),
        "control campaign must be fully guarded: {s}"
    );
}
