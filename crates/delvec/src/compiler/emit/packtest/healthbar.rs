use super::*;

/// spec-0073: **a fight shows its health**, proven on the pinned server with
/// the real generated functions — one template per declared bar, plus one that
/// proves several bars stand at once when the campaign declares more than one.
///
/// Per bar (`health_bar_<key>`):
///
/// 1. the fight is met exactly as the campaign meets it — `spawn_<wave>`, or
///    `spawn_actor_<id>` and, for an actor the campaign unleashes,
///    `unleash_<id>` — so the max is the one the real summon captured;
/// 2. the dummy and the fight are lifted to a private altitude (one per bar,
///    far enough apart that no two templates' ranges meet), because every
///    template's dummy shares the server and a count of the audience is only
///    exact where no sibling can stand;
/// 3. in range, after the real `hb_<key>`: one player, visible, value equal to
///    max, and max equal to the fight's summed `max_health` — the declared
///    literal when every body declares it, otherwise the server's own
///    `attribute … base get` summed by `scoreboard players operation`, a read
///    that shares no function with the engine's `get`-and-accumulate;
/// 4. one body's `Health` set to 1 (a PackTest dummy is immune to `/damage`, so
///    health is moved with `data modify`, the `souls_reseat_undefeated`
///    pattern): the value moved by exactly the difference;
/// 5. the dummy lifted out of `range`: no player;
/// 6. with a bonfire that re-seats this fight, the real `bonfire_rest_<i>`:
///    value equals max again once the dummy stands back in range;
/// 7. every body killed: hidden.
///
/// Emits nothing for a campaign that declares no bar → byte-identical.
pub(super) fn emit_health_bar_packtests(
    plan: &Plan,
    out: &mut BuildOutput,
    wave_placements: &WavePlacements,
) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let bars = crate::compiler::healthbar::bars(plan.campaign);
    if bars.is_empty() {
        return;
    }
    let bonfire = plan.bonfires().next().map(|b| b.index);
    let hostile: BTreeSet<String> = crate::compiler::combat::hostile_actors(plan.campaign)
        .iter()
        .map(|a| a.id.as_str().to_string())
        .collect();
    // A rest re-seats every met `respawns_on_rest` wave too, so a template that
    // rests owns that whole board: cleared on entry and on exit (pin_dummy rule 4).
    let board: Vec<String> = plan
        .reseat_waves()
        .iter()
        .flat_map(|r| {
            [
                format!("kill @e[tag={}]", plan::wave_tag(r.id.as_str())),
                format!(
                    "scoreboard players set {} dw.sys 0",
                    wave_seated_holder(r.id.as_str())
                ),
            ]
        })
        .collect();
    for (k, bar) in bars.iter().enumerate() {
        use delvewright_dsl::FightKind;
        let safe = &bar.safe;
        // How the fight is met, and whether a rest re-seats it.
        let (meet, rests, clear) = match bar.fight.kind() {
            FightKind::Wave => {
                if !wave_placements.contains_key(bar.fight.id()) {
                    continue; // never seated → no spawn function (DW0310 owns a dangling spawn)
                }
                let rests = plan
                    .reseat_waves()
                    .iter()
                    .any(|w| w.id.as_str() == bar.fight.id())
                    || plan
                        .undefeated_reseat_waves()
                        .iter()
                        .any(|w| w.id.as_str() == bar.fight.id());
                (
                    vec![format!("function {ns}:spawn_{safe}")],
                    rests,
                    format!("kill @e[tag={}]", plan::wave_tag(bar.fight.id())),
                )
            }
            FightKind::Actor => {
                let Some(a) = plan
                    .campaign
                    .quests
                    .content
                    .actors
                    .iter()
                    .find(|a| a.id.as_str() == bar.fight.id())
                else {
                    continue;
                };
                if plan
                    .body_point(delvewright_dsl::BodyRef::Actor(a))
                    .is_none()
                {
                    continue; // never placed → no spawn function (DW0325)
                }
                let mut meet = vec![format!("function {ns}:spawn_actor_{safe}")];
                if hostile.contains(bar.fight.id()) {
                    meet.push(format!("function {ns}:unleash_{safe}"));
                }
                let rests = plan.reseat_actors().iter().any(|r| r.id == a.id);
                (meet, rests, format!("kill @e[tag=dw_actor_{safe}]"))
            }
        };
        let rest = bonfire.filter(|_| rests);
        let id = bar.id(ns);
        let live = bar.live();
        let refresh = format!("function {ns}:{}", bar.refresh_fn());
        let range = bar.bar.range;
        let lift = 200 + 200 * k as i32;
        let x = format!("hb{k}");
        let (pin, sel) = pin_dummy(&format!("dw_{x}"));
        let chip = format!("dw_{x}_chip");
        let get = |what: &str, holder: &str| {
            format!("execute store result score #{holder}_{x} dw.sys run bossbar get {id} {what}")
        };
        let mut b = packtest_header(&format!(
            "{title}: the health bar over {} `{}` shows the fight to a player in range, follows \
             its health, and hides when it is over (spec-0073)",
            bar.fight.kind().word(),
            bar.fight.id()
        ));
        b.push(format!("function {ns}:setup"));
        b.push(pin);
        b.extend(board.iter().cloned());
        b.push(clear.clone());
        b.push(format!("tag @e[tag={chip}] remove {chip}"));
        b.extend(meet);
        // A private stage: the dummy first, then the fight to it.
        b.push(format!("execute at {sel} run tp {sel} ~ ~{lift} ~"));
        b.push(format!("execute at {sel} run tp @e[{live}] ~ ~ ~"));
        b.push(refresh.clone());
        b.push(get("players", "p"));
        b.push(format!("assert score #p_{x} dw.sys matches 1"));
        b.push(get("visible", "s"));
        b.push(format!("assert score #s_{x} dw.sys matches 1"));
        b.push(get("value", "v"));
        b.push(get("max", "m"));
        b.push(format!("assert score #m_{x} dw.sys matches 1.."));
        b.push(format!(
            "scoreboard players operation #e_{x} dw.sys = #v_{x} dw.sys"
        ));
        b.push(format!(
            "scoreboard players operation #e_{x} dw.sys -= #m_{x} dw.sys"
        ));
        b.push(format!("assert score #e_{x} dw.sys matches 0"));
        match declared_max_health(plan.campaign, bar) {
            Some(total) => b.push(format!("assert score #m_{x} dw.sys matches {total}")),
            None => {
                b.push(format!(
                    "execute as @e[{live}] store result score @s dw.sys run attribute @s \
                     minecraft:max_health base get 1"
                ));
                b.push(format!("scoreboard players set #x_{x} dw.sys 0"));
                b.push(format!(
                    "scoreboard players operation #x_{x} dw.sys += @e[{live}] dw.sys"
                ));
                b.push(format!("scoreboard players reset @e[{live}] dw.sys"));
                b.push(format!(
                    "scoreboard players operation #x_{x} dw.sys -= #m_{x} dw.sys"
                ));
                b.push(format!("assert score #x_{x} dw.sys matches 0"));
            }
        }
        // Chip one body to 1 HP: the value moves by exactly what it lost.
        b.push(format!("tag @e[{live},limit=1] add {chip}"));
        b.push(format!(
            "execute store result score #h_{x} dw.sys run data get entity @e[tag={chip},limit=1] \
             Health 1"
        ));
        b.push(format!(
            "data modify entity @e[tag={chip},limit=1] Health set value 1.0f"
        ));
        b.push(refresh.clone());
        b.push(get("value", "w"));
        b.push(format!(
            "scoreboard players operation #d_{x} dw.sys = #v_{x} dw.sys"
        ));
        b.push(format!(
            "scoreboard players operation #d_{x} dw.sys -= #w_{x} dw.sys"
        ));
        b.push(format!(
            "scoreboard players operation #d_{x} dw.sys -= #h_{x} dw.sys"
        ));
        b.push(format!("scoreboard players add #d_{x} dw.sys 1"));
        b.push(format!("assert score #d_{x} dw.sys matches 0"));
        // Out of range: nobody sees it, and it is still standing.
        b.push(format!("execute at {sel} run tp {sel} ~ ~{} ~", range + 8));
        b.push(refresh.clone());
        b.push(get("players", "q"));
        b.push(format!("assert score #q_{x} dw.sys matches 0"));
        b.push(get("visible", "t"));
        b.push(format!("assert score #t_{x} dw.sys matches 1"));
        // A rest re-seats the chipped fight whole: full again, through the REAL
        // rest function and the capture its re-seat runs.
        if let Some(i) = rest {
            b.push(format!("function {ns}:bonfire_rest_{i}"));
            b.push(format!("execute at @e[{live},limit=1] run tp {sel} ~ ~ ~"));
            b.push(refresh.clone());
            b.push(get("value", "r"));
            b.push(get("max", "n"));
            b.push(format!("assert score #n_{x} dw.sys matches 1.."));
            b.push(format!(
                "scoreboard players operation #r_{x} dw.sys -= #n_{x} dw.sys"
            ));
            b.push(format!("assert score #r_{x} dw.sys matches 0"));
        }
        // The last body falls: hidden, on the same refresh.
        b.push(clear.clone());
        b.push(refresh.clone());
        b.push(get("visible", "u"));
        b.push(format!("assert score #u_{x} dw.sys matches 0"));
        b.push(format!("tag @e[tag={chip}] remove {chip}"));
        b.push(clear);
        b.extend(board.iter().cloned());
        b.push(format!("tag {sel} remove dw_{x}"));
        out.insert(
            format!(
                "packtest-datapack/data/{ns}/test/health_bar_{}.mcfunction",
                bar.key
            ),
            lines(&b).into_bytes(),
        );
    }
    // Several bars at once: world init leaves exactly the declared set, each by
    // its own id — the several-bars-at-once behaviour proven where it is used.
    if bars.len() >= 2 {
        let mut b = packtest_header(&format!(
            "{title}: world init stands exactly the {} declared health bars (spec-0073)",
            bars.len()
        ));
        b.push(format!("function {ns}:setup"));
        b.push("execute store result score #n_hbl dw.sys run bossbar list".to_string());
        b.push(format!("assert score #n_hbl dw.sys matches {}", bars.len()));
        for (k, bar) in bars.iter().enumerate() {
            b.push(format!(
                "execute store success score #e{k}_hbl dw.sys run bossbar get {} max",
                bar.id(ns)
            ));
            b.push(format!("assert score #e{k}_hbl dw.sys matches 1"));
        }
        out.insert(
            format!("packtest-datapack/data/{ns}/test/health_bar_several.mcfunction"),
            lines(&b).into_bytes(),
        );
    }
}

/// The summed `max_health` a fight's bodies declare, when EVERY body declares
/// it — a number the compiler can state without the server. `None` when any
/// body leaves it to the game, and then the template asks the server instead.
fn declared_max_health(
    c: &delvewright_dsl::Campaign,
    bar: &crate::compiler::healthbar::Bar<'_>,
) -> Option<i64> {
    use delvewright_dsl::FightKind;
    let floor = |h: f64| h.floor() as i64;
    match bar.fight.kind() {
        FightKind::Wave => {
            let w = c
                .quests
                .content
                .waves
                .iter()
                .find(|w| w.id.as_str() == bar.fight.id())?;
            w.mobs.iter().try_fold(0i64, |acc, m| {
                let h = m.attributes.as_ref()?.max_health?;
                Some(acc + floor(h) * i64::from(m.count))
            })
        }
        FightKind::Actor => {
            let a = c
                .quests
                .content
                .actors
                .iter()
                .find(|a| a.id.as_str() == bar.fight.id())?;
            Some(floor(a.attributes.as_ref()?.max_health?))
        }
    }
}
