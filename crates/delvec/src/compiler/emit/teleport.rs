//! Teleports.

use super::*;

/// The generated function name for a `teleport` effect (DSL v0.10, spec-0031).
///
/// A named function rather than an inline line, for the same reason `volley` and
/// `collapse` have one: **the body is compiler-PROVEN geometry** — a box resolved
/// through `Plan::zone_box` and a destination resolved to a literal cell — and a
/// body that only ever exists spliced into a `seq_<root>_<n>` beside four other
/// effects is a body no runtime test can call. The generated PackTest calls
/// exactly this function, so the runtime proof of totality binds to the emission
/// rather than to a command the test re-typed for itself.
pub(super) fn teleport_fn(eff: &QuestEffect) -> String {
    format!("teleport_{}", payload_verb_key(eff))
}

/// The one emitted line of a box `teleport`: move everything in the volume.
///
/// `None` when either anchor is unresolved — `check_effect_anchors` (`DW0360`)
/// owns that failure, and an invalid selector emitted here would report it as
/// something else — and for a party teleport, whose body is
/// [`party_teleport_lines`].
pub(super) fn teleport_command(plan: &Plan, eff: &QuestEffect) -> Option<String> {
    let (from, _) = eff.teleport()?;
    let (lo, hi) = plan.zone_box(from)?;
    let d = teleport_destination(plan, eff)?;
    Some(format!(
        "tp @e[{}] {} {} {}",
        entity_box_selector(lo, hi),
        d[0],
        d[1],
        d[2]
    ))
}

/// Where a `teleport` puts what it carries: its `to` mark's cell centre, as the
/// `tp` writes it. `None` when the mark's anchor is unresolved (`DW0360`).
pub(super) fn teleport_destination(plan: &Plan, eff: &QuestEffect) -> Option<[String; 3]> {
    Some(ent_xyz(teleport_cell(plan, eff)?))
}

/// The cell a `teleport`'s `to` mark resolves to — of a box or of the party.
/// `None` when the mark's anchor is unresolved (`DW0360`).
pub(super) fn teleport_cell(plan: &Plan, eff: &QuestEffect) -> Option<[i32; 3]> {
    let to = eff.teleport_to()?;
    Some(to.cell(anchor_point_any(plan, to.anchor.as_str())?))
}

/// The `dw.sys` scratch that numbers the members a party teleport seats.
pub(super) const TPP_COUNT: &str = "#tpp";

/// Each member's seat number while a party teleport runs: given and taken
/// inside the teleport's own function.
pub(super) const TPP_SEAT: &str = "dw.tpp";

/// The shared macro that writes a cutscene viewer's seat into the place the
/// cutscene returns them to.
pub(super) const TPP_KEEP_FN: &str = "tpp_keep";

/// **A party carry's seats**, as the build fixed them
/// ([`crate::compiler::nav::fix_party_seats`]).
///
/// # Panics
///
/// When read before the seats were fixed, or for a mark with none: the build
/// refuses a party carry without a seat for every member (`DW1141`) before
/// anything is emitted, so either is a compiler defect.
pub(super) fn party_seats<'p>(plan: &'p Plan<'_>, to: [i32; 3]) -> &'p [[i32; 3]] {
    plan.party_seats
        .get()
        .and_then(|m| m.get(&to))
        .map(Vec::as_slice)
        .unwrap_or_else(|| {
            panic!(
                "a party teleport onto {to:?} was emitted with no seats fixed — `DW1141` \
                 refuses one before emission, so this is a compiler defect"
            )
        })
}

/// The selector condition, per seat number `k` of `n`, that names the
/// members given seat `k`: the last seat takes any number past it, so a
/// member beyond the party's four is put beside the fourth rather than left.
fn seat_term(k: usize, n: usize) -> String {
    if k + 1 == n {
        format!("{}..", k + 1)
    } else {
        format!("{}", k + 1)
    }
}

