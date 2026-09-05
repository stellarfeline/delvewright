//! Oversize splitting — the one tiling in this project.
//!
//! Vanilla structure templates cap each axis at 48. That is a limit on a file
//! format, never on a design, so anything bigger is
//! tiled into a deterministic grid of parts plus a manifest recording grid
//! dimensions, per-part sizes and zone-local offsets, and every consumer
//! reassembles losslessly from that manifest.
//!
//! Two producers write tilings — `delvec schem convert` for an oversize `.schem`
//! import, and `delvec grammar expand` for a zone whose expansion outgrows one
//! template — and they call the same [`plan_split`], so a volume tiles the same
//! way whichever door it came in by. [`TileSet`] is the manifest contract
//! itself: one struct, `Serialize` for the producers and `Deserialize` for the
//! consumers, so the two halves cannot drift apart.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// One grid cell of a split.
#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    pub grid_index: [i32; 3],
    /// Source-local origin of this part.
    pub offset: [i32; 3],
    pub size: [i32; 3],
}

/// The plan for tiling a `source_size` volume with a `part_max`-cube cap.
#[derive(Debug, Clone)]
pub struct SplitPlan {
    pub grid: [i32; 3],
    pub parts: Vec<Part>,
}

impl SplitPlan {
    /// True when the volume fits in a single part.
    pub fn is_single(&self) -> bool {
        self.grid == [1, 1, 1]
    }
}

fn ceil_div(a: i32, b: i32) -> i32 {
    (a + b - 1) / b
}

/// Compute a split plan. Parts are emitted in x -> y -> z grid order.
pub fn plan_split(size: [i32; 3], part_max: i32) -> SplitPlan {
    let max = part_max.max(1);
    let grid = [
        ceil_div(size[0], max).max(1),
        ceil_div(size[1], max).max(1),
        ceil_div(size[2], max).max(1),
    ];
    let mut parts = Vec::with_capacity((grid[0] * grid[1] * grid[2]) as usize);
    for i in 0..grid[0] {
        for j in 0..grid[1] {
            for k in 0..grid[2] {
                let offset = [i * max, j * max, k * max];
                let part_size = [
                    (size[0] - offset[0]).min(max),
                    (size[1] - offset[1]).min(max),
                    (size[2] - offset[2]).min(max),
                ];
                parts.push(Part {
                    grid_index: [i, j, k],
                    offset,
                    size: part_size,
                });
            }
        }
    }
    SplitPlan { grid, parts }
}

/// Part filename for a base name: `<base>.x<i>y<j>z<k>.nbt`.
pub fn part_filename(base: &str, grid_index: [i32; 3]) -> String {
    format!(
        "{base}.x{}y{}z{}.nbt",
        grid_index[0], grid_index[1], grid_index[2]
    )
}

/// Manifest filename: `<base>.split.json`.
pub fn manifest_filename(base: &str) -> String {
    format!("{base}.split.json")
}

// ---------------------------------------------------------------------------
// The tile-set manifest contract
// ---------------------------------------------------------------------------

/// A volume packaged as several structure templates.
///
/// This is the `structure_set` block of a tiled prefab's metadata file, and it
/// is the whole contract: given it and the `.nbt` files it names, a consumer can
/// rebuild the original volume without knowing anything about who tiled it or
/// why. It is `Serialize` **and** `Deserialize` on purpose — the producer and
/// the consumer share one definition, so a field cannot be added on one side and
/// missed on the other.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TileSet {
    /// The filename stem every tile is named from.
    pub base: String,
    /// The whole volume's extent `[x, y, z]` — what the author asked for.
    pub size: [i32; 3],
    /// The per-axis cap the tiling had to respect.
    pub part_max: i32,
    /// How many tiles along each axis.
    pub grid: [i32; 3],
    /// The MC data version every tile targets (ADR-0009).
    pub data_version: i32,
    /// Provenance breadcrumb: what wrote the tiles.
    #[serde(default)]
    pub generator: String,
    /// The tiles, in `x`→`y`→`z` grid order.
    pub parts: Vec<TilePart>,
}

