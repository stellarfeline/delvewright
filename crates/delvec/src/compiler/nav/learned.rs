//! **A shut way declares how it is learned** (spec-0118): every gated
//! connection a body meets shut says whether the player sees it from the
//! junction where its branch leaves the open route (`seen`), is told at its
//! face by a pressable answer (`told`), or is paid for the walk by a pickup,
//! interaction or fight on the branch (`rewarded`) — and the engine confirms
//! the word it is given.
//!
//! An undeclared shut way the party meets is `DW1053`; a declaration the world
//! does not confirm is `DW1054`. Judged over the configurations, faces and
//! reach of [`super::shutmodel`], the one model `DW1011` reads too.

use super::shutmodel::{
    Configurations, GateUnderTest, SeparationConfig, faces_of, gated_connections, gated_gate,
    meet, opens_low, party_reach,
};
use super::*;
use crate::compiler::failure::Failure;
use delvewright_dsl::layout::Learned;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

delvewright_dsl::dw_code! {
    /// `DW1053`: **a shut way the party meets, learned by no declared means**
    /// (spec-0118 §6.1). A gated connection is shut in a configuration the
    /// critical path passes through, the party's reach holds one of its faces,
    /// and its `gating` names no `learned`: nothing says how the player is to
    /// learn the way is shut, so nothing can be confirmed.
    pub const DW_SHUT_WAY_UNLEARNED: DwCode = DwCode::new("DW1053", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// `DW1054`: **a declaration the world does not confirm** (spec-0118 §6.2):
    /// `seen`, but from the junction the eye meets another cell before the
    /// bar; `told`, but nothing answers a press at the seam; `rewarded`, but no
    /// pickup, interaction or fight stands on the branch's ground. As a
    /// warning, a `learned` on a connection never met shut in any judged
    /// configuration (§6.3).
    pub const DW_SHUT_WAY_UNCONFIRMED: DwCode = DwCode::new("DW1054", ExitTier::Build);
}

/// How many failing connections a refusal names before summarising the rest.
const LIST_LIMIT: usize = 6;

/// One gated connection as this check judges it.
#[derive(Debug, Clone)]
pub(super) struct ShutWay {
    /// The gate under test — the one shape `DW1011` judges too.
    pub gate: GateUnderTest,
    /// The connection's id (`edge/…`).
    pub edge: String,
    /// Its class and gating in words, for a report.
    pub words: String,
    /// The places on its low and high faces, by node, where the seam says.
    pub sides: Option<(String, String)>,
    /// What the author declared.
    pub learned: Option<Learned>,
    /// The seam anchor a press is answered at (`anchor/seam-<edge>`).
    pub seam: String,
    /// The `use` trigger that answers a press at the seam, if one does
    /// ([`delvewright_dsl::QuestsContent::answers_press_at`]).
    pub answered_by: Option<String>,
}

/// A pickup, interaction or fight the campaign binds to a cell.
#[derive(Debug, Clone)]
pub(super) struct Reward {
    /// The anchor, as the document names it.
    pub anchor: String,
    /// What it is, in words (`a loot holder`, `a fight`).
    pub kind: &'static str,
    /// Where it resolves.
    pub pos: [i32; 3],
}

/// What the sight walk from the junction found.
#[derive(Debug, Clone, PartialEq, Eq)]
enum SeenFinding {
    /// The first cell met on the walk to some region cell is the bar.
    Seen([i32; 3]),
    /// Every walk met another cell first: `(region cell, blocking cell)`.
    Hidden(Vec<([i32; 3], [i32; 3])>),
}

/// What the branch ground holds.
#[derive(Debug, Clone, PartialEq, Eq)]
enum RewardFinding {
    /// A reward stands on or beside the branch ground.
    Rewarded(String),
    /// None does: the ground's size, and the nearest reward with its distance.
    Bare(usize, Option<(String, i32)>),
}

/// One shut way met: a connection, a configuration and a face the party is at.
#[derive(Debug, Clone)]
struct Met {
    way: usize,
    config: usize,
    high: bool,
    openable: Option<bool>,
    junction: [i32; 3],
    length: usize,
    rise: i32,
    seen: SeenFinding,
    reward: RewardFinding,
}

/// What [`check_shut_ways_learned`] measured, whether or not it refused.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShutWayBinding {
    /// Gated connections the world resolves a seam gate for.
    pub gated: usize,
    /// Distinct quest configurations judged.
    pub configurations: usize,
    /// Shut ways met at a face: `(connection, configuration, face)` triples.
    pub met: usize,
    /// Of them, triples whose connection declares `learned`.
    pub declared: usize,
    /// Of the declared, by word.
    pub seen: usize,
    /// Of the declared, by word.
    pub told: usize,
    /// Of the declared, by word.
    pub rewarded: usize,
    /// Triples whose connection declares nothing.
    pub undeclared: usize,
    /// Declared triples the world does not confirm.
    pub unconfirmed: usize,
    /// Connections that declare `learned` and are never met shut.
    pub inert: usize,
    /// The ledger rows, one per triple.
    pub rows: Vec<serde_json::Value>,
    /// The inert connections by id.
    pub inert_ways: Vec<String>,
    /// The triples met, for the agreement with `DW1011` (spec-0118 §5).
    pub(super) met_triples: BTreeSet<(usize, usize, bool)>,
}

