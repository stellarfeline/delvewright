//! **Every effect root × every campaign-wide walker, enumerated from the type.**
//!
//! `effect_root_sweep` proves the *enumeration* reaches every root.
//! `gate_model_roots`, `timeline_effect_roots`, `flow_effect_roots`,
//! `flag_objective_roots`, `anchor_seal` and `l10n_effect_roots` each prove one
//! walker reaches the roots. What none of them is, and what this file is, is a
//! **matrix**: the roots come out of `EffectRootKind::ALL`, the fixture for each
//! comes out of an exhaustive `match`, and every walker is asked about every root.
//!
//! Why that shape and not another per-walker test. This repo's most-repeated
//! defect is a walk that enumerates *some* roots, and the
//! reason it kept recurring after each fix is that a per-walker test proves one
//! walker against the roots its author remembered. Add a root and every one of
//! those tests stays green while saying nothing about it. Here, adding a root is a
//! **compile error** in `probe_at`, and the new row is then asked the same
//! questions as every other row — so a root that some walker cannot see is red the
//! day it is added rather than the day content routes an effect through it.
//!
//! The sixth root proves the point in reverse: `shortcuts[].on_unlock` was a
//! `Vec<QuestEffect>` emission had lowered for two versions, invisible to every
//! walker, and no existing test could have said so.
//!
//! ## The probe, and what each walker is asked
//!
//! One campaign per root, each carrying the same two-effect probe at that root and
//! nowhere else:
//!
//! ```json
//! [ { "set-flag": "flag/probe-<root>" },
//!   { "narrate": "<unique text>", "requires_flags": ["flag/probe-<root>"] } ]
//! ```
//!
//! Six walkers, one distinct observable each — chosen so that a walker which
//! stopped inheriting the enumeration would go red here rather than merely
//! narrow:
//!
//! | walker | observable |
//! |---|---|
//! | `dsl::for_each_effect_root` | the `RootBinding` counts ≥ 1 site at this root |
//! | `dsl::for_each_campaign_effect` | the probe narrate is yielded, with an `EffectSite` for this root |
//! | `dsl::l10n_inventory` (→ `for_each_effect_root_mut`) | the probe text has an inventory key |
//! | `compiler::flow::gate_flags` | `flag/probe-<root>` is a flag some gate reads |
//! | `emit::declared_flags` | `setup` declares `dw.f_probe_<root>` |
//! | `emit::all_campaign_effects` + `emit_effect_bundle` | the probe text is in an emitted function |
//!
//! and a seventh, asked separately because its observable is a *failed* build:
//! `emit::check_effect_anchors` (`DW0360`) must reject a bogus anchor at every
//! root.
//!
//! ## Binding
//!
//! Every assertion names its root, and the matrix asserts up front that it built
//! one fixture per member of `EffectRootKind::ALL` — a run that examined fewer
//! roots than exist is a vacuous green, not a pass (CLAUDE.md).

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildFailure, BuildOutput};
use delvewright_compiler::flow::gate_flags;
use delvewright_compiler::load::{LoadedCampaign, load_campaign_dir};
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{Campaign, EffectRootKind, EffectSite, QuestEffect, parse_campaign};

/// The `souls-shortcut` fixture is the base for every row, because it is the only
/// one in the tree that already carries a **shortcut** — root 6 needs real
/// geometry (`DW0371`/`DW0373`/`DW0374`), which no in-memory mutation can invent.
/// Everything else a row needs is added to the parsed campaign.
const NS: &str = "souls-shortcut";

// ---------------------------------------------------------------------------
// per-root identity
// ---------------------------------------------------------------------------

/// A kebab slug for a root, derived from [`EffectRootKind::label`] so a new root
/// needs no second name.
fn slug(k: EffectRootKind) -> String {
    k.label().replace([' ', '_', '[', ']', '.'], "-")
}

/// The flag the probe at root `k` sets — and, on the same bundle's narrate,
/// reads.
fn probe_flag(k: EffectRootKind) -> String {
    format!("flag/probe-{}", slug(k))
}

/// The scoreboard objective `emit::declared_flags` must declare for that flag.
fn probe_objective(k: EffectRootKind) -> String {
    format!("dw.f_probe_{}", slug(k).replace('-', "_"))
}

/// The probe's player-visible line — unique per root, so finding it in the l10n
/// inventory or in an emitted function attributes to exactly one root.
fn probe_text(k: EffectRootKind) -> String {
    format!("A probe fired from the {} root.", k.label())
}

/// The anchor the `DW0360` row names — deliberately absent from every prefab.
fn bogus_anchor(k: EffectRootKind) -> String {
    format!("anchor/nowhere-{}", slug(k))
}

