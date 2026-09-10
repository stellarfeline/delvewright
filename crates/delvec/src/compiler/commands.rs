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
//!
//! ## Value-level exceptions: bounds the HANDLER keeps, not the parser
//!
//! There is a class of refusal this tree can never express. Brigadier describes
//! how a command **parses**; a bound the command's own handler checks *after* the
//! parse has succeeded appears nowhere in it. So a line can be structurally
//! perfect, pass this validator, and be refused by the running server — and
//! because a refused command inside a function is not a parse failure, the rest of
//! the function still runs and nothing anywhere reads the reply. Each such bound
//! that has cost something is written down here, one function apiece:
//!
//! - **An SNBT integer literal whose suffix cannot hold it.** `text_opacity:255b`
//!   is structurally perfect and unparseable — NBT bytes are signed, so the server
//!   answers "Failed to parse number: Value out of range" and drops the entire
//!   function. The check is cheap, needs no NBT grammar, and cannot mistake a
//!   string for a number because quoted spans are skipped; see
//!   [`snbt_range_error`].
//! - **A `forceload` area over [`FORCELOAD_MAX_CHUNKS`].** `forceload add -76 -76
//!   176 176` is a perfectly good `forceload add <column_pos> <column_pos>`, and
//!   the pinned server answers `Too many chunks in the specified area (maximum
//!   256, but specified 289)` and marks **nothing**. The world then boots with the
//!   placement's chunks unloaded, `place template` no-ops for every piece outside
//!   the chunks something else happens to load, and the delve never finishes
//!   setting itself up. See [`forceload_area_error`] for the refusal and
//!   [`forceload_add_lines`] for the emission side that cannot produce one.

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
        let raw = include_str!("../../data/commands-1.21.11.json");
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
        if let Some(reason) = snbt_range_error(body) {
            return Err(CommandError {
                line: line.to_string(),
                reason,
            });
        }
        if let Some(reason) = forceload_area_error(&tokens) {
            return Err(CommandError {
                line: line.to_string(),
                reason,
            });
        }
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

/// An SNBT integer literal whose suffix cannot hold its value, e.g. the
/// `text_opacity:255b` that makes 1.21.11 refuse a whole function.
/// NBT bytes and shorts are **signed**: `b` is -128..=127 and `s` is
/// -32768..=32767, so "fully opaque" is `-1b`, never `255b`.
///
/// Deliberately narrow, so it cannot mistake text for a number:
/// - quoted spans (`"…"`, `'…'`) are skipped entirely — a literal `255b` inside
///   a `text:'{"text":"…"}'` component is prose, not a value;
/// - the number must sit in an SNBT **value position**: the first non-space
///   character before it is one of `:,[{;=`;
/// - and it must END there: the character after the suffix may not continue an
///   identifier (so `minecraft:music_disc_11`, `room2b`, `dw.o_5b` are not
///   numbers and are never examined).
///
/// Everything outside that shape is left to mecha and the server, exactly as the
/// rest of this validator's value-blindness is — including a bare
/// `… set value 200b`, where the number stands alone as its own token and is
/// therefore indistinguishable from a word in a `/say`. Narrow and sound beats
/// wide and guessing: the shape that actually ships NBT is `key:value`.
fn snbt_range_error(s: &str) -> Option<String> {
    let b: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    while i < b.len() {
        let ch = b[i];
        if let Some(q) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        if ch == '"' || ch == '\'' {
            quote = Some(ch);
            i += 1;
            continue;
        }
        if !ch.is_ascii_digit() && ch != '-' {
            i += 1;
            continue;
        }
        // Candidate number start: must sit in an SNBT value position.
        let mut before = i;
        while before > 0 && b[before - 1].is_whitespace() {
            before -= 1;
        }
        let opens = before > 0 && matches!(b[before - 1], ':' | ',' | '[' | '{' | ';' | '=');
        let mut j = i;
        if b[j] == '-' {
            j += 1;
        }
        let digits_start = j;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if j == digits_start || !opens {
            // Not a number here, or not a value position: skip the whole run so a
            // digit inside an identifier is never re-examined as a fresh start.
            i = j.max(i + 1);
            continue;
        }
        let suffix = b.get(j).copied();
        let ends = b
            .get(j + 1)
            .is_none_or(|c| !c.is_ascii_alphanumeric() && *c != '_' && *c != '.');
        if ends && let Some(sfx) = suffix {
            let range = match sfx {
                'b' | 'B' => Some((-128i64, 127i64, "byte")),
                's' | 'S' => Some((-32768i64, 32767i64, "short")),
                _ => None,
            };
            if let Some((lo, hi, name)) = range {
                let text: String = b[i..j].iter().collect();
                let value: i64 = text.parse().unwrap_or(i64::MAX);
                if value < lo || value > hi {
                    return Some(format!(
                        "SNBT `{text}{sfx}` is out of range for an NBT {name} ({lo}..={hi}) — \
                         1.21.11 answers \"Failed to parse number: Value out of range\" and \
                         refuses to load the whole function"
                    ));
                }
            }
        }
        i = j + usize::from(suffix.is_some());
    }
    None
}