/// **The body of a party teleport** (`from: "party"`, DSL v0.88).
///
/// Every member present is numbered, in `@a`'s order, and member `k` is put
/// on seat `k` — the mark's cell, then its nearest neighbours a body walks to
/// ([`crate::compiler::nav::World::party_seats`]) — so no two members land on
/// one cell. A member is a player who has taken a class (every player, in a
/// campaign with no classes: [`super::grant::owed_players`]), alive, and not
/// waiting out a respawn: a body on its death screen and a waiting member are
/// seated by the respawn, one still choosing by the class. A member watching a
/// cutscene is not moved under the camera: the place the cutscene puts them
/// back on is rewritten to their seat (`<fn>_keep`), so the camera's return
/// lands them there.
pub(super) fn party_teleport_lines(
    plan: &Plan,
    eff: &QuestEffect,
    name: &str,
) -> Option<Vec<String>> {
    eff.party_teleport()?;
    let to = teleport_cell(plan, eff)?;
    Some(party_carry_lines(plan, to, name, &[]))
}

/// The `execute` prefix that runs as every **member present**: a player who
/// has taken a class (every player, in a campaign with no classes:
/// [`super::grant::owed_players`]), alive, and not waiting out a respawn. The
/// one statement of who a party carry moves, read by every carry and by the
/// crossing's owed line.
fn as_members_present(plan: &Plan) -> String {
    let members = super::grant::owed_players(plan);
    let waiting = if respawn_wait(plan).is_some() {
        format!(" unless score @s {RW_CLOCK} matches 1..")
    } else {
        String::new()
    };
    format!("execute as {members} unless data entity @s {{Health:0.0f}}{waiting}")
}

/// **The one body of a party carry onto `to`**, emitted as function `name`:
/// a party teleport's ([`party_teleport_lines`]) and a crossing's
/// ([`crossing_fns`]).
///
/// Every member present ([`as_members_present`]) is numbered, in `@a`'s order,
/// and member `k` is put on seat `k` ([`party_seats`]). A member watching a
/// cutscene is not moved under the camera: the place the cutscene puts them
/// back on is rewritten to their seat (`<name>_keep`). `carried` runs while
/// the numbers still stand, so it can address every member carried
/// (`@a[scores={dw.tpp=1..}]`); the numbers are taken off last.
fn party_carry_lines(plan: &Plan, to: [i32; 3], name: &str, carried: &[String]) -> Vec<String> {
    let ns = &plan.namespace;
    let seats = party_seats(plan, to);
    let n = seats.len();
    let mut out = vec![
        format!("scoreboard players reset @a {TPP_SEAT}"),
        format!("scoreboard players set {TPP_COUNT} dw.sys 0"),
        format!(
            "{} store result score @s {TPP_SEAT} run scoreboard players add {TPP_COUNT} dw.sys 1",
            as_members_present(plan)
        ),
    ];
    for (k, seat) in seats.iter().enumerate() {
        let [x, y, z] = ent_xyz(*seat);
        out.push(format!(
            "tp @a[tag=!{CUTSCENE_TAG},scores={{{TPP_SEAT}={}}}] {x} {y} {z}",
            seat_term(k, n)
        ));
    }
    if campaign_has_cutscene(plan.campaign) {
        out.push(format!(
            "execute as @a[tag={CUTSCENE_TAG},scores={{{TPP_SEAT}=1..}}] if {} run function \
             {ns}:{name}_keep",
            cs_place_saved()
        ));
    }
    out.extend(carried.iter().cloned());
    out.push(format!("scoreboard players reset @a {TPP_SEAT}"));
    out
}

/// A party carry's `<name>_keep` (as a member watching a cutscene): their
/// seat, written into the place the cutscene returns them to.
fn party_keep_lines(plan: &Plan, to: [i32; 3]) -> Vec<String> {
    let ns = &plan.namespace;
    let seats = party_seats(plan, to);
    let n = seats.len();
    let mut out = Vec::new();
    for (k, seat) in seats.iter().enumerate() {
        let p = crate::compiler::nav::cell_center(*seat);
        out.push(format!(
            "execute if score @s {TPP_SEAT} matches {} run data modify storage dw:cs tpp.pos set \
             value [{}d, {}d, {}d]",
            seat_term(k, n),
            p[0],
            p[1],
            p[2]
        ));
    }
    out.push(format!(
        "execute store result storage dw:cs tpp.id int 1 run scoreboard players get @s {CS_ID}"
    ));
    out.push(format!("function {ns}:{TPP_KEEP_FN} with storage dw:cs tpp"));
    out
}

