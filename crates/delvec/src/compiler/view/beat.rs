//! **The configuration a camera stands in** (spec-0089 §4).
//!
//! A showcase camera stating `after: {step, path?}` is taken after that step
//! of that exported path: the world as that path leaves it once every step up
//! to and including the stated one is played, in the path's own order
//! ([`crate::compiler::view::moment`]) — every write fired on the way laid in
//! firing order over the load bytes, the cast where the cast ledger stations
//! it then, and the sky the path's beats set. The region writes are
//! [`crate::compiler::nav::Configuration`]'s, read under the path's order
//! rather than the proofs' causal ancestry: a picture is of one play of the
//! path, and on it every earlier step has been played.
//!
//! A camera with no `after` stands at load: arrival at the critical path's
//! first step, every world-load seal and setup write in place, no beat fired.
//!
//! Every rule of the field is a rule of the camera record and is refused under
//! the record's own code (`DW0721`), by `delvec cameras` and `delvec build`
//! alike, through [`stand`].

use std::collections::{BTreeMap, BTreeSet};

use crate::compiler::blockstate::BlockMap;
use crate::compiler::flow::{JournalStep, Playthrough};
use crate::compiler::nav::{Configuration, World, configuration_at};
use crate::compiler::placed::{Ledger, Placed};
use crate::compiler::plan::{PathFiring, Plan, RegionEvents, Step};
use crate::compiler::view::camera::{CAMERAS_FILE, Camera};
use crate::compiler::view::moment::{CastMark, Laid};
use crate::compiler::view::world::EntityRecord;

/// The key of the world a camera with no `after` stands in.
pub const AT_LOAD: &str = "at-load";

/// A step's id in the vocabulary `critical-path.json` speaks: the objective it
/// proves (`obj/<id>`) or the trigger it performs (`trigger/<id>`). A step
/// with neither (a class pick, a rest, a loop) has no id and cannot be named.
pub fn step_id(step: &Step) -> Option<&str> {
    step.objective().or_else(|| step.trigger())
}

/// One exported path as a camera reads it: its steps, its region writes in its
/// own step space, and its own ancestry.
#[derive(Clone)]
pub struct PathModel {
    /// `None` for the critical path; the branch id otherwise.
    pub branch: Option<String>,
    /// The branch's slug (`branch-path-<slug>.json`); `None` for the
    /// critical path.
    pub slug: Option<String>,
    pub steps: Vec<Step>,
    events: RegionEvents,
    /// `None`: the plan's own ancestry ([`Plan::gate_fired_before`]).
    ancestors: Option<BTreeMap<usize, BTreeSet<usize>>>,
    /// A branch's playthrough and what its path fires; `None` for the
    /// critical path, which is the plan's.
    story: Option<(Playthrough, PathFiring)>,
    /// The playthrough's flow journal, asked once.
    journal: std::sync::OnceLock<Vec<JournalStep>>,
}

impl PathModel {
    /// The critical path.
    pub fn critical(plan: &Plan) -> PathModel {
        PathModel {
            branch: None,
            slug: None,
            steps: plan.critical_path.clone(),
            events: plan.region_events.clone(),
            ancestors: None,
            story: None,
            journal: std::sync::OnceLock::new(),
        }
    }

    /// **The configuration of a moment on this path**: the writes fired at
    /// every step before `arrival`, in the path's own order — the world one
    /// play of the path leaves, which a picture of the moment is of. (The
    /// proofs read [`PathModel::at`], under causal ancestry.)
    pub fn at_moment(&self, world: &World, arrival: usize) -> Configuration {
        let played = |g: usize, s: usize| g == 0 || g < s;
        configuration_at(world, &self.events, &played, arrival)
    }

    /// The flow journal of this path's playthrough.
    pub fn journal(&self, plan: &Plan) -> &[JournalStep] {
        self.journal.get_or_init(|| {
            let flow = crate::compiler::flow::Flow::new(plan.campaign);
            match &self.story {
                Some((pt, _)) => flow.journal(pt),
                None => flow.journal(&flow.playthrough()),
            }
        })
    }

