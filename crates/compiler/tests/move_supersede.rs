//! `move-npc` supersession: one body, one live walk driver.
//!
//! `move-npc` compiles to a self-scheduling per-tick driver (`mv_tick_<npc>_<to>`)
//! that teleports `@e[tag=dw_npc_<id>]` along a precomputed waypoint polyline. The
//! re-entry latch (`#mrun_<bare>`) is keyed per **(npc, to_anchor, gate)** — it stops
//! a walk from restarting itself, and nothing else. Firing a SECOND `move-npc` for the
//! SAME body while an earlier walk still runs therefore used to leave two drivers
//! alive: both tp the same entity every tick, the interleave garbles the path, and the
//! walk with more remaining ticks writes the last position — so the body parked at the
//! FIRST walk's endpoint, not the last-fired one (root-caused live on the island,
//! 2026-08-06: a 408-tick beach→mouth walk overlapped by a 21-tick walk to
//! checkpoint-1 left the NPC at the mouth, 3.0 blocks off the cast-ledger cell).
//!
//! The contract: **a later `move-npc` for the same body supersedes any walk still
//! running for that body.** The superseded driver halts on its next tick and the new
//! walk's tp sequence runs alone from its own first waypoint. (Waypoints are
//! precomputed from the walk's declared start, so the new leg still snaps to its own
//! first waypoint — exactly what single-walk content already gets when a walk fires
//! while its NPC stands somewhere else.)
//!
//! These tests **execute the emitted commands**: a small interpreter for the mcfunction
//! subset the drivers use ([`Sim`]) runs the real start functions through the real
//! scheduler loop and reads the body's final position off the `tp` commands. Nothing
//! here asserts a spelling; the assertions are about where the body ends up.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

/// One `move-npc` for the keeper (the pre-existing single-walk shape).
const QUESTS_ONE_WALK: &str = r#"{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {
    "quests": [
      {
        "id": "quest/open-the-door",
        "trigger": { "type": "campaign-start" },
        "objectives": [
          { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" },
          { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit", "radius": 2,
            "after": ["obj/talk"] }
        ],
        "on_objective_complete": {
          "obj/talk": [
            { "type": "open-gate", "anchor": "anchor/door" },
            { "type": "move-npc", "npc": "npc/keeper", "to_anchor": "anchor/exit" }
          ]
        },
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ]
  }
}"#;

/// Two `move-npc` legs for the SAME body: a long one (keeper-stand → exit, 28
/// waypoint ticks) and a short one (exit → door, 21) — the island's overlap shape,
/// where the long leg outlives the short one and would win the tp race.
fn quests_two_walks() -> String {
    QUESTS_ONE_WALK.replacen(
        r#"{ "type": "move-npc", "npc": "npc/keeper", "to_anchor": "anchor/exit" }"#,
        r#"{ "type": "move-npc", "npc": "npc/keeper", "to_anchor": "anchor/exit" },
            { "type": "move-npc", "npc": "npc/keeper", "to_anchor": "anchor/door" }"#,
        1,
    )
}

fn build_with(quests: String) -> BuildOutput {
    let raw = RawCampaign {
        world: read_hw("world.json"),
        npcs: read_hw("npcs.json"),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests,
        dialogue: read_hw("dialogue.json"),
        world_edits: None,
    };
    let campaign: Campaign = parse_campaign(&raw).expect("campaign parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            let bytes = std::fs::read(common::prefabs_dir().join(&piece.structure_file)).unwrap();
            structures.insert(piece.structure_file.clone(), bytes);
        }
    }
    emit::build(
        &plan,
        &BTreeMap::new(),
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
    .expect("every emitted command validates")
}

const FN_DIR: &str = "datapack/data/hello-world/function";

/// Every emitted `mv_*` function, keyed by bare name.
fn move_fns(out: &BuildOutput) -> BTreeMap<String, Vec<String>> {
    let mut m = BTreeMap::new();
    for (p, b) in out.iter() {
        if let Some(name) = p
            .strip_prefix(&format!("{FN_DIR}/"))
            .and_then(|n| n.strip_suffix(".mcfunction"))
            && name.starts_with("mv_")
        {
            let body = String::from_utf8(b.clone()).unwrap();
            m.insert(
                name.to_string(),
                body.lines()
                    .map(str::to_string)
                    .filter(|l| !l.trim().is_empty())
                    .collect(),
            );
        }
    }
    m
}

// ---------------------------------------------------------------------------
// A minimal mcfunction interpreter for the walk-driver subset.
// ---------------------------------------------------------------------------

/// One teleport the simulation observed: the tick it happened on, the function that
/// issued it, and where it put the body.
#[derive(Clone, Debug, PartialEq)]
struct Tp {
    tick: u32,
    func: String,
    pos: [f64; 3],
}