/// One function per distinct `teleport` (deduped by content key). Empty for a
/// campaign that declares none, so pre-0.10 output is byte-identical.
pub(super) fn teleport_fns(plan: &Plan) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for eff in all_campaign_effects(plan.campaign) {
        if eff.teleport_to().is_none() {
            continue;
        }
        let name = teleport_fn(eff);
        if !seen.insert(name.clone()) {
            continue;
        }
        if let Some(body) = party_teleport_lines(plan, eff, &name) {
            out.push((name, lines(&body)));
            continue;
        }
        let Some(cmd) = teleport_command(plan, eff) else {
            continue;
        };
        out.push((name, lines(&[cmd])));
    }
    out
}

/// The helpers a party carry calls: each party teleport's and each crossing's
/// `<fn>_keep` and the shared [`TPP_KEEP_FN`], in a campaign with a cutscene.
/// Empty otherwise.
pub(super) fn party_teleport_helper_fns(
    plan: &Plan,
    crossings: &[Crossing],
) -> Vec<(String, String)> {
    if !campaign_has_cutscene(plan.campaign) {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for eff in all_campaign_effects(plan.campaign) {
        if eff.party_teleport().is_none() {
            continue;
        }
        let name = teleport_fn(eff);
        if !seen.insert(name.clone()) {
            continue;
        }
        if let Some(to) = teleport_cell(plan, eff) {
            out.push((format!("{name}_keep"), lines(&party_keep_lines(plan, to))));
        }
    }
    for x in crossings {
        out.push((
            format!("{}_keep", x.fn_name()),
            lines(&party_keep_lines(plan, x.to)),
        ));
    }
    if !out.is_empty() {
        out.push((
            TPP_KEEP_FN.to_string(),
            lines(&[
                "$data modify storage dw:cs places.p$(id).pos set from storage dw:cs tpp.pos"
                    .to_string(),
            ]),
        ));
    }
    out
}

/// The `setup` lines declaring [`TPP_SEAT`], for a campaign with a party
/// carry, and [`XC`], for one that crosses. Empty otherwise.
pub(super) fn party_teleport_objectives(plan: &Plan, crossings: &[Crossing]) -> Vec<String> {
    let mut out = Vec::new();
    if !plan.party_carries.is_empty() || !crossings.is_empty() {
        out.push(format!("scoreboard objectives add {TPP_SEAT} dummy"));
    }
    if !crossings.is_empty() {
        out.push(format!("scoreboard objectives add {XC} dummy"));
    }
    out
}

/// **A crossing**: completing an objective carries the party across the void
/// into another area's entry point (`Plan::transport`, and a branch's own
/// crossings, [`BranchTransportOverlay`]). It is a party carry onto that
/// entry point ([`party_carry_lines`]), so it moves every member present, each
/// onto a seat of their own, whoever completed the objective.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Crossing {
    /// The area entered.
    pub area: String,
    /// Its entry point: the first seat.
    pub to: [i32; 3],
    /// The crossing's number (1-based, in area order): what `#xto` holds
    /// after it, so the owed line knows where the party went last.
    pub id: usize,
}

impl Crossing {
    /// The function that carries the party across (`cross_<area>`).
    pub(crate) fn fn_name(&self) -> String {
        format!("cross_{}", plan::safe_local(&self.area))
    }
}

/// Each member's count of the crossings they were carried across; on
/// [`plan::PARTY`], the party's.
pub(super) const XC: &str = "dw.xc";
/// The `dw.sys` holder naming the party's last crossing ([`Crossing::id`]).
const XTO: &str = "#xto";
/// The `dw.sys` holder keeping the checkpoint armed when the party last
/// crossed (`-1`: none).
const XCP: &str = "#xcp";
/// The `dw.sys` scratch the owed line sets when a checkpoint was armed since.
const XNEW: &str = "#xnew";
/// The function a member behind the party's crossings runs (as them).
pub(super) const CROSS_OWED_FN: &str = "cross_owed";

/// **Every crossing the datapack performs**, one per area entered, in area
/// order (ADR-0006): the exported path's and every branch's. An entry point
/// that resolves to no area's entry anchor is left out (`DW0872` owns it).
pub(crate) fn crossings(plan: &Plan, overlay: &BranchTransportOverlay) -> Vec<Crossing> {
    let cells: std::collections::BTreeSet<[i32; 3]> = plan
        .transport
        .values()
        .copied()
        .chain(overlay.values().flatten().map(|r| r.pos))
        .collect();
    let mut by_area: BTreeMap<String, [i32; 3]> = BTreeMap::new();
    for a in &plan.areas {
        if let Some(ResolvedAnchor::Point { pos, .. }) = plan.anchors.entry_anchor(&a.area_id)
            && cells.contains(pos)
        {
            by_area.insert(a.area_id.clone(), *pos);
        }
    }
    by_area
        .into_iter()
        .enumerate()
        .map(|(i, (area, to))| Crossing { area, to, id: i + 1 })
        .collect()
}

