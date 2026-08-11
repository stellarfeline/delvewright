//! GPU integration tests — the actual Nucleation/wgpu render path.
//!
//! `#[ignore]` by default: they need a GPU adapter AND the 1.21.11 client jar
//! (never committed — EULA). Run locally with the textures available:
//!
//! ```sh
//! DELVEWRIGHT_CLIENT_JAR=~/.chunky/resources/minecraft.jar \
//!   cargo test -p delvewright-render --test gpu -- --ignored --nocapture
//! ```
//!
//! ## Double-render stability finding (measured 2026-07-30, macOS/Metal)
//!
//! On a fixed machine + driver + wgpu version, a double render of the same prefab
//! is **byte-identical** — all nine keep-gate-room shots produced identical PNG
//! bytes and zero per-pixel delta. But this is NOT guaranteed across GPUs /
//! drivers / platforms (float rasterization), so renders are **excluded from
//! ADR-0006 byte-identity** — they are validation artifacts, not shipped output.
//! This test therefore asserts **pixel-equality within a tolerance** (the portable
//! guarantee); on this machine it in fact holds exactly.

use std::path::{Path, PathBuf};

use delvewright_render::cutaway::Cutaway;
use delvewright_render::detect;
use delvewright_render::fidelity;
use delvewright_render::nbt;
use delvewright_render::render::{self, RenderParams};
use delvewright_render::shots;

/// Resolve textures the way the CLI does; `None` → skip the test.
fn textures() -> Option<String> {
    if let Ok(t) = std::env::var("DELVEWRIGHT_CLIENT_JAR")
        && Path::new(&t).exists()
    {
        return Some(t);
    }
    if let Ok(home) = std::env::var("HOME") {
        let d = format!("{home}/.chunky/resources/minecraft.jar");
        if Path::new(&d).exists() {
            return Some(d);
        }
    }
    None
}

fn prefab(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../campaigns/prefabs")
        .join(name)
}

#[test]
#[ignore = "needs a GPU adapter + the 1.21.11 client jar"]
fn fidelity_gate_fixture_has_no_placeholder() {
    let Some(tex) = textures() else {
        eprintln!("skip: no client jar");
        return;
    };
    let pack = render::load_pack(&tex).expect("load pack");
    let st = fidelity::fixture_structure();
    let params = RenderParams {
        yaw_deg: 25.0,
        pitch_deg: 35.0,
        zoom: 1.0,
        target: None,
        dim: 512,
    };
    let frame = render::render_structure(&st, &pack, &Cutaway::none(), &params).expect("render");
    assert!(
        detect::scan_default(&frame.rgba, frame.width, frame.height).is_none(),
        "the newest-block fixture (heavy_core excluded) must render placeholder-free"
    );
}

#[test]
#[ignore = "needs a GPU adapter + the 1.21.11 client jar"]
fn detector_catches_heavy_core_when_included() {
    // Prove the gate's detector fires end-to-end on a real unresolved model by
    // adding heavy_core to a one-block structure and rendering it.
    let Some(tex) = textures() else {
        eprintln!("skip: no client jar");
        return;
    };
    let pack = render::load_pack(&tex).expect("load pack");
    let st = nbt::Structure {
        size: [1, 2, 1],
        palette: vec![
            "minecraft:stone_bricks".into(),
            "minecraft:heavy_core".into(),
        ],
        blocks: vec![([0, 0, 0], 0), ([0, 1, 0], 1)],
    };
    let params = RenderParams {
        yaw_deg: 30.0,
        pitch_deg: 25.0,
        zoom: 1.0,
        target: None,
        dim: 256,
    };
    let frame = render::render_structure(&st, &pack, &Cutaway::none(), &params).expect("render");
    assert!(
        detect::scan_default(&frame.rgba, frame.width, frame.height).is_some(),
        "heavy_core's unresolved model must trip the missing-texture detector"
    );
}

/// A tall body carved out of solid mass, with ONE interior wall whose position
/// is the parameter: 20 × 10 × 40 of deepslate, hollowed from `wall_x + 1` to
/// the far side, under a cap [`CAP`] courses thick.
///
/// The cap is the whole point and it is measured, not invented: on the bell's
/// `cistern_deep` (40 × 10 × 100) the top TWO courses are 100 % solid, so the
/// layer a one-layer dollhouse reveals is more rock and two completely different
/// interiors make the same picture. A small roofed prefab has a cap of one, which
/// is why the boolean looked right for as long as only small prefabs were shot.
fn carved_mass(wall_x: i32) -> nbt::Structure {
    /// Courses of solid rock over the void.
    const CAP: i32 = 3;
    let size = [20, 10, 40];
    let mut blocks = Vec::new();
    for x in 0..size[0] {
        for y in 0..size[1] {
            for z in 0..size[2] {
                let void = x > wall_x
                    && x < size[0] - 1
                    && y > 0
                    && y < size[1] - CAP
                    && z > 0
                    && z < size[2] - 1;
                if !void {
                    blocks.push(([x, y, z], 0));
                }
            }
        }
    }
    nbt::Structure {
        size,
        palette: vec!["minecraft:deepslate".into()],
        blocks,
    }
}

