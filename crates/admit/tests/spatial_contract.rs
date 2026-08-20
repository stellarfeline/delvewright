//! **One checker, two doors** (spec-0036 §1c, AC6).
//!
//! `delve-grammar expand` judges a contract against the blocks it just wrote.
//! `delve-admit audit` judges the same contract against the same blocks read
//! back off disk. Same bytes plus same resolved contract must give the same
//! verdict whichever door it came through — which is true here because there is
//! one implementation of the obligations and this crate calls it.

use std::path::{Path, PathBuf};
use std::process::Command;

use delvewright_admit::structure::{PaletteEntry, Structure};
use delvewright_grammar::ir::{
    EdgeClass, Mark, MarkAt, Node, Opens, Program, Reorient, Rounding, Size, Split, Way,
};
use delvewright_grammar::library::spatial_contract::spatial_contract;
use delvewright_grammar::{Axis, Box3, ExpandOptions, export_prefab};

const ADMIT: &str = env!("CARGO_BIN_EXE_delve-admit");

/// The region the corpus program is documented at.
const PIECE: Box3 = Box3::at_origin([11, 6, 15]);

fn library(tag: &str) -> PathBuf {
    library_of(tag, &spatial_contract())
}

fn library_of(tag: &str, program: &Program) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dw-admit-contract-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    export_prefab(program, PIECE, &ExpandOptions::seeded(1), "twin-room")
        .expect("the program exports")
        .write_to_dir(&dir)
        .unwrap();
    dir
}

