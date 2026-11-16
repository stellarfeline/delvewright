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
pub(super) fn party_teleport_lines(plan: &Plan, eff: &QuestEffect, name: &str) -> Option<Vec<String>> {
    eff.party_teleport()?;
    let ns = &plan.namespace;
    let to = teleport_cell(plan, eff)?;
    let seats = party_seats(plan, to);
    let n = seats.len();
    let members = super::grant::owed_players(plan);
    let waiting = if respawn_wait(plan).is_some() {
        format!(" unless score @s {RW_CLOCK} matches 1..")
    } else {
        String::new()
    };
    let mut out = vec![
        format!("scoreboard players reset @a {TPP_SEAT}"),
        format!("scoreboard players set {TPP_COUNT} dw.sys 0"),
        format!(
            "execute as {members} unless data entity @s {{Health:0.0f}}{waiting} store result \
             score @s {TPP_SEAT} run scoreboard players add {TPP_COUNT} dw.sys 1"
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
    out.push(format!("scoreboard players reset @a {TPP_SEAT}"));
    Some(out)
}

/// A party teleport's `<fn>_keep` (as a member watching a cutscene): their
/// seat, written into the place the cutscene returns them to.
fn party_keep_lines(plan: &Plan, eff: &QuestEffect) -> Option<Vec<String>> {
    let ns = &plan.namespace;
    let to = teleport_cell(plan, eff)?;
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
    Some(out)
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

/// The helpers a party teleport calls: each one's `<fn>_keep` and the shared
/// [`TPP_KEEP_FN`], in a campaign with a cutscene. Empty otherwise.
pub(super) fn party_teleport_helper_fns(plan: &Plan) -> Vec<(String, String)> {
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
        if let Some(body) = party_keep_lines(plan, eff) {
            out.push((format!("{name}_keep"), lines(&body)));
        }
    }
    if !out.is_empty() {
        out.push((
            TPP_KEEP_FN.to_string(),
            lines(&["$data modify storage dw:cs places.p$(id).pos set from storage dw:cs tpp.pos"
                .to_string()]),
        ));
    }
    out
}

/// The `setup` line declaring [`TPP_SEAT`], for a campaign with a party
/// teleport. Empty otherwise.
pub(super) fn party_teleport_objectives(plan: &Plan) -> Vec<String> {
    if plan.party_carries.is_empty() {
        return Vec::new();
    }
    vec![format!("scoreboard objectives add {TPP_SEAT} dummy")]
}
