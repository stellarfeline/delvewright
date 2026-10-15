//! The compass, the journal and the highlight, driven on the framework dummy
//! (spec-0104 §6–§8).
//!
//! Every template here is one atomic function: it saves every `#party` score
//! it writes, drives the quest state to exactly the objectives it means to
//! have open, calls the engine's own functions as its dummy, asserts, and
//! restores what it saved — so a sibling holding the state across ticks (the
//! campaign playthrough) never sees it move.

use super::*;
use crate::compiler::emit::guidance::{
    COMPASS_DATA, GUIDE, Guide, HlBody, JOURNAL_DATA, PICK, TRACK, Target, Watched,
};

/// Save, drive and restore `#party` scores for one template.
struct Drive {
    suffix: String,
    saved: Vec<String>,
    set: Vec<String>,
}

impl Drive {
    fn new(suffix: &str) -> Self {
        Drive {
            suffix: suffix.to_string(),
            saved: Vec::new(),
            set: Vec::new(),
        }
    }

    /// Save `#party <obj>` once, and set it to `v`.
    fn put(&mut self, obj: &str, v: i32) {
        if !self.saved.iter().any(|s| s == obj) {
            self.saved.push(obj.to_string());
        }
        self.set
            .push(format!("scoreboard players set {} {obj} {v}", plan::PARTY));
    }

    fn holder(&self, i: usize) -> String {
        format!("#sv_{}_{i}", self.suffix)
    }

    /// The save lines (run first), then the drive lines.
    fn open(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .saved
            .iter()
            .enumerate()
            .map(|(i, obj)| {
                format!(
                    "execute store result score {} {GUIDE} run scoreboard players get {} {obj}",
                    self.holder(i),
                    plan::PARTY
                )
            })
            .collect();
        v.extend(self.set.iter().cloned());
        v
    }

    /// The restore lines (run last).
    fn close(&self) -> Vec<String> {
        self.saved
            .iter()
            .enumerate()
            .map(|(i, obj)| {
                format!(
                    "scoreboard players operation {} {obj} = {} {GUIDE}",
                    plan::PARTY,
                    self.holder(i)
                )
            })
            .collect()
    }
}

/// Drive the quest state so that exactly `open` are pending: every quest shut
/// but theirs; in theirs, every objective complete but the open ones and the
/// objectives that follow them; each open objective's gate opened.
fn isolate(plan: &Plan, open: &[&Watched], d: &mut Drive) {
    let c = plan.campaign;
    let quests: BTreeSet<usize> = open.iter().map(|w| w.quest).collect();
    let open_ids: BTreeSet<&str> = open.iter().map(|w| w.id.as_str()).collect();
    for q in &c.quests.content.quests {
        d.put(&quest_active_score(q.id.as_str()), 0);
    }
    for qi in &quests {
        let q = &c.quests.content.quests[*qi];
        d.put(&quest_active_score(q.id.as_str()), 1);
        for o in &q.objectives {
            let follows = o.after().iter().any(|a| open_ids.contains(a.as_str()));
            let keep_open = open_ids.contains(o.id().as_str()) || follows;
            d.put(&obj_score(o.id().as_str()), if keep_open { 0 } else { 1 });
        }
    }
    for w in open {
        let o = &c.quests.content.quests[w.quest].objectives[w.objective];
        let g = o.gate();
        for f in g.requires_flags {
            d.put(&plan::flag_score(f.as_str()), 1);
        }
        for f in g.forbids_flags {
            d.put(&plan::flag_score(f.as_str()), 0);
        }
    }
}

/// Whether `w` can be made pending by [`isolate`]: its gate reads flags only.
fn drivable(plan: &Plan, w: &Watched) -> bool {
    let o = &plan.campaign.quests.content.quests[w.quest].objectives[w.objective];
    o.requires_state().is_empty()
}

