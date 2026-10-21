//! A generated PackTest template whose drive takes every player's body runs
//! alone: in its own test environment, so its own batch, with no other test's
//! dummy on the server.
//!
//! PackTest batches the suite by test environment, up to fifty tests a batch,
//! every `# @dummy` test with its own dummy, all on one server. A campaign
//! function that moves or re-modes `@a` — a cutscene's start (`gamemode
//! spectator @a`) and its return (`tp @a <mark>`) — acts on every dummy in the
//! batch. A delve's party is one to four players; a batch is up to fifty. The
//! return stacks every live dummy on one cell, and vanilla's entity cramming
//! (`max_entity_cramming`, 24) kills them: measured on the gallery, 26 and 28
//! dummies stacked on the final cutscene's mark, 12–19 "was squished too much"
//! deaths per run; and a sibling (`creator_camera_in_a_block`) whose dummy,
//! taken into the scene and left at another test's position, no longer moved
//! on its own `tp @s`, so its stamp read that place.
//!
//! The rule is read off the finished tree, so it reaches templates not yet
//! written: a template is **solo** when its body, or any function it dispatches
//! (transitively, `tick` and `schedule` included), holds a line that teleports,
//! re-modes or makes spectate a party-wide `@a`. Each solo template gains
//! `# @environment <ns>:solo/<template>` and the environment
//! `test_environment/solo/<template>.json` (an empty `all_of`, the world as it
//! is). Every other template stays in `minecraft:default`.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};

/// The binding: how many templates were examined and which were made solo,
/// each with the line that took the bodies.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SoloBinding {
    /// Generated PackTest templates examined.
    pub templates: usize,
    /// `(template, "<function>: <line>")` per solo template, in template order.
    pub solo: Vec<(String, String)>,
}

impl SoloBinding {
    /// The record written to `validation/packtest-solo.json`.
    pub fn to_json(&self) -> Value {
        json!({
            "templates": self.templates,
            "solo_count": self.solo.len(),
            "solo": self.solo.iter().map(|(t, w)| json!({"template": t, "takes": w})).collect::<Vec<_>>(),
        })
    }

    /// The build line.
    pub fn line(&self) -> String {
        format!(
            "packtest solo binding: {} of {} template(s) take every body and run in their own test environment",
            self.solo.len(),
            self.templates
        )
    }
}

/// Whether an `@a` selector reaches every player: bare, or filtered by nothing
/// that narrows it to a named, counted, placed or tagged few.
fn party_wide(sel: &str) -> bool {
    if sel == "@a" {
        return true;
    }
    let Some(inner) = sel.strip_prefix("@a[").and_then(|s| s.strip_suffix(']')) else {
        return false;
    };
    !inner.split(',').any(|term| {
        let term = term.trim();
        ["limit=", "distance=", "dx=", "dy=", "dz=", "name="]
            .iter()
            .any(|k| term.starts_with(k))
            || (term.starts_with("tag=") && !term.starts_with("tag=!"))
    })
}

/// Whether a line teleports, re-modes or makes spectate every player.
pub(crate) fn takes_every_body(line: &str) -> bool {
    let line = line.trim();
    if line.starts_with('#') {
        return false;
    }
    let payload = match line.rfind(" run ") {
        Some(i) if line.starts_with("execute ") => &line[i + " run ".len()..],
        _ => line,
    };
    let toks: Vec<&str> = payload.split_whitespace().collect();
    let target = match toks.first().copied() {
        Some("tp" | "teleport") => toks.get(1),
        Some("gamemode" | "spectate") => toks.get(2),
        _ => None,
    };
    target.is_some_and(|t| party_wide(t))
}

/// The first line in a function that takes every body, if any.
fn own_take(body: &str) -> Option<&str> {
    body.lines().find(|l| takes_every_body(l)).map(str::trim)
}

