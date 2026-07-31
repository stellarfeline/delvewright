//! Load a campaign directory (`world.json`, `npcs.json`, …) into the DSL's
//! [`RawCampaign`], keeping the raw bytes for input hashing (manifest).

use std::collections::BTreeMap;
use std::path::Path;

use delvewright_dsl::RawCampaign;

/// The six stage filenames a campaign directory must contain.
pub const STAGE_FILES: [&str; 6] = [
    "world.json",
    "npcs.json",
    "classes.json",
    "quest-plan.json",
    "quests.json",
    "dialogue.json",
];

/// A loaded campaign directory: the parsed-ready [`RawCampaign`] plus the exact
/// raw file contents (by filename) for deterministic input hashing.
pub struct LoadedCampaign {
    /// The six raw stage documents, ready for `parse_campaign`.
    pub raw: RawCampaign,
    /// Filename → exact file bytes, for `manifest.json` input hashes.
    pub inputs: BTreeMap<String, Vec<u8>>,
    /// i18n l10n sidecars found under `l10n/`: language code (filename stem) →
    /// raw sidecar bytes. Empty when the campaign ships no `l10n/` directory.
    pub l10n: BTreeMap<String, Vec<u8>>,
}

/// Read all six stage files from `dir` plus any `l10n/*.json` sidecars. Fails if a
/// stage file is missing/unreadable; a missing `l10n/` directory is not an error
/// (English-only campaign).
pub fn load_campaign_dir(dir: &Path) -> std::io::Result<LoadedCampaign> {
    let mut inputs = BTreeMap::new();
    let mut read = |name: &str| -> std::io::Result<String> {
        let bytes = std::fs::read(dir.join(name))?;
        let s = String::from_utf8(bytes.clone()).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{name}: {e}"))
        })?;
        inputs.insert(name.to_string(), bytes);
        Ok(s)
    };
    let world = read("world.json")?;
    let npcs = read("npcs.json")?;
    let classes = read("classes.json")?;
    let quest_plan = read("quest-plan.json")?;
    let quests = read("quests.json")?;
    let dialogue = read("dialogue.json")?;
    let l10n = load_l10n_dir(&dir.join("l10n"))?;
    Ok(LoadedCampaign {
        raw: RawCampaign {
            world,
            npcs,
            classes,
            quest_plan,
            quests,
            dialogue,
        },
        inputs,
        l10n,
    })
}

/// Read every `<code>.json` sidecar in an `l10n/` directory → `code` (filename
/// stem) → raw bytes. Returns an empty map if the directory does not exist.
/// Iteration is sorted for determinism (ADR-0006).
fn load_l10n_dir(l10n_dir: &Path) -> std::io::Result<BTreeMap<String, Vec<u8>>> {
    let mut out = BTreeMap::new();
    if !l10n_dir.is_dir() {
        return Ok(out);
    }
    for entry in std::fs::read_dir(l10n_dir)? {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Some(code) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        out.insert(code.to_string(), std::fs::read(&path)?);
    }
    Ok(out)
}
