//! A generated PackTest template that writes a pressable's cell hands the
//! block back when it ends.
//!
//! Every template of a suite runs on one server and one world, in an order the
//! compiler does not choose, and solo batches (`crate::compiler::solo`) share
//! that world too. Scores are owned by the template that reads them
//! (`crate::compiler::batchstate`); a block is not a score, and a pressable's
//! block is the one block siblings read from load: `v04_prop` asserts every
//! pressable stands, and a highlight's hold kills its display when the block it
//! watches is gone. A campaign's own effects may replace that block — the
//! gallery's case turns into a sea lantern when its objective completes — so a
//! template that drives such an effect leaves a world in which a sibling that
//! runs after it reads the effect's block instead of the pressable's, and the
//! same bytes give a red or a green by order alone.
//!
//! The rule is read off the finished tree, so it reaches templates not yet
//! written: a template whose body, or any function it dispatches (transitively,
//! `schedule` included, and `tick` when the template `await`s and so lets the
//! server tick), holds a `setblock` or `fill` over a pressable's cell ends with
//! that pressable's block laid back, from [`crate::compiler::pressable::props`],
//! the list `setup_finish` lays. Appended last, after every assertion and every
//! `await`, so the template's own reading is untouched.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

/// How many templates were examined and which hand blocks back.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HandbackBinding {
    /// Generated PackTest templates examined.
    pub templates: usize,
    /// Pressable cells in the world.
    pub cells: usize,
    /// `(template, cells handed back)` per template that writes one.
    pub handed: Vec<(String, usize)>,
}

impl HandbackBinding {
    /// The record written to `validation/packtest-handback.json`.
    pub fn to_json(&self) -> Value {
        json!({
            "templates": self.templates,
            "pressable_cells": self.cells,
            "handing_back": self.handed.len(),
            "handed": self.handed.iter().map(|(t, n)| json!({"template": t, "cells": n})).collect::<Vec<_>>(),
        })
    }

    /// The build line.
    pub fn line(&self) -> String {
        format!(
            "packtest hand-back binding: {} of {} template(s) write one of {} pressable cell(s) and lay it back when they end",
            self.handed.len(),
            self.templates,
            self.cells
        )
    }
}

/// The absolute cells a line writes with `setblock` or `fill`; relative or
/// local coordinates write nothing this can name.
fn written_cells(line: &str) -> Option<([i32; 3], [i32; 3])> {
    let line = line.trim();
    if line.starts_with('#') {
        return None;
    }
    let payload = match line.rfind(" run ") {
        Some(i) if line.starts_with("execute ") => &line[i + " run ".len()..],
        _ => line,
    };
    let toks: Vec<&str> = payload.split_whitespace().collect();
    let n = match toks.first().copied() {
        Some("setblock") => 3,
        Some("fill") => 6,
        _ => return None,
    };
    let nums: Option<Vec<i32>> = toks.get(1..=n)?.iter().map(|t| t.parse().ok()).collect();
    let v = nums?;
    if n == 3 {
        let c = [v[0], v[1], v[2]];
        Some((c, c))
    } else {
        Some((
            [v[0].min(v[3]), v[1].min(v[4]), v[2].min(v[5])],
            [v[0].max(v[3]), v[1].max(v[4]), v[2].max(v[5])],
        ))
    }
}

