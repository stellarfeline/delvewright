//! **spec-0040 §3c link 4 — the part's own row names the same box.**
//!
//! The other three links of the allocation cascade were already built and add no
//! surface: the whole's brief facts are `cmp` guards over its own region, a
//! `split` partitions so allocations cannot sum past the region, and a part
//! cannot write outside its box. What was missing is the link that makes the
//! cascade an *ordering*: a part developed on its own, at extents nobody
//! allocated, could be composed into a map and nothing anywhere compared the two
//! numbers. spec-0040 §1.9 is the measured cost — a site plan whose region was
//! the arithmetic sum of the parts' pre-existing depths, so extent flowed UP and
//! the whole inherited a total that was a fact before anything composed it.
//!
//! What is asserted here, in the order the acceptance criterion (§7.12) states
//! it: agreeing extents are green; a row grown one block on one axis is an audit
//! red naming the prefix and both triples; a reoriented include agrees under its
//! declared frame; and the compared-row count is stated, non-zero where there is
//! something to compare and **by name** where there is not.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const GRAMMAR: &str = env!("CARGO_BIN_EXE_delve-grammar");

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dw-grammar-alloc-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn combined(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

fn audit(root: &Path) -> Output {
    Command::new(GRAMMAR)
        .args(["audit", "--campaign-root"])
        .arg(root)
        .output()
        .unwrap()
}

/// The part: one program, region-polymorphic, green on every always-on gate.
const PART: &str = r#"{
  "version": "1.0.0",
  "name": "part",
  "start": "all",
  "rules": { "all": [ { "weight": 1, "body": {
    "op": "split", "axis": "y",
    "sizes": [ {"size":"absolute","blocks":{"expr":"int","value":1}},
               {"size":"relative","weight":{"expr":"int","value":1}} ],
    "children": [ {"op":"fill","material":"minecraft:stone"}, {"op":"void"} ] } } ] }
}"#;

/// The map: its own top-level `split` IS the site plan, and the first piece is
/// the box it allocates the composed part. Nothing about the part decides it.
const MAP: &str = r#"{
  "version": "1.5.0",
  "name": "map",
  "start": "plan",
  "include": [ { "program": "part.json", "prefix": "p" } ],
  "rules": { "plan": [ { "weight": 1, "body": {
    "op": "split", "axis": "x",
    "sizes": [ {"size":"absolute","blocks":{"expr":"int","value":6}},
               {"size":"relative","weight":{"expr":"int","value":1}} ],
    "children": [ {"op":"call","symbol":"p/all"}, {"op":"void"} ] } } ] }
}"#;

/// The same map, turning the part a quarter-turn about the vertical at the
/// include site. The world box is unchanged; what the part reads is not.
const MAP_TURNED: &str = r#"{
  "version": "1.5.0",
  "name": "map",
  "start": "plan",
  "include": [ { "program": "part.json", "prefix": "p" } ],
  "rules": { "plan": [ { "weight": 1, "body": {
    "op": "split", "axis": "x",
    "sizes": [ {"size":"absolute","blocks":{"expr":"int","value":6}},
               {"size":"relative","weight":{"expr":"int","value":1}} ],
    "children": [ { "op": "reorient",
                    "orient": {"x": "local_z", "z": "local_x"},
                    "body": {"op":"call","symbol":"p/all"} },
                  {"op":"void"} ] } } ] }
}"#;

/// A map that composes the part and never calls it: the allocation the row is
/// compared against does not exist.
const MAP_UNPLACED: &str = r#"{
  "version": "1.5.0",
  "name": "map",
  "start": "plan",
  "include": [ { "program": "part.json", "prefix": "p" } ],
  "rules": { "plan": [ { "weight": 1, "body": {
    "op": "split", "axis": "y",
    "sizes": [ {"size":"absolute","blocks":{"expr":"int","value":1}},
               {"size":"relative","weight":{"expr":"int","value":1}} ],
    "children": [ {"op":"fill","material":"minecraft:stone"}, {"op":"void"} ] } } ] }
}"#;

/// A content root: the map program, the part program, and a manifest whose part
/// row declares `part_region`.
fn root(dir: &Path, map: &str, part_region: &str) -> PathBuf {
    let programs = dir.join("campaigns/demo/design/programs");
    fs::create_dir_all(&programs).unwrap();
    fs::write(programs.join("part.json"), PART).unwrap();
    fs::write(programs.join("map.json"), map).unwrap();
    fs::write(
        programs.join("zones.json"),
        format!(
            r#"{{ "zones": [
  {{ "id": "map",  "program": "map.json",  "region": [10, 4, 5], "seed": 1 }},
  {{ "id": "part", "program": "part.json", "region": {part_region}, "seed": 2 }}
] }}"#
        ),
    )
    .unwrap();
    dir.to_path_buf()
}

