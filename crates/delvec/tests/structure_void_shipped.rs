//! **A `structure_void` cell is shipped as no block at all.**
//!
//! Every model the engine builds reads a template's `structure_void` as a cell
//! the piece does not place: the neighbour's blocks show through, and the
//! proofs walk that. The pinned game's `/place template` places every block a
//! template lists, the void block included, so a template shipped with its
//! voids writes a hole over whatever stands there — the shape a bridge deck was
//! deleted by at three bridge mouths of a site-plan build, while every proof
//! over the model passed. The emitter therefore ships each template with its
//! void cells omitted, which is also the only shape vanilla's own save writes.
//!
//! The case is hello-world assembled against a synthetic room whose interior
//! carries `structure_void` cells; the datapack's template must list none of
//! them, and the placement sentinel must be a block the world holds.

mod common;

use std::collections::BTreeMap;

use delvec::admit::structure::{PaletteEntry, STRUCTURE_VOID, Structure, as_placed};
use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::parse_campaign;

/// The cells written `structure_void`: a 3×3 patch of interior air one above
/// the floor slab, and the room's lowest corner — the cell the placement
/// sentinel's `(y, z, x)` order reaches first.
fn voided_cells() -> Vec<[i32; 3]> {
    let mut cells = vec![[0, 0, 0]];
    for x in 4..7 {
        for z in 4..7 {
            cells.push([x, 1, z]);
        }
    }
    cells
}

/// `common::box_nbt` with [`voided_cells`] written `structure_void`.
fn voided_box() -> (Vec<u8>, usize) {
    let plain = common::box_nbt([11, 6, 11], false);
    let mut s = Structure::read(&plain).expect("the box parses");
    for c in voided_cells() {
        s.set_cell(c, PaletteEntry::simple(STRUCTURE_VOID), None);
    }
    let placed = s.blocks.len() - voided_cells().len();
    (s.write(), placed)
}

fn build(nbt: &[u8]) -> BuildOutput {
    let dir = common::shown_prefabs_dir("structure-void-shipped");
    std::fs::write(dir.join("hello-room.nbt"), nbt).unwrap();
    common::declare_shown_faces(&dir, "hello-room");
    let prefabs = PrefabRegistry::load_dir(&dir).unwrap();
    let loaded = load_campaign_dir(&common::hello_world_dir()).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("hello-world parses");
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for t in area.pieces.iter().flat_map(|p| &p.templates) {
            structures.insert(t.structure_file.clone(), nbt.to_vec());
        }
    }
    emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
    .expect("the voided room builds")
}

fn shipped_templates(out: &BuildOutput) -> Vec<(&String, Structure)> {
    let templates: Vec<_> = out
        .iter()
        .filter(|(k, _)| k.contains("/structure/") && k.ends_with(".nbt"))
        .map(|(k, v)| (k, Structure::read(v).expect("a shipped template parses")))
        .collect();
    assert!(!templates.is_empty(), "the build ships no template at all");
    templates
}

#[test]
fn a_shipped_template_lists_no_structure_void() {
    let (nbt, placed) = voided_box();
    let source = Structure::read(&nbt).unwrap();
    assert!(
        source.block_names().contains(STRUCTURE_VOID),
        "the fixture must carry the void it is about"
    );
    let out = build(&nbt);
    for (path, t) in shipped_templates(&out) {
        assert!(
            !t.block_names().contains(STRUCTURE_VOID),
            "{path}: the template ships `structure_void`, which `/place template` writes into \
             the world over whatever stands there"
        );
        assert_eq!(
            t.blocks.len(),
            placed,
            "{path}: every block but the {} void cell(s) is still placed",
            voided_cells().len()
        );
        for c in voided_cells() {
            assert!(t.block_at(c).is_none(), "{path}: void cell {c:?} is listed");
        }
    }
}

#[test]
fn the_placement_sentinel_is_a_block_the_world_holds() {
    let (nbt, _) = voided_box();
    let out = build(&nbt);
    let verify = out
        .iter()
        .find(|(k, _)| k.ends_with("/function/place_verify.mcfunction"))
        .map(|(_, v)| String::from_utf8_lossy(v).into_owned())
        .expect("the build emits place_verify");
    let sentinels: Vec<&str> = verify
        .lines()
        .filter(|l| l.starts_with("execute if block "))
        .collect();
    assert!(!sentinels.is_empty(), "no template sentinel in:\n{verify}");
    for l in sentinels {
        assert!(
            !l.contains("structure_void"),
            "a sentinel on a void cell names a block the shipped template does not place: {l}"
        );
    }
}

/// `as_placed` over a template with no void returns `None` — the shipped bytes
/// are the library's, byte for byte — and over one with voids is a function of
/// its input: the same bytes twice give the same bytes.
#[test]
fn as_placed_is_identity_without_voids_and_deterministic_with_them() {
    let plain = common::box_nbt([11, 6, 11], false);
    assert!(as_placed(&plain).is_none());
    let (nbt, _) = voided_box();
    let a = as_placed(&nbt).expect("voids are removed");
    let b = as_placed(&nbt).expect("voids are removed");
    assert_eq!(a, b);
    let s = Structure::read(&a).unwrap();
    // The palette is re-indexed without the void: every block's state names a
    // real entry, and the entries are the box's own.
    for blk in &s.blocks {
        let name = &s.palette[blk.state as usize].name;
        assert_ne!(name, STRUCTURE_VOID);
    }
    assert!(
        as_placed(&a).is_none(),
        "a shipped template is already as placed"
    );
}
