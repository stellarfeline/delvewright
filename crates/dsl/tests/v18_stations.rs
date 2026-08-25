//! DSL v0.18 (spec-0052): `nodes[].stations[]` — the named places inside a
//! place, and the campaign vocabulary they join.
//!
//! # How these tests are kept falsifiable
//!
//! One green document, and **every red is a one-field perturbation of it**, so
//! each assertion is half of a red→green pair rather than a document written to
//! fail. If a check stopped working its perturbation would go green and the test
//! would say so — which is the property the file's own
//! `perturbing_the_namespace_rule_to_the_vacuous_shape_goes_red` and
//! `perturbing_the_uniqueness_scope_to_a_constant_goes_red` exist to demonstrate
//! directly, by asserting that the SAFETY of each rule is what the green depends
//! on.
//!
//! Nothing here calls `synthesized_anchors`, `owed_anchors` or
//! `synthesized_anchor_kinds` to decide what to assert: the names each place
//! owes are read off the graph written below by hand.

mod common;

use delvewright_dsl::{RawCampaign, check_campaign};
use serde_json::{Value, json};

/// The graph the tests perturb: the hello-world map, at the version stations
/// need, with three declared places inside its places.
///
/// * `node/porch` declares **`anchor/fire-pit`** and **`anchor/kit-shelf`**,
///   two points — the shape every seat, subject and volume centre resolves as.
/// * `node/hall` declares **`anchor/vestry-door`**, a gate — a region that
///   seals and clears, which is what `open-gate` and `close-gate` address.
///
/// Written by hand rather than generated, so that "the porch owes three names"
/// is a fact read off this text and not one the checker computed for itself.
const GRAPH: &str = r#"{
  "campaign_id": "hello-world",
  "dsl_version": "0.18.0",
  "stage": "layout-graph",
  "content": {
    "nodes": [
      { "id": "node/porch", "intent": "threshold", "size_class": "alcove",
        "stations": [
          { "anchor": "anchor/fire-pit", "kind": "point",
            "note": "The camp's fire, which the story names and the box centre is not." },
          { "anchor": "anchor/kit-shelf", "kind": "point" }
        ] },
      { "id": "node/hall", "intent": "hub", "size_class": "room",
        "stations": [
          { "anchor": "anchor/vestry-door", "kind": "gate" }
        ] },
      { "id": "node/vault", "intent": "goal-chamber", "size_class": "alcove" },
      { "id": "node/cellar", "intent": "cache", "size_class": "room" },
      { "id": "node/yard", "intent": "vista", "size_class": "hall" },
      { "id": "node/pit", "intent": "sump", "size_class": "alcove" }
    ],
    "edges": [
      { "id": "edge/porch-hall", "class": "walk", "a": "node/porch", "b": "node/hall" },
      { "id": "edge/hall-vault", "class": "barred", "a": "node/hall", "b": "node/vault",
        "opens_from": "a", "gating": { "quest": "quest/open-the-door" } },
      { "id": "edge/hall-cellar", "class": "stair", "a": "node/hall", "b": "node/cellar",
        "shortcut": true },
      { "id": "edge/porch-cellar", "class": "drop", "a": "node/porch", "b": "node/cellar",
        "falls": "a-to-b" },
      { "id": "edge/vault-yard", "class": "walk", "a": "node/vault", "b": "node/yard" },
      { "id": "edge/yard-pit", "class": "drop", "a": "node/yard", "b": "node/pit",
        "falls": "a-to-b" },
      { "id": "edge/pit-yard", "class": "stair", "a": "node/pit", "b": "node/yard" },
      { "id": "edge/porch-vault-sightline", "class": "vision", "a": "node/porch",
        "b": "node/vault" }
    ],
    "entry": "node/porch",
    "goal": "node/vault",
    "critical_path": ["node/porch", "node/hall", "node/vault"],
    "beats": [
      { "quest": "quest/open-the-door", "objective": "obj/talk", "node": "node/porch" },
      { "quest": "quest/open-the-door", "objective": "obj/exit", "node": "node/hall" }
    ]
  }
}"#;

const BRIEF: &str = r#"{
  "campaign_id": "hello-world",
  "dsl_version": "0.13.0",
  "stage": "geometry-brief",
  "content": {
    "facts": [
      { "id": "fact/region-span", "value": 64.0, "unit": "blocks",
        "note": "The site is sixty-four blocks across." }
    ]
  }
}"#;

