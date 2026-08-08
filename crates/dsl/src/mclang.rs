//! The pinned client's own language set (Minecraft Java 1.21.11, ADR-0009).
//!
//! A delve's resource pack carries one `assets/delvewright/lang/<code>.json` per
//! declared language and the client auto-selects the file matching its locale
//! (spec-0029). Which `<code>`s exist is therefore not our choice: it is a fact
//! about the pinned client, and a file under a name no client asks for is a
//! language silently dropped.
//!
//! ## Provenance — derived, not transcribed
//!
//! [`CLIENT_LANGS`] is **derived from Mojang's own metadata for exactly 1.21.11**,
//! not copied from a wiki page:
//!
//! ```text
//! version manifest  https://launchermeta.mojang.com/mc/game/version_manifest_v2.json
//! 1.21.11 metadata  https://piston-meta.mojang.com/v1/packages/
//!                     53338515d3ee0af37b9ba26553c0d5464bc51082/1.21.11.json
//!                   sha1 53338515d3ee0af37b9ba26553c0d5464bc51082
//! asset index "29"  https://piston-meta.mojang.com/v1/packages/
//!                     e9f7ce469a9f70f7e1197bc61b6b74ad8c87ab11/29.json
//!                   sha1 e9f7ce469a9f70f7e1197bc61b6b74ad8c87ab11
//! selector          every object key matching `minecraft/lang/<code>.json`
//! result            142 codes, plus `en_us` (which ships inside the jar, not in
//!                   the asset index, and so is added explicitly)
//! ```
//!
//! Re-derive with `tools/derive-client-langs.py`, which prints the table and the
//! digests it read. The table is **baked into the source on purpose**: the compiler
//! must never reach the network during a build (ADR-0006 determinism), exactly as
//! the server jar's provenance is recorded rather than fetched.
//!
//! ## Why a membership table rather than a rewrite
//!
//! Our declared codes are BCP-47-ish (`zh-cn`); Minecraft's file stems are
//! lowercase `<language>_<region>` (`zh_cn`). [`mc_lang_code`] normalises and then
//! **checks membership against this derived set** — which is what makes
//! normalisation safe. A bare `-`→`_` rewrite on its own would happily invent
//! `de_de` from `de-de` and equally happily invent `de` from `de`, and the second
//! is a filename no client ever asks for. An unmatched code is `DW0184`.

/// Every language-file stem the pinned 1.21.11 client can load, sorted, with
/// `en_us` first. See the module header for how this was derived and how to
/// re-derive it.
pub const CLIENT_LANGS: &[&str] = &[
    "en_us", "af_za", "ar_sa", "ast_es", "az_az", "ba_ru", "bar", "be_by", "be_latn", "bg_bg",
    "br_fr", "brb", "bs_ba", "ca_es", "cs_cz", "cv_cu", "cy_gb", "da_dk", "de_at", "de_ch",
    "de_de", "el_gr", "en_au", "en_ca", "en_gb", "en_nz", "en_pt", "en_ud", "enp", "enws", "eo_uy",
    "es_ar", "es_cl", "es_ec", "es_es", "es_mx", "es_uy", "es_ve", "esan", "et_ee", "eu_es",
    "fa_ir", "fi_fi", "fil_ph", "fo_fo", "fr_ca", "fr_ch", "fr_fr", "fra_de", "fur_it", "fy_nl",
    "ga_ie", "gd_gb", "gl_es", "go_fr", "got_de", "hal_ua", "haw_us", "he_il", "hi_in", "hn_no",
    "hr_hr", "hu_hu", "hy_am", "id_id", "ig_ng", "io_en", "is_is", "isv", "it_it", "ja_jp",
    "jbo_en", "ka_ge", "kk_kz", "kn_in", "ko_kr", "ksh", "kw_gb", "ky_kg", "la_la", "lb_lu",
    "li_li", "lmo", "lo_la", "lol_us", "lt_lt", "lv_lv", "lzh", "mk_mk", "mn_mn", "ms_my", "mt_mt",
    "nah", "nds_de", "nl_be", "nl_nl", "nn_no", "no_no", "oc_fr", "ovd", "pl_pl", "pls", "pt_br",
    "pt_pt", "qcb_es", "qid", "qya_aa", "ro_ro", "rpr", "ru_ru", "ry_ua", "sah_sah", "se_no",
    "sk_sk", "sl_si", "so_so", "sq_al", "sr_cs", "sr_sp", "sv_se", "sxu", "szl", "ta_in", "th_th",
    "tl_ph", "tlh_aa", "tok", "tr_tr", "tt_ru", "tzo_mx", "uk_ua", "uz_uz", "val_es", "vec_it",
    "vi_vn", "vp_vl", "vro", "yi_de", "yo_ng", "zh_cn", "zh_hk", "zh_tw", "zlm_arab",
];

