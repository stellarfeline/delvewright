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
    world["dsl_version"] = serde_json::json!("0.23.0");
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

/// **DECLARE `walk_y`, on a base that derives nothing from it.** The same move
/// and the same message on `void`, whose area origin is a fixed datum rather
/// than `walk_ref_y - walk_y`.
///
/// It is a row of its own because a remedy proven only where the number is also
/// CONSUMED is a remedy proven for the ocean's arithmetic. `walk_y` is owed on
/// every base (spec-0060 §4 rule 1 — a piece placed on ANY base without it is
/// this code), so the refusal a creator meets on a base with no sea has to
/// name a move that base can take, and this is the assertion that it does.
#[test]
fn dw0886_declaring_the_walk_plane_seats_the_piece_where_no_origin_needs_it() {
    let dir = common::ocean_prefabs_dir("remedy-walk-void", common::OceanRoom::Shore);
    let camp = campaign("walk-void", Some(serde_json::json!("void")));

    let declared = {
        let path = dir.join("hello-room.json");
        let doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        doc["walk_y"].clone()
    };
    edit_meta(&dir, "hello-room", |m| {
        m.remove("walk_y");
    });
    let (code, before) = build("walk-void-red", &camp, &dir);
    assert_eq!(code, 1, "refused at validation:\n{before}");
    assert!(before.contains("DW0886"), "{before}");
    assert!(
        before.contains("on a `void` horizon"),
        "the refusal names the base it was raised on:\n{before}"
    );
    assert!(
        before.contains("DECLARE `walk_y`"),
        "the message names the move:\n{before}"
    );

    // The move, taken.
    edit_meta(&dir, "hello-room", |m| {
        m.insert("walk_y".into(), declared.clone());
    });
    let (code, after) = build("walk-void-green", &camp, &dir);
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
// DW0886 — the two moves it names a SITE PLAN
// ---------------------------------------------------------------------------

/// A site-plan campaign, copied and handed to a closure that edits its plan.
///
/// The two fixtures are the two shapes this pair of rows needs: `shore-plan`
/// stands its whole map one course over the sea and analyzes green on an
/// `ocean`, and `blockout` cuts an undercroft into the rock beneath a cell,
/// which no amount of raising can lift clear because the cell stands on it.
fn site_plan_campaign(
    tag: &str,
    fixture: &str,
    horizon: Option<serde_json::Value>,
    edit: impl FnOnce(&mut serde_json::Value),
) -> PathBuf {
    let camp = tmp(&format!("camp-{tag}"));
    common::copy_dir_all(
        &common::repo_root()
            .join("crates/delvec/tests/fixtures")
            .join(fixture),
        &camp,
    );
    let read = |p: &Path| -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
    };
    let write = |p: &Path, v: &serde_json::Value| {
        std::fs::write(p, serde_json::to_string_pretty(v).unwrap() + "\n").unwrap();
    };
    if let Some(h) = horizon {
        let p = camp.join("world.json");
        let mut world = read(&p);
        let content = world["content"].as_object_mut().unwrap();
        content.insert("horizon".into(), h);
        content.insert("boundary".into(), serde_json::json!({ "margin": 20 }));
        write(&p, &world);
    }
    let p = camp.join("site-plan.json");
    let mut plan = read(&p);
    edit(&mut plan);
    write(&p, &plan);
    camp
}

/// `delvec analyze` — the tier `DW0886` refuses at, so a red here is a refusal
/// that cost no assembly.
fn analyze(camp: &Path, prefabs: &Path) -> (i32, String) {
    let r = delvec(&[
        "analyze",
        camp.to_str().unwrap(),
        "--prefabs",
        prefabs.to_str().unwrap(),
    ]);
    (r.status.code().unwrap_or(-1), log(&r))
}

