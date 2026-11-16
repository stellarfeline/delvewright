//! The party is every member, present or not (spec-0141 §4).
//!
//! A delve is played by one to four players, and nothing the DSL declares names
//! how many. Two shapes in a shipped datapack quietly assume the answer is one
//! player who is always there:
//!
//! * **A lasting write onto every present player.** `give @a`, `tag @a add`,
//!   `spawnpoint @a`, `bossbar set … players @a`: each reaches the players
//!   online on the tick it runs. A member who was disconnected on that tick, or
//!   who joins the delve after it, is left without the state the rest of the
//!   party carries from then on.
//! * **A pick of one player.** `@p`, `@r`, a player selector with `limit=`:
//!   with one player the pick is the party; with several it is a choice the
//!   emitter made for the creator, and it needs a rule.
//!
//! [`census`] reads the shipped datapack and puts every such line in one of two
//! states: it stands at a site [`SERVED`] (a write) or [`PICKS`] (a pick) names,
//! with what reaches an absent member or which player is picked and why; or it
//! is unnamed, which refuses the build (`DW1126`, `DW1127`). It runs on every
//! build and is feature-blind: an emitter written later is judged by existing.
//!
//! What it does not read: a line the root `tick` holds (it is restated every
//! tick, so a member who arrives is reached on their first), a line that tells
//! the present what happened (`tellraw`, `title`, `playsound`, `stopsound`,
//! `particle`, `dialog` — a beat is told to those who were there), and the body
//! of a function dispatched `as @a` (counted, [`AbsentCensus::dispatched`]).

use std::collections::BTreeMap;

use delvewright_dsl::{DwCode, ExitTier};

use crate::compiler::affordance::{matching_bracket, selector_terms, shipped_functions};
use crate::compiler::emit::BuildOutput;