/// A minimal plan: what makes a campaign a site-plan campaign at all, which is
/// what puts the derived vocabulary in front of the checks.
const PLAN: &str = r#"{
  "campaign_id": "hello-world",
  "dsl_version": "0.14.0",
  "stage": "site-plan",
  "content": {
    "region": { "min": [0, 60, 0], "extent": [64, 24, 64] },
    "datums": [ { "id": "datum/grade", "y": 64 } ],
    "boxes": [
      { "node": "node/porch",  "min": [0, 0],  "extent": [8, 8],
        "floor": { "datum": "datum/grade" }, "ceiling": { "clearance": 4 } },
      { "node": "node/hall",   "min": [12, 0], "extent": [16, 16],
        "floor": { "datum": "datum/grade" }, "ceiling": { "clearance": 8 } },
      { "node": "node/vault",  "min": [32, 0], "extent": [8, 8],
        "floor": { "datum": "datum/grade" }, "ceiling": { "clearance": 4 } },
      { "node": "node/cellar", "min": [12, 20], "extent": [8, 8],
        "floor": { "y": 56 }, "ceiling": { "clearance": 4 } },
      { "node": "node/yard",   "min": [44, 0], "extent": [16, 16],
        "floor": { "datum": "datum/grade" }, "ceiling": "open" },
      { "node": "node/pit",    "min": [44, 20], "extent": [8, 8],
        "floor": { "y": 56 }, "ceiling": { "clearance": 4 } }
    ],
    "seams": []
  }
}"#;

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

/// The hello-world campaign as a **site-plan** campaign, with its content bound
/// to the derived vocabulary — the same four substitutions
/// `v14_site_plan.rs` makes, for the same reasons.
fn campaign(graph: String) -> RawCampaign {
    let base = common::valid_raw();
    let retarget = |doc: &str, edits: &[(&str, &str)]| -> String {
        let mut out = doc.to_string();
        for (from, to) in edits {
            out = out.replace(from, to);
        }
        out
    };
    let mut world: Value = serde_json::from_str(&base.world).expect("hello-world's world parses");
    world["content"]["areas"] = json!([]);
    RawCampaign {
        world: serde_json::to_string(&world).expect("re-serialize"),
        npcs: retarget(
            &base.npcs,
            &[
                ("\"area/keep\"", "\"area/site\""),
                ("\"anchor/keeper-stand\"", "\"anchor/node-porch\""),
            ],
        ),
        quest_plan: retarget(&base.quest_plan, &[("\"area/keep\"", "\"area/site\"")]),
        quests: retarget(
            &base.quests,
            &[
                ("\"anchor/exit\"", "\"anchor/node-hall\""),
                ("\"anchor/door\"", "\"anchor/seam-hall-vault\""),
            ],
        ),
        site_plan: Some(PLAN.to_string()),
        detail_plan: None,
        layout_graph: Some(graph),
        geometry_brief: Some(BRIEF.to_string()),
        ..base
    }
}

/// Validate the green campaign with the GRAPH perturbed by one edit.
fn graph_with(patch: impl FnOnce(&mut Value)) -> Vec<delvewright_dsl::Diagnostic> {
    let mut v: Value = serde_json::from_str(GRAPH).expect("the green graph parses");
    patch(&mut v);
    check_campaign(&campaign(serde_json::to_string(&v).expect("re-serialize")))
}

fn codes(d: &[delvewright_dsl::Diagnostic]) -> Vec<&str> {
    d.iter().map(|x| x.code.as_str()).collect()
}

/// The node at `i` in the graph value, for a perturbation to reach into.
fn node(v: &mut Value, i: usize) -> &mut Value {
    &mut v["content"]["nodes"][i]
}

// ---------------------------------------------------------------------------
// The green document, and what it establishes
// ---------------------------------------------------------------------------

/// **The green.** Every red below is one edit away from this, so a check that
/// stopped working would turn its own perturbation green.
///
/// Binding: 3 stations declared over 6 nodes, of which 1 gate.
#[test]
fn the_green_graph_declares_three_stations_and_validates() {
    let d = graph_with(|_| {});
    let stationy: Vec<_> = d
        .iter()
        .filter(|x| matches!(x.code.as_str(), "DW0869" | "DW0870" | "DW0871" | "DW0141"))
        .collect();
    assert!(
        stationy.is_empty(),
        "the green graph must raise no station refusal: {stationy:#?}"
    );
}

