//! A sign says its words (spec-0115): where each declared sign comes to
//! stand, the proofs that it does (`DW0431`) and that every line fits its
//! face (`DW1048`), the binding line and the ledger, and the block entities a
//! configuration's written world lays.

use std::collections::{BTreeMap, BTreeSet};

use delvewright_dsl::{SignFace, SignKind, Verb};

use crate::compiler::blockstate::BlockMap;
use crate::compiler::failure::Failure;
use crate::compiler::plan::{EffectRoot, Plan, Step, for_each_gate_effect, quest_complete_step};

/// How a declared sign's cell comes to hold a sign block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Placer {
    /// A piece's template lays it: it stands from load and is lettered in
    /// `setup_finish`.
    Template,
    /// A trigger's `prop`, placed in `setup_finish`.
    TriggerProp(String),
    /// An `interact` objective's `prop`, placed when the objective activates.
    InteractProp {
        /// The objective.
        objective: String,
        /// Its critical-path step, when the path performs it.
        step: Option<usize>,
    },
    /// A `set-block` or a `fill-region` effect.
    Write {
        /// `set-block` / `fill-region`.
        verb: &'static str,
        /// The effect's JSON pointer.
        path: String,
        /// `(step, walk order)` on the critical path, when the path fires it.
        at: Option<(usize, usize)>,
    },
}

impl Placer {
    fn word(&self) -> String {
        match self {
            Placer::Template => "load".to_string(),
            Placer::TriggerProp(t) => format!("prop of {t}"),
            Placer::InteractProp { objective, .. } => format!("prop of {objective}"),
            Placer::Write { verb, path, .. } => format!("{verb} at {path}"),
        }
    }
}

/// One declared sign, as the build found it.
#[derive(Clone, Debug)]
pub struct SignRow {
    /// `sign/<kebab>`.
    pub id: String,
    /// The anchor the row names.
    pub anchor: String,
    /// The cell.
    pub cell: [i32; 3],
    /// The sign block that stands, or is written, at the cell: the template's
    /// when a piece lays one, else the first a document writes.
    pub block: Option<String>,
    /// Every write that puts a sign block into the cell.
    pub placers: Vec<Placer>,
}

impl SignRow {
    /// The kind of sign at the cell.
    pub fn kind(&self) -> Option<SignKind> {
        self.block.as_deref().and_then(SignKind::of)
    }
}

/// What the sign pass proved, for the emitter, the binding line and the
/// ledger.
#[derive(Clone, Debug, Default)]
pub struct SignGate {
    /// One row per declared sign whose anchor resolves, declaration-ordered.
    pub rows: Vec<SignRow>,
    /// `letter-sign` effects the campaign writes.
    pub letters: usize,
}

impl SignGate {
    /// Whether `id` is lettered in `setup_finish` because a piece's template
    /// lays its sign (and no trigger prop replaces it there).
    pub fn lettered_at_load(&self, id: &str) -> bool {
        self.rows.iter().any(|r| {
            r.id == id
                && r.placers.contains(&Placer::Template)
                && !r
                    .placers
                    .iter()
                    .any(|p| matches!(p, Placer::TriggerProp(_)))
        })
    }
}

/// The critical-path step an effect root fires at, or `None` for a root the
/// path never forces (a trap, a death, a purchase, a trigger nobody performs).
fn site_step(plan: &Plan, root: &EffectRoot<'_>) -> Option<usize> {
    match root {
        EffectRoot::ObjectiveComplete { objective, .. } => {
            plan.objective_steps.get(*objective).copied()
        }
        EffectRoot::QuestComplete(q) => plan
            .path_firing
            .quests
            .contains(q.id.as_str())
            .then(|| quest_complete_step(q, &plan.objective_steps)),
        EffectRoot::Trigger(t) => plan.path_firing.trigger_step.get(t.id.as_str()).copied(),
        _ => None,
    }
}

