//! Shared test helpers: locate fixtures and materialize a campaign directory
//! from the DSL's patch-style invalid fixtures (base hello-world + overrides).
//!
//! Compiled once per integration-test binary; not every helper is used by each,
//! so unused-warnings here are expected.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// The six stage filenames (matching `delvec::compiler::load::STAGE_FILES`).
pub const STAGE_FILES: [&str; 6] = [
    "world.json",
    "npcs.json",
    "classes.json",
    "quest-plan.json",
    "quests.json",
    "dialogue.json",
];

/// Repo root (two levels up from `crates/delvec/src/compiler`).
pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .unwrap()
        .to_path_buf()
}

/// The canonical valid hello-world campaign directory.
pub fn hello_world_dir() -> PathBuf {
    repo_root().join("crates/dsl/fixtures/valid/hello-world")
}

/// The multi-area / multi-piece keep-crawl campaign directory.
pub fn keep_crawl_dir() -> PathBuf {
    repo_root().join("crates/dsl/fixtures/valid/keep-crawl")
}

/// The v0.3 branching keep-trial campaign directory (all gameplay verbs).
pub fn keep_trial_dir() -> PathBuf {
    repo_root().join("crates/dsl/fixtures/valid/keep-trial")
}

/// The v0.6 cutscene fixture (`cutscene-shots`): hello-world's world and cast
/// with a two-shot `cutscene` on its exit beat. The campaign the spec-0019
/// live flow (`validation/rehearsal-flow.sh`) plays, kept here so tier 1
/// fails first if the fixture ever stops producing the proposal that flow
/// asserts against.
pub fn cutscene_shots_dir() -> PathBuf {
    repo_root().join("crates/dsl/fixtures/valid/cutscene-shots")
}

/// The v0.3 vertical keep-vertical campaign directory (3D stair layout).
pub fn keep_vertical_dir() -> PathBuf {
    repo_root().join("crates/dsl/fixtures/valid/keep-vertical")
}

/// Materialize a full campaign directory at `dst` = the campaign in `base` with
/// each stage in `patch["documents"]` overwritten by its replacement envelope. Any
/// `l10n/` sidecar directory in `base` is copied verbatim (i18n coverage must hold
/// for a materialized campaign that declares languages).
pub fn materialize_from(base: &Path, patch: &serde_json::Value, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for f in STAGE_FILES {
        std::fs::copy(base.join(f), dst.join(f)).unwrap();
    }
    copy_l10n_dir(base, dst);
    if let Some(docs) = patch.get("documents").and_then(|d| d.as_object()) {
        for (stage, doc) in docs {
            let file = dst.join(format!("{stage}.json"));
            let text = serde_json::to_string_pretty(doc).unwrap();
            std::fs::write(file, text).unwrap();
        }
    }
}

/// Strip any i18n from a materialized campaign at `dir`: drop `world.languages`
/// and remove the `l10n/` directory. Used by build/solver fixtures derived from a
/// language-declaring campaign (e.g. keep-trial) that alter player-visible strings
/// and so would otherwise fail l10n coverage — they test build behavior, not i18n.
pub fn make_english_only(dir: &Path) {
    let world_path = dir.join("world.json");
    let mut world: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&world_path).unwrap()).unwrap();
    if let Some(content) = world.get_mut("content").and_then(|c| c.as_object_mut()) {
        content.remove("languages");
    }
    std::fs::write(&world_path, serde_json::to_string_pretty(&world).unwrap()).unwrap();
    let _ = std::fs::remove_dir_all(dir.join("l10n"));
}

/// Copy `base/l10n/*.json` into `dst/l10n/` if the source directory exists.
pub fn copy_l10n_dir(base: &Path, dst: &Path) {
    let src = base.join("l10n");
    if !src.is_dir() {
        return;
    }
    let dst_l10n = dst.join("l10n");
    std::fs::create_dir_all(&dst_l10n).unwrap();
    for entry in std::fs::read_dir(&src).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            std::fs::copy(&path, dst_l10n.join(path.file_name().unwrap())).unwrap();
        }
    }
}

