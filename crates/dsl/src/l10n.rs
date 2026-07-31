//! Native i18n: author-declared languages, l10n sidecar documents, the
//! authoritative key inventory, and the localization pass (spec-0001 i18n
//! addendum; owner-approved 2026-07-31).
//!
//! **English is canonical.** Stage docs stay pure English; the strings the
//! compiler emits to players come from the stage docs. A campaign that declares
//! `world.languages = ["<code>", …]` must ship one `l10n/<code>.json` sidecar per
//! declared language, each a flat map of **stable key → translated string**.
//!
//! The **key inventory** ([`inventory`]) is derived deterministically from the
//! stage docs; it is the single source of truth for both coverage validation
//! ([`validate_l10n`]) and the build-time swap ([`localize`]). Both walk the exact
//! same traversal ([`each_string`]), so a key can never be checked but not applied
//! (or vice-versa).
//!
//! ## Key scheme (stable, path-derived, collision-free)
//!
//! Keys are dotted paths built from the local part of each DSL id (the segment
//! after `<prefix>/`, kebab preserved). Ids are unique within their namespace, so
//! every key is unique.
//!
//! | Key | Source string |
//! |-----|---------------|
//! | `world.title` | stage-1 `content.title` |
//! | `area.<area>.name` | each stage-1 area `name` |
//! | `class.<class>.name` / `.blurb` | each stage-3 class |
//! | `class.<class>.kit.<i>.name` | a kit item's display `name` (only if set) |
//! | `npc.<npc>.name` | each stage-2 NPC `name` |
//! | `quest.<quest>.goal` | each stage-4 planned-quest `goal` |
//! | `obj.<quest>.<obj>.title` / `.hint` | a stage-5 objective's `title`/`hint` (only if set) |
//! | `dlg.<npc>.<node>.text` | each stage-6 dialogue node `text` |
//! | `dlg.<npc>.<node>.opt.<i>.label` | each dialogue option `label` |
//! | `wave.<wave>.mob.<i>.name` | a wave mob's custom `name` (only if set) |
//!
//! Player-visible strings only. Deliberately **excluded** (authoring context the
//! player never sees, so translating them is pointless and out of scope): world
//! `theme`/`premise`, NPC `persona` fields, persona `relationships`.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::diagnostic::{Diagnostic, codes};
use crate::envelope::{Campaign, is_supported_version};
use crate::ids::CampaignId;
use crate::stages::QuestEffect;

/// Walk the player-visible strings of a single quest effect (DSL v0.4): a
/// `narrate` line and a named `give-item`'s display name. `keybase` is the
/// effect's stable position-derived key prefix.
fn effect_strings(eff: &mut QuestEffect, keybase: &str, f: &mut dyn FnMut(&str, &mut String)) {
    match eff {
        QuestEffect::Narrate { text, .. } => f(&format!("{keybase}.narrate"), text),
        QuestEffect::GiveItem { name: Some(n), .. } => f(&format!("{keybase}.give"), n),
        _ => {}
    }
}

/// The implicit, always-canonical language. Never appears in `world.languages`
/// and never has a sidecar; `delvec build --lang en` emits the pure-English delve.
pub const CANONICAL_LANG: &str = "en";

/// The kind marker on an l10n sidecar envelope (`"l10n"`), analogous to the stage
/// marker on a stage document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum L10nKind {
    /// A localization sidecar (`l10n/<code>.json`).
    L10n,
}

/// An l10n sidecar document: `{ dsl_version, campaign_id, kind: "l10n", lang,
/// content }`, mirroring the stage-doc envelope style. `content` is a flat map of
/// [inventory](crate::l10n::inventory) key → translated string.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct L10nDoc {
    /// DSL version string (same versioning as the stage docs).
    pub dsl_version: String,
    /// Owning campaign id (must match the stage docs).
    pub campaign_id: CampaignId,
    /// Document kind marker (`l10n`).
    pub kind: L10nKind,
    /// The BCP-47-style language code this sidecar translates into (must match the
    /// `l10n/<code>.json` filename and appear in `world.languages`).
    pub lang: String,
    /// Flat map of inventory key → translated string.
    pub content: BTreeMap<String, String>,
}

