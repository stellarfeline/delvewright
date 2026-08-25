//! DSL v0.14 (spec-0049 §4): the site plan — the geometric embedding of the
//! layout graph.
//!
//! # How these tests are kept falsifiable
//!
//! Geometry is where a self-agreeing fixture hides best. A partition, a
//! face-sharing relation and a reachable rise are all things the checker can
//! compute — so a fixture BUILT by that arithmetic agrees with it by
//! construction, and perturbing the rule leaves every assertion green. The
//! metrics round walked into that trap and the layout-graph round named it.
//!
//! So the map below is **drawn by hand and its answers are written by hand**.
//! The comment over [`PLAN`] is a plan view with every box's cell range spelled
//! out, and every number the tests assert — which pairs are disjoint, which
//! faces are shared and at what gap, what each stair's rise is — is read off
//! that drawing rather than out of the code. Nothing in this file calls
//! `shared_face`, `overlap` or any other checker function to decide what to
//! assert.
//!
//! And every red is a **one-field perturbation of that same green document**,
//! so each assertion is half of a red→green pair rather than a document written
//! to fail. If a check stopped working its perturbation would go green, and the
//! test would say so.

mod common;

use delvewright_dsl::{RawCampaign, check_campaign};
use serde_json::{Value, json};

/// The graph the plan embeds. Six places, seven ways a body passes and one line
/// of sight, bound to the hello-world campaign's one quest.
///
/// ```text
///                              edge/porch-vault-sightline (vision)
///        ┌───────────────────────────────────────────────────────┐
///        │                                                       │
///   [porch] ──walk── [hall] ══barred (quest)══ [vault] ──walk── [yard]
///        │             ║                                           ║ ╲
///       drop        stair (shortcut)                             drop stair
///        ╲             ║                                           ╲ ╱
///         ╲────────► [cellar]                                     [pit]
/// ```
const GRAPH: &str = r#"{
  "campaign_id": "hello-world",
  "dsl_version": "0.13.0",
  "stage": "layout-graph",
  "content": {
    "nodes": [
      { "id": "node/porch", "intent": "threshold", "size_class": "alcove" },
      { "id": "node/hall", "intent": "hub", "size_class": "room" },
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
        "note": "The site is sixty-four blocks across." },
      { "id": "fact/hall-span", "value": 16.0, "unit": "blocks",
        "note": "The hall is sixteen wall to wall." },
      { "id": "fact/hall-height", "value": 8.0, "unit": "blocks",
        "note": "Eight blocks of headroom in the hall, so the vault reads as tall." },
      { "id": "fact/porch-to-vault", "value": 24.0, "unit": "blocks",
        "note": "The vault stands at least twenty-four blocks off the porch, so the approach is walked rather than glanced at." },
      { "id": "fact/grade", "value": 64.0, "unit": "blocks",
        "note": "Grade is at sixty-four." }
    ]
  }
}"#;

/// The green map, drawn by hand.
///
/// Plan view at grade (east is `+x`, south is `+z`); each place's cell range is
/// written out, and every number the tests below assert is read off THIS
/// drawing rather than computed:
///
/// ```text
///            x: 0        13        30        45         63
///   z:  4..11    [porch ]  [ hall  ]  [vault ]  ┐
///                 4..11     13..28     30..37   │
///                 y 64..67  y 64..71   y 64..67 │
///   z: 13..28    [cellar]              [   yard    ]
///                 4..11                 30..45
///                 y 59..70              y 64..71  (sky-open)
///   z: 16..23                           [pit] 32..39, y 59..62 (under the yard)
/// ```
///
/// Stated by hand, so the assertions are not the checker agreeing with itself:
///
/// * **all fifteen box pairs are disjoint.** The three at grade are separated on
///   `x`; `cellar` clears them on `z`; `yard` clears `hall` on `x`; `pit` sits
///   under `yard` and clears it on `y`.
/// * **seven faces are shared, each with a gap of exactly one cell** — that cell
///   being the wall the two places have in common. `porch|hall` at x 12,
///   `hall|vault` at x 29, `hall|cellar` at x 12, `porch|cellar` at z 12,
///   `vault|yard` at z 12, and `yard|pit` at y 63 (twice, once per connection).
/// * **the two stairs climb 5 blocks each, and both are hosted in the LOWER
///   place**, because treads rise off a walk plane and the only one a stair has
///   is the lower of the two. `hall` (64) down to `cellar` (59) is massed in
///   `cellar`; `pit` (59) up to `yard` (64) is massed in `pit`. The 1:1 standard
///   pitch needs 5 blocks of run, and each host affords 8 across the axis its
///   run is spent on, so both are buildable.
/// * **the two drops fall 5 blocks each**, which is exactly the designed-drop
///   cap — the deepest fall the policy allows, and therefore green.
/// * **five identities hold**: the region is 64 across, the hall is 16 by 8, the
///   porch centre `(7.5, 7.5)` stands 26 blocks from the vault centre
///   `(33.5, 7.5)` which is at least the 24 the brief asks for, and grade is 64.
const PLAN: &str = r#"{
  "campaign_id": "hello-world",
  "dsl_version": "0.14.0",
  "stage": "site-plan",
  "content": {
    "region": { "min": [0, 48, 0], "extent": [64, 48, 64] },
    "datums": [
      { "id": "datum/grade", "y": 64, "note": "The plane the delve opens on." },
      { "id": "datum/undercroft", "y": 59 }
    ],
    "boxes": [
      { "node": "node/porch", "min": [4, 4], "extent": [8, 8],
        "floor": { "datum": "datum/grade" }, "ceiling": { "clearance": 4 } },
      { "node": "node/hall", "min": [13, 4], "extent": [16, 16],
        "floor": { "datum": "datum/grade" }, "ceiling": { "clearance": 8 } },
      { "node": "node/vault", "min": [30, 4], "extent": [8, 8],
        "floor": { "datum": "datum/grade" }, "ceiling": { "clearance": 4 } },
      { "node": "node/cellar", "min": [4, 13], "extent": [8, 16],
        "floor": { "datum": "datum/undercroft" }, "ceiling": { "clearance": 12 } },
      { "node": "node/yard", "min": [30, 13], "extent": [16, 16],
        "floor": { "datum": "datum/grade" }, "ceiling": "open" },
      { "node": "node/pit", "min": [32, 16], "extent": [8, 8],
        "floor": { "y": 59 }, "ceiling": { "clearance": 4 } }
    ],
    "seams": [
      { "edge": "edge/porch-hall", "face": "east", "at": [6, 64], "opening": "arch" },
      { "edge": "edge/hall-vault", "face": "east", "at": [6, 64], "opening": "door" },
      { "edge": "edge/hall-cellar", "face": "west", "at": [14, 64], "opening": "passage",
        "stair_in": "node/cellar" },
      { "edge": "edge/porch-cellar", "face": "south", "at": [6, 64], "opening": "passage" },
      { "edge": "edge/vault-yard", "face": "south", "at": [32, 64], "opening": "arch" },
      { "edge": "edge/yard-pit", "face": "down", "at": [32, 16], "opening": "passage" },
      { "edge": "edge/pit-yard", "face": "up", "at": [32, 16], "opening": "passage",
        "stair_in": "node/pit" }
    ],
    "volumes": [
      { "id": "volume/undercroft-rock", "region": { "min": [4, 48, 4], "extent": [44, 10, 36] },
        "role": "massif", "note": "The rock the cellar and the pit are cut into." },
      { "id": "volume/sky-over-the-yard",
        "region": { "min": [30, 72, 13], "extent": [16, 20, 16] }, "role": "clearance" }
    ],
    "identities": [
      { "fact": "fact/region-span", "measure": { "of": "region-extent", "axis": "x" },
        "cmp": "eq" },
      { "fact": "fact/hall-span",
        "measure": { "of": "box-extent", "node": "node/hall", "axis": "x" }, "cmp": "eq" },
      { "fact": "fact/hall-height", "measure": { "of": "box-height", "node": "node/hall" },
        "cmp": "eq" },
      { "fact": "fact/porch-to-vault",
        "measure": { "of": "distance-xz", "from": "node/porch", "to": "node/vault" },
        "cmp": "ge" },
      { "fact": "fact/grade", "measure": { "of": "datum-y", "datum": "datum/grade" },
        "cmp": "eq" }
    ],
    "sightlines": [
      { "edge": "edge/porch-vault-sightline", "from": [8, 65, 8], "to": [33, 65, 8] }
    ],
    "views": [
      { "id": "view/from-the-south", "eye": [20, 80, 60], "look_at": [20, 66, 12],
        "note": "The approach the silhouette is judged from." }
    ],
    "lighting": { "fixture": "torch", "min_light": 7 }
  }
}"#;

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

