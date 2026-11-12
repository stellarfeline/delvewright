//! A generated PackTest template lays down every world container it reads.
//!
//! The suite runs on PackTest's own level, not on the delve's placed world:
//! the town's buildings, and the prefab containers in them, stand only once
//! some template's drive has run the campaign's placement (`place_all`, from
//! `tick` once `#placed` reads 0). Every template that pins `#placed` to 1
//! places nothing. A template whose body acts on a container at a fixed world
//! cell — empties it, has the objective's activation fill it, counts what it
//! holds — therefore read a container that stood or did not by the order
//! PackTest ran the suite in: the cheerful town's `obj_activate_fetch_the_fireworks`
//! and `collect_container`, run alone, both read 0 from the rocket store, the
//! barrel no template had placed yet; in the full suite each passed or failed
//! by whether a placing template had run before it. Isolating the template in
//! an environment of its own does not change that (`collect_container` was
//! already solo): the state is the world's existence, not a sibling's write.
//!
//! The rule is read off the finished tree, so it reaches templates not yet
//! written: every absolute cell a template's own body addresses as a block
//! entity (`… block <x> <y> <z> …`) that the assembled world holds a container
//! at is laid down with that state, by `setblock`, right after the template's
//! own `setup` (or, with none, after its opening comment lines) — before any
//! line that reads or writes it. `setblock` of the state already standing
//! changes nothing, so a template whose world was placed reads what it did.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

/// How many templates were examined and which lay a container.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OwnCellsBinding {
    /// Generated PackTest templates examined.
    pub templates: usize,
    /// `(template, cells laid)` per template that lays one.
    pub laying: Vec<(String, usize)>,
}

impl OwnCellsBinding {
    /// The record written to `validation/packtest-own-cells.json`.
    pub fn to_json(&self) -> Value {
        json!({
            "templates": self.templates,
            "laying_count": self.laying.len(),
            "laying": self.laying.iter().map(|(t, n)| json!({"template": t, "cells": n})).collect::<Vec<_>>(),
        })
    }

    /// The build line.
    pub fn line(&self) -> String {
        format!(
            "packtest own-cells binding: {} of {} template(s) read a world container and lay it down before they read it",
            self.laying.len(),
            self.templates
        )
    }
}

/// The absolute cells a line addresses as a block entity: every
/// `block <x> <y> <z>` whose three coordinates are whole numbers. Relative and
/// local coordinates name no cell.
pub(crate) fn block_entity_cells(line: &str) -> Vec<[i32; 3]> {
    let line = line.trim();
    if line.starts_with('#') {
        return Vec::new();
    }
    let toks: Vec<&str> = line.split_whitespace().collect();
    toks.windows(4)
        .filter(|w| w[0] == "block")
        .filter_map(|w| Some([w[1].parse().ok()?, w[2].parse().ok()?, w[3].parse().ok()?]))
        .collect()
}