/// Percentage of the frame whose pixels differ at all (the PR #372 measure).
fn frame_delta_pct(a: &render::Frame, b: &render::Frame) -> f64 {
    assert_eq!(a.rgba.len(), b.rgba.len());
    let differing = a
        .rgba
        .chunks_exact(4)
        .zip(b.rgba.chunks_exact(4))
        .filter(|(p, q)| p != q)
        .count();
    let total = (a.width as usize) * (a.height as usize);
    100.0 * differing as f64 / total as f64
}

#[test]
#[ignore = "needs a GPU adapter + the 1.21.11 client jar"]
fn a_moved_interior_wall_is_invisible_under_the_old_strip_and_plain_in_a_section() {
    // The motivating finding, kept available forever without a patch: on a tall
    // solid body, the pre-cutaway shot set (4 exteriors + a one-layer `top`)
    // cannot see an interior wall move, and the sections can.
    let Some(tex) = textures() else {
        eprintln!("skip: no client jar");
        return;
    };
    let pack = render::load_pack(&tex).expect("load pack");
    let (a, b) = (carved_mass(6), carved_mass(12));
    let size = a.size;
    let plan = shots::plan_piece(size, None);

    let mut old_worst: f64 = 0.0;
    let mut section_best: f64 = 0.0;
    for shot in &plan {
        let params = RenderParams {
            yaw_deg: shot.yaw_deg,
            pitch_deg: shot.pitch_deg,
            zoom: shot.zoom,
            target: shot.target,
            dim: 256,
        };
        let fa = render::render_structure(&a, &pack, &shot.cutaway, &params).expect("render a");
        let fb = render::render_structure(&b, &pack, &shot.cutaway, &params).expect("render b");
        let pct = frame_delta_pct(&fa, &fb);
        eprintln!("  {:<9} {:<12} {pct:6.2}%", shot.name, shot.cutaway);
        // The shots that existed before the cutaway became a parameter.
        if shot.name.starts_with("ext-") || shot.name == "top" {
            old_worst = old_worst.max(pct);
        } else {
            section_best = section_best.max(pct);
        }
    }
    assert!(
        old_worst < 1.0,
        "the pre-cutaway shot set should be near blind to this change, saw {old_worst:.2}%"
    );
    assert!(
        section_best > 5.0,
        "a section must make a moved interior wall plain, saw {section_best:.2}%"
    );
}

#[test]
#[ignore = "needs a GPU adapter + the 1.21.11 client jar"]
fn sensitivity_runs_in_both_directions() {
    // Two genuinely different massings differ; the SAME massing rendered twice
    // does not. A gate that can only fail in one direction proves nothing.
    let Some(tex) = textures() else {
        eprintln!("skip: no client jar");
        return;
    };
    let pack = render::load_pack(&tex).expect("load pack");
    // The wall moves in X, so it is read from an X section — the same pairing
    // the planner makes, not a hand-picked friendly angle.
    let cut: delvewright_render::cutaway::Cutaway = "x-min:50%".parse().unwrap();
    let (yaw, pitch) = cut
        .viewpoint([20, 10, 40])
        .expect("a section has a viewpoint");
    let params = RenderParams {
        yaw_deg: yaw,
        pitch_deg: pitch,
        zoom: 1.0,
        target: None,
        dim: 256,
    };
    let one = render::render_structure(&carved_mass(6), &pack, &cut, &params).expect("render");
    let other = render::render_structure(&carved_mass(12), &pack, &cut, &params).expect("render");
    let same = render::render_structure(&carved_mass(6), &pack, &cut, &params).expect("render");
    assert!(
        frame_delta_pct(&one, &other) > 5.0,
        "two massings must differ visibly"
    );
    assert_eq!(
        frame_delta_pct(&one, &same),
        0.0,
        "the same massing rendered twice must be identical"
    );
}

#[test]
#[ignore = "needs a GPU adapter + the 1.21.11 client jar"]
fn piece_double_render_is_stable() {
    let Some(tex) = textures() else {
        eprintln!("skip: no client jar");
        return;
    };
    let pack = render::load_pack(&tex).expect("load pack");
    let p = prefab("keep-gate-room.nbt");
    if !p.exists() {
        eprintln!("skip: {} absent (no content symlink)", p.display());
        return;
    }
    let st = nbt::parse_structure(&p).expect("parse");
    let plan = shots::plan_piece(st.size, None); // exterior + top-down (no meta needed)
    for shot in &plan {
        let params = RenderParams {
            yaw_deg: shot.yaw_deg,
            pitch_deg: shot.pitch_deg,
            zoom: shot.zoom,
            target: shot.target,
            dim: 256,
        };
        let a = render::render_structure(&st, &pack, &shot.cutaway, &params).expect("render a");
        let b = render::render_structure(&st, &pack, &shot.cutaway, &params).expect("render b");
        assert_eq!(a.rgba.len(), b.rgba.len());
        // Portable guarantee: pixel-equal within tolerance. (Observed exactly
        // byte-identical on macOS/Metal — see module docs.)
        let worst = a
            .rgba
            .iter()
            .zip(&b.rgba)
            .map(|(x, y)| (*x as i32 - *y as i32).abs())
            .max()
            .unwrap_or(0);
        assert!(
            worst <= 2,
            "shot {} unstable: max channel delta {worst}",
            shot.name
        );
    }
}