/// The hello-world campaign, made into a **site-plan** campaign when a plan is
/// handed over.
///
/// Four substitutions, and every one of them is an obligation the round that
/// derived the blockout added rather than a convenience:
///
/// * **`areas[]` is emptied.** A campaign has one placement authority
///   (`DW0839`), and this one's is the plan.
/// * **the area references move to `area/site`.** That is the one place a
///   site-plan campaign has, and it is where the NPC stands and the quest
///   happens.
/// * **the anchors move to the synthesized vocabulary.** A site-plan campaign
///   has no prefab to name an anchor, so `anchor/node-<place>` is what its
///   content binds to — the names `delvewright_dsl::synthesized_anchors`
///   reports and the derivation really places.
/// * **the `open-gate` names the barred seam's own region.** `DW0818`'s
///   byte-side half: a barred way nothing opens is a wall, and the region such
///   an effect must target only exists once a plan does.
///
/// A campaign handed no plan is left exactly as it was, which is what keeps the
/// "a campaign with no plan binds nothing" case honest.
fn campaign(plan: Option<String>, graph: Option<String>, brief: Option<String>) -> RawCampaign {
    let base = common::valid_raw();
    if plan.is_none() {
        return RawCampaign {
            site_plan: plan,
            detail_plan: None,
            layout_graph: graph,
            geometry_brief: brief,
            ..base
        };
    }
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
        site_plan: plan,
        detail_plan: None,
        layout_graph: graph,
        geometry_brief: brief,
        ..base
    }
}

fn codes_of(raw: &RawCampaign) -> Vec<String> {
    check_campaign(raw).into_iter().map(|d| d.code).collect()
}

/// Validate the green campaign with the plan perturbed by one edit.
fn plan_with(patch: impl FnOnce(&mut Value)) -> Vec<String> {
    let mut v: Value = serde_json::from_str(PLAN).expect("the green plan parses");
    patch(&mut v);
    let text = serde_json::to_string(&v).expect("re-serialize");
    codes_of(&campaign(
        Some(text),
        Some(GRAPH.to_string()),
        Some(BRIEF.to_string()),
    ))
}

/// The same, with the message texts, for the assertions that check a refusal
/// says the numbers an author needs.
fn plan_diags(patch: impl FnOnce(&mut Value)) -> Vec<delvewright_dsl::Diagnostic> {
    let mut v: Value = serde_json::from_str(PLAN).expect("the green plan parses");
    patch(&mut v);
    let text = serde_json::to_string(&v).expect("re-serialize");
    check_campaign(&campaign(
        Some(text),
        Some(GRAPH.to_string()),
        Some(BRIEF.to_string()),
    ))
}

fn boxes(v: &mut Value) -> &mut Vec<Value> {
    v["content"]["boxes"].as_array_mut().expect("boxes")
}

fn seams(v: &mut Value) -> &mut Vec<Value> {
    v["content"]["seams"].as_array_mut().expect("seams")
}

/// The index of the box for `node`, by hand from the document above.
fn box_of(node: &str) -> usize {
    match node {
        "node/porch" => 0,
        "node/hall" => 1,
        "node/vault" => 2,
        "node/cellar" => 3,
        "node/yard" => 4,
        "node/pit" => 5,
        other => panic!("no box for {other}"),
    }
}

/// The index of the seam for `edge`, by hand.
fn seam_of(edge: &str) -> usize {
    match edge {
        "edge/porch-hall" => 0,
        "edge/hall-vault" => 1,
        "edge/hall-cellar" => 2,
        "edge/porch-cellar" => 3,
        "edge/vault-yard" => 4,
        "edge/yard-pit" => 5,
        "edge/pit-yard" => 6,
        other => panic!("no seam for {other}"),
    }
}

fn has(codes: &[String], code: &str) -> bool {
    codes.iter().any(|c| c == code)
}

// ---------------------------------------------------------------------------
// The green baseline, and the binding it reports
// ---------------------------------------------------------------------------

/// The map above is accepted, and the only lines it draws are the two advisories
/// this pipeline always draws: the pacing projection, which refuses nothing, and
/// the notice that every verdict above rests on uncalibrated standards.
#[test]
fn the_hand_drawn_plan_is_accepted() {
    let got = codes_of(&campaign(
        Some(PLAN.to_string()),
        Some(GRAPH.to_string()),
        Some(BRIEF.to_string()),
    ));
    assert_eq!(
        got,
        vec!["DW0822".to_string(), "DW0813".to_string()],
        "the green plan should raise nothing but the two advisories"
    );
}

/// A campaign that carries no site plan is untouched, and the ledger says so
/// with a zero rather than by omission.
#[test]
fn a_campaign_with_no_plan_binds_nothing() {
    let raw = campaign(None, None, None);
    assert!(codes_of(&raw).is_empty());
    let c = delvewright_dsl::parse_campaign(&raw).expect("parses");
    let b = delvewright_dsl::LayoutBinding::of(&c);
    assert_eq!(b.plan, delvewright_dsl::PlanBinding::default());
    assert_eq!(b.plan.boxes, 0);
    assert_eq!(b.plan.box_pairs, 0);
}

/// The binding count, **stated by hand** from the document above and compared
/// against what the ledger computes. Written out rather than derived, because a
/// count the code produced and the code checked would agree however wrong it
/// was.
#[test]
fn the_binding_ledger_counts_what_the_plan_holds() {
    let raw = campaign(
        Some(PLAN.to_string()),
        Some(GRAPH.to_string()),
        Some(BRIEF.to_string()),
    );
    let c = delvewright_dsl::parse_campaign(&raw).expect("parses");
    let b = delvewright_dsl::LayoutBinding::of(&c);
    assert_eq!(b.plan.boxes, 6);
    assert_eq!(b.plan.box_pairs, 15, "six places make fifteen pairs");
    assert_eq!(
        b.plan.seams, 7,
        "seven of the eight connections carry a body"
    );
    assert_eq!(b.plan.stair_seams, 2, "hall-cellar and pit-yard");
    assert_eq!(b.plan.drop_seams, 2, "porch-cellar and yard-pit");
    assert_eq!(b.plan.datums, 2);
    assert_eq!(b.plan.volumes, 2);
    assert_eq!(b.plan.identities, 5);
    assert_eq!(b.plan.sightlines, 1);
    assert_eq!(b.plan.views, 1);
    let line = b.plan.line();
    assert!(line.contains("6 box(es) (15 pair(s) compared)"), "{line}");
    assert!(line.contains("7 seam(s) (2 stair, 2 drop)"), "{line}");
    assert!(line.contains("5 identity(ies)"), "{line}");
}

