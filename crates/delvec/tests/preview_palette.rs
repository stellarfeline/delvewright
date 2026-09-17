//! The CPU draft rasteriser paints every block the pinned version has.
//!
//! `delvec cameras --preview`, `delvec snapshot` and `delvec edit preview` draw
//! the assembled world with [`delvec::compiler::snapshot::block_color`]; the GPU
//! path textures the same world from the pinned client jar. A block the CPU
//! surface cannot colour comes out as the missing-texture magenta, so a creator
//! placing a camera sees a frame that judges composition and lies about
//! material — and the two surfaces disagree about a question that has one
//! answer.
//!
//! The container is the pinned block registry, enumerated here rather than
//! restated: every id 1.21.11 has, minus the states that are absence and the
//! two authoring markers whose magenta is a deliberate alarm.

use delvec::compiler::snapshot::{FALLBACK_COLOR, block_color};
use delvec::compiler::view::blockcolor::{DEFAULT_BIOME, PaletteTable, is_air};
use delvec::schem::blocks::{BlockRegistry, MC_VERSION};

/// Authoring markers the solver strips before anything is drawn. Vanilla
/// deletes `jigsaw` at placement and `structure_block` is a tool, not scenery;
/// neither may reach the voxel model, and painting them the fallback is how a
/// leak announces itself. They are the only ids excluded by name.
const AUTHORING_MARKERS: &[&str] = &["minecraft:jigsaw", "minecraft:structure_block"];

#[test]
fn every_pinned_block_has_a_preview_colour() {
    let registry = BlockRegistry::v1_21_11();
    let total = registry.ids().len();
    assert!(
        total > 1000,
        "the pinned registry is the container: {total}"
    );

    let mut air = 0usize;
    let mut markers = 0usize;
    let mut painted = 0usize;
    let mut unpainted: Vec<&str> = Vec::new();
    for id in registry.ids() {
        if is_air(id) {
            air += 1;
            continue;
        }
        if AUTHORING_MARKERS.contains(&id) {
            markers += 1;
            assert_eq!(
                block_color(id).0,
                FALLBACK_COLOR,
                "`{id}` is excluded because its magenta is the alarm; if it now has a \
                 colour the alarm is gone and the exclusion is wrong"
            );
            continue;
        }
        if block_color(id).0 == FALLBACK_COLOR {
            unpainted.push(id);
        } else {
            painted += 1;
        }
    }

    eprintln!(
        "preview palette binding: {painted} of {} paintable block(s) coloured, out of {total} in \
         the pinned registry ({air} air-like, {} authoring marker(s))",
        painted + unpainted.len(),
        markers
    );
    assert_eq!(air, 5, "the air-like states of 1.21.11");
    assert_eq!(markers, AUTHORING_MARKERS.len());
    assert!(
        unpainted.is_empty(),
        "{} of {} pinned block(s) fall to the missing-texture magenta in the CPU preview while \
         the GPU render paints them: {:?}",
        unpainted.len(),
        painted + unpainted.len(),
        unpainted
    );
}

/// **The pin bump is the event that can make the table wrong, so the pin is what
/// the table is held to** — here, with no jar and therefore in CI.
///
/// A colour is a fact about one Minecraft version's textures. Moving ADR-0009's
/// pin moves every one of them, and nothing else in this repository would
/// notice: the key set survives a version that adds no block, the file parses,
/// and the frames come out plausible and wrong. The table records the version
/// its asset source declared; this holds that to the engine's own pin, so the
/// commit that moves the pin is red until `tools/maintenance/refresh-block-
/// appearance.py` has been run against the new jar.
#[test]
fn the_vendored_table_is_the_pinned_version_s() {
    let table = PaletteTable::pinned();
    assert_eq!(
        table.mc_version.as_deref(),
        Some(MC_VERSION),
        "the vendored appearance table is {:?} and the engine is pinned to {MC_VERSION}. Every \
         colour in it is a fact about one version's textures, so the table is re-derived against \
         the new jar: `python3 tools/maintenance/refresh-block-appearance.py <client.jar>`",
        table.mc_version
    );
}

/// The vendored table is the whole pinned registry minus absence, and it
/// resolved all of it: an entry the derivation could not make would be a block
/// the interactive viewer draws as the placeholder too, and there are none.
#[test]
fn the_vendored_table_covers_the_registry_it_was_derived_from() {
    let table = PaletteTable::pinned();
    let registry = BlockRegistry::v1_21_11();
    assert_eq!(table.biome, DEFAULT_BIOME);
    assert!(
        table.unresolved.is_empty(),
        "blocks the pinned jar could not be read for: {:?}",
        table.unresolved
    );
    let air = registry.ids().filter(|id| is_air(id)).count();
    assert_eq!(
        table.entries.len(),
        registry.ids().len() - air,
        "the table is derived from the registry, so it holds every non-air id"
    );
    for id in table.entries.keys() {
        assert!(
            registry.has(id),
            "`{id}` is in the vendored table and not in the pinned registry"
        );
    }
}

