//! `delvec l10n-inventory` and `l10n-apply`: a campaign's translation
//! sidecars.

use std::path::Path;
use std::process::ExitCode;

use crate::EXIT_INTERNAL;
use crate::cli::campaign::load_or_refuse;
use crate::cli::report::print_diags;

/// One `l10n-inventory` row: an inventory key, its canonical English source, the
/// kind of text it is, the NPC whose voice it is (when the key scheme names one),
/// the situation it is said in, the translation the current sidecar already
/// carries (absent = untranslated), and whether that translation was made from
/// different English than the line reads now (`stale`, the `DW0187` condition).
#[derive(serde::Serialize)]
struct InventoryEntry<'a> {
    key: &'a str,
    en: &'a str,
    kind: Option<delvewright_dsl::TextKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    speaker: Option<&'a str>,
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    situation: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    existing: Option<&'a str>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    stale: bool,
}

/// The persona context a translator needs to keep a character's voice: who they
/// are and — above all — how they speak. Deliberately excludes `secret`,
/// `backstory` and `relationships`: plot context no line's *register* depends on.
#[derive(serde::Serialize)]
struct NpcContext<'a> {
    id: &'a str,
    name: &'a str,
    archetype: &'a str,
    speech_style: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    demeanor: Option<&'a str>,
    motivation: &'a str,
}

