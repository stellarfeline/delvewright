use super::*;

/// The race templates (spec-0139 §8), synchronous, per race. The racer is
/// the template's own dummy, moved by `tp` into a checkpoint's box and asked
/// by the race's own body function, the one the tick runs as every racer.
///
/// `race_<r>_in_order` starts the race, takes the dummy through every
/// checkpoint of every lap in order and asserts its count after each, then
/// asserts the race over — the dummy was the last racer short of the finish —
/// and that a start makes it run again with nobody credited. A credit that
/// does not count, a finish that does not end the race, or a start that
/// leaves the last heat's counts reds it.
///
/// `race_<r>_skip`, for a race of two checkpoints or more, starts the race
/// and stands the dummy in each checkpoint further on than its next, the
/// finish included: its count stays where it was and nobody has finished. It
/// then stands it in its next and asserts the credit. A checkpoint credited
/// out of turn reds it.
///
/// `race_<r>_bystander` spawns a second player beside the dummy, stood above
/// every checkpoint, and asks the body function as both on every step the
/// dummy runs the course: when the dummy finishes, the race is over and the
/// bystander was never on the course. A race that waits on a present player
/// who never started reds it.
pub(super) fn emit_race_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let mut races_seen = 0usize;
    for r in &plan.campaign.quests.content.races {
        let id = r.id.as_str();
        let safe = plan::safe_local(id);
        let boxes = super::super::race_boxes(plan, r);
        if boxes.len() != r.checkpoints.len() || boxes.is_empty() {
            continue;
        }
        let n = boxes.len();
        let total = r.total() as usize;
        let p = super::super::race_progress(id);
        let (on, over, first) = (
            super::super::race_running(id),
            super::super::race_over(id),
            super::super::race_first(id),
        );
        let start = format!("function {ns}:{}", super::super::race_start_fn(id));
        let end = format!("function {ns}:{}", super::super::race_end_fn(id));
        let into = |sel: &str, k: usize| {
            let (lo, hi) = boxes[k % n];
            format!(
                "tp {sel} {} {} {}",
                (lo[0] + hi[0]) / 2,
                (lo[1] + hi[1]) / 2,
                (lo[2] + hi[2]) / 2
            )
        };
        // What a template leaves behind: the race at rest, the dummy home
        // and off the race's board.
        let tidy = |sel: &str, tag: &str| {
            let mut t = vec![
                end.clone(),
                format!("scoreboard players set {over} dw.sys 0"),
                format!("scoreboard players set {first} dw.sys 0"),
                format!("scoreboard players reset {sel} {p}"),
                format!(
                    "scoreboard players reset {sel} {}",
                    super::super::race_next(id)
                ),
            ];
            if let Some(b) = &r.board {
                t.push(format!(
                    "scoreboard players reset {} dw.sys",
                    super::super::board_counter(b.as_str())
                ));
                t.push(format!(
                    "scoreboard players reset {sel} {}",
                    super::super::board_objective(b.as_str())
                ));
            }
            t.push(format!(
                "tag {sel} remove {}",
                super::super::race_on_course(id)
            ));
            t.push(format!("tp {sel} ~ ~ ~"));
            t.push(format!("tag {sel} remove {tag}"));
            t
        };

        let tag = format!("dw_pt_rc_{safe}");
        let (pin, me) = pin_dummy(&tag);
        let body = format!(
            "execute as {me} run function {ns}:{}",
            super::super::race_body_fn(id)
        );
        let mut t = packtest_header(&format!(
            "{title}: race `{id}` credits its {n} checkpoint(s) in order over {} lap(s), is over \
             when its last racer finishes, and starts again from nothing (spec-0139)",
            r.laps
        ));
        t.push(pin);
        if let Some(b) = &r.board {
            // The board's first place is this template's to give.
            t.push(format!(
                "scoreboard players reset {} dw.sys",
                super::super::board_counter(b.as_str())
            ));
            t.push(format!(
                "scoreboard players reset {me} {}",
                super::super::board_objective(b.as_str())
            ));
        }
        t.push(start.clone());
        t.push(format!("assert score {on} dw.sys matches 1"));
        for k in 0..total {
            t.push(into(&me, k));
            t.push(body.clone());
            t.push(format!("assert score {me} {p} matches {}", k + 1));
            if k + 1 < total {
                t.push(format!("assert score {on} dw.sys matches 1"));
                t.push(format!("assert score {over} dw.sys matches 0"));
            }
        }
        t.push(format!("assert score {first} dw.sys matches 1"));
        t.push(format!("assert score {on} dw.sys matches 0"));
        t.push(format!("assert score {over} dw.sys matches 1"));
        if let Some(b) = &r.board {
            let rows = delvewright_dsl::board_rows(plan.campaign, b.as_str()).max(1);
            t.push(format!(
                "assert score {me} {} matches {rows}",
                super::super::board_objective(b.as_str())
            ));
        }
        // A racer that has finished is credited nothing more.
        t.push(into(&me, 0));
        t.push(body.clone());
        t.push(format!("assert score {me} {p} matches {total}"));
        t.push(start.clone());
        t.push(format!("assert score {on} dw.sys matches 1"));
        t.push(format!("assert score {over} dw.sys matches 0"));
        t.push(format!("assert not score {me} {p} matches 0.."));
        t.extend(tidy(&me, &tag));
        out.insert(
            format!("packtest-datapack/data/{ns}/test/race_{safe}_in_order.mcfunction"),
            lines(&t).into_bytes(),
        );

        // A second member who never starts: the race ends when the one who
        // started finishes.
        let tag = format!("dw_pt_rb_{safe}");
        let (pin, me) = pin_dummy(&tag);
        let idle = format!("dwrcb{races_seen}");
        races_seen += 1;
        let them = format!("@a[name={idle},limit=1]");
        let both = [&me, &them].map(|sel| {
            format!(
                "execute as {sel} run function {ns}:{}",
                super::super::race_body_fn(id)
            )
        });
        let above = boxes.iter().map(|(_, hi)| hi[1]).max().unwrap_or(0) + 4;
        let (lo0, hi0) = boxes[0];
        let mut t = packtest_header(&format!(
            "{title}: race `{id}` is over when the member who started it finishes, though \
             another present member never started (spec-0139)"
        ));
        t.push(pin);
        t.push(format!("dummy {idle} spawn"));
        t.push(format!(
            "tp {them} {} {above} {}",
            (lo0[0] + hi0[0]) / 2,
            (lo0[2] + hi0[2]) / 2
        ));
        if let Some(b) = &r.board {
            t.push(format!(
                "scoreboard players reset {} dw.sys",
                super::super::board_counter(b.as_str())
            ));
            t.push(format!(
                "scoreboard players reset {me} {}",
                super::super::board_objective(b.as_str())
            ));
        }
        t.push(start.clone());
        for k in 0..total {
            t.push(into(&me, k));
            t.extend(both.iter().cloned());
        }
        t.push(format!("assert score {me} {p} matches {total}"));
        t.push(format!("assert score {them} {p} matches 0"));
        t.push(format!("assert score {on} dw.sys matches 0"));
        t.push(format!("assert score {over} dw.sys matches 1"));
        t.extend(tidy(&me, &tag));
        t.push(format!("scoreboard players reset {them} {p}"));
        t.push(format!(
            "scoreboard players reset {them} {}",
            super::super::race_next(id)
        ));
        t.push(format!("dummy {idle} leave"));
        out.insert(
            format!("packtest-datapack/data/{ns}/test/race_{safe}_bystander.mcfunction"),
            lines(&t).into_bytes(),
        );

        if n < 2 {
            continue;
        }
        let tag = format!("dw_pt_rs_{safe}");
        let (pin, me) = pin_dummy(&tag);
        let body = format!(
            "execute as {me} run function {ns}:{}",
            super::super::race_body_fn(id)
        );
        let mut t = packtest_header(&format!(
            "{title}: race `{id}` credits no checkpoint further on than a racer's next, the \
             finish included (spec-0139)"
        ));
        t.push(pin);
        t.push(start.clone());
        for k in 1..n {
            t.push(into(&me, k));
            t.push(body.clone());
            t.push(format!("assert score {me} {p} matches 0"));
        }
        t.push(format!("assert score {first} dw.sys matches 0"));
        t.push(format!("assert score {over} dw.sys matches 0"));
        t.push(into(&me, 0));
        t.push(body.clone());
        t.push(format!("assert score {me} {p} matches 1"));
        // …and with the first credited, the ones past the second still wait.
        for k in 2..n {
            t.push(into(&me, k));
            t.push(body.clone());
            t.push(format!("assert score {me} {p} matches 1"));
        }
        t.push(format!("assert score {on} dw.sys matches 1"));
        t.extend(tidy(&me, &tag));
        out.insert(
            format!("packtest-datapack/data/{ns}/test/race_{safe}_skip.mcfunction"),
            lines(&t).into_bytes(),
        );
    }
}
