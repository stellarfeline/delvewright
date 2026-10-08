use super::*;

/// spec-0031 PackTests: one template per resolved lethal volume, each of which
/// **puts an entity in the volume and asserts the volume kills it**.
///
/// A compile-time proof that a box is impassable proves nothing about a box that
/// kills; the two halves are independent, and a green over the first alone is the
/// vacuous pass CLAUDE.md names. So the runtime half is bound per volume, not once
/// per campaign: `validation/lethal-gate.json` reports the template count beside
/// the volume count, and a campaign with N volumes and fewer than N templates is
/// legible as such without re-deriving it.
///
/// The body drives the volume's real generated function on a summoned NoAI dummy
/// and asserts its `Health` reached zero. Zero-health rather than
/// "no entity matches": a mob killed by `/damage` plays its death animation for
/// ~20 ticks before vanilla removes it, so `unless entity` would be asserting the
/// scheduler rather than the kill. `/damage` is synchronous, so the whole claim
/// lands in one tick.
///
/// The dummy is a mob, which is exactly the line under test — the volume's
/// non-player selector, minus [`LETHAL_EXEMPT_TYPES`]. The player half runs
/// through the same `/damage` on a per-player re-bind and is asserted by the
/// compiler unit tests, which read the emitted command text directly (PackTest's
/// framework dummies are not a substitute for a real player here).
/// spec-0086 PackTests: one pair per loop, in the shape of `lethal_<id>` /
/// `lethal_<id>_claim`, each red for its own reason.
///
/// * `loop_<id>` opens the loop's gate, puts a NoAI dummy at the slab's anchor
///   cell, drives the loop's own poll once — the line the tick runs — and asserts
///   the dummy moved by exactly the offset, read at ×1000 off its `Pos`, and that
///   the count rose by one. Stripping the `tp` reds it.
/// * `loop_<id>_released` shuts the gate on one of its own terms, drives the same
///   poll, and asserts the dummy did not move and the count did not rise.
///   Stripping the gate guard from the poll reds it.
///
/// Both are synchronous — no `await` — so each runs as one uninterrupted
/// function on the shared batch server, and the two never see each other's gate;
/// each puts back every score it wrote ([`restoring_what_it_writes`]), so no
/// other test of the batch sees it either.
pub(super) fn emit_loop_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    for l in &plan.loops {
        let Some(decl) = loop_decl(plan, l) else {
            continue;
        };
        let tag = format!("dw_looptest_{}", l.safe);
        let sel = format!("@e[tag={tag},limit=1]");
        let at = l.cross();
        let summon = format!(
            "summon minecraft:zombie {} {} {} \
             {{Tags:[\"{tag}\"],NoAI:1b,Silent:1b,PersistenceRequired:1b,Invulnerable:1b}}",
            f64::from(at[0]) + 0.5,
            at[1],
            f64::from(at[2]) + 0.5
        );
        let count = l.counts.as_ref().map(|c| plan::state_score(c));
        let read = |t: &mut Vec<String>, phase: &str| {
            for (i, axis) in ["x", "y", "z"].iter().enumerate() {
                t.push(format!(
                    "execute store result score #lp_{axis}{phase}_{} dw.sys run data get \
                     entity {sel} Pos[{i}] 1000",
                    l.safe
                ));
            }
        };
        let delta = |t: &mut Vec<String>, want: [i32; 3]| {
            for (i, axis) in ["x", "y", "z"].iter().enumerate() {
                let k = &l.safe;
                t.push(format!(
                    "scoreboard players operation #lp_{axis}1_{k} dw.sys -= #lp_{axis}0_{k} dw.sys"
                ));
                t.push(format!(
                    "assert score #lp_{axis}1_{k} dw.sys matches {}",
                    want[i] * 1000
                ));
            }
        };
        let open: Vec<String> = decl
            .requires_flags
            .iter()
            .map(|f| {
                format!(
                    "scoreboard players set {} {} 1",
                    plan::PARTY,
                    plan::flag_score(f.as_str())
                )
            })
            .chain(decl.forbids_flags.iter().map(|f| {
                format!(
                    "scoreboard players reset {} {}",
                    plan::PARTY,
                    plan::flag_score(f.as_str())
                )
            }))
            .chain(state_drive_lines(plan, &decl.requires_state, true))
            .collect();

        // --- the loop holds and moves the body by exactly its offset ---
        let mut t = packtest_header(&format!(
            "{title}: loop `{}` moves a body in its slab by exactly its offset (spec-0086)",
            l.id
        ));
        t.push(format!("function {ns}:setup"));
        t.push(format!("kill @e[tag={tag}]"));
        t.extend(open.iter().cloned());
        if let Some(c) = &count {
            t.push(format!(
                "execute store result score #lp_n0_{} dw.sys run scoreboard players get {} {c}",
                l.safe,
                plan::PARTY
            ));
        }
        t.push(summon.clone());
        // Bound, not assumed: the dummy really is in the poll's own box.
        t.push(format!(
            "execute store result score #lp_in_{} dw.sys if entity @e[tag={tag},{}]",
            l.safe,
            box_selector_args(l.slab.0, l.slab.1)
        ));
        t.push(format!("assert score #lp_in_{} dw.sys matches 1", l.safe));
        read(&mut t, "0");
        t.push(format!("function {ns}:loop_{}_poll", l.safe));
        read(&mut t, "1");
        delta(&mut t, l.offset);
        if let Some(c) = &count {
            t.push(format!(
                "execute store result score #lp_n1_{} dw.sys run scoreboard players get {} {c}",
                l.safe,
                plan::PARTY
            ));
            t.push(format!(
                "scoreboard players operation #lp_n1_{k} dw.sys -= #lp_n0_{k} dw.sys",
                k = l.safe
            ));
            t.push(format!("assert score #lp_n1_{} dw.sys matches 1", l.safe));
        }
        t.push(format!("kill @e[tag={tag}]"));

        // --- the loop stood down moves nothing ---
        let shut: Vec<String> = if let Some(f) = decl.forbids_flags.first() {
            vec![format!(
                "scoreboard players set {} {} 1",
                plan::PARTY,
                plan::flag_score(f.as_str())
            )]
        } else if let Some(f) = decl.requires_flags.first() {
            vec![format!(
                "scoreboard players reset {} {}",
                plan::PARTY,
                plan::flag_score(f.as_str())
            )]
        } else {
            state_drive_lines(plan, decl.requires_state.get(..1).unwrap_or(&[]), false)
        };
        let mut r = packtest_header(&format!(
            "{title}: loop `{}` stood down moves nothing (spec-0086)",
            l.id
        ));
        r.push(format!("function {ns}:setup"));
        r.push(format!("kill @e[tag={tag}]"));
        r.extend(open);
        r.extend(shut);
        let before = match &count {
            Some(c) => {
                r.push(format!(
                    "execute store result score #lp_n0_{} dw.sys run scoreboard players get {} {c}",
                    l.safe,
                    plan::PARTY
                ));
                true
            }
            None => false,
        };
        r.push(summon);
        r.push(format!(
            "execute store result score #lp_in_{} dw.sys if entity @e[tag={tag},{}]",
            l.safe,
            box_selector_args(l.slab.0, l.slab.1)
        ));
        r.push(format!("assert score #lp_in_{} dw.sys matches 1", l.safe));
        read(&mut r, "0");
        r.push(format!("function {ns}:loop_{}_poll", l.safe));
        read(&mut r, "1");
        delta(&mut r, [0, 0, 0]);
        if before && let Some(c) = &count {
            r.push(format!(
                "execute store result score #lp_n1_{} dw.sys run scoreboard players get {} {c}",
                l.safe,
                plan::PARTY
            ));
            r.push(format!(
                "scoreboard players operation #lp_n1_{k} dw.sys -= #lp_n0_{k} dw.sys",
                k = l.safe
            ));
            r.push(format!("assert score #lp_n1_{} dw.sys matches 0", l.safe));
        }
        r.push(format!("kill @e[tag={tag}]"));
        let counted: Vec<(String, String)> = count
            .iter()
            .map(|c| (plan::PARTY.to_string(), c.clone()))
            .collect();
        let t = restoring_what_it_writes(t, &counted, &format!("{}_m", l.safe));
        let r = restoring_what_it_writes(r, &counted, &format!("{}_r", l.safe));
        out.insert(
            format!(
                "packtest-datapack/data/{ns}/test/loop_{}.mcfunction",
                l.safe
            ),
            lines(&t).into_bytes(),
        );
        out.insert(
            format!(
                "packtest-datapack/data/{ns}/test/loop_{}_released.mcfunction",
                l.safe
            ),
            lines(&r).into_bytes(),
        );
    }
}

