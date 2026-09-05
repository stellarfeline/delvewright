//! DSL v0.19 (spec-0053): a place that is a **route**, and a hand-off that is
//! not a **door**.
//!
//! # The motivating shape
//!
//! The campaign brief this vocabulary was written for says *"one cut ledge, one
//! body wide, climbing across the whole seaward face"*. `node/cliff-road` below
//! is that ledge: **4 by 72**, declared `way_class: "road"`. Every rung of the
//! size-class ladder refuses it and no calibration of the ladder could admit it
//! — for a rung to span 4..72 on an axis, an alcove and an expanse would have to
//! be the same thing. `the_old_way_of_stating_it_is_refused_with_a_reachable_remedy`
//! is the other half of that pair: the same ledge stated as a `size_class`, and
//! what the engine tells the author to write instead.
//!
//! # How these tests are kept falsifiable
//!
//! One green document, and **every red is a one-field perturbation of it**, so
//! each assertion is half of a red→green pair rather than a document written to
//! fail. Two tests go further and assert from outside that the green depends on
//! each rule's SAFETY: made vacuous, the rule refuses the green document and the
//! test reports it.

mod common;

use delvewright_dsl::{RawCampaign, check_campaign};
use serde_json::{Value, json};

/// The graph the tests perturb.
///
/// * **`node/cliff-road`** — the motivating shape. A way, `road`, 4 by 72.
/// * **`node/duct`** — a `corridor`, 4 by 8: the narrow way, at the one width
///   the kit quantum lets a plan draw it.
/// * The rest are ordinary size-classed places, so that the two vocabularies are
///   exercised side by side in one document rather than in two.
const GRAPH: &str = r#"{
  "campaign_id": "hello-world",
  "dsl_version": "0.19.0",
  "stage": "layout-graph",
  "content": {
    "nodes": [
      { "id": "node/porch", "intent": "threshold", "size_class": "alcove" },
      { "id": "node/cliff-road", "intent": "approach", "way_class": "road",
        "note": "One cut ledge, one body wide, climbing across the whole seaward face." },
      { "id": "node/hall", "intent": "hub", "size_class": "room" },
      { "id": "node/duct", "intent": "crawl", "way_class": "corridor" },
      { "id": "node/vault", "intent": "goal-chamber", "size_class": "alcove" },
      { "id": "node/court", "intent": "vista", "size_class": "hall" }
    ],
    "edges": [
      { "id": "edge/porch-road", "class": "walk", "a": "node/porch", "b": "node/cliff-road" },
      { "id": "edge/road-hall", "class": "walk", "a": "node/cliff-road", "b": "node/hall" },
      { "id": "edge/hall-duct", "class": "walk", "a": "node/hall", "b": "node/duct" },
      { "id": "edge/duct-vault", "class": "walk", "a": "node/duct", "b": "node/vault" },
      { "id": "edge/court-hall", "class": "walk", "a": "node/court", "b": "node/hall" }
    ],
    "entry": "node/porch",
    "goal": "node/vault",
    "critical_path": ["node/porch", "node/cliff-road", "node/hall", "node/duct", "node/vault"],
    "beats": [
      { "quest": "quest/open-the-door", "objective": "obj/talk", "node": "node/porch" },
      { "quest": "quest/open-the-door", "objective": "obj/exit", "node": "node/hall" }
    ]
  }
}"#;

const BRIEF: &str = r#"{
  "campaign_id": "hello-world",
  "dsl_version": "0.19.0",
  "stage": "geometry-brief",
  "content": {
    "facts": [
      { "id": "fact/region-span", "value": 128.0, "unit": "blocks",
        "note": "The site is a hundred and twenty-eight blocks across." }
    ]
  }
}"#;

