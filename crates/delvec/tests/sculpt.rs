//! **`delvec sculpt`** (spec-0087): a prefab from a declared form — its surface,
//! its determinism, its gates, its refusals and its pocket proof.
//!
//! The primary is the gallery's form (`gallery/forms/gallery-carcass.json`);
//! every refusal is that form plus one declared edit, the way the gallery's own
//! probes are written.

mod common;

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use common::sculpt::{Run, apply, fixture_form, gallery_form, gallery_form_path, probe_patch};
use delvec::sculpt::{self, Form, SculptError};

fn delvec(args: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_delvec"))
        .args(args)
        .output()
        .expect("delvec runs");
    (
        o.status.code().unwrap_or(-1),
        format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        ),
    )
}

fn form_of(v: &serde_json::Value) -> Form {
    serde_json::from_value(v.clone()).expect("the form parses")
}

/// A refusal of the document arm: exit 1, `DW0951` naming `needle`, and no
/// output directory — nothing was fitted.
fn assert_form_refused(run: &Run, needle: &str) {
    assert_eq!(run.code, 1, "{}", run.said);
    assert!(
        run.said
            .lines()
            .any(|l| l.starts_with("DW0951 [error]") && l.contains(needle)),
        "the refusal names {needle:?}: {}",
        run.said
    );
    assert!(
        !run.out.exists(),
        "a refused form writes nothing, and `{}` exists",
        run.out.display()
    );
    assert!(
        !run.said.contains("pockets:"),
        "the document arm refuses before anything is fitted: {}",
        run.said
    );
}

// ---------------------------------------------------------------------------
// 1. Surface
// ---------------------------------------------------------------------------

/// Every property name a JSON Schema declares, at any depth.
fn schema_properties(v: &serde_json::Value, out: &mut BTreeSet<String>) {
    match v {
        serde_json::Value::Object(m) => {
            if let Some(serde_json::Value::Object(props)) = m.get("properties") {
                for (k, sub) in props {
                    out.insert(k.clone());
                    schema_properties(sub, out);
                }
            }
            for (k, sub) in m {
                if k != "properties" {
                    schema_properties(sub, out);
                }
            }
        }
        serde_json::Value::Array(a) => a.iter().for_each(|x| schema_properties(x, out)),
        _ => {}
    }
}

#[test]
fn the_form_schema_is_its_own_export_and_stays_out_of_all() {
    let (code, said) = delvec(&["schema", "--stage", "sculpt-form"]);
    assert_eq!(code, 0, "{said}");
    let schema: serde_json::Value = serde_json::from_str(&said).expect("one JSON document");
    assert_eq!(schema["title"], "sculpt form");
    assert_eq!(schema["x-delvewright-file"], "forms/*.json");
    let mut props = BTreeSet::new();
    schema_properties(&schema, &mut props);
    for field in [
        "box", "ground", "sink", "solids", "palette", "lights", "anchors", "sub",
    ] {
        assert!(props.contains(field), "the export declares `{field}`");
    }

    let (code, all) = delvec(&["schema", "--stage", "all"]);
    assert_eq!(code, 0);
    let all: serde_json::Value = serde_json::from_str(&all).unwrap();
    assert!(all.get("sculpt-form").is_none());
    let mut every = BTreeSet::new();
    schema_properties(&all, &mut every);
    assert!(
        !every.contains("form_version") && !every.contains("radius_from"),
        "`--stage all` carries no form unit"
    );
}

/// **`compiler.md`'s field list for the form is the form's**, in both
/// directions: every property the schema export declares is a row of the
/// table, and every row is a property.
#[test]
fn compiler_md_lists_the_form_fields_the_struct_has() {
    let mut props = BTreeSet::new();
    schema_properties(&sculpt::form::schema(), &mut props);
    let md = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/reference/compiler.md"),
    )
    .unwrap();
    let head = "**The form document's fields**";
    let tail = "**What the form arm refuses**";
    let from = md.find(head).expect("the form's field table");
    let to = md[from..].find(tail).expect("the table's end") + from;
    let rows: BTreeSet<String> = md[from..to]
        .lines()
        .filter(|l| l.starts_with("| `"))
        .map(|l| l[3..].split('`').next().unwrap().to_string())
        .collect();
    let missing: Vec<&String> = props.difference(&rows).collect();
    let extra: Vec<&String> = rows.difference(&props).collect();
    assert!(missing.is_empty(), "compiler.md lacks rows for {missing:?}");
    assert!(
        extra.is_empty(),
        "compiler.md has rows for non-fields {extra:?}"
    );
    eprintln!(
        "form field binding: {} schema propert(ies), {} documented row(s)",
        props.len(),
        rows.len()
    );
    assert!(props.len() >= 30, "the walk found the nested fields too");
}

