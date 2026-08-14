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
    let plan = shots::plan_piece(&st, meta.as_ref(), &[]).unwrap();
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

/// A long box of deepslate whose −Z face alone is gold: the only warm material
/// in the model, so "am I looking at the north face" is decidable from the
/// pixels. Deep on Z, so a camera fitted to the whole box sits far back and the
/// near face would be a thumbnail — the trial's actual complaint.
fn one_gold_face(size: [i32; 3]) -> nbt::Structure {
    let mut blocks = Vec::new();
    for x in 0..size[0] {
        for y in 0..size[1] {
            for z in 0..size[2] {
                let shell = x == 0
                    || x == size[0] - 1
                    || z == 0
                    || z == size[2] - 1
                    || y == 0
                    || y == size[1] - 1;
                let idx = if z == 0 {
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
    nbt::Structure {
        size,
        palette: vec![
            "minecraft:air".into(),
            "minecraft:deepslate".into(),
            "minecraft:gold_block".into(),
        ],
        blocks,
    }
}

/// Share of frame pixels that are strongly warm — gold, and nothing else here.
fn warm_share(f: &render::Frame) -> f64 {
    let warm = f
        .rgba
        .chunks_exact(4)
        .filter(|p| p[0] as i32 - p[2] as i32 > 40)
        .count();
    warm as f64 / (f.width * f.height) as f64
}

/// Share of frame pixels that are not the renderer's fixed background. Sampled
/// from the frame's own corner rather than hardcoded, so it cannot drift with a
/// colour-space change in the renderer.
fn covered_share(f: &render::Frame) -> f64 {
    let bg = [f.rgba[0], f.rgba[1], f.rgba[2]];
    let hit = f
        .rgba
        .chunks_exact(4)
        .filter(|p| (0..3).any(|c| (p[c] as i32 - bg[c] as i32).abs() > 6))
        .count();
    hit as f64 / (f.width * f.height) as f64
}

fn render_shot(
    st: &nbt::Structure,
    pack: &nucleation::meshing::ResourcePackSource,
    shot: &shots::PieceShot,
    dim: u32,
) -> render::Frame {
    render::render_structure(
        st,
        pack,
        shot.cutaway,
        &RenderParams {
            yaw_deg: shot.yaw_deg,
            pitch_deg: shot.pitch_deg,
            fov_deg: shot.fov_deg,
            framing: shot.framing,
            dim,
        },
    )
    .expect("render")
}

fn plan_view(st: &nbt::Structure, spec: &str) -> shots::PieceShot {
    let v = delvewright_render::view::View::parse(spec).expect("parse view");
    shots::plan_piece(st, None, std::slice::from_ref(&v))
        .expect("plan")
        .shots
        .pop()
        .expect("the view is the last shot")
}

/// The pixel-level half of the claim the whole surface exists for: `face=<f>`
/// photographs THAT face. A planner that dropped the face on the way to the
/// camera would produce four copies of one picture and pass every unit test
/// while failing here. The fixed set cannot answer this at all — it contains no
/// level camera, which is the defect.
#[test]
#[ignore = "needs a GPU adapter + the 1.21.11 client jar"]
fn a_face_view_photographs_that_face() {
    let Some(tex) = textures() else {
        eprintln!("skip: no client jar");
        return;
    };
    let pack = render::load_pack(&tex).expect("load pack");
    let st = one_gold_face([9, 9, 31]);

    let north = warm_share(&render_shot(&st, &pack, &plan_view(&st, "face=north"), 256));
    assert!(
        north > 0.4,
        "the gold −Z face must dominate its own elevation — got {north:.3}"
    );
    for face in ["south", "east", "west"] {
        let shot = plan_view(&st, &format!("face={face}"));
        let f = render_shot(&st, &pack, &shot, 256);
        assert!(
            detect::is_featureless(&f.rgba, f.width, f.height).is_none(),
            "face={face} rendered an empty frame"
        );
        let share = warm_share(&f);
        assert!(
            share < 0.02,
            "face={face} shows the north face's gold ({share:.3}) — the face is not reaching \
             the camera"
        );
    }
}

/// …and it frames it. A face view of a 31-deep box fills far more of the frame
/// than the same box's corner-isometric, which is the whole reason the framed
/// box is the face and not the model: fitting the model from the front backs the
/// camera off past the far end and leaves the face a thumbnail.
#[test]
#[ignore = "needs a GPU adapter + the 1.21.11 client jar"]
fn a_face_view_fills_the_frame_where_the_planned_set_cannot() {
    let Some(tex) = textures() else {
        eprintln!("skip: no client jar");
        return;
    };
    let pack = render::load_pack(&tex).expect("load pack");
    let st = one_gold_face([9, 9, 31]);
    let plan = shots::plan_piece(&st, None, &[]).expect("plan");
    let ext = plan.shots.iter().find(|s| s.name == "ext-ne").unwrap();
    let ext_cover = covered_share(&render_shot(&st, &pack, ext, 256));
    let view_cover = covered_share(&render_shot(&st, &pack, &plan_view(&st, "face=north"), 256));
    assert!(
        view_cover > ext_cover * 1.5,
        "a declared elevation covers {view_cover:.3} of the frame against the corner \
         isometric's {ext_cover:.3} — it is not framing the face"
    );
}

/// A view CAN be aimed at nothing, and when it is, the run says so on the pixels
/// rather than writing a blank file that reads as one more shot of the piece.
#[test]
#[ignore = "needs a GPU adapter + the 1.21.11 client jar"]
fn a_view_aimed_at_nothing_is_reported_as_dw0727() {
    let Some(tex) = textures() else {
        eprintln!("skip: no client jar");
        return;
    };
    let pack = render::load_pack(&tex).expect("load pack");
    let st = one_gold_face([9, 9, 31]);
    let shot = plan_view(&st, "name=blind,face=north,zoom=400");
    let f = render_shot(&st, &pack, &shot, 256);
    let empty = detect::is_featureless(&f.rgba, f.width, f.height)
        .expect("a camera 400x past the fit is inside the model and sees nothing");
    let d = shots::empty_frame_diagnostic("piece", &shot, &empty);
    assert_eq!(d.code, "DW0727");
    assert!(d.message.contains("blind"), "{}", d.message);
    assert!(d.message.contains("is NOT in this set"), "{}", d.message);
}

/// Determinism on the surface this added: the same view spec twice, the same
/// bytes. (The portable guarantee is pixel-equality within tolerance — see the
/// module docs — but a declared camera is solved arithmetically, so any drift in
/// that arithmetic shows up here first.)
#[test]
#[ignore = "needs a GPU adapter + the 1.21.11 client jar"]
fn a_declared_view_renders_identically_twice() {
    let Some(tex) = textures() else {
        eprintln!("skip: no client jar");
        return;
    };
    let pack = render::load_pack(&tex).expect("load pack");
    let st = one_gold_face([9, 9, 31]);
    for spec in ["face=north", "face=east,zoom=2", "yaw=25,pitch=15,fov=60"] {
        let shot = plan_view(&st, spec);
        let a = render_shot(&st, &pack, &shot, 256);
        let b = render_shot(&st, &pack, &shot, 256);
        let worst = a
            .rgba
            .iter()
            .zip(&b.rgba)
            .map(|(x, y)| (*x as i32 - *y as i32).abs())
            .max()
            .unwrap_or(0);
        assert!(worst <= 2, "`{spec}` unstable: max channel delta {worst}");
    }
}
