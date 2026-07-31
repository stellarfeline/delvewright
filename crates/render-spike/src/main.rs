//! SPIKE: headless Nucleation renderer for Delvewright vanilla-structure `.nbt`.
//!
//! Nucleation (0.9, MIT) has no importer for the *binary gzip* vanilla
//! structure `.nbt` our prefab generator emits — its format manager only
//! detects Sponge `.schem`, litematic, Bedrock `.mcstructure`, MCEdit,
//! world regions, and *text* structure SNBT. So we gunzip the `.nbt`
//! ourselves (fastnbt, the same stack the compiler uses), read the vanilla
//! `size` / `palette` / `blocks` schema, and rebuild it as a
//! `UniversalSchematic` via the public `set_block` API. Textures come from a
//! resource pack (the pinned 1.21.11 client jar) — that is what actually
//! determines block fidelity. Render is wgpu -> Metal, off-screen, no display.
//!
//! Usage: nuke-render <structure.nbt> <resourcepack.(zip|jar|dir)> <out.png>
//!                    [--yaw=45] [--pitch=30] [--zoom=1.0] [--size=1024]

use std::collections::HashMap;
use std::io::Read as _;
use std::time::Instant;

use fastnbt::Value;
use flate2::read::GzDecoder;
use nucleation::meshing::{MeshConfig, MeshOutput, ResourcePackSource};
use nucleation::rendering::camera::CameraConfig;
use nucleation::rendering::gpu::GpuRenderer;
use nucleation::{BlockState, UniversalSchematic};

fn as_i32(v: &Value) -> i32 {
    match v {
        Value::Byte(b) => *b as i32,
        Value::Short(s) => *s as i32,
        Value::Int(i) => *i,
        Value::Long(l) => *l as i32,
        _ => panic!("expected int-like NBT, got {v:?}"),
    }
}

/// Build `minecraft:foo[a=b,c=d]` from a vanilla palette entry compound.
fn palette_state_string(entry: &HashMap<String, Value>) -> String {
    let Some(Value::String(name)) = entry.get("Name") else {
        panic!("palette entry missing Name: {entry:?}");
    };
    let mut s = name.clone();
    if let Some(Value::Compound(props)) = entry.get("Properties") {
        if !props.is_empty() {
            // Sorted for determinism / readability; order is irrelevant to parsing.
            let mut kv: Vec<(&String, String)> = props
                .iter()
                .map(|(k, v)| {
                    let Value::String(val) = v else {
                        panic!("property value not a string: {v:?}");
                    };
                    (k, val.clone())
                })
                .collect();
            kv.sort_by(|a, b| a.0.cmp(b.0));
            let body: Vec<String> = kv.iter().map(|(k, v)| format!("{k}={v}")).collect();
            s.push('[');
            s.push_str(&body.join(","));
            s.push(']');
        }
    }
    s
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!(
            "usage: nuke-render <structure.nbt> <resourcepack> <out.png> \
             [--yaw=N] [--pitch=N] [--zoom=N] [--size=N]"
        );
        std::process::exit(2);
    }
    let nbt_path = &args[1];
    let pack_path = &args[2];
    let out_path = &args[3];

    let mut yaw = 45.0f32;
    let mut pitch = 30.0f32;
    let mut zoom = 1.0f32;
    let mut dim = 1024u32;
    for a in &args[4..] {
        if let Some(v) = a.strip_prefix("--yaw=") {
            yaw = v.parse().unwrap();
        } else if let Some(v) = a.strip_prefix("--pitch=") {
            pitch = v.parse().unwrap();
        } else if let Some(v) = a.strip_prefix("--zoom=") {
            zoom = v.parse().unwrap();
        } else if let Some(v) = a.strip_prefix("--size=") {
            dim = v.parse().unwrap();
        }
    }

    // --- 1. gunzip + parse vanilla structure NBT ---------------------------
    let raw = std::fs::read(nbt_path).expect("read nbt");
    let mut gz = GzDecoder::new(&raw[..]);
    let mut buf = Vec::new();
    gz.read_to_end(&mut buf).expect("gunzip nbt");
    let root: HashMap<String, Value> = fastnbt::from_bytes(&buf).expect("parse structure nbt");

    let Some(Value::List(palette)) = root.get("palette") else {
        panic!("structure has no palette list");
    };
    let states: Vec<String> = palette
        .iter()
        .map(|e| {
            let Value::Compound(c) = e else {
                panic!("palette entry not compound")
            };
            palette_state_string(c)
        })
        .collect();
    eprintln!("palette ({} states):", states.len());
    for s in &states {
        eprintln!("  {s}");
    }

    let Some(Value::List(blocks)) = root.get("blocks") else {
        panic!("structure has no blocks list");
    };

    // --- 2. rebuild as a UniversalSchematic --------------------------------
    let mut schem = UniversalSchematic::new("prefab-spike".to_string());
    let mut placed = 0usize;
    for b in blocks {
        let Value::Compound(bc) = b else {
            panic!("block not compound")
        };
        let Some(Value::List(pos)) = bc.get("pos") else {
            panic!("block missing pos")
        };
        let (x, y, z) = (as_i32(&pos[0]), as_i32(&pos[1]), as_i32(&pos[2]));
        let state_idx = as_i32(bc.get("state").expect("block missing state")) as usize;
        let state_str = &states[state_idx];
        let bs = BlockState::from_block_string(state_str).expect("parse block state");
        schem.set_block(x, y, z, &bs);
        placed += 1;
    }
    eprintln!("placed {placed} blocks");

    // --- 3. load resource pack (the 1.21.11 client jar) --------------------
    let t_pack = Instant::now();
    let pack = ResourcePackSource::from_file(pack_path).expect("load resource pack");
    eprintln!("resource pack loaded in {:.2}s", t_pack.elapsed().as_secs_f64());

    // --- 4. mesh -----------------------------------------------------------
    let t_mesh = Instant::now();
    let mesh: MeshOutput = schem
        .to_mesh(&pack, &MeshConfig::default())
        .expect("mesh schematic");
    let meshes = vec![mesh];
    let verts: usize = meshes.iter().map(|m| m.total_vertices()).sum();
    let tris: usize = meshes.iter().map(|m| m.total_triangles()).sum();
    eprintln!(
        "meshed in {:.2}s: {verts} verts, {tris} tris",
        t_mesh.elapsed().as_secs_f64()
    );

    // --- 5. render headlessly (wgpu -> Metal, off-screen) ------------------
    let camera = CameraConfig {
        yaw_deg: yaw,
        pitch_deg: pitch,
        zoom,
        fov_deg: 45.0,
        sphere_fit: true,
        background: Some([0.1, 0.1, 0.12, 1.0]),
        ..Default::default()
    };
    let t_gpu = Instant::now();
    let renderer = pollster::block_on(GpuRenderer::new(&meshes, dim, dim, None))
        .expect("create GPU renderer");
    let pixels = renderer.render_frame(&camera).expect("render frame");
    eprintln!("rendered in {:.2}s", t_gpu.elapsed().as_secs_f64());

    let img = image::RgbaImage::from_raw(dim, dim, pixels).expect("build image");
    img.save(out_path).expect("save png");
    eprintln!("saved {out_path}");
}
