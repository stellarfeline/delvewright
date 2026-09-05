//! **A map with no layout graph has no anchor names to be wrong about.**
//!
//! A site-plan campaign's anchor vocabulary is DERIVED — a `node-` per place, a
//! `seam-` per barred way, an `unlock-` on the openable side of a one-sided one,
//! `spawn` for the entry, and every `stations[]` name its nodes declare — and all
//! of it is read off `layout-graph.json`. Remove that document and the derived
//! set is not empty, it is unknown; `DW0824` is the finding, and every rule that
//! resolves a name against the set was refusing correct names on the strength of
//! a document that is not there.
//!
//! Measured on the gallery's site-plan point with `layout-graph.json` removed,
//! engine `8c0485f7`: **15 refusals** — `DW0824`, `DW0842` (already folded), and
//! thirteen more (`DW0142` x10, `DW0371` x2, `DW0343` x1), each printing a
//! prescription that cannot be taken, because it says to write one of the names
//! the graph creates. On the fixture below, five refusals for one missing file.
//!
//! What is asserted here is BOTH sides. The silence is only a fold if the same
//! rules still refuse a name the graph really does not place, so every code that
//! goes quiet in one test is made to fire in another, on the same fixture with
//! the graph restored.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::json;

/// The parsed campaign at `dir` — the predicate below is asked of a `Campaign`,
/// not of a directory.
fn campaign_at(dir: &Path) -> delvewright_dsl::Campaign {
    let loaded =
        delvewright_compiler::load::load_campaign_dir(dir).expect("the campaign is readable");
    delvewright_dsl::parse_campaign(&loaded.raw).expect("the campaign parses")
}

fn tempdir(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("dw-vocab-{name}"));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// A private copy of the blockout site-plan fixture, patched, with or without
/// its layout graph. `whose` is the caller's own name: `cargo test` runs these on
/// different threads and one shared directory name is a red that comes and goes.
fn campaign(whose: &str, graph: bool, patch: impl FnOnce(&mut serde_json::Value)) -> PathBuf {
    let tmp = tempdir(whose);
    let dir = tmp.join("campaign");
    common::copy_dir_all(
        &common::repo_root().join("crates/compiler/tests/fixtures/blockout"),
        &dir,
    );
    common::patch_file(&dir.join("quests.json"), patch);
    if !graph {
        std::fs::remove_file(dir.join("layout-graph.json")).unwrap();
    }
    dir
}

/// `delvec validate`'s diagnostics, tallied by code.
fn codes(campaign: &Path) -> BTreeMap<String, usize> {
    let prefabs = campaign.parent().unwrap().join("prefabs");
    std::fs::create_dir_all(&prefabs).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_delvec"))
        .arg("--prefabs")
        .arg(&prefabs)
        .arg("validate")
        .arg(campaign)
        .output()
        .expect("`delvec validate` runs");
    let text = String::from_utf8(out.stdout).expect("stdout is utf-8");
    let mut tally: BTreeMap<String, usize> = BTreeMap::new();
    for line in text.lines() {
        if let Some(code) = line.split_whitespace().next()
            && code.starts_with("DW")
        {
            *tally.entry(code.to_string()).or_default() += 1;
        }
    }
    tally
}

/// The `use` trigger a barred gate owes an author (`DW0429`), so that the
/// shortcut and seal fixtures below are green for the reason under test and not
/// for a second one.
fn door_trigger() -> serde_json::Value {
    json!([{
        "id": "trigger/cell-door",
        "at": "anchor/seam-hall-cell",
        "on": {"on": "use"},
        "once": false,
        "audience": "presser",
        "effects": [{
            "type": "narrate",
            "style": "actionbar",
            "text": "The door will not give from this side."
        }]
    }])
}

/// A shortcut over the fixture's one barred way — the `DW0371` surface.
fn with_shortcut(gate: &str) -> impl FnOnce(&mut serde_json::Value) + use<> {
    let gate = gate.to_string();
    move |v: &mut serde_json::Value| {
        v["content"]["shortcuts"] = json!([{
            "id": "shortcut/cell-back",
            "gate": gate,
            "unlock": "anchor/unlock-hall-cell",
        }]);
        v["content"]["triggers"] = door_trigger();
    }
}