/// Lay each template's written pressable cells back at its end, in place, and
/// return the binding.
pub fn hand_back(
    ns: &str,
    props: &[crate::compiler::light::Placement],
    out: &mut BTreeMap<String, Vec<u8>>,
) -> HandbackBinding {
    let collect = |out: &BTreeMap<String, Vec<u8>>, prefix: &str| -> BTreeMap<String, String> {
        out.iter()
            .filter_map(|(p, b)| {
                let name = p.strip_prefix(prefix)?.strip_suffix(".mcfunction")?;
                Some((name.to_string(), std::str::from_utf8(b).ok()?.to_string()))
            })
            .collect()
    };
    let mut fns: BTreeMap<String, String> = BTreeMap::new();
    for pack in ["datapack", "creator-datapack", "packtest-datapack"] {
        fns.extend(collect(out, &format!("{pack}/data/{ns}/function/")));
    }
    let test_prefix = format!("packtest-datapack/data/{ns}/test/");
    let templates = collect(out, &test_prefix);
    let mut binding = HandbackBinding {
        templates: templates.len(),
        cells: props.len(),
        handed: Vec::new(),
    };
    if props.is_empty() {
        return binding;
    }
    for (name, body) in &templates {
        let mut seen: BTreeSet<String> = BTreeSet::new();
        let mut stack: Vec<String> = body
            .lines()
            .filter_map(|l| crate::compiler::batchstate::dispatch_target(ns, l))
            .collect();
        if body.lines().any(|l| l.trim_start().starts_with("await ")) {
            stack.push("tick".to_string());
        }
        while let Some(f) = stack.pop() {
            if !seen.insert(f.clone()) {
                continue;
            }
            if let Some(b) = fns.get(&f) {
                stack.extend(
                    b.lines()
                        .filter_map(|l| crate::compiler::batchstate::dispatch_target(ns, l)),
                );
            }
        }
        let mut regions: Vec<([i32; 3], [i32; 3])> =
            body.lines().filter_map(written_cells).collect();
        for f in &seen {
            if let Some(b) = fns.get(f) {
                regions.extend(b.lines().filter_map(written_cells));
            }
        }
        let back: Vec<&crate::compiler::light::Placement> = props
            .iter()
            .filter(|p| {
                regions
                    .iter()
                    .any(|(lo, hi)| (0..3).all(|i| lo[i] <= p.pos[i] && p.pos[i] <= hi[i]))
            })
            .collect();
        if back.is_empty() {
            continue;
        }
        let mut text = body.clone();
        if !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str("# The pressables this template's drive may have written, laid back.\n");
        for p in &back {
            let [x, y, z] = p.pos;
            text.push_str(&format!("setblock {x} {y} {z} {}\n", p.block));
        }
        out.insert(format!("{test_prefix}{name}.mcfunction"), text.into_bytes());
        binding.handed.push((name.clone(), back.len()));
    }
    binding
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(entries: &[(&str, &str)]) -> BTreeMap<String, Vec<u8>> {
        entries
            .iter()
            .map(|(p, b)| (p.to_string(), b.as_bytes().to_vec()))
            .collect()
    }

    #[test]
    fn a_template_whose_drive_writes_a_pressable_lays_it_back_and_no_other_does() {
        let props = vec![crate::compiler::light::Placement {
            pos: [15, 67, 9],
            block: "minecraft:stone_button[face=floor,facing=north]".to_string(),
        }];
        let mut out = tree(&[
            (
                "datapack/data/n/function/complete.mcfunction",
                "execute if score #p dw.f matches 1 run setblock 15 67 9 minecraft:sea_lantern\n",
            ),
            (
                "datapack/data/n/function/tick.mcfunction",
                "execute as @a[scores={dw.i=1..}] run function n:complete\n",
            ),
            (
                "packtest-datapack/data/n/test/completes.mcfunction",
                "#> completes\n# @dummy\nfunction n:complete\n",
            ),
            (
                "packtest-datapack/data/n/test/waits.mcfunction",
                "#> waits\n# @dummy\nawait score #p dw.sys matches 1\n",
            ),
            (
                "packtest-datapack/data/n/test/reads.mcfunction",
                "#> reads\n# @dummy\nassert block 15 67 9 minecraft:stone_button\n",
            ),
        ]);
        let b = hand_back("n", &props, &mut out);
        assert_eq!(b.templates, 3);
        assert_eq!(
            b.handed,
            vec![("completes".to_string(), 1), ("waits".to_string(), 1)]
        );
        assert!(
            std::str::from_utf8(&out["packtest-datapack/data/n/test/completes.mcfunction"])
                .unwrap()
                .ends_with("setblock 15 67 9 minecraft:stone_button[face=floor,facing=north]\n")
        );
        assert_eq!(
            std::str::from_utf8(&out["packtest-datapack/data/n/test/reads.mcfunction"]).unwrap(),
            "#> reads\n# @dummy\nassert block 15 67 9 minecraft:stone_button\n"
        );
    }

    #[test]
    fn a_fill_over_the_cell_counts_and_a_relative_write_names_nothing() {
        assert_eq!(
            written_cells("fill 10 60 5 20 70 15 minecraft:air"),
            Some(([10, 60, 5], [20, 70, 15]))
        );
        assert_eq!(written_cells("setblock ~ ~1 ~ minecraft:stone"), None);
        assert_eq!(written_cells("# setblock 1 2 3 minecraft:stone"), None);
    }
}
