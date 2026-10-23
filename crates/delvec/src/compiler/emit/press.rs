//! spec-0117: every pressable's block, its click box in each state, the state
//! it is in, and the answer it gives a press outside its window — emitted.
//!
//! The model is [`crate::compiler::pressable`]; this module only writes its
//! commands. Three things are emitted per pressable:
//!
//! * **the block** — `setup_finish` places it (a door's two halves), and a
//!   boxed block's click box is summoned there in the state the pressable
//!   starts in ([`press_setup`]);
//! * **the state** — `pst_<owner>` writes `#pst_<owner> dw.sys` 0 (shut), 1
//!   (open) or 2 (settled) from the owner's own gate terms, read by every
//!   answer and by a boxed trigger's reconciliation ([`pst_fns`]);
//! * **the box's window** — a boxed block that acts on its own stands boxed
//!   outside its window: an `interact`'s box goes at its activation and returns
//!   at completion, a trigger's is reconciled against its state each tick
//!   ([`press_tick`]), a shortcut's or a disarm's returns when it is spent.
//!
//! The answers themselves are ordinary presser triggers (emitted with every
//! trigger); [`answer_guard`] is the one fragment that gates each to its state.

use super::*;
use crate::compiler::pressable::{AnswerPlan, Owner, PressPlan, State, box_tags};

/// The engine's refusal cue: the sound the pinned game pairs with its own
/// refused use (a locked container pressed without its key), played to the one
/// player whose press was refused (spec-0117 §5.2).
pub(super) const CUE_REFUSED: &str = "minecraft:block.chest.locked";

/// The line the refusal cue is played by: at the presser, to the presser.
pub(super) fn refusal_cue_line() -> String {
    format!("execute at @s run playsound {CUE_REFUSED} player @s")
}

/// The `dw.sys` holder of an owner's state (`#pst_<owner>`).
pub(super) fn pst_holder(owner_safe: &str) -> String {
    format!("#pst_{owner_safe}")
}

/// The `dw.sys` holder an answer's dispatch writes its state to (`#ans_<owner>`):
/// 1 not yet, 2 missing item, 3 done — the trace a PackTest reads, since an
/// action-bar line leaves no game state.
pub(super) fn ans_holder(owner_safe: &str) -> String {
    format!("#ans_{owner_safe}")
}

/// The tag a presser carries for the rest of the tick in which their press
/// **acted** on `owner` (`dw_acted_<owner>`). A press that settles its owner —
/// a `once` trigger fired, a shortcut thrown, a disarm spent — fires the act's
/// dispatch and its owner's `done` answer from the same report, in an order
/// the game does not promise; an answer that finds the tag stays quiet, so the
/// press that acted is never also told it was already done. `tick` clears it.
pub(super) fn acted_tag(owner_safe: &str) -> String {
    format!("dw_acted_{owner_safe}")
}

/// The `tick` lines that clear every acted tag, once per owner that answers.
pub(super) fn acted_clear_lines(plan: &Plan) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for a in plan.answers() {
        let t = acted_tag(&a.owner_safe);
        let line = format!("tag @a[tag={t}] remove {t}");
        if !out.contains(&line) {
            out.push(line);
        }
    }
    out
}

/// The name of an owner's state function (`pst_<owner>`).
pub(super) fn pst_fn(owner_safe: &str) -> String {
    format!("pst_{owner_safe}")
}

/// A space-prefixed run of `execute` sub-commands, as `pending_guard` writes
/// them, made space-terminated with no leading space — the shape every other
/// fragment here takes.
fn terminated(prefixed: &str) -> String {
    let t = prefixed.trim();
    if t.is_empty() {
        String::new()
    } else {
        format!("{t} ")
    }
}

