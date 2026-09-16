//! The one `#rrggbb` colour literal rule.
//!
//! Two surfaces write a colour as a hex literal and both mean the same thing: a
//! potion's bottle override ([`crate::PotionContents::color`]) and a firework
//! star's `colors` / `fade_colors` (spec-0068 §3.1). Vanilla stores both as the
//! packed 24-bit integer [`packed`] returns, so the reading and the packing are
//! one rule in one place rather than a validator's private predicate beside an
//! emitter's private `from_str_radix`.

/// True if `s` is a `#rrggbb` colour literal.
///
/// Case-insensitive in the hex digits, because the exported schema's pattern is
/// and a document is read the way its schema says it is read.
#[must_use]
pub fn is_hex(s: &str) -> bool {
    let Some(hex) = s.strip_prefix('#') else {
        return false;
    };
    hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit())
}

/// The regular expression the exported JSON schema carries for a `#rrggbb`
/// field — the same rule [`is_hex`] applies, written the way a schema reader
/// consumes it.
pub const HEX_PATTERN: &str = "^#[0-9a-fA-F]{6}$";

/// The packed 24-bit integer vanilla stores for a `#rrggbb` literal
/// (`#ffd700` → `16766720`). `None` for anything [`is_hex`] refuses.
#[must_use]
pub fn packed(s: &str) -> Option<u32> {
    if !is_hex(s) {
        return None;
    }
    u32::from_str_radix(&s[1..], 16).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_literal_the_way_the_pattern_does() {
        assert!(is_hex("#ffd700"));
        assert!(is_hex("#FFD700"));
        assert!(!is_hex("ffd700"));
        assert!(!is_hex("#fff"));
        assert!(!is_hex("#ffd7000"));
        assert!(!is_hex("#gggggg"));
        assert!(!is_hex(""));
    }

    #[test]
    fn packs_what_vanilla_stores() {
        assert_eq!(packed("#ffd700"), Some(16_766_720));
        assert_eq!(packed("#ffffff"), Some(16_777_215));
        assert_eq!(packed("#8b0000"), Some(9_109_504));
        assert_eq!(packed("#0a0b0c"), Some(658_188));
        assert_eq!(packed("#fff"), None);
    }
}