impl ShutWayBinding {
    /// The one line a build prints about this check.
    pub fn line(&self) -> String {
        format!(
            "shut ways (DW1053/DW1054): {} gated connection(s); over {} configuration(s), {} shut \
             way(s) met at a face — {} declared ({} seen, {} told, {} rewarded), {} undeclared, \
             {} declaration(s) not confirmed; {} declared and never met shut",
            self.gated,
            self.configurations,
            self.met,
            self.declared,
            self.seen,
            self.told,
            self.rewarded,
            self.undeclared,
            self.unconfirmed,
            self.inert,
        )
    }

    /// The ledger `validation/shut-ways.json` carries.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "codes": [DW_SHUT_WAY_UNLEARNED.id(), DW_SHUT_WAY_UNCONFIRMED.id()],
            "gated": self.gated,
            "configurations": self.configurations,
            "met": self.met,
            "declared": self.declared,
            "seen": self.seen,
            "told": self.told,
            "rewarded": self.rewarded,
            "undeclared": self.undeclared,
            "unconfirmed": self.unconfirmed,
            "inert": self.inert,
            "inert_ways": self.inert_ways,
            "rows": self.rows,
        })
    }

    /// The warnings this check raises: a `learned` on a connection never met
    /// shut (§6.3), counted and named, never refused.
    pub fn findings(&self) -> Vec<delvewright_dsl::Diagnostic> {
        self.inert_ways
            .iter()
            .map(|edge| {
                delvewright_dsl::Diagnostic::warning(
                    DW_SHUT_WAY_UNCONFIRMED,
                    "build",
                    "shut-way binding",
                    format!(
                        "`{edge}` declares how the player learns it is shut (`gating.learned`), \
                         and no configuration the critical path passes through meets it shut at \
                         a face, so the word is confirmed by nothing. The judged configurations \
                         are the critical path's; a way met shut only on a branch's own path is \
                         not in them. The word is kept and counted; nothing is refused."
                    ),
                )
            })
            .collect()
    }
}

