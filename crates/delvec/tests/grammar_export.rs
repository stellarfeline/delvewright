//! Freezing a grammar program as a prefab (spec-0027 §2): the `.nbt` and the
//! metadata beside it, and the ADR-0006 promise that both regenerate byte for
//! byte.

use delvec::grammar::export::{
    GENERATOR, MAX_STRUCTURE_AXIS, UNBOUND_LIGHTING_PROFILE, ZoneExport, export_prefab,
    export_zone, program_hash,
};
use delvec::grammar::library::{castle, church, temple};
use delvec::grammar::{Box3, ExpandOptions, Program};

/// The temple region the rest of the test suite uses, small enough for a
/// vanilla structure template on every axis.
const TEMPLE_REGION: Box3 = Box3::at_origin([13, 14, 21]);
/// The castle region — the library's one program that declares an anchor.
const CASTLE_REGION: Box3 = Box3::at_origin([41, 14, 25]);
/// A castle stretched past the structure-template cap on two axes, so a tiling
/// is a grid and not a row. The castle is the library's one program with a
/// `mark`, which is what makes it the right subject: an anchor must survive
/// tiling in zone coordinates.
const TILED_REGION: Box3 = Box3::at_origin([90, 14, 130]);

fn cases() -> Vec<(Program, Box3, &'static str)> {
    vec![
        (temple(), TEMPLE_REGION, "grammar-temple"),
        (castle(), CASTLE_REGION, "grammar-castle"),
        (church(), Box3::at_origin([15, 16, 30]), "grammar-church"),
    ]
}

/// ADR-0006, the whole point of the provenance row: the same program at the
/// same seed over the same region gives the same two files, bit for bit — not
/// merely an equivalent building.
#[test]
fn exporting_twice_gives_byte_identical_nbt_and_metadata() {
    for (program, region, id) in cases() {
        for seed in [0u64, 1, 7, u64::MAX] {
            let opts = ExpandOptions::seeded(seed);
            let a = export_prefab(&program, region, &opts, id).unwrap();
            let b = export_prefab(&program, region, &opts, id).unwrap();
            assert_eq!(a.nbt, b.nbt, "{id} .nbt drifted at seed {seed}");
            assert_eq!(
                a.metadata_json, b.metadata_json,
                "{id} metadata drifted at seed {seed}"
            );
            // The metadata now carries declared anchors, so this is a promise
            // about them too — and the castle proves it is not a promise about
            // an empty map.
            assert_eq!(a.metadata.anchors, b.metadata.anchors);
            if id == "grammar-castle" {
                assert!(!a.metadata.anchors.is_empty());
            }
        }
    }
}

/// The same promise for a zone that does not fit one structure template. The
/// tiling is a pure function of the region, so *every* tile and the manifest
/// reproduce byte for byte — a cut that moved with the seed, the clock or a hash
/// ordering would show up here as a changed tile, and the export would still
/// "work" in every other test.
#[test]
fn exporting_a_tiled_zone_twice_gives_byte_identical_tiles_and_manifest() {
    for seed in [0u64, 1, 7, u64::MAX] {
        let opts = ExpandOptions::seeded(seed);
        let a = tiled(&castle(), TILED_REGION, &opts, "grammar-keep");
        let b = tiled(&castle(), TILED_REGION, &opts, "grammar-keep");
        assert_eq!(
            a.metadata_json, b.metadata_json,
            "the manifest drifted at seed {seed}"
        );
        assert_eq!(a.tiles.len(), b.tiles.len());
        for (x, y) in a.tiles.iter().zip(&b.tiles) {
            assert_eq!(x.file, y.file);
            assert_eq!(x.nbt, y.nbt, "tile {} drifted at seed {seed}", x.file);
        }
        assert!(
            !a.metadata.anchors.is_empty(),
            "the castle's mark must reach a tiled manifest too"
        );
    }
}

