//! The flat colour table paints every block the pinned version has.
//!
//! `delvec blocking-chart` draws its cutaway floor plans with
//! [`delvec::compiler::blocking::block_color`], and `delvec palette` prints the
//! same vendored table; the frames texture the same world from the pinned
//! client jar. A block the table cannot colour comes out as the
//! missing-texture magenta, so a creator reading a floor plan sees a plan that
//! lies about material — and the two surfaces disagree about a question that
//! has one answer.
//!
//! The container is the pinned block registry, enumerated here rather than
//! restated: every id 1.21.11 has, minus the states that are absence and the
//! two authoring markers whose magenta is a deliberate alarm.

use std::collections::BTreeMap;

use delvec::compiler::blocking::{FALLBACK_COLOR, block_color};
use delvec::compiler::view::blockcolor::{DEFAULT_BIOME, PaletteTable, is_air};
use delvec::schem::blocks::MC_VERSION;

/// Every block id the pinned version has, read from the vendored registry the
/// vendored appearance table is derived from.
///
/// Read from the file rather than reached through `delvewright_dsl::blocks`,
/// which exposes no iterator over its ids: that crate's number is the
/// campaign-format version every campaign document carries, and a test is not a
/// reason to move it. The path is the one `crates/delvec/data/PROVENANCE.md`
/// records, and `admit_fluid_predicates_agree.rs` reads the same file the same
/// way.
fn pinned_ids() -> Vec<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../dsl/data")
        .join(format!("blocks-{MC_VERSION}.json"));
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let registry: BTreeMap<String, serde_json::Value> =
        serde_json::from_slice(&bytes).expect("the vendored block registry is valid JSON");
    registry.into_keys().collect()
}

/// Authoring markers the solver strips before anything is drawn. Vanilla
/// deletes `jigsaw` at placement and `structure_block` is a tool, not scenery;
/// neither may reach the voxel model, and painting them the fallback is how a
/// leak announces itself. They are the only ids excluded by name.
const AUTHORING_MARKERS: &[&str] = &["minecraft:jigsaw", "minecraft:structure_block"];

#[test]
fn every_pinned_block_has_a_preview_colour() {
    let ids = pinned_ids();
    let total = ids.len();
    assert!(
        total > 1000,
        "the pinned registry is the container: {total}"
    );

    let mut air = 0usize;
    let mut markers = 0usize;
    let mut painted = 0usize;
    let mut unpainted: Vec<&str> = Vec::new();
    for id in ids.iter().map(String::as_str) {
        if is_air(id) {
            air += 1;
            continue;
        }
        if AUTHORING_MARKERS.contains(&id) {
            markers += 1;
            assert_eq!(
                block_color(id).0,
                FALLBACK_COLOR,
                "`{id}` is excluded because its magenta is the alarm; a colour here \
                 silences the alarm and makes the exclusion wrong"
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
        "{} of {} pinned block(s) fall to the missing-texture magenta in the colour table while \
         the frames paint them: {:?}",
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
    let ids = pinned_ids();
    assert_eq!(table.biome, DEFAULT_BIOME);
    assert!(
        table.unresolved.is_empty(),
        "blocks the pinned jar could not be read for: {:?}",
        table.unresolved
    );
    let air = ids.iter().filter(|id| is_air(id)).count();
    assert_eq!(
        table.entries.len(),
        ids.len() - air,
        "the table is derived from the registry, so it holds every non-air id"
    );
    for id in table.entries.keys() {
        assert!(
            ids.contains(id),
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
    let ids = pinned_ids();
    let fresh = PaletteTable::derive(&deriver, ids.iter().map(String::as_str));
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
