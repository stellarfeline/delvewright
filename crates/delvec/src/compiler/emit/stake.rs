//! The recovery stake (spec-0032).

use super::*;

/// The `dw.sys` fake player holding the amount a death is forfeiting, while it is
/// being computed. Scratch, never read outside one function chain.
pub(super) const STK_AMT: &str = "#stk_amt";

/// `dw.sys` scratch holding the block coordinates of the marker under discussion.
pub(super) const STK_X: &str = "#stk_x";

pub(super) const STK_Y: &str = "#stk_y";

pub(super) const STK_Z: &str = "#stk_z";

/// `dw.sys` scratch counting how many players still have a live stake at
/// `#stk_x/y/z` — the reference count that decides whether a marker may be retired.
pub(super) const STK_REF: &str = "#stk_ref";

/// `dw.sys` scratch: whether a `collect_by: anyone` sweep actually took anything.
pub(super) const STK_GOT: &str = "#stk_got";

/// The `dw.sys` fake player holding the constant `100`, for a proportional forfeit.
pub(super) const STK_HUNDRED: &str = "#stk_100";

/// The tag `stk_collect` puts on the player who clicked, for `stk_pick` to find.
pub(super) const STK_CLICKER: &str = "dw_stk_clicker";

/// The tag `stk_pick` leaves on the box that player clicked.
pub(super) const STK_HIT: &str = "dw_stk_hit";

/// `dw.sys` scratch: the latest `interaction.timestamp` of a box the clicker used.
pub(super) const STK_BEST: &str = "#stk_best";

/// `dw.sys` scratch: one box's `interaction.timestamp`.
pub(super) const STK_T: &str = "#stk_t";

/// `dw.sys` scratch: whether the clicker is the box's last user (`on target`).
pub(super) const STK_MINE: &str = "#stk_mine";

/// Run as each stake box: which one the player who fired `stk_collect` clicked.
pub(super) const STK_PICK_FN: &str = "stk_pick";

/// The one function that summons a marker: **the place**, made once however many
/// wagers a death leaves there.
pub(super) const STK_PLACE_FN: &str = "stk_place";

/// The one right-click handler, behind the one advancement on [`stk_tag`].
pub(super) const STK_COLLECT_FN: &str = "stk_collect";

/// The one live-wager count at `#stk_x/y/z`, over every declared stake.
pub(super) const STK_REF_FN: &str = "stk_ref";

/// The one function permitted to retire a marker (`DW0421`).
pub(super) const STK_GC_FN: &str = "stk_gc";

/// The per-player objective holding slot `k`'s **amount** for stake `s`.
pub(super) fn stk_amount_obj(s: &str, k: u32) -> String {
    format!("dw.kv{k}_{s}")
}

/// The per-player objective holding whether slot `k` of stake `s` is **live**.
pub(super) fn stk_live_obj(s: &str, k: u32) -> String {
    format!("dw.kl{k}_{s}")
}

/// The per-player objective holding one axis of slot `k`'s marker position.
pub(super) fn stk_pos_obj(s: &str, k: u32, axis: usize) -> String {
    format!("dw.k{}{k}_{s}", ["x", "y", "z"][axis])
}

/// The interaction hitbox tag for **every** stake marker in the campaign.
///
/// **One tag for every marker, and one marker for every place.** A marker is a
/// *place* — the spot a death left its wagers — and which players have a wager
/// there, in which datum, is the per-player ledger's business rather than the
/// entity's. That sentence was already written here when the tag was
/// `dw_stk_<s>`, and the binding contradicted it: keyed to the STAKE, a death
/// that forfeits four datums summons four `minecraft:interaction` boxes,
/// `1.0 × 2.0`, at one position. Coincident boxes are an exact ray-pick tie the
/// client resolves by entity iteration order — the defect `DW0878` refuses for
/// authored affordances, produced by the compiler for its own hardware.
///
/// **Placement cannot repair it, and that is why the tag is what changed.** The
/// compile-time table is keyed on (respawn seat, death region), never on the
/// stake, so every stake a death drops resolves to one anchor; and the rule's
/// common branch — a death on ordinary walkable ground leaves its stake where the
/// player fell — positions at a cell chosen at RUNTIME, which no compile-time
/// separation can reach at all. There is one place per death either way. So the
/// place holds one box, and the wagers left there are counted in the ledger.
pub(super) fn stk_tag() -> String {
    "dw_stk".to_string()
}