/// The declared names join the campaign vocabulary at the same authority as
/// every synthesized one — which is the whole decision (spec-0052 §2).
///
/// Binding: 3 declared names checked against the vocabulary, plus the 9
/// synthesized ones this graph produces (`spawn`, six `anchor/node-…`, and the
/// one barred way's `anchor/seam-…` and `anchor/unlock-…`).
#[test]
fn a_declared_station_is_in_the_campaign_vocabulary() {
    let c = delvewright_dsl::parse_campaign(&campaign(GRAPH.to_string()))
        .expect("the green campaign parses");
    let vocab = delvewright_dsl::synthesized_anchors(&c);
    for name in ["anchor/fire-pit", "anchor/kit-shelf", "anchor/vestry-door"] {
        assert!(vocab.contains(name), "`{name}` must be nameable: {vocab:?}");
    }
    // The synthesized set is still exactly itself beside them.
    for name in ["spawn", "anchor/node-porch", "anchor/seam-hall-vault"] {
        assert!(
            vocab.contains(name),
            "declaring a station must not displace `{name}`: {vocab:?}"
        );
    }
    // 9 synthesized, counted off the graph above: `spawn`, six `anchor/node-…`
    // (porch, hall, vault, cellar, yard, pit), and the one `barred` edge's
    // `anchor/seam-hall-vault` plus `anchor/unlock-hall-vault` — it opens from
    // `a`, so it owes an unlock side. Plus the 3 declared.
    assert_eq!(
        vocab.len(),
        12,
        "9 synthesized + 3 declared, counted off the graph by hand: {vocab:?}"
    );
}

/// **The owed set grows upstream** (spec-0052 §6): a node's stations join the
/// names its piece must answer for, and no other node's.
///
/// Binding: 2 nodes examined — the one that declares stations and one that does
/// not, so the assertion is not one a rule that owed everything to everybody
/// would also pass.
#[test]
fn a_node_owes_its_own_stations_and_no_others() {
    let c = delvewright_dsl::parse_campaign(&campaign(GRAPH.to_string()))
        .expect("the green campaign parses");
    let porch = delvewright_dsl::owed_anchors(&c, &delvewright_dsl::NodeId("node/porch".into()));
    assert!(porch.contains("anchor/fire-pit"), "{porch:?}");
    assert!(porch.contains("anchor/kit-shelf"), "{porch:?}");
    assert!(
        !porch.contains("anchor/vestry-door"),
        "the porch must not owe the hall's station: {porch:?}"
    );
    let vault = delvewright_dsl::owed_anchors(&c, &delvewright_dsl::NodeId("node/vault".into()));
    assert!(
        !vault.iter().any(|n| n.starts_with("anchor/fire")),
        "a node declaring no station owes none: {vault:?}"
    );
}

// ---------------------------------------------------------------------------
// §7.1 — DW0869: a station in the engine's namespace
// ---------------------------------------------------------------------------

/// The spec's own trip: a node declares `anchor/seam-vestry-door`, refused
/// **even though no such edge exists**, because the prefix is the rule.
#[test]
fn dw0869_a_station_named_in_the_derived_namespace_is_refused() {
    let d = graph_with(|v| {
        node(v, 0)["stations"][0]["anchor"] = json!("anchor/seam-vestry-door");
    });
    assert!(
        codes(&d).contains(&"DW0869"),
        "a reserved prefix must be refused: {:?}",
        codes(&d)
    );
    let msg = &d.iter().find(|x| x.code == "DW0869").unwrap().message;
    assert!(
        msg.contains("anchor/seam-"),
        "the refusal must name the prefix it refused: {msg}"
    );
}