/// The gate a `serves: {gate}` referent is sealed under, and the one it is
/// open (settled) under, as space-terminated fragments.
pub(super) fn gate_terms(plan: &Plan, gate: &str) -> (String, String) {
    if let Some(sc) = plan.shortcuts.iter().find(|s| s.gate_anchor == gate) {
        return (
            format!("unless score #sc_{} dw.sys matches 1 ", sc.safe),
            format!("if score #sc_{} dw.sys matches 1 ", sc.safe),
        );
    }
    let safe = plan::safe_local(gate);
    (
        format!("if score #gseal_{safe} dw.sys matches 1 "),
        format!(
            "if score #gsealed_{safe} dw.sys matches 1 unless score #gseal_{safe} dw.sys matches 1 "
        ),
    )
}

/// The `(open, settled…)` guards of an owner, as space-terminated `execute`
/// fragments: the press is open where `open` holds (always, when empty) and
/// settled where any `settled` holds. Read from the owner's own gate
/// authorities (`pending_guard`, `trigger_poll_guards`, the sentinels) and
/// never restated.
pub(super) fn owner_guards(plan: &Plan, owner: &Owner) -> (String, Vec<String>) {
    let c = plan.campaign;
    match owner {
        Owner::Interact {
            quest, objective, ..
        } => {
            let q = &c.quests.content.quests[*quest];
            let o = &q.objectives[*objective];
            let qa = quest_active_score(q.id.as_str());
            (
                terminated(&pending_guard(plan, o, &qa)),
                vec![format!(
                    "if score {} {} matches 1 ",
                    plan::PARTY,
                    obj_score(o.id().as_str())
                )],
            )
        }
        Owner::Trigger { id } | Owner::GateAnswer { id, .. } => {
            let Some(t) = c
                .quests
                .content
                .triggers
                .iter()
                .find(|t| t.id.as_str() == id)
            else {
                return (String::new(), Vec::new());
            };
            let (once, forbid, flags) = trigger_poll_guards(plan, t);
            let open = format!("{once}{forbid}{flags}");
            let mut settled = Vec::new();
            if t.once {
                settled.push(format!(
                    "if score #trig_{} dw.sys matches 1 ",
                    plan::safe_local(id)
                ));
            }
            match &t.serves {
                Some(delvewright_dsl::Serves::Objective(o)) => settled.push(format!(
                    "if score {} {} matches 1 ",
                    plan::PARTY,
                    obj_score(o.as_str())
                )),
                Some(delvewright_dsl::Serves::Gate(g)) => {
                    settled.push(gate_terms(plan, g.as_str()).1)
                }
                None => {}
            }
            (open, settled)
        }
        Owner::Shortcut { id } => {
            let safe = plan::safe_local(id);
            (
                format!("unless score #sc_{safe} dw.sys matches 1 "),
                vec![format!("if score #sc_{safe} dw.sys matches 1 ")],
            )
        }
        Owner::Bonfire { index } => (
            format!("if score #bf_{index} dw.sys matches 1 "),
            Vec::new(),
        ),
        Owner::Shop { .. } => (String::new(), Vec::new()),
        Owner::TrapDisarm { id } => {
            let safe = plan::safe_local(id);
            (
                format!("unless score #trapdis_{safe} dw.sys matches 1 "),
                vec![format!("if score #trapdis_{safe} dw.sys matches 1 ")],
            )
        }
        Owner::TimedGateDisarm { id } => {
            let safe = plan::safe_local(id);
            (
                format!("unless score #tgdis_{safe} dw.sys matches 1 "),
                vec![format!("if score #tgdis_{safe} dw.sys matches 1 ")],
            )
        }
    }
}

/// The body of `pst_<owner>`: the owner's state into `#pst_<owner>`.
pub(super) fn pst_lines(plan: &Plan, owner: &Owner) -> Vec<String> {
    let h = pst_holder(&owner.safe());
    let (open, settled) = owner_guards(plan, owner);
    let mut v = vec![format!(
        "scoreboard players set {h} dw.sys {}",
        State::Shut.value()
    )];
    let set = |s: State| format!("scoreboard players set {h} dw.sys {}", s.value());
    if open.is_empty() {
        v.push(set(State::Open));
    } else {
        v.push(format!("execute {open}run {}", set(State::Open)));
    }
    for s in settled {
        v.push(format!("execute {s}run {}", set(State::Settled)));
    }
    v
}

