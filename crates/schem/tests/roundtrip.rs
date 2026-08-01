//! Round-trip, strip, determinism, and split/reassembly tests over the in-code
//! reference schematics (`delvewright_schem::fixtures`).

use std::collections::BTreeMap;

use delvewright_schem::convert::{DATA_VERSION, read_structure};
use delvewright_schem::diag::{DW_DATAVERSION, DW_SPLIT, DW_STRIP};
use delvewright_schem::fixtures;
use delvewright_schem::nbt::Nbt;
use delvewright_schem::split::{part_filename, plan_split};
use delvewright_schem::{ConvertOutput, convert};

fn single(bytes: &[u8]) -> Vec<u8> {
    match convert(bytes, "test", 48).unwrap().output {
        ConvertOutput::Single(v) => v,
        ConvertOutput::Split { .. } => panic!("expected single output"),
    }
}

/// The expected structure-palette state string for the basic fixture cell.
fn basic_expected(x: i32, y: i32, z: i32) -> &'static str {
    match fixtures::basic_block_at(x, y, z) {
        0 => "minecraft:air",
        1 => "minecraft:stone",
        _ => unreachable!(),
    }
}

#[test]
fn v2_basic_block_for_block() {
    let view = read_structure(&single(&fixtures::v2_basic())).unwrap();
    assert_eq!(view.data_version, DATA_VERSION);
    assert_eq!(view.size, [3, 3, 3]);
    for x in 0..3 {
        for y in 0..3 {
            for z in 0..3 {
                assert_eq!(
                    view.state_at(x, y, z),
                    basic_expected(x, y, z),
                    "cell {x},{y},{z}"
                );
            }
        }
    }
}

#[test]
fn v3_basic_block_for_block() {
    let view = read_structure(&single(&fixtures::v3_basic())).unwrap();
    assert_eq!(view.data_version, DATA_VERSION);
    assert_eq!(view.size, [3, 3, 3]);
    for x in 0..3 {
        for y in 0..3 {
            for z in 0..3 {
                assert_eq!(view.state_at(x, y, z), basic_expected(x, y, z));
            }
        }
    }
}

#[test]
fn v2_and_v3_produce_identical_structures() {
    // Same geometry expressed in either Sponge version must converge on the same
    // structure bytes.
    assert_eq!(single(&fixtures::v2_basic()), single(&fixtures::v3_basic()));
    assert_eq!(
        single(&fixtures::v2_block_entities()),
        single(&fixtures::v3_block_entities())
    );
}

#[test]
fn double_convert_is_byte_identical() {
    // Determinism (ADR-0006): repeated conversion yields identical bytes.
    for f in [
        fixtures::v2_basic(),
        fixtures::v3_basic(),
        fixtures::v2_block_entities(),
    ] {
        assert_eq!(single(&f), single(&f));
    }
    // Oversize (split) path too: every part and the manifest must match.
    let a = convert(&fixtures::v2_oversize(), "castle", 48)
        .unwrap()
        .output;
    let b = convert(&fixtures::v2_oversize(), "castle", 48)
        .unwrap()
        .output;
    match (a, b) {
        (
            ConvertOutput::Split {
                parts: pa,
                manifest_json: ma,
                ..
            },
            ConvertOutput::Split {
                parts: pb,
                manifest_json: mb,
                ..
            },
        ) => {
            assert_eq!(pa, pb);
            assert_eq!(ma, mb);
        }
        _ => panic!("expected split output"),
    }
}