/// Every declared stake, paired with the `safe_local` segment naming its functions
/// and objectives. Empty for a campaign that declares none, which is what keeps
/// every existing campaign's emission byte-identical.
pub(super) fn stakes<'a>(plan: &'a Plan) -> Vec<(&'a delvewright_dsl::Stake, String)> {
    plan.campaign
        .quests
        .content
        .stakes
        .iter()
        .map(|s| (s, plan::safe_local(s.id.as_str())))
        .collect()
}

/// The commands that compute a death's forfeit into [`STK_AMT`], per the stake's
/// declared `forfeit` rule.
///
/// Integer arithmetic throughout (ADR-0006): vanilla's `*=` and `/=` are the only
/// operators involved, and `/=` floors — documented rather than papered over,
/// because a balance is non-negative by the clamp below and flooring and truncation
/// agree there.
pub(super) fn stake_forfeit_lines(plan: &Plan, st: &delvewright_dsl::Stake) -> Vec<String> {
    let obj = plan::state_score(st.state.as_str());
    let mut out = Vec::new();
    match st.forfeit() {
        delvewright_dsl::Forfeit::None => {
            out.push(format!("scoreboard players set {STK_AMT} dw.sys 0"));
        }
        delvewright_dsl::Forfeit::All => {
            out.push(format!(
                "scoreboard players operation {STK_AMT} dw.sys = @s {obj}"
            ));
        }
        delvewright_dsl::Forfeit::Proportion { percent } => {
            out.push(format!(
                "scoreboard players operation {STK_AMT} dw.sys = @s {obj}"
            ));
            out.push(format!(
                "scoreboard players operation {STK_AMT} dw.sys *= #stk_p{percent} dw.sys"
            ));
            out.push(format!(
                "scoreboard players operation {STK_AMT} dw.sys /= {STK_HUNDRED} dw.sys"
            ));
        }
        delvewright_dsl::Forfeit::Fixed { amount } => {
            out.push(format!(
                "scoreboard players operation {STK_AMT} dw.sys = @s {obj}"
            ));
            // min(balance, amount) — a fixed forfeit can never overdraw a purse.
            if amount < i32::MAX {
                out.push(format!(
                    "execute if score {STK_AMT} dw.sys matches {}.. run scoreboard players set {STK_AMT} dw.sys {amount}",
                    amount.saturating_add(1)
                ));
            }
        }
    }
    // A negative balance forfeits nothing: a death must never HAND the player
    // money, which is what an unclamped `= @s <balance>` would do.
    out.push(format!(
        "execute if score {STK_AMT} dw.sys matches ..-1 run scoreboard players set {STK_AMT} dw.sys 0"
    ));
    let _ = plan;
    out
}

