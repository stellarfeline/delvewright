//! **The configuration a camera stands in** (spec-0089 §4).
//!
//! A showcase camera stating `after: {step, path?}` is taken after that step
//! of that exported path: the world as the region model holds it on arrival at
//! the step after it, which is the state the proofs route the leg leaving the
//! step under. Its bytes are [`crate::compiler::nav::Configuration::blocks`]
//! over the assembled block map — the one derivation of a configuration's
//! block map, which `DW0891` reads too — and nothing here lays a write itself.
//!
//! A camera with no `after` stands at load: the configuration arriving at the
//! critical path's first step, every world-load seal in place, no beat fired.
//!
//! Every rule of the field is a rule of the camera record and is refused under
//! the record's own code (`DW0721`), by `delvec cameras` and `delvec build`
//! alike, through [`stand`].

use std::collections::{BTreeMap, BTreeSet};

use crate::compiler::blockstate::BlockMap;
use crate::compiler::nav::{Configuration, World, configuration_at};
use crate::compiler::plan::{Plan, RegionEvents, Step};
use crate::compiler::view::camera::{CAMERAS_FILE, Camera};

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
                let cp = plan
                    .branch_critical_path(&flow, &flow.playthrough_in(w))
                    .ok()?;
                let (events, ancestors) = plan.branch_gate_model(&cp);
                Some(PathModel {
                    branch: Some(r.branch.id.clone()),
                    slug: Some(r.branch.slug.clone()),
                    steps: cp.steps,
                    events,
                    ancestors: Some(ancestors),
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
    /// Cells whose block differs from load.
    pub cells_moved: usize,
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
                "after: {} after {} (step {} of {} on {}): {} cells moved from load, {} unforced \
                 write(s) not laid, {block_entities_omitted} block entit(ies) omitted; biomes: at \
                 load; clock: as the picture",
                self.camera, a.step, a.index, a.of, a.path, self.cells_moved, self.unforced
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

/// What a set of cameras stands in: the load map, and per distinct key the
/// configuration's bytes.
#[derive(Debug, Clone, PartialEq)]
pub struct Stands {
    /// Per camera, in the order asked.
    pub stands: Vec<Stand>,
    /// Per distinct key, in first-asked order, the block map its world holds.
    pub worlds: Vec<(String, BlockMap)>,
}

/// **The bytes a picture starts from**: the world as shipped
/// ([`crate::compiler::assembled::shipped_blocks`] — the assembly, its relight
/// fixtures and its sealed gates), less every gated trap's trigger whose gate
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

/// Where a run's cameras stand, with what the world writer needs beside the
/// block maps: which biome a cell stands in, and the spawn `level.dat` names.
pub struct Stood {
    pub stands: Stands,
    /// The build's biome answer ([`crate::compiler::horizon::biome_map`]).
    pub biome: Box<dyn Fn([i32; 3]) -> String>,
    /// The campaign's start cell.
    pub spawn: [i32; 3],
}

/// The load configuration's bytes: the critical path's arrival at step 0 over
/// the assembled map.
pub fn load_blocks(plan: &Plan, world: &World, base: &BlockMap) -> BlockMap {
    PathModel::critical(plan).at(plan, world, 0).blocks(base)
}

/// **Where one camera stands**, or the record's refusal of its `after`.
///
/// `branches` is [`branch_paths`], asked only when a camera names a path.
pub fn stand(
    plan: &Plan,
    world: &World,
    base: &BlockMap,
    load: &BlockMap,
    critical: &PathModel,
    branches: &mut dyn FnMut() -> Vec<(String, Option<PathModel>)>,
    cam: &Camera,
) -> Result<(Stand, Option<BlockMap>), String> {
    let Some(after) = &cam.after else {
        return Ok((
            Stand {
                camera: cam.name.clone(),
                key: AT_LOAD.to_string(),
                after: None,
                cells_moved: 0,
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
    let cfg = path.at(plan, world, i + 1);
    let moved = cfg.moved_from(load);
    if moved == 0 {
        let first = (0..path.steps.len())
            .filter(|j| step_id(&path.steps[*j]).is_some())
            .find(|j| path.at(plan, world, j + 1).moved_from(load) > 0)
            .and_then(|j| step_id(&path.steps[j]).map(str::to_string));
        return Err(format!(
            "{who} states `after.step` `{}`, and the world after it on {label} is the world at load \
             block for block: a picture of nothing different, which goes stale the moment a beat \
             is added. REMOVE `after` from camera `{}`, or name a step after which a block moves — \
             {}",
            after.step,
            cam.name,
            match first {
                Some(f) => format!("the first on {label} is `{f}`"),
                None => format!("{label} moves no block at all"),
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
            unforced: cfg.unforced_writes(),
        },
        Some(cfg.blocks(base)),
    ))
}

/// Where every camera stands, and one block map per distinct key — the load
/// map under [`AT_LOAD`] whenever a camera stands there. Refuses at the first
/// camera whose `after` the record's rules refuse, naming it.
pub fn stands(
    plan: &Plan,
    world: &World,
    base: &BlockMap,
    cameras: &[Camera],
) -> Result<Stands, String> {
    let load = load_blocks(plan, world, base);
    let critical = PathModel::critical(plan);
    let mut cache: Option<Vec<(String, Option<PathModel>)>> = None;
    let mut out = Stands {
        stands: Vec::new(),
        worlds: Vec::new(),
    };
    for cam in cameras {
        let mut branches = || cache.get_or_insert_with(|| branch_paths(plan)).clone();
        let (s, blocks) = stand(plan, world, base, &load, &critical, &mut branches, cam)?;
        if !out.worlds.iter().any(|(k, _)| *k == s.key) {
            out.worlds
                .push((s.key.clone(), blocks.unwrap_or_else(|| load.clone())));
        }
        out.stands.push(s);
    }
    Ok(out)
}

/// The record's `after` rules, as the build asks them: every camera of the
/// record stands somewhere. Returns how many cameras state an `after`.
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
    stands(plan, world, base, &with).map(|s| s.stands.len())
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
