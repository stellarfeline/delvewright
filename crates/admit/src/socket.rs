//! **Socket carving**: declare a jigsaw socket on a converted piece.
//!
//! Mirrors the generator's doorway construction exactly (keep-socket-v1): carve a
//! `w × h` opening to air in the wall, drop a `minecraft:jigsaw` marker at the
//! bottom-centre cell (`local_pos`) with the structure-form jigsaw block entity,
//! and append the matching `connectors[]` entry to the metadata. The result is
//! byte-identical in shape to a generated socket, so the compiler's jigsaw solver
//! mates it like any library piece.

use std::collections::BTreeMap;

use delvewright_schem::nbt::Nbt;

use crate::meta::{Connector, PrefabMeta};
use crate::structure::{PaletteEntry, Structure};

/// A socket declaration.
#[derive(Debug, Clone)]
pub struct SocketDecl {
    /// The jigsaw cell — bottom-centre of the opening, at the wall.
    pub local_pos: [i32; 3],
    /// Outward facing (`north`/`south`/`east`/`west`).
    pub facing: String,
    /// Opening `[width, height]` (default `[3, 3]`).
    pub opening: [i32; 2],
    /// Jigsaw name/target (default `keep:socket`) and pool (default `keep:pool`).
    pub name: String,
    pub target: String,
    pub pool: String,
}

impl SocketDecl {
    pub fn new(local_pos: [i32; 3], facing: &str) -> SocketDecl {
        SocketDecl {
            local_pos,
            facing: facing.to_string(),
            opening: [3, 3],
            name: "keep:socket".to_string(),
            target: "keep:socket".to_string(),
            pool: "keep:pool".to_string(),
        }
    }
}

/// The vanilla jigsaw `orientation` for a horizontal outward `facing`.
fn orientation(facing: &str) -> Result<&'static str, String> {
    Ok(match facing {
        "north" => "north_up",
        "south" => "south_up",
        "west" => "west_up",
        "east" => "east_up",
        other => {
            return Err(format!(
                "unsupported socket facing `{other}` (cardinal only)"
            ));
        }
    })
}

/// The `w × h` opening cells at a wall socket, given the jigsaw (bottom-centre)
/// cell, facing axis, and opening size. Width is centred on the jigsaw cell;
/// height climbs from the jigsaw cell.
fn opening_cells(local_pos: [i32; 3], facing: &str, opening: [i32; 2]) -> Vec<[i32; 3]> {
    let [px, py, pz] = local_pos;
    let (w, h) = (opening[0], opening[1]);
    let half = (w - 1) / 2;
    let mut cells = Vec::new();
    for dh in 0..h {
        for d in -half..=(w - 1 - half) {
            let cell = match facing {
                // wall lies in the z plane: spread across x.
                "north" | "south" => [px + d, py + dh, pz],
                // wall lies in the x plane: spread across z.
                "west" | "east" => [px, py + dh, pz + d],
                _ => [px, py + dh, pz],
            };
            cells.push(cell);
        }
    }
    cells
}

/// Carve `decl` into `s` and record it in `meta`. Idempotent on the metadata side.
pub fn carve(s: &mut Structure, meta: &mut PrefabMeta, decl: &SocketDecl) -> Result<(), String> {
    let orient = orientation(&decl.facing)?;
    if !s.in_bounds(decl.local_pos) {
        return Err(format!(
            "socket local_pos {:?} is outside the structure bounds {:?}",
            decl.local_pos, s.size
        ));
    }
    // Carve the opening to air (all cells except the jigsaw cell itself).
    for cell in opening_cells(decl.local_pos, &decl.facing, decl.opening) {
        if !s.in_bounds(cell) {
            return Err(format!(
                "socket opening cell {cell:?} is outside the structure bounds {:?}",
                s.size
            ));
        }
        if cell != decl.local_pos {
            s.set_cell(cell, PaletteEntry::simple("minecraft:air"), None);
        }
    }
    // Place the jigsaw marker with the structure-form block entity.
    let entry = PaletteEntry::with_props("minecraft:jigsaw", &[("orientation", orient)]);
    let mut be: BTreeMap<String, Nbt> = BTreeMap::new();
    be.insert(
        "id".to_string(),
        Nbt::String("minecraft:jigsaw".to_string()),
    );
    be.insert("name".to_string(), Nbt::String(decl.name.clone()));
    be.insert("target".to_string(), Nbt::String(decl.target.clone()));
    be.insert("pool".to_string(), Nbt::String(decl.pool.clone()));
    be.insert(
        "final_state".to_string(),
        Nbt::String("minecraft:air".to_string()),
    );
    be.insert("joint".to_string(), Nbt::String("aligned".to_string()));
    s.set_cell(decl.local_pos, entry, Some(Nbt::Compound(be)));

    // Record the connector in metadata.
    meta.add_connector(Connector {
        name: decl.name.clone(),
        target: decl.target.clone(),
        local_pos: decl.local_pos,
        facing: decl.facing.clone(),
        opening: decl.opening,
        joint: "aligned".to_string(),
    });
    Ok(())
}
