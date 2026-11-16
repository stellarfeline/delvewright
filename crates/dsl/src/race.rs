//! Races: an ordered course of checkpoints every racer passes in order
//! (spec-0139).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{ActorId, BoardId, Mark, RaceId};

/// **A checkpoint** (spec-0139 §2): the box `at ± extent`, a mark at its
/// centre. A racer is inside when its hitbox meets the box. A mark and not a
/// bare anchor, because a course runs where the story's places do not stand —
/// through a wood, round a corner — and its centre is said from the nearest
/// anchor. It places no body, so its offset may leave the anchor's piece.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Checkpoint {
    /// The centre of the box.
    pub at: Mark,
    /// Half-extents `[x, y, z]` in blocks from the centre: the box is
    /// `at ± extent`.
    pub extent: [u32; 3],
}

/// **A race** (spec-0139): a course of checkpoints, each an anchor-centred
/// box, that a racer passes in the order written; the last is the finish
/// line. A racer is a body with a count of checkpoints passed — every player,
/// on foot or carried, and every actor the race lists. A racer inside the
/// checkpoint that is its next is credited it; a racer inside one further on
/// is credited nothing and, when it is a player, reads `missed`. A racer
/// credited the last checkpoint of the last lap has finished and takes the
/// next place on `board`.
///
/// The race credits nobody until a `start-race` starts it. It is over when
/// every racer has finished, when `grace_ticks` have passed since the first
/// finished, or when an `end-race` ends it — whichever comes first — and an
/// objective that declares this race its `course` completes once it is over.
/// A class of its own and not an arrangement of
/// triggers: the order is a fact of the course, held per racer, and a racer
/// may be a body that is not a player.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Race {
    /// Unique race id (`race/<kebab>`).
    pub id: RaceId,
    /// The checkpoints, in the order a racer passes them; the last is the
    /// finish line. At least one (`DW1117`); two that follow one another
    /// share no cell (`DW1118`).
    pub checkpoints: Vec<Checkpoint>,
    /// How many times the course is run before the finish. Absent = once.
    /// More than one lap follows the last checkpoint with the first, so it
    /// needs two checkpoints or more (`DW1117`).
    #[serde(default = "one_lap", skip_serializing_if = "is_one_lap")]
    pub laps: u32,
    /// The actors that race beside the players: each is credited the
    /// checkpoints its body passes, as a player is, and takes its place on
    /// `board` when it finishes. A `move-actor` with `along` walks one round
    /// the course.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub racers: Vec<ActorId>,
    /// The board each finisher takes the next place on. Absent = the finish
    /// writes no board.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board: Option<BoardId>,
    /// How long the race stays open after its first finisher, in ticks:
    /// when they have passed the race is over, whoever is still on the
    /// course. Absent = it stays open until every racer has finished or an
    /// `end-race` ends it. At least one tick (`DW1117`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grace_ticks: Option<u32>,
    /// The line a player reads on the action bar while it stands in a
    /// checkpoint further on than its next: what it missed, and that this
    /// one does not count.
    pub missed: String,
    /// The line a player reads on the action bar when a checkpoint is
    /// credited, followed by the count passed and the count to pass
    /// (`<passed> 2/6`). Absent = a credit shows nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub passed: Option<String>,
}

fn one_lap() -> u32 {
    1
}

fn is_one_lap(n: &u32) -> bool {
    *n == 1
}

impl Race {
    /// How many checkpoints a racer is credited before it has finished: the
    /// course's checkpoints, once per lap.
    pub fn total(&self) -> u32 {
        self.checkpoints.len() as u32 * self.laps
    }
}