/// The most chunks one `forceload` command may name, on the pinned server.
///
/// Read off the server itself rather than a wiki: `forceload add 992 992 1247
/// 1247` (16 × 16) is answered `Marked 256 chunks …`, and one chunk more on
/// either axis — including the 1 × 257 strip, so the bound is on the **area**
/// and not on a side — is answered `Too many chunks in the specified area
/// (maximum 256, but specified N)`. `forceload remove` over a rectangle is
/// judged by the same handler and refuses identically; `forceload remove <x>
/// <z>` names one chunk and can never reach it.
pub const FORCELOAD_MAX_CHUNKS: i64 = 256;

/// The longest side a split tile may have. 16 × 16 is exactly
/// [`FORCELOAD_MAX_CHUNKS`], so a tile capped on both axes is always inside the
/// ceiling however the two sides fall out.
const FORCELOAD_TILE_CHUNKS: i64 = 16;

/// The `forceload add` line(s) that mark every chunk touched by the world-block
/// rectangle `(x1, z1)..(x2, z2)` — **the only way this compiler emits one.**
///
/// A single command cannot name more than [`FORCELOAD_MAX_CHUNKS`] chunks, and
/// the span is not something a creator writes: it is derived, from a piece's own
/// bounding box or from the ring a horizon grows around one. A 101 × 101 piece
/// under a `valley` horizon derives `-76 -76 176 176` — 17 × 17 = 289 chunks —
/// which the server refuses whole. Refusing at compile time instead would hand
/// the creator a legal piece under a legal horizon and no act that clears it, so
/// the span is **split** rather than refused, and the creator never has to know:
/// vanilla caps what one command may name, never how many chunks a world may
/// hold.
///
/// A rectangle already inside the ceiling emits exactly the line it always did,
/// coordinates and all, so every campaign and every baseline that never reached
/// the ceiling is byte-identical. Only what the server refuses changes shape.
///
/// The split is a grid: each axis is cut into `ceil(len / 16)` runs of as near
/// equal length as they divide, so no tile exceeds 16 chunks on a side and none
/// is a one-chunk sliver beside a full one. Tiles are emitted x-major, each
/// named by the block coordinates of its own chunk range — the same chunk set the
/// caller asked for, in `ceil(w/16) * ceil(h/16)` commands.
pub fn forceload_add_lines(x1: i32, z1: i32, x2: i32, z2: i32) -> Vec<String> {
    let (cx0, cx1) = chunk_bounds(x1, x2);
    let (cz0, cz1) = chunk_bounds(z1, z2);
    let w = i64::from(cx1 - cx0) + 1;
    let h = i64::from(cz1 - cz0) + 1;
    if w * h <= FORCELOAD_MAX_CHUNKS {
        return vec![format!("forceload add {x1} {z1} {x2} {z2}")];
    }
    let mut out = Vec::new();
    for (ax0, ax1) in chunk_runs(cx0, cx1) {
        for (az0, az1) in chunk_runs(cz0, cz1) {
            out.push(format!(
                "forceload add {} {} {} {}",
                ax0 * 16,
                az0 * 16,
                ax1 * 16 + 15,
                az1 * 16 + 15
            ));
        }
    }
    out
}

/// The inclusive chunk range two world-block coordinates cover, either order.
fn chunk_bounds(a: i32, b: i32) -> (i32, i32) {
    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
    (lo.div_euclid(16), hi.div_euclid(16))
}

