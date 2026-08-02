//! Vendored 1.21.11 Brigadier command-tree validator (ADR-0011).
//!
//! Every emitted `.mcfunction` line is checked against the command tree that
//! Mojang's data generator produces (vendored under `data/commands-1.21.11.json`,
//! see `data/PROVENANCE.md`). mecha re-validates the same lines in CI as an
//! independent cross-check; disagreement fails CI.
//!
//! ## Validation depth (documented honestly)
//!
//! The validator checks command **structure**, not argument **values**:
//!
//! - The first token must be a known command root (`scoreboard`, `execute`, …).
//! - `literal` nodes are matched exactly; the walk descends the tree.
//! - `argument` nodes accept their tokens without parsing the value's internal
//!   syntax. Multi-token parsers are given their fixed arity — `vec3`/`block_pos`
//!   consume 3 tokens, `vec2`/`column_pos`/`rotation` 2, `minecraft:message` and
//!   greedy `brigadier:string` consume the rest of the line; everything else
//!   consumes exactly one (brace/bracket/quote-balanced) token.
//! - `redirect`s are followed (e.g. `if score … matches N` → back to `execute`),
//!   and the `execute … run <cmd>` tail is re-validated from the tree root.
//! - A line is valid iff all tokens are consumed and the final node is
//!   `executable`.
//!
//! What it deliberately does NOT do: verify a `vec3` token is numeric, that an
//! NBT/JSON token is well-formed, or that an item/block id exists (item ids are
//! covered by the DSL registry; the rest is mecha's job). This is enough to catch
//! misspelled commands, wrong argument arity, and bogus subcommand paths.

use std::collections::BTreeMap;

use serde::Deserialize;

/// A node in the Brigadier command tree.
#[derive(Debug, Deserialize)]
struct Node {
    #[serde(default, rename = "type")]
    node_type: String,
    #[serde(default)]
    children: BTreeMap<String, Node>,
    #[serde(default)]
    executable: bool,
    #[serde(default)]
    parser: Option<String>,
    #[serde(default)]
    properties: Option<serde_json::Value>,
    #[serde(default)]
    redirect: Option<Vec<String>>,
}

/// The loaded command tree.
#[derive(Debug)]
pub struct CommandTree {
    root: Node,
}

/// Why a command line failed validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandError {
    /// The offending line.
    pub line: String,
    /// Human-readable reason.
    pub reason: String,
}

impl CommandTree {
    /// Load the vendored 1.21.11 command tree (embedded at compile time).
    pub fn v1_21_11() -> Self {
        let raw = include_str!("../data/commands-1.21.11.json");
        let root: Node = serde_json::from_str(raw).expect("vendored command tree is valid JSON");
        Self { root }
    }

