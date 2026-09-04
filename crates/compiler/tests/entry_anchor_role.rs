//! **An entry is a role, not a spelling** (spec-0046).
//!
//! The compiler used to find a campaign's entry point by matching an anchor's
//! NAME against `plan::ENTRY_ANCHOR_NAMES` (`spawn`, then `entry`). Two of the
//! three producers of prefab metadata can write those names. The third — the
//! grammar back end — structurally cannot: every anchor it exports is keyed
//! `anchor/<stem>`, deliberately, so that a mark cannot name an anchor the DSL
//! could not reference. A generated zone could therefore never declare an entry
//! point, and an area bound to one was never transported into.
//!
//! The identification now lives on the anchor, as a declared role every
//! producer can write and none has to spell. These tests bind the whole path:
//! what the grammar exports, what the compiler resolves, what it refuses, and
//! that nothing outside the one resolver asks the question for itself.
//!
//! The fixture is `branch-transport` — **two** areas with a promised crossing
//! between them — for the reason spec-0046 §4.1 names: in a one-area campaign,
//! or one whose consecutive critical objectives share an area, no crossing
//! exists and the resolution is never asked for. `the_shipped_library_promises_
//! the_crossing` is the control that keeps every case below from passing
//! vacuously.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildFailure, BuildOutput};
use delvewright_compiler::plan::{self, Plan};
use delvewright_compiler::registry::{AnchorRole, PrefabRegistry};
use delvewright_dsl::parse_campaign;
use delvewright_grammar::ir::Node;
use delvewright_grammar::{Box3, ExpandOptions, Mark, MarkAt, Program, export_prefab};
use serde_json::Value;

/// The landing's entry cell in world coordinates — where the bolt branch's
/// crossing has to land, however the piece declares that anchor.
const LANDING_ENTRY: [i32; 3] = [262, 66, 8];

fn fixture_dir() -> PathBuf {
    common::compiler_fixtures_dir().join("branch-transport")
}

// ---------------------------------------------------------------------------
// the declaration, taken from the producer that could not spell a name
// ---------------------------------------------------------------------------

/// The anchor key and role a **grammar program** exports when its `mark`
/// declares an entry — read off a real export rather than typed here, so the
/// compiler's half of these tests cannot drift from what the exporter writes.
///
/// The key is the whole point: `anchor/landing` is not `spawn`, is not `entry`,
/// and no rename could make it either without breaking the invariant that a mark
/// names an anchor the DSL can reference.
fn grammar_declared_entry() -> (String, Value) {
    let program = Program::new("marker", "root").rule(
        "root",
        Node::Mark {
            mark: Mark::new("landing", MarkAt::FloorCenter).role(AnchorRole::Entry),
            body: Box::new(Node::Skip),
        },
    );
    let export = export_prefab(
        &program,
        Box3::at_origin([5, 4, 5]),
        &ExpandOptions::seeded(3),
        "role-zone",
    )
    .expect("the zone exports");
    let (key, anchor) = export
        .metadata
        .anchors
        .iter()
        .next()
        .map(|(k, a)| (k.clone(), serde_json::to_value(a).unwrap()))
        .expect("the marked zone exports its one anchor");
    assert!(
        !plan::ENTRY_ANCHOR_NAMES.contains(&key.as_str()),
        "fixture drift: an exported key that IS an entry-anchor name would make \
         every case below vacuous — the role would never be consulted"
    );
    assert_eq!(anchor["role"], Value::from("entry"));
    (key, anchor["role"].clone())
}

