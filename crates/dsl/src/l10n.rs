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
//! | `obj.<quest>.<obj>.missing_item_hint` | a stage-5 `interact`'s `missing_item_hint` (v0.7, only if set) |
//! | `dlg.<npc>.<node>.text` | each stage-6 dialogue node `text` |
//! | `dlg.<npc>.<node>.opt.<i>.label` | each dialogue option `label` |
//! | `wave.<wave>.mob.<i>.name` | a wave mob's custom `name` (only if set) |
//! | `fx.…​.narrate` / `fx.…​.give` | a `narrate` line / named `give-item` in an effect list |
//!
//! ## Nested effects (DSL v0.6)
//!
//! Effect strings nested inside a `sequence` step or a lifecycle bundle
//! (`on_respawn`/`on_caught`/`on_arrive`) are player-visible too, so they are
//! inventoried under **position-derived** child keys: the parent effect's `fx.…`
//! key, then a stable segment ([`crate::stages::QuestEffect::nested_effect_lists_keyed_mut`])
//! — `seq.<step>` for a sequence step, `respawn`/`caught`/`arrive` for the bundles —
//! then the effect's index in that list, then the leaf (`.narrate`/`.give`).
//! Example: a narrate in sequence step 1, effect 0 of `on_objective_complete`
//! effect 0 → `fx.<quest>.oc.<obj>.0.seq.1.0.narrate`. Nesting is arbitrary-depth
//! (a `move-actor.on_arrive` inside a `sequence` step nests both segments). Keys are
//! purely position-derived → deterministic and stable across builds (ADR-0006).
//!
//! Player-visible strings only. Deliberately **excluded** (authoring context the
//! player never sees, so translating them is pointless and out of scope): world
//! `theme`/`premise`, NPC `persona` fields, persona `relationships`.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::diagnostic::{Diagnostic, codes};
use crate::envelope::{Campaign, SUPPORTED_DSL_VERSIONS, is_supported_version};
use crate::ids::CampaignId;
use crate::stages::{NarrateStyle, QuestEffect};

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

/// Walk the player-visible strings of `eff` **and every effect nested inside it**
/// (DSL v0.6): a `narrate`/`give-item` inside a `sequence` step or an
/// `on_respawn`/`on_caught`/`on_arrive` bundle is player-visible and must enter the
/// inventory (and be localized on the emission path), else it ships English-only in
/// a translated build. Child keys extend `keybase` with the effect's stable key
/// segment ([`QuestEffect::nested_effect_lists_keyed_mut`]) and the effect's index
/// within that list, e.g. `<keybase>.seq.<step>.<j>.narrate` for a narrate in
/// sequence step `<step>`, effect `<j>`. Deterministic and stable across builds.
fn effect_strings_deep(eff: &mut QuestEffect, keybase: &str, f: &mut dyn FnMut(&str, &mut String)) {
    effect_strings(eff, keybase, f);
    for (seg, list) in eff.nested_effect_lists_keyed_mut() {
        for (j, inner) in list.iter_mut().enumerate() {
            effect_strings_deep(inner, &format!("{keybase}.{seg}.{j}"), f);
        }
    }
}

/// The implicit, always-canonical language. Never appears in `world.languages`
/// and never has a sidecar; `delvec build --lang en` emits the pure-English delve.
pub const CANONICAL_LANG: &str = "en";

/// The reserved sigil opening the compiler's machine-readable **completion-marker**
/// channel — `[dw:complete <campaign_id> <token>]`, the only evidence the
/// validation bot accepts that an objective (or the campaign) actually completed.
/// The channel rides chat, so any authored or translated player-visible string
/// carrying this sigil could forge a completion and make a critical-path step pass
/// hollow. [`validate_marker_channel`] (`DW0182`) reserves it, making the collision
/// structurally impossible instead of merely implausible.
pub const MARKER_SIGIL: &str = "[dw:complete";

