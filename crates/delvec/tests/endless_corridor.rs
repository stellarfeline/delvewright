//! spec-0086 — **an endless corridor.**
//!
//! `tests/fixtures/long-gallery` is the primary: one area, one corridor of six
//! identical bays ([`common::corridor`]) whose view closes inside one bay behind
//! two staggered baffles, and one loop whose slab stands across bay 3's mouth
//! and lands a body on bay 2's, released on the count it raises. Every probe
//! below is that primary plus one declared edit, built through the real
//! `delvec build` entry point, so the validation funnel (`DW0949`) and the build
//! (`DW0945`–`DW0948`, `DW0311`) are each reached the way an author reaches them.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

use common::corridor::{self, Cuts, FLOOR_Y, bay};

/// A private copy of the primary with `quests.json` edited by `patch`.
fn campaign(who: &str, patch: impl FnOnce(&mut Value)) -> PathBuf {
    campaign_with(who, patch, |_| {})
}

/// [`campaign`] with a second edit to any other stage document, by name.
fn campaign_with(who: &str, quests: impl FnOnce(&mut Value), other: impl FnOnce(&Path)) -> PathBuf {
    let src = common::compiler_fixtures_dir().join("long-gallery");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("endless-corridor-{who}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for f in common::STAGE_FILES {
        std::fs::copy(src.join(f), dir.join(f)).unwrap();
    }
    common::patch_file(&dir.join("quests.json"), quests);
    other(&dir);
    dir
}

/// What one `delvec build` run said.
struct Run {
    status: i32,
    stderr: String,
    out: PathBuf,
}

impl Run {
    fn green(&self) -> &Self {
        assert_eq!(self.status, 0, "expected a green build:\n{}", self.stderr);
        self
    }

    fn refused(&self, code: &str) -> String {
        assert_ne!(self.status, 0, "expected {code}, got a green build");
        let line = self
            .stderr
            .lines()
            .find(|l| l.starts_with(code) && l.contains("[error]"))
            .unwrap_or_else(|| panic!("expected {code}:\n{}", self.stderr));
        line.to_string()
    }

    /// The first `[error]` line under `code` whose text contains `needle`.
    fn refused_with(&self, code: &str, needle: &str) -> String {
        assert_ne!(self.status, 0, "expected {code}, got a green build");
        self.stderr
            .lines()
            .find(|l| l.starts_with(code) && l.contains("[error]") && l.contains(needle))
            .unwrap_or_else(|| panic!("expected {code} naming `{needle}`:\n{}", self.stderr))
            .to_string()
    }

    fn json(&self, rel: &str) -> Value {
        serde_json::from_str(
            &std::fs::read_to_string(self.out.join(rel))
                .unwrap_or_else(|e| panic!("{rel}: {e}\n{}", self.stderr)),
        )
        .unwrap()
    }

    fn json_opt(&self, rel: &str) -> Option<Value> {
        std::fs::read_to_string(self.out.join(rel))
            .ok()
            .map(|t| serde_json::from_str(&t).unwrap())
    }

    fn text(&self, rel: &str) -> String {
        std::fs::read_to_string(self.out.join(rel))
            .unwrap_or_else(|e| panic!("{rel}: {e}\n{}", self.stderr))
    }

    fn binding(&self) -> String {
        self.stderr
            .lines()
            .find(|l| l.starts_with("loop binding:"))
            .unwrap_or_else(|| panic!("no loop binding line:\n{}", self.stderr))
            .to_string()
    }
}

fn build_over(dir: &Path, prefabs: &Path) -> Run {
    let out = dir.with_extension("out");
    let _ = std::fs::remove_dir_all(&out);
    let o = Command::new(env!("CARGO_BIN_EXE_delvec"))
        .arg("build")
        .arg(dir)
        .arg("--out")
        .arg(&out)
        .arg("--prefabs")
        .arg(prefabs)
        .output()
        .expect("delvec runs");
    Run {
        status: o.status.code().unwrap_or(-1),
        stderr: format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        ),
        out,
    }
}

fn build(dir: &Path) -> Run {
    let tag = dir.file_name().unwrap().to_string_lossy().to_string();
    build_over(dir, &corridor::gallery_prefabs(&tag, &Cuts::default()))
}

fn build_cut(dir: &Path, cuts: &Cuts) -> Run {
    let tag = dir.file_name().unwrap().to_string_lossy().to_string();
    build_over(dir, &corridor::gallery_prefabs(&tag, cuts))
}

fn loop_mut(q: &mut Value) -> &mut Value {
    &mut q["content"]["loops"][0]
}

