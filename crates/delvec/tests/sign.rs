//! A sign says its words (spec-0115): the surface, the emission over every
//! route a sign comes to stand by, the proofs (`DW0431`, `DW1048`), the
//! binding line, the written world and determinism, over a one-room keep the
//! test writes itself.
//!
//! The room is a 15 × 8 × 9 stone box whose floor is the course `y = 3`. Four
//! signs stand in it by four routes: the piece's template lays a wall sign on
//! the north wall (`anchor/board`), a `use` trigger's prop places a standing
//! sign (`anchor/post`), an `interact` objective's prop places another
//! (`anchor/plaque`), and the first beat's `set-block` hangs a wall sign
//! (`anchor/slate`), which the second beat re-letters.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildFailure, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, DSL_VERSION, parse_campaign};

const ROOM: &str = "sign-room";
const SIZE: [i32; 3] = [15, 8, 9];
const WALK_Y: i32 = 4;
/// The template's sign, against the north wall.
const BOARD: [i32; 3] = [5, 5, 1];
const BOARD_BLOCK: &str = "minecraft:oak_wall_sign[facing=south]";

fn cells(board: &'static str) -> Vec<([i32; 3], &'static str)> {
    let [sx, sy, sz] = SIZE;
    let mut by: BTreeMap<[i32; 3], &'static str> = BTreeMap::new();
    for x in 0..sx {
        for z in 0..sz {
            for y in 0..sy {
                let shell = x == 0 || x == sx - 1 || z == 0 || z == sz - 1;
                if y < WALK_Y || y == sy - 1 || shell {
                    by.insert([x, y, z], "minecraft:stone");
                }
            }
        }
    }
    for x in [3, 7, 11] {
        for z in [2, 6] {
            by.insert([x, sy - 1, z], "minecraft:glowstone");
        }
    }
    by.insert(BOARD, board);
    by.into_iter().collect()
}

fn anchors() -> serde_json::Value {
    serde_json::json!({
        "spawn": { "pos": [2, 4, 4], "facing": "east", "role": "entry" },
        "anchor/keeper-stand": { "pos": [12, 4, 6], "facing": "west" },
        "anchor/exit": { "pos": [2, 4, 2] },
        "anchor/board": { "pos": BOARD },
        "anchor/post": { "pos": [9, 4, 4] },
        "anchor/plaque": { "pos": [11, 4, 2] },
        "anchor/slate": { "pos": [7, 5, 1] },
        "anchor/bare": { "pos": [4, 4, 6] }
    })
}

