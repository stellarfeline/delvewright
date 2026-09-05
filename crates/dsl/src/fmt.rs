//! `delvec fmt` — the canonical form for authored Delvewright JSON.
//!
//! # Why this exists
//!
//! Authored JSON was formatted by whatever wrote it last. A three-key insertion
//! into `nobodys-cave-island/l10n/zh-cn.json` produced a **103-insertion /
//! 100-deletion** diff, because the file was not canonically ordered and the
//! writing tool's `sort_keys` re-laid the whole thing out. A canonical order
//! makes an insertion a one-line insertion and makes two authors editing
//! different keys a non-conflict.
//!
//! # The hard constraint
//!
//! **Only OBJECT KEYS may be sorted.** Array order is semantic everywhere in
//! this DSL — `quests[]`, `objectives[]`, `effects[]`, `options[]`, `steps[]`
//! are ordered, and reordering one changes the game. So a code path that could
//! reorder an array is a correctness bug, not a style bug, and this module does
//! not merely avoid one — it *proves* the absence on every file it writes:
//! [`format_text`] re-parses its own output and runs [`equivalent`], which
//! compares arrays **index-wise** and objects as key→value maps. A formatter
//! that sorted an array would fail its own check and refuse to write, rather
//! than shipping a silently reordered campaign. (`DW0772`.)
//!
//! # The canonical form
//!
//! Every choice below is argued from the two goals — *minimal diff on
//! insertion* and *no semantic change ever* — never from taste.
//!
//! | rule | why |
//! |---|---|
//! | object keys sorted by Unicode scalar value | the whole point: an inserted key lands in exactly one place, so the diff is one line. Byte order of UTF-8 == code-point order, so Rust's `str` `Ord` and Python's `sorted()` agree — the existing Python authoring tools already emit this order. |
//! | two-space indent, one value per line | the motivating file and every `tools/*.py` writer already use `indent=2` (and it is `serde_json`'s pretty default), so the one-time normalization is near-zero on the largest authored files. One value per line makes an inserted array element a whole-line insertion instead of a rewrite of a long line. |
//! | non-ASCII written raw, never `\uXXXX` | the campaigns are half Chinese. Escaping would triple every sidecar and make its diffs unreadable — a direct defeat of the task's own motivation. |
//! | control characters escaped, shortest form (`\n`, `\t`, `\b`, `\f`, `\r`, else `\u00xx`) | required by JSON; the shortest form is the one every other writer here emits, so it is already the fixed point. |
//! | **number literals preserved byte-for-byte** | the only rule that is *not* about diffs. Re-rendering a number through `f64` loses integers above 2^53 and can move the last digit of a decimal — a silent semantic change, which the hard constraint forbids outright. No author writes numbers in a way that churns a diff, so there is nothing to gain against a real risk. |
//! | exactly one trailing newline | POSIX text; and without it, appending anything rewrites the last line. |
//! | duplicate object keys refused (`DW0771`) | JSON's own grammar allows them and `serde_json` silently keeps the last, so today a duplicate key is data the compiler discards without a word. Formatting it would make that loss permanent and invisible, so `fmt` refuses instead. |
//!
//! Deliberately NOT canonicalized: number literals (above), string Unicode
//! normalization (NFC vs NFD is the author's text, not the formatter's), and
//! anything schema-shaped. The formatter knows the JSON grammar and nothing
//! about the DSL — it must format an l10n sidecar, a stage document, a prefab
//! metadata card and whatever stage 8 turns out to be, with no per-schema list
//! to keep in step.
//!
//! # Determinism (ADR-0006)
//!
//! Output is a pure function of input bytes: no wall clock, no RNG, no
//! `HashMap` iteration (keys are sorted explicitly), no absolute paths in
//! output. Directory discovery sorts entries rather than trusting `read_dir`.

use crate::diagnostic::{DwCode, ExitTier};
use std::cmp::Ordering;
use std::path::{Path, PathBuf};

/// Indent width of the canonical form. Not configurable: a formatter with
/// options has as many canonical forms as it has flag combinations, and the
/// point of this one is that there is exactly one.
pub const INDENT: usize = 2;