/// **RAISE this place's `floor`.** The first move `DW0886` names a site plan
/// whose places stand at or below the sea plane — and the message's own sentence
/// about it is that a `datum` lifts every place standing on it, which is what
/// makes the move ONE number rather than one per room.
///
/// The refusal is manufactured by sinking the whole map two courses — both
/// datums, the sightline it carries, and the brief fact an identity holds the
/// loft's plane to — so the perturbation is a rigid translation and nothing else
/// about the plan changes. The move puts it back. Measured on the fixture: sunk,
/// `analyze` exits 1 with `DW0886` naming all four places on the grade datum and
/// no other code; raised, exit 0.
#[test]
fn dw0886_raising_the_plan_seats_every_place_standing_on_the_datum() {
    let prefabs = common::prefabs_dir();
    let sunk = site_plan_campaign("dw0886-sunk", "shore-plan", None, |plan| {
        for datum in plan["content"]["datums"].as_array_mut().unwrap() {
            let y = datum["y"].as_i64().unwrap();
            datum["y"] = serde_json::json!(y - 2);
        }
        for line in plan["content"]["sightlines"].as_array_mut().unwrap() {
            for end in ["from", "to"] {
                let y = line[end][1].as_i64().unwrap();
                line[end][1] = serde_json::json!(y - 2);
            }
        }
    });
    // The brief is where the loft's plane is written down, so a rigid lift of
    // the plan moves the fact with it — otherwise `DW0833` fires and this row
    // would be proving a different refusal.
    let brief_path = sunk.join("geometry-brief.json");
    let mut brief: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&brief_path).unwrap()).unwrap();
    for fact in brief["content"]["facts"].as_array_mut().unwrap() {
        if fact["id"] == "fact/loft-datum" {
            let v = fact["value"].as_f64().unwrap();
            fact["value"] = serde_json::json!(v - 2.0);
        }
    }
    std::fs::write(
        &brief_path,
        serde_json::to_string_pretty(&brief).unwrap() + "\n",
    )
    .unwrap();

    let (code, before) = analyze(&sunk, &prefabs);
    assert_eq!(code, 1, "refused at validation, nothing placed:\n{before}");
    assert!(before.contains("DW0886"), "{before}");
    assert!(
        before.contains("stands its walk plane at world y=62"),
        "the refusal names where the place puts a body's feet:\n{before}"
    );
    assert!(
        before.contains("RAISE this place's `floor`"),
        "the message names the move:\n{before}"
    );
    assert!(
        before.contains("moving the datum lifts every place standing on it"),
        "and says the move is one number:\n{before}"
    );

    // The move: the fixture as committed, standing one course over the sea.
    let raised = site_plan_campaign("dw0886-raised", "shore-plan", None, |_| {});
    let (code, after) = analyze(&raised, &prefabs);
    assert_eq!(code, 0, "raising the plan reaches a green:\n{after}");
    assert!(!after.contains("DW0886"), "{after}");
    // And the binding says what it judged, so this green is over five places
    // rather than over none.
    assert!(
        after.contains("site plan: 5 of 5 box(es) judged against this base"),
        "the binding states the model it reached:\n{after}"
    );
}

/// **CHOOSE another horizon.** The second move, and the only one a place that is
/// DELIBERATELY under grade can take: `blockout`'s undercroft is a cache cut into
/// the rock beneath the cell, so raising it is refused by the cell standing on
/// top of it (`DW0827`) and the plan is right as it is — what is wrong is the
/// sea. Both bases with no sea are taken, because a move that named two and
/// worked on one would be half a remedy.
#[test]
fn dw0886_choosing_a_horizon_with_no_sea_seats_a_place_cut_below_grade() {
    let prefabs = common::prefabs_dir();
    let ocean = site_plan_campaign(
        "dw0886-cache-ocean",
        "blockout",
        Some(serde_json::json!("ocean")),
        |_| {},
    );
    let (code, before) = analyze(&ocean, &prefabs);
    assert_eq!(code, 1, "refused at validation:\n{before}");
    assert!(before.contains("DW0886"), "{before}");
    assert!(
        before.contains("`node/undercroft`"),
        "the refusal names the place:\n{before}"
    );
    assert!(
        before.contains("CHOOSE another horizon"),
        "the message names the move:\n{before}"
    );

    for base in ["void", "valley"] {
        let camp = site_plan_campaign(
            &format!("dw0886-cache-{base}"),
            "blockout",
            Some(serde_json::json!({ "base": base })),
            |_| {},
        );
        let (code, after) = analyze(&camp, &prefabs);
        assert_eq!(
            code, 0,
            "`{base}` has no sea for a cache to stand in:\n{after}"
        );
        assert!(!after.contains("DW0886"), "{after}");
        assert!(
            after.contains("site plan: 7 of 7 box(es) judged against this base"),
            "the check still examined every place on `{base}`:\n{after}"
        );
    }
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
    // Refused at VALIDATION, exit 1: both numbers are in the document and the
    // origin is derived from one of them, so nothing has to be placed to know
    // the two disagree. Same code, one stage earlier.
    assert_eq!(code, 1, "refused at validation:\n{before}");
    assert!(before.contains("DW0344"), "{before}");
    assert!(
        before.contains("`walk_y: 3`"),
        "the move names the plane that seats this waterline:\n{before}"
    );
    assert!(
        !before.contains("place template"),
        "and nothing was placed:\n{before}"
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
    world["dsl_version"] = serde_json::json!("0.23.0");
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

// ---------------------------------------------------------------------------
// DW0890 — the approved hour is the built hour
// ---------------------------------------------------------------------------
//
// Eight moves across three shapes, and every one of them is an edit to a
// document or to the `design/` directory beside it. They are taken here rather
// than only asserted in `design_record.rs` because the question this file asks
// is not *does the rule fire* — it is *does the move the rule names reach a
// different verdict*, which is a different assertion and the one nobody was
// making about `DW0885`.

/// A hello-world copy carrying `rows` in `design.json` and `files` under
/// `design/`, with `world.json`'s hour set to `time`.
fn design_campaign(tag: &str, time: &str, files: &[&str], rows: &[(&str, &str, &str)]) -> PathBuf {
    let camp = tmp(&format!("camp-{tag}"));
    common::copy_dir_all(&common::hello_world_dir(), &camp);
    common::patch_file(&camp.join("world.json"), |v| {
        v["content"]["time"] = serde_json::json!(time);
    });
    for f in files {
        let p = camp.join("design").join(f);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        // Nothing opens image bytes (spec-0061 §12), so nothing here writes any.
        std::fs::write(p, b"an approved picture").unwrap();
    }
    if !rows.is_empty() {
        write_design(&camp, rows);
    }
    camp
}

/// Write (or rewrite) `design.json` over `rows` of `(name, time, weather)`.
fn write_design(camp: &Path, rows: &[(&str, &str, &str)]) {
    let refs: Vec<serde_json::Value> = rows
        .iter()
        .map(|(name, time, weather)| {
            serde_json::json!({
                "name": name,
                "shows": "the picture, in one sentence",
                "time": time,
                "weather": weather,
            })
        })
        .collect();
    std::fs::write(
        camp.join("design.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "dsl_version": delvewright_dsl::DSL_VERSION,
            "campaign_id": "hello-world",
            "stage": "design",
            "content": { "references": refs },
        }))
        .unwrap(),
    )
    .unwrap();
}

