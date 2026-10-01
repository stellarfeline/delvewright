//! A player who is watching is out of play everywhere (spec-0077 §5).
//!
//! The observation tag ([`CUTSCENE_TAG`]) marks a player who is watching, not
//! playing: a cutscene viewer, and a player waiting out a declared respawn wait.
//! Its staging invariant is that campaign machinery neither asks anything of a
//! watcher nor harms one. A cutscene holds every player for its length, so a
//! rule that reads where a player stands met a watcher only through the cutscene
//! camera. A respawn wait holds ONE player, a spectator, while the rest play on:
//! a watcher can then stand anywhere, and every rule that reads where a player
//! stands — a proximity trigger, a lane's patrol, a reach box, a killing volume
//! — reads the watcher too unless its selector says otherwise.
//!
//! [`census`] reads the shipped datapack and puts every positional player
//! selector in one of three states: it excludes the observation tag; it stands
//! at a site [`ALLOWED`] names, with the reason a watcher may be seen there; or
//! it is unguarded, which refuses the build. It runs only when the campaign
//! declares `world.respawn_wait`, the state that puts a watcher outside a
//! cutscene, and it is feature-blind: an emitter written later is judged by
//! existing.
//!
//! "Positional" is a selector whose answer depends on where a player stands:
//! `@a`/`@r` (or a player-typed `@e`) with a box or `distance` term, and `@p` or
//! a player-typed `@n`, which pick by nearness.

use std::collections::BTreeMap;

use crate::compiler::affordance::{
    matching_bracket, selector_has_term, selector_terms, shipped_functions,
};
use crate::compiler::emit::{BuildOutput, CUTSCENE_TAG};

/// A site where a positional player selector may see a watcher, with why.
pub struct AllowedSite {
    /// The site's name, as the ledger counts it.
    pub site: &'static str,
    /// Why a watcher may be seen there.
    pub reason: &'static str,
    /// Does this (function name, command line) stand at the site?
    pub matches: fn(&str, &str) -> bool,
}

/// The sites where a positional player selector may see a watcher.
pub const ALLOWED: &[AllowedSite] = &[
    AllowedSite {
        site: "area night vision",
        reason: "an area's `night-vision` mitigation gives or clears an effect that only \
                 changes what a body sees; it asks nothing of a watcher and does not harm one",
        matches: |_, line| {
            (line.contains("effect give @a[") || line.contains("effect clear @a["))
                && line.contains("minecraft:night_vision")
        },
    },
    AllowedSite {
        site: "ladder staged blow",
        reason: "`wave_strike_*` / `wave_chip_*` are the bot ladder's staging, run by rcon and \
                 by no campaign machinery; `by @p` only names the attacker a kill is credited to",
        matches: |name, line| {
            (name.starts_with("wave_strike_") || name.starts_with("wave_chip_"))
                && line.contains("minecraft:player_attack by @p")
        },
    },
];

/// The census of positional player selectors over a shipped tree.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ObserverCensus {
    /// Shipped functions read.
    pub functions: usize,
    /// Positional player selectors found.
    pub selectors: usize,
    /// Of those, the ones that exclude the observation tag.
    pub guarded: usize,
    /// Of those, the ones at an allowed site, per site.
    pub allowed: BTreeMap<&'static str, usize>,
    /// Of those, the ones in neither state: `(function, selector, line)`.
    pub unguarded: Vec<(String, String, String)>,
}

impl ObserverCensus {
    /// The ledger written to `validation/observer-census.json`.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "functions_read": self.functions,
            "positional_player_selectors": self.selectors,
            "exclude_observation_tag": self.guarded,
            "allowed": self.allowed.iter().map(|(site, n)| {
                let reason = ALLOWED.iter().find(|a| a.site == *site).map(|a| a.reason);
                serde_json::json!({ "site": site, "count": n, "reason": reason })
            }).collect::<Vec<_>>(),
            "unguarded": self.unguarded.len(),
        })
    }

    /// The one-line binding statement a build prints.
    pub fn binding(&self) -> String {
        let allowed: usize = self.allowed.values().sum();
        format!(
            "observer binding: {} positional player selector(s) over {} shipped function(s); {} \
             exclude `{CUTSCENE_TAG}`, {} at an allowed site, {} unguarded (DW0925).",
            self.selectors,
            self.functions,
            self.guarded,
            allowed,
            self.unguarded.len()
        )
    }
}