/// The prefab library directory. The library lives in the content repo
/// (`delvewright-campaigns`), reached at `campaigns/prefabs` — the `campaigns/`
/// symlink locally, and a content-repo checkout at that path in CI (spec-0007
/// Step 0). Mirrors the compiler's default `--prefabs campaigns/prefabs`.
///
/// **A fixture may only bind prefabs that exist at the PINNED content SHA**
/// (`versions.toml` `[content].sha`, which `.github/actions/checkout-content`
/// checks out). The local `campaigns/` symlink usually points at a working
/// checkout that is far AHEAD of the pin, so a fixture written against a newer
/// prefab passes locally and fails CI with `DW0300` ("no matching prefab
/// metadata") — the classic works-on-my-machine shape, and the reason this note
/// exists. To reproduce CI exactly, point the symlink at a clone checked out at
/// the pinned SHA. Bumping the pin to make a fixture build is a content-repo
/// decision, never a fix for a test.
///
/// # This asserts the library PARSES, and that is not decoration
///
/// The note above documented the hazard and the hazard kept happening — three
/// separate rounds lost to it on 2026-08-08 alone — because the failure does
/// not look like what it is. `PrefabRegistry::load_dir` reports a metadata file
/// this `delvec` cannot parse as `DW0346` in `load_diagnostics()`, and **the
/// CLI drains that list** (`main::validate_loaded`) so `delvec` users get the
/// real message. Integration tests build a `Plan` directly and never drain it,
/// so the prefab is simply absent from the registry and the first thing anyone
/// sees is `DW0300` "no matching prefab metadata" — a message that then states,
/// confidently and wrongly, "this is a prefab-library/naming issue".
///
/// A key newer than this engine is no longer one of those cases — the document
/// has one definition, it keeps what it does not model, and the report is a
/// `DW0543` warning. What remains is a genuinely malformed file: a wrong-typed
/// value, an absent required block, a tile-set manifest this delvec cannot
/// place.
///
/// So this checks it once and says what actually happened. Docs are the weakest
/// form a lesson can take (CLAUDE.md debug doctrine); a tooling default that
/// makes the pitfall impossible is stronger, and this is the one place all 72
/// call sites already go through.
pub fn prefabs_dir() -> PathBuf {
    static CHECKED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    let dir = repo_root().join("campaigns/prefabs");
    CHECKED.get_or_init(|| {
        let Ok(reg) = delvec::compiler::registry::PrefabRegistry::load_dir(&dir) else {
            // An unreadable directory is the caller's own problem and every call
            // site already fails clearly on it; only the PARSE case impersonates
            // something else.
            return;
        };
        // Errors only, and the distinction is the point: an ERROR means the
        // file did not parse and the prefab is absent from the registry, which
        // is the state that impersonates a naming problem. A warning (`DW0543`,
        // a key newer than this engine) leaves the prefab loaded and usable, so
        // it cannot produce that impersonation — and it has its own test, which
        // states its binding count, in `tests/registry_load.rs`.
        let diags: Vec<_> = reg
            .load_diagnostics()
            .iter()
            .filter(|d| d.severity == delvewright_dsl::Severity::Error)
            .collect();
        assert!(
            diags.is_empty(),
            "the prefab library at {} has {} file(s) this delvec cannot parse, so those \
             prefabs are ABSENT from the registry and every fixture binding one will fail \
             as DW0300 \"no matching prefab metadata\" — which is not what went wrong.\n\n\
             Look first at whether the `campaigns/` symlink points at a content checkout \
             this engine cannot read: a wrong-typed value or an absent required block \
             drops the whole file. Point it at the SHA `versions.toml` [content].sha \
             pins, which is what CI builds against.\n\n{}",
            dir.display(),
            diags.len(),
            diags
                .iter()
                .map(|d| format!("  {} {}", d.code, d.message))
                .collect::<Vec<_>>()
                .join("\n")
        );
    });
    dir
}

/// The DSL invalid-fixture directory (patch files).
pub fn dsl_invalid_dir() -> PathBuf {
    repo_root().join("crates/dsl/fixtures/invalid")
}

/// This crate's own test fixtures.
pub fn compiler_fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Recursively copy a directory tree (used to make a private, mutable copy of
/// `prefabs_dir()` for tests that corrupt prefab metadata/structures — the real
/// `campaigns/prefabs` is a checkout of the separate content repo and must never
/// be written to by a test).
pub fn copy_dir_all(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let path = entry.unwrap().path();
        let to = dst.join(path.file_name().unwrap());
        if path.is_dir() {
            copy_dir_all(&path, &to);
        } else {
            std::fs::copy(&path, &to).unwrap();
        }
    }
}

/// Materialize a full campaign directory at `dst` = the valid hello-world base
/// with each stage in `patch["documents"]` overwritten by its replacement
/// envelope. Returns nothing; panics on IO error (tests).
pub fn materialize(patch: &serde_json::Value, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    let base = hello_world_dir();
    for f in STAGE_FILES {
        std::fs::copy(base.join(f), dst.join(f)).unwrap();
    }
    if let Some(docs) = patch.get("documents").and_then(|d| d.as_object()) {
        for (stage, doc) in docs {
            let file = dst.join(format!("{stage}.json"));
            let text = serde_json::to_string_pretty(doc).unwrap();
            std::fs::write(file, text).unwrap();
        }
    }
}

/// The build-input map for a campaign directory: the stage documents and, under
/// i18n v2 (spec-0029), every `l10n/<code>.json` sidecar — the resource pack now
/// carries a lang file per declared language, so a build of a multilingual
/// campaign that is handed no sidecars fails (`DW0180`) rather than silently
/// shipping one language. A test that builds a campaign declaring `languages`
/// must pass this instead of an empty map.
pub fn campaign_inputs(dir: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    delvec::compiler::load::load_campaign_dir(dir)
        .expect("campaign dir loads")
        .inputs
}

/// Patch a JSON document **structurally**: parse the text, hand the closure the
/// parsed value, and return it re-rendered in canonical form.
///
/// Tests used to splice fixtures with `str::replace` over exact indented text.
/// That coupling is invisible when it breaks: `str::replace` matching nothing
/// returns the input unchanged, so the test goes on to assert against an
/// **unpatched** campaign and passes for the wrong reason. Reformatting every
/// fixture into canonical form exposed four such silent no-ops at
/// once — including the `DW0307` unroutable-move test, which had been asserting
/// against a campaign with no `move-npc` in it. A structural patch cannot miss:
/// an absent key is a panic, not a quiet pass.
pub fn patch_doc(text: &str, f: impl FnOnce(&mut serde_json::Value)) -> String {
    let mut v: serde_json::Value = serde_json::from_str(text).expect("fixture is valid JSON");
    f(&mut v);
    delvewright_dsl::to_canonical_string(&v).expect("patched fixture serializes")
}

