//! **A shut way declares how it is learned** (spec-0118): every gated
//! connection a body meets shut, or an eye sees shut, says whether the player
//! sees it from the junction where its branch leaves the open route (`seen`),
//! is told at its face by a pressable answer (`told`), is paid for the walk by
//! a pickup, interaction or fight on the branch (`rewarded`), or does not learn
//! of it at all until the shot that opens it shows it (`hidden`) — and the
//! engine confirms the word it is given.
//!
//! An undeclared shut way the party meets is `DW1053`; a declaration the world
//! does not confirm is `DW1054`. Judged over the configurations, faces and
//! reach of [`super::shutmodel`], the one model `DW1011` reads too, and with the
//! one eye: every sight walk here is stopped by [`World::hides`] — occlusion in
//! the pinned jar — and by nothing else (`docs/reference/sight-through-blocks.md`).
//! The text a hidden way keeps is `DW1060`'s ([`crate::compiler::telling`]),
//! which reads the shot this module finds ([`opening_lists`]).

use super::shutmodel::{
    Configurations, GateUnderTest, SeparationConfig, faces_of, gated_connections, gated_gate, meet,
    opens_low, party_reach,
};
use super::*;
use crate::compiler::failure::Failure;
use delvewright_dsl::layout::Learned;
use std::collections::{BTreeMap, BTreeSet};

delvewright_dsl::dw_code! {
    /// `DW1053`: **a shut way the party meets, learned by no declared means**
    /// (spec-0118 §6.1). A gated connection is shut in a configuration the
    /// critical path passes through, the party's reach holds one of its faces
    /// or an eye in the reach sees it, and its `gating` names no `learned`:
    /// nothing says how the player is to learn the way is shut, so nothing can
    /// be confirmed.
    pub const DW_SHUT_WAY_UNLEARNED: DwCode = DwCode::new("DW1053", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// `DW1054`: **a declaration the world does not confirm** (spec-0118 §6.2):
    /// `seen`, but from the junction the eye meets a hiding cell before the
    /// bar; `told`, but no `use` trigger serves the seam; `rewarded`, but no
    /// pickup, interaction or fight stands on the branch's ground; `hidden`,
    /// but an eye in the reach sees the way or its far side, or the beat that
    /// opens it carries no shot of its seam. As a warning, a word on a
    /// connection never met shut in any judged configuration (§6.3).
    pub const DW_SHUT_WAY_UNCONFIRMED: DwCode = DwCode::new("DW1054", ExitTier::Build);
}

/// How many failing connections a refusal names before summarising the rest.
const LIST_LIMIT: usize = 6;

/// An inclusive box of world cells.
type Bx = ([i32; 3], [i32; 3]);

/// Each cell's predecessor on the breadth-first tree of body moves.
type Tree = BTreeMap<[i32; 3], Option<[i32; 3]>>;

/// One effect list that opens a gated connection's seam gate, and the
/// `cutscene` in it whose `look_at`, `path` or shot names that seam, if one
/// does (spec-0118 §5.3 b).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpeningList {
    /// The list's pointer (`/content/quests/0/objectives/2/on_complete`).
    pub(crate) list: String,
    /// The index in the list of the `cutscene` that shows the seam.
    pub(crate) shot: Option<usize>,
}

impl OpeningList {
    /// The pointer of the shot's own effect, when there is one.
    pub(crate) fn shot_pointer(&self) -> Option<String> {
        self.shot.map(|i| format!("{}/{i}", self.list))
    }
}

/// Whether a `cutscene` effect looks at `anchor`: its single shot's `look_at`
/// or a `path` waypoint, or any multi-shot shot's `look_at`, `path` or
/// `subject` names it.
fn shows(e: &delvewright_dsl::QuestEffect, anchor: &str) -> bool {
    let delvewright_dsl::Verb::Cutscene {
        shots,
        path,
        look_at,
        ..
    } = &e.verb
    else {
        return false;
    };
    let named = |m: &delvewright_dsl::Mark| m.anchor.as_str() == anchor;
    look_at.as_ref().is_some_and(named)
        || path.iter().any(named)
        || shots.iter().any(|s| {
            s.look_at.as_ref().is_some_and(named)
                || s.path.iter().any(named)
                || matches!(&s.subject, Some(delvewright_dsl::CameraSubject::Anchor(m)) if named(m))
        })
}

/// **The effect lists that open a seam gate, each with its shot** — the one
/// answer to *does the beat that opens this way show it*, which `DW1054`
/// (`hidden`'s shot) and `DW1060` (the step the text is read against) both
/// read. A document fact: no world is needed. Lists in pointer order (ADR-0006).
pub(crate) fn opening_lists(c: &delvewright_dsl::Campaign, anchor: &str) -> Vec<OpeningList> {
    let mut lists: BTreeMap<String, Vec<(usize, &delvewright_dsl::QuestEffect)>> = BTreeMap::new();
    crate::compiler::plan::for_each_gate_effect(c, &mut |site, e| {
        if let Some((list, i)) = site.path.rsplit_once('/')
            && let Ok(i) = i.parse::<usize>()
        {
            lists.entry(list.to_string()).or_default().push((i, e));
        }
    });
    lists
        .into_iter()
        .filter(|(_, effs)| {
            effs.iter().any(
                |(_, e)| matches!(e.gate_region_write(), Some((a, false)) if a.as_str() == anchor),
            )
        })
        .map(|(list, effs)| OpeningList {
            shot: effs
                .iter()
                .filter(|(_, e)| shows(e, anchor))
                .map(|(i, _)| *i)
                .min(),
            list,
        })
        .collect()
}

/// One gated connection declared `learned: hidden`, as the text check reads
/// it: the edge, its index in the layout graph, its seam anchor and its secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HiddenWay {
    /// The connection's id.
    pub(crate) edge: String,
    /// Its index in `edges[]`.
    pub(crate) index: usize,
    /// `anchor/seam-<edge>`.
    pub(crate) seam: String,
    /// The words it keeps.
    pub(crate) secret: Vec<String>,
}

/// Every connection the world builds shut and declares `learned: hidden`.
pub(crate) fn hidden_ways(c: &delvewright_dsl::Campaign) -> Vec<HiddenWay> {
    let Some(g) = c.layout_graph.as_ref() else {
        return Vec::new();
    };
    g.content
        .edges
        .iter()
        .enumerate()
        .filter(|(_, e)| e.shuts() && e.learned() == Some(Learned::Hidden))
        .map(|(i, e)| HiddenWay {
            edge: e.id().to_string(),
            index: i,
            seam: delvewright_dsl::siteplan::seam_anchor(e.id()),
            secret: e.secret().to_vec(),
        })
        .collect()
}

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
    /// The `use` trigger that answers a press on the seam's gate, if one does
    /// ([`delvewright_dsl::QuestsContent::gate_answers`]).
    pub answered_by: Option<String>,
    /// The block the gate is shut with, as the anchor declares it — named
    /// when an eye looks through it.
    pub bar_block: String,
    /// The play spaces of the places on its low and high faces, where the seam
    /// says: what `hidden` holds the eye out of.
    pub far: Option<(Bx, Bx)>,
    /// The effect lists that open it, each with its shot ([`opening_lists`]).
    pub shots: Vec<OpeningList>,
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
    /// The eye reaches this cell of the bar through nothing that hides.
    Seen([i32; 3]),
    /// Every walk met a hiding cell first: `(region cell, blocking cell)`.
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

/// How a shut way is met (spec-0118 §5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum How {
    /// The party's reach holds a cell of the face.
    Face,
    /// No reach cell is at the face, and an eye at this reach cell on the
    /// face's side sees the gate.
    Eye([i32; 3]),
}

/// One shut way met: a connection, a configuration and a face the party is
/// at, or sees from.
#[derive(Debug, Clone)]
struct Met {
    way: usize,
    config: usize,
    high: bool,
    how: How,
    openable: Option<bool>,
    junction: [i32; 3],
    length: usize,
    rise: i32,
    seen: SeenFinding,
    reward: RewardFinding,
    cut: Cut,
}

/// Where the branch ground is cut off the open route (spec-0118 §5.2).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Cut {
    /// The junction and the branch path's own cells, one body wide.
    Path,
    /// The full declared width of a way's station the branch path walks.
    Section {
        /// The way in words.
        way: String,
        /// The station in words.
        station: String,
        /// Its declared width, in columns.
        width: usize,
    },
}

/// A line of sight from an eye to a cell that meets no hiding cell first.
#[derive(Debug, Clone, PartialEq, Eq)]
struct EyeLine {
    /// The reach cell whose eye looks.
    eye: [i32; 3],
    /// The cell it reaches.
    target: [i32; 3],
    /// What the target is, in words: the gate's block, or a place.
    what: String,
    /// Whether the target is a cell of the gate itself.
    shutter: bool,
}