/// Every positional player selector in `line`, as `(selector kind, args)`.
pub fn positional_player_selectors(line: &str) -> Vec<(char, String)> {
    let mut out = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0usize;
    while i + 1 < bytes.len() {
        if bytes[i] != b'@' {
            i += 1;
            continue;
        }
        let kind = bytes[i + 1] as char;
        let after = i + 2;
        // A selector is a whole word: `@p` stands after a space (or opens the
        // line) and is followed by its arguments, a space or the line's end, so
        // `@phil` in a narrated line is not one.
        let word_start = i == 0 || bytes[i - 1] == b' ';
        let word_end = matches!(bytes.get(after), None | Some(b'[') | Some(b' '));
        if !matches!(kind, 'a' | 'p' | 'r' | 'e' | 'n') || !word_start || !word_end {
            i += 1;
            continue;
        }
        let args = if bytes.get(after) == Some(&b'[') {
            match matching_bracket(line, after + 1) {
                Some(close) => {
                    let a = line[after + 1..close].to_string();
                    i = close + 1;
                    Some(a)
                }
                None => break,
            }
        } else {
            i = after;
            None
        };
        let args_str = args.as_deref().unwrap_or("");
        let positional_term = selector_terms(args_str).any(|t| {
            t.split_once('=').is_some_and(|(k, _)| {
                matches!(k, "x" | "y" | "z" | "dx" | "dy" | "dz" | "distance")
            })
        });
        let player_typed =
            selector_terms(args_str).any(|t| t == "type=minecraft:player" || t == "type=player");
        let positional = match kind {
            'p' => true,
            'a' | 'r' => positional_term,
            'e' => player_typed && positional_term,
            'n' => player_typed,
            _ => false,
        };
        if positional {
            out.push((kind, args_str.to_string()));
        }
    }
    out
}

/// Read the shipped datapack and sort every positional player selector.
pub fn census(out: &BuildOutput) -> ObserverCensus {
    let guard = format!("tag=!{CUTSCENE_TAG}");
    let mut c = ObserverCensus::default();
    for (name, body) in shipped_functions(out) {
        c.functions += 1;
        for line in body.lines() {
            for (kind, args) in positional_player_selectors(line) {
                c.selectors += 1;
                if selector_has_term(&args, &guard) {
                    c.guarded += 1;
                } else if let Some(site) = ALLOWED.iter().find(|a| (a.matches)(&name, line)) {
                    *c.allowed.entry(site.site).or_default() += 1;
                } else {
                    let sel = if args.is_empty() {
                        format!("@{kind}")
                    } else {
                        format!("@{kind}[{args}]")
                    };
                    c.unguarded
                        .push((name.clone(), sel, line.trim().to_string()));
                }
            }
        }
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_positional_shapes_are_found_and_the_rest_are_not() {
        let found = |l: &str| positional_player_selectors(l).len();
        assert_eq!(found("execute if entity @a[distance=..5] run say x"), 1);
        assert_eq!(found("effect give @a[x=1,dx=2,y=3,dy=4,z=5,dz=6] speed"), 1);
        assert_eq!(found("damage @s 1 minecraft:player_attack by @p"), 1);
        assert_eq!(found("tp @a[tag=x] 0 0 0"), 0);
        assert_eq!(found("execute as @e[tag=w,distance=..3] run kill @s"), 0);
        assert_eq!(found("tag @e[type=minecraft:player,distance=..3] add t"), 1);
        assert_eq!(found("execute at @n[type=player] run say x"), 1);
        assert_eq!(found("tellraw @a {\"text\":\"ask @phil\"}"), 0);
    }
}