// ---------------------------------------------------------------------------
// 2. Determinism
// ---------------------------------------------------------------------------

#[test]
fn the_gallery_form_sculpts_to_the_same_bytes_and_its_provenance_reproduces_them() {
    let form = form_of(&gallery_form());
    let a = sculpt::sculpt(&form, 7, None).expect("the gallery form sculpts");
    let b = sculpt::sculpt(&form, 7, None).expect("and again");
    assert_eq!(a.files, b.files, "byte-identical parts");
    assert_eq!(a.metadata_json, b.metadata_json, "byte-identical metadata");
    let other = sculpt::sculpt(&form, 8, None).unwrap();
    assert_ne!(a.files, other.files, "the seed reaches the bytes");

    let by = a
        .metadata
        .license
        .as_ref()
        .and_then(|l| l.generated_by.clone())
        .expect("generated_by");
    assert_eq!(by.generator, "sculpt");
    assert_eq!(by.program, form.id);
    assert_eq!(by.program_hash, sculpt::form::form_hash(&form));
    assert_eq!(by.seed, 7);
    assert_eq!(by.region, [30, 20, 44]);

    // From the row alone: find the form it names, check its hash, re-sculpt.
    let found: Vec<Form> = std::fs::read_dir(gallery_form_path().parent().unwrap())
        .unwrap()
        .filter_map(|e| {
            let p = e.ok()?.path();
            (p.extension()? == "json").then(|| {
                serde_json::from_str::<Form>(&std::fs::read_to_string(p).unwrap()).unwrap()
            })
        })
        .filter(|f| f.id == by.program && sculpt::form::form_hash(f) == by.program_hash)
        .collect();
    assert_eq!(found.len(), 1, "the row names exactly one committed form");
    let again = sculpt::sculpt(&found[0], by.seed, None).unwrap();
    assert_eq!(again.files, a.files);
    assert_eq!(again.metadata_json, a.metadata_json);
}

#[test]
fn the_command_writes_the_same_files_twice() {
    let form = gallery_form();
    let one = common::sculpt::sculpt(&form, "twice-a", &["--seed", "3"]);
    let two = common::sculpt::sculpt(&form, "twice-b", &["--seed", "3"]);
    assert_eq!(one.code, 0, "{}", one.said);
    assert_eq!(two.code, 0, "{}", two.said);
    for name in ["gallery-carcass.nbt", "gallery-carcass.json"] {
        assert_eq!(
            std::fs::read(one.out.join(name)).unwrap(),
            std::fs::read(two.out.join(name)).unwrap(),
            "{name} is byte-identical across runs"
        );
    }
}

// ---------------------------------------------------------------------------
// 3. One byte boundary, one stair rule
// ---------------------------------------------------------------------------

fn sculpt_sources() -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/sculpt");
    let mut out: Vec<(String, String)> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| {
            let p = e.unwrap().path();
            (
                p.file_name().unwrap().to_string_lossy().to_string(),
                std::fs::read_to_string(&p).unwrap(),
            )
        })
        .collect();
    out.sort();
    out
}

