//! DSL v0.10 trade and the recovery stake (spec-0032): what the compiler emits,
//! what the placement table answers, and what the completability proof refuses.
//!
//! `hello-room` is the fixture geometry throughout, for the reason the lethal-volume
//! suite gives: it is a corridor with exactly one doorway, so "the only way back is
//! through here" is a two-line declaration rather than a synthetic world.
//!
//! **Every assertion here that quantifies over something states its binding count**
//! (spec-0032 AC10). A test that walked zero rows of the placement table, or zero
//! offers of a shop, would pass while proving nothing — which is the vacuity rule
//! CLAUDE.md names, and the reason `stake-gate.json` exists at all.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};

fn hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

/// A hello-world `quests` doc carrying the given stage-5 sections.
///
/// `extra` is spliced into `content`, `talk_effects` after the `open-gate` on
/// `obj/talk` — the two seams every case below needs.
fn quests_doc(extra: &str, talk_effects: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "0.10.0",
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
          "obj/talk": [ {{ "type": "open-gate", "anchor": "anchor/door" }}{talk_effects} ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ]{extra}
  }}
}}"#
    )
}

fn parse_hw(quests: &str) -> Campaign {
    let raw = RawCampaign {
        world: hw("world.json"),
        npcs: hw("npcs.json"),
        classes: hw("classes.json"),
        quest_plan: hw("quest-plan.json"),
        quests: quests.to_string(),
        dialogue: hw("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
    };
    let mut c = parse_campaign(&raw).expect("campaign parses");
    delvewright_dsl::tag_translatables(&mut c);
    c
}

fn structures(plan: &Plan) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                out.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    out
}

