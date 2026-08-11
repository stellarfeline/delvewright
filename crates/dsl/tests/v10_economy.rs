//! DSL v0.10 trade and the recovery stake (spec-0032), validation tier: what a
//! campaign may declare, what it may not, and what the version fence reserves.
//!
//! The geometric half — where a stake lands, whether the party can walk back to it,
//! whether the ground under it is ground the runtime rewrites — is deliberately not
//! here. Those are questions about the solved layout, which this crate does not
//! have; they live in `crates/compiler/src/stake.rs` and are exercised by
//! `crates/compiler/tests/v10_economy.rs`, exactly as a lethal volume's geometry is.

use delvewright_dsl::{Campaign, RawCampaign, parse_campaign, validate_campaign};

fn hw(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../campaigns/campaigns/hello-world")
            .join(name),
    )
    .or_else(|_| {
        std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("fixtures/valid/hello-world")
                .join(name),
        )
    })
    .unwrap_or_else(|e| panic!("hello-world/{name}: {e}"))
}

/// A hello-world campaign whose quests stage carries `extra` inside `content`.
fn campaign(version: &str, extra: &str) -> Campaign {
    let quests = format!(
        r#"{{
  "dsl_version": "{version}",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {{
    "quests": [
      {{
        "id": "quest/open-the-door",
        "trigger": {{ "type": "campaign-start" }},
        "objectives": [
          {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }},
          {{ "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit",
             "radius": 2, "after": ["obj/talk"] }}
        ],
        "on_objective_complete": {{
          "obj/talk": [ {{ "type": "open-gate", "anchor": "anchor/door" }} ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ]{extra}
  }}
}}"#
    );
    let raw = RawCampaign {
        world: hw("world.json"),
        npcs: hw("npcs.json"),
        classes: hw("classes.json"),
        quest_plan: hw("quest-plan.json"),
        quests,
        dialogue: hw("dialogue.json"),
        world_edits: None,
    };
    parse_campaign(&raw).expect("campaign parses")
}

fn codes(c: &Campaign) -> Vec<String> {
    validate_campaign(c)
        .into_iter()
        .map(|d| d.code.to_string())
        .collect()
}

/// A well-formed economy: a named player-scoped purse, a stake that forfeits it, a
/// beat that leaves one, and a shop whose price is the shared gate.
const GOOD: &str = r#",
    "state": [
      { "id": "state/embers", "scope": "player", "initial": 5, "name": "Embers" }
    ],
    "stakes": [
      { "id": "stake/embers", "state": "state/embers",
        "collected_message": "You take back what the drop took." }
    ],
    "on_death": [ { "type": "drop-stake", "stake": "stake/embers" } ],
    "shops": [
      { "id": "shop/brazier", "anchor": "anchor/keeper-stand", "title": "The brazier",
        "offers": [
          { "label": "Bank an ember",
            "effects": [
              { "type": "narrate", "text": "You have nothing left to give.",
                "requires_state": [ { "state": "state/embers", "op": "at-most", "value": 0 } ] },
              { "type": "add-state", "state": "state/embers", "amount": -1,
                "requires_state": [ { "state": "state/embers", "op": "at-least", "value": 1 } ] }
            ] }
        ] }
    ]"#;

#[test]
fn a_well_formed_economy_validates_clean() {
    assert!(
        codes(&campaign("0.10.0", GOOD)).is_empty(),
        "{:#?}",
        validate_campaign(&campaign("0.10.0", GOOD))
    );
}

/// `DW0520` — a stake's datum must exist and must be **per-player**. The scope half
/// is the multiplayer decision spec-0032 records "for correction rather than left to
/// emerge": one shared purse turns a teammate's death into everyone's penalty, and
/// nothing in the JSON would say so.
#[test]
fn dw0520_a_stake_needs_a_player_scoped_datum() {
    let party = GOOD.replace(r#""scope": "player""#, r#""scope": "party""#);
    assert_ne!(party, GOOD);
    assert!(
        codes(&campaign("0.10.0", &party)).contains(&"DW0520".to_string()),
        "a party-scoped purse is not a personal wager"
    );

    let missing = GOOD.replace(r#""state": "state/embers""#, r#""state": "state/ash""#);
    assert_ne!(missing, GOOD);
    assert!(
        codes(&campaign("0.10.0", &missing)).contains(&"DW0520".to_string()),
        "a stake whose datum the campaign never declares"
    );
}

/// `DW0521` — a `drop-stake` naming a stake the campaign never declares.
#[test]
fn dw0521_drop_stake_must_name_a_declared_stake() {
    let bad = GOOD.replace(r#""stake": "stake/embers" }"#, r#""stake": "stake/ash" }"#);
    assert_ne!(bad, GOOD);
    let got = codes(&campaign("0.10.0", &bad));
    assert!(got.contains(&"DW0521".to_string()), "{got:?}");
}

/// `DW0522` — a declared stake no beat ever leaves. The vacuity rule `DW0502`
/// states for a datum with no reader, applied to a whole mechanism: the forfeit
/// rule, the retention policy and the entire compile-time placement table would
/// describe something no beat can fire.
#[test]
fn dw0522_a_stake_nothing_drops_is_a_finding() {
    let orphan = GOOD.replace(
        r#""on_death": [ { "type": "drop-stake", "stake": "stake/embers" } ],"#,
        "",
    );
    assert_ne!(orphan, GOOD);
    let got = codes(&campaign("0.10.0", &orphan));
    assert!(got.contains(&"DW0522".to_string()), "{got:?}");
}

/// `DW0523` — a button the player can press that cannot answer. A refusal counts as
/// an answer, which is exactly the shape spec-0032 asks a shop to use for "you
/// cannot afford that".
#[test]
fn dw0523_an_offer_that_cannot_answer_is_a_finding() {
    let inert = GOOD.replace(
        r#""effects": [
              { "type": "narrate", "text": "You have nothing left to give.",
                "requires_state": [ { "state": "state/embers", "op": "at-most", "value": 0 } ] },
              { "type": "add-state", "state": "state/embers", "amount": -1,
                "requires_state": [ { "state": "state/embers", "op": "at-least", "value": 1 } ] }
            ]"#,
        r#""effects": []"#,
    );
    assert_ne!(inert, GOOD);
    let got = codes(&campaign("0.10.0", &inert));
    assert!(got.contains(&"DW0523".to_string()), "{got:?}");

    // …and a shop with no offers at all is the same code, because vanilla's
    // 1.21.11 dialog codec rejects an empty action list at pack load: this is not
    // an empty shop, it is a dialog that fails to load.
    let no_offers = r#",
    "state": [ { "id": "state/embers", "scope": "player", "name": "Embers" } ],
    "shops": [ { "id": "shop/brazier", "anchor": "anchor/keeper-stand",
                 "title": "The brazier", "offers": [] } ]"#;
    assert!(
        codes(&campaign("0.10.0", no_offers)).contains(&"DW0523".to_string()),
        "a shop with no offers is a dialog that cannot load"
    );
}

/// `DW0524` — a proportional forfeit above 100%: a death that takes more than the
/// whole purse.
#[test]
fn dw0524_a_proportion_above_one_hundred_is_a_finding() {
    let over = GOOD.replace(
        r#""collected_message""#,
        r#""forfeit": { "kind": "proportion", "percent": 150 }, "collected_message""#,
    );
    assert_ne!(over, GOOD);
    let got = codes(&campaign("0.10.0", &over));
    assert!(got.contains(&"DW0524".to_string()), "{got:?}");
}

/// `DW0527` — the ordering hazard this feature's own first shop walked into, and
/// the reason it is a diagnostic rather than a note.
///
/// Written "purchase, then apology", buying your LAST ember prints both: the debit
/// runs, the balance falls to the boundary, and the apology's gate — evaluated
/// after it — now holds. Written "apology, then purchase", every read happens
/// before the write and the emission is correct. The rule fires on the first and
/// not the second.
#[test]
fn dw0527_a_gate_read_after_a_conditional_write_is_a_finding() {
    let hazard = GOOD.replace(
        r#"{ "type": "narrate", "text": "You have nothing left to give.",
                "requires_state": [ { "state": "state/embers", "op": "at-most", "value": 0 } ] },
              { "type": "add-state", "state": "state/embers", "amount": -1,
                "requires_state": [ { "state": "state/embers", "op": "at-least", "value": 1 } ] }"#,
        r#"{ "type": "add-state", "state": "state/embers", "amount": -1,
                "requires_state": [ { "state": "state/embers", "op": "at-least", "value": 1 } ] },
              { "type": "narrate", "text": "You have nothing left to give.",
                "requires_state": [ { "state": "state/embers", "op": "at-most", "value": 0 } ] }"#,
    );
    assert_ne!(hazard, GOOD, "the two orderings really are different text");
    assert!(
        codes(&campaign("0.10.0", &hazard)).contains(&"DW0527".to_string()),
        "purchase-then-apology is the hazard"
    );
    assert!(
        !codes(&campaign("0.10.0", GOOD)).contains(&"DW0527".to_string()),
        "apology-then-purchase is correct and must not be diagnosed"
    );

    // …and the ordinary sequenced idiom — an UNCONDITIONAL write followed by a
    // comparison — is deliberately not diagnosed: `pay the toll, then the door
    // opens because the toll is now zero` plainly means the post-write value.
    let sequenced = r#",
    "state": [ { "id": "state/toll", "scope": "party", "initial": 3 } ],
    "triggers": [
      { "id": "trigger/pay", "at": "anchor/door", "on": { "on": "use" },
        "effects": [
          { "type": "set-state", "state": "state/toll", "value": 0 },
          { "type": "open-gate", "anchor": "anchor/door",
            "requires_state": [ { "state": "state/toll", "op": "at-most", "value": 0 } ] }
        ] }
    ]"#;
    assert!(
        !codes(&campaign("0.10.0", sequenced)).contains(&"DW0527".to_string()),
        "an unconditional write is the sequenced idiom, not the hazard"
    );
}

/// Every spec-0032 surface is **reserved** below 0.10.0 (`DW0141`), so a campaign
/// on any older stage envelope compiles exactly as it did.
#[test]
fn the_whole_surface_is_reserved_below_v10() {
    for (label, extra) in [
        (
            "shops",
            r#",
    "shops": [ { "id": "shop/x", "anchor": "anchor/exit", "title": "T",
                 "offers": [ { "label": "L", "effects": [ { "type": "set-flag", "flag": "flag/f" } ] } ] } ]"#,
        ),
        (
            "stakes",
            r#",
    "state": [ { "id": "state/e", "scope": "player" } ],
    "stakes": [ { "id": "stake/e", "state": "state/e", "collected_message": "back" } ]"#,
        ),
        (
            "a datum's name",
            r#",
    "state": [ { "id": "state/e", "scope": "player", "name": "Embers" } ]"#,
        ),
        (
            "drop-stake",
            r#",
    "stakes": [ { "id": "stake/e", "state": "state/e", "collected_message": "back" } ],
    "on_death": [ { "type": "drop-stake", "stake": "stake/e" } ]"#,
        ),
    ] {
        let got = codes(&campaign("0.9.0", extra));
        assert!(
            got.contains(&"DW0141".to_string()),
            "{label} must be reserved below 0.10.0, got {got:?}"
        );
    }
}

/// A shop offer is the **seventh gate consumer**, and the shared gate is the only
/// place a price can be written. Read off the closed consumer set rather than a
/// list, so an eighth consumer added tomorrow must answer the same question.
#[test]
fn a_shop_offer_is_a_gate_consumer() {
    use delvewright_dsl::gate::{GateConsumer, for_each_gate};
    let c = campaign("0.10.0", GOOD);
    let mut priced = 0usize;
    let binding = for_each_gate(&c, &mut |site, gate| {
        if site.consumer == GateConsumer::ShopOffer && !gate.requires_state.is_empty() {
            priced += 1;
        }
    });
    assert_eq!(binding.consumers_enumerated, GateConsumer::COUNT);
    let offers = binding
        .sites
        .iter()
        .find(|(k, _)| *k == GateConsumer::ShopOffer)
        .unwrap()
        .1;
    assert_eq!(offers, 1, "the walk reached the shop's one offer");
    assert!(
        GateConsumer::ShopOffer.evaluates_per_player() == Some(true),
        "an offer's gate is evaluated against the buying player — which is what \
         makes a `player`-scoped purse a legal price"
    );
}

/// Every player-visible string spec-0032 adds enters the l10n inventory under a
/// stable key. Binding: the key count is asserted, so an inventory that stopped
/// visiting the shop would not pass by finding nothing.
#[test]
fn every_new_player_visible_string_is_inventoried() {
    let mut c = campaign("0.10.0", GOOD);
    let mut keys: Vec<String> = Vec::new();
    delvewright_dsl::l10n::each_string(&mut c, &mut |k, _| keys.push(k.to_string()));
    for want in [
        "state.embers.name",
        "shop.brazier.title",
        "shop.brazier.offer.0.label",
        "stake.embers.collected",
    ] {
        assert!(
            keys.iter().any(|k| k == want),
            "`{want}` is inventoried; have {keys:#?}"
        );
    }
}