/// A private copy of the prefab library in which both areas' pieces declare
/// their entry point the way a generated zone has to: under the key a grammar
/// export produces, and — when `with_role` — carrying the role it writes.
///
/// The rename is the perturbation and the role is the variable. Same cells, same
/// facings, same pieces, same campaign: only what says *this is the way in*
/// changes.
fn library_declaring_entry(tag: &str, with_role: bool) -> (PathBuf, String) {
    let (key, role) = grammar_declared_entry();
    let dir = std::env::temp_dir().join(format!("dw-entry-role-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    common::copy_dir_all(&common::prefabs_dir(), &dir);
    for piece in ["hello-room.json", "cave-shore.json"] {
        let path = dir.join(piece);
        let mut meta: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let anchors = meta["anchors"].as_object_mut().unwrap();
        let mut anchor = anchors
            .remove("spawn")
            .unwrap_or_else(|| panic!("fixture drift: {piece} must declare a `spawn` anchor"));
        if with_role {
            anchor["role"] = role.clone();
        }
        anchors.insert(key.clone(), anchor);
        std::fs::write(&path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();
    }
    (dir, key)
}

/// The same library with `role` written onto two anchors named here — the
/// duplicate case, and its own control when the two sit in different areas.
fn library_with_roles_on(tag: &str, sites: &[(&str, &str)]) -> PathBuf {
    let (_, role) = grammar_declared_entry();
    let dir = std::env::temp_dir().join(format!("dw-entry-dup-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    common::copy_dir_all(&common::prefabs_dir(), &dir);
    for (piece, anchor) in sites {
        let path = dir.join(piece);
        let mut meta: Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        meta["anchors"]
            .as_object_mut()
            .unwrap()
            .get_mut(*anchor)
            .unwrap_or_else(|| panic!("fixture drift: {piece} declares no `{anchor}`"))["role"] =
            role.clone();
        std::fs::write(&path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();
    }
    dir
}

// ---------------------------------------------------------------------------
// building the fixture
// ---------------------------------------------------------------------------

fn with_plan<T>(prefabs_dir: &Path, f: impl FnOnce(&Plan) -> T) -> T {
    let loaded = delvewright_compiler::load::load_campaign_dir(&fixture_dir()).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("valid campaign parses");
    let prefabs = PrefabRegistry::load_dir(prefabs_dir).unwrap();
    assert_eq!(
        campaign.world.content.areas.len(),
        2,
        "fixture drift: a one-area campaign has no crossing, and this file would \
         prove nothing"
    );
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    f(&plan)
}

/// Build, and hand back whatever came of it — a datapack or the diagnostic that
/// stopped it.
fn build_with(prefabs_dir: &Path) -> Result<BuildOutput, BuildFailure> {
    let loaded = delvewright_compiler::load::load_campaign_dir(&fixture_dir()).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("valid campaign parses");
    let prefabs = PrefabRegistry::load_dir(prefabs_dir).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(prefabs_dir.join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
}

/// The `transport` cell the bolt branch's crossing promises, read off the
/// exported branch path — the artifact the harness actually walks.
fn bolt_transport(out: &BuildOutput) -> Option<Value> {
    let raw = out.get("validation/branch-path-bolt.json")?;
    let doc: Value = serde_json::from_slice(raw).ok()?;
    doc["steps"]
        .as_array()?
        .iter()
        .find(|s| s["objective"] == "obj/bolt")
        .and_then(|s| s.get("transport").cloned())
}

// ---------------------------------------------------------------------------
// §4.1 — a declared role resolves
// ---------------------------------------------------------------------------

/// **The control.** With the library as shipped the crossing is found, so
/// "still found" below is a claim about the role and not about a fixture that
/// never crossed.
#[test]
fn the_shipped_library_promises_the_crossing() {
    let out = build_with(&common::prefabs_dir()).expect("the shipped library builds");
    assert_eq!(
        bolt_transport(&out),
        Some(serde_json::json!(LANDING_ENTRY)),
        "fixture drift: the bolt branch must cross into area/landing at obj/bolt"
    );
}

/// spec-0046 §4.1: a zone whose `mark` declares `role: entry` exports metadata
/// carrying that role, and a campaign binding it resolves the entry point — the
/// same cell, reached through a key no name list could ever match.
#[test]
fn an_anchor_declaring_the_role_is_the_entry_point() {
    let (dir, key) = library_declaring_entry("resolves", true);

    let (entry, named) = with_plan(&dir, |p| {
        (
            p.entry_point("area/landing"),
            p.anchors
                .role_name("area/landing", AnchorRole::Entry)
                .map(str::to_string),
        )
    });
    assert_eq!(
        entry,
        Some(LANDING_ENTRY),
        "the declared anchor is the area's entry point"
    );
    assert_eq!(named.as_deref(), Some(key.as_str()));

    let out = build_with(&dir).expect("a campaign whose entry is declared by role builds");
    assert_eq!(
        bolt_transport(&out),
        Some(serde_json::json!(LANDING_ENTRY)),
        "an area whose entry anchor is DECLARED must still be transported into"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// §4.2 — without the role, the same library fails, naming what is missing
// ---------------------------------------------------------------------------

/// spec-0046 §4.2: the same pieces, the same keys, the role removed. Nothing
/// resolves an entry anchor any more, and the build says so under its own code
/// rather than by any other route — the message names the role first and the
/// two fallback spellings as what they are.
#[test]
fn without_the_role_the_same_library_is_refused_by_code() {
    let (dir, _) = library_declaring_entry("refused", false);
    let err = build_with(&dir).expect_err("no area resolves an entry anchor");
    let BuildFailure::Diagnostic { code, message } = err else {
        panic!("the refusal must be a diagnostic, not an internal error: {err:?}");
    };
    assert_eq!(code.id(), "DW0345", "{message}");
    assert!(
        message.contains(r#""role": "entry""#),
        "the refusal names the declaration that is missing: {message}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// §4.3 — one resolver, and nothing outside it asks the question
// ---------------------------------------------------------------------------

/// spec-0046 §4.3, asserted over the resolver every consumer is required to
/// call rather than per call site: a fourth consumer written tomorrow inherits
/// this or it inherits nothing.
///
/// Both directions of the resolution are exercised on ONE plan: the by-area
/// lookup (transport, the POV planner, `setworldspawn`, the class teleport, the
/// first-join placement), the by-name lookup (the gate-deadlock proof's start
/// node), and the sweep (the trap-safety start set).
#[test]
fn every_shape_of_the_question_goes_through_the_one_resolver() {
    let (dir, key) = library_declaring_entry("resolver", true);
    with_plan(&dir, |p| {
        assert_eq!(p.entry_point("area/landing"), Some(LANDING_ENTRY));
        assert_eq!(
            p.entry_point_facing("area/landing").map(|(pos, _)| pos),
            Some(LANDING_ENTRY)
        );
        assert_eq!(
            p.anchors.entry_anchor_name("area/landing").as_deref(),
            Some(key.as_str())
        );
        let starts: Vec<_> = p.entry_points().collect();
        assert!(
            starts.contains(&LANDING_ENTRY) && starts.len() == 2,
            "one start per area, both declared by role: {starts:?}"
        );
    });
    let _ = std::fs::remove_dir_all(&dir);
}

/// The other half of §4.3, and the half a call-site assertion cannot make: **no
/// source file outside the resolver matches an entry-anchor name.**
///
/// Three consumers once did — inter-area transport, the POV shot planner and
/// the trap-safety start set — and each asked an honest question about the wrong
/// key and got an honest `None`. Nothing errored, so nothing was caught until an
/// island-tileset area was walked by hand.
///
/// The exemptions below are the occurrences that are not about anchor names at
/// all, each named with its reason. The table may only shrink: an entry that no
/// longer matches is a failure, so a fixed exemption cannot rot into a permanent
/// one.
#[test]
fn no_source_file_outside_the_resolver_matches_an_entry_anchor_name() {
    /// `(file, needle, why)` — a line containing `needle` in `file` is not an
    /// anchor-name match.
    const EXEMPT: &[(&str, &str, &str)] = &[
        (
            "plan.rs",
            "pub const ENTRY_ANCHOR_NAMES",
            "the fallback list itself — this is the resolver",
        ),
        (
            "solver.rs",
            "m.role == \"entry\"",
            "a POOL member's layout role (which prefab seeds the jigsaw layout), \
             which is a different vocabulary from an anchor's",
        ),
        (
            "viewer.rs",
            "pub const WAY_IN_STEMS",
            "the prefab viewer's opening-camera GUESS over a single piece, \
             consulted only when the piece declares no role; it has no area and \
             no plan to ask",
        ),
        // Both needles are the RUST spelling of the field, because that is what
        // the sweep reads. Written in the JSON spelling (`"id": "spawn",`) they
        // matched nothing, which is not a silence: an exemption that matches
        // nothing exempts nothing, so the two lines below reported as findings
        // and the sweep failed against the very source it was written for.
        (
            "render_plan.rs",
            "id: \"spawn\".to_string(),",
            "a review SHOT's id, not an anchor name",
        ),
        (
            "render_plan.rs",
            "kind: \"spawn\",",
            "a review shot's kind, not an anchor name",
        ),
        (
            "index.rs",
            "\"spawn\"",
            "shot ids in the view index's own fixtures",
        ),
        // Arrived with the derived-map slice, which this sweep predates: the
        // gym writes stage documents, and a layout graph's `entry` says which
        // NODE a map is entered at. That is a question about the graph, and it
        // is answered before any anchor exists — a map has one entry node
        // whether or not anything is ever placed in it.
        (
            "gym.rs",
            "\"entry\": entry_node,",
            "the LAYOUT GRAPH's `entry` field — which node the map is entered \
             at — which is a different vocabulary from an anchor's name",
        ),
    ];

    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect_rs(&src, &mut files);
    assert!(
        files.len() > 20,
        "the sweep found {} source file(s) — it is not looking at the crate",
        files.len()
    );

    let mut used: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    let mut findings = Vec::new();
    let mut examined = 0usize;
    for file in &files {
        let name = file.file_name().unwrap().to_str().unwrap().to_string();
        for (n, line) in std::fs::read_to_string(file).unwrap().lines().enumerate() {
            let hits = plan::ENTRY_ANCHOR_NAMES
                .iter()
                .filter(|needle| line.contains(&format!("\"{needle}\"")))
                .count();
            if hits == 0 {
                continue;
            }
            examined += 1;
            match EXEMPT
                .iter()
                .find(|(f, needle, _)| *f == name && line.contains(needle))
            {
                Some((f, needle, _)) => *used.entry((f, needle)).or_default() += 1,
                None => findings.push(format!("{name}:{}: {}", n + 1, line.trim())),
            }
        }
    }
    assert!(
        findings.is_empty(),
        "a source line outside the resolver spells an entry-anchor name. Resolve \
         the entry point through `Plan::entry_point` / `Plan::entry_points` / \
         `AnchorTable::entry_anchor_name`, or — if this line is not about anchor \
         names — add it to EXEMPT with its reason:\n  {}",
        findings.join("\n  ")
    );
    assert!(
        examined > 0,
        "the sweep matched ZERO lines: it has stopped measuring what it was \
         written for"
    );
    let stale: Vec<_> = EXEMPT
        .iter()
        .filter(|(f, needle, _)| !used.contains_key(&(*f, *needle)))
        .map(|(f, needle, _)| format!("{f}: {needle}"))
        .collect();
    assert!(
        stale.is_empty(),
        "EXEMPT names line(s) that no longer exist — this table may only shrink:\n  {}",
        stale.join("\n  ")
    );
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_rs(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

// ---------------------------------------------------------------------------
// §4.4 — two entries in one area is a refusal
// ---------------------------------------------------------------------------

/// spec-0046 §4.4: an area has one place the party arrives at, and two claims to
/// it is `DW0804` rather than a silent first-wins. The two anchors are in ONE
/// area, which is what the criterion's own vacuity note demands.
#[test]
fn two_declared_entries_in_one_area_are_refused() {
    let dir = library_with_roles_on(
        "same-area",
        &[
            ("cave-shore.json", "spawn"),
            ("cave-shore.json", "anchor/exit"),
        ],
    );
    let loaded = delvewright_compiler::load::load_campaign_dir(&fixture_dir()).unwrap();
    let campaign = parse_campaign(&loaded.raw).unwrap();
    let prefabs = PrefabRegistry::load_dir(&dir).unwrap();
    let Err(err) = Plan::build(&campaign, &prefabs) else {
        panic!("one area declaring two entries must not plan");
    };
    assert_eq!(err.failure.code.id(), "DW0804", "{}", err.failure.message);
    assert!(
        err.failure.message.contains("area/landing")
            && err.failure.message.contains("spawn")
            && err.failure.message.contains("anchor/exit"),
        "the refusal names the area and BOTH claimants: {}",
        err.failure.message
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The control that says `DW0804` is about an AREA and not about a campaign: the
/// same two declarations, one per area, is what every campaign with two areas
/// looks like and it builds.
#[test]
fn one_declared_entry_per_area_is_the_ordinary_case() {
    let dir = library_with_roles_on(
        "per-area",
        &[("cave-shore.json", "spawn"), ("hello-room.json", "spawn")],
    );
    let out = build_with(&dir).expect("one entry per area is not a conflict");
    assert_eq!(bolt_transport(&out), Some(serde_json::json!(LANDING_ENTRY)));
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// the fallback stays a fallback
// ---------------------------------------------------------------------------

/// A piece that declares a role is never reached by a name — which is what makes
/// the compatibility list safe to keep. Here `spawn` keeps its cell and
/// `anchor/exit` is declared the entry: the role wins, and the party arrives at
/// the declared cell rather than at the one that is merely spelled right.
#[test]
fn a_declared_role_outranks_a_spelled_name() {
    let dir = library_with_roles_on("outranks", &[("cave-shore.json", "anchor/exit")]);
    let (entry, exit) = with_plan(&dir, |p| {
        (
            p.entry_point("area/landing"),
            p.point("area/landing", "anchor/exit"),
        )
    });
    assert_eq!(entry, exit, "the declared anchor is the entry point");
    assert_ne!(
        entry,
        Some(LANDING_ENTRY),
        "and the anchor merely NAMED `spawn` is not"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