/// The owners whose state is read: every owner with a declared answer, and
/// every boxed trigger whose box is reconciled against its state. One function
/// per owner, in first-read order.
fn pst_owners(plan: &Plan) -> Vec<Owner> {
    let mut out: Vec<Owner> = Vec::new();
    let mut add = |o: &Owner| {
        if !out.contains(o) {
            out.push(o.clone());
        }
    };
    for p in plan.presses() {
        if reconciled(p) {
            add(&p.owner);
        }
    }
    for a in plan.answers() {
        add(&a.owner);
    }
    out
}

/// The `pst_<owner>` functions.
pub(super) fn pst_fns(plan: &Plan) -> Vec<(String, String)> {
    pst_owners(plan)
        .iter()
        .map(|o| (pst_fn(&o.safe()), lines(&pst_lines(plan, o))))
        .collect()
}

/// Whether a press's box is reconciled against its state each tick: a boxed
/// trigger whose block acts on its own. (An `interact`, a shortcut, a bonfire
/// and a disarm move theirs at their own events.)
fn reconciled(p: &PressPlan) -> bool {
    p.reconciled()
}

/// The summon of `p`'s click box with every rider's tag, or nothing for a
/// block pressed on its own.
pub(super) fn box_summon(plan: &Plan, p: &PressPlan) -> Option<String> {
    p.box_summon(&box_tags(plan, p))
}

/// The guarded summon of `p`'s box: never a second one over the same block.
pub(super) fn box_summon_once(plan: &Plan, p: &PressPlan) -> Option<String> {
    box_summon(plan, p).map(|s| format!("execute unless entity @e[tag={}] run {s}", p.tag))
}

/// The `kill` of `p`'s box.
pub(super) fn box_kill(p: &PressPlan) -> String {
    format!("kill @e[tag={}]", p.tag)
}

/// **`setup_finish`**: every pressable's block (a door's two halves), then
/// every click box that stands in its pressable's first state. A gate's own
/// answer places no block — the gate's is there — and its box goes up with the
/// seal ([`gate_arm_fns`]).
pub(super) fn press_setup(plan: &Plan) -> Vec<String> {
    let mut out = Vec::new();
    for p in plan.presses() {
        out.extend(p.setblocks());
    }
    for p in plan.presses() {
        if matches!(p.owner, Owner::GateAnswer { .. }) {
            continue;
        }
        if p.boxed_in(p.initial())
            && let Some(s) = box_summon_once(plan, p)
        {
            out.push(s);
        }
    }
    out
}

/// **`tick`**: each reconciled box made its state — none while the trigger is
/// open, one while it is shut or settled (none once settled if released).
pub(super) fn press_tick(plan: &Plan) -> Vec<String> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for p in plan.presses() {
        if !reconciled(p) {
            continue;
        }
        let safe = p.owner.safe();
        let h = pst_holder(&safe);
        let Some(summon) = box_summon(plan, p) else {
            continue;
        };
        out.push(format!("function {ns}:{}", pst_fn(&safe)));
        let (absent, present) = if p.release {
            ("1..", "0")
        } else {
            ("1", "0..")
        };
        out.push(format!(
            "execute if score {h} dw.sys matches {absent} run {}",
            box_kill(p)
        ));
        if p.release {
            out.push(format!(
                "execute if score {h} dw.sys matches {present} unless entity @e[tag={}] run {summon}",
                p.tag
            ));
        } else {
            out.push(format!(
                "execute unless score {h} dw.sys matches 1 unless entity @e[tag={}] run {summon}",
                p.tag
            ));
        }
    }
    out
}

/// What an `interact`'s activation runs for its block: the box goes, where the
/// block acts on its own in the window.
pub(super) fn interact_activation(plan: &Plan, oid: &str) -> Vec<String> {
    match crate::compiler::pressable::press_of_objective(plan, oid) {
        Some(p) if p.boxed_in(State::Shut) && !p.boxed_in(State::Open) => vec![box_kill(p)],
        _ => Vec::new(),
    }
}

