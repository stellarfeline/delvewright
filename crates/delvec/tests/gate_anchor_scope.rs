//! **A gate the resolver could not read, and a gate that answered about a
//! different building.**
//!
//! Two defects, measured on a campaign of eight zones before either was repaired.
//!
//! The compiler resolved a gate anchor only from the explicit `region` + `block`
//! fields, while the zones declare their gates as `resolves_to: "bar:<region>"` —
//! the form the spatial contract writes, where the cells and the block already
//! live in the contract's own bar and repeating them on the anchor would be a
//! second authority for one fact. Four such anchors, none carrying
//! `region`/`block`, and nothing anywhere derived one from the other: not one of
//! the campaign's five shortcuts could be wired.
//!
//! And one shortcut passed anyway. Two pieces belonging to neither of that
//! campaign's areas declared an anchor of the same name in the old form, and
//! `DW0343` was satisfied by their word — the registry scanned the whole loaded
//! library rather than the pieces the campaign's areas can place. That is not the
//! unbound vacuity mode: the check examined something and reported truthfully
//! about it. The lookup asked the right question about the wrong object.
//!
//! What each half is proved against here: the first on
//! `PrefabMeta::gate_anchor`, which is the ONE authority both readers ask, and
//! the second on `gates::check_close_gates` over a genuinely multi-area campaign
//! (`keep-crawl`: `area/gatehouse` binds a bare piece, `area/keep` binds a pool),
//! because a same-area duplicate and a cross-area one must come out differently
//! and only a two-area campaign can tell them apart.

mod common;

use delvewright_compiler::gates;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::prefab::PrefabMeta;
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};

// --- the ONE authority: `PrefabMeta::gate_anchor` --------------------------

/// A piece whose `anchor/door` is declared **only** through the spatial
/// contract: a `pos` inside the bar, and a `resolves_to` naming it.
fn contract_piece(bar_boxes: &str, resolves_to: &str) -> PrefabMeta {
    let text = format!(
        r#"{{
  "prefab_id": "prefab/contract-door",
  "structure": {{ "file": "contract-door.nbt", "id": "contract_door",
                  "size": [8, 5, 8], "data_version": 4440 }},
  "anchors": {{
    "anchor/door": {{ "pos": [4, 1, 3], "resolves_to": "{resolves_to}" }}
  }},
  "spatial_contract": {{
    "entry": "room",
    "spaces": {{ "room": {{ "envelope": "enclosed",
                            "boxes": [{{ "from": [1, 1, 1], "to": [6, 3, 6] }}] }} }},
    "edges": [
      {{ "a": "room", "b": "exterior", "class": "barred",
         "bar": {{ "region": "door-leaf", "block": "minecraft:iron_bars",
                   "boxes": {bar_boxes} }} }}
    ]
  }}
}}"#
    );
    PrefabMeta::from_json(&text).expect("fixture metadata parses")
}

/// The whole of defect 1 in one assertion: a gate declared through the contract
/// resolves to the contract's own box and the contract's own block, exactly as an
/// explicit `region`/`block` would. The information was always in the document.
#[test]
fn a_gate_declared_through_the_contract_resolves_to_the_bar() {
    let meta = contract_piece(
        r#"[{ "from": [3, 1, 3], "to": [5, 3, 3] }]"#,
        "bar:door-leaf",
    );
    let gate = meta
        .gate_anchor("anchor/door")
        .expect("a contract bar is a gate the compiler can fill")
        .expect("and it is a gate, not a point");
    assert_eq!(gate.from, [3, 1, 3]);
    assert_eq!(gate.to, [5, 3, 3]);
    assert_eq!(gate.block, "minecraft:iron_bars");
}

/// A bar declared as several boxes that TILE one box is still one box, because
/// the union adds no cell the contract did not call bar. A doorway written as a
/// lintel row plus the jambs under it is the ordinary case and must resolve.
#[test]
fn a_bar_whose_boxes_tile_one_box_resolves_to_that_box() {
    let meta = contract_piece(
        r#"[{ "from": [3, 1, 3], "to": [5, 2, 3] },
             { "from": [3, 3, 3], "to": [5, 3, 3] }]"#,
        "bar:door-leaf",
    );
    let gate = meta.gate_anchor("anchor/door").unwrap().unwrap();
    assert_eq!((gate.from, gate.to), ([3, 1, 3], [5, 3, 3]));
}

