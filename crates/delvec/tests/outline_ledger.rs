//! The bot's pick ray reads the engine's outline table, not a copy of it.
//!
//! `delvewright_dsl::blockshape::outline` is the one authority for a block
//! state's outline: the pinned jar's `BlockState.getShape`, the box the
//! client's crosshair pick meets. The harness casts that ray too
//! (`harness/src/crosshair.ts`), and it cannot link Rust, so it reads a table
//! written from this authority: `harness/src/data/outline-shapes-1.21.11.json`.
//! This test writes that table and refuses when the committed file is not what
//! it writes, so the harness can never hold a stale or hand-edited copy.
//!
//! The table asks [`outline`] about **every state of every block** in the
//! pinned registry and keeps, per block, only the properties that move the
//! answer (a property is kept when flipping it alone changes the outline in some
//! state), as a dense table over those properties: the harness reads a live
//! block's properties off the server and indexes it, with no lookup rule of its
//! own to disagree.
//!
//! The second file, `harness/test/fixtures/outline-agreement-1.21.11.json`, is
//! the agreement cases: every state of the lever, every button and every sign,
//! as full state text, with [`outline`]'s answer for it. The harness's
//! `outline.test.ts` asks its own reader the same states and is held to zero
//! differences.
//!
//! Rewrite both with `DELVEWRIGHT_WRITE_OUTLINES=1 cargo test -p delvec --test outline_ledger`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use delvewright_dsl::blocks::BlockRegistry;
use delvewright_dsl::blockshape::{OutlineBox, Sixteenths, outline};
use serde_json::{Value, json};

const LEDGER: &str = "harness/src/data/outline-shapes-1.21.11.json";
const AGREEMENT: &str = "harness/test/fixtures/outline-agreement-1.21.11.json";

fn repo(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

fn blocks_of(s: Sixteenths) -> f64 {
    f64::from(s.num) / (16.0 * f64::from(s.den))
}

/// One outline, in blocks from the cell's minimum corner, as the JSON carries it.
fn shape(boxes: &[OutlineBox]) -> Vec<[f64; 6]> {
    boxes
        .iter()
        .map(|b| {
            [
                blocks_of(b.min[0]),
                blocks_of(b.min[1]),
                blocks_of(b.min[2]),
                blocks_of(b.max[0]),
                blocks_of(b.max[1]),
                blocks_of(b.max[2]),
            ]
        })
        .collect()
}

/// Every state of `id`, as `(values-index-per-property, full state text)`, in
/// mixed-radix order over the properties' sorted names (the last varies fastest).
fn states(id: &str, props: &BTreeMap<String, Vec<String>>) -> Vec<(Vec<usize>, String)> {
    let names: Vec<&String> = props.keys().collect();
    let mut out = vec![(Vec::new(), Vec::<String>::new())];
    for n in &names {
        let mut next = Vec::new();
        for (ix, kv) in &out {
            for (i, v) in props[*n].iter().enumerate() {
                let mut ix = ix.clone();
                ix.push(i);
                let mut kv = kv.clone();
                kv.push(format!("{n}={v}"));
                next.push((ix, kv));
            }
        }
        out = next;
    }
    out.into_iter()
        .map(|(ix, kv)| {
            let text = if kv.is_empty() {
                id.to_string()
            } else {
                format!("{id}[{}]", kv.join(","))
            };
            (ix, text)
        })
        .collect()
}

fn canonical(v: &Value) -> String {
    delvewright_dsl::fmt::format_text(&serde_json::to_string(v).unwrap())
        .expect("the ledger is JSON the formatter reads")
}

fn write_or_check(rel: &str, text: &str) {
    let path = repo(rel);
    if std::env::var("DELVEWRIGHT_WRITE_OUTLINES").is_ok() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
    }
    let committed = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!("{rel} is not readable ({e}); write it with DELVEWRIGHT_WRITE_OUTLINES=1")
    });
    assert!(
        committed == text,
        "{rel} is not what the engine's outline table writes; rewrite it with \
         DELVEWRIGHT_WRITE_OUTLINES=1 cargo test -p delvec --test outline_ledger"
    );
}

struct Ledger {
    doc: Value,
    states: usize,
    blocks: usize,
    shapes: usize,
}

