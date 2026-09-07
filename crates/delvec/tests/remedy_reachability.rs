//! **Every remedy a diagnostic names is reachable** (spec-0060 §10.3).
//!
//! `CLAUDE.md`: *a gate that names a remedy owes a check that the remedy is
//! reachable*, and *a remedy one gate prescribes and another refuses is the
//! pair's defect*. Nothing in this engine held that check, which is why the
//! cycle spec-0060 §1 describes was found by walking it rather than by a red:
//! `ocean` refused a piece and named `void`, `void` refused it and named
//! `valley`, `valley` refused the campaign and named a site plan, and adding a
//! site plan to an `areas[]` campaign is two placement authorities.
//!
//! So this file is the check. Each case is one move a diagnostic's own message
//! prescribes: the world that meets the refusal, the single edit the message
//! tells the author to make, and the assertion that the edit reaches a
//! **different verdict**. A case that reddens the same way after the move is a
//! message sending its reader nowhere.
//!
//! `tools/check-dw-codes.py` holds the other direction: a diagnostic whose
//! message names a base or a document as a move owes a row here, so a code added
//! later cannot prescribe a remedy nobody ever took.
//!
//! # What is not here, and why
//!
//! `DW0885`'s **BURY** and **PLACE** moves. Both are properties of a world with
//! more than one thing in it: burial needs a base that builds terrain, which
//! needs a site plan (`DW0855`), and placing something against a face needs a
//! second placed piece the solver mated. Both are taken, and green, on the
//! gallery's site-plan point — 11 placed pieces, of which 1 is judged and
//! answers for its one exposed side, the other 10 buried by the surround and by
//! each other. That is a whole-campaign build, so it is asserted where it is
//! built (the gallery job's `validation/piece-exposure.json`) rather than
//! duplicated here at fifteen times the cost. Recorded as a debt of THIS file,
//! not as a discharge of the criterion.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command as Proc, Output};

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildFailure, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::parse_campaign;

mod common;

// ---------------------------------------------------------------------------
// Running a whole campaign through the binary
// ---------------------------------------------------------------------------

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("remedy-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn delvec(args: &[&str]) -> Output {
    Proc::new(env!("CARGO_BIN_EXE_delvec"))
        .args(args)
        .output()
        .expect("delvec runs")
}

fn log(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

/// A hello-world campaign with `horizon` set to `value` (absent = `void`).
fn campaign(tag: &str, horizon: Option<serde_json::Value>) -> PathBuf {
    let camp = tmp(&format!("camp-{tag}"));
    common::copy_dir_all(&common::hello_world_dir(), &camp);
    let mut world: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(camp.join("world.json")).unwrap()).unwrap();
    world["dsl_version"] = serde_json::json!("0.21.1");
    if let Some(h) = horizon {
        let content = world["content"].as_object_mut().unwrap();
        content.insert("horizon".into(), h);
        content.insert("boundary".into(), serde_json::json!({ "margin": 20 }));
    }
    std::fs::write(
        camp.join("world.json"),
        serde_json::to_string_pretty(&world).unwrap(),
    )
    .unwrap();
    camp
}

/// `delvec build`, as an exit code and everything it said.
fn build(tag: &str, camp: &Path, prefabs: &Path) -> (i32, String) {
    let out = tmp(&format!("out-{tag}"));
    let r = delvec(&[
        "build",
        camp.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        prefabs.to_str().unwrap(),
    ]);
    (r.status.code().unwrap_or(-1), log(&r))
}