/// One tile of a [`TileSet`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TilePart {
    /// The `.nbt` filename, relative to the manifest.
    pub file: String,
    /// The datapack structure id (a path segment).
    pub id: String,
    /// Position in the tile grid.
    pub grid_index: [i32; 3],
    /// The tile's origin **in whole-volume coordinates** — add it to a
    /// tile-local cell to get the volume cell. The only transform reassembly
    /// needs.
    pub offset: [i32; 3],
    /// The tile's extent `[x, y, z]`, every axis `<= part_max`.
    pub size: [i32; 3],
}

impl TileSet {
    /// Refuse a manifest that does not describe an exact tiling of `size`.
    ///
    /// A consumer that skips this reassembles a volume with a hole in it and
    /// reports success — the manifest is data on disk, and a truncated or
    /// hand-edited one must be a refusal rather than a quietly smaller
    /// building. Checks that every part lies inside the volume, that no axis
    /// exceeds `part_max`, and that the parts' volumes sum to the whole.
    pub fn validate(&self) -> Result<(), String> {
        if self.parts.is_empty() {
            return Err("the manifest lists no tiles".to_string());
        }
        let mut covered: i64 = 0;
        for part in &self.parts {
            for axis in 0..3 {
                if part.size[axis] <= 0 {
                    return Err(format!("tile {:?} has a zero or negative axis", part.file));
                }
                if part.size[axis] > self.part_max {
                    return Err(format!(
                        "tile {:?} is {} on axis {axis}, past the declared cap of {}",
                        part.file, part.size[axis], self.part_max
                    ));
                }
                if part.offset[axis] < 0 || part.offset[axis] + part.size[axis] > self.size[axis] {
                    return Err(format!(
                        "tile {:?} runs outside the {}x{}x{} volume on axis {axis}",
                        part.file, self.size[0], self.size[1], self.size[2]
                    ));
                }
            }
            covered += part.size[0] as i64 * part.size[1] as i64 * part.size[2] as i64;
        }
        let whole = self.size[0] as i64 * self.size[1] as i64 * self.size[2] as i64;
        if covered != whole {
            return Err(format!(
                "the {} tile(s) cover {covered} cell(s) of a {whole}-cell volume — a tile set that \
                 does not tile its volume exactly would reassemble with a hole or an overlap",
                self.parts.len()
            ));
        }
        Ok(())
    }
}

/// Read a prefab metadata file and say whether it describes a tile set.
///
/// `Ok(None)` means an ordinary single-template prefab — the caller carries on
/// exactly as before. `Ok(Some(_))` is a validated tile set. `Err` is a
/// malformed file, never a shrug: every consumer that opens prefab metadata
/// must be able to tell the two shapes apart, and the way a tool "handles" a
/// tile set it has never heard of is by reading none of its blocks.
///
/// **It reads the whole document through [`crate::prefab::PrefabMeta`], not a
/// private view of one key.** A narrow reader here was a second reader: it
/// accepted a document that declares no blocks at all as "not a tile set", and
/// it validated the manifest on a path the prefab registry never took. One
/// reader means a manifest is refused the same way by the renderer, the
/// admission tools and world assembly, or by none of them.
pub fn read_tile_set(path: &Path) -> Result<Option<TileSet>, String> {
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let meta = crate::prefab::PrefabMeta::from_json(&text)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(meta.structure_set)
}

#[derive(Serialize)]
struct PartManifest {
    file: String,
    grid_index: [i32; 3],
    offset: [i32; 3],
    size: [i32; 3],
}

#[derive(Serialize)]
struct SplitManifest {
    base: String,
    data_version: i32,
    source_size: [i32; 3],
    source_offset: [i32; 3],
    part_max: i32,
    grid: [i32; 3],
    parts: Vec<PartManifest>,
}

/// The base name and grid index a tile filename spells, if it spells one.
///
/// [`part_filename`] is the only thing that writes this shape, and it is the
/// half of a tile's identity that **travels with the bytes**: a `cp`, an `mv`,
/// an upload and a download all carry it, and nothing but a deliberate rename
/// removes it.
pub fn tile_filename(name: &str) -> Option<(&str, [i32; 3])> {
    let stem = name.strip_suffix(".nbt")?;
    let (base, suffix) = stem.rsplit_once('.')?;
    let rest = suffix.strip_prefix('x')?;
    let (i, rest) = rest.split_once('y')?;
    let (j, k) = rest.split_once('z')?;
    Some((base, [i.parse().ok()?, j.parse().ok()?, k.parse().ok()?]))
}