/// Whether `a` (transitively) follows `b` inside their quest.
fn follows(plan: &Plan, a: &Watched, b: &Watched) -> bool {
    if a.quest != b.quest {
        return false;
    }
    let q = &plan.campaign.quests.content.quests[a.quest];
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut stack: Vec<String> = q.objectives[a.objective]
        .after()
        .iter()
        .map(|x| x.as_str().to_string())
        .collect();
    while let Some(x) = stack.pop() {
        if x == b.id {
            return true;
        }
        if !seen.insert(x.clone()) {
            continue;
        }
        if let Some(o) = q.objectives.iter().find(|o| o.id().as_str() == x) {
            stack.extend(o.after().iter().map(|y| y.as_str().to_string()));
        }
    }
    false
}

/// The key (or text) a lowered string is matched by inside a page.
fn match_of(v: &Value) -> String {
    match v.get("translate").and_then(Value::as_str) {
        Some(k) => format!(
            "translate:{},fallback:{}",
            snbt_string(k),
            snbt_string(v.get("fallback").and_then(Value::as_str).unwrap_or(""))
        ),
        None => format!(
            "text:{}",
            snbt_string(v.get("text").and_then(Value::as_str).unwrap_or(""))
        ),
    }
}

/// Count the journal pages whose `extra` holds every compound in `parts`, into
/// `holder`, whichever way the page is stored (bare or as `{raw: …}`).
fn count_pages(holder: &str, parts: &[String]) -> Vec<String> {
    let extra = format!("{{extra:[{}]}}", parts.join(","));
    let base = format!(
        "entity @s Inventory[{{components:{{\"minecraft:custom_data\":{JOURNAL_DATA}}}}}].components.\"minecraft:written_book_content\".pages"
    );
    vec![
        format!("execute store result score {holder}a {GUIDE} if data {base}[{{raw:{extra}}}]"),
        format!("execute store result score {holder}b {GUIDE} if data {base}[{extra}]"),
        format!("scoreboard players operation {holder} {GUIDE} = {holder}a {GUIDE}"),
        format!("scoreboard players operation {holder} {GUIDE} += {holder}b {GUIDE}"),
    ]
}

fn tracker(p: [i32; 3]) -> String {
    format!(
        "minecraft:compass[minecraft:lodestone_tracker={{target:{{pos:[I;{},{},{}],dimension:\"minecraft:overworld\"}},tracked:false}}]",
        p[0], p[1], p[2]
    )
}

fn compass_held(holder: &str, item: &str) -> Vec<String> {
    vec![
        format!(
            "execute store result score {holder}a {GUIDE} if items entity @s container.* {item}"
        ),
        format!(
            "execute store result score {holder}b {GUIDE} if items entity @s weapon.offhand {item}"
        ),
        format!("scoreboard players operation {holder} {GUIDE} = {holder}a {GUIDE}"),
        format!("scoreboard players operation {holder} {GUIDE} += {holder}b {GUIDE}"),
    ]
}

