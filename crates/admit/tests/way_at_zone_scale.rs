//! **A contingent edge, on a building that does not fit in one file.**
//!
//! Two changes met here and neither could reach this on its own.
//!
//! * A `way` — a traversal edge severed as built and opened by content — got
//!   its exported form, its `way:<name>` anchor resolution and its verdicts
//!   through the second door. All of that was demonstrated on a **single
//!   structure template**, because that is the only packaging the door accepted.
//! * The second door learned to open on a **tile-set manifest**, which is what a
//!   composed zone IS. That was demonstrated on the corpus contract, whose only
//!   contingent edge is a `barred` one — so it says nothing about a way.
//!
//! The pair is a way-carrying contract addressed by a manifest, and it is not
//! hypothetical plumbing: the contract a manifest declares is **zone-relative**,
//! and a way's cells are the one part of a contract the bytes deliberately need
//! not hold. So the questions this file asks are the ones neither side could:
//!
//! 1. **Does a zone-relative way region land where the door thinks it does?**
//!    The fixture is sized so the way **straddles a tiling seam** — its cells
//!    live in two different `.nbt` files — and the test asserts that straddle
//!    from the manifest before it believes any verdict. A way that fell inside
//!    one tile would be this file testing the case it is not about.
//! 2. **Does each sign behave at zone scale?** `cleared` is proved by blocks the
//!    reassembled grid holds across a seam; `laid` is proved by cells no tile
//!    holds at all, which is the harder half — "the contract describes a
//!    building it is not true of" is invisible where the evidence is absence.
//! 3. **Does the dropped-contract corroborator survive `way:`?** The door
//!    refuses a document that declares no contract while its anchors still carry
//!    a `resolves_to`. That test is `resolves_to.is_some()` and the key is a free
//!    string, so a new `way:` prefix cannot narrow it — asserted here on a
//!    manifest whose anchor resolves to `way:gate`, rather than assumed.

use std::path::{Path, PathBuf};
use std::process::Command;

use delvewright_admit::structure::{PaletteEntry, Structure};
use delvewright_grammar::export::export_zone;
use delvewright_grammar::ir::{
    EdgeClass, Mark, MarkAt, Node, Opens, Program, Reorient, Rounding, Size, Split, Way,
};
use delvewright_grammar::library::spatial_contract::spatial_contract;
use delvewright_grammar::{Axis, Box3, ExpandOptions};

const ADMIT: &str = env!("CARGO_BIN_EXE_delve-admit");

/// A zone past the 48-per-axis cap on X, chosen so the doorway **straddles a
/// tiling seam**.
///
/// The corpus partition is `rel(1) · abs(3) · rel(1)` on X, so at 97 the doorway
/// occupies x 47–49 while [`delvewright_dsl::split::plan_split`] cuts at every
/// multiple of 48. The way region therefore spans tile `x0` and tile `x1`, which
/// is the whole point of the fixture — and the tests assert it rather than
/// trusting this comment.
const ZONE: Box3 = Box3::at_origin([97, 6, 15]);

/// The partition's Z course, which is where every cell this file edits lives.
const PARTITION_Z: i32 = 7;

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dw-admit-way-zone-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

// ---------------------------------------------------------------------------
// The two fixtures — the corpus contract with its barred door respelled
// ---------------------------------------------------------------------------

/// The corpus piece with its barred door respelled as the `walk` + `cleared`
/// way it means, plus a mark inside the way region.
///
/// The single-template twin of this program lives in `tests/spatial_contract.rs`
/// and is what proves the two spellings normalise into one prover. This one
/// exists to be *tiled*: same rules, same roles, same blocks, a region that does
/// not fit in one file.
fn cleared_way() -> Program {
    let mut program = spatial_contract();
    let contract = program.contract.as_mut().expect("the corpus declares one");
    for edge in &mut contract.edges {
        if let EdgeClass::Barred { rise, bar, .. } = &edge.class {
            edge.class = EdgeClass::Walk {
                rise: *rise,
                // `barred`'s `via` is optional and the corpus leaves it out; a
                // way-carrying edge has no such choice, because the cells a way
                // opens must belong to the edge.
                via: Some(bar.region.clone()),
                way: Some(Way {
                    opens: Opens::Cleared,
                    region: bar.region.clone(),
                    block: bar.block.clone(),
                }),
            };
        }
    }
    // An anchor inside the way region — at the seam cell itself, x = 48, the
    // first cell of the second tile. What it resolves to is the question in §3.
    for alt in program.rules.get_mut("doorway").expect("the doorway rule") {
        let body = std::mem::replace(&mut alt.body, Node::Void);
        alt.body = Node::Mark {
            mark: Mark::new("gate-watch", MarkAt::offset(1, 2, 0)),
            body: Box::new(body),
        };
    }
    program
}