/// A JSON document, parsed for formatting.
///
/// Deliberately **not** `serde_json::Value`: that type parses a number into
/// `i64`/`u64`/`f64` and loses the literal the author wrote, which is precisely
/// the byte the "no semantic change ever" constraint says must survive. It also
/// silently accepts duplicate object keys.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Null,
    Bool(bool),
    /// The number literal, **verbatim** — never re-rendered.
    Number(String),
    /// A string with all escapes decoded.
    Str(String),
    /// Ordered, and it stays ordered. See the module docs.
    Array(Vec<Node>),
    /// Key → value in the order the file had them; sorted only when written.
    Object(Vec<(String, Node)>),
}

/// A parse failure, located for an author.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    /// `DW0771` for a duplicate object key, `DW0770` for a syntax error.
    pub code: DwCode,
    /// 1-based line.
    pub line: usize,
    /// 1-based column, in characters.
    pub col: usize,
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.col, self.message)
    }
}

/// `DW0770`: authored JSON that is not valid JSON (`delvec fmt`).
pub const DW_FMT_PARSE: DwCode = DwCode::new("DW0770", ExitTier::Build);
/// `DW0771`: a duplicate object key in authored JSON (`delvec fmt`).
pub const DW_FMT_DUPLICATE_KEY: DwCode = DwCode::new("DW0771", ExitTier::Build);
/// `DW0772`: the formatter's own output is not equivalent to its input —
/// internal error, nothing is written (`delvec fmt`).
pub const DW_FMT_NOT_EQUIVALENT: DwCode = DwCode::new("DW0772", ExitTier::Build);
/// `DW0773`: a file is not in canonical form (`delvec fmt --check`).
pub const DW_FMT_UNFORMATTED: DwCode = DwCode::new("DW0773", ExitTier::Build);
/// `DW0774`: `delvec fmt` matched no files — a formatter or a check that binds
/// to nothing is vacuous, not a pass (CLAUDE.md).
pub const DW_FMT_NO_BINDING: DwCode = DwCode::new("DW0774", ExitTier::Build);

// ---------------------------------------------------------------- parsing --