/// **`DECLARE the hour the record states in `world.json``** and **`AUTHOR the
/// design again under the hour this world reaches`** — the two moves shape a
/// names for a world and a record that disagree.
#[test]
fn dw0890_declaring_the_recorded_hour_and_re_approving_the_design_both_build() {
    let dir = common::prefabs_dir();
    let red = design_campaign(
        "sky-red",
        "noon",
        &["concept/shore-far.png"],
        &[("concept/shore-far", "night", "clear")],
    );
    let (code, before) = build("sky-red", &red, &dir);
    assert_eq!(code, 1, "refused:\n{before}");
    assert!(before.contains("DW0890"), "{before}");
    assert!(
        before.contains("DECLARE the hour the record states in `world.json`"),
        "the message names the move:\n{before}"
    );
    assert!(
        before.contains("AUTHOR the design again under the hour this world reaches"),
        "and the second one:\n{before}"
    );

    // Move one: declare the hour the record states.
    let a = design_campaign(
        "sky-declare",
        "night",
        &["concept/shore-far.png"],
        &[("concept/shore-far", "night", "clear")],
    );
    let (code, after) = build("sky-declare", &a, &dir);
    assert_eq!(code, 0, "declaring `night` builds:\n{after}");
    assert!(!after.contains("DW0890 [error]"), "{after}");

    // Move two: re-approve the design under the hour this world reaches.
    let b = design_campaign(
        "sky-reapprove",
        "noon",
        &["concept/shore-far.png"],
        &[("concept/shore-far", "noon", "clear")],
    );
    let (code, after) = build("sky-reapprove", &b, &dir);
    assert_eq!(code, 0, "re-approving under `noon` builds:\n{after}");
    assert!(!after.contains("DW0890 [error]"), "{after}");
}

/// **`DELETE that effect`** — shape a's third move, for an hour the world
/// reaches only because a `set-time` puts it there.
#[test]
fn dw0890_deleting_the_set_time_effect_that_added_the_hour_builds() {
    let dir = common::prefabs_dir();
    let camp = design_campaign(
        "sky-effect",
        "night",
        &["concept/shore-far.png"],
        &[("concept/shore-far", "night", "clear")],
    );
    // The effect is APPENDED to whatever the campaign already fires, and the
    // green half restores the original bytes — deleting the whole bundle would
    // take the campaign's `open-gate` with it and redden `DW0317` instead,
    // which is a different verdict for the wrong reason.
    let quests = std::fs::read_to_string(camp.join("quests.json")).unwrap();
    common::patch_file(&camp.join("quests.json"), |v| {
        let q = &mut v["content"]["quests"][0];
        let id = q["objectives"][0]["id"].as_str().unwrap().to_string();
        let bundle = q["on_objective_complete"][&id]
            .as_array_mut()
            .expect("hello-world fires an effect on its first objective");
        bundle.push(serde_json::json!({ "type": "set-time", "time": "dawn" }));
    });
    let (code, before) = build("sky-effect-red", &camp, &dir);
    assert_eq!(code, 1, "refused:\n{before}");
    assert!(before.contains("DW0890"), "{before}");
    assert!(
        before.contains("the third move is to DELETE that effect"),
        "the message names the move:\n{before}"
    );

    std::fs::write(camp.join("quests.json"), &quests).unwrap();
    let (code, after) = build("sky-effect-green", &camp, &dir);
    assert_eq!(code, 0, "deleting the effect builds:\n{after}");
    assert!(!after.contains("DW0890 [error]"), "{after}");
}