/// The corpus piece with its threshold missing instead of barred: the
/// partition's floor course is claimed as `deck`, left empty, and the door is a
/// `walk` whose way is `laid`.
///
/// The sign whose cells the shipped bytes do **not** hold — at zone scale, where
/// the three cells the metadata names are split across two files that each hold
/// air there and would each, alone, be telling the truth.
fn laid_way() -> Program {
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

/// Freeze a program as a TILED zone: several `.nbt` files and one manifest
/// carrying the zone-relative contract.
fn tiled(tag: &str, program: &Program) -> Zone {
    let dir = scratch(tag);
    export_zone(program, ZONE, &ExpandOptions::seeded(1), "zone")
        .expect("the program exports at this region")
        .write_to_dir(&dir)
        .unwrap();
    let manifest = dir.join("zone.json");
    let set = delvewright_schem::split::read_tile_set(&manifest)
        .expect("the manifest reads")
        .expect("the export tiled");
    assert!(
        set.parts.len() > 1,
        "the fixture must actually tile, or this file is about the case it is not testing"
    );
    Zone { dir, manifest, set }
}

struct Zone {
    dir: PathBuf,
    manifest: PathBuf,
    set: delvewright_schem::split::TileSet,
}

impl Zone {
    /// The declared contract, straight off the manifest.
    fn document(&self) -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(&self.manifest).unwrap()).unwrap()
    }

    /// The `way` block of the one contingent edge the fixtures declare.
    fn way(&self) -> serde_json::Value {
        let doc = self.document();
        doc["spatial_contract"]["edges"]
            .as_array()
            .expect("the manifest declares edges")
            .iter()
            .find_map(|e| e.get("way").cloned())
            .expect("one edge carries a way")
    }

    /// Every zone cell the way's declared boxes cover.
    fn way_cells(&self) -> Vec<[i32; 3]> {
        let mut out = Vec::new();
        for r in self.way()["boxes"].as_array().expect("boxes") {
            let from: Vec<i32> = r["from"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap() as i32)
                .collect();
            let to: Vec<i32> = r["to"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap() as i32)
                .collect();
            for x in from[0]..=to[0] {
                for y in from[1]..=to[1] {
                    for z in from[2]..=to[2] {
                        out.push([x, y, z]);
                    }
                }
            }
        }
        out
    }

    /// How many distinct tile files the way's cells fall into.
    ///
    /// The binding count of this whole file: at `1` the fixture is a
    /// single-template test wearing a manifest, and every verdict below would be
    /// about the case the two branches had each already covered.
    fn tiles_the_way_spans(&self) -> usize {
        let cells = self.way_cells();
        let mut files: Vec<&str> = self
            .set
            .parts
            .iter()
            .filter(|p| {
                cells
                    .iter()
                    .any(|c| (0..3).all(|a| c[a] >= p.offset[a] && c[a] < p.offset[a] + p.size[a]))
            })
            .map(|p| p.file.as_str())
            .collect();
        files.sort_unstable();
        files.dedup();
        files.len()
    }

    /// Write one zone cell into whichever tile holds it.
    fn set_cell(&self, pos: [i32; 3], block: &str) {
        let part = self
            .set
            .parts
            .iter()
            .find(|p| (0..3).all(|a| pos[a] >= p.offset[a] && pos[a] < p.offset[a] + p.size[a]))
            .unwrap_or_else(|| panic!("no tile holds {pos:?}"));
        let path = self.dir.join(&part.file);
        let mut s = Structure::read(&std::fs::read(&path).unwrap()).unwrap();
        s.set_cell(
            [
                pos[0] - part.offset[0],
                pos[1] - part.offset[1],
                pos[2] - part.offset[2],
            ],
            PaletteEntry::simple(block),
            None,
        );
        s.prune_palette();
        std::fs::write(&path, s.write()).unwrap();
    }
}

