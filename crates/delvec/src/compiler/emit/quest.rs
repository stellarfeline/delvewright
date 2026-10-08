//! Quests: arming order, pending guards, the lobby and the outro.

use super::*;

/// The closing line on the completion advancement: the authored `world.outro`
/// (l10n key `world.outro`), else the finale quest's `goal` (key
/// `quest.<id>.goal`) — the thing the party just accomplished, already inventoried
/// and translated. Both are campaign content, so no English is baked in here.
/// Falls back to the delve title only if the finale names no planned quest, which
/// cross-stage validation (`DW0160`) already rejects.
pub(super) fn campaign_outro(c: &delvewright_dsl::Campaign) -> String {
    if let Some(outro) = &c.world.content.outro {
        return outro.clone();
    }
    let finale = c.quest_plan.content.finale.as_str();
    c.quest_plan
        .content
        .quests
        .iter()
        .find(|q| q.id.as_str() == finale)
        .map(|q| q.goal.clone())
        .unwrap_or_else(|| c.world.content.title.clone())
}

/// The `dw.sys` fake player holding the live online-player count, recomputed each
/// tick — the lobby gate's only input (spec-0018 `world.min_players`). Emitted
/// only for a campaign that declares `min_players >= 2`.
pub(super) const LOBBY_COUNT: &str = "#lobby";

/// The lobby's waiting message (spec-0018): a live "x / n" actionbar for players
/// who have not taken a class yet, while the party is short. The count is a
/// vanilla `score` component reading [`LOBBY_COUNT`], so it updates itself
/// without any per-count emission. English-first (CLAUDE.md language policy); a
/// compiler default, not an authored string, so it is not l10n-inventoried.
pub(super) fn lobby_actionbar(min_players: u8, chrome: &delvewright_dsl::Chrome) -> String {
    // One sentence with two `with` arguments — the live count and the size the
    // delve requires — so a translation orders them for its own language instead
    // of inheriting English's `<prefix> <n> / <n>`.
    tr_with(
        &chrome.get(delvewright_dsl::chrome::LOBBY_WAITING),
        &[
            ("color", json!("yellow")),
            (
                "with",
                json!([
                    { "score": { "name": LOBBY_COUNT, "objective": "dw.sys" }, "color": "gold" },
                    { "text": min_players.to_string(), "color": "gold" }
                ]),
            ),
        ],
    )
    .to_string()
}

/// The intra-quest activation + pending guard for an objective (v0.3): the quest
/// must be active, every `after` prerequisite and `requires_flags` flag set, no
/// `forbids_flags` flag set (v0.6 negative gate; `unless … matches 1` so an
/// unset score counts as "not set"), and the objective itself not yet complete.
/// Returns the ` if …`/` unless …` fragment (leading space); callers append the
/// type-specific condition + `run`, prepending `execute as @a` when they need a
/// player to test (proximity, inventory, a fired trigger) and a bare `execute`
/// otherwise.
///
/// **Every term reads the party holder** (spec-0018). That is what makes an
/// `after: [obj/a, obj/b]` AND-join a division of labor: player A clearing
/// `obj/a` in one room and player B clearing `obj/b` in another both write
/// `#party`, so the successor's guard opens for the whole party. It is also what
/// keeps the drivers single-fire under `as @a`: vanilla evaluates the conditions
/// per selected player in turn, so the first player's `run` sets the party score
/// and every later player's `unless score #party …` fails in the same tick.
/// Stage-5 quests in **arming order**: a quest whose completion arms another is
/// visited before the quest it arms.
///
/// The arming graph is exactly the `Trigger::QuestComplete` edges — the only two
/// trigger kinds are `CampaignStart` (a root, armed by `setup`) and
/// `QuestComplete`, so this is the whole of it.
///
/// **Stable.** Declaration order breaks every tie and seeds the ready set, so a
/// campaign already declared in arming order — which every campaign built so far
/// is — comes back in exactly its declared order and emits byte-identically. It
/// is also **total**: a cycle (already an error elsewhere; a quest cannot arm
/// itself through any path and still be reachable) leaves an unresolved tail,
/// which is appended in declaration order rather than dropped, because a lost
/// quest would be a far worse failure than a badly-ordered one.
pub(super) fn quests_in_arming_order(
    c: &delvewright_dsl::Campaign,
) -> Vec<&delvewright_dsl::Quest> {
    let quests = &c.quests.content.quests;
    let index: BTreeMap<&str, usize> = quests
        .iter()
        .enumerate()
        .map(|(i, q)| (q.id.as_str(), i))
        .collect();
    let mut indegree = vec![0usize; quests.len()];
    let mut arms: Vec<Vec<usize>> = vec![Vec::new(); quests.len()];
    for (i, q) in quests.iter().enumerate() {
        if let Trigger::QuestComplete { quest } = &q.trigger
            && let Some(&p) = index.get(quest.as_str())
            && p != i
        {
            indegree[i] += 1;
            arms[p].push(i);
        }
    }
    // Kahn's algorithm with a declaration-ordered ready queue: among quests that
    // become ready together, the earliest-declared is always emitted first.
    let mut ready: Vec<usize> = (0..quests.len()).filter(|&i| indegree[i] == 0).collect();
    let mut order: Vec<usize> = Vec::with_capacity(quests.len());
    while !ready.is_empty() {
        let i = ready.remove(0);
        order.push(i);
        for &dep in &arms[i] {
            indegree[dep] -= 1;
            if indegree[dep] == 0 {
                let pos = ready.partition_point(|&r| r < dep);
                ready.insert(pos, dep);
            }
        }
    }
    let mut seen = vec![false; quests.len()];
    let mut out: Vec<&delvewright_dsl::Quest> = Vec::with_capacity(quests.len());
    for i in order {
        seen[i] = true;
        out.push(&quests[i]);
    }
    for (i, q) in quests.iter().enumerate() {
        if !seen[i] {
            out.push(q);
        }
    }
    out
}

pub(super) fn pending_guard(plan: &Plan, o: &Objective, quest_active: &str) -> String {
    let p = plan::PARTY;
    let mut g = format!(" if score {p} {quest_active} matches 1");
    for a in o.after() {
        g.push_str(&format!(
            " if score {p} {} matches 1",
            obj_score(a.as_str())
        ));
    }
    // The objective's whole gate, in gate field order: required flags, forbidden
    // flags, then (DSL v0.10) the numeric terms. Written through `gate_cond` so
    // this guard cannot end up knowing about two of the gate's three fields —
    // which is exactly how the numeric axis would have been missed. Empty terms
    // contribute nothing, so a pre-0.10 campaign's guard is byte-identical.
    g.push_str(&gate_cond(plan, o.gate()));
    g.push_str(&format!(
        " unless score {p} {} matches 1",
        obj_score(o.id().as_str())
    ));
    g
}