/// The two-effect probe bundle, as JSON — the form every root binds it from, so
/// the same text is what a campaign author would have written.
fn probe_bundle(k: EffectRootKind) -> String {
    format!(
        r#"[
          {{ "type": "set-flag", "flag": "{flag}" }},
          {{ "type": "narrate", "style": "chat", "text": "{text}",
             "requires_flags": ["{flag}"] }}
        ]"#,
        flag = probe_flag(k),
        text = probe_text(k),
    )
}

/// The `DW0360` probe: one anchor-bearing effect naming an anchor that resolves
/// to nothing.
fn bogus_anchor_bundle(k: EffectRootKind) -> String {
    format!(
        r#"[ {{ "type": "set-block", "anchor": "{}", "block": "minecraft:stone" }} ]"#,
        bogus_anchor(k)
    )
}

fn json_effects(src: &str) -> Vec<QuestEffect> {
    serde_json::from_str(src).expect("probe bundle parses as quest effects")
}

// ---------------------------------------------------------------------------
// fixture plumbing
// ---------------------------------------------------------------------------

/// A prefab tree with an `anchor/trap` (a dispenser-backed trigger cell) added to
/// `hello-room`, which root 4 needs and the fixture does not otherwise have.
fn prefabs_with_trap() -> PathBuf {
    // Materialized EXACTLY ONCE per process, behind a `OnceLock`.
    //
    // It was a `if dir.join("hello-room.json").exists() { return dir }` cache in
    // front of a `remove_dir_all` + copy. Both tests in this binary run in
    // parallel threads and both call this, so on a cold cache both could see
    // `exists() == false`, and the loser's `remove_dir_all` then deleted the
    // library the winner was already handing to `PrefabRegistry::load_dir` —
    // surfacing as `DW0300: no matching prefab metadata was found in the prefabs
    // dir` in exactly one of the two tests. A time-of-check/time-of-use window of
    // microseconds on a fast local disk, and wide enough to fire on CI once a
    // sibling test binary was added that copies the same 76-file library
    // alongside it. `OnceLock` closes it by construction: one initializer runs,
    // every other caller blocks on it and then reads a finished directory, and
    // nothing destructive ever runs concurrently with a read.
    //
    // CLAUDE.md: an intermittent red is a finding, not something to re-run.
    static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let dir =
            std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("effect_root_walkers_prefabs");
        let _ = std::fs::remove_dir_all(&dir);
        common::copy_dir_all(&common::prefabs_dir(), &dir);
        let path = dir.join("hello-room.json");
        let mut meta: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let anchors = meta
            .get_mut("anchors")
            .and_then(|a| a.as_object_mut())
            .unwrap();
        anchors.insert(
            "anchor/trap".to_string(),
            serde_json::json!({ "pos": [5, 1, 6], "dispenser": [4, 1, 6] }),
        );
        // …and a place for the shop probe to stand that nothing else claims.
        // `hello-room` offers four anchors and every one of them is already
        // spoken for by something that owns a hitbox: `anchor/exit` carries the
        // shortcut's unlock affordance (two interaction boxes on one cell is
        // `DW0878`), `anchor/door` is the gate region whose sealed door arms a
        // press body (`DW0422`), `anchor/keeper-stand` is where `npc/keeper`
        // stands (`DW0359`), and `spawn` is where the party arrives. The probe is
        // about EFFECT ROOTS, so it must not be co-declaring a geometry defect on
        // the way to exercising one.
        anchors.insert(
            "anchor/shop".to_string(),
            serde_json::json!({ "pos": [7, 1, 8] }),
        );
        std::fs::write(&path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();
        dir
    })
    .clone()
}

fn load() -> LoadedCampaign {
    load_campaign_dir(&common::compiler_fixtures_dir().join(NS)).unwrap()
}