/// The loop step `critical-path.json` carries.
fn loop_step(path: &Value) -> Option<Value> {
    path["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["action"] == "loop")
        .cloned()
}

#[test]
fn the_primary_builds_and_exercises_the_loop() {
    let run = build(&campaign("primary", |_| {}));
    run.green();
    let path = run.json("critical-path.json");
    let step = loop_step(&path).expect("the path exercises the loop");
    assert_eq!(step["loop"], "loop/gallery");
    assert_eq!(step["offset"], json!([0, 0, -6]));
    assert_eq!(step["times"], 2);
    let b = run.binding();
    assert!(b.contains("open faces 0"), "{b}");
    assert!(b.contains("forced route meets 1 of 1 holding"), "{b}");
    let _ = (FLOOR_Y, bay(0));
}

// ---------------------------------------------------------------------------
// Criterion 1 — the surface
// ---------------------------------------------------------------------------

#[test]
fn the_schema_exports_loops_with_every_field() {
    let o = Command::new(env!("CARGO_BIN_EXE_delvec"))
        .args(["schema", "--stage", "all"])
        .output()
        .expect("delvec runs");
    assert!(o.status.success());
    let all: Value = serde_json::from_slice(&o.stdout).unwrap();
    let q = &all["quests"];
    let defs = q.get("definitions").or_else(|| q.get("$defs")).unwrap();
    let props: Vec<&str> = defs["Loop"]["properties"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    for f in [
        "id",
        "region",
        "to",
        "requires_flags",
        "forbids_flags",
        "requires_state",
        "counts",
        "on_cross",
    ] {
        assert!(props.contains(&f), "`loops[]` exports `{f}`: {props:?}");
    }
    assert_eq!(props.len(), 8, "{props:?}");
}

// ---------------------------------------------------------------------------
// Criterion 2 — the offset is derived
// ---------------------------------------------------------------------------

#[test]
fn the_tp_line_carries_the_derived_offset_with_and_without_a_mark_offset() {
    let run = build(&campaign("offset-anchor", |_| {}));
    run.green();
    let f = run.text("datapack/data/long-gallery/function/loop_gallery.mcfunction");
    assert!(f.contains("tp @s ~0 ~0 ~-6\n"), "{f}");
    // `to` spelled as the slab's own anchor plus an offset two bays back: the
    // offset is still the difference of two cells, now -12.
    let run = build(&campaign("offset-mark", |q| {
        loop_mut(q)["to"] = json!({ "anchor": "anchor/slab", "offset": [0, 0, -12] });
    }));
    run.green();
    let f = run.text("datapack/data/long-gallery/function/loop_gallery.mcfunction");
    assert!(f.contains("tp @s ~0 ~0 ~-12\n"), "{f}");
}

#[test]
fn a_landing_outside_its_anchors_piece_is_dw0897() {
    let run = build(&campaign("offset-out", |q| {
        loop_mut(q)["to"] = json!({ "anchor": "anchor/slab", "offset": [0, 0, -60] });
    }));
    let line = run.refused("DW0897");
    assert!(line.contains("loop/gallery"), "{line}");
}

// ---------------------------------------------------------------------------
// Criterion 3 — the slab rule
// ---------------------------------------------------------------------------

#[test]
fn dw0945_a_zero_offset_is_not_a_slab() {
    let run = build(&campaign("slab-zero", |q| {
        loop_mut(q)["to"] = json!({ "anchor": "anchor/slab" });
    }));
    let line = run.refused("DW0945");
    assert!(line.contains("the offset is zero"), "{line}");
}

#[test]
fn dw0945_an_offset_along_two_axes_is_not_a_slab() {
    let run = build(&campaign("slab-two-axes", |q| {
        loop_mut(q)["to"] = json!({ "anchor": "anchor/slab", "offset": [1, 0, -6] });
    }));
    let line = run.refused("DW0945");
    assert!(line.contains("more than one axis"), "{line}");
}

#[test]
fn dw0945_a_move_that_does_not_clear_the_slab() {
    let run = build(&campaign("slab-short", |q| {
        loop_mut(q)["region"]["extent"] = json!([1, 1, 1]);
        loop_mut(q)["to"] = json!({ "anchor": "anchor/slab", "offset": [0, 0, -2] });
    }));
    let line = run.refused("DW0945");
    assert!(line.contains("does not clear the slab"), "{line}");
    assert!(line.contains("t = 3"), "{line}");
}

#[test]
fn dw0945_a_vertical_slab_one_cell_thick_cannot_catch_a_fall() {
    let run = build(&campaign("slab-thin", |q| {
        loop_mut(q)["region"]["extent"] = json!([1, 0, 0]);
        loop_mut(q)["to"] = json!({ "anchor": "anchor/slab", "offset": [0, -1, 0] });
    }));
    let line = run.refused("DW0945");
    assert!(
        line.contains("too thin to catch a crossing along y"),
        "{line}"
    );
    assert!(
        line.contains("3.92"),
        "the fall law's limit is named: {line}"
    );
}

#[test]
fn dw0945_a_slab_over_another_loops_slab() {
    let run = build(&campaign("slab-twice", |q| {
        let mut second = loop_mut(q).clone();
        second["id"] = json!("loop/gallery-again");
        q["content"]["loops"].as_array_mut().unwrap().push(second);
    }));
    let line = run.refused("DW0945");
    assert!(line.contains("overlaps loop `loop/gallery"), "{line}");
}

#[test]
fn dw0945_a_slab_over_a_lethal_keep_out() {
    // A visible hazard, so `DW0891` is satisfied and the slab rule is what
    // speaks: magma under every cell the keep-out catches.
    let mut blocks = Vec::new();
    for x in 2..=3 {
        for z in bay(3)..=bay(3) + 1 {
            blocks.push(([x, FLOOR_Y - 1, z], "minecraft:magma_block"));
        }
    }
    let run = build_cut(
        &campaign("slab-lethal", |q| {
            q["content"]["lethal_volumes"] = json!([{
                "id": "lethal/under-the-slab",
                "message": "The floor gives way.",
                "region": { "anchor": "anchor/in-the-slab", "extent": [0, 0, 0] },
                "shown_by": ["minecraft:magma_block"]
            }]);
        }),
        &Cuts {
            blocks,
            ..Cuts::default()
        },
    );
    let line = run.refused("DW0945");
    assert!(
        line.contains("lethal volume `lethal/under-the-slab`"),
        "{line}"
    );
}

/// A horizontal slab one course tall is crossed by a body that jumps: its feet
/// rise above the slab's top face for several ticks, the selector stops
/// meeting its hitbox, and it passes over unmoved.
#[test]
fn dw0945_a_slab_a_jump_clears() {
    let run = build(&campaign("slab-low", |q| {
        loop_mut(q)["region"] = json!({ "anchor": "anchor/in-the-slab", "extent": [0, 0, 0] });
        loop_mut(q)["to"] = json!({ "anchor": "anchor/in-the-slab", "offset": [0, 0, -6] });
    }));
    let line = run.refused("DW0945");
    assert!(line.contains("too low to catch a jumping body"), "{line}");
    assert!(line.contains("16/16") && line.contains("20/16"), "{line}");
}

#[test]
fn dw0945_a_slab_cell_a_body_cannot_be_in() {
    let run = build(&campaign("slab-wall", |q| {
        loop_mut(q)["region"]["extent"] = json!([2, 1, 0]);
    }));
    let line = run.refused("DW0945");
    assert!(line.contains("is not passable"), "{line}");
}

/// The two poll bounds are the rig's readings, held where the rule reads them.
#[test]
fn the_poll_bounds_are_the_rigs_readings() {
    assert_eq!(
        delvewright_dsl::metrics::POLL_HORIZONTAL_BLOCKS_PER_TICK,
        0.5878
    );
    assert_eq!(delvewright_dsl::metrics::POLL_FALL_BLOCKS_PER_TICK, 3.92);
    let src =
        std::fs::read_to_string(common::repo_root().join("crates/dsl/src/metrics.rs")).unwrap();
    for name in [
        "POLL_HORIZONTAL_BLOCKS_PER_TICK",
        "POLL_FALL_BLOCKS_PER_TICK",
    ] {
        let at = src.find(&format!("pub const {name}")).unwrap();
        let doc = &src[src[..at].rfind("\n\n").unwrap()..at];
        assert!(
            doc.contains("tools/spike-seamless-loop/"),
            "{name}'s doc names the rig as its instrument"
        );
    }
    // The readings themselves, as the rig recorded them.
    let obs = std::fs::read_to_string(
        common::repo_root().join("tools/spike-seamless-loop/observations.json"),
    )
    .unwrap();
    assert!(
        obs.contains("0.5878") || obs.contains("0.587"),
        "the sprint-jump tick"
    );
}

// ---------------------------------------------------------------------------
// Criterion 4 — a closed view
// ---------------------------------------------------------------------------

#[test]
fn dw0947_a_wall_removed_so_the_view_leaves_the_corridor() {
    let z = bay(2);
    let run = build_cut(
        &campaign("open-wall", |_| {}),
        &Cuts {
            // A window at eye height: a body cannot stand in it, so the only
            // thing it changes is what the landing's eyes can see.
            air: vec![[0, FLOOR_Y + 1, z + 1]],
            ..Cuts::default()
        },
    );
    let line = run.refused("DW0947");
    assert!(line.contains("the eye at"), "names an eye: {line}");
    assert!(line.contains("fog end 1024"), "names its fog end: {line}");
    assert!(
        line.contains("sees the open cell"),
        "names the open cell: {line}"
    );
    assert!(run.binding().contains("open faces 1"), "{}", run.binding());
}

#[test]
fn dw0947_a_roof_cell_of_the_landing_bay_removed_opens_the_sky() {
    let run = build_cut(
        &campaign("open-roof", |_| {}),
        &Cuts {
            air: vec![[1, corridor::SIZE[1] - 1, bay(2)]],
            ..Cuts::default()
        },
    );
    let line = run.refused("DW0947");
    assert!(line.contains("open sky"), "{line}");
}

/// A straight hall — no baffle, its far end open onto nothing — is open,
/// and every eye reads the attribute's default fog end: this tree paints no
/// atmosphere (spec-0086 §4.7).
#[test]
fn dw0947_a_straight_hall_with_no_atmosphere_reads_fog_end_1024() {
    let mut air = Vec::new();
    for k in 0..corridor::BAYS {
        for y in FLOOR_Y..FLOOR_Y + 3 {
            for x in 1..=3 {
                air.push([x, y, bay(k) + 2]);
                air.push([x, y, bay(k) + 5]);
            }
        }
    }
    // …and a window at eye height in the far wall, onto nothing.
    for x in 1..=3 {
        air.push([x, FLOOR_Y + 1, corridor::SIZE[2] - 1]);
    }
    let run = build_cut(
        &campaign("straight-hall", |_| {}),
        &Cuts {
            air,
            ..Cuts::default()
        },
    );
    let line = run.refused("DW0947");
    assert!(line.contains("fog end 1024 blocks"), "{line}");
    assert!(
        run.binding().contains("fog end 1024..1024"),
        "{}",
        run.binding()
    );
}

/// The straight hall of [`dw0947_a_straight_hall_with_no_atmosphere_reads_fog_end_1024`]:
/// both baffles of every bay cut, and a window at eye height in the far wall.
fn straight_hall_air() -> Vec<[i32; 3]> {
    let mut air = Vec::new();
    for k in 0..corridor::BAYS {
        for y in FLOOR_Y..FLOOR_Y + 3 {
            for x in 1..=3 {
                air.push([x, y, bay(k) + 2]);
                air.push([x, y, bay(k) + 5]);
            }
        }
    }
    for x in 1..=3 {
        air.push([x, FLOOR_Y + 1, corridor::SIZE[2] - 1]);
    }
    air
}

/// The fixture's world with one fogged atmosphere, carried by the gallery's
/// area from the first tick (spec-0080).
fn fogged(dir: &Path) {
    common::patch_file(&dir.join("world.json"), |w| {
        w["content"]["atmospheres"] = json!([{
            "id": "atmosphere/murk",
            "precipitation": "none",
            "attributes": { "visual/fog_end_distance": 3.0 }
        }]);
        w["content"]["areas"][0]["atmosphere"] = json!("atmosphere/murk");
    });
}

/// The corridor written into a piece `pad` cells wider on each side along x —
/// void around it — so the area's paint, which is the piece's bounds, reaches
/// past every eye by more than the client's blend ([`delvec::compiler::horizon::BLEND_REACH`]
/// is 12). Every cell and anchor shifts by `pad` along x.
fn padded_prefabs(tag: &str, cuts: &Cuts, pad: i32) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("long-gallery-padded-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    let cells: Vec<([i32; 3], &str)> = corridor::gallery_cells(cuts)
        .into_iter()
        .map(|(c, b)| ([c[0] + pad, c[1], c[2]], b))
        .collect();
    let mut anchors = corridor::gallery_anchors();
    for (_, a) in anchors.as_object_mut().unwrap().iter_mut() {
        let x = a["pos"][0].as_i64().unwrap();
        a["pos"][0] = json!(x + i64::from(pad));
    }
    let [sx, sy, sz] = corridor::SIZE;
    common::write_single_prefab(
        &dir,
        "long-gallery",
        [sx + 2 * pad, sy, sz],
        &cells,
        anchors,
    );
    common::declare_shown_faces(&dir, "long-gallery");
    dir
}

/// spec-0086 criterion 4 × spec-0080: **a straight hall under a fogged
/// atmosphere builds** — the area's paint reaches past every eye by more than
/// the blend, so each eye reads the declared fog end whole and the view closes
/// in the fog inside the span; the binding's fog range is the declared one.
#[test]
fn a_straight_hall_under_a_fogged_atmosphere_builds() {
    let dir = campaign_with("straight-hall-fogged", |_| {}, fogged);
    let cuts = Cuts {
        air: straight_hall_air(),
        ..Cuts::default()
    };
    let run = build_over(&dir, &padded_prefabs("fogged", &cuts, 16));
    run.green();
    let binding = run.binding();
    assert!(binding.contains("fog end 3..3"), "{binding}");
}

/// …and **the same hall with its paint cut to the play space is refused**:
/// the unpadded piece's paint ends within the blend of every eye, so each eye
/// reads the atmosphere mixed with the unpainted default, and the refusal
/// prints the kernel-weighted fog end it read — a number between the declared
/// 3 and the default 1024, never either.
#[test]
fn dw0947_the_same_hall_with_its_paint_cut_to_the_play_space() {
    let dir = campaign_with("straight-hall-fog-cut", |_| {}, fogged);
    let run = build_cut(
        &dir,
        &Cuts {
            air: straight_hall_air(),
            ..Cuts::default()
        },
    );
    let line = run.refused("DW0947");
    let fog: f64 = line
        .split("(fog end ")
        .nth(1)
        .and_then(|t| t.split(" blocks)").next())
        .and_then(|t| t.parse().ok())
        .unwrap_or_else(|| panic!("the refusal prints the eye's fog end: {line}"));
    assert!(
        fog > 3.0 && fog < 1024.0,
        "the fog end read is the kernel-weighted mix, not either pure value: {fog}\n{line}"
    );
}

// ---------------------------------------------------------------------------
// Criterion 5 — identical blocks and light
// ---------------------------------------------------------------------------

#[test]
fn dw0946_a_lantern_missing_from_the_landing_bay() {
    let lamp = [2, corridor::SIZE[1] - 2, bay(2) + 1];
    let run = build_cut(
        &campaign("tiling-lamp", |_| {}),
        &Cuts {
            air: vec![lamp],
            ..Cuts::default()
        },
    );
    let line = run.refused("DW0946");
    assert!(line.contains("lantern"), "names both states: {line}");
    assert!(line.contains("minecraft:air"), "names both states: {line}");
}

#[test]
fn dw0946_a_lamp_behind_a_window_lights_one_bay() {
    let run = build_cut(
        &campaign("tiling-light", |_| {}),
        &Cuts {
            blocks: vec![([5, FLOOR_Y + 1, bay(1) + 3], "minecraft:lantern")],
            ..Cuts::default()
        },
    );
    let line = run.refused("DW0946");
    assert!(line.contains("is lit"), "{line}");
    assert!(line.contains("reachable sky"), "names the sky: {line}");

    // Both levels are the one light flood's (`light::LightModel::flood`, the
    // flood the relight pass runs): recomputed here over the same piece, at the
    // primary's own origin, they are the numbers the message printed.
    let primary = build(&campaign("tiling-light-primary", |_| {}));
    primary.green();
    let step = loop_step(&primary.json("critical-path.json")).unwrap();
    let o = corridor::origin_from_step(&step);
    let o = [o[0] as i32, o[1] as i32, o[2] as i32];
    let cells: std::collections::BTreeMap<[i32; 3], String> = corridor::gallery_cells(&Cuts {
        blocks: vec![([5, FLOOR_Y + 1, bay(1) + 3], "minecraft:lantern")],
        ..Cuts::default()
    })
    .into_iter()
    .map(|(c, b)| ([c[0] + o[0], c[1] + o[1], c[2] + o[2]], b.to_string()))
    .collect();
    let s = corridor::SIZE;
    let lit = delvec::compiler::light::LightModel::from_blocks_within(
        cells,
        o,
        [o[0] + s[0] - 1, o[1] + s[1] - 1, o[2] + s[2] - 1],
    )
    .flood(15);
    let nums: Vec<i64> = line
        .split(|ch: char| !(ch.is_ascii_digit() || ch == '-'))
        .filter(|t| !t.is_empty() && *t != "-")
        .filter_map(|t| t.parse().ok())
        .collect();
    // `loop/gallery`: … cell [x, y, z] is lit A … slab, [x, y, z], is lit B …
    let at = nums.iter().position(|&n| n == 946).map_or(0, |i| i + 1);
    let n = &nums[at..];
    let (c1, a, c2, b) = (
        [n[0] as i32, n[1] as i32, n[2] as i32],
        n[3],
        [n[4] as i32, n[5] as i32, n[6] as i32],
        n[7],
    );
    assert_eq!(i64::from(*lit.get(&c1).unwrap_or(&0)), a, "{line}");
    assert_eq!(i64::from(*lit.get(&c2).unwrap_or(&0)), b, "{line}");
    assert_ne!(a, b);
}

// ---------------------------------------------------------------------------
// Criterion 8 — the gate
// ---------------------------------------------------------------------------

#[test]
fn dw0949_a_loop_with_no_gate_term() {
    let run = build(&campaign("gate-none", |q| {
        loop_mut(q)
            .as_object_mut()
            .unwrap()
            .remove("requires_state");
    }));
    let line = run.refused("DW0949");
    assert!(line.contains("declares no gate term"), "{line}");
}

#[test]
fn dw0949_a_player_datum_in_the_gate() {
    let run = build(&campaign("gate-player", |q| {
        q["content"]["state"]
            .as_array_mut()
            .unwrap()
            .push(json!({ "id": "state/mine", "scope": "player" }));
        loop_mut(q)["requires_state"]
            .as_array_mut()
            .unwrap()
            .push(json!({ "state": "state/mine", "op": "at-most", "value": 3 }));
    }));
    let line = run.refused("DW0949");
    assert!(line.contains("state/mine"), "{line}");
    assert!(line.contains("requires_state/1"), "names the term: {line}");
}

#[test]
fn dw0949_a_player_datum_counted() {
    let run = build(&campaign("gate-counts", |q| {
        q["content"]["state"][0]["scope"] = json!("player");
    }));
    run.refused_with("DW0949", "counts its crossings into `state/crossings`");
}

#[test]
fn dw0949_a_teleport_in_on_cross() {
    let run = build(&campaign("gate-teleport", |q| {
        loop_mut(q)["on_cross"] = json!([{
            "type": "teleport",
            "from": { "anchor": "anchor/landing", "extent": [1, 1, 0] },
            "to": { "anchor": "anchor/porch" }
        }]);
    }));
    let line = run.refused("DW0949");
    assert!(line.contains("on_cross/0"), "names the term: {line}");
}

// ---------------------------------------------------------------------------
// Criterion 6 — configurations
// ---------------------------------------------------------------------------

/// `fill-region` with the west wall's box: the whole wall, or bay 2's alone.
fn crack(anchor: &str, half: i64, when: Option<i64>) -> Value {
    let mut e = json!({
        "type": "fill-region",
        "region": { "anchor": anchor, "extent": [0, 1, half] },
        "block": "minecraft:cracked_stone_bricks"
    });
    if let Some(n) = when {
        e["when"] = json!({ "requires_state": [
            { "state": "state/crossings", "op": "equals", "value": n }
        ]});
    }
    e
}

#[test]
fn an_on_cross_that_cracks_every_wall_at_the_second_crossing_tiles() {
    let half = i64::from((bay(corridor::BAYS) - 2 - bay(0)) / 2);
    let run = build(&campaign("cfg-every-wall", |q| {
        loop_mut(q)["on_cross"] = json!([crack("anchor/west-wall", half, Some(2))]);
    }));
    run.green();
    let gate = run.json("validation/loop-gate.json");
    assert_eq!(
        gate["rows"][0]["configurations"], 2,
        "the base and crossing 2"
    );
    let cross = run.text("datapack/data/long-gallery/function/loop_gallery_cross.mcfunction");
    assert!(cross.contains("cracked_stone_bricks"), "{cross}");
}

#[test]
fn dw0946_an_on_cross_that_cracks_the_landing_bay_alone() {
    let run = build(&campaign("cfg-landing-wall", |q| {
        loop_mut(q)["on_cross"] = json!([crack("anchor/west-wall-landing", 2, Some(2))]);
    }));
    let line = run.refused("DW0946");
    assert!(
        line.contains("after crossing 2 of the exercise step"),
        "names the configuration: {line}"
    );
    assert!(
        line.contains("`fill-region` at `/content/loops/0/on_cross/0`"),
        "names the write and its root: {line}"
    );
}

#[test]
fn dw0946_an_objectives_fill_that_reaches_one_bay() {
    let run = build(&campaign("cfg-objective", |q| {
        let quest = &mut q["content"]["quests"][0];
        quest["objectives"].as_array_mut().unwrap().insert(
            0,
            json!({
                "anchor": "anchor/porch",
                "happening": { "text": "the party steps into the porch", "verb": "arrives" },
                "id": "obj/porch",
                "radius": 1,
                "type": "reach-anchor"
            }),
        );
        quest["objectives"][1]["after"] = json!(["obj/porch"]);
        quest["on_objective_complete"] = json!({
            "obj/porch": [crack("anchor/west-wall-landing", 2, None)]
        });
    }));
    let line = run.refused("DW0946");
    assert!(
        line.contains(
            "after `fill-region` at `/content/quests/0/on_objective_complete/obj/porch/0`"
        ),
        "names the write, its root and the configuration after that objective: {line}"
    );
}

// ---------------------------------------------------------------------------
// Criterion 7 — volumes and bodies
// ---------------------------------------------------------------------------

/// A pit two courses deep at offset 4 of bay `k`, against the passage's east
/// side and off the route, with a killing volume at its bottom whose keep-out
/// stops under the walk plane — a hazard that shows itself as a hole.
fn pit_cells(k: i32) -> Vec<[i32; 3]> {
    vec![[3, FLOOR_Y - 1, bay(k) + 4], [3, FLOOR_Y - 2, bay(k) + 4]]
}

fn pit_volume(k: i32) -> Value {
    json!({
        "id": format!("lethal/pit-{k}"),
        "message": "The pit takes you.",
        "region": { "anchor": format!("anchor/pit-{k}"), "extent": [0, 0, 0] }
    })
}

/// The primary with a magma strip in every bay but `skip`, each anchored by
/// its own prefab anchor.
fn magma_campaign(who: &str, skip: Option<i32>) -> (PathBuf, PathBuf) {
    let mut cuts = Cuts::default();
    let mut vols = Vec::new();
    for k in 0..corridor::BAYS {
        cuts.air.extend(pit_cells(k));
        if Some(k) != skip {
            vols.push(pit_volume(k));
        }
    }
    let dir = campaign(who, |q| {
        q["content"]["lethal_volumes"] = Value::Array(vols);
    });
    let prefabs = corridor::gallery_prefabs(&format!("magma-{who}"), &cuts);
    // The volumes' anchors, added to the piece's document.
    let meta = prefabs.join("long-gallery.json");
    let mut doc: Value = serde_json::from_str(&std::fs::read_to_string(&meta).unwrap()).unwrap();
    for k in 0..corridor::BAYS {
        doc["anchors"][format!("anchor/pit-{k}")] = json!({ "pos": [3, FLOOR_Y - 2, bay(k) + 4] });
    }
    std::fs::write(&meta, serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();
    (dir, prefabs)
}

#[test]
fn dw0946_a_volume_under_the_slabs_bay_without_one_under_the_landings() {
    let (dir, prefabs) = magma_campaign("vol-missing", Some(2));
    let run = build_over(&dir, &prefabs);
    let line = run.refused("DW0946");
    assert!(
        line.contains("lethal volume `lethal/pit-"),
        "names the volume: {line}"
    );
    assert!(line.contains("without its image"), "{line}");
}

#[test]
fn a_volume_under_every_bay_tiles_and_the_population_gains_the_landing() {
    let (dir, prefabs) = magma_campaign("vol-every", None);
    let run = build_over(&dir, &prefabs);
    run.green();
    let b = run.binding();
    assert!(b.contains("volumes in span"), "{b}");
    let gate = run.json("validation/loop-gate.json");
    assert!(
        gate["rows"][0]["volumes_in_span"].as_u64().unwrap() > 0,
        "{gate}"
    );
    // The population is flooded from every place the party is put: the loop
    // adds its landing slab (9 cells) and the exercise step's landing (1).
    let with = run.json("validation/lethal-gate.json")["danger_visibility"]["roots"]
        .as_u64()
        .unwrap();
    let dir2 = campaign("vol-every-no-loop", |q| {
        let vols = q["content"]["lethal_volumes"].clone();
        q["content"].as_object_mut().unwrap().remove("loops");
        q["content"].as_object_mut().unwrap().remove("state");
        q["content"]["lethal_volumes"] = vols;
    });
    // The same volumes, the same piece, no loop.
    let (with_vols, _) = magma_campaign("vol-every-copy", None);
    std::fs::copy(with_vols.join("quests.json"), dir2.join("quests.json")).unwrap();
    common::patch_file(&dir2.join("quests.json"), |q| {
        q["content"].as_object_mut().unwrap().remove("loops");
        q["content"].as_object_mut().unwrap().remove("state");
    });
    let run2 = build_over(&dir2, &prefabs);
    run2.green();
    let without = run2.json("validation/lethal-gate.json")["danger_visibility"]["roots"]
        .as_u64()
        .unwrap();
    assert_eq!(
        with,
        without + 10,
        "landing slab and exercise landing join the roots"
    );
}

#[test]
fn dw0948_a_figure_posted_in_the_hall() {
    let dir = campaign_with(
        "body-npc",
        |_| {},
        |d| {
            common::patch_file(&d.join("npcs.json"), |n| {
                n["content"]["npcs"] = json!([{
                    "anchor": "anchor/in-the-hall",
                    "area": "area/gallery",
                    "base_entity": "minecraft:villager",
                    "id": "npc/curator",
                    "name": "The Curator",
                    "persona": {
                        "archetype": "patient keeper",
                        "backstory": "She has walked this gallery longer than it has had an end.",
                        "demeanor": "Calm.",
                        "motivation": "Keep the gallery tidy.",
                        "secret": "She has never reached the far door.",
                        "speech_style": "Short sentences."
                    },
                    "role": "flavor"
                }]);
            });
            common::patch_file(&d.join("quests.json"), |q| {
                q["content"]["quests"][0]["cast"] = json!({
                    "npc/curator": {
                        "at": "anchor/in-the-hall",
                        "dialogue": "dlg/hello",
                        "doing": "keeping to the second bay"
                    }
                });
            });
            common::patch_file(&d.join("dialogue.json"), |t| {
                t["content"]["dialogues"] = json!([{
                    "npc": "npc/curator",
                    "root": "dlg/hello",
                    "nodes": [ { "id": "dlg/hello", "text": "Mind the lamps.", "options": [] } ]
                }]);
            });
        },
    );
    let run = build(&dir);
    let line = run.refused("DW0948");
    assert!(line.contains("npc `npc/curator`"), "names the body: {line}");
    assert_eq!(run.json_opt("validation/loop-gate.json"), None);
    // The npc's post and the quest's cast placement both stand it there.
    assert!(
        run.binding().contains("bodies in span 2"),
        "{}",
        run.binding()
    );
}

#[test]
fn dw0542_a_bonfire_seat_inside_the_slab_names_the_loop() {
    let run = build(&campaign_with(
        "seat-in-slab",
        |q| {
            let quest = &mut q["content"]["quests"][0];
            quest["objectives"].as_array_mut().unwrap().insert(
                0,
                json!({
                    "anchor": "anchor/porch",
                    "happening": { "text": "the party steps into the porch", "verb": "arrives" },
                    "id": "obj/porch",
                    "radius": 1,
                    "type": "reach-anchor"
                }),
            );
            quest["objectives"][1]["after"] = json!(["obj/porch"]);
            quest["on_objective_complete"] = json!({
                "obj/porch": [{
                    "type": "bonfire",
                    "anchor": "anchor/in-the-slab",
                    "happening": { "text": "a fire is lit in the gallery", "verb": "opens" }
                }]
            });
        },
        |d| {
            common::patch_file(&d.join("classes.json"), |c| {
                c["content"]["classes"][0]["kit"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({ "count": 2, "item": "minecraft:honey_bottle", "flask": true }));
            });
        },
    ));
    let line = run.refused("DW0542");
    assert!(
        line.contains("loop `loop/gallery`"),
        "names the loop: {line}"
    );
}

// ---------------------------------------------------------------------------
// Criterion 9 — the seal
// ---------------------------------------------------------------------------

/// The primary released by a flag `obj/<objective>` sets at `anchor`, with
/// `obj/end` after it.
fn released_by_a_bell(who: &str, anchor: &str) -> PathBuf {
    campaign(who, |q| {
        let l = loop_mut(q);
        l.as_object_mut().unwrap().remove("requires_state");
        l.as_object_mut().unwrap().remove("counts");
        l["forbids_flags"] = json!(["flag/rung"]);
        q["content"]["state"] = json!([]);
        let quest = &mut q["content"]["quests"][0];
        quest["objectives"].as_array_mut().unwrap().insert(
            0,
            json!({
                "anchor": anchor,
                "happening": { "text": "the party rings the bell", "verb": "arrives" },
                "id": "obj/bell",
                "radius": 1,
                "type": "reach-anchor"
            }),
        );
        quest["objectives"][1]["after"] = json!(["obj/bell"]);
        quest["on_objective_complete"] = json!({
            "obj/bell": [{
                "type": "set-flag",
                "flag": "flag/rung",
                "happening": { "text": "the bell is rung", "verb": "learns" }
            }]
        });
    })
}

#[test]
fn dw0311_a_route_across_the_slab_before_the_release() {
    let run = build(&released_by_a_bell("seal-late", "anchor/bell"));
    let line = run.refused("DW0311");
    assert!(
        line.contains("loop `loop/gallery`"),
        "names the loop: {line}"
    );
    assert!(
        line.contains("forbids_flags: flag/rung"),
        "names the open term: {line}"
    );
    assert!(
        line.contains("critical-path step"),
        "names the step it was read at: {line}"
    );
    for absent in ["doorway", "gap", "fence"] {
        assert!(
            !line.contains(absent),
            "no walk prescription (`{absent}`): {line}"
        );
    }
}

#[test]
fn the_release_moved_before_the_crossing_builds_green_and_the_loop_is_unmet() {
    let run = build(&released_by_a_bell("seal-early", "anchor/porch"));
    run.green();
    assert!(loop_step(&run.json("critical-path.json")).is_none());
    let warn = run
        .stderr
        .lines()
        .find(|l| l.starts_with("DW0950"))
        .unwrap_or_else(|| panic!("DW0950 is advised:\n{}", run.stderr));
    assert!(warn.contains("[warning]"), "{warn}");
    assert!(warn.contains("loop/gallery"), "{warn}");
    assert!(
        run.binding().contains("forced route meets 0 of 1"),
        "{}",
        run.binding()
    );
}

#[test]
fn a_count_release_crosses_twice_and_credits_both_crossings() {
    let run = build(&campaign("seal-count", |q| {
        loop_mut(q)["on_cross"] = json!([
            { "type": "play-sound", "sound": "minecraft:block.bell.use",
              "when": { "requires_state": [ { "state": "state/crossings", "op": "equals", "value": 1 } ] } },
            { "type": "play-sound", "sound": "minecraft:block.bell.resonate",
              "when": { "requires_state": [ { "state": "state/crossings", "op": "equals", "value": 2 } ] } }
        ]);
    }));
    run.green();
    let step = loop_step(&run.json("critical-path.json")).unwrap();
    assert_eq!(step["times"], 2);
    // Both crossings' writes, read off the loop replay the plan built the path
    // with: the first crossing fires effect 0 and the second effect 1.
    let dir = campaign("seal-count-plan", |q| {
        loop_mut(q)["on_cross"] = json!([
            { "type": "play-sound", "sound": "minecraft:block.bell.use",
              "when": { "requires_state": [ { "state": "state/crossings", "op": "equals", "value": 1 } ] } },
            { "type": "play-sound", "sound": "minecraft:block.bell.resonate",
              "when": { "requires_state": [ { "state": "state/crossings", "op": "equals", "value": 2 } ] } }
        ]);
    });
    let c = common::campaign_at(&dir);
    let prefabs = corridor::gallery_prefabs("seal-count-plan", &Cuts::default());
    let reg = delvec::compiler::registry::PrefabRegistry::load_dir(&prefabs).unwrap();
    let plan = delvec::compiler::plan::Plan::build(&c, &reg).unwrap();
    let ex = &plan.loop_exercises[0];
    assert_eq!(ex.times, 2);
    assert!(ex.releases);
    assert_eq!(ex.fired, vec![vec![0], vec![1]]);
}

// ---------------------------------------------------------------------------
// Criterion 10 — the exercise step and one enumeration
// ---------------------------------------------------------------------------

#[test]
fn the_leg_after_the_step_begins_at_the_landing_and_moves_with_to() {
    let run = build(&campaign("step-landing", |_| {}));
    run.green();
    let step = loop_step(&run.json("critical-path.json")).unwrap();
    for k in ["pos", "cross", "offset", "times", "transport"] {
        assert!(step.get(k).is_some(), "the step carries `{k}`: {step}");
    }
    let wp = run.json("validation/critical-path-waypoints.json");
    let legs = wp["legs"].as_array().unwrap();
    assert_eq!(legs[1]["from"], step["transport"], "{wp}");

    // Perturb `to` two bays back: the proof stays green and the leg moves.
    let moved = build(&campaign("step-landing-moved", |q| {
        loop_mut(q)["to"] = json!({ "anchor": "anchor/two-back" });
    }));
    moved.green();
    let step2 = loop_step(&moved.json("critical-path.json")).unwrap();
    assert_ne!(step2["transport"], step["transport"]);
    let wp2 = moved.json("validation/critical-path-waypoints.json");
    assert_eq!(wp2["legs"][1]["from"], step2["transport"], "{wp2}");
}

// ---------------------------------------------------------------------------
// Criterion 11 — pockets
// ---------------------------------------------------------------------------

#[test]
fn no_pocket_in_the_span_while_the_loop_holds() {
    let run = build(&campaign("pocket-none", |_| {}));
    run.green();
    let leave = run.json("validation/leave-proof.json");
    assert_eq!(leave["trapped"], 0, "{leave}");
    assert!(
        leave["configurations"].as_u64().unwrap() >= 2,
        "held and released: {leave}"
    );
}

#[test]
fn dw0921_a_genuine_pocket_in_the_span_names_the_slabs_state() {
    let mut air = Vec::new();
    for k in 0..corridor::BAYS {
        air.push([3, FLOOR_Y - 1, bay(k) + 4]);
        air.push([3, FLOOR_Y - 2, bay(k) + 4]);
    }
    let run = build_cut(
        &campaign("pocket-pit", |_| {}),
        &Cuts {
            air,
            ..Cuts::default()
        },
    );
    let line = run.refused("DW0921");
    assert!(
        line.contains("with the slab of loop `loop/gallery`"),
        "{line}"
    );
}

// ---------------------------------------------------------------------------
// Criteria 13 and 14 — emission, PackTest, binding, ledger
// ---------------------------------------------------------------------------

#[test]
fn the_poll_the_move_and_the_answer_are_emitted_in_order() {
    let run = build(&campaign("emit", |q| {
        loop_mut(q)["on_cross"] = json!([
            { "type": "play-sound", "sound": "minecraft:block.bell.use" }
        ]);
    }));
    run.green();
    let ns = "long-gallery";
    let tick = run.text(&format!("datapack/data/{ns}/function/tick.mcfunction"));
    assert!(
        tick.contains(&format!("function {ns}:loop_gallery_poll\n")),
        "{tick}"
    );
    let poll = run.text(&format!(
        "datapack/data/{ns}/function/loop_gallery_poll.mcfunction"
    ));
    assert!(
        poll.starts_with("execute if score #party dw.s_crossings matches ..1 as @e["),
        "{poll}"
    );
    for term in [
        "x=",
        "dx=",
        "tag=!dw_fixture",
        "tag=!dw_cutscene",
        &format!("run function {ns}:loop_gallery\n"),
    ] {
        assert!(poll.contains(term), "the poll carries `{term}`: {poll}");
    }
    let body = run.text(&format!(
        "datapack/data/{ns}/function/loop_gallery.mcfunction"
    ));
    let count = body
        .find("scoreboard players add #party dw.s_crossings 1")
        .unwrap();
    let tp = body.find("tp @s ~0 ~0 ~-6").unwrap();
    let cross = body
        .find(&format!("function {ns}:loop_gallery_cross"))
        .unwrap();
    assert!(count < tp && tp < cross, "count, move, answer: {body}");
    let answer = run.text(&format!(
        "datapack/data/{ns}/function/loop_gallery_cross.mcfunction"
    ));
    assert!(
        !answer.contains("@s"),
        "the answer addresses no body: {answer}"
    );

    let a = run.text(&format!(
        "packtest-datapack/data/{ns}/test/loop_gallery.mcfunction"
    ));
    assert!(
        a.contains(&format!("function {ns}:loop_gallery_poll\n")),
        "{a}"
    );
    assert!(a.contains("matches -6000"), "asserts the offset: {a}");
    assert!(
        a.contains("#lp_n1_gallery dw.sys matches 1"),
        "asserts the count: {a}"
    );
    let r = run.text(&format!(
        "packtest-datapack/data/{ns}/test/loop_gallery_released.mcfunction"
    ));
    assert!(
        r.contains("scoreboard players set #party dw.s_crossings 2"),
        "shuts the gate: {r}"
    );
    assert!(
        r.contains(&format!("function {ns}:loop_gallery_poll\n")),
        "{r}"
    );
    assert_eq!(
        r.matches("matches 0\n").count(),
        4,
        "no move on any axis, no count: {r}"
    );
}

#[test]
fn the_ledger_states_the_binding_per_loop_and_what_is_unchecked() {
    let run = build(&campaign("ledger", |_| {}));
    run.green();
    let gate = run.json("validation/loop-gate.json");
    assert_eq!(gate["loops"], 1);
    let row = &gate["rows"][0];
    assert_eq!(row["id"], "loop/gallery");
    assert_eq!(row["offset"], json!([0, 0, -6]));
    assert_eq!(row["exercise"][0]["times"], 2);
    assert_eq!(row["open_faces"], 0);
    assert!(row["visible"].as_u64().unwrap() > 0);
    assert!(row["eyes"].as_u64().unwrap() > 0);
    assert!(row["span"].is_array());
    assert_eq!(gate["unchecked"].as_array().unwrap().len(), 5);
    let b = run.binding();
    for part in [
        "1 loop(s)",
        "slab cells 9",
        "open faces 0",
        "at 2 skies",
        "exercise steps 1",
    ] {
        assert!(b.contains(part), "`{part}` in {b}");
    }
}

#[test]
fn a_campaign_with_no_loop_writes_no_ledger() {
    let run = build(&campaign("no-loop", |q| {
        q["content"].as_object_mut().unwrap().remove("loops");
        q["content"].as_object_mut().unwrap().remove("state");
    }));
    run.green();
    assert!(!run.out.join("validation/loop-gate.json").exists());
    assert!(!run.stderr.contains("loop binding:"));
    assert!(
        !run.text("datapack/data/long-gallery/function/tick.mcfunction")
            .contains("loop_")
    );
}
