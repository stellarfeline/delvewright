//! **The party is seen in its own cutscenes** (spec-0095): a stand-in for each
//! player in play, where they stood, for the cutscene's whole length.
//!
//! # Why
//!
//! A cutscene puts every player in spectator riding the dolly camera, so their
//! bodies leave the world for its length: a wide shot of the boat the party is
//! sitting in shows an empty boat. The bodies are what the world holds; a
//! cutscene that removes them is a creator's statement (a vision, a memory, a
//! scene elsewhere), so `party: absent` states it and the default keeps them.
//!
//! # What vanilla gives (cited, pinned 1.21.11)
//!
//! * `minecraft:mannequin` draws the player model from a `profile` (a
//!   `ResolvableProfile`, the codec a player head's `minecraft:profile`
//!   component uses), holds `equipment` like any living entity, and takes
//!   `pose`, `immovable` and `hide_description`.
//! * The `fill_player_head` loot function, run with `this` a player, writes
//!   `ResolvableProfile.createResolved(player.getGameProfile())` onto a
//!   `minecraft:player_head`: the player's whole profile, its signed `textures`
//!   property included — read from `FillPlayerHead.run` with `javap` on the
//!   pinned jar. `data modify` carries it from the head's component onto the
//!   mannequin's `profile`, properties and signature intact (measured on the
//!   pinned server). No name is typed and nothing is looked up over the network:
//!   the skin is the one the server already holds for that player.
//! * `item replace entity <stand-in> <slot> from entity <player> <slot>` copies
//!   each armour slot and both hands; an empty slot copies as air.
//! * A killed mannequin drops none of its equipment (measured), so the copied
//!   gear is never duplicated; it leaves by the engine's one unseen removal.
//!
//! # The proof (`DW0971`)
//!
//! An emission self-check over the shipped tree: every cutscene the campaign
//! declares `present` places its stand-ins in its `start`, before the party
//! goes to spectator, and removes them in its `end`; every cutscene declared
//! `absent` places none. It cannot fire on a correct build; it is the standing
//! proof that a stand-in never outlives its cutscene and that a declaration is
//! never compiled into nothing.

use crate::compiler::emit::{BuildFailure, BuildOutput};
use delvewright_dsl::{CutsceneParty, DwCode, ExitTier};

delvewright_dsl::dw_code! {
    /// `DW0971`: a cutscene's stand-ins disagree with its declaration — a
    /// `present` cutscene that places none, places them after the party left
    /// its bodies, or does not remove them at its end; or an `absent` cutscene
    /// that places some (spec-0095 §5). An engine defect, never an authoring one.
    pub const DW_STANDIN_LIFETIME: DwCode = DwCode::new("DW0971", ExitTier::Build);
}

/// The shared function that places one stand-in for `@s`, at `@s`.
pub const STANDIN_FN: &str = "cs_standin";

/// The loot table that hands a stand-in its player's profile.
pub const STANDIN_LOOT: &str = "standin_profile";

/// The class tag every stand-in carries for its whole life.
pub const STANDIN_TAG: &str = "dw_standin";

/// Carried from the summon until the cutscene's `start` claims the stand-in.
const NEW_TAG: &str = "dw_standin_new";

/// Carried only inside one call of [`STANDIN_FN`], so the lines that dress the
/// stand-in address the one just summoned and no other.
const THIS_TAG: &str = "dw_standin_this";

/// The entity a stand-in is.
pub const STANDIN_ENTITY: &str = "minecraft:mannequin";

/// The slots copied from the player, in emission order. The head slot is
/// written last by the profile step's own `armor.head` copy.
pub const SLOTS: [&str; 6] = [
    "armor.head",
    "armor.chest",
    "armor.legs",
    "armor.feet",
    "weapon.mainhand",
    "weapon.offhand",
];

/// The per-cutscene tag of the stand-ins one cutscene placed.
pub fn cutscene_tag(bare: &str) -> String {
    format!("{STANDIN_TAG}_{bare}")
}

/// Who gets a stand-in: every player in play — not one already watching (a
/// respawn wait, or another cutscene, whose own stand-in already shows them)
/// and not one out of the body in spectator (a creator's free camera).
pub fn audience(cutscene_tag: &str) -> String {
    format!("@a[tag=!{cutscene_tag},gamemode=!spectator]")
}

fn this_sel() -> String {
    format!("@e[type={STANDIN_ENTITY},tag={THIS_TAG},limit=1]")
}

