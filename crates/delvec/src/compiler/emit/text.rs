//! The one text authority (`DW0185`): `tr`, `tr_with`, `snbt_component`, `snbt_text_component`, and the serialisation primitives.

use super::*;

/// `DW0185`: no emitted byte may still carry a translation tag. See
/// [`DW_UNTRANSLATED_LITERAL`] for what a hit means and how to fix it.
///
/// Public so the spec-0029 test suite can drive it against a synthetic tree: the
/// only way to produce a leak from a real campaign is a defective emitter, and a
/// red that needs a defective emitter to exist is a red nobody can re-run.
pub fn check_untranslated_literals(
    out: &BuildOutput,
    pack_assets: &BTreeMap<String, Vec<u8>>,
) -> Result<(), BuildFailure> {
    // EVERY offending file, not the first: a leak is usually one emitter used from
    // several places, and a one-at-a-time diagnostic turns one fix into ten builds.
    let mut hits: Vec<String> = Vec::new();
    // The resource pack is scanned as its own assets rather than through the zip:
    // the zip embeds PNG bytes, and a byte scan of compressed/binary payloads is a
    // scan whose result depends on what a prefab happens to contain.
    for (path, bytes) in out.iter().chain(pack_assets.iter()) {
        // Classified, not guessed. A build output is either compiler-authored TEXT
        // — which this check owns — or a verbatim copy of a binary input asset,
        // which carries no authored string because the compiler never writes one
        // into it. Anything else is an output nobody has classified: it fails
        // here, so a new binary artifact cannot quietly opt out of the check.
        if is_verbatim_binary_output(path) {
            continue;
        }
        let Ok(text) = std::str::from_utf8(bytes) else {
            return Err(BuildFailure::Diagnostic {
                code: DW_UNTRANSLATED_LITERAL,
                message: format!(
                    "`{path}` is not UTF-8 text and is not a known verbatim binary output, so \
                     the untranslated-literal scan cannot read it. Classify it in \
                     `emit::is_verbatim_binary_output` (and say why in \
                     `docs/reference/compiler.md`) if it is a byte copy of an input asset"
                ),
            });
        };
        if !delvewright_dsl::has_tr_sigil(text) {
            continue;
        }
        // Name the offending key and the line, so the fix is mechanical.
        let (key, line) = text
            .lines()
            .find_map(|l| {
                let i = l.find(delvewright_dsl::TR_SIGIL)?;
                let rest = &l[i + delvewright_dsl::TR_SIGIL.len_utf8()..];
                let k = rest.split(delvewright_dsl::TR_SIGIL).next()?;
                let shown: String = l.chars().take(160).collect();
                Some((k.to_string(), shown.replace(delvewright_dsl::TR_SIGIL, "⟦")))
            })
            .unwrap_or_else(|| ("<unknown>".to_string(), String::new()));
        hits.push(format!("  {path}: `{key}` in: {line}"));
    }
    if hits.is_empty() {
        return Ok(());
    }
    let n = hits.len();
    let shown = hits.iter().take(25).cloned().collect::<Vec<_>>().join("\n");
    Err(BuildFailure::Diagnostic {
        code: DW_UNTRANSLATED_LITERAL,
        message: format!(
            "{n} emitted file(s) carry an authored player-visible string outside a text \
             component, so it would ship as a literal no client can translate. Lower each \
             through `emit::tr` / `emit::snbt_component` (which emit \
             `{{\"translate\":\"<key>\",\"fallback\":\"<English>\"}}`); if the site is genuinely \
             not a component and not read by a player — a manifest field, a reviewer \
             chronicle, `critical-path.json`, a generated PackTest source — read the string \
             through `dsl::l10n::plain` and add the site to the named-exclusion table in \
             `docs/reference/compiler.md`.\n{shown}"
        ),
    })
}

/// Whether a build output is a **verbatim copy of a binary input asset** rather
/// than compiler-authored text: a prefab structure `.nbt`, an NPC-skin PNG, the
/// art-font atlas, and the resource-pack zip that packages them (whose own
/// compiler-authored members — the lang files, the font provider — are scanned
/// separately, before zipping). The compiler writes no authored string into any of
/// these, so they are outside the `DW0185` scan; everything else must be readable
/// text, and a new output that is neither fails the scan rather than skipping it.
pub(super) fn is_verbatim_binary_output(path: &str) -> bool {
    path.ends_with(".nbt") || path.ends_with(".png") || path == "resourcepack.zip"
}

/// Escape a player-facing string as a double-quoted SNBT string. On 1.21.11
/// `CustomName` is a **text component**, so a bare quoted SNBT string is read as
/// literal text (the JSON-string form `'{"text":"…"}'` renders verbatim, incl. in
/// death messages — the M2 defect). Only `\` and `"` need escaping inside SNBT.
pub(super) fn snbt_string(s: &str) -> String {
    let esc = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{esc}\"")
}

/// The `CustomName:…,CustomNameVisible:1b,` NBT fragment (trailing comma) that
/// labels a floating objective marker with its objective `title`. When the
/// objective has no title, the marker carries NO name — an empty fragment — so it
/// still glows and is findable but never surfaces the raw objective id (e.g.
/// `obj/door`) as player-visible floating text — presentation hygiene. A titled
/// marker carries its title verbatim.
pub(super) fn marker_name_fields(title: Option<&str>) -> String {
    match title {
        Some(t) => format!("CustomName:{},CustomNameVisible:1b,", snbt_component(t)),
        None => String::new(),
    }
}