/// The step index on `steps` an effect root fires at, for a written world
/// read along one path.
fn step_on(steps: &[Step], root: &EffectRoot<'_>) -> Option<usize> {
    match root {
        EffectRoot::ObjectiveComplete { objective, .. } => {
            steps.iter().position(|s| s.objective() == Some(*objective))
        }
        EffectRoot::QuestComplete(q) => q
            .objectives
            .iter()
            .filter_map(|o| {
                steps
                    .iter()
                    .position(|s| s.objective() == Some(o.id().as_str()))
            })
            .max(),
        EffectRoot::Trigger(t) => steps
            .iter()
            .position(|s| s.trigger() == Some(t.id.as_str())),
        _ => None,
    }
}

/// Every `letter-sign` the campaign writes: `(sign, front, back, root step,
/// walk order, pointer)`, in walk order.
type Letter<'a> = (
    &'a str,
    Option<&'a SignFace>,
    Option<&'a SignFace>,
    EffectRoot<'a>,
    usize,
    String,
);

fn letters<'a>(plan: &'a Plan<'a>) -> Vec<Letter<'a>> {
    let mut out = Vec::new();
    let mut order = 0usize;
    for_each_gate_effect(plan.campaign, &mut |site, e| {
        order += 1;
        if let Verb::LetterSign { sign, front, back } = &e.verb {
            out.push((
                sign.as_str(),
                front.as_ref(),
                back.as_ref(),
                site.root,
                order,
                site.path.clone(),
            ));
        }
    });
    out
}