/// What an `interact`'s completion runs for its block: the box returns unless
/// released, or a retained box goes when released.
pub(super) fn interact_completion(plan: &Plan, oid: &str) -> Vec<String> {
    let Some(p) = crate::compiler::pressable::press_of_objective(plan, oid) else {
        return Vec::new();
    };
    settle_lines(plan, p)
}

/// What a pressable's settlement runs for its box: summoned where the settled
/// state has one and the open state did not; killed where the open state had
/// one and the settled state does not.
pub(super) fn settle_lines(plan: &Plan, p: &PressPlan) -> Vec<String> {
    match (p.boxed_in(State::Open), p.boxed_in(State::Settled)) {
        (false, true) => box_summon_once(plan, p).into_iter().collect(),
        (true, false) => vec![box_kill(p)],
        _ => Vec::new(),
    }
}

/// The press owned by `owner`, if one is placed.
pub(super) fn press_of_owner<'p>(plan: &'p Plan, owner: &Owner) -> Option<&'p PressPlan> {
    plan.presses().iter().find(|p| &p.owner == owner)
}

/// The cells a sealed side is pressed from, as one selector volume: the
/// half-space beside the door on its sealed side, eight cells deep and eight
/// beyond the door's edges ([`crate::compiler::wrongside::SealedSide`]).
pub(super) fn sealed_side_volume(side: &crate::compiler::wrongside::SealedSide) -> String {
    const R: i32 = 8;
    let (lo, hi) = (side.gate_lo, side.gate_hi);
    let mut a = [lo[0] - R, lo[1] - R, lo[2] - R];
    let mut b = [hi[0] + R, hi[1] + R, hi[2] + R];
    if side.dir < 0 {
        a[side.axis] = lo[side.axis] - R;
        b[side.axis] = lo[side.axis] - 1;
    } else {
        a[side.axis] = hi[side.axis] + 1;
        b[side.axis] = hi[side.axis] + R;
    }
    format!(
        "x={},y={},z={},dx={},dy={},dz={}",
        a[0],
        a[1],
        a[2],
        b[0] - a[0],
        b[1] - a[1],
        b[2] - a[2]
    )
}

/// The side term a shortcut door's answers carry: the gate's own answer speaks
/// to a presser **on** the sealed side, the shortcut's `not_yet` to one **off**
/// it. A gate term the compiler derives from `wrongside`; the author types
/// nothing.
pub(super) fn side_term(plan: &Plan, gate: &str, on_sealed_side: bool) -> String {
    let Some(side) = plan
        .shortcuts
        .iter()
        .find(|s| s.gate_anchor == gate)
        .and_then(|s| s.sealed_side.as_ref())
    else {
        return String::new();
    };
    let word = if on_sealed_side { "if" } else { "unless" };
    format!("{word} entity @s[{}] ", sealed_side_volume(side))
}

/// **The guard an answer trigger's dispatch carries** (spec-0117 §5.2): the
/// prelude that computes its owner's state, and the space-terminated fragment
/// that holds the press to the one state it answers. `None` for a trigger that
/// is no answer.
pub(super) fn answer_guard(
    plan: &Plan,
    t: &delvewright_dsl::EnvTrigger,
) -> Option<(String, String)> {
    let a = crate::compiler::pressable::answer_of(plan, t.id.as_str())?;
    Some(answer_guard_of(plan, a))
}

