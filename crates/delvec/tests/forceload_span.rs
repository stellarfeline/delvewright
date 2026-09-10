//! A world whose forceload span is bigger than one command may name.
//!
//! The span a delve forceloads is **derived, never typed**: it is a placed
//! piece's own bounding box, or the ring a horizon grows around one. So a
//! campaign can reach a span vanilla refuses without anybody having written a
//! number down — and that is what happened. A 101 x 101 piece under a `valley`
//! horizon derives `forceload add -76 -76 176 176`, which is 17 x 17 = 289
//! chunks against a ceiling of 256, and the pinned server answers `Too many
//! chunks in the specified area (maximum 256, but specified 289)` and marks
//! nothing at all. Because a refused command inside a function is not a parse
//! failure, the function carried on, the world booted, and every gate this
//! repository owns stayed green while `place template` no-opped on unloaded
//! chunks and the delve never finished setting itself up.
//!
//! `commands::forceload_add_lines` splits the span, and `commands.rs`'s
//! `forceload_area_error` refuses any line that skipped it. This is the build
//! that binds both: the smallest campaign whose derived span reaches the
//! ceiling, with the same geometry as the artefact that produced the finding.
//! Remove the split from `emit.rs` and this build fails, because the compiler
//! now refuses its own output.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

mod common;

/// The scene side that reaches the ceiling under a default `valley` horizon.
///
/// The ring is the scene grown by 76 on every side, so the span runs `-76` to
/// `SIDE - 1 + 76` and covers `(SIDE + 75).div_euclid(16) + 6` chunks per axis.
/// 17 per axis is 289 chunks and the first side that gets there is 101 — which
/// is exactly the footprint of the castle that found this.
const SIDE: i32 = 101;

/// The vanilla per-axis cap on one structure template, and therefore the cut a
/// piece this wide ships on: 101 becomes 48 + 48 + 5.
const PART_MAX: i32 = 48;

/// The most chunks one `forceload` command may name. Stated here rather than
/// imported so this test asserts the number against the compiler's own constant
/// instead of restating whatever the compiler currently believes.
const CEILING: i64 = 256;

