//! **What of the package's source is production**: the one rule every test that
//! reads `crates/delvec/src` as text reads it through.
//!
//! A file's production text is the file with every item that a column-zero
//! `#[cfg(test)]` applies to removed — the item, and nothing after it. An item
//! that is an out-of-line module (`#[cfg(test)] mod tests;`) removes the
//! module's file too, and every file under its directory, so a test module
//! written as its own file is test code exactly as one written inline is.
//!
//! The extent of an item is read with a lexer that knows comments, string and
//! character literals, so a brace inside a string does not end a function. An
//! attribute that is not at column zero (one nested inside an inline module)
//! is not seen; nothing in the package nests one.

use std::path::{Path, PathBuf};

const ATTR: &str = "#[cfg(test)]\n";

/// `text` without the items a column-zero `#[cfg(test)]` applies to, and the
/// names of the out-of-line modules among those items.
pub fn production(text: &str) -> (String, Vec<String>) {
    let mut out = String::new();
    let mut modules = Vec::new();
    let mut rest = text;
    loop {
        let at = if rest.starts_with(ATTR) {
            Some(0)
        } else {
            rest.find(&format!("\n{ATTR}")).map(|i| i + 1)
        };
        let Some(at) = at else {
            out.push_str(rest);
            break;
        };
        out.push_str(&rest[..at]);
        let item = &rest[at + ATTR.len()..];
        let len = item_len(item);
        if let Some(name) = out_of_line_module(&item[..len]) {
            modules.push(name);
        }
        rest = &item[len..];
    }
    (out, modules)
}

/// The length of the item at the start of `s`: through the `;` that ends it at
/// nesting depth zero, or through the `}` that closes its body (and a `;`
/// directly after that brace).
fn item_len(s: &str) -> usize {
    let b: Vec<(usize, char)> = s.char_indices().collect();
    let at = |k: usize| b.get(k).map(|&(_, c)| c);
    let end_of = |k: usize| b.get(k).map_or(s.len(), |&(i, c)| i + c.len_utf8());
    let mut depth = 0i32;
    let mut k = 0;
    while k < b.len() {
        let c = b[k].1;
        match c {
            '/' if at(k + 1) == Some('/') => {
                while k < b.len() && b[k].1 != '\n' {
                    k += 1;
                }
            }
            '/' if at(k + 1) == Some('*') => {
                let mut nest = 1;
                k += 2;
                while k < b.len() && nest > 0 {
                    if b[k].1 == '/' && at(k + 1) == Some('*') {
                        nest += 1;
                        k += 1;
                    } else if b[k].1 == '*' && at(k + 1) == Some('/') {
                        nest -= 1;
                        k += 1;
                    }
                    k += 1;
                }
                continue;
            }
            '"' => {
                k += 1;
                while k < b.len() && b[k].1 != '"' {
                    if b[k].1 == '\\' {
                        k += 1;
                    }
                    k += 1;
                }
            }
            'r' if (k == 0
                || !is_ident(b[k - 1].1)
                || (b[k - 1].1 == 'b' && (k < 2 || !is_ident(b[k - 2].1))))
                && matches!(at(k + 1), Some('"') | Some('#')) =>
            {
                let mut j = k + 1;
                let mut hashes = 0;
                while at(j) == Some('#') {
                    hashes += 1;
                    j += 1;
                }
                if at(j) == Some('"') {
                    j += 1;
                    'raw: while j < b.len() {
                        if b[j].1 == '"' && (1..=hashes).all(|h| at(j + h) == Some('#')) {
                            j += hashes;
                            break 'raw;
                        }
                        j += 1;
                    }
                    k = j;
                }
            }
            '\'' => {
                if at(k + 1) == Some('\\') {
                    k += 2;
                    while k < b.len() && b[k].1 != '\'' {
                        k += 1;
                    }
                } else if at(k + 2) == Some('\'') {
                    k += 2;
                }
            }
            '(' | '[' | '{' => depth += 1,
            ')' | ']' => depth -= 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let mut j = k + 1;
                    while matches!(at(j), Some(' ') | Some('\t')) {
                        j += 1;
                    }
                    return if at(j) == Some(';') {
                        end_of(j)
                    } else {
                        end_of(k)
                    };
                }
            }
            ';' if depth == 0 => return end_of(k),
            _ => {}
        }
        k += 1;
    }
    s.len()
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// `Some(name)` when `item` is `mod name;` (after its attributes and docs).
fn out_of_line_module(item: &str) -> Option<String> {
    let line = item
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with("//") && !l.starts_with("#["))?;
    let decl = line.strip_suffix(';')?;
    let decl = match decl.strip_prefix("pub") {
        Some(rest) => rest.trim_start_matches(|c| c != ' ').trim_start(),
        None => decl,
    };
    let name = decl.strip_prefix("mod ")?.trim();
    name.chars().all(is_ident).then(|| name.to_string())
}

/// Every `.rs` file under `src` that is compiled outside `cfg(test)`, as its
/// path relative to `src` (with `/` separators) and its production text, in
/// path order. A file that is an out-of-line test module, or lies under one's
/// directory, is not in the list.
pub fn production_sources(src: &Path) -> Vec<(String, String)> {
    let mut files: Vec<PathBuf> = Vec::new();
    let mut stack = vec![src.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).expect("read the crate's own sources") {
            let p = e.expect("dir entry").path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                files.push(p);
            }
        }
    }
    files.sort();
    let rel = |p: &Path| {
        p.strip_prefix(src)
            .expect("under src")
            .to_string_lossy()
            .replace('\\', "/")
    };
    let mut texts: Vec<(String, String)> = Vec::new();
    // Module paths (relative, without `.rs`) whose files are test code.
    let mut test_modules: Vec<String> = Vec::new();
    for p in &files {
        let text = std::fs::read_to_string(p).expect("read source");
        let (prod, modules) = production(&text);
        let r = rel(p);
        // The directory a file's child modules live in.
        let stem = r.strip_suffix(".rs").expect("a .rs path");
        let dir = match stem.rsplit_once('/') {
            Some((parent, "mod")) => parent.to_string(),
            None if matches!(stem, "mod" | "lib" | "main") => String::new(),
            _ => stem.to_string(),
        };
        for m in modules {
            test_modules.push(if dir.is_empty() {
                m
            } else {
                format!("{dir}/{m}")
            });
        }
        texts.push((r, prod));
    }
    texts
        .into_iter()
        .filter(|(r, _)| {
            let stem = r.strip_suffix(".rs").expect("a .rs path");
            let stem = stem.strip_suffix("/mod").unwrap_or(stem);
            !test_modules
                .iter()
                .any(|m| stem == m || stem.starts_with(&format!("{m}/")))
        })
        .collect()
}