/// [`answer_guard`] for a known answer.
pub(super) fn answer_guard_of(plan: &Plan, a: &AnswerPlan) -> (String, String) {
    let ns = &plan.namespace;
    let h = pst_holder(&a.owner_safe);
    let prelude = format!("function {ns}:{}", pst_fn(&a.owner_safe));
    if let Some(sc) = &a.far_side_of {
        // The sealed door, pressed from where it does not open, while barred.
        let gate = plan
            .shortcuts
            .iter()
            .find(|s| &s.id == sc)
            .map(|s| s.gate_anchor.clone())
            .unwrap_or_default();
        return (
            prelude,
            format!(
                "if score {h} dw.sys matches {} {}",
                State::Open.value(),
                side_term(plan, &gate, false)
            ),
        );
    }
    let state = State::of_answer(a.state);
    let mut g = format!(
        "if score {h} dw.sys matches {} unless entity @s[tag={}] ",
        state.value(),
        acted_tag(&a.owner_safe)
    );
    if a.state == delvewright_dsl::AnswerState::MissingItem
        && let Some(item) = a
            .press
            .and_then(|i| plan.presses().get(i))
            .and_then(|p| p.requires_item.clone())
    {
        g.push_str(&format!("unless items entity @s weapon.mainhand {item} "));
    }
    (prelude, g)
}

/// The two lines an answer's own bundle carries after its line: the trace and
/// the refusal cue.
pub(super) fn answer_body_tail(a: &AnswerPlan) -> Vec<String> {
    vec![
        format!(
            "scoreboard players set {} dw.sys {}",
            ans_holder(&a.owner_safe),
            a.state.trace()
        ),
        refusal_cue_line(),
    ]
}

/// The `seal_arm_<safe>` functions (spec-0117 §3a): the boxes of a sealed
/// gate's own answers, fitted over the gate's block at each answer's cell,
/// carrying every answer that rides them. A `close-gate` calls it when it
/// seals; a shortcut's gate is sealed from load, so `setup_finish` does.
pub(super) fn gate_arm_fns(plan: &Plan) -> Vec<(String, String)> {
    let mut gates: Vec<String> = Vec::new();
    for p in plan.presses() {
        if let Owner::GateAnswer { gate, .. } = &p.owner
            && !gates.contains(gate)
        {
            gates.push(gate.clone());
        }
    }
    let mut out = Vec::new();
    for g in gates {
        let mut body: Vec<String> = Vec::new();
        let mut seen: Vec<[i32; 3]> = Vec::new();
        for p in plan.presses() {
            if !matches!(&p.owner, Owner::GateAnswer { gate, .. } if gate == &g)
                || seen.contains(&p.cell)
            {
                continue;
            }
            seen.push(p.cell);
            if let Some(s) = box_summon(plan, p) {
                body.push(s);
            }
        }
        out.push((gate_arm_fn(&plan::safe_local(&g)), lines(&body)));
    }
    out
}

/// The name of a gate's arming function.
pub(super) fn gate_arm_fn(safe: &str) -> String {
    format!("seal_arm_{safe}")
}

/// Whether gate `anchor` has an own answer whose boxes `seal_arm_<safe>`
/// summons.
pub(super) fn gate_has_answer_box(plan: &Plan, anchor: &str) -> bool {
    plan.presses()
        .iter()
        .any(|p| matches!(&p.owner, Owner::GateAnswer { gate, .. } if gate == anchor))
}

/// The cells a gate's own answer boxes stand over, with the gate's block —
/// what `DW1013` judges those boxes against (the block is the gate's, laid by
/// the prefab or the `close-gate`, not by `setup_finish`).
pub fn gate_blocks(plan: &Plan) -> BTreeMap<[i32; 3], String> {
    plan.presses()
        .iter()
        .filter(|p| matches!(p.owner, Owner::GateAnswer { .. }))
        .map(|p| (p.cell, p.block.clone()))
        .collect()
}

/// The advancement criterion a press of `p` in `state` is reported by: the
/// block's own `default_block_use` where no box stands, the box's
/// `player_interacted_with_entity` where one does.
pub(super) fn press_criterion_of(p: &PressPlan, state: State) -> Value {
    if p.block_detects(state) {
        block_use_criterion(&p.block, p.cell)
    } else {
        json!({
            "trigger": "minecraft:player_interacted_with_entity",
            "conditions": {
                "entity": {
                    "type": "minecraft:interaction",
                    "nbt": format!("{{Tags:[\"{}\"]}}", p.tag)
                }
            }
        })
    }
}