/// `hidden`'s eye over one configuration the way is shut in (spec-0118 §5.3).
#[derive(Debug, Clone)]
struct HiddenFinding {
    way: usize,
    config: usize,
    /// Reach cells in the configuration.
    reach: usize,
    /// Distinct targets the eye was tried against.
    targets: usize,
    /// Sight lines walked.
    lines: usize,
    /// The first line that sees, by moves from the route then by cell.
    seen: Option<EyeLine>,
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
    /// Shut ways met: `(connection, configuration, face)` triples.
    pub met: usize,
    /// Of them, met at a face the party's reach holds.
    pub at_face: usize,
    /// Of them, met by the eye alone.
    pub by_eye: usize,
    /// Of them, triples whose connection declares `learned`.
    pub declared: usize,
    /// Of the declared, by word.
    pub seen: usize,
    /// Of the declared, by word.
    pub told: usize,
    /// Of the declared, by word.
    pub rewarded: usize,
    /// Of the declared, by word.
    pub hidden: usize,
    /// Triples whose connection declares nothing.
    pub undeclared: usize,
    /// Declarations the world does not confirm: declared triples, and a hidden
    /// way's configurations where no triple carries the refusal.
    pub unconfirmed: usize,
    /// Connections whose word is never met shut (a `hidden` one: never shut).
    pub inert: usize,
    /// Hidden ways judged.
    pub hidden_ways: usize,
    /// (Hidden way, configuration it is shut in) pairs judged.
    pub hidden_configurations: usize,
    /// Sight lines walked for the hidden ways.
    pub eye_lines: usize,
    /// The ledger rows, one per triple.
    pub rows: Vec<serde_json::Value>,
    /// The hidden ways' rows, one per configuration each is shut in.
    pub hidden_rows: Vec<serde_json::Value>,
    /// The inert connections by id.
    pub inert_ways: Vec<String>,
    /// The triples met at a face, for the agreement with `DW1011` (spec-0118 §5).
    pub(super) met_triples: BTreeSet<(usize, usize, bool)>,
    /// The triples met by the eye alone.
    pub(super) eye_triples: BTreeSet<(usize, usize, bool)>,
}

impl ShutWayBinding {
    /// The one line a build prints about this check.
    pub fn line(&self) -> String {
        format!(
            "shut ways (DW1053/DW1054): {} gated connection(s); over {} configuration(s), {} \
             shut way(s) met — {} at a face, {} by the eye alone — {} declared ({} seen, {} told, \
             {} rewarded, {} hidden), {} undeclared, {} declaration(s) not confirmed; {} declared \
             and never met shut; {} hidden way(s) judged over {} configuration(s), {} eye line(s) \
             examined",
            self.gated,
            self.configurations,
            self.met,
            self.at_face,
            self.by_eye,
            self.declared,
            self.seen,
            self.told,
            self.rewarded,
            self.hidden,
            self.undeclared,
            self.unconfirmed,
            self.inert,
            self.hidden_ways,
            self.hidden_configurations,
            self.eye_lines,
        )
    }

    /// The ledger `validation/shut-ways.json` carries.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "codes": [DW_SHUT_WAY_UNLEARNED.id(), DW_SHUT_WAY_UNCONFIRMED.id()],
            "line": self.line(),
            "gated": self.gated,
            "resolved": self.resolved,
            "configurations": self.configurations,
            "met": self.met,
            "at_face": self.at_face,
            "by_eye": self.by_eye,
            "declared": self.declared,
            "seen": self.seen,
            "told": self.told,
            "rewarded": self.rewarded,
            "hidden": self.hidden,
            "undeclared": self.undeclared,
            "unconfirmed": self.unconfirmed,
            "inert": self.inert,
            "hidden_ways": self.hidden_ways,
            "hidden_configurations": self.hidden_configurations,
            "eye_lines": self.eye_lines,
            "inert_ways": self.inert_ways,
            "rows": self.rows,
            "hidden_rows": self.hidden_rows,
        })
    }

    /// The warnings this check raises: a word on a connection never met shut
    /// (§6.3), counted and named, never refused.
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
                         and no configuration the critical path passes through meets it shut — \
                         a `hidden` way: holds it shut at all — so the word is confirmed by \
                         nothing. The judged configurations are the critical path's; a way met \
                         shut only on a branch's own path is not in them. The word is kept and \
                         counted; nothing is refused."
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

/// The play space of the place `node` as the plan placed it, if it did.
fn space_of(plan: &Plan, node: &str) -> Option<Bx> {
    let b = plan.blockout.as_ref()?;
    let pb = b.boxes.iter().find(|x| x.node.0 == node)?;
    let (lo, hi) = pb.space();
    let n = |v: i64| i32::try_from(v).ok();
    Some((
        [n(lo[0])?, n(lo[1])?, n(lo[2])?],
        [n(hi[0])?, n(hi[1])?, n(hi[2])?],
    ))
}

/// `DW1053` and `DW1054` over a campaign's gated connections, judged once per
/// quest configuration the critical path passes through. Returns the binding
/// beside the verdict so the caller can print it whichever way it went.
pub fn check_shut_ways_learned(
    plan: &Plan,
    world: &World,
    prefabs: &crate::compiler::registry::PrefabRegistry,
) -> (ShutWayBinding, Result<(), Failure>) {
    let gated = gated_connections(plan);
    let q = &plan.campaign.quests.content;
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
        // The press answer is the predicate `DW0429` reads (spec-0117 §6).
        // A door-shut seam's key is pressed on the seam's own door, and its
        // `not_yet` is the shut way's answer (spec-0124 §5.1, the press).
        let answered_by = q
            .gate_answers(&seal.anchor)
            .next()
            .map(|t| t.id.to_string())
            .or_else(|| seam_key_answer(plan, &seal.anchor));
        let far = sides
            .as_ref()
            .and_then(|(lo, hi)| Some((space_of(plan, lo)?, space_of(plan, hi)?)));
        let bar_block = crate::compiler::plan::gate_region_block_any(&plan.anchors, &seal.anchor)
            .map(|(_, _, b)| b)
            .unwrap_or_default();
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
            bar_block,
            far,
            shots: opening_lists(plan.campaign, &seal.anchor),
        });
    }
    let configurations = Configurations::of(plan, world);
    let regions: Vec<([i32; 3], [i32; 3])> = ways.iter().map(|w| w.gate.region).collect();
    let judged = configurations.judged(plan, world, &regions);
    let sections = super::width::plan_cross_sections(plan, prefabs);
    let (mut binding, verdict) = verify_shut_ways(&ways, world, &judged, &rewards(plan), &sections);
    binding.gated = gated.len();
    // The text a hidden way keeps is DW1060's, judged at validation over every
    // language; the ledger carries its English reading beside the eye.
    let text = crate::compiler::telling::secret_verdicts(plan.campaign, &BTreeMap::new());
    for row in &mut binding.hidden_rows {
        if let Some(edge) = row["connection"].as_str()
            && let Some(v) = text.get(edge)
        {
            row["text"] = v.clone();
        }
    }
    (binding, verdict)
}

/// The tree of the reach graph's moves from the route cells, over the party's
/// closure `graph` ([`super::shutmodel::party_reach`]): each cell's
/// predecessor, and every cell in order from the route, ties broken by the
/// cell (ADR-0006). It walks the closure's own edges ([`Reach::succ`], the
/// open sea joined to itself as [`Reach::next`] joins it) and holds exactly
/// its cells: the order the eye is asked in, never a second answer to where a
/// body gets to.
///
/// A path takes the fewest leaps first — any edge but a step of the graph's
/// cell relation (an edge that is no flown launch: the walk, the climb, the
/// wade and the straight drop) — and of
/// those the fewest moves, as `DW1011`'s witness does: a branch is walked
/// cell by cell where it can be, so its path is one body wide and its junction
/// is where a body walking off the route leaves it, and a jump is on the path
/// only where the walk cannot go.
pub(super) fn route_tree(graph: &Reach, route: &[[i32; 3]]) -> (Tree, Vec<[i32; 3]>) {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let mut pred: Tree = BTreeMap::new();
    let mut best: BTreeMap<[i32; 3], (u32, u32)> = BTreeMap::new();
    let mut heap: BinaryHeap<Reverse<(u32, u32, [i32; 3])>> = BinaryHeap::new();
    for c in route {
        if best.insert(*c, (0, 0)).is_none() {
            pred.insert(*c, None);
            heap.push(Reverse((0, 0, *c)));
        }
    }
    while let Some(Reverse((leaps, moves, cur))) = heap.pop() {
        if best.get(&cur).is_some_and(|b| *b < (leaps, moves)) {
            continue;
        }
        let edges = graph
            .succ
            .get(&cur)
            .into_iter()
            .flatten()
            .map(|e| (e.to, e.yaw.is_none()));
        let sea = graph
            .open_sea
            .contains(&cur)
            .then(|| graph.open_sea.iter().map(|&c| (c, false)))
            .into_iter()
            .flatten();
        for (n, step) in edges.chain(sea) {
            if n == cur || !graph.states.contains_key(&n) {
                continue;
            }
            let cost = (leaps + u32::from(!step), moves + 1);
            if best.get(&n).is_none_or(|b| cost < *b) {
                best.insert(n, cost);
                pred.insert(n, Some(cur));
                heap.push(Reverse((cost.0, cost.1, n)));
            }
        }
    }
    let mut order: Vec<[i32; 3]> = best.keys().copied().collect();
    order.sort_by_key(|c| (best[c], *c));
    (pred, order)
}

/// The path from the route to `target` over the tree, route end first.
fn path_to(tree: &Tree, target: [i32; 3]) -> Vec<[i32; 3]> {
    let mut path = vec![target];
    let mut at = target;
    while let Some(Some(p)) = tree.get(&at) {
        path.push(*p);
        at = *p;
    }
    path.reverse();
    path
}

