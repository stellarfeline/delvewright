//! spec-0133 §8: the arithmetic of every limit, proven on the pinned server.
//!
//! Per limited objective, two templates. `limit_<obj>_expires` opens the
//! objective with the engine's own functions — its quest active, its `after`
//! ancestors settled, its required flags held — runs the function its clock
//! starts in, steps `ticks + 1` ticks of the real server, and asserts the
//! objective expired, not done, the first flag its `on_expire` writes held, and
//! every driver of the objective closed. `limit_<obj>_done` opens it the same
//! way, completes it before the window ends, steps the same `ticks + 1`, and
//! asserts it done and never expired.
//!
//! The two share the objective's party scores, and each waits on the server's
//! own clock, so each runs alone in its own test environment: no sibling's
//! template can complete or expire the objective under it.

use super::*;

/// The function the clock of `o` (in quest `q`) starts in, and the latch that
/// keeps it from running twice.
fn start_of(q: &delvewright_dsl::Quest, o: &Objective) -> Option<(String, String)> {
    let l = o.limit()?;
    Some(match &l.from {
        delvewright_dsl::LimitFrom::Activation => (
            format!("activate_{}", safe_obj_fn(o.id().as_str())),
            activation_flag(o.id().as_str()),
        ),
        delvewright_dsl::LimitFrom::Objective(s) => (
            format!("activate_{}", safe_obj_fn(s.as_str())),
            activation_flag(s.as_str()),
        ),
        delvewright_dsl::LimitFrom::QuestStart => (
            quest_start_fn(q.id.as_str()),
            quest_start_latch(q.id.as_str()),
        ),
    })
}

/// Every objective `id` is transitively `after`, in `q`.
fn ancestors<'a>(q: &'a delvewright_dsl::Quest, id: &str) -> Vec<&'a Objective> {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut stack = vec![id];
    while let Some(cur) = stack.pop() {
        if let Some(o) = q.objectives.iter().find(|o| o.id().as_str() == cur) {
            // A clock starts at an objective that certainly comes first.
            for a in o.after_required() {
                if seen.insert(a.as_str()) {
                    stack.push(a.as_str());
                }
            }
        }
    }
    q.objectives
        .iter()
        .filter(|o| seen.contains(o.id().as_str()))
        .collect()
}

/// The flags the first `set-flag` of `on_expire` writes, at any depth: the
/// producer the expiry template reads.
fn first_flag(effs: &[QuestEffect]) -> Option<String> {
    for e in effs {
        if let Some(f) = e.set_flag() {
            return Some(f.as_str().to_string());
        }
        for list in e.nested_effect_lists() {
            if let Some(f) = first_flag(list) {
                return Some(f);
            }
        }
    }
    None
}

/// The lines that open `o` to its pending state from a clean slate, and the
/// lines that put the slate back.
///
/// Every score the objective's guard reads is written here: its quest, its
/// `after` ancestors, its settlement scores, and its whole [`Gate`] — flags
/// and numeric terms alike, through [`packtest_gate_drive`], the one authority
/// that opens a gate in a template — so no kind's template reads a term a
/// sibling left behind (`DW0807`).
fn open_and_close(
    plan: &Plan,
    q: &delvewright_dsl::Quest,
    o: &Objective,
    latch: &str,
) -> (Vec<String>, Vec<String>) {
    let c = plan.campaign;
    let p = plan::PARTY;
    let oid = o.id().as_str();
    let qa = quest_active_score(q.id.as_str());
    let mut open = vec![
        format!("scoreboard players set {p} {} 0", obj_score(oid)),
        format!("scoreboard players set {p} {} 0", expired_score(oid)),
        format!("scoreboard players set {p} {} 0", settled_score(oid)),
        format!(
            "scoreboard players set {p} {} 0",
            quest_score(q.id.as_str())
        ),
        format!("scoreboard players set {latch} dw.sys 0"),
        format!("scoreboard players set {} dw.sys 0", ran_out_latch(oid)),
    ];
    let mut close = vec![
        format!("scoreboard players set {p} {qa} 0"),
        format!("scoreboard players set {p} {} 0", obj_score(oid)),
        format!("scoreboard players set {p} {} 0", expired_score(oid)),
        format!("scoreboard players set {p} {} 0", settled_score(oid)),
        format!(
            "scoreboard players set {p} {} 0",
            quest_score(q.id.as_str())
        ),
        format!("scoreboard players set {latch} dw.sys 0"),
        format!("scoreboard players set {} dw.sys 0", ran_out_latch(oid)),
    ];
    for a in ancestors(q, oid) {
        open.push(format!(
            "scoreboard players set {p} {} 1",
            after_score(c, a.id().as_str())
        ));
        close.push(format!(
            "scoreboard players set {p} {} 0",
            after_score(c, a.id().as_str())
        ));
    }
    open.extend(packtest_gate_drive(plan, o.gate(), true));
    for f in o.requires_flags() {
        close.push(format!(
            "scoreboard players set {p} {} 0",
            plan::flag_score(f.as_str())
        ));
    }
    for cmp in o.requires_state() {
        close.push(format!(
            "scoreboard players set {} {} {}",
            super::super::state::state_holder(plan, &cmp.state),
            plan::state_score(cmp.state.as_str()),
            super::super::state::state_initial(plan, &cmp.state)
        ));
    }
    open.push(format!("scoreboard players set {p} {qa} 1"));
    (open, close)
}