/// The plan the tests perturb.
///
/// `node/court` meets `node/hall` along a **contact** — a front 16 cells wide,
/// which is wider than the broadest standard opening (`opening.gateway`, 5) and
/// therefore could not have been a portal. Every other seam is an ordinary
/// portal, so both kinds are resolved, derived and measured in one plan.
const PLAN: &str = r#"{
  "campaign_id": "hello-world",
  "dsl_version": "0.19.0",
  "stage": "site-plan",
  "content": {
    "region": { "min": [0, 56, 0], "extent": [128, 32, 128] },
    "datums": [ { "id": "datum/grade", "y": 64 } ],
    "boxes": [
      { "node": "node/porch", "min": [0, 0], "extent": [8, 8],
        "floor": { "datum": "datum/grade" }, "ceiling": { "clearance": 4 } },
      { "node": "node/cliff-road", "min": [9, 0], "extent": [4, 72],
        "floor": { "datum": "datum/grade" }, "ceiling": { "clearance": 8 } },
      { "node": "node/hall", "min": [14, 0], "extent": [16, 16],
        "floor": { "datum": "datum/grade" }, "ceiling": { "clearance": 8 } },
      { "node": "node/duct", "min": [31, 0], "extent": [4, 8],
        "floor": { "datum": "datum/grade" }, "ceiling": { "clearance": 4 } },
      { "node": "node/vault", "min": [36, 0], "extent": [8, 8],
        "floor": { "datum": "datum/grade" }, "ceiling": { "clearance": 4 } },
      { "node": "node/court", "min": [14, 17], "extent": [16, 16],
        "floor": { "datum": "datum/grade" }, "ceiling": { "clearance": 8 } }
    ],
    "seams": [
      { "edge": "edge/porch-road", "face": "east", "at": [1, 64], "opening": "arch" },
      { "edge": "edge/road-hall", "face": "east", "at": [1, 64], "opening": "arch" },
      { "edge": "edge/hall-duct", "face": "east", "at": [1, 64], "opening": "arch" },
      { "edge": "edge/duct-vault", "face": "east", "at": [1, 64], "opening": "arch" },
      { "edge": "edge/court-hall", "face": "north", "at": [14, 64], "contact": {} }
    ]
  }
}"#;

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

/// The hello-world campaign as a **site-plan** campaign, with its content bound
/// to the derived vocabulary — the same substitutions `v18_stations.rs` makes,
/// for the same reasons.
fn campaign(graph: String, plan: String) -> RawCampaign {
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
                ("\"anchor/door\"", "\"anchor/node-vault\""),
            ],
        ),
        site_plan: Some(plan),
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
    check_campaign(&campaign(
        serde_json::to_string(&v).expect("re-serialize"),
        PLAN.to_string(),
    ))
}

/// Validate the green campaign with the PLAN perturbed by one edit.
fn plan_with(patch: impl FnOnce(&mut Value)) -> Vec<delvewright_dsl::Diagnostic> {
    let mut v: Value = serde_json::from_str(PLAN).expect("the green plan parses");
    patch(&mut v);
    check_campaign(&campaign(
        GRAPH.to_string(),
        serde_json::to_string(&v).expect("re-serialize"),
    ))
}

fn codes(d: &[delvewright_dsl::Diagnostic]) -> Vec<&str> {
    d.iter().map(|x| x.code.as_str()).collect()
}

/// Every diagnostic carrying `code`, so a test can quote the message it asserts
/// on rather than asserting on a count.
fn with_code<'a>(
    d: &'a [delvewright_dsl::Diagnostic],
    code: &str,
) -> Vec<&'a delvewright_dsl::Diagnostic> {
    d.iter().filter(|x| x.code == code).collect()
}

fn node(v: &mut Value, i: usize) -> &mut Value {
    &mut v["content"]["nodes"][i]
}

fn seam(v: &mut Value, i: usize) -> &mut Value {
    &mut v["content"]["seams"][i]
}

fn boxx(v: &mut Value, i: usize) -> &mut Value {
    &mut v["content"]["boxes"][i]
}

// ---------------------------------------------------------------------------
// The green, and the motivating shape
// ---------------------------------------------------------------------------