    /// The placements ledger as this path fires it
    /// ([`Ledger::on_path`]); the build's own on the critical path.
    pub fn ledger(&self, plan: &Plan, ledger: &Ledger) -> Ledger {
        match (&self.story, &self.ancestors) {
            (Some((_, firing)), Some(ancestors)) => ledger.on_path(
                plan,
                &crate::compiler::placed::OnPath {
                    steps: &self.steps,
                    obj_step: &firing.obj_step,
                    firing,
                    ancestors,
                },
            ),
            _ => ledger.clone(),
        }
    }

    /// The path in words: `the critical path`, or `branch `<id>`'s path`.
    pub fn label(&self) -> String {
        match &self.branch {
            Some(b) => format!("branch `{b}`'s path"),
            None => "the critical path".to_string(),
        }
    }

    /// The configuration arriving at step `arrival` of this path.
    pub fn at(&self, plan: &Plan, world: &World, arrival: usize) -> Configuration {
        match &self.ancestors {
            None => {
                let anc = |g: usize, s: usize| plan.gate_fired_before(g, s);
                configuration_at(world, &self.events, &anc, arrival)
            }
            Some(a) => {
                let anc = |g: usize, s: usize| g == 0 || a.get(&s).is_some_and(|x| x.contains(&g));
                configuration_at(world, &self.events, &anc, arrival)
            }
        }
    }

    /// Every step index carrying `id`, in path order.
    fn indices_of(&self, id: &str) -> Vec<usize> {
        self.steps
            .iter()
            .enumerate()
            .filter(|(_, s)| step_id(s) == Some(id))
            .map(|(i, _)| i)
            .collect()
    }

    /// The ids this path carries, each once, in path order.
    fn ids(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for s in &self.steps {
            if let Some(id) = step_id(s)
                && !out.contains(&id)
            {
                out.push(id);
            }
        }
        out
    }
}

/// Every branch the build declares, as `(id, slug, path)`: `None` for a
/// declared branch no flow world plays, or whose path cannot be built (the
/// branch proofs refuse that by name).
pub fn branch_paths(plan: &Plan) -> Vec<(String, Option<PathModel>)> {
    let realized = crate::compiler::branch::realize(plan.campaign);
    if realized.is_empty() {
        return Vec::new();
    }
    let flow = crate::compiler::flow::Flow::new(plan.campaign);
    realized
        .iter()
        .map(|r| {
            let model = r.world.and_then(|w| {
                let pt = flow.story_playthrough(w);
                let cp = plan.branch_critical_path(&flow, &pt).ok()?;
                let (events, ancestors) = plan.branch_gate_model(&cp);
                Some(PathModel {
                    branch: Some(r.branch.id.clone()),
                    slug: Some(r.branch.slug.clone()),
                    steps: cp.steps,
                    events,
                    ancestors: Some(ancestors),
                    story: Some((pt, cp.firing)),
                    journal: std::sync::OnceLock::new(),
                })
            });
            (r.branch.id.clone(), model)
        })
        .collect()
}

/// Where one camera stands.
#[derive(Debug, Clone, PartialEq)]
pub struct Stand {
    /// The camera's name.
    pub camera: String,
    /// The world's key: [`AT_LOAD`], or `after-<step>` with the branch's slug
    /// appended (`-on-<slug>`).
    pub key: String,
    /// What the camera is taken after; `None` at load.
    pub after: Option<StandAfter>,
    /// Cells whose block differs from load in the world written.
    pub cells_moved: usize,
    /// Members of the cast standing elsewhere than at load, or out of the
    /// world where they stood at load, or back in it.
    pub cast_moved: usize,
    /// Whether the path has set another sky than load's by then.
    pub sky_moved: bool,
    /// Unforced writes the configuration holds and does not lay.
    pub unforced: usize,
}

/// The step a camera stands after, as the binding line names it.
#[derive(Debug, Clone, PartialEq)]
pub struct StandAfter {
    pub step: String,
    /// One-based position of the step on its path.
    pub index: usize,
    /// The path's step count.
    pub of: usize,
    /// The path in words.
    pub path: String,
}