delvewright_dsl::dw_code! {
    /// `DW1126`: a shipped command writes lasting state onto every present
    /// player and nothing names what reaches a member who was absent. An
    /// **engine self-check**: the campaign cannot cause or repair it.
    pub const DW_ABSENT_UNSERVED: DwCode = DwCode::new("DW1126", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW1127`: a shipped command picks one player among several and nothing
    /// names which and why. An **engine self-check**.
    pub const DW_PICK_UNNAMED: DwCode = DwCode::new("DW1127", ExitTier::Build);
}

/// A coded refusal of [`check`].
#[derive(Debug, Clone)]
pub struct AbsentRefusal {
    /// [`DW_ABSENT_UNSERVED`] or [`DW_PICK_UNNAMED`].
    pub code: DwCode,
    /// What was found, and that it is the engine's to fix.
    pub message: String,
}

/// A named site: the lines that stand at it, and what the name asserts.
pub struct Site {
    /// The site's name, as the ledger counts it.
    pub site: &'static str,
    /// For a write: what reaches a member who was absent. For a pick: which
    /// player is picked among several, and why that is right for any party.
    pub rule: &'static str,
    /// Does this (function name, command line) stand at the site?
    pub matches: fn(&str, &str) -> bool,
}

/// The commands whose effect on a player outlasts the tick they run on, by the
/// first word of the pinned command tree.
pub const LASTING: &[&str] = &[
    "advancement",
    "attribute",
    "bossbar",
    "clear",
    "damage",
    "effect",
    "experience",
    "gamemode",
    "give",
    "item",
    "kill",
    "recipe",
    "ride",
    "scoreboard",
    "spawnpoint",
    "spectate",
    "tag",
    "team",
    "teleport",
    "tp",
    "xp",
];

/// The sites where a lasting write onto every present player is served to a
/// member who was absent.
pub const SERVED: &[Site] = &[
    Site {
        site: "grant",
        rule: "`grant_<i>` counts each issue on the party and on every player it reached; a \
               member whose count is behind is handed the stack by the tick's owed line \
               (`emit::grant`)",
        matches: |name, _| {
            name.strip_prefix("grant_")
                .is_some_and(|i| i.bytes().all(|b| b.is_ascii_digit()))
        },
    },
    Site {
        site: "checkpoint",
        rule: "`spawnpoint @a` is a hint vanilla may drop; the respawn edge seats every \
               respawning player on the active checkpoint (`cp_seat_<i>`, read off `#cp`), and \
               a player who takes a class once one is armed arrives on it — so a member absent \
               at the beat is seated where the party is",
        matches: |_, line| command(line).starts_with("spawnpoint @a "),
    },
    Site {
        site: "cutscene bracket",
        rule: "a cutscene takes every present player into the observation state and returns \
               them; a member who left inside the bracket is repaired on their first tick back \
               (`cs_repair`), and one who was never in it has nothing to restore",
        matches: |name, _| name.starts_with("cs_") || name.starts_with("rw_"),
    },
    Site {
        site: "refresh",
        rule: "a line of a function that runs on every tick or every refresh of a live \
               surface (a health bar's audience, a pulse's listeners, a highlight's team, an \
               area's mitigation, a boundary's return, the guidance refresh): a member who \
               arrives is reached on the next one",
        matches: |name, _| {
            name == "setup"
                || name == "setup_finish"
                || name.starts_with("hb_")
                || name.starts_with("pulse_")
                || name.starts_with("hl_")
                || name.starts_with("guide_")
                || name.starts_with("compass_")
                || name.starts_with("journal_")
                || name.starts_with("boundary")
                || name.starts_with("area_")
                || name.starts_with("mitigat")
                || name.starts_with("stealth_")
                || name.starts_with("lethal_")
                || name.starts_with("limit_show_")
                || name.starts_with("race_")
                || name.starts_with("census_")
                || name.starts_with("class_")
                || name.starts_with("clock")
                || name.starts_with("atmos")
        },
    },
    Site {
        site: "hazard",
        rule: "`damage` and `kill` strike the bodies present; a member who is not in the world \
               has no body to strike",
        matches: |_, line| {
            let c = command(line);
            c.starts_with("damage ") || c.starts_with("kill ")
        },
    },
    Site {
        site: "status effect",
        rule: "a `give-effect` / `clear-effect` beat changes what the bodies present feel for \
               its duration; it is a beat, told to those who were there",
        matches: |_, line| command(line).starts_with("effect "),
    },
    Site {
        site: "advancement",
        rule: "an objective's or the campaign's advancement is a toast for the players \
               present; progress is the party's (`#party`) and is not read off it",
        matches: |_, line| command(line).starts_with("advancement grant @a"),
    },
    Site {
        site: "party reset",
        rule: "a start that clears every present player's entry of a per-run score or tag (a \
               race's, a board's, a stealth beat's grace): an absent member holds no live entry \
               of that run, and one who arrives is given theirs by the surface's own tick",
        matches: |_, line| {
            let c = command(line);
            c.starts_with("scoreboard players reset @a")
                || (c.starts_with("tag @a") && c.contains(" remove "))
                || c.starts_with("scoreboard players set @a")
        },
    },
    Site {
        site: "limit countdown",
        rule: "KNOWN GAP (spec-0141 §6): a limited objective's countdown bar is shown to the \
               players present when its clock starts; a member who joins the delve later does \
               not see it. The clock, its expiry and its telling are the party's",
        matches: |_, line| {
            let c = command(line);
            c.starts_with("bossbar set ") && c.ends_with(" players @a")
        },
    },
];

/// The sites where one player is picked among several.
pub const PICKS: &[Site] = &[
    Site {
        site: "tagged body",
        rule: "a selector narrowed by a positive `tag=` or a `name=` and `limit=1` names one \
               known body (a template's own dummy, a blow's marked striker, a strike's chosen \
               target), not a choice among the party",
        matches: |_, line| {
            player_selectors(line).iter().any(|(k, args)| {
                *k == 'a'
                    && limited(args)
                    && selector_terms(args).any(|t| {
                        (t.starts_with("tag=") && !t.starts_with("tag=!")) || t.starts_with("name=")
                    })
            })
        },
    },
    Site {
        site: "watcher's subject",
        rule: "a body that watches turns to the nearest player in play: each watcher looks at \
               whoever is closest to it, which is what a party standing around it sees",
        matches: |_, line| line.contains("rotate @s facing entity @p["),
    },
    Site {
        site: "cutscene return mark",
        rule: "the fallback place of a player with no saved place of their own (one who \
               joined while the cutscene played) is beside a player in play: the party, not the \
               entry point",
        matches: |name, line| name.starts_with("cs_") && line.contains("execute at @p"),
    },
    Site {
        site: "respawn-wait camera",
        rule: "a player waiting out a respawn watches the nearest living teammate in play",
        matches: |name, _| name.starts_with("rw_"),
    },
    Site {
        site: "ladder staged blow",
        rule: "`wave_strike_*` / `wave_chip_*` are the bot ladder's staging, run by rcon and by \
               no campaign machinery; `by @p` only names the attacker a kill is credited to",
        matches: |name, line| {
            (name.starts_with("wave_strike_") || name.starts_with("wave_chip_"))
                && line.contains("minecraft:player_attack by @p")
        },
    },
    Site {
        site: "guide's nearest",
        rule: "a compass or a highlight aimed at a moving body aims at the one nearest the \
               player it is drawn for (`@s`), per player",
        matches: |name, _| {
            name.starts_with("guide_") || name.starts_with("compass_") || name.starts_with("hl_")
        },
    },
    Site {
        site: "strike's target",
        rule: "an assembly's aimed blow picks its target by the rule its strike declares \
               (`aim`: nearest or furthest), among the players in play",
        matches: |name, _| name.starts_with("asm_"),
    },
];

