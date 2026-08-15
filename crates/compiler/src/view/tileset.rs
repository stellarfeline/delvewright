//! Reviewing a tiled zone as ONE scene.
//!
//! A zone past the 48-per-axis structure-template cap ships as several `.nbt`
//! files plus a manifest (`delvewright_dsl::split::TileSet`). That is
//! packaging, and packaging must not reach the person doing the reviewing: an
//! author looking at a gate ward wants the gate ward, not tile 0,0,1 of it. So
//! the renderer reassembles the tiles into one [`Structure`] before it plans a
//! single shot, and everything downstream — the shot plan, the cutaways, the
//! filenames — behaves as though the zone had always been one file.
//!
//! The reverse direction matters as much: pointing the renderer at one tile of a
//! set is *refused*, not rendered. Rendering it would succeed, look plausible,
//! and show a building sliced in half at an arbitrary plane, which is the kind of
//! review that passes and means nothing.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvewright_dsl::split::{TileSet, fragment_refusal, read_tile_set, tile_evidence};

use crate::view::nbt::{NbtError, Structure, parse_structure};

/// What a path handed to `delve-render piece` turned out to be.
#[derive(Debug)]
pub enum PieceInput {
    /// One structure template, read as before.
    Single(Structure),
    /// A tile set, reassembled into the whole zone.
    Zone {
        /// The whole zone, as one structure.
        structure: Structure,
        /// How many tiles it came from.
        tiles: usize,
        /// The tile grid.
        grid: [i32; 3],
    },
}

impl PieceInput {
    /// The structure to render, whichever shape it arrived in.
    pub fn structure(&self) -> &Structure {
        match self {
            PieceInput::Single(s) => s,
            PieceInput::Zone { structure, .. } => structure,
        }
    }
}

/// Resolve the path an author passed into something renderable.
///
/// Accepts either a structure `.nbt` or a tile-set manifest `.json`. Returns the
/// resolved input and the metadata file to read anchors from.
pub fn load_piece(input: &Path) -> Result<(PieceInput, PathBuf), NbtError> {
    if input.extension().and_then(|s| s.to_str()) == Some("json") {
        let set = read_tile_set(input).map_err(NbtError)?;
        let Some(set) = set else {
            return Err(NbtError(format!(
                "{} is a single-template prefab's metadata, not a tile-set manifest — render it \
                 by passing the `.nbt` beside it",
                input.display()
            )));
        };
        let dir = input.parent().unwrap_or(Path::new("."));
        let structure = assemble(dir, &set)?;
        return Ok((
            PieceInput::Zone {
                structure,
                tiles: set.parts.len(),
                grid: set.grid,
            },
            input.to_path_buf(),
        ));
    }

    let evidence = tile_evidence(input).map_err(NbtError)?;
    if let Some(message) = fragment_refusal(
        input,
        &evidence,
        "render",
        "show the building cut in half at a packaging boundary",
    ) {
        return Err(NbtError(message));
    }

    let structure = parse_structure(input)?;
    Ok((PieceInput::Single(structure), input.with_extension("json")))
}

/// Rebuild a whole zone from its tiles.
///
/// Each tile's cells are translated by the manifest's zone-local `offset`, which
/// is the only transform there is. Palettes are merged by block-state string, so
/// tiles written with different palette orderings still reassemble correctly —
/// the exporter gives every tile of a set the same palette, but reassembly does
/// not get to assume its producer was ours.
pub fn assemble(dir: &Path, set: &TileSet) -> Result<Structure, NbtError> {
    set.validate().map_err(NbtError)?;

    let mut palette: Vec<String> = Vec::new();
    let mut index_of: BTreeMap<String, usize> = BTreeMap::new();
    let mut blocks: Vec<([i32; 3], usize)> = Vec::new();

    for part in &set.parts {
        let path = dir.join(&part.file);
        let tile = parse_structure(&path)?;
        if tile.size != part.size {
            return Err(NbtError(format!(
                "{}: the tile is {}x{}x{} but the manifest declares {}x{}x{} — the manifest and \
                 the tiles beside it are not the same export",
                path.display(),
                tile.size[0],
                tile.size[1],
                tile.size[2],
                part.size[0],
                part.size[1],
                part.size[2]
            )));
        }
        let remap: Vec<usize> = tile
            .palette
            .iter()
            .map(|state| match index_of.get(state) {
                Some(i) => *i,
                None => {
                    let i = palette.len();
                    palette.push(state.clone());
                    index_of.insert(state.clone(), i);
                    i
                }
            })
            .collect();
        for (pos, state) in &tile.blocks {
            blocks.push((
                [
                    pos[0] + part.offset[0],
                    pos[1] + part.offset[1],
                    pos[2] + part.offset[2],
                ],
                remap[*state],
            ));
        }
    }

    Ok(Structure {
        size: set.size,
        palette,
        blocks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use delvewright_dsl::split::TilePart;

    fn part(file: &str, offset: [i32; 3], size: [i32; 3]) -> TilePart {
        TilePart {
            file: file.to_string(),
            id: file.trim_end_matches(".nbt").to_string(),
            grid_index: [0, 0, 0],
            offset,
            size,
        }
    }

    fn set(size: [i32; 3], parts: Vec<TilePart>) -> TileSet {
        TileSet {
            base: "zone".to_string(),
            size,
            part_max: 48,
            grid: [1, 1, 2],
            data_version: 4671,
            generator: "crates/grammar".to_string(),
            parts,
        }
    }

    /// A manifest whose tiles do not cover the zone is refused rather than
    /// reassembled into a building with a hole in it.
    #[test]
    fn a_manifest_that_does_not_tile_its_zone_is_refused() {
        let short = set(
            [4, 4, 100],
            vec![part("zone.x0y0z0.nbt", [0, 0, 0], [4, 4, 48])],
        );
        let err = assemble(Path::new("/nonexistent"), &short).unwrap_err();
        assert!(err.0.contains("cover"), "{}", err.0);
        assert!(err.0.contains("hole"), "{}", err.0);
    }

    /// ...and one whose tiles exceed the cap it declares, likewise: the fault is
    /// found before any file is opened.
    #[test]
    fn a_tile_past_the_declared_cap_is_refused() {
        let big = set(
            [4, 4, 49],
            vec![part("zone.x0y0z0.nbt", [0, 0, 0], [4, 4, 49])],
        );
        let err = assemble(Path::new("/nonexistent"), &big).unwrap_err();
        assert!(err.0.contains("past the declared cap"), "{}", err.0);
    }
}