/// `export_zone` on a region a template can hold is `export_prefab`, to the
/// byte. Tiling must not change the ordinary case in any way — not the bytes,
/// not the filenames, not one key of the metadata.
#[test]
fn a_zone_that_fits_one_template_exports_exactly_as_before() {
    for (program, region, id) in cases() {
        let opts = ExpandOptions::seeded(7);
        let direct = export_prefab(&program, region, &opts, id).unwrap();
        let ZoneExport::Single(via_zone) = export_zone(&program, region, &opts, id).unwrap() else {
            panic!("{id} fits a template and must not be tiled");
        };
        assert_eq!(via_zone.nbt, direct.nbt, "{id} .nbt");
        assert_eq!(
            via_zone.metadata_json, direct.metadata_json,
            "{id} metadata"
        );
        assert_eq!(via_zone.structure_file, direct.structure_file);
        assert_eq!(via_zone.metadata_file, direct.metadata_file);
    }

    // ...including at exactly the cap, which is the boundary the arithmetic
    // could plausibly be wrong at in either direction.
    let cap = MAX_STRUCTURE_AXIS;
    let at_cap = Box3::at_origin([cap, cap, cap]);
    let opts = ExpandOptions::seeded(0);
    assert!(
        matches!(
            export_zone(&temple(), at_cap, &opts, "grammar-temple").unwrap(),
            ZoneExport::Single(_)
        ),
        "a region of exactly {cap} on every axis is one template, not four"
    );
    let over = Box3::at_origin([cap, cap, cap + 1]);
    assert!(
        matches!(
            export_zone(&temple(), over, &opts, "grammar-temple").unwrap(),
            ZoneExport::Tiled(_)
        ),
        "one cell past the cap is a tile set, and never a refusal"
    );
}

/// The manifest describes the ZONE: the whole region, the seed and program hash
/// that regenerate it, zone-relative tile offsets, and every anchor in zone
/// coordinates. A reader of this file never needs to know a cut happened except
/// to find the files.
#[test]
fn the_manifest_describes_the_zone_and_not_a_tile() {
    let set = tiled(
        &castle(),
        TILED_REGION,
        &ExpandOptions::seeded(7),
        "grammar-keep",
    );
    let json: serde_json::Value = serde_json::from_str(&set.metadata_json).unwrap();

    assert_eq!(json["prefab_id"], "prefab/grammar-keep");
    assert!(
        json.get("structure").is_none(),
        "a tile set must not pretend to be one structure: a consumer that has not learned about \
         tile sets has to FAIL to parse this, not read it as a prefab with no blocks"
    );
    let set_json = &json["structure_set"];
    assert_eq!(set_json["size"], serde_json::json!([90, 14, 130]));
    assert_eq!(set_json["part_max"], MAX_STRUCTURE_AXIS);
    assert_eq!(set_json["grid"], serde_json::json!([2, 1, 3]));
    assert_eq!(set_json["data_version"], 4671);
    assert_eq!(set_json["generator"], GENERATOR);
    // **The zone's light, measured over the whole zone** — a tiled zone is one
    // building and light crosses a packaging plane like any other cell, so the
    // figure is taken before the cut and there is one of it, not one per tile.
    assert_ne!(
        json["lighting"]["profile"], UNBOUND_LIGHTING_PROFILE,
        "a keep with floor in it is measured at export, not left for a hand step: {}",
        json["lighting"]
    );
    assert!(
        json["lighting"]["measured_min_light"].is_number()
            && json["lighting"]["method"]
                .as_str()
                .unwrap_or_default()
                .contains("min over"),
        "the measurement states the binding it was taken over: {}",
        json["lighting"]
    );

    // The provenance row regenerates the whole set at once, not a tile.
    assert_eq!(json["license"]["generated_by"]["seed"], 7);
    assert_eq!(
        json["license"]["generated_by"]["program_hash"],
        program_hash(&castle())
    );

    // Every anchor is in zone coordinates: the same values the untiled export of
    // the same expansion would carry, and inside the zone rather than inside a
    // tile.
    assert!(!set.metadata.anchors.is_empty());
    for (name, anchor) in &set.metadata.anchors {
        // A grammar `mark` is always a point anchor; the document's `pos` is
        // optional because a gate anchor carries a region instead.
        let pos = anchor
            .pos
            .unwrap_or_else(|| panic!("{name} exported no pos"));
        for axis in 0..3 {
            assert!(
                pos[axis] >= 0 && pos[axis] < TILED_REGION.size[axis] as i32,
                "{name} at {pos:?} is outside the zone"
            );
        }
    }
    assert!(
        set.metadata
            .anchors
            .values()
            .filter_map(|a| a.pos)
            .any(|p| p.iter().any(|&c| c >= MAX_STRUCTURE_AXIS as i32)),
        "the castle's anchor sits past the cap on some axis, which is the whole point: a \
         tile-local coordinate would have been < {MAX_STRUCTURE_AXIS} and looked fine"
    );
}