fn try_build(c: &Campaign) -> Result<BuildOutput, emit::BuildFailure> {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(c, &prefabs).expect("plan builds");
    let s = structures(&plan);
    emit::build(
        &plan,
        &BTreeMap::new(),
        &s,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
}

fn build(c: &Campaign) -> BuildOutput {
    try_build(c).expect("build succeeds")
}

fn failure_code(c: &Campaign) -> String {
    match try_build(c) {
        Ok(_) => panic!("expected the build to fail"),
        Err(emit::BuildFailure::Diagnostic { code, message }) => {
            eprintln!("{code}: {message}");
            code.to_string()
        }
        Err(other) => panic!("expected a diagnostic failure, got {other:?}"),
    }
}

fn text(out: &BuildOutput, path: &str) -> String {
    String::from_utf8(
        out.get(path)
            .unwrap_or_else(|| {
                panic!(
                    "`{path}` is emitted; have {:#?}",
                    out.keys().collect::<Vec<_>>()
                )
            })
            .clone(),
    )
    .unwrap()
}

fn fnc(out: &BuildOutput, name: &str) -> String {
    text(
        out,
        &format!("datapack/data/hello-world/function/{name}.mcfunction"),
    )
}

// ---------------------------------------------------------------------------
// The declarations every case below builds on
// ---------------------------------------------------------------------------

/// A player-scoped, NAMED datum — i.e. a currency — plus a stake that forfeits all
/// of it, plus the `on_death` beat that leaves one.
const PURSE_AND_STAKE: &str = r#",
    "state": [
      { "id": "state/embers", "scope": "player", "initial": 5, "name": "Embers",
        "note": "what the keeper takes and the drop takes back" }
    ],
    "stakes": [
      { "id": "stake/embers", "state": "state/embers",
        "collected_message": "You take back what the drop took." }
    ],
    "on_death": [ { "type": "drop-stake", "stake": "stake/embers" } ],
    "shops": [
      { "id": "shop/brazier", "anchor": "spawn", "title": "The brazier",
        "offers": [
          { "label": "Bank an ember", "tooltip": "Costs one ember.",
            "effects": [
              { "type": "narrate", "text": "You have nothing left to give.",
                "requires_state": [ { "state": "state/embers", "op": "at-most", "value": 0 } ] },
              { "type": "add-state", "state": "state/embers", "amount": -1,
                "requires_state": [ { "state": "state/embers", "op": "at-least", "value": 1 } ] }
            ] }
        ] }
    ]"#;

fn purse_campaign() -> Campaign {
    parse_hw(&quests_doc(PURSE_AND_STAKE, ""))
}

// ---------------------------------------------------------------------------
// AC1 — a currency is a NAMED datum, and the name reaches the player
// ---------------------------------------------------------------------------

/// A named datum **announces its new balance whenever it changes, from any cause**
/// — and the announcement belongs to the datum, not to each verb that writes it.
///
/// That is not a stylistic preference. The first implementation hung the readout
/// off the three state verbs, and reading the generated `shop_pick_0_0` showed why
/// it cannot live there: a readout emitted inside a gated effect carries that
/// effect's gate, and the gate is evaluated AFTER the write it reports. Spending
/// your last ember behind `requires_state: at-least 1` moves the balance to 0, the
/// inherited guard stops holding, and the one change the player most needs to see
/// is the one they are never told about. A shadow score removes the class.
///
/// Binding: the tick driver, the announcer and the shadow seed are each asserted,
/// and the negative half below proves an unnamed datum emits none of them.
#[test]
fn a_named_datum_announces_every_change_from_any_cause() {
    let out = build(&purse_campaign());

    let tick = fnc(&out, "tick");
    assert!(
        tick.contains(
            "execute as @a unless score @s dw.s_embers = @s dw.sh_embers run function \
             hello-world:st_show_embers"
        ),
        "one driver, keyed on the datum changing rather than on any write site:\n{tick}"
    );

    let show = fnc(&out, "st_show_embers");
    assert!(
        show.contains("title @s actionbar") && show.contains("\"score\""),
        "the balance is a live score component, not a number baked at emit time:\n{show}"
    );
    assert!(
        show.contains("\"translate\":\"state.embers.name\""),
        "…and the name travels as a translatable component:\n{show}"
    );
    assert!(
        show.contains("scoreboard players operation @s dw.sh_embers = @s dw.s_embers"),
        "…and it fires once per change, not once per tick:\n{show}"
    );

    assert!(
        fnc(&out, "state_seed").contains("scoreboard players set @s dw.sh_embers 5"),
        "the shadow is seeded to the datum's own initial, so joining a world \
         announces nothing — an announcement is for a CHANGE"
    );

    // The defect this shape exists to prevent: no readout may sit inside a gated
    // effect, where the gate is evaluated after the write it reports.
    for (path, bytes) in &out {
        if !path.ends_with(".mcfunction") || path.starts_with("packtest-datapack/") {
            continue;
        }
        for line in String::from_utf8(bytes.clone()).unwrap().lines() {
            if line.contains("actionbar") && line.contains("dw.s_embers") {
                assert!(
                    path.ends_with("st_show_embers.mcfunction"),
                    "the balance is announced from ONE place; `{path}` also does it: {line}"
                );
            }
        }
    }
}

/// A datum with **no** `name` is silent bookkeeping: it emits the same three
/// `scoreboard players` lines spec-0031 shipped and no readout at all. This is what
/// makes spec-0032 free for a spec-0031 campaign.
#[test]
fn an_unnamed_datum_emits_no_readout() {
    let unnamed = PURSE_AND_STAKE.replace(r#""name": "Embers","#, "");
    assert_ne!(
        unnamed, PURSE_AND_STAKE,
        "the fixture really carried a name"
    );
    let out = build(&parse_hw(&quests_doc(&unnamed, "")));
    for (path, bytes) in &out {
        if !path.ends_with(".mcfunction") {
            continue;
        }
        for line in String::from_utf8(bytes.clone()).unwrap().lines() {
            assert!(
                !(line.contains("actionbar") && line.contains("dw.s_embers")),
                "an unnamed datum states no balance: {path}: {line}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// AC2 — a price is the shared gate, and the shop adds NO comparison of its own
// ---------------------------------------------------------------------------

/// **The shop declares no comparison surface.** Asserted from the generated JSON
/// Schema — i.e. from the types — rather than from a hand-written list, so a
/// `price` field added tomorrow is red here even if nothing else notices.
///
/// The positive half (the shop offer carries the whole gate, all three fields) is
/// `crates/dsl/tests/gate_consumers.rs`, which enumerates the consumers from the
/// same schema; this is the negative half.
#[test]
fn a_shop_offer_declares_no_comparison_field_of_its_own() {
    let schema = delvewright_dsl::stage_schema(delvewright_dsl::envelope::Stage::Quests);
    let defs = schema
        .get("$defs")
        .and_then(|d| d.as_object())
        .expect("the quests schema has $defs");
    let offer = defs
        .get("ShopOffer")
        .and_then(|o| o.get("properties"))
        .and_then(|p| p.as_object())
        .expect("ShopOffer is in the schema");
    let names: Vec<&str> = offer.keys().map(String::as_str).collect();
    assert!(
        names.contains(&"requires_state"),
        "the price IS the shared gate: {names:?}"
    );
    for banned in ["price", "cost", "costs", "compare", "at_least", "requires"] {
        assert!(
            !names.contains(&banned),
            "a shop offer must express a price as the shared gate, never as `{banned}`: {names:?}"
        );
    }
    // Nothing else in the shop's own structs may carry a comparison either.
    for ty in ["Shop", "Stake"] {
        let props = defs
            .get(ty)
            .and_then(|o| o.get("properties"))
            .and_then(|p| p.as_object())
            .unwrap_or_else(|| panic!("{ty} is in the schema"));
        assert!(
            !props.contains_key("requires_state"),
            "{ty} is not a gate consumer; only the OFFER is"
        );
    }
}

/// A shop rides the machinery that already ships: an interaction hitbox, a
/// player-interaction advancement that supplies the acting player, a
/// `minecraft:multi_action` dialog whose buttons run `/trigger`, and tick dispatch.
///
/// Binding: the offer count is asserted, so a shop that emitted zero buttons could
/// not pass this by having nothing to check.
#[test]
fn a_shop_is_the_rest_flow_with_different_buttons() {
    let out = build(&purse_campaign());

    let setup = fnc(&out, "setup_finish") + &fnc(&out, "setup");
    assert!(
        setup.contains("summon minecraft:interaction")
            && setup.contains("dw_shop_0")
            && setup.contains("dw_hw_dw_shop_0"),
        "the hitbox and its VISIBLE marker are both armed"
    );
    assert!(
        setup.contains("scoreboard objectives add dw.shop trigger"),
        "`/trigger` is the only command a non-op player may run"
    );

    let adv: serde_json::Value = serde_json::from_str(&text(
        &out,
        "datapack/data/hello-world/advancement/shop_0.json",
    ))
    .unwrap();
    assert_eq!(
        adv["criteria"]["interact"]["trigger"],
        serde_json::json!("minecraft:player_interacted_with_entity")
    );
    assert_eq!(
        adv["rewards"]["function"],
        serde_json::json!("hello-world:shop_open_0"),
        "the reward runs AS the player who clicked — the only way to know who is buying"
    );

    let dialog: serde_json::Value =
        serde_json::from_str(&text(&out, "datapack/data/hello-world/dialog/shop_0.json")).unwrap();
    assert_eq!(dialog["type"], serde_json::json!("minecraft:multi_action"));
    let actions = dialog["actions"].as_array().expect("actions is a list");
    assert_eq!(actions.len(), 1, "one offer, one button — binding count");
    assert_eq!(
        actions[0]["action"]["command"],
        serde_json::json!("/trigger dw.shop set 1")
    );
    assert!(
        actions[0]["label"].get("translate").is_some(),
        "the caption travels as a translatable component"
    );

    let tick = fnc(&out, "tick");
    assert!(
        tick.contains("scores={dw.shop=1,dw.shop_at=0}")
            && tick.contains("function hello-world:shop_pick_0_0"),
        "tick dispatch, keyed on the answer AND on which shop the player opened:\n{tick}"
    );
}

/// AC3, the runtime half: a purchase that cannot be afforded is refused **and says
/// so**, and both halves are the ordinary per-effect gate — the engine adds no
/// `refused` field.
#[test]
fn an_unaffordable_purchase_is_refused_and_says_so() {
    let out = build(&purse_campaign());
    let pick = fnc(&out, "shop_pick_0_0");
    assert!(
        pick.starts_with("scoreboard players reset @s dw.shop"),
        "the trigger is disarmed FIRST, before anything can `return`:\n{pick}"
    );
    assert!(
        pick.contains(
            "if score @s dw.s_embers matches 1.. run scoreboard players remove @s dw.s_embers 1"
        ),
        "the debit is gated on affording it:\n{pick}"
    );
    assert!(
        pick.contains("if score @s dw.s_embers matches ..0")
            && pick.contains("actionbar") | pick.contains("tellraw"),
        "the refusal is an ordinary gated effect, on the complementary range:\n{pick}"
    );
}

/// A gated offer is inert to a **direct** `/trigger` — the bot's path, which
/// bypasses any UI. Same discipline a dialogue option's handler already applies.
#[test]
fn an_offer_gate_is_inert_to_a_direct_trigger() {
    let gated = PURSE_AND_STAKE.replace(
        r#"{ "label": "Bank an ember", "tooltip": "Costs one ember.","#,
        r#"{ "label": "Bank an ember", "tooltip": "Costs one ember.",
            "requires_state": [ { "state": "state/embers", "op": "at-least", "value": 3 } ],"#,
    );
    assert_ne!(gated, PURSE_AND_STAKE);
    let out = build(&parse_hw(&quests_doc(&gated, "")));
    let pick = fnc(&out, "shop_pick_0_0");
    assert!(
        pick.contains("unless score @s dw.s_embers matches 3.. run return fail"),
        "the offer's own gate shuts a direct `/trigger`:\n{pick}"
    );
}

/// **A shop's interaction point is visible to the body-eclipse proof** (`DW0359`).
///
/// This is the instance fix's general form, and it is here because the gap was
/// real and shipped: there are TWO affordance authorities — `emit::affordances`,
/// which `DW0420`/`DW0421` read, and `eclipse::affordances`, which carries a
/// resolved cell and is read by `DW0359`, and by any later proof that needs to
/// know where an affordance stands. Registering the shop with only the first left
/// its hitbox invisible
/// to every proof that reasons about WHERE an affordance stands, and this PR's own
/// two fixtures both put a shop on an anchor an NPC was standing on. Both compiled
/// green.
///
/// A body on the shop's anchor must now be a build failure naming the shop.
#[test]
fn a_body_standing_on_a_shop_eclipses_it() {
    // `npc/keeper` stands at `anchor/keeper-stand` in the hello-world base, so a
    // shop declared there is exactly the defect.
    let eclipsed = PURSE_AND_STAKE.replace(
        r#""anchor": "spawn", "title""#,
        r#""anchor": "anchor/keeper-stand", "title""#,
    );
    assert_ne!(
        eclipsed, PURSE_AND_STAKE,
        "the shop really moved onto the NPC"
    );
    let code = failure_code(&parse_hw(&quests_doc(&eclipsed, "")));
    assert_eq!(
        code,
        delvewright_compiler::eclipse::DW_BODY_ECLIPSE,
        "a keeper standing in front of his own brazier is a brazier nobody can press"
    );
}

/// …and a recovery stake is deliberately NOT in that authority, which is a
/// decision rather than an omission.
///
/// A stake's marker is summoned at runtime, at a position chosen at runtime — so
/// there is no compile-time cell for a body-eclipse proof, or any other proof
/// about where an affordance stands, to test. Asserted so the absence cannot be
/// mistaken for the gap the test above closes.
#[test]
fn a_stake_has_no_compile_time_cell_to_eclipse() {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let c = purse_campaign();
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let posts = delvewright_compiler::eclipse::affordance_cells(&plan);
    assert!(
        posts.iter().any(|(kind, _, _)| *kind == "shop"),
        "the shop IS in the authority: {posts:#?}"
    );
    assert!(
        !posts.iter().any(|(kind, _, _)| *kind == "recovery stake"),
        "a stake has no compile-time cell, so it cannot be in a proof that needs \
         one — what CAN be said about its anchor is `DW0525`/`DW0526`: {posts:#?}"
    );
}

// ---------------------------------------------------------------------------
// AC4 / AC5 — the placement table
// ---------------------------------------------------------------------------

/// The compile-time table is emitted as a fixed `execute if` chain: one line per
/// (death region × respawn seat), keyed on the corpse's position and on `#cp`.
/// **No runtime search** — every anchor in the chain is a literal.
///
/// Binding: the row count is read off the emitted artifact and asserted non-zero,
/// and the artifact's own ledger is cross-checked against it.
#[test]
fn the_placement_table_is_a_compile_time_chain_with_no_search() {
    let with_volume = format!(
        "{PURSE_AND_STAKE},\n    \"lethal_volumes\": [ {{ \"id\": \"lethal/the-drop\", \
         \"region\": {{ \"anchor\": \"anchor/exit\", \"extent\": [0, 0, 0] }}, \
         \"message\": \"The floor gives way.\" }} ]"
    );
    let out = build(&parse_hw(&quests_doc(&with_volume, "")));
    let route = fnc(&out, "stk_route_embers");

    let rows: Vec<&str> = route
        .lines()
        .filter(|l| l.contains("if score #cp dw.sys matches"))
        .collect();
    assert!(
        !rows.is_empty(),
        "the table binds to at least one (region, seat) pair — a zero binding is a \
         failure, not a pass:\n{route}"
    );
    for r in &rows {
        assert!(
            r.contains("if entity @s[x=") && r.contains("run return run function"),
            "each row tests the corpse against a compile-time box:\n{r}"
        );
    }
    assert!(
        route.lines().last().unwrap().contains("stk_here_"),
        "and the degenerate branch — a death on ground you can walk back to leaves \
         the stake where you fell — is the fallthrough:\n{route}"
    );

    // Every anchor the chain points at is a literal position, not a search.
    for (n, _) in rows.iter().enumerate() {
        let put = fnc(&out, &format!("stk_put_embers_{n}"));
        assert!(
            put.starts_with("execute positioned "),
            "anchor {n} is a literal:\n{put}"
        );
        assert!(
            !put.contains("sort=nearest") && !put.contains("if block"),
            "nothing here searches at runtime:\n{put}"
        );
    }

    let ledger: serde_json::Value =
        serde_json::from_str(&text(&out, "validation/stake-gate.json")).unwrap();
    assert_eq!(ledger["unbound"], serde_json::json!(false));
    assert_eq!(
        ledger["rows_proved"].as_u64().unwrap() as usize,
        rows.len(),
        "the ledger's row count and the emitted chain are the same number, derived \
         once: {ledger}"
    );
    assert!(ledger["respawn_seats"].as_u64().unwrap() >= 1);
    assert!(ledger["quest_state_configurations"].as_u64().unwrap() >= 1);
}

/// The rule degenerates correctly: with no lethal volume and no runtime-mutable
/// ground near it, a death on ordinary walkable ground leaves the stake **at the
/// death point** — which the emitted `stk_here_` branch spells as `at @s`, the
/// corpse's own position.
///
/// That the corpse stands on the death position is measured, not assumed
/// (`docs/notes/death-and-teleport-spike.md`, 15 trials, drift 0.000).
#[test]
fn an_ordinary_death_leaves_the_stake_where_the_player_fell() {
    let out = build(&purse_campaign());
    assert_eq!(
        fnc(&out, "stk_here_embers").trim(),
        "execute at @s run function hello-world:stk_fill_embers"
    );
    let drop = fnc(&out, "stk_drop_embers");
    assert!(
        drop.contains("function hello-world:stk_route_embers"),
        "the drop routes through the table even in the degenerate case:\n{drop}"
    );
}

/// **A `clear-region` is ground the runtime removes, exactly as a lift car is.**
///
/// The merge-induced coverage question, answered rather than assumed: spec-0031's
/// `fill-region` / `clear-region` are two new ways a block stops being there, and
/// a stake standing on one would be destroyed by the next firing — the same defect
/// `DW0526` was written for, arriving from a new verb.
///
/// They enter through `QuestEffect::region_write`, the DSL's own answer to "which
/// verbs rewrite a box", so a later verb of that family is covered by existing.
/// Asserted with a binding count, because a walk that found no region write would
/// pass this by examining nothing.
#[test]
fn a_region_write_is_ground_a_stake_may_not_stand_on() {
    // The clear rides `on_death`, which is an OPTIONAL root — nobody is forced to
    // die — so the completability model deliberately drops it. That is the whole
    // point of the second half of this test.
    let with_clear = PURSE_AND_STAKE.replace(
        r#""on_death": [ { "type": "drop-stake", "stake": "stake/embers" } ]"#,
        r#""on_death": [ { "type": "drop-stake", "stake": "stake/embers" },
                     { "type": "clear-region",
                       "region": { "anchor": "anchor/exit", "extent": [1, 1, 1] } } ]"#,
    );
    assert_ne!(
        with_clear, PURSE_AND_STAKE,
        "the clear really was spliced in"
    );
    let c = parse_hw(&quests_doc(&with_clear, ""));
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let mutable = delvewright_compiler::stake::runtime_mutable_regions(&plan);

    let named: Vec<&String> = mutable.iter().map(|(l, _)| l).collect();
    assert!(
        named.iter().any(|l| l.contains("clear-region")),
        "the `clear-region` volume is ground a stake may not stand on: {named:#?}"
    );
    // …and the trigger it hangs off is an OPTIONAL root, which the completability
    // model deliberately drops. This set must not: a clear the party may never
    // trigger is still ground the marker cannot survive if they do.
    let clears = plan
        .region_events
        .iter()
        .filter(|e| e.write == delvewright_compiler::plan::RegionWrite::Clear)
        .count();
    assert_eq!(
        clears, 0,
        "the completability model dropped this optional-root clear (an optional \
          firing may fill, never open) — which is exactly why \
         `runtime_mutable_regions` cannot simply BE `plan.region_events`: a clear \
          the party may never trigger is still ground the marker cannot survive if \
          they do. Same geometry, opposite conservatism."
    );
    eprintln!("mutable regions examined: {}", mutable.len());
    assert!(mutable.len() >= 2, "binding count: {mutable:#?}");
}

/// A stake anchor never sits on a block runtime can remove. The gate region of
/// `anchor/door` is rewritten by `open-gate`, so its cells and the cells standing
/// on them are excluded from every candidate set — asserted by comparing the
/// chosen anchors against the mutable set the compiler computed.
#[test]
fn no_anchor_stands_on_ground_the_runtime_rewrites() {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let c = purse_campaign();
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let mutable = delvewright_compiler::stake::runtime_mutable_regions(&plan);
    assert!(
        !mutable.is_empty(),
        "this fixture really has runtime-mutable ground (the `open-gate` region) — a \
         zero binding would make the assertion below vacuous"
    );
    let out = build(&c);
    let route = fnc(&out, "stk_route_embers");
    let mut anchors = 0usize;
    for n in 0.. {
        let path = format!("datapack/data/hello-world/function/stk_put_embers_{n}.mcfunction");
        let Some(body) = out.get(&path) else { break };
        anchors += 1;
        let body = String::from_utf8(body.clone()).unwrap();
        let coords: Vec<f64> = body
            .split_whitespace()
            .skip(2)
            .take(3)
            .map(|t| t.parse().unwrap())
            .collect();
        let cell = [
            coords[0].floor() as i32,
            coords[1].floor() as i32,
            coords[2].floor() as i32,
        ];
        for (label, r) in &mutable {
            let below = [cell[0], cell[1] - 1, cell[2]];
            for probe in [cell, below] {
                let inside = (0..3).all(|i| r.0[i] <= probe[i] && probe[i] <= r.1[i]);
                assert!(
                    !inside,
                    "anchor {cell:?} stands in or on {label} — the next rewrite would \
                     destroy the marker"
                );
            }
        }
    }
    eprintln!(
        "examined {anchors} anchors against {} mutable regions",
        mutable.len()
    );
    let _ = route;
}

/// AC8: a campaign in which a stake could be left with no walkable route back
/// fails to compile, naming the death region and the quest state.
///
/// **The scenario is the real one, not a synthetic break.** A lethal volume laid
/// across the keep's one doorway cuts the corridor in two; the party's respawn
/// point is moved to the far side (`set-checkpoint` is reached by declaration, not
/// by walking, so nothing else objects); and the critical path never crosses the
/// volume, so `DW0510` has nothing to say. What is left is exactly the finding this
/// rule exists for: **there are cells a player can walk to and die on, and from the
/// respawn point in force there is no way back to them** — so the stake the death
/// leaves would be stranded forever.
///
/// Note the ordering that makes this test meaningful at all: `DW0311`, `DW0510` and
/// `DW0511` all run BEFORE the stake table, so a case that tripped any of them would
/// be proving one of their rules rather than this one.
#[test]
fn a_stake_with_no_route_back_fails_to_compile() {
    let cut = format!(
        "{PURSE_AND_STAKE},\n    \"lethal_volumes\": [ {{ \"id\": \"lethal/the-threshold\", \
         \"region\": {{ \"anchor\": \"anchor/door\", \"extent\": [3, 3, 1] }}, \
         \"message\": \"The threshold has gone.\" }} ]"
    );
    // One objective only, so no leg of the critical path crosses the volume.
    let one_beat = quests_doc(&cut, "").replace(
        r#"          {{ "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit",
             "radius": 2, "after": ["obj/talk"] }}"#,
        "",
    );
    let one_beat = one_beat
        .replace(
            r#"{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" },"#,
            r#"{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }"#,
        )
        .replace(
            r#"{ "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit",
             "radius": 2, "after": ["obj/talk"] }"#,
            "",
        )
        .replace(
            r#"[ { "type": "open-gate", "anchor": "anchor/door" } ]"#,
            r#"[ { "type": "set-checkpoint", "anchor": "anchor/exit" } ]"#,
        );
    let code = failure_code(&parse_hw(&one_beat));
    assert_eq!(
        code,
        delvewright_compiler::stake::DW_STAKE_NO_ROUTE_BACK,
        "a respawn point with no way back to where the party can die strands a stake"
    );
}

// ---------------------------------------------------------------------------
// AC6 / AC7 — collection and retention
// ---------------------------------------------------------------------------

/// Collecting restores exactly the amount recorded, retires the hardware through
/// the ONE function allowed to (`DW0421`), and is idempotent under a double
/// right-click in one tick — structurally, because taking a slot clears its live
/// flag as part of taking it.
#[test]
fn collecting_restores_the_amount_and_is_idempotent() {
    let out = build(&purse_campaign());
    let collect = fnc(&out, "stk_collect_embers");
    assert!(
        collect.starts_with("advancement revoke @s only hello-world:stk_embers"),
        "the grant is consumed first:\n{collect}"
    );
    assert!(
        collect.contains("run return run function hello-world:stk_take_embers_0"),
        "`return run` means at most ONE slot is taken per press:\n{collect}"
    );

    let take = fnc(&out, "stk_take_embers_0");
    assert!(
        take.contains("scoreboard players operation @s dw.s_embers += @s dw.kv0_embers"),
        "the exact amount recorded comes back:\n{take}"
    );
    assert!(
        take.contains("scoreboard players set @s dw.kl0_embers 0"),
        "…and the slot goes dead in the same breath, which is what makes a second \
         press in the same tick a no-op:\n{take}"
    );

    // `DW0421` — exactly one function may kill the hardware.
    let mut killers = Vec::new();
    for (path, bytes) in &out {
        if !path.ends_with(".mcfunction") || path.starts_with("packtest-datapack/") {
            continue;
        }
        if String::from_utf8(bytes.clone())
            .unwrap()
            .contains("kill @e[tag=dw_hw_dw_stk_embers")
        {
            killers.push(path.clone());
        }
    }
    assert_eq!(
        killers,
        vec!["datapack/data/hello-world/function/stk_gc_embers.mcfunction".to_string()],
        "one piece of hardware, one legal killer"
    );
}

/// Every retention policy value is exercised, and each emits a different drop
/// (AC7). The table is the test: a policy that stopped changing the emission
/// would collapse two rows into one and fail here.
#[test]
fn every_retention_policy_value_is_exercised() {
    let policy = |body: &str| -> String {
        let src = PURSE_AND_STAKE.replace(
            r#"{ "id": "stake/embers", "state": "state/embers","#,
            &format!(r#"{{ "id": "stake/embers", "state": "state/embers", {body}"#),
        );
        assert_ne!(src, PURSE_AND_STAKE, "the policy really was spliced in");
        let out = build(&parse_hw(&quests_doc(&src, "")));
        out.get("datapack/data/hello-world/function/stk_drop_embers.mcfunction")
            .map(|b| String::from_utf8(b.clone()).unwrap())
            .unwrap_or_default()
    };

    let replace = policy(r#""max_live": 1, "on_full": "replace","#);
    assert!(
        replace.contains("run function hello-world:stk_evict_embers"),
        "`replace` retires the oldest and places a new one:\n{replace}"
    );

    let keep = policy(r#""max_live": 1, "on_full": "keep","#);
    assert!(
        keep.contains("run return fail") && !keep.contains("stk_evict"),
        "`keep` leaves the existing wager alone and forfeits nothing:\n{keep}"
    );

    let memorial = policy(r#""max_live": 3, "on_full": "keep","#);
    for k in 0..3 {
        assert!(
            memorial.contains(&format!("dw.kl{k}_embers")),
            "a memorial at up to three death sites needs three slots:\n{memorial}"
        );
    }

    let none = policy(r#""max_live": 0,"#);
    assert!(
        none.trim().is_empty(),
        "`max_live: 0` is the no-death-cost configuration: nothing is forfeited and \
         nothing is placed:\n{none}"
    );

    let free = policy(r#""forfeit": { "kind": "none" },"#);
    assert!(
        free.contains("scoreboard players set #stk_amt dw.sys 0"),
        "`forfeit: none` marks the spot without taking anything:\n{free}"
    );

    let half = policy(r#""forfeit": { "kind": "proportion", "percent": 50 },"#);
    assert!(
        half.contains("*= #stk_p50 dw.sys") && half.contains("/= #stk_100 dw.sys"),
        "a proportion is integer arithmetic — no floats anywhere (ADR-0006):\n{half}"
    );

    let fixed = policy(r#""forfeit": { "kind": "fixed", "amount": 10 },"#);
    assert!(
        fixed.contains("matches 11.. run scoreboard players set #stk_amt dw.sys 10"),
        "a fixed forfeit is capped at the balance, so a purse can never go negative:\n{fixed}"
    );

    let anyone = policy(r#""collect_by": "anyone","#);
    assert!(
        !anyone.is_empty(),
        "`collect_by` does not change the drop, only the collection"
    );
    let out = build(&parse_hw(&quests_doc(
        &PURSE_AND_STAKE.replace(
            r#"{ "id": "stake/embers", "state": "state/embers","#,
            r#"{ "id": "stake/embers", "state": "state/embers", "collect_by": "anyone","#,
        ),
        "",
    )));
    assert!(
        fnc(&out, "stk_collect_embers")
            .contains("execute as @a run function hello-world:stk_pool_embers"),
        "`anyone` sweeps every player's wager at this place into the collector's purse"
    );
}

/// The stake ledger is per player, not party-shared (AC7) — the multiplayer
/// decision spec-0032 records "for correction rather than left to emerge".
#[test]
fn the_stake_ledger_is_per_player() {
    let out = build(&purse_campaign());
    let party = delvewright_compiler::plan::PARTY;
    for f in ["stk_drop_embers", "stk_slot_embers_0", "stk_take_embers_0"] {
        let body = fnc(&out, f);
        assert!(
            !body.contains(party),
            "`{f}` must never touch the party holder — a stake is a personal wager:\n{body}"
        );
        assert!(
            body.contains("@s "),
            "…and it is written against the acting player:\n{body}"
        );
    }
}

// ---------------------------------------------------------------------------
// Byte identity and determinism
// ---------------------------------------------------------------------------

/// Determinism (ADR-0006): two builds of an economy campaign are byte-equal.
#[test]
fn an_economy_build_is_byte_identical_across_runs() {
    let c = purse_campaign();
    assert_eq!(build(&c), build(&c));
}

/// **A campaign that declares neither a shop nor a stake emits exactly what it
/// emitted before spec-0032** — asserted against the same campaign built with the
/// economy sections removed, over the whole tree.
#[test]
fn a_campaign_without_an_economy_is_untouched() {
    let bare = build(&parse_hw(&quests_doc("", "")));
    for (path, bytes) in &bare {
        if !path.ends_with(".mcfunction") {
            continue;
        }
        let body = String::from_utf8(bytes.clone()).unwrap();
        for token in ["dw.shop", "dw.kv0_", "stk_", "#stk_"] {
            assert!(
                !body.contains(token),
                "`{token}` leaked into `{path}` of a campaign that declares no economy"
            );
        }
    }
    assert!(
        !bare.contains_key("validation/stake-gate.json"),
        "no stake, no ledger — a ledger that exists and reports zero is a finding"
    );
}

/// The runtime tier carries exactly the two halves it can witness — and NOT the
/// death edge, which a PackTest fake player cannot produce (measured twice: a fake
/// player is permanently undamageable).
///
/// Asserted in both directions on purpose. The positive half stops the templates
/// quietly disappearing; the negative half stops somebody adding a
/// "stake on death" template that binds to nothing and reports green, which is
/// worse than an absence because review cannot see it.
#[test]
fn the_packtest_tier_covers_what_it_can_witness_and_claims_nothing_more() {
    let out = build(&purse_campaign());
    let shop = text(
        &out,
        "packtest-datapack/data/hello-world/test/v10_shop_purchase.mcfunction",
    );
    assert!(
        shop.contains("function hello-world:shop_pick_0_0") && shop.contains("assert score"),
        "the purchase and the refusal are both driven and asserted:\n{shop}"
    );

    let stake = text(
        &out,
        "packtest-datapack/data/hello-world/test/v10_stake_embers.mcfunction",
    );
    assert!(
        stake.contains("function hello-world:stk_drop_embers")
            && stake.contains("function hello-world:stk_collect_embers")
            && stake.contains("if entity @e[tag=dw_stk_embers]"),
        "the drop → collect round trip is driven end to end:\n{stake}"
    );
    assert!(
        stake.contains("NOT a death test"),
        "…and the template says what it does not cover:\n{stake}"
    );

    // No generated template may drive the death edge: there is nothing at this tier
    // that could make a fake player die, so such a template would assert on a
    // branch that never runs.
    for (path, bytes) in &out {
        if !path.starts_with("packtest-datapack/") {
            continue;
        }
        let body = String::from_utf8(bytes.clone()).unwrap();
        assert!(
            !body.contains("function hello-world:on_death_fire"),
            "`{path}` drives the death beat, which this tier cannot witness"
        );
    }
}

// --- the CI fixture -------------------------------------------------------

/// The `economy` fixture validates clean and emits the whole chain.
#[test]
fn the_ci_fixture_validates_and_emits_the_chain() {
    use delvewright_compiler::load::load_campaign_dir;
    use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry};

    let dir = common::compiler_fixtures_dir().join("economy");
    let loaded = load_campaign_dir(&dir).unwrap();
    let mut c = parse_campaign(&loaded.raw).expect("the economy fixture parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let diags = delvewright_dsl::validate_campaign_with(
        &c,
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    );
    assert!(diags.is_empty(), "the fixture validates clean: {diags:#?}");

    delvewright_dsl::tag_translatables(&mut c);
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let s = structures(&plan);
    let out = emit::build(
        &plan,
        &BTreeMap::new(),
        &s,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
    .expect("the economy fixture builds");
    for f in [
        "stk_drop_embers",
        "stk_route_embers",
        "stk_fill_embers",
        "stk_collect_embers",
        "stk_gc_embers",
        "shop_open_0",
        "shop_pick_0_0",
    ] {
        assert!(
            out.contains_key(&format!("datapack/data/economy/function/{f}.mcfunction")),
            "the fixture emits `{f}`"
        );
    }
    let ledger: serde_json::Value = serde_json::from_str(
        &String::from_utf8(out["validation/stake-gate.json"].clone()).unwrap(),
    )
    .unwrap();
    assert_eq!(ledger["unbound"], serde_json::json!(false), "{ledger}");
    assert_eq!(ledger["stakes_declared"], serde_json::json!(1));
}
