//! **A leg the party must cross is examined, or the build says why it cannot be.**
//!
//! A leg is a move from where the party stands to where the next critical
//! objective stands. The population starts at the **campaign spawn**, because
//! that is where the party stands when the delve begins.
//!
//! It did not. Two enumerations decided what a leg was — `obj_areas.windows(2)`
//! in `plan::build_critical_path`, which decides where an inter-area crossing is
//! emitted, and `nav::positions_of`, which decides what `DW0311` walks — and
//! both were rooted at the FIRST OBJECTIVE. The spawn is a leg's origin and is
//! not an objective, so the party's opening move was in no pair at all: no
//! crossing was emitted for it and nothing examined it. Such a campaign compiled
//! clean, passed every game test, and stranded the bot at the spawn with `No path
//! to the goal!` and no diagnostic code.
//!
//! The second half was quieter and older. A crossing was emitted **only if** the
//! destination area's prefab declared an entry point (spec-0046) — 5 of the 37
//! metadata files in the shipped library do — and when it did not, nothing was
//! said. The leg fell through to the walk proof, which reported `DW0311`: *a
//! wedged doorway seam, a void gap, an unbroken fence ring*. Every word of that
//! is true about a walk, and the leg was never a walk. An author who does what it
//! says goes and widens a doorway.
//!
//! The fixture is `first-leg`: two areas a yard apart, one beat in each, and the
//! vault bound to a piece that declares no entry. Three states, one variable
//! between each pair — which piece the vault binds (`DW0872` → green) and which
//! area the first beat plays in (green → `DW0873`).

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildFailure, BuildOutput};
use delvewright_compiler::plan::{self, Plan};
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::parse_campaign;
use serde_json::{Value, json};

/// A piece that declares an entry point, and one that does not — the whole of
/// the `DW0872` perturbation. Both are read back off the library below rather
/// than trusted here.
const HAS_ENTRY: &str = "prefab/cave-shore";
const NO_ENTRY: &str = "prefab/keep-room-small-b";

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dw-first-leg-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// The fixture, with `patch` applied to its stage documents.
fn fixture(tag: &str, patch: impl FnOnce(&Path)) -> PathBuf {
    let dir = scratch(tag);
    common::copy_dir_all(&common::compiler_fixtures_dir().join("first-leg"), &dir);
    patch(&dir);
    dir
}