// ---------------------------------------------------------------------------
// The identity itself
// ---------------------------------------------------------------------------

/// **Green when the row names the box.** The map's own `split` allocates the
/// part `6x4x5`; the part's row declares `6x4x5`; the audit passes, and it says
/// how many rows it compared rather than leaving the reader to assume.
#[test]
fn a_row_that_names_the_allocated_box_is_green_and_the_count_is_stated() {
    let dir = scratch("agree");
    let out = audit(&root(&dir, MAP, "[6, 4, 5]"));
    let text = combined(&out);
    assert!(out.status.success(), "{text}");
    // Non-zero, or the whole identity is vacuous over this corpus (§7.12).
    assert!(
        text.contains("part-allocation  bound 1"),
        "the compared-row count is not stated as 1: {text}"
    );
    assert!(!text.contains("DW0806"), "{text}");
}

/// **Red when the row is grown one block on one axis** — the acceptance
/// criterion's perturbation, and the direction the measured failure went in.
/// The refusal names the prefix and both extent triples.
#[test]
fn a_row_one_block_wider_than_its_allocation_is_an_audit_red() {
    let dir = scratch("grown");
    let out = audit(&root(&dir, MAP, "[7, 4, 5]"));
    let text = combined(&out);
    assert!(!out.status.success(), "{text}");
    assert!(text.contains("DW0806"), "{text}");
    assert!(text.contains("part-allocation"), "{text}");
    // The prefix, so a reader knows WHICH composed part is in debt…
    assert!(text.contains("\"p\""), "{text}");
    assert!(text.contains("\"part\""), "{text}");
    // …and both triples, so neither number has to be looked up.
    assert!(
        text.contains("6x4x5"),
        "the allocated box is not named: {text}"
    );
    assert!(
        text.contains("7x4x5"),
        "the row's extents are not named: {text}"
    );
}

/// **A row grown one block is red on EVERY axis, not just the first.** A
/// comparison that read one axis, or that compared volumes, would pass two of
/// these three.
#[test]
fn the_identity_reads_all_three_axes() {
    for (i, region) in [("x", "[7, 4, 5]"), ("y", "[6, 5, 5]"), ("z", "[6, 4, 6]")] {
        let dir = scratch(&format!("axis-{i}"));
        let out = audit(&root(&dir, MAP, region));
        assert!(
            !out.status.success() && combined(&out).contains("DW0806"),
            "axis {i}: {}",
            combined(&out)
        );
    }
}

/// **A reoriented include agrees under its declared frame.**
///
/// The world box is `6x4x5` either way — the map's plan did not move. What moved
/// is what the part reads, and the part reads `5x4x6`, which is what its row
/// must declare. Both halves are asserted: the turned frame's own triple is
/// green, and the UNturned triple — the one a comparison of world extents would
/// have accepted — is red. Without the second half this test would pass on a
/// comparison that ignored the reorientation entirely.
#[test]
fn a_reoriented_include_compares_rotated_extents() {
    let dir = scratch("turned-green");
    let out = audit(&root(&dir, MAP_TURNED, "[5, 4, 6]"));
    assert!(out.status.success(), "{}", combined(&out));

    let dir = scratch("turned-red");
    let out = audit(&root(&dir, MAP_TURNED, "[6, 4, 5]"));
    let text = combined(&out);
    assert!(!out.status.success(), "{text}");
    assert!(text.contains("DW0806"), "{text}");
    // The world box is still 6x4x5, and the message says so beside the local
    // triple, so a reader can see the frame rather than infer it.
    assert!(text.contains("5x4x6"), "{text}");
}

/// **A composed part the map never places has no box, and that is a red.**
///
/// This is the vacuous green the identity exists to end: with no allocation
/// recorded, a comparison that skipped the row would pass a map that does not
/// contain the part at all.
#[test]
fn a_composed_part_that_is_never_placed_is_a_red_not_a_skip() {
    let dir = scratch("unplaced");
    let out = audit(&root(&dir, MAP_UNPLACED, "[6, 4, 5]"));
    let text = combined(&out);
    assert!(!out.status.success(), "{text}");
    assert!(text.contains("DW0806"), "{text}");
    assert!(text.contains("NO box"), "{text}");
}

// ---------------------------------------------------------------------------
// The binding count
// ---------------------------------------------------------------------------