/// `delvec l10n-inventory <campaign-dir> [--lang <code>]` — the l10n key inventory
/// as JSON on stdout.
///
/// The inventory is [`delvewright_dsl::l10n::inventory`] itself, i.e. **exactly** the
/// key set `DW0180`/`DW0181` enforce, so a translator (human, in-agent, or an
/// external API via `tools/creator/i18n-translate.py`) can be handed the work list up front
/// instead of discovering it by writing an empty sidecar and reading the coverage
/// diagnostics back. Rows carry the canonical English, the kind of text (via
/// [`delvewright_dsl::key_kind`]), the speaking NPC (via
/// [`delvewright_dsl::key_speaker`]), the situation (via
/// [`delvewright_dsl::key_situations`]) and any translation the current
/// `l10n/<lang>.json` already has, marked `stale` when its recorded `source` is not
/// today's English — so re-running only fills the gaps (idempotence).
///
/// Deliberately runs **before** validation gating: an incomplete sidecar is the
/// normal state when you ask for the inventory. Only an unparseable campaign fails
/// (exit 1); no prefab library is needed.
pub(crate) fn run_l10n_inventory(campaign_dir: &Path, lang: &str, json: bool) -> ExitCode {
    let loaded = match load_or_refuse(campaign_dir, json) {
        Ok(l) => l,
        Err(exit) => return ExitCode::from(exit),
    };
    let campaign = match delvec::compiler::load::parse_loaded(&loaded) {
        Ok(c) => c,
        Err(diags) => {
            print_diags(&diags, json);
            return ExitCode::from(1);
        }
    };
    // A malformed sidecar reads as absent — every key is then reported untranslated,
    // which is the honest work list (and what `validate` says about it too).
    let sidecar = loaded
        .l10n
        .get(lang)
        .and_then(|b| serde_json::from_slice::<delvewright_dsl::L10nDoc>(b).ok());
    let existing = sidecar.as_ref().map(|d| &d.content);
    let recorded = sidecar.as_ref().map(|d| &d.source);

    let inv = delvewright_dsl::l10n::inventory(&campaign);
    let situations = delvewright_dsl::key_situations(&campaign);
    let entries: Vec<InventoryEntry<'_>> = inv
        .iter()
        .map(|(key, en)| {
            let existing = existing.and_then(|m| m.get(key)).map(String::as_str);
            InventoryEntry {
                key,
                en,
                kind: delvewright_dsl::key_kind(key),
                speaker: delvewright_dsl::key_speaker(key),
                situation: situations.get(key).map(Vec::as_slice).unwrap_or(&[]),
                existing,
                stale: existing.is_some()
                    && recorded
                        .and_then(|m| m.get(key))
                        .is_some_and(|was| was != en),
            }
        })
        .collect();
    let npcs: Vec<NpcContext<'_>> = campaign
        .npcs
        .content
        .npcs
        .iter()
        .map(|n| NpcContext {
            id: delvewright_dsl::local_id(n.id.as_str()),
            name: &n.name,
            archetype: &n.persona.archetype,
            speech_style: &n.persona.speech_style,
            demeanor: n.persona.demeanor.as_deref(),
            motivation: &n.persona.motivation,
        })
        .collect();

    let doc = serde_json::json!({
        "campaign_id": campaign.world.campaign_id.as_str(),
        // The sidecar envelope a fresh `l10n/<lang>.json` must carry, taken from
        // the stage docs (the existing sidecar's own version is preserved by the
        // writing tool, not restated here).
        "dsl_version": campaign.world.dsl_version,
        "lang": lang,
        "declared": campaign.world.content.languages.iter().any(|l| l == lang),
        "sidecar_present": sidecar.is_some(),
        "world_title": campaign.world.content.title,
        "npcs": npcs,
        "entries": entries,
    });
    match serde_json::to_string_pretty(&doc) {
        Ok(s) => {
            println!("{s}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("internal error: cannot serialize inventory: {e}");
            ExitCode::from(EXIT_INTERNAL)
        }
    }
}

/// `delvec l10n-apply <campaign-dir> --lang <code> --table <file>` — write the
/// sidecar from a table of canonical English → translation (spec-0071 §4).
///
/// ## The verb an agent that translates did not have
///
/// `l10n-inventory` hands out the work list and `tools/creator/i18n-translate.py`
/// fills a sidecar by calling an outside model. An agent that is itself the
/// translator had neither: to write `l10n/<code>.json` it had to address every
/// row by its inventory key, and the effect keys are **positional**
/// (`fx.<quest>.oc.<objective>.<i>`), so inserting one effect renumbers every
/// sibling and silently re-attaches translations to the wrong lines. A creator
/// agent measured doing this kept its own table keyed by English text and wrote
/// the merge by hand — a derivation, typed.
///
/// So the agent hands over what it actually knows — *this English becomes this
/// line* — and the tool does the keys. Both ends already exist: the inventory
/// carries each key's canonical English, and the sidecar records the `source`
/// each row was translated from.
///
/// ## What it writes
///
/// Exactly the inventory, so an orphan row is impossible by construction:
///
/// * a row whose English the table carries is written from the table, with its
///   `source` recorded — that is what makes a later English edit detectable
///   (`DW0187`) rather than audited;
/// * a row the sidecar already translates **from unchanged English** is kept,
///   provenance and all, so re-running translates only what moved;
/// * a row the sidecar translates with no `source` at all is kept too, and
///   counted separately: nothing can say whether it still matches its English,
///   and throwing away a translation over that would be worse than saying so.
///
/// ## The one thing the table form cannot say
///
/// Two inventory rows can hold the same English and want two different
/// translations. A table keyed by English has no way to distinguish them, so the
/// run **lists** every such English with its keys and the sidecar stays directly
/// editable for them. Named, not hidden: the alternative is a tool that silently
/// gives one answer to two questions.
pub(crate) fn run_l10n_apply(
    campaign_dir: &Path,
    lang: &str,
    table_path: &Path,
    json: bool,
) -> ExitCode {
    use delvewright_dsl::{L10nDoc, L10nKind};
    use std::collections::{BTreeMap, BTreeSet};

    let loaded = match load_or_refuse(campaign_dir, json) {
        Ok(l) => l,
        Err(exit) => return ExitCode::from(exit),
    };
    let campaign = match delvec::compiler::load::parse_loaded(&loaded) {
        Ok(c) => c,
        Err(diags) => {
            print_diags(&diags, json);
            return ExitCode::from(1);
        }
    };
    let raw_table = if table_path == Path::new("-") {
        let mut buf = String::new();
        match std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf) {
            Ok(_) => buf,
            Err(e) => {
                eprintln!("cannot read the table from stdin: {e}");
                return ExitCode::from(1);
            }
        }
    } else {
        match std::fs::read_to_string(table_path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("cannot read the table {}: {e}", table_path.display());
                return ExitCode::from(1);
            }
        }
    };
    let table: BTreeMap<String, String> = match serde_json::from_str(&raw_table) {
        Ok(t) => t,
        Err(e) => {
            eprintln!(
                "the table must be a JSON object mapping canonical English to its translation: {e}"
            );
            return ExitCode::from(1);
        }
    };

    let inventory = delvewright_dsl::l10n::inventory(&campaign);
    let existing = loaded
        .l10n
        .get(lang)
        .and_then(|b| serde_json::from_slice::<L10nDoc>(b).ok());

    let mut content: BTreeMap<String, String> = BTreeMap::new();
    let mut source: BTreeMap<String, String> = BTreeMap::new();
    let mut from_table = 0usize;
    let mut kept = 0usize;
    let mut kept_unguarded = 0usize;
    let mut missing: Vec<(&str, &str)> = Vec::new();
    let mut matched: BTreeSet<&str> = BTreeSet::new();
    // English → the keys that hold it. More than one key is the named limit.
    let mut by_english: BTreeMap<&str, Vec<&str>> = BTreeMap::new();

    for (key, en) in &inventory {
        by_english
            .entry(en.as_str())
            .or_default()
            .push(key.as_str());
        if let Some(t) = table.get(en) {
            matched.insert(en.as_str());
            content.insert(key.clone(), t.clone());
            source.insert(key.clone(), en.clone());
            from_table += 1;
            continue;
        }
        let Some(doc) = existing.as_ref() else {
            missing.push((key, en));
            continue;
        };
        match (doc.content.get(key), doc.source.get(key)) {
            (Some(t), Some(was)) if was == en => {
                content.insert(key.clone(), t.clone());
                source.insert(key.clone(), en.clone());
                kept += 1;
            }
            (Some(t), None) => {
                content.insert(key.clone(), t.clone());
                kept_unguarded += 1;
            }
            _ => missing.push((key, en)),
        }
    }

    let unmatched: Vec<&str> = table
        .keys()
        .map(String::as_str)
        .filter(|en| !matched.contains(en))
        .collect();
    let shared: Vec<(&str, &Vec<&str>)> = by_english
        .iter()
        .filter(|(_, keys)| keys.len() > 1)
        .map(|(en, keys)| (*en, keys))
        .collect();

    let doc = L10nDoc {
        dsl_version: campaign.world.dsl_version.clone(),
        campaign_id: campaign.world.campaign_id.clone(),
        kind: L10nKind::L10n,
        lang: lang.to_string(),
        content,
        source,
    };
    let text = match delvewright_dsl::to_canonical_string(&doc) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("internal error: cannot serialize the sidecar: {e}");
            return ExitCode::from(EXIT_INTERNAL);
        }
    };
    let dir = campaign_dir.join("l10n");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("cannot create {}: {e}", dir.display());
        return ExitCode::from(1);
    }
    let path = dir.join(format!("{lang}.json"));
    if let Err(e) = std::fs::write(&path, &text) {
        eprintln!("cannot write {}: {e}", path.display());
        return ExitCode::from(1);
    }

    let translated = doc.content.len();
    let total = inventory.len();
    if json {
        let report = serde_json::json!({
            "lang": lang,
            "path": path.display().to_string(),
            "rows_total": total,
            "rows_translated": translated,
            "from_table": from_table,
            "kept": kept,
            "kept_without_source": kept_unguarded,
            "missing": missing.iter().map(|(k, en)| serde_json::json!({"key": k, "en": en})).collect::<Vec<_>>(),
            "unmatched_table_entries": unmatched,
            "shared_english": shared.iter().map(|(en, keys)| serde_json::json!({"en": en, "keys": keys})).collect::<Vec<_>>(),
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&report).unwrap_or_default()
        );
    } else {
        println!(
            "{translated} of {total} rows translated → {}",
            path.display()
        );
        println!(
            "  {from_table} from the table, {kept} kept from unchanged English, \
             {kept_unguarded} kept with no recorded source"
        );
        if missing.is_empty() {
            println!("  no English string is missing a translation");
        } else {
            println!("  {} English string(s) still untranslated:", missing.len());
            for (key, en) in &missing {
                println!("    {key}: {en}");
            }
        }
        if unmatched.is_empty() {
            println!("  every table entry matched a row");
        } else {
            println!(
                "  {} table entry(ies) matched no inventory row:",
                unmatched.len()
            );
            for en in &unmatched {
                println!("    {en}");
            }
        }
        if !shared.is_empty() {
            println!(
                "  {} English string(s) are held by more than one key — the table form cannot \
                 give them different translations; edit the sidecar directly where they must \
                 differ:",
                shared.len()
            );
            for (en, keys) in &shared {
                println!("    {en}\n      {}", keys.join(", "));
            }
        }
    }
    if missing.is_empty() && unmatched.is_empty() {
        ExitCode::SUCCESS
    } else {
        // The same signal `fmt --check` gives: the artifact on disk is written,
        // and it does not yet say what the campaign needs.
        ExitCode::from(1)
    }
}

// ---------------------------------------------------------------------------
// `snapshot` (spec-0015: the visual authoring loop)
// ---------------------------------------------------------------------------