/// Executes the `mv_*` functions: `execute if/unless score` guards, the
/// `scoreboard players set/add/operation` writes, `schedule function … 1t`,
/// `return fail`, `function`, and `tp`. Anything else is inert (nothing else in a
/// walk driver moves the body).
struct Sim {
    fns: BTreeMap<String, Vec<String>>,
    scores: BTreeMap<(String, String), i64>,
    /// Functions due on the NEXT tick (scheduled `1t`), in schedule order.
    /// `schedule function` defaults to `replace`, so a name appears at most once.
    pending: Vec<String>,
    tick: u32,
    tps: Vec<Tp>,
}

impl Sim {
    fn new(fns: BTreeMap<String, Vec<String>>) -> Self {
        Self {
            fns,
            scores: BTreeMap::new(),
            pending: Vec::new(),
            tick: 0,
            tps: Vec::new(),
        }
    }

    fn score(&self, player: &str, obj: &str) -> Option<i64> {
        self.scores
            .get(&(player.to_string(), obj.to_string()))
            .copied()
    }

    /// A `matches` range test. An unset score matches nothing.
    fn matches(v: Option<i64>, range: &str) -> bool {
        let Some(v) = v else { return false };
        match range.split_once("..") {
            None => range.parse::<i64>().map(|n| v == n).unwrap_or(false),
            Some(("", hi)) => v <= hi.parse::<i64>().unwrap(),
            Some((lo, "")) => v >= lo.parse::<i64>().unwrap(),
            Some((lo, hi)) => v >= lo.parse::<i64>().unwrap() && v <= hi.parse::<i64>().unwrap(),
        }
    }

    /// The `execute` clause chain of `line`; `None` when the line is a bare command.
    /// Returns (conditions hold, the command after `run`).
    fn eval_execute<'a>(&self, line: &'a str) -> (bool, &'a str) {
        let Some(rest) = line.strip_prefix("execute ") else {
            return (true, line);
        };
        let (clauses, cmd) = rest.split_once(" run ").expect("execute … run …");
        let tok: Vec<&str> = clauses.split_whitespace().collect();
        let mut i = 0;
        let mut hold = true;
        while i < tok.len() {
            let negate = match tok[i] {
                "if" => false,
                "unless" => true,
                other => panic!("unsupported execute clause `{other}` in `{line}`"),
            };
            assert_eq!(
                tok[i + 1],
                "score",
                "unsupported execute clause in `{line}`"
            );
            let a = self.score(tok[i + 2], tok[i + 3]);
            let ok = if tok[i + 4] == "matches" {
                let r = Self::matches(a, tok[i + 5]);
                i += 6;
                r
            } else {
                let op = tok[i + 4];
                let b = self.score(tok[i + 5], tok[i + 6]);
                i += 7;
                // A comparison against an unset score is false, both ways.
                match (a, b) {
                    (Some(a), Some(b)) => match op {
                        "<" => a < b,
                        "<=" => a <= b,
                        "=" => a == b,
                        ">=" => a >= b,
                        ">" => a > b,
                        other => panic!("unsupported comparison `{other}` in `{line}`"),
                    },
                    _ => false,
                }
            };
            hold &= ok ^ negate;
        }
        (hold, cmd)
    }

    /// Run `name` to completion (or to its `return`).
    fn call(&mut self, name: &str) {
        let body = self.fns.get(name).cloned().unwrap_or_default();
        for line in body {
            let (hold, cmd) = self.eval_execute(&line);
            if !hold {
                continue;
            }
            let tok: Vec<&str> = cmd.split_whitespace().collect();
            match tok.as_slice() {
                ["return", ..] => return,
                ["scoreboard", "players", "set", p, o, v] => {
                    self.scores
                        .insert((p.to_string(), o.to_string()), v.parse().unwrap());
                }
                ["scoreboard", "players", "add", p, o, v] => {
                    let cur = self.score(p, o).unwrap_or(0);
                    self.scores.insert(
                        (p.to_string(), o.to_string()),
                        cur + v.parse::<i64>().unwrap(),
                    );
                }
                ["scoreboard", "players", "operation", p, o, "=", p2, o2] => {
                    if let Some(v) = self.score(p2, o2) {
                        self.scores.insert((p.to_string(), o.to_string()), v);
                    }
                }
                ["schedule", "function", f, "1t"] => {
                    let bare = f.split_once(':').map(|(_, n)| n).unwrap_or(f).to_string();
                    if !self.pending.contains(&bare) {
                        self.pending.push(bare);
                    }
                }
                ["function", f] => {
                    let bare = f.split_once(':').map(|(_, n)| n).unwrap_or(f).to_string();
                    if self.fns.contains_key(&bare) {
                        self.call(&bare);
                    }
                }
                ["tp", _sel, x, y, z, ..] => self.tps.push(Tp {
                    tick: self.tick,
                    func: name.to_string(),
                    pos: [x.parse().unwrap(), y.parse().unwrap(), z.parse().unwrap()],
                }),
                _ => {}
            }
        }
    }

    /// Advance one server tick: run everything scheduled for it, in schedule order.
    fn step(&mut self) {
        self.tick += 1;
        for f in std::mem::take(&mut self.pending) {
            self.call(&f);
        }
    }

    fn run(&mut self, ticks: u32) {
        for _ in 0..ticks {
            self.step();
        }
    }

    fn final_pos(&self) -> Option<[f64; 3]> {
        self.tps.last().map(|t| t.pos)
    }
}