/// **The branch ground and where it is cut** (spec-0118 §5.2): the ground
/// behind the narrowest cross-section the branch path passes that separates
/// it from the open route. The path's own cells are one body wide, the
/// narrowest a path can be, and are tried first; where the open side flows
/// round them, the stations of declared ways the path walks
/// ([`super::width::CrossSection`], the width model of spec-0120) are tried
/// narrowest first, each cut across its full declared width, the one nearer
/// the face first among equals. The first that leaves ground is the cut;
/// where none does, the ground is empty and the cut is the path's.
fn cut_branch(
    w: &World,
    face: &[[i32; 3]],
    route: &BTreeSet<[i32; 3]>,
    junction: [i32; 3],
    branch: &[[i32; 3]],
    sections: &[super::width::CrossSection],
) -> (BTreeSet<[i32; 3]>, Cut) {
    let path: Vec<[i32; 3]> = std::iter::once(junction)
        .chain(branch.iter().copied())
        .collect();
    let ground = branch_ground(w, face, route, &path, &BTreeSet::new());
    if !ground.is_empty() {
        return (ground, Cut::Path);
    }
    let mut walked: Vec<(usize, std::cmp::Reverse<usize>, &super::width::CrossSection)> = sections
        .iter()
        .filter_map(|s| {
            path.iter()
                .rposition(|c| s.cells.contains(c))
                .map(|i| (s.width, std::cmp::Reverse(i), s))
        })
        .collect();
    walked.sort_by(|a, b| {
        (a.0, a.1, &a.2.way, &a.2.station).cmp(&(b.0, b.1, &b.2.way, &b.2.station))
    });
    for (_, _, s) in walked {
        let ground = branch_ground(w, face, route, &path, &s.cells);
        if !ground.is_empty() {
            return (
                ground,
                Cut::Section {
                    way: s.way.clone(),
                    station: s.station.clone(),
                    width: s.width,
                },
            );
        }
    }
    (BTreeSet::new(), Cut::Path)
}

/// **The ground behind one cut**: the cells a body reaches from the shut
/// way's face that it cannot reach from the open route without passing the
/// cut — the junction, the branch path, and the cross-section `across` when
/// one is given. The open side is the reach graph's closure of the route cells
/// ([`World::body_flood`]) over the world with the cut walled up
/// ([`World::walls_over`], the one rule a walk over the graph takes floor out
/// of a body's reach by); the ground is the unconfined closure
/// from the face, less that open side, less every cell beside it, and less
/// the route itself. A cut cell is beside the open side when one move of the
/// graph ([`World::body_edges`]) lands there; any other cell when one step of
/// the graph's cell relation (an edge that is no flown launch) does. On
/// an open floor the open side flows round the cut, so the ground is empty:
/// an open floor is not a branch, and nothing on it pays for one.
fn branch_ground(
    w: &World,
    face: &[[i32; 3]],
    route: &BTreeSet<[i32; 3]>,
    path: &[[i32; 3]],
    across: &BTreeSet<[i32; 3]>,
) -> BTreeSet<[i32; 3]> {
    let cut: BTreeSet<[i32; 3]> = path.iter().chain(across).copied().collect();
    // The cut is a wall ([`World::walls_over`]): a jump that lands past it
    // flies through its cells without standing in them, so a cut made by
    // leaving cells out of the walk is crossed in the air.
    let walls = World::walls_over(&cut, &|p| !w.solid_at(p));
    let tops = World::wall_tops(&walls);
    let walled = w.with_sealed(&walls);
    let roots: Vec<[i32; 3]> = route.iter().copied().filter(|c| !cut.contains(c)).collect();
    let open = walled.body_flood(&roots, &Confinement::cells(&|c| !tops.contains(&c)));
    // A cell beside the open side is open ground too: on an open floor the
    // cut's own cells, taken out of the flood, stand beside it — a cut cell
    // from which one move of the graph, a leap included, lands on the open
    // side. Any other cell is beside it when one step of the graph's cell
    // relation (an edge that is no flown launch) takes it there: a leap off
    // the far side of a real cut lands on the open side too, and the ground
    // behind the cut is not open for that.
    let beside = |c: [i32; 3]| {
        let leaps = cut.contains(&c);
        w.body_edges(c)
            .iter()
            .any(|e| (leaps || e.yaw.is_none()) && open.contains(&e.to))
    };
    w.body_flood(face, &Confinement::none())
        .into_iter()
        .filter(|c| !open.contains(c) && !route.contains(c) && !beside(*c))
        .collect()
}

/// The player's eye standing in `e`: the cell's centre raised by `eye_height`.
fn eye_of(e: [i32; 3], eye_height: f64) -> [f64; 3] {
    let c = cell_center(e);
    [c[0], c[1] + eye_height, c[2]]
}

/// Whether the player's eye at `eye` sees the cell `target`: the one sight
/// rule ([`crate::compiler::framing`]'s `in_sight`, spec-0110 §5.5) asked of
/// the cell's box — nine lines, its centre and eight inset corners, stopped
/// only by a cell that hides what is behind it ([`World::hides`], the pinned
/// occlusion table; `docs/reference/sight-through-blocks.md` §2). `None` when
/// some line is clear; otherwise the first hiding cell on the centre line.
/// Every sight question of this module asks it, and it walks no line of its
/// own.
pub(crate) fn eye_meets(w: &World, eye: [f64; 3], target: [i32; 3]) -> Option<[i32; 3]> {
    use crate::compiler::framing::{CellBox, in_sight};
    let lines = in_sight(w, eye, &CellBox::cell(target), &[]);
    if lines.seen() {
        None
    } else {
        lines.centre_block
    }
}

/// From the player's eye at `j`, whether some cell of `region` is seen
/// (spec-0118 §5.3, `seen`): reached through nothing that hides. A bar seen
/// through a grille, a fence or a pane is seen.
fn sight(w: &World, j: [i32; 3], region: Bx, eye_height: f64) -> SeenFinding {
    let eye = eye_of(j, eye_height);
    let (a, b) = region;
    let mut hidden: Vec<([i32; 3], [i32; 3])> = Vec::new();
    for r in crate::compiler::assembled::region_cells(a, b) {
        match eye_meets(w, eye, r) {
            None => return SeenFinding::Seen(r),
            Some(hit) => hidden.push((r, hit)),
        }
    }
    SeenFinding::Hidden(hidden)
}

/// Which side of a gate's thin axis a cell stands on: `Some(false)` low,
/// `Some(true)` high, `None` in the gate's own slab.
fn side_of(c: [i32; 3], region: Bx, axis: usize) -> Option<bool> {
    let (a, b) = region;
    if c[axis] < a[axis].min(b[axis]) {
        Some(false)
    } else if c[axis] > a[axis].max(b[axis]) {
        Some(true)
    } else {
        None
    }
}

/// **The eye's population** (spec-0118 §5.1): the nearest reach cell, by moves
/// from the route then by cell, on the `high` side of the gate's axis whose
/// eye sees a cell of the gate region that does not itself hide — a grille, a
/// pane, a door, a trapdoor, which the eye looks through; `None` when no eye
/// there does. A shutter that hides reads as the wall it stands in.
fn seen_from_side(
    w: &World,
    order: &[[i32; 3]],
    region: Bx,
    axis: usize,
    high: bool,
    eye_height: f64,
) -> Option<[i32; 3]> {
    // A gate cell that hides is a wall the player sees, not a way: looking at
    // it is not looking through it (spec-0118 criterion 3).
    let cells: Vec<[i32; 3]> = crate::compiler::assembled::region_cells(region.0, region.1)
        .filter(|c| !w.hides(*c))
        .collect();
    order.iter().copied().find(|e| {
        side_of(*e, region, axis) == Some(high)
            && cells
                .iter()
                .any(|r| eye_meets(w, eye_of(*e, eye_height), *r).is_none())
    })
}