/// Lay each template's world containers down before it reads them, in place,
/// and return the binding.
pub fn lay_containers(
    ns: &str,
    blocks: &crate::compiler::blockstate::BlockMap,
    out: &mut BTreeMap<String, Vec<u8>>,
) -> OwnCellsBinding {
    let test_prefix = format!("packtest-datapack/data/{ns}/test/");
    let templates: BTreeMap<String, String> = out
        .iter()
        .filter_map(|(p, b)| {
            let name = p.strip_prefix(&test_prefix)?.strip_suffix(".mcfunction")?;
            Some((name.to_string(), std::str::from_utf8(b).ok()?.to_string()))
        })
        .collect();
    let mut binding = OwnCellsBinding {
        templates: templates.len(),
        laying: Vec::new(),
    };
    let setup = format!("function {ns}:setup");
    for (name, body) in &templates {
        let cells: BTreeSet<[i32; 3]> = body.lines().flat_map(block_entity_cells).collect();
        let lay: Vec<String> = cells
            .iter()
            .filter_map(|c| {
                let state = blocks.get(c)?.as_str();
                crate::compiler::loot::is_container(state)
                    .then(|| format!("setblock {} {} {} {state}", c[0], c[1], c[2]))
            })
            .collect();
        if lay.is_empty() {
            continue;
        }
        let lines: Vec<&str> = body.lines().collect();
        let at = lines
            .iter()
            .position(|l| l.trim() == setup)
            .map(|i| i + 1)
            .unwrap_or_else(|| {
                lines
                    .iter()
                    .position(|l| !l.starts_with('#') && !l.trim().is_empty())
                    .unwrap_or(lines.len())
            });
        let mut text: Vec<String> = lines[..at].iter().map(|s| s.to_string()).collect();
        text.extend(lay.iter().cloned());
        text.extend(lines[at..].iter().map(|s| s.to_string()));
        let mut joined = text.join("\n");
        if body.ends_with('\n') {
            joined.push('\n');
        }
        out.insert(
            format!("{test_prefix}{name}.mcfunction"),
            joined.into_bytes(),
        );
        binding.laying.push((name.clone(), lay.len()));
    }
    binding
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> crate::compiler::blockstate::BlockMap {
        let mut m = BTreeMap::new();
        m.insert(
            [207, 164, 76],
            "minecraft:barrel[facing=up,open=false]".to_string(),
        );
        m.insert([1, 2, 3], "minecraft:stone".to_string());
        crate::compiler::blockstate::interned(m)
    }

    fn tree(body: &str) -> BTreeMap<String, Vec<u8>> {
        let mut out = BTreeMap::new();
        out.insert(
            "packtest-datapack/data/camp/test/t.mcfunction".to_string(),
            body.as_bytes().to_vec(),
        );
        out
    }

    fn read(out: &BTreeMap<String, Vec<u8>>) -> String {
        String::from_utf8(out["packtest-datapack/data/camp/test/t.mcfunction"].clone()).unwrap()
    }

    /// The town's `obj_activate_fetch_the_fireworks`, as it was emitted: it
    /// empties, fills and reads the rocket store and never puts it there.
    const ACTIVATE: &str = "#> t\n# @dummy\n# @timeout 100\n\nscoreboard players reset @s\nfunction camp:setup\nscoreboard players set #placed dw.sys 1\nitem replace block 207 164 76 container.0 with minecraft:air\nexecute store success score #fx0 dw.sys if items block 207 164 76 container.0 minecraft:firework_rocket\nassert score #fx0 dw.sys matches 0\nfunction camp:activate_o_fetch\nexecute store success score #fx1 dw.sys if items block 207 164 76 container.0 minecraft:firework_rocket\nassert score #fx1 dw.sys matches 1\n";

    #[test]
    fn a_template_that_reads_a_world_container_lays_it_first() {
        let mut out = tree(ACTIVATE);
        let b = lay_containers("camp", &world(), &mut out);
        assert_eq!(b.templates, 1);
        assert_eq!(b.laying, vec![("t".to_string(), 1)]);
        let text = read(&out);
        let lines: Vec<&str> = text.lines().collect();
        let laid = lines
            .iter()
            .position(|l| *l == "setblock 207 164 76 minecraft:barrel[facing=up,open=false]")
            .expect(&text);
        let setup = lines
            .iter()
            .position(|l| *l == "function camp:setup")
            .unwrap();
        let first_use = lines
            .iter()
            .position(|l| l.contains(" block 207 164 76"))
            .unwrap();
        assert!(setup < laid && laid < first_use, "{text}");
        assert_eq!(text.matches("setblock").count(), 1, "{text}");
        assert!(text.ends_with('\n'));
    }

    #[test]
    fn a_cell_that_is_no_container_or_relative_is_left_alone() {
        let body = "# t\nfunction camp:setup\nexecute if block 1 2 3 minecraft:stone run say x\nitem replace block ~ ~ ~ container.0 with minecraft:air\ndata get block 9 9 9\n";
        let mut out = tree(body);
        let b = lay_containers("camp", &world(), &mut out);
        assert!(b.laying.is_empty());
        assert_eq!(read(&out), body);
    }

    #[test]
    fn with_no_setup_the_container_is_laid_after_the_opening_comments() {
        let body = "#> t\n# @dummy\nitem replace block 207 164 76 container.0 with minecraft:air\n";
        let mut out = tree(body);
        lay_containers("camp", &world(), &mut out);
        assert_eq!(
            read(&out),
            "#> t\n# @dummy\nsetblock 207 164 76 minecraft:barrel[facing=up,open=false]\nitem replace block 207 164 76 container.0 with minecraft:air\n"
        );
    }

    #[test]
    fn the_cells_a_line_addresses() {
        assert_eq!(
            block_entity_cells(
                "execute store result score #c dw.sys if items block 1 -2 3 container.* x"
            ),
            vec![[1, -2, 3]]
        );
        assert!(block_entity_cells("setblock 1 2 3 minecraft:barrel").is_empty());
        assert!(block_entity_cells("# item replace block 1 2 3 container.0 with x").is_empty());
    }
}