/// The fixture campaign with `bundle` bound at root `k` and nowhere else.
///
/// **The exhaustive `match` is the point of this file.** A ninth root cannot be
/// added without deciding, here, how a campaign binds it — which is the question
/// nobody was asked when `shortcuts[].on_unlock` was written.
fn probe_at(loaded: &LoadedCampaign, k: EffectRootKind, bundle_json: &str) -> Campaign {
    let mut c = parse_campaign(&loaded.raw).expect("fixture parses");
    let bundle = json_effects(bundle_json);
    // Both stages are raised to the version the probe needs: `set-checkpoint`
    // (root 5) is v0.6 surface on the dialogue stage, `on_death` (root 7) is v0.10
    // on the quests stage. Raising a version never removes surface.
    c.quests.dsl_version = "0.10.0".to_string();
    c.dialogue.dsl_version = "0.6.0".to_string();
    match k {
        EffectRootKind::ObjectiveComplete => {
            let q = &mut c.quests.content.quests[0];
            let key = q.on_objective_complete.keys().next().cloned().unwrap();
            q.on_objective_complete
                .get_mut(&key)
                .unwrap()
                .extend(bundle);
        }
        EffectRootKind::QuestComplete => {
            // Ahead of `campaign-complete`, which ends the delve.
            let done = &mut c.quests.content.quests[0].on_complete;
            done.splice(0..0, bundle);
        }
        EffectRootKind::Trigger => {
            c.quests.content.triggers[0].effects.extend(bundle);
        }
        EffectRootKind::TrapPayload => {
            let mut trap: delvewright_dsl::Trap = serde_json::from_str(
                r#"{ "id": "trap/probe", "at": "anchor/trap", "trigger": "pressure-plate",
                     "lethality": "harmful", "payload": [] }"#,
            )
            .expect("probe trap parses");
            trap.payload = bundle;
            c.quests.content.traps.push(trap);
        }
        EffectRootKind::DialogueRespawn => {
            let eff: delvewright_dsl::DialogueEffect = serde_json::from_str(&format!(
                r#"{{ "type": "set-checkpoint", "anchor": "anchor/exit",
                      "on_respawn": {bundle_json} }}"#
            ))
            .expect("probe set-checkpoint parses");
            c.dialogue.content.dialogues[0].nodes[0].options[0]
                .effects
                .push(eff);
        }
        EffectRootKind::ShortcutUnlock => {
            c.quests.content.shortcuts[0].on_unlock = bundle;
        }
        EffectRootKind::OnDeath => {
            c.quests.content.on_death = bundle;
        }
        // Root 8 (spec-0032). A shop is the smallest object that can host one: an
        // anchor the fixture prefab already provides, a title, and one offer whose
        // effects ARE the probe bundle.
        EffectRootKind::ShopOffer => {
            let mut shop: delvewright_dsl::Shop = serde_json::from_str(
                r#"{ "id": "shop/probe", "anchor": "anchor/shop", "title": "Wares",
                     "offers": [{ "label": "Buy", "effects": [] }] }"#,
            )
            .expect("probe shop parses");
            shop.offers[0].effects = bundle;
            c.quests.content.shops.push(shop);
        }
    }
    c
}

fn prefabs() -> PrefabRegistry {
    PrefabRegistry::load_dir(&prefabs_with_trap()).unwrap()
}

fn assert_validates(c: &Campaign, k: EffectRootKind) {
    let d = common::fenced_diagnostics(
        c,
        &FullItemRegistry::v1_21_11(),
        &prefabs(),
        &FullEntityRegistry::v1_21_11(),
    );
    assert!(
        d.is_empty(),
        "the {} fixture must validate cleanly, or a later assertion would be \
         measuring a broken campaign: {d:#?}",
        k.label()
    );
}

fn build(loaded: &LoadedCampaign, c: &Campaign) -> Result<BuildOutput, BuildFailure> {
    let pf = prefabs();
    let plan = Plan::build(c, &pf).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(prefabs_with_trap().join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &pf,
        None,
        &BTreeMap::new(),
    )
}