/// A bar whose boxes do NOT fill their bounding box is refused, not widened.
/// Widening it would hand the assembler — which voids every resolved gate region
/// — cells the contract never called bar, and a `close-gate` would build iron
/// bars across the threshold under the door.
#[test]
fn a_bar_that_does_not_fill_its_bounding_box_is_refused() {
    let meta = contract_piece(
        r#"[{ "from": [3, 1, 3], "to": [5, 3, 3] },
             { "from": [4, 0, 3], "to": [4, 0, 3] }]"#,
        "bar:door-leaf",
    );
    let why = meta
        .gate_anchor("anchor/door")
        .expect_err("a bar that is not a box has no single region to fill");
    assert!(why.contains("bounding box"), "{why}");
    assert!(why.contains("door-leaf"), "{why}");
}

/// A `resolves_to` naming a bar the contract does not declare is the two halves
/// of one document having come apart, and is refused rather than read as a point.
#[test]
fn a_bar_the_contract_does_not_declare_is_refused() {
    let meta = contract_piece(
        r#"[{ "from": [3, 1, 3], "to": [5, 3, 3] }]"#,
        "bar:no-such-bar",
    );
    let why = meta.gate_anchor("anchor/door").expect_err("no such bar");
    assert!(why.contains("no-such-bar"), "{why}");
}

/// Every other contract element is a place a body stands, looks through or walks
/// over — not a thing content fills — so an anchor resolving into one is not a
/// gate and is not an error either. A `way` is in this list deliberately: its
/// `opens` is a direction the single-region gate model has nowhere to put.
#[test]
fn an_anchor_resolving_into_a_space_or_a_way_is_not_a_gate() {
    for element in [
        "space:room",
        "no_body:ledge",
        "via:doorway",
        "way:broken-flight",
    ] {
        let meta = contract_piece(r#"[{ "from": [3, 1, 3], "to": [5, 3, 3] }]"#, element);
        assert_eq!(
            meta.gate_anchor("anchor/door").expect("not an error"),
            None,
            "`{element}` is not a gate"
        );
    }
}

/// Both forms at once is refused when they disagree, rather than resolved by a
/// precedence rule no reader of the document could see.
#[test]
fn two_forms_that_disagree_are_refused() {
    let text = r#"{
  "prefab_id": "prefab/two-minds",
  "structure": { "file": "x.nbt", "id": "x", "size": [8, 5, 8], "data_version": 4440 },
  "anchors": {
    "anchor/door": {
      "pos": [4, 1, 3], "resolves_to": "bar:door-leaf",
      "region": { "from": [0, 0, 0], "to": [1, 1, 1] }, "block": "minecraft:stone"
    }
  },
  "spatial_contract": {
    "entry": "room",
    "spaces": { "room": { "envelope": "enclosed",
                          "boxes": [{ "from": [1, 1, 1], "to": [6, 3, 6] }] } },
    "edges": [
      { "a": "room", "b": "exterior", "class": "barred",
        "bar": { "region": "door-leaf", "block": "minecraft:iron_bars",
                 "boxes": [{ "from": [3, 1, 3], "to": [5, 3, 3] }] } }
    ]
  }
}"#;
    let meta = PrefabMeta::from_json(text).unwrap();
    let why = meta
        .gate_anchor("anchor/door")
        .expect_err("the two disagree");
    assert!(why.contains("disagree"), "{why}");
    assert!(
        why.contains("minecraft:stone") && why.contains("minecraft:iron_bars"),
        "{why}"
    );
}

/// The same two forms AGREEING is what a piece that was hand-authored and later
/// exported looks like, and is not an error.
#[test]
fn two_forms_that_agree_resolve() {
    let text = r#"{
  "prefab_id": "prefab/one-mind",
  "structure": { "file": "x.nbt", "id": "x", "size": [8, 5, 8], "data_version": 4440 },
  "anchors": {
    "anchor/door": {
      "pos": [4, 1, 3], "resolves_to": "bar:door-leaf",
      "region": { "from": [3, 1, 3], "to": [5, 3, 3] }, "block": "minecraft:iron_bars"
    }
  },
  "spatial_contract": {
    "entry": "room",
    "spaces": { "room": { "envelope": "enclosed",
                          "boxes": [{ "from": [1, 1, 1], "to": [6, 3, 6] }] } },
    "edges": [
      { "a": "room", "b": "exterior", "class": "barred",
        "bar": { "region": "door-leaf", "block": "minecraft:iron_bars",
                 "boxes": [{ "from": [3, 1, 3], "to": [5, 3, 3] }] } }
    ]
  }
}"#;
    let meta = PrefabMeta::from_json(text).unwrap();
    let gate = meta.gate_anchor("anchor/door").unwrap().unwrap();
    assert_eq!((gate.from, gate.to), ([3, 1, 3], [5, 3, 3]));
}