impl Stand {
    /// The per-camera binding line (spec-0089 §7).
    pub fn line(&self, block_entities_omitted: usize) -> String {
        match &self.after {
            None => format!("after: {} at load", self.camera),
            Some(a) => format!(
                "after: {} after {} (step {} of {} on {}): {} cells moved from load, {} of the \
                 cast moved, sky {}, {} unforced write(s) not laid, {block_entities_omitted} block \
                 entit(ies) omitted; biomes: at load; clock: as the picture",
                self.camera,
                a.step,
                a.index,
                a.of,
                a.path,
                self.cells_moved,
                self.cast_moved,
                if self.sky_moved {
                    "set by the story"
                } else {
                    "as at load"
                },
                self.unforced
            ),
        }
    }
}

/// The world key of a camera taken after `step` on a path with `slug`.
pub fn after_key(step: &str, slug: Option<&str>) -> String {
    let base = format!("after-{}", step.replace('/', "-"));
    match slug {
        Some(s) => format!("{base}-on-{s}"),
        None => base,
    }
}

/// An inclusive box of cells, `(min, max)`.
pub type Region = ([i32; 3], [i32; 3]);

/// **What a moment holds** on one path: its writes in firing order, the
/// ledger rows it holds, its bodies with the cast stood where the ledger puts
/// it, and its sky.
#[derive(Clone)]
pub struct Moment {
    pub laid: Laid,
    /// The path's ledger rows the moment holds, in firing order.
    pub rows: Vec<Placed>,
    /// Every body the world holds.
    pub bodies: Vec<EntityRecord>,
    pub cast: Vec<CastMark>,
    /// The latest `set-time` and `set-weather` the path fired by then.
    pub sky: (Option<String>, Option<String>),
    /// Unforced writes the configuration holds and does not lay.
    pub unforced: usize,
}

impl Moment {
    /// How many of the cast stand elsewhere than in `other`.
    pub fn cast_moved_from(&self, other: &Moment) -> usize {
        let mut ids: BTreeSet<&str> = self.cast.iter().map(|m| m.npc.as_str()).collect();
        ids.extend(other.cast.iter().map(|m| m.npc.as_str()));
        let at = |m: &Moment, id: &str| m.cast.iter().find(|c| c.npc == id).map(|c| c.at);
        ids.into_iter()
            .filter(|id| at(self, id) != at(other, id))
            .count()
    }
}

/// What every moment of one run is read against: the build's placements
/// ledger and the walks its NPCs are planned to take.
pub struct MomentInputs<'a> {
    pub ledger: &'a Ledger,
    pub moves: Vec<crate::compiler::nav::MovePlan>,
}