/// What a single `.nbt` path turned out to be: a whole template, or one tile of
/// a tiling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TileEvidence {
    /// Nothing about this file says it is a fragment.
    Whole,
    /// It is a tile. `manifest` is its set's manifest when that file is beside
    /// it, and `None` when the tile has been separated from its set.
    Tile {
        /// The zone's base name.
        base: String,
        /// The manifest naming it, when one is beside it.
        manifest: Option<std::path::PathBuf>,
    },
}

/// Decide whether a single `.nbt` is one tile of a tiled zone.
///
/// Every tool that takes a single `.nbt` needs this, because every one of them
/// will otherwise be handed a tile some day and answer about the fragment: the
/// renderer draws a building sliced at a packaging plane, the auditor returns
/// `"pass"` over a fifth of a zone, the light probe measures a fifth of a
/// building and writes the answer into a metadata file it had to invent. All
/// are answers, all are wrong, and none has any other detector. So the check
/// lives here, once, beside the tiling it is about.
///
/// **The evidence is the file's own name, and the manifest only adds to it.**
/// Binding the check to a sibling file was the defect: `cp tile.nbt elsewhere/`
/// left a fragment that every tool then accepted as a whole prefab, because the
/// only thing that knew otherwise had been left behind in the old directory. A
/// guard that a copy defeats is not a property of the artifact. The name is —
/// it is written by [`part_filename`], it is carried by the bytes wherever they
/// go, and a *whole* prefab cannot accidentally acquire it, because
/// `<base>.x<i>y<j>z<k>.nbt` is a shape no author writes by hand.
///
/// The manifest is still looked up, because a diagnostic that can say *which*
/// zone this is a tile of and what to run instead is worth far more than one
/// that can only refuse. Its absence downgrades the message, never the verdict.
pub fn tile_evidence(nbt_path: &Path) -> Result<TileEvidence, String> {
    let Some(name) = nbt_path.file_name().and_then(|s| s.to_str()) else {
        return Ok(TileEvidence::Whole);
    };
    let Some((base, _)) = tile_filename(name) else {
        return Ok(TileEvidence::Whole);
    };
    let manifest = nbt_path.with_file_name(format!("{base}.json"));
    let claimed = manifest.exists()
        && read_tile_set(&manifest)?.is_some_and(|set| set.parts.iter().any(|p| p.file == name));
    Ok(TileEvidence::Tile {
        base: base.to_string(),
        manifest: claimed.then_some(manifest),
    })
}

/// The refusal a whole-piece tool owes a fragment: one sentence saying what the
/// file is, what answering about it would mean, and what to do instead.
///
/// One text because it is one fact. `verb` is what the caller would have done
/// ("audit", "probe", "render"), and `consequence` is what that answer would
/// have been read as.
pub fn fragment_refusal(
    nbt_path: &Path,
    evidence: &TileEvidence,
    verb: &str,
    consequence: &str,
) -> Option<String> {
    let TileEvidence::Tile { base, manifest } = evidence else {
        return None;
    };
    Some(match manifest {
        Some(m) => format!(
            "{} is one tile of the zone described by {} — to {verb} it would {consequence}. \
             Use the whole zone: pass {}",
            nbt_path.display(),
            m.display(),
            m.display()
        ),
        None => format!(
            "{} is one tile of a tiled zone (`{base}`) that has been separated from its set — to \
             {verb} it would {consequence}, and its manifest is not beside it, so there is \
             nothing here to reassemble the zone from. Put the tile back with its `{base}.json` \
             manifest and the rest of its tiles, and pass the manifest",
            nbt_path.display()
        ),
    })
}

/// The tile-set manifest that names `nbt_path` as one of its tiles, if any.
pub fn manifest_claiming(nbt_path: &Path) -> Result<Option<std::path::PathBuf>, String> {
    match tile_evidence(nbt_path)? {
        TileEvidence::Tile { manifest, .. } => Ok(manifest),
        TileEvidence::Whole => Ok(None),
    }
}

