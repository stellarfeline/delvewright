//! `dw-shot` — sandbox free-angle renderer over the engine's `delvewright-render`.
//!
//! Usage: dw-shot <structure.nbt> <out.png> <yaw_deg> <pitch_deg> <zoom> <size> <textures.jar>

use delvewright_render::nbt;
use delvewright_render::render::{self, RenderParams};

fn main() -> Result<(), String> {
    let a: Vec<String> = std::env::args().collect();
    if a.len() != 8 {
        return Err(
            "usage: dw-shot <nbt> <out.png> <yaw> <pitch> <zoom> <size> <textures.jar>".into(),
        );
    }
    let st = nbt::parse_structure(std::path::Path::new(&a[1])).map_err(|e| e.0)?;
    let pack = render::load_pack(&a[7])?;
    let dim: u32 = a[6].parse().map_err(|_| "bad size")?;
    let p = RenderParams {
        yaw_deg: a[3].parse().map_err(|_| "bad yaw")?,
        pitch_deg: a[4].parse().map_err(|_| "bad pitch")?,
        zoom: a[5].parse().map_err(|_| "bad zoom")?,
        target: None,
        dim,
    };
    let f = render::render_structure(&st, &pack, false, &p)?;
    image::save_buffer(&a[2], &f.rgba, f.width, f.height, image::ColorType::Rgba8)
        .map_err(|e| e.to_string())?;
    eprintln!("wrote {}", a[2]);
    Ok(())
}