/// Reserve the machine completion-marker channel (`DW0182`): no player-visible
/// string — authored English (the whole [`inventory`]) or any declared language's
/// sidecar rendition — may contain [`MARKER_SIGIL`]. Language-independent; runs on
/// every `validate` / `analyze` / `build`. Checks translations too: a translator
/// (LLM or human) copying the sigil through is exactly the forgery this closes.
pub fn validate_marker_channel(
    c: &Campaign,
    sidecars: &BTreeMap<String, L10nDoc>,
) -> Vec<Diagnostic> {
    let mut d = Vec::new();
    let mut flag = |where_: String, key: &str, text: &str| {
        if text.contains(MARKER_SIGIL) {
            d.push(Diagnostic::error(
                codes::MARKER_RESERVED,
                "l10n",
                where_,
                format!(
                    "player-visible string `{key}` contains the reserved completion-marker \
                     sigil `{MARKER_SIGIL}` — that chat sequence is the validation bot's \
                     completion oracle, and authored text carrying it could forge a passing \
                     critical-path step. Reword the line to drop `{MARKER_SIGIL}`"
                ),
            ));
        }
    };
    for (key, text) in inventory(c) {
        flag(format!("#/{key}"), &key, &text);
    }
    for (lang, doc) in sidecars {
        for (key, text) in &doc.content {
            flag(format!("l10n/{lang}.json#/content/{key}"), key, text);
        }
    }
    d
}

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

