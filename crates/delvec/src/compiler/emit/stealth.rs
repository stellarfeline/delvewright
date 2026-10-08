//! Stealth beats and the judge audit.

use super::*;

/// The selector arguments a stealth judge's per-player test may use: the box, and
/// nothing else. Sorted, and an allowlist rather than a denylist — see
/// [`DW_STEALTH_JUDGE_NOT_POSITIONAL`].
pub(super) const STEALTH_JUDGE_ARGS: [&str; 6] = ["dx", "dy", "dz", "x", "y", "z"];

/// **What the stealth-judge audit looked at**, so its verdict reads as a
/// measurement rather than a silence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StealthJudgeAudit {
    /// Stealth beats the plan holds — the denominator `judges` must equal.
    pub beats: usize,
    /// `stealth_eval_*` functions found in the emitted set.
    pub judges: usize,
    /// Per-player `if entity @s[…]` tests across all of them. This is the
    /// binding count: zero means no judge asks anything of anybody.
    pub examined: usize,
    /// Every selector argument any of those tests used, sorted and deduped —
    /// printed whether or not it is a finding, so a reader sees the vocabulary
    /// rather than a verdict about it.
    pub arguments: Vec<String>,
    /// The violations: `(function, offending argument)`, sorted.
    pub offenders: Vec<(String, String)>,
    /// True when the judge count and the beat count disagree — this scan was
    /// looking at a smaller world than it claims to cover.
    pub scan_incomplete: bool,
}

/// Audit the emitted stealth judges. Pure over the emitted function list, so the
/// rule is unit-testable against a fabricated judge as well as against a real one.
pub fn audit_stealth_judges(functions: &[(String, String)], beats: usize) -> StealthJudgeAudit {
    let mut judges = 0usize;
    let mut examined = 0usize;
    let mut arguments: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut offenders: std::collections::BTreeSet<(String, String)> =
        std::collections::BTreeSet::new();
    for (name, body) in functions {
        if !name.starts_with("stealth_eval_") {
            continue;
        }
        judges += 1;
        for line in body.lines() {
            let Some(open) = line.find("if entity @s[") else {
                continue;
            };
            let start = open + "if entity @s[".len();
            let Some(len) = line[start..].find(']') else {
                continue;
            };
            examined += 1;
            for arg in line[start..start + len].split(',') {
                let key = arg.split('=').next().unwrap_or("").trim().to_string();
                if key.is_empty() {
                    continue;
                }
                if !STEALTH_JUDGE_ARGS.contains(&key.as_str()) {
                    offenders.insert((name.clone(), key.clone()));
                }
                arguments.insert(key);
            }
        }
    }
    StealthJudgeAudit {
        beats,
        judges,
        examined,
        arguments: arguments.into_iter().collect(),
        offenders: offenders.into_iter().collect(),
        scan_incomplete: judges != beats,
    }
}

impl StealthJudgeAudit {
    /// The `DW0852` violation, or `None`.
    pub fn finding(&self) -> Option<(DwCode, String)> {
        if self.scan_incomplete {
            return Some((
                DW_STEALTH_JUDGE_NOT_POSITIONAL,
                format!(
                    "internal invariant violation: the stealth-judge audit found {} \
                     `stealth_eval_*` function(s) for {} declared stealth beat(s), so it was \
                     examining a smaller world than it claims to cover and its pass would have \
                     meant nothing. Something renamed or dropped the per-player judge. This is a \
                     compiler bug; stop and escalate.",
                    self.judges, self.beats
                ),
            ));
        }
        if self.offenders.is_empty() {
            return None;
        }
        let listing: Vec<String> = self
            .offenders
            .iter()
            .map(|(f, a)| format!("`{f}` uses `{a}`"))
            .collect();
        Some((
            DW_STEALTH_JUDGE_NOT_POSITIONAL,
            format!(
                "a stealth judge asks a player for something other than where they are: {}. \
                 A stealth beat is HIDING, and hiding is a place — presence in a declared zone \
                 is the whole of it, which is what `emit_stealth_functions` has promised since \
                 v0.6, and what a playtester met the other way round: a scene that quietly \
                 required crouching when nothing in the story had asked for it. The per-player \
                 test may use {} and nothing else, and a selector argument outside that set is \
                 a demand made of the player whatever it is spelled. Examined {} per-player \
                 test(s) across {} judge(s) for {} declared beat(s), using {}. If a beat \
                 genuinely needs a posture or an item, that is a DSL surface to propose, not a \
                 predicate to add here — do NOT widen this list.",
                listing.join(", "),
                STEALTH_JUDGE_ARGS.join("/"),
                self.examined,
                self.judges,
                self.beats,
                self.arguments.join(", "),
            ),
        ))
    }