// ---------------------------------------------------------------------------
// The version fence
// ---------------------------------------------------------------------------

/// A site plan exists only at 0.14.0, and the refusal names the document rather
/// than a field inside it — raising one field's version would not make the file
/// legal.
#[test]
fn a_site_plan_below_its_version_is_refused() {
    let got = plan_with(|v| v["dsl_version"] = json!("0.13.0"));
    assert!(has(&got, "DW0141"), "{got:?}");
}

// ---------------------------------------------------------------------------
// THE ORDERING TOOTH (spec-0049 §7.1)
// ---------------------------------------------------------------------------

/// A site plan with no layout graph is refused **by name**, and the refusal
/// names the missing document rather than every dangling reference inside the
/// plan.
///
/// This is the inversion the whole stage exists to make uncompilable: an
/// embedding cannot reach green ahead of the thing it embeds.
#[test]
fn a_plan_without_its_graph_is_refused_naming_the_graph() {
    let diags = check_campaign(&campaign(
        Some(PLAN.to_string()),
        None,
        Some(BRIEF.to_string()),
    ));
    let codes: Vec<&str> = diags.iter().map(|d| d.code.as_str()).collect();
    assert!(codes.contains(&"DW0824"), "{codes:?}");
    let d = diags
        .iter()
        .find(|d| d.code == "DW0824")
        .expect("the tooth");
    assert!(d.message.contains("layout-graph.json"), "{}", d.message);
    // And nothing else is reported about the plan: the missing document is the
    // finding, not the six boxes that now name nothing.
    assert_eq!(
        diags.iter().filter(|d| d.code == "DW0824").count(),
        1,
        "one refusal, naming the missing document: {codes:?}"
    );
}

/// The same for the brief. Both halves of `spec-0049 §7.1` are real.
#[test]
fn a_plan_without_its_brief_is_refused_naming_the_brief() {
    let diags = check_campaign(&campaign(
        Some(PLAN.to_string()),
        Some(GRAPH.to_string()),
        None,
    ));
    let d = diags
        .iter()
        .find(|d| d.code == "DW0824" && d.message.contains("geometry-brief.json"))
        .expect("the brief half of the tooth");
    assert!(d.message.contains("identities"), "{}", d.message);
}

/// **The inversion is not compilable, not merely refused.**
///
/// The two refusals above are the reachable half. This is the structural half,
/// and it is a property of the TYPE rather than of a check: a box carries a
/// required `node`, so there is no site plan — well formed or otherwise — that
/// describes a space without naming the place it is the space of. A document
/// that tries does not parse.
#[test]
fn a_box_cannot_be_written_without_naming_a_place() {
    let got = plan_with(|v| {
        boxes(v)[box_of("node/porch")]
            .as_object_mut()
            .expect("box")
            .remove("node");
    });
    assert!(
        has(&got, "DW0100"),
        "a box with no `node` must fail to PARSE, not merely fail a check: {got:?}"
    );
}

/// And the region cannot be omitted or derived: there is no spelling for
/// "compute the extent from the boxes", so extent-flows-up is unrepresentable
/// rather than forbidden.
#[test]
fn the_region_cannot_be_omitted_or_derived() {
    let missing = plan_with(|v| {
        v["content"]
            .as_object_mut()
            .expect("content")
            .remove("region");
    });
    assert!(has(&missing, "DW0100"), "{missing:?}");
    // There is no "fit" spelling either — the schema takes a box of numbers and
    // nothing else, so a document that asks for a derived extent does not parse.
    let derived = plan_with(|v| v["content"]["region"] = json!("fit"));
    assert!(has(&derived, "DW0100"), "{derived:?}");
}

// ---------------------------------------------------------------------------
// DW0824 — the graph and the plan agree exactly
// ---------------------------------------------------------------------------

#[test]
fn a_place_with_no_box_is_refused() {
    let got = plan_with(|v| {
        boxes(v).remove(box_of("node/pit"));
        // and its seams, so the only finding is the missing box
        let s = seams(v);
        s.remove(seam_of("edge/pit-yard"));
        s.remove(seam_of("edge/yard-pit"));
    });
    assert!(has(&got, "DW0824"), "{got:?}");
}

#[test]
fn a_box_naming_no_place_is_refused() {
    let got = plan_with(|v| boxes(v)[box_of("node/pit")]["node"] = json!("node/nowhere"));
    assert!(has(&got, "DW0824"), "{got:?}");
}

#[test]
fn a_connection_with_no_seam_is_refused() {
    let got = plan_with(|v| {
        seams(v).remove(seam_of("edge/vault-yard"));
    });
    let d = plan_diags(|v| {
        seams(v).remove(seam_of("edge/vault-yard"));
    });
    assert!(has(&got, "DW0824"), "{got:?}");
    assert!(
        d.iter()
            .any(|x| x.code == "DW0824" && x.message.contains("allocated, not")),
        "the refusal says why allocation is the point"
    );
}

#[test]
fn a_second_seam_for_one_connection_is_refused() {
    let got = plan_with(|v| {
        let extra = seams(v)[seam_of("edge/vault-yard")].clone();
        seams(v).push(extra);
    });
    assert!(has(&got, "DW0824"), "{got:?}");
}

/// A `vision` connection carries a sightline and a traversal one carries a seam;
/// swapping them is refused in both directions.
#[test]
fn a_vision_connection_may_not_carry_a_seam() {
    let got = plan_with(|v| {
        seams(v)[seam_of("edge/vault-yard")]["edge"] = json!("edge/porch-vault-sightline");
    });
    assert!(has(&got, "DW0824"), "{got:?}");
}

#[test]
fn a_traversal_connection_may_not_carry_a_sightline() {
    let got = plan_with(|v| {
        v["content"]["sightlines"][0]["edge"] = json!("edge/vault-yard");
    });
    assert!(has(&got, "DW0824"), "{got:?}");
}

/// A sightline whose end is not in the place its connection names would send the
/// stage-5 proof to walk a different segment and call the answer the vista's.
#[test]
fn a_sightline_end_outside_its_place_is_refused() {
    let got = plan_with(|v| v["content"]["sightlines"][0]["to"] = json!([20, 65, 8]));
    assert!(has(&got, "DW0824"), "{got:?}");
}

/// Stair massing may only be declared on a stair, and only in one of the two
/// places that stair joins.
#[test]
fn stair_massing_on_a_walk_is_refused() {
    let got = plan_with(|v| {
        seams(v)[seam_of("edge/porch-hall")]["stair_in"] = json!("node/porch");
    });
    assert!(has(&got, "DW0824"), "{got:?}");
}

#[test]
fn stair_massing_in_a_third_place_is_refused() {
    let got = plan_with(|v| {
        seams(v)[seam_of("edge/hall-cellar")]["stair_in"] = json!("node/yard");
    });
    assert!(has(&got, "DW0824"), "{got:?}");
}

// ---------------------------------------------------------------------------
// DW0825 / DW0826 / DW0827 — the boxes and the region
// ---------------------------------------------------------------------------