fn ledger() -> Ledger {
    let reg = BlockRegistry::v1_21_11();
    let mut shapes: Vec<Vec<[f64; 6]>> = Vec::new();
    let mut index_of = |s: Vec<[f64; 6]>| -> usize {
        if let Some(i) = shapes.iter().position(|x| *x == s) {
            return i;
        }
        shapes.push(s);
        shapes.len() - 1
    };
    let mut blocks = serde_json::Map::new();
    let mut unknown = Vec::new();
    let mut total = 0;
    for id in reg.ids() {
        let props = reg.properties(id).expect("a registry id has properties");
        let all = states(id, props);
        let mut answer: BTreeMap<Vec<usize>, usize> = BTreeMap::new();
        let mut known = true;
        for (ix, text) in &all {
            match outline(text) {
                Some(b) => {
                    answer.insert(ix.clone(), index_of(shape(b)));
                }
                None => known = false,
            }
        }
        if !known {
            unknown.push(id.to_string());
            continue;
        }
        total += all.len();
        let names: Vec<&String> = props.keys().collect();
        // A property moves the outline when flipping it alone changes the answer.
        let relevant: Vec<usize> = (0..names.len())
            .filter(|&p| {
                answer.iter().any(|(ix, s)| {
                    (0..props[names[p]].len()).any(|v| {
                        let mut o = ix.clone();
                        o[p] = v;
                        answer[&o] != *s
                    })
                })
            })
            .collect();
        // Dense over the relevant properties, the rest at their first value;
        // every state must agree with the row it indexes.
        let mut table = Vec::new();
        let mut row_of: BTreeMap<Vec<usize>, usize> = BTreeMap::new();
        for (ix, s) in &answer {
            if (0..names.len()).all(|p| relevant.contains(&p) || ix[p] == 0) {
                row_of.insert(relevant.iter().map(|&p| ix[p]).collect(), *s);
            }
        }
        for (ix, s) in &answer {
            let key: Vec<usize> = relevant.iter().map(|&p| ix[p]).collect();
            assert_eq!(
                row_of[&key], *s,
                "{id}: an unkept property moves the outline"
            );
        }
        for s in row_of.values() {
            table.push(json!(s));
        }
        let kept: Vec<Value> = relevant
            .iter()
            .map(|&p| json!([names[p], props[names[p]]]))
            .collect();
        blocks.insert(
            id.to_string(),
            json!({ "properties": kept, "shapes": table }),
        );
    }
    assert!(
        unknown.is_empty(),
        "the outline table holds no row for {} registry block(s): {unknown:?}",
        unknown.len()
    );
    let n_blocks = blocks.len();
    let n_shapes = shapes.len();
    let doc = json!({
        "blocks": blocks,
        "shapes": shapes,
        "source": "delvewright_dsl::blockshape::outline over crates/dsl/data/outline-shapes-1.21.11.tsv, asked every state of every block of the pinned registry; written by crates/delvec/tests/outline_ledger.rs",
        "states": total,
        "version": "1.21.11",
    });
    Ledger {
        doc,
        states: total,
        blocks: n_blocks,
        shapes: n_shapes,
    }
}

/// The blocks the agreement cases cover: the lever, every button, every sign.
fn agreement_block(id: &str) -> bool {
    id == "minecraft:lever" || id.ends_with("_button") || id.ends_with("_sign")
}

#[test]
fn the_harness_reads_the_engines_outline_table() {
    let l = ledger();
    let states = BlockRegistry::v1_21_11()
        .ids()
        .map(|id| states(id, BlockRegistry::v1_21_11().properties(id).unwrap()).len())
        .sum::<usize>();
    assert_eq!(
        l.states, states,
        "every state of every block is in the table"
    );
    assert!(
        l.blocks > 1000 && l.shapes > 1,
        "{} blocks, {} shapes",
        l.blocks,
        l.shapes
    );
    eprintln!(
        "outline ledger: {} blocks, {} states, {} distinct outlines",
        l.blocks, l.states, l.shapes
    );
    write_or_check(LEDGER, &canonical(&l.doc));
}

#[test]
fn the_agreement_cases_are_the_engines_answers() {
    let reg = BlockRegistry::v1_21_11();
    let mut groups: Vec<(Vec<[f64; 6]>, Vec<String>)> = Vec::new();
    let mut ids = Vec::new();
    let mut n = 0;
    for id in reg.ids().filter(|id| agreement_block(id)) {
        ids.push(id.to_string());
        for (_, text) in states(id, reg.properties(id).unwrap()) {
            let s = shape(outline(&text).expect("an agreement block is in the table"));
            assert!(!s.is_empty(), "{text} has an outline");
            n += 1;
            match groups.iter_mut().find(|(g, _)| *g == s) {
                Some((_, v)) => v.push(text),
                None => groups.push((s, vec![text])),
            }
        }
    }
    assert!(ids.iter().any(|i| i == "minecraft:lever"));
    assert!(ids.iter().any(|i| i.ends_with("_button")));
    assert!(ids.iter().any(|i| i.ends_with("_wall_hanging_sign")));
    eprintln!("outline agreement: {} blocks, {n} states", ids.len());
    let doc = json!({
        "blocks": ids,
        "cases": groups
            .iter()
            .map(|(s, v)| json!({ "boxes": s, "states": v }))
            .collect::<Vec<_>>(),
        "source": "delvewright_dsl::blockshape::outline asked every state of the lever, every button and every sign; written by crates/delvec/tests/outline_ledger.rs",
        "states": n,
    });
    write_or_check(AGREEMENT, &canonical(&doc));
}