/// The lines a `present` cutscene's `start` runs, before `gamemode spectator`.
pub fn start_lines(ns: &str, bare: &str, cutscene_tag: &str) -> Vec<String> {
    let new = format!("@e[type={STANDIN_ENTITY},tag={NEW_TAG}]");
    vec![
        format!(
            "execute as {} at @s run function {ns}:{STANDIN_FN}",
            audience(cutscene_tag)
        ),
        format!("tag {new} add {}", self::cutscene_tag(bare)),
        format!("tag {new} remove {NEW_TAG}"),
    ]
}

/// The body of [`STANDIN_FN`]: run as a player, at the player.
pub fn standin_fn_body(ns: &str) -> Vec<String> {
    let me = this_sel();
    let mut out = vec![
        format!(
            "summon {STANDIN_ENTITY} ~ ~ ~ {{Tags:[\"{STANDIN_TAG}\",\"{NEW_TAG}\",\"{THIS_TAG}\"],pose:\"standing\",immovable:1b,NoGravity:1b,Invulnerable:1b,Silent:1b,hide_description:1b}}"
        ),
        // Facing: the player's yaw, level head.
        format!("tp {me} ~ ~ ~ ~ 0"),
        // The skin: the player's own profile, through a head the loot function fills.
        format!("loot replace entity {me} armor.head loot {ns}:{STANDIN_LOOT}"),
        format!(
            "data modify entity {me} profile set from entity {me} equipment.head.components.\"minecraft:profile\""
        ),
    ];
    for slot in SLOTS {
        out.push(format!(
            "item replace entity {me} {slot} from entity @s {slot}"
        ));
    }
    out.push(format!(
        "tag @e[type={STANDIN_ENTITY},tag={THIS_TAG}] remove {THIS_TAG}"
    ));
    out
}

/// The [`STANDIN_LOOT`] table: one player head filled from `this`.
pub fn loot_table() -> serde_json::Value {
    serde_json::json!({
        "pools": [{
            "rolls": 1,
            "entries": [{
                "type": "minecraft:item",
                "name": "minecraft:player_head",
                "functions": [{ "function": "minecraft:fill_player_head", "entity": "this" }]
            }]
        }]
    })
}

/// What the stand-in proof looked at.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct StandinGate {
    /// Cutscenes declared `present` (deduplicated by their start function).
    pub present: usize,
    /// Cutscenes declared `absent`.
    pub absent: usize,
}

impl StandinGate {
    /// The binding line, printed on every build that has a cutscene.
    pub fn line(&self) -> String {
        format!(
            "stand-in binding: {} cutscene(s) examined, {} present (stand-ins placed and removed), {} absent (none placed)",
            self.present + self.absent,
            self.present,
            self.absent
        )
    }

    /// The `validation/stand-in-gate.json` body.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "spec": "spec-0095",
            "code": DW_STANDIN_LIFETIME.id(),
            "cutscenes": self.present + self.absent,
            "present": self.present,
            "absent": self.absent,
        })
    }
}

fn fn_body<'a>(out: &'a BuildOutput, ns: &str, name: &str) -> Option<&'a str> {
    out.get(&format!("datapack/data/{ns}/function/{name}.mcfunction"))
        .and_then(|b| std::str::from_utf8(b).ok())
}

fn refuse(message: String) -> BuildFailure {
    BuildFailure::Diagnostic {
        code: DW_STANDIN_LIFETIME,
        message,
    }
}