/// A `close-gate` over the same way — the `DW0343` surface, which lives in the
/// compiler because the fill block is prefab metadata.
fn with_close_gate(anchor: &str) -> impl FnOnce(&mut serde_json::Value) + use<> {
    let anchor = anchor.to_string();
    move |v: &mut serde_json::Value| {
        v["content"]["quests"][0]["on_complete"]
            .as_array_mut()
            .unwrap()
            .insert(
                0,
                json!({
                    "type": "close-gate",
                    "anchor": anchor,
                    "happening": {"verb": "seals", "text": "The cell door drops back."},
                }),
            );
        v["content"]["triggers"] = door_trigger();
    }
}

// ---------------------------------------------------------------------------
// The cause is reported once
// ---------------------------------------------------------------------------

/// **One missing document, one refusal.** The fixture names four anchors across
/// `npcs` and `quests`; with the graph gone each of them used to be refused
/// individually, ahead of the line that was the author's.
#[test]
fn a_map_with_no_graph_refuses_once() {
    let tally = codes(&campaign("plain", false, |_| {}));
    assert_eq!(
        tally.get("DW0824").copied(),
        Some(1),
        "the finding is reported: {tally:?}"
    );
    assert_eq!(
        tally.get("DW0142"),
        None,
        "and no anchor name is judged against a vocabulary that is not there: {tally:?}"
    );
    assert_eq!(
        tally.values().sum::<usize>(),
        1,
        "one line for one missing file: {tally:?}"
    );
}

/// The other two codes that resolve a name against the derived set, each on the
/// surface that raises it. `DW0371` is `dsl::validate`'s shortcut check and
/// `DW0343` is `compiler::gates`, two crates asking one authority.
#[test]
fn no_graph_silences_every_code_that_reads_the_derived_set() {
    let sc = codes(&campaign(
        "shortcut-nograph",
        false,
        with_shortcut("anchor/seam-hall-cell"),
    ));
    assert_eq!(sc.get("DW0371"), None, "{sc:?}");
    assert_eq!(sc.get("DW0343"), None, "{sc:?}");
    assert_eq!(sc.get("DW0824").copied(), Some(1), "{sc:?}");

    let cg = codes(&campaign(
        "seal-nograph",
        false,
        with_close_gate("anchor/seam-hall-cell"),
    ));
    assert_eq!(cg.get("DW0343"), None, "{cg:?}");
    assert_eq!(cg.get("DW0824").copied(), Some(1), "{cg:?}");
}

// ---------------------------------------------------------------------------
// …and nothing stopped refusing
// ---------------------------------------------------------------------------

/// **The half that makes the silence a fold rather than a hole.** With the graph
/// present, a name it does not place is refused exactly as before — by each of
/// the three codes, on the same fixtures the tests above run green.
#[test]
fn a_name_the_graph_does_not_place_is_still_refused() {
    let npc = codes(&campaign("bad-npc", true, |v| {
        v["content"]["quests"][0]["objectives"][1]["anchor"] = json!("anchor/node-nowhere");
    }));
    assert_eq!(npc.get("DW0142").copied(), Some(1), "{npc:?}");

    let sc = codes(&campaign(
        "bad-shortcut",
        true,
        with_shortcut("anchor/seam-nowhere"),
    ));
    assert_eq!(sc.get("DW0371").copied(), Some(1), "{sc:?}");

    let cg = codes(&campaign(
        "bad-seal",
        true,
        with_close_gate("anchor/seam-nowhere"),
    ));
    assert_eq!(cg.get("DW0343").copied(), Some(1), "{cg:?}");
}

/// **A prefab campaign is untouched at every state of its documents**: its
/// vocabulary is prefab metadata, which the map pipeline has nothing to do with.
/// Asserted on the predicate itself, where the distinction is declared.
#[test]
fn the_question_is_asked_of_the_placement_authority() {
    let site = common::repo_root().join("crates/compiler/tests/fixtures/blockout");
    let with_graph = campaign_at(&site);
    assert!(
        !delvewright_dsl::anchor_vocabulary_unknowable(&with_graph),
        "a site plan WITH its graph knows its own names"
    );

    let hello = campaign_at(&common::hello_world_dir());
    assert!(
        !delvewright_dsl::anchor_vocabulary_unknowable(&hello),
        "a prefab campaign's names come from metadata, never from a graph"
    );

    let stripped = campaign("predicate", false, |_| {});
    let no_graph = campaign_at(&stripped);
    assert!(
        delvewright_dsl::anchor_vocabulary_unknowable(&no_graph),
        "and a site plan with no graph has no vocabulary to judge against"
    );
}