/// [`patch_doc`] against a file, in place.
pub fn patch_file(path: &Path, f: impl FnOnce(&mut serde_json::Value)) {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
    std::fs::write(path, patch_doc(&text, f)).unwrap();
}

/// The effect list a quest runs when one of its objectives completes —
/// `content.quests[<quest>].on_objective_complete[<objective>]`. Panics if the
/// path is absent, which is the whole point (see [`patch_doc`]).
pub fn objective_effects<'a>(
    doc: &'a mut serde_json::Value,
    quest: usize,
    objective: &str,
) -> &'a mut Vec<serde_json::Value> {
    doc["content"]["quests"][quest]["on_objective_complete"][objective]
        .as_array_mut()
        .unwrap_or_else(|| panic!("quests[{quest}].on_objective_complete[{objective}] is an array"))
}

/// Every validation diagnostic `validate_campaign_with` raises — the list `delvec`
/// prints and derives its exit code from (`compiler::main`).
pub fn validation_diagnostics(
    c: &delvewright_dsl::Campaign,
    items: &delvec::compiler::registry::FullItemRegistry,
    prefabs: &delvec::compiler::registry::PrefabRegistry,
    entities: &delvec::compiler::registry::FullEntityRegistry,
) -> Vec<delvewright_dsl::Diagnostic> {
    delvewright_dsl::validate_campaign_with(c, items, prefabs, entities)
}

// ---------------------------------------------------------------------------
// The declarations every campaign owes
// ---------------------------------------------------------------------------

/// The eleven story-node effect verbs `DW0481` demands a `happening` on, with the
/// verb their placeholder beat states.
const STORY_VERBS: [(&str, &str); 11] = [
    ("open-gate", "opens"),
    ("close-gate", "seals"),
    ("campaign-complete", "survives"),
    ("spawn-wave", "arrives"),
    ("despawn-npc", "departs"),
    ("move-npc", "arrives"),
    ("spawn-npc", "arrives"),
    ("spawn-actor", "arrives"),
    ("despawn-actor", "departs"),
    ("move-actor", "arrives"),
    ("unleash-actor", "arrives"),
];

fn happening(verb: &str, text: String) -> serde_json::Value {
    serde_json::json!({ "verb": verb, "text": text })
}

