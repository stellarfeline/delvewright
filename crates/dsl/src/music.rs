//! A scene states its music (spec-0109): the sound category every played or
//! stopped sound names, and the obligation that every biome a camera can stand
//! in declares a track or silence (`DW1031`).
//!
//! The pinned client chooses music from one thing, the biome under the camera
//! (`Minecraft.getSituationalMusic` reads `audio/background_music` from the
//! camera's attribute probe). A biome that declares none inherits the overworld
//! dimension's own (`music.game`), which is vanilla's calm overworld music —
//! the track nobody chose. So every atmosphere and the ground (`horizon`)
//! declare `audio/background_music`, silence spelled `{}`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::diagnostic::{Diagnostic, DwCode, ExitTier};
use crate::envelope::Campaign;
use crate::world::Horizon;

/// **A sound category** as the pinned command tree spells it under
/// `playsound <sound> <source>` and `stopsound <targets> <source>` — the eleven
/// literals of 1.21.11. A played sound's category decides which of the
/// player's volume sliders governs it, and whether the music fade and a
/// `stop-sound music` reach it (spec-0109 §2.4, §3.4).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SoundCategory {
    /// `master` — vanilla's default for `playsound`; governed by the master
    /// slider alone.
    #[default]
    Master,
    /// `music` — the music slider and the scene's music fade govern it, and
    /// `stop-sound music` cuts it.
    Music,
    /// `record` — the jukebox slider.
    Record,
    /// `weather` — the weather slider.
    Weather,
    /// `block` — the blocks slider.
    Block,
    /// `hostile` — the hostile-creatures slider.
    Hostile,
    /// `neutral` — the friendly-creatures slider.
    Neutral,
    /// `player` — the players slider.
    Player,
    /// `ambient` — the ambient/environment slider.
    Ambient,
    /// `voice` — the voice/speech slider.
    Voice,
    /// `ui` — the interface slider.
    Ui,
}

impl SoundCategory {
    /// The command tree's literal.
    pub fn token(self) -> &'static str {
        match self {
            SoundCategory::Master => "master",
            SoundCategory::Music => "music",
            SoundCategory::Record => "record",
            SoundCategory::Weather => "weather",
            SoundCategory::Block => "block",
            SoundCategory::Hostile => "hostile",
            SoundCategory::Neutral => "neutral",
            SoundCategory::Player => "player",
            SoundCategory::Ambient => "ambient",
            SoundCategory::Voice => "voice",
            SoundCategory::Ui => "ui",
        }
    }

    /// Every category, in the command tree's order of declaration in the enum.
    pub const ALL: [SoundCategory; 11] = [
        SoundCategory::Master,
        SoundCategory::Music,
        SoundCategory::Record,
        SoundCategory::Weather,
        SoundCategory::Block,
        SoundCategory::Hostile,
        SoundCategory::Neutral,
        SoundCategory::Player,
        SoundCategory::Ambient,
        SoundCategory::Voice,
        SoundCategory::Ui,
    ];
}

crate::dw_code! {
    /// (spec-0109 §6.1) **A scene that does not state its music, or states one
    /// no delve body can hear.** An atmosphere whose `attributes` lack
    /// `audio/background_music`; `horizon` absent, in its retired string form,
    /// or without `attributes.audio/background_music`; a `creative` entry in
    /// any `audio/background_music` (no delve body is in creative mode); a
    /// `horizon.attributes` id outside `audio/`. Validation tier, at the world
    /// document. Prescription: write `audio/background_music` — `{}` for
    /// silence, or `{default: {sound, min_delay, max_delay,
    /// replace_current_music}}` for a track.
    pub const MUSIC_UNSTATED: DwCode = DwCode::new("DW1031", ExitTier::Build);
}

/// The attribute every biome a camera can stand in must carry.
pub const BACKGROUND_MUSIC: &str = "audio/background_music";

/// The two spellings the remedy names.
const SPELLINGS: &str = "`{}` for silence, or `{\"default\": {\"sound\": <id>, \"min_delay\": \
                         <ticks>, \"max_delay\": <ticks>, \"replace_current_music\": <bool>}}` \
                         for a track";

/// An attribute key without its optional `minecraft:` prefix.
pub fn canonical_attribute(key: &str) -> &str {
    key.strip_prefix("minecraft:").unwrap_or(key)
}

/// The `audio/background_music` value of an attribute map, under either
/// spelling of its key.
pub fn background_music(
    attributes: &std::collections::BTreeMap<String, serde_json::Value>,
) -> Option<&serde_json::Value> {
    attributes
        .iter()
        .find(|(k, _)| canonical_attribute(k) == BACKGROUND_MUSIC)
        .map(|(_, v)| v)
}