/// Every emitted function the recovery stake needs (DSL v0.10, spec-0032).
///
/// # A marker is a PLACE, and a death leaves one place
///
/// The hardware is emitted once for the campaign, not once per stake, and that is
/// the whole shape of this function. A death that forfeits several datums fires
/// several `drop-stake` effects, and every one of them resolves to the SAME
/// position — the placement table is keyed on (respawn seat, death region) and
/// never on the stake, and the rule's common branch positions at the death point
/// itself. Summoning one `minecraft:interaction` per stake therefore put four
/// coincident `1.0 × 2.0` boxes at one cell: an exact ray-pick tie the client
/// resolves by entity iteration order, which is the defect `DW0878` refuses for
/// authored affordances and the compiler was producing for its own hardware.
///
/// **No placement rule can repair that**, because the common branch's position is
/// chosen at runtime. So the place holds one box and one glowing display, and what
/// was left there is counted in the per-player ledger — which is where a wager
/// always lived.
///
/// The chain. Shared functions first, then the per-stake ones:
///
/// | function | run as | what it does |
/// |---|---|---|
/// | `stk_place` | the corpse, positioned | summon the marker if this place has none |
/// | `stk_collect` | the collecting player | read the marker's position, then offer it to every stake |
/// | `stk_ref` | each player | count live wagers at `#stk_x/y/z`, over every stake |
/// | `stk_gc` | each marker | retire a marker nobody has a wager at — **the one legal killer of its hardware** (`DW0421`) |
/// | `stk_drop_<s>` | the corpse | apply the retention policy, compute and debit the forfeit, then route |
/// | `stk_route_<s>` | the corpse | **the compile-time table**, as one `execute if` chain — the death region test, the respawn-seat test, and the anchor each pair resolved to |
/// | `stk_put_<s>_<n>` | the corpse | position at table anchor `n` |
/// | `stk_here_<s>` | the corpse | position at the death point (the rule's degenerate branch) |
/// | `stk_fill_<s>` | the corpse, positioned | make the place, then take this stake's first free slot |
/// | `stk_slot_<s>_<k>` | the corpse, positioned | write the amount and the marker's position into slot `k` |
/// | `stk_evict_<s>` | the corpse | the `replace` policy: free slot 0 |
/// | `stk_collect_<s>` | the collecting player | identify which of this stake's slots the marker holds, and take it |
/// | `stk_take_<s>_<k>` | the collecting player | restore the amount, clear the slot, say so |
/// | `stk_pool_<s>` | each player | the `collect_by: anyone` sweep |
///
/// **Idempotency under a double right-click in one tick** (AC6) is structural, not
/// timed: `stk_take_<s>_<k>` sets the slot's live flag to 0 as part of taking it, so
/// a second pass in the same tick matches no slot and does nothing. There is no
/// second-click window to lose a race in.
pub(super) fn emit_stake_functions(
    plan: &Plan,
    table: Option<&crate::compiler::stake::StakeTable>,
) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut fns: Vec<(String, String)> = Vec::new();
    let tag = stk_tag();
    let hw = crate::compiler::affordance::hardware_tag(&tag);
    // Every stake that can actually leave a marker. `max_live: 0` is the
    // no-death-cost configuration and places nothing, so it owns no part of the
    // shared hardware either.
    let marking: Vec<(&delvewright_dsl::Stake, String)> = stakes(plan)
        .into_iter()
        .filter(|(st, _)| st.max_live() > 0)
        .collect();

    // --- stk_place: the place, made once --------------------------------------
    // The item every marker renders as is `DW0880`'s subject: a place wears one
    // face, so every stake that can leave a marker has been proved to declare the
    // same one and reading the first is reading all of them.
    if let Some((first, _)) = marking.first() {
        fns.push((
            STK_PLACE_FN.to_string(),
            lines(&[
                format!(
                    "execute unless entity @e[tag={tag},distance=..1] run summon minecraft:interaction ~ ~ ~ {{width:1.0f,height:2.0f,response:1b,Invulnerable:1b,Tags:[{FIXTURE_NBT}\"{tag}\"]}}"
                ),
                format!(
                    "execute unless entity @e[tag={hw},distance=..1] run summon minecraft:item_display ~ ~ ~ {{Glowing:1b,Tags:[{FIXTURE_NBT}\"dw_marker\",\"{hw}\"],billboard:\"center\",item:{{id:\"{}\",count:1}}}}",
                    first.marker_item()
                ),
            ]),
        ));
    }

    for (st, safe) in stakes(plan) {
        let max = st.max_live();
        let obj = plan::state_score(st.state.as_str());

        // --- stk_drop: policy, forfeit, route --------------------------------
        let mut drop: Vec<String> = Vec::new();
        if max > 0 {
            // "every slot is live" as one condition; with `max_live: 1` it is the
            // single `if score … matches 1` a souls loop wants.
            let full: String = (0..max)
                .map(|k| format!(" if score @s {} matches 1", stk_live_obj(&safe, k)))
                .collect();
            match st.on_full() {
                delvewright_dsl::OnFull::Keep => {
                    drop.push(format!("execute{full} run return fail"));
                }
                delvewright_dsl::OnFull::Replace => {
                    drop.push(format!("execute{full} run function {ns}:stk_evict_{safe}"));
                }
            }
            drop.extend(stake_forfeit_lines(plan, st));
            drop.push(format!(
                "scoreboard players operation @s {obj} -= {STK_AMT} dw.sys"
            ));
            drop.push(format!("function {ns}:stk_route_{safe}"));
        }
        fns.push((format!("stk_drop_{safe}"), lines(&drop)));

        if max > 0 {
            // --- stk_evict: the `replace` policy -----------------------------
            // The evicted marker is NOT killed here. Its liveness is decided by the
            // reference count in `stk_gc`, which is the one mechanism that
            // retires a marker — so an eviction whose marker sits in an unloaded
            // chunk is not a leak, it is a retirement deferred to the tick that
            // chunk next loads.
            fns.push((
                format!("stk_evict_{safe}"),
                lines(&[
                    format!("scoreboard players set @s {} 0", stk_live_obj(&safe, 0)),
                    format!("scoreboard players set @s {} 0", stk_amount_obj(&safe, 0)),
                ]),
            ));

            // --- stk_route: THE TABLE ----------------------------------------
            let mut route: Vec<String> = Vec::new();
            if let Some(t) = table {
                for row in &t.rows {
                    let seat = &t.seats[row.seat];
                    let region = &t.regions[row.region];
                    route.push(format!(
                        "execute if score #cp dw.sys matches {} if entity @s[{}] run return run function {ns}:stk_put_{safe}_{}",
                        seat.cp,
                        selector_box(region.region),
                        row.anchor
                    ));
                }
            }
            // The degenerate branch, and the common one: a death on ordinary
            // walkable ground leaves its stake where the player fell, because the
            // nearest point of the route back to a place you can walk to IS that
            // place. `DW0525` is what makes that safe to say unconditionally.
            route.push(format!("function {ns}:stk_here_{safe}"));
            fns.push((format!("stk_route_{safe}"), lines(&route)));

            if let Some(t) = table {
                for (n, a) in t.anchors.iter().enumerate() {
                    let v = ent_xyz(*a);
                    fns.push((
                        format!("stk_put_{safe}_{n}"),
                        lines(&[format!(
                            "execute positioned {} {} {} run function {ns}:stk_fill_{safe}",
                            v[0], v[1], v[2]
                        )]),
                    ));
                }
            }
            fns.push((
                format!("stk_here_{safe}"),
                lines(&[format!("execute at @s run function {ns}:stk_fill_{safe}")]),
            ));

            // --- stk_fill: the place, then the first free slot -----------------
            let mut fill: Vec<String> = vec![format!("function {ns}:{STK_PLACE_FN}")];
            for k in 0..max {
                fill.push(format!(
                    "execute unless score @s {} matches 1 run return run function {ns}:stk_slot_{safe}_{k}",
                    stk_live_obj(&safe, k)
                ));
            }
            fns.push((format!("stk_fill_{safe}"), lines(&fill)));

            for k in 0..max {
                let mut slot = vec![
                    format!(
                        "scoreboard players operation @s {} = {STK_AMT} dw.sys",
                        stk_amount_obj(&safe, k)
                    ),
                    format!("scoreboard players set @s {} 1", stk_live_obj(&safe, k)),
                ];
                // The position is read back off the marker rather than written from
                // the anchor the table chose, so the compile-time branch and the
                // runtime "here" branch record it the SAME way — and a collector
                // comparing the two is comparing two `data get`s of one double, not
                // a coordinate against a rounding of it.
                for axis in 0..3 {
                    slot.push(format!(
                        "execute store result score @s {} run data get entity @e[tag={tag},limit=1,sort=nearest] Pos[{axis}]",
                        stk_pos_obj(&safe, k, axis)
                    ));
                }
                fns.push((format!("stk_slot_{safe}_{k}"), lines(&slot)));
            }

            // --- stk_collect_<s>: what THIS stake holds at the marker ---------
            // The marker's position is already in `#stk_x/y/z` — `stk_collect`
            // read it once for every stake, because it is a property of the place
            // and not of any wager left there.
            let mut collect: Vec<String> = Vec::new();
            match st.collect_by() {
                delvewright_dsl::CollectBy::Owner => {
                    for k in 0..max {
                        collect.push(format!(
                            "execute{} run return run function {ns}:stk_take_{safe}_{k}",
                            slot_match(&safe, k)
                        ));
                    }
                }
                delvewright_dsl::CollectBy::Anyone => {
                    collect.push(format!("scoreboard players set {STK_AMT} dw.sys 0"));
                    collect.push(format!("scoreboard players set {STK_GOT} dw.sys 0"));
                    collect.push(format!("execute as @a run function {ns}:stk_pool_{safe}"));
                    collect.push(format!(
                        "execute if score {STK_GOT} dw.sys matches 1.. run scoreboard players operation @s {obj} += {STK_AMT} dw.sys"
                    ));
                    collect.push(format!(
                        "execute if score {STK_GOT} dw.sys matches 1.. run title @s actionbar {}",
                        tr(&st.collected_message)
                    ));
                }
            }
            fns.push((format!("stk_collect_{safe}"), lines(&collect)));

            if matches!(st.collect_by(), delvewright_dsl::CollectBy::Anyone) {
                let mut pool: Vec<String> = Vec::new();
                for k in 0..max {
                    pool.push(format!(
                        "execute{} run scoreboard players operation {STK_AMT} dw.sys += @s {}",
                        slot_match(&safe, k),
                        stk_amount_obj(&safe, k)
                    ));
                    pool.push(format!(
                        "execute{} run scoreboard players set {STK_GOT} dw.sys 1",
                        slot_match(&safe, k)
                    ));
                    pool.push(format!(
                        "execute{} run scoreboard players set @s {} 0",
                        slot_match(&safe, k),
                        stk_amount_obj(&safe, k)
                    ));
                    // Clearing the live flag LAST: the three lines above all test it.
                    pool.push(format!(
                        "execute{} run scoreboard players set @s {} 0",
                        slot_match(&safe, k),
                        stk_live_obj(&safe, k)
                    ));
                }
                fns.push((format!("stk_pool_{safe}"), lines(&pool)));
            } else {
                for k in 0..max {
                    let mut take = vec![
                        format!(
                            "scoreboard players operation @s {obj} += @s {}",
                            stk_amount_obj(&safe, k)
                        ),
                        format!("scoreboard players set @s {} 0", stk_amount_obj(&safe, k)),
                        format!("scoreboard players set @s {} 0", stk_live_obj(&safe, k)),
                    ];
                    take.push(format!("title @s actionbar {}", tr(&st.collected_message)));
                    fns.push((format!("stk_take_{safe}_{k}"), lines(&take)));
                }
            }
        }
    }

    // --- stk_collect / stk_ref / stk_gc: one place, every wager ---------------
    if !marking.is_empty() {
        // The right-click. One advancement fires it, because there is one box to
        // click; the place is located once and then offered to every stake, so a
        // death that left three datums here gives all three back in one press.
        //
        // **The place is the box this player CLICKED, never the one nearest them.**
        // The advancement says only that the player interacted with some `dw_stk`
        // box; which one is the interaction entity's own record — `on target` is
        // the last player to use it, and `interaction.timestamp` the tick they did.
        // Of the boxes this player has used, the most recent is the one just
        // clicked (`stk_pick`, one pass keeping the greatest timestamp). Taking the
        // `dw_stk` nearest the player instead offered the wagers at whatever place
        // stood closest: a player reaching past one stake to click another took
        // the purse at the near one — another player's, under `collect_by: anyone`
        // — and left their own standing.
        let mut collect: Vec<String> = vec![
            format!("advancement revoke @s only {ns}:{STK_COLLECT_FN}"),
            format!("tag @s add {STK_CLICKER}"),
            format!("scoreboard players set {STK_BEST} dw.sys -1"),
            format!(
                "execute as @e[type=minecraft:interaction,tag={tag}] run function {ns}:{STK_PICK_FN}"
            ),
            format!("tag @s remove {STK_CLICKER}"),
            format!("execute unless entity @e[tag={STK_HIT},limit=1] run return fail"),
        ];
        for (axis, s) in [STK_X, STK_Y, STK_Z].iter().enumerate() {
            collect.push(format!(
                "execute store result score {s} dw.sys run data get entity @e[tag={STK_HIT},limit=1] Pos[{axis}]"
            ));
        }
        collect.push(format!("tag @e[tag={STK_HIT}] remove {STK_HIT}"));
        for (_, safe) in &marking {
            collect.push(format!("function {ns}:stk_collect_{safe}"));
        }
        fns.push((STK_COLLECT_FN.to_string(), lines(&collect)));

        // Run AS each `dw_stk` box: keep it if the clicker used it, and used it
        // later than every box kept so far.
        fns.push((
            STK_PICK_FN.to_string(),
            lines(&[
                format!("scoreboard players set {STK_MINE} dw.sys 0"),
                format!(
                    "execute store success score {STK_MINE} dw.sys on target if entity @s[tag={STK_CLICKER}]"
                ),
                format!("execute unless score {STK_MINE} dw.sys matches 1 run return 0"),
                format!(
                    "execute store result score {STK_T} dw.sys run data get entity @s interaction.timestamp"
                ),
                format!("execute unless score {STK_T} dw.sys > {STK_BEST} dw.sys run return 0"),
                format!("scoreboard players operation {STK_BEST} dw.sys = {STK_T} dw.sys"),
                format!("tag @e[tag={STK_HIT}] remove {STK_HIT}"),
                format!("tag @s add {STK_HIT}"),
            ]),
        ));

        // Who still has a wager here — over every stake, because one live wager in
        // any datum is what keeps this place a place.
        let mut refs: Vec<String> = Vec::new();
        for (st, safe) in &marking {
            for k in 0..st.max_live() {
                refs.push(format!(
                    "execute{} run scoreboard players add {STK_REF} dw.sys 1",
                    slot_match(safe, k)
                ));
            }
        }
        fns.push((STK_REF_FN.to_string(), lines(&refs)));

        let mut gc: Vec<String> = Vec::new();
        for (axis, s) in [STK_X, STK_Y, STK_Z].iter().enumerate() {
            gc.push(format!(
                "execute store result score {s} dw.sys run data get entity @s Pos[{axis}]"
            ));
        }
        gc.push(format!("scoreboard players set {STK_REF} dw.sys 0"));
        gc.push(format!("execute as @a run function {ns}:{STK_REF_FN}"));
        gc.push(format!(
            "execute if score {STK_REF} dw.sys matches 0 run kill @e[tag={hw},limit=1,sort=nearest]"
        ));
        gc.push(format!(
            "execute if score {STK_REF} dw.sys matches 0 run kill @s"
        ));
        fns.push((STK_GC_FN.to_string(), lines(&gc)));
    }
    fns
}