impl<'a> MomentInputs<'a> {
    /// The ledger and the plan's walks over `world` (none when they do not
    /// plan: the build refuses that by name).
    pub fn of(plan: &Plan, world: &World, ledger: &'a Ledger) -> MomentInputs<'a> {
        MomentInputs {
            ledger,
            moves: crate::compiler::nav::plan_moves(plan, world).unwrap_or_default(),
        }
    }
}

/// **The moment at arrival `arrival` of `path`** ([`crate::compiler::view::moment`]).
pub fn moment(
    plan: &Plan,
    world: &World,
    path: &PathModel,
    arrival: usize,
    inputs: &MomentInputs<'_>,
) -> Moment {
    let cfg = path.at_moment(world, arrival);
    let ledger = path.ledger(plan, inputs.ledger);
    let rows: Vec<&Placed> =
        crate::compiler::placed::rows_under(&ledger, arrival, &|g, s| g == 0 || g < s);
    let laid = Laid::of(&cfg.laid_writes(), &rows);
    let journal = path.journal(plan);
    let cast = crate::compiler::view::moment::story_at(&path.steps, journal, arrival)
        .map(|story| crate::compiler::view::moment::cast_at(plan, &story, &inputs.moves))
        .unwrap_or_default();
    let bodies = crate::compiler::view::moment::bodies_of(&rows, &cast, &ledger.rows);
    let sky = crate::compiler::view::moment::clock_at(plan, &path.steps, journal, arrival);
    Moment {
        laid,
        rows: rows.into_iter().cloned().collect(),
        bodies,
        cast,
        sky,
        unforced: cfg.unforced_writes(),
    }
}

/// What a set of cameras stands in: per distinct key the moment's bytes and
/// what its world holds beyond them.
#[derive(Clone)]
pub struct Stands {
    /// Per camera, in the order asked.
    pub stands: Vec<Stand>,
    /// Per distinct key, in first-asked order, the block map its world holds.
    pub worlds: Vec<(String, BlockMap)>,
    /// Per distinct key, the regions its writes lay: the writer holds the
    /// level generator's block out of them.
    pub laid: Vec<(String, Vec<Region>)>,
    /// Per distinct key, the moment its world is of.
    pub moments: Vec<(String, Moment)>,
}

impl std::fmt::Debug for Stands {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stands")
            .field("stands", &self.stands)
            .field(
                "worlds",
                &self.worlds.iter().map(|(k, _)| k).collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl Stands {
    /// The regions the moment of world `key` lays (empty for a key it does
    /// not hold).
    pub fn laid_in(&self, key: &str) -> &[Region] {
        self.laid
            .iter()
            .find(|(k, _)| k == key)
            .map_or(&[], |(_, r)| r.as_slice())
    }

    /// The moment world `key` is of.
    pub fn moment(&self, key: &str) -> Option<&Moment> {
        self.moments.iter().find(|(k, _)| k == key).map(|(_, m)| m)
    }
}

/// **The bytes a picture starts from**: the world as shipped
/// ([`crate::compiler::assembled::shipped_blocks`] — the assembly, its relight
/// fixtures, its trigger props and its sealed gates), less every gated trap's trigger whose gate
/// is shut at load — the datapack's `trap_gate_init` removes it and puts it
/// back only when the gate opens. A configuration's bytes are
/// [`Configuration::blocks`] over this map.
pub fn picture_base(
    plan: &Plan,
    assembled: &crate::compiler::assembled::Assembled,
    placements: &[crate::compiler::light::Placement],
) -> BlockMap {
    let mut m = crate::compiler::assembled::shipped_blocks(
        plan,
        &assembled.blocks,
        placements,
        &assembled.gate_seals,
        None,
    );
    for t in &plan.traps {
        if trap_shut_at_load(plan, t) {
            m.remove(&t.trigger_cell);
        }
    }
    m
}

/// Whether a trap's gate is shut before any beat: it requires a flag (no flag
/// is set at load), forbids none it could hold, or compares a datum its
/// declared starting value fails.
fn trap_shut_at_load(plan: &Plan, t: &crate::compiler::plan::TrapPlan) -> bool {
    use delvewright_dsl::CompareOp;
    if !t.requires_flags.is_empty() {
        return true;
    }
    t.requires_state.iter().any(|cmp| {
        let initial = plan
            .campaign
            .quests
            .content
            .state
            .iter()
            .find(|d| d.id == cmp.state)
            .map_or(0, |d| d.initial);
        !match cmp.op {
            CompareOp::Equals => initial == cmp.value,
            CompareOp::NotEquals => initial != cmp.value,
            CompareOp::AtLeast => initial >= cmp.value,
            CompareOp::AtMost => initial <= cmp.value,
        }
    })
}

/// Where a run's cameras stand, with the one writer of their worlds
/// ([`crate::compiler::view::review::WorldMaker`]) and the clock a world is
/// written at: the world's own, arriving at the critical path's first step.
pub struct Stood {
    pub stands: Stands,
    pub maker: crate::compiler::view::review::WorldMaker,
    /// `level.dat`'s `DayTime` for every world written.
    pub day_time: i64,
    /// Every body that turns to face the nearest player within its reach
    /// ([`crate::compiler::watching::watchers`] with `who` nearest): the tag
    /// its body carries and its reach in blocks. A picture's lens stands in for
    /// that player ([`crate::compiler::view::moment::facing_the_lens`]).
    pub watchers: Vec<(String, u32)>,
}

impl Stood {
    /// The watches a picture turns: every watcher a player of any class
    /// draws.
    pub fn watchers_of(plan: &Plan) -> Vec<(String, u32)> {
        crate::compiler::watching::watchers(plan)
            .into_iter()
            .filter(|w| w.class_tag.is_none())
            .map(|w| (w.tag, w.within))
            .collect()
    }
}

/// The load configuration's bytes: the critical path's arrival at step 0 over
/// the assembled map.
pub fn load_blocks(plan: &Plan, world: &World, base: &BlockMap) -> BlockMap {
    PathModel::critical(plan).at(plan, world, 0).blocks(base)
}

/// The regions the load configuration's writes lay — every world-load seal
/// and step-0 write ([`load_blocks`]'s writes).
pub fn load_laid(plan: &Plan, world: &World) -> Vec<Region> {
    PathModel::critical(plan).at(plan, world, 0).laid_regions()
}

/// The moment at load: arrival at the critical path's first step.
pub fn load_moment(plan: &Plan, world: &World, inputs: &MomentInputs<'_>) -> Moment {
    moment(plan, world, &PathModel::critical(plan), 0, inputs)
}

/// **Where one camera stands**, or the record's refusal of its `after`.
///
/// `branches` is [`branch_paths`], asked only when a camera names a path.
#[allow(clippy::too_many_arguments)]
pub fn stand(
    plan: &Plan,
    world: &World,
    base: &BlockMap,
    load: &Moment,
    critical: &PathModel,
    branches: &mut dyn FnMut() -> Vec<(String, Option<PathModel>)>,
    cam: &Camera,
    inputs: &MomentInputs<'_>,
) -> Result<(Stand, Option<Moment>), String> {
    let Some(after) = &cam.after else {
        return Ok((
            Stand {
                camera: cam.name.clone(),
                key: AT_LOAD.to_string(),
                after: None,
                cells_moved: 0,
                cast_moved: 0,
                sky_moved: false,
                unforced: 0,
            },
            None,
        ));
    };
    let who = format!("{CAMERAS_FILE}: camera `{}`", cam.name);
    let owned;
    let path: &PathModel = match &after.path {
        None => critical,
        Some(p) => {
            let all = branches();
            let declared: Vec<String> = all.iter().map(|(id, _)| id.clone()).collect();
            match all.into_iter().find(|(id, _)| id == p) {
                Some((_, Some(m))) => {
                    owned = m;
                    &owned
                }
                Some((_, None)) => {
                    return Err(format!(
                        "{who} states `after.path` `{p}`, a branch the build declares and no \
                         world plays, so it has no path to stand on. Name the critical path (drop \
                         `after.path`) or a branch that is played: {}",
                        list_or_none(&declared)
                    ));
                }
                None => {
                    return Err(format!(
                        "{who} states `after.path` `{p}`, which names no branch the build \
                         declares. Branches (validation/branch-plan.json): {}",
                        list_or_none(&declared)
                    ));
                }
            }
        }
    };
    let label = path.label();
    let at = path.indices_of(&after.step);
    let i = match at.as_slice() {
        [i] => *i,
        [] => {
            return Err(format!(
                "{who} states `after.step` `{}`, which is no step of {label}. A camera is taken \
                 after a step the path carries, named by its objective or trigger id. Steps: {}",
                after.step,
                list_or_none(&path.ids().iter().map(|s| s.to_string()).collect::<Vec<_>>())
            ));
        }
        many => {
            let once: Vec<String> = path
                .ids()
                .into_iter()
                .filter(|id| path.indices_of(id).len() == 1)
                .map(str::to_string)
                .collect();
            return Err(format!(
                "{who} states `after.step` `{}`, which {label} carries {} times (steps {}): an id \
                 that names more than one step is a candidate, not a match, and the picture would \
                 be of whichever was meant. Name a step the path carries once: {}",
                after.step,
                many.len(),
                many.iter()
                    .map(|i| (i + 1).to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
                list_or_none(&once)
            ));
        }
    };
    let m = moment(plan, world, path, i + 1, inputs);
    // **A moment is distinct from load** when a block, a member of the cast
    // or the sky differs: each is something a picture of it shows.
    let differs = |m: &Moment| {
        (
            m.laid.moved_from(&load.laid, base),
            m.cast_moved_from(load),
            m.sky != load.sky,
        )
    };
    let (moved, cast_moved, sky_moved) = differs(&m);
    if moved == 0 && cast_moved == 0 && !sky_moved {
        // The remedy names a step a camera can state: one the path carries
        // once, so the step it names is never refused as a candidate.
        let first = (0..path.steps.len())
            .filter(|j| step_id(&path.steps[*j]).is_some_and(|id| path.indices_of(id).len() == 1))
            .find(|j| {
                let (b, c, s) = differs(&moment(plan, world, path, j + 1, inputs));
                b > 0 || c > 0 || s
            })
            .and_then(|j| step_id(&path.steps[j]).map(str::to_string));
        return Err(format!(
            "{who} states `after.step` `{}`, and the world after it on {label} is the world at load \
             — no block, no member of the cast and no sky differs: a picture of nothing different, \
             which goes stale the moment a beat is added. REMOVE `after` from camera `{}`, or name \
             a step after which something moves — {}",
            after.step,
            cam.name,
            match first {
                Some(f) => format!("the first on {label} is `{f}`"),
                None => format!(
                    "{label} moves no block, no member of the cast and no sky after a step it \
                     carries once"
                ),
            }
        ));
    }
    Ok((
        Stand {
            camera: cam.name.clone(),
            key: after_key(&after.step, path.slug.as_deref()),
            after: Some(StandAfter {
                step: after.step.clone(),
                index: i + 1,
                of: path.steps.len(),
                path: label,
            }),
            cells_moved: moved,
            cast_moved,
            sky_moved,
            unforced: m.unforced,
        },
        Some(m),
    ))
}

/// Where every camera stands, and one world per distinct key — the load
/// moment under [`AT_LOAD`] whenever a camera stands there. Refuses at the
/// first camera whose `after` the record's rules refuse, naming it.
pub fn stands(
    plan: &Plan,
    world: &World,
    base: &BlockMap,
    cameras: &[Camera],
    ledger: &Ledger,
) -> Result<Stands, String> {
    let inputs = MomentInputs::of(plan, world, ledger);
    let load = load_moment(plan, world, &inputs);
    let critical = PathModel::critical(plan);
    let mut cache: Option<Vec<(String, Option<PathModel>)>> = None;
    let mut out = Stands {
        stands: Vec::new(),
        worlds: Vec::new(),
        laid: Vec::new(),
        moments: Vec::new(),
    };
    for cam in cameras {
        let mut branches = || cache.get_or_insert_with(|| branch_paths(plan)).clone();
        let (s, m) = stand(
            plan,
            world,
            base,
            &load,
            &critical,
            &mut branches,
            cam,
            &inputs,
        )?;
        if !out.worlds.iter().any(|(k, _)| *k == s.key) {
            let m = m.unwrap_or_else(|| load.clone());
            out.worlds.push((s.key.clone(), m.laid.bytes(base)));
            out.laid.push((s.key.clone(), m.laid.regions()));
            out.moments.push((s.key.clone(), m));
        }
        out.stands.push(s);
    }
    Ok(out)
}

/// The record's `after` rules, as the build asks them: every camera of the
/// record stands somewhere. Returns how many cameras state an `after`. The
/// build asks before its ledger is written, so a moment is judged by its
/// region writes, its cast and its sky.
pub fn check_record(
    plan: &Plan,
    world: &World,
    base: &BlockMap,
    cameras: &[Camera],
) -> Result<usize, String> {
    if cameras.iter().all(|c| c.after.is_none()) {
        return Ok(0);
    }
    let with: Vec<Camera> = cameras
        .iter()
        .filter(|c| c.after.is_some())
        .cloned()
        .collect();
    let none = Ledger::default();
    let inputs = MomentInputs::of(plan, world, &none);
    let load = load_moment(plan, world, &inputs);
    let critical = PathModel::critical(plan);
    let mut cache: Option<Vec<(String, Option<PathModel>)>> = None;
    for cam in &with {
        let mut branches = || cache.get_or_insert_with(|| branch_paths(plan)).clone();
        stand(
            plan,
            world,
            base,
            &load,
            &critical,
            &mut branches,
            cam,
            &inputs,
        )?;
    }
    Ok(with.len())
}

/// The summary binding line: `configurations: <k> written for <c> camera(s),
/// <a> after a step, <l> at load`.
pub fn summary(stands: &Stands) -> String {
    let after = stands.stands.iter().filter(|s| s.after.is_some()).count();
    format!(
        "configurations: {} written for {} camera(s), {after} after a step, {} at load",
        stands.worlds.len(),
        stands.stands.len(),
        stands.stands.len() - after
    )
}

fn list_or_none(items: &[String]) -> String {
    if items.is_empty() {
        "none".to_string()
    } else {
        items.join(", ")
    }
}