/// **Where each declared sign comes to stand** (spec-0115 §4.2) over `load`,
/// the assembled world a piece's template lays, and the proof that it does
/// (`DW0431`): a row whose cell no configuration fills with a sign, and a
/// `letter-sign` whose beat fires before any write has put the sign there,
/// are refused naming the cells that do hold one.
pub fn prove(plan: &Plan, load: &BlockMap) -> Result<SignGate, Failure> {
    let c = plan.campaign;
    let mut rows: Vec<SignRow> = Vec::new();
    let in_box = |cell: [i32; 3], (lo, hi): ([i32; 3], [i32; 3])| {
        (0..3).all(|k| lo[k].min(hi[k]) <= cell[k] && cell[k] <= lo[k].max(hi[k]))
    };
    for s in &plan.signs {
        let mut placers: Vec<Placer> = Vec::new();
        let mut block: Option<String> = None;
        if let Some(b) = load.get(&s.cell).map(|b| b.as_str())
            && SignKind::of(b).is_some()
        {
            placers.push(Placer::Template);
            block = Some(b.to_string());
        }
        for t in &c.quests.content.triggers {
            if let Some(p) = &t.prop
                && t.at_anchor().and_then(|a| plan.point_any(a)) == Some(s.cell)
                && SignKind::of(&p.block).is_some()
            {
                placers.push(Placer::TriggerProp(t.id.as_str().to_string()));
                block.get_or_insert_with(|| p.block.clone());
            }
        }
        for q in &c.quests.content.quests {
            for o in &q.objectives {
                if let delvewright_dsl::Objective::Interact {
                    id,
                    anchor,
                    prop: Some(p),
                    ..
                } = o
                    && plan.point_any(anchor.as_str()) == Some(s.cell)
                    && SignKind::of(&p.block).is_some()
                {
                    placers.push(Placer::InteractProp {
                        objective: id.as_str().to_string(),
                        step: plan.objective_steps.get(id.as_str()).copied(),
                    });
                    block.get_or_insert_with(|| p.block.clone());
                }
            }
        }
        let mut order = 0usize;
        for_each_gate_effect(c, &mut |site, e| {
            order += 1;
            let written = match &e.verb {
                Verb::SetBlock { anchor, block } => plan
                    .point_any(anchor.as_str())
                    .filter(|p| *p == s.cell)
                    .map(|_| ("set-block", block)),
                Verb::FillRegion { region, block } => plan
                    .zone_box(region)
                    .filter(|b| in_box(s.cell, *b))
                    .map(|_| ("fill-region", block)),
                _ => None,
            };
            if let Some((verb, b)) = written
                && SignKind::of(b).is_some()
            {
                placers.push(Placer::Write {
                    verb,
                    path: site.path.clone(),
                    at: site_step(plan, &site.root).map(|st| (st, order)),
                });
                block.get_or_insert_with(|| b.clone());
            }
        });
        rows.push(SignRow {
            id: s.id.clone(),
            anchor: s.anchor.clone(),
            cell: s.cell,
            block,
            placers,
        });
    }
    let standing: Vec<String> = rows
        .iter()
        .filter(|r| !r.placers.is_empty())
        .map(|r| {
            format!(
                "\n    `{}` at [{}, {}, {}] — a `{}`",
                r.anchor,
                r.cell[0],
                r.cell[1],
                r.cell[2],
                r.block.as_deref().unwrap_or("?")
            )
        })
        .collect();
    let remedy = || {
        format!(
            "The cells this campaign's signs stand in are:{}\n  A sign is FURNITURE: a piece's \
             template places it (mark an anchor where the piece's sign stands), or a write the \
             story fires places it — a trigger's or an `interact`'s `prop`, a `set-block`, a \
             `fill-region` — and the `signs[]` row letters what stands there. Do NOT type the \
             words into a block string, and do NOT hand-patch the `.nbt`.",
            if standing.is_empty() {
                "\n    (none: no piece and no write puts a sign anywhere a row names)".to_string()
            } else {
                standing.concat()
            }
        )
    };
    let mut bad: Vec<String> = Vec::new();
    for r in &rows {
        if r.placers.is_empty() {
            let found = load
                .get(&r.cell)
                .map(|b| b.as_str())
                .unwrap_or("minecraft:air");
            bad.push(format!(
                "  sign `{}` -> anchor `{}` at [{}, {}, {}] holds `{}`, and no write puts a sign \
                 there",
                r.id, r.anchor, r.cell[0], r.cell[1], r.cell[2], found
            ));
        }
    }
    let all = letters(plan);
    for (sign, _, _, root, ord, path) in &all {
        let Some(r) = rows.iter().find(|r| r.id == *sign) else {
            continue;
        };
        if r.placers.is_empty() {
            continue;
        }
        let at = site_step(plan, root);
        let stands = r.placers.iter().any(|p| match p {
            Placer::Template | Placer::TriggerProp(_) => true,
            Placer::InteractProp { step, .. } => match (step, at) {
                (Some(s), Some(a)) => *s <= a,
                (_, None) => true,
                (None, Some(_)) => false,
            },
            Placer::Write { at: w, .. } => match (w, at) {
                (Some((ws, wo)), Some(a)) => (*ws, *wo) < (a, *ord),
                (Some(_), None) => true,
                (None, _) => false,
            },
        });
        if !stands {
            bad.push(format!(
                "  `letter-sign` at {path} re-letters `{}` at [{}, {}, {}]{}, before any write \
                 puts its sign there ({})",
                r.id,
                r.cell[0],
                r.cell[1],
                r.cell[2],
                at.map(|a| format!(" at step {}", a + 1))
                    .unwrap_or_default(),
                r.placers
                    .iter()
                    .map(Placer::word)
                    .collect::<Vec<_>>()
                    .join("; ")
            ));
        }
    }
    if !bad.is_empty() {
        return Err(crate::compiler::loot::refuse_content_cell(
            &bad,
            format!(
                "{} sign declaration(s) or beat(s) name a cell that holds no sign when it is \
                 lettered. `data merge block` against a cell with no sign FAILS SILENTLY, so \
                 this would have shipped as a blank wall where the words should be.",
                bad.len()
            ),
            remedy(),
        ));
    }
    Ok(SignGate {
        rows,
        letters: all.len(),
    })
}

/// The kind of sign each declared row's cell holds, for the fit check
/// (`DW1048`): the template's, else the first a document writes.
pub fn kinds(gate: &SignGate) -> BTreeMap<String, SignKind> {
    gate.rows
        .iter()
        .filter_map(|r| Some((r.id.clone(), r.kind()?)))
        .collect()
}

