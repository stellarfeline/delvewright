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
    Configurations, GateUnderTest, SeparationConfig, faces_of, gated_connections, gated_gate, meet,
    opens_low, party_reach,
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
    Rewarded(String, &'static str),
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
    /// Gated connections the layout graph declares (`Edge::shuts`).
    pub gated: usize,
    /// Of them, connections whose seam gate the world resolves — the
    /// population judged.
    pub resolved: usize,
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
            "shut ways (DW1053/DW1054): {} gated connection(s) ({} with a seam gate the world \
             resolves); over {} configuration(s), {} shut way(s) met at a face — {} declared ({} seen, {} told, {} rewarded), {} undeclared, \
             {} declaration(s) not confirmed; {} declared and never met shut",
            self.gated,
            self.resolved,
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
            "resolved": self.resolved,
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
fn reward_anchors(q: &delvewright_dsl::QuestsContent) -> Vec<(String, &'static str)> {
    use delvewright_dsl::Objective;
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
    named
}

/// The reward anchors of [`reward_anchors`], each resolved to its cell; an
/// anchor that resolves to no point is not a reward a cell holds.
fn rewards(plan: &Plan) -> Vec<Reward> {
    let mut out: Vec<Reward> = Vec::new();
    for (anchor, kind) in reward_anchors(&plan.campaign.quests.content) {
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
    let (mut binding, verdict) = verify_shut_ways(&ways, world, &judged, &rewards(plan));
    binding.gated = gated.len();
    (binding, verdict)
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

/// **The branch ground** (spec-0118 §5.2): the cells a body reaches from the
/// shut way's face that it cannot reach from the open route without passing
/// the junction or walking the branch path — the ground the branch alone
/// leads to. The open side is the body-move closure of the route cells with
/// the junction and the branch path taken out; the ground is the closure from
/// the face, less that open side, less every cell one move from it, and less
/// the route itself. On an open floor
/// the open side flows round the branch path, so the ground is empty: an open
/// floor is not a branch, and nothing on it pays for one.
fn branch_ground(
    w: &World,
    face: &[[i32; 3]],
    route: &BTreeSet<[i32; 3]>,
    junction: [i32; 3],
    branch: &[[i32; 3]],
) -> BTreeSet<[i32; 3]> {
    let cut: BTreeSet<[i32; 3]> = branch.iter().copied().chain([junction]).collect();
    let flood = |roots: Vec<[i32; 3]>, avoid: &BTreeSet<[i32; 3]>| {
        let mut seen: BTreeSet<[i32; 3]> = roots.into_iter().collect();
        let mut queue: VecDeque<[i32; 3]> = seen.iter().copied().collect();
        while let Some(cur) = queue.pop_front() {
            for n in w.body_moves(cur) {
                if !avoid.contains(&n) && seen.insert(n) {
                    queue.push_back(n);
                }
            }
        }
        seen
    };
    let open = flood(
        route.iter().copied().filter(|c| !cut.contains(c)).collect(),
        &cut,
    );
    let none = BTreeSet::new();
    // A cell one move from the open side is open ground too: on an open floor
    // the branch path's own cells, taken out of the flood, stand beside it.
    flood(face.to_vec(), &none)
        .into_iter()
        .filter(|c| {
            !open.contains(c)
                && !route.contains(c)
                && !w.body_moves(*c).iter().any(|n| open.contains(n))
        })
        .collect()
}

/// From the player's eye at `j`, whether the bar of `region` is the first
/// thing the eye meets (spec-0118 §5.3): `nav::walk_cells` to each region
/// cell's centre, stopped by [`World::blocks_camera`].
fn sight(w: &World, j: [i32; 3], region: ([i32; 3], [i32; 3]), eye_height: f64) -> SeenFinding {
    let c = cell_center(j);
    let eye = [c[0], c[1] + eye_height, c[2]];
    let (a, b) = region;
    let inside = |c: &[i32; 3]| (0..3).all(|i| a[i].min(b[i]) <= c[i] && c[i] <= a[i].max(b[i]));
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
        return RewardFinding::Rewarded(r.anchor.clone(), r.kind);
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
        resolved: ways.len(),
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
                    let ground = branch_ground(w, &face, &route_set, junction, branch);
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
                        matches!(m.reward, RewardFinding::Rewarded(..))
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
        None => format!("its {} face", if m.high { "high" } else { "low" }),
    };
    match m.openable {
        Some(true) => format!("{side}, the side it opens from"),
        Some(false) => format!("{side}, the side it does not open from"),
        None => side,
    }
}

fn when<'a>(configs: &'a [SeparationConfig<'_>], ci: usize) -> &'a str {
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
        RewardFinding::Rewarded(a, kind) => format!("`{a}` ({kind}) stands on the branch"),
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
        Some(Learned::Rewarded) => format!("{place}; declared `rewarded`, but {}", reward_words(m)),
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
        RewardFinding::Rewarded(a, kind) => serde_json::json!({ "rewarded_by": a, "kind": kind }),
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

/// `DW1053` and `DW1054` over synthetic worlds, and the agreement with
/// `DW1011` over the one shut-gate model (spec-0118 acceptance 2, 4–8).
#[cfg(test)]
mod learned_tests {
    use super::super::separate::verify_gates_separate;
    use super::*;

    const WALL: i32 = 4;

    /// A world of stone cells (floor and walls), feet at y = 1 over a floor
    /// at y = 0. `open` lists the walkable feet cells; every other cell of the
    /// box `lo..=hi` up to `WALL` high is stone, and `extra` adds stone.
    fn carve(lo: [i32; 2], hi: [i32; 2], open: &BTreeSet<[i32; 2]>, extra: &[[i32; 3]]) -> World {
        let mut solid = BTreeSet::new();
        for x in lo[0] - 1..=hi[0] + 1 {
            for z in lo[1] - 1..=hi[1] + 1 {
                solid.insert([x, 0, z]);
                if !open.contains(&[x, z]) {
                    for y in 1..=WALL {
                        solid.insert([x, y, z]);
                    }
                }
            }
        }
        solid.extend(extra.iter().copied());
        World::from_solid_cells(solid)
    }

    fn way(region: ([i32; 3], [i32; 3]), axis: usize, learned: Option<Learned>) -> ShutWay {
        ShutWay {
            gate: GateUnderTest {
                label: "gated walk `edge/test`".to_string(),
                region,
                axis: Some(axis),
                opens_low: None,
                unlock: None,
            },
            edge: "edge/test".to_string(),
            words: "gated walk `edge/test` (opened on `flag/open`)".to_string(),
            sides: Some(("node/near".to_string(), "node/far".to_string())),
            learned,
            seam: "anchor/seam-test".to_string(),
            answered_by: None,
        }
    }

    /// A route corridor along x at z = 0 (x 0..=10), a branch off it through
    /// a mouth one wide at `[5, 1]`, three wide (x 4..=6) from z = 2 to `len`,
    /// and a one-wide bar at
    /// `[5, 1..=top, len + 1]` with a room past it. `shut` fills the bar.
    fn branch(len: i32, top: i32, shut: bool, extra: &[[i32; 3]]) -> World {
        let mut open = BTreeSet::new();
        for x in 0..=10 {
            open.insert([x, 0]);
        }
        // The branch leaves the route by a mouth one wide, then widens.
        open.insert([5, 1]);
        for z in 2..=len {
            for x in 4..=6 {
                open.insert([x, z]);
            }
        }
        for z in len + 2..=len + 4 {
            for x in 4..=6 {
                open.insert([x, z]);
            }
        }
        let mut extra = extra.to_vec();
        let mut w_open = open.clone();
        w_open.insert([5, len + 1]);
        let mut solid_extra = Vec::new();
        for y in top + 1..=WALL {
            solid_extra.push([5, y, len + 1]);
        }
        if shut {
            for y in 1..=top {
                solid_extra.push([5, y, len + 1]);
            }
        }
        extra.extend(solid_extra);
        carve([0, 0], [10, len + 4], &w_open, &extra)
    }

    fn route() -> Vec<[i32; 3]> {
        (0..=10).map(|x| [x, 1, 0]).collect()
    }

    fn judge(
        w: &World,
        base: &World,
        ways: &[ShutWay],
        rewards: &[Reward],
        eye: f64,
    ) -> (ShutWayBinding, Result<(), Failure>) {
        verify_with_eye(
            ways,
            base,
            &[(w, "while `obj/test` is next".to_string(), route(), vec![0])],
            rewards,
            eye,
        )
    }

    /// Acceptance 4: a bar at the end of a straight branch off a route cell is
    /// seen; the ledger row names the junction and the bar cell; a solid cell
    /// on the sight line is refused `DW1054` naming it.
    #[test]
    fn a_bar_at_the_end_of_a_straight_branch_is_seen() {
        let region = ([5, 1, 8], [5, 2, 8]);
        let (base, shut) = (branch(7, 2, false, &[]), branch(7, 2, true, &[]));
        let ways = [way(region, 2, Some(Learned::Seen))];
        let (b, verdict) = judge(&shut, &base, &ways, &[], PLAYER_EYE);
        assert!(verdict.is_ok(), "{verdict:?}");
        assert_eq!((b.met, b.seen, b.unconfirmed), (1, 1, 0), "{}", b.line());
        let row = &b.rows[0];
        assert_eq!(row["junction"], serde_json::json!([5, 1, 0]), "{row}");
        // Measured in body moves, not cells: a run of seven cells is walked
        // and jumped in fewer.
        let moves = row["branch_length"].as_u64().unwrap();
        assert!((1..=7).contains(&moves), "{row}");
        assert!(row["branch_rise"].as_i64().unwrap() >= 0, "{row}");
        assert_eq!(row["face"], "node/near", "{row}");
        assert!(row["seen"]["bar_cell"].is_array(), "{row}");

        // A block hung on the line: the branch is three wide, so a body walks
        // round it, and the eye meets it before the bar.
        let blocker = [5, 2, 4];
        let (base, shut) = (
            branch(7, 2, false, &[blocker]),
            branch(7, 2, true, &[blocker]),
        );
        let (b, verdict) = judge(&shut, &base, &ways, &[], PLAYER_EYE);
        let err = verdict.expect_err("a block on the sight line hides the bar");
        assert_eq!(err.code.id(), "DW1054", "{}", err.message);
        assert!(err.message.contains("[5, 2, 4]"), "{}", err.message);
        assert!(err.message.contains("declared `seen`"), "{}", err.message);
        assert_eq!(b.unconfirmed, 1);
        assert_eq!(
            b.rows[0]["seen"]["blocked_at"],
            serde_json::json!([[5, 2, 4]])
        );
    }

    const PLAYER_EYE: f64 = delvewright_dsl::metrics::PLAYER_EYE_HEIGHT;

    /// Acceptance 4: the eye is the player's, 1.62 above the junction's floor,
    /// not the mob's 1.5. A one-high bar ten cells down the branch; a block on
    /// the 1.5 line and off the 1.62 line, chosen by walking both.
    #[test]
    fn the_eye_is_the_players_not_the_mobs() {
        let region = ([5, 1, 10], [5, 1, 10]);
        let target = [5.5, 1.5, 10.5];
        let eye = |h: f64| [5.5, 1.0 + h, 0.5];
        let walked = |h: f64| {
            let mut cells = BTreeSet::new();
            walk_cells(eye(h), target, |c| {
                cells.insert(c);
                false
            });
            cells
        };
        let low = walked(1.5);
        let high = walked(PLAYER_EYE);
        let blocker = [5, 1, 5];
        assert!(low.contains(&blocker) && !high.contains(&blocker));
        let (base, shut) = (
            branch(9, 1, false, &[blocker]),
            branch(9, 1, true, &[blocker]),
        );
        let ways = [way(region, 2, Some(Learned::Seen))];
        let (_, at_player) = judge(&shut, &base, &ways, &[], PLAYER_EYE);
        assert!(at_player.is_ok(), "{at_player:?}");
        let (_, at_mob) = judge(&shut, &base, &ways, &[], 1.5);
        assert_eq!(at_mob.expect_err("1.5 meets the block").code.id(), "DW1054");
        // The core every caller reaches is the player's eye.
        let (_, core) = verify_shut_ways(
            &ways,
            &base,
            &[(&shut, "now".to_string(), route(), vec![0])],
            &[],
        );
        assert!(core.is_ok(), "{core:?}");
    }

    /// Acceptance 4: a hatch, thin on y, is seen from the route cell beside
    /// it, and a cover over a ladder's hole from a body on the rung below.
    #[test]
    fn a_hatch_is_seen_from_beside_and_a_cover_from_the_rung_below() {
        // The route runs along z = 4; the hatch is in the floor at [5, 0, 5],
        // over a cellar three deep.
        let build = |shut: bool| {
            let mut solid = BTreeSet::new();
            for x in -1..=11 {
                for z in 3..=6 {
                    if !(4..=6).contains(&x) || !(4..=6).contains(&z) || z == 3 {
                        solid.insert([x, 0, z]);
                    }
                    for y in 1..=WALL {
                        if z == 3 || z == 6 || x == -1 || x == 11 {
                            solid.insert([x, y, z]);
                        }
                    }
                }
            }
            for x in 4..=6 {
                for z in 4..=6 {
                    if [x, z] != [5, 5] {
                        solid.insert([x, 0, z]);
                    }
                    solid.insert([x, -4, z]);
                }
            }
            for x in 3..=7 {
                for z in 3..=7 {
                    if !(4..=6).contains(&x) || !(4..=6).contains(&z) {
                        for y in -4..=0 {
                            solid.insert([x, y, z]);
                        }
                    }
                }
            }
            if shut {
                solid.insert([5, 0, 5]);
            }
            World::from_solid_cells(solid)
        };
        let (base, shut) = (build(false), build(true));
        let hatch = way(([5, 0, 5], [5, 0, 5]), 1, Some(Learned::Seen));
        let route: Vec<[i32; 3]> = (0..=10).map(|x| [x, 1, 4]).collect();
        let (b, verdict) = verify_shut_ways(
            std::slice::from_ref(&hatch),
            &base,
            &[(&shut, "now".to_string(), route, vec![0])],
            &[],
        );
        assert!(verdict.is_ok(), "{verdict:?} {:?}", b.rows);
        assert_eq!((b.met, b.seen), (1, 1), "{}", b.line());
        assert_eq!(b.rows[0]["junction"], serde_json::json!([5, 1, 4]));

        // The cover: a ladder up a pillar into a hole in a ceiling at y = 5,
        // the cover in the course above it.
        const LADDER: &str = "minecraft:ladder[facing=east,waterlogged=false]";
        let build = |shut: bool| {
            let mut cells: Vec<([i32; 3], &str)> = Vec::new();
            for x in 0..7 {
                for z in 0..7 {
                    cells.push(([x, 0, z], "minecraft:stone"));
                    if [x, z] != [3, 3] {
                        cells.push(([x, 5, z], "minecraft:stone"));
                    }
                }
            }
            for y in 1..=5 {
                cells.push(([3, y, 3], LADDER));
                cells.push(([2, y, 3], "minecraft:stone"));
            }
            if shut {
                cells.push(([3, 6, 3], delvewright_dsl::siteplan::SEAM_COVER));
            }
            crate::compiler::nav::testkit::blocks_world(&cells)
        };
        let (base, shut) = (build(false), build(true));
        let cover = way(([3, 6, 3], [3, 6, 3]), 1, Some(Learned::Seen));
        let (b, verdict) = verify_shut_ways(
            std::slice::from_ref(&cover),
            &base,
            &[(&shut, "now".to_string(), vec![[3, 3, 3]], vec![0])],
            &[],
        );
        assert!(verdict.is_ok(), "{verdict:?} {:?}", b.rows);
        assert_eq!(b.seen, 1, "{}", b.line());
        assert_eq!(b.rows[0]["junction"], serde_json::json!([3, 3, 3]));
    }

    /// Acceptance 5 (core half): `told` holds where a `use` trigger answers
    /// at the seam, and is refused `DW1054` with the `DW0429` prescription
    /// where nothing does; the prescription parses as a trigger the press
    /// predicate counts.
    #[test]
    fn told_reads_the_press_answer_and_prescribes_its_trigger() {
        let region = ([5, 1, 8], [5, 2, 8]);
        let (base, shut) = (branch(7, 2, false, &[]), branch(7, 2, true, &[]));
        let mut told = way(region, 2, Some(Learned::Told));
        told.answered_by = Some("trigger/test".to_string());
        let (b, verdict) = judge(&shut, &base, std::slice::from_ref(&told), &[], PLAYER_EYE);
        assert!(verdict.is_ok(), "{verdict:?}");
        assert_eq!(b.told, 1);
        told.answered_by = None;
        let (_, verdict) = judge(&shut, &base, &[told], &[], PLAYER_EYE);
        let err = verdict.expect_err("nothing answers");
        assert_eq!(err.code.id(), "DW1054");
        let json = err
            .message
            .split("Anchor a trigger on the seam — `")
            .nth(1)
            .and_then(|t| t.split('`').next())
            .expect("the message carries the prescription")
            .replace("trigger/<name>", "trigger/seam-answer");
        let trigger: delvewright_dsl::EnvTrigger =
            serde_json::from_str(&json).expect("the prescription is a trigger");
        let mut q: delvewright_dsl::QuestsContent =
            serde_json::from_value(serde_json::json!({ "quests": [] })).expect("an empty mission");
        q.triggers.push(trigger);
        assert!(q.answers_press_at("anchor/seam-test"));
        assert_eq!(
            json,
            delvewright_dsl::trigger::press_answer_prescription("anchor/seam-test", "the way")
                .trim_matches('`')
                .replace("trigger/<name>", "trigger/seam-answer")
        );
    }

    /// Acceptance 6 (core half): a chest on the branch ground pays for it; the
    /// same chest on a route cell far from the branch is refused, named as
    /// the nearest reward with its distance.
    #[test]
    fn a_chest_on_the_branch_pays_for_it() {
        let region = ([5, 1, 8], [5, 2, 8]);
        let (base, shut) = (branch(7, 2, false, &[]), branch(7, 2, true, &[]));
        let ways = [way(region, 2, Some(Learned::Rewarded))];
        let chest = |pos| Reward {
            anchor: "anchor/chest".to_string(),
            kind: "a loot holder",
            pos,
        };
        let (b, verdict) = judge(&shut, &base, &ways, &[chest([6, 1, 6])], PLAYER_EYE);
        assert!(verdict.is_ok(), "{verdict:?}");
        assert_eq!(b.rewarded, 1);
        assert_eq!(b.rows[0]["rewarded"]["rewarded_by"], "anchor/chest");
        let (_, verdict) = judge(&shut, &base, &ways, &[chest([0, 1, 0])], PLAYER_EYE);
        let err = verdict.expect_err("a chest at the far end of the route is no pay");
        assert_eq!(err.code.id(), "DW1054");
        assert!(
            err.message
                .contains("the nearest is `anchor/chest`, 6 block(s) from it"),
            "{}",
            err.message
        );
    }

    /// Acceptance 6: a `reach-anchor` is not a reward; a `collect`, an
    /// `interact`, a `loot[]` holder, a `kill`'s wave and an ambush are.
    #[test]
    fn a_reach_anchor_is_not_a_reward() {
        let q: delvewright_dsl::QuestsContent = serde_json::from_value(serde_json::json!({
            "quests": [{
                "id": "quest/q",
                "trigger": { "type": "campaign-start" },
                "on_complete": [],
                "objectives": [
                    { "type": "reach-anchor", "id": "obj/reach", "anchor": "anchor/ledge", "radius": 2 },
                    { "type": "interact", "id": "obj/touch", "anchor": "anchor/lever" },
                    { "type": "kill", "id": "obj/kill", "wave": "wave/w" },
                    { "type": "collect", "id": "obj/take", "item": "minecraft:apple", "count": 1,
                      "anchor": "anchor/basket" }
                ]
            }],
            "waves": [{ "id": "wave/w", "anchor": "anchor/pit",
                        "mobs": [{ "entity": "minecraft:zombie", "count": 1 }] }],
            "loot": [{ "id": "loot/c", "anchor": "anchor/chest",
                       "items": [{ "item": "minecraft:apple" }] }]
        }))
        .expect("a quests document parses");
        let names: BTreeSet<String> = reward_anchors(&q).into_iter().map(|(a, _)| a).collect();
        assert!(!names.contains("anchor/ledge"), "{names:?}");
        for a in [
            "anchor/lever",
            "anchor/pit",
            "anchor/basket",
            "anchor/chest",
        ] {
            assert!(names.contains(a), "{a} in {names:?}");
        }
    }

    /// Acceptance 7: a shut way met with no word is refused `DW1053` once, at
    /// its first configuration, naming the three words; never met, it passes
    /// with `U` at 0. Acceptance 8: a word on a way never met is counted
    /// inert and warned under `DW1054`.
    #[test]
    fn an_undeclared_way_met_is_refused_once_and_one_never_met_passes() {
        let region = ([5, 1, 8], [5, 2, 8]);
        let (base, shut) = (branch(7, 2, false, &[]), branch(7, 2, true, &[]));
        let ways = [way(region, 2, None)];
        let two = [
            (&shut, "while `obj/a` is next".to_string(), route(), vec![0]),
            (&shut, "while `obj/b` is next".to_string(), route(), vec![0]),
        ];
        let (b, verdict) = verify_shut_ways(&ways, &base, &two, &[]);
        let err = verdict.expect_err("met with no word");
        assert_eq!(err.code.id(), "DW1053");
        assert_eq!(
            err.message.matches("edge/test").count(),
            1,
            "{}",
            err.message
        );
        assert!(
            err.message.contains("while `obj/a` is next"),
            "{}",
            err.message
        );
        assert!(
            !err.message.contains("while `obj/b` is next"),
            "{}",
            err.message
        );
        for w in [
            "`seen` — the bar is seen",
            "`told` — nothing answers",
            "`rewarded` — no pickup",
        ] {
            assert!(err.message.contains(w), "{w}: {}", err.message);
        }
        assert_eq!((b.met, b.undeclared), (2, 2));

        // Never met: the route is walled off from the branch.
        let wall: Vec<[i32; 3]> = (4..=6)
            .flat_map(|x| (1..=WALL).map(move |y| [x, y, 1]))
            .collect();
        let walled = branch(7, 2, true, &wall);
        let walled_base = branch(7, 2, false, &wall);
        let one = [(&walled, "now".to_string(), route(), vec![0])];
        let (b, verdict) = verify_shut_ways(&ways, &walled_base, &one, &[]);
        assert!(verdict.is_ok(), "{verdict:?}");
        assert_eq!((b.met, b.undeclared, b.inert), (0, 0, 0), "{}", b.line());

        let declared = [way(region, 2, Some(Learned::Seen))];
        let (b, verdict) = verify_shut_ways(&declared, &walled_base, &one, &[]);
        assert!(verdict.is_ok(), "{verdict:?}");
        assert_eq!(
            (b.inert, b.inert_ways.clone()),
            (1, vec!["edge/test".to_string()])
        );
        let warnings = b.findings();
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].code, "DW1054");
        assert!(
            b.line().contains("1 declared and never met shut"),
            "{}",
            b.line()
        );
    }

    /// spec-0118 §5.2: an open floor is not a branch. A hall eleven square with
    /// the route along its near wall and a barred door in its far wall: the
    /// branch path runs across open floor, the open side flows round it, and
    /// the branch ground is empty — so a chest standing in the hall's far
    /// corner, off the route and off the path, does not pay for the walk. The
    /// ground read as "not a route cell" held the whole hall, and confirmed it.
    #[test]
    fn a_reward_elsewhere_on_an_open_floor_does_not_confirm_rewarded() {
        let build = |shut: bool| {
            let mut open = BTreeSet::new();
            for x in 0..=10 {
                for z in 0..=10 {
                    open.insert([x, z]);
                }
            }
            open.insert([5, 11]);
            for x in 4..=6 {
                open.insert([x, 12]);
            }
            let mut extra: Vec<[i32; 3]> = (3..=WALL).map(|y| [5, y, 11]).collect();
            if shut {
                extra.extend([[5, 1, 11], [5, 2, 11]]);
            }
            carve([0, 0], [10, 12], &open, &extra)
        };
        let (base, shut) = (build(false), build(true));
        let ways = [way(([5, 1, 11], [5, 2, 11]), 2, Some(Learned::Rewarded))];
        let route: Vec<[i32; 3]> = (0..=10).map(|x| [x, 1, 0]).collect();
        let chest = Reward {
            anchor: "anchor/far-corner".to_string(),
            kind: "a loot holder",
            pos: [9, 1, 9],
        };
        let (b, verdict) = verify_shut_ways(
            &ways,
            &base,
            &[(&shut, "now".to_string(), route, vec![0])],
            std::slice::from_ref(&chest),
        );
        let err = verdict.expect_err("a chest in an open hall is no branch's pay");
        assert_eq!(err.code.id(), "DW1054", "{}", err.message);
        assert_eq!(b.rows[0]["rewarded"]["ground_cells"], 0, "{:?}", b.rows);
    }

    /// Acceptance 9: the binding line prints every count, zeroes included.
    #[test]
    fn the_binding_line_states_every_count() {
        let b = ShutWayBinding::default();
        assert_eq!(
            b.line(),
            "shut ways (DW1053/DW1054): 0 gated connection(s) (0 with a seam gate the world \
             resolves); over 0 configuration(s), 0 shut way(s) met at a face — 0 declared (0 \
             seen, 0 told, 0 rewarded), 0 undeclared, 0 declaration(s) not confirmed; 0 declared \
             and never met shut"
        );
        assert_eq!(b.to_json()["met"], 0);
    }

    /// A small deterministic generator (ADR-0006): no RNG crate, a fixed LCG.
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self, n: u64) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (self.0 >> 33) % n
        }
    }

    /// Acceptance 2: `DW1011` and this check meet the same (gate,
    /// configuration, face) triples over generated worlds — two yards either
    /// side of a wall, doors in it, pillars, gaps and pits scattered, route
    /// cells and the shut set drawn per configuration.
    #[test]
    fn both_proofs_meet_the_same_faces_over_generated_worlds() {
        let mut met_any = 0usize;
        for seed in 0..40u64 {
            let mut r = Lcg(seed * 7919 + 13);
            let doors: Vec<i32> = (0..3).map(|i| 2 + 4 * i + r.next(2) as i32).collect();
            let build = |shut: &BTreeSet<usize>, r: &mut Lcg| {
                let mut solid = BTreeSet::new();
                for x in -1..=14 {
                    for z in -1..=16 {
                        solid.insert([x, 0, z]);
                        if x == -1 || x == 14 || z == -1 || z == 16 {
                            for y in 1..=WALL {
                                solid.insert([x, y, z]);
                            }
                        }
                    }
                    for y in 1..=WALL {
                        let door = doors.iter().position(|d| *d == x);
                        let open = door.is_some_and(|d| !shut.contains(&d)) && y <= 2;
                        if !open {
                            solid.insert([x, y, 8]);
                        }
                    }
                }
                for _ in 0..r.next(12) {
                    let (x, z) = (r.next(14) as i32, r.next(16) as i32);
                    if z != 8 && z != 7 && z != 9 {
                        for y in 1..=(1 + r.next(3) as i32) {
                            solid.insert([x, y, z]);
                        }
                    }
                }
                World::from_solid_cells(solid)
            };
            let gates: Vec<GateUnderTest> = doors
                .iter()
                .map(|d| GateUnderTest {
                    label: format!("gated walk `edge/door-{d}`"),
                    region: ([*d, 1, 8], [*d, 2, 8]),
                    axis: Some(2),
                    opens_low: None,
                    unlock: None,
                })
                .collect();
            let base = build(&BTreeSet::new(), &mut Lcg(seed));
            let worlds: Vec<(World, Vec<[i32; 3]>, Vec<usize>)> = (0..3)
                .map(|_| {
                    let shut: BTreeSet<usize> = (0..3).filter(|_| r.next(3) > 0).collect();
                    let w = build(&shut, &mut Lcg(seed));
                    let route: Vec<[i32; 3]> = (0..=r.next(4))
                        .map(|_| [r.next(14) as i32, 1, r.next(16) as i32])
                        .collect();
                    (w, route, shut.into_iter().collect())
                })
                .collect();
            let configs: Vec<SeparationConfig<'_>> = worlds
                .iter()
                .enumerate()
                .map(|(i, (w, route, shut))| {
                    (w, format!("config {i}"), route.clone(), shut.clone())
                })
                .collect();
            let (sep, _) = verify_gates_separate(&gates, &[], &base, &configs);
            let ways: Vec<ShutWay> = gates
                .iter()
                .map(|g| ShutWay {
                    gate: g.clone(),
                    ..way(g.region, 2, Some(Learned::Seen))
                })
                .collect();
            let (learned, _) = verify_shut_ways(&ways, &base, &configs, &[]);
            assert_eq!(sep.met, learned.met_triples, "seed {seed}");
            met_any += learned.met_triples.len();
        }
        assert!(met_any > 0, "the generator meets some face");
    }
}