/// A text-component SNBT **compound** for a player-visible string:
/// `{text:"<escaped>"}`. Used for mannequin `description` (DSL v0.4) and any
/// component-form NBT field. This is deliberately NOT the stringified-JSON form
/// `'{"text":…}'`, which 1.21.11 renders as literal raw JSON above an entity's
/// head (owner-verified). The generated summons carry no `'{"text"` substring.
pub(super) fn snbt_text_component(s: &str) -> String {
    match delvewright_dsl::l10n::untag(s) {
        Some((key, english)) => snbt_translate(key, english),
        None => format!("{{text:{}}}", snbt_string(s)),
    }
}

/// Lower an authored player-visible string into a JSON **text component**
/// (spec-0029 §1): a translation-tagged string becomes
/// `{"translate": "<l10n key>", "fallback": "<English source>"}`, anything else
/// (a compiler-baked literal such as the default boundary message) stays
/// `{"text": …}`.
///
/// The `fallback` rides on the component rather than on the pack's own
/// `en_us.json` deliberately: a player who **declines** the resource-pack prompt
/// has no lang files at all, and the delve must still be playable in English
/// (spec-0029 §3).
pub(super) fn tr(s: &str) -> Value {
    match delvewright_dsl::l10n::untag(s) {
        Some((key, english)) => json!({ "translate": key, "fallback": english }),
        None => json!({ "text": s }),
    }
}

/// [`tr`] with extra component fields (`color`, `bold`, `italic`, `font`, …)
/// merged in. Styling is orthogonal to whether the body is a literal or a
/// translate key, so every styled site keeps its styling verbatim.
pub(super) fn tr_with(s: &str, fields: &[(&str, Value)]) -> Value {
    let mut v = tr(s);
    let obj = v.as_object_mut().expect("tr() builds an object");
    for (k, val) in fields {
        obj.insert((*k).to_string(), val.clone());
    }
    v
}

/// The **SNBT** form of [`tr`], for a text component living in an NBT field
/// (`CustomName`, a mannequin `description`, an item `custom_name`). Emitted as an
/// SNBT compound, never the stringified-JSON form, for the same reason
/// [`snbt_text_component`] always was: 1.21.11 renders `'{"text":…}'` above an
/// entity's head verbatim.
pub(crate) fn snbt_component(s: &str) -> String {
    match delvewright_dsl::l10n::untag(s) {
        Some((key, english)) => snbt_translate(key, english),
        // An untagged string keeps the bare quoted-string component form 1.21.11
        // already read it as, so a compiler-baked name stays byte-for-byte what it
        // was before spec-0029.
        None => snbt_string(s),
    }
}

/// The SNBT `{fallback:…,translate:…}` compound both SNBT component forms share.
/// Field order is alphabetical, matching the JSON components' `BTreeMap` order so
/// the two forms read the same in a diff.
pub(super) fn snbt_translate(key: &str, english: &str) -> String {
    format!(
        "{{fallback:{},translate:{}}}",
        snbt_string(english),
        snbt_string(key)
    )
}

/// The human string behind an authored value, for the **named exclusions**: sites
/// that are not text components and are not read by a player. Re-exported here so
/// every exclusion in `emit` is greppable as `plain(`.
pub(super) fn plain(s: &str) -> &str {
    delvewright_dsl::l10n::plain(s)
}

/// The delve title for a **compiler artifact**, not for a player: the generated
/// PackTest sources' `#>` descriptions and the reviewer render plan. These are
/// not text components and no client ever renders them, so they carry the English
/// source string rather than a translate key — a named exclusion, listed as such
/// in `docs/reference/compiler.md`.
pub(super) fn artifact_title(c: &delvewright_dsl::Campaign) -> &str {
    plain(&c.world.content.title)
}

/// Format an `f64` deterministically for SNBT with a guaranteed decimal point
/// (so `20` renders as `20.0`, read as a double). Uses `{:?}` (shortest
/// round-trip) which is stable across platforms.
pub(super) fn fmt_f64(x: f64) -> String {
    let s = format!("{x:?}");
    if s.contains('.') || s.contains('e') || s.contains("inf") || s.contains("NaN") {
        s
    } else {
        format!("{s}.0")
    }
}

/// The canonical pretty-printed bytes of an emitted JSON artifact — the same
/// rendering [`put_json`] writes, factored out so collision detection can compare
/// artifacts byte-for-byte before inserting.
pub(super) fn json_bytes(value: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(value).expect("json serializes");
    bytes.push(b'\n');
    bytes
}

/// One `dw.sys` score, as a chat component.
pub(super) fn sys_score(holder: &str) -> Value {
    json!({ "score": { "name": holder, "objective": "dw.sys" } })
}

/// Join lines with `\n` and a trailing newline.
pub(super) fn lines(v: &[String]) -> String {
    let mut s = v.join("\n");
    s.push('\n');
    s
}

/// Serialize a JSON value canonically (sorted keys via serde_json default map,
/// 2-space pretty, trailing newline) into `out` at `path`.
pub(super) fn put_json(out: &mut BuildOutput, path: &str, value: &Value) {
    let mut bytes = serde_json::to_vec_pretty(value).expect("json serializes");
    bytes.push(b'\n');
    out.insert(path.to_string(), bytes);
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut s = String::with_capacity(64);
    for b in digest {
        s.push_str(&format!("{b:02x}"));
    }
    s
}