    /// The binding ledger (`validation/stealth-judge.json`), emitted by **every**
    /// campaign — a stealth-less one ships zeroes rather than nothing at all. The
    /// staging gate reads this file as this rule's binding count, and it reports an
    /// absent file as *nobody ran the check*, which is not what "this campaign
    /// fields no stealth" means. A zero here is read against the beats the
    /// campaign declares: zero beats and zero tests is INAPPLICABLE, beats with no
    /// tests is UNBOUND, and both are said out loud.
    pub fn ledger(&self) -> serde_json::Value {
        serde_json::json!({
            "beats": self.beats,
            "judges": self.judges,
            "examined": self.examined,
            "arguments": self.arguments,
            "allowed_arguments": STEALTH_JUDGE_ARGS,
            "offenders": self.offenders.iter().map(|(f, a)| serde_json::json!({"function": f, "argument": a})).collect::<Vec<_>>(),
            "verdict": if self.finding().is_some() { "fail" } else { "pass" },
        })
    }
}

/// Generate the stealth-beat functions (DSL v0.6, spec-0014; no sneak
/// requirement — holding sneak collides with the
/// spectator cutscene camera). For each beat: an `arm` that activates the
/// session and resets per-player grace; a per-tick judge that, per player,
/// tests "inside some zone box" (zone presence alone = hidden), tracks a grace
/// counter, and fires `on_caught` after `grace_ticks` of exposure. Zone
/// membership is a pure position selector, so the whole check is deterministic
/// and provable.
pub(super) fn emit_stealth_functions(plan: &Plan) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut fns: Vec<(String, String)> = Vec::new();
    for beat in &plan.stealth_beats {
        let i = beat.index;
        // stealth_begin_<i>: activate + reset grace.
        fns.push((
            format!("stealth_begin_{i}"),
            lines(&[
                format!("scoreboard players set #stealth dw.sys {i}"),
                "execute as @a run scoreboard players set @s dw.st_grace 0".to_string(),
            ]),
        ));
        // stealth_tick_<i>: judge every player who is actually playing. A player
        // in the cutscene state is skipped entirely (CUTSCENE_TAG): the judge is
        // the only writer of `dw.st_grace`, so skipping it freezes the clock —
        // grace neither accrues nor expires, and `on_caught` cannot fire at a
        // player who is watching a cinematic in spectator mode.
        fns.push((
            format!("stealth_tick_{i}"),
            lines(&[format!(
                "execute as @a[tag=!{CUTSCENE_TAG}] run function {ns}:stealth_eval_{i}"
            )]),
        ));
        // stealth_eval_<i> (as @s): compute safe flag, update grace, fire caught.
        let mut eval: Vec<String> = vec!["scoreboard players set @s dw.st_safe 0".to_string()];
        for (_, pos, extent) in &beat.zones {
            let lo = [
                pos[0] - extent[0] as i32,
                pos[1] - extent[1] as i32,
                pos[2] - extent[2] as i32,
            ];
            let size = [
                2 * extent[0] as i32,
                2 * extent[1] as i32,
                2 * extent[2] as i32,
            ];
            eval.push(format!(
                "execute if entity @s[x={},dx={},y={},dy={},z={},dz={}] run \
                 scoreboard players set @s dw.st_safe 1",
                lo[0], size[0], lo[1], size[1], lo[2], size[2]
            ));
        }
        eval.push(
            "execute if score @s dw.st_safe matches 1 run scoreboard players set @s dw.st_grace 0"
                .to_string(),
        );
        eval.push(
            "execute if score @s dw.st_safe matches 0 run scoreboard players add @s dw.st_grace 1"
                .to_string(),
        );
        eval.push(format!(
            "execute if score @s dw.st_grace matches {}.. run function {ns}:stealth_caught_{i}",
            beat.grace_ticks
        ));
        fns.push((format!("stealth_eval_{i}"), lines(&eval)));
        // stealth_caught_<i> (as @s): reset grace, run on_caught.
        // `Audience::Solo` (spec-0018): being spotted is one player's event —
        // `stealth_eval_<i>` judges each player separately, so the consequence
        // lands on the player it judged.
        let mut caught: Vec<String> = vec!["scoreboard players set @s dw.st_grace 0".to_string()];
        caught.extend(emit_effect_bundle(plan, &beat.on_caught, Audience::Solo));
        fns.push((format!("stealth_caught_{i}"), lines(&caught)));
    }
    fns
}