fn tmp(name: &str) -> PathBuf {
    let p = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn delvec(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_delvec"))
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

/// Every non-air cell of a sealed `SIDE x 5 x SIDE` room, lit from its ceiling.
///
/// Sealed on all six sides for the reason `common::write_tiled_zone` gives: a
/// world a body can fall out of would make this test measure boundary safety
/// instead of the thing it is about.
fn room_cells() -> Vec<([i32; 3], &'static str)> {
    let (sx, sy, sz) = (SIDE, 5, SIDE);
    let mut cells = Vec::new();
    for z in 0..sz {
        for y in 0..sy {
            for x in 0..sx {
                if x == 0 || x == sx - 1 || y == 0 || y == sy - 1 || z == 0 || z == sz - 1 {
                    cells.push(([x, y, z], "minecraft:stone"));
                } else if y == sy - 2 && x % 6 == 3 && z % 6 == 3 {
                    // Light in the ceiling course, on a grid: a lit interior,
                    // not a `lit` claim about a dark box.
                    cells.push(([x, y, z], "minecraft:glowstone"));
                }
            }
        }
    }
    cells
}

/// The inclusive run boundaries a side of `len` is cut into at `PART_MAX`.
fn cuts(len: i32) -> Vec<(i32, i32)> {
    let mut runs = Vec::new();
    let mut at = 0;
    while at < len {
        let n = PART_MAX.min(len - at);
        runs.push((at, n));
        at += n;
    }
    runs
}

/// Write the sealed room into `dir` as a tiled prefab library entry: the grid of
/// `.nbt` tiles the 48-per-axis cap forces, plus the one document naming them.
fn write_wide_room(dir: &Path, id: &str) {
    std::fs::create_dir_all(dir).unwrap();
    let cells = room_cells();
    let xs = cuts(SIDE);
    let zs = cuts(SIDE);
    let mut parts = Vec::new();
    for (ix, (x0, dx)) in xs.iter().enumerate() {
        for (iz, (z0, dz)) in zs.iter().enumerate() {
            let tile: Vec<([i32; 3], &str)> = cells
                .iter()
                .filter(|(p, _)| p[0] >= *x0 && p[0] < x0 + dx && p[2] >= *z0 && p[2] < z0 + dz)
                .map(|(p, n)| ([p[0] - x0, p[1], p[2] - z0], *n))
                .collect();
            let file = format!("{id}.x{ix}y0z{iz}.nbt");
            std::fs::write(dir.join(&file), common::structure_nbt([*dx, 5, *dz], &tile)).unwrap();
            parts.push(serde_json::json!({
                "file": file,
                "id": format!("{id}.x{ix}y0z{iz}"),
                "grid_index": [ix as i32, 0, iz as i32],
                "offset": [*x0, 0, *z0],
                "size": [*dx, 5, *dz],
            }));
        }
    }
    let meta = serde_json::json!({
        "prefab_id": format!("prefab/{id}"),
        "structure_set": {
            "base": id,
            "size": [SIDE, 5, SIDE],
            "part_max": PART_MAX,
            "grid": [xs.len() as i32, 1, zs.len() as i32],
            "data_version": 4671,
            "generator": "crates/delvec/tests/forceload_span.rs",
            "parts": parts,
        },
        "anchors": {
            "spawn": { "pos": [5, 1, 5], "facing": "south", "role": "entry" },
            "anchor/keeper-stand": { "pos": [5, 1, 6], "facing": "north" },
            "anchor/door": {
                "region": { "from": [4, 1, 4], "to": [4, 1, 4] },
                "block": "minecraft:iron_bars"
            },
            "anchor/exit": { "pos": [6, 1, 6] }
        },
        "connectors": [],
        "lighting": { "profile": "lit", "measured_min_light": 8, "measured": "2026-08-15" },
        "license": {
            "source": "original",
            "spdx": "GPL-3.0-or-later",
            "note": "Test fixture.",
            "provenance": "Synthesised by crates/delvec/tests/forceload_span.rs."
        }
    });
    std::fs::write(
        dir.join(format!("{id}.json")),
        serde_json::to_string_pretty(&meta).unwrap() + "\n",
    )
    .unwrap();
    common::declare_walk_y(dir, id);
}

/// The chunk a world-block coordinate is in.
fn chunk_of(v: i32) -> i32 {
    v.div_euclid(16)
}

/// Every chunk a `forceload add` line marks, and how many.
fn marks(line: &str) -> Vec<(i32, i32)> {
    let c: Vec<i32> = line
        .split_whitespace()
        .skip(2)
        .map(|s| s.parse().expect("an emitted coordinate is an integer"))
        .collect();
    assert_eq!(c.len(), 4, "an emitted forceload names a rectangle: {line}");
    let (x0, x1) = (chunk_of(c[0].min(c[2])), chunk_of(c[0].max(c[2])));
    let (z0, z1) = (chunk_of(c[1].min(c[3])), chunk_of(c[1].max(c[3])));
    (x0..=x1)
        .flat_map(|cx| (z0..=z1).map(move |cz| (cx, cz)))
        .collect()
}

/// **A world whose derived span is bigger than one command may name still
/// forceloads every chunk of it.**
///
/// The build is the binding: if `emit.rs` stops going through
/// `forceload_add_lines`, the compiler's own command validator refuses the
/// emitted `setup.mcfunction` and this build exits non-zero — which is the
/// perturbation this test is here to catch.
#[test]
fn a_span_past_the_ceiling_forceloads_every_chunk_of_itself() {
    let root = tmp("forceload-span");
    let prefabs = root.join("prefabs");
    common::copy_dir_all(&common::prefabs_dir(), &prefabs);
    write_wide_room(&prefabs, "wide-room");
    // No `shown_faces`: the room is sealed, so the party's air never reaches its
    // outside and `DW0885` never asks. A declaration here would be six claims
    // binding zero, which is the shape this repository calls a finding.

    let campaign = common::campaign_bound_to(&root.join("campaign"), "wide-room");
    common::patch_file(&campaign.join("world.json"), |v| {
        let c = v["content"].as_object_mut().unwrap();
        c.insert("horizon".into(), serde_json::json!("valley"));
        c.insert("boundary".into(), serde_json::json!({ "margin": 20 }));
    });

    let out = root.join("out");
    let r = delvec(&[
        "build",
        campaign.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        prefabs.to_str().unwrap(),
    ]);
    assert!(
        r.status.success(),
        "a world with a legal piece under a legal horizon builds:\n{}",
        log(&r)
    );

    let setup =
        std::fs::read_to_string(out.join("datapack/data/hello-world/function/setup.mcfunction"))
            .expect("setup emitted");
    let lines: Vec<&str> = setup
        .lines()
        .filter(|l| l.starts_with("forceload add "))
        .collect();

    // The scene's own piece is one line; the horizon's ring is the one that
    // reaches the ceiling. Fewer than three lines means the ring came out inside
    // it after all and this test is binding to nothing.
    assert!(
        lines.len() >= 3,
        "the ring past the ceiling was split into several lines ({} emitted):\n{setup}",
        lines.len()
    );

    let mut union: std::collections::BTreeSet<(i32, i32)> = Default::default();
    let mut biggest = 0i64;
    for line in &lines {
        let m = marks(line);
        biggest = biggest.max(m.len() as i64);
        assert!(
            m.len() as i64 <= CEILING,
            "1.21.11 refuses this line whole and marks nothing: {line}"
        );
        union.extend(m);
    }
    assert!(
        biggest > 0,
        "the lines mark chunks — a zero here is the check binding to nothing"
    );

    // Every chunk of the ring the horizon derived is marked. The ring runs from
    // -76 to SIDE-1+76, so this is the chunk set the one refused command asked
    // for, arrived at by counting rather than by trusting the split.
    let (lo, hi) = (chunk_of(-76), chunk_of(SIDE - 1 + 76));
    let want: std::collections::BTreeSet<(i32, i32)> = (lo..=hi)
        .flat_map(|cx| (lo..=hi).map(move |cz| (cx, cz)))
        .collect();
    assert_eq!(
        (hi - lo + 1) as i64 * (hi - lo + 1) as i64,
        289,
        "the fixture reaches the span that found this: 17 x 17"
    );
    assert!(
        want.is_subset(&union),
        "{} of the ring's {} chunks are marked by no line",
        want.difference(&union).count(),
        want.len()
    );
}