/// The quantum is 4, so a 6-block footprint is off the grid. The refusal carries
/// the two multiples an author would move to.
#[test]
fn a_box_off_the_kit_grid_is_refused_with_both_numbers() {
    let d = plan_diags(|v| boxes(v)[box_of("node/porch")]["extent"] = json!([6, 8]));
    let msg = d
        .iter()
        .find(|x| x.code == "DW0825")
        .map(|x| x.message.clone())
        .unwrap_or_default();
    assert!(!msg.is_empty(), "{d:?}");
    assert!(msg.contains('4') && msg.contains('8'), "{msg}");
}

/// The region is the brief's number flowing down, and a box is never grounds to
/// grow it — which is what the prescription says.
#[test]
fn a_box_outside_the_region_is_refused_and_the_region_is_not_grown() {
    let d = plan_diags(|v| boxes(v)[box_of("node/vault")]["min"] = json!([60, 4]));
    let msg = d
        .iter()
        .find(|x| x.code == "DW0826")
        .map(|x| x.message.clone())
        .unwrap_or_default();
    assert!(!msg.is_empty(), "{d:?}");
    assert!(msg.contains("never grounds to grow"), "{msg}");
}

/// The whole's own mass answers to the region too: a massif outside it is the
/// extent flowing up through the back door.
#[test]
fn a_volume_outside_the_region_is_refused() {
    let got = plan_with(|v| v["content"]["volumes"][0]["region"]["min"] = json!([4, 48, 40]));
    assert!(has(&got, "DW0826"), "{got:?}");
}

/// Two places may share a face; they may never share a cell.
#[test]
fn overlapping_boxes_are_refused_with_the_intersection() {
    // Slide the vault one cell west so it stands in the hall's east wall AND in
    // the hall itself: hall is x 13..28, so a vault at x 28..35 overlaps at 28.
    let d = plan_diags(|v| boxes(v)[box_of("node/vault")]["min"] = json!([28, 4]));
    let msg = d
        .iter()
        .find(|x| x.code == "DW0827")
        .map(|x| x.message.clone())
        .unwrap_or_default();
    assert!(!msg.is_empty(), "{d:?}");
    assert!(msg.contains("x 28..28"), "the intersection is named: {msg}");
}

/// The whole's mass stands beside a place, under it and over it — never in it.
#[test]
fn a_volume_inside_a_box_is_refused() {
    // The sky reservation dropped to grade lands inside the yard.
    let d = plan_diags(|v| v["content"]["volumes"][1]["region"]["min"] = json!([30, 64, 13]));
    let msg = d
        .iter()
        .find(|x| x.code == "DW0835")
        .map(|x| x.message.clone())
        .unwrap_or_default();
    assert!(!msg.is_empty(), "{d:?}");
    assert!(msg.contains("node/yard"), "{msg}");
}

// ---------------------------------------------------------------------------
// DW0828 / DW0829 — seams are allocated on a face both boxes already have
// ---------------------------------------------------------------------------

/// Two boxes one cell apart share a face. Two cells apart, they do not — and the
/// refusal says so with the gap it measured.
#[test]
fn a_seam_on_a_face_the_boxes_do_not_share_is_refused() {
    let d = plan_diags(|v| boxes(v)[box_of("node/vault")]["min"] = json!([31, 4]));
    let msg = d
        .iter()
        .find(|x| x.code == "DW0828")
        .map(|x| x.message.clone())
        .unwrap_or_default();
    assert!(!msg.is_empty(), "{d:?}");
    assert!(
        msg.contains("2 cells between them"),
        "the gap it measured: {msg}"
    );
    assert!(msg.contains("already have"), "{msg}");
}

/// The right two boxes, the wrong face of the first one.
#[test]
fn a_seam_on_the_wrong_face_of_the_right_boxes_is_refused() {
    let got = plan_with(|v| seams(v)[seam_of("edge/porch-hall")]["face"] = json!("west"));
    assert!(has(&got, "DW0828"), "{got:?}");
}

/// `at` names the opening's low corner on the shared face's own two axes; a
/// corner off that face allocates the seam nowhere.
#[test]
fn a_seam_anchored_off_the_shared_face_is_refused() {
    let got = plan_with(|v| seams(v)[seam_of("edge/porch-hall")]["at"] = json!([40, 64]));
    assert!(has(&got, "DW0828"), "{got:?}");
}

/// The opening is a standard from the metrics table, never quietly cropped to
/// fit: `hall|vault` share a face four cells tall, and a five-tall gateway
/// overruns it.
#[test]
fn an_opening_that_does_not_fit_the_shared_face_is_refused() {
    let d = plan_diags(|v| seams(v)[seam_of("edge/hall-vault")]["opening"] = json!("gateway"));
    let msg = d
        .iter()
        .find(|x| x.code == "DW0829")
        .map(|x| x.message.clone())
        .unwrap_or_default();
    assert!(!msg.is_empty(), "{d:?}");
    assert!(msg.contains("5x5"), "{msg}");
}

/// An opening name the table does not define cannot compile — the table is the
/// single authority for this vocabulary, so no check downstream ever meets one.
#[test]
fn an_opening_the_table_does_not_define_is_refused() {
    let got = plan_with(|v| seams(v)[seam_of("edge/hall-vault")]["opening"] = json!("portcullis"));
    assert!(has(&got, "DW0812"), "{got:?}");
}

/// A body standing on the floor of a side it enters from must be able to get
/// onto the sill. Two blocks up is past a jump.
#[test]
fn a_sill_a_body_cannot_reach_is_refused() {
    let d = plan_diags(|v| seams(v)[seam_of("edge/hall-vault")]["at"] = json!([6, 66]));
    let msg = d
        .iter()
        .find(|x| x.code == "DW0829" && x.message.contains("sill"))
        .map(|x| x.message.clone())
        .unwrap_or_default();
    assert!(!msg.is_empty(), "{d:?}");
    assert!(msg.contains("2 blocks over the floor"), "{msg}");
}

// ---------------------------------------------------------------------------
// DW0830 / DW0831 — the climb and the fall
// ---------------------------------------------------------------------------

/// `pit` hosts a stair that climbs 5 to the yard, and the gentlest standard
/// pitch needs 5 blocks of run. Shrink the pit to a 4-block footprint and no
/// standard pitch fits — the refusal names the rise, the run needed and the run
/// available, which are the three numbers a plan edit needs.
#[test]
fn a_stair_that_no_standard_pitch_fits_is_refused_with_its_numbers() {
    let d = plan_diags(|v| boxes(v)[box_of("node/pit")]["extent"] = json!([4, 4]));
    let msg = d
        .iter()
        .find(|x| x.code == "DW0830")
        .map(|x| x.message.clone())
        .unwrap_or_default();
    assert!(!msg.is_empty(), "{d:?}");
    assert!(msg.contains("climbs 5 block(s)"), "{msg}");
    assert!(msg.contains("affords 4"), "{msg}");
}

/// A stair that climbs nothing is a walk that has been called a stair, and the
/// rise is not authored — it IS the difference between two floors the plan
/// already chose.
///
/// Two codes come back from this one edit, and that is the model working rather
/// than a muddy test: `cellar` is the foot of the stair AND the floor the porch
/// drops onto, so raising its plane to grade makes the stair climb nothing
/// (`DW0830`) and the fall fall nothing (`DW0831`) in the same breath. A rise
/// authored per seam could have been edited on one and left wrong on the other;
/// derived from the floor, there is one number and it moves once.
#[test]
fn a_stair_between_two_places_on_one_plane_is_refused() {
    let got = plan_with(|v| {
        // Raise the cellar to grade: the hall's west face is still shared with
        // it, and what changes is that the stair now climbs zero.
        boxes(v)[box_of("node/cellar")]["floor"] = json!({ "y": 64 });
    });
    assert!(has(&got, "DW0830"), "{got:?}");
    assert!(has(&got, "DW0831"), "one datum, both relations: {got:?}");
}