/// The Minecraft language-file stem for a declared language code, or `None` when
/// the pinned client has no such file (`DW0184`).
///
/// Resolution, in order:
///
/// 1. normalise — lowercase, `-` → `_` — and accept it if the pinned client ships
///    that exact stem (`zh-cn` → `zh_cn`, `pt-br` → `pt_br`);
/// 2. a **bare language** with no region resolves to `<lang>_<lang>` when the
///    client ships one (`de` → `de_de`, `fr` → `fr_fr`), else to the client's sole
///    file for that language (`ja` → `ja_jp`, `ko` → `ko_kr`);
/// 3. anything else — including a bare code with several regions and no
///    `<lang>_<lang>` (`zh`, `sr`, `be`) — is unresolved, because guessing which
///    region an author meant is exactly how a language ships invisible.
pub fn mc_lang_code(code: &str) -> Option<&'static str> {
    let norm = code.to_ascii_lowercase().replace('-', "_");
    // `en` is the canonical language ([`crate::l10n::CANONICAL_LANG`]) and the
    // jar's own default file, so it resolves to `en_us` rather than being read as
    // an ambiguous bare code over `en_au`/`en_ca`/`en_gb`/`en_nz`.
    if norm == "en" {
        return Some("en_us");
    }
    if let Some(hit) = CLIENT_LANGS.iter().find(|c| **c == norm) {
        return Some(hit);
    }
    if norm.contains('_') {
        return None;
    }
    let prefix = format!("{norm}_");
    let mut matches = CLIENT_LANGS.iter().filter(|c| c.starts_with(&prefix));
    let first = matches.next()?;
    let doubled = format!("{norm}_{norm}");
    if let Some(hit) = CLIENT_LANGS.iter().find(|c| **c == doubled) {
        return Some(hit);
    }
    match matches.next() {
        None => Some(first),
        Some(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The derived table is sorted (after `en_us`), unique, and the size the
    /// asset index yielded. A silent shrink would drop languages; a silent growth
    /// would mean the table stopped matching the pinned client.
    #[test]
    fn table_is_the_derived_set() {
        assert_eq!(CLIENT_LANGS[0], "en_us");
        assert_eq!(
            CLIENT_LANGS.len(),
            143,
            "142 asset-index language files + en_us from the jar"
        );
        let rest = &CLIENT_LANGS[1..];
        assert!(
            rest.windows(2).all(|w| w[0] < w[1]),
            "the derived table must stay sorted and unique"
        );
    }

    /// Every code the hand-maintained pre-derivation table accepted still
    /// resolves, to the same stem: this table is a strict superset, so no campaign
    /// that compiled before can stop compiling.
    #[test]
    fn superset_of_the_previous_hand_table() {
        for (declared, mc) in [
            ("en", "en_us"),
            ("en-us", "en_us"),
            ("en-gb", "en_gb"),
            ("zh-cn", "zh_cn"),
            ("zh-tw", "zh_tw"),
            ("zh-hk", "zh_hk"),
            ("ja", "ja_jp"),
            ("ja-jp", "ja_jp"),
            ("ko", "ko_kr"),
            ("ko-kr", "ko_kr"),
            ("de", "de_de"),
            ("de-de", "de_de"),
            ("fr", "fr_fr"),
            ("fr-fr", "fr_fr"),
            ("es", "es_es"),
            ("es-es", "es_es"),
            ("es-mx", "es_mx"),
            ("pt-br", "pt_br"),
            ("pt", "pt_pt"),
            ("pt-pt", "pt_pt"),
            ("ru", "ru_ru"),
            ("ru-ru", "ru_ru"),
            ("it", "it_it"),
            ("it-it", "it_it"),
            ("pl", "pl_pl"),
            ("pl-pl", "pl_pl"),
            ("nl", "nl_nl"),
            ("nl-nl", "nl_nl"),
            ("tr", "tr_tr"),
            ("tr-tr", "tr_tr"),
            ("uk", "uk_ua"),
            ("uk-ua", "uk_ua"),
            ("cs", "cs_cz"),
            ("cs-cz", "cs_cz"),
            ("sv", "sv_se"),
            ("sv-se", "sv_se"),
            ("th", "th_th"),
            ("th-th", "th_th"),
            ("vi", "vi_vn"),
            ("vi-vn", "vi_vn"),
            ("id", "id_id"),
            ("id-id", "id_id"),
        ] {
            assert_eq!(mc_lang_code(declared), Some(mc), "`{declared}`");
        }
    }

    /// An ambiguous bare code is unresolved rather than guessed, and a code the
    /// client does not ship is unresolved rather than invented.
    #[test]
    fn ambiguity_and_invention_are_rejected() {
        for bad in ["zh", "sr", "be", "xx", "de-xx", "klingon", ""] {
            assert_eq!(mc_lang_code(bad), None, "`{bad}` must not resolve");
        }
    }
}