    /// Validate a single command line. `Ok(())` if it is structurally valid, an
    /// [`CommandError`] otherwise. Blank lines and `#` comments are accepted.
    pub fn validate_line(&self, line: &str) -> Result<(), CommandError> {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return Ok(());
        }
        // A leading `$` marks a macro line (`$say [DelveNote] pos=$(x) …`). The
        // structural skeleton must still match the command tree after expansion;
        // strip the `$` and validate the remainder. `$(name)` placeholders sit
        // inside single balanced tokens, so they do not perturb arity.
        let trimmed = trimmed.strip_prefix('$').unwrap_or(trimmed);
        // mcfunction lines carry no leading slash; tolerate one anyway.
        let body = trimmed.strip_prefix('/').unwrap_or(trimmed);
        let tokens = tokenize(body).map_err(|reason| CommandError {
            line: line.to_string(),
            reason,
        })?;
        if self.matches(&self.root, &tokens, 0) {
            Ok(())
        } else {
            Err(format!(
                "does not match the 1.21.11 command tree (root `{}`)",
                tokens.first().map(String::as_str).unwrap_or("")
            ))
        }
        .map_err(|reason| CommandError {
            line: line.to_string(),
            reason,
        })
    }

    /// Validate every line of an mcfunction body, returning all failures.
    pub fn validate_function(&self, body: &str) -> Vec<CommandError> {
        body.lines()
            .filter_map(|l| self.validate_line(l).err())
            .collect()
    }

    /// Backtracking match: does `node` accept `tokens[i..]`? Brigadier tries the
    /// literal first, then each argument branch (order-independent), succeeding
    /// on any complete parse. Handles ambiguity like `teleport @s 5 65 2`
    /// (targets+location) vs `teleport <destination>`.
    fn matches(&self, node: &Node, tokens: &[String], i: usize) -> bool {
        if i >= tokens.len() {
            return node.executable;
        }
        let tok = &tokens[i];
        let kids = self.effective_children(node);
        // 1) exact literal.
        if let Some(child) = kids.get(tok)
            && child.node_type == "literal"
            && self.matches(child, tokens, i + 1)
        {
            return true;
        }
        // 2) any argument branch.
        for child in kids.values().filter(|n| n.node_type == "argument") {
            // Single-entity arity (round-7, live-server proven): a
            // `minecraft:entity` argument whose tree properties say
            // `amount: "single"` REJECTS a multi-entity selector. `/damage @a[…]
            // 40 minecraft:generic` is structurally well-formed but the server
            // refuses to load the whole function ("Only one entity is allowed,
            // but the provided selector allows more than one") — silently
            // deleting every beat in it. The tree carries the fact, so the
            // compiler enforces it rather than leaving it to folklore.
            if single_entity_violation(child, tok) {
                continue;
            }
            match arity(child) {
                Arity::Greedy => {
                    // Consumes the rest of the line.
                    if child.executable {
                        return true;
                    }
                }
                Arity::Fixed(n) => {
                    if i + n <= tokens.len() && self.matches(child, tokens, i + n) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// The children to match the next token against: a node's own children, or
    /// its redirect target's children, or (for the `run` leaf) the tree root.
    fn effective_children<'a>(&'a self, node: &'a Node) -> &'a BTreeMap<String, Node> {
        if !node.children.is_empty() {
            return &node.children;
        }
        if let Some(path) = &node.redirect {
            return &self.resolve(path).children;
        }
        // A non-executable leaf with no children is a root redirect (`execute … run`).
        if !node.executable {
            return &self.root.children;
        }
        &node.children
    }

    /// Resolve a redirect path (e.g. `["execute"]`) from the root.
    fn resolve(&self, path: &[String]) -> &Node {
        let mut node = &self.root;
        for seg in path {
            match node.children.get(seg) {
                Some(child) => node = child,
                None => return &self.root,
            }
        }
        node
    }
}

/// Does `tok` violate a `minecraft:entity` argument's declared single-entity
/// arity? `@p`/`@r`/`@s` and a bare player name select one by definition; `@a`
/// and `@e` select many unless the selector body pins `limit=1`.
fn single_entity_violation(node: &Node, tok: &str) -> bool {
    if node.parser.as_deref() != Some("minecraft:entity") {
        return false;
    }
    let single = node
        .properties
        .as_ref()
        .and_then(|p| p.get("amount"))
        .and_then(|a| a.as_str())
        == Some("single");
    if !single {
        return false;
    }
    let multi = tok.starts_with("@a") || tok.starts_with("@e");
    multi && !tok.contains("limit=1")
}

/// Token arity of an argument parser.
enum Arity {
    Fixed(usize),
    Greedy,
}

fn arity(node: &Node) -> Arity {
    match node.parser.as_deref() {
        Some("minecraft:vec3") | Some("minecraft:block_pos") => Arity::Fixed(3),
        Some("minecraft:vec2") | Some("minecraft:column_pos") | Some("minecraft:rotation") => {
            Arity::Fixed(2)
        }
        Some("minecraft:message") => Arity::Greedy,
        Some("brigadier:string") => {
            let greedy = node
                .properties
                .as_ref()
                .and_then(|p| p.get("type"))
                .and_then(|t| t.as_str())
                == Some("greedy");
            if greedy {
                Arity::Greedy
            } else {
                Arity::Fixed(1)
            }
        }
        _ => Arity::Fixed(1),
    }
}

/// Split a command line into tokens, keeping brace/bracket-balanced and quoted
/// spans together (so `{…}`, `[…]`, and `"…"`/`'…'` count as single tokens).
fn tokenize(s: &str) -> Result<Vec<String>, String> {
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut depth: i32 = 0;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for ch in s.chars() {
        if let Some(q) = quote {
            cur.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == q {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => {
                quote = Some(ch);
                cur.push(ch);
            }
            '{' | '[' | '(' => {
                depth += 1;
                cur.push(ch);
            }
            '}' | ']' | ')' => {
                depth -= 1;
                if depth < 0 {
                    return Err(format!("unbalanced closing `{ch}`"));
                }
                cur.push(ch);
            }
            c if c.is_whitespace() && depth == 0 => {
                if !cur.is_empty() {
                    tokens.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if quote.is_some() {
        return Err("unterminated quote".to_string());
    }
    if depth != 0 {
        return Err("unbalanced brackets".to_string());
    }
    if !cur.is_empty() {
        tokens.push(cur);
    }
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> CommandTree {
        CommandTree::v1_21_11()
    }

    #[test]
    fn accepts_known_commands() {
        let t = tree();
        for line in [
            "scoreboard objectives add dw.sys dummy",
            "scoreboard objectives setdisplay sidebar dw.campaign",
            "scoreboard players enable @a dw.class",
            "scoreboard players set @s dw.o_talk 1",
            "scoreboard players reset @s dw.dlg_keeper",
            "function hello-world:setup",
            "place template hello-world:hello-room 0 64 0",
            "teleport @s 5 65 2",
            "dialog show @s hello-world:class_select",
            "advancement grant @s only hello-world:campaign_complete",
            "advancement revoke @s only hello-world:keeper_interact",
            "fill 4 65 6 5 67 6 minecraft:air replace minecraft:iron_bars",
            "tellraw @s [{\"text\":\"hi\"}]",
            "give @s minecraft:bread 3",
            "execute unless score #init dw.sys matches 1 run function hello-world:setup",
            "execute as @a[scores={dw.class=1}] run function hello-world:apply",
            "execute as @a if score @s dw.o_talk matches 1 unless score @s dw.o_exit matches 1 if entity @s[x=5,y=65,z=8,distance=..2] run function hello-world:done",
            "# a comment",
            "",
        ] {
            assert!(t.validate_line(line).is_ok(), "should accept: {line}");
        }
    }

    #[test]
    fn rejects_bad_commands() {
        let t = tree();
        for line in [
            "scoreboard objectives addd dw.sys dummy", // misspelled literal
            "notacommand foo bar",                     // unknown root
            "place template hello-world:hello-room 0 64", // block_pos short a token
        ] {
            assert!(t.validate_line(line).is_err(), "should reject: {line}");
        }
    }

    /// A `minecraft:entity` argument declared `amount: "single"` rejects a
    /// multi-entity selector (round-7, spec-0018 live-server finding).
    ///
    /// `/damage @a[…] 40 minecraft:generic` parses as a perfectly ordinary
    /// command shape, and the pre-check compiler emitted it happily — but
    /// 1.21.11 refuses to LOAD any function containing it ("Only one entity is
    /// allowed, but the provided selector allows more than one"), which silently
    /// kills every other beat in that function too. The vendored tree carries
    /// the arity, so the compiler enforces it instead of leaving it to folklore.
    #[test]
    fn rejects_a_multi_entity_selector_where_the_tree_demands_one() {
        let t = tree();
        for line in [
            "damage @a[tag=!dw_cutscene] 40 minecraft:generic",
            "damage @e[type=zombie] 4 minecraft:generic",
            "damage @a 6 minecraft:generic",
        ] {
            assert!(
                t.validate_line(line).is_err(),
                "`/damage` takes ONE entity; should reject: {line}"
            );
        }
        // The legal spellings: rebind to a single player, or pin the selector.
        for line in [
            "execute as @a[tag=!dw_cutscene] run damage @s 40 minecraft:generic",
            "damage @s[tag=!dw_cutscene] 40 minecraft:generic",
            "damage @a[tag=dw_t_dmg,limit=1] 6 minecraft:generic",
            "damage @p 6 minecraft:generic",
        ] {
            assert!(t.validate_line(line).is_ok(), "should accept: {line}");
        }
        // Multi-entity arguments elsewhere are untouched.
        for line in [
            "kill @e[tag=dw_actor_giant]",
            "effect give @a[tag=x] minecraft:night_vision 12 0 true",
            "tag @e[tag=dw_tmp] remove dw_tmp",
        ] {
            assert!(t.validate_line(line).is_ok(), "should accept: {line}");
        }
    }

    #[test]
    fn tokenize_keeps_braces_and_quotes() {
        let toks =
            tokenize("give @s minecraft:iron_sword[custom_name={\"text\":\"A B\"}] 1").unwrap();
        assert_eq!(toks.len(), 4);
        assert_eq!(toks[3], "1");
    }
}
