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
    COMPASS_DATA, GUIDE, Guide, HlAct, HlBody, JOURNAL_DATA, PICK, TRACK, Target, Watched,
    display_entity,
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
        "summon minecraft:armor_stand {} {} {} {{Invisible:1b,NoGravity:1b,Marker:1b,Tags:[\"{tag}\",\"dw_pt_cmpn\"]}}",
        center(home[0]),
        corner(home[1]),
        center(home[2])
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
            // A runtime marker has no compile-time cell to stand a dummy by.
            HlBody::Runtime { .. } => continue,
            HlBody::Blocks { at, .. } | HlBody::Holder { at, .. } => *at,
            HlBody::Bodies { .. } => {
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
        };
        let mut d = Drive::new(&format!("hl{}", h.safe));
        let s = &h.safe;
        let hl = format!("#hl_{s}");
        let lit = match &h.body {
            HlBody::Blocks { .. } => format!("@e[tag=dw_hl_{s},nbt={{Glowing:1b}}]"),
            HlBody::Bodies { tag, .. } => {
                format!("@e[tag={tag},type=!minecraft:interaction,team=dw_hl]")
            }
            HlBody::Holder { tag, .. } => {
                format!("@e[tag={tag},team=dw_hl,nbt={{Glowing:1b}}]")
            }
            HlBody::Runtime { .. } => continue,
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
        // A wave's bodies exist only once it is spawned: one stand-in body of
        // its tag stands at its anchor for this template, removed after.
        let stand_in = matches!(h.body, HlBody::Bodies { npc: false, .. });
        if let (true, HlBody::Bodies { tag, .. }) = (stand_in, &h.body) {
            b.push(format!(
                "summon minecraft:armor_stand {} {} {} {{Invisible:1b,NoGravity:1b,Tags:[\"{tag}\",\"dw_pt_hlw\"]}}",
                center(at[0]),
                corner(at[1]),
                center(at[2])
            ));
        }
        // A holder stands by its own body function, as the build stands it.
        if let HlBody::Holder { body_fn, .. } = &h.body {
            b.push(format!("function {ns}:{body_fn}"));
        }
        if let Some(l) = &latch {
            b.push(format!(
                "execute store result score #sv_{s}_latch {GUIDE} run scoreboard players get {l} dw.sys"
            ));
        }
        b.extend(d.open());
        stage_cells(&h.body, &mut b);
        let far = i32::from(h.within) + 2;
        let pos = |dx: i32| format!("{} {} {}", center(at[0] + dx), corner(at[1]), center(at[2]));
        let Some(near_pos) = near_stand(h, at) else {
            continue;
        };
        // Start dark, whatever a sibling or the tick left lit.
        b.push(format!("scoreboard players set {hl} {GUIDE} 1"));
        b.push(format!("function {ns}:hl_off_{s}"));
        b.push(format!("tp {me} {near_pos}"));
        // Not armed: dark, however near.
        if let Some(dis) = &disarm {
            b.extend(dis.iter().cloned());
            b.push(format!("function {ns}:hl_poll_{s}"));
            b.push(format!(
                "execute store result score #{s}_n {GUIDE} if entity {lit}"
            ));
            b.push(format!("assert score #{s}_n {GUIDE} matches 0"));
            assert_no_display(&h.body, s, "u", &mut b);
        }
        // Armed and near: lit.
        b.extend(arm.iter().cloned());
        b.push(format!("function {ns}:hl_poll_{s}"));
        b.push(format!(
            "execute store result score #{s}_n {GUIDE} if entity {lit}"
        ));
        b.push(format!("assert score #{s}_n {GUIDE} matches 1.."));
        assert_displays_stand(&h.body, s, &mut b);
        // Armed and far: dark. Every other player within the range — a
        // sibling's dummy standing at the same thing — is held out of play for
        // this one poll, as a watcher is, and handed back at once.
        b.push(format!("tp {me} {}", pos(far)));
        let hold = format!("dw_pt_hold_{s}");
        let probe = match &h.body {
            HlBody::Blocks { at, .. } | HlBody::Holder { at, .. } => format!(
                "execute positioned {} {} {} run tag @a[distance=..{},tag=!{CUTSCENE_TAG}] add {hold}",
                center(at[0]),
                center(at[1]),
                center(at[2]),
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
            HlBody::Runtime { .. } => continue,
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
        assert_no_display(&h.body, s, "f", &mut b);
        // Leave nothing lit, and the state as it was.
        b.push(format!("function {ns}:hl_off_{s}"));
        if let Some(l) = &latch {
            b.push(format!(
                "scoreboard players operation {l} dw.sys = #sv_{s}_latch {GUIDE}"
            ));
        }
        b.extend(d.close());
        if stand_in {
            b.push("kill @e[tag=dw_pt_hlw]".to_string());
        }
        out.insert(
            format!("packtest-datapack/data/{ns}/test/guide_highlight_{s}.mcfunction"),
            lines(&b).into_bytes(),
        );
    }
}

/// For a block highlight whose cells hold a block the objective's activation
/// places, the lines that place it as the activation does: the state the
/// display draws. Nothing for a cell the world carries.
fn stage_cells(body: &HlBody, b: &mut Vec<String>) {
    if let HlBody::Blocks {
        cells,
        staged: true,
        ..
    } = body
    {
        for ([x, y, z], state) in cells {
            b.push(format!(
                "execute unless block {x} {y} {z} {state} run setblock {x} {y} {z} {state}"
            ));
        }
    }
}

/// For a block highlight, the assertion that no display of it exists at all
/// (`k` names the holder).
fn assert_no_display(body: &HlBody, s: &str, k: &str, b: &mut Vec<String>) {
    if matches!(body, HlBody::Blocks { .. }) {
        b.push(format!(
            "execute store result score #{s}_x{k} {GUIDE} if entity @e[tag=dw_hl_{s}]"
        ));
        b.push(format!("assert score #{s}_x{k} {GUIDE} matches 0"));
    }
}

/// For a block highlight, the assertion that each display stands where its
/// kind draws the lit cell whole — a block display at the cell's corner, an
/// item display at its centre, written here from the cell, never read from
/// the emitter, and found within a thousandth of a block of it — and that the
/// displays number the cells.
fn assert_displays_stand(body: &HlBody, s: &str, b: &mut Vec<String>) {
    let HlBody::Blocks { cells, .. } = body else {
        return;
    };
    for (i, (cell, state)) in cells.iter().enumerate() {
        let kind = display_entity(state);
        let half = if kind == "minecraft:item_display" {
            0.5
        } else {
            0.0
        };
        let [x, y, z] = cell.map(|v| format!("{:.1}", f64::from(v) + half));
        b.push(format!(
            "execute store result score #{s}_p{i} {GUIDE} if entity @e[type={kind},tag=dw_hl_{s},x={x},y={y},z={z},distance=..0.001]"
        ));
        b.push(format!("assert score #{s}_p{i} {GUIDE} matches 1"));
    }
    b.push(format!(
        "execute store result score #{s}_pn {GUIDE} if entity @e[tag=dw_hl_{s}]"
    ));
    b.push(format!(
        "assert score #{s}_pn {GUIDE} matches {}",
        cells.len()
    ));
}

/// The state a hand's press leaves a pressed block in: a lever's or a
/// button's `powered` turned over. `None` for a state with no `powered`.
fn pressed_state(state: &str) -> Option<String> {
    let (name, props) = state.split_once('[')?;
    let props = props.trim_end_matches(']');
    let mut hit = false;
    let kv: Vec<String> = props
        .split(',')
        .map(|p| match p.split_once('=') {
            Some(("powered", v)) => {
                hit = true;
                format!("powered={}", if v == "true" { "false" } else { "true" })
            }
            _ => p.to_string(),
        })
        .collect();
    hit.then(|| format!("{name}[{}]", kv.join(",")))
}

/// **Where a template's dummy stands to be near a highlight** — the one
/// answer both highlight templates read. spec-0117 §3b: a pressable's
/// highlight is lit from its lit set, so the dummy stands in its first cell;
/// an empty set lights from nowhere, and there is nothing to stand in
/// (`None`). Any other body is lit within range, so the dummy stands one cell
/// inside it, along x from `at`.
fn near_stand(h: &HlAct, at: [i32; 3]) -> Option<String> {
    match &h.body {
        HlBody::Blocks {
            lit: Some(boxes), ..
        } => boxes
            .first()
            .map(|(lo, _)| format!("{} {} {}", center(lo[0]), corner(lo[1]), center(lo[2]))),
        _ => Some(format!(
            "{} {} {}",
            center(at[0] + i32::from(h.within) - 1),
            corner(at[1]),
            center(at[2])
        )),
    }
}

/// A block highlight leaves the world once its act is spent (spec-0104 §7.3):
/// no display of it exists after a trigger fires — its `once` latch written
/// and its pressed block pressed, the two writes a press makes before
/// anything else — nor after an objective completes. One template per block
/// highlight whose act a firing or a completion ends: an objective's, and a
/// trigger's that is `once` or whose block a press changes.
pub(super) fn emit_highlight_spent_packtests(plan: &Plan, out: &mut BuildOutput, g: &Guide) {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let title = artifact_title(c);
    for h in &g.highlights {
        let HlBody::Blocks { cells, at, .. } = &h.body else {
            continue;
        };
        let s = &h.safe;
        let n = cells.len();
        let mut d = Drive::new(&format!("hs{s}"));
        let mut b: Vec<String>;
        let (pin, me) = pin_dummy(&format!("dw_pt_hs_{s}"));
        let lit_check = |b: &mut Vec<String>, k: &str, want: String| {
            b.push(format!(
                "execute store result score #{s}_s{k} {GUIDE} if entity @e[tag=dw_hl_{s}]"
            ));
            b.push(format!("assert score #{s}_s{k} {GUIDE} matches {want}"));
        };
        let Some(near) = near_stand(h, *at) else {
            continue;
        };
        let near = format!("tp {me} {near}");
        match (&h.objective, &h.trigger) {
            (Some(oid), _) => {
                let st = Guide::state_holder(&plan::safe_local(oid));
                d.put(&obj_score(oid), 0);
                b = packtest_header(&format!(
                    "{title}: the highlight on `{oid}` leaves the world when it completes (spec-0104 §7.3)"
                ));
                b.push(format!("function {ns}:setup"));
                b.push(pin);
                b.extend(d.open());
                stage_cells(&h.body, &mut b);
                b.push(format!("scoreboard players set {st} {GUIDE} 1"));
                b.push(format!("scoreboard players set #hl_{s} {GUIDE} 1"));
                b.push(format!("function {ns}:hl_off_{s}"));
                b.push(near);
                b.push(format!("function {ns}:hl_poll_{s}"));
                lit_check(&mut b, "l", n.to_string());
                b.push(format!(
                    "execute as {me} at @s run function {ns}:complete_{}",
                    safe_obj_fn(oid)
                ));
                lit_check(&mut b, "c", "0".to_string());
                // And on the next tick, read from the state the completion wrote.
                b.push(format!("function {ns}:guide_scan"));
                b.push(format!("function {ns}:hl_poll_{s}"));
                lit_check(&mut b, "t", "0".to_string());
                b.push(format!("function {ns}:hl_off_{s}"));
                b.extend(d.close());
                b.push(format!("function {ns}:guide_scan"));
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
                let pressed: Vec<([i32; 3], String, String)> = cells
                    .iter()
                    .filter_map(|(c, st)| pressed_state(st).map(|p| (*c, st.clone(), p)))
                    .collect();
                if !t.once && pressed.is_empty() {
                    continue;
                }
                for f in &t.requires_flags {
                    d.put(&plan::flag_score(f.as_str()), 1);
                }
                for f in &t.forbids_flags {
                    d.put(&plan::flag_score(f.as_str()), 0);
                }
                let l = format!("#trig_{}", plan::safe_local(tid));
                b = packtest_header(&format!(
                    "{title}: the highlight on `{tid}` leaves the world when the trigger fires (spec-0104 §7.3)"
                ));
                b.push(format!("function {ns}:setup"));
                b.push(pin);
                b.push(format!(
                    "execute store result score #sv_{s}_hsl {GUIDE} run scoreboard players get {l} dw.sys"
                ));
                b.extend(d.open());
                b.push(format!("scoreboard players set {l} dw.sys 0"));
                b.push(format!("scoreboard players set #hl_{s} {GUIDE} 1"));
                b.push(format!("function {ns}:hl_off_{s}"));
                b.push(near);
                b.push(format!("function {ns}:hl_poll_{s}"));
                lit_check(&mut b, "l", n.to_string());
                // The press: the latch the trigger's function writes first,
                // and the block a hand turns over.
                b.push(format!("scoreboard players set {l} dw.sys 1"));
                for ([x, y, z], _, p) in &pressed {
                    b.push(format!("setblock {x} {y} {z} {p}"));
                }
                b.push(format!("function {ns}:hl_poll_{s}"));
                lit_check(&mut b, "f", "0".to_string());
                // The world and the latch as they were.
                for ([x, y, z], st, _) in &pressed {
                    b.push(format!("setblock {x} {y} {z} {st}"));
                }
                b.push(format!("function {ns}:hl_off_{s}"));
                b.push(format!(
                    "scoreboard players operation {l} dw.sys = #sv_{s}_hsl {GUIDE}"
                ));
                b.extend(d.close());
            }
            _ => continue,
        }
        out.insert(
            format!("packtest-datapack/data/{ns}/test/guide_highlight_spent_{s}.mcfunction"),
            lines(&b).into_bytes(),
        );
    }
}

/// A compass following a `kill` points at the nearest living body of its
/// wave, and moves to the next when that one dies (spec-0104 §6.3).
pub(super) fn emit_compass_wave_packtest(plan: &Plan, out: &mut BuildOutput, g: &Guide) {
    if !g.compass {
        return;
    }
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let Some(w) = g
        .tracked()
        .filter(|w| drivable(plan, w))
        .find(|w| matches!(w.target, Some(Target::Wave { .. })))
    else {
        return;
    };
    let Some(Target::Wave { tag, .. }) = &w.target else {
        return;
    };
    let k = w.ordinal.expect("tracked");
    let mut d = Drive::new("cmpw");
    isolate(plan, &[w], &mut d);
    let mut b = packtest_header(&format!(
        "{title}: a compass following `{}` points at the nearest living body of its wave, and at the next once that one dies (spec-0104 §6.3)",
        w.id
    ));
    b.push(format!("function {ns}:setup"));
    b.push("clear @s".to_string());
    b.push(format!("function {ns}:guide_seed"));
    b.extend(d.open());
    b.push(format!("scoreboard players set @s {TRACK} 0"));
    b.push(format!("function {ns}:compass_give"));
    // Every other body of the wave in the world — a sibling's spawn of the
    // real wave, anywhere the batch put it — is held out of the wave for this
    // template and handed back at its end: the nearest body is otherwise
    // whichever a sibling left nearer.
    b.push(format!(
        "tag @e[tag={tag},tag=!dw_pt_cmpw] add dw_pt_cmpw_held"
    ));
    b.push(format!("tag @e[tag=dw_pt_cmpw_held] remove {tag}"));
    // Two bodies of the wave's tag, two and four blocks from the player: mobs
    // that die as a wave's do, so the one killed lies in its death animation
    // while the compass is re-aimed.
    for (name, dx) in [("near", 2), ("far", 4)] {
        b.push(format!(
            "execute at @s run summon minecraft:zombie ~{dx} ~ ~ {{NoAI:1b,Silent:1b,PersistenceRequired:1b,DeathLootTable:\"minecraft:empty\",Tags:[\"{tag}\",\"dw_pt_cmpw\",\"dw_pt_cmpw_{name}\"]}}"
        ));
    }
    b.push(format!("function {ns}:compass_refresh"));
    b.push(format!("assert score @s {TRACK} matches {k}"));
    let target = format!(
        "entity @s Inventory[{{components:{{\"minecraft:custom_data\":{COMPASS_DATA}}}}}].components.\"minecraft:lodestone_tracker\".target.pos"
    );
    let aimed_at = |b: &mut Vec<String>, name: &str| {
        for (i, axis) in ["x", "y", "z"].iter().enumerate() {
            b.push(format!(
                "execute store result score #cmpw_b{axis} {GUIDE} run data get entity @e[tag=dw_pt_cmpw_{name},limit=1] Pos[{i}]"
            ));
            b.push(format!(
                "execute store result score #cmpw_c{axis} {GUIDE} run data get {target}[{i}]"
            ));
            b.push(format!(
                "execute store success score #cmpw_e{axis} {GUIDE} if score #cmpw_b{axis} {GUIDE} = #cmpw_c{axis} {GUIDE}"
            ));
            b.push(format!("assert score #cmpw_e{axis} {GUIDE} matches 1"));
        }
    };
    aimed_at(&mut b, "near");
    // The nearer dies; the next aim is the other.
    b.push("kill @e[tag=dw_pt_cmpw_near]".to_string());
    b.push(format!("function {ns}:compass_aim_followed"));
    aimed_at(&mut b, "far");
    // The bodies leave the wave before they go, so no sibling counts them.
    b.push(format!("tag @e[tag=dw_pt_cmpw] remove {tag}"));
    b.push("kill @e[tag=dw_pt_cmpw]".to_string());
    b.push(format!("tag @e[tag=dw_pt_cmpw_held] add {tag}"));
    b.push("tag @e[tag=dw_pt_cmpw_held] remove dw_pt_cmpw_held".to_string());
    b.extend(d.close());
    b.push("clear @s".to_string());
    out.insert(
        format!("packtest-datapack/data/{ns}/test/guide_compass_wave.mcfunction"),
        lines(&b).into_bytes(),
    );
}
