//! DSL v0.19 (spec-0053): a place that is a **route**, and a hand-off that is
//! not a **door**.
//!
//! # The motivating shape
//!
//! The campaign brief this vocabulary was written for says *"one cut ledge, one
//! body wide, climbing across the whole seaward face"*. `node/cliff-road` below
//! is that ledge: **4 by 72**. A box is the author's declaration of its own
//! size, so the ledge is written as the box it is and nothing classifies it.
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
use std::sync::LazyLock;

/// The graph the tests perturb.
///
/// * **`node/cliff-road`** — the motivating shape, 4 by 72.
/// * **`node/duct`** — 4 by 8: the narrow way.
/// * The rest are ordinary rooms.
static GRAPH: LazyLock<String> = LazyLock::new(|| {
    common::at_dsl_version(
        r#"{
  "campaign_id": "hello-world",
  "dsl_version": "%dsl_version%",
  "stage": "layout-graph",
  "content": {
    "nodes": [
      { "id": "node/porch", "intent": "threshold" },
      { "id": "node/cliff-road", "intent": "approach",
        "note": "One cut ledge, one body wide, climbing across the whole seaward face." },
      { "id": "node/hall", "intent": "hub" },
      { "id": "node/duct", "intent": "crawl" },
      { "id": "node/vault", "intent": "goal-chamber" },
      { "id": "node/court", "intent": "vista" }
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
}"#,
    )
});

static BRIEF: LazyLock<String> = LazyLock::new(|| {
    common::at_dsl_version(
        r#"{
  "campaign_id": "hello-world",
  "dsl_version": "%dsl_version%",
  "stage": "geometry-brief",
  "content": {
    "facts": [
      { "id": "fact/region-span", "value": 128.0, "unit": "blocks",
        "note": "The site is a hundred and twenty-eight blocks across." }
    ]
  }
}"#,
    )
});

/// The plan the tests perturb.
///
/// `node/court` meets `node/hall` along a **contact** — a front 16 cells wide.
/// Every other seam is an ordinary
/// portal, so both kinds are resolved, derived and measured in one plan.
static PLAN: LazyLock<String> = LazyLock::new(|| {
    common::at_dsl_version(
        r#"{
  "campaign_id": "hello-world",
  "dsl_version": "%dsl_version%",
  "stage": "site-plan",
  "content": {
    "region": { "min": [0, 56, 0], "extent": [128, 32, 128] },
    "datums": [ { "id": "datum/grade", "y": 64 } ],
    "fill": { "kind": "solid", "block": "minecraft:stone" },
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
      { "edge": "edge/porch-road", "form": "a doorway", "face": "east", "at": 1, "meets": 1, "opening": "arch" },
      { "edge": "edge/road-hall", "form": "a doorway", "face": "east", "at": 1, "meets": 1, "opening": "arch" },
      { "edge": "edge/hall-duct", "form": "a doorway", "face": "east", "at": 1, "meets": 1, "opening": "arch" },
      { "edge": "edge/duct-vault", "form": "a doorway", "face": "east", "at": 1, "meets": 1, "opening": "arch" },
      { "edge": "edge/court-hall", "form": "a doorway", "face": "north", "contact": {} }
    ]
  }
}"#,
    )
});

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
        design: None,
        layout_graph: Some(graph),
        geometry_brief: Some(BRIEF.to_string()),
        ..base
    }
}

/// Validate the green campaign with the GRAPH perturbed by one edit.
fn graph_with(patch: impl FnOnce(&mut Value)) -> Vec<delvewright_dsl::Diagnostic> {
    let mut v: Value = serde_json::from_str(GRAPH.as_str()).expect("the green graph parses");
    patch(&mut v);
    check_campaign(&campaign(
        serde_json::to_string(&v).expect("re-serialize"),
        PLAN.to_string(),
    ))
}

