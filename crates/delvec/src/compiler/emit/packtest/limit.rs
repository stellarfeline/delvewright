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

/// **What a limit's templates hold still while the clock runs**: the lines
/// after `setup` that take the objective's state out of every hand but the
/// template's, and the lines at the end that give it back.
///
/// - A **census** the gate reads (spec-0134 §1) is written by the engine's
///   `census_tick` every few ticks from the bodies in its box, which `setup`
///   starts: over a window of thousands of ticks it puts the datum back to the
///   bodies there (none), the objective is no longer pending, and its clock
///   settles nothing. The template stops the tick and starts it again after.
/// - A **collect** completes for any player holding its item (the `tick`
///   driver reads `@a`), and a dummy an earlier template handed the item stays
///   on the server after its own test ends: inside this window it completes
///   the objective the template means to see expire. The template takes the
///   item from every player first — it runs alone, so no running sibling is
///   touched (`compiler::solo`).
fn held_still(plan: &Plan, o: &Objective) -> (Vec<String>, Vec<String>) {
    let ns = &plan.namespace;
    let q = &plan.campaign.quests.content;
    let (mut hold, mut give_back) = (Vec::new(), Vec::new());
    let counted = o.requires_state().iter().any(|cmp| {
        q.state_decl(cmp.state.as_str())
            .is_some_and(|d| d.counts.is_some())
    });
    if counted {
        let tick = super::super::census::CENSUS_TICK_FN;
        hold.push(format!("schedule clear {ns}:{tick}"));
        give_back.push(format!(
            "schedule function {ns}:{tick} {}t replace",
            super::super::census::CENSUS_PERIOD_TICKS
        ));
    }
    if let Objective::Collect { item, .. } = o {
        hold.push(format!("clear @a {item}"));
    }
    (hold, give_back)
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
            let (hold, give_back) = held_still(plan, o);
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
            b.extend(hold.iter().cloned());
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
            // with every other term of it held open again in this same tick —
            // the quest the settlement may have completed, the `after`
            // ancestors, the gate (a census the engine rewrote, a datum
            // `on_expire` moved) — so only the expired term can close it. The
            // answer is written whichever way the guard goes: a guard whose
            // early term fails ends the command before a `store` writes, and
            // the holder then had no score to assert.
            b.push(format!("scoreboard players set {} {qa} 1", plan::PARTY));
            for a in ancestors(q, oid) {
                b.push(format!(
                    "scoreboard players set {} {} 1",
                    plan::PARTY,
                    after_score(c, a.id().as_str())
                ));
            }
            b.extend(packtest_gate_drive(plan, o.gate(), true));
            b.push(format!("scoreboard players set #lim_open_{safe} dw.sys 0"));
            b.push(format!(
                "execute{} run scoreboard players set #lim_open_{safe} dw.sys 1",
                pending_guard(plan, o, &qa)
            ));
            b.push(format!("assert score #lim_open_{safe} dw.sys matches 0"));
            b.extend(close.iter().cloned());
            b.extend(give_back.iter().cloned());
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
            b.extend(hold.iter().cloned());
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
            b.extend(give_back.iter().cloned());
            out.insert(
                format!("packtest-datapack/data/{ns}/test/{name}.mcfunction"),
                lines(&b).into_bytes(),
            );
        }
    }
}