/// The final command of a line: what follows the last ` run ` of an `execute`,
/// else the line itself.
pub fn command(line: &str) -> &str {
    let line = line.trim();
    if !line.starts_with("execute ") {
        return line;
    }
    match line.rfind(" run ") {
        Some(i) => &line[i + 5..],
        None => "",
    }
}

/// Whether a selector's arguments limit how many it selects.
fn limited(args: &str) -> bool {
    selector_terms(args).any(|t| t.starts_with("limit="))
}

/// Every selector `line` holds as a word of its own, as `(kind, args)`. A
/// selector is read only outside every bracket, brace and quoted string, so
/// `@p` in a narrated line, in an item's `custom_name` or in a summon's NBT is
/// text and not a selector.
pub fn selectors(line: &str) -> Vec<(char, String)> {
    let mut out = Vec::new();
    let bytes = line.as_bytes();
    let mut depth = 0usize;
    let mut quote: Option<u8> = None;
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = quote {
            if b == b'\\' {
                i += 2;
                continue;
            }
            if b == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        match b {
            b'"' => quote = Some(b'"'),
            // A single quote opens a string only where a value can start.
            b'\'' if i == 0 || matches!(bytes[i - 1], b' ' | b':' | b'=' | b',' | b'[' | b'{') => {
                quote = Some(b'\'')
            }
            b'[' | b'{' => depth += 1,
            b']' | b'}' => depth = depth.saturating_sub(1),
            b'@' if depth == 0 && (i == 0 || bytes[i - 1] == b' ') && i + 1 < bytes.len() => {
                let kind = bytes[i + 1] as char;
                let after = i + 2;
                let word_end = matches!(bytes.get(after), None | Some(b'[') | Some(b' '));
                if matches!(kind, 'a' | 'p' | 'r' | 'e' | 'n' | 's') && word_end {
                    if bytes.get(after) == Some(&b'[') {
                        match matching_bracket(line, after + 1) {
                            Some(close) => {
                                out.push((kind, line[after + 1..close].to_string()));
                                i = close + 1;
                                continue;
                            }
                            None => break,
                        }
                    }
                    out.push((kind, String::new()));
                    i = after;
                    continue;
                }
            }
            _ => {}
        }
        i += 1;
    }
    out
}

/// Every player selector in `line`: `@a`, `@p`, `@r`, and a player-typed
/// `@e`/`@n`.
pub fn player_selectors(line: &str) -> Vec<(char, String)> {
    selectors(line)
        .into_iter()
        .filter(|(kind, args)| {
            matches!(kind, 'a' | 'p' | 'r')
                || (matches!(kind, 'e' | 'n')
                    && selector_terms(args)
                        .any(|t| t == "type=minecraft:player" || t == "type=player"))
        })
        .collect()
}

/// Whether `line` picks one player among several: `@p`, `@r`, a player-typed
/// `@n`, or `@a` / a player-typed `@e` with a `limit=`.
pub fn picks_a_player(line: &str) -> bool {
    player_selectors(line).iter().any(|(k, args)| match k {
        'p' | 'r' | 'n' => true,
        _ => limited(args),
    })
}