/// The reward anchors a campaign binds to cells (spec-0118 §5.3): a `collect`
/// objective's anchor (and the holder it adopts), an `interact` objective's
/// anchor, a `loot[]` entry's anchor, the anchor of a wave a `kill` objective
/// names, and an `ambush`'s `at`. A `reach-anchor` is not a reward.
fn rewards(plan: &Plan) -> Vec<Reward> {
    use delvewright_dsl::Objective;
    let q = &plan.campaign.quests.content;
    let mut named: Vec<(String, &'static str)> = Vec::new();
    for quest in &q.quests {
        for o in &quest.objectives {
            match o {
                Objective::Collect { anchor, .. } => {
                    named.push((anchor.as_str().to_string(), "a pickup"));
                    if let Some(h) = o.collect_holder() {
                        named.push((h.as_str().to_string(), "a pickup"));
                    }
                }
                Objective::Interact { anchor, .. } => {
                    named.push((anchor.as_str().to_string(), "an interaction"));
                }
                Objective::Kill { wave, .. } => {
                    if let Some(w) = q.waves.iter().find(|w| &w.id == wave) {
                        named.push((w.anchor.as_str().to_string(), "a fight"));
                    }
                }
                _ => {}
            }
        }
    }
    for l in &q.loot {
        named.push((l.anchor.as_str().to_string(), "a loot holder"));
    }
    for a in &q.ambushes {
        named.push((a.at.as_str().to_string(), "a fight"));
    }
    let mut out: Vec<Reward> = Vec::new();
    for (anchor, kind) in named {
        if let Some(pos) = super::shutmodel::point_anchor(plan, &anchor)
            && !out.iter().any(|r| r.anchor == anchor && r.pos == pos)
        {
            out.push(Reward { anchor, kind, pos });
        }
    }
    out
}

/// `DW1053` and `DW1054` over a campaign's gated connections, judged once per
/// quest configuration the critical path passes through. Returns the binding
/// beside the verdict so the caller can print it whichever way it went.
pub fn check_shut_ways_learned(
    plan: &Plan,
    world: &World,
) -> (ShutWayBinding, Result<(), Failure>) {
    let gated = gated_connections(plan);
    let q = &plan.campaign.quests.content;
    let triggers = q.all_triggers();
    let mut ways: Vec<ShutWay> = Vec::new();
    for (seal, _) in world.every_gate() {
        let Some(g) = gated.get(&seal.anchor) else {
            continue;
        };
        let e = g.edge;
        let sides = g.a_low.map(|a_low| {
            let (a, b) = (e.a().to_string(), e.b().to_string());
            if a_low { (a, b) } else { (b, a) }
        });
        let answered_by = triggers
            .iter()
            .find(|t| {
                matches!(t.on, delvewright_dsl::TriggerOn::Use)
                    && t.at_anchor() == Some(seal.anchor.as_str())
            })
            .map(|t| t.id.to_string());
        debug_assert_eq!(
            answered_by.is_some(),
            q.answers_press_at(&seal.anchor),
            "the press answer is the predicate DW0429 reads"
        );
        ways.push(ShutWay {
            gate: gated_gate(plan, g, seal),
            edge: e.id().to_string(),
            words: format!(
                "gated {} `{}` (opened on {})",
                e.class(),
                e.id(),
                e.gating().map(|g| g.words()).unwrap_or_default()
            ),
            sides,
            learned: e.learned(),
            seam: seal.anchor.clone(),
            answered_by,
        });
    }
    let configurations = Configurations::of(plan, world);
    let regions: Vec<([i32; 3], [i32; 3])> = ways.iter().map(|w| w.gate.region).collect();
    let judged = configurations.judged(plan, world, &regions);
    verify_shut_ways(&ways, world, &judged, &rewards(plan))
}

/// The breadth-first tree of body moves from the route cells: each cell's
/// predecessor, in visit order.
fn route_tree(w: &World, route: &[[i32; 3]]) -> BTreeMap<[i32; 3], Option<[i32; 3]>> {
    let mut pred: BTreeMap<[i32; 3], Option<[i32; 3]>> = BTreeMap::new();
    let mut queue: VecDeque<[i32; 3]> = VecDeque::new();
    for c in route {
        if pred.insert(*c, None).is_none() {
            queue.push_back(*c);
        }
    }
    while let Some(cur) = queue.pop_front() {
        for n in w.body_moves(cur) {
            if let std::collections::btree_map::Entry::Vacant(v) = pred.entry(n) {
                v.insert(Some(cur));
                queue.push_back(n);
            }
        }
    }
    pred
}

/// The path from the route to `target` over the tree, route end first.
fn path_to(tree: &BTreeMap<[i32; 3], Option<[i32; 3]>>, target: [i32; 3]) -> Vec<[i32; 3]> {
    let mut path = vec![target];
    let mut at = target;
    while let Some(Some(p)) = tree.get(&at) {
        path.push(*p);
        at = *p;
    }
    path.reverse();
    path
}

/// The body-move closure from `roots` that enters no cell of `route` — the
/// pocket a branch opens into, the route itself excluded.
fn branch_ground(
    w: &World,
    roots: &[[i32; 3]],
    route: &BTreeSet<[i32; 3]>,
) -> BTreeSet<[i32; 3]> {
    let mut seen: BTreeSet<[i32; 3]> = roots.iter().copied().collect();
    let mut queue: VecDeque<[i32; 3]> = seen.iter().copied().collect();
    while let Some(cur) = queue.pop_front() {
        for n in w.body_moves(cur) {
            if route.contains(&n) {
                continue;
            }
            if seen.insert(n) {
                queue.push_back(n);
            }
        }
    }
    seen
}

/// From the player's eye at `j`, whether the bar of `region` is the first
/// thing the eye meets (spec-0118 §5.3): `nav::walk_cells` to each region
/// cell's centre, stopped by [`World::blocks_camera`].
fn sight(w: &World, j: [i32; 3], region: ([i32; 3], [i32; 3]), eye_height: f64) -> SeenFinding {
    let c = cell_center(j);
    let eye = [c[0], c[1] + eye_height, c[2]];
    let (a, b) = region;
    let inside =
        |c: &[i32; 3]| (0..3).all(|i| a[i].min(b[i]) <= c[i] && c[i] <= a[i].max(b[i]));
    let mut hidden: Vec<([i32; 3], [i32; 3])> = Vec::new();
    for r in crate::compiler::assembled::region_cells(a, b) {
        let t = [
            f64::from(r[0]) + 0.5,
            f64::from(r[1]) + 0.5,
            f64::from(r[2]) + 0.5,
        ];
        match walk_cells(eye, t, |cell| w.blocks_camera(cell)) {
            Some(hit) if inside(&hit) => return SeenFinding::Seen(r),
            Some(hit) => hidden.push((r, hit)),
            // A region cell the model does not hold camera-blocking is not a
            // bar to be seen; the walk reached it through nothing.
            None => hidden.push((r, r)),
        }
    }
    SeenFinding::Hidden(hidden)
}

fn manhattan(a: [i32; 3], b: [i32; 3]) -> i32 {
    (a[0] - b[0]).abs() + (a[1] - b[1]).abs() + (a[2] - b[2]).abs()
}

/// Whether a reward stands in the branch ground or beside it, and if none
/// does, the nearest.
fn reward_on(ground: &BTreeSet<[i32; 3]>, rewards: &[Reward]) -> RewardFinding {
    let near = |p: [i32; 3]| {
        ground.contains(&p)
            || [
                [1, 0, 0],
                [-1, 0, 0],
                [0, 1, 0],
                [0, -1, 0],
                [0, 0, 1],
                [0, 0, -1],
            ]
            .iter()
            .any(|d| ground.contains(&[p[0] + d[0], p[1] + d[1], p[2] + d[2]]))
    };
    if let Some(r) = rewards.iter().find(|r| near(r.pos)) {
        return RewardFinding::Rewarded(r.anchor.clone());
    }
    let nearest = rewards
        .iter()
        .filter_map(|r| {
            ground
                .iter()
                .map(|g| manhattan(*g, r.pos))
                .min()
                .map(|d| (d, r.anchor.clone()))
        })
        .min()
        .map(|(d, a)| (a, d));
    RewardFinding::Bare(ground.len(), nearest)
}

/// The pure core of [`check_shut_ways_learned`]: the ways, the world with every
/// gate open (`base`, where faces are found), one configuration per judged
/// quest state, and the reward anchors. Split out so it is unit-testable over
/// a synthetic [`World`] without a [`Plan`].
pub(super) fn verify_shut_ways(
    ways: &[ShutWay],
    base: &World,
    configs: &[SeparationConfig<'_>],
    rewards: &[Reward],
) -> (ShutWayBinding, Result<(), Failure>) {
    verify_with_eye(
        ways,
        base,
        configs,
        rewards,
        delvewright_dsl::metrics::PLAYER_EYE_HEIGHT,
    )
}

fn verify_with_eye(
    ways: &[ShutWay],
    base: &World,
    configs: &[SeparationConfig<'_>],
    rewards: &[Reward],
    eye_height: f64,
) -> (ShutWayBinding, Result<(), Failure>) {
    let mut binding = ShutWayBinding {
        gated: ways.len(),
        configurations: configs.len(),
        ..ShutWayBinding::default()
    };
    let faces: Vec<Option<super::shutmodel::Faces>> = ways
        .iter()
        .map(|w| faces_of(base, w.gate.region, w.gate.axis))
        .collect();
    let opens: Vec<Option<bool>> = ways
        .iter()
        .zip(&faces)
        .map(|(w, f)| opens_low(&w.gate, f.as_ref()))
        .collect();
    let per_config: Vec<Vec<Met>> = crate::par::map(
        &configs.iter().enumerate().collect::<Vec<_>>(),
        |(ci, (w, _, route, shut))| {
            let (seeds, reach) = party_reach(w, route);
            let route_set: BTreeSet<[i32; 3]> = seeds.iter().copied().collect();
            let mut tree: Option<BTreeMap<[i32; 3], Option<[i32; 3]>>> = None;
            let mut out: Vec<Met> = Vec::new();
            for &gi in shut {
                let Some(f) = faces[gi].as_ref() else {
                    continue;
                };
                let m = meet(w, f, &reach);
                for high in m.met() {
                    let tree = tree.get_or_insert_with(|| route_tree(w, &seeds));
                    let face: Vec<[i32; 3]> = m
                        .face(high)
                        .iter()
                        .copied()
                        .filter(|c| reach.contains(c))
                        .collect();
                    // The nearest face cell by moves from the route, ties
                    // broken by the cell (ADR-0006).
                    let target = face
                        .iter()
                        .copied()
                        .min_by_key(|c| (path_to(tree, *c).len(), *c))
                        .expect("a met face holds a reached cell");
                    let path = path_to(tree, target);
                    let ji = path
                        .iter()
                        .rposition(|c| route_set.contains(c))
                        .unwrap_or(0);
                    let junction = path[ji];
                    let branch = &path[ji + 1..];
                    let rise = branch
                        .iter()
                        .map(|c| c[1])
                        .max()
                        .map_or(0, |y| y - junction[1]);
                    let ground = branch_ground(w, &face, &route_set);
                    out.push(Met {
                        way: gi,
                        config: *ci,
                        high,
                        openable: opens[gi].map(|low_opens| low_opens != high),
                        junction,
                        length: branch.len(),
                        rise,
                        seen: sight(w, junction, ways[gi].gate.region, eye_height),
                        reward: reward_on(&ground, rewards),
                    });
                }
            }
            out
        },
    );
    let met: Vec<Met> = per_config.into_iter().flatten().collect();
    let mut met_ways: BTreeSet<usize> = BTreeSet::new();
    // The first failing triple per connection, in (configuration, face) order.
    let mut unlearned: BTreeMap<usize, &Met> = BTreeMap::new();
    let mut unconfirmed: BTreeMap<usize, (&Met, usize)> = BTreeMap::new();
    for m in &met {
        let way = &ways[m.way];
        met_ways.insert(m.way);
        binding.met_triples.insert((m.way, m.config, m.high));
        binding.met += 1;
        let confirmed = match way.learned {
            None => {
                binding.undeclared += 1;
                unlearned.entry(m.way).or_insert(m);
                None
            }
            Some(l) => {
                binding.declared += 1;
                let ok = match l {
                    Learned::Seen => {
                        binding.seen += 1;
                        matches!(m.seen, SeenFinding::Seen(_))
                    }
                    Learned::Told => {
                        binding.told += 1;
                        way.answered_by.is_some()
                    }
                    Learned::Rewarded => {
                        binding.rewarded += 1;
                        matches!(m.reward, RewardFinding::Rewarded(_))
                    }
                };
                if !ok {
                    binding.unconfirmed += 1;
                    unconfirmed.entry(m.way).or_insert((m, 0)).1 += 1;
                }
                Some(ok)
            }
        };
        binding.rows.push(row(way, m, configs, confirmed));
    }
    for (i, w) in ways.iter().enumerate() {
        if w.learned.is_some() && !met_ways.contains(&i) {
            binding.inert += 1;
            binding.inert_ways.push(w.edge.clone());
        }
    }
    if !unlearned.is_empty() {
        let named: Vec<String> = unlearned
            .values()
            .take(LIST_LIMIT)
            .map(|m| unlearned_words(&ways[m.way], m, configs))
            .collect();
        let more = unlearned.len().saturating_sub(named.len());
        let tail = if more > 0 {
            format!("; and {more} more connection(s)")
        } else {
            String::new()
        };
        return (
            binding,
            Err(Failure {
                code: DW_SHUT_WAY_UNLEARNED,
                message: format!(
                    "{} story-shut way(s) the party meets say nothing about how the player learns \
                     they are shut: {}{tail}. A player who walks a branch to a shut way and finds \
                     it shut at the end has been cheated of the walk unless they could see it \
                     from where they turned off, are told at its face, or are paid for the walk. \
                     Write the word that is true in the connection's `gating.learned` — `seen`, \
                     `told` or `rewarded` — or make one true and write it.",
                    unlearned.len(),
                    named.join("; ")
                ),
            }),
        );
    }
    if !unconfirmed.is_empty() {
        let named: Vec<String> = unconfirmed
            .values()
            .take(LIST_LIMIT)
            .map(|(m, n)| {
                let mut s = unconfirmed_words(&ways[m.way], m, configs);
                if *n > 1 {
                    s.push_str(&format!(" (not confirmed at {n} face(s) met)"));
                }
                s
            })
            .collect();
        let more = unconfirmed.len().saturating_sub(named.len());
        let tail = if more > 0 {
            format!("; and {more} more connection(s)")
        } else {
            String::new()
        };
        return (
            binding,
            Err(Failure {
                code: DW_SHUT_WAY_UNCONFIRMED,
                message: format!(
                    "{} shut way(s) declare how the player learns they are shut, and the world \
                     does not confirm it: {}{tail}. Make the world true to the word — open the \
                     geometry so the bar is seen, write the trigger, place the reward — or change \
                     the word to the one that is true.",
                    unconfirmed.len(),
                    named.join("; ")
                ),
            }),
        );
    }
    (binding, Ok(()))
}

/// The face in words: by the place on that side, where the seam names it.
fn face_words(way: &ShutWay, m: &Met) -> String {
    let side = match &way.sides {
        Some((low, high)) => format!("the `{}` side", if m.high { high } else { low }),
        None => format!(
            "its {} face",
            if m.high { "high" } else { "low" }
        ),
    };
    match m.openable {
        Some(true) => format!("{side}, the side it opens from"),
        Some(false) => format!("{side}, the side it does not open from"),
        None => side,
    }
}

fn when(configs: &[SeparationConfig<'_>], ci: usize) -> &str {
    configs.get(ci).map_or("", |c| c.1.as_str())
}

fn seen_words(m: &Met) -> String {
    match &m.seen {
        SeenFinding::Seen(r) => format!("the bar is seen from {:?} (at {r:?})", m.junction),
        SeenFinding::Hidden(h) => {
            let blockers: BTreeSet<[i32; 3]> = h.iter().map(|(_, b)| *b).collect();
            let list: Vec<String> = blockers.iter().map(|b| format!("{b:?}")).collect();
            format!(
                "the bar is hidden from {:?} behind {}",
                m.junction,
                list.join(", ")
            )
        }
    }
}

fn told_words(way: &ShutWay) -> String {
    match &way.answered_by {
        Some(t) => format!("`{t}` answers a press at `{}`", way.seam),
        None => format!("nothing answers a press at `{}`", way.seam),
    }
}

fn reward_words(m: &Met) -> String {
    match &m.reward {
        RewardFinding::Rewarded(a) => format!("`{a}` stands on the branch"),
        RewardFinding::Bare(n, nearest) => {
            let mut s = format!("no pickup, interaction or fight within the branch's {n} cell(s)");
            if let Some((a, d)) = nearest {
                s.push_str(&format!(" (the nearest is `{a}`, {d} block(s) from it)"));
            }
            s
        }
    }
}

fn place_words(way: &ShutWay, m: &Met, configs: &[SeparationConfig<'_>]) -> String {
    format!(
        "{} is shut {} and the party meets it at {}; the branch leaves the route at {:?}, \
         {} move(s) long, rising {}",
        way.words,
        when(configs, m.config),
        face_words(way, m),
        m.junction,
        m.length,
        m.rise
    )
}

fn unlearned_words(way: &ShutWay, m: &Met, configs: &[SeparationConfig<'_>]) -> String {
    format!(
        "{}. `seen` — {}; `told` — {}; `rewarded` — {}",
        place_words(way, m, configs),
        seen_words(m),
        told_words(way),
        reward_words(m)
    )
}

fn unconfirmed_words(way: &ShutWay, m: &Met, configs: &[SeparationConfig<'_>]) -> String {
    let place = place_words(way, m, configs);
    match way.learned {
        Some(Learned::Seen) => format!(
            "{place}; declared `seen`, but from {:?} the eye meets another cell before the bar: {}",
            m.junction,
            seen_words(m)
        ),
        Some(Learned::Told) => format!(
            "{place}; declared `told`, but nothing answers a press at `{}`. Anchor a trigger on \
             the seam — {}",
            way.seam,
            delvewright_dsl::trigger::press_answer_prescription(&way.seam, "the way")
        ),
        Some(Learned::Rewarded) => format!(
            "{place}; declared `rewarded`, but {}",
            reward_words(m)
        ),
        None => place,
    }
}

fn row(
    way: &ShutWay,
    m: &Met,
    configs: &[SeparationConfig<'_>],
    confirmed: Option<bool>,
) -> serde_json::Value {
    let seen = match &m.seen {
        SeenFinding::Seen(r) => serde_json::json!({ "seen_from": m.junction, "bar_cell": r }),
        SeenFinding::Hidden(h) => {
            let blockers: BTreeSet<[i32; 3]> = h.iter().map(|(_, b)| *b).collect();
            serde_json::json!({ "blocked_at": blockers })
        }
    };
    let told = match &way.answered_by {
        Some(t) => serde_json::json!({ "answered_by": t }),
        None => serde_json::json!({ "answered_by": null, "at": way.seam }),
    };
    let reward = match &m.reward {
        RewardFinding::Rewarded(a) => serde_json::json!({ "rewarded_by": a }),
        RewardFinding::Bare(n, nearest) => serde_json::json!({
            "rewarded_by": null,
            "ground_cells": n,
            "nearest": nearest.as_ref().map(|(a, d)| serde_json::json!({ "anchor": a, "distance": d })),
        }),
    };
    serde_json::json!({
        "connection": way.edge,
        "gating": way.words,
        "configuration": when(configs, m.config),
        "face": match &way.sides {
            Some((low, high)) => if m.high { high.clone() } else { low.clone() },
            None => if m.high { "high".to_string() } else { "low".to_string() },
        },
        "openable": m.openable,
        "junction": m.junction,
        "branch_length": m.length,
        "branch_rise": m.rise,
        "learned": way.learned.map(Learned::word),
        "confirmed": confirmed,
        "seen": seen,
        "told": told,
        "rewarded": reward,
    })
}