fn all_text(out: &BuildOutput) -> String {
    out.iter()
        .filter(|(p, _)| p.ends_with(".mcfunction"))
        .filter_map(|(_, b)| std::str::from_utf8(b).ok())
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// the matrix
// ---------------------------------------------------------------------------

/// **Every root is visited by every campaign-wide walker.**
///
/// One row per `EffectRootKind`, six walkers per row. A walker that stops
/// inheriting the single enumeration fails here by name, on the root it lost.
#[test]
fn every_root_is_visited_by_every_walker() {
    let loaded = load();
    let mut rows = 0usize;
    for k in EffectRootKind::ALL {
        rows += 1;
        let c = probe_at(&loaded, k, &probe_bundle(k));
        assert_validates(&c, k);

        // W1 — the enumeration itself: the campaign binds a site at this root.
        let binding = delvewright_dsl::for_each_effect_root(&c, &mut |_, _| {});
        assert_eq!(binding.roots_enumerated, EffectRootKind::COUNT);
        let sites = binding.sites.iter().find(|(kk, _)| *kk == k).unwrap().1;
        assert!(
            sites >= 1,
            "the {} fixture must bind the root it probes ({})",
            k.label(),
            binding.summary()
        );

        // W2 — `for_each_campaign_effect`: the probe is yielded, attributed to a
        // site of this root's shape. The attribution matters: the historical bug
        // was a walk that could not REPRESENT a root, not one that skipped a list.
        let mut seen_site: Option<EffectSite> = None;
        delvewright_dsl::for_each_campaign_effect(&c, &mut |_path, site, eff| {
            if let QuestEffect::Narrate { text, .. } = eff
                && *text == probe_text(k)
            {
                seen_site = Some(site.clone());
            }
        });
        let site = seen_site.unwrap_or_else(|| {
            panic!(
                "`for_each_campaign_effect` never yielded the probe narrate at the {} root",
                k.label()
            )
        });
        assert_eq!(
            site_kind(&site),
            k,
            "the probe at the {} root was attributed to the wrong site",
            k.label()
        );

        // W3 — the l10n inventory (and with it `for_each_effect_root_mut`, from
        // which it is generated): the probe line is translatable.
        let inv = delvewright_dsl::l10n_inventory(&c);
        assert!(
            inv.values().any(|v| *v == probe_text(k)),
            "the {} root's narrate is not in the l10n inventory — it would ship \
             English-only in every translated build, silently",
            k.label()
        );

        // W4 — the flow model's READER half: the probe's gate is a flag read.
        assert!(
            gate_flags(&c).contains(&probe_flag(k)),
            "`flow::gate_flags` cannot see a `requires_flags` at the {} root, so a \
             branch choice only such a gate reads would never split the worlds",
            k.label()
        );

        let out = build(&loaded, &c).expect("the probed fixture builds");

        // W5 — `emit::declared_flags`: the flag's scoreboard objective exists. A
        // flag whose objective is never declared is a gate that never opens.
        let setup = std::str::from_utf8(
            out.get(&format!("datapack/data/{NS}/function/setup.mcfunction"))
                .expect("setup exists"),
        )
        .unwrap();
        assert!(
            setup.contains(&format!(
                "scoreboard objectives add {} dummy",
                probe_objective(k)
            )),
            "the {} root's `set-flag` declares no objective: {setup}",
            k.label()
        );

        // W6 — emission itself lowers the bundle.
        assert!(
            all_text(&out).contains(&probe_text(k)),
            "no emitted function carries the {} root's narrate — the bundle was \
             walked by the proofs and dropped by the emitter",
            k.label()
        );
    }
    assert_eq!(
        rows,
        EffectRootKind::COUNT,
        "the matrix examined {rows} roots of {} — a run that binds fewer roots \
         than exist is vacuous, not a pass",
        EffectRootKind::COUNT
    );
}

/// **`DW0360` is total over the roots too** — the seventh walker, asked
/// separately because its observable is a rejected build.
///
/// `emit::check_effect_anchors` is the backstop that turns a typo'd anchor into a
/// diagnostic instead of an effect that silently emits nothing. It hand-listed
/// three of five roots once, and the delve the owner played had a trap
/// that sprang and did nothing.
#[test]
fn a_bogus_anchor_is_rejected_at_every_root() {
    let loaded = load();
    for k in EffectRootKind::ALL {
        let c = probe_at(&loaded, k, &bogus_anchor_bundle(k));
        let err = build(&loaded, &c).err().unwrap_or_else(|| {
            panic!(
                "an unresolvable anchor at the {} root built CLEAN — the effect \
                 would ship emitting nothing at all",
                k.label()
            )
        });
        let BuildFailure::Diagnostic { code, message } = err else {
            panic!("{} root: expected a diagnostic, got {err:?}", k.label());
        };
        assert_eq!(code, "DW0360", "{} root: {message}", k.label());
        assert!(
            message.contains(&bogus_anchor(k)),
            "{} root: the diagnostic must name the anchor it could not resolve: \
             {message}",
            k.label()
        );
    }
}

/// Which root an [`EffectSite`] belongs to. Exhaustive on purpose: a new root
/// that `for_each_campaign_effect` cannot express fails to compile here, which is
/// exactly how root 5's absence was found.
fn site_kind(site: &EffectSite) -> EffectRootKind {
    match site {
        EffectSite::Objective { .. } => EffectRootKind::ObjectiveComplete,
        EffectSite::QuestComplete { .. } => EffectRootKind::QuestComplete,
        EffectSite::Trigger { .. } => EffectRootKind::Trigger,
        EffectSite::Trap { .. } => EffectRootKind::TrapPayload,
        EffectSite::DialogueRespawn { .. } => EffectRootKind::DialogueRespawn,
        EffectSite::ShortcutUnlock { .. } => EffectRootKind::ShortcutUnlock,
        EffectSite::OnDeath => EffectRootKind::OnDeath,
        EffectSite::ShopOffer { .. } => EffectRootKind::ShopOffer,
    }
}

/// The unused-import guard: `BTreeSet` is the return type `gate_flags` is asserted
/// against above, named so a signature change is a compile error here rather than
/// a silently different assertion.
#[allow(dead_code)]
fn gate_flag_type(c: &Campaign) -> BTreeSet<String> {
    gate_flags(c)
}