/// The function that performs the crossing onto `pos`, as `complete_<obj>`
/// calls it. `None` only for a destination [`crossings`] left out.
pub(super) fn crossing_call(plan: &Plan, crossings: &[Crossing], pos: [i32; 3]) -> Option<String> {
    crossings
        .iter()
        .find(|x| x.to == pos)
        .map(|x| format!("function {}:{}", plan.namespace, x.fn_name()))
}

/// The crossing functions: each `cross_<area>` and the shared
/// [`CROSS_OWED_FN`]. Empty for a campaign that never crosses.
///
/// `cross_<area>` counts the crossing on the party ([`XC`]), names it
/// ([`XTO`]), keeps the checkpoint armed at that moment ([`XCP`]), and carries
/// every member present, writing the party's count onto each member carried.
///
/// A member who was not carried — away, on their death screen, waiting out a
/// respawn, or still choosing a class — is behind the party's count, and the
/// tick's owed line ([`crossing_tick`]) runs [`CROSS_OWED_FN`] as them once
/// they are a member present again: they are put where the party respawns
/// when a checkpoint was armed since the crossing (`cp_seat_<i>`), else on
/// the last crossing's entry point. The void between areas has no walk back,
/// so a member left in the old area could never follow.
pub(super) fn crossing_fns(plan: &Plan, crossings: &[Crossing]) -> Vec<(String, String)> {
    if crossings.is_empty() {
        return Vec::new();
    }
    let cp = plan.any_checkpoint();
    let mut out = Vec::new();
    for x in crossings {
        let name = x.fn_name();
        let mut body = vec![
            format!("scoreboard players add {} {XC} 1", plan::PARTY),
            format!("scoreboard players set {XTO} dw.sys {}", x.id),
        ];
        if cp {
            body.push(format!("scoreboard players set {XCP} dw.sys -1"));
            body.push(format!(
                "execute if score #cp dw.sys matches 0.. run scoreboard players operation {XCP} \
                 dw.sys = #cp dw.sys"
            ));
        }
        body.extend(party_carry_lines(
            plan,
            x.to,
            &name,
            &[format!(
                "scoreboard players operation @a[scores={{{TPP_SEAT}=1..}}] {XC} = {} {XC}",
                plan::PARTY
            )],
        ));
        out.push((name, lines(&body)));
    }
    let mut owed = Vec::new();
    let landing = if cp {
        owed.push(format!("scoreboard players set {XNEW} dw.sys 0"));
        owed.push(format!(
            "execute if score #cp dw.sys matches 0.. unless score #cp dw.sys = {XCP} dw.sys run \
             scoreboard players set {XNEW} dw.sys 1"
        ));
        for line in cp_seat_dispatch(plan) {
            let rest = line.strip_prefix("execute ").expect("a dispatch line is an execute");
            owed.push(format!("execute if score {XNEW} dw.sys matches 1 {rest}"));
        }
        format!("execute if score {XNEW} dw.sys matches 0 ")
    } else {
        "execute ".to_string()
    };
    for x in crossings {
        let [px, py, pz] = ent_xyz(x.to);
        owed.push(format!(
            "{landing}if score {XTO} dw.sys matches {} run tp @s {px} {py} {pz}",
            x.id
        ));
    }
    owed.push(format!(
        "scoreboard players operation @s {XC} = {} {XC}",
        plan::PARTY
    ));
    out.push((CROSS_OWED_FN.to_string(), lines(&owed)));
    out
}

/// The tick's owed line: a member present, not watching a cutscene, whose
/// count is behind the party's crossings runs [`CROSS_OWED_FN`]. Empty for a
/// campaign that never crosses.
pub(super) fn crossing_tick(plan: &Plan, crossings: &[Crossing]) -> Vec<String> {
    if crossings.is_empty() {
        return Vec::new();
    }
    vec![format!(
        "{} if entity @s[tag=!{CUTSCENE_TAG}] if score {p} {XC} matches 1.. unless score @s {XC} \
         = {p} {XC} run function {}:{CROSS_OWED_FN}",
        as_members_present(plan),
        plan.namespace,
        p = plan::PARTY,
    )]
}