/// The compass's templates (§6): aim, cycle, spin and re-give; and a
/// `talk-to`'s target following the body.
pub(super) fn emit_compass_packtests(plan: &Plan, out: &mut BuildOutput, g: &Guide) {
    if !g.compass {
        return;
    }
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let tracked: Vec<&Watched> = g.tracked().filter(|w| drivable(plan, w)).collect();
    let cell = |w: &Watched| match &w.target {
        Some(Target::Cell(p)) => Some(*p),
        _ => None,
    };
    // Two that can be open at once, the first with a fixed cell.
    let pair = tracked.iter().enumerate().find_map(|(i, a)| {
        cell(a)?;
        tracked[i + 1..]
            .iter()
            .find(|b| cell(b).is_some() && !follows(plan, b, a) && !follows(plan, a, b))
            .map(|b| (*a, *b))
    });
    let one = tracked.iter().find(|w| cell(w).is_some()).copied();
    let Some(first) = pair.map(|p| p.0).or(one) else {
        return;
    };
    let mut d = Drive::new("cmp");
    let open: Vec<&Watched> = match pair {
        Some((a, b)) => vec![a, b],
        None => vec![first],
    };
    isolate(plan, &open, &mut d);
    let mut b = packtest_header(&format!(
        "{title}: the compass aims at the followed objective, lists the open ones in its menu, follows a pick, spins when nothing is open, and comes back when lost (spec-0104 §6)"
    ));
    b.push(format!("function {ns}:setup"));
    b.push("clear @s".to_string());
    b.push(format!("function {ns}:guide_seed"));
    b.extend(d.open());
    b.push(format!("scoreboard players set @s {TRACK} 0"));
    b.push(format!("function {ns}:compass_give"));
    b.push(format!("function {ns}:compass_refresh"));
    let k1 = first.ordinal.expect("tracked");
    b.push(format!("assert score @s {TRACK} matches {k1}"));
    b.extend(compass_held(
        "#cmp_a",
        &tracker(cell(first).expect("a cell")),
    ));
    b.push(format!("assert score #cmp_a {GUIDE} matches 1"));
    if let Some((_, second)) = pair {
        let k2 = second.ordinal.expect("tracked");
        // The menu holds the two open ones, each button its own pick.
        b.push(format!("function {ns}:compass_menu"));
        b.push(format!(
            "execute store result score #cmp_m {GUIDE} run data get storage {ns}:guide menu.dialog.actions"
        ));
        b.push(format!("assert score #cmp_m {GUIDE} matches 2"));
        for k in [k1, k2] {
            b.push(format!(
                "execute store result score #cmp_k {GUIDE} if data storage {ns}:guide menu.dialog.actions[{{action:{{command:\"trigger {PICK} set {k}\"}}}}]"
            ));
            b.push(format!("assert score #cmp_k {GUIDE} matches 1"));
        }
        // A pick of the second moves the needle to it, and back.
        b.push(format!("scoreboard players set @s {PICK} {k2}"));
        b.push(format!("function {ns}:compass_pick"));
        b.push(format!("assert score @s {TRACK} matches {k2}"));
        b.push(format!("assert score @s {PICK} matches 0"));
        b.extend(compass_held(
            "#cmp_b",
            &tracker(cell(second).expect("a cell")),
        ));
        b.push(format!("assert score #cmp_b {GUIDE} matches 1"));
        b.push(format!("scoreboard players set @s {PICK} {k1}"));
        b.push(format!("function {ns}:compass_pick"));
        b.push(format!("assert score @s {TRACK} matches {k1}"));
    }
    // Everything done: the needle spins.
    for w in &open {
        b.push(format!(
            "scoreboard players set {} {} 1",
            plan::PARTY,
            obj_score(&w.id)
        ));
    }
    // …and the objectives that followed them shut with their quests, so
    // nothing at all is open.
    for qi in open.iter().map(|w| w.quest).collect::<BTreeSet<_>>() {
        b.push(format!(
            "scoreboard players set {} {} 0",
            plan::PARTY,
            quest_active_score(plan.campaign.quests.content.quests[qi].id.as_str())
        ));
    }
    b.push(format!("function {ns}:compass_refresh"));
    b.push(format!("assert score @s {TRACK} matches 0"));
    b.extend(compass_held(
        "#cmp_s",
        "minecraft:compass[minecraft:lodestone_tracker={tracked:false}]",
    ));
    b.push(format!("assert score #cmp_s {GUIDE} matches 1"));
    // Lost: the next refresh gives it back, once.
    b.push("clear @s".to_string());
    b.push(format!("function {ns}:compass_refresh"));
    b.push(format!("function {ns}:compass_refresh"));
    b.extend(compass_held(
        "#cmp_g",
        &format!("minecraft:compass[custom_data~{COMPASS_DATA}]"),
    ));
    b.push(format!("assert score #cmp_g {GUIDE} matches 1"));
    b.extend(d.close());
    b.push("clear @s".to_string());
    out.insert(
        format!("packtest-datapack/data/{ns}/test/guide_compass.mcfunction"),
        lines(&b).into_bytes(),
    );

    // A `talk-to`'s target is the body where it stands now.
    let Some(w) = g
        .tracked()
        .filter(|w| drivable(plan, w))
        .find(|w| matches!(w.target, Some(Target::Body(_))))
    else {
        return;
    };
    let Some(Target::Body(tag)) = &w.target else {
        return;
    };
    let q = &plan.campaign.quests.content.quests[w.quest];
    let o = &q.objectives[w.objective];
    let Some(home) = crate::compiler::promise::objective_place(plan, q, o) else {
        return;
    };
    let mut d = Drive::new("cmpn");
    isolate(plan, &[w], &mut d);
    let sel = format!("@e[tag={tag},type=!minecraft:interaction]");
    let mut b = packtest_header(&format!(
        "{title}: a compass following `{}` points at the person where they stand (spec-0104 §6.3)",
        w.id
    ));
    b.push(format!("function {ns}:setup"));
    b.push("clear @s".to_string());
    b.push(format!("function {ns}:guide_seed"));
    b.extend(d.open());
    b.push(format!("scoreboard players set @s {TRACK} 0"));
    b.push(format!("function {ns}:compass_give"));
    // A sibling may have despawned or walked the person; one body of its tag
    // stands at the station for this template, so the read below has one to
    // read, and is removed after.
    b.push(format!(
        "summon minecraft:armor_stand {}.5 {} {}.5 {{Invisible:1b,NoGravity:1b,Marker:1b,Tags:[\"{tag}\",\"dw_pt_cmpn\"]}}",
        home[0], home[1], home[2]
    ));
    b.push(format!(
        "execute store result score #cmpn_n {GUIDE} if entity {sel}"
    ));
    b.push(format!("assert score #cmpn_n {GUIDE} matches 1.."));
    // The compass points where the body stands now, whatever moved it
    // there: its `Pos` is read and compared with the tracker's target. The body
    // is not moved here: a `tp` or a `data modify … Pos` of a non-player body
    // leaves it unselectable until the tick ends (observed on the pinned
    // server), and a template is one tick.
    b.push(format!("function {ns}:compass_refresh"));
    let one = sel.replace(']', ",limit=1]");
    let target = format!(
        "entity @s Inventory[{{components:{{\"minecraft:custom_data\":{COMPASS_DATA}}}}}].components.\"minecraft:lodestone_tracker\".target.pos"
    );
    for (i, axis) in ["x", "y", "z"].iter().enumerate() {
        b.push(format!(
            "execute store result score #cmpn_b{axis} {GUIDE} run data get entity {one} Pos[{i}]"
        ));
        b.push(format!(
            "execute store result score #cmpn_c{axis} {GUIDE} run data get {target}[{i}]"
        ));
        b.push(format!(
            "execute store success score #cmpn_e{axis} {GUIDE} if score #cmpn_b{axis} {GUIDE} = #cmpn_c{axis} {GUIDE}"
        ));
        b.push(format!("assert score #cmpn_e{axis} {GUIDE} matches 1"));
    }
    b.push("kill @e[tag=dw_pt_cmpn]".to_string());
    b.extend(d.close());
    b.push("clear @s".to_string());
    out.insert(
        format!("packtest-datapack/data/{ns}/test/guide_compass_person.mcfunction"),
        lines(&b).into_bytes(),
    );
}