/// Make every body-taking template solo, in place, and return the binding.
pub fn isolate(ns: &str, out: &mut BTreeMap<String, Vec<u8>>) -> SoloBinding {
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

    let mut binding = SoloBinding {
        templates: templates.len(),
        solo: Vec::new(),
    };
    for (name, body) in &templates {
        let mut witness = own_take(body).map(|l| format!("{name}: {l}"));
        if witness.is_none() {
            let mut seen: BTreeSet<String> = BTreeSet::new();
            let mut stack: Vec<String> = body
                .lines()
                .filter_map(|l| crate::compiler::batchstate::dispatch_target(ns, l))
                .collect();
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
            witness = seen
                .iter()
                .find_map(|f| own_take(fns.get(f)?).map(|l| format!("{f}: {l}")));
        }
        let Some(witness) = witness else {
            continue;
        };
        let mut lines: Vec<&str> = body.lines().collect();
        let at = usize::from(lines.first().is_some_and(|l| l.starts_with("#>")));
        let directive = format!("# @environment {ns}:solo/{name}");
        lines.insert(at, &directive);
        let mut text = lines.join("\n");
        if body.ends_with('\n') {
            text.push('\n');
        }
        out.insert(format!("{test_prefix}{name}.mcfunction"), text.into_bytes());
        let mut env = serde_json::to_vec_pretty(&json!({
            "type": "minecraft:all_of",
            "definitions": [],
        }))
        .expect("json serializes");
        env.push(b'\n');
        out.insert(
            format!("packtest-datapack/data/{ns}/test_environment/solo/{name}.json"),
            env,
        );
        binding.solo.push((name.clone(), witness));
    }
    binding
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_party_wide_take_is_told_from_a_narrowed_one() {
        assert!(takes_every_body("tp @a @e[tag=dw_csmark_x,limit=1]"));
        assert!(takes_every_body("gamemode spectator @a"));
        assert!(takes_every_body(
            "execute if score #p dw.sys matches 1 run gamemode adventure @a[tag=!dw_free]"
        ));
        assert!(!takes_every_body("tp @a[tag=dw_t_x,limit=1] 0 0 0"));
        assert!(!takes_every_body("tp @s 0 0 0"));
        assert!(!takes_every_body("spawnpoint @a 1 2 3"));
        assert!(!takes_every_body("tellraw @a \"x\""));
        assert!(!takes_every_body(
            "execute as @a[predicate=!g:sneak] run spectate @n[tag=cam] @s"
        ));
    }

    fn tree(entries: &[(&str, &str)]) -> BTreeMap<String, Vec<u8>> {
        entries
            .iter()
            .map(|(p, b)| (p.to_string(), b.as_bytes().to_vec()))
            .collect()
    }

    #[test]
    fn a_template_reaching_a_take_runs_alone_and_no_other_does() {
        let mut out = tree(&[
            (
                "datapack/data/n/function/cs_end.mcfunction",
                "tp @a @e[tag=m,limit=1]\n",
            ),
            (
                "datapack/data/n/function/complete.mcfunction",
                "schedule function n:cs_end 40t\n",
            ),
            ("datapack/data/n/function/quiet.mcfunction", "say hi\n"),
            (
                "packtest-datapack/data/n/test/drives.mcfunction",
                "#> drives\n# @dummy\nfunction n:complete\n",
            ),
            (
                "packtest-datapack/data/n/test/stays.mcfunction",
                "#> stays\n# @dummy\nfunction n:quiet\n",
            ),
        ]);
        let b = isolate("n", &mut out);
        assert_eq!(b.templates, 2);
        assert_eq!(
            b.solo,
            vec![(
                "drives".to_string(),
                "cs_end: tp @a @e[tag=m,limit=1]".to_string()
            )]
        );
        assert_eq!(
            std::str::from_utf8(&out["packtest-datapack/data/n/test/drives.mcfunction"]).unwrap(),
            "#> drives\n# @environment n:solo/drives\n# @dummy\nfunction n:complete\n"
        );
        assert!(out.contains_key("packtest-datapack/data/n/test_environment/solo/drives.json"));
        assert_eq!(
            std::str::from_utf8(&out["packtest-datapack/data/n/test/stays.mcfunction"]).unwrap(),
            "#> stays\n# @dummy\nfunction n:quiet\n"
        );
        assert!(!out.contains_key("packtest-datapack/data/n/test_environment/solo/stays.json"));
    }
}
