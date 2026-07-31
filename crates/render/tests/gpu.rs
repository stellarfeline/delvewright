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
    let frame = render::render_structure(&st, &pack, false, &params).expect("render");
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
    let frame = render::render_structure(&st, &pack, false, &params).expect("render");
    assert!(
        detect::scan_default(&frame.rgba, frame.width, frame.height).is_some(),
        "heavy_core's unresolved model must trip the missing-texture detector"
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
        let a = render::render_structure(&st, &pack, shot.cutaway, &params).expect("render a");
        let b = render::render_structure(&st, &pack, shot.cutaway, &params).expect("render b");
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