struct Audit {
    ok: bool,
    stderr: String,
    report: serde_json::Value,
}

fn audit(path: &Path) -> Audit {
    let out = Command::new(ADMIT)
        .args(["audit", path.to_str().unwrap()])
        .output()
        .expect("delve-admit runs");
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let report: serde_json::Value = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("a machine-readable report on stdout: {e}\nstderr: {stderr}"));
    Audit {
        ok: out.status.success(),
        stderr,
        report,
    }
}

// ---------------------------------------------------------------------------
// §1 and §2 — a cleared way whose cells live in two files
// ---------------------------------------------------------------------------

/// **The green.** A `cleared` way declared in zone coordinates is judged over
/// the reassembled zone, and its cells really do cross a packaging plane.
#[test]
fn a_cleared_way_that_straddles_a_seam_is_judged_over_the_whole_zone() {
    let zone = tiled("cleared-green", &cleared_way());

    // The bytes carry the surface under test, and the fixture is the shape this
    // file claims — both established before any verdict is believed.
    let way = zone.way();
    assert_eq!(way["opens"], "cleared", "{way}");
    assert_eq!(zone.way_cells().len(), 9, "{way}");
    assert_eq!(
        zone.tiles_the_way_spans(),
        2,
        "the way must span a tiling seam, or this test is about a single template: {way}"
    );

    let a = audit(&zone.manifest);
    assert!(a.ok, "{}", a.stderr);
    let c = &a.report["contract"];
    assert_eq!(c["state"], "judged", "{c}");
    assert_eq!(
        c["files"].as_u64().unwrap() as usize,
        zone.set.parts.len(),
        "the verdict names every file it covers: {c}"
    );
    assert_eq!(
        c["cells"].as_u64().unwrap(),
        (ZONE.size[0] as u64) * (ZONE.size[1] as u64) * (ZONE.size[2] as u64),
        "over the assembled zone, not a tile: {c}"
    );
    assert_eq!(c["failed_gates"], 0, "{c}");
    assert!(c["objects"].as_u64().unwrap() > 0, "{c}");
    assert!(
        a.stderr.contains("way \"gate\": cleared over 9 cell(s)"),
        "the way is enumerated with its sign and binding count: {}",
        a.stderr
    );
}

/// **The red.** Take the iron out of the doorway — three cells in one file and
/// six in the next — and the manifest describes a building it is no longer true
/// of. The failure has to be seen across the seam or not at all: each tile on
/// its own now holds air where it always could have.
#[test]
fn a_cleared_way_stops_holding_across_the_seam_and_is_refused() {
    let zone = tiled("cleared-red", &cleared_way());
    assert_eq!(zone.tiles_the_way_spans(), 2);
    for cell in zone.way_cells() {
        zone.set_cell(cell, "minecraft:air");
    }

    let a = audit(&zone.manifest);
    assert!(!a.ok, "an unbarred zone must not be admitted: {}", a.stderr);
    assert!(a.stderr.contains("DW0782"), "{}", a.stderr);
    assert!(
        a.stderr.contains("the way \"gate\" does not open anything"),
        "the refusal names the way in the author's own spelling: {}",
        a.stderr
    );
    assert_eq!(a.report["contract"]["state"], "judged");
    assert!(
        a.report["contract"]["failed_gates"].as_u64().unwrap() >= 1,
        "{}",
        a.report["contract"]
    );
    assert_eq!(a.report["verdict"], "fail", "{}", a.report);
}