/// Whether `line`'s final command writes lasting state onto every present
/// player: a [`LASTING`] command whose first selector is `@a`, or one aimed at
/// `@s` under an `execute … as @a` of the same line.
pub fn writes_every_present_player(line: &str) -> bool {
    let cmd = command(line);
    let Some(verb) = cmd.split(' ').next() else {
        return false;
    };
    if !LASTING.contains(&verb) {
        return false;
    }
    // `scoreboard objectives …` and `bossbar add|remove|set … <not players>`
    // name no player.
    if cmd.starts_with("scoreboard objectives ") {
        return false;
    }
    let first = first_selector(cmd);
    match first {
        Some(('a', args)) => !limited(&args),
        Some(('s', _)) => {
            let head = &line.trim()[..line.trim().len() - cmd.len()];
            head.contains(" as @a ") || head.contains(" as @a[")
        }
        _ => false,
    }
}

/// The first selector word of a command, of any kind, as `(kind, args)`.
fn first_selector(cmd: &str) -> Option<(char, String)> {
    selectors(cmd).into_iter().next()
}

/// The census of the two shapes over a shipped tree.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AbsentCensus {
    /// Shipped functions read.
    pub functions: usize,
    /// Lines of the root `tick` that write every present player (restated
    /// every tick; not judged).
    pub restated: usize,
    /// Lines that dispatch a function `as @a` outside the root `tick` (their
    /// bodies are not read).
    pub dispatched: usize,
    /// Lasting writes onto every present player outside the root `tick`.
    pub writes: usize,
    /// Of those, the ones at a served site, per site.
    pub served: BTreeMap<&'static str, usize>,
    /// Of those, the ones at no site: `(function, line)`.
    pub unserved: Vec<(String, String)>,
    /// Lines that pick one player among several.
    pub picks: usize,
    /// Of those, the ones at a named site, per site.
    pub named: BTreeMap<&'static str, usize>,
    /// Of those, the ones at no site: `(function, line)`.
    pub unnamed: Vec<(String, String)>,
}

impl AbsentCensus {
    /// The ledger written to `validation/absent-census.json`.
    pub fn to_json(&self) -> serde_json::Value {
        let rows = |m: &BTreeMap<&'static str, usize>, table: &[Site]| {
            m.iter()
                .map(|(site, n)| {
                    let rule = table.iter().find(|s| s.site == *site).map(|s| s.rule);
                    serde_json::json!({ "site": site, "count": n, "rule": rule })
                })
                .collect::<Vec<_>>()
        };
        serde_json::json!({
            "functions_read": self.functions,
            "restated_on_tick": self.restated,
            "dispatched_as_every_player": self.dispatched,
            "lasting_writes_onto_every_present_player": self.writes,
            "served": rows(&self.served, SERVED),
            "unserved": self.unserved.len(),
            "picks_of_one_player": self.picks,
            "named": rows(&self.named, PICKS),
            "unnamed": self.unnamed.len(),
        })
    }

    /// The one-line binding statement a build prints.
    pub fn binding(&self) -> String {
        format!(
            "absent-member census (spec-0141): {} lasting write(s) onto every present player \
             over {} shipped function(s), {} served at a named site, {} unserved (DW1126); {} \
             pick(s) of one player, {} named, {} unnamed (DW1127); {} line(s) restated on the \
             tick and {} dispatch(es) as every player not judged.",
            self.writes,
            self.functions,
            self.served.values().sum::<usize>(),
            self.unserved.len(),
            self.picks,
            self.named.values().sum::<usize>(),
            self.unnamed.len(),
            self.restated,
            self.dispatched,
        )
    }
}

/// Read the shipped datapack and sort every write and every pick.
pub fn census(out: &BuildOutput) -> AbsentCensus {
    let mut c = AbsentCensus::default();
    for (name, body) in shipped_functions(out) {
        c.functions += 1;
        // The creator overlay and the rehearsal inventory are tooling.
        if name.starts_with("creator/") {
            continue;
        }
        for line in body.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if picks_a_player(line) {
                c.picks += 1;
                match PICKS.iter().find(|s| (s.matches)(&name, line)) {
                    Some(s) => *c.named.entry(s.site).or_default() += 1,
                    None => c.unnamed.push((name.clone(), line.to_string())),
                }
            }
            if name == "tick" {
                if writes_every_present_player(line) {
                    c.restated += 1;
                }
                continue;
            }
            if command(line).starts_with("function ")
                && (line.contains(" as @a ") || line.contains(" as @a["))
            {
                c.dispatched += 1;
            }
            if writes_every_present_player(line) {
                c.writes += 1;
                match SERVED.iter().find(|s| (s.matches)(&name, line)) {
                    Some(s) => *c.served.entry(s.site).or_default() += 1,
                    None => c.unserved.push((name.clone(), line.to_string())),
                }
            }
        }
    }
    c
}