/// Cut the inclusive chunk range `lo..=hi` into the fewest runs of at most
/// [`FORCELOAD_TILE_CHUNKS`] chunks, as near equal in length as they divide. The
/// remainder is spread over the leading runs, so the longest and the shortest run
/// differ by at most one chunk and the result is a pure function of the range.
fn chunk_runs(lo: i32, hi: i32) -> Vec<(i32, i32)> {
    let len = i64::from(hi - lo) + 1;
    // `len` is a chunk count and therefore >= 1, so this is a plain ceiling
    // divide (signed `div_ceil` is not stable on the pinned toolchain).
    let n = (len + FORCELOAD_TILE_CHUNKS - 1) / FORCELOAD_TILE_CHUNKS;
    let base = len / n;
    let extra = len % n;
    let mut runs = Vec::with_capacity(n as usize);
    let mut start = i64::from(lo);
    for k in 0..n {
        let this = base + i64::from(k < extra);
        runs.push((start as i32, (start + this - 1) as i32));
        start += this;
    }
    runs
}

/// A `forceload` whose rectangle covers more chunks than one command may name.
///
/// The tree cannot say this: `<column_pos>` accepts any pair of numbers, and the
/// bound lives in the command's handler. So it is asked here, of every emitted
/// line, from every emission site — the campaign compiler and the gallery
/// admission pack alike. [`forceload_add_lines`] is what keeps the answer `None`;
/// this is what makes a second emitter that forgets it impossible to ship.
///
/// Judged only when all four coordinates are plain integers. A relative or local
/// coordinate (`~`, `^`) resolves against an execution position this compiler
/// does not know, so it is left to the server exactly as the rest of this
/// validator's value-blindness is; the compiler emits none.
fn forceload_area_error(tokens: &[String]) -> Option<String> {
    for i in 0..tokens.len() {
        if tokens[i] != "forceload" {
            continue;
        }
        if !matches!(
            tokens.get(i + 1).map(String::as_str),
            Some("add" | "remove")
        ) {
            continue;
        }
        // Four coordinates is `<from> <to>`, a rectangle. Two is `<from>` alone,
        // one chunk, which can never reach the ceiling.
        let Some(slots) = tokens.get(i + 2..i + 6) else {
            continue;
        };
        let coords: Vec<i32> = slots.iter().filter_map(|t| t.parse::<i32>().ok()).collect();
        if coords.len() != 4 {
            continue;
        }
        let (cx0, cx1) = chunk_bounds(coords[0], coords[2]);
        let (cz0, cz1) = chunk_bounds(coords[1], coords[3]);
        let chunks = (i64::from(cx1 - cx0) + 1) * (i64::from(cz1 - cz0) + 1);
        if chunks > FORCELOAD_MAX_CHUNKS {
            let verb = &tokens[i + 1];
            return Some(format!(
                "`forceload {verb}` names {chunks} chunks ([{cx0}, {cz0}]..[{cx1}, {cz1}]) and one \
                 command may name at most {FORCELOAD_MAX_CHUNKS} — 1.21.11 answers \"Too many \
                 chunks in the specified area (maximum {FORCELOAD_MAX_CHUNKS}, but specified \
                 {chunks})\", marks nothing at all, and runs the rest of the function anyway. \
                 Emit the span through `commands::forceload_add_lines`, which splits it."
            ));
        }
    }
    None
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

    /// An SNBT byte/short literal that overflows its suffix.
    ///
    /// `delvec prefab`'s gallery summoned its labels with `text_opacity:255b`. The
    /// command is structurally flawless, so every structural check passed — and
    /// 1.21.11 dropped `admit:finish` in its entirety ("Failed to parse number:
    /// Value out of range. Value:\"255\""), taking the spawn platform, the
    /// worldspawn and every label with it. Nothing read the server's answer, so
    /// nothing failed until someone looked at an empty world.
    #[test]
    fn rejects_an_snbt_integer_its_suffix_cannot_hold() {
        let t = tree();
        for line in [
            "summon minecraft:text_display 0 64 0 {text:'{\"text\":\"x\"}',text_opacity:255b}",
            "summon minecraft:armor_stand 0 64 0 {Invisible:200b}",
            "summon minecraft:zombie 0 64 0 {Health:40000s}",
            "data modify entity @s Tags set value [{a:-129b}]",
        ] {
            assert!(t.validate_line(line).is_err(), "should reject: {line}");
        }
        // In range, and the shapes that merely LOOK like numbers: a suffixed
        // digit run inside an identifier, and one inside a quoted string.
        for line in [
            "summon minecraft:text_display 0 64 0 {text:'{\"text\":\"x\"}',text_opacity:-1b}",
            "summon minecraft:armor_stand 0 64 0 {Invisible:1b,NoGravity:1b}",
            "summon minecraft:zombie 0 64 0 {Health:20000s}",
            "give @s minecraft:music_disc_11 1",
            "scoreboard players set #t admit.sys 0",
            "say the wall is 255b thick",
            "tellraw @s [{\"text\":\"255b\"}]",
            "execute if entity @s[x=0,dx=255,y=64,dy=5,z=0,dz=255] run say in",
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

    /// Chunks a `forceload` rectangle covers, the way the server counts them.
    fn chunk_count(x1: i32, z1: i32, x2: i32, z2: i32) -> i64 {
        let (cx0, cx1) = chunk_bounds(x1, x2);
        let (cz0, cz1) = chunk_bounds(z1, z2);
        (i64::from(cx1 - cx0) + 1) * (i64::from(cz1 - cz0) + 1)
    }

    /// Read the four coordinates back out of an emitted line.
    fn coords_of(line: &str) -> (i32, i32, i32, i32) {
        let t: Vec<i32> = line
            .split_whitespace()
            .skip(2)
            .map(|s| s.parse().expect("emitted coordinate is an integer"))
            .collect();
        assert_eq!(t.len(), 4, "emitted a rectangle: {line}");
        (t[0], t[1], t[2], t[3])
    }

    /// The bound the pinned server keeps, read off the server itself: `forceload
    /// add 992 992 1247 1247` is answered `Marked 256 chunks …` and `forceload
    /// add 4000 4000 4015 8111` — a 1 × 257 strip — is answered `Too many chunks
    /// in the specified area (maximum 256, but specified 257)`. So the ceiling is
    /// on the area, both verbs are judged by it, and 256 itself is legal.
    #[test]
    fn refuses_a_forceload_naming_more_chunks_than_one_command_may() {
        let t = tree();
        // 16 x 16, chunk-aligned: exactly the ceiling, and the server marks it.
        assert_eq!(chunk_count(992, 992, 1247, 1247), FORCELOAD_MAX_CHUNKS);
        assert!(t.validate_line("forceload add 992 992 1247 1247").is_ok());
        for line in [
            // The castle's own span, under a `valley` horizon: 17 x 17 = 289.
            "forceload add -76 -76 176 176",
            // 1 x 257 — the ceiling is on the AREA, so a strip reaches it too.
            "forceload add 4000 4000 4015 8111",
            // `remove` over a rectangle goes to the same handler.
            "forceload remove -76 -76 176 176",
            // …and reached through an `execute … run` tail, which the validator
            // re-enters from the tree root.
            "execute if score #init dw.sys matches 1 run forceload add -76 -76 176 176",
        ] {
            let err = t
                .validate_line(line)
                .expect_err("the server refuses this and marks nothing: {line}");
            assert!(
                err.reason.contains("Too many chunks in the specified area"),
                "the refusal quotes what the server says: {}",
                err.reason
            );
        }
        // One chunk named by `<from>` alone, and a relative coordinate this
        // compiler cannot resolve: neither is judged.
        for line in ["forceload remove 58 58", "forceload add ~ ~ ~100 ~100"] {
            assert!(t.validate_line(line).is_ok(), "should accept: {line}");
        }
    }

    /// A span inside the ceiling emits the one line it always did, coordinates
    /// untouched — so nothing that never reached the ceiling moves a byte.
    #[test]
    fn a_span_inside_the_ceiling_is_emitted_whole() {
        assert_eq!(
            forceload_add_lines(0, 0, 100, 100),
            vec!["forceload add 0 0 100 100".to_string()]
        );
        assert_eq!(
            forceload_add_lines(-2, -2, 2, 2),
            vec!["forceload add -2 -2 2 2".to_string()]
        );
    }

    /// The castle's span, split. Every tile is inside the ceiling, and together
    /// they mark exactly the chunks the one refused command asked for.
    #[test]
    fn a_span_over_the_ceiling_is_split_into_lines_the_server_accepts() {
        let t = tree();
        let lines = forceload_add_lines(-76, -76, 176, 176);
        assert_eq!(
            lines,
            vec![
                "forceload add -80 -80 63 63".to_string(),
                "forceload add -80 64 63 191".to_string(),
                "forceload add 64 -80 191 63".to_string(),
                "forceload add 64 64 191 191".to_string(),
            ],
            "17 x 17 chunks cut into 9|8 by 9|8 — no sliver beside a full tile"
        );
        let mut marked: std::collections::BTreeSet<(i32, i32)> = Default::default();
        for line in &lines {
            assert!(
                t.validate_line(line).is_ok(),
                "each tile is emittable: {line}"
            );
            let (x1, z1, x2, z2) = coords_of(line);
            assert!(
                chunk_count(x1, z1, x2, z2) <= FORCELOAD_MAX_CHUNKS,
                "each tile is inside the ceiling: {line}"
            );
            let (cx0, cx1) = chunk_bounds(x1, x2);
            let (cz0, cz1) = chunk_bounds(z1, z2);
            for cx in cx0..=cx1 {
                for cz in cz0..=cz1 {
                    assert!(marked.insert((cx, cz)), "tiles do not overlap: {line}");
                }
            }
        }
        // The chunk set is the one the single command named: [-5,-5]..[11,11].
        let want: std::collections::BTreeSet<(i32, i32)> = (-5..=11)
            .flat_map(|cx| (-5..=11).map(move |cz| (cx, cz)))
            .collect();
        assert_eq!(marked, want, "289 chunks, exactly the span asked for");
    }

    /// Whatever the shape of the rectangle, the split covers it exactly and no
    /// tile can be refused. Swept over sides that straddle every interesting
    /// boundary — under the ceiling, on it, a strip, and far past it.
    #[test]
    fn every_split_covers_its_span_and_stays_inside_the_ceiling() {
        let t = tree();
        let sides = [1i32, 15, 16, 17, 255, 256, 257, 1000, 4095, 4096, 9999];
        let mut swept = 0usize;
        let mut split = 0usize;
        for w in sides {
            for h in sides {
                // Anchored off a chunk boundary on purpose: a span that starts
                // mid-chunk covers one more chunk than its width suggests, which
                // is exactly how `1000 1000 1255 1255` reaches 289.
                let (x1, z1) = (-7, 3);
                let (x2, z2) = (x1 + w - 1, z1 + h - 1);
                let lines = forceload_add_lines(x1, z1, x2, z2);
                swept += 1;
                if lines.len() > 1 {
                    split += 1;
                }
                let mut marked: std::collections::BTreeSet<(i32, i32)> = Default::default();
                for line in &lines {
                    assert!(t.validate_line(line).is_ok(), "emittable: {line}");
                    let (a, b, c, d) = coords_of(line);
                    assert!(
                        chunk_count(a, b, c, d) <= FORCELOAD_MAX_CHUNKS,
                        "inside the ceiling: {line}"
                    );
                    let (cx0, cx1) = chunk_bounds(a, c);
                    let (cz0, cz1) = chunk_bounds(b, d);
                    for cx in cx0..=cx1 {
                        for cz in cz0..=cz1 {
                            marked.insert((cx, cz));
                        }
                    }
                }
                let (cx0, cx1) = chunk_bounds(x1, x2);
                let (cz0, cz1) = chunk_bounds(z1, z2);
                let want: std::collections::BTreeSet<(i32, i32)> = (cx0..=cx1)
                    .flat_map(|cx| (cz0..=cz1).map(move |cz| (cx, cz)))
                    .collect();
                assert_eq!(
                    marked, want,
                    "{w} x {h} blocks: the span is covered exactly"
                );
            }
        }
        assert_eq!(swept, 121, "every pair of sides was swept");
        assert!(
            split > 0 && split < swept,
            "the sweep reaches both sides of the ceiling ({split} split of {swept})"
        );
    }
}