/// The last waypoint a driver teleports to — the walk's endpoint.
fn endpoint(fns: &BTreeMap<String, Vec<String>>, driver: &str) -> [f64; 3] {
    let last = fns[driver]
        .iter()
        .filter(|l| l.contains(" run tp "))
        .next_back()
        .unwrap_or_else(|| panic!("`{driver}` has no waypoint tp"));
    let t: Vec<&str> = last.split_whitespace().collect();
    let n = t.iter().position(|w| *w == "tp").unwrap();
    [
        t[n + 2].parse().unwrap(),
        t[n + 3].parse().unwrap(),
        t[n + 4].parse().unwrap(),
    ]
}

/// A later `move-npc` for the same body supersedes the walk still running: the first
/// driver stops teleporting, and the body ends at the SECOND walk's endpoint.
#[test]
fn later_move_npc_supersedes_the_running_walk() {
    let fns = move_fns(&build_with(quests_two_walks()));
    let (long_start, long_tick) = ("mv_keeper_exit", "mv_tick_keeper_exit");
    let (short_start, short_tick) = ("mv_keeper_door", "mv_tick_keeper_door");
    // Premise: the FIRST walk is the longer one, so under the defect its trailing
    // teleports overwrite the second walk's arrival (the island's shape).
    let long_len = fns[long_tick]
        .iter()
        .filter(|l| l.contains(" run tp "))
        .count();
    let short_len = fns[short_tick]
        .iter()
        .filter(|l| l.contains(" run tp "))
        .count();
    assert!(
        long_len > short_len + 5,
        "fixture premise: the first walk must outlast the second ({long_len} vs {short_len})"
    );

    let mut sim = Sim::new(fns.clone());
    sim.call(long_start);
    sim.run(6); // the long walk is under way …
    let cut = sim.tick;
    sim.call(short_start); // … and a second walk is fired for the same body
    sim.run(long_len as u32 + short_len as u32 + 10);

    let stale: Vec<&Tp> = sim
        .tps
        .iter()
        .filter(|t| t.tick > cut && t.func == long_tick)
        .collect();
    assert!(
        stale.is_empty(),
        "the superseded walk kept teleporting the body after tick {cut}: {} stray tp(s), \
         first at tick {} → {:?}",
        stale.len(),
        stale[0].tick,
        stale[0].pos
    );
    assert_eq!(
        sim.final_pos(),
        Some(endpoint(&fns, short_tick)),
        "the body must end at the LAST-FIRED walk's endpoint, not the superseded walk's \
         ({:?})",
        endpoint(&fns, long_tick)
    );
}

/// Supersession must not cancel a walk that nothing superseded: a single leg run to
/// completion still reaches its own endpoint, and a re-fire while it runs is still
/// refused (the pre-existing `#mrun_` re-entry latch).
#[test]
fn an_unsuperseded_walk_still_completes() {
    let fns = move_fns(&build_with(quests_two_walks()));
    let mut sim = Sim::new(fns.clone());
    sim.call("mv_keeper_exit");
    sim.run(4);
    sim.call("mv_keeper_exit"); // re-fire of the SAME walk: refused, walk continues
    sim.run(60);
    assert_eq!(
        sim.final_pos(),
        Some(endpoint(&fns, "mv_tick_keeper_exit")),
        "an unsuperseded walk reaches its own endpoint"
    );
    let restarts = sim.tps.iter().filter(|t| t.pos == sim.tps[0].pos).count();
    assert_eq!(
        restarts, 1,
        "the re-fire must not restart the walk from waypoint 0"
    );
}

/// A body with exactly one walk cannot be superseded by anything, so it carries no
/// supersession machinery at all — pre-existing single-walk campaigns stay
/// byte-identical (ADR-0006).
#[test]
fn a_single_walk_body_carries_no_supersession_machinery() {
    let fns = move_fns(&build_with(QUESTS_ONE_WALK.to_string()));
    assert_eq!(
        fns.keys().filter(|k| k.starts_with("mv_tick_")).count(),
        1,
        "fixture premise: exactly one walk"
    );
    for (name, body) in &fns {
        assert!(
            !body.iter().any(|l| l.contains("#mgen_")),
            "a single-walk body needs no walk generation: `{name}`:\n{}",
            body.join("\n")
        );
    }
}
