//! DSL v0.10 trade and the recovery stake (spec-0032), validation tier: what a
//! campaign may declare and what it may not.
//!
//! The geometric half — where a stake lands, whether the party can walk back to it,
//! whether the ground under it is ground the runtime rewrites — is deliberately not
//! here. Those are questions about the solved layout, which this crate does not
//! have; they live in `crates/delvec/src/compiler/stake.rs` and are exercised by
//! `crates/delvec/tests/v10_economy.rs`, exactly as a lethal volume's geometry is.

use delvewright_dsl::{Campaign, DSL_VERSION, RawCampaign, parse_campaign, validate_campaign};

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
fn campaign(extra: &str) -> Campaign {
    let quests = format!(
        r#"{{
  "dsl_version": "{DSL_VERSION}",
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
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
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
      { "id": "shop/brazier", "anchor": "spawn", "title": "The brazier",
        "offers": [
          { "label": "Bank an ember",
            "effects": [
              { "type": "narrate",
                "when": { "requires_state": [ { "state": "state/embers", "op": "at-most", "value": 0 } ] }, "text": "You have nothing left to give." },
              { "type": "add-state",
                "when": { "requires_state": [ { "state": "state/embers", "op": "at-least", "value": 1 } ] }, "state": "state/embers", "amount": -1 }
            ] }
        ] }
    ]"#;

#[test]
fn a_well_formed_economy_validates_clean() {
    assert!(
        codes(&campaign(GOOD)).is_empty(),
        "{:#?}",
        validate_campaign(&campaign(GOOD))
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
        codes(&campaign(&party)).contains(&"DW0520".to_string()),
        "a party-scoped purse is not a personal wager"
    );

    let missing = GOOD.replace(r#""state": "state/embers""#, r#""state": "state/ash""#);
    assert_ne!(missing, GOOD);
    assert!(
        codes(&campaign(&missing)).contains(&"DW0520".to_string()),
        "a stake whose datum the campaign never declares"
    );
}

/// `DW0521` — a `drop-stake` naming a stake the campaign never declares.
#[test]
fn dw0521_drop_stake_must_name_a_declared_stake() {
    let bad = GOOD.replace(r#""stake": "stake/embers" }"#, r#""stake": "stake/ash" }"#);
    assert_ne!(bad, GOOD);
    let got = codes(&campaign(&bad));
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
    let got = codes(&campaign(&orphan));
    assert!(got.contains(&"DW0522".to_string()), "{got:?}");
}

/// `DW0523` — a button the player can press that cannot answer. A refusal counts as
/// an answer, which is exactly the shape spec-0032 asks a shop to use for "you
/// cannot afford that".
#[test]
fn dw0523_an_offer_that_cannot_answer_is_a_finding() {
    let inert = GOOD.replace(
        r#""effects": [
              { "type": "narrate",
                "when": { "requires_state": [ { "state": "state/embers", "op": "at-most", "value": 0 } ] }, "text": "You have nothing left to give." },
              { "type": "add-state",
                "when": { "requires_state": [ { "state": "state/embers", "op": "at-least", "value": 1 } ] }, "state": "state/embers", "amount": -1 }
            ]"#,
        r#""effects": []"#,
    );
    assert_ne!(inert, GOOD);
    let got = codes(&campaign(&inert));
    assert!(got.contains(&"DW0523".to_string()), "{got:?}");

    // …and a shop with no offers at all is the same code, because vanilla's
    // 1.21.11 dialog codec rejects an empty action list at pack load: this is not
    // an empty shop, it is a dialog that fails to load.
    let no_offers = r#",
    "state": [ { "id": "state/embers", "scope": "player", "name": "Embers" } ],
    "shops": [ { "id": "shop/brazier", "anchor": "anchor/keeper-stand",
                 "title": "The brazier", "offers": [] } ]"#;
    assert!(
        codes(&campaign(no_offers)).contains(&"DW0523".to_string()),
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
    let got = codes(&campaign(&over));
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
        r#"{ "type": "narrate",
                "when": { "requires_state": [ { "state": "state/embers", "op": "at-most", "value": 0 } ] }, "text": "You have nothing left to give." },
              { "type": "add-state",
                "when": { "requires_state": [ { "state": "state/embers", "op": "at-least", "value": 1 } ] }, "state": "state/embers", "amount": -1 }"#,
        r#"{ "type": "add-state",
                "when": { "requires_state": [ { "state": "state/embers", "op": "at-least", "value": 1 } ] }, "state": "state/embers", "amount": -1 },
              { "type": "narrate",
                "when": { "requires_state": [ { "state": "state/embers", "op": "at-most", "value": 0 } ] }, "text": "You have nothing left to give." }"#,
    );
    assert_ne!(hazard, GOOD, "the two orderings really are different text");
    assert!(
        codes(&campaign(&hazard)).contains(&"DW0527".to_string()),
        "purchase-then-apology is the hazard"
    );
    assert!(
        !codes(&campaign(GOOD)).contains(&"DW0527".to_string()),
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
          { "type": "open-gate",
            "when": { "requires_state": [ { "state": "state/toll", "op": "at-most", "value": 0 } ] }, "anchor": "anchor/door" }
        ] }
    ]"#;
    assert!(
        !codes(&campaign(sequenced)).contains(&"DW0527".to_string()),
        "an unconditional write is the sequenced idiom, not the hazard"
    );
}