impl crate::quest::QuestsContent {
    /// The race `id`, when declared.
    pub fn race(&self, id: &str) -> Option<&Race> {
        self.races.iter().find(|r| r.id.as_str() == id)
    }
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

use crate::diagnostic::{Diagnostic, DwCode, ExitTier, codes};
use crate::envelope::Campaign;
use crate::registry::AnchorRegistry;
use crate::validate::{AnchorProviders, station_kind_diag};

crate::dw_code! {
    /// (spec-0139 §7) **A race that cannot be run as declared**: no
    /// checkpoint; no lap; a grace of no tick; several laps round one checkpoint (the next
    /// checkpoint would be the one the racer stands in); an actor listed
    /// twice among its `racers`, or racing two races; an unnamed actor
    /// racing for a place on a `board`; a `move-actor` `along` a race that
    /// does not list the actor; or an objective that declares a `course` and
    /// no `ride` (a course walked on foot is credited, and its leg is not
    /// proven). Validation-tier (exit 1). Prescription: one checkpoint or
    /// more, one lap or more, each racer once and named, and `ride` beside
    /// `course`.
    pub const RACE_CANNOT_RUN: DwCode = DwCode::new("DW1117", ExitTier::Build);
}

/// Stage-5 race checks (spec-0139): id syntax and uniqueness, every
/// reference to a race, an actor, a board and an anchor resolving
/// (`DW0112`, `DW0142`), and the shape a race can be run in (`DW1117`).
pub(crate) fn race_checks(c: &Campaign, anchors: &dyn AnchorRegistry, d: &mut Vec<Diagnostic>) {
    let q = &c.quests.content;
    let races = &q.races;
    let providers = AnchorProviders::build(c, anchors);
    for (i, r) in races.iter().enumerate() {
        crate::ids::id_syntax!(d, r.id, "quests", format!("/content/races/{i}/id"));
    }
    crate::ids::dup_check(
        races
            .iter()
            .enumerate()
            .map(|(i, r)| (r.id.as_str(), format!("/content/races/{i}/id"))),
        "quests",
        "race",
        d,
    );
    let mut raced: std::collections::BTreeMap<&str, &str> = std::collections::BTreeMap::new();
    for (i, r) in races.iter().enumerate() {
        let at = |tail: &str| format!("/content/races/{i}{tail}");
        let refuse = |d: &mut Vec<Diagnostic>, tail: &str, message: String| {
            d.push(Diagnostic::error(
                RACE_CANNOT_RUN,
                "quests",
                at(tail),
                message,
            ));
        };
        if r.checkpoints.is_empty() {
            refuse(
                d,
                "/checkpoints",
                format!(
                    "race `{}` has no checkpoint: a race is a course, and its last checkpoint \
                     is its finish line. Declare one checkpoint or more, in the order a racer \
                     passes them",
                    r.id
                ),
            );
        }
        if r.laps == 0 {
            refuse(
                d,
                "/laps",
                format!(
                    "race `{}` is run for no lap: nobody could finish it. Declare one lap or \
                     more, or drop `laps` for one",
                    r.id
                ),
            );
        }
        if r.grace_ticks == Some(0) {
            refuse(
                d,
                "/grace_ticks",
                format!(
                    "race `{}` stays open for no tick after its first finisher: it would be \
                     over in the tick it is won, before a board could show a second place. \
                     Declare one tick or more, or drop `grace_ticks` to wait for every racer",
                    r.id
                ),
            );
        }
        if r.laps > 1 && r.checkpoints.len() == 1 {
            refuse(
                d,
                "/laps",
                format!(
                    "race `{}` is run for {} laps round one checkpoint: the checkpoint after \
                     it is the one the racer already stands in, and every lap would be \
                     credited where it stands. Declare two checkpoints or more round the \
                     course",
                    r.id, r.laps
                ),
            );
        }
        for (k, cp) in r.checkpoints.iter().enumerate() {
            let anchor = cp.at.anchor.as_str();
            let path = at(&format!("/checkpoints/{k}/at/anchor"));
            if let Some(f) = station_kind_diag(
                &providers,
                anchor,
                crate::layout::StationKind::Point,
                "a checkpoint's centre",
                "quests",
                path.clone(),
            ) {
                d.push(f);
            }
            if !providers.resolvable(anchor) {
                d.push(Diagnostic::error(
                    codes::ANCHOR_UNRESOLVED,
                    "quests",
                    path,
                    format!(
                        "race `{}` centres checkpoint {} on anchor `{anchor}`, which no prefab \
                         bound in this campaign provides — {}",
                        r.id,
                        k + 1,
                        providers.anchor_remedy(
                            "use an anchor the prefab exposes (anchor names come from prefab \
                             metadata; do NOT invent one)"
                        ),
                    ),
                ));
            }
        }
        if let Some(b) = &r.board
            && !q.boards.iter().any(|x| x.id == *b)
        {
            d.push(Diagnostic::error(
                codes::DANGLING_REF,
                "quests",
                at("/board"),
                format!(
                    "race `{}` places its finishers on unknown board `{b}` — declare it in \
                     the stage-5 `boards` list, or fix the reference",
                    r.id
                ),
            ));
        }
        let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
        for (k, a) in r.racers.iter().enumerate() {
            let tail = format!("/racers/{k}");
            let Some(actor) = q.actors.iter().find(|x| x.id == *a) else {
                d.push(Diagnostic::error(
                    codes::DANGLING_REF,
                    "quests",
                    at(&tail),
                    format!(
                        "race `{}` lists unknown actor `{a}` among its racers — declare it in \
                         the stage-5 `actors` list, or fix the reference",
                        r.id
                    ),
                ));
                continue;
            };
            if !seen.insert(a.as_str()) {
                refuse(
                    d,
                    &tail,
                    format!(
                        "race `{}` lists actor `{a}` twice among its racers: a racer is one \
                         body with one count. List it once",
                        r.id
                    ),
                );
                continue;
            }
            if let Some(other) = raced.insert(a.as_str(), r.id.as_str()) {
                refuse(
                    d,
                    &tail,
                    format!(
                        "actor `{a}` races `{other}` and `{}`: a `move-actor` walks it round \
                         one course. List it in one race",
                        r.id
                    ),
                );
            }
            if r.board.is_some() && actor.name.is_none() {
                refuse(
                    d,
                    &tail,
                    format!(
                        "race `{}` places its finishers on a board, and its racer `{a}` \
                         carries no `name`: its row would read as an id. Give the actor a \
                         `name`",
                        r.id
                    ),
                );
            }
        }
    }
    // Every objective's `course` names a declared race, beside a `ride`.
    for (qi, quest) in q.quests.iter().enumerate() {
        for (oi, o) in quest.objectives.iter().enumerate() {
            let Some(id) = o.course() else { continue };
            let path = format!("/content/quests/{qi}/objectives/{oi}/course");
            if q.race(id.as_str()).is_none() {
                d.push(Diagnostic::error(
                    codes::DANGLING_REF,
                    "quests",
                    path,
                    format!(
                        "objective `{}` runs the course of unknown race `{id}` — declare it \
                         in the stage-5 `races` list, or fix the reference",
                        o.id()
                    ),
                ));
            } else if o.ride().is_none() {
                d.push(Diagnostic::error(
                    RACE_CANNOT_RUN,
                    "quests",
                    path,
                    format!(
                        "objective `{}` runs the course of `{id}` and declares no `ride`: the \
                         build proves a course ridden, checkpoint by checkpoint along its \
                         vehicle's medium, and proves no course walked on foot. Declare the \
                         vehicle the party races on as the objective's `ride`, or drop \
                         `course` (the race still credits a racer on foot)",
                        o.id()
                    ),
                ));
            }
        }
    }
    // Every `start-race` / `end-race` / `move-actor … along` names a declared
    // race, and an actor walked along one is one of its racers.
    crate::for_each_campaign_effect(c, &mut |path, _site, e| {
        let (id, walker) = match &e.verb {
            crate::Verb::StartRace { race } | crate::Verb::EndRace { race } => (race, None),
            crate::Verb::MoveActor {
                along: Some(race),
                actor,
                ..
            } => (race, Some(actor)),
            _ => return,
        };
        let Some(r) = q.race(id.as_str()) else {
            d.push(Diagnostic::error(
                codes::DANGLING_REF,
                "quests",
                path.to_string(),
                format!(
                    "`{}` names unknown race `{id}` — declare it in the stage-5 `races` list, \
                     or fix the reference",
                    e.verb.tag()
                ),
            ));
            return;
        };
        if let Some(actor) = walker
            && !r.racers.contains(actor)
        {
            d.push(Diagnostic::error(
                RACE_CANNOT_RUN,
                "quests",
                path.to_string(),
                format!(
                    "`move-actor` walks `{actor}` along `{id}`, and that race does not list it \
                     among its `racers`: the walk would pass every checkpoint and be credited \
                     none. List the actor in the race's `racers`, or drop `along`"
                ),
            ));
        }
    });
}