/// One measured line of the ledger.
fn measured(text: &str) -> u32 {
    crate::compiler::textfit::sign_line_width(text)
}

/// **The sign binding line** (spec-0115 §5.1), every figure computed from the
/// declarations, `0 sign(s)` included.
pub fn binding_line(plan: &Plan, gate: &SignGate) -> String {
    let c = plan.campaign;
    let signs = &c.quests.content.signs;
    let faces: Vec<&SignFace> = signs
        .iter()
        .flat_map(|s| std::iter::once(&s.front).chain(s.back.as_ref()))
        .collect();
    let lines: usize = faces.iter().map(|f| f.lines.len()).sum();
    let blank: usize = faces
        .iter()
        .map(|f| f.lines.iter().filter(|l| l.is_empty()).count())
        .sum();
    let at_load = gate
        .rows
        .iter()
        .filter(|r| gate.lettered_at_load(&r.id))
        .count();
    let count = |f: &dyn Fn(&Placer) -> bool| -> usize {
        gate.rows
            .iter()
            .map(|r| r.placers.iter().filter(|p| f(p)).count())
            .sum()
    };
    let set = count(&|p| {
        matches!(
            p,
            Placer::Write {
                verb: "set-block",
                ..
            }
        )
    });
    let fill = count(&|p| {
        matches!(
            p,
            Placer::Write {
                verb: "fill-region",
                ..
            }
        )
    });
    let prop = count(&|p| matches!(p, Placer::TriggerProp(_) | Placer::InteractProp { .. }));
    let kinds = kinds(gate);
    let mut widest: BTreeMap<SignKind, u32> = BTreeMap::new();
    for l in delvewright_dsl::sign_lines(c) {
        let Some(k) = kinds.get(&l.sign) else {
            continue;
        };
        let mut w = measured(&l.text);
        for doc in plan.sidecars.values() {
            if let Some(t) = doc.content.get(&l.key) {
                w = w.max(measured(t));
            }
        }
        let e = widest.entry(*k).or_insert(0);
        *e = (*e).max(w);
    }
    format!(
        "sign binding: {} sign(s), {} face(s), {} line(s) ({} blank); {} lettered at load, {} \
         after a write ({} set-block, {} prop, {} fill), {} re-lettered by letter-sign; widest \
         line {} of 90 px (standing) and {} of 60 px (hanging) over {} language(s)",
        signs.len(),
        faces.len(),
        lines,
        blank,
        at_load,
        set + prop + fill,
        set,
        prop,
        fill,
        gate.letters,
        widest.get(&SignKind::Standing).copied().unwrap_or(0),
        widest.get(&SignKind::Hanging).copied().unwrap_or(0),
        1 + plan.sidecars.len(),
    )
}

/// `validation/signs.json` (spec-0115 §5.1): one row per declared sign.
pub fn ledger(plan: &Plan, gate: &SignGate) -> serde_json::Value {
    let c = plan.campaign;
    let lines = delvewright_dsl::sign_lines(c);
    let rows: Vec<serde_json::Value> = gate
        .rows
        .iter()
        .map(|r| {
            let kind = r.kind();
            let mut faces: BTreeMap<String, BTreeMap<String, Vec<serde_json::Value>>> =
                BTreeMap::new();
            for l in lines.iter().filter(|l| l.sign == r.id) {
                let face = format!(
                    "{}:{}",
                    l.side.token(),
                    l.key.rsplit_once('.').map_or("", |(b, _)| b)
                );
                let mut langs = vec![(
                    "en".to_string(),
                    crate::compiler::textfit::authored(&l.text).to_string(),
                )];
                for (lang, doc) in &plan.sidecars {
                    if let Some(t) = doc.content.get(&l.key) {
                        langs.push((lang.clone(), t.clone()));
                    }
                }
                for (lang, text) in langs {
                    faces
                        .entry(face.clone())
                        .or_default()
                        .entry(lang)
                        .or_default()
                        .push(serde_json::json!({
                            "line": l.index,
                            "key": l.key,
                            "text": text,
                            "width": measured(&text),
                        }));
                }
            }
            serde_json::json!({
                "id": r.id,
                "anchor": r.anchor,
                "cell": r.cell,
                "block": r.block,
                "kind": kind.map(|k| k.word()),
                "budget": kind.map(SignKind::budget),
                "placed_by": r.placers.iter().map(Placer::word).collect::<Vec<_>>(),
                "lettered_in": lettered_in(r),
                "faces": faces,
            })
        })
        .collect();
    serde_json::json!({
        "examined": gate.rows.len(),
        "declared": c.quests.content.signs.len(),
        "letter_signs": gate.letters,
        "signs": rows,
    })
}

