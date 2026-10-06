//! **The one record of a licence this engine has**, and the one allowlist it is
//! judged against (ADR-0013).
//!
//! [`LicenseEvidence`] is the shape a prefab catalog card carries
//! (`delvec::admit::catalog`, spec-0007) and the shape a campaign's
//! `world.textures[]` row carries (spec-0084): one struct, so a licence is
//! recorded the same way wherever an asset enters. [`license_allowed`] is the
//! allowlist both are judged by, under one code, `DW0741`.
//!
//! What each surface additionally demands of the record differs, because what
//! each surface admits differs. A catalog card's rules live with the card. An
//! image a campaign declares is held to [`image_license_refusals`]: it ships
//! inside the delve's resource pack, so its provenance is what a release's
//! aggregated attribution is built from, and every field that attribution needs
//! is required.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// License and provenance evidence for one asset (ADR-0013 / ADR-0007).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LicenseEvidence {
    /// SPDX id (or `original`, for work made for this project).
    pub spdx: String,
    /// Where the asset came from: `original`, or the site or person it was taken
    /// from.
    pub source: String,
    /// The licensing URL ("free download" is not a licence).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Archived proof (an archive.org URL or an in-repo evidence path).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived_proof: Option<String>,
    /// The attribution line a CC BY licence obliges, as it enters the aggregated
    /// attribution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribution: Option<String>,
    /// Free-form provenance note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Enforce the ADR-0013 license allowlist. `Ok(())` ⇒ allowed; `Err(reason)` ⇒
/// rejected. NC / ND / ShareAlike / unknown are rejected. Case-insensitive.
///
/// ShareAlike (`-SA`) is rejected: spec-0007 lists "CC0 / CC BY / original" for
/// assets (CC BY-SA is the *campaign* licence, not an asset licence).
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
        return Err("ShareAlike (SA) not admitted for assets".to_string());
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

/// Every reason the licence of an **image a campaign declares** (spec-0084 §3.1)
/// cannot ship, each one sentence naming its remedy; empty ⇒ admitted.
///
/// The allowlist ([`license_allowed`]); `original` requires `source: original`
/// (an original image has no other source); any other licence requires `url`
/// (a licence must be verifiable); a `CC-BY-*` licence requires `attribution`
/// (the line the licence obliges, which the release's attribution is built
/// from).
pub fn image_license_refusals(ev: &LicenseEvidence) -> Vec<String> {
    let mut out = Vec::new();
    if let Err(reason) = license_allowed(&ev.spdx) {
        out.push(format!(
            "license `{}` is refused: {reason} — only CC0, CC-BY, MIT, Apache-2.0, \
             GPL-3.0-compatible, or `original` images may ship (ADR-0013); use an image \
             under one of those, or draw one",
            ev.spdx
        ));
        return out;
    }
    let spdx = ev.spdx.trim().to_ascii_uppercase();
    let blank = |v: &Option<String>| v.as_deref().unwrap_or("").trim().is_empty();
    if spdx == "ORIGINAL" {
        if !ev.source.trim().eq_ignore_ascii_case("original") {
            out.push(format!(
                "license `original` with `source` `{}` — an original image is made for this \
                 project and has no other source; set `source` to `original`, or record the \
                 licence the image was taken under",
                ev.source
            ));
        }
        return out;
    }
    if blank(&ev.url) {
        out.push(format!(
            "license `{}` has no `url` — a licence must be verifiable, not just 'free to \
             download'; add `license.url` pointing at the licence page",
            ev.spdx
        ));
    }
    if (spdx == "CC-BY" || spdx.starts_with("CC-BY-")) && blank(&ev.attribution) {
        out.push(format!(
            "license `{}` has no `attribution` — a CC BY licence obliges a credit line, and \
             the release's attribution is built from this field; add `license.attribution`",
            ev.spdx
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(spdx: &str, source: &str) -> LicenseEvidence {
        LicenseEvidence {
            spdx: spdx.into(),
            source: source.into(),
            url: None,
            archived_proof: None,
            attribution: None,
            note: None,
        }
    }

    #[test]
    fn an_original_image_needs_only_its_source() {
        assert!(image_license_refusals(&ev("original", "original")).is_empty());
        assert_eq!(image_license_refusals(&ev("original", "a site")).len(), 1);
    }

    #[test]
    fn a_third_party_image_needs_its_url_and_a_cc_by_one_its_credit() {
        let mut e = ev("CC-BY-4.0", "a site");
        assert_eq!(image_license_refusals(&e).len(), 2);
        e.url = Some("https://example.org/licence".into());
        assert_eq!(image_license_refusals(&e).len(), 1);
        e.attribution = Some("Artist — Title".into());
        assert!(image_license_refusals(&e).is_empty());
        let mut m = ev("MIT", "a repo");
        m.url = Some("https://example.org/licence".into());
        assert!(image_license_refusals(&m).is_empty());
    }

    #[test]
    fn the_allowlist_refuses_first_and_alone() {
        let r = image_license_refusals(&ev("CC-BY-NC-4.0", "a site"));
        assert_eq!(r.len(), 1);
        assert!(r[0].contains("NonCommercial"), "{r:?}");
    }
}