/// `DW0971` over the shipped tree. `declared` is every cutscene's start
/// function name (`cs_<bare>`) with its declared party, from the DSL.
pub fn check(
    ns: &str,
    declared: &[(String, CutsceneParty)],
    out: &BuildOutput,
) -> Result<StandinGate, BuildFailure> {
    let mut gate = StandinGate::default();
    let call = format!("function {ns}:{STANDIN_FN}");
    for (start, party) in declared {
        let bare = start.strip_prefix("cs_").unwrap_or(start);
        let tag = cutscene_tag(bare);
        let Some(body) = fn_body(out, ns, start) else {
            return Err(refuse(format!(
                "the cutscene `{start}` is declared but its start function is not in the shipped datapack."
            )));
        };
        let lines: Vec<&str> = body.lines().collect();
        let placed = lines.iter().position(|l| l.contains(&call));
        match party {
            CutsceneParty::Absent => {
                gate.absent += 1;
                if placed.is_some() {
                    return Err(refuse(format!(
                        "the cutscene `{start}` is declared `party: absent` and its start places stand-ins."
                    )));
                }
            }
            CutsceneParty::Present => {
                gate.present += 1;
                let Some(placed) = placed else {
                    return Err(refuse(format!(
                        "the cutscene `{start}` is declared `party: present` (the default) and its start places no stand-in: the party would be missing from every shot."
                    )));
                };
                let spectator = lines
                    .iter()
                    .position(|l| l.starts_with("gamemode spectator"));
                if spectator.is_none_or(|s| s < placed) {
                    return Err(refuse(format!(
                        "the cutscene `{start}` places its stand-ins after the party is put in spectator (or never puts it there): each stand-in must be summoned from a body still in the world."
                    )));
                }
                let claimed = lines
                    .iter()
                    .any(|l| l.starts_with("tag @e[") && l.ends_with(&format!(" add {tag}")));
                if !claimed {
                    return Err(refuse(format!(
                        "the cutscene `{start}` places stand-ins without claiming them as `{tag}`, so its end cannot remove them."
                    )));
                }
                let end = fn_body(out, ns, &format!("cs_end_{bare}")).unwrap_or("");
                let removed = end
                    .lines()
                    .any(|l| l.contains(&format!("@e[tag={tag}]")) && l.contains(" tp @s ~ "));
                if !removed {
                    return Err(refuse(format!(
                        "the cutscene `{start}` places stand-ins (`{tag}`) and `cs_end_{bare}` does not remove them: they would outlive the cutscene."
                    )));
                }
            }
        }
    }
    Ok(gate)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(start: &[String], end: &[String]) -> BuildOutput {
        let mut out = BuildOutput::new();
        out.insert(
            "datapack/data/ns/function/cs_x.mcfunction".to_string(),
            start.join("\n").into_bytes(),
        );
        out.insert(
            "datapack/data/ns/function/cs_end_x.mcfunction".to_string(),
            end.join("\n").into_bytes(),
        );
        out
    }

    fn good_start() -> Vec<String> {
        let mut s = start_lines("ns", "x", "dw_cutscene");
        s.push("tag @a add dw_cutscene".to_string());
        s.push("gamemode spectator @a".to_string());
        s
    }

    fn good_end() -> Vec<String> {
        vec![
            "gamemode adventure @a".to_string(),
            "execute as @e[tag=dw_standin_x] at @s run tp @s ~ -128 ~".to_string(),
        ]
    }

    fn present() -> Vec<(String, CutsceneParty)> {
        vec![("cs_x".to_string(), CutsceneParty::Present)]
    }

    #[test]
    fn a_correct_bracket_is_green_and_counted() {
        let gate = check("ns", &present(), &tree(&good_start(), &good_end())).unwrap();
        assert_eq!((gate.present, gate.absent), (1, 0));
        assert!(gate.line().contains("1 present"));
    }

    fn code(r: Result<StandinGate, BuildFailure>) -> String {
        match r {
            Err(BuildFailure::Diagnostic { code, .. }) => code.id().to_string(),
            other => panic!("expected a diagnostic, got {other:?}"),
        }
    }

    #[test]
    fn a_stand_in_its_end_does_not_remove_is_dw0971() {
        let end = vec!["gamemode adventure @a".to_string()];
        assert_eq!(
            code(check("ns", &present(), &tree(&good_start(), &end))),
            "DW0971"
        );
    }

    #[test]
    fn a_present_cutscene_with_no_stand_in_is_dw0971() {
        let start = vec!["gamemode spectator @a".to_string()];
        assert_eq!(
            code(check("ns", &present(), &tree(&start, &good_end()))),
            "DW0971"
        );
    }

    #[test]
    fn a_stand_in_placed_after_spectator_is_dw0971() {
        let mut start = vec!["gamemode spectator @a".to_string()];
        start.extend(start_lines("ns", "x", "dw_cutscene"));
        assert_eq!(
            code(check("ns", &present(), &tree(&start, &good_end()))),
            "DW0971"
        );
    }

    #[test]
    fn an_absent_cutscene_that_places_stand_ins_is_dw0971() {
        let absent = vec![("cs_x".to_string(), CutsceneParty::Absent)];
        assert_eq!(
            code(check("ns", &absent, &tree(&good_start(), &good_end()))),
            "DW0971"
        );
        let start = vec!["gamemode spectator @a".to_string()];
        let gate = check("ns", &absent, &tree(&start, &[])).unwrap();
        assert_eq!((gate.present, gate.absent), (0, 1));
    }

    #[test]
    fn the_stand_in_copies_every_slot_and_the_profile() {
        let body = standin_fn_body("ns").join("\n");
        for slot in SLOTS {
            assert!(
                body.contains(&format!("{slot} from entity @s {slot}")),
                "{slot}"
            );
        }
        assert!(body.contains("loot replace entity") && body.contains("ns:standin_profile"));
        assert!(body.contains("profile set from entity"));
        // The profile is read off the head BEFORE the head slot is overwritten.
        let profile = body.find("profile set from").unwrap();
        let head = body.find("armor.head from entity @s").unwrap();
        assert!(profile < head);
    }
}