fn scratch(tag: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("sign")
        .join(format!("{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

const SIGNS: &str = r#""signs": [
  { "id": "sign/board", "anchor": "anchor/board",
    "front": { "lines": ["THE KEEP", "", "mind the door"] },
    "back": { "lines": ["", "[[bold|go back]]"], "color": "red", "glowing": true } },
  { "id": "sign/post", "anchor": "anchor/post", "front": { "lines": ["POST"] } },
  { "id": "sign/plaque", "anchor": "anchor/plaque", "front": { "lines": ["PLAQUE"], "color": "blue" } },
  { "id": "sign/slate", "anchor": "anchor/slate", "front": { "lines": ["FIRST"] } }
]"#;

const SLATE: &str = r#"{ "type": "set-block", "anchor": "anchor/slate", "block": "minecraft:oak_wall_sign[facing=south]" }"#;

const RELETTER: &str = r#"{ "type": "letter-sign", "sign": "sign/slate", "front": { "lines": ["", "SECOND"], "color": "green" } }"#;

/// Write the keep. `signs` is the `"signs": […]` member (or empty), `talk` the
/// first beat's effects and `read` the second's. Returns `(campaign, prefabs)`.
fn write_with(
    tag: &str,
    board: &'static str,
    signs: &str,
    talk: &str,
    read: &str,
) -> (PathBuf, PathBuf) {
    let root = scratch(tag);
    let lib = root.join("prefabs");
    common::write_single_prefab(&lib, ROOM, SIZE, &cells(board), anchors());
    let dir = common::campaign_bound_to(&root.join("campaign"), ROOM);
    let signs = if signs.is_empty() {
        String::new()
    } else {
        format!(", {signs}")
    };
    let quests = format!(
        r#"{{
  "dsl_version": "{DSL_VERSION}",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {{
    "quests": [
      {{
        "id": "quest/open-the-door",
        "trigger": {{ "type": "campaign-start" }},
        "objectives": [
          {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }},
          {{ "type": "interact", "id": "obj/read", "anchor": "anchor/plaque",
             "prop": {{ "block": "minecraft:oak_sign[rotation=4]" }}, "after": ["obj/talk"] }},
          {{ "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit",
             "radius": 1, "after": ["obj/read"] }}
        ],
        "on_objective_complete": {{
          "obj/talk": [ {talk} ],
          "obj/read": [ {read} ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ],
    "triggers": [
      {{ "id": "trigger/read-the-post", "at": "anchor/post", "on": {{ "on": "use" }},
         "prop": {{ "block": "minecraft:oak_sign[rotation=8]" }},
         "effects": [ {{ "type": "narrate", "text": "The post." }} ] }}
    ]{signs}
  }}
}}"#
    );
    std::fs::write(dir.join("quests.json"), quests).unwrap();
    common::declare_story_dir(&dir);
    (dir, lib)
}

fn write(tag: &str, signs: &str, talk: &str, read: &str) -> (PathBuf, PathBuf) {
    write_with(tag, BOARD_BLOCK, signs, talk, read)
}

fn campaign(dir: &Path) -> Campaign {
    let loaded = load_campaign_dir(dir).unwrap();
    let mut c = parse_campaign(&loaded.raw).expect("the case parses");
    delvewright_dsl::tag_translatables(&mut c);
    c
}

fn english(dir: &Path) -> Campaign {
    let loaded = load_campaign_dir(dir).unwrap();
    parse_campaign(&loaded.raw).expect("the case parses")
}

fn validate(dir: &Path, lib: &Path) -> Vec<delvewright_dsl::Diagnostic> {
    let c = english(dir);
    let prefabs = PrefabRegistry::load_dir(lib).unwrap();
    delvewright_dsl::validate_campaign_with(
        &c,
        &delvec::compiler::registry::FullItemRegistry::v1_21_11(),
        &prefabs,
        &delvec::compiler::registry::FullEntityRegistry::v1_21_11(),
    )
}

fn structures(plan: &Plan, lib: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut s = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                s.insert(
                    t.structure_file.clone(),
                    std::fs::read(lib.join(&t.structure_file)).unwrap(),
                );
            }
        }
    }
    s
}

fn try_build(dir: &Path, lib: &Path) -> Result<BuildOutput, BuildFailure> {
    let refused: Vec<_> = validate(dir, lib)
        .into_iter()
        .filter(|d| d.severity == delvewright_dsl::Severity::Error)
        .collect();
    assert!(
        refused.is_empty(),
        "the case does not validate: {refused:#?}"
    );
    let c = campaign(dir);
    let prefabs = PrefabRegistry::load_dir(lib).unwrap();
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let s = structures(&plan, lib);
    let inputs = load_campaign_dir(dir).unwrap().inputs;
    emit::build(
        &plan,
        &inputs,
        &s,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
}

fn build(dir: &Path, lib: &Path) -> BuildOutput {
    match try_build(dir, lib) {
        Ok(x) => x,
        Err(BuildFailure::Diagnostic { code, message }) => {
            panic!("expected the build to succeed, got {code}: {message}")
        }
        Err(e) => panic!("expected the build to succeed, got {e:?}"),
    }
}

fn refusal(dir: &Path, lib: &Path) -> (String, String) {
    match try_build(dir, lib) {
        Err(BuildFailure::Diagnostic { code, message }) => (code.to_string(), message),
        Ok(_) => panic!("expected a refusal, the build succeeded"),
        Err(e) => panic!("expected a diagnostic refusal, got {e:?}"),
    }
}

fn function(out: &BuildOutput, name: &str) -> Option<Vec<String>> {
    let key = out.keys().find(|k| {
        k.starts_with("datapack/data/") && k.ends_with(&format!("/function/{name}.mcfunction"))
    })?;
    Some(
        String::from_utf8(out[key].clone())
            .unwrap()
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
            .map(str::to_string)
            .collect(),
    )
}

/// The function body whose lines contain `needle`, as `(name, lines)`.
fn holding(out: &BuildOutput, needle: &str) -> Vec<(String, Vec<String>)> {
    out.keys()
        .filter(|k| k.starts_with("datapack/data/") && k.ends_with(".mcfunction"))
        .filter_map(|k| {
            let name = k
                .rsplit('/')
                .next()?
                .trim_end_matches(".mcfunction")
                .to_string();
            let lines = function(out, &name)?;
            lines
                .iter()
                .any(|l| l.contains(needle))
                .then_some((name, lines))
        })
        .collect()
}

/// The line after the one containing `first`, in `lines`.
fn after<'a>(lines: &'a [String], first: &str) -> Option<&'a str> {
    let i = lines.iter().position(|l| l.contains(first))?;
    lines.get(i + 1).map(String::as_str)
}