/// The tiles are the zone, cut up — no cell lost, none duplicated, none moved.
///
/// Read back through the structure reader every consumer uses, so this is a
/// statement about the bytes on disk and not about the exporter's intentions.
#[test]
fn the_tiles_reassemble_into_exactly_the_expansion() {
    let set = tiled(
        &castle(),
        TILED_REGION,
        &ExpandOptions::seeded(7),
        "grammar-keep",
    );
    let model = &set.expansion.model;
    let parts = &set.metadata.structure_set.as_ref().unwrap().parts;
    assert_eq!(parts.len(), set.tiles.len());

    let mut covered = 0u64;
    for (part, tile) in parts.iter().zip(&set.tiles) {
        assert_eq!(part.file, tile.file);
        for axis in 0..3 {
            assert!(
                part.size[axis] <= MAX_STRUCTURE_AXIS as i32,
                "{} is {} on axis {axis}, which no structure template can hold",
                part.file,
                part.size[axis]
            );
        }
        let view = delvec::schem::convert::read_structure(&tile.nbt).unwrap();
        assert_eq!(view.size, part.size, "{}", part.file);
        for x in 0..part.size[0] {
            for y in 0..part.size[1] {
                for z in 0..part.size[2] {
                    let zone = [x + part.offset[0], y + part.offset[1], z + part.offset[2]];
                    assert_eq!(
                        view.state_at(x, y, z),
                        model.get(zone).unwrap().to_string(),
                        "{} cell {x},{y},{z} (zone {zone:?})",
                        part.file
                    );
                    covered += 1;
                }
            }
        }
    }
    assert_eq!(
        covered,
        TILED_REGION.volume(),
        "the tiles must cover the zone exactly once"
    );
}

/// Where the cuts fall depends on the region and nothing else. Two different
/// programs, two different seeds, one region: one tiling.
#[test]
fn the_cuts_are_a_function_of_the_region_alone() {
    let a = tiled(
        &castle(),
        TILED_REGION,
        &ExpandOptions::seeded(1),
        "grammar-keep",
    );
    let b = tiled(
        &temple(),
        TILED_REGION,
        &ExpandOptions::seeded(9999),
        "grammar-keep",
    );
    assert_eq!(a.metadata.grid(), b.metadata.grid());
    let layout = |s: &delvec::grammar::export::TileSetExport| -> Vec<([i32; 3], [i32; 3])> {
        s.metadata
            .structure_set
            .as_ref()
            .unwrap()
            .parts
            .iter()
            .map(|p| (p.offset, p.size))
            .collect()
    };
    assert_eq!(
        layout(&a),
        layout(&b),
        "a cut that moved with the program or the seed would make two exports of the same zone \
         unmergeable, and nothing else would notice"
    );
}