/// A stair has to stand somewhere, and the plan says where.
#[test]
fn a_stair_with_no_host_is_refused() {
    let got = plan_with(|v| {
        seams(v)[seam_of("edge/pit-yard")]
            .as_object_mut()
            .expect("seam")
            .remove("stair_in");
    });
    assert!(has(&got, "DW0830"), "{got:?}");
}

/// The designed-drop cap is a **policy** cap, deliberately far tighter than what
/// a body survives. The green plan's two drops sit exactly on it; one deeper is
/// refused.
#[test]
fn a_drop_past_the_designed_cap_is_refused_as_policy() {
    let d = plan_diags(|v| {
        // Sink the pit one block. Its ceiling course still meets the yard's
        // floor course, so the face stays shared and only the fall grows.
        boxes(v)[box_of("node/pit")]["floor"] = json!({ "y": 58 });
        boxes(v)[box_of("node/pit")]["ceiling"] = json!({ "clearance": 5 });
    });
    let msg = d
        .iter()
        .find(|x| x.code == "DW0831")
        .map(|x| x.message.clone())
        .unwrap_or_default();
    assert!(!msg.is_empty(), "{d:?}");
    assert!(msg.contains("drops 6 blocks"), "{msg}");
    assert!(msg.contains("policy"), "{msg}");
}

/// A drop that rises is a mislabelled stair.
#[test]
fn a_drop_that_rises_is_refused() {
    let got = plan_with(|v| {
        // Turn the yard→pit fall around by lifting the pit above the yard's
        // floor while keeping the shared plane.
        v["content"]["boxes"][box_of("node/pit")]["floor"] = json!({ "y": 70 });
    });
    assert!(has(&got, "DW0831") || has(&got, "DW0828"), "{got:?}");
}

// ---------------------------------------------------------------------------
// DW0832 — the size-class ladder becomes geometry
// ---------------------------------------------------------------------------

/// The hall is a `room`, whose footprint runs 8..16; a 32-block hall is not one.
#[test]
fn a_box_outside_its_size_class_is_refused() {
    let d = plan_diags(|v| boxes(v)[box_of("node/hall")]["extent"] = json!([32, 16]));
    let msg = d
        .iter()
        .find(|x| x.code == "DW0832")
        .map(|x| x.message.clone())
        .unwrap_or_default();
    assert!(!msg.is_empty(), "{d:?}");
    assert!(msg.contains("outside the class's 8..16"), "{msg}");
}

/// Headroom answers to the class too.
///
/// Shown on the `yard`, whose class (`hall`) asks for eight cells — not on the
/// hall itself, whose class (`room`) asks for four, so four is legal there. The
/// first draft of this test asserted the wrong one and went green on an
/// identity failure instead, which is what a hand-stated fixture is for.
#[test]
fn a_box_under_its_class_clearance_is_refused() {
    let got = plan_with(|v| boxes(v)[box_of("node/yard")]["ceiling"] = json!({ "clearance": 4 }));
    assert!(has(&got, "DW0832"), "{got:?}");
}

// ---------------------------------------------------------------------------
// DW0833 / DW0834 — the plan held to the brief
// ---------------------------------------------------------------------------

/// The identity names both numbers and quotes the brief's own sentence, because
/// the author's next action is deciding which of the two to move.
#[test]
fn a_broken_identity_names_both_numbers_and_the_briefs_sentence() {
    let d = plan_diags(|v| {
        boxes(v)[box_of("node/hall")]["ceiling"] = json!({ "clearance": 12 });
    });
    let msg = d
        .iter()
        .find(|x| x.code == "DW0833")
        .map(|x| x.message.clone())
        .unwrap_or_default();
    assert!(!msg.is_empty(), "{d:?}");
    assert!(msg.contains("measures 12"), "{msg}");
    assert!(msg.contains("exactly 8"), "{msg}");
    assert!(msg.contains("so the vault reads as tall"), "{msg}");
}

/// Every one of the five measures is a real measurement, and each can be broken
/// on its own. Written as a table of one-field perturbations, each with the
/// answer stated by hand.
#[test]
fn every_measure_is_falsifiable_on_its_own() {
    // (which identity, what to change, why the changed plan breaks it)
    let region = plan_with(|v| v["content"]["region"]["extent"] = json!([60, 48, 64]));
    assert!(has(&region, "DW0833"), "region extent 60 != 64: {region:?}");

    let span = plan_with(|v| {
        boxes(v)[box_of("node/hall")]["extent"] = json!([12, 16]);
    });
    assert!(has(&span, "DW0833"), "hall span 12 != 16: {span:?}");

    let height = plan_with(|v| boxes(v)[box_of("node/hall")]["ceiling"] = json!({"clearance": 9}));
    assert!(has(&height, "DW0833"), "hall height 9 != 8: {height:?}");

    // The porch centre is (7.5, 7.5) and the vault centre (33.5, 7.5), which is
    // 26 apart. Slide the vault back to x 20..27 and the centres are 16 apart,
    // under the 24 the brief asks for.
    let standoff = plan_with(|v| boxes(v)[box_of("node/vault")]["min"] = json!([20, 4]));
    assert!(has(&standoff, "DW0833"), "16 < 24: {standoff:?}");

    let grade = plan_with(|v| v["content"]["datums"][0]["y"] = json!(65));
    assert!(has(&grade, "DW0833"), "grade 65 != 64: {grade:?}");
}

/// A plan with no identity binds the whole map to nothing, and says so on every
/// run. A warning rather than a refusal, so a deliberately minimal plan stays
/// compilable — and printed, so the emptiness is never quietly a pass.
#[test]
fn an_empty_identity_gate_is_a_stated_finding_not_a_silent_pass() {
    let d = plan_diags(|v| v["content"]["identities"] = json!([]));
    let found = d
        .iter()
        .find(|x| x.code == "DW0834")
        .expect("the empty gate is named");
    assert_eq!(found.severity, delvewright_dsl::Severity::Warning);
    assert!(found.message.contains("the plan declares no identity"));
    // And it does not turn a validate red.
    assert!(
        !d.iter()
            .any(|x| x.severity == delvewright_dsl::Severity::Error)
    );
}

/// The other side of the same emptiness.
#[test]
fn an_empty_brief_is_named_as_the_empty_side() {
    let empty_brief = r#"{"campaign_id":"hello-world","dsl_version":"0.13.0",
      "stage":"geometry-brief","content":{}}"#;
    let d = check_campaign(&campaign(
        Some(PLAN.to_string()),
        Some(GRAPH.to_string()),
        Some(empty_brief.to_string()),
    ));
    assert!(
        d.iter()
            .any(|x| x.code == "DW0834" && x.message.contains("the brief states no fact")),
        "{d:?}"
    );
}

/// An identity may only bind to a number the brief wrote down.
#[test]
fn an_identity_naming_no_fact_is_refused() {
    let got = plan_with(|v| v["content"]["identities"][0]["fact"] = json!("fact/nowhere"));
    assert!(has(&got, "DW0824"), "{got:?}");
}

// ---------------------------------------------------------------------------
// Plan-internal references and ids
// ---------------------------------------------------------------------------

