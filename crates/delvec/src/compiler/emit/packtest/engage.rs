use super::*;

/// The lowest course an engagement pad stands on: above every course a delve
/// builds on, below the pinned overworld's build top (320).
const PAD_FLOOR_Y: i32 = 300;
/// The highest course an engagement pad may stand on, leaving its two
/// courses of headroom under the build top.
const PAD_CEILING_Y: i32 = 316;
/// How far past its farthest body the pad runs, and how wide it is on either
/// side: a blow knocks a body back, and a bowman steps away to shoot.
const PAD_MARGIN: i32 = 8;

/// One kind of hostile body a wave seats, as the engagement template drives it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct EngagedKind {
    /// The entity id.
    pub entity: String,
    /// The bodies of the kind a one-player seating puts in the world.
    pub count: u32,
    /// The reach the proofs read for it (`nav::follow::mob_reach`, the least
    /// over the wave's stacks of the kind).
    pub reach: f64,
    /// Where the template stands the kind from the party, blocks along +x.
    pub distance: f64,
    /// Whether `distance` lies beyond the lane's `aggro_radius` — the only
    /// distance at which a mob held at the radius would let the party go.
    pub beyond_lane: bool,
}

/// **The distance the template stands a kind from the party**: for a lane
/// mob whose reach runs past the lane's radius, midway between the two — so a
/// mob still held at the radius after its release lets the party go, and a
/// knock or a step back does not carry one past its reach; for any other, half
/// its reach (at least one block).
pub(crate) fn engage_distance(reach: f64, lane_radius: Option<u32>) -> (f64, bool) {
    match lane_radius.map(f64::from) {
        Some(r) if reach > r + 1.0 => ((r + reach) / 2.0, true),
        _ => ((reach / 2.0).max(1.0), false),
    }
}

/// The hostile kinds of `w`, in declaration order: every stack whose species
/// the game files under `monster` (`nav::follow::is_monster`).
pub(crate) fn engaged_kinds(w: &delvewright_dsl::Wave) -> Vec<EngagedKind> {
    let mut out: Vec<EngagedKind> = Vec::new();
    for m in &w.mobs {
        if !crate::compiler::nav::follow::is_monster(&m.entity) {
            continue;
        }
        let reach = crate::compiler::nav::follow::mob_reach(w, m).0;
        match out.iter_mut().find(|k| k.entity == m.entity) {
            Some(k) => {
                k.count += m.count.base();
                k.reach = k.reach.min(reach);
            }
            None => out.push(EngagedKind {
                entity: m.entity.clone(),
                count: m.count.base(),
                reach,
                distance: 0.0,
                beyond_lane: false,
            }),
        }
    }
    let lane = w.lane.as_ref().map(|l| l.aggro_radius);
    for k in &mut out {
        (k.distance, k.beyond_lane) = engage_distance(k.reach, lane);
    }
    out
}

/// What the engagement templates bound, zeroes included.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EngageBinding {
    /// Waves declared.
    pub waves: usize,
    /// Waves with a hostile stack, each given a template.
    pub hostile: usize,
    /// Hostile kinds asserted across them.
    pub kinds: usize,
    /// Of those, kinds stood past their lane's radius.
    pub beyond_lane: usize,
    /// Hostile waves with no seat, given no template.
    pub unseated: Vec<String>,
}

impl EngageBinding {
    /// The binding line.
    pub fn line(&self) -> String {
        format!(
            "engagement binding (PackTest): {} hostile wave(s) of {} declared, {} hostile kind(s) \
             asserted to keep the party as their target within their reach, {} of them stood past \
             their lane's aggro radius; {} hostile wave(s) unseated and unasserted{}",
            self.hostile,
            self.waves,
            self.kinds,
            self.beyond_lane,
            self.unseated.len(),
            if self.unseated.is_empty() {
                String::new()
            } else {
                format!(" ({})", self.unseated.join(", "))
            }
        )
    }
}