/// Every file the export names is a file it writes, under the name the manifest
/// gives — so a directory assembled by writing a tile set is self-consistent.
#[test]
fn writing_a_tile_set_lands_every_file_the_manifest_names() {
    let dir = std::env::temp_dir().join(format!("dw-grammar-tiles-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let set = tiled(
        &castle(),
        TILED_REGION,
        &ExpandOptions::seeded(7),
        "grammar-keep",
    );
    ZoneExport::Tiled(set.clone()).write_to_dir(&dir).unwrap();

    assert_eq!(
        std::fs::read_to_string(dir.join("grammar-keep.json")).unwrap(),
        set.metadata_json
    );
    for (part, tile) in set
        .metadata
        .structure_set
        .as_ref()
        .unwrap()
        .parts
        .iter()
        .zip(&set.tiles)
    {
        let on_disk = std::fs::read(dir.join(&part.file)).unwrap();
        assert_eq!(on_disk, tile.nbt, "{}", part.file);
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

/// Export `program` over an oversize region, insisting it tiled.
fn tiled(
    program: &Program,
    region: Box3,
    options: &ExpandOptions,
    id: &str,
) -> delvec::grammar::export::TileSetExport {
    match export_zone(program, region, options, id).unwrap() {
        ZoneExport::Tiled(set) => set,
        ZoneExport::Single(_) => panic!("{id} over {:?} should have tiled", region.size),
    }
}

/// ...and the provenance row is a real key, not decoration: change the seed or
/// the program and both the row and the bytes move together.
#[test]
fn the_provenance_row_identifies_the_bytes_it_sits_beside() {
    let program = temple();
    let one = export_prefab(
        &program,
        TEMPLE_REGION,
        &ExpandOptions::seeded(7),
        "grammar-temple",
    )
    .unwrap();
    // The export always states the row: `generated_by` is optional in the
    // document (an ingested piece has nothing that regenerates it) and never
    // optional here (an expansion always does).
    let row = one
        .metadata
        .license
        .as_ref()
        .and_then(|l| l.generated_by.as_ref())
        .expect("a grammar export always carries its regeneration inputs");
    assert_eq!(row.seed, 7);
    assert_eq!(row.generator, "grammar");
    assert_eq!(row.program, "temple");
    assert_eq!(row.program_hash, program_hash(&program));

    // A different seed is a different row. (The temple is deterministic in
    // shape, so the NBT may or may not move; the row must.)
    let other = export_prefab(
        &program,
        TEMPLE_REGION,
        &ExpandOptions::seeded(8),
        "grammar-temple",
    )
    .unwrap();
    assert_ne!(
        one.metadata_json, other.metadata_json,
        "the seed must reach the metadata"
    );

    // A different program is a different hash *and* different bytes.
    let mut sandstone = temple();
    sandstone.set_param("roof", 1).unwrap();
    let restyled = export_prefab(
        &sandstone,
        TEMPLE_REGION,
        &ExpandOptions::seeded(7),
        "grammar-temple",
    )
    .unwrap();
    assert_ne!(
        restyled
            .metadata
            .license
            .and_then(|l| l.generated_by)
            .unwrap()
            .program_hash,
        one.metadata
            .license
            .clone()
            .and_then(|l| l.generated_by)
            .unwrap()
            .program_hash
    );
    assert_ne!(restyled.nbt, one.nbt);
}

/// A program that declares anchors exports them, in the hand-built field shape
/// (`anchors: { "<name>": { pos, facing } }`) — the castle's courtyard staging
/// point, at the floor centre of the scope the `mark` sits on.
///
/// The position is arithmetic, not a snapshot: a 41x14x25 region puts the
/// castle's plan on world X, the layout leaves the middle X band (9..32) to
/// `castle_center`, and its Z split leaves the courtyard at z 9..16 — floor
/// centre `[9 + 11, 0, 9 + 3]`.
#[test]
fn a_marked_program_exports_the_anchors_it_declared() {
    let export = export_prefab(
        &castle(),
        CASTLE_REGION,
        &ExpandOptions::seeded(7),
        "grammar-castle",
    )
    .unwrap();
    let json: serde_json::Value = serde_json::from_str(&export.metadata_json).unwrap();
    assert_eq!(
        json["anchors"],
        serde_json::json!({
            "anchor/courtyard": { "pos": [20, 0, 12], "facing": "north" }
        }),
        "the castle's `mark` must reach the metadata"
    );
}

/// The metadata a grammar prefab exports is exactly the hand-built shape minus
/// what expansion cannot know. The omissions are load-bearing, so they are
/// asserted rather than left to review — and the empty `anchors` of the temple,
/// which declares no marks, is the honest counterpart of the castle's map above
/// rather than a stub.
#[test]
fn the_metadata_declares_no_anchors_no_sockets_and_no_measurement() {
    let export = export_prefab(
        &temple(),
        TEMPLE_REGION,
        &ExpandOptions::seeded(7),
        "grammar-temple",
    )
    .unwrap();

    assert!(
        export.metadata.anchors.is_empty(),
        "a program that declares no anchor exports no anchor: nothing infers one \
         from the block pattern after the fact"
    );
    let json: serde_json::Value = serde_json::from_str(&export.metadata_json).unwrap();
    assert_eq!(
        json["connectors"],
        serde_json::json!([]),
        "jigsaw socketing is its own design and a guessed socket is worse than none — but the \
         export SAYS so with an empty list: `no sockets` and `written before sockets existed` \
         are different claims, and a reader that cannot tell them apart is the whole reason \
         this document has one shape"
    );
    // **The piece's light, measured over the bytes this export just froze.** The
    // export used to declare `unmeasured` for every piece, which made
    // `delvec prefab lighting --write` and the next `expand` a pair of
    // mutually-defeating actions: the command wrote the profile the expansion
    // then reset. `an_expansion_measures_its_own_light_and_a_re_expansion_keeps_it`
    // is the standing proof that the pair is closed.
    assert_ne!(
        json["lighting"]["profile"], UNBOUND_LIGHTING_PROFILE,
        "a temple with floor in it is measured at export: {}",
        json["lighting"]
    );
    assert!(
        json["lighting"]["measured_min_light"].is_number()
            && json["lighting"]["measured"].is_string(),
        "a measured profile carries its measurement — the type refuses one without: {}",
        json["lighting"]
    );
    assert!(
        json["lighting"]["method"]
            .as_str()
            .unwrap_or_default()
            .contains("NOT a live-server probe"),
        "the record says what kind of measurement it is, and never claims to be the live one: {}",
        json["lighting"]
    );

    assert_eq!(json["prefab_id"], "prefab/grammar-temple");
    assert_eq!(json["structure"]["file"], "grammar-temple.nbt");
    assert_eq!(json["structure"]["id"], "grammar-temple");
    assert_eq!(json["structure"]["data_version"], 4671);
    assert_eq!(json["structure"]["generator"], GENERATOR);
    assert_eq!(json["structure"]["size"], serde_json::json!([13, 14, 21]));
    assert_eq!(json["license"]["spdx"], "GPL-3.0-or-later");
    assert_eq!(json["license"]["source"], "original");
    assert!(export.metadata_json.ends_with("}\n"));
}

/// The declared `structure.size` is the region, and the `.nbt` really holds that
/// many cells at local coordinates — the metadata is not describing a different
/// file.
#[test]
fn the_structure_matches_the_size_the_metadata_declares() {
    let export = export_prefab(
        &temple(),
        TEMPLE_REGION,
        &ExpandOptions::seeded(7),
        "grammar-temple",
    )
    .unwrap();
    let view = delvec::schem::convert::read_structure(&export.nbt).unwrap();
    assert_eq!(view.size, export.metadata.size());
    assert_eq!(view.data_version, export.metadata.data_version().unwrap());

    // ...and every cell is the block the model put there, rebased to local.
    let model = &export.expansion.model;
    for pos in TEMPLE_REGION.positions() {
        let expected = model.get(pos).unwrap().to_string();
        assert_eq!(
            view.state_at(pos[0], pos[1], pos[2]),
            expected,
            "cell {pos:?}"
        );
    }
}

/// A structure template is local-coordinate, so where the box sat in the world
/// cannot show up in the frozen prefab. If it ever did, two exports of "the same
/// piece" would differ for no reason a reviewer could see.
#[test]
fn moving_the_region_does_not_move_the_exported_bytes() {
    let program = temple();
    let opts = ExpandOptions::seeded(3);
    let here = export_prefab(&program, TEMPLE_REGION, &opts, "grammar-temple").unwrap();
    let there = export_prefab(
        &program,
        Box3::new([-104, 62, 813], TEMPLE_REGION.size),
        &opts,
        "grammar-temple",
    )
    .unwrap();
    assert_eq!(here.nbt, there.nbt);
    assert_eq!(here.metadata_json, there.metadata_json);
}

/// Both files land under the names the metadata itself gives, so a library
/// directory assembled by writing exports is internally consistent.
#[test]
fn writing_an_export_uses_the_names_the_metadata_declares() {
    let dir = std::env::temp_dir().join(format!("dw-grammar-export-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let export = export_prefab(
        &temple(),
        TEMPLE_REGION,
        &ExpandOptions::seeded(7),
        "grammar-temple",
    )
    .unwrap();
    export.write_to_dir(&dir).unwrap();

    let nbt = std::fs::read(dir.join(export.metadata.templates()[0].file)).unwrap();
    let meta = std::fs::read_to_string(dir.join("grammar-temple.json")).unwrap();
    assert_eq!(nbt, export.nbt);
    assert_eq!(meta, export.metadata_json);

    std::fs::remove_dir_all(&dir).unwrap();
}

/// Not an assertion — a way to read the exact metadata the exporter writes,
/// for docs and review: `cargo test -p delvec --test grammar_export -- \
/// --ignored --nocapture show_the_metadata`.
#[test]
#[ignore = "prints the exported metadata for review; asserts nothing"]
fn show_the_metadata() {
    for (program, region, id) in cases() {
        let export = export_prefab(&program, region, &ExpandOptions::seeded(7), id).unwrap();
        print!("{}", export.metadata_json);
    }
}

// ---------------------------------------------------------------------------
// `shown_faces` — the program's claim about its own outside (`DW0885`)
// ---------------------------------------------------------------------------

/// **The export writes the program's declaration, on every expansion.**
///
/// That last clause is the whole reason the field lives on the program. The
/// export REWRITES the metadata file every run, so a `shown_faces` typed into
/// the metadata by hand is erased by the next `delvec grammar expand` — and a
/// value a tool erases is not a declaration. Written from the program it comes
/// back every time, which this asserts by exporting twice.
#[test]
fn the_export_writes_the_program_s_shown_faces_every_time() {
    for _ in 0..2 {
        let export = export_prefab(
            &castle(),
            CASTLE_REGION,
            &ExpandOptions::seeded(7),
            "grammar-castle",
        )
        .unwrap();
        assert_eq!(
            export.metadata.shown_faces,
            vec!["north", "south", "east", "west"],
            "the exported document carries the program's own claim"
        );
        let json: serde_json::Value = serde_json::from_str(&export.metadata_json).unwrap();
        assert_eq!(
            json["shown_faces"],
            serde_json::json!(["north", "south", "east", "west"])
        );
    }
}

/// A zone too big for one template carries the claim too: it is a fact about
/// the BUILDING, so it belongs to the manifest and not to a tile.
#[test]
fn a_tiled_zone_carries_the_claim_on_its_manifest() {
    let set = tiled(
        &castle(),
        TILED_REGION,
        &ExpandOptions::seeded(7),
        "grammar-keep",
    );
    assert_eq!(
        set.metadata.shown_faces,
        vec!["north", "south", "east", "west"]
    );
}

/// **The corpus's declaration is held to the corpus's own bytes.**
///
/// `DW0885` refuses a declared side with no solid cell on it, and it does so at
/// build time in somebody else's campaign. The library's own claim is checked
/// here instead, where the program that makes it lives: every side the castle
/// declares has masonry on that plane of its box.
#[test]
fn every_side_the_castle_declares_has_a_block_on_it() {
    let export = export_prefab(
        &castle(),
        CASTLE_REGION,
        &ExpandOptions::seeded(7),
        "grammar-castle",
    )
    .unwrap();
    let size = CASTLE_REGION.size;
    let model = &export.expansion.model;
    for side in &export.metadata.shown_faces {
        let (axis, plane) = match side.as_str() {
            "west" => (0usize, 0i32),
            "east" => (0, size[0] as i32 - 1),
            "down" => (1, 0),
            "up" => (1, size[1] as i32 - 1),
            "north" => (2, 0),
            "south" => (2, size[2] as i32 - 1),
            other => panic!("{other} is not a side"),
        };
        let mut solid = 0usize;
        for a in 0..size[(axis + 1) % 3] as i32 {
            for b in 0..size[(axis + 2) % 3] as i32 {
                let mut c = [0i32; 3];
                c[axis] = plane;
                c[(axis + 1) % 3] = a;
                c[(axis + 2) % 3] = b;
                if model.get(c).is_some() {
                    solid += 1;
                }
            }
        }
        assert!(
            solid > 0,
            "the castle declares {side} and put nothing on it"
        );
    }
}

/// A side that is not one of the six, and a side written twice, are both
/// refused where they are written rather than at somebody's build.
#[test]
fn a_shown_face_that_is_not_a_side_is_refused() {
    for faces in [vec!["nrth"], vec!["north", "north"], vec!["top"]] {
        let mut p = castle();
        p.shown_faces = faces.iter().map(|s| s.to_string()).collect();
        let err = p.validate().expect_err("this list must be refused");
        assert!(
            format!("{err}").contains("`shown_faces`"),
            "{faces:?}: {err}"
        );
    }
}

/// The version fence: a document that declares an older version may not write
/// the field, and the refusal names the version that introduced it.
#[test]
fn shown_faces_is_fenced_by_the_document_s_version() {
    let older = castle().at_version("1.8.0");
    let err = older
        .validate()
        .expect_err("1.8.0 predates the shown_faces surface");
    let text = format!("{err}");
    assert!(text.contains("`shown_faces` list"), "{text}");
    assert!(text.contains("1.9.0"), "{text}");
    // And a program that declares none is unaffected at every version, which is
    // what keeps every document written before this compiling to the same bytes.
    let mut bare = castle().at_version("1.8.0");
    bare.shown_faces.clear();
    bare.validate().expect("an empty list writes nothing");
}

// ---------------------------------------------------------------------------
// The write-then-expand pair
// ---------------------------------------------------------------------------

/// **The test that found the defect, run against the repair.**
///
/// `DW0894` refuses to show a piece nobody has measured the light of. Its remedy
/// used to be `delvec prefab lighting --write`, and an export rewrites the whole
/// metadata document — so the next `delvec grammar expand` reset the field the
/// command had just written. Two actions, each undoing the other, one
/// prescribing what the other refuses: the pair defect this repository names,
/// and it made `DW0894` a diagnostic that would refuse every generated zone
/// unless a creator re-ran a measurement after every single expansion, with
/// nothing telling them to.
///
/// The repair is not to preserve the field. A lighting profile is a MEASUREMENT
/// of the bytes, and carrying one across an expansion that writes new bytes makes
/// the document's own `method` line ("min over N floor cell(s) …") false about
/// the piece it now sits beside. `shown_faces` took the preserve-and-write-through
/// shape correctly because it is a DECLARATION — part of what the building is —
/// and the discriminator between the two is exactly which kind of fact it is. So
/// expansion measures its own light, and the hand step disappears rather than
/// being protected.
///
/// Walked here end to end, through the real binary, in the order that found it:
/// expand, `--write`, expand again, read back.
#[test]
fn an_expansion_measures_its_own_light_and_a_re_expansion_keeps_it() {
    let bin = env!("CARGO_BIN_EXE_delvec");
    let dir = std::env::temp_dir().join(format!("delve-pair-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let doc = dir.join("pairtest.json");

    let expand = || {
        let r = std::process::Command::new(bin)
            .args(["grammar", "expand", "--program", "ambush-door"])
            .args(["--region", "11x5x13", "--id", "pairtest", "-o"])
            .arg(&dir)
            .output()
            .unwrap();
        assert!(r.status.success(), "{r:?}");
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&doc).unwrap()).unwrap();
        v["lighting"].clone()
    };

    // 1. Straight out of the expansion, with no hand step at all — which is the
    //    half that makes the pair impossible to re-enter, not merely survivable.
    let fresh = expand();
    assert_eq!(fresh["profile"], "dark", "{fresh}");
    assert!(fresh["measured_min_light"].is_number(), "{fresh}");

    // 2. The command still writes, and writes the same thing: one measurement,
    //    one implementation, two doors onto it.
    let w = std::process::Command::new(bin)
        .args(["prefab", "lighting"])
        .arg(dir.join("pairtest.nbt"))
        .arg("--write")
        .output()
        .unwrap();
    assert!(w.status.success(), "{w:?}");
    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&doc).unwrap()).unwrap();
    assert_eq!(written["lighting"], fresh, "the two doors agree exactly");

    // 3. And the expansion that used to erase it now re-derives it.
    assert_eq!(
        expand(),
        fresh,
        "the measurement survives the next expansion"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// A program with **nowhere in it to stand** exports `unmeasured`, and that is
/// the true answer rather than a missing one: with no player space there is no
/// floor to be dark and no measurement to state. Five of the rule library's 36
/// programs are that shape, and every one of them is a demonstration of an IR
/// construct rather than a building.
///
/// Inventing a profile for such a piece would be the default the field exists to
/// refuse — the same rule `walk_y` beside it already follows.
#[test]
fn a_program_with_no_player_space_exports_no_measurement() {
    let bin = env!("CARGO_BIN_EXE_delvec");
    let dir = std::env::temp_dir().join(format!("delve-nospace-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let r = std::process::Command::new(bin)
        .args(["grammar", "expand", "--program", "idiom-mirror"])
        .args([
            "--region", "15x11x2", "--seed", "1", "--id", "nospace", "-o",
        ])
        .arg(&dir)
        .output()
        .unwrap();
    assert!(r.status.success(), "{r:?}");
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("nospace.json")).unwrap()).unwrap();
    assert_eq!(v["lighting"]["profile"], UNBOUND_LIGHTING_PROFILE, "{v}");
    assert!(
        v["lighting"].get("measured_min_light").is_none(),
        "an unmeasured piece must not carry a fabricated measurement: {}",
        v["lighting"]
    );
    let _ = std::fs::remove_dir_all(&dir);
}