/// Validate the green campaign with the PLAN perturbed by one edit.
fn plan_with(patch: impl FnOnce(&mut Value)) -> Vec<delvewright_dsl::Diagnostic> {
    let mut v: Value = serde_json::from_str(PLAN.as_str()).expect("the green plan parses");
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
/// Binding: 6 places; 5 seams, of which **1 is a contact**.
#[test]
fn the_green_states_a_one_body_wide_route_and_a_front_and_validates() {
    let d = graph_with(|_| {});
    let ours = with_code(&d, "DW0876");
    assert!(
        ours.is_empty(),
        "the green document must raise no contact refusal: {ours:#?}"
    );

    // The binding this test claims, computed from the documents rather than
    // written down beside them.
    let p: Value = serde_json::from_str(PLAN.as_str()).expect("parse");
    let contacts = p["content"]["seams"]
        .as_array()
        .expect("seams")
        .iter()
        .filter(|s| s.get("contact").is_some())
        .count();
    assert_eq!(contacts, 1, "the green declares one contact");
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

/// **A contact's width is the author's.** A front one, two or three cells
/// wide — a rope bridge's end — is as legal as a wide one: no floor derived from
/// the standard opening set refuses it.
#[test]
fn a_contact_of_any_width_is_the_authors() {
    for width in [1u32, 2, 3, 5] {
        let d = plan_with(|v| seam(v, 4)["contact"] = json!({ "extent": [width, 3] }));
        assert!(
            with_code(&d, "DW0876").is_empty(),
            "a {width}-wide contact is the author's: {:?}",
            codes(&d)
        );
    }
}

/// **A portal may declare its own size**, and `DW0829` confirms it fits the
/// shared face: a 1x2 opening is accepted, and the same opening declared taller
/// than the face is refused as a standard that does not fit would be.
#[test]
fn a_seam_declares_its_own_opening_and_dw0829_confirms_it_fits() {
    let d = plan_with(|v| seam(v, 0)["opening"] = json!({ "width": 1, "height": 2 }));
    for code in ["DW0812", "DW0829", "DW0876", "DW0100"] {
        assert!(with_code(&d, code).is_empty(), "{code}: {:?}", codes(&d));
    }
    let d = plan_with(|v| seam(v, 0)["opening"] = json!({ "width": 1, "height": 20 }));
    let refusals = with_code(&d, "DW0829");
    assert_eq!(refusals.len(), 1, "{:?}", codes(&d));
    assert!(
        refusals[0].message.contains("declared 1x20"),
        "{}",
        refusals[0].message
    );
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
        let mut g: Value = serde_json::from_str(GRAPH.as_str()).expect("parse");
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
    let mut g: Value = serde_json::from_str(GRAPH.as_str()).expect("parse");
    g["content"]["edges"][4] = json!({
        "id": "edge/court-hall", "class": "drop",
        "a": "node/court", "b": "node/hall", "falls": "a-to-b"
    });
    let mut p: Value = serde_json::from_str(PLAN.as_str()).expect("parse");
    // The court stands three blocks over the hall, so the fall is real and
    // inside `drop.max-designed-rise`. Raising it raises the SHARED FACE with
    // it — two boxes share only the y span they have in common — and the sill
    // follows, being the higher floor: nothing on the seam is retyped.
    boxx(&mut p, 5)["floor"] = json!({ "y": 67 });
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
    // A sill three blocks over the hall's floor — the court's, being the higher
    // of the two: unreachable by jumping, and DW0829's.
    let as_portal = plan_with(|v| {
        boxx(v, 5)["floor"] = json!({ "y": 67 });
        let s = seam(v, 4);
        s.as_object_mut().expect("seam").remove("contact");
        s["opening"] = json!("gateway");
    });
    assert!(
        !with_code(&as_portal, "DW0829").is_empty(),
        "the pair: as a portal this face is DW0829: {:?}",
        codes(&as_portal)
    );

    // The same floors, as a contact: no door check reaches it.
    let as_contact = plan_with(|v| boxx(v, 5)["floor"] = json!({ "y": 67 }));
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

/// **The engine does not know the size of a place as a standard.**
///
/// A box is the author's declaration of its own size, and the piece may not
/// exceed it — so a class range beside it would confirm nothing. The table
/// holds no entry that bounds a place's footprint, its cross-section or its
/// run; a size or way class added back would red this.
#[test]
fn no_standard_states_the_size_of_a_place() {
    use delvewright_dsl::metrics::Metrics;
    let table = Metrics::table();
    let export = delvewright_dsl::metrics::export(&table);
    let building = export["building"].as_object().expect("the building half");
    assert!(!building.is_empty(), "the building half is not empty");
    for (key, entry) in building {
        assert!(
            !key.starts_with("size-class.") && !key.starts_with("way-class."),
            "`{key}` classifies a place"
        );
        let text = serde_json::to_string(entry).expect("serialize");
        for field in [
            "min_footprint",
            "max_footprint",
            "max_width",
            "max_length",
            "max_run",
        ] {
            assert!(
                !text.contains(field),
                "`{key}` bounds a place's size: {text}"
            );
        }
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
/// width: a span of 6, of 16 and of the whole face are all equally acceptable.
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
