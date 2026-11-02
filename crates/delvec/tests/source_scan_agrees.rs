//! **The coverage floor's production rule is `source_scan`'s.** `tools/ci/coverage-floor.py`
//! drops every coverage region inside a `#[cfg(test)]` item before it judges the
//! floor (spec-0126 §1.2); `common::source_scan` is the rule's home. The same
//! generated files are asked of both, and any difference is red.
//!
//! The cases are generated from a fixed seed, so the set is the same on every
//! run: inline items, out-of-line modules at every visibility, and the lexical
//! shapes that decide an item's extent (braces in strings, raw strings, char
//! literals, nested block comments, a `;` after a closing brace).

use std::path::{Path, PathBuf};
use std::process::Command;

mod common;

use common::source_scan;

/// The cases the agreement is judged over, at least this many.
const CASES: usize = 64;
const TREES: usize = 12;

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn pick<'a>(&mut self, xs: &[&'a str]) -> &'a str {
        xs[(self.next() as usize) % xs.len()]
    }
}

const PRODUCTION: &[&str] = &[
    "pub fn a() -> u32 {\n    1\n}\n",
    "fn b(s: &str) -> bool {\n    s == \"}\" || s == \"{\"\n}\n",
    "const C: &str = r#\"a \" } { b\"#;\n",
    "struct D {\n    x: u8,\n}\n",
    "fn e() -> char {\n    '}'\n}\n",
    "/* a { nested /* comment } */ still */\nfn f() {}\n",
    "// a line comment with a { brace\nfn g() {}\n",
    "impl D {\n    fn h(&self) -> u8 { self.x }\n}\n",
    "pub(crate) mod inner {\n    pub fn i() {}\n}\n",
    "static J: [u8; 2] = [1, 2];\n",
    "fn k() -> char { '\\'' }\n",
    "macro_rules! m {\n    () => {};\n}\n",
];

const TEST_ITEMS: &[&str] = &[
    "mod tests {\n    #[test]\n    fn t() { assert_eq!(1, 1); }\n}\n",
    "fn helper() -> &'static str {\n    \"} not the end {\"\n}\n",
    "mod t_out;\n",
    "pub mod t_pub;\n",
    "pub(crate) mod t_crate;\n",
    "/// a doc line\nmod t_doc;\n",
    "const T: &str = r##\"x \"# } y\"##;\n",
    "impl D {\n    fn test_only(&self) {}\n};\n",
    "use std::collections::BTreeMap;\n",
    "#[allow(dead_code)]\nfn attributed() {}\n",
];

fn generate(rng: &mut Lcg) -> String {
    let mut text = String::new();
    let parts = 2 + (rng.next() % 6) as usize;
    for _ in 0..parts {
        if rng.next() % 3 == 0 {
            text.push_str("#[cfg(test)]\n");
            text.push_str(rng.pick(TEST_ITEMS));
        } else {
            text.push_str(rng.pick(PRODUCTION));
        }
        if rng.next() % 4 == 0 {
            text.push('\n');
        }
    }
    text
}

fn python(args: &[&str]) -> serde_json::Value {
    let tool = common::repo_root().join("tools/ci/coverage-floor.py");
    let out = Command::new("python3")
        .arg(&tool)
        .args(args)
        .output()
        .expect("python3 runs");
    assert!(
        out.status.success(),
        "coverage-floor.py {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("coverage-floor.py prints JSON")
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("source-scan-agrees-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn both_rules_keep_the_same_production_text_over_every_generated_file() {
    let dir = scratch("files");
    let mut rng = Lcg(0x0126);
    let mut disagreements = Vec::new();
    let mut with_test_items = 0;
    let mut with_modules = 0;
    for n in 0..CASES {
        let text = generate(&mut rng);
        let path = dir.join(format!("case{n:03}.rs"));
        std::fs::write(&path, &text).unwrap();
        let (prod, modules) = source_scan::production(&text);
        let py = python(&["--production", path.to_str().unwrap()]);
        let py_prod = py["production"].as_str().unwrap();
        let py_modules: Vec<String> = py["modules"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m.as_str().unwrap().to_string())
            .collect();
        if prod != text {
            with_test_items += 1;
        }
        if !modules.is_empty() {
            with_modules += 1;
        }
        if prod != py_prod || modules != py_modules {
            disagreements.push(format!("case{n:03}"));
        }
    }
    println!(
        "source_scan agreement: {CASES} generated files, {with_test_items} with a cfg(test) \
         item, {with_modules} with an out-of-line test module; {} disagreements",
        disagreements.len()
    );
    assert!(
        with_test_items > CASES / 4 && with_modules > 4,
        "the generator must exercise both kinds"
    );
    assert!(
        disagreements.is_empty(),
        "coverage-floor.py and source_scan disagree on: {disagreements:?}"
    );
}

#[test]
fn both_rules_keep_the_same_production_files_over_every_generated_tree() {
    let mut rng = Lcg(0x6210);
    let mut disagreements = Vec::new();
    let mut dropped = 0;
    for n in 0..TREES {
        let src = scratch(&format!("tree{n:02}")).join("src");
        std::fs::create_dir_all(src.join("t_out")).unwrap();
        std::fs::create_dir_all(src.join("inner")).unwrap();
        std::fs::write(src.join("lib.rs"), generate(&mut rng)).unwrap();
        std::fs::write(src.join("inner.rs"), generate(&mut rng)).unwrap();
        for f in [
            "t_out.rs",
            "t_out/deep.rs",
            "t_pub.rs",
            "t_crate.rs",
            "t_doc.rs",
            "inner/t_out.rs",
            "inner/t_pub/mod.rs",
            "inner/kept.rs",
        ] {
            let p = src.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, generate(&mut rng)).unwrap();
        }
        let rust: Vec<(String, String)> = source_scan::production_sources(&src);
        let py: Vec<(String, String)> = python(&["--production-sources", src.to_str().unwrap()])
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                (
                    r[0].as_str().unwrap().to_string(),
                    r[1].as_str().unwrap().to_string(),
                )
            })
            .collect();
        dropped += 10 - rust.len();
        if rust != py {
            disagreements.push(format!("tree{n:02}"));
        }
    }
    println!(
        "source_scan agreement: {TREES} generated trees of 10 files, {dropped} files dropped \
         as test modules; {} disagreements",
        disagreements.len()
    );
    assert!(dropped > 0, "the generator must drop a test module's file");
    assert!(
        disagreements.is_empty(),
        "coverage-floor.py and source_scan disagree on: {disagreements:?}"
    );
}