/// The local part of a type-prefixed id: the segment after the first `/` (kebab
/// preserved). `npc/keeper` → `keeper`. Ids without a `/` pass through unchanged.
fn local(id: &str) -> &str {
    id.split_once('/').map(|(_, r)| r).unwrap_or(id)
}

/// Walk every player-visible string in `c` in a fixed, deterministic order,
/// invoking `f(key, &mut value)` for each. The single traversal shared by
/// [`inventory`] and [`localize`] — they cannot drift.
pub fn each_string(c: &mut Campaign, f: &mut dyn FnMut(&str, &mut String)) {
    // Stage 1 — world title + area names.
    f("world.title", &mut c.world.content.title);
    for area in &mut c.world.content.areas {
        let key = format!("area.{}.name", local(area.id.as_str()));
        f(&key, &mut area.name);
    }
    // Stage 3 — class names/blurbs + optional kit item display names.
    for class in &mut c.classes.content.classes {
        let cl = local(class.id.as_str()).to_string();
        f(&format!("class.{cl}.name"), &mut class.name);
        f(&format!("class.{cl}.blurb"), &mut class.blurb);
        for (i, item) in class.kit.iter_mut().enumerate() {
            if let Some(name) = item.name.as_mut() {
                f(&format!("class.{cl}.kit.{i}.name"), name);
            }
        }
    }
    // Stage 2 — NPC names.
    for npc in &mut c.npcs.content.npcs {
        let key = format!("npc.{}.name", local(npc.id.as_str()));
        f(&key, &mut npc.name);
    }
    // Stage 4 — quest goals.
    for q in &mut c.quest_plan.content.quests {
        let key = format!("quest.{}.goal", local(q.id.as_str()));
        f(&key, &mut q.goal);
    }
    // Stage 5 — objective titles/hints (when set).
    for q in &mut c.quests.content.quests {
        let ql = local(q.id.as_str()).to_string();
        for o in &mut q.objectives {
            let ol = local(o.id().as_str()).to_string();
            if let Some(title) = o.title_mut().as_mut() {
                f(&format!("obj.{ql}.{ol}.title"), title);
            }
            if let Some(hint) = o.hint_mut().as_mut() {
                f(&format!("obj.{ql}.{ol}.hint"), hint);
            }
        }
        // Stage 5 — v0.4 effect strings: `narrate` text + named `give-item`
        // (deterministic: `on_objective_complete` is a BTreeMap). Empty for
        // v0.2/v0.3 campaigns → inventory unchanged.
        for (oid, effs) in &mut q.on_objective_complete {
            let ol = local(oid.as_str()).to_string();
            for (i, eff) in effs.iter_mut().enumerate() {
                effect_strings(eff, &format!("fx.{ql}.oc.{ol}.{i}"), f);
            }
        }
        for (i, eff) in q.on_complete.iter_mut().enumerate() {
            effect_strings(eff, &format!("fx.{ql}.done.{i}"), f);
        }
    }
    // Stage 5 — v0.4 environment-trigger effect strings.
    for t in &mut c.quests.content.triggers {
        let tl = local(t.id.as_str()).to_string();
        for (i, eff) in t.effects.iter_mut().enumerate() {
            effect_strings(eff, &format!("fx.trig.{tl}.{i}"), f);
        }
    }
    // Stage 6 — dialogue node text + option labels.
    for tree in &mut c.dialogue.content.dialogues {
        let np = local(tree.npc.as_str()).to_string();
        for node in &mut tree.nodes {
            let nd = local(node.id.as_str()).to_string();
            f(&format!("dlg.{np}.{nd}.text"), &mut node.text);
            for (i, opt) in node.options.iter_mut().enumerate() {
                f(&format!("dlg.{np}.{nd}.opt.{i}.label"), &mut opt.label);
            }
        }
    }
    // Stage 5 — wave mob custom names (when set).
    for w in &mut c.quests.content.waves {
        let wl = local(w.id.as_str()).to_string();
        for (i, mob) in w.mobs.iter_mut().enumerate() {
            if let Some(name) = mob.name.as_mut() {
                f(&format!("wave.{wl}.mob.{i}.name"), name);
            }
        }
    }
}

