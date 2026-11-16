use super::*;

/// The mode templates (spec-0140 §7), per mode.
///
/// `mode_<m>_holds` is synchronous, and runs alone: it starts from the whole
/// party ledger at the campaign's start — the entry, the settlement and the
/// leaving reach every clock and every quest the mode touches, and a template
/// owns every term on that way (`DW0807`) — which no sibling may find under
/// it. With the mode's kept quests in progress it opens the guard of the first objective of a quest the mode holds and of
/// the first objective of a quest it keeps, and reads each guard — the very
/// one every driver of the objective runs under — before the entry, under the
/// mode and after the leaving: the held objective completes before and after
/// and not under; it is told before and after and not under; the kept one
/// completes throughout. It then enters the mode again and completes what it
/// keeps: the settlement leaves it. Entered with nothing in progress, the
/// mode is left at once.
///
/// `mode_<m>_clock`, for the held limited objective of the shortest window,
/// runs alone and on the server's own clock, from the same ledger. It starts the objective's
/// clock, enters the mode, waits longer than the whole window and asserts
/// the objective not expired and its seconds where they stood; it leaves
/// the mode, waits the seconds left and asserts the objective expired.
pub(super) fn emit_mode_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let q = &c.quests.content;
    let p = plan::PARTY;
    let title = artifact_title(c);
    let score = super::super::mode::MODE_SCORE;
    let off = format!("function {ns}:{}", super::super::mode::MODE_OFF_FN);
    for (i, m) in q.modes.iter().enumerate() {
        let n = i + 1;
        let id = m.id.as_str();
        let safe = plan::safe_local(id);
        let enter = format!("function {ns}:{}", super::super::mode::mode_enter_fn(id));
        let leave = format!("execute if score {p} {score} matches {n} run {off}");
        let kept: Vec<&delvewright_dsl::Quest> =
            q.quests.iter().filter(|x| m.keeps(x.id.as_str())).collect();
        if kept.is_empty() {
            continue;
        }
        // The kept quests in progress, and what was there given back.
        let mut progress = Vec::new();
        let mut done = Vec::new();
        let mut give_back = Vec::new();
        for (k, x) in kept.iter().enumerate() {
            let (qa, qd) = (
                quest_active_score(x.id.as_str()),
                quest_score(x.id.as_str()),
            );
            progress.push(format!(
                "scoreboard players operation #pt_md_a{k} dw.sys = {p} {qa}"
            ));
            progress.push(format!(
                "scoreboard players operation #pt_md_d{k} dw.sys = {p} {qd}"
            ));
            progress.push(format!("scoreboard players set {p} {qa} 1"));
            progress.push(format!("scoreboard players set {p} {qd} 0"));
            done.push(format!("scoreboard players set {p} {qd} 1"));
            give_back.push(format!(
                "scoreboard players operation {p} {qa} = #pt_md_a{k} dw.sys"
            ));
            give_back.push(format!(
                "scoreboard players operation {p} {qd} = #pt_md_d{k} dw.sys"
            ));
        }
        let held_objective = q
            .quests
            .iter()
            .filter(|x| !m.keeps(x.id.as_str()))
            .find_map(|x| x.objectives.first().map(|o| (x, o)));
        let kept_objective = kept
            .iter()
            .find_map(|x| x.objectives.first().map(|o| (*x, o)));
        // A guard read into `#pt_md dw.sys`, and asserted.
        let read = |guard: &str, want: u8| {
            vec![
                "scoreboard players set #pt_md dw.sys 0".to_string(),
                format!("execute{guard} run scoreboard players set #pt_md dw.sys 1"),
                format!("assert score #pt_md dw.sys matches {want}"),
            ]
        };

        let holds = format!("mode_{safe}_holds");
        let mut t = vec![
            format!(
                "#> {title}: mode `{id}` holds every quest but the {} it keeps — not told, \
                 not completable — gives them back when it is left, and is left when what it \
                 keeps is done (spec-0140)",
                kept.len()
            ),
            limit::alone(ns, &holds, out),
            "# @dummy".to_string(),
            "# @timeout 100".to_string(),
            String::new(),
        ];
        t.extend(quest::fresh_ledger(plan));
        // The kept objective's guard first: its drive may write the map's
        // mode, which the entry then sets.
        if let Some((x, o)) = kept_objective {
            t.extend(objective_state_clear(plan, o));
            t.extend(packtest_guards(plan, x.id.as_str(), o, true));
        }
        if let Some((x, o)) = held_objective {
            t.extend(objective_state_clear(plan, o));
            t.extend(packtest_guards(plan, x.id.as_str(), o, true));
        }
        t.extend(progress.iter().cloned());
        t.push(format!("scoreboard players set {p} {score} 0"));
        let held_guards = held_objective.map(|(x, o)| {
            let qa = quest_active_score(x.id.as_str());
            (pending_guard(plan, o, &qa), told_guard(plan, o, &qa))
        });
        let kept_guard =
            kept_objective.map(|(x, o)| pending_guard(plan, o, &quest_active_score(x.id.as_str())));
        let all = |t: &mut Vec<String>, held: u8| {
            if let Some((pending, told)) = &held_guards {
                t.extend(read(pending, held));
                t.extend(read(told, held));
            }
            if let Some(g) = &kept_guard {
                t.extend(read(g, 1));
            }
        };
        all(&mut t, 1);
        t.push(enter.clone());
        t.push(format!("assert score {p} {score} matches {n}"));
        all(&mut t, 0);
        t.push(leave.clone());
        t.push(format!("assert score {p} {score} matches 0"));
        all(&mut t, 1);
        // Left by itself when what it keeps is done.
        t.push(enter.clone());
        t.push(format!("assert score {p} {score} matches {n}"));
        t.extend(done.iter().cloned());
        t.push(format!(
            "function {ns}:{}",
            super::super::mode::mode_settle_fn(id)
        ));
        t.push(format!("assert score {p} {score} matches 0"));
        // …and never on with nothing in progress.
        t.push(enter.clone());
        t.push(format!("assert score {p} {score} matches 0"));
        t.extend(give_back.iter().cloned());
        out.insert(
            format!("packtest-datapack/data/{ns}/test/{holds}.mcfunction"),
            lines(&t).into_bytes(),
        );

        // -- the clock ---------------------------------------------------
        let Some((x, o, l)) = super::super::mode::clocks_held_by(c, m)
            .into_iter()
            .filter_map(|(x, o)| Some((x, o, o.limit()?)))
            .min_by_key(|(_, _, l)| l.ticks)
        else {
            continue;
        };
        let Some((start, latch)) = limit::start_of(x, o) else {
            continue;
        };
        let oid = o.id().as_str();
        let secs = l.ticks.div_ceil(20);
        let holder = super::super::limit::seconds_holder(oid);
        let held = super::super::mode::clock_held(oid);
        let expired = expired_score(oid);
        let (open, close) = limit::open_and_close(plan, x, o, &latch);
        let (still, moving) = limit::held_still(plan, o);
        let name = format!("mode_{safe}_clock");
        let stood = l.ticks + 25;
        let rest = secs * 20 + 5;
        let mut b = vec![
            format!(
                "#> {title}: under mode `{id}` the clock of held `{oid}` stands still, and \
                 goes on from the second it showed when the mode is left (spec-0140)"
            ),
            limit::alone(ns, &name, out),
            "# @dummy".to_string(),
            format!("# @timeout {}", stood + rest + 100),
            String::new(),
            format!("function {ns}:setup"),
        ];
        b.extend(still.iter().cloned());
        // Every quest at the campaign's start, so no act of a body left on
        // the server completes what the mode keeps while the clock is
        // watched.
        b.extend(quest::fresh_ledger(plan));
        b.extend(open.iter().cloned());
        b.extend(progress.iter().cloned());
        b.push(format!("scoreboard players set {p} {score} 0"));
        b.push(format!("function {ns}:{start}"));
        b.push(format!("assert score {holder} dw.sys matches {secs}"));
        b.push(enter.clone());
        b.push(format!("assert score {p} {score} matches {n}"));
        b.push(format!("assert score {held} dw.sys matches 1.."));
        b.push(format!("await delay {stood}t"));
        b.push(format!("assert score {p} {score} matches {n}"));
        b.push(format!("assert score {holder} dw.sys matches {secs}"));
        b.push(format!(
            "execute store success score #pt_mc dw.sys if score {p} {expired} matches 1"
        ));
        b.push("assert score #pt_mc dw.sys matches 0".to_string());
        b.push(leave.clone());
        b.push(format!("assert score {p} {score} matches 0"));
        b.push(format!("assert score {held} dw.sys matches 0"));
        b.push(format!("await delay {rest}t"));
        b.push(format!("assert score {p} {expired} matches 1"));
        b.extend(close.iter().cloned());
        b.extend(moving.iter().cloned());
        b.extend(give_back.iter().cloned());
        out.insert(
            format!("packtest-datapack/data/{ns}/test/{name}.mcfunction"),
            lines(&b).into_bytes(),
        );
    }
}