fn plan_of(dir: &Path) -> Result<(delvewright_dsl::Campaign, PrefabRegistry), String> {
    let loaded = delvewright_compiler::load::load_campaign_dir(dir).expect("fixture loads");
    let campaign = parse_campaign(&loaded.raw).expect("fixture parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).expect("library loads");
    Ok((campaign, prefabs))
}

/// Plan the fixture, and hand back the refusal if it has one.
fn refusal(dir: &Path) -> Result<(), (String, String)> {
    let (campaign, prefabs) = plan_of(dir).unwrap();
    match Plan::build(&campaign, &prefabs) {
        Ok(_) => Ok(()),
        Err(e) => Err((e.failure.code.id().to_string(), e.failure.message)),
    }
}

/// Plan **and emit** the fixture — the whole ladder an author runs.
fn build(dir: &Path) -> Result<BuildOutput, BuildFailure> {
    let loaded = delvewright_compiler::load::load_campaign_dir(dir).expect("fixture loads");
    let campaign = parse_campaign(&loaded.raw).expect("fixture parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).expect("library loads");
    let plan = Plan::build(&campaign, &prefabs).map_err(|e| BuildFailure::Diagnostic {
        code: e.failure.code,
        message: e.failure.message,
    })?;
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
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
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
}

/// Bind the vault to a piece that can be arrived in.
fn vault_can_receive(dir: &Path) {
    common::patch_file(&dir.join("world.json"), |w| {
        for a in w["content"]["areas"].as_array_mut().expect("areas[]") {
            if a["id"] == "area/vault" {
                a["prefab"] = json!(HAS_ENTRY);
            }
        }
    });
    // `cave-shore` provides `anchor/exit`, not `anchor/npc-stand`.
    common::patch_file(&dir.join("quests.json"), |q| {
        for x in q["content"]["quests"].as_array_mut().expect("quests[]") {
            if x["id"] == "quest/vault" {
                x["objectives"][0]["anchor"] = json!("anchor/exit");
            }
        }
    });
}

// ---------------------------------------------------------------------------
// the library the perturbation rests on — read, never assumed
// ---------------------------------------------------------------------------

/// The two pieces really do differ in the one way the fixture turns on, and the
/// count is taken off the library rather than written down beside it.
#[test]
fn the_perturbation_is_which_piece_declares_an_entry() {
    let dir = common::prefabs_dir();
    let entry_of = |piece: &str| -> bool {
        let stem = piece.strip_prefix("prefab/").unwrap();
        let meta: Value = serde_json::from_str(
            &std::fs::read_to_string(dir.join(format!("{stem}.json"))).unwrap(),
        )
        .unwrap();
        meta["anchors"]
            .as_object()
            .unwrap()
            .values()
            .any(|a| a.get("role") == Some(&Value::from(plan::AnchorRole::Entry.to_string())))
    };
    assert!(entry_of(HAS_ENTRY), "{HAS_ENTRY} must be arrivable-in");
    assert!(!entry_of(NO_ENTRY), "{NO_ENTRY} must not be");

    // How rare this is, is the reason the silence mattered. Computed over the
    // library, with its denominator: a constant here would be a binding count
    // that cannot move.
    let mut total = 0usize;
    let mut with_entry = 0usize;
    for e in std::fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        if p.extension().and_then(|x| x.to_str()) != Some("json") {
            continue;
        }
        total += 1;
        let meta: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        let has = meta
            .get("anchors")
            .and_then(Value::as_object)
            .is_some_and(|anchors| {
                anchors.values().any(|a| {
                    a.get("role") == Some(&Value::from(plan::AnchorRole::Entry.to_string()))
                })
            });
        with_entry += usize::from(has);
    }
    assert!(total > 0, "the library must not be empty");
    assert!(
        with_entry * 4 < total,
        "an area that can be arrived in is the exception, not the rule: {with_entry} of \
         {total} pieces declare an entry. If that has stopped being true, this file's \
         premise has changed and the diagnostics below are worth re-reading"
    );
}

// ---------------------------------------------------------------------------
// DW0872 — a crossing into an area with nowhere to arrive
// ---------------------------------------------------------------------------

/// The fixture as committed: the second beat is in an area whose piece declares
/// no entry. Nothing about the geometry is wrong; there is simply nowhere in the
/// vault to put the party down.
#[test]
fn a_crossing_into_an_area_with_no_entry_is_dw0872() {
    let dir = fixture("no-entry", |_| {});
    let (code, message) = refusal(&dir).expect_err("a crossing nothing can land must be refused");
    assert_eq!(code, "DW0872", "{message}");

    // The message names the leg — both ends, so the author can find it — and the
    // declaration that is missing, in the vocabulary spec-0046 gave it.
    for expected in [
        "obj/bell",
        "area/hall",
        "obj/take",
        "area/vault",
        r#""role": "entry""#,
    ] {
        assert!(
            message.contains(expected),
            "the refusal must name `{expected}`: {message}"
        );
    }
    // …and it must NOT be the walk refusal's vocabulary. This leg was never a
    // walk, and an author sent to look at a doorway looks at a doorway.
    for absent in ["doorway", "fence", "cannot walk"] {
        assert!(
            !message.contains(absent),
            "a leg that was never a walk must not be described as one (`{absent}`): {message}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// **The control, and the whole of the variable.** The same campaign with the
/// vault bound to a piece that declares an entry builds — and the crossing is
/// emitted on the beat the party leaves from.
#[test]
fn the_same_campaign_builds_when_the_vault_can_be_arrived_in() {
    let dir = fixture("has-entry", vault_can_receive);
    let out = build(&dir).expect("a crossing with somewhere to land builds");
    let cp: Value =
        serde_json::from_slice(out.get("validation/critical-path-waypoints.json").unwrap())
            .unwrap();
    assert!(
        cp["legs"].as_array().is_some_and(|l| !l.is_empty()),
        "the build must export the legs it proved"
    );

    let (campaign, prefabs) = plan_of(&dir).unwrap();
    let plan = Plan::build(&campaign, &prefabs).unwrap();
    assert_eq!(
        plan.transport.keys().collect::<Vec<_>>(),
        vec!["obj/bell"],
        "the crossing rides on the completion of the beat the party leaves from"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// DW0873 — the party's first leg, which nothing used to examine
// ---------------------------------------------------------------------------

/// The stranding campaign: the party begins in the hall and the first thing
/// asked of them is in the vault. This is the case that used to build green,
/// pass every game test, and strand the bot.
#[test]
fn a_first_beat_across_the_yard_is_dw0873() {
    let dir = fixture("stranded", |d| {
        vault_can_receive(d);
        // One variable: the first beat moves out of the area the party starts in.
        common::patch_file(&d.join("quest-plan.json"), |p| {
            for q in p["content"]["quests"].as_array_mut().expect("quests[]") {
                if q["id"] == "quest/hall" {
                    q["area"] = json!("area/vault");
                }
            }
        });
        common::patch_file(&d.join("quests.json"), |q| {
            for x in q["content"]["quests"].as_array_mut().expect("quests[]") {
                if x["id"] == "quest/hall" {
                    x["objectives"][0]["anchor"] = json!("anchor/exit");
                }
            }
        });
    });
    let (code, message) = refusal(&dir).expect_err("a first leg nothing can carry must be refused");
    assert_eq!(code, "DW0873", "{message}");
    for expected in ["area/hall", "area/vault", "obj/bell", "quest plan"] {
        assert!(
            message.contains(expected),
            "the refusal must name `{expected}`: {message}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// the population, counted
// ---------------------------------------------------------------------------

/// **The party's own first move is in the population, and the count says so.**
///
/// `nav::critical_leg_count` is the number of legs the completability proof
/// routes. The old enumeration paired objectives, so a campaign of `n`
/// objectives had `n - 1` legs and the first one — the party's — was not among
/// them. It has `n`.
///
/// Asserted on a real campaign rather than on the fixture, and by comparing two
/// campaigns of different length, so it cannot pass on a compiler that returns a
/// constant.
#[test]
fn the_first_leg_is_counted() {
    let count = |dir: &Path| -> (usize, usize) {
        let loaded = delvewright_compiler::load::load_campaign_dir(dir).expect("loads");
        let campaign = parse_campaign(&loaded.raw).expect("parses");
        let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
        let plan = Plan::build(&campaign, &prefabs).expect("plans");
        // Every leg of these two is a walk. A crossing is a ride and is not
        // counted, so a campaign with one would make the arithmetic below say
        // something else.
        assert!(
            plan.transport.is_empty(),
            "{dir:?} must cross no areas for this count to be objectives-many"
        );
        let objectives = plan.objective_steps.len();
        (
            objectives,
            delvewright_compiler::nav::critical_leg_count(&plan),
        )
    };
    let hello = count(&common::hello_world_dir());
    let trial = count(&common::keep_trial_dir());
    assert_ne!(
        hello.0, trial.0,
        "the two campaigns must differ in length, or this proves nothing about counting"
    );
    for (objectives, legs) in [hello, trial] {
        assert_eq!(
            legs, objectives,
            "one leg per objective: the party walks to each of them, and the first walk \
             starts at the campaign spawn. `objectives - 1` is the old population, with \
             the party's own first move missing from it"
        );
    }
}

/// **The first leg's own route starts where the party starts.** The count above
/// says how many legs there are; this says which cell the first one leaves from,
/// read off the artifact the harness actually walks.
#[test]
fn the_first_exported_route_leaves_the_campaign_spawn() {
    let dir = fixture("route", vault_can_receive);
    let out = build(&dir).expect("builds");
    let doc: Value =
        serde_json::from_slice(out.get("validation/critical-path-waypoints.json").unwrap())
            .unwrap();

    let (campaign, prefabs) = plan_of(&dir).unwrap();
    let plan = Plan::build(&campaign, &prefabs).unwrap();
    let (start_area, start) = plan.campaign_start().expect("the campaign has a start");
    assert_eq!(start_area, "area/hall");

    let first = doc["legs"][0]["waypoints"][0]
        .as_array()
        .expect("the first leg's first waypoint")
        .iter()
        .map(|v| v.as_i64().unwrap() as i32)
        .collect::<Vec<_>>();
    // The route is snapped to standable floor, so the cell may be a neighbour of
    // the anchor rather than the anchor itself; what must be true is that it is
    // the party's own cell and not the first objective's.
    let d = (first[0] - start[0]).abs() + (first[1] - start[1]).abs() + (first[2] - start[2]).abs();
    assert!(
        d <= 2,
        "the first exported route must leave the campaign spawn {start:?}, not {first:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// the walk refusal still fires for a walk
// ---------------------------------------------------------------------------

/// **`DW0311` is not silenced.** A same-area leg that geometry really does wall
/// off is still the walk refusal, in its own words. The two codes above fire
/// *before* it and only for legs that were never walks; a leg that is one still
/// gets the doorway-and-fence message, because for a walk that message is right.
///
/// The wall is the piece's own gate: `hello-room` places `anchor/door` shut at
/// world-load, and this campaign never opens it. The beat moves to the far side
/// of it — inside the spawn area, so nothing about areas is involved.
#[test]
fn a_walk_a_shut_door_wedges_is_still_the_walk_refusal() {
    let dir = fixture("wedged", |d| {
        vault_can_receive(d);
        common::patch_file(&d.join("quests.json"), |q| {
            for x in q["content"]["quests"].as_array_mut().expect("quests[]") {
                if x["id"] == "quest/hall" {
                    x["objectives"][0]["anchor"] = json!("anchor/exit");
                }
            }
        });
    });
    let err = build(&dir).expect_err("a walk the geometry wedges must be refused");
    let BuildFailure::Diagnostic { code, message } = err else {
        panic!("the refusal must be a diagnostic: {err:?}");
    };
    assert!(
        code == "DW0311" || code == "DW0317",
        "a wedged same-area walk stays the geometry's own refusal, not a crossing's: \
         {code:?} {message}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