/// **A hostile wave's bodies take the party as their target within the
/// reach the compiler's proofs read** (`nav::follow::mob_reach`): one template
/// per wave with a hostile stack. The wave is seated, every hostile body is
/// stood on a barrier pad above the world at its kind's distance from the
/// dummy ([`engage_distance`]), struck once by the dummy, its lane clock run,
/// and the template waits until every body holds the dummy as its target at
/// once (`engage_poll_<wave>`, counted every tick) — by the blow or by sight;
/// one that never does times it out. Whether the party survives, or how hard
/// the fight is, is not asked (ADR-0039): only that the bodies engage at the
/// reach the proofs assume. A lane mob whose follow range is held at the
/// lane's radius drops a target past it, so the template reds on that
/// emission.
pub(super) fn emit_engagement_packtests(
    plan: &Plan,
    out: &mut BuildOutput,
    placements: &WavePlacements,
) -> EngageBinding {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let top = plan
        .areas
        .iter()
        .flat_map(|a| a.pieces.iter().map(|p| p.bbox().1[1]))
        .max()
        .unwrap_or(0);
    let y = (top + 4).clamp(PAD_FLOOR_Y, PAD_CEILING_Y);
    let mut b = EngageBinding {
        waves: plan.campaign.quests.content.waves.len(),
        ..EngageBinding::default()
    };
    for w in &plan.campaign.quests.content.waves {
        let kinds = engaged_kinds(w);
        if kinds.is_empty() {
            continue;
        }
        let Some(seat) = placements.get(w.id.as_str()).and_then(|c| c.first()) else {
            b.unseated.push(w.id.as_str().to_string());
            continue;
        };
        b.hostile += 1;
        b.kinds += kinds.len();
        b.beyond_lane += kinds.iter().filter(|k| k.beyond_lane).count();
        let safe = plan::safe_local(w.id.as_str());
        let tag = plan::wave_tag(w.id.as_str());
        let name = format!("engage_{safe}");
        let (pin, sel) = pin_dummy(&format!("dw_pt_eng_{safe}"));
        let far = kinds
            .iter()
            .map(|k| k.distance.ceil() as i32)
            .max()
            .unwrap_or(1);
        let (x0, z0) = (seat[0], seat[2]);
        let (xa, za, xb, zb) = (
            x0 - 2,
            z0 - PAD_MARGIN,
            x0 + far + PAD_MARGIN,
            z0 + PAD_MARGIN,
        );
        let pad = format!("{xa} {} {za} {xb} {} {zb}", y - 1, y - 1);
        let mark = format!("dw_pt_eng_{safe}");
        let all = format!("#ea_{safe}");
        let on = format!("#eon_{safe}");
        let total: u32 = kinds.iter().map(|k| k.count).sum();
        // The poll: every body of every hostile kind that holds the dummy as
        // its target, counted every tick while the template waits on it.
        let poll = format!("engage_poll_{safe}");
        let mut pl = vec![format!("scoreboard players set {all} dw.sys 0")];
        for (i, k) in kinds.iter().enumerate() {
            let held = format!("#e{i}_{safe}");
            pl.push(format!("scoreboard players set {held} dw.sys 0"));
            pl.push(format!(
                "execute as @e[tag={tag},type={}] on target if entity @s[tag={mark}] run \
                 scoreboard players add {held} dw.sys 1",
                k.entity
            ));
            pl.push(format!(
                "execute if score {held} dw.sys matches {}.. run scoreboard players add {all} \
                 dw.sys {}",
                k.count, k.count
            ));
        }
        pl.push(format!(
            "execute if score {on} dw.sys matches 1 run schedule function {ns}:{poll} 1t"
        ));
        out.insert(
            format!("packtest-datapack/data/{ns}/function/{poll}.mcfunction"),
            lines(&pl).into_bytes(),
        );
        let mut t = vec![
            format!(
                "#> {title}: every hostile of `{}` takes the party as its target within the reach \
                 the proofs read (spec-0016 §6)",
                w.id
            ),
            super::limit::alone(ns, &name, out),
            "# @dummy".to_string(),
            "# @timeout 200".to_string(),
            String::new(),
            format!("function {ns}:setup"),
            pin,
            // The dummy is the party standing in the fight: in no cutscene
            // (`cs_repair` returns a player left tagged to its checkpoint),
            // and free of the playable region's clock while it stands on the
            // pad above the world.
            format!("tag {sel} remove {}", super::super::cutscene::CUTSCENE_TAG),
            format!("tag {sel} add {}", crate::compiler::creator::FREE_TAG),
            format!("kill @e[tag={tag}]"),
            format!("forceload add {xa} {za} {xb} {zb}"),
            format!("fill {pad} minecraft:barrier replace minecraft:air"),
            format!("tp {sel} {}.5 {y} {}.5", x0, z0),
            format!("effect give {sel} minecraft:resistance 60 4 true"),
        ];
        t.extend(super::super::wave::packtest_spawn_lines(plan, &safe));
        for k in &kinds {
            t.push(format!(
                "execute as @e[tag={tag},type={}] run tp @s {} {y} {}.5",
                k.entity,
                fmt_f64(f64::from(x0) + 0.5 + k.distance),
                z0
            ));
        }
        t.push(format!(
            "execute as @e[tag={tag}] run damage @s 1 minecraft:player_attack by {sel}"
        ));
        if w.lane.is_some() {
            t.push(format!("function {ns}:lane_tick_{safe}"));
        }
        t.push(format!("scoreboard players set {on} dw.sys 1"));
        t.push(format!("function {ns}:{poll}"));
        // Every body takes the dummy as its target at some tick, by the blow
        // or by sight, while the dummy stands within its reach; one that never
        // does times the template out.
        t.push(format!("await score {all} dw.sys matches {total}"));
        t.push(format!("scoreboard players set {on} dw.sys 0"));
        t.push(format!("kill @e[tag={tag}]"));
        t.push(format!(
            "fill {pad} minecraft:air replace minecraft:barrier"
        ));
        t.push(format!("effect clear {sel}"));
        t.push(format!(
            "tag {sel} remove {}",
            crate::compiler::creator::FREE_TAG
        ));
        t.push(format!("tag {sel} remove {mark}"));
        out.insert(
            format!("packtest-datapack/data/{ns}/test/{name}.mcfunction"),
            lines(&t).into_bytes(),
        );
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A lane pillager (reach 32) and a lane vindicator (12) are stood past
    /// their 8-block radius; a vindicator on a 14-block lane reaches the
    /// radius only and is not.
    #[test]
    fn a_kind_is_stood_inside_its_reach_and_past_its_lane() {
        assert_eq!(engage_distance(32.0, Some(8)), (20.0, true));
        assert_eq!(engage_distance(12.0, Some(8)), (10.0, true));
        assert_eq!(engage_distance(14.0, Some(14)), (7.0, false));
        assert_eq!(engage_distance(12.0, None), (6.0, false));
        assert_eq!(engage_distance(1.0, None), (1.0, false));
    }
}