#[test]
fn the_sculpt_module_writes_through_one_boundary_and_derives_corners_one_way() {
    let sources = sculpt_sources();
    assert!(sources.len() >= 5, "the module's files: {sources:?}");
    for (name, text) in &sources {
        for forbidden in [
            "parse_schematic",
            "schem::schematic",
            "fastnbt",
            "fn derive_shape",
            "\"inner_left\"",
            "\"outer_right\"",
            "GzBuilder",
        ] {
            assert!(
                !text.contains(forbidden),
                "src/sculpt/{name} carries `{forbidden}`: a second byte boundary or a second \
                 stair-shape derivation"
            );
        }
    }
    let all: String = sources.iter().map(|(_, t)| t.as_str()).collect();
    assert!(
        all.contains("export::freeze_model("),
        "parts go through the export writer"
    );
    assert!(
        all.contains("derive_shape("),
        "corners come from the one rule"
    );
    let export = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/grammar/export.rs"),
    )
    .unwrap();
    assert!(export.contains("convert::build_region(&schem"));
}

#[test]
fn the_gallery_form_passes_the_audit_with_every_stair_examined() {
    let form = form_of(&gallery_form());
    let s = sculpt::sculpt(&form, 0, None).unwrap();
    let run = common::sculpt::sculpt(&gallery_form(), "audit", &[]);
    assert_eq!(run.code, 0, "{}", run.said);
    let (code, said) = delvec(&[
        "prefab",
        "audit",
        run.out.join("gallery-carcass.nbt").to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{said}");
    let start = said.find("{\n").expect("the audit's JSON report");
    let report: serde_json::Value = serde_json::Deserializer::from_str(&said[start..])
        .into_iter::<serde_json::Value>()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(report["verdict"], "pass");
    assert_eq!(report["underspecified"], 0);
    assert_eq!(
        report["stairs_examined"], s.readings.stairs,
        "examined = written"
    );
    assert!(s.readings.stairs > 0);
}

// ---------------------------------------------------------------------------
// 4. The grammar's gates run here
// ---------------------------------------------------------------------------

#[test]
fn the_grammar_gates_run_over_every_sculpt_with_their_bindings() {
    let s = sculpt::sculpt(&form_of(&gallery_form()), 0, None).unwrap();
    for id in [
        "blocks-exist",
        "states-complete",
        "stair-shape",
        "non-empty",
    ] {
        let g = s
            .gates
            .iter()
            .find(|g| g.id == id)
            .unwrap_or_else(|| panic!("gate `{id}` ran"));
        assert!(g.passed() && g.bound > 0, "{g:?}");
    }
    let src =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/sculpt/mod.rs"))
            .unwrap();
    for call in [
        "gates::gate_blocks_exist(",
        "gates::gate_states_complete(",
        "gates::gate_stair_shape(",
        "gates::gate_non_empty(",
        "gates::seal_zero_bindings(",
    ] {
        assert!(src.contains(call), "the sculpt calls `{call}`");
    }
}

#[test]
fn a_misspelt_palette_block_is_refused_by_the_blocks_exist_gate() {
    let form = apply(
        gallery_form(),
        &[
            serde_json::json!({"op": "replace", "path": "/palette/1/full/0/0", "value": "minecraft:dead_horn_coral_blok"}),
        ],
    );
    let run = common::sculpt::sculpt(&form, "misspelt", &[]);
    assert_form_refused(&run, "gate `blocks-exist`");
}

// ---------------------------------------------------------------------------
// 5. The document arm
// ---------------------------------------------------------------------------

#[test]
fn a_form_with_no_ground_is_refused_before_anything_is_fitted() {
    let run = common::sculpt::sculpt(
        &apply(gallery_form(), &probe_patch("a-body-with-no-ground")),
        "no-ground",
        &[],
    );
    assert_form_refused(&run, "declares no `ground`");
}

#[test]
fn a_form_with_no_entry_anchor_is_refused() {
    let run = common::sculpt::sculpt(
        &apply(
            gallery_form(),
            &[serde_json::json!({"op": "remove", "path": "/anchors/anchor~1entry/role"})],
        ),
        "no-entry",
        &[],
    );
    assert_form_refused(&run, "`role: entry`");
}

#[test]
fn a_family_without_both_shapes_is_refused() {
    let run = common::sculpt::sculpt(
        &apply(
            gallery_form(),
            &[
                serde_json::json!({"op": "replace", "path": "/palette/0/family", "value": "calcite"}),
            ],
        ),
        "no-family",
        &[],
    );
    assert_form_refused(&run, "family \"calcite\" has no stair");
}

#[test]
fn a_glowing_palette_tone_or_ground_is_refused_by_the_relight_table() {
    for (tag, path, block) in [
        ("glowstone", "/palette/0/full/0/0", "minecraft:glowstone"),
        (
            "sea-lantern",
            "/palette/3/full/0/0",
            "minecraft:sea_lantern",
        ),
        ("glowing-ground", "/ground/block", "minecraft:shroomlight"),
        (
            "glowing-shelf",
            "/solids/6/material/full/0/0",
            "minecraft:ochre_froglight[axis=y]",
        ),
    ] {
        assert!(
            delvec::compiler::light::emission(block) > 0,
            "{block} emits, by the table the refusal reads"
        );
        let run = common::sculpt::sculpt(
            &apply(
                gallery_form(),
                &[serde_json::json!({"op": "replace", "path": path, "value": block})],
            ),
            tag,
            &[],
        );
        assert_form_refused(&run, "emits light");
    }
}

#[test]
fn a_shelf_steeper_than_one_block_per_block_is_refused() {
    let run = common::sculpt::sculpt(
        &apply(
            gallery_form(),
            &[
                serde_json::json!({"op": "replace", "path": "/solids/6/path/1", "value": [24.5, 16.0, 32.0]}),
            ],
        ),
        "steep-shelf",
        &[],
    );
    assert_form_refused(&run, "steeper than one block per block");
}

#[test]
fn a_light_that_emits_nothing_is_refused() {
    let run = common::sculpt::sculpt(
        &apply(
            gallery_form(),
            &[
                serde_json::json!({"op": "replace", "path": "/lights/0/block", "value": "minecraft:stone"}),
            ],
        ),
        "dark-light",
        &[],
    );
    assert_form_refused(&run, "emits no light");
}

// ---------------------------------------------------------------------------
// 6. Ground, entry, planes, faces
// ---------------------------------------------------------------------------

#[test]
fn the_walk_plane_is_the_ground_and_the_planes_instrument_agrees() {
    let form = form_of(&gallery_form());
    let s = sculpt::sculpt(&form, 0, None).unwrap();
    let top = form.ground.as_ref().unwrap().top as i32;
    assert_eq!(s.metadata.walk_y, Some(top + 1));
    let run = common::sculpt::sculpt(&gallery_form(), "planes", &[]);
    assert_eq!(run.code, 0, "{}", run.said);
    let (code, said) = delvec(&[
        "prefab",
        "planes",
        run.out.join("gallery-carcass.nbt").to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{said}");
    assert!(
        said.contains(&format!("\"walk_y\": {}", top + 1)),
        "`delvec prefab planes` measures the walk plane written: {said}"
    );
}

#[test]
fn shown_faces_are_the_sides_the_bytes_put_blocks_on() {
    use delvec::schem::nav::Voxels as _;
    let s = sculpt::sculpt(&form_of(&gallery_form()), 0, None).unwrap();
    let m = &s.model;
    let max = m.maximum();
    let mut sides = Vec::new();
    for (name, axis, at) in [
        ("down", 1usize, 0),
        ("east", 0, max[0] - 1),
        ("north", 2, 0),
        ("south", 2, max[2] - 1),
        ("up", 1, max[1] - 1),
        ("west", 0, 0),
    ] {
        let any = m
            .region()
            .positions()
            .filter(|p| p[axis] == at)
            .any(|p| m.get(p).is_some_and(|b| !b.is_air()));
        if any {
            sides.push(name.to_string());
        }
    }
    assert_eq!(s.metadata.shown_faces, sides);
    for side in ["north", "south", "east", "west"] {
        assert!(sides.iter().any(|s| s == side), "the apron reaches {side}");
    }
}

#[test]
fn the_entry_is_in_the_walk_from_grade() {
    let form = form_of(&gallery_form());
    let s = sculpt::sculpt(&form, 0, None).unwrap();
    let standable = delvec::schem::nav::standable_cells(&s.model);
    let grade = delvec::schem::nav::ground_entry(&s.model);
    let walked = delvec::schem::nav::reachable_from(&s.model, &standable, &grade);
    let entry = form
        .anchors
        .values()
        .find(|a| a.role == Some(delvewright_dsl::prefab::AnchorRole::Entry))
        .unwrap();
    assert!(!grade.is_empty());
    assert!(walked.contains(&entry.pos), "{:?}", entry.pos);
    assert_eq!(s.readings.entry.anchors_walked, s.readings.entry.anchors);
}

#[test]
fn a_buried_entry_is_refused_naming_its_cell() {
    let run = common::sculpt::sculpt(
        &apply(
            gallery_form(),
            &[
                serde_json::json!({"op": "replace", "path": "/anchors/anchor~1entry/pos", "value": [15, 9, 30]}),
            ],
        ),
        "buried-entry",
        &[],
    );
    assert_eq!(run.code, 3, "{}", run.said);
    assert!(
        run.said.lines().any(|l| l.starts_with("DW0952 [error]")
            && l.contains("\"anchor/entry\" at [15, 9, 30] is not a cell a body can stand in")),
        "{}",
        run.said
    );
    assert!(!run.out.exists());
}

// ---------------------------------------------------------------------------
// 7. Pockets
// ---------------------------------------------------------------------------

/// The callers of the leave relation, enumerated: the pocket proof's flood is
/// `World::trapped_places`, whose only callers are `DW0921`'s judgement and the
/// sculpt — one function, never a second implementation.
#[test]
fn the_sculpt_judges_pockets_with_the_relation_dw0921_floods() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut callers: Vec<String> = Vec::new();
    let mut flooders: Vec<String> = Vec::new();
    let mut stack = vec![src.clone()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if p.extension().is_none_or(|x| x != "rs") {
                continue;
            }
            let text = std::fs::read_to_string(&p).unwrap();
            let rel = p.strip_prefix(&src).unwrap().to_string_lossy().to_string();
            for line in text.lines() {
                let l = line.trim_start();
                if l.starts_with("//") {
                    continue;
                }
                if l.contains(".trapped_places(") {
                    callers.push(rel.clone());
                }
                if l.contains(".cells_a_body_cannot_leave(") {
                    flooders.push(rel.clone());
                }
                if rel.starts_with("sculpt") && l.contains("body_moves(") {
                    panic!("src/{rel} floods body_moves itself: {line}");
                }
            }
        }
    }
    callers.sort();
    flooders.sort();
    assert_eq!(
        callers,
        vec!["compiler/nav.rs".to_string(), "sculpt/mod.rs".to_string()],
        "the leave judgement's callers"
    );
    assert_eq!(flooders, vec!["compiler/nav.rs".to_string()]);
}

#[test]
fn every_run_prints_the_pocket_line() {
    let run = common::sculpt::sculpt(&gallery_form(), "pocket-line", &[]);
    assert_eq!(run.code, 0, "{}", run.said);
    assert_eq!(run.pocket_places(), Some(0), "{}", run.said);
    assert!(run.said.contains("pockets: 0 place(s), 0 cell(s), of "));
    assert!(run.said.contains("a body can reach from 4 anchor(s)"));
}

#[test]
fn a_pit_in_the_back_is_a_pocket_refused_at_generation() {
    let run = common::sculpt::sculpt(
        &apply(gallery_form(), &probe_patch("a-body-with-a-pocket")),
        "pocket",
        &[],
    );
    assert_eq!(run.code, 3, "{}", run.said);
    assert!(run.pocket_places().is_some_and(|p| p >= 1), "{}", run.said);
    assert!(
        run.said.lines().any(|l| l.starts_with("DW0952 [error]")
            && l.contains("place(s) a body can get into and not out of")
            && l.contains("a body gets in from")),
        "{}",
        run.said
    );
    assert!(!run.out.exists());
}

/// The rig that measured spec-0087 §2.2, as a form: refused, with the number of
/// places written down in the commit that landed this test.
#[test]
fn the_stranded_rig_s_own_form_is_refused_with_its_pockets() {
    let run = common::sculpt::sculpt_file(
        &fixture_form("stranded-rig.json"),
        &Path::new(env!("CARGO_TARGET_TMPDIR")).join("sculpt-rig-out"),
        &["--seed", "1"],
    );
    assert_eq!(run.code, 3, "{}", run.said);
    let p = run.pocket_places().expect("the pocket line");
    eprintln!("stranded rig: {p} pocket place(s)");
    assert!(p > 0, "{}", run.said);
}

// ---------------------------------------------------------------------------
// 8. Light
// ---------------------------------------------------------------------------

#[test]
fn the_light_probe_binds_the_sculpted_body_and_the_metadata_carries_its_measurement() {
    let run = common::sculpt::sculpt(&gallery_form(), "lighting", &[]);
    assert_eq!(run.code, 0, "{}", run.said);
    let (code, said) = delvec(&[
        "prefab",
        "lighting",
        run.out.join("gallery-carcass.nbt").to_str().unwrap(),
    ]);
    assert!(code == 0 || code == 1, "{said}");
    let start = said.find('{').unwrap();
    let probe: serde_json::Value = serde_json::Deserializer::from_str(&said[start..])
        .into_iter::<serde_json::Value>()
        .next()
        .unwrap()
        .unwrap();
    let entry = probe["binding"]["entry_cells"].as_u64().unwrap_or_default();
    let measured = probe["binding"]["measured_cells"]
        .as_u64()
        .unwrap_or_default();
    assert!(entry > 0 && measured > 0, "{probe}");
    let meta: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(run.out.join("gallery-carcass.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        meta["lighting"]["profile"], probe["profile"],
        "the profile written is the one the probe measures"
    );
}

/// A one-area campaign around a sculpted piece, on the base that builds ground
/// around it (valley), at night: the inside and the back are reach objectives.
fn carcass_campaign(tag: &str, form: &serde_json::Value) -> (i32, String) {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("sculpt-camp-{tag}"));
    let _ = std::fs::remove_dir_all(&root);
    let prefabs = root.join("prefabs");
    let camp = root.join("campaign");
    std::fs::create_dir_all(&camp).unwrap();
    let form_path = root.join("form.json");
    std::fs::write(&form_path, serde_json::to_string_pretty(form).unwrap()).unwrap();
    let run = common::sculpt::sculpt_file(&form_path, &prefabs, &[]);
    assert_eq!(run.code, 0, "{}", run.said);
    let ver = delvewright_dsl::DSL_VERSION;
    let doc = |stage: &str, content: serde_json::Value| {
        serde_json::to_string_pretty(&serde_json::json!({
            "campaign_id": "sculpt-test", "content": content, "dsl_version": ver, "stage": stage
        }))
        .unwrap()
    };
    let reach = |id: &str, anchor: &str, after: Option<&str>| {
        let mut o = serde_json::json!({
            "anchor": anchor, "happening": {"text": format!("they reach {anchor}"), "verb": "arrives"},
            "id": id, "radius": 2, "type": "reach-anchor"
        });
        if let Some(a) = after {
            o["after"] = serde_json::json!([a]);
        }
        o
    };
    let files = [
        (
            "world.json",
            doc(
                "world",
                serde_json::json!({
                    "areas": [{"id": "area/carcass", "name": "The Carcass", "prefab": "prefab/gallery-carcass"}],
                    "boundary": {"margin": 16, "message": "The valley ends here."},
                    "difficulty": "normal",
                    "horizon": {"base": "valley", "ratio": 2.5, "rim_height": 48},
                    "min_players": 1,
                    "outro": "Walked.", "premise": "A sculpted body on its own ground.", "seed": 1,
                    "target_minutes": 5, "theme": "A test.", "time": "night", "title": "Sculpt Test",
                    "weather": "clear"
                }),
            ),
        ),
        ("npcs.json", doc("npcs", serde_json::json!({"npcs": []}))),
        (
            "classes.json",
            doc("classes", serde_json::json!({"classes": []})),
        ),
        (
            "dialogue.json",
            doc("dialogue", serde_json::json!({"dialogues": []})),
        ),
        (
            "quest-plan.json",
            doc(
                "quest-plan",
                serde_json::json!({"finale": "quest/walk", "quests": [{
                    "act": 1, "area": "area/carcass", "depends_on": [], "goal": "Walk it.",
                    "id": "quest/walk", "mandatory": true, "npcs": []
                }]}),
            ),
        ),
        (
            "quests.json",
            doc(
                "quests",
                serde_json::json!({"quests": [{
                    "happening": {"text": "they take on quest/walk", "verb": "learns"},
                    "id": "quest/walk",
                    "objectives": [reach("obj/inside", "anchor/inside", None),
                                   reach("obj/back", "anchor/back", Some("obj/inside"))],
                    "on_complete": [{"happening": {"text": "done", "verb": "survives"}, "type": "campaign-complete"}],
                    "trigger": {"type": "campaign-start"}
                }]}),
            ),
        ),
    ];
    for (name, text) in files {
        std::fs::write(camp.join(name), text).unwrap();
    }
    delvec(&[
        "--prefabs",
        prefabs.to_str().unwrap(),
        "build",
        camp.to_str().unwrap(),
        "-o",
        root.join("out").to_str().unwrap(),
    ])
}

#[test]
fn the_body_builds_lit_by_its_own_lights_and_dark_without_them() {
    let (code, said) = carcass_campaign("lit", &gallery_form());
    assert_eq!(code, 0, "{said}");
    assert!(
        said.contains("DW0921 binding"),
        "the leave proof ran: {said}"
    );

    let dark = apply(
        gallery_form(),
        &[serde_json::json!({"op": "remove", "path": "/lights"})],
    );
    let (code, said) = carcass_campaign("dark", &dark);
    assert_eq!(code, 2, "{said}");
    assert!(said.contains("DW0210 [error]"), "{said}");
}

#[test]
fn without_its_shelf_the_back_is_not_walked() {
    let mut form = gallery_form();
    let solids = form["solids"].as_array_mut().unwrap();
    solids.retain(|s| s["shape"] != "shelf");
    let (code, said) = carcass_campaign("no-shelf", &form);
    assert_eq!(code, 3, "{said}");
    assert!(said.contains("DW0311 [error]"), "{said}");
}

// ---------------------------------------------------------------------------
// 9. Scale
// ---------------------------------------------------------------------------

#[test]
fn a_box_past_the_schematic_cap_is_written_as_parts() {
    let form: Form =
        serde_json::from_str(&std::fs::read_to_string(fixture_form("scale.json")).unwrap())
            .unwrap();
    let cells: u64 = form.extent.iter().map(|v| *v as u64).product();
    assert!(cells > 10_000_000, "{cells}");
    assert_eq!(form.sub, 2);
    let run = common::sculpt::sculpt_file(
        &fixture_form("scale.json"),
        &Path::new(env!("CARGO_TARGET_TMPDIR")).join("sculpt-scale-out"),
        &[],
    );
    assert_eq!(run.code, 0, "{}", run.said);
    assert!(!run.said.contains("DW0710"), "{}", run.said);
    let parts = std::fs::read_dir(&run.out)
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .path()
                .extension()
                .is_some_and(|x| x == "nbt")
        })
        .count();
    assert!(parts > 1, "{parts}");
    assert!(
        run.said.contains(&format!(
            "{parts} structure file(s) — {cells} cell(s), {parts} part(s)"
        )),
        "the command prints cells and parts: {}",
        run.said
    );
}

// ---------------------------------------------------------------------------
// The error type the library returns
// ---------------------------------------------------------------------------

#[test]
fn the_library_names_the_code_of_each_refusal() {
    let no_ground = form_of(&apply(
        gallery_form(),
        &probe_patch("a-body-with-no-ground"),
    ));
    match sculpt::sculpt(&no_ground, 0, None) {
        Err(e @ SculptError::Form(_)) => assert_eq!(e.code().unwrap().id(), "DW0951"),
        other => panic!("{:?}", other.map(|s| s.id)),
    }
    let pocket = form_of(&apply(gallery_form(), &probe_patch("a-body-with-a-pocket")));
    match sculpt::sculpt(&pocket, 0, None) {
        Err(e @ SculptError::Body(..)) => assert_eq!(e.code().unwrap().id(), "DW0952"),
        other => panic!("{:?}", other.map(|s| s.id)),
    }
}