/// The second observer. The committed table stands in for the jar on machines
/// that have none, so the one thing it must be is what the jar says — measured
/// by re-deriving it here rather than by trusting the file.
///
/// `#[ignore]` by default: it needs the 1.21.11 client jar, which is EULA-bound
/// and never committed, so CI has none. **Its entry point is
/// `tools/maintenance/refresh-block-appearance.py`**, which is the pin-bump step
/// that re-derives the table and then runs this: the one occasion a jar is in
/// hand is the one occasion the table can change, and the regeneration does not
/// count as done until this has compared every entry.
#[test]
#[ignore = "needs the 1.21.11 client jar"]
fn the_vendored_table_is_what_the_pinned_jar_says() {
    use delvec::compiler::view::assets::Assets;
    use delvec::compiler::view::blockcolor::Deriver;
    use delvec::compiler::view::cli::resolve_textures;

    let path = resolve_textures(None).expect("a client jar");
    let assets = Assets::open(std::path::Path::new(&path)).expect("open the jar");
    assert_eq!(
        assets.declared_version().as_deref(),
        Some(MC_VERSION),
        "{path} is not the pinned jar"
    );
    let deriver = Deriver::with_biome(&assets, DEFAULT_BIOME);
    let fresh = PaletteTable::derive(&deriver, BlockRegistry::v1_21_11().ids());
    let committed = PaletteTable::pinned();
    let moved: Vec<&String> = fresh
        .entries
        .iter()
        .filter(|(k, v)| committed.entries.get(*k) != Some(*v))
        .map(|(k, _)| k)
        .collect();
    eprintln!(
        "vendored table binding: {} of {} entry(ies) re-derived from {path} and compared",
        fresh.entries.len(),
        committed.entries.len()
    );
    assert!(
        moved.is_empty(),
        "{} entry(ies) differ from the jar — re-derive with \
         `python3 tools/maintenance/refresh-block-appearance.py {path}`: {:?}",
        moved.len(),
        moved
    );
    assert_eq!(fresh.entries.len(), committed.entries.len());
    assert_eq!(fresh.unresolved, committed.unresolved);
    assert_eq!(fresh.mc_version, committed.mc_version);
}

/// The frame itself, not just the lookup: a room built of the stone the report
/// named comes out with no missing-texture pixel in it.
///
/// This is the shape of what a creator saw — `delvec cameras --preview` draws
/// through exactly this call — and the reason it is a frame and not a table
/// lookup is that the fidelity gate judges pixels, so this is judged in the same
/// medium (`delvec::render::detect::is_magenta`).
#[test]
fn a_room_of_deepslate_draws_with_no_missing_texture_pixel() {
    use delvec::compiler::snapshot::{Camera, FrameOpts, VoxelGrid, render_frame};
    use delvec::render::detect::is_magenta;
    use std::collections::BTreeMap;

    // Walls, floor and dressing, all from the ids the report named.
    const WALL: &str = "minecraft:polished_deepslate";
    const FLOOR: &str = "minecraft:deepslate_tiles";
    const TRIM: &str = "minecraft:tuff_bricks";
    const DRESS: &str = "minecraft:cracked_stone_bricks";
    let mut blocks: BTreeMap<[i32; 3], String> = BTreeMap::new();
    for x in -8..=8 {
        for z in -8..=8 {
            blocks.insert([x, 63, z], FLOOR.to_string());
            blocks.insert([x, 70, z], TRIM.to_string());
            for y in 64..70 {
                if x == -8 || x == 8 || z == -8 || z == 8 {
                    blocks.insert([x, y, z], WALL.to_string());
                }
            }
        }
    }
    for x in -3..=3 {
        blocks.insert([x, 64, -7], DRESS.to_string());
    }
    let grid = VoxelGrid::build(&blocks);
    assert!(
        grid.unpainted().is_empty(),
        "unpainted: {:?}",
        grid.unpainted()
    );

    let frame = render_frame(
        &grid,
        &Camera {
            pos: [0.5, 66.0, 6.5],
            yaw: 180.0,
            pitch: 0.0,
            fov: 70.0,
        },
        &FrameOpts {
            width: 320,
            height: 180,
            sea_level: None,
            labels: false,
        },
    );
    let px = &frame.canvas.rgba;
    let total = (frame.width() as usize) * (frame.height() as usize);
    let magenta = (0..total)
        .filter(|i| is_magenta(px[i * 4], px[i * 4 + 1], px[i * 4 + 2]))
        .count();
    eprintln!("deepslate room binding: {total} pixel(s) drawn, {magenta} magenta");
    assert!(total > 0);
    assert_eq!(
        magenta, 0,
        "{magenta} of {total} pixels are the placeholder"
    );
}