// --- the scope of uniqueness: `DW0857` -------------------------------------

/// `keep-crawl` (two areas — a bare piece and a pool) with a `close-gate` on
/// `anchor` appended to its first quest's `on_complete`.
fn keep_crawl_closing(anchor: &str) -> Campaign {
    let dir = common::keep_crawl_dir();
    let read = |n: &str| std::fs::read_to_string(dir.join(n)).unwrap();
    let mut quests: serde_json::Value = serde_json::from_str(&read("quests.json")).unwrap();
    quests["content"]["quests"][0]["on_complete"] = serde_json::json!([
        { "type": "close-gate", "anchor": anchor }
    ]);
    let raw = RawCampaign {
        world: read("world.json"),
        npcs: read("npcs.json"),
        classes: read("classes.json"),
        quest_plan: read("quest-plan.json"),
        quests: quests.to_string(),
        dialogue: read("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
    };
    parse_campaign(&raw).expect("campaign parses")
}

/// Copy the prefab library somewhere writable and hand it to `f` as a mutable
/// tree of `prefab id -> metadata JSON`.
fn with_prefabs<T>(
    tag: &str,
    f: impl FnOnce(&std::path::Path),
    then: impl FnOnce(&PrefabRegistry) -> T,
) -> T {
    let tmp = std::env::temp_dir().join(format!("dw-gate-scope-{tag}"));
    let _ = std::fs::remove_dir_all(&tmp);
    common::copy_dir_all(&common::prefabs_dir(), &tmp);
    f(&tmp);
    let reg = PrefabRegistry::load_dir(&tmp).unwrap();
    let out = then(&reg);
    let _ = std::fs::remove_dir_all(&tmp);
    out
}

/// Give `file`'s `anchor/door` the same gate region + block `hello-room` has.
fn add_door_gate(dir: &std::path::Path, file: &str) {
    let p = dir.join(file);
    let mut meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
    meta["anchors"]["anchor/door"] = serde_json::json!({
        "region": { "from": [1, 1, 1], "to": [2, 3, 1] },
        "block": "minecraft:iron_bars"
    });
    std::fs::write(&p, serde_json::to_string_pretty(&meta).unwrap()).unwrap();
}

/// **Defect 2.** `area/gatehouse` binds `prefab/hello-room`, which declares
/// `anchor/door` as a gate; a member of `area/keep`'s pool is given the same
/// name. Two placed areas now provide it, the compiler's by-name lookup returns
/// whichever area id sorts first, and nothing an author can see says which.
/// Refused.
#[test]
fn a_gate_two_areas_provide_is_dw0857() {
    let c = keep_crawl_closing("anchor/door");
    let d = with_prefabs(
        "cross-area",
        |dir| add_door_gate(dir, "keep-room-small-a.json"),
        |reg| gates::check_close_gates(&c, reg),
    );
    let hit = d
        .iter()
        .find(|x| x.code == gates::DW_GATE_ANCHOR_AMBIGUOUS)
        .unwrap_or_else(|| panic!("a gate two areas provide must be DW0857: {d:#?}"));
    assert!(hit.message.contains("area/gatehouse"), "{}", hit.message);
    assert!(hit.message.contains("area/keep"), "{}", hit.message);
    assert!(hit.message.contains("anchor/door"), "{}", hit.message);
    assert_eq!(hit.code, "DW0857");

    // **The remedy is one a campaign author can perform, and the message says
    // which is which.** This diagnostic used to end "Rename the gate in one of
    // these areas" — the anchor name is a key in the piece's exported metadata,
    // shared by every campaign that binds that piece, so a campaign author was
    // being sent to edit a library the campaign does not contain.
    assert!(
        !hit.message
            .contains("Rename the gate in one of these areas"),
        "the old prescription told the author to edit a library they do not own: {}",
        hit.message
    );
    // The campaign-side move, named where it lives.
    assert!(hit.message.contains("`world.areas[]`"), "{}", hit.message);
    // And which piece each area is bound THROUGH, without which an author
    // cannot choose which of the two areas to change.
    assert!(
        hit.message.contains("prefab/hello-room")
            && hit.message.contains("prefab/keep-room-small-a"),
        "the message must name the piece each area provides the anchor through: {}",
        hit.message
    );
    // Where the change genuinely is in the piece, that is said rather than
    // prescribed: silence dressed as advice is not a remedy.
    assert!(
        hit.message.contains("you cannot reach it from here"),
        "{}",
        hit.message
    );
}

/// The same campaign untouched: one area provides `anchor/door`, so the anchor
/// resolves exactly as it always did and nothing is refused. What the diagnostic
/// refuses is the AMBIGUITY, not the crossing — a campaign that was unambiguous
/// must not move.
#[test]
fn a_gate_one_area_provides_is_clean() {
    let c = keep_crawl_closing("anchor/door");
    let d = with_prefabs("one-area", |_| {}, |reg| gates::check_close_gates(&c, reg));
    assert!(
        d.is_empty(),
        "an unambiguous gate anchor must stay clean: {d:#?}"
    );
}

/// Two pieces of ONE area sharing a gate-anchor name is not this finding. That is
/// what a `prefab_pool` is for — its members share anchor names so that whichever
/// member the solver seats provides the anchor — and it has always resolved
/// within the area.
#[test]
fn two_pool_members_of_one_area_are_not_ambiguous() {
    let c = keep_crawl_closing("anchor/gate");
    let d = with_prefabs(
        "same-area",
        |dir| {
            add_door_gate(dir, "keep-room-small-a.json");
            add_door_gate(dir, "keep-room-small-b.json");
            for f in ["keep-room-small-a.json", "keep-room-small-b.json"] {
                let p = dir.join(f);
                let mut m: serde_json::Value =
                    serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
                let gate = m["anchors"]["anchor/door"].take();
                m["anchors"].as_object_mut().unwrap().remove("anchor/door");
                m["anchors"]["anchor/gate"] = gate;
                std::fs::write(&p, serde_json::to_string_pretty(&m).unwrap()).unwrap();
            }
        },
        |reg| gates::check_close_gates(&c, reg),
    );
    assert!(
        !d.iter().any(|x| x.code == gates::DW_GATE_ANCHOR_AMBIGUOUS),
        "two members of ONE area sharing a name is a pool working as designed: {d:#?}"
    );
}

/// **The denominator, in the shape the campaign actually hit.** The DSL tier
/// already requires a shortcut's gate anchor to be a NAME some area of the
/// campaign provides (`DW0350`, over the union of the campaign's areas). What
/// nothing asked was whether the piece that makes it a GATE is one this campaign
/// can place: the registry's gate question scanned the whole loaded library.
///
/// So here `anchor/boulder` is a plain point anchor on the bound
/// `prefab/hello-room` — the name resolves, the DSL tier is satisfied — and the
/// only piece declaring it as a fillable gate is `prefab/island-mountain`, which
/// no area of `keep-crawl` binds. `DW0343` passed on that piece's word, and a
/// `close-gate` compiled that could never be filled. Asked of the campaign's own
/// pieces, it refuses.
#[test]
fn a_gate_only_an_unbound_piece_declares_does_not_satisfy_dw0343() {
    let c = keep_crawl_closing("anchor/boulder");
    let d = with_prefabs(
        "denominator",
        |dir| {
            // The bound piece provides the NAME, as a point. This is what makes
            // the test about the denominator and not about a dangling reference.
            let p = dir.join("hello-room.json");
            let mut m: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
            m["anchors"]["anchor/boulder"] =
                serde_json::json!({ "pos": [5, 1, 5], "facing": "north" });
            std::fs::write(&p, serde_json::to_string_pretty(&m).unwrap()).unwrap();
        },
        |reg| {
            // The library really does hold the unbound gate provider this test is
            // about; a fixture that quietly stopped holding it would make the
            // assertion below pass for the wrong reason — which is the vacuity
            // mode this whole file exists about.
            assert!(
                matches!(
                    reg.gate_anchor_in("prefab/island-mountain", "anchor/boulder"),
                    Ok(Some(_))
                ),
                "`prefab/island-mountain` must still declare `anchor/boulder` as a gate"
            );
            assert!(
                matches!(
                    reg.gate_anchor_in("prefab/hello-room", "anchor/boulder"),
                    Ok(None)
                ),
                "the bound piece must provide the name WITHOUT making it a gate"
            );
            gates::check_close_gates(&c, reg)
        },
    );
    assert!(
        d.iter().any(|x| x.code == gates::DW_GATE_NO_BLOCK),
        "a gate only an UNBOUND piece declares must not satisfy DW0343: {d:#?}"
    );
}