/// The test-environment directive and its document that run a template
/// alone.
fn alone(ns: &str, name: &str, out: &mut BuildOutput) -> String {
    let mut env = serde_json::to_vec_pretty(&json!({
        "type": "minecraft:all_of",
        "definitions": [],
    }))
    .expect("json serializes");
    env.push(b'\n');
    out.insert(
        format!("packtest-datapack/data/{ns}/test_environment/solo/{name}.json"),
        env,
    );
    format!("# @environment {ns}:solo/{name}")
}

/// Emit the two templates of every limit (spec-0133 §8). Nothing for a
/// campaign with no limit.
pub(super) fn emit_limit_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let title = artifact_title(c);
    for q in &c.quests.content.quests {
        let qa = quest_active_score(q.id.as_str());
        for o in &q.objectives {
            let Some(l) = o.limit() else { continue };
            let Some((start, latch)) = start_of(q, o) else {
                continue;
            };
            let oid = o.id().as_str();
            let safe = plan::safe_local(oid);
            let step = l.ticks + 1;
            let (open, close) = open_and_close(plan, q, o, &latch);
            let expired = expired_score(oid);
            let done_score = obj_score(oid);

            // -- expires --------------------------------------------------
            let name = format!("limit_{}_expires", safe_obj_fn(oid));
            let mut b = vec![
                format!("#> {title}: `{oid}` expires when its window runs out (spec-0133 §8)"),
                alone(ns, &name, out),
                "# @dummy".to_string(),
                format!("# @timeout {}", step + 100),
                String::new(),
                format!("function {ns}:setup"),
            ];
            b.extend(open.iter().cloned());
            b.push(format!("function {ns}:{start}"));
            b.push(format!("await delay {step}t"));
            b.push(format!("assert score {} {expired} matches 1", plan::PARTY));
            b.push(format!(
                "execute store success score #lim_done_{safe} dw.sys if score {} {done_score} matches 1",
                plan::PARTY
            ));
            b.push(format!("assert score #lim_done_{safe} dw.sys matches 0"));
            let flag = first_flag(l.on_expire());
            if let Some(f) = &flag {
                b.push(format!(
                    "assert score {} {} matches 1",
                    plan::PARTY,
                    plan::flag_score(f)
                ));
            }
            // Every driver of the objective is closed: its guard no longer holds,
            // with its gate held open again in this same tick — a census the
            // engine rewrote, or a datum `on_expire` moved, cannot close it in
            // the expired term's place.
            b.extend(packtest_gate_drive(plan, o.gate(), true));
            b.push(format!(
                "execute store success score #lim_open_{safe} dw.sys{}",
                pending_guard(plan, o, &qa)
            ));
            b.push(format!("assert score #lim_open_{safe} dw.sys matches 0"));
            b.extend(close.iter().cloned());
            if let Some(f) = &flag {
                b.push(format!(
                    "scoreboard players set {} {} 0",
                    plan::PARTY,
                    plan::flag_score(f)
                ));
            }
            out.insert(
                format!("packtest-datapack/data/{ns}/test/{name}.mcfunction"),
                lines(&b).into_bytes(),
            );

            // -- done -----------------------------------------------------
            let name = format!("limit_{}_done", safe_obj_fn(oid));
            let mut b = vec![
                format!(
                    "#> {title}: `{oid}` done inside its window is never expired (spec-0133 §8)"
                ),
                alone(ns, &name, out),
                "# @dummy".to_string(),
                format!("# @timeout {}", step + 100),
                String::new(),
                format!("function {ns}:setup"),
            ];
            b.extend(open.iter().cloned());
            b.push(format!("function {ns}:{start}"));
            b.push(format!("function {ns}:complete_{}", safe_obj_fn(oid)));
            b.push(format!("await delay {step}t"));
            b.push(format!(
                "assert score {} {done_score} matches 1",
                plan::PARTY
            ));
            b.push(format!(
                "execute store success score #lim_x_{safe} dw.sys if score {} {expired} matches 1",
                plan::PARTY
            ));
            b.push(format!("assert score #lim_x_{safe} dw.sys matches 0"));
            b.extend(close.iter().cloned());
            out.insert(
                format!("packtest-datapack/data/{ns}/test/{name}.mcfunction"),
                lines(&b).into_bytes(),
            );
        }
    }
}