#[test]
fn a_floor_naming_no_datum_is_a_dangling_reference() {
    let got = plan_with(|v| {
        boxes(v)[box_of("node/porch")]["floor"] = json!({ "datum": "datum/nowhere" });
    });
    assert!(has(&got, "DW0112"), "{got:?}");
}

#[test]
fn duplicate_and_malformed_plan_ids_are_the_ordinary_id_rules() {
    let dup = plan_with(|v| v["content"]["datums"][1]["id"] = json!("datum/grade"));
    assert!(has(&dup, "DW0111"), "{dup:?}");
    let bad = plan_with(|v| v["content"]["views"][0]["id"] = json!("view/Not Kebab"));
    assert!(has(&bad, "DW0110"), "{bad:?}");
}

/// The lighting block is the engine's existing area-lighting object, so it
/// answers the same range rule with the same code — not a twin of it.
#[test]
fn the_plans_lighting_answers_the_same_range_rule_an_areas_does() {
    let got = plan_with(|v| v["content"]["lighting"]["min_light"] = json!(15));
    assert!(has(&got, "DW0196"), "{got:?}");
}

// ---------------------------------------------------------------------------
// One campaign, one placement authority (spec-0049 §6)
// ---------------------------------------------------------------------------

/// `DW0839`: `areas[]` and a site plan in one campaign is two owners for one
/// question.
///
/// The perturbation is the smallest one that exists — the campaign gets its
/// `areas[]` back — so the green above and the red here differ in exactly the
/// thing the rule is about.
#[test]
fn two_placement_authorities_in_one_campaign_are_refused() {
    let mut raw = campaign(
        Some(PLAN.to_string()),
        Some(GRAPH.to_string()),
        Some(BRIEF.to_string()),
    );
    raw.world = common::valid_raw().world; // the `areas[]` this campaign gave up
    let got = codes_of(&raw);
    assert!(
        has(&got, "DW0839"),
        "a world with both authorities must be refused: {got:?}"
    );
    let d = check_campaign(&raw)
        .into_iter()
        .find(|d| d.code == "DW0839")
        .expect("the refusal is present");
    assert_eq!(d.stage, "world");
    assert!(
        d.message.contains("fixed stride") && d.message.contains("region"),
        "the refusal says what each authority owns: {}",
        d.message
    );
}

// ---------------------------------------------------------------------------
// The opener obligation's byte-side half (spec-0049 §3.3 `DW0818`)
// ---------------------------------------------------------------------------

/// `DW0818`: a barred way nothing opens is a wall, and the region such an effect
/// must target is the seam's own.
///
/// The graph half of this rule already stood: a `barred` edge must declare a
/// `gating` naming a flag something really sets. That says the way is MEANT to
/// open. This is the other half, and it could not be written before this round
/// because the region an opener addresses did not exist until the derivation
/// synthesized it.
#[test]
fn a_barred_way_no_effect_opens_is_refused_by_name() {
    // The green campaign's `open-gate` names the seam. Point it somewhere else —
    // an anchor no seam owns — and the barred way has nothing that opens it.
    let mut raw = campaign(
        Some(PLAN.to_string()),
        Some(GRAPH.to_string()),
        Some(BRIEF.to_string()),
    );
    raw.quests = raw
        .quests
        .replace("\"anchor/seam-hall-vault\"", "\"anchor/node-porch\"");
    let d = check_campaign(&raw);
    let got: Vec<String> = d.iter().map(|x| x.code.clone()).collect();
    assert!(
        has(&got, "DW0818"),
        "a barred way nothing opens must be refused: {got:?}"
    );
    let m = d
        .iter()
        .find(|x| x.code == "DW0818" && x.message.contains("barred and nothing"))
        .expect("the opener refusal is present");
    assert_eq!(m.stage, "layout-graph", "the fault is the graph's claim");
    assert!(
        m.message.contains("anchor/seam-hall-vault"),
        "the refusal names the region an opener must address: {}",
        m.message
    );
}

/// The same obligation, satisfied by a `shortcut` rather than by an `open-gate`.
///
/// Two verbs open a gate and the rule knows both, which is what stops it from
/// being a rule about `open-gate` that a shortcut campaign has to work around.
#[test]
fn a_shortcut_satisfies_the_opener_obligation_too() {
    let mut raw = campaign(
        Some(PLAN.to_string()),
        Some(GRAPH.to_string()),
        Some(BRIEF.to_string()),
    );
    // Drop the `open-gate` and lift the bar from the far side instead.
    let mut quests: Value = serde_json::from_str(&raw.quests).expect("quests parse");
    quests["content"]["quests"][0]["on_objective_complete"]["obj/talk"] = json!([]);
    quests["content"]["shortcuts"] = json!([{
        "id": "shortcut/vault-door",
        "gate": "anchor/seam-hall-vault",
        "unlock": "anchor/unlock-hall-vault"
    }]);
    raw.quests = serde_json::to_string(&quests).expect("re-serialize");
    let got = codes_of(&raw);
    assert!(
        !got.iter().any(|c| c == "DW0818"),
        "a shortcut lifting the bar is an opener: {got:?}"
    );
}

/// `DW0830`: a stair's treads stand in the LOWER of the two places, and hosting
/// them in the higher one is refused with both floors.
///
/// **Found by building, not by reading.** This check used to ask only whether
/// the named host affords the RUN, so a plan hosting a downward stair in the
/// upper place reached green here — and the derivation then laid a mound on the
/// wrong side of the opening and filled the hole it was supposed to arrive at.
/// The stage-5 observer caught that as a seam whose opening was still solid,
/// which is a true refusal for the wrong defect: the repair belongs to the plan,
/// and this is where the plan is judged.
///
/// The perturbation is the one field: the green plan hosts `edge/hall-cellar` in
/// `cellar`, and this moves it to `hall`.
#[test]
fn stair_massing_in_the_higher_place_is_refused_with_both_floors() {
    let d = plan_diags(|v| {
        seams(v)[seam_of("edge/hall-cellar")]["stair_in"] = json!("node/hall");
    });
    let msg = d
        .iter()
        .find(|x| x.code == "DW0830" && x.message.contains("HIGHER"))
        .map(|x| x.message.clone())
        .unwrap_or_default();
    assert!(!msg.is_empty(), "{d:?}");
    assert!(msg.contains("`node/hall` stands at y 64"), "{msg}");
    assert!(msg.contains("`node/cellar` at y 59"), "{msg}");
    assert!(
        msg.contains("Host it in `node/cellar`"),
        "the refusal names the place the treads belong in: {msg}"
    );
}

// ---------------------------------------------------------------------------
// Every stage-5 verb reaches a DERIVED world's anchors
// ---------------------------------------------------------------------------

/// The names the derivation synthesizes for this fixture's graph — the whole
/// spatial vocabulary a site-plan campaign has, because there is no prefab.
const SYNTHESIZED: &[&str] = &[
    "anchor/node-porch",
    "anchor/node-hall",
    "anchor/node-vault",
    "anchor/node-cellar",
    "anchor/node-yard",
    "anchor/node-pit",
    "anchor/seam-hall-vault",
    "anchor/unlock-hall-vault",
];