/// **A synchronous PackTest leaves the shared batch server as it found it.**
/// Every test of a batch runs on one server, and a test with no `await` runs
/// whole inside one tick, so a score it writes and leaves is read by every test
/// that runs after it in that tick: a loop test that shut its gate by sealing
/// the hall left the hall sealed, and the shop test after it was refused its
/// purchase. So every score `body` writes through `scoreboard players
/// set|reset <holder> <objective>` (the compiler's own `#lp_` scratch
/// excepted), and each of `also` (written by a function the test calls), is
/// saved right after the test's `setup` — its value, or that it was unset — and
/// put back as the test's last act. A test that fails an assertion stops
/// before the restore, as a failed test may.
fn restoring_what_it_writes(
    body: Vec<String>,
    also: &[(String, String)],
    key: &str,
) -> Vec<String> {
    let mut written: Vec<(String, String)> = Vec::new();
    for line in &body {
        let w: Vec<&str> = line.split_whitespace().collect();
        if let [
            "scoreboard",
            "players",
            "set" | "reset",
            holder,
            objective,
            ..,
        ] = w.as_slice()
            && !holder.starts_with("#lp_")
        {
            let pair = (holder.to_string(), objective.to_string());
            if !written.contains(&pair) {
                written.push(pair);
            }
        }
    }
    for pair in also {
        if !written.contains(pair) {
            written.push(pair.clone());
        }
    }
    let any = "-2147483648..2147483647";
    let at = body
        .iter()
        .position(|l| l.starts_with("function ") && l.ends_with(":setup"))
        .map_or(0, |i| i + 1);
    let mut out: Vec<String> = body[..at].to_vec();
    for (i, (holder, objective)) in written.iter().enumerate() {
        out.push(format!("scoreboard players reset #lp_sv{i}_{key} dw.sys"));
        out.push(format!(
            "execute if score {holder} {objective} matches {any} run scoreboard players operation #lp_sv{i}_{key} dw.sys = {holder} {objective}"
        ));
    }
    out.extend_from_slice(&body[at..]);
    for (i, (holder, objective)) in written.iter().enumerate() {
        out.push(format!("scoreboard players reset {holder} {objective}"));
        out.push(format!(
            "execute if score #lp_sv{i}_{key} dw.sys matches {any} run scoreboard players operation {holder} {objective} = #lp_sv{i}_{key} dw.sys"
        ));
    }
    out
}