fn declare_effects(node: &mut serde_json::Value) {
    match node {
        serde_json::Value::Object(map) => {
            let verb = map
                .get("type")
                .and_then(|t| t.as_str())
                .and_then(|t| STORY_VERBS.iter().find(|(v, _)| *v == t).map(|(_, h)| *h));
            if let Some(h) = verb
                && !map.contains_key("happening")
            {
                let t = map["type"].as_str().unwrap().to_string();
                map.insert(
                    "happening".into(),
                    happening(h, format!("the `{t}` beat plays")),
                );
            }
            for v in map.values_mut() {
                declare_effects(v);
            }
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(declare_effects),
        _ => {}
    }
}

/// Fill in the declarations the engine demands of every campaign and that a
/// fixture about something else has no opinion on: a `happening` on every quest,
/// objective, ambush and story-node effect (`DW0481`); a `cast` entry per quest
/// for every stage-2 NPC, standing at its own anchor and offering its tree's root
/// (`DW0460`); a `title` and `hint` on every `kill`, and a `title` and
/// `item_name` on a `collect` that adopts a container (`DW0860`–`DW0863`).
///
/// A field the fixture states is left alone, so a test about one of these
/// declarations writes its own and this never contradicts it. What it never
/// supplies is a wording: a `sealed_hint` or a shortcut's press answer is a
/// content decision (`DW0429`), and a fixture that seals something says what the
/// seal answers.
pub fn declare_story(
    quests: &mut serde_json::Value,
    npcs: &serde_json::Value,
    dialogue: &serde_json::Value,
) {
    let roots: std::collections::BTreeMap<String, String> = dialogue["content"]["dialogues"]
        .as_array()
        .map(|trees| {
            trees
                .iter()
                .filter_map(|t| {
                    Some((
                        t["npc"].as_str()?.to_string(),
                        t["root"].as_str()?.to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    let bodies: Vec<(String, String)> = npcs["content"]["npcs"]
        .as_array()
        .map(|ns| {
            ns.iter()
                .filter_map(|n| {
                    Some((
                        n["id"].as_str()?.to_string(),
                        n["anchor"].as_str()?.to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    let content = &mut quests["content"];
    if let Some(list) = content["quests"].as_array_mut() {
        for q in list {
            let id = q["id"].as_str().unwrap_or("").to_string();
            if q.get("happening").is_none() {
                q["happening"] = happening("learns", format!("the party takes on {id}"));
            }
            if let Some(objs) = q["objectives"].as_array_mut() {
                for o in objs {
                    let oid = o["id"].as_str().unwrap_or("").to_string();
                    let kind = o["type"].as_str().unwrap_or("").to_string();
                    if o.get("happening").is_none() {
                        let verb = match kind.as_str() {
                            "reach-anchor" => "arrives",
                            "kill" => "survives",
                            "collect" => "gains",
                            "interact" => "opens",
                            _ => "learns",
                        };
                        o["happening"] = happening(verb, format!("the party completes {oid}"));
                    }
                    if kind == "kill" {
                        if o.get("title").is_none() {
                            o["title"] = serde_json::json!(format!("Fight: {oid}"));
                        }
                        if o.get("hint").is_none() {
                            o["hint"] = serde_json::json!("The wave comes to you here.");
                        }
                    }
                    if kind == "collect" && o.get("container").is_some() {
                        if o.get("title").is_none() {
                            o["title"] = serde_json::json!(format!("Collect: {oid}"));
                        }
                        if o.get("item_name").is_none() {
                            o["item_name"] = serde_json::json!("the token");
                        }
                    }
                }
            }
            if !bodies.is_empty() {
                if q.get("cast").is_none() {
                    q["cast"] = serde_json::json!({});
                }
                for (npc, anchor) in &bodies {
                    if q["cast"].get(npc).is_none() {
                        q["cast"][npc] = serde_json::json!({
                            "at": anchor,
                            "doing": format!("keeping to {anchor}"),
                            "dialogue": roots.get(npc).cloned().unwrap_or_else(|| "none".to_string()),
                        });
                    }
                }
            }
        }
    }
    if let Some(list) = content.get_mut("ambushes").and_then(|a| a.as_array_mut()) {
        for a in list {
            if a.get("happening").is_none() {
                let id = a["id"].as_str().unwrap_or("").to_string();
                a["happening"] = happening("arrives", format!("ambush {id} springs"));
            }
        }
    }
    declare_effects(content);
}

/// The dialogue half of [`declare_story`]: a `happening` on every option that
/// sets a flag (the story-weight options `DW0481` reads), and on every
/// story-node effect an option runs.
pub fn declare_dialogue_story(dialogue: &mut serde_json::Value) {
    if let Some(trees) = dialogue["content"]["dialogues"].as_array_mut() {
        for t in trees {
            if let Some(nodes) = t["nodes"].as_array_mut() {
                for n in nodes {
                    if let Some(opts) = n["options"].as_array_mut() {
                        for o in opts {
                            let sets_flag = o["effects"]
                                .as_array()
                                .map(|es| es.iter().any(|e| e["type"] == "set-flag"))
                                .unwrap_or(false);
                            if sets_flag && o.get("happening").is_none() {
                                let label = o["label"].as_str().unwrap_or("").to_string();
                                o["happening"] =
                                    happening("believes", format!("the party answers: {label}"));
                            }
                        }
                    }
                }
            }
        }
    }
    declare_effects(&mut dialogue["content"]);
}

/// [`declare_story`] over a campaign directory, in place: `quests.json` against
/// the directory's own `npcs.json` and `dialogue.json`, then the dialogue.
pub fn declare_story_dir(dir: &Path) {
    let read = |name: &str| -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(dir.join(name)).unwrap()).unwrap()
    };
    let npcs = read("npcs.json");
    let dialogue = read("dialogue.json");
    patch_file(&dir.join("quests.json"), |q| {
        declare_story(q, &npcs, &dialogue)
    });
    patch_file(&dir.join("dialogue.json"), declare_dialogue_story);
}

/// [`declare_story`] over a quests document written in a test, against the
/// `npcs.json` and `dialogue.json` of `base` — the campaign directory the
/// document stands in for.
pub fn declared_quests_doc(quests: &str, base: &Path) -> String {
    let read = |name: &str| -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(base.join(name)).unwrap()).unwrap()
    };
    let npcs = read("npcs.json");
    let dialogue = read("dialogue.json");
    patch_doc(quests, |q| declare_story(q, &npcs, &dialogue))
}

/// [`declare_dialogue_story`] over a dialogue document written in a test.
pub fn declared_dialogue_doc(dialogue: &str) -> String {
    patch_doc(dialogue, declare_dialogue_story)
}

// ---------------------------------------------------------------------------
// A tiled zone, synthesised
// ---------------------------------------------------------------------------

/// Gzip-framed vanilla structure NBT: `size`, a palette and the given non-air
/// cells.
///
/// It writes the `size` tag, which the hand-rolled builders scattered through
/// the emit tests do not — and `size` is the template's own claim about how big
/// it is, which is exactly what `DW0803` compares the metadata against. A
/// generator that omits it produces bytes no extent check can bind to.
pub fn structure_nbt(size: [i32; 3], cells: &[([i32; 3], &str)]) -> Vec<u8> {
    use fastnbt::Value;
    use std::collections::HashMap;
    use std::io::Write;

    let mut names: Vec<String> = Vec::new();
    let mut blocks: Vec<Value> = Vec::new();
    for (p, n) in cells {
        let state = names.iter().position(|x| x == n).unwrap_or_else(|| {
            names.push((*n).to_string());
            names.len() - 1
        });
        let mut b = HashMap::new();
        b.insert(
            "pos".to_string(),
            Value::List(p.iter().map(|v| Value::Int(*v)).collect()),
        );
        b.insert("state".to_string(), Value::Int(state as i32));
        blocks.push(Value::Compound(b));
    }
    let palette = Value::List(
        names
            .iter()
            .map(|n| {
                let mut c = HashMap::new();
                c.insert("Name".to_string(), Value::String(n.clone()));
                Value::Compound(c)
            })
            .collect(),
    );
    let mut root = HashMap::new();
    root.insert(
        "size".to_string(),
        Value::List(size.iter().map(|v| Value::Int(*v)).collect()),
    );
    root.insert("DataVersion".to_string(), Value::Int(4671));
    root.insert("palette".to_string(), palette);
    root.insert("blocks".to_string(), Value::List(blocks));
    let raw = fastnbt::to_bytes(&Value::Compound(root)).unwrap();
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(&raw).unwrap();
    enc.finish().unwrap()
}

/// The zone the tiled-placement tests are built on: a sealed stone corridor
/// `9 x 5 x 60`, which is past the vanilla 48-per-axis cap on `z` and therefore
/// ships as two tiles (`z` 0..47 and 48..59).
pub const TILED_ZONE_SIZE: [i32; 3] = [9, 5, 60];

/// The `z` at which [`TILED_ZONE_SIZE`] is cut, which is the vanilla cap.
pub const TILED_ZONE_CUT: i32 = 48;

/// Every non-air cell of the sealed corridor, in whole-zone coordinates.
///
/// Sealed on all six sides so the world it makes is one a body cannot fall out
/// of: boundary safety is about a campaign's content and must not be the thing
/// a tiling test ends up measuring.
fn tiled_zone_cells() -> Vec<([i32; 3], &'static str)> {
    let [sx, sy, sz] = TILED_ZONE_SIZE;
    let mut cells = Vec::new();
    for z in 0..sz {
        for y in 0..sy {
            for x in 0..sx {
                if x == 0 || x == sx - 1 || y == 0 || y == sy - 1 || z == 0 || z == sz - 1 {
                    cells.push(([x, y, z], "minecraft:stone"));
                } else if y == sy - 2 && z % 6 == 3 && x == sx / 2 {
                    // A light line, so the corridor is a lit interior rather
                    // than a declared-lit claim about a dark box.
                    cells.push(([x, y, z], "minecraft:glowstone"));
                }
            }
        }
    }
    cells
}

/// Write the tiled corridor into `dir` as a prefab library entry: two `.nbt`
/// tiles plus the one metadata document naming them.
///
/// `anchors` is written verbatim into the document, in WHOLE-ZONE coordinates —
/// which is the property tiling must not disturb: an anchor is a fact about the
/// building, and a cut never moves one.
///
/// `extra` is added to the corridor's own cells, also in whole-zone
/// coordinates, and is how a caller puts something in the zone that the cut
/// then has to survive — a pool of water past the cut, say. It is an argument
/// rather than a second writer because what a caller varies is what is IN the
/// zone; a `write_tiled_zone_with_water` would leave whoever wants lava, or a
/// chest, with nowhere to go.
pub fn write_tiled_zone(
    dir: &Path,
    id: &str,
    anchors: serde_json::Value,
    extra: &[([i32; 3], &str)],
) {
    std::fs::create_dir_all(dir).unwrap();
    let [sx, sy, sz] = TILED_ZONE_SIZE;
    let cut = TILED_ZONE_CUT;
    let cells = tiled_zone_cells();
    // Later wins, so `extra` may carve as well as add: a cell it names replaces
    // the shell cell underneath it rather than fighting with it.
    let mut by_pos: std::collections::BTreeMap<[i32; 3], &str> = cells.into_iter().collect();
    for (pos, name) in extra {
        by_pos.insert(*pos, *name);
    }
    let cells: Vec<([i32; 3], &str)> = by_pos.into_iter().collect();
    let mut parts = Vec::new();
    for (i, (z0, depth)) in [(0, cut), (cut, sz - cut)].into_iter().enumerate() {
        let tile: Vec<([i32; 3], &str)> = cells
            .iter()
            .filter(|(p, _)| p[2] >= z0 && p[2] < z0 + depth)
            .map(|(p, n)| ([p[0], p[1], p[2] - z0], *n))
            .collect();
        let file = format!("{id}.x0y0z{i}.nbt");
        std::fs::write(dir.join(&file), structure_nbt([sx, sy, depth], &tile)).unwrap();
        parts.push(serde_json::json!({
            "file": file,
            "id": format!("{id}.x0y0z{i}"),
            "grid_index": [0, 0, i as i32],
            "offset": [0, 0, z0],
            "size": [sx, sy, depth],
        }));
    }
    let meta = serde_json::json!({
        "prefab_id": format!("prefab/{id}"),
        "structure_set": {
            "base": id,
            "size": TILED_ZONE_SIZE,
            "part_max": cut,
            "grid": [1, 1, 2],
            "data_version": 4671,
            "generator": "crates/delvec/tests/common",
            "parts": parts,
        },
        "anchors": anchors,
        "connectors": [],
        "lighting": { "profile": "lit", "measured_min_light": 8, "measured": "2026-08-15" },
        "license": {
            "source": "original",
            "spdx": "GPL-3.0-or-later",
            "note": "Test fixture.",
            "provenance": "Synthesised by crates/delvec/tests/common::write_tiled_zone."
        }
    });
    std::fs::write(
        dir.join(format!("{id}.json")),
        serde_json::to_string_pretty(&meta).unwrap() + "\n",
    )
    .unwrap();
}

/// Which ocean-legitimate `hello-room` a fixture wants.
pub enum OceanRoom {
    /// The shore: two courses of solid plinth with a tide pool cut into the
    /// floor course, so the walk plane stands at local y=3 — one block clear of
    /// the sea, which is the island convention. Nothing the sea does reaches it.
    Shore,
    /// The cellar: the same declared waterline, and a room whose floor is the
    /// piece's own local y=0, so its walk plane is a block UNDER the surface.
    /// A legal piece — `DW0344` is satisfied, the waterline really is where the
    /// declaration says — and the shape `DW0851` exists for: open a face and the
    /// sea is on the walk plane.
    Cellar,
}

/// A copy of the prefab library in which `hello-room` is a piece that can stand
/// on an ocean, for every fixture that declares `horizon: ocean`.
///
/// The shipped `hello-room` cannot. `horizon: ocean` puts an area origin at
/// y=60 under a sea at 62, and a piece with its floor at local y=0 then stands
/// its walk plane at y=61 — a block under the surface — while declaring nothing
/// about where it meets the sea. That is not a modelling nicety: the room
/// carries four iron bars at local y=1 and 2, `/place template` hands each of
/// them the water already in the cell, and the delivered room floods from its
/// own gate. `DW0344` refuses the placement and `DW0851` refuses the flood, both
/// correctly.
///
/// So an ocean fixture gets a piece built for an ocean: the same room and the
/// same anchor names, with a tide pool whose surface is the sea's own plane, so
/// `waterline_y: 2` is a measurement rather than a fiction. [`OceanRoom`]
/// chooses whether its walk plane stands above that plane or below it.
///
/// Returns the directory, which is a temp copy: nothing here touches the library.
pub fn ocean_prefabs_dir(tag: &str, room: OceanRoom) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(tag);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    copy_dir_all(&prefabs_dir(), &dir);
    let plinth = match room {
        OceanRoom::Shore => 3,  // solid to local y=2; the walk plane is y=3
        OceanRoom::Cellar => 1, // solid to local y=0; the walk plane is y=1
    };
    let size = [11, plinth + 6, 11];
    let floor = plinth; // the first air course: where a body's feet go
    let mut cells: Vec<([i32; 3], &str)> = Vec::new();
    for x in 0..size[0] {
        for z in 0..size[2] {
            for y in 0..plinth {
                // The shore's tide pool is a cell of the floor course; the
                // cellar has no floor course above the sea, so its water is a
                // sealed well in the corner buttress below.
                let pool = matches!(room, OceanRoom::Shore) && [x, y, z] == [1, 2, 1];
                cells.push((
                    [x, y, z],
                    if pool {
                        "minecraft:water"
                    } else {
                        "minecraft:stone"
                    },
                ));
            }
            // Walls and roof around the room the anchors stand in. Four
            // glowstones in the roof: a dark room is `DW0210`, and a fixture
            // that trips a check it is not about proves nothing about the one
            // it is.
            let lamp = matches!((x, z), (3, 3) | (3, 7) | (7, 3) | (7, 7));
            for y in floor..size[1] {
                if x == 0 || x == size[0] - 1 || z == 0 || z == size[2] - 1 || y == size[1] - 1 {
                    let roof = y == size[1] - 1;
                    cells.push((
                        [x, y, z],
                        if roof && lamp {
                            "minecraft:glowstone"
                        } else {
                            "minecraft:stone"
                        },
                    ));
                }
            }
        }
    }
    if let OceanRoom::Cellar = room {
        // The well: one water cell at local y=2 — the sea's plane, which is what
        // `waterline_y` declares — sealed on every face by the corner buttress,
        // so it is the piece's own water and not a leak (`DW0318`).
        for c in [[1, 1, 1], [1, 3, 1], [2, 2, 1], [1, 2, 2]] {
            cells.push((c, "minecraft:stone"));
        }
        cells.push(([1, 2, 1], "minecraft:water"));
    }
    std::fs::write(dir.join("hello-room.nbt"), structure_nbt(size, &cells)).unwrap();
    let meta = serde_json::json!({
        "prefab_id": "prefab/hello-room",
        "structure": {
            "file": "hello-room.nbt",
            "id": "hello-room",
            "size": size,
            "data_version": 4671,
            "generator": "crates/delvec/tests/common::ocean_prefabs_dir",
        },
        "waterline_y": 2,
        "anchors": {
            "spawn": { "pos": [5, floor, 2], "facing": "south", "role": "entry" },
            "anchor/keeper-stand": { "pos": [5, floor, 4], "facing": "north" },
            "anchor/exit": { "pos": [5, floor, 8] },
            "anchor/door": {
                "region": { "from": [4, floor, 6], "to": [5, floor + 2, 6] },
                "block": "minecraft:iron_bars"
            },
        },
        "connectors": [],
        "lighting": { "profile": "lit", "measured_min_light": 8, "measured": "2026-08-15" },
        "license": {
            "source": "original",
            "spdx": "GPL-3.0-or-later",
            "note": "Test fixture.",
            "provenance": "Synthesised by crates/delvec/tests/common::ocean_prefabs_dir."
        }
    });
    std::fs::write(
        dir.join("hello-room.json"),
        serde_json::to_string_pretty(&meta).unwrap() + "\n",
    )
    .unwrap();
    dir
}

/// Write a single-template prefab into `dir`: one `.nbt` plus its metadata.
///
/// The companion of [`write_tiled_zone`] on the other packaging, so a test that
/// is about something else — where a piece ends, where its water goes — can
/// compose a piece without caring which packaging it got.
pub fn write_single_prefab(
    dir: &Path,
    id: &str,
    size: [i32; 3],
    cells: &[([i32; 3], &str)],
    anchors: serde_json::Value,
) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join(format!("{id}.nbt")), structure_nbt(size, cells)).unwrap();
    let meta = serde_json::json!({
        "prefab_id": format!("prefab/{id}"),
        "structure": {
            "file": format!("{id}.nbt"),
            "id": id,
            "size": size,
            "data_version": 4671,
            "generator": "crates/delvec/tests/common",
        },
        "anchors": anchors,
        "connectors": [],
        "lighting": { "profile": "lit", "measured_min_light": 8, "measured": "2026-08-15" },
        "license": {
            "source": "original",
            "spdx": "GPL-3.0-or-later",
            "note": "Test fixture.",
            "provenance": "Synthesised by crates/delvec/tests/common::write_single_prefab."
        }
    });
    std::fs::write(
        dir.join(format!("{id}.json")),
        serde_json::to_string_pretty(&meta).unwrap() + "\n",
    )
    .unwrap();
}

/// The hello-world campaign materialised at `dst`, with its one area rebound to
/// `prefab/<id>`. Shared by every test that needs a real campaign around a
/// synthetic piece.
pub fn campaign_bound_to(dst: &Path, id: &str) -> PathBuf {
    std::fs::create_dir_all(dst).unwrap();
    for f in STAGE_FILES {
        std::fs::copy(hello_world_dir().join(f), dst.join(f)).unwrap();
    }
    patch_file(&dst.join("world.json"), |v| {
        v["content"]["areas"][0]["prefab"] = serde_json::json!(format!("prefab/{id}"));
    });
    dst.to_path_buf()
}

/// **The prefab library, with every piece declaring its own outside** — a copy,
/// for the fixtures that put the party outdoors.
///
/// `DW0885` asks a placed piece whether its outward solid boundary is buried or
/// declared, and it asks only where the party's own air reaches: a fixture whose
/// campaign is one sealed room never meets it. A fixture that opens a doorway
/// onto the void, or stands a shore piece under an open sky, does — and the
/// shipped library declares no `shown_faces`, because which of a piece's sides
/// are finished exterior surface is a claim about the ASSET and belongs to the
/// content repository's own adoption of the field.
///
/// So a fixture about daylight, or teleports, or a boundary drop, gets a library
/// in which every piece says its sides are its own. It is the same move
/// [`ocean_prefabs_dir`] makes for a different check, and for the reason stated
/// there: *a fixture that trips a check it is not about proves nothing about the
/// one it is.* The declaration is honest in these worlds — a free-standing box in
/// a void is a box whose every side is what the player would see.
///
/// It is the whole library rather than a named piece so that a fixture drawing
/// from a POOL, which cannot know which prefab the solver will seat, is covered
/// by the same call.
///
/// Returns the directory, which is a temp copy: nothing here touches the library.
pub fn shown_prefabs_dir(tag: &str) -> PathBuf {
    // A fresh directory per CALL, not per tag. Tests in one binary run on
    // several threads, and a helper that deletes and rewrites one shared path
    // is two tests reading a library while a third is halfway through copying
    // it — an intermittent red, which is an under-specified test rather than
    // something to re-run.
    static NTH: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let nth = NTH.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("shown-{tag}-{nth}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    copy_dir_all(&prefabs_dir(), &dir);
    let mut patched = 0usize;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if path.file_name().and_then(|f| f.to_str()) == Some("pools.json") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let mut doc: serde_json::Value = serde_json::from_str(&text).unwrap();
        let Some(obj) = doc.as_object_mut() else {
            continue;
        };
        if !obj.contains_key("prefab_id") {
            continue;
        }
        // Exactly the sides that have something on them. `DW0885`'s second arm
        // refuses a `shown_faces` naming a face of pure air — a side that is all
        // doorway is not a side a piece can finish — so a blanket six would be
        // the fixture tripping the very check it is here to step out of the way
        // of. The sides are read off the piece's own bytes, the same way the
        // compiler reads them.
        declare_shown_faces_at(&path);
        patched += 1;
    }
    // The binding count of the helper itself: a copy that patched nothing is a
    // library the fixtures below are not actually using, and it would look
    // exactly like one that worked.
    assert!(
        patched > 0,
        "shown_prefabs_dir patched no prefab document in {} — the copy is not the library",
        dir.display()
    );
    dir
}

/// **Declare, on the prefab document at `dir/<id>.json`, exactly the sides its
/// own bytes put a block on** (`DW0885`).
///
/// The companion of [`shown_prefabs_dir`] for a fixture that WRITES its own
/// piece rather than copying the library's: same rule, same reading of the same
/// bytes, one implementation.
pub fn declare_shown_faces(dir: &Path, id: &str) {
    declare_shown_faces_at(&dir.join(format!("{id}.json")));
}

fn declare_shown_faces_at(path: &Path) {
    let mut doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let dir = path.parent().unwrap().to_path_buf();
    let obj = doc.as_object_mut().unwrap();
    let sides = solid_sides(&dir, obj);
    assert!(
        !sides.is_empty(),
        "{} has no solid cell on any of its six sides",
        path.display()
    );
    obj.insert("shown_faces".to_string(), serde_json::json!(sides));
    std::fs::write(path, serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();
}

/// Which of a prefab's six sides carry at least one block, read off the piece's
/// own `.nbt` bytes exactly as the compiler reads them — including a tiled piece,
/// whose tiles are composed back into the one box they were cut out of.
fn solid_sides(dir: &Path, meta: &serde_json::Map<String, serde_json::Value>) -> Vec<String> {
    let mut files: Vec<(String, [i64; 3])> = Vec::new();
    let mut size = [0i64; 3];
    if let Some(st) = meta.get("structure") {
        size = read3(&st["size"]);
        files.push((st["file"].as_str().unwrap().to_string(), [0, 0, 0]));
    } else if let Some(set) = meta.get("structure_set") {
        size = read3(&set["size"]);
        for part in set["parts"].as_array().unwrap() {
            files.push((
                part["file"].as_str().unwrap().to_string(),
                read3(&part["offset"]),
            ));
        }
    }
    let mut present = [[false; 2]; 3];
    for (file, offset) in files {
        let bytes = std::fs::read(dir.join(&file)).unwrap();
        for (cell, name) in delvec::compiler::assembled::structure_named_cells(&bytes) {
            if name == "minecraft:air" {
                continue;
            }
            for axis in 0..3 {
                let c = i64::from(cell[axis]) + offset[axis];
                if c == 0 {
                    present[axis][0] = true;
                }
                if c == size[axis] - 1 {
                    present[axis][1] = true;
                }
            }
        }
    }
    // The six words, in the order the compiler's own vocabulary spells them.
    let names = [["west", "east"], ["down", "up"], ["north", "south"]];
    let mut out: Vec<String> = Vec::new();
    for axis in 0..3 {
        for lo in 0..2 {
            if present[axis][lo] {
                out.push(names[axis][lo].to_string());
            }
        }
    }
    out.sort();
    out
}

fn read3(v: &serde_json::Value) -> [i64; 3] {
    let a = v.as_array().unwrap();
    [
        a[0].as_i64().unwrap(),
        a[1].as_i64().unwrap(),
        a[2].as_i64().unwrap(),
    ]
}

/// **A synthetic stone box as gzipped structure-template bytes**, lit from
/// inside so the darkness gate never pre-empts whatever the caller is proving.
///
/// `open_x0` drops the whole `x == 0` slab — floor, wall and ceiling together.
/// That one flag is the variable two different proofs turn: it makes the column
/// beside the interior floor bottomless, so a body standing there is one step
/// from leaving the world (`DW0322`), and it is also what lets the party's air
/// out of the room, so the piece's own outside becomes something a body can be
/// in front of (`DW0885`).
///
/// Shared rather than copied. It lived in `tests/boundary_assembled.rs` and was
/// copied verbatim into a second file, which
/// `tools/check-structure-emitters.py` caught in the only way it can: the copy
/// was a new site naming `fastnbt::to_bytes` that judged no palette, and the
/// repair the tool offers first is an exemption. Two fixtures framing the same
/// box would have been two entries on an exclusion list, which is exactly the
/// growth that check exists to refuse — so the function moved here, both call
/// sites lost the ingredient, and the list got shorter instead.
///
/// The palette is three literal ids in this one function — air, stone,
/// glowstone — and the bytes never leave the test that asks for them: they are
/// handed to `emit::build` in memory and written into no prefab library, which
/// is why this is a fixture and not an emitter.
pub fn box_nbt(size: [i32; 3], open_x0: bool) -> Vec<u8> {
    use fastnbt::Value;
    let [sx, sy, sz] = size;
    let mut blocks: Vec<Value> = Vec::new();
    let mut push = |x: i32, y: i32, z: i32, state: i32| {
        let mut c = std::collections::HashMap::new();
        c.insert(
            "pos".to_string(),
            Value::List(vec![Value::Int(x), Value::Int(y), Value::Int(z)]),
        );
        c.insert("state".to_string(), Value::Int(state));
        blocks.push(Value::Compound(c));
    };
    for x in 0..sx {
        if open_x0 && x == 0 {
            continue;
        }
        for y in 0..sy {
            for z in 0..sz {
                if y == 0 || y == sy - 1 || x == 0 || x == sx - 1 || z == 0 || z == sz - 1 {
                    push(x, y, z, 1); // stone
                }
            }
        }
    }
    // Interior glowstone: the lighting gate is not what these tests are about.
    for x in [2, sx / 2, sx - 3] {
        for z in [2, sz / 2, sz - 3] {
            push(x, sy - 2, z, 2);
        }
    }
    let palette = Value::List(vec![
        box_pal_entry("minecraft:air"),
        box_pal_entry("minecraft:stone"),
        box_pal_entry("minecraft:glowstone"),
    ]);
    let mut root = std::collections::HashMap::new();
    root.insert("DataVersion".to_string(), Value::Int(4671));
    root.insert(
        "size".to_string(),
        Value::List(vec![Value::Int(sx), Value::Int(sy), Value::Int(sz)]),
    );
    root.insert("palette".to_string(), palette);
    root.insert("blocks".to_string(), Value::List(blocks));
    root.insert("entities".to_string(), Value::List(vec![]));
    let raw = fastnbt::to_bytes(&Value::Compound(root)).unwrap();
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    std::io::Write::write_all(&mut gz, &raw).unwrap();
    gz.finish().unwrap()
}

fn box_pal_entry(name: &str) -> fastnbt::Value {
    let mut c = std::collections::HashMap::new();
    c.insert("Name".to_string(), fastnbt::Value::String(name.to_string()));
    fastnbt::Value::Compound(c)
}
