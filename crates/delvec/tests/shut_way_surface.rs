//! **The shut-way surface** (spec-0118 §4, criterion 1): `gating.learned` is
//! the closed set `seen | told | rewarded | hidden`, `gating.secret` names the
//! words a hidden way keeps, and `DW0818` holds the two together — over the
//! blockout fixture's gated cell door and the ferry fixture's carry.

mod common;

use delvewright_dsl::Diagnostic;
use serde_json::{Value, json};

/// The fixture's raw documents with `edit` applied to the layout graph's edge
/// `edge`, parsed — `Err` carrying the parse refusals.
fn parsed(
    fixture: &str,
    edge: &str,
    edit: impl FnOnce(&mut Value),
) -> Result<delvewright_dsl::Campaign, Vec<Diagnostic>> {
    let dir = common::repo_root().join(format!("crates/delvec/tests/fixtures/{fixture}"));
    let mut loaded = delvec::compiler::load::load_campaign_dir(&dir).expect("readable");
    let mut graph: Value =
        serde_json::from_str(loaded.raw.layout_graph.as_deref().expect("a graph")).unwrap();
    let e = graph["content"]["edges"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|e| e["id"] == edge)
        .unwrap_or_else(|| panic!("{edge}"));
    edit(e);
    loaded.raw.layout_graph = Some(graph.to_string());
    delvewright_dsl::parse_campaign(&loaded.raw)
}

/// The layout graph's refusals of a campaign, as `(code, message)`.
fn graph_refusals(c: &delvewright_dsl::Campaign) -> Vec<(String, String)> {
    let mut d = Vec::new();
    let mut reads = delvewright_dsl::metrics::Reads::new();
    delvewright_dsl::layout::check(c, &mut reads, &mut d);
    d.into_iter()
        .filter(|x| x.severity == delvewright_dsl::Severity::Error)
        .map(|x| (x.code, x.message))
        .collect()
}

fn cell_door(edit: impl FnOnce(&mut Value)) -> Vec<(String, String)> {
    let c = parsed("blockout", "edge/hall-cell", edit).expect("parses");
    graph_refusals(&c)
}

#[test]
fn a_hidden_way_names_its_secret_and_only_a_hidden_way_does() {
    // Well formed: hidden with a secret.
    let r = cell_door(|e| {
        e["gating"]["learned"] = json!("hidden");
        e["gating"]["secret"] = json!(["vault"]);
    });
    assert!(r.is_empty(), "{r:?}");

    // `hidden` without `secret`.
    let r = cell_door(|e| e["gating"]["learned"] = json!("hidden"));
    assert_eq!(r.len(), 1, "{r:?}");
    assert_eq!(r[0].0, "DW0818");
    assert!(r[0].1.contains("names no `secret`"), "{}", r[0].1);

    // `secret` beside another word, and beside no word.
    for learned in [Some("seen"), None] {
        let r = cell_door(|e| {
            match learned {
                Some(l) => e["gating"]["learned"] = json!(l),
                None => {
                    e["gating"].as_object_mut().unwrap().remove("learned");
                }
            }
            e["gating"]["secret"] = json!(["bar"]);
        });
        assert_eq!(r.len(), 1, "{learned:?}: {r:?}");
        assert_eq!(r[0].0, "DW0818");
        assert!(r[0].1.contains("names a `secret` beside"), "{}", r[0].1);
    }

    // An empty word keeps nothing.
    let r = cell_door(|e| {
        e["gating"]["learned"] = json!("hidden");
        e["gating"]["secret"] = json!(["vault", " "]);
    });
    assert_eq!(r.len(), 1, "{r:?}");
    assert!(r[0].1.contains("`secret[1]` is empty"), "{}", r[0].1);
}

#[test]
fn a_word_with_no_term_is_dw0818_naming_learned_beside_opens_from() {
    let r = cell_door(|e| e["gating"] = json!({"learned": "seen", "opens_from": "a"}));
    assert!(
        r.iter().any(|(c, m)| c == "DW0818"
            && m.contains("`learned: seen`")
            && m.contains("beside `opens_from`")),
        "{r:?}"
    );
}

#[test]
fn a_word_or_a_secret_on_a_carry_is_dw0100() {
    let c = common::repo_root().join("crates/delvec/tests/fixtures/ferry-site");
    let loaded = delvec::compiler::load::load_campaign_dir(&c).expect("readable");
    let graph: Value = serde_json::from_str(loaded.raw.layout_graph.as_deref().unwrap()).unwrap();
    let carry = graph["content"]["edges"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["class"] == "carry")
        .expect("a carry")["id"]
        .as_str()
        .unwrap()
        .to_string();
    for (key, value) in [("learned", json!("hidden")), ("secret", json!(["ferry"]))] {
        let Err(d) = parsed("ferry-site", &carry, |e| {
            if e.get("gating").is_none() {
                e["gating"] = json!({});
            }
            e["gating"][key] = value.clone();
        }) else {
            panic!("a `{key}` on a carry must not parse");
        };
        assert!(
            d.iter()
                .any(|x| x.code == "DW0100" && x.message.contains(key)),
            "{key}: {:?}",
            d.iter().map(|x| (&x.code, &x.message)).collect::<Vec<_>>()
        );
    }
}

#[test]
fn the_schema_exports_four_words_and_the_secret() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_delvec"))
        .args(["schema", "--stage", "layout-graph"])
        .output()
        .expect("delvec runs");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let s = String::from_utf8_lossy(&out.stdout);
    for needle in [
        "\"seen\"",
        "\"told\"",
        "\"rewarded\"",
        "\"hidden\"",
        "\"secret\"",
    ] {
        assert!(s.contains(needle), "layout-graph schema lacks {needle}");
    }
}