/// The journal's templates (§8): pages open and done, the mark, a dropped book
/// removed and given back, and a joining player's book written from the state.
pub(super) fn emit_journal_packtests(
    plan: &Plan,
    out: &mut BuildOutput,
    g: &Guide,
    chrome: &delvewright_dsl::Chrome,
) {
    if !g.journal {
        return;
    }
    let ns = &plan.namespace;
    let c = plan.campaign;
    let title = artifact_title(c);
    let open_key = match_of(&tr(&chrome.get(delvewright_dsl::chrome::JOURNAL_OPEN)));
    let mark_key = match_of(&tr(&chrome.get(delvewright_dsl::chrome::JOURNAL_FOLLOWING)));
    let title_of = |w: &Watched| {
        let o = &c.quests.content.quests[w.quest].objectives[w.objective];
        match_of(&tr(o.title().unwrap_or_default()))
    };
    let Some(first) = g.announced().find(|w| drivable(plan, w)) else {
        return;
    };
    let t1 = title_of(first);
    let next = g.announced().find(|w| {
        w.quest == first.quest
            && drivable(plan, w)
            && c.quests.content.quests[w.quest].objectives[w.objective]
                .after()
                .iter()
                .any(|a| a.as_str() == first.id)
    });

    // Pages open and done, and the mark.
    let mut d = Drive::new("jnl");
    isolate(plan, &[first], &mut d);
    if let Some(n) = next {
        let o = &c.quests.content.quests[n.quest].objectives[n.objective];
        let gate = o.gate();
        for f in gate.requires_flags {
            d.put(&plan::flag_score(f.as_str()), 1);
        }
        for f in gate.forbids_flags {
            d.put(&plan::flag_score(f.as_str()), 0);
        }
    }
    let mut b = packtest_header(&format!(
        "{title}: the journal lists `{}` while it is open, marks it when followed, and strikes it through when done (spec-0104 §8)",
        first.id
    ));
    b.push(format!("function {ns}:setup"));
    b.push("clear @s".to_string());
    b.push(format!("function {ns}:guide_seed"));
    b.extend(d.open());
    b.push(format!("scoreboard players set @s {TRACK} 0"));
    b.push(format!("function {ns}:journal_rewrite"));
    b.push(format!(
        "execute store result score #jnl_h {GUIDE} if items entity @s container.* minecraft:written_book[custom_data~{JOURNAL_DATA}]"
    ));
    b.push(format!("assert score #jnl_h {GUIDE} matches 1"));
    let open_page = vec![format!("{{{open_key}}}"), format!("{{bold:1b,{t1}}}")];
    let done_page = vec![format!("{{strikethrough:1b,{t1}}}")];
    b.extend(count_pages("#jnl_o", &open_page));
    b.push(format!("assert score #jnl_o {GUIDE} matches 1"));
    b.extend(count_pages("#jnl_d", &done_page));
    b.push(format!("assert score #jnl_d {GUIDE} matches 0"));
    if let Some(k) = first.ordinal {
        let marked = vec![format!("{{bold:1b,{t1}}}"), format!("{{{mark_key}}}")];
        b.extend(count_pages("#jnl_m", &marked));
        b.push(format!("assert score #jnl_m {GUIDE} matches 0"));
        b.push(format!("scoreboard players set @s {TRACK} {k}"));
        b.push(format!("function {ns}:journal_rewrite"));
        b.extend(count_pages("#jnl_m", &marked));
        b.push(format!("assert score #jnl_m {GUIDE} matches 1"));
    }
    b.push(format!(
        "scoreboard players set {} {} 1",
        plan::PARTY,
        obj_score(&first.id)
    ));
    b.push(format!("function {ns}:journal_rewrite"));
    b.extend(count_pages("#jnl_o", &open_page));
    b.push(format!("assert score #jnl_o {GUIDE} matches 0"));
    b.extend(count_pages("#jnl_d", &done_page));
    b.push(format!("assert score #jnl_d {GUIDE} matches 1"));
    if let Some(n) = next {
        let tn = title_of(n);
        b.extend(count_pages("#jnl_n", &[format!("{{bold:1b,{tn}}}")]));
        b.push(format!("assert score #jnl_n {GUIDE} matches 1"));
    }
    b.extend(d.close());
    b.push("clear @s".to_string());
    out.insert(
        format!("packtest-datapack/data/{ns}/test/guide_journal_pages.mcfunction"),
        lines(&b).into_bytes(),
    );

    // A dropped book is removed, and given back at the next rewrite.
    let mut b = packtest_header(&format!(
        "{title}: a dropped journal is removed and given back (spec-0104 §8.4)"
    ));
    b.push(format!("function {ns}:setup"));
    b.push("clear @s".to_string());
    b.push(format!("function {ns}:guide_seed"));
    b.push(format!(
        "summon minecraft:item ~ ~1 ~ {{Item:{{id:\"minecraft:written_book\",count:1,components:{{\"minecraft:custom_data\":{JOURNAL_DATA}}}}},Tags:[\"dw_pt_jdrop\"]}}"
    ));
    b.push(format!("function {ns}:guide_strays"));
    b.push(format!(
        "execute store result score #jdrop_e {GUIDE} if entity @e[type=minecraft:item,tag=dw_pt_jdrop]"
    ));
    b.push(format!("assert score #jdrop_e {GUIDE} matches 0"));
    b.push(format!("function {ns}:journal_rewrite"));
    b.push(format!(
        "execute store result score #jdrop_h {GUIDE} if items entity @s container.* minecraft:written_book[custom_data~{JOURNAL_DATA}]"
    ));
    b.push(format!("assert score #jdrop_h {GUIDE} matches 1"));
    b.push("clear @s".to_string());
    out.insert(
        format!("packtest-datapack/data/{ns}/test/guide_journal_dropped.mcfunction"),
        lines(&b).into_bytes(),
    );

    // A player who joins with no book is given one written from the state.
    let mut d = Drive::new("jjn");
    isolate(plan, &[first], &mut d);
    let mut b = packtest_header(&format!(
        "{title}: a player who joins is given a journal written from the current state (spec-0104 §8.4)"
    ));
    b.push(format!("function {ns}:setup"));
    b.push("clear @s".to_string());
    b.extend(d.open());
    b.push("tag @s remove dw_guide_seeded".to_string());
    b.push(format!("function {ns}:guide_seed"));
    b.push(format!("function {ns}:guide_refresh"));
    b.extend(count_pages("#jjn_o", &open_page));
    b.push(format!("assert score #jjn_o {GUIDE} matches 1"));
    b.extend(d.close());
    b.push("clear @s".to_string());
    out.insert(
        format!("packtest-datapack/data/{ns}/test/guide_journal_join.mcfunction"),
        lines(&b).into_bytes(),
    );
}