struct Parser<'a> {
    src: &'a [u8],
    pos: usize,
    line: usize,
    col: usize,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        Parser {
            src: src.as_bytes(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    fn err(&self, code: DwCode, message: impl Into<String>) -> ParseError {
        ParseError {
            code,
            line: self.line,
            col: self.col,
            message: message.into(),
        }
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    /// Advance one byte, keeping line/col honest. Column counts characters, so
    /// UTF-8 continuation bytes (`10xxxxxx`) do not advance it.
    fn bump(&mut self) -> Option<u8> {
        let b = self.src.get(self.pos).copied()?;
        self.pos += 1;
        if b == b'\n' {
            self.line += 1;
            self.col = 1;
        } else if b & 0xC0 != 0x80 {
            self.col += 1;
        }
        Some(b)
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.bump();
        }
    }

    fn expect(&mut self, want: u8) -> Result<(), ParseError> {
        match self.peek() {
            Some(b) if b == want => {
                self.bump();
                Ok(())
            }
            Some(b) => Err(self.err(
                DW_FMT_PARSE,
                format!(
                    "expected `{}`, found `{}`",
                    want as char,
                    escape_for_message(b)
                ),
            )),
            None => Err(self.err(
                DW_FMT_PARSE,
                format!("expected `{}`, found end of file", want as char),
            )),
        }
    }

    fn literal(&mut self, word: &str, node: Node) -> Result<Node, ParseError> {
        if self.src[self.pos..].starts_with(word.as_bytes()) {
            for _ in 0..word.len() {
                self.bump();
            }
            Ok(node)
        } else {
            Err(self.err(DW_FMT_PARSE, format!("expected `{word}`")))
        }
    }

    fn value(&mut self) -> Result<Node, ParseError> {
        self.skip_ws();
        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => Ok(Node::Str(self.string()?)),
            Some(b't') => self.literal("true", Node::Bool(true)),
            Some(b'f') => self.literal("false", Node::Bool(false)),
            Some(b'n') => self.literal("null", Node::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(b) => Err(self.err(
                DW_FMT_PARSE,
                format!("unexpected `{}`", escape_for_message(b)),
            )),
            None => Err(self.err(DW_FMT_PARSE, "unexpected end of file")),
        }
    }

    fn object(&mut self) -> Result<Node, ParseError> {
        self.expect(b'{')?;
        let mut entries: Vec<(String, Node)> = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.bump();
            return Ok(Node::Object(entries));
        }
        loop {
            self.skip_ws();
            let key_line = self.line;
            let key_col = self.col;
            let key = self.string()?;
            // A duplicate key is data the compiler already discards silently
            // (serde_json keeps the last). Formatting would erase the evidence,
            // so refuse and say which key and where.
            if entries.iter().any(|(k, _)| *k == key) {
                return Err(ParseError {
                    code: DW_FMT_DUPLICATE_KEY,
                    line: key_line,
                    col: key_col,
                    message: format!(
                        "duplicate object key `{key}`. JSON allows it and the compiler's \
                         parser silently keeps the LAST one, so one of these two values is \
                         already being discarded without a word. Formatting would make that \
                         loss permanent and invisible, so `delvec fmt` refuses: delete or \
                         rename whichever occurrence is wrong."
                    ),
                });
            }
            self.skip_ws();
            self.expect(b':')?;
            let value = self.value()?;
            entries.push((key, value));
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.bump();
                    self.skip_ws();
                    if self.peek() == Some(b'}') {
                        return Err(self.err(
                            DW_FMT_PARSE,
                            "trailing comma before `}` (JSON has no trailing commas)",
                        ));
                    }
                }
                Some(b'}') => {
                    self.bump();
                    return Ok(Node::Object(entries));
                }
                Some(b) => {
                    return Err(self.err(
                        DW_FMT_PARSE,
                        format!("expected `,` or `}}`, found `{}`", escape_for_message(b)),
                    ));
                }
                None => {
                    return Err(self.err(DW_FMT_PARSE, "unterminated object"));
                }
            }
        }
    }

    fn array(&mut self) -> Result<Node, ParseError> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.bump();
            return Ok(Node::Array(items));
        }
        loop {
            items.push(self.value()?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.bump();
                    self.skip_ws();
                    if self.peek() == Some(b']') {
                        return Err(self.err(
                            DW_FMT_PARSE,
                            "trailing comma before `]` (JSON has no trailing commas)",
                        ));
                    }
                }
                Some(b']') => {
                    self.bump();
                    return Ok(Node::Array(items));
                }
                Some(b) => {
                    return Err(self.err(
                        DW_FMT_PARSE,
                        format!("expected `,` or `]`, found `{}`", escape_for_message(b)),
                    ));
                }
                None => {
                    return Err(self.err(DW_FMT_PARSE, "unterminated array"));
                }
            }
        }
    }

    /// The JSON number grammar. The raw literal is captured and kept verbatim —
    /// see the module docs for why it is never re-rendered.
    fn number(&mut self) -> Result<Node, ParseError> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.bump();
        }
        match self.peek() {
            Some(b'0') => {
                self.bump();
            }
            Some(b'1'..=b'9') => {
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.bump();
                }
            }
            _ => return Err(self.err(DW_FMT_PARSE, "expected a digit after `-`")),
        }
        if self.peek() == Some(b'.') {
            self.bump();
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.err(DW_FMT_PARSE, "expected a digit after `.`"));
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.bump();
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.bump();
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.bump();
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.err(DW_FMT_PARSE, "expected a digit in the exponent"));
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.bump();
            }
        }
        // Every byte consumed above is ASCII, so the slice is valid UTF-8.
        let raw = std::str::from_utf8(&self.src[start..self.pos])
            .expect("a JSON number literal is ASCII")
            .to_string();
        Ok(Node::Number(raw))
    }

    fn string(&mut self) -> Result<String, ParseError> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            let Some(b) = self.peek() else {
                return Err(self.err(DW_FMT_PARSE, "unterminated string"));
            };
            match b {
                b'"' => {
                    self.bump();
                    return Ok(out);
                }
                b'\\' => {
                    self.bump();
                    let Some(esc) = self.bump() else {
                        return Err(self.err(DW_FMT_PARSE, "unterminated escape"));
                    };
                    match esc {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{0008}'),
                        b'f' => out.push('\u{000c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hi = self.hex4()?;
                            let ch = if (0xD800..0xDC00).contains(&hi) {
                                // A high surrogate must be followed by `\uDCxx`.
                                if self.peek() != Some(b'\\') {
                                    return Err(self.err(
                                        DW_FMT_PARSE,
                                        "lone high surrogate: `\\uD800`–`\\uDBFF` must be \
                                         followed by a low surrogate escape",
                                    ));
                                }
                                self.bump();
                                if self.peek() != Some(b'u') {
                                    return Err(self.err(
                                        DW_FMT_PARSE,
                                        "lone high surrogate: expected `\\u` low surrogate",
                                    ));
                                }
                                self.bump();
                                let lo = self.hex4()?;
                                if !(0xDC00..0xE000).contains(&lo) {
                                    return Err(self.err(
                                        DW_FMT_PARSE,
                                        "high surrogate not followed by a low surrogate",
                                    ));
                                }
                                let cp = 0x1_0000u32
                                    + ((hi as u32 - 0xD800) << 10)
                                    + (lo as u32 - 0xDC00);
                                char::from_u32(cp)
                                    .ok_or_else(|| self.err(DW_FMT_PARSE, "invalid code point"))?
                            } else if (0xDC00..0xE000).contains(&hi) {
                                return Err(self.err(
                                    DW_FMT_PARSE,
                                    "lone low surrogate escape (`\\uDC00`–`\\uDFFF`)",
                                ));
                            } else {
                                char::from_u32(hi as u32)
                                    .ok_or_else(|| self.err(DW_FMT_PARSE, "invalid code point"))?
                            };
                            out.push(ch);
                        }
                        other => {
                            return Err(self.err(
                                DW_FMT_PARSE,
                                format!("invalid escape `\\{}`", escape_for_message(other)),
                            ));
                        }
                    }
                }
                0x00..=0x1F => {
                    return Err(self.err(
                        DW_FMT_PARSE,
                        format!("raw control character U+{b:04X} in a string; escape it"),
                    ));
                }
                _ => {
                    // Copy the whole UTF-8 sequence byte by byte; the source was
                    // a `&str`, so it is well-formed by construction.
                    let start = self.pos;
                    self.bump();
                    while matches!(self.peek(), Some(c) if c & 0xC0 == 0x80) {
                        self.bump();
                    }
                    out.push_str(
                        std::str::from_utf8(&self.src[start..self.pos])
                            .expect("the source was a &str"),
                    );
                }
            }
        }
    }

    fn hex4(&mut self) -> Result<u16, ParseError> {
        let mut v: u16 = 0;
        for _ in 0..4 {
            let Some(b) = self.bump() else {
                return Err(self.err(DW_FMT_PARSE, "truncated `\\u` escape"));
            };
            let d = match b {
                b'0'..=b'9' => b - b'0',
                b'a'..=b'f' => b - b'a' + 10,
                b'A'..=b'F' => b - b'A' + 10,
                _ => {
                    return Err(self.err(
                        DW_FMT_PARSE,
                        format!(
                            "`\\u` escape needs 4 hex digits, found `{}`",
                            escape_for_message(b)
                        ),
                    ));
                }
            };
            v = v * 16 + d as u16;
        }
        Ok(v)
    }
}