/// The authoritative key → canonical-English inventory derived from the stage
/// docs. Deterministic (keys are unique and the traversal order is fixed).
pub fn inventory(c: &Campaign) -> BTreeMap<String, String> {
    let mut c2 = c.clone();
    let mut out = BTreeMap::new();
    each_string(&mut c2, &mut |key, value| {
        out.insert(key.to_string(), value.clone());
    });
    out
}

/// Replace every inventoried string in `c` with its translation from
/// `translations` (an l10n sidecar's `content`). Keys absent from the map are left
/// as canonical English — but a fully-validated sidecar ([`validate_l10n`]) covers
/// the inventory exactly, so a build only reaches here with complete coverage.
pub fn localize(c: &mut Campaign, translations: &BTreeMap<String, String>) {
    each_string(c, &mut |key, value| {
        if let Some(t) = translations.get(key) {
            *value = t.clone();
        }
    });
}

/// Coverage + envelope validation for every declared language's l10n sidecar
/// (`DW0180` / `DW0181`). Language-independent: it runs on every `validate` /
/// `analyze` / `build`, regardless of `--lang`. Returns no diagnostics for a
/// campaign that declares no languages. `sidecars` is keyed by language code
/// (the `l10n/<code>.json` filename stem).
pub fn validate_l10n(c: &Campaign, sidecars: &BTreeMap<String, L10nDoc>) -> Vec<Diagnostic> {
    let mut d = Vec::new();
    let declared = &c.world.content.languages;
    if declared.is_empty() {
        return d;
    }
    let inv = inventory(c);
    let inv_keys: BTreeSet<&str> = inv.keys().map(String::as_str).collect();
    let campaign_id = c.world.campaign_id.as_str();

    for lang in declared {
        if lang == CANONICAL_LANG {
            d.push(Diagnostic::error(
                codes::L10N_MISSING,
                "l10n",
                format!("/world/languages/{lang}"),
                format!(
                    "`{lang}` is the canonical language and must not be declared in \
                     `world.languages` (English is implicit)"
                ),
            ));
            continue;
        }
        let Some(doc) = sidecars.get(lang) else {
            d.push(Diagnostic::error(
                codes::L10N_MISSING,
                "l10n",
                format!("l10n/{lang}.json"),
                format!("declared language `{lang}` has no `l10n/{lang}.json` sidecar"),
            ));
            continue;
        };
        // Envelope consistency (folded into DW0180 — the sidecar does not correctly
        // cover the declared language).
        if doc.campaign_id.as_str() != campaign_id {
            d.push(Diagnostic::error(
                codes::L10N_MISSING,
                "l10n",
                format!("l10n/{lang}.json"),
                format!(
                    "sidecar campaign_id `{}` differs from `{campaign_id}`",
                    doc.campaign_id
                ),
            ));
        }
        if doc.lang != *lang {
            d.push(Diagnostic::error(
                codes::L10N_MISSING,
                "l10n",
                format!("l10n/{lang}.json"),
                format!(
                    "sidecar `lang` field `{}` differs from filename code `{lang}`",
                    doc.lang
                ),
            ));
        }
        if !is_supported_version(&doc.dsl_version) {
            d.push(Diagnostic::error(
                codes::L10N_MISSING,
                "l10n",
                format!("l10n/{lang}.json"),
                format!("sidecar has unsupported dsl_version `{}`", doc.dsl_version),
            ));
        }
        // Coverage: exactly the inventory — missing (DW0180) and orphan (DW0181).
        let side_keys: BTreeSet<&str> = doc.content.keys().map(String::as_str).collect();
        for missing in inv_keys.difference(&side_keys) {
            d.push(Diagnostic::error(
                codes::L10N_MISSING,
                "l10n",
                format!("l10n/{lang}.json"),
                format!("missing translation for key `{missing}`"),
            ));
        }
        for orphan in side_keys.difference(&inv_keys) {
            d.push(Diagnostic::error(
                codes::L10N_ORPHAN,
                "l10n",
                format!("l10n/{lang}.json#/content/{orphan}"),
                format!("orphan key `{orphan}` is not in the string inventory"),
            ));
        }
    }
    d
}
