//! spec-0029: the `flood` verb — declaring that a stretch of ground is
//! deliberately at the waterline, and the two proofs that keep the declaration
//! from being an exemption with a nicer name.
//!
//! The load-bearing fixture is [`flood_cannot_silence_the_149_class`]: the
//! spec-0026 tide-mill red with a `flood` declared straight over it stays red —
//! it just changes which red. `flood` is not a suppression switch; it is a claim
//! that the sea reaches those cells, and the sea cannot reach a sealed interior.

mod common;

use std::path::Path;
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn delvec(args: &[&str]) -> Output {
    Command::new(BIN).args(args).output().expect("run delvec")
}

fn code(out: &Output) -> i32 {
    out.status.code().unwrap_or(-1)
}

/// Everything `delvec` said: diagnostics are split across stdout (advisories)
/// and stderr (the fatal one), and these fixtures assert on both.
fn said(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

fn tmp(name: &str) -> std::path::PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A private prefab copy with `hello-room`'s `walk_y` set to `walk_y` — the
/// spec-0026 datum knob that decides which world y the room's floor lands at.
fn prefabs_with_walk_y(name: &str, walk_y: i64) -> std::path::PathBuf {
    let dir = tmp(name);
    common::copy_dir_all(&common::prefabs_dir(), &dir);
    let path = dir.join("hello-room.json");
    let mut meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    meta["walk_y"] = serde_json::json!(walk_y);
    std::fs::write(&path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();
    dir
}

/// hello-world with a patched `horizon` (raw JSON; `null` leaves the stage-1
/// document untouched, i.e. the default `void` world) and an optional stage-7
/// edit script.
fn campaign(
    name: &str,
    horizon: Option<&str>,
    edits: Option<serde_json::Value>,
) -> std::path::PathBuf {
    let camp = tmp(name);
    common::copy_dir_all(&common::hello_world_dir(), &camp);
    if let Some(h) = horizon {
        let mut world: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(camp.join("world.json")).unwrap())
                .unwrap();
        world["dsl_version"] = serde_json::json!("0.6.0");
        let content = world["content"].as_object_mut().unwrap();
        content.insert("horizon".into(), serde_json::from_str(h).unwrap());
        content.insert("boundary".into(), serde_json::json!({ "margin": 20 }));
        std::fs::write(
            camp.join("world.json"),
            serde_json::to_string_pretty(&world).unwrap(),
        )
        .unwrap();
    }
    if let Some(batches) = edits {
        let doc = serde_json::json!({
            "dsl_version": "0.9.0",
            "campaign_id": "hello-world",
            "stage": "world-edits",
            "content": { "batches": batches },
        });
        std::fs::write(
            camp.join("world-edits.json"),
            serde_json::to_string_pretty(&doc).unwrap(),
        )
        .unwrap();
    }
    camp
}

fn build(camp: &Path, out: &Path, prefabs: &Path) -> Output {
    delvec(&[
        "build",
        camp.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        prefabs.to_str().unwrap(),
    ])
}

/// A `select` of a piece-local box of hello-world's single `hello-room` piece.
fn select(name: &str, min: [i32; 3], max: [i32; 3]) -> serde_json::Value {
    serde_json::json!({
        "verb": "select",
        "name": name,
        "shape": {
            "kind": "box",
            "frame": { "kind": "piece-local", "piece": 0, "prefab": "prefab/hello-room" },
            "min": min,
            "max": max,
        }
    })
}

/// The shore-notch fixture's edits: build a seabed shelf under the room's west
/// wall line, then cut the wall down to the waterline over `notch_z`, so the
/// room gains standable cells whose feet sit exactly at the ocean line (y=62)
/// and which the open sea can flow into. `envelope_z` is the z-range the
/// `flood` envelope covers — narrower than `notch_z` is a shoreline that does
/// not stop where it was declared to.
fn shore_notch(notch_z: (i32, i32), envelope_z: Option<(i32, i32)>) -> serde_json::Value {
    let mut edits = vec![
        select("region/seabed", [0, -1, notch_z.0], [0, -1, notch_z.1]),
        serde_json::json!({
            "verb": "fill",
            "region": "region/seabed",
            "recipe": { "blocks": [{ "block": "minecraft:stone", "weight": 1.0 }], "scale": 0.3 }
        }),
        select("region/notch", [0, 0, notch_z.0], [0, 1, notch_z.1]),
        serde_json::json!({ "verb": "carve", "region": "region/notch" }),
    ];
    if let Some((lo, hi)) = envelope_z {
        edits.push(select("region/shore", [0, 0, lo], [0, 0, hi]));
        edits.push(serde_json::json!({ "verb": "flood", "region": "region/shore" }));
    }
    serde_json::json!([{ "id": "batch/shore", "area": "area/keep", "edits": edits }])
}

fn water_lines(out: &Path) -> Vec<String> {
    let f = out.join("datapack/data/hello-world/function/world_edits.mcfunction");
    std::fs::read_to_string(f)
        .unwrap()
        .lines()
        .filter(|l| l.contains("minecraft:water"))
        .map(str::to_string)
        .collect()
}

/// **The test this whole feature exists to pass.** The spec-0026 #149 fixture —
/// an interior room mis-datumed onto world y=61, a block under the sea — with a
/// `flood` declared straight over the whole piece. It does NOT go green: the
/// room is sealed, the sea reaches none of it, and the declaration is `DW0394`
/// (it binds nothing). Drop the declaration and the original `DW0364` is back.
///
/// A declaration that merely suppressed `DW0364` would have turned this build
/// green, which is exactly the hole spec-0026 closed.
#[test]
fn flood_cannot_silence_the_149_class() {
    let prefabs = prefabs_with_walk_y("flood-149-prefabs", 3);

    // Without any declaration: the tide-mill red.
    let bare = campaign("flood-149-bare", Some("\"ocean\""), None);
    let b = build(&bare, &tmp("flood-149-bare-out"), &prefabs);
    let stdout = said(&b);
    assert_eq!(code(&b), 3, "the #149 fixture must be red:\n{stdout}");
    assert!(stdout.contains("DW0364"), "expected DW0364:\n{stdout}");

    // With `flood` declared over the whole drowned piece: still red, and red
    // about the DECLARATION, not silenced.
    let edits = serde_json::json!([{
        "id": "batch/wishful",
        "area": "area/keep",
        "edits": [
            select("region/everything", [0, 0, 0], [10, 5, 10]),
            { "verb": "flood", "region": "region/everything" },
        ]
    }]);
    let camp = campaign("flood-149-declared", Some("\"ocean\""), Some(edits));
    let d = build(&camp, &tmp("flood-149-declared-out"), &prefabs);
    let stdout = said(&d);
    assert_eq!(
        code(&d),
        3,
        "a declared flood must not make #149 green:\n{stdout}"
    );
    assert!(stdout.contains("DW0394"), "expected DW0394:\n{stdout}");
}

/// Red → green over the same content: a shoreline notch cut to the ocean line is
/// `DW0364` (standable ground under the waterline); declaring the sea into it
/// builds clean AND emits the water, so the model and the delivered world agree
/// about that cell instead of leaving it to first-boot fluid ticks.
#[test]
fn shore_notch_is_dw0364_until_the_sea_is_declared_into_it() {
    let prefabs = prefabs_with_walk_y("flood-shore-prefabs", 1);

    let red = campaign(
        "flood-shore-red",
        Some("\"ocean\""),
        Some(shore_notch((5, 5), None)),
    );
    let r = build(&red, &tmp("flood-shore-red-out"), &prefabs);
    let stdout = said(&r);
    assert_eq!(
        code(&r),
        3,
        "an undeclared shore notch must be red:\n{stdout}"
    );
    assert!(stdout.contains("DW0364"), "expected DW0364:\n{stdout}");

    let green = campaign(
        "flood-shore-green",
        Some("\"ocean\""),
        Some(shore_notch((5, 5), Some((5, 5)))),
    );
    let out = tmp("flood-shore-green-out");
    let g = build(&green, &out, &prefabs);
    assert_eq!(
        code(&g),
        0,
        "a declared shoreline must build:\n{}",
        said(&g)
    );
    // The declaration is materialized, not merely believed: the notch cell ships
    // as water, so nothing downstream depends on vanilla flow arriving.
    assert_eq!(
        water_lines(&out).len(),
        1,
        "exactly the reached cell is emitted as water: {:?}",
        water_lines(&out)
    );
}

/// ADR-0006 over the new verb: the flooded fixture builds byte-identically
/// twice. The reach is a `BTreeSet` walk over a fixed neighbour order, so it
/// carries no iteration-order dependence into the emitted `setblock` list.
#[test]
fn flood_build_is_byte_identical_twice() {
    let prefabs = prefabs_with_walk_y("flood-det-prefabs", 1);
    let camp = campaign(
        "flood-det",
        Some("\"ocean\""),
        Some(shore_notch((5, 5), Some((5, 5)))),
    );
    let (a, b) = (tmp("flood-det-a"), tmp("flood-det-b"));
    for out in [&a, &b] {
        let r = build(&camp, out, &prefabs);
        assert_eq!(code(&r), 0, "{}", said(&r));
    }
    let fa = a.join("datapack/data/hello-world/function/world_edits.mcfunction");
    let fb = b.join("datapack/data/hello-world/function/world_edits.mcfunction");
    assert_eq!(
        std::fs::read(&fa).unwrap(),
        std::fs::read(&fb).unwrap(),
        "the flood materialization must be byte-stable"
    );
}

/// `DW0395`: the notch is two cells wide and the envelope covers one of them.
/// The water the declaration admits does not stop inside it — it flows on into
/// an undeclared air cell of the piece, which the model would then ship as dry.
#[test]
fn flood_that_overflows_its_envelope_is_dw0395() {
    let prefabs = prefabs_with_walk_y("flood-escape-prefabs", 1);
    let camp = campaign(
        "flood-escape",
        Some("\"ocean\""),
        Some(shore_notch((5, 6), Some((5, 5)))),
    );
    let r = build(&camp, &tmp("flood-escape-out"), &prefabs);
    let stdout = said(&r);
    assert_eq!(code(&r), 3, "an overflowing flood must be red:\n{stdout}");
    assert!(stdout.contains("DW0395"), "expected DW0395:\n{stdout}");
}

/// `DW0394`: a `flood` in a world whose horizon has no ambient water at all.
/// There is no sea to admit, so the verb is a no-op — and a no-op declaration is
/// a finding, never a silent pass.
#[test]
fn flood_in_a_waterless_horizon_is_dw0394() {
    let edits = serde_json::json!([{
        "id": "batch/dry",
        "area": "area/keep",
        "edits": [
            select("region/void-shore", [0, 0, 0], [10, 1, 10]),
            { "verb": "flood", "region": "region/void-shore" },
        ]
    }]);
    let camp = campaign("flood-void", None, Some(edits));
    let r = build(&camp, &tmp("flood-void-out"), &common::prefabs_dir());
    let stdout = said(&r);
    assert_eq!(code(&r), 3, "flood in a void world must be red:\n{stdout}");
    assert!(stdout.contains("DW0394"), "expected DW0394:\n{stdout}");
    assert!(
        stdout.contains("no ambient water at all"),
        "the message must name the real cause:\n{stdout}"
    );
}

/// The v0.9 version fence (`DW0141`): declaring `flood` in a world-edits stage
/// below 0.9.0 is a reserved-feature rejection, so a pre-0.9 script keeps
/// emitting byte-for-byte what it emitted before this verb existed.
#[test]
fn flood_below_v09_is_reserved() {
    let camp = campaign(
        "flood-fence",
        Some("\"ocean\""),
        Some(shore_notch((5, 5), Some((5, 5)))),
    );
    // Drop the stage back to 0.6.0, where every other verb in this script lives.
    let path = camp.join("world-edits.json");
    let mut doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    doc["dsl_version"] = serde_json::json!("0.6.0");
    std::fs::write(&path, serde_json::to_string_pretty(&doc).unwrap()).unwrap();

    let prefabs = prefabs_with_walk_y("flood-fence-prefabs", 1);
    let r = build(&camp, &tmp("flood-fence-out"), &prefabs);
    let stdout = said(&r);
    assert_eq!(
        code(&r),
        1,
        "a reserved verb is a validation rejection:\n{stdout}"
    );
    assert!(stdout.contains("DW0141"), "expected DW0141:\n{stdout}");
}