/// Edit one prefab document in a library copy.
fn edit_meta(
    dir: &Path,
    id: &str,
    f: impl FnOnce(&mut serde_json::Map<String, serde_json::Value>),
) {
    let path = dir.join(format!("{id}.json"));
    let mut doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    f(doc.as_object_mut().unwrap());
    std::fs::write(&path, serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();
}

// ---------------------------------------------------------------------------
// DW0886 — the four shapes, and the move each one names
// ---------------------------------------------------------------------------

/// **DECLARE `walk_y`.** The move `DW0886` names for a piece that states no walk
/// plane at all, and the only one it names: there is deliberately no default.
#[test]
fn dw0886_declaring_the_walk_plane_seats_the_piece() {
    let dir = common::ocean_prefabs_dir("remedy-walk", common::OceanRoom::Shore);
    let camp = campaign("walk", Some(serde_json::json!("ocean")));

    // The refusal: the field is gone, which is exactly what an admission step
    // that did not model it left behind.
    let declared = {
        let path = dir.join("hello-room.json");
        let doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        doc["walk_y"].clone()
    };
    edit_meta(&dir, "hello-room", |m| {
        m.remove("walk_y");
    });
    let (code, before) = build("walk-red", &camp, &dir);
    assert_eq!(code, 1, "refused at validation:\n{before}");
    assert!(before.contains("DW0886"), "{before}");
    assert!(
        before.contains("DECLARE `walk_y`"),
        "the message names the move:\n{before}"
    );

    // The move, taken.
    edit_meta(&dir, "hello-room", |m| {
        m.insert("walk_y".into(), declared.clone());
    });
    let (code, after) = build("walk-green", &camp, &dir);
    assert_eq!(code, 0, "the move reaches a different verdict:\n{after}");
    assert!(!after.contains("DW0886"), "{after}");
}

/// **DECLARE `waterline_y: <n>`**, where `<n>` is the number the message reads
/// out of the piece's own bytes. A move that named no number would be a move an
/// author has to guess at.
#[test]
fn dw0886_declaring_the_waterline_the_bytes_hold_seats_the_shore() {
    let dir = common::ocean_prefabs_dir("remedy-shore", common::OceanRoom::Shore);
    let camp = campaign("shore", Some(serde_json::json!("ocean")));

    edit_meta(&dir, "hello-room", |m| {
        m.remove("waterline_y");
    });
    let (code, before) = build("shore-red", &camp, &dir);
    assert_eq!(code, 1, "refused at validation:\n{before}");
    assert!(before.contains("DW0886"), "{before}");
    assert!(
        before.contains("DECLARE `waterline_y: 2`"),
        "the move names the number the bytes hold:\n{before}"
    );

    edit_meta(&dir, "hello-room", |m| {
        m.insert("waterline_y".into(), serde_json::json!(2));
    });
    let (code, after) = build("shore-green", &camp, &dir);
    assert_eq!(code, 0, "the move reaches a different verdict:\n{after}");
}

/// **DECLARE the walk plane the piece really has.** The second move `DW0886`
/// names for a piece that stands a body under its own declared plane — and the
/// one an author can take without touching the bytes.
#[test]
fn dw0886_declaring_the_true_walk_plane_lifts_the_piece_clear_of_the_sea() {
    let dir = common::ocean_prefabs_dir("remedy-wade", common::OceanRoom::Shore);
    let camp = campaign("wade", Some(serde_json::json!("ocean")));

    // A plane two courses above the floor the piece actually has: the body
    // stands at local y=3, the declaration says 5, so an ocean seats the area
    // two blocks lower and puts that floor under the sea.
    edit_meta(&dir, "hello-room", |m| {
        m.insert("walk_y".into(), serde_json::json!(5));
    });
    let (code, before) = build("wade-red", &camp, &dir);
    assert_eq!(code, 1, "refused at validation:\n{before}");
    assert!(before.contains("DW0886"), "{before}");
    assert!(
        before.contains("`walk_y: 3`"),
        "the move names the plane the piece really has:\n{before}"
    );

    edit_meta(&dir, "hello-room", |m| {
        m.insert("walk_y".into(), serde_json::json!(3));
    });
    let (code, after) = build("wade-green", &camp, &dir);
    assert_eq!(code, 0, "the move reaches a different verdict:\n{after}");
}

/// **CHOOSE the horizon the piece was built for.** A piece whose water reaches
/// its own outer face belongs to a world that puts a sea against that face; the
/// move is the base, and taking it is one word in one document.
#[test]
fn dw0886_choosing_the_ocean_holds_the_water_a_void_world_would_lose() {
    let dir = common::ocean_prefabs_dir("remedy-runoff", common::OceanRoom::Shore);
    // The tide pool, opened onto the piece's own west face: the water now has a
    // run direction that leaves these bytes, and what is beyond that face is the
    // horizon's business rather than the piece's.
    let size = [11, 9, 11];
    let mut cells: Vec<([i32; 3], &str)> = Vec::new();
    for x in 0..size[0] {
        for z in 0..size[2] {
            for y in 0..3 {
                let pool = y == 2 && z == 1 && x <= 1;
                cells.push((
                    [x, y, z],
                    if pool {
                        "minecraft:water"
                    } else {
                        "minecraft:stone"
                    },
                ));
            }
            let lamp = matches!((x, z), (3, 3) | (3, 7) | (7, 3) | (7, 7));
            for y in 3..size[1] {
                if x == 0 || x == size[0] - 1 || z == 0 || z == size[2] - 1 || y == size[1] - 1 {
                    // The west wall is open at the pool's own course, so the
                    // water reaches the piece's outer face.
                    if x == 0 && y == 3 && z == 1 {
                        continue;
                    }
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
    std::fs::write(
        dir.join("hello-room.nbt"),
        common::structure_nbt(size, &cells),
    )
    .unwrap();
    edit_meta(&dir, "hello-room", |m| {
        m.insert("structure".into(), {
            let mut s = m["structure"].clone();
            s["size"] = serde_json::json!(size);
            s
        });
    });

    let void = campaign("runoff-void", None);
    let (code, before) = build("runoff-red", &void, &dir);
    assert_eq!(code, 1, "refused at validation under void:\n{before}");
    assert!(before.contains("DW0886"), "{before}");
    assert!(
        before.contains("CHOOSE the horizon the piece was built for"),
        "the message names the move:\n{before}"
    );
    assert!(
        before.contains("`ocean`"),
        "and which base it means:\n{before}"
    );

    let ocean = campaign("runoff-ocean", Some(serde_json::json!("ocean")));
    let (code, after) = build("runoff-green", &ocean, &dir);
    assert_ne!(
        code, 1,
        "the move reaches a different verdict — the sea is against that face now:\n{after}"
    );
    assert!(
        !after.contains("runs out of"),
        "and the runoff finding is gone:\n{after}"
    );
}

// ---------------------------------------------------------------------------
// DW0344 — both arms
// ---------------------------------------------------------------------------

/// **DW0344, first arm: DECLARE the walk plane that seats this waterline.**
///
/// The area's origin is derived from the walk plane, so the two declarations are
/// claims about one building and the message names the number that reconciles
/// them.
#[test]
fn dw0344_declaring_the_walk_plane_lands_the_waterline_on_the_sea() {
    let dir = common::ocean_prefabs_dir("remedy-arm1", common::OceanRoom::Shore);
    let camp = campaign("arm1", Some(serde_json::json!("ocean")));

    edit_meta(&dir, "hello-room", |m| {
        m.insert("walk_y".into(), serde_json::json!(2));
    });
    let (code, before) = build("arm1-red", &camp, &dir);
    assert_eq!(code, 3, "refused at the build:\n{before}");
    assert!(before.contains("DW0344"), "{before}");
    assert!(
        before.contains("`walk_y: 3`"),
        "the move names the plane that seats this waterline:\n{before}"
    );

    edit_meta(&dir, "hello-room", |m| {
        m.insert("walk_y".into(), serde_json::json!(3));
    });
    let (code, after) = build("arm1-green", &camp, &dir);
    assert_eq!(code, 0, "the move reaches a different verdict:\n{after}");
}

/// **DW0344, second arm: RAISE the piece's low floor.**
///
/// This is the move that could not be taken at all before spec-0060. The check
/// asked whether the placement BOX reached the sea plane, and a box's minimum y
/// IS the area origin, so it was true of every piece in every ocean world
/// whatever its bytes held — no plinth, no rebuild and no amount of authoring
/// moved it. The decision spec-0060 §10.3 asks for is recorded here: the move is
/// **made observable**, not removed from the message. The quantifier is now the
/// walk cell, so the count in the refusal is a number the author's own edit
/// moves, and this is that edit.
///
/// Driven through `Plan` and `emit` directly rather than the CLI, deliberately:
/// `DW0886` asks the same question of the documents and would refuse this world
/// before a block was placed, which is the whole point of it existing. What is
/// under test here is the build-tier backstop for a world that got past that —
/// the piece a campaign edits after placement.
#[test]
fn dw0344_raising_the_low_floor_takes_the_piece_out_of_the_sea() {
    // A room whose main floor is at local y=3 with a pit down at y=1: the walk
    // plane is honestly declared as 3, so the area is seated at 60 and the pit
    // stands a body at world y=61 — under a sea at 62.
    let size = [11, 9, 11];
    let pit = |x: i32, z: i32| (3..=5).contains(&x) && (3..=5).contains(&z);
    let room = |with_pit: bool| {
        let mut cells: Vec<([i32; 3], &str)> = Vec::new();
        for x in 0..size[0] {
            for z in 0..size[2] {
                for y in 0..3 {
                    if with_pit && y == 2 && pit(x, z) {
                        // The pit: one course of the plinth taken out, so a body
                        // stands in it at local y=2 and can step back out. Two
                        // courses would put the floor beyond a step and the
                        // cell would never enter the walk region at all — the
                        // check would then be judging a cell no party reaches,
                        // which is not the finding.
                        continue;
                    }
                    cells.push(([x, y, z], "minecraft:stone"));
                }
                let lamp = matches!((x, z), (2, 2) | (2, 8) | (8, 2) | (8, 8));
                for y in 3..size[1] {
                    if x == 0 || x == size[0] - 1 || z == 0 || z == size[2] - 1 || y == size[1] - 1
                    {
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
        common::structure_nbt(size, &cells)
    };

    let verdict = |tag: &str, with_pit: bool| -> Result<BuildOutput, BuildFailure> {
        let dir = common::ocean_prefabs_dir(tag, common::OceanRoom::Shore);
        let nbt = room(with_pit);
        std::fs::write(dir.join("hello-room.nbt"), &nbt).unwrap();
        edit_meta(&dir, "hello-room", |m| {
            m.insert("walk_y".into(), serde_json::json!(3));
            m.remove("waterline_y");
            let mut s = m["structure"].clone();
            s["size"] = serde_json::json!(size);
            m.insert("structure".into(), s);
        });
        let prefabs = PrefabRegistry::load_dir(&dir).unwrap();
        let camp_dir = campaign(&format!("arm2-{tag}"), Some(serde_json::json!("ocean")));
        let loaded = load_campaign_dir(&camp_dir).unwrap();
        let parsed = parse_campaign(&loaded.raw).expect("the campaign parses");
        let plan = Plan::build(&parsed, &prefabs).expect("the plan builds");
        let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        for area in &plan.areas {
            for t in area.pieces.iter().flat_map(|p| &p.templates) {
                structures.insert(t.structure_file.clone(), nbt.clone());
            }
        }
        emit::build(
            &plan,
            &loaded.inputs,
            &structures,
            &CommandTree::v1_21_11(),
            &prefabs,
            None,
            &BTreeMap::new(),
        )
    };

    let (code, message) = match verdict("remedy-arm2-pit", true) {
        Err(BuildFailure::Diagnostic { code, message }) => (code.to_string(), message),
        other => panic!("a pit under the sea must be refused, got {other:?}"),
    };
    assert_eq!(code, "DW0344", "{message}");
    assert!(
        message.contains("where a body's feet go"),
        "the quantifier is the walk cell:\n{message}"
    );
    assert!(
        message.contains("RAISE the piece's low floor"),
        "the message names the move:\n{message}"
    );

    // The move: the same room with the pit filled. One change to the bytes, and
    // the count the refusal named is what moves.
    match verdict("remedy-arm2-filled", false) {
        Ok(_) => {}
        Err(other) => panic!("raising the floor must reach a different verdict, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// DW0855 — both moves
// ---------------------------------------------------------------------------

/// **`set `horizon` to `void` or `ocean`.`** The move `DW0855` names for a
/// campaign that places `areas[]` and states no extent.
#[test]
fn dw0855_setting_a_base_that_needs_no_map_builds() {
    let dir = common::ocean_prefabs_dir("remedy-valley", common::OceanRoom::Shore);
    let valley = campaign("valley", Some(serde_json::json!({ "base": "valley" })));
    let (code, before) = build("valley-red", &valley, &dir);
    assert_eq!(code, 1, "refused at validation:\n{before}");
    assert!(before.contains("DW0855"), "{before}");
    assert!(
        before.contains("set `horizon` to `void` or `ocean`"),
        "the message names the move:\n{before}"
    );

    for base in ["void", "ocean"] {
        let camp = campaign(&format!("valley-to-{base}"), Some(serde_json::json!(base)));
        let (code, after) = build(&format!("valley-to-{base}-out"), &camp, &dir);
        assert_eq!(
            code, 0,
            "`{base}` needs no map to be a horizon of:\n{after}"
        );
        assert!(!after.contains("DW0855"), "{after}");
    }
}

/// **`Give the campaign a site plan.`** The other move `DW0855` names — and the
/// one the cycle in spec-0060 §1 turned on, because adding a site plan to an
/// `areas[]` campaign is `DW0839`, two placement authorities for one world.
///
/// The move is therefore not "add a document to this campaign"; it is "author
/// the campaign under the other placement authority", and this asserts that the
/// campaign which takes it validates. The gallery's site-plan point is that
/// campaign — a real one, in this repository, declaring `{base: valley}`.
#[test]
fn dw0855_a_site_plan_campaign_on_a_terrain_base_validates() {
    let camp = tmp("site-plan");
    let gallery = common::repo_root().join("gallery");
    // The primary, then the overlay laid over it — the same materialisation
    // `tools/gallery_domain.py` performs for a build point.
    for src in [gallery.clone(), gallery.join("overlays/site-plan")] {
        for entry in std::fs::read_dir(&src).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            if path.is_dir() {
                if name == "l10n" {
                    common::copy_dir_all(&path, &camp.join("l10n"));
                }
                continue;
            }
            if name.ends_with(".json") && name != "overlay.json" {
                std::fs::copy(&path, camp.join(&name)).unwrap();
            }
        }
    }
    let world: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(camp.join("world.json")).unwrap()).unwrap();
    assert_eq!(
        world["content"]["horizon"]["base"], "valley",
        "the point that takes this move must declare the base that needs a map"
    );
    assert!(
        camp.join("site-plan.json").is_file(),
        "and it must carry the document the move names"
    );

    let out = delvec(&[
        "validate",
        camp.to_str().unwrap(),
        "--prefabs",
        "gallery-prefabs",
    ]);
    let text = log(&out);
    assert!(
        !text.contains("DW0855"),
        "a campaign that states its extent is not this refusal:\n{text}"
    );
    assert!(
        !text.contains("DW0839"),
        "and it is not two placement authorities either — which is what makes the move \
         reachable rather than a cycle:\n{text}"
    );
}

// ---------------------------------------------------------------------------
// DW0885 — the move a piece can take on its own
// ---------------------------------------------------------------------------

/// **`DECLARE the side shown.`** The one of `DW0885`'s three moves that is a
/// property of the piece, and therefore the one a library can carry.
///
/// The different verdict is a different code from a different check about a
/// different fact: `DW0885` runs before `DW0322` in `emit::build`, so reaching
/// the boundary proof at all is this check having passed.
#[test]
fn dw0885_declaring_the_sides_shown_reaches_the_next_proof() {
    let nbt = common::box_nbt([11, 6, 11], true);
    let refusal = |tag: &str, shown: Option<&[&str]>| -> (String, String) {
        let dir = common::shown_prefabs_dir(tag);
        std::fs::write(dir.join("hello-room.nbt"), &nbt).unwrap();
        edit_meta(&dir, "hello-room", |m| match shown {
            Some(sides) => {
                m.insert("shown_faces".into(), serde_json::json!(sides));
            }
            None => {
                m.remove("shown_faces");
            }
        });
        let prefabs = PrefabRegistry::load_dir(&dir).unwrap();
        let loaded = load_campaign_dir(&common::hello_world_dir()).unwrap();
        let parsed = parse_campaign(&loaded.raw).expect("the campaign parses");
        let plan = Plan::build(&parsed, &prefabs).expect("the plan builds");
        let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        for area in &plan.areas {
            for t in area.pieces.iter().flat_map(|p| &p.templates) {
                structures.insert(t.structure_file.clone(), nbt.clone());
            }
        }
        match emit::build(
            &plan,
            &loaded.inputs,
            &structures,
            &CommandTree::v1_21_11(),
            &prefabs,
            None,
            &BTreeMap::new(),
        ) {
            Err(BuildFailure::Diagnostic { code, message }) => (code.to_string(), message),
            other => panic!("expected a diagnostic, got {other:?}"),
        }
    };

    let (code, message) = refusal("remedy-885-silent", None);
    assert_eq!(code, "DW0885", "{message}");
    assert!(
        message.contains("DECLARE the side shown"),
        "the message names the move:\n{message}"
    );

    let (code, message) = refusal(
        "remedy-885-declared",
        Some(&["down", "east", "north", "south", "up"]),
    );
    assert_ne!(
        code, "DW0885",
        "the move reaches a different verdict:\n{message}"
    );
}

// ---------------------------------------------------------------------------
// DW0887 — the claim about the bytes
// ---------------------------------------------------------------------------

/// **`DELETE the declaration`** and **`CORRECT the number`** — the two moves
/// `DW0887` names that an author takes in the document, and both of them reach a
/// different verdict.
///
/// The third, `AUTHOR the shore the piece claims`, is a change to the `.nbt`;
/// it is the same edit the first case below undoes, taken from the other end,
/// and the tide pool the fixture starts with IS a piece that has taken it.
#[test]
fn dw0887_deleting_or_correcting_the_declaration_makes_the_claim_true() {
    // A shore whose tide pool has been paved over: the declaration is now a
    // claim about water that is not there.
    let dir = common::ocean_prefabs_dir("remedy-fiction", common::OceanRoom::Shore);
    let size = [11, 9, 11];
    let mut cells: Vec<([i32; 3], &str)> = Vec::new();
    for x in 0..size[0] {
        for z in 0..size[2] {
            for y in 0..3 {
                cells.push(([x, y, z], "minecraft:stone"));
            }
            let lamp = matches!((x, z), (3, 3) | (3, 7) | (7, 3) | (7, 7));
            for y in 3..size[1] {
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
    std::fs::write(
        dir.join("hello-room.nbt"),
        common::structure_nbt(size, &cells),
    )
    .unwrap();
    edit_meta(&dir, "hello-room", |m| {
        let mut st = m["structure"].clone();
        st["size"] = serde_json::json!(size);
        m.insert("structure".into(), st);
    });

    let camp = campaign("fiction", Some(serde_json::json!("ocean")));
    let (code, before) = build("fiction-red", &camp, &dir);
    assert_eq!(code, 1, "a fiction is refused at validation:\n{before}");
    assert!(before.contains("DW0887"), "{before}");
    assert!(
        before.contains("DELETE the declaration"),
        "the message names the move:\n{before}"
    );

    // The move: delete it. A piece that authors no shore has no waterline to
    // state, and deleting it does not make the piece unseatable — the ocean
    // seats it by `walk_y`.
    edit_meta(&dir, "hello-room", |m| {
        m.remove("waterline_y");
    });
    let (code, after) = build("fiction-green", &camp, &dir);
    assert_eq!(code, 0, "the move reaches a different verdict:\n{after}");

    // And the other move, on the piece that really does author a shore: a
    // declaration one course off its own top water block is corrected to the
    // number the bytes hold.
    let dir = common::ocean_prefabs_dir("remedy-correct", common::OceanRoom::Shore);
    edit_meta(&dir, "hello-room", |m| {
        m.insert("waterline_y".into(), serde_json::json!(4));
    });
    let camp = campaign("correct", Some(serde_json::json!("ocean")));
    let (code, before) = build("correct-red", &camp, &dir);
    assert_eq!(code, 1, "a wrong plane is refused at validation:\n{before}");
    assert!(before.contains("DW0887"), "{before}");
    assert!(
        before.contains("top authored water block") || before.contains("local y=2"),
        "the message names the number the bytes hold:\n{before}"
    );
    edit_meta(&dir, "hello-room", |m| {
        m.insert("waterline_y".into(), serde_json::json!(2));
    });
    let (code, after) = build("correct-green", &camp, &dir);
    assert_eq!(code, 0, "the move reaches a different verdict:\n{after}");
}

// ---------------------------------------------------------------------------
// DW0320 — the boundary an ocean needs
// ---------------------------------------------------------------------------

/// **`Add a boundary`, or `set `horizon` to `void``** — `DW0320`'s two moves,
/// and the second is the one that names a base.
///
/// This code is in this file because the cross-check put it here: nothing about
/// spec-0060 changed it, and a message that tells an author to change their
/// horizon owes the same proof as the three that motivated the rule.
#[test]
fn dw0320_adding_a_boundary_or_choosing_void_both_reach_a_different_verdict() {
    let dir = common::ocean_prefabs_dir("remedy-boundary", common::OceanRoom::Shore);

    // An ocean campaign with no `boundary` at all.
    let camp = tmp("camp-boundary");
    common::copy_dir_all(&common::hello_world_dir(), &camp);
    let mut world: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(camp.join("world.json")).unwrap()).unwrap();
    world["dsl_version"] = serde_json::json!("0.21.1");
    world["content"]
        .as_object_mut()
        .unwrap()
        .insert("horizon".into(), serde_json::json!("ocean"));
    let write = |camp: &Path, world: &serde_json::Value| {
        std::fs::write(
            camp.join("world.json"),
            serde_json::to_string_pretty(world).unwrap(),
        )
        .unwrap();
    };
    write(&camp, &world);
    let (code, before) = build("boundary-red", &camp, &dir);
    assert_eq!(code, 1, "refused at validation:\n{before}");
    assert!(before.contains("DW0320"), "{before}");
    assert!(
        before.contains("set `horizon` to `void`"),
        "the message names the move:\n{before}"
    );

    // Move one: add the boundary the message names.
    let mut with_boundary = world.clone();
    with_boundary["content"]
        .as_object_mut()
        .unwrap()
        .insert("boundary".into(), serde_json::json!({ "margin": 20 }));
    let camp_a = tmp("camp-boundary-added");
    common::copy_dir_all(&camp, &camp_a);
    write(&camp_a, &with_boundary);
    let (code, after) = build("boundary-added", &camp_a, &dir);
    assert_eq!(
        code, 0,
        "adding a boundary reaches a different verdict:\n{after}"
    );

    // Move two: the base that needs none.
    let mut as_void = world.clone();
    as_void["content"]
        .as_object_mut()
        .unwrap()
        .insert("horizon".into(), serde_json::json!("void"));
    let camp_b = tmp("camp-boundary-void");
    common::copy_dir_all(&camp, &camp_b);
    write(&camp_b, &as_void);
    let (code, after) = build("boundary-void", &camp_b, &dir);
    assert_eq!(code, 0, "`void` needs no boundary:\n{after}");
    assert!(!after.contains("DW0320"), "{after}");
}
