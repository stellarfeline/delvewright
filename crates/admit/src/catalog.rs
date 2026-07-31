//! **Catalog cards** — `catalog/<asset-id>.json` (spec-0007 step 2).
//!
//! The card is the verification record the vision agent writes per candidate and
//! the vocabulary `/new-delve` queries when picking prefab sets. It is validated
//! with the **same rigor as the DSL stages**: a serde model with
//! `deny_unknown_fields`, enum-typed verdicts, a `1..=5` quality bound, and a
//! **license allowlist** (ADR-0013 — CC0 / CC BY / original / MIT / Apache-2.0 /
//! GPL-compatible; NC / ND / unknown reject).

use serde::{Deserialize, Serialize};

use crate::diag::{DW_CATALOG, DW_LICENSE, Diagnostic};

/// A catalog card. `deny_unknown_fields` makes the schema closed.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogCard {
    /// Stable asset id (`author/pack/piece`); the card lives at `catalog/<id>.json`.
    pub asset_id: String,
    /// 2–3 sentences of prose.
    pub description: String,
    /// Structured tags (the searchable vocabulary).
    pub tags: Tags,
    /// Style-fit verdict + rationale.
    pub style_fit: StyleFit,
    /// Quality 1–5.
    pub quality: u8,
    /// Render paths (deterministic `delve-render` output the verdict was ruled on).
    #[serde(default)]
    pub renders: Vec<String>,
    /// Which demand-sheet categories this asset fills.
    #[serde(default)]
    pub demand_categories: Vec<String>,
    /// License + provenance evidence (ADR-0013 / ADR-0007).
    pub license: LicenseEvidence,
    /// Gallery-walk curation (populated by `curate merge`; absent until then).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub curation: Option<Curation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tags {
    /// Free-form theme(s): `keep`, `crypt`, `village`, ...
    #[serde(default)]
    pub theme: Vec<String>,
    /// Era / architectural style (free-form): `medieval`, `roman`, ...
    pub era_style: String,
    /// Dominant palette blocks / colours (free-form).
    #[serde(default)]
    pub palette: Vec<String>,
    /// Structural condition.
    pub condition: Condition,
    /// Scale class.
    pub scale_class: ScaleClass,
    /// Offered piece types: `room`, `corridor`, `set-piece`, ...
    #[serde(default)]
    pub piece_types: Vec<String>,
    /// Interior / exterior / both.
    pub interior_exterior: InteriorExterior,
    /// Biomes the piece fits (free-form).
    #[serde(default)]
    pub biome_fit: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Condition {
    Intact,
    Ruined,
    Mixed,
    Weathered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScaleClass {
    Small,
    Medium,
    Large,
    SetPiece,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteriorExterior {
    Interior,
    Exterior,
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StyleFit {
    pub verdict: Verdict,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Approve,
    Borderline,
    Reject,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LicenseEvidence {
    /// SPDX id (or `original`).
    pub spdx: String,
    /// Where the asset came from: `original`, `modrinth`, `planetminecraft`, ...
    pub source: String,
    /// The licensing URL ("free download" ≠ licensed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Archived proof (an archive.org URL or an in-repo evidence path).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived_proof: Option<String>,
    /// Attribution string for the aggregated `ATTRIBUTION` file (CC BY).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribution: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Curation {
    /// Gallery-walk notes harvested from `dw.note`.
    #[serde(default)]
    pub notes: Vec<CurationNote>,
    /// Optional aggregate summary the agent writes when merging.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurationNote {
    pub at: String,
    pub text: String,
    pub pos: [i64; 3],
}

impl CatalogCard {
    pub fn from_json(text: &str) -> Result<CatalogCard, String> {
        serde_json::from_str(text).map_err(|e| format!("invalid catalog card: {e}"))
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("catalog card serializes") + "\n"
    }

    /// Validate the card's semantic constraints (schema shape is already enforced
    /// by serde on parse). Returns error diagnostics; empty ⇒ valid.
    pub fn validate(&self) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        if !(1..=5).contains(&self.quality) {
            diags.push(Diagnostic::error(
                DW_CATALOG,
                format!("quality {} out of range (1..=5)", self.quality),
            ));
        }
        if self.description.trim().is_empty() {
            diags.push(Diagnostic::error(DW_CATALOG, "description is empty"));
        }
        if self.style_fit.rationale.trim().is_empty() {
            diags.push(Diagnostic::error(
                DW_CATALOG,
                "style_fit.rationale is empty (a verdict needs a reason)",
            ));
        }
        // License allowlist (ADR-0013).
        if let Err(reason) = license_allowed(&self.license.spdx) {
            diags.push(Diagnostic::error(
                DW_LICENSE,
                format!("license `{}` rejected: {reason}", self.license.spdx),
            ));
        }
        // "Free download ≠ licensed": a non-original license needs a source URL.
        let original = self.license.spdx.eq_ignore_ascii_case("original")
            || self.license.source.eq_ignore_ascii_case("original");
        if !original && self.license.url.as_deref().unwrap_or("").trim().is_empty() {
            diags.push(Diagnostic::error(
                DW_LICENSE,
                "non-original license has no `url` (license must be verifiable, not just 'free to download')",
            ));
        }
        diags
    }
}

/// Enforce the ADR-0013 license allowlist. `Ok(())` ⇒ allowed; `Err(reason)` ⇒
/// rejected. NC / ND / ShareAlike / unknown are rejected. Case-insensitive.
///
/// FLAGGED for owner review: ShareAlike (`-SA`) is rejected for **prefab** assets
/// — spec-0007 lists "CC0 / CC BY / original" for prefabs (CC BY-SA is the
/// *campaign* license, not a prefab license). Loosen here if the owner wants CC
/// BY-SA prefabs admitted.
pub fn license_allowed(spdx: &str) -> Result<(), String> {
    let s = spdx.trim().to_ascii_uppercase();
    if s.is_empty() {
        return Err("empty license".to_string());
    }
    if s.contains("-NC") || s.contains("NONCOMMERCIAL") {
        return Err("NonCommercial (NC) is forbidden".to_string());
    }
    if s.contains("-ND") || s.contains("NODERIV") {
        return Err("NoDerivatives (ND) is forbidden".to_string());
    }
    if s.contains("-SA") || s.contains("SHAREALIKE") {
        return Err("ShareAlike (SA) not admitted for prefab assets".to_string());
    }
    let ok = s == "ORIGINAL"
        || s == "CC0"
        || s.starts_with("CC0-")
        || s == "CC-BY"
        || s.starts_with("CC-BY-") // versions, already NC/ND/SA-filtered above
        || s == "MIT"
        || s == "APACHE-2.0"
        || s == "BSD-2-CLAUSE"
        || s == "BSD-3-CLAUSE"
        || s == "GPL-3.0-ONLY"
        || s == "GPL-3.0-OR-LATER"
        || s == "LGPL-3.0-ONLY"
        || s == "LGPL-3.0-OR-LATER";
    if ok {
        Ok(())
    } else {
        Err(format!("`{spdx}` is not in the ADR-0013 allowlist"))
    }
}