/// The local part of a type-prefixed DSL id, exactly as the key scheme derives it
/// (`npc/keeper` → `keeper`). Public so a consumer that pairs inventory keys back
/// with their source objects (`delvec l10n-inventory`, matching `dlg.<npc>.…` keys
/// to the NPC that speaks them) derives the same segment the keys are built from.
pub fn local_id(id: &str) -> &str {
    local(id)
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
    // Stage 1 — boundary return message (v0.6, only when authored). The compiler's
    // default English is baked at emit time, so it is not inventoried; an authored
    // message is translated like every other player-facing string.
    if let Some(b) = c.world.content.boundary.as_mut()
        && let Some(msg) = b.message.as_mut()
    {
        f("world.boundary.message", msg);
    }
    // Stage 1 — campaign outro (v0.6, only when authored): the closing line on the
    // completion advancement. Unauthored, the emitter falls back to the finale
    // quest's `goal`, which is inventoried in its own right — so the last sentence
    // of a delve is campaign-derived and translated either way.
    if let Some(outro) = c.world.content.outro.as_mut() {
        f("world.outro", outro);
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
            // Stage 5 — v0.7 `interact.missing_item_hint`: narrated in chat to the
            // player who clicks without the required item in hand, so it is as
            // player-visible as `hint` and translates like it. Absent on every
            // pre-0.7 objective → inventory unchanged.
            if let crate::stages::Objective::Interact {
                missing_item_hint: Some(m),
                ..
            } = o
            {
                f(&format!("obj.{ql}.{ol}.missing_item_hint"), m);
            }
        }
        // Stage 5 — v0.4 effect strings: `narrate` text + named `give-item`
        // (deterministic: `on_objective_complete` is a BTreeMap). Empty for
        // v0.2/v0.3 campaigns → inventory unchanged.
        for (oid, effs) in &mut q.on_objective_complete {
            let ol = local(oid.as_str()).to_string();
            for (i, eff) in effs.iter_mut().enumerate() {
                effect_strings_deep(eff, &format!("fx.{ql}.oc.{ol}.{i}"), f);
            }
        }
        for (i, eff) in q.on_complete.iter_mut().enumerate() {
            effect_strings_deep(eff, &format!("fx.{ql}.done.{i}"), f);
        }
        // Stage 5 — v0.7 cast-ledger bark lines (spec-0020). Barks are spoken
        // in-game exactly like narrate text, so they translate like it too.
        // `doing` is deliberately NOT inventoried: it is authoring context for
        // the dialogue stage, never shown to a player. (`cast` is a BTreeMap and
        // the placement list is ordered, so the traversal stays deterministic.)
        for (npc, entry) in &mut q.cast {
            let np = local(npc.as_str()).to_string();
            for (b, p) in entry.placements_mut().into_iter().enumerate() {
                let Some(crate::stages::CastDialogue::Barks(pool)) = p.dialogue.as_mut() else {
                    continue;
                };
                for (i, line) in pool.barks.iter_mut().enumerate() {
                    f(&format!("cast.{ql}.{np}.{b}.bark.{i}"), line);
                }
            }
        }
    }
    // Stage 5 — v0.4 environment-trigger effect strings.
    for t in &mut c.quests.content.triggers {
        let tl = local(t.id.as_str()).to_string();
        for (i, eff) in t.effects.iter_mut().enumerate() {
            effect_strings_deep(eff, &format!("fx.trig.{tl}.{i}"), f);
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
    // Stage 5 — loot item custom names (spec-0021), keyed like a class kit
    // item's name so a named prop in a chest translates like any other.
    for l in &mut c.quests.content.loot {
        let ll = local(l.id.as_str()).to_string();
        for (i, item) in l.items.iter_mut().enumerate() {
            if let Some(name) = item.name.as_mut() {
                f(&format!("loot.{ll}.item.{i}.name"), name);
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

/// The NPC an inventory key belongs to (its **local** id), when the key scheme
/// encodes one: `dlg.<npc>.…` (that NPC's dialogue tree — `.text` is the NPC's own
/// line, `.opt.<i>.label` the player's reply *within* it), `npc.<npc>.name`, and
/// `cast.<quest>.<npc>.…` (a v0.7 bark line — the NPC's own murmured speech, so a
/// translator gets the same persona context a dialogue line gets).
/// Returns `None` for every other key kind.
///
/// Lives beside [`each_string`] — the traversal that *defines* the key scheme — so
/// the two cannot drift silently (a CLI test asserts every speaker derived from a
/// real campaign's inventory resolves to a declared NPC). Consumed by
/// `delvec l10n-inventory`, which hands a translator the speaking character's
/// persona (`speech_style` above all) alongside the English line.
pub fn key_speaker(key: &str) -> Option<&str> {
    let (kind, rest) = key.split_once('.')?;
    match kind {
        "dlg" | "npc" => rest.split('.').next(),
        // `cast.<quest>.<npc>.<branch>.bark.<i>` — the npc is the second segment.
        "cast" => rest.split('.').nth(1),
        _ => None,
    }
}

/// One `narrate` `art` occurrence (DSL v0.6, spec-0014): its stage-doc path (for
/// diagnostics), its l10n inventory key, and the canonical English text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtNarrate {
    /// JSON-pointer-ish path within the `quests` stage doc.
    pub path: String,
    /// The l10n inventory key (`fx.…​.narrate`) — always present, since every
    /// `narrate` lives in an inventoried effect position.
    pub key: String,
    /// The canonical English text.
    pub text: String,
}

/// Every `narrate` effect using the v0.6 `art` style, in a fixed deterministic
/// order. Its l10n `key` is derived by the **same** traversal/keying as
/// [`inventory`]/[`each_string`], so the compiler's art-font glyph check
/// (`DW0328`) can look each art string up in every declared-language sidecar.
/// Every art narrate lives in a quest `on_objective_complete`/`on_complete` or an
/// environment trigger — all inventoried — so each `key` is guaranteed present in
/// a fully-covered sidecar.
pub fn art_narrates(c: &Campaign) -> Vec<ArtNarrate> {
    let mut out = Vec::new();
    each_effect_ref(c, &mut |path, keybase, eff| {
        if let Some(text) = eff.narrate_art_text() {
            out.push(ArtNarrate {
                path: format!("{path}/text"),
                key: format!("{keybase}.narrate"),
                text: text.to_string(),
            });
        }
    });
    out
}

/// One on-screen `narrate` occurrence — `title`, `subtitle` or `art` — its stage-doc
/// path, its l10n inventory key, its style, and the canonical English text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScreenNarrate {
    /// JSON-pointer-ish path within the `quests` stage doc.
    pub path: String,
    /// The l10n inventory key (`fx.…​.narrate`).
    pub key: String,
    /// Which on-screen channel vanilla draws it in — this selects the width budget.
    pub style: NarrateStyle,
    /// The canonical English text.
    pub text: String,
}

/// Every `narrate` effect vanilla draws **on screen** (`title` / `subtitle` / `art`),
/// in a fixed deterministic order. Like [`art_narrates`], each `key` is derived by the
/// **same** traversal/keying as [`inventory`]/[`each_string`], so the compiler's
/// text-fit check (`DW0330`) can look every string up in each declared-language
/// sidecar and report it under the offending locale and key. `chat` narrates are
/// excluded: chat wraps and scrolls, so it has no width budget.
pub fn on_screen_narrates(c: &Campaign) -> Vec<ScreenNarrate> {
    let mut out = Vec::new();
    each_effect_ref(c, &mut |path, keybase, eff| {
        if let Some((style, text)) = eff.narrate_on_screen() {
            out.push(ScreenNarrate {
                path: format!("{path}/text"),
                key: format!("{keybase}.narrate"),
                style,
                text: text.to_string(),
            });
        }
    });
    out
}

/// Visit every quest/trigger effect — **top-level and every transitively-nested**
/// one (a `sequence` step, an `on_respawn`/`on_caught`/`on_arrive` bundle) — in the
/// fixed inventory order, invoking `f(path, keybase, effect)`. `path` is the
/// effect's JSON-pointer within the `quests` stage doc (for diagnostics); `keybase`
/// is its l10n key prefix, derived by the **same** position-keying as
/// [`each_string`]/[`effect_strings_deep`] (so an art narrate's key matches its
/// inventory key, and a nested `play-sound`/`give-item` ref is reported at a precise
/// path). Shared by [`art_narrates`], [`sound_refs`] and [`play_sound_actor_refs`]
/// so the consumer checks (`DW0326`/`DW0328`/`DW0335`) descend nested effects
/// exactly as emission and the l10n inventory already do (task: nested-effect
/// consumer recursion). Top-level positions keep their prior path/key, so a
/// nesting-free campaign is unaffected; nested refs are additive.
fn each_effect_ref<'a>(c: &'a Campaign, f: &mut dyn FnMut(&str, &str, &'a QuestEffect)) {
    for (qi, q) in c.quests.content.quests.iter().enumerate() {
        let ql = local(q.id.as_str());
        for (oid, effs) in &q.on_objective_complete {
            let ol = local(oid.as_str());
            for (i, eff) in effs.iter().enumerate() {
                effect_deep(
                    eff,
                    &format!(
                        "/content/quests/{qi}/on_objective_complete/{}/{i}",
                        oid.as_str()
                    ),
                    &format!("fx.{ql}.oc.{ol}.{i}"),
                    f,
                );
            }
        }
        for (i, eff) in q.on_complete.iter().enumerate() {
            effect_deep(
                eff,
                &format!("/content/quests/{qi}/on_complete/{i}"),
                &format!("fx.{ql}.done.{i}"),
                f,
            );
        }
    }
    for (ti, t) in c.quests.content.triggers.iter().enumerate() {
        let tl = local(t.id.as_str());
        for (i, eff) in t.effects.iter().enumerate() {
            effect_deep(
                eff,
                &format!("/content/triggers/{ti}/effects/{i}"),
                &format!("fx.trig.{tl}.{i}"),
                f,
            );
        }
    }
}

/// Visit `eff` and every transitively-nested effect (depth-first, pre-order),
/// threading the JSON-pointer `path` and l10n `keybase` through each nested list via
/// [`QuestEffect::nested_effect_lists_labeled`] (path segment + key segment + the
/// per-effect index). The key segments match [`effect_strings_deep`] exactly.
fn effect_deep<'a>(
    eff: &'a QuestEffect,
    path: &str,
    keybase: &str,
    f: &mut dyn FnMut(&str, &str, &'a QuestEffect),
) {
    f(path, keybase, eff);
    for (pseg, kseg, list) in eff.nested_effect_lists_labeled() {
        for (j, inner) in list.iter().enumerate() {
            effect_deep(
                inner,
                &format!("{path}/{pseg}/{j}"),
                &format!("{keybase}.{kseg}.{j}"),
                f,
            );
        }
    }
}

/// One vanilla sound-event reference (DSL v0.6/v0.4): its stage-doc path and the
/// referenced id, for registry validation (`DW0326`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoundRef {
    /// JSON-pointer-ish path within the `quests` stage doc.
    pub path: String,
    /// The referenced sound-event id (`minecraft:` prefix optional).
    pub sound: String,
}

/// Every vanilla sound-event id referenced by a quest/trigger effect — a
/// `play-sound`'s `sound` (v0.6) and a `narrate`'s optional `sound` (v0.4) — in a
/// fixed deterministic order, for `DW0326` validation.
pub fn sound_refs(c: &Campaign) -> Vec<SoundRef> {
    let mut out = Vec::new();
    each_effect_ref(c, &mut |path, _key, eff| {
        for (sub, sound) in eff.sound_refs() {
            out.push(SoundRef {
                path: format!("{path}/{sub}"),
                sound: sound.to_string(),
            });
        }
    });
    out
}

/// Every `play-sound` effect using the deferred `at: actor` target, in a fixed
/// order, as `(path, actor-id)` pairs. The actor variant is accepted by the
/// schema but rejected (`DW0335`) until the actors surface (spec-0014 `actors[]`)
/// lands; the compiler applies that check. `SoundRef::sound` carries the actor id.
pub fn play_sound_actor_refs(c: &Campaign) -> Vec<SoundRef> {
    let mut out = Vec::new();
    each_effect_ref(c, &mut |path, _key, eff| {
        if let Some(actor) = eff.play_sound_actor() {
            out.push(SoundRef {
                path: format!("{path}/at/actor"),
                sound: actor.to_string(),
            });
        }
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
                     `world.languages` — English is implicit; remove `{lang}` from the list"
                ),
            ));
            continue;
        }
        let Some(doc) = sidecars.get(lang) else {
            d.push(Diagnostic::error(
                codes::L10N_MISSING,
                "l10n",
                format!("l10n/{lang}.json"),
                format!(
                    "declared language `{lang}` has no `l10n/{lang}.json` sidecar — add the \
                     sidecar (a full key→translation map), or remove `{lang}` from \
                     `world.languages`"
                ),
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
                    "sidecar `campaign_id` `{}` differs from the campaign's `{campaign_id}` — set \
                     the sidecar's `campaign_id` to `{campaign_id}`",
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
                    "sidecar `lang` field `{}` differs from filename code `{lang}` — set the \
                     sidecar's `lang` to `{lang}` (it must match the `l10n/{lang}.json` filename)",
                    doc.lang
                ),
            ));
        }
        if !is_supported_version(&doc.dsl_version) {
            d.push(Diagnostic::error(
                codes::L10N_MISSING,
                "l10n",
                format!("l10n/{lang}.json"),
                format!(
                    "sidecar has unsupported dsl_version `{}` — set it to a supported version \
                     matching the stage docs (one of {SUPPORTED_DSL_VERSIONS:?})",
                    doc.dsl_version
                ),
            ));
        }
        // Coverage: exactly the inventory — missing (DW0180) and orphan (DW0181).
        let side_keys: BTreeSet<&str> = doc.content.keys().map(String::as_str).collect();
        for missing in inv_keys.difference(&side_keys) {
            d.push(Diagnostic::error(
                codes::L10N_MISSING,
                "l10n",
                format!("l10n/{lang}.json"),
                format!(
                    "sidecar is missing a translation for inventory key `{missing}` — add \
                     `{missing}` to `l10n/{lang}.json` (coverage must be exact)"
                ),
            ));
        }
        for orphan in side_keys.difference(&inv_keys) {
            d.push(Diagnostic::error(
                codes::L10N_ORPHAN,
                "l10n",
                format!("l10n/{lang}.json#/content/{orphan}"),
                format!(
                    "orphan key `{orphan}` is not in the string inventory — remove it from \
                     `l10n/{lang}.json` (the sidecar must cover exactly the inventory, no extras)"
                ),
            ));
        }
    }
    d
}
