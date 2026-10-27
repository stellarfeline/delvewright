//! A generated PackTest template's dummy enters as a new player: every score
//! its name holds is reset before the template's first command.
//!
//! A scoreboard entry belongs to a player's NAME, not to the body wearing it.
//! PackTest names each `# @dummy` from a small pool (`Dummy<n>`), so over one
//! suite many dummies share a name with an earlier one, and the later one is
//! born holding everything the earlier one left on that name. The death edge
//! (`cp_respawn_check`) reads exactly such a residue: a dummy that died in one
//! template ("was squished too much") and left before it came back leaves
//! `dw.deaths` above `dw.death_ack`, and the next dummy of that name fires the
//! edge on its first tick: alive, with another player present, it is put on
//! the respawn wait (`rw_start`: tagged `dw_cutscene`, made spectator). Measured
//! on the gallery: `watch_npc_warden`, whose dummy `Dummy623` had died in an
//! earlier template, read `#wr_1` 0 because the warden's watch looks only at a
//! player not in a cutscene.
//!
//! A real player who joins a delve for the first time holds no score, and
//! that is the body every template is written against, so each `# @dummy`
//! template gains `scoreboard players reset @s` as its first command (`@s` is
//! the template's own dummy). The rule is applied to the finished tree, after
//! every emitter, so it reaches templates not yet written.

use serde_json::{Value, json};
use std::collections::BTreeMap;

/// The line every dummy template starts with.
pub const RESET: &str = "scoreboard players reset @s";

/// What [`fresh_bodies`] did to the tree.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct FreshBinding {
    /// Generated PackTest templates examined.
    pub templates: usize,
    /// Templates that carry a dummy and now reset its name's scores at entry.
    pub reset: usize,
}

impl FreshBinding {
    /// The record written to `validation/packtest-fresh.json`.
    pub fn to_json(&self) -> Value {
        json!({"templates": self.templates, "reset": self.reset, "line": RESET})
    }

    /// The build line.
    pub fn line(&self) -> String {
        format!(
            "packtest fresh-body binding: {} of {} template(s) carry a dummy and reset every score its name holds before their first command",
            self.reset, self.templates
        )
    }
}

/// Reset each dummy template's dummy at entry, in place, and return the binding.
pub fn fresh_bodies(ns: &str, out: &mut BTreeMap<String, Vec<u8>>) -> FreshBinding {
    let prefix = format!("packtest-datapack/data/{ns}/test/");
    let mut binding = FreshBinding::default();
    let names: Vec<String> = out
        .keys()
        .filter(|p| p.starts_with(&prefix) && p.ends_with(".mcfunction"))
        .cloned()
        .collect();
    for path in names {
        binding.templates += 1;
        let Ok(body) = std::str::from_utf8(&out[&path]) else {
            continue;
        };
        let mut lines: Vec<&str> = body.lines().collect();
        let Some(first) = lines
            .iter()
            .position(|l| !l.trim().is_empty() && !l.starts_with('#'))
        else {
            continue;
        };
        // The directives are the comment lines above the first command.
        if !lines[..first].iter().any(|l| l.trim() == "# @dummy") {
            continue;
        }
        if lines[first].trim() == RESET {
            binding.reset += 1;
            continue;
        }
        lines.insert(first, RESET);
        let mut text = lines.join("\n");
        if body.ends_with('\n') {
            text.push('\n');
        }
        out.insert(path, text.into_bytes());
        binding.reset += 1;
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
    fn a_dummy_template_resets_its_dummy_first_and_no_other_changes() {
        let mut out = tree(&[
            (
                "packtest-datapack/data/n/test/a.mcfunction",
                "#> a\n# @environment n:solo/a\n# @dummy\n# @timeout 100\n\ntag @s add t\n",
            ),
            (
                "packtest-datapack/data/n/test/b.mcfunction",
                "#> b\nsay no dummy\n",
            ),
            ("datapack/data/n/function/f.mcfunction", "# @dummy\nsay f\n"),
        ]);
        let b = fresh_bodies("n", &mut out);
        assert_eq!(
            b,
            FreshBinding {
                templates: 2,
                reset: 1
            }
        );
        assert_eq!(
            std::str::from_utf8(&out["packtest-datapack/data/n/test/a.mcfunction"]).unwrap(),
            "#> a\n# @environment n:solo/a\n# @dummy\n# @timeout 100\n\nscoreboard players reset @s\ntag @s add t\n"
        );
        assert_eq!(
            std::str::from_utf8(&out["packtest-datapack/data/n/test/b.mcfunction"]).unwrap(),
            "#> b\nsay no dummy\n"
        );
        assert_eq!(
            std::str::from_utf8(&out["datapack/data/n/function/f.mcfunction"]).unwrap(),
            "# @dummy\nsay f\n"
        );
        // Idempotent: a second pass writes nothing more.
        let again = fresh_bodies("n", &mut out);
        assert_eq!(again.reset, 1);
        assert_eq!(
            std::str::from_utf8(&out["packtest-datapack/data/n/test/a.mcfunction"])
                .unwrap()
                .matches(RESET)
                .count(),
            1
        );
    }
}