/// **`AUTHOR the approved image into `design/<dir>/``** and **`DELETE the row
/// from `design.json``** — shape b's two moves, for a row that names no file.
#[test]
fn dw0890_adding_the_missing_image_and_deleting_its_row_both_build() {
    let dir = common::prefabs_dir();
    let red = design_campaign(
        "row-red",
        "night",
        &["concept/shore-far.png"],
        &[
            ("concept/shore-far", "night", "clear"),
            ("concept/tower-far", "night", "clear"),
        ],
    );
    let (code, before) = build("row-red", &red, &dir);
    assert_eq!(code, 1, "refused:\n{before}");
    assert!(before.contains("DW0890"), "{before}");
    assert!(
        before.contains("AUTHOR the approved image into `design/concept/`"),
        "the message names the move:\n{before}"
    );
    assert!(
        before.contains("DELETE the row from `design.json`"),
        "and the second one:\n{before}"
    );

    // Move one: copy the approved image in.
    let a = design_campaign(
        "row-image",
        "night",
        &["concept/shore-far.png", "concept/tower-far.png"],
        &[
            ("concept/shore-far", "night", "clear"),
            ("concept/tower-far", "night", "clear"),
        ],
    );
    let (code, after) = build("row-image", &a, &dir);
    assert_eq!(code, 0, "the image resolves the row:\n{after}");
    assert!(!after.contains("DW0890 [error]"), "{after}");

    // Move two: delete the row.
    let b = design_campaign(
        "row-delete",
        "night",
        &["concept/shore-far.png"],
        &[("concept/shore-far", "night", "clear")],
    );
    let (code, after) = build("row-delete", &b, &dir);
    assert_eq!(code, 0, "deleting the row builds:\n{after}");
    assert!(!after.contains("DW0890 [error]"), "{after}");
}

/// **`AUTHOR its row in `design.json``**, **`DELETE the file`** and **`AUTHOR
/// the document`** — shape c's three moves, for an approved image nobody
/// recorded.
#[test]
fn dw0890_recording_the_image_deleting_it_and_authoring_the_document_all_build() {
    let dir = common::prefabs_dir();
    let red = design_campaign(
        "file-red",
        "night",
        &["concept/shore-far.png", "concept/shore-near.png"],
        &[("concept/shore-far", "night", "clear")],
    );
    let (code, before) = build("file-red", &red, &dir);
    assert_eq!(code, 1, "refused:\n{before}");
    assert!(before.contains("DW0890"), "{before}");
    assert!(
        before.contains("AUTHOR its row in `design.json`"),
        "the message names the move:\n{before}"
    );
    assert!(
        before.contains("DELETE the file if it was never approved"),
        "and the second one:\n{before}"
    );

    // Move one: record it.
    let a = design_campaign(
        "file-record",
        "night",
        &["concept/shore-far.png", "concept/shore-near.png"],
        &[
            ("concept/shore-far", "night", "clear"),
            ("concept/shore-near", "night", "clear"),
        ],
    );
    let (code, after) = build("file-record", &a, &dir);
    assert_eq!(code, 0, "recording it builds:\n{after}");
    assert!(!after.contains("DW0890 [error]"), "{after}");

    // Move two: remove the file that was never approved.
    let b = design_campaign(
        "file-remove",
        "night",
        &["concept/shore-far.png"],
        &[("concept/shore-far", "night", "clear")],
    );
    let (code, after) = build("file-remove", &b, &dir);
    assert_eq!(code, 0, "removing it builds:\n{after}");
    assert!(!after.contains("DW0890 [error]"), "{after}");

    // Move three: a campaign with images and no document at all is told to
    // AUTHOR the document, and authoring it reaches a different verdict.
    let c = design_campaign("file-no-doc", "night", &["concept/shore-far.png"], &[]);
    let (code, before) = build("file-no-doc-red", &c, &dir);
    assert_eq!(code, 1, "refused:\n{before}");
    assert!(
        before.contains("has no `design.json` at all") && before.contains("AUTHOR the document"),
        "the message names the move:\n{before}"
    );
    write_design(&c, &[("concept/shore-far", "night", "clear")]);
    let (code, after) = build("file-no-doc-green", &c, &dir);
    assert_eq!(code, 0, "authoring the document builds:\n{after}");
    assert!(!after.contains("DW0890 [error]"), "{after}");
}