fn pointer_key(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

/// A `creative` entry in a `background_music` value: declared for a game
/// mode no delve body is ever in.
fn creative_refusal(who: &str, path: String, d: &mut Vec<Diagnostic>) {
    d.push(Diagnostic::error(
        MUSIC_UNSTATED,
        "world",
        path,
        format!(
            "{who} declares a `creative` track in `audio/background_music`. The pinned client \
             plays it only to a player in creative mode (`abilities.instabuild && \
             abilities.mayfly`), and no delve body is ever in creative mode — adventure and \
             spectator never select it — so it is a declaration nothing can hear. Remove the \
             `creative` entry; `default` is what every delve body hears, and `underwater` what a \
             submerged camera hears."
        ),
    ));
}

/// `DW1031` (spec-0109 §6.1): every atmosphere and the ground state their
/// music, and say nothing no delve body can hear.
pub(crate) fn check(c: &Campaign, d: &mut Vec<Diagnostic>) {
    let w = &c.world.content;
    for (i, a) in w.atmospheres.iter().enumerate() {
        let base = format!("/content/atmospheres/{i}/attributes");
        match background_music(&a.attributes) {
            None => d.push(Diagnostic::error(
                MUSIC_UNSTATED,
                "world",
                base,
                format!(
                    "atmosphere `{}` does not state its music: its `attributes` carry no \
                     `audio/background_music`. A biome that declares none inherits the \
                     overworld dimension's, vanilla's calm overworld music, so a camera standing \
                     in this place would hear a track nobody chose. Add \
                     `audio/background_music`: {SPELLINGS}.",
                    a.id
                ),
            )),
            Some(v) => {
                if v.get("creative").is_some() {
                    let key = a
                        .attributes
                        .keys()
                        .find(|k| canonical_attribute(k) == BACKGROUND_MUSIC)
                        .map(|k| pointer_key(k))
                        .unwrap_or_default();
                    creative_refusal(
                        &format!("atmosphere `{}`", a.id),
                        format!("{base}/{key}/creative"),
                        d,
                    );
                }
            }
        }
    }
    match &w.horizon {
        None => d.push(Diagnostic::error(
            MUSIC_UNSTATED,
            "world",
            "/content/horizon",
            format!(
                "`horizon` is absent, so the ground — the biome every cell no atmosphere paints \
                 stands in — does not state its music, and it would inherit the overworld \
                 dimension's calm overworld music. Write `horizon` in its object form with the \
                 ground's music: `{{\"base\": \"void\", \"attributes\": \
                 {{\"audio/background_music\": …}}}}`, where the value is {SPELLINGS}."
            ),
        )),
        Some(Horizon::Name(base)) => d.push(Diagnostic::error(
            MUSIC_UNSTATED,
            "world",
            "/content/horizon",
            format!(
                "`horizon: \"{}\"` is the retired string form, which has no room for the ground's \
                 music: the ground is a biome a camera stands in, and it must state a track or \
                 silence. Write the object form: `{{\"base\": \"{}\", \"attributes\": \
                 {{\"audio/background_music\": …}}}}`, where the value is {SPELLINGS}.",
                base.token(),
                base.token()
            ),
        )),
        Some(Horizon::Spec(spec)) => {
            for key in spec.attributes.keys() {
                if !canonical_attribute(key).starts_with("audio/") {
                    d.push(Diagnostic::error(
                        MUSIC_UNSTATED,
                        "world",
                        format!("/content/horizon/attributes/{}", pointer_key(key)),
                        format!(
                            "`horizon.attributes` admits the `audio/` ids only, and `{key}` is \
                             not one. The ground's look is the horizon's own (its base lays the \
                             sky the void biome carries); only what the ground sounds like is \
                             the campaign's to state here. Remove the line, or carry the look \
                             on an atmosphere a place stands in."
                        ),
                    ));
                }
            }
            match background_music(&spec.attributes) {
                None => d.push(Diagnostic::error(
                    MUSIC_UNSTATED,
                    "world",
                    "/content/horizon/attributes",
                    format!(
                        "`horizon.attributes` carries no `audio/background_music`, so the ground \
                         does not state its music and would inherit the overworld dimension's \
                         calm overworld music. Add `audio/background_music`: {SPELLINGS}."
                    ),
                )),
                Some(v) => {
                    if v.get("creative").is_some() {
                        let key = spec
                            .attributes
                            .keys()
                            .find(|k| canonical_attribute(k) == BACKGROUND_MUSIC)
                            .map(|k| pointer_key(k))
                            .unwrap_or_default();
                        creative_refusal(
                            "the ground (`horizon`)",
                            format!("/content/horizon/attributes/{key}/creative"),
                            d,
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_eleven_categories_are_the_command_trees_literals() {
        let tokens: Vec<&str> = SoundCategory::ALL.iter().map(|c| c.token()).collect();
        assert_eq!(
            tokens,
            [
                "master", "music", "record", "weather", "block", "hostile", "neutral", "player",
                "ambient", "voice", "ui"
            ]
        );
        for c in SoundCategory::ALL {
            let wire = serde_json::to_value(c).unwrap();
            assert_eq!(wire, serde_json::json!(c.token()));
        }
        assert_eq!(SoundCategory::default(), SoundCategory::Master);
    }
}
