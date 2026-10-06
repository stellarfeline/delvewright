//! Helpers for the `delvec sculpt` tests (spec-0087): the gallery form, a
//! declared edit over it, and the command run over the result.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The gallery's sculpt form — the primary every form test edits.
pub fn gallery_form_path() -> PathBuf {
    super::repo_root().join("gallery/forms/gallery-carcass.json")
}

/// The gallery form as JSON.
pub fn gallery_form() -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(gallery_form_path()).unwrap()).unwrap()
}

/// A committed fixture form under `tests/fixtures/sculpt/`.
pub fn fixture_form(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sculpt")
        .join(name)
}

/// The edits a form probe under `gallery/forms/probes/<name>/` declares.
pub fn probe_patch(name: &str) -> Vec<serde_json::Value> {
    let p = super::repo_root()
        .join("gallery/forms/probes")
        .join(name)
        .join("probe.json");
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();
    v["patch"].as_array().unwrap().clone()
}

fn unescape(t: &str) -> String {
    t.replace("~1", "/").replace("~0", "~")
}

/// Apply RFC 6901 `add` / `remove` / `replace` edits, the rules the gallery's
/// probes are written in (`tools/ci/gallery_domain.py::apply_patch`).
pub fn apply(mut doc: serde_json::Value, ops: &[serde_json::Value]) -> serde_json::Value {
    for op in ops {
        let verb = op["op"].as_str().unwrap();
        let path = op["path"].as_str().unwrap();
        let tokens: Vec<String> = path[1..].split('/').map(unescape).collect();
        let (last, parents) = tokens.split_last().unwrap();
        let mut node = &mut doc;
        for t in parents {
            node = match node {
                serde_json::Value::Object(m) => m.get_mut(t).unwrap(),
                serde_json::Value::Array(a) => a.get_mut(t.parse::<usize>().unwrap()).unwrap(),
                _ => panic!("`{path}` passes through a scalar"),
            };
        }
        match (node, verb) {
            (serde_json::Value::Object(m), "remove") => {
                m.remove(last).expect("remove names a key the form has");
            }
            (serde_json::Value::Object(m), _) => {
                m.insert(last.clone(), op["value"].clone());
            }
            (serde_json::Value::Array(a), "add") if last == "-" => a.push(op["value"].clone()),
            (serde_json::Value::Array(a), "add") => {
                a.insert(last.parse().unwrap(), op["value"].clone())
            }
            (serde_json::Value::Array(a), "remove") => {
                a.remove(last.parse().unwrap());
            }
            (serde_json::Value::Array(a), _) => {
                a[last.parse::<usize>().unwrap()] = op["value"].clone()
            }
            _ => panic!("`{path}` has a scalar parent"),
        }
    }
    doc
}

/// What one `delvec sculpt` run did.
pub struct Run {
    /// Exit status.
    pub code: i32,
    /// Everything it printed, stdout then stderr.
    pub said: String,
    /// The output directory it was pointed at.
    pub out: PathBuf,
}

impl Run {
    /// The `pockets: P place(s), …` line's `P`, if it printed one.
    pub fn pocket_places(&self) -> Option<usize> {
        self.said.lines().find_map(|l| {
            let rest = l.trim().strip_prefix("pockets: ")?;
            rest.split(' ').next()?.parse().ok()
        })
    }
}

/// Write `form` under the test's own scratch directory `tag` and sculpt it.
pub fn sculpt(form: &serde_json::Value, tag: &str, extra: &[&str]) -> Run {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("sculpt-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("form.json");
    std::fs::write(&path, serde_json::to_string_pretty(form).unwrap()).unwrap();
    sculpt_file(&path, &dir.join("out"), extra)
}

/// Sculpt a form file into `out`.
pub fn sculpt_file(form: &Path, out: &Path, extra: &[&str]) -> Run {
    let o = Command::new(env!("CARGO_BIN_EXE_delvec"))
        .arg("sculpt")
        .arg(form)
        .arg("-o")
        .arg(out)
        .args(extra)
        .output()
        .expect("delvec runs");
    Run {
        code: o.status.code().unwrap_or(-1),
        said: format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        ),
        out: out.to_path_buf(),
    }
}