/// **A corpus with nothing to compare says so by name.**
///
/// The part keeps its include and loses its row, which is the composed-only case
/// §3c calls structural: the allocated box is the only box that part is ever
/// judged at. The audit is green — and the line is printed anyway, because a
/// count that appears only when it is interesting is a count nobody learns to
/// read, and a zero binding that is simply omitted is this project's first
/// vacuity mode.
#[test]
fn a_corpus_with_no_compared_row_states_the_zero_by_name() {
    let dir = scratch("zero");
    let programs = dir.join("campaigns/demo/design/programs");
    fs::create_dir_all(&programs).unwrap();
    fs::write(programs.join("part.json"), PART).unwrap();
    fs::write(programs.join("map.json"), MAP).unwrap();
    fs::write(
        programs.join("zones.json"),
        r#"{ "zones": [
  { "id": "map", "program": "map.json", "region": [10, 4, 5], "seed": 1 }
] }"#,
    )
    .unwrap();
    let out = audit(&dir);
    let text = combined(&out);
    assert!(out.status.success(), "{text}");
    assert!(text.contains("part-allocation  bound 0"), "{text}");
    assert!(
        text.contains("nothing in this corpus composes a part that also has its own row"),
        "the zero is not stated by name: {text}"
    );
    // And the include surface still bound, so the zero is about the ROW and not
    // about a corpus that composes nothing.
    assert!(
        text.contains("include") && text.contains("bound 1"),
        "{text}"
    );
}

/// **The composed prefix is the one that is compared, not the include's local
/// name.** A nested include's vocabulary carries `outer/inner`, and two composed
/// documents may each write the prefix `p`. Keying the comparison on the local
/// prefix would compare one part's row against another part's box.
#[test]
fn a_nested_composition_is_compared_at_its_full_prefix_path() {
    let dir = scratch("nested");
    let programs = dir.join("campaigns/demo/design/programs");
    fs::create_dir_all(&programs).unwrap();
    fs::write(programs.join("part.json"), PART).unwrap();
    // zone.json composes the part under `p` into its own left half…
    fs::write(
        programs.join("zone.json"),
        r#"{
  "version": "1.5.0",
  "name": "zone",
  "start": "zone",
  "include": [ { "program": "part.json", "prefix": "p" } ],
  "rules": { "zone": [ { "weight": 1, "body": {
    "op": "split", "axis": "z",
    "sizes": [ {"size":"absolute","blocks":{"expr":"int","value":3}},
               {"size":"relative","weight":{"expr":"int","value":1}} ],
    "children": [ {"op":"call","symbol":"p/all"}, {"op":"void"} ] } } ] }
}"#,
    )
    .unwrap();
    // …and map.json composes the zone under `z`, so the part's vocabulary is
    // `z/p` and its box is 6 x 4 x 3.
    fs::write(
        programs.join("map.json"),
        r#"{
  "version": "1.5.0",
  "name": "map",
  "start": "plan",
  "include": [ { "program": "zone.json", "prefix": "z" } ],
  "rules": { "plan": [ { "weight": 1, "body": {
    "op": "split", "axis": "x",
    "sizes": [ {"size":"absolute","blocks":{"expr":"int","value":6}},
               {"size":"relative","weight":{"expr":"int","value":1}} ],
    "children": [ {"op":"call","symbol":"z/zone"}, {"op":"void"} ] } } ] }
}"#,
    )
    .unwrap();
    let manifest = |part: &str| {
        format!(
            r#"{{ "zones": [
  {{ "id": "map",  "program": "map.json",  "region": [10, 4, 5], "seed": 1 }},
  {{ "id": "zone", "program": "zone.json", "region": [6, 4, 5],  "seed": 2 }},
  {{ "id": "part", "program": "part.json", "region": {part},     "seed": 3 }}
] }}"#
        )
    };
    fs::write(programs.join("zones.json"), manifest("[6, 4, 3]")).unwrap();
    let out = audit(&dir);
    let text = combined(&out);
    assert!(out.status.success(), "{text}");
    // Three comparisons, not two, and the third is the point: the zone's own
    // row against the map's box, the part's row against the box the map's
    // composition of the zone hands `z/p`, and the part's row again against the
    // box the ZONE's own audit hands `p`. A part composed into two documents
    // confronts an allocation in each; nothing here picks one of them.
    assert!(text.contains("part-allocation  bound 3"), "{text}");

    // The part's row moved to the zone's own extents — the number that would be
    // right if the nesting were flattened by one level. It is red.
    fs::write(programs.join("zones.json"), manifest("[6, 4, 5]")).unwrap();
    let out = audit(&dir);
    let text = combined(&out);
    assert!(!out.status.success(), "{text}");
    assert!(
        text.contains("DW0806") && text.contains("\"z/p\""),
        "{text}"
    );
}