/// The configurations a row stands lettered in, in words: from load, or
/// from the step its first write fires at.
fn lettered_in(r: &SignRow) -> Vec<String> {
    let mut out: BTreeSet<String> = BTreeSet::new();
    for p in &r.placers {
        out.insert(match p {
            Placer::Template | Placer::TriggerProp(_) => "from load".to_string(),
            Placer::InteractProp { step: Some(s), .. } => format!("from step {}", s + 1),
            Placer::Write {
                at: Some((s, _)), ..
            } => format!("after step {}", s + 1),
            Placer::InteractProp { step: None, .. } | Placer::Write { at: None, .. } => {
                "unforced".to_string()
            }
        });
    }
    out.into_iter().collect()
}

/// **The block entities a configuration's written world lays** (spec-0115
/// §4.3): for every declared sign whose cell holds a sign block in `blocks`,
/// its compound after the latest `letter-sign` that fires before step
/// `arrival` of `steps`, with every line tagged by its key as the build tags
/// it.
pub fn entities_at(
    plan: &Plan,
    blocks: &BlockMap,
    steps: &[Step],
    arrival: usize,
) -> crate::compiler::view::world::BlockEntities {
    use crate::compiler::emit::sign::{FaceValue, SignCompound};
    let c = plan.campaign;
    let ns = delvewright_dsl::pack_namespace(c.world.campaign_id.as_str());
    let keybases: BTreeMap<String, String> = delvewright_dsl::stated_faces(c)
        .into_iter()
        .map(|(_, _, _, keybase, _, path)| (path, format!("{ns}{keybase}")))
        .collect();
    let mut fired: Vec<(usize, usize, Letter<'_>)> = letters(plan)
        .into_iter()
        .filter_map(|l| {
            let s = step_on(steps, &l.3)?;
            (s < arrival).then_some((s, l.4, l))
        })
        .collect();
    fired.sort_by_key(|(s, o, _)| (*s, *o));
    let mut out = crate::compiler::view::world::BlockEntities::new();
    for s in &plan.signs {
        let Some(kind) = blocks.get(&s.cell).and_then(|b| SignKind::of(b.as_str())) else {
            continue;
        };
        let Some(row) = c
            .quests
            .content
            .signs
            .iter()
            .find(|r| r.id.as_str() == s.id)
        else {
            continue;
        };
        let mut compound = SignCompound::declared(row, Some(&ns));
        for (_, _, (sign, front, back, _, _, path)) in &fired {
            if *sign != s.id {
                continue;
            }
            if let Some(f) = front {
                let k = keybases.get(&format!("{path}/front")).map(String::as_str);
                compound.front = FaceValue::of(f, k);
            }
            if let Some(b) = back {
                let k = keybases.get(&format!("{path}/back")).map(String::as_str);
                compound.back = FaceValue::of(b, k);
            }
        }
        out.insert(
            s.cell,
            crate::compiler::view::world::BlockEntity {
                id: match kind {
                    SignKind::Standing => "minecraft:sign".to_string(),
                    SignKind::Hanging => "minecraft:hanging_sign".to_string(),
                },
                data: compound.nbt(),
            },
        );
    }
    out
}