fn audit(dir: &Path) -> (bool, String) {
    let out = Command::new(ADMIT)
        .args(["audit", dir.join("twin-room.nbt").to_str().unwrap()])
        .output()
        .expect("delve-admit runs");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

/// **The green.** The piece `delve-grammar expand` passed, admitted through the
/// other door, still passes — and says what each opt-out was, by name.
#[test]
fn the_second_door_agrees_with_the_first_on_a_piece_that_holds() {
    let dir = library("green");
    let (ok, stderr) = audit(&dir);
    assert!(ok, "{stderr}");
    assert!(
        stderr.contains("no_body \"shelf\": posted"),
        "the enumeration a reviewer reads is printed by this door too: {stderr}"
    );
    assert!(
        stderr.contains("exterior face"),
        "the face contract is enumerated: {stderr}"
    );
}

/// **The red.** Take the iron out of the barred door and nothing else changes:
/// the same metadata now describes a building it is not true of, and the door
/// the operator actually runs refuses it with the code named.
#[test]
fn a_piece_whose_blocks_no_longer_match_its_contract_is_refused_as_dw0782() {
    let dir = library("red");
    let nbt = dir.join("twin-room.nbt");
    let mut s = Structure::read(&std::fs::read(&nbt).unwrap()).unwrap();
    for y in 1..=3 {
        for x in 4..=6 {
            s.set_cell([x, y, 7], PaletteEntry::simple("minecraft:air"), None);
        }
    }
    s.prune_palette();
    std::fs::write(&nbt, s.write()).unwrap();

    let (ok, stderr) = audit(&dir);
    assert!(!ok, "an unbarred bar must not be admitted: {stderr}");
    assert!(stderr.contains("DW0782"), "{stderr}");
    assert!(
        stderr.contains("contract-edge-proof FAILED"),
        "the gate that disagreed is named: {stderr}"
    );
    assert!(
        stderr.contains("does not bar anything"),
        "and what it found: {stderr}"
    );
    assert!(
        stderr.contains("examined 1 object(s)"),
        "with its binding count: {stderr}"
    );
}

// ---------------------------------------------------------------------------
// A way, through the same two doors (spec-0042 §2.3, §6.2)
// ---------------------------------------------------------------------------

/// The corpus piece with its bar region also declared as the edge's transit
/// volume.
///
/// `barred`'s `via` is optional and the corpus leaves it out, so the doorway
/// cells belong to the bar and to nothing else. A way-carrying edge has no such
/// choice — the cells a way lays or clears must belong to the edge, so it
/// declares one — and comparing the two spellings without this would compare
/// two different declarations about where the doorway is and read the
/// difference as a disagreement between provers.
fn spatial_contract_with_the_gate_as_its_transit_volume() -> Program {
    let mut program = spatial_contract();
    let contract = program.contract.as_mut().expect("the corpus declares one");
    for edge in &mut contract.edges {
        if let EdgeClass::Barred { bar, via, .. } = &mut edge.class {
            *via = Some(bar.region.clone());
        }
    }
    program
}

/// The corpus piece with its barred door respelled as the walk-plus-cleared-way
/// it means (spec-0042 §2.2), plus a mark inside the way region.
///
/// Same rules, same roles, same blocks — the twin of the fixture above with one
/// edge written the other way round, which is what makes the pair of audits
/// below a statement about the SPELLING and not about two buildings.
fn spatial_contract_as_a_cleared_way() -> Program {
    let mut program = spatial_contract_with_the_gate_as_its_transit_volume();
    let contract = program.contract.as_mut().expect("the corpus declares one");
    for edge in &mut contract.edges {
        if let EdgeClass::Barred { rise, bar, .. } = &edge.class {
            edge.class = EdgeClass::Walk {
                rise: *rise,
                via: Some(bar.region.clone()),
                way: Some(Way {
                    opens: Opens::Cleared,
                    region: bar.region.clone(),
                    block: bar.block.clone(),
                }),
            };
        }
    }
    for alt in program.rules.get_mut("doorway").expect("the doorway rule") {
        let body = std::mem::replace(&mut alt.body, Node::Void);
        alt.body = Node::Mark {
            mark: Mark::new("gate-watch", MarkAt::offset(1, 2, 0)),
            body: Box::new(body),
        };
    }
    program
}

/// Every failed gate of an audit, by id — the verdict, without the wording.
fn failed_gates(stderr: &str) -> Vec<String> {
    stderr
        .lines()
        .filter_map(|l| l.split_once(" FAILED (examined "))
        .map(|(head, _)| head.rsplit(' ').next().unwrap_or("").to_string())
        .collect()
}

/// **The green, through the way spelling.** The piece `delve-grammar expand`
/// passed with a `way` on its edge is admitted by the door the operator runs,
/// and that door enumerates the way — by name, sign and cell count — rather
/// than folding an opt-out into a number.
///
/// The way is what makes this non-vacuous: the barred sibling above proves the
/// second door reads a contract, and proves nothing at all about the surface
/// spec-0042 adds, because the metadata it audits has no `way` key in it.
#[test]
fn the_second_door_reads_a_way_and_enumerates_it() {
    let dir = library_of("way-green", &spatial_contract_as_a_cleared_way());
    // The bytes really do carry the surface under test, before the verdict is
    // believed: an audit of a metadata document with no `way` in it would pass
    // for reasons that have nothing to do with this test.
    let json = std::fs::read_to_string(dir.join("twin-room.json")).unwrap();
    assert!(json.contains("\"way\""), "{json}");
    assert!(json.contains("\"opens\": \"cleared\""), "{json}");
    assert!(json.contains("\"way:gate\""), "{json}");

    let (ok, stderr) = audit(&dir);
    assert!(ok, "{stderr}");
    assert!(
        stderr.contains("way \"gate\": cleared over 9 cell(s)"),
        "the way is enumerated with its binding count: {stderr}"
    );
    assert!(
        stderr.contains("closed on the bytes as shipped"),
        "and with what the two block-level proof parts bound to: {stderr}"
    );
}

/// **One prover, through the second door** (spec-0042 AC6).
///
/// The identical building, audited twice from disk: once declared `barred`,
/// once declared `walk` + `way { cleared }`. Both spellings normalise into one
/// contingency before anything is proved, so the two audits agree on the
/// verdict — green on the shipped bytes, and red on the same gate when the iron
/// is taken out — and they agree because there is one prover, not because two
/// happen to be written the same way.
///
/// The red half is what gives the pair teeth: a comparison of two greens cannot
/// tell a shared prover from two provers that both say yes to everything.
#[test]
fn both_spellings_of_one_door_give_the_second_door_the_same_verdict() {
    let barred = library_of(
        "pair-barred",
        &spatial_contract_with_the_gate_as_its_transit_volume(),
    );
    let way = library_of("pair-way", &spatial_contract_as_a_cleared_way());

    let (bar_ok, bar_err) = audit(&barred);
    let (way_ok, way_err) = audit(&way);
    assert!(bar_ok && way_ok, "{bar_err}\n---\n{way_err}");
    assert_eq!(failed_gates(&bar_err), failed_gates(&way_err));
    assert!(failed_gates(&bar_err).is_empty());

    // The same edit to both, on the bytes on disk: take the iron out of the
    // doorway and nothing else changes.
    for dir in [&barred, &way] {
        let nbt = dir.join("twin-room.nbt");
        let mut s = Structure::read(&std::fs::read(&nbt).unwrap()).unwrap();
        for y in 1..=3 {
            for x in 4..=6 {
                s.set_cell([x, y, 7], PaletteEntry::simple("minecraft:air"), None);
            }
        }
        s.prune_palette();
        std::fs::write(&nbt, s.write()).unwrap();
    }

    let (bar_ok, bar_err) = audit(&barred);
    let (way_ok, way_err) = audit(&way);
    assert!(!bar_ok, "{bar_err}");
    assert!(!way_ok, "{way_err}");
    let failed = failed_gates(&bar_err);
    assert_eq!(failed, failed_gates(&way_err), "{bar_err}\n---\n{way_err}");
    assert!(
        failed.contains(&"contract-edge-proof".to_string()),
        "the closed proof is what disagreed, on both spellings: {failed:?}"
    );
    assert!(
        way_err.contains("DW0782"),
        "and the door refuses with the code named: {way_err}"
    );

    // The prover is one; the WORDING is the author's own. A `barred` edge is
    // told about its bar and a way-carrying edge about its way, from the same
    // proof part running on the same bytes.
    assert!(
        bar_err.contains("the bar does not bar anything"),
        "{bar_err}"
    );
    assert!(
        way_err.contains("the way \"gate\" does not open anything"),
        "{way_err}"
    );
}

/// The same building with its threshold missing instead of barred: the
/// partition's floor course is claimed as `deck` and left empty, and the door is
/// a `walk` whose way is `laid`.
///
/// The other sign, through the door the operator runs. A `laid` way is the one
/// whose cells the shipped bytes do *not* hold — the audit reads air where the
/// metadata names a region — so it is the case where "the contract describes a
/// building it is not true of" is hardest to see, and the one this door exists
/// for.
fn spatial_contract_as_a_laid_deck() -> Program {
    let mut program = spatial_contract();
    let contract = program.contract.as_mut().expect("the corpus declares one");
    for edge in &mut contract.edges {
        if let EdgeClass::Barred { rise, .. } = &edge.class {
            edge.class = EdgeClass::Walk {
                rise: *rise,
                via: Some("gate".to_string()),
                way: Some(Way {
                    opens: Opens::Laid,
                    region: "deck".to_string(),
                    block: "floor".to_string(),
                }),
            };
        }
    }
    program.rule(
        "doorway",
        Node::Claim {
            region: "gate".to_string(),
            body: Box::new(Node::Split(Split {
                axis: Axis::Y,
                sizes: vec![Size::abs(1), Size::abs(3), Size::rel(1)],
                rounding: Rounding::Truncate,
                repeat: false,
                orient: Reorient::KEEP,
                children: vec![
                    Node::Claim {
                        region: "deck".to_string(),
                        body: Box::new(Node::Void),
                    },
                    Node::Void,
                    Node::fill("shell"),
                ],
            })),
        },
    )
}

/// **A laid way, through the second door — green, and red one block later.**
///
/// The audit reads a region the bytes do not hold and has to judge it anyway:
/// severed as shipped, crossable on the single-delta copy. Laying the threshold
/// in the `.nbt` and changing nothing else turns the same metadata into a
/// description of a building it is not true of, and the door refuses it naming
/// the way — which is the closed half of the proof running on bytes read off
/// disk rather than on the model an expansion had in memory.
#[test]
fn the_second_door_judges_a_laid_way_on_bytes_that_do_not_hold_it_yet() {
    let dir = library_of("way-laid", &spatial_contract_as_a_laid_deck());
    let (ok, stderr) = audit(&dir);
    assert!(ok, "{stderr}");
    assert!(
        stderr.contains("way \"deck\": laid over 3 cell(s)"),
        "the way is enumerated with its sign and binding count: {stderr}"
    );
    assert!(
        stderr.contains("reached only once deck is laid"),
        "and the seam is named per space, which is what tells `reachable` from \
         `reachable eventually`: {stderr}"
    );

    // Lay the threshold in the bytes. The way now opens nothing.
    let nbt = dir.join("twin-room.nbt");
    let mut s = Structure::read(&std::fs::read(&nbt).unwrap()).unwrap();
    for x in 4..=6 {
        s.set_cell([x, 0, 7], PaletteEntry::simple("minecraft:stone"), None);
    }
    std::fs::write(&nbt, s.write()).unwrap();

    let (ok, stderr) = audit(&dir);
    assert!(!ok, "a way over ground that is already there: {stderr}");
    assert!(stderr.contains("DW0782"), "{stderr}");
    assert!(
        stderr.contains("the way \"deck\" does not open anything"),
        "{stderr}"
    );
}