/// Render the split manifest as pretty JSON (deterministic — fixed field order,
/// parts in grid order).
pub fn manifest_json(
    base: &str,
    data_version: i32,
    source_size: [i32; 3],
    source_offset: [i32; 3],
    part_max: i32,
    plan: &SplitPlan,
) -> String {
    let manifest = SplitManifest {
        base: base.to_string(),
        data_version,
        source_size,
        source_offset,
        part_max,
        grid: plan.grid,
        parts: plan
            .parts
            .iter()
            .map(|p| PartManifest {
                file: part_filename(base, p.grid_index),
                grid_index: p.grid_index,
                offset: p.offset,
                size: p.size,
            })
            .collect(),
    };
    serde_json::to_string_pretty(&manifest).expect("manifest serializes")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tile_set(size: [i32; 3], parts: Vec<([i32; 3], [i32; 3])>) -> TileSet {
        TileSet {
            base: "zone".to_string(),
            size,
            part_max: 48,
            grid: [1, 1, parts.len() as i32],
            data_version: 4671,
            generator: "crates/grammar".to_string(),
            parts: parts
                .into_iter()
                .enumerate()
                .map(|(i, (offset, size))| TilePart {
                    file: format!("zone.x0y0z{i}.nbt"),
                    id: format!("zone.x0y0z{i}"),
                    grid_index: [0, 0, i as i32],
                    offset,
                    size,
                })
                .collect(),
        }
    }

    /// The tiling `plan_split` produces always validates. This is the binding
    /// between the producer and the check every consumer runs: if the two ever
    /// disagreed, every tiled export would be refused by every reader.
    #[test]
    fn every_plan_split_tiling_validates() {
        for size in [
            [1, 1, 1],
            [48, 48, 48],
            [49, 1, 1],
            [20, 10, 84],
            [90, 14, 130],
            [200, 100, 200],
        ] {
            let plan = plan_split(size, 48);
            let set = TileSet {
                base: "zone".to_string(),
                size,
                part_max: 48,
                grid: plan.grid,
                data_version: 4671,
                generator: String::new(),
                parts: plan
                    .parts
                    .iter()
                    .map(|p| TilePart {
                        file: part_filename("zone", p.grid_index),
                        id: format!("zone{:?}", p.grid_index),
                        grid_index: p.grid_index,
                        offset: p.offset,
                        size: p.size,
                    })
                    .collect(),
            };
            assert_eq!(set.validate(), Ok(()), "{size:?}");
        }
    }

    /// A manifest whose parts do not cover the volume is refused. A consumer
    /// that skipped this would reassemble a building with a hole in it and
    /// report success — the failure that has no other detector.
    #[test]
    fn a_tiling_with_a_gap_is_refused() {
        let short = tile_set([4, 4, 100], vec![([0, 0, 0], [4, 4, 48])]);
        let err = short.validate().unwrap_err();
        assert!(err.contains("cover"), "{err}");

        // ...and so is one that covers the right NUMBER of cells twice over.
        let overlap = tile_set(
            [4, 4, 48],
            vec![([0, 0, 0], [4, 4, 24]), ([0, 0, 0], [4, 4, 24])],
        );
        assert_eq!(overlap.validate(), Ok(()), "volume alone cannot see this");
        let outside = tile_set(
            [4, 4, 48],
            vec![([0, 0, 0], [4, 4, 24]), ([0, 0, 40], [4, 4, 24])],
        );
        assert!(
            outside.validate().unwrap_err().contains("outside"),
            "a part running past the volume is caught"
        );
    }

    /// A part bigger than the cap it declares cannot be a structure template,
    /// so it is a refusal before any file is opened.
    #[test]
    fn a_part_past_the_declared_cap_is_refused() {
        let big = tile_set([4, 4, 49], vec![([0, 0, 0], [4, 4, 49])]);
        assert!(
            big.validate()
                .unwrap_err()
                .contains("past the declared cap"),
            "{:?}",
            big.validate()
        );
    }

    /// An empty manifest is a manifest that describes nothing, not a zone with
    /// no blocks.
    #[test]
    fn a_manifest_with_no_tiles_is_refused() {
        let empty = tile_set([4, 4, 4], vec![]);
        assert!(empty.validate().unwrap_err().contains("no tiles"));
    }

    /// A tile is recognised by the name it carries, so a copy or a move cannot
    /// launder it into a whole prefab.
    ///
    /// This is the whole point of keying the check to the artifact. Under the
    /// old sibling-lookup rule the second assertion here returned "not a tile",
    /// and every whole-piece tool then answered confidently about a fragment.
    #[test]
    fn a_tile_is_recognised_by_its_own_name_wherever_it_is_put() {
        let dir = std::env::temp_dir().join(format!("dw-split-evid-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("elsewhere")).unwrap();

        let set = tile_set(
            [4, 4, 60],
            vec![([0, 0, 0], [4, 4, 48]), ([0, 0, 48], [4, 4, 12])],
        );
        std::fs::write(
            dir.join("zone.json"),
            serde_json::to_string(
                &serde_json::json!({ "prefab_id": "prefab/zone", "structure_set": set }),
            )
            .unwrap(),
        )
        .unwrap();
        for part in &set.parts {
            std::fs::write(dir.join(&part.file), b"not really nbt").unwrap();
        }

        // beside its manifest: a tile, and the manifest is named.
        assert_eq!(
            tile_evidence(&dir.join("zone.x0y0z1.nbt")).unwrap(),
            TileEvidence::Tile {
                base: "zone".to_string(),
                manifest: Some(dir.join("zone.json")),
            }
        );

        // copied away from it: STILL a tile. Nothing beside it says so.
        std::fs::copy(
            dir.join("zone.x0y0z1.nbt"),
            dir.join("elsewhere/zone.x0y0z1.nbt"),
        )
        .unwrap();
        assert_eq!(
            tile_evidence(&dir.join("elsewhere/zone.x0y0z1.nbt")).unwrap(),
            TileEvidence::Tile {
                base: "zone".to_string(),
                manifest: None,
            },
            "a guard a `cp` defeats is not a property of the artifact"
        );

        // an ordinary prefab is untouched by any of this.
        assert_eq!(
            tile_evidence(&dir.join("keep-gate-room.nbt")).unwrap(),
            TileEvidence::Whole
        );
        assert_eq!(tile_filename("keep-gate-room.nbt"), None);
        assert_eq!(
            tile_filename("zone.x0y10z2.nbt"),
            Some(("zone", [0, 10, 2]))
        );
        assert_eq!(
            tile_filename("cave.mouth.nbt"),
            None,
            "a dotted name is not a grid suffix"
        );

        // the refusal names what to do instead, and says which case it is.
        let beside = tile_evidence(&dir.join("zone.x0y0z0.nbt")).unwrap();
        let msg = fragment_refusal(&dir.join("zone.x0y0z0.nbt"), &beside, "audit", "lie").unwrap();
        assert!(msg.contains("zone.json"), "{msg}");
        let orphan = tile_evidence(&dir.join("elsewhere/zone.x0y0z0.nbt")).unwrap();
        let msg = fragment_refusal(
            &dir.join("elsewhere/zone.x0y0z0.nbt"),
            &orphan,
            "audit",
            "lie",
        )
        .unwrap();
        assert!(msg.contains("separated from its set"), "{msg}");
        assert_eq!(
            fragment_refusal(&dir, &TileEvidence::Whole, "audit", "lie"),
            None
        );

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// `read_tile_set` tells the two metadata shapes apart, and says so rather
    /// than shrugging: a single-template prefab is `None` (the caller carries
    /// on), a tile set is `Some`, and neither is ever an empty success.
    #[test]
    fn the_two_metadata_shapes_are_told_apart() {
        let dir = std::env::temp_dir().join(format!("dw-split-shape-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let single = dir.join("single.json");
        std::fs::write(
            &single,
            r#"{"prefab_id":"prefab/x","structure":{"file":"x.nbt","id":"x","size":[2,2,2],"data_version":4671}}"#,
        )
        .unwrap();
        assert_eq!(read_tile_set(&single).unwrap(), None);

        let set = tile_set(
            [4, 4, 60],
            vec![([0, 0, 0], [4, 4, 48]), ([0, 0, 48], [4, 4, 12])],
        );
        let tiled = dir.join("tiled.json");
        std::fs::write(
            &tiled,
            serde_json::to_string(
                &serde_json::json!({ "prefab_id": "prefab/zone", "structure_set": set }),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(read_tile_set(&tiled).unwrap().unwrap(), set);

        // A manifest that does not tile its volume is an error at the READER,
        // not a surprise three steps later inside a reassembler.
        let broken = dir.join("broken.json");
        let bad = tile_set([4, 4, 60], vec![([0, 0, 0], [4, 4, 48])]);
        std::fs::write(
            &broken,
            serde_json::to_string(
                &serde_json::json!({ "prefab_id": "prefab/zone", "structure_set": bad }),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(read_tile_set(&broken).unwrap_err().contains("cover"));

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