/// All three prefixes and the entry name, so the rule is not one that happens to
/// catch the single case a test author thought of.
///
/// Binding: 4 reserved spellings, 4 of 4 refused.
#[test]
fn dw0869_covers_every_reserved_spelling() {
    let mut refused = 0;
    let reserved = [
        "anchor/node-anything",
        "anchor/seam-anything",
        "anchor/unlock-anything",
        "spawn",
    ];
    for name in reserved {
        let d = graph_with(|v| {
            node(v, 0)["stations"][0]["anchor"] = json!(name);
        });
        assert!(
            codes(&d).contains(&"DW0869"),
            "`{name}` must be refused: {:?}",
            codes(&d)
        );
        refused += 1;
    }
    assert_eq!(
        refused,
        reserved.len(),
        "every reserved spelling must be refused, not merely one"
    );
}

// ---------------------------------------------------------------------------
// §7.2 — DW0870: two claims on one name
// ---------------------------------------------------------------------------

/// The spec's own trip: two nodes each declare `anchor/fire-pit`. The refusal
/// names both nodes and states the scope.
#[test]
fn dw0870_two_nodes_claiming_one_name_is_refused() {
    let d = graph_with(|v| {
        node(v, 1)["stations"][0]["anchor"] = json!("anchor/fire-pit");
        node(v, 1)["stations"][0]["kind"] = json!("point");
    });
    let hit = d.iter().find(|x| x.code == "DW0870");
    assert!(hit.is_some(), "expected DW0870: {:?}", codes(&d));
    let msg = &hit.unwrap().message;
    assert!(
        msg.contains("node/porch"),
        "must name the first node: {msg}"
    );
    assert!(
        msg.contains("node/hall"),
        "must name the second node: {msg}"
    );
    assert!(
        msg.contains("AREA"),
        "must state the scope of uniqueness: {msg}"
    );
}

/// The other half of §7.2: one node declaring the name twice.
#[test]
fn dw0870_one_node_claiming_a_name_twice_is_refused() {
    let d = graph_with(|v| {
        node(v, 0)["stations"][1]["anchor"] = json!("anchor/fire-pit");
    });
    assert!(codes(&d).contains(&"DW0870"), "{:?}", codes(&d));
}

// ---------------------------------------------------------------------------
// §7.3 — DW0871: a reference of the wrong shape
// ---------------------------------------------------------------------------

/// A gate verb naming a **point** station. This is the direction the campaign
/// meets: `open-gate` addresses a region that seals and clears, and a seat is
/// not one.
#[test]
fn dw0871_a_gate_verb_naming_a_point_station_is_refused() {
    let mut base = campaign(GRAPH.to_string());
    base.quests = base
        .quests
        .replace("\"anchor/seam-hall-vault\"", "\"anchor/fire-pit\"");
    let d = check_campaign(&base);
    let hit = d.iter().find(|x| x.code == "DW0871");
    assert!(hit.is_some(), "expected DW0871: {:?}", codes(&d));
    let msg = &hit.unwrap().message;
    assert!(
        msg.contains("point") && msg.contains("gate"),
        "must name both the declared shape and the demanded one: {msg}"
    );
}

/// The other direction, and **the one nothing in this engine could report
/// before**: a point consumer handed a gate. `point_any_in` silently returns the
/// gate region's low corner, so a body was seated in a wall and no check said so.
#[test]
fn dw0871_a_point_consumer_naming_a_gate_station_is_refused() {
    let mut base = campaign(GRAPH.to_string());
    base.npcs = base
        .npcs
        .replace("\"anchor/node-porch\"", "\"anchor/vestry-door\"");
    let d = check_campaign(&base);
    assert!(codes(&d).contains(&"DW0871"), "{:?}", codes(&d));
}

/// The remedy `DW0871` prescribes must be **reachable**: it tells the author to
/// change the station's `kind`, so doing exactly that must go green.
#[test]
fn dw0871s_remedy_is_reachable() {
    let mut base = campaign(GRAPH.to_string());
    base.npcs = base
        .npcs
        .replace("\"anchor/node-porch\"", "\"anchor/vestry-door\"");
    assert!(
        codes(&check_campaign(&base)).contains(&"DW0871"),
        "precondition: the wrong shape is refused"
    );
    // Now perform the remedy the message names.
    let mut v: Value = serde_json::from_str(GRAPH).expect("parses");
    v["content"]["nodes"][1]["stations"][0]["kind"] = json!("point");
    base.layout_graph = Some(serde_json::to_string(&v).expect("re-serialize"));
    let after = check_campaign(&base);
    assert!(
        !codes(&after).contains(&"DW0871"),
        "performing the prescribed remedy must clear the refusal: {:?}",
        codes(&after)
    );
}