/// **`hidden`'s eye in one configuration** (spec-0118 §5.3 a): for every reach
/// cell, in order of moves from the route, every target — the gate's cells
/// that do not themselves hide, and every cell a body can be in inside the
/// place on the far side of the gate from that eye — and the first line that
/// meets no hiding cell before its target.
///
/// **The far side from an eye** (authored): an eye in one of the way's two
/// places looks for the other. An eye in a third place looks for the side the
/// party does not reach in this configuration — the place behind the way —
/// and, where the reach holds both sides or neither, for the side across the
/// gate's axis from it, both sides for an eye in the gate's own slab.
fn judge_hidden(
    w: &World,
    way: &ShutWay,
    axis: usize,
    order: &[[i32; 3]],
    eye_height: f64,
) -> (usize, usize, Option<EyeLine>) {
    let region = way.gate.region;
    let gate: Vec<[i32; 3]> = crate::compiler::assembled::region_cells(region.0, region.1)
        .filter(|c| !w.hides(*c))
        .collect();
    let body = |b: Option<Bx>| -> Vec<[i32; 3]> {
        let Some((lo, hi)) = b else {
            return Vec::new();
        };
        crate::compiler::assembled::region_cells(lo, hi)
            .filter(|c| super::shutmodel::can_be(w, *c))
            .collect()
    };
    let (low_box, high_box) = (way.far.map(|f| f.0), way.far.map(|f| f.1));
    let low_cells = body(low_box);
    let high_cells = body(high_box);
    let place = |high: bool| -> String {
        match &way.sides {
            Some((lo, hi)) => format!(
                "a cell a body can be in in `{}`",
                if high { hi } else { lo }
            ),
            None => "a cell a body can be in beyond it".to_string(),
        }
    };
    let targets = gate.len() + low_cells.len() + high_cells.len();
    let inside = |b: Option<Bx>, c: &[i32; 3]| {
        b.is_some_and(|(lo, hi)| (0..3).all(|i| lo[i] <= c[i] && c[i] <= hi[i]))
    };
    let reached_low = order.iter().any(|c| inside(low_box, c));
    let reached_high = order.iter().any(|c| inside(high_box, c));
    let mut lines = 0usize;
    for e in order {
        let eye = eye_of(*e, eye_height);
        // `Some(true)`: the eye looks for the high side's place.
        let looks_high: Option<bool> = if inside(low_box, e) {
            Some(true)
        } else if inside(high_box, e) {
            Some(false)
        } else if reached_low != reached_high {
            Some(reached_low)
        } else {
            side_of(*e, region, axis).map(|high| !high)
        };
        let far: Vec<(&[i32; 3], bool)> = match looks_high {
            Some(true) => high_cells.iter().map(|c| (c, true)).collect(),
            Some(false) => low_cells.iter().map(|c| (c, false)).collect(),
            None => low_cells
                .iter()
                .map(|c| (c, false))
                .chain(high_cells.iter().map(|c| (c, true)))
                .collect(),
        };
        for t in &gate {
            lines += 1;
            if eye_meets(w, eye, *t).is_none() {
                return (
                    targets,
                    lines,
                    Some(EyeLine {
                        eye: *e,
                        target: *t,
                        what: format!("`{}`", way.bar_block),
                        shutter: true,
                    }),
                );
            }
        }
        for (t, high) in far {
            lines += 1;
            if eye_meets(w, eye, *t).is_none() {
                return (
                    targets,
                    lines,
                    Some(EyeLine {
                        eye: *e,
                        target: *t,
                        what: place(high),
                        shutter: false,
                    }),
                );
            }
        }
    }
    (targets, lines, None)
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

/// Whether the beat that opens a way shows it: some list opens it, and every
/// list that opens it carries the shot.
fn shot_ok(way: &ShutWay) -> bool {
    !way.shots.is_empty() && way.shots.iter().all(|s| s.shot.is_some())
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
    sections: &[super::width::CrossSection],
) -> (ShutWayBinding, Result<(), Failure>) {
    verify_with_eye(
        ways,
        base,
        configs,
        rewards,
        sections,
        delvewright_dsl::metrics::PLAYER_EYE_HEIGHT,
    )
}

/// What one configuration yields: the shut ways met in it, and the hidden
/// eye over each way shut in it that owes one.
/// A configuration whose reach graph passed its budget yields its closure
/// and nothing else: a partial closure takes no verdict (spec-0113 §5.3).
type ConfigFindings = (Vec<Met>, Vec<HiddenFinding>, Option<u64>, Reach);

#[allow(clippy::too_many_lines)]
fn verify_with_eye(
    ways: &[ShutWay],
    base: &World,
    configs: &[SeparationConfig<'_>],
    rewards: &[Reward],
    sections: &[super::width::CrossSection],
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
    let per_config: Vec<ConfigFindings> = crate::par::map(
        &configs.iter().enumerate().collect::<Vec<_>>(),
        |(ci, (w, _, route, shut))| {
            let (seeds, graph) = party_reach(w, route);
            if let Some(budget) = graph.over_budget {
                return (Vec::new(), Vec::new(), Some(budget), graph);
            }
            let reach = graph.cells();
            let route_set: BTreeSet<[i32; 3]> = seeds.iter().copied().collect();
            let (tree, order) = route_tree(&graph, &seeds);
            let mut out: Vec<Met> = Vec::new();
            let mut hidden: Vec<HiddenFinding> = Vec::new();
            for &gi in shut {
                let Some(f) = faces[gi].as_ref() else {
                    continue;
                };
                let region = ways[gi].gate.region;
                // A hidden way owes its eye in every configuration it is shut
                // in, met or not; an undeclared one is judged for its message.
                if matches!(ways[gi].learned, Some(Learned::Hidden) | None) {
                    let (targets, lines, seen) =
                        judge_hidden(w, &ways[gi], f.axis, &order, eye_height);
                    hidden.push(HiddenFinding {
                        way: gi,
                        config: *ci,
                        reach: reach.len(),
                        targets,
                        lines,
                        seen,
                    });
                }
                let m = meet(w, f, &reach);
                let held = m.met();
                for high in [false, true] {
                    let (how, target, face): (How, [i32; 3], Vec<[i32; 3]>) =
                        if held.contains(&high) {
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
                                .min_by_key(|c| (path_to(&tree, *c).len(), *c))
                                .expect("a met face holds a reached cell");
                            (How::Face, target, face)
                        } else if let Some(e) =
                            seen_from_side(w, &order, region, f.axis, high, eye_height)
                        {
                            (How::Eye(e), e, Vec::new())
                        } else {
                            continue;
                        };
                    let path = path_to(&tree, target);
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
                    // A way met by the eye alone pays for no walk: its ground
                    // is empty.
                    let (ground, cut) = if how == How::Face {
                        cut_branch(w, &face, &route_set, junction, branch, sections)
                    } else {
                        (BTreeSet::new(), Cut::Path)
                    };
                    out.push(Met {
                        way: gi,
                        config: *ci,
                        high,
                        how,
                        openable: opens[gi].map(|low_opens| low_opens != high),
                        junction,
                        length: branch.len(),
                        rise,
                        seen: sight(w, junction, region, eye_height),
                        reward: reward_on(&ground, rewards),
                        cut,
                    });
                }
            }
            (out, hidden, None, graph)
        },
    );
    let mut met: Vec<Met> = Vec::new();
    let mut hidden: Vec<HiddenFinding> = Vec::new();
    let mut over_budget: Option<u64> = None;
    let mut graphs = ReachBinding::default();
    for (ci, (m, h, over, graph)) in per_config.into_iter().enumerate() {
        over_budget = over_budget.or(over);
        graphs.add(configs[ci].0, &graph);
        met.extend(m);
        hidden.extend(h);
    }
    if let Some(budget) = over_budget {
        return (binding, Err(reach_budget_failure(&graphs, budget)));
    }
    // A hidden way is confirmed when no configuration it is shut in has an
    // eye that sees it, and the beat that opens it shows it.
    let hidden_seen: BTreeMap<usize, &HiddenFinding> = {
        let mut first: BTreeMap<usize, &HiddenFinding> = BTreeMap::new();
        for h in &hidden {
            if h.seen.is_some() {
                first.entry(h.way).or_insert(h);
            }
        }
        first
    };
    let hidden_ok = |gi: usize| !hidden_seen.contains_key(&gi) && shot_ok(&ways[gi]);
    let mut met_ways: BTreeSet<usize> = BTreeSet::new();
    // The first failing triple per connection, in (configuration, face) order.
    let mut unlearned: BTreeMap<usize, &Met> = BTreeMap::new();
    let mut unconfirmed: BTreeMap<usize, (Option<&Met>, usize)> = BTreeMap::new();
    for m in &met {
        let way = &ways[m.way];
        met_ways.insert(m.way);
        match m.how {
            How::Face => {
                binding.met_triples.insert((m.way, m.config, m.high));
                binding.at_face += 1;
            }
            How::Eye(_) => {
                binding.eye_triples.insert((m.way, m.config, m.high));
                binding.by_eye += 1;
            }
        }
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
                    Learned::Hidden => {
                        binding.hidden += 1;
                        hidden_ok(m.way)
                    }
                };
                if !ok {
                    binding.unconfirmed += 1;
                    let e = unconfirmed.entry(m.way).or_insert((Some(m), 0));
                    e.1 += 1;
                }
                Some(ok)
            }
        };
        binding.rows.push(row(way, m, configs, confirmed));
    }
    // The hidden ways: every configuration each is shut in, met or not.
    let mut shut_hidden: BTreeSet<usize> = BTreeSet::new();
    for h in hidden
        .iter()
        .filter(|h| ways[h.way].learned == Some(Learned::Hidden))
    {
        shut_hidden.insert(h.way);
        binding.hidden_configurations += 1;
        binding.eye_lines += h.lines;
        if !met_ways.contains(&h.way) && (h.seen.is_some() || !shot_ok(&ways[h.way])) {
            binding.unconfirmed += 1;
            unconfirmed.entry(h.way).or_insert((None, 0)).1 += 1;
        }
        binding
            .hidden_rows
            .push(hidden_row(&ways[h.way], h, configs));
    }
    binding.hidden_ways = shut_hidden.len();
    for (i, w) in ways.iter().enumerate() {
        let inert = match w.learned {
            Some(Learned::Hidden) => !shut_hidden.contains(&i),
            Some(_) => !met_ways.contains(&i),
            None => false,
        };
        if inert {
            binding.inert += 1;
            binding.inert_ways.push(w.edge.clone());
        }
    }
    if !unlearned.is_empty() {
        let named: Vec<String> = unlearned
            .values()
            .take(LIST_LIMIT)
            .map(|m| {
                let h = hidden
                    .iter()
                    .find(|h| h.way == m.way && h.config == m.config);
                unlearned_words(&ways[m.way], m, configs, h)
            })
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
                     from where they turned off, are told at its face, or are paid for the walk; \
                     a player who sees a shut way and is never told of it has been shown a secret \
                     nobody kept. Write the word that is true in the connection's \
                     `gating.learned` — `seen`, `told`, `rewarded` or `hidden` — or make one \
                     true and write it.",
                    unlearned.len(),
                    named.join("; ")
                ),
            }),
        );
    }
    if !unconfirmed.is_empty() {
        let named: Vec<String> = unconfirmed
            .iter()
            .take(LIST_LIMIT)
            .map(|(gi, (m, n))| {
                let way = &ways[*gi];
                let mut s = if way.learned == Some(Learned::Hidden) {
                    hidden_unconfirmed_words(way, hidden_seen.get(gi).copied(), configs)
                } else {
                    m.map_or_else(String::new, |m| unconfirmed_words(way, m, configs))
                };
                if *n > 1 {
                    s.push_str(&format!(" (not confirmed {n} time(s))"));
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
                     geometry so the bar is seen, write the trigger, place the reward, make the \
                     shutter hide, wall the line of sight, add the shot to the opening beat — or \
                     change the word to the one that is true.",
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

/// The key that answers a press on a door-shut seam while it is shut: an
/// `interact` anchored on the seam whose `answers.not_yet` the seam's door
/// carries (spec-0124 §5.1, §5.2). Read off the build's one answer derivation.
fn seam_key_answer(plan: &Plan, seam: &str) -> Option<String> {
    plan.answers().iter().find_map(|a| match &a.owner {
        crate::compiler::pressable::Owner::Interact { id, .. }
            if a.state == delvewright_dsl::AnswerState::NotYet && a.at.as_deref() == Some(seam) =>
        {
            Some(id.clone())
        }
        _ => None,
    })
}

fn told_words(way: &ShutWay) -> String {
    match &way.answered_by {
        Some(t) => format!("`{t}` answers a press on `{}`", way.seam),
        None => format!("no `use` trigger serves `{}`", way.seam),
    }
}

fn reward_words(m: &Met) -> String {
    match &m.reward {
        RewardFinding::Rewarded(a, kind) => format!("`{a}` ({kind}) stands on the branch"),
        RewardFinding::Bare(n, nearest) => {
            let mut s = format!(
                "no pickup, interaction or fight within the branch's {n} cell(s) behind {}",
                cut_words(&m.cut)
            );
            if let Some((a, d)) = nearest {
                s.push_str(&format!(" (the nearest is `{a}`, {d} block(s) from it)"));
            }
            s
        }
    }
}

/// The eye line in words: where it stands, what it reaches, and why that
/// target is not hidden.
fn eye_words(l: &EyeLine) -> String {
    let mut s = format!(
        "the way is seen from {:?}: the eye reaches {:?} ({}) through nothing that hides",
        l.eye, l.target, l.what
    );
    if l.shutter {
        s.push_str(&format!(
            " — {} is looked through; a shutter that hides is a block the pinned game draws as a \
             full opaque cube",
            l.what
        ));
    }
    s
}

/// The shot in words: the effect lists that open the way and whether each
/// shows its seam.
fn shot_words(way: &ShutWay) -> String {
    if way.shots.is_empty() {
        return format!(
            "no effect list opens `{}` (an `open-gate` of it), so no beat can show it",
            way.seam
        );
    }
    let missing: Vec<String> = way
        .shots
        .iter()
        .filter(|s| s.shot.is_none())
        .map(|s| format!("`{}`", s.list))
        .collect();
    if missing.is_empty() {
        format!("the opening beat carries a shot of `{}`", way.seam)
    } else {
        format!(
            "the beat that opens it carries no shot of `{}`: {} open(s) it with no `cutscene` \
             whose `look_at` or `path` is `{}`",
            way.seam,
            missing.join(", "),
            way.seam
        )
    }
}

fn hidden_words(way: &ShutWay, h: Option<&HiddenFinding>) -> String {
    let eye = match h.and_then(|h| h.seen.as_ref()) {
        Some(l) => format!("{}, so it cannot be hidden as built", eye_words(l)),
        None => "nothing in the reach sees it".to_string(),
    };
    format!("{eye}; {}", shot_words(way))
}

/// The cut in words: where the branch ground is cut off the open route.
fn cut_words(cut: &Cut) -> String {
    match cut {
        Cut::Path => "the branch path's own cells, one body wide".to_string(),
        Cut::Section {
            way,
            station,
            width,
        } => format!("{station} of {way}, {width} column(s) wide"),
    }
}

fn cut_json(cut: &Cut) -> serde_json::Value {
    match cut {
        Cut::Path => serde_json::json!({ "at": "path", "width": 1 }),
        Cut::Section {
            way,
            station,
            width,
        } => serde_json::json!({ "at": station, "way": way, "width": width }),
    }
}

fn place_words(way: &ShutWay, m: &Met, configs: &[SeparationConfig<'_>]) -> String {
    let met = match m.how {
        How::Face => format!("the party meets it at {}", face_words(way, m)),
        How::Eye(e) => format!(
            "the party sees it from {e:?}, on {}, and stands at no face of it",
            face_words(way, m)
        ),
    };
    format!(
        "{} is shut {} and {met}; the branch leaves the route at {:?}, {} move(s) long, \
         rising {}",
        way.words,
        when(configs, m.config),
        m.junction,
        m.length,
        m.rise
    )
}

fn unlearned_words(
    way: &ShutWay,
    m: &Met,
    configs: &[SeparationConfig<'_>],
    h: Option<&HiddenFinding>,
) -> String {
    format!(
        "{}. `seen` — {}; `told` — {}; `rewarded` — {}; `hidden` — {}",
        place_words(way, m, configs),
        seen_words(m),
        told_words(way),
        reward_words(m),
        hidden_words(way, h)
    )
}

fn unconfirmed_words(way: &ShutWay, m: &Met, configs: &[SeparationConfig<'_>]) -> String {
    let place = place_words(way, m, configs);
    match way.learned {
        Some(Learned::Seen) => format!(
            "{place}; declared `seen`, but from {:?} the eye meets a cell that hides before the \
             bar: {}",
            m.junction,
            seen_words(m)
        ),
        Some(Learned::Told) => format!(
            "{place}; declared `told`, but no `use` trigger serves `{}`. Anchor a trigger on \
             the seam — {} — whose body is the gate's own block; or, where no anchor names a cell \
             of the gate, a knocker (a bell or a button with its `prop`, and its `answers.done`) \
             face-adjacent to it",
            way.seam,
            delvewright_dsl::trigger::press_answer_prescription(&way.seam, "the way")
        ),
        Some(Learned::Rewarded) => format!("{place}; declared `rewarded`, but {}", reward_words(m)),
        Some(Learned::Hidden) | None => place,
    }
}

fn hidden_unconfirmed_words(
    way: &ShutWay,
    seen: Option<&HiddenFinding>,
    configs: &[SeparationConfig<'_>],
) -> String {
    let mut fails: Vec<String> = Vec::new();
    if let Some((h, l)) = seen.and_then(|h| h.seen.as_ref().map(|l| (h, l))) {
        fails.push(format!("{}: {}", when(configs, h.config), eye_words(l)));
    }
    if !shot_ok(way) {
        fails.push(shot_words(way));
    }
    format!(
        "{} is declared `hidden`, but {}",
        way.words,
        fails.join("; and ")
    )
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
    let met = match m.how {
        How::Face => serde_json::json!("at a face"),
        How::Eye(e) => serde_json::json!({ "by_the_eye_from": e }),
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
        "met": met,
        "junction": m.junction,
        "branch_length": m.length,
        "branch_rise": m.rise,
        "branch_cut": cut_json(&m.cut),
        "learned": way.learned.map(Learned::word),
        "confirmed": confirmed,
        "seen": seen,
        "told": told,
        "rewarded": reward,
    })
}

fn hidden_row(
    way: &ShutWay,
    h: &HiddenFinding,
    configs: &[SeparationConfig<'_>],
) -> serde_json::Value {
    let eye = match &h.seen {
        Some(l) => serde_json::json!({
            "seen_from": l.eye,
            "reaches": l.target,
            "what": l.what,
        }),
        None => serde_json::json!("hidden"),
    };
    let shots: Vec<serde_json::Value> = way
        .shots
        .iter()
        .map(|s| serde_json::json!({ "list": s.list, "shot": s.shot_pointer() }))
        .collect();
    serde_json::json!({
        "connection": way.edge,
        "configuration": when(configs, h.config),
        "reach": h.reach,
        "targets": h.targets,
        "eye_lines": h.lines,
        "eye": eye,
        "shots": shots,
        "text": null,
    })
}

/// `DW1053` and `DW1054` over synthetic worlds, and the agreement with
/// `DW1011` over the one shut-gate model (spec-0118 acceptance 2, 4–8).
#[cfg(test)]
mod learned_tests {
    use super::super::separate::verify_gates_separate;
    use super::super::width::CrossSection;
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
            bar_block: "minecraft:stone".to_string(),
            far: None,
            shots: Vec::new(),
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
            &[],
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
    /// not the mob's 1.5. A one-high bar ten cells down the branch, and
    /// blockers across the branch that stop every one of the nine lines from
    /// the 1.5 eye and leave one from the 1.62 eye clear — found by asking the
    /// check itself at both heights over rows of blockers, never placed by
    /// hand.
    #[test]
    fn the_eye_is_the_players_not_the_mobs() {
        let region = ([5, 1, 10], [5, 1, 10]);
        let ways = [way(region, 2, Some(Learned::Seen))];
        let rows: Vec<Vec<[i32; 3]>> = (2..=9)
            .flat_map(|z| (1..=2).map(move |y| (4..=6).map(|x| [x, y, z]).collect()))
            .collect();
        let mut found = None;
        'search: for (i, a) in rows.iter().enumerate() {
            for b in rows.iter().skip(i) {
                let blockers: Vec<[i32; 3]> = a.iter().chain(b.iter()).copied().collect();
                let (base, shut) = (
                    branch(9, 1, false, &blockers),
                    branch(9, 1, true, &blockers),
                );
                let (_, at_player) = judge(&shut, &base, &ways, &[], PLAYER_EYE);
                let (_, at_mob) = judge(&shut, &base, &ways, &[], 1.5);
                if let (Ok(()), Err(mob)) = (&at_player, &at_mob) {
                    assert_eq!(mob.code.id(), "DW1054");
                    found = Some((base, shut));
                    break 'search;
                }
            }
        }
        let (base, shut) = found.expect("some blockers stop the 1.5 eye and not the 1.62 one");
        // The core every caller reaches is the player's eye.
        let (_, core) = verify_shut_ways(
            &ways,
            &base,
            &[(&shut, "now".to_string(), route(), vec![0])],
            &[],
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
            .replace("trigger/<name>", "trigger/seam-answer")
            .replace("<the door cell's anchor>", "anchor/seam-test-leaf");
        let trigger: delvewright_dsl::EnvTrigger =
            serde_json::from_str(&json).expect("the prescription is a trigger");
        let mut q: delvewright_dsl::QuestsContent =
            serde_json::from_value(serde_json::json!({ "quests": [] })).expect("an empty mission");
        q.triggers.push(trigger);
        assert_eq!(q.gate_answers("anchor/seam-test").count(), 1);
        assert_eq!(
            json,
            delvewright_dsl::trigger::press_answer_prescription("anchor/seam-test", "the way")
                .trim_matches('`')
                .replace("trigger/<name>", "trigger/seam-answer")
                .replace("<the door cell's anchor>", "anchor/seam-test-leaf")
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
        let (b, verdict) = verify_shut_ways(&ways, &base, &two, &[], &[]);
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
            "`told` — no `use` trigger serves",
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
        let (b, verdict) = verify_shut_ways(&ways, &walled_base, &one, &[], &[]);
        assert!(verdict.is_ok(), "{verdict:?}");
        assert_eq!((b.met, b.undeclared, b.inert), (0, 0, 0), "{}", b.line());

        let declared = [way(region, 2, Some(Learned::Seen))];
        let (b, verdict) = verify_shut_ways(&declared, &walled_base, &one, &[], &[]);
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
            &[],
        );
        let err = verdict.expect_err("a chest in an open hall is no branch's pay");
        assert_eq!(err.code.id(), "DW1054", "{}", err.message);
        assert_eq!(b.rows[0]["rewarded"]["ground_cells"], 0, "{:?}", b.rows);
    }

    /// A declared way's station as the width model hands it: `width` columns
    /// along x from `x0` at `z`, the body cell at `y` and the one over it.
    fn section(station: &str, x0: i32, width: i32, y: i32, z: i32) -> CrossSection {
        CrossSection {
            way: "the stair `edge/test-stair`".to_string(),
            station: station.to_string(),
            width: usize::try_from(width).unwrap(),
            cells: (x0..x0 + width)
                .flat_map(|x| [[x, y, z], [x, y + 1, z]])
                .collect(),
        }
    }

    fn judge_cut(
        w: &World,
        base: &World,
        ways: &[ShutWay],
        route: Vec<[i32; 3]>,
        rewards: &[Reward],
        sections: &[CrossSection],
    ) -> (ShutWayBinding, Result<(), Failure>) {
        verify_shut_ways(
            ways,
            base,
            &[(w, "now".to_string(), route, vec![0])],
            rewards,
            sections,
        )
    }

    /// spec-0118 §5.2: the one-cell choke still cuts. The branch leaves the
    /// route by a mouth one wide; the path's own cells separate the pocket,
    /// so the cut is the path's even with a three-wide declared station on
    /// the branch, and the ground is the one the path alone cut.
    #[test]
    fn the_old_one_cell_choke_still_cuts() {
        let region = ([5, 1, 8], [5, 2, 8]);
        let (base, shut) = (branch(7, 2, false, &[]), branch(7, 2, true, &[]));
        let ways = [way(region, 2, Some(Learned::Rewarded))];
        let chest = Reward {
            anchor: "anchor/chest".to_string(),
            kind: "a loot holder",
            pos: [6, 1, 6],
        };
        let wide = [section("the branch's middle", 4, 3, 1, 4)];
        let (alone, verdict) = judge_cut(
            &shut,
            &base,
            &ways,
            route(),
            std::slice::from_ref(&chest),
            &[],
        );
        assert!(verdict.is_ok(), "{verdict:?}");
        let (b, verdict) = judge_cut(
            &shut,
            &base,
            &ways,
            route(),
            std::slice::from_ref(&chest),
            &wide,
        );
        assert!(verdict.is_ok(), "{verdict:?}");
        assert_eq!(b.rewarded, 1);
        assert_eq!(
            b.rows[0]["branch_cut"],
            serde_json::json!({ "at": "path", "width": 1 })
        );
        assert_eq!(alone.rows, b.rows, "a wider station moves nothing");
    }

    /// A yard (route along z = 0) with a stair three wide up its back wall to
    /// a ledge, walled at both sides, and a one-wide bar in the ledge's back
    /// wall with a room past it. Course `i` (z = 4 + i) stands `i` high.
    fn stair_yard(shut: bool) -> World {
        let mut solid = BTreeSet::new();
        for x in -1..=11 {
            for z in -1..=14 {
                solid.insert([x, 0, z]);
                let yard = (0..=10).contains(&x) && (0..=4).contains(&z);
                let up = (4..=6).contains(&x) && (5..=13).contains(&z);
                if !yard && !up {
                    for y in 1..=10 {
                        solid.insert([x, y, z]);
                    }
                }
            }
        }
        for x in 4..=6 {
            for z in 5..=8 {
                for y in 1..=z - 4 {
                    solid.insert([x, y, z]);
                }
            }
            // The ledge, its floor five up, z 9..=11.
            for z in 9..=11 {
                for y in 1..=5 {
                    solid.insert([x, y, z]);
                }
            }
            // The back wall at z 12 with the bar at x 5, and the room past it.
            for y in 1..=10 {
                if !(x == 5 && (6..=7).contains(&y)) {
                    solid.insert([x, y, 12]);
                }
            }
            for z in 13..=13 {
                for y in 1..=5 {
                    solid.insert([x, y, z]);
                }
            }
        }
        if shut {
            solid.extend([[5, 6, 12], [5, 7, 12]]);
        }
        World::from_solid_cells(solid)
    }

    /// spec-0118 §5.2: a stair three wide is cut at its run. The path's own
    /// cells do not separate the ledge — the open side climbs the other two
    /// columns — so the ground is empty and `rewarded` is refused, as before;
    /// handed the stair's courses (three wide) and a station across the yard
    /// (five wide, which the yard's floor flows round), the narrowest that
    /// separates is a course, nearest the face first, and the chest on the
    /// ledge pays.
    #[test]
    fn a_three_wide_stair_cuts_at_its_run() {
        let (base, shut) = (stair_yard(false), stair_yard(true));
        let ways = [way(([5, 6, 12], [5, 7, 12]), 2, Some(Learned::Rewarded))];
        let route: Vec<[i32; 3]> = (0..=10).map(|x| [x, 1, 0]).collect();
        let chest = Reward {
            anchor: "anchor/ledge-chest".to_string(),
            kind: "a loot holder",
            pos: [4, 6, 11],
        };
        let rewards = std::slice::from_ref(&chest);
        let (b, verdict) = judge_cut(&shut, &base, &ways, route.clone(), rewards, &[]);
        let err = verdict.expect_err("the path alone does not cut a stair three wide");
        assert_eq!(err.code.id(), "DW1054", "{}", err.message);
        assert_eq!(b.rows[0]["rewarded"]["ground_cells"], 0, "{:?}", b.rows);
        assert!(
            err.message
                .contains("behind the branch path's own cells, one body wide"),
            "{}",
            err.message
        );
        let mut sections: Vec<CrossSection> = (1..=4)
            .map(|i| section(&format!("course {} of the run", 5 - i), 4, 3, i + 1, 4 + i))
            .collect();
        sections.push(section("the yard's middle", 3, 5, 1, 2));
        let (b, verdict) = judge_cut(&shut, &base, &ways, route.clone(), rewards, &sections);
        assert!(verdict.is_ok(), "{verdict:?} {:?}", b.rows);
        assert_eq!(b.rewarded, 1);
        assert_eq!(
            b.rows[0]["branch_cut"],
            serde_json::json!({ "at": "course 1 of the run", "way": "the stair `edge/test-stair`", "width": 3 }),
            "{:?}",
            b.rows
        );
        // The wide station alone is round-flowed: no cut, no ground.
        let (b, verdict) = judge_cut(&shut, &base, &ways, route, rewards, &sections[4..]);
        assert_eq!(verdict.expect_err("no narrowing").code.id(), "DW1054");
        assert_eq!(b.rows[0]["branch_cut"]["at"], "path");
    }

    /// spec-0118 §5.2: a branch with no narrowing is refused as before. The
    /// open hall of the test above, with a declared station three wide that
    /// the branch path walks across the open floor: the floor flows round
    /// it, so it cuts nothing, the ground is empty and `rewarded` is refused
    /// `DW1054`, cut at the path.
    #[test]
    fn a_branch_with_no_narrowing_is_refused_as_before() {
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
        let mid = [section("the hall's middle", 4, 3, 1, 5)];
        let (b, verdict) = judge_cut(
            &shut,
            &base,
            &ways,
            route,
            std::slice::from_ref(&chest),
            &mid,
        );
        let err = verdict.expect_err("an open floor is no branch");
        assert_eq!(err.code.id(), "DW1054", "{}", err.message);
        assert_eq!(b.rows[0]["rewarded"]["ground_cells"], 0, "{:?}", b.rows);
        assert_eq!(b.rows[0]["branch_cut"]["at"], "path");
    }

    /// Acceptance 9: the binding line prints every count, zeroes included.
    #[test]
    fn the_binding_line_states_every_count() {
        let b = ShutWayBinding::default();
        assert_eq!(
            b.line(),
            "shut ways (DW1053/DW1054): 0 gated connection(s); over 0 configuration(s), 0 shut \
             way(s) met — 0 at a face, 0 by the eye alone — 0 declared (0 seen, 0 told, 0 \
             rewarded, 0 hidden), 0 undeclared, 0 declaration(s) not confirmed; 0 declared and \
             never met shut; 0 hidden way(s) judged over 0 configuration(s), 0 eye line(s) \
             examined"
        );
        assert_eq!(b.to_json()["met"], 0);
        assert_eq!(b.to_json()["hidden_rows"], serde_json::json!([]));
    }

    /// A world of named blocks: every cell of `solid` stone, `named` laid over
    /// it (air removes a cell), classified by the one occupancy model — so
    /// what hides is the pinned jar's occlusion table.
    fn named(solid: &BTreeSet<[i32; 3]>, named: &[([i32; 3], &str)]) -> World {
        let mut map: BTreeMap<[i32; 3], String> = solid
            .iter()
            .map(|c| (*c, "minecraft:stone".to_string()))
            .collect();
        for (c, b) in named {
            if *b == "minecraft:air" {
                map.remove(c);
            } else {
                map.insert(*c, (*b).to_string());
            }
        }
        World::from_occupancy(
            crate::compiler::assembled::occupancy_of(map, &BTreeSet::new()),
            Premises::geometry_only(),
        )
    }

    /// [`branch`]'s cells as a named world, `extra` laid over them.
    fn branch_named(len: i32, top: i32, shut: bool, extra: &[([i32; 3], &str)]) -> World {
        let mut open = BTreeSet::new();
        for x in 0..=10 {
            open.insert([x, 0]);
        }
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
        open.insert([5, len + 1]);
        let mut solid = BTreeSet::new();
        for x in -1..=11 {
            for z in -1..=len + 5 {
                solid.insert([x, 0, z]);
                if !open.contains(&[x, z]) {
                    for y in 1..=WALL {
                        solid.insert([x, y, z]);
                    }
                }
            }
        }
        for y in top + 1..=WALL {
            solid.insert([5, y, len + 1]);
        }
        if shut {
            for y in 1..=top {
                solid.insert([5, y, len + 1]);
            }
        }
        named(&solid, extra)
    }

    /// Acceptance 2, **one eye**: every sight walk here is stopped by
    /// `World::hides` and by nothing else — a grille on the line is looked
    /// through and a wall is not, where the collision walk stopped at both.
    #[test]
    fn the_eye_looks_through_a_grille_and_not_through_a_wall() {
        let region = ([5, 1, 8], [5, 2, 8]);
        let ways = [way(region, 2, Some(Learned::Seen))];
        let on_line = [5, 2, 4];
        for (block, seen) in [
            ("minecraft:iron_bars", true),
            ("minecraft:stone_bricks", false),
        ] {
            let (base, shut) = (
                branch_named(7, 2, false, &[(on_line, block)]),
                branch_named(7, 2, true, &[(on_line, block)]),
            );
            assert!(shut.blocks_camera(on_line), "{block} collides");
            assert_eq!(shut.hides(on_line), !seen, "{block}");
            let (_, verdict) = judge(&shut, &base, &ways, &[], PLAYER_EYE);
            assert_eq!(verdict.is_ok(), seen, "{block}: {verdict:?}");
        }
    }

    /// Acceptance 4, **the declared loosening**: a fence post, then a glass
    /// pane, standing on the line of sight leaves the bar `seen`.
    #[test]
    fn a_bar_behind_a_fence_or_a_pane_is_seen() {
        let region = ([5, 1, 8], [5, 2, 8]);
        let ways = [way(region, 2, Some(Learned::Seen))];
        for block in ["minecraft:oak_fence", "minecraft:glass_pane"] {
            let on_line = [5, 2, 4];
            let (base, shut) = (
                branch_named(7, 2, false, &[(on_line, block)]),
                branch_named(7, 2, true, &[(on_line, block)]),
            );
            let (b, verdict) = judge(&shut, &base, &ways, &[], PLAYER_EYE);
            assert!(verdict.is_ok(), "{block}: {verdict:?}");
            assert_eq!(b.seen, 1, "{block}: {}", b.line());
        }
    }

    /// The hearth: a hall (x 0..=10, z 0..=6) whose shut way is a one-wide
    /// shutter at `[5, 1..=2, 7]` into a passage (x 3..=7, z 8..=12), with a
    /// row of campfires at `[4..=6, 1, 6]` in its approach — a second gate,
    /// solid to a body, looked through by an eye — under a mantel at y 3..=4.
    /// `window` is laid at `[3, 2, 7]` in the passage's wall.
    fn hearth(shutter: &str, window: Option<&str>, shut: bool) -> World {
        let mut solid = BTreeSet::new();
        let open = |x: i32, z: i32| {
            ((0..=10).contains(&x) && (0..=6).contains(&z))
                || ((3..=7).contains(&x) && (8..=12).contains(&z))
        };
        for x in -1..=11 {
            for z in -1..=13 {
                solid.insert([x, 0, z]);
                if !open(x, z) {
                    for y in 1..=WALL {
                        solid.insert([x, y, z]);
                    }
                }
            }
        }
        let mut blocks: Vec<([i32; 3], &str)> = Vec::new();
        for x in 4..=6 {
            blocks.push(([x, 1, 6], "minecraft:campfire"));
            for y in 3..=WALL {
                blocks.push(([x, y, 6], "minecraft:stone_bricks"));
            }
        }
        for y in 1..=2 {
            blocks.push(([5, y, 7], if shut { shutter } else { "minecraft:air" }));
        }
        if let Some(w) = window {
            blocks.push(([3, 2, 7], w));
        }
        named(&solid, &blocks)
    }

    fn hearth_way(learned: Option<Learned>, shutter: &str, shot: Option<usize>) -> ShutWay {
        ShutWay {
            bar_block: shutter.to_string(),
            far: Some((([0, 1, 0], [10, 2, 6]), ([3, 1, 8], [7, 2, 12]))),
            shots: vec![OpeningList {
                list: "/content/quests/0/objectives/1/on_complete".to_string(),
                shot,
            }],
            ..way(([5, 1, 7], [5, 2, 7]), 2, learned)
        }
    }

    fn judge_hearth(
        shut: &World,
        base: &World,
        ways: &[ShutWay],
    ) -> (ShutWayBinding, Result<(), Failure>) {
        judge(shut, base, ways, &[], PLAYER_EYE)
    }

    /// Acceptance 3, **met by the eye**: the shutter's approach is held by a
    /// second gate, so no reach cell stands at its face; a grille is seen from
    /// the hall and, with no `learned`, refused `DW1053` with `E` at 1 and `A`
    /// at 0. The same shutter a full opaque cube is a wall, not met, and
    /// passes with `M` at 0.
    #[test]
    fn a_grille_behind_a_fire_is_met_by_the_eye() {
        let base = hearth("minecraft:air", None, false);
        let bars = "minecraft:iron_bars[east=true,west=true]";
        let shut = hearth(bars, None, true);
        let ways = [hearth_way(None, bars, Some(1))];
        let (b, verdict) = judge_hearth(&shut, &base, &ways);
        let err = verdict.expect_err("a grille in view owes its word");
        assert_eq!(err.code.id(), "DW1053", "{}", err.message);
        assert!(
            err.message.contains("the party sees it from"),
            "{}",
            err.message
        );
        assert!(err.message.contains("`hidden` —"), "{}", err.message);
        assert_eq!((b.met, b.at_face, b.by_eye), (1, 0, 1), "{}", b.line());
        assert!(
            b.rows[0]["met"]["by_the_eye_from"].is_array(),
            "{:?}",
            b.rows
        );

        let stone = "minecraft:polished_deepslate";
        let shut = hearth(stone, None, true);
        let ways = [hearth_way(None, stone, Some(1))];
        let (b, verdict) = judge_hearth(&shut, &base, &ways);
        assert!(verdict.is_ok(), "{verdict:?}");
        assert_eq!(b.met, 0, "{}", b.line());
    }

    /// Acceptance 18, **`hidden`, the eye**: an opaque shutter with a second
    /// gate in its approach and no line from the hall into the passage is
    /// hidden; the shutter iron bars is refused naming the eye cell and the bar
    /// and saying it is looked through; a window in the passage's wall is
    /// refused naming the far cell, glazed with glass still refused, walled
    /// with stone green.
    #[test]
    fn a_hidden_way_is_held_to_the_eye() {
        let base = hearth("minecraft:air", None, false);
        let stone = "minecraft:polished_deepslate";
        let ways = [hearth_way(Some(Learned::Hidden), stone, Some(1))];
        let (b, verdict) = judge_hearth(&hearth(stone, None, true), &base, &ways);
        assert!(verdict.is_ok(), "{verdict:?}");
        assert_eq!(
            (b.hidden_ways, b.hidden_configurations),
            (1, 1),
            "{}",
            b.line()
        );
        assert!(b.eye_lines > 0, "{}", b.line());
        assert_eq!(b.hidden_rows[0]["eye"], "hidden", "{:?}", b.hidden_rows);

        let bars = "minecraft:iron_bars[east=true,west=true]";
        let ways = [hearth_way(Some(Learned::Hidden), bars, Some(1))];
        let (_, verdict) = judge_hearth(&hearth(bars, None, true), &base, &ways);
        let err = verdict.expect_err("a grille hides nothing");
        assert_eq!(err.code.id(), "DW1054", "{}", err.message);
        assert!(err.message.contains("declared `hidden`"), "{}", err.message);
        assert!(err.message.contains("is looked through"), "{}", err.message);
        assert!(
            err.message.contains("[5, 1, 7]") || err.message.contains("[5, 2, 7]"),
            "{}",
            err.message
        );

        let ways = [hearth_way(Some(Learned::Hidden), stone, Some(1))];
        for (window, refused) in [
            ("minecraft:air", true),
            ("minecraft:glass", true),
            ("minecraft:stone", false),
        ] {
            let shut = hearth(stone, Some(window), true);
            let base = hearth("minecraft:air", Some(window), false);
            let (_, verdict) = judge_hearth(&shut, &base, &ways);
            assert_eq!(verdict.is_err(), refused, "{window}: {verdict:?}");
            if refused {
                let m = verdict.unwrap_err().message;
                assert!(
                    m.contains("a cell a body can be in in `node/far`"),
                    "{window}: {m}"
                );
            }
        }
    }

    /// Acceptance 19, **`hidden`, the shot**: the hidden hearth with no shot of
    /// its seam in the beat that opens it is refused naming the effect list.
    #[test]
    fn a_hidden_way_with_no_shot_is_refused_naming_the_list() {
        let base = hearth("minecraft:air", None, false);
        let stone = "minecraft:polished_deepslate";
        let ways = [hearth_way(Some(Learned::Hidden), stone, None)];
        let (_, verdict) = judge_hearth(&hearth(stone, None, true), &base, &ways);
        let err = verdict.expect_err("no shot");
        assert_eq!(err.code.id(), "DW1054", "{}", err.message);
        assert!(
            err.message
                .contains("carries no shot of `anchor/seam-test`"),
            "{}",
            err.message
        );
        assert!(
            err.message
                .contains("/content/quests/0/objectives/1/on_complete"),
            "{}",
            err.message
        );
    }

    /// Acceptance 19: a `cutscene` shows a seam when its `look_at` or a `path`
    /// waypoint names it; one looking at another anchor does not.
    #[test]
    fn a_shot_shows_the_seam_it_looks_at_or_flies_through() {
        let eff = |v: serde_json::Value| -> delvewright_dsl::QuestEffect {
            serde_json::from_value(v).expect("an effect")
        };
        let seam = "anchor/seam-hearth-door";
        let looks = eff(serde_json::json!({
            "type": "cutscene", "seconds": 3,
            "path": [{"anchor": "anchor/hall-eye"}],
            "look_at": {"anchor": seam},
        }));
        let flies = eff(serde_json::json!({
            "type": "cutscene", "seconds": 3,
            "path": [{"anchor": "anchor/hall-eye"}, {"anchor": seam}],
        }));
        let elsewhere = eff(serde_json::json!({
            "type": "cutscene", "seconds": 3,
            "path": [{"anchor": "anchor/hall-eye"}],
            "look_at": {"anchor": "anchor/hearth-fire"},
        }));
        assert!(shows(&looks, seam));
        assert!(shows(&flies, seam));
        assert!(!shows(&elsewhere, seam));
    }

    /// Acceptance 8: a `hidden` way never shut in any judged configuration is
    /// counted inert and warned, not refused.
    #[test]
    fn a_hidden_way_never_shut_is_inert() {
        let base = hearth("minecraft:air", None, false);
        let stone = "minecraft:polished_deepslate";
        let ways = [hearth_way(Some(Learned::Hidden), stone, Some(1))];
        let (b, verdict) = verify_with_eye(
            &ways,
            &base,
            &[(
                &base,
                "while `obj/test` is next".to_string(),
                route(),
                Vec::new(),
            )],
            &[],
            &[],
            PLAYER_EYE,
        );
        assert!(verdict.is_ok(), "{verdict:?}");
        assert_eq!((b.inert, b.hidden_ways), (1, 0), "{}", b.line());
        let w = b.findings();
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].code, "DW1054");
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
            let (learned, _) = verify_shut_ways(&ways, &base, &configs, &[], &[]);
            assert_eq!(sep.met, learned.met_triples, "seed {seed}");
            met_any += learned.met_triples.len();
        }
        assert!(met_any > 0, "the generator meets some face");
    }

    /// **spec-0124 criterion 3**: the shut model reads a door-shut seam's leaf
    /// as it reads a bar — shut, its column is no cell a body can be in; open,
    /// it is — through the one predicate (`shutmodel::can_be`), and `DW1011`
    /// and this check meet the same (gate, configuration, face) triples over
    /// generated worlds with door-shut and bar-shut seams mixed in one wall.
    #[test]
    fn both_proofs_meet_the_same_faces_with_doors_and_bars_mixed() {
        use super::super::shutmodel::can_be;
        let (mut met_any, mut door_cells) = (0usize, 0usize);
        let (mut doors_shut, mut doors_open) = (0usize, 0usize);
        for seed in 0..40u64 {
            let mut r = Lcg(seed * 104_729 + 7);
            let doors: Vec<i32> = (0..3).map(|i| 2 + 4 * i + r.next(2) as i32).collect();
            let leaf: Vec<bool> = (0..3).map(|_| r.next(2) == 0).collect();
            let build = |shut: &BTreeSet<usize>, r: &mut Lcg| {
                let mut blocks: BTreeMap<[i32; 3], String> = BTreeMap::new();
                let mut open_cells: BTreeSet<[i32; 3]> = BTreeSet::new();
                let stone = || "minecraft:stone".to_string();
                for x in -1..=14 {
                    for z in -1..=16 {
                        blocks.insert([x, 0, z], stone());
                        if x == -1 || x == 14 || z == -1 || z == 16 {
                            for y in 1..=WALL {
                                blocks.insert([x, y, z], stone());
                            }
                        }
                    }
                    for y in 1..=WALL {
                        let seam = doors.iter().position(|d| *d == x).filter(|_| y <= 2);
                        match seam {
                            Some(d) if leaf[d] => {
                                let open = !shut.contains(&d);
                                let half = if y == 1 { "lower" } else { "upper" };
                                blocks.insert(
                                    [x, y, 8],
                                    format!(
                                        "minecraft:oak_door[facing=north,half={half},\
                                         hinge=left,open={open},powered=false]"
                                    ),
                                );
                                if open {
                                    open_cells.insert([x, y, 8]);
                                }
                            }
                            Some(d) if !shut.contains(&d) => {}
                            _ => {
                                blocks.insert([x, y, 8], stone());
                            }
                        }
                    }
                }
                for _ in 0..r.next(12) {
                    let (x, z) = (r.next(14) as i32, r.next(16) as i32);
                    if z != 8 && z != 7 && z != 9 {
                        for y in 1..=(1 + r.next(3) as i32) {
                            blocks.insert([x, y, z], stone());
                        }
                    }
                }
                World::from_occupancy(
                    crate::compiler::assembled::occupancy_of(blocks, &open_cells),
                    crate::compiler::nav::world::Premises::geometry_only(),
                )
                .without_gate_use()
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
            for (w, _, shut) in &worlds {
                for (d, x) in doors.iter().enumerate().filter(|(d, _)| leaf[*d]) {
                    door_cells += 1;
                    let closed = shut.contains(&d);
                    assert_eq!(
                        can_be(w, [*x, 1, 8]),
                        !closed,
                        "seed {seed}: a door seam's column is a body cell exactly when it is open"
                    );
                    if closed {
                        doors_shut += 1;
                    } else {
                        doors_open += 1;
                    }
                }
            }
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
            let (learned, _) = verify_shut_ways(&ways, &base, &configs, &[], &[]);
            assert_eq!(sep.met, learned.met_triples, "seed {seed}");
            met_any += learned.met_triples.len();
        }
        assert!(met_any > 0, "the generator meets some face");
        assert!(
            doors_shut > 0 && doors_open > 0,
            "the generator stands door seams both shut ({doors_shut}) and open ({doors_open}) \
             over {door_cells} door column(s)"
        );
    }
}