fn assert_chest_and_command_strip(schem_bytes: &[u8]) {
    let result = convert(schem_bytes, "test", 48).unwrap();
    let bytes = match &result.output {
        ConvertOutput::Single(v) => v.clone(),
        _ => panic!("expected single"),
    };
    let view = read_structure(&bytes).unwrap();

    // Command block replaced with air; no block entity carried.
    assert_eq!(view.state_at(2, 0, 0), "minecraft:air");
    assert!(!view.block_entities.contains_key(&[2, 0, 0]));

    // A strip diagnostic was emitted at the command block's local position.
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == DW_STRIP && d.pos == Some([2, 0, 0])),
        "expected a strip diagnostic at 2,0,0"
    );

    // Chest survives with its facing state and carried Items.
    assert_eq!(view.state_at(0, 0, 0), "minecraft:chest[facing=north]");
    let be = view
        .block_entities
        .get(&[0, 0, 0])
        .expect("chest block entity carried")
        .as_compound()
        .expect("block entity is a compound");
    assert_eq!(
        be.get("id").and_then(Nbt::as_str),
        Some("minecraft:chest"),
        "carried id set to lowercase structure form"
    );
    assert!(
        be.get("Items").and_then(Nbt::as_list).is_some(),
        "Items kept"
    );
}

#[test]
fn v2_strips_command_block_keeps_chest() {
    assert_chest_and_command_strip(&fixtures::v2_block_entities());
}

#[test]
fn v3_strips_command_block_keeps_chest() {
    assert_chest_and_command_strip(&fixtures::v3_block_entities());
}

#[test]
fn oversize_splits_and_reassembles() {
    let size = [60, 10, 60];
    let result = convert(&fixtures::v2_oversize(), "castle", 48).unwrap();
    assert_eq!(result.grid, [2, 1, 2]);

    // The split is a diagnostic, not a silent behavior — the author must see it.
    assert!(
        result.diagnostics.iter().any(|d| d.code == DW_SPLIT),
        "expected a DW0701 split diagnostic: {:?}",
        result.diagnostics
    );

    let parts = match &result.output {
        ConvertOutput::Split { parts, .. } => parts,
        _ => panic!("expected split"),
    };
    assert_eq!(parts.len(), 4);

    // Reassemble every part into a full grid at its planned offset.
    let plan = plan_split(size, 48);
    let mut full: BTreeMap<[i32; 3], String> = BTreeMap::new();
    for part in &plan.parts {
        let name = part_filename("castle", part.grid_index);
        let (_, bytes) = parts
            .iter()
            .find(|(n, _)| *n == name)
            .expect("part present");
        let view = read_structure(bytes).unwrap();
        assert_eq!(view.size, part.size);
        for lx in 0..part.size[0] {
            for ly in 0..part.size[1] {
                for lz in 0..part.size[2] {
                    let world = [
                        part.offset[0] + lx,
                        part.offset[1] + ly,
                        part.offset[2] + lz,
                    ];
                    full.insert(world, view.state_at(lx, ly, lz).to_string());
                }
            }
        }
    }

    // Lossless: every source cell matches the reassembled grid.
    assert_eq!(full.len(), (size[0] * size[1] * size[2]) as usize);
    for x in 0..size[0] {
        for y in 0..size[1] {
            for z in 0..size[2] {
                let expected = match fixtures::oversize_block_at(x, y, z) {
                    0 => "minecraft:air",
                    _ => "minecraft:stone",
                };
                assert_eq!(full[&[x, y, z]], expected, "cell {x},{y},{z}");
            }
        }
    }
}

#[test]
fn wrong_data_version_is_dw0702_warning() {
    let result = convert(&fixtures::v2_wrong_data_version(), "test", 48).unwrap();
    assert!(
        result.diagnostics.iter().any(|d| d.code == DW_DATAVERSION
            && d.severity == delvewright_schem::diag::Severity::Warning),
        "expected a DW0702 warning for a mismatched source DataVersion: {:?}",
        result.diagnostics
    );
    // Block states are still reinterpreted (best-effort), not rejected.
    let view = read_structure(&single(&fixtures::v2_wrong_data_version())).unwrap();
    assert_eq!(
        view.data_version, DATA_VERSION,
        "output is re-stamped to the pinned target"
    );
}

#[test]
fn palette_report_lists_all_states() {
    let result = convert(&fixtures::v2_block_entities(), "test", 48).unwrap();
    assert_eq!(
        result.palette,
        vec![
            "minecraft:air".to_string(),
            "minecraft:chest[facing=north]".to_string(),
            "minecraft:command_block[conditional=false]".to_string(),
        ]
    );
}