// ---------------------------------------------------------------------------
// §7.6 — the per-stage fence
// ---------------------------------------------------------------------------

/// A graph declaring `stations[]` below `STATIONS_SINCE` is refused, and the
/// refusal names the version to raise it to.
#[test]
fn the_fence_refuses_stations_below_the_version() {
    let d = graph_with(|v| {
        v["dsl_version"] = json!("0.17.0");
    });
    let hit = d.iter().find(|x| x.code == "DW0141");
    assert!(hit.is_some(), "expected the fence: {:?}", codes(&d));
    assert!(
        hit.unwrap()
            .message
            .contains(delvewright_dsl::STATIONS_SINCE),
        "the fence must name the version to raise to: {}",
        hit.unwrap().message
    );
}

/// The other direction, and the half that makes the fence a fence rather than a
/// blanket refusal: **a graph below the version that declares no station is
/// untouched**.
#[test]
fn the_fence_leaves_a_graph_with_no_station_alone() {
    let d = graph_with(|v| {
        v["dsl_version"] = json!("0.13.0");
        for i in 0..6 {
            if let Some(o) = v["content"]["nodes"][i].as_object_mut() {
                o.remove("stations");
            }
        }
    });
    assert!(
        !codes(&d).contains(&"DW0141"),
        "a graph declaring no station must not meet the fence: {:?}",
        codes(&d)
    );
}

// ---------------------------------------------------------------------------
// The perturbations: is each rule's SAFETY what the green depends on?
// ---------------------------------------------------------------------------

/// **Perturb the namespace rule toward the vacuous shape.**
///
/// `DW0869` is only a rule because it is CONDITIONAL on the name. A version of
/// it that fired unconditionally would refuse the green document — which is what
/// this asserts, from the outside: the green graph's three perfectly legal
/// station names must NOT be in the reserved set, so a rule that answered "yes"
/// for everything would be visible immediately.
///
/// Binding: 3 legal names, 0 of 3 refused; 4 reserved names, 4 of 4 refused
/// (`dw0869_covers_every_reserved_spelling`). A rule made unconditional moves
/// the first number to 3 and this test goes red.
#[test]
fn perturbing_the_namespace_rule_to_the_vacuous_shape_goes_red() {
    let d = graph_with(|_| {});
    let refused: Vec<_> = d.iter().filter(|x| x.code == "DW0869").collect();
    assert!(
        refused.is_empty(),
        "the namespace rule must be conditional on the NAME: made unconditional it \
         refuses these legal stations, which is what this test would then report: {refused:#?}"
    );
}

/// **Perturb the uniqueness scope to a constant.**
///
/// `DW0870` compares each station name against the names already seen. A version
/// that compared against a constant — or that reported every station as a
/// duplicate of itself — would refuse the green document, whose three station
/// names are pairwise distinct by inspection.
///
/// Binding: 3 distinct names, 0 duplicate refusals; and
/// `dw0870_two_nodes_claiming_one_name_is_refused` is the same rule's other
/// side, so the pair pins it from both directions.
#[test]
fn perturbing_the_uniqueness_scope_to_a_constant_goes_red() {
    let d = graph_with(|_| {});
    let refused: Vec<_> = d.iter().filter(|x| x.code == "DW0870").collect();
    assert!(
        refused.is_empty(),
        "the uniqueness rule must compare a name against the OTHER names: a constant \
         scope refuses these three distinct stations: {refused:#?}"
    );
}

/// A station **no quest references is legal** (spec-0052 §4, "not refused,
/// deliberately") — the mid-authoring state.
///
/// `anchor/kit-shelf` is named by nothing in the green campaign, and the green
/// document validates, so this is already true of every test above; it is
/// asserted by name because a later round tightening "unused station" into a
/// refusal would break the graph-before-mission order the pipeline exists to
/// make structural.
#[test]
fn a_station_no_quest_references_is_legal() {
    let d = graph_with(|_| {});
    assert!(
        !d.iter().any(|x| x.message.contains("anchor/kit-shelf")),
        "an unreferenced station must draw no diagnostic: {:#?}",
        d.iter()
            .filter(|x| x.message.contains("anchor/kit-shelf"))
            .collect::<Vec<_>>()
    );
}