/// The highlight's templates (§7.3): a block or a person lit while a dummy is
/// within range and the act is armed, and dark otherwise.
pub(super) fn emit_highlight_packtests(plan: &Plan, out: &mut BuildOutput, g: &Guide) {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let title = artifact_title(c);
    for h in &g.highlights {
        // Where the dummy stands to be near, and where it stands to be far.
        let at = match &h.body {
            HlBody::Blocks { at, .. } => *at,
            HlBody::Bodies { npc: true, .. } => {
                let Some(npc) = h
                    .objective
                    .as_deref()
                    .and_then(|oid| {
                        c.quests
                            .content
                            .quests
                            .iter()
                            .flat_map(|q| q.objectives.iter().map(move |o| (q, o)))
                            .find(|(_, o)| o.id().as_str() == oid)
                    })
                    .and_then(|(q, o)| crate::compiler::promise::objective_place(plan, q, o))
                else {
                    continue;
                };
                npc
            }
            // A wave's bodies exist only once it is spawned.
            HlBody::Bodies { npc: false, .. } => continue,
        };
        let mut d = Drive::new(&format!("hl{}", h.safe));
        let s = &h.safe;
        let hl = format!("#hl_{s}");
        let lit = match &h.body {
            HlBody::Blocks { .. } => format!("@e[tag=dw_hl_{s},nbt={{Glowing:1b}}]"),
            HlBody::Bodies { tag, .. } => {
                format!("@e[tag={tag},type=!minecraft:interaction,team=dw_hl]")
            }
        };
        let mut arm: Vec<String> = Vec::new();
        // `None`: the act cannot be shut without changing what it is (a
        // repeatable trigger with no flag gate), so only range is judged.
        let mut disarm: Option<Vec<String>> = None;
        let mut latch: Option<String> = None;
        match (&h.objective, &h.trigger) {
            (Some(oid), _) => {
                let st = Guide::state_holder(&plan::safe_local(oid));
                arm.push(format!("scoreboard players set {st} {GUIDE} 1"));
                disarm = Some(vec![format!("scoreboard players set {st} {GUIDE} 0")]);
            }
            (None, Some(tid)) => {
                let Some(t) = c
                    .quests
                    .content
                    .triggers
                    .iter()
                    .find(|t| t.id.as_str() == tid)
                else {
                    continue;
                };
                if !t.requires_state.is_empty() {
                    continue;
                }
                for f in &t.requires_flags {
                    d.put(&plan::flag_score(f.as_str()), 1);
                }
                for f in &t.forbids_flags {
                    d.put(&plan::flag_score(f.as_str()), 0);
                }
                if t.once {
                    let l = format!("#trig_{}", plan::safe_local(tid));
                    arm.push(format!("scoreboard players set {l} dw.sys 0"));
                    disarm = Some(vec![format!("scoreboard players set {l} dw.sys 1")]);
                    latch = Some(l);
                } else if let Some(f) = t.requires_flags.first() {
                    let fs = plan::flag_score(f.as_str());
                    arm.push(format!("scoreboard players set {} {fs} 1", plan::PARTY));
                    disarm = Some(vec![format!(
                        "scoreboard players set {} {fs} 0",
                        plan::PARTY
                    )]);
                }
            }
            _ => continue,
        }
        let mut b = packtest_header(&format!(
            "{title}: the highlight on `{}` lights within {} blocks while armed, and is dark beyond it and before it is armed (spec-0104 §7.3)",
            h.objective.as_deref().or(h.trigger.as_deref()).unwrap_or(s),
            h.within
        ));
        b.push(format!("function {ns}:setup"));
        let (pin, me) = pin_dummy(&format!("dw_pt_{s}"));
        b.push(pin);
        if let Some(l) = &latch {
            b.push(format!(
                "execute store result score #sv_{s}_latch {GUIDE} run scoreboard players get {l} dw.sys"
            ));
        }
        b.extend(d.open());
        let near = i32::from(h.within) - 1;
        let far = i32::from(h.within) + 2;
        let pos = |dx: i32| format!("{}.5 {} {}.5", at[0] + dx, at[1], at[2]);
        if matches!(h.body, HlBody::Blocks { .. }) {
            b.push(format!("function {ns}:hl_show_{s}"));
        }
        // Start dark, whatever a sibling or the tick left lit.
        b.push(format!("scoreboard players set {hl} {GUIDE} 1"));
        b.push(format!("function {ns}:hl_off_{s}"));
        b.push(format!("tp {me} {}", pos(near)));
        // Not armed: dark, however near.
        if let Some(dis) = &disarm {
            b.extend(dis.iter().cloned());
            b.push(format!("function {ns}:hl_poll_{s}"));
            b.push(format!(
                "execute store result score #{s}_n {GUIDE} if entity {lit}"
            ));
            b.push(format!("assert score #{s}_n {GUIDE} matches 0"));
        }
        // Armed and near: lit.
        b.extend(arm.iter().cloned());
        b.push(format!("function {ns}:hl_poll_{s}"));
        b.push(format!(
            "execute store result score #{s}_n {GUIDE} if entity {lit}"
        ));
        b.push(format!("assert score #{s}_n {GUIDE} matches 1.."));
        // Armed and far: dark. Every other player within the range — a
        // sibling's dummy standing at the same thing — is held out of play for
        // this one poll, as a watcher is, and handed back at once.
        b.push(format!("tp {me} {}", pos(far)));
        let hold = format!("dw_pt_hold_{s}");
        let probe = match &h.body {
            HlBody::Blocks { at, .. } => format!(
                "execute positioned {}.5 {}.5 {}.5 run tag @a[distance=..{},tag=!{CUTSCENE_TAG}] add {hold}",
                at[0],
                at[1],
                at[2],
                i32::from(h.within) + 1
            ),
            HlBody::Bodies { tag, npc } => {
                let sel = if *npc {
                    format!("@e[tag={tag},type=!minecraft:interaction]")
                } else {
                    format!("@e[tag={tag}]")
                };
                format!(
                    "execute as {sel} at @s run tag @a[distance=..{},tag=!{CUTSCENE_TAG}] add {hold}",
                    i32::from(h.within) + 1
                )
            }
        };
        b.push(probe);
        b.push(format!("tag @a[tag={hold}] add {CUTSCENE_TAG}"));
        b.push(format!("function {ns}:hl_poll_{s}"));
        b.push(format!("tag @a[tag={hold}] remove {CUTSCENE_TAG}"));
        b.push(format!("tag @a[tag={hold}] remove {hold}"));
        b.push(format!(
            "execute store result score #{s}_n {GUIDE} if entity {lit}"
        ));
        b.push(format!("assert score #{s}_n {GUIDE} matches 0"));
        // Leave nothing lit, and the state as it was.
        b.push(format!("function {ns}:hl_off_{s}"));
        if let Some(l) = &latch {
            b.push(format!(
                "scoreboard players operation {l} dw.sys = #sv_{s}_latch {GUIDE}"
            ));
        }
        b.extend(d.close());
        if h.objective.is_some() {
            b.push(format!("kill @e[tag=dw_hl_{s}]"));
        }
        out.insert(
            format!("packtest-datapack/data/{ns}/test/guide_highlight_{s}.mcfunction"),
            lines(&b).into_bytes(),
        );
    }
}