/// [`census`], refusing a tree with an unserved write (`DW1126`) or an unnamed
/// pick (`DW1127`).
pub fn check(out: &BuildOutput) -> Result<AbsentCensus, AbsentRefusal> {
    let c = census(out);
    if let Some((name, line)) = c.unserved.first() {
        return Err(AbsentRefusal {
            code: DW_ABSENT_UNSERVED,
            message: format!(
                "ENGINE SELF-CHECK FAILED — this is a defect in delvec, not in the campaign; \
                 report it. A delve is played by one to four players who may join late or drop \
                 out, and {} command(s) this build ships write lasting state onto the players \
                 present on one tick with nothing named that reaches a member who was absent. \
                 The first is in `{name}`: `{line}`. The emitter that wrote it must serve the \
                 absent member (issue it as a grant, restate it on a refresh, or repair it at \
                 the member's return), and the site must be named with that rule in \
                 `absent::SERVED`. Nothing in the campaign repairs this.",
                c.unserved.len()
            ),
        });
    }
    if let Some((name, line)) = c.unnamed.first() {
        return Err(AbsentRefusal {
            code: DW_PICK_UNNAMED,
            message: format!(
                "ENGINE SELF-CHECK FAILED — this is a defect in delvec, not in the campaign; \
                 report it. {} command(s) this build ships pick one player among several (`@p`, \
                 `@r`, a `limit=`) with no rule named for a party of two to four. The first is \
                 in `{name}`: `{line}`. The emitter that wrote it must address every player, \
                 the acting player (`@s`) or the party holder, or the site must be named in \
                 `absent::PICKS` with which player is picked and why. Nothing in the campaign \
                 repairs this.",
                c.unnamed.len()
            ),
        });
    }
    Ok(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_final_command_of_an_execute_is_read() {
        assert_eq!(command("give @a minecraft:torch 1"), "give @a minecraft:torch 1");
        assert_eq!(
            command("execute if score #party dw.f_x matches 1 run give @a minecraft:torch 1"),
            "give @a minecraft:torch 1"
        );
        assert_eq!(command("execute as @a if entity @s"), "");
    }

    #[test]
    fn a_lasting_write_onto_every_present_player_is_found() {
        assert!(writes_every_present_player("give @a minecraft:torch 1"));
        assert!(writes_every_present_player("tag @a add dw_x"));
        assert!(writes_every_present_player(
            "execute if score #a dw.sys matches 1 run spawnpoint @a 1 2 3"
        ));
        assert!(writes_every_present_player(
            "bossbar set ns:limit_x players @a"
        ));
        assert!(writes_every_present_player(
            "execute as @a at @s run tag @s add dw_x"
        ));
        assert!(writes_every_present_player(
            "effect give @a[x=1,dx=2] minecraft:speed 5 0 true"
        ));
    }

    #[test]
    fn a_telling_a_holder_write_and_one_player_are_not() {
        assert!(!writes_every_present_player("tellraw @a {\"text\":\"give @a x\"}"));
        assert!(!writes_every_present_player(
            "scoreboard players set #party dw.f_x 1"
        ));
        assert!(!writes_every_present_player("give @s minecraft:torch 1"));
        assert!(!writes_every_present_player(
            "scoreboard objectives add dw.x dummy"
        ));
        assert!(!writes_every_present_player(
            "tag @a[tag=dw_t_x,limit=1] add dw_y"
        ));
        assert!(!writes_every_present_player("tag @e[tag=dw_wave_x] add dw_y"));
    }

    #[test]
    fn a_pick_of_one_player_is_found() {
        assert!(picks_a_player("execute at @p run summon minecraft:marker"));
        assert!(picks_a_player("tp @r 1 2 3"));
        assert!(picks_a_player("tag @a[sort=nearest,limit=1] add x"));
        assert!(picks_a_player(
            "tag @e[type=minecraft:player,limit=1] add x"
        ));
        assert!(!picks_a_player("tag @a add x"));
        assert!(!picks_a_player("tag @e[tag=dw_x,limit=1] add x"));
        assert!(!picks_a_player("tellraw @a {\"text\":\"go to @p now\"}"));
        assert!(!picks_a_player(
            "give @a minecraft:paper[custom_name={\"fallback\":\"Keeper's @p note\"}] 1"
        ));
    }
}