/// **A site-plan campaign can author every stage-5 verb, and this is the test
/// that was missing.**
///
/// Anchor resolution — *is this name one some area provides?* — was asked in
/// eleven places in `dsl::validate`, each walking `world.areas` and the prefab
/// registry itself. When a site plan became a second placement authority, ONE of
/// the eleven was taught that a derived world's anchors come from
/// `siteplan::synthesized_anchors` instead. The other ten went on enumerating
/// prefabs a derived world does not have, so a `shortcut` naming the very
/// `anchor/unlock-<edge>` the derivation places for it was refused as an invented
/// name — and so were a trap, a shop, a loot chest, a lethal volume, an actor and
/// a trigger. Every stage-5 verb but the ones that happen to walk was
/// unauthorable on a derived map.
///
/// **Nothing was red**, and nothing could have been: a check resolving against a
/// smaller world than the campaign has refuses CONTENT, not itself. It surfaced
/// only when somebody tried to write the second verb.
///
/// The assertion is deliberately about the FAILURE SHAPE rather than about a
/// list of codes: any diagnostic that names a synthesized anchor and says it
/// comes from no prefab is this defect, whichever verb raised it. That is what
/// makes the test cover the eleventh walk somebody writes next.
#[test]
fn no_stage_five_verb_calls_a_synthesized_anchor_an_invented_name() {
    let mut raw = campaign(
        Some(PLAN.to_string()),
        Some(GRAPH.to_string()),
        Some(BRIEF.to_string()),
    );
    let mut quests: Value = serde_json::from_str(&raw.quests).expect("quests parse");
    // **The fence, or this test proves nothing.** Every verb below sits behind a
    // per-stage `dsl_version` fence, and the hello-world fixture's quests stage
    // is 0.2.0 — so the first draft of this test added seven verbs to a document
    // none of their checks were allowed to look at, and passed on the UNREPAIRED
    // tree. That is the constitution's `unfenced` vacuity mode exactly, and it
    // was caught only by red-demoing the repair it was written for.
    quests["dsl_version"] = json!("0.14.0");
    // The fixture is a 0.2.0 document, and the newer stage requires an
    // objective's player-facing `title`.
    quests["content"]["quests"][0]["objectives"][1]["title"] = json!("Leave by the vault");
    let c = &mut quests["content"];

    // The bar is lifted from the far side rather than by the objective, so the
    // opener obligation is still satisfied and the `shortcut` is real.
    c["quests"][0]["on_objective_complete"]["obj/talk"] = json!([]);
    c["shortcuts"] = json!([{
        "id": "shortcut/vault-door",
        "gate": "anchor/seam-hall-vault",
        "unlock": "anchor/unlock-hall-vault"
    }]);
    c["shops"] = json!([{
        "id": "shop/counter",
        "anchor": "anchor/node-hall",
        "title": "The counter",
        "marker_item": "minecraft:emerald",
        "offers": [{
            "label": "Bread, one emerald",
            "effects": [{ "type": "give-item", "item": "minecraft:bread", "count": 1 }]
        }]
    }]);
    c["loot"] = json!([{
        "id": "loot/cellar-chest",
        "anchor": "anchor/node-cellar",
        "items": [{ "item": "minecraft:bread", "count": 1 }]
    }]);
    c["lethal_volumes"] = json!([{
        "id": "lethal/the-sump",
        "damage_type": "fall",
        "message": "The floor of the pit is not a floor.",
        "region": { "anchor": "anchor/node-pit", "extent": [2, 2, 2] }
    }]);
    c["actors"] = json!([{
        "id": "actor/yard-moth",
        "anchor": "anchor/node-yard",
        "entity": "minecraft:zombie",
        "name": "The Yard Watcher"
    }]);
    c["triggers"] = json!([{
        "id": "trigger/the-bar",
        "at": "anchor/seam-hall-vault",
        "on": { "on": "use" },
        "once": false,
        "audience": "presser",
        "effects": [{
            "type": "narrate", "style": "actionbar",
            "text": "The bar does not lift from this side."
        }]
    }]);
    raw.quests = serde_json::to_string(&quests).expect("re-serialize");

    let bad: Vec<String> = check_campaign(&raw)
        .into_iter()
        .filter(|d| {
            d.message.contains("prefab") && SYNTHESIZED.iter().any(|a| d.message.contains(a))
        })
        .map(|d| format!("{} {}", d.code, d.message))
        .collect();
    assert!(
        bad.is_empty(),
        "a derived world has no prefab to ask, and these checks asked one anyway:\n{}",
        bad.join("\n")
    );
}

/// The live instance, named on its own so a regression says which verb.
///
/// The pre-existing `a_shortcut_satisfies_the_opener_obligation_too` above
/// asserted the absence of ONE code (`DW0818`) on this exact document — while
/// `DW0371` was firing on it the whole time. A test that checks for one code and
/// ignores the rest of the verdict is how a refusal hides inside a green test.
#[test]
fn a_shortcut_on_a_derived_world_resolves_its_gate_and_its_unlock() {
    let mut raw = campaign(
        Some(PLAN.to_string()),
        Some(GRAPH.to_string()),
        Some(BRIEF.to_string()),
    );
    let mut quests: Value = serde_json::from_str(&raw.quests).expect("quests parse");
    // `shortcut_checks` is fenced at 0.6.0; below it the declaration is parsed
    // and never examined, which is a green that means nothing.
    quests["dsl_version"] = json!("0.14.0");
    quests["content"]["quests"][0]["on_objective_complete"]["obj/talk"] = json!([]);
    quests["content"]["shortcuts"] = json!([{
        "id": "shortcut/vault-door",
        "gate": "anchor/seam-hall-vault",
        "unlock": "anchor/unlock-hall-vault"
    }]);
    raw.quests = serde_json::to_string(&quests).expect("re-serialize");
    let got = codes_of(&raw);
    assert!(
        !has(&got, "DW0371"),
        "the derivation places both anchors; nothing here is invented: {got:?}"
    );
}

// ---------------------------------------------------------------------------
// A refusal never prescribes a document this campaign may not write
// ---------------------------------------------------------------------------

/// The prescriptions a **derived** map's author cannot carry out, and the reason
/// each one is on this list. Every entry is a fragment some refusal in
/// `dsl::validate` printed before `crate::placement` existed.
///
/// They are not "words about prefabs" — a refusal is allowed to SAY that no
/// prefab provides a name, because on a derived map that is true and vacuous.
/// Each of these is an **instruction**: go and do this thing. On a site-plan
/// campaign each one is refused by another gate, or names a document that does
/// not exist and must not be created.
const FORBIDDEN_ON_A_DERIVED_MAP: &[(&str, &str)] = &[
    (
        "declare it in stage-1 `world.areas`",
        "`DW0839` refuses `areas[]` in a campaign carrying a site plan, and `DW0160` refuses an \
         entry with no prefab bound to it",
    ),
    (
        "use one of the world stage's area ids",
        "that set is required to be EMPTY here (`DW0839`), so the instruction names nothing",
    ),
    (
        "anchor names come from prefab metadata",
        "a derived map has no prefab and no metadata to read",
    ),
    (
        "do NOT invent one",
        "a derived map's anchor names ARE synthesized from node ids the author picked; this \
         forbids the only correct answer",
    ),
    (
        "bind a prefab/pool",
        "binding a prefab means declaring an `areas[]` entry, which `DW0839` refuses",
    ),
    (
        "an anchor the prefab exposes",
        "there is no prefab to expose one",
    ),
    (
        "an anchor a prefab exposes",
        "there is no prefab to expose one",
    ),
    (
        "an anchor some area's prefab exposes",
        "there is no area with a prefab",
    ),
    (
        "bind the trap to an `anchor/trap` marker",
        "an `anchor/trap` marker comes from prefab metadata a derived map does not have",
    ),
];