/// **The other sign, over cells no tile holds.** A `laid` way names three cells
/// the export deliberately leaves empty, split across two files — the case where
/// the evidence for the contract is an absence, and where a per-tile reading
/// would find every tile innocent.
///
/// Then the same three cells are laid in the bytes and nothing else changes: the
/// way now opens nothing, and the door refuses naming it.
#[test]
fn a_laid_way_is_judged_across_a_seam_and_refused_once_the_bytes_hold_it() {
    let zone = tiled("laid", &laid_way());

    let way = zone.way();
    assert_eq!(way["opens"], "laid", "{way}");
    let cells = zone.way_cells();
    assert_eq!(cells.len(), 3, "{way}");
    assert_eq!(
        zone.tiles_the_way_spans(),
        2,
        "the laid region must span a tiling seam: {way}"
    );

    let a = audit(&zone.manifest);
    assert!(a.ok, "{}", a.stderr);
    assert_eq!(a.report["contract"]["state"], "judged");
    assert!(
        a.stderr.contains("way \"deck\": laid over 3 cell(s)"),
        "the way is enumerated with its sign and binding count: {}",
        a.stderr
    );
    assert!(
        a.stderr.contains("reached only once deck is laid"),
        "and the seam is named per space, which is what tells `reachable` from \
         `reachable eventually`: {}",
        a.stderr
    );

    // Lay the threshold, in both files.
    for cell in &cells {
        assert_eq!(cell[1], 0, "the deck is the floor course: {cell:?}");
        assert_eq!(cell[2], PARTITION_Z, "in the partition: {cell:?}");
        zone.set_cell(*cell, "minecraft:stone");
    }

    let a = audit(&zone.manifest);
    assert!(
        !a.ok,
        "a way over ground that is already there: {}",
        a.stderr
    );
    assert!(a.stderr.contains("DW0782"), "{}", a.stderr);
    assert!(
        a.stderr.contains("the way \"deck\" does not open anything"),
        "{}",
        a.stderr
    );
    assert_eq!(a.report["verdict"], "fail", "{}", a.report);
}

// ---------------------------------------------------------------------------
// §3 — the corroborator, over an anchor that resolves into a way
// ---------------------------------------------------------------------------

/// **A manifest cannot drop its contract either, and `way:` does not weaken the
/// thing that catches it.**
///
/// The door's hatch — "this document declares no contract" — is corroborated
/// against `resolves_to`, which only an exporter writes and only out of a
/// contract. The exporter now writes a fifth prefix, `way:<name>`, for an anchor
/// inside a way region; the corroborator tests only that the key is *present*,
/// so the prefix cannot narrow it. Asserted here rather than reasoned about: the
/// anchor resolves to `way:gate`, the contract is deleted from the manifest, and
/// the composed zone is refused with its binding count.
#[test]
fn a_manifest_whose_anchor_resolves_into_a_way_cannot_drop_its_contract() {
    let zone = tiled("dropped", &cleared_way());

    let doc = zone.document();
    let resolved: Vec<&str> = doc["anchors"]
        .as_object()
        .expect("the manifest carries anchors")
        .values()
        .filter_map(|a| a.get("resolves_to").and_then(|v| v.as_str()))
        .collect();
    assert!(
        resolved.contains(&"way:gate"),
        "the fixture's anchor must resolve into the WAY, not the transit volume \
         containing it — otherwise this test says nothing about the new prefix: {resolved:?}"
    );

    let mut doc = doc;
    doc.as_object_mut().unwrap().remove("spatial_contract");
    std::fs::write(
        &zone.manifest,
        serde_json::to_string_pretty(&doc).unwrap() + "\n",
    )
    .unwrap();

    let a = audit(&zone.manifest);
    assert!(
        !a.ok,
        "a dropped contract is not an absent one, at zone scale either: {}",
        a.stderr
    );
    assert!(a.stderr.contains("DW0783"), "{}", a.stderr);
    assert!(
        a.stderr.contains("resolves_to"),
        "the refusal names the corroboration that contradicted the absence: {}",
        a.stderr
    );
    let c = &a.report["contract"];
    assert_eq!(c["state"], "refused", "{c}");
    assert!(
        c["resolved_anchors"].as_u64().unwrap() > 0,
        "the drop detector states its own binding count: {c}"
    );
    assert_eq!(
        c["files"].as_u64().unwrap() as usize,
        zone.set.parts.len(),
        "and the refusal still says how much it did not judge: {c}"
    );
}