/// Criterion 1: the schema exports `signs[]` with exactly its four fields, a
/// face with `lines` (1..=4), `color` (the sixteen dyes) and `glowing`, and
/// `letter-sign` with `sign`, `front`, `back`.
#[test]
fn the_surface_is_signs_a_face_and_letter_sign() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_delvec"))
        .args(["schema", "--stage", "quests"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let schema: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let defs = &schema["$defs"];
    let keys = |v: &serde_json::Value| -> Vec<String> {
        let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
        k.sort();
        k
    };
    assert_eq!(
        keys(&defs["Sign"]["properties"]),
        ["anchor", "back", "front", "id"]
    );
    let face = &defs["SignFace"]["properties"];
    assert_eq!(keys(face), ["color", "glowing", "lines"]);
    assert_eq!(face["lines"]["minItems"], 1);
    assert_eq!(face["lines"]["maxItems"], 4);
    let dyes: Vec<&str> = defs["Dye"]["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["const"].as_str().unwrap())
        .collect();
    assert_eq!(dyes.len(), 16, "{dyes:?}");
    let text = serde_json::to_string(defs).unwrap();
    assert!(text.contains("\"letter-sign\""), "the effect is exported");
    let letter = defs
        .as_object()
        .unwrap()
        .values()
        .flat_map(|d| d["oneOf"].as_array().cloned().unwrap_or_default())
        .find(|v| v["properties"]["type"]["const"] == "letter-sign")
        .expect("letter-sign is a variant");
    let mut lk = keys(&letter["properties"]);
    lk.retain(|k| !["type", "when", "happening", "audience", "in"].contains(&k.as_str()));
    assert_eq!(lk, ["back", "front", "sign"]);
}

/// Criterion 3: one sign per route, each lettered where §4.2 puts it, by the
/// one compound with `is_waxed:1b`; every line passes the pinned tree (the
/// build validates every emitted line, so building is the assertion).
#[test]
fn every_route_letters_its_sign_where_it_stands() {
    let (dir, lib) = write("routes", SIGNS, SLATE, RELETTER);
    let out = build(&dir, &lib);
    for s in ["board", "post", "plaque", "slate"] {
        let body = function(&out, &format!("letter_{s}")).unwrap_or_else(|| panic!("letter_{s}"));
        assert_eq!(body.len(), 1, "{body:?}");
        assert!(body[0].starts_with("data merge block "), "{body:?}");
        assert!(body[0].ends_with(",is_waxed:1b}"), "{body:?}");
        assert!(body[0].contains("{front_text:{messages:["), "{body:?}");
        assert!(body[0].contains("back_text:{messages:["), "{body:?}");
    }
    let board = &function(&out, "letter_board").unwrap()[0];
    assert!(
        board.contains("color:\"red\",has_glowing_text:1b"),
        "{board}"
    );
    assert!(
        board.contains("translate:\"delve.hello-world.sign.board.front.0\""),
        "{board}"
    );
    // The template's sign: `setup_finish`, with no write before it.
    let finish = function(&out, "setup_finish").unwrap();
    let ns_call = |s: &str| finish.iter().any(|l| l.ends_with(&format!(":letter_{s}")));
    assert!(ns_call("board"), "{finish:#?}");
    // The trigger's prop: right after its `setblock`.
    assert!(
        after(&finish, "minecraft:oak_sign[rotation=8]")
            .is_some_and(|l| l.ends_with(":letter_post")),
        "{finish:#?}"
    );
    // The interact's prop: right after its `setblock`, in the activation.
    let act = holding(&out, "minecraft:oak_sign[rotation=4]");
    assert!(
        act.iter()
            .any(|(_, l)| after(l, "minecraft:oak_sign[rotation=4]")
                .is_some_and(|n| n.ends_with(":letter_plaque"))),
        "{act:#?}"
    );
    // The set-block: right after its `setblock`, in its bundle.
    let sb = holding(&out, "setblock 7 ");
    assert!(
        sb.iter()
            .any(|(_, l)| after(l, "minecraft:oak_wall_sign[facing=south]")
                .is_some_and(|n| n.ends_with(":letter_slate"))),
        "{sb:#?}"
    );
    // The letter-sign: its own body, named only the face it states.
    let re = function(&out, "reletter_slate_0").unwrap();
    assert!(re[0].contains("front_text:{messages:[\"\",{"), "{re:?}");
    assert!(!re[0].contains("back_text"), "{re:?}");
    assert!(re[0].contains("color:\"green\""), "{re:?}");
    assert!(!holding(&out, ":reletter_slate_0").is_empty());
    // PackTest: one template per sign and per letter-sign.
    for t in [
        "sign_board",
        "sign_post",
        "sign_plaque",
        "sign_slate",
        "reletter_slate_0",
    ] {
        assert!(
            out.keys()
                .any(|k| k.ends_with(&format!("/test/{t}.mcfunction"))),
            "PackTest `{t}`"
        );
    }
    let ledger: serde_json::Value = serde_json::from_slice(&out["validation/signs.json"]).unwrap();
    assert_eq!(ledger["examined"], 4);
    let rows = ledger["signs"].as_array().unwrap();
    for r in rows {
        for k in [
            "id",
            "anchor",
            "cell",
            "block",
            "kind",
            "budget",
            "placed_by",
            "lettered_in",
            "faces",
        ] {
            assert!(r.get(k).is_some(), "{k} in {r}");
        }
    }
    let board = rows.iter().find(|r| r["id"] == "sign/board").unwrap();
    assert_eq!(board["budget"], 90);
    assert_eq!(board["placed_by"], serde_json::json!(["load"]));
    assert_eq!(
        board["faces"]["front:sign.board.front"]["en"][0]["width"],
        46
    );
}

/// A campaign with no sign emits none of it.
#[test]
fn a_campaign_with_no_sign_emits_nothing_of_it() {
    let (dir, lib) = write("none", "", SLATE, r#"{ "type": "narrate", "text": "ok" }"#);
    let out = build(&dir, &lib);
    assert!(holding(&out, "data merge block").is_empty());
    assert!(!out.keys().any(|k| k.contains("letter_")));
    assert!(!out.contains_key("validation/signs.json"));
}

/// Criterion 11: the binding line prints on every build, `0 sign(s)` for a
/// campaign with none.
#[test]
fn the_binding_line_states_a_measured_zero() {
    let (dir, lib) = write("cli", "", SLATE, r#"{ "type": "narrate", "text": "ok" }"#);
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_delvec"))
        .arg("build")
        .arg(&dir)
        .arg("-o")
        .arg(dir.parent().unwrap().join("out"))
        .arg("--prefabs")
        .arg(&lib)
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{err}");
    assert!(
        err.lines()
            .any(|l| l.starts_with("sign binding: 0 sign(s), 0 face(s), 0 line(s)")),
        "{err}"
    );
}

/// `DW0431`: a row whose cell holds no sign, and a beat that letters a sign
/// before the write that places it.
#[test]
fn a_sign_where_none_stands_and_a_beat_before_it_are_dw0431() {
    let bare = SIGNS.replace("\"anchor/slate\", \"front\"", "\"anchor/bare\", \"front\"");
    let (dir, lib) = write(
        "bare",
        &bare,
        r#"{ "type": "narrate", "text": "ok" }"#,
        r#"{ "type": "narrate", "text": "ok" }"#,
    );
    let (code, msg) = refusal(&dir, &lib);
    assert_eq!(code, "DW0431", "{msg}");
    assert!(
        msg.contains("sign/slate") && msg.contains("anchor/board"),
        "{msg}"
    );
    let (dir, lib) = write("early", SIGNS, RELETTER, SLATE);
    let (code, msg) = refusal(&dir, &lib);
    assert_eq!(code, "DW0431", "{msg}");
    assert!(
        msg.contains("letter-sign") && msg.contains("set-block"),
        "{msg}"
    );
}

/// `DW1048` at the build: a template's sign is read off the assembled world,
/// so a line over its face is refused there, naming the sign and the kind.
#[test]
fn a_template_line_over_its_face_is_dw1048_at_the_build() {
    let wide = SIGNS.replace("mind the door", "MIND THE DOOR NOW");
    let (dir, lib) = write("wide", &wide, SLATE, RELETTER);
    let (code, msg) = refusal(&dir, &lib);
    assert_eq!(code, "DW1048", "{msg}");
    assert!(
        msg.contains("sign/board") && msg.contains("standing or wall sign"),
        "{msg}"
    );
    // The same line on a hanging template: the 60-pixel face refuses a line the
    // standing face would hold.
    let mid = SIGNS.replace("mind the door", "MIND THE DOOR");
    let (dir, lib) = write_with(
        "hanging",
        "minecraft:oak_wall_hanging_sign[facing=east]",
        &mid,
        SLATE,
        RELETTER,
    );
    let (code, msg) = refusal(&dir, &lib);
    assert_eq!(code, "DW1048", "{msg}");
    assert!(msg.contains("hanging sign"), "{msg}");
    let (dir, lib) = write("standing", &mid, SLATE, RELETTER);
    build(&dir, &lib);
}

/// `DW1048` at validation: a sidecar line, named with its language, and a face
/// with nothing visible — through the one check both tiers call.
#[test]
fn a_translation_over_its_face_and_a_blank_face_are_dw1048() {
    let (dir, _) = write("tr", SIGNS, SLATE, RELETTER);
    let mut c = english(&dir);
    c.world.content.languages = vec!["zh-cn".to_string()];
    let doc: delvewright_dsl::L10nDoc = serde_json::from_value(serde_json::json!({
        "dsl_version": DSL_VERSION, "campaign_id": "hello-world", "kind": "l10n", "lang": "zh-cn",
        "content": { "sign.post.front.0": "一二三四五六七八九十一" }
    }))
    .unwrap();
    let sidecars = BTreeMap::from([("zh-cn".to_string(), doc)]);
    let d = delvec::compiler::textfit::check_sign_lines(&c, &sidecars, &|id| {
        (id == "sign/post").then_some(delvewright_dsl::SignKind::Standing)
    });
    assert_eq!(d.len(), 1, "{d:#?}");
    assert_eq!(d[0].code, "DW1048");
    assert!(
        d[0].message.contains("(zh-cn)") && d[0].message.contains("99 font px"),
        "{}",
        d[0].message
    );
    // Ten Han fit the 90 px face.
    let ten: delvewright_dsl::L10nDoc = serde_json::from_value(serde_json::json!({
        "dsl_version": DSL_VERSION, "campaign_id": "hello-world", "kind": "l10n", "lang": "zh-cn",
        "content": { "sign.post.front.0": "一二三四五六七八九十" }
    }))
    .unwrap();
    let ok = BTreeMap::from([("zh-cn".to_string(), ten)]);
    assert!(
        delvec::compiler::textfit::check_sign_lines(&c, &ok, &|_| Some(
            delvewright_dsl::SignKind::Standing
        ))
        .is_empty()
    );
    // A face that shows nothing.
    c.quests.content.signs[1].front.lines = vec![String::new(), " ".to_string()];
    let d = delvec::compiler::textfit::check_sign_lines(&c, &BTreeMap::new(), &|_| None);
    assert!(
        d.iter()
            .any(|x| x.code == "DW1048" && x.message.contains("none of them shows a glyph")),
        "{d:#?}"
    );
}

/// Criterion 7: the written world lays the compound of every lettered sign
/// standing in it — the template's and the prop's at load — after the latest
/// `letter-sign` fired before the arrival, and a lettered cell is not counted
/// as drawn blank.
#[test]
fn the_written_world_carries_the_words() {
    use delvec::compiler::view::world::{BlockEntities, Nbt, block_entities_omitted};
    let (dir, lib) = write("world", SIGNS, SLATE, RELETTER);
    let c = english(&dir);
    let prefabs = PrefabRegistry::load_dir(&lib).unwrap();
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let s = structures(&plan, &lib);
    let assembled = delvec::compiler::assembled::assemble(&plan, &s);
    let base = delvec::compiler::view::beat::picture_base(&plan, &assembled, &[]);
    let load = delvec::compiler::view::beat::load_entities(&plan, &base);
    let at = |e: &BlockEntities, id: &str| -> Vec<[i32; 3]> {
        e.iter()
            .filter(|(_, b)| b.id == id)
            .map(|(c, _)| *c)
            .collect()
    };
    let board = plan
        .signs
        .iter()
        .find(|s| s.id == "sign/board")
        .unwrap()
        .cell;
    let post = plan
        .signs
        .iter()
        .find(|s| s.id == "sign/post")
        .unwrap()
        .cell;
    let slate = plan
        .signs
        .iter()
        .find(|s| s.id == "sign/slate")
        .unwrap()
        .cell;
    let mut stood = at(&load, "minecraft:sign");
    stood.sort();
    let mut want = vec![board, post];
    want.sort();
    assert_eq!(stood, want, "{load:#?}");
    assert_eq!(block_entities_omitted(&base, &load), 0);
    assert_eq!(block_entities_omitted(&base, &BlockEntities::new()), 2);
    let face = |e: &BlockEntities, cell: [i32; 3], key: &str| -> Nbt {
        e[&cell]
            .data
            .iter()
            .find(|(k, _)| k == key)
            .unwrap()
            .1
            .clone()
    };
    let Nbt::Compound(front) = face(&load, board, "front_text") else {
        panic!()
    };
    assert!(
        front
            .iter()
            .any(|(k, v)| k == "messages" && matches!(v, Nbt::List(l) if l.len() == 4))
    );
    // The set-block's sign, standing in a configuration after the beat that
    // writes it: lettered as declared before the re-lettering beat, and as the
    // `letter-sign` states after it.
    let mut with_slate = base.clone();
    with_slate.insert(
        slate,
        delvec::compiler::blockstate::BlockState::new(BOARD_BLOCK),
    );
    let steps = &plan.critical_path;
    let read = steps
        .iter()
        .position(|s| s.objective() == Some("obj/read"))
        .unwrap();
    let before = delvec::compiler::sign::entities_at(&plan, &with_slate, steps, read);
    let later = delvec::compiler::sign::entities_at(&plan, &with_slate, steps, read + 1);
    let text = |e: &BlockEntities| format!("{:?}", face(e, slate, "front_text"));
    assert!(
        text(&before).contains("sign.slate.front.0"),
        "{}",
        text(&before)
    );
    assert!(
        text(&later).contains(".front.1") && text(&later).contains("green"),
        "{}",
        text(&later)
    );
}

/// Two builds of the same campaign are byte-identical (ADR-0006).
#[test]
fn two_builds_are_byte_identical() {
    let (dir, lib) = write("twice", SIGNS, SLATE, RELETTER);
    assert_eq!(build(&dir, &lib), build(&dir, &lib));
}