/// **The green.** Every red below is one edit away from this.
///
/// Binding: 6 places, of which **2 are ways**; 5 seams, of which **1 is a
/// contact**.
#[test]
fn the_green_states_a_one_body_wide_route_and_a_front_and_validates() {
    let d = graph_with(|_| {});
    let ours: Vec<_> = d
        .iter()
        .filter(|x| matches!(x.code.as_str(), "DW0875" | "DW0876" | "DW0832"))
        .collect();
    assert!(
        ours.is_empty(),
        "the green document must raise no way or contact refusal: {ours:#?}"
    );

    // The binding this test claims, computed from the documents rather than
    // written down beside them.
    let g: Value = serde_json::from_str(GRAPH).expect("parse");
    let p: Value = serde_json::from_str(PLAN).expect("parse");
    let ways = g["content"]["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .filter(|n| n.get("way_class").is_some())
        .count();
    let contacts = p["content"]["seams"]
        .as_array()
        .expect("seams")
        .iter()
        .filter(|s| s.get("contact").is_some())
        .count();
    assert_eq!(ways, 2, "the green declares two ways");
    assert_eq!(contacts, 1, "the green declares one contact");
}

/// **The motivating shape, and the old way of stating it.**
///
/// `node/cliff-road` is 4 by 72. Stated as a way it is accepted; stated the only
/// way the vocabulary had before — a rung of the size ladder — it is refused,
/// and the refusal has to name a remedy the author can actually perform.
///
/// The remedy is checked by PERFORMING it: the same box, declared `way_class`,
/// goes green. A refusal whose prescription does not clear it is a refusal that
/// sends the author to read the compiler.
#[test]
fn the_old_way_of_stating_it_is_refused_with_a_reachable_remedy() {
    // Every rung, so this is "the ladder cannot classify it" and not "the author
    // picked the wrong rung".
    for rung in ["alcove", "room", "hall", "arena", "expanse"] {
        let d = graph_with(|v| {
            let n = node(v, 1);
            n.as_object_mut().expect("node").remove("way_class");
            n["size_class"] = json!(rung);
        });
        let refusals = with_code(&d, "DW0832");
        assert!(
            !refusals.is_empty(),
            "a 4x72 box declared `{rung}` must be refused: {:?}",
            codes(&d)
        );
        assert!(
            refusals[0].message.contains("size class"),
            "the refusal must say which vocabulary it judged against: {}",
            refusals[0].message
        );
    }

    // The remedy, performed.
    let d = graph_with(|_| {});
    assert!(
        with_code(&d, "DW0832").is_empty(),
        "the same box declared a way must be accepted: {:?}",
        codes(&d)
    );
}

// ---------------------------------------------------------------------------
// DW0875 — a place is classified exactly once
// ---------------------------------------------------------------------------

#[test]
fn dw0875_refuses_a_place_that_is_classified_twice() {
    let d = graph_with(|v| node(v, 1)["size_class"] = json!("hall"));
    let refusals = with_code(&d, "DW0875");
    assert_eq!(
        refusals.len(),
        1,
        "exactly the doubly-classified place is refused: {:?}",
        codes(&d)
    );
    assert!(
        refusals[0].message.contains("BOTH")
            && refusals[0]
                .message
                .contains("delete whichever one this place is not"),
        "{}",
        refusals[0].message
    );
}

#[test]
fn dw0875_refuses_a_place_classified_by_nothing() {
    let d = graph_with(|v| {
        node(v, 1)
            .as_object_mut()
            .expect("node")
            .remove("way_class");
    });
    let refusals = with_code(&d, "DW0875");
    assert_eq!(refusals.len(), 1, "{:?}", codes(&d));
    // The remedy names both vocabularies, because the author's next action is
    // choosing from one of them.
    let m = &refusals[0].message;
    assert!(
        m.contains("Defined size classes:") && m.contains("hall"),
        "{m}"
    );
    assert!(
        m.contains("Defined way classes:") && m.contains("road"),
        "{m}"
    );
}

/// `DW0812` answers a misspelled way class exactly as it answers a misspelled
/// rung — the resolve widened to the object class rather than gaining a second
/// path (spec-0053 §6, last row).
#[test]
fn dw0812_refuses_an_unknown_way_class_and_names_the_defined_set() {
    let d = graph_with(|v| node(v, 1)["way_class"] = json!("highway"));
    let refusals = with_code(&d, "DW0812");
    assert_eq!(refusals.len(), 1, "{:?}", codes(&d));
    let m = &refusals[0].message;
    assert!(m.contains("way class") && m.contains("highway"), "{m}");
    assert!(
        m.contains("corridor") && m.contains("road"),
        "the refusal names the whole defined set: {m}"
    );
    assert_eq!(refusals[0].path, "/content/nodes/1/way_class");
}

// ---------------------------------------------------------------------------
// DW0832's way branch — the three trips spec-0053 §6 names
// ---------------------------------------------------------------------------

/// Trip 1: the cross-section is outside the class's range.
///
/// A 32x40 box declared `corridor`, which is spec-0053 §6's own named trip.
#[test]
fn dw0832_refuses_a_way_whose_cross_section_is_off_its_class() {
    let d = plan_with(|v| {
        boxx(v, 1)["extent"] = json!([32, 40]);
    });
    let refusals = with_code(&d, "DW0832");
    assert!(!refusals.is_empty(), "{:?}", codes(&d));
    assert!(
        refusals[0].message.contains("cross-section")
            && refusals[0].message.contains("shorter extent"),
        "the refusal says which extent it measured: {}",
        refusals[0].message
    );
}

/// Trip 2: **a square box can never be a way**, and that is structural.
///
/// The run must exceed `max_width` and the cross-section must not, so one number
/// cannot satisfy both. This is the opt-out property `CLAUDE.md` demands: the
/// defect — declaring a room a way to escape the ladder — is incapable of
/// supplying the proof the way branch asks for.
#[test]
fn a_square_box_can_never_be_a_way_at_any_class() {
    for (class, side) in [("corridor", 4), ("road", 8), ("road", 16)] {
        let d = plan_with(|v| {
            boxx(v, 1)["extent"] = json!([side, side]);
        });
        let d = {
            // The box's node must declare the class under test.
            let mut g: Value = serde_json::from_str(GRAPH).expect("parse");
            node(&mut g, 1)["way_class"] = json!(class);
            let mut p: Value = serde_json::from_str(PLAN).expect("parse");
            boxx(&mut p, 1)["extent"] = json!([side, side]);
            let _ = d;
            check_campaign(&campaign(
                serde_json::to_string(&g).expect("re-serialize"),
                serde_json::to_string(&p).expect("re-serialize"),
            ))
        };
        let refusals = with_code(&d, "DW0832");
        assert!(
            !refusals.is_empty(),
            "a {side}x{side} box declared `{class}` must be refused: {:?}",
            codes(&d)
        );
        assert!(
            refusals.iter().any(|x| x
                .message
                .contains("does not exceed the class's widest cross-section")),
            "the refusal must be about the elongation: {:#?}",
            refusals
        );
    }
}

/// Trip 3: a box one cell under the class's clearance.
#[test]
fn dw0832_refuses_a_way_one_cell_under_its_clearance() {
    let d = plan_with(|v| {
        // `road` seeds `min_clearance` at 6; the green declares 8.
        boxx(v, 1)["ceiling"] = json!({ "clearance": 5 });
    });
    let refusals = with_code(&d, "DW0832");
    assert!(!refusals.is_empty(), "{:?}", codes(&d));
    assert!(
        refusals[0].message.contains("headroom"),
        "{}",
        refusals[0].message
    );
}

// ---------------------------------------------------------------------------
// DW0876 — the four shapes of one claim
// ---------------------------------------------------------------------------

#[test]
fn dw0876_refuses_a_seam_that_declares_both_kinds() {
    let d = plan_with(|v| seam(v, 4)["opening"] = json!("arch"));
    let refusals = with_code(&d, "DW0876");
    assert_eq!(refusals.len(), 1, "{:?}", codes(&d));
    assert!(
        refusals[0].message.contains("BOTH"),
        "{}",
        refusals[0].message
    );
}

#[test]
fn dw0876_refuses_a_seam_that_declares_neither_kind() {
    let d = plan_with(|v| {
        seam(v, 4).as_object_mut().expect("seam").remove("contact");
    });
    let refusals = with_code(&d, "DW0876");
    assert_eq!(refusals.len(), 1, "{:?}", codes(&d));
    let m = &refusals[0].message;
    assert!(m.contains("neither"), "{m}");
    // The remedy names both spellings and the defined standards.
    assert!(
        m.contains("\"contact\": {}") && m.contains("gateway"),
        "{m}"
    );
}

/// **The floor that keeps the surface honest** (spec-0053 §4, §6 row 3).
///
/// A contact must be WIDER than the broadest standard opening, so anything at or
/// under that width could have been a portal. `opening.gateway` is 5 wide, so a
/// 3-wide contact — a door dodging the standard set — is refused by its own
/// width, and a 5-wide one is too: the floor is exclusive on purpose.
#[test]
fn dw0876_refuses_a_contact_no_wider_than_the_broadest_standard_opening() {
    for width in [3u32, 5] {
        let d = plan_with(|v| seam(v, 4)["contact"] = json!({ "extent": [width, 8] }));
        let refusals = with_code(&d, "DW0876");
        assert_eq!(
            refusals.len(),
            1,
            "a {width}-wide contact must be refused: {:?}",
            codes(&d)
        );
        let m = &refusals[0].message;
        assert!(
            m.contains("not wider than the broadest standard opening"),
            "{m}"
        );
        assert!(
            m.contains("could have been one"),
            "the refusal must say WHY the floor is there: {m}"
        );
    }
    // Six is wider than five, and goes green — the pair, so the assertion above
    // is about the floor and not about contacts in general.
    let d = plan_with(|v| seam(v, 4)["contact"] = json!({ "extent": [6, 8] }));
    assert!(with_code(&d, "DW0876").is_empty(), "{:?}", codes(&d));
}

/// Spec-0053 §6 row 3, second half: a span leaving the face `DW0828`
/// established.
#[test]
fn dw0876_refuses_a_contact_span_that_leaves_the_shared_face() {
    let d = plan_with(|v| seam(v, 4)["contact"] = json!({ "extent": [64, 8] }));
    let refusals = with_code(&d, "DW0876");
    assert_eq!(refusals.len(), 1, "{:?}", codes(&d));
    let m = &refusals[0].message;
    assert!(m.contains("leaves the face the two boxes share"), "{m}");
    assert!(
        m.contains("Omitting `contact.extent`"),
        "the remedy must name the spelling that cannot leave the face: {m}"
    );
}

/// Spec-0053 §6 row 4: `stair`, `barred` and `vision` contacts are excluded,
/// and the exclusion is the falsifier re-armed rather than an oversight.
///
/// All three are asserted, not one: "the class the author happened to try" is
/// not the rule.
#[test]
fn dw0876_refuses_a_contact_on_a_class_a_front_cannot_be() {
    for class in ["stair", "barred", "vision"] {
        let mut g: Value = serde_json::from_str(GRAPH).expect("parse");
        let e = &mut g["content"]["edges"][4];
        e["class"] = json!(class);
        match class {
            "stair" => {}
            "barred" => {
                e["opens_from"] = json!("a");
                e["gating"] = json!({ "quest": "quest/open-the-door" });
            }
            _ => {}
        }
        let d = check_campaign(&campaign(
            serde_json::to_string(&g).expect("re-serialize"),
            PLAN.to_string(),
        ));
        // A `vision` connection carries a sightline rather than a seam, so
        // `DW0824` reaches it first — the engine already refuses that pairing
        // and this rule does not need to duplicate it. The claim asserted here
        // is that no `vision` contact compiles, by whichever code owns it.
        let refused = !with_code(&d, "DW0876").is_empty() || !with_code(&d, "DW0824").is_empty();
        assert!(
            refused,
            "a `{class}` contact must not compile: {:?}",
            codes(&d)
        );
        if class != "vision" {
            let refusals = with_code(&d, "DW0876");
            assert!(
                refusals
                    .iter()
                    .any(|x| x.message.contains("carries `walk` or `drop` only")),
                "{:#?}",
                refusals
            );
        }
    }
}

/// A `drop` contact is legal — a rim falling to a lower court is a genuine broad
/// hand-off (spec-0053 §4). The pair to the test above: without this, "a contact
/// carries walk or drop" would be indistinguishable from "a contact carries
/// walk".
#[test]
fn a_drop_contact_is_legal() {
    let mut g: Value = serde_json::from_str(GRAPH).expect("parse");
    g["content"]["edges"][4] = json!({
        "id": "edge/court-hall", "class": "drop",
        "a": "node/court", "b": "node/hall", "falls": "a-to-b"
    });
    let mut p: Value = serde_json::from_str(PLAN).expect("parse");
    // The court stands three blocks over the hall, so the fall is real and
    // inside `drop.max-designed-rise`. Raising it raises the SHARED FACE with
    // it — two boxes share only the y span they have in common — so the span's
    // anchor moves to the new face's low corner, which is the number both
    // `DW0876` and `DW0828` print in their refusals.
    boxx(&mut p, 5)["floor"] = json!({ "y": 67 });
    seam(&mut p, 4)["at"] = json!([14, 67]);
    let d = check_campaign(&campaign(
        serde_json::to_string(&g).expect("re-serialize"),
        serde_json::to_string(&p).expect("re-serialize"),
    ));
    assert!(
        with_code(&d, "DW0876").is_empty() && with_code(&d, "DW0831").is_empty(),
        "a drop contact inside the policy cap must compile: {:#?}",
        with_code(&d, "DW0876")
    );
}

/// `DW0829`'s standard-name resolution and sill rule are PORTAL checks, and the
/// spec says so rather than shoehorning a 55-cell front into a doorway
/// (spec-0053 §4).
///
/// The pair: the same face, as a portal with an unreachable sill, IS refused —
/// so this test is about the contact and not about the sill rule having stopped
/// working.
#[test]
fn no_door_check_applies_to_a_contact() {
    // A sill four blocks over the floor: unreachable by jumping, and DW0829's.
    let as_portal = plan_with(|v| {
        let s = seam(v, 4);
        s.as_object_mut().expect("seam").remove("contact");
        s["opening"] = json!("gateway");
        s["at"] = json!([14, 68]);
    });
    assert!(
        !with_code(&as_portal, "DW0829").is_empty(),
        "the pair: as a portal this face is DW0829: {:?}",
        codes(&as_portal)
    );

    // The same anchor, as a contact: no door check reaches it.
    let as_contact = plan_with(|v| seam(v, 4)["at"] = json!([14, 68]));
    assert!(
        with_code(&as_contact, "DW0829").is_empty(),
        "a contact has no opening name to resolve and no single sill: {:?}",
        codes(&as_contact)
    );
}

// ---------------------------------------------------------------------------
// The fence, both directions
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// §7 — what the engine must NOT learn
// ---------------------------------------------------------------------------

/// **The engine does not know a route's LENGTH.**
///
/// A way class's entry has three fields and none of them is a length; and the
/// table's whole standard vocabulary contains no entry whose value is the run of
/// anything. A `max_length` added tomorrow would red this.
///
/// It is asserted over the exported table rather than over the struct, because
/// the export is what a consumer outside this crate reads and is where such a
/// field would first become a standard somebody could build against.
#[test]
fn no_standard_states_the_length_of_a_route() {
    use delvewright_dsl::metrics::{MetricKind, Metrics};
    let table = Metrics::table();
    let export = delvewright_dsl::metrics::export(&table);
    let building = export["building"].as_object().expect("the building half");

    let mut examined = 0usize;
    for name in table.names_of(MetricKind::WayClass) {
        let key = format!("way-class.{name}");
        let v = building[&key]["value"].as_object().expect("a way class");
        examined += 1;
        let mut fields: Vec<&str> = v.keys().map(String::as_str).collect();
        fields.sort_unstable();
        assert_eq!(
            fields,
            ["max_width", "min_clearance", "min_width"],
            "`{key}` bounds the cross-section and nothing else"
        );
    }
    assert!(examined > 0, "the way vocabulary is not empty");

    // And nowhere else in the table either: no entry names a length, a run or a
    // traverse of a WAY. (`nominal_traverse_blocks` on a size class is the
    // ladder's own, and is exactly what a way class deliberately lacks.)
    for (key, entry) in building {
        let text = serde_json::to_string(entry).expect("serialize");
        assert!(
            !text.contains("max_length") && !text.contains("max_run"),
            "`{key}` states the length of something: {text}"
        );
    }
}

/// **The engine does not know the WIDTH OF A FRONT as a standard.**
///
/// The standard opening set is exactly what it was: no entry was added whose
/// dimensions are a campaign's measured geometry, which spec-0053 §7 names as
/// the exact workaround the version exists to forbid.
///
/// The list is written out by hand so that adding `opening.gate-front` — the
/// spec's own example of content wearing a standard's clothes — reds this test
/// rather than passing under a count.
#[test]
fn no_standard_opening_states_a_measured_front() {
    use delvewright_dsl::metrics::{MetricKind, Metrics};
    let table = Metrics::table();
    let mut names = table.names_of(MetricKind::Opening);
    names.sort_unstable();
    assert_eq!(
        names,
        ["arch", "door", "gateway", "passage"],
        "the standard opening set is the doorway vocabulary, and a front is never one of \
         its entries: a contact's span is a fact of two boxes, so an `opening.gate-front` \
         of 21x4 would be this campaign's geometry published as a standard"
    );
}

/// A contact's span is never compared against a table entry that PRESCRIBES a
/// width. The only table number it meets is the derived floor, which refuses
/// narrowness and prescribes nothing — demonstrated by the pair: a span of 6, of
/// 16 and of the whole face are all equally acceptable.
#[test]
fn a_contacts_width_is_the_plans_business_and_is_never_prescribed() {
    for extent in [
        json!({ "extent": [6, 8] }),
        json!({ "extent": [16, 8] }),
        json!({}),
    ] {
        let d = plan_with(|v| seam(v, 4)["contact"] = extent.clone());
        assert!(
            with_code(&d, "DW0876").is_empty(),
            "a span of {extent} must be the plan's business: {:?}",
            codes(&d)
        );
    }
}

// ---------------------------------------------------------------------------
// The perturbation proofs — the green depends on each rule's SAFETY
// ---------------------------------------------------------------------------

/// Made vacuous, the exactly-one rule refuses the green document.
///
/// The rule is `both || neither`. The vacuous shape a careless author would
/// reach for is *"a node must declare `size_class`"* — which is what the field
/// was before — and under it every way in the green graph is a refusal. This
/// asserts from OUTSIDE that the green's two ways depend on the rule comparing a
/// node's two fields rather than demanding one of them.
#[test]
fn perturbing_the_place_class_rule_to_the_vacuous_shape_goes_red() {
    let g: Value = serde_json::from_str(GRAPH).expect("parse");
    let vacuous: Vec<&Value> = g["content"]["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .filter(|n| n.get("size_class").is_none())
        .collect();
    assert_eq!(
        vacuous.len(),
        2,
        "a rule demanding `size_class` would refuse these {} place(s), which is what makes \
         the real rule's comparison load-bearing: {vacuous:#?}",
        vacuous.len()
    );
}

/// Made vacuous, the contact floor refuses the green document.
///
/// The floor is *"wider than the broadest standard opening"*, derived from the
/// table. The vacuous shape is a CONSTANT — and this asserts that the green's
/// contact would be refused by a floor set anywhere above the standard set,
/// which is what makes "derived from the table" the load-bearing half rather
/// than "some number".
#[test]
fn perturbing_the_contact_floor_to_a_constant_goes_red() {
    use delvewright_dsl::metrics::{MetricKind, Metrics, Reads};
    let table = Metrics::table();
    let mut reads = Reads::new();
    let derived = table
        .broadest_opening_width(&mut reads)
        .expect("the table defines openings");

    // The green's contact spans the whole shared face of two 16-wide boxes.
    let p: Value = serde_json::from_str(PLAN).expect("parse");
    let court = p["content"]["boxes"][5]["extent"][0]
        .as_u64()
        .expect("extent") as u32;
    assert!(
        court > derived,
        "the green's front is {court} wide against a derived floor of {derived}, so it \
         clears the floor BECAUSE the floor comes from the opening set"
    );

    // A floor set at the front's own width refuses it: the demonstration that
    // the number is what decides, not the shape of the check.
    let d = plan_with(|v| seam(v, 4)["contact"] = json!({ "extent": [derived, 8] }));
    assert!(
        !with_code(&d, "DW0876").is_empty(),
        "a span at exactly the derived floor is refused, so the comparison is strict: {:?}",
        codes(&d)
    );

    // And the whole opening set is what the floor is taken over, not one name.
    let widths: Vec<u32> = table
        .names_of(MetricKind::Opening)
        .into_iter()
        .filter_map(|n| {
            match table
                .resolve(MetricKind::Opening, n)
                .ok()?
                .value(&mut reads)
            {
                delvewright_dsl::metrics::MetricValue::Opening(o) => Some(o.width),
                _ => None,
            }
        })
        .collect();
    assert_eq!(
        widths.iter().copied().max(),
        Some(derived),
        "the floor is the MAXIMUM over the whole opening set: {widths:?}"
    );
}