/// Every area and anchor reference this fixture breaks on purpose. The count is
/// the test's binding and is read off THIS list rather than written down beside
/// it, so a planted name that stops being examined fails the test instead of
/// quietly shrinking the population.
fn planted_bad_names() -> Vec<&'static str> {
    vec![
        "area/nowhere",
        "anchor/nowhere-npc",
        "anchor/nowhere-objective",
        "anchor/nowhere-shortcut-gate",
        "anchor/nowhere-shortcut-unlock",
        "anchor/nowhere-shop",
        "anchor/nowhere-loot",
        "anchor/nowhere-lethal",
        "anchor/nowhere-actor",
        "anchor/nowhere-trigger",
    ]
}

/// **A refusal on a derived map never sends its author to a prefab.**
///
/// The companion of
/// [`no_stage_five_verb_calls_a_synthesized_anchor_an_invented_name`], one step
/// on. That test proves a name the derivation really places RESOLVES; this one
/// proves that when a name genuinely does not resolve, the sentence the author
/// reads afterwards prescribes something they are allowed to do.
///
/// The pair it exists for: `DW0112` printed *"declare it in stage-1
/// `world.areas`"*, which is exactly what `DW0839` refuses in a campaign
/// carrying a site plan; `DW0142` printed *"anchor names come from prefab
/// metadata; do NOT invent one"* against names that are synthesized by design.
/// CLAUDE.md: when one gate's prescription is another gate's refusal, the defect
/// belongs to the PAIR, and a gate that names a remedy owes a check that the
/// remedy is reachable. This is that check.
///
/// The assertion is deliberately about the FORBIDDEN PRESCRIPTION rather than
/// about a list of codes, so it covers the eighteenth refusal site somebody
/// writes without that site having to know this test exists.
#[test]
fn no_refusal_on_a_derived_map_prescribes_a_prefab_document() {
    let mut raw = campaign(
        Some(PLAN.to_string()),
        Some(GRAPH.to_string()),
        Some(BRIEF.to_string()),
    );

    // Every verb below is fenced; the quests stage of the hello-world fixture is
    // 0.2.0, and adding a declaration a check is not allowed to look at is the
    // `unfenced` vacuity mode.
    let mut quests: Value = serde_json::from_str(&raw.quests).expect("quests parse");
    quests["dsl_version"] = json!("0.14.0");
    quests["content"]["quests"][0]["objectives"][1]["title"] = json!("Leave by the vault");
    quests["content"]["quests"][0]["on_objective_complete"]["obj/talk"] = json!([]);
    let c = &mut quests["content"];
    c["quests"][0]["objectives"][1]["anchor"] = json!("anchor/nowhere-objective");
    c["shortcuts"] = json!([{
        "id": "shortcut/vault-door",
        "gate": "anchor/nowhere-shortcut-gate",
        "unlock": "anchor/nowhere-shortcut-unlock"
    }]);
    c["shops"] = json!([{
        "id": "shop/counter",
        "anchor": "anchor/nowhere-shop",
        "title": "The counter",
        "marker_item": "minecraft:emerald",
        "offers": [{
            "label": "Bread, one emerald",
            "effects": [{ "type": "give-item", "item": "minecraft:bread", "count": 1 }]
        }]
    }]);
    c["loot"] = json!([{
        "id": "loot/cellar-chest",
        "anchor": "anchor/nowhere-loot",
        "items": [{ "item": "minecraft:bread", "count": 1 }]
    }]);
    c["lethal_volumes"] = json!([{
        "id": "lethal/the-sump",
        "damage_type": "fall",
        "message": "The floor of the pit is not a floor.",
        "region": { "anchor": "anchor/nowhere-lethal", "extent": [2, 2, 2] }
    }]);
    c["actors"] = json!([{
        "id": "actor/yard-moth",
        "anchor": "anchor/nowhere-actor",
        "entity": "minecraft:zombie",
        "name": "The Yard Watcher"
    }]);
    c["triggers"] = json!([{
        "id": "trigger/the-bar",
        "at": "anchor/nowhere-trigger",
        "on": { "on": "use" },
        "once": false,
        "audience": "presser",
        "effects": [{
            "type": "narrate", "style": "actionbar",
            "text": "The bar does not lift from this side."
        }]
    }]);
    raw.quests = serde_json::to_string(&quests).expect("re-serialize");
    raw.npcs = raw
        .npcs
        .replace("\"anchor/node-porch\"", "\"anchor/nowhere-npc\"");

    let anchors_broken = check_campaign(&raw);

    // A broken `area` makes `AnchorProviders::for_area` return `None`, which
    // SKIPS the per-area anchor rules — so the area case is a SECOND campaign,
    // or this test would silently measure fewer sites than exist.
    let mut area_raw = raw.clone();
    area_raw.npcs = area_raw.npcs.replace("\"area/site\"", "\"area/nowhere\"");
    area_raw.quest_plan = area_raw
        .quest_plan
        .replace("\"area/site\"", "\"area/nowhere\"");
    let areas_broken = check_campaign(&area_raw);

    let all: Vec<delvewright_dsl::Diagnostic> =
        anchors_broken.into_iter().chain(areas_broken).collect();

    // Binding, computed from the fixture: every name planted above must reach a
    // refusal, or this test is examining a smaller population than it claims.
    let unexamined: Vec<&str> = planted_bad_names()
        .into_iter()
        .filter(|name| !all.iter().any(|d| d.message.contains(name)))
        .collect();
    assert!(
        unexamined.is_empty(),
        "these deliberately-broken references reached no refusal, so this test binds to less \
         than it says: {unexamined:?}"
    );

    let bad: Vec<String> = all
        .iter()
        .flat_map(|d| {
            FORBIDDEN_ON_A_DERIVED_MAP
                .iter()
                .filter(move |(phrase, _)| d.message.contains(phrase))
                .map(move |(phrase, why)| {
                    format!("{} at {}: says \"{phrase}\" — but {why}", d.code, d.path)
                })
        })
        .collect();
    assert!(
        bad.is_empty(),
        "{} refusal(s) over {} diagnostic(s) prescribe something this campaign is refused for \
         doing:\n{}",
        bad.len(),
        all.len(),
        bad.join("\n")
    );
}

/// **The same question asked of a campaign with NO map at all** — `areas[]`
/// empty and no site plan.
///
/// This is a real authoring state (a story layer written before its map) and it
/// is the one the old message served worst: *"declare it in stage-1
/// `world.areas`"* names one half of a choice the author has not made, and
/// silently commits them to the half that then refuses their site plan.
///
/// Refusing the campaign is correct — it has no map. What the refusal owes is
/// BOTH branches, so the author can pick.
#[test]
fn a_campaign_with_no_map_is_told_about_both_placement_authorities() {
    let mut raw = campaign(None, None, None);
    let mut world: Value = serde_json::from_str(&raw.world).expect("world parses");
    world["content"]["areas"] = json!([]);
    raw.world = serde_json::to_string(&world).expect("re-serialize");

    let d = check_campaign(&raw);
    let area_refusals: Vec<&delvewright_dsl::Diagnostic> =
        d.iter().filter(|x| x.code == "DW0112").collect();
    assert!(
        !area_refusals.is_empty(),
        "a campaign that declares no area must still be refused: {d:?}"
    );
    for x in &area_refusals {
        assert!(
            x.message.contains("`world.areas`") && x.message.contains("`site-plan.json`"),
            "a refusal here must name BOTH placement authorities, or it decides for the \
             author: {}",
            x.message
        );
    }
}