fn escape_for_message(b: u8) -> String {
    if (0x20..0x7F).contains(&b) {
        (b as char).to_string()
    } else {
        format!("\\x{b:02x}")
    }
}

/// Parse authored JSON, keeping number literals verbatim and refusing duplicate
/// object keys.
pub fn parse(text: &str) -> Result<Node, ParseError> {
    let mut p = Parser::new(text);
    // A UTF-8 BOM is invisible in an editor and makes every JSON parser in the
    // pipeline fail somewhere less obvious than here.
    if p.src.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return Err(p.err(
            DW_FMT_PARSE,
            "file starts with a UTF-8 BOM; Delvewright JSON is plain UTF-8 with no BOM",
        ));
    }
    let node = p.value()?;
    p.skip_ws();
    if p.pos != p.src.len() {
        return Err(p.err(DW_FMT_PARSE, "trailing content after the top-level value"));
    }
    Ok(node)
}

// ---------------------------------------------------------------- writing --

/// Render a node in canonical form, including the single trailing newline.
pub fn canonical(node: &Node) -> String {
    let mut out = String::new();
    write_node(node, 0, &mut out);
    out.push('\n');
    out
}

fn write_node(node: &Node, depth: usize, out: &mut String) {
    match node {
        Node::Null => out.push_str("null"),
        Node::Bool(true) => out.push_str("true"),
        Node::Bool(false) => out.push_str("false"),
        Node::Number(raw) => out.push_str(raw),
        Node::Str(s) => write_string(s, out),
        Node::Array(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push_str("[\n");
            for (i, item) in items.iter().enumerate() {
                // NOTE: `items` is written in its own order and is never sorted.
                // See the module docs; `equivalent` proves it on every write.
                indent(depth + 1, out);
                write_node(item, depth + 1, out);
                if i + 1 < items.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            indent(depth, out);
            out.push(']');
        }
        Node::Object(entries) => {
            if entries.is_empty() {
                out.push_str("{}");
                return;
            }
            // The ONE sort in this module. By Unicode scalar value, which for
            // Rust `&str` is UTF-8 byte order — the same order Python's
            // `sorted()` gives, so the Python authoring tools already agree.
            let mut sorted: Vec<&(String, Node)> = entries.iter().collect();
            sorted.sort_by(|a, b| cmp_key(&a.0, &b.0));
            out.push_str("{\n");
            for (i, (key, value)) in sorted.iter().enumerate() {
                indent(depth + 1, out);
                write_string(key, out);
                out.push_str(": ");
                write_node(value, depth + 1, out);
                if i + 1 < sorted.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            indent(depth, out);
            out.push('}');
        }
    }
}

/// Total order on object keys: Unicode scalar value. Extracted so there is one
/// place to look when asking "what is canonical key order".
fn cmp_key(a: &str, b: &str) -> Ordering {
    a.cmp(b)
}

fn indent(depth: usize, out: &mut String) {
    for _ in 0..depth * INDENT {
        out.push(' ');
    }
}

/// Write a JSON string: the shortest legal escape for what must be escaped, and
/// nothing else. Non-ASCII goes out as raw UTF-8 — see the module docs.
fn write_string(s: &str, out: &mut String) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                use std::fmt::Write as _;
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

// ------------------------------------------------------------ equivalence --

/// Prove two documents mean the same thing: **arrays compared index-wise**
/// (so any reordering is a failure), objects compared as key→value maps (so the
/// key sort is the one difference allowed).
///
/// `Err` carries a JSON-pointer-ish path to the first divergence.
pub fn equivalent(before: &Node, after: &Node) -> Result<(), String> {
    fn walk(a: &Node, b: &Node, path: &str) -> Result<(), String> {
        match (a, b) {
            (Node::Null, Node::Null) => Ok(()),
            (Node::Bool(x), Node::Bool(y)) if x == y => Ok(()),
            (Node::Number(x), Node::Number(y)) if x == y => Ok(()),
            (Node::Str(x), Node::Str(y)) if x == y => Ok(()),
            (Node::Array(x), Node::Array(y)) => {
                if x.len() != y.len() {
                    return Err(format!(
                        "{path}: array length changed ({} → {})",
                        x.len(),
                        y.len()
                    ));
                }
                for (i, (xi, yi)) in x.iter().zip(y.iter()).enumerate() {
                    // Index-wise, deliberately: this is the check that makes
                    // "arrays are never reordered" a machine fact.
                    walk(xi, yi, &format!("{path}/{i}"))?;
                }
                Ok(())
            }
            (Node::Object(x), Node::Object(y)) => {
                let mut xs: Vec<&(String, Node)> = x.iter().collect();
                let mut ys: Vec<&(String, Node)> = y.iter().collect();
                xs.sort_by(|p, q| cmp_key(&p.0, &q.0));
                ys.sort_by(|p, q| cmp_key(&p.0, &q.0));
                if xs.len() != ys.len() {
                    return Err(format!(
                        "{path}: object key count changed ({} → {})",
                        xs.len(),
                        ys.len()
                    ));
                }
                for (xe, ye) in xs.iter().zip(ys.iter()) {
                    if xe.0 != ye.0 {
                        return Err(format!("{path}: key `{}` became `{}`", xe.0, ye.0));
                    }
                    walk(&xe.1, &ye.1, &format!("{path}/{}", xe.0))?;
                }
                Ok(())
            }
            _ => Err(format!("{path}: value kind or content changed")),
        }
    }
    walk(before, after, "")
}

/// The whole formatter: parse, render canonically, and **prove** the render did
/// not change what the document means before returning it.
///
/// The self-check is not belt-and-braces. It is the reason a reviewer can trust
/// the array rule without reading the writer: any future edit that reorders an
/// array — a `sort_by` added to the wrong `Vec`, a `BTreeMap` substituted for a
/// `Vec` — fails here on the first real file instead of shipping a campaign
/// whose objectives run in a new order.
pub fn format_text(text: &str) -> Result<String, ParseError> {
    format_with(text, canonical)
}

/// [`format_text`] with the renderer injected, so the equivalence guard can be
/// shown to FIRE — a guard nobody has watched fail is a guard nobody knows
/// works. `tests::the_guard_catches_a_renderer_that_sorts_arrays` hands this a
/// deliberately array-sorting renderer and asserts `DW0772`.
fn format_with(text: &str, render: impl Fn(&Node) -> String) -> Result<String, ParseError> {
    let before = parse(text)?;
    let out = render(&before);
    let after = parse(&out).map_err(|e| ParseError {
        code: DW_FMT_NOT_EQUIVALENT,
        line: e.line,
        col: e.col,
        message: format!(
            "the formatter emitted JSON it cannot itself parse: {}",
            e.message
        ),
    })?;
    if let Err(why) = equivalent(&before, &after) {
        return Err(ParseError {
            code: DW_FMT_NOT_EQUIVALENT,
            line: 1,
            col: 1,
            message: format!(
                "internal error: formatting would change what this document means \
                 ({why}). Nothing was written. This is a compiler bug — arrays are \
                 ordered and must never be reordered; please report it."
            ),
        });
    }
    Ok(out)
}

// -------------------------------------------------------------- discovery --

/// A `delvec build` output root is marked by the `manifest.json` the compiler
/// itself writes there. Directory discovery stops at one: emitted trees are not
/// authored content, some are checked in (`campaigns/*/out/`), and rewriting one
/// would break the byte-identity contract it exists to record.
pub const BUILD_OUTPUT_MARKER: &str = "manifest.json";

/// Every `*.json` file `delvec fmt <path>` would format, in a deterministic
/// order.
///
/// * a file argument is taken as given — you pointed at it;
/// * a directory is walked recursively, entries sorted by name (never
///   `read_dir` order — ADR-0006);
/// * dot-directories (`.git`, `.github`) are skipped;
/// * a directory holding [`BUILD_OUTPUT_MARKER`] is skipped whole.
pub fn discover(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    if root.is_file() {
        out.push(root.to_path_buf());
        return Ok(out);
    }
    walk(root, &mut out)?;
    Ok(out)
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if dir.join(BUILD_OUTPUT_MARKER).is_file() {
        return Ok(());
    }
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<_, _>>()?;
    entries.sort();
    for path in entries {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if name.starts_with('.') {
            continue;
        }
        // `symlink_metadata` so a symlinked directory is not followed: the
        // content repo is symlinked into this one at `campaigns/`, and a walk
        // that followed it would silently reach a second repository.
        let meta = std::fs::symlink_metadata(&path)?;
        if meta.is_dir() {
            walk(&path, out)?;
        } else if meta.is_file() && path.extension().and_then(|e| e.to_str()) == Some("json") {
            out.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_keys_are_sorted_arrays_are_not() {
        let src = r#"{"b":1,"a":[3,1,2],"c":{"z":0,"y":0}}"#;
        assert_eq!(
            format_text(src).unwrap(),
            "{\n  \"a\": [\n    3,\n    1,\n    2\n  ],\n  \"b\": 1,\n  \"c\": {\n    \"y\": 0,\n    \"z\": 0\n  }\n}\n"
        );
    }

    #[test]
    fn number_literals_survive_verbatim() {
        // 2^53 + 1 and a trailing-zero decimal: both change under an f64
        // round-trip, and neither may change here.
        let src = r#"{"big":9007199254740993,"exact":1.50,"exp":1e3,"neg":-0.0}"#;
        let out = format_text(src).unwrap();
        assert!(out.contains("9007199254740993"), "{out}");
        assert!(out.contains("1.50"), "{out}");
        assert!(out.contains("1e3"), "{out}");
        assert!(out.contains("-0.0"), "{out}");
    }

    #[test]
    fn non_ascii_is_never_escaped_and_escapes_are_decoded() {
        let src = r#"{"zh":"洞中公羊","emoji":"😀"}"#;
        let out = format_text(src).unwrap();
        assert!(out.contains("洞中公羊"), "{out}");
        assert!(out.contains('\u{1F600}'), "{out}");
        assert!(!out.contains("\\u"), "{out}");
    }

    #[test]
    fn control_characters_keep_their_shortest_escape() {
        let src = "{\"a\":\"x\\ny\\tz\\u0001\"}";
        let out = format_text(src).unwrap();
        assert_eq!(out, "{\n  \"a\": \"x\\ny\\tz\\u0001\"\n}\n");
    }

    #[test]
    fn idempotent() {
        let src = r#"{"b":[{"q":1,"p":[2,1]}],"a":"é"}"#;
        let once = format_text(src).unwrap();
        assert_eq!(format_text(&once).unwrap(), once);
    }

    #[test]
    fn duplicate_key_is_refused() {
        let e = format_text(r#"{"a":1,"a":2}"#).unwrap_err();
        assert_eq!(e.code, "DW0771");
    }

    #[test]
    fn syntax_errors_are_located() {
        let e = format_text("{\n  \"a\": 1,\n}\n").unwrap_err();
        assert_eq!(e.code, "DW0770");
        assert_eq!(e.line, 3);
    }

    #[test]
    fn empty_containers_stay_on_one_line() {
        assert_eq!(
            format_text(r#"{"a":[],"b":{}}"#).unwrap(),
            "{\n  \"a\": [],\n  \"b\": {}\n}\n"
        );
    }

    /// The red demonstration. Swap in a renderer that sorts arrays — the exact
    /// correctness bug the hard constraint forbids — and the formatter must
    /// refuse with `DW0772` rather than write the file.
    #[test]
    fn the_guard_catches_a_renderer_that_sorts_arrays() {
        fn sorting_render(node: &Node) -> String {
            fn sabotage(n: &Node) -> Node {
                match n {
                    Node::Array(items) => {
                        let mut v: Vec<Node> = items.iter().map(sabotage).collect();
                        v.sort_by_key(|n| match n {
                            Node::Number(raw) => raw.clone(),
                            Node::Str(s) => s.clone(),
                            _ => String::new(),
                        });
                        Node::Array(v)
                    }
                    Node::Object(e) => {
                        Node::Object(e.iter().map(|(k, v)| (k.clone(), sabotage(v))).collect())
                    }
                    other => other.clone(),
                }
            }
            canonical(&sabotage(node))
        }
        // Sanity: the honest renderer is accepted on the same input.
        let src = r#"{"objectives":["3","1","2"]}"#;
        assert!(format_with(src, canonical).is_ok());
        let e = format_with(src, sorting_render).unwrap_err();
        assert_eq!(e.code, "DW0772");
        assert!(e.message.contains("/objectives/0"), "{}", e.message);
    }

    #[test]
    fn equivalent_catches_a_reordered_array() {
        // The guard that makes the array rule a machine fact rather than a
        // promise: hand it a reordering and it must refuse.
        let a = parse("[1,2,3]").unwrap();
        let b = parse("[3,2,1]").unwrap();
        assert!(equivalent(&a, &b).is_err());
        assert!(equivalent(&a, &a).is_ok());
    }
}
