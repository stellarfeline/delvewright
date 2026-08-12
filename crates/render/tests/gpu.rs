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
        fov_deg: shots::ORBIT_FOV_DEG,
        framing: shots::Framing::Orbit {
            zoom: 1.0,
            target: None,
        },
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
        fov_deg: shots::ORBIT_FOV_DEG,
        framing: shots::Framing::Orbit {
            zoom: 1.0,
            target: None,
        },
        dim: 256,
    };
    let frame = render::render_structure(&st, &pack, false, &params).expect("render");
    assert!(
        detect::scan_default(&frame.rgba, frame.width, frame.height).is_some(),
        "heavy_core's unresolved model must trip the missing-texture detector"
    );
}

/// The pixel-level half of the `facing` binding (the plan-level half is
/// `shots::tests::opposite_facings_on_one_cell_aim_opposite_ways`): one body,
/// one eye point, two opposite facings — two **different pictures**. A renderer
/// that dropped the facing on the way to the camera would produce one picture
/// twice and fail here, which the planner test alone cannot see.
#[test]
#[ignore = "needs a GPU adapter + the 1.21.11 client jar"]
fn opposite_facings_render_different_pictures() {
    let Some(tex) = textures() else {
        eprintln!("skip: no client jar");
        return;
    };
    let pack = render::load_pack(&tex).expect("load pack");
    // A corridor with a distinctive block at one end only, so the two views
    // cannot coincide by symmetry.
    let mut blocks = Vec::new();
    for x in 0..5 {
        for y in 0..5 {
            for z in 0..11 {
                let shell = x == 0 || x == 4 || z == 0 || z == 10 || y == 0 || y == 4;
                let idx = if z == 1 && shell {
                    2 // one end wall in a different material
                } else if shell {
                    1
                } else {
                    0
                };
                blocks.push(([x, y, z], idx));
            }
        }
    }
    let st = nbt::Structure {
        size: [5, 5, 11],
        palette: vec![
            "minecraft:air".into(),
            "minecraft:deepslate".into(),
            "minecraft:gold_block".into(),
        ],
        blocks,
    };
    let eye = [2.5, 1.0 + delvewright_render::occupancy::EYE_HEIGHT, 5.5];
    let frame = |yaw: f32| {
        render::render_structure(
            &st,
            &pack,
            false,
            &RenderParams {
                yaw_deg: yaw,
                pitch_deg: 0.0,
                fov_deg: shots::PLAYER_FOV_DEG,
                framing: shots::Framing::Eye { pos: eye },
                dim: 256,
            },
        )
        .expect("render")
    };
    let north = frame(delvewright_render::occupancy::Facing::North.view_yaw_deg());
    let south = frame(delvewright_render::occupancy::Facing::South.view_yaw_deg());
    let differing = north
        .rgba
        .iter()
        .zip(&south.rgba)
        .filter(|(a, b)| (**a as i32 - **b as i32).abs() > 8)
        .count();
    let total = north.rgba.len();
    assert!(
        differing * 100 / total > 10,
        "north and south from one cell differ in only {differing}/{total} channels — the \
         anchor's facing is not reaching the camera"
    );
}

/// An interior view a reviewer compares to a concept painting must not be
/// MIRRORED — a flipped room reads as a correct picture of a different room, and
/// nothing in a shot manifest can catch it. Pinned against a corridor whose east
/// wall is gold: a body looking north must see the gold on its right.
#[test]
#[ignore = "needs a GPU adapter + the 1.21.11 client jar"]
fn an_eye_view_is_not_mirrored() {
    let Some(tex) = textures() else {
        eprintln!("skip: no client jar");
        return;
    };
    let pack = render::load_pack(&tex).expect("load pack");
    let mut blocks = Vec::new();
    for x in 0..5 {
        for y in 0..5 {
            for z in 0..11 {
                let shell = x == 0 || x == 4 || z == 0 || z == 10 || y == 0 || y == 4;
                // The +X (east) wall only.
                let idx = if x == 4 {
                    2
                } else if shell {
                    1
                } else {
                    0
                };
                blocks.push(([x, y, z], idx));
            }
        }
    }
    let st = nbt::Structure {
        size: [5, 5, 11],
        palette: vec![
            "minecraft:air".into(),
            "minecraft:deepslate".into(),
            "minecraft:gold_block".into(),
        ],
        blocks,
    };
    let f = render::render_structure(
        &st,
        &pack,
        false,
        &RenderParams {
            yaw_deg: delvewright_render::occupancy::Facing::North.view_yaw_deg(),
            pitch_deg: 0.0,
            fov_deg: shots::PLAYER_FOV_DEG,
            framing: shots::Framing::Eye {
                pos: [2.5, 1.0 + delvewright_render::occupancy::EYE_HEIGHT, 5.5],
            },
            dim: 256,
        },
    )
    .expect("render");
    // Gold is the only strongly warm material in the frame: R clearly over B.
    let (mut left, mut right) = (0u32, 0u32);
    for y in 0..256u32 {
        for x in 0..256u32 {
            let i = ((y * 256 + x) * 4) as usize;
            let (r, b) = (f.rgba[i] as i32, f.rgba[i + 2] as i32);
            if r - b > 40 {
                if x < 128 {
                    left += 1;
                } else {
                    right += 1;
                }
            }
        }
    }
    assert!(
        right > 0 && right > left * 4,
        "east wall looking north must fall on the RIGHT of frame — got {left} left / {right} \
         right; the interior view is mirrored"
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
    let meta = delvewright_render::meta::PrefabMeta::beside_nbt(&p).expect("meta");
    let plan = shots::plan_piece(&st, meta.as_ref());
    for shot in &plan.shots {
        let params = RenderParams {
            yaw_deg: shot.yaw_deg,
            pitch_deg: shot.pitch_deg,
            fov_deg: shot.fov_deg,
            framing: shot.framing,
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