/// A shop offer is the **seventh gate consumer**, and the shared gate is the only
/// place a price can be written. Read off the closed consumer set rather than a
/// list, so an eighth consumer added tomorrow must answer the same question.
#[test]
fn a_shop_offer_is_a_gate_consumer() {
    use delvewright_dsl::gate::{GateConsumer, for_each_gate};
    let c = campaign(GOOD);
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
    let mut c = campaign(GOOD);
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

// ---------------------------------------------------------------------------
// DW0901 — a purchase whose literals do not add up (spec-0071 §2)
// ---------------------------------------------------------------------------
//
// `GOOD` above is the reference purchase and the false-positive guard: the
// apology is gated `at-most 0`, the charge `at-least 1`, and it charges 1. Every
// red below is that shape with one literal moved.

/// One shop offer, with `apology` and `charge` gate terms and a charge `amount`,
/// on a purse whose datum is declared. The literals are the only difference
/// between these fixtures, which is what makes each refusal unambiguous.
fn offer(apology: &str, charge_gate: &str, amount: i32) -> String {
    format!(
        r#",
    "state": [
      {{ "id": "state/embers", "scope": "player", "initial": 5, "name": "Embers" }}
    ],
    "shops": [
      {{ "id": "shop/brazier", "anchor": "spawn", "title": "The brazier",
        "offers": [
          {{ "label": "A case key",
            "effects": [
              {{ "type": "narrate"{apology}, "text": "You cannot afford that." }},
              {{ "type": "add-state"{charge_gate}, "state": "state/embers", "amount": {amount} }}
            ] }}
        ] }}
    ]"#
    )
}

/// `when` carrying one numeric term on the purse.
fn gate(op: &str, value: i32) -> String {
    format!(
        r#", "when": {{ "requires_state": [ {{ "state": "state/embers", "op": "{op}", "value": {value} }} ] }}"#
    )
}

fn dw0901(c: &Campaign) -> delvewright_dsl::Diagnostic {
    validate_campaign(c)
        .into_iter()
        .find(|d| d.code == "DW0901")
        .unwrap_or_else(|| panic!("DW0901 expected; got {:#?}", validate_campaign(c)))
}

/// The motivating shape: the offer gates on 15 and charges 16, so a player at 15
/// buys and is left at −1. Both literals and the datum are in the message.
#[test]
fn dw0901_a_charge_deeper_than_its_own_gate() {
    let c = campaign(&offer(&gate("at-most", 14), &gate("at-least", 15), -16));
    let d = dw0901(&c);
    assert!(d.message.contains("charges 16"), "{}", d.message);
    assert!(d.message.contains("opens at 15"), "{}", d.message);
    assert!(d.message.contains("state/embers"), "{}", d.message);
    assert!(
        d.message.contains("leaves `state/embers` at -1"),
        "{}",
        d.message
    );
}

/// The sale starts at 15 and the apology stops at 13, so a player holding exactly
/// 14 presses the button and nothing happens at all.
#[test]
fn dw0901_a_gap_between_the_sale_and_the_apology() {
    let c = campaign(&offer(&gate("at-most", 13), &gate("at-least", 15), -15));
    let d = dw0901(&c);
    assert!(d.message.contains("leaves a gap"), "{}", d.message);
    assert!(d.message.contains("14..14"), "{}", d.message);
    assert!(d.message.contains("at-most 14"), "{}", d.message);
}

/// The apology runs to 15 and the sale starts at 15, so a player holding 15 is
/// charged **and** told they cannot afford it.
#[test]
fn dw0901_an_overlap_between_the_sale_and_the_apology() {
    let c = campaign(&offer(&gate("at-most", 15), &gate("at-least", 15), -15));
    let d = dw0901(&c);
    assert!(d.message.contains("overlaps the sale"), "{}", d.message);
    assert!(d.message.contains("15..15"), "{}", d.message);
}

/// The price is on the **offer's own gate** — the correct spelling spec-0032
/// prescribes — and the charge is bare. The enclosing gate is read together with
/// the effect's own, so 15/15 is clean and 15/16 is refused.
#[test]
fn dw0901_reads_the_offer_s_own_gate() {
    let priced = |amount: i32| {
        format!(
            r#",
    "state": [
      {{ "id": "state/embers", "scope": "player", "initial": 5, "name": "Embers" }}
    ],
    "shops": [
      {{ "id": "shop/brazier", "anchor": "spawn", "title": "The brazier",
        "offers": [
          {{ "label": "A case key",
            "requires_state": [ {{ "state": "state/embers", "op": "at-least", "value": 15 }} ],
            "effects": [
              {{ "type": "add-state", "state": "state/embers", "amount": {amount} }}
            ] }}
        ] }}
    ]"#
        )
    };
    assert!(
        !codes(&campaign(&priced(-15))).contains(&"DW0901".to_string()),
        "a button shown only at 15 that charges 15 adds up: {:#?}",
        validate_campaign(&campaign(&priced(-15)))
    );
    let d = dw0901(&campaign(&priced(-16)));
    assert!(d.message.contains("charges 16"), "{}", d.message);
    assert!(
        d.message.contains("/content/shops/0/offers/0")
            || d.path.contains("/content/shops/0/offers/0"),
        "the refusal points at the offer: {} {}",
        d.path,
        d.message
    );
}

/// **The rule is not about shops.** The same shape inside an environment
/// trigger's effects is the same refusal — the quantifier is every effect list
/// whose effects charge a datum.
#[test]
fn dw0901_binds_outside_a_shop() {
    let toll = r#",
    "state": [
      { "id": "state/embers", "scope": "party", "initial": 5 }
    ],
    "triggers": [
      { "id": "trigger/toll", "at": "anchor/door", "on": { "on": "approach", "range": 2 },
        "requires_state": [ { "state": "state/embers", "op": "at-least", "value": 3 } ],
        "effects": [
          { "type": "add-state", "state": "state/embers", "amount": -4 }
        ] }
    ]"#;
    let d = dw0901(&campaign(toll));
    assert_eq!(d.stage, "quests");
    assert!(d.message.contains("charges 4"), "{}", d.message);
    assert!(d.message.contains("opens at 3"), "{}", d.message);
}

/// A debit in a list that says nothing else about the datum is a design, not a
/// purchase: no second copy of the number exists to disagree with, so the rule
/// stays silent. The narrowest place the rule declines to speak, stated as a
/// test so it cannot widen by accident.
#[test]
fn dw0901_says_nothing_about_a_datum_the_list_does_not_price() {
    let drain = r#",
    "state": [
      { "id": "state/embers", "scope": "party", "initial": 5 }
    ],
    "triggers": [
      { "id": "trigger/toll", "at": "anchor/door", "on": { "on": "approach", "range": 2 },
        "effects": [
          { "type": "add-state", "state": "state/embers", "amount": -4 }
        ] }
    ]"#;
    assert!(
        !codes(&campaign(drain)).contains(&"DW0901".to_string()),
        "{:#?}",
        validate_campaign(&campaign(drain))
    );
}

/// A charge with no gate of its own, beside an arm that prices the same datum:
/// the copy is missing rather than wrong, and it is the same defect.
#[test]
fn dw0901_an_ungated_charge_beside_a_priced_arm() {
    let c = campaign(&offer(&gate("at-most", 14), "", -15));
    let d = dw0901(&c);
    assert!(d.message.contains("nothing gates it"), "{}", d.message);
}

// (the binding test needs the new type and cannot exist at this revision)

/// A campaign-wide effect walk names **which** offer it is standing in. The
/// index is parsed back out of the root's path, and a shop with two offers is
/// what tells a right answer from a constant.
#[test]
fn an_effect_site_names_the_offer_it_stands_in() {
    use delvewright_dsl::{EffectSite, for_each_campaign_effect};
    let two = r#",
    "state": [
      { "id": "state/embers", "scope": "party", "initial": 5 }
    ],
    "shops": [
      { "id": "shop/brazier", "anchor": "spawn", "title": "The brazier",
        "offers": [
          { "label": "First", "effects": [ { "type": "narrate", "text": "One." } ] },
          { "label": "Second", "effects": [ { "type": "narrate", "text": "Two." } ] }
        ] }
    ]"#;
    let c = campaign(two);
    let mut seen: Vec<usize> = Vec::new();
    for_each_campaign_effect(&c, &mut |_p, site, _e| {
        if let EffectSite::ShopOffer { offer, .. } = site {
            seen.push(*offer);
        }
    });
    assert_eq!(
        seen,
        vec![0, 1],
        "each offer's effects know their own index"
    );
}
