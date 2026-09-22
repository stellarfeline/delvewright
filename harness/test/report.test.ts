import { test } from "node:test";
import assert from "node:assert/strict";
import { RunReport, STAGES, reportPathFromEnv } from "../src/report.ts";
import {
  waveAttribution,
  type DeathTrial,
  type Encounter,
} from "../src/combat.ts";

const ENC: Encounter = {
  wave: "wave/bellkeeper",
  objective: "obj/the-keeper",
  step: 16,
  tier: "boss",
  pos: [101, 93, -99],
  count: 1,
  respawnsOnRest: false,
  checkpoint: [97, 71, -96],
  census: {
    census: "the-drowned-bell:wave_census_bellkeeper",
    brand: "the-drowned-bell:wave_brand_bellkeeper",
    unbrand: "the-drowned-bell:wave_unbrand_bellkeeper",
  },
  muster: {
    probe: "the-drowned-bell:wave_muster_bellkeeper",
    strike: "the-drowned-bell:wave_strike_bellkeeper",
    chip: "the-drowned-bell:wave_chip_bellkeeper",
    scale: 1000,
    unread: -1,
    bodies: 1,
    checked: 2,
    types: [
      {
        entity: "minecraft:drowned",
        facts: ["name=Bellkeeper"],
        droppedFacts: [],
        readsAttackDamage: false,
        readsFollowRange: false,
      },
    ],
    profiles: [
      {
        typeIndex: 0,
        count: 1,
        mask: 1,
        label: "1 × minecraft:drowned `Bellkeeper`",
        maxHealth: 30,
        armorAtLeast: 0,
        armorToughnessAtLeast: 0,
      },
    ],
  },
};

const TRIAL: DeathTrial = {
  encounter: "obj/the-keeper",
  wave: "wave/bellkeeper",
  attempt: 1,
  phase: "mid-fight",
  outcome: "re-engaged",
  cause: "delve-bot died",
  respawnPos: [97, 71, -96],
  atCheckpoint: true,
  kitKept: true,
  returned: true,
  reEngaged: true,
  objectiveComplete: false,
  reseats: false,
  reseat: undefined,
  reengage: undefined,
  objectivesIntact: true,
  lostObjectives: [],
  completed: true,
  abortedWith: undefined,
};

test("the ladder's labelled stages: spec-0023's two, framed by spec-0025's branch run", () => {
  // `branch-run` comes first because it says WHICH storyline the stages below it
  // are about — a passed critical path means something different on each branch.
  // `death-loop` comes last because it deliberately kills the player:
  // it is the one stage that must not run until everything else is proven.
  assert.deepEqual([...STAGES], ["branch-run", "critical-path", "die-retry", "death-loop"]);
});

test("a report for a build with no branches carries no branches section at all", () => {
  // Absent, not empty: an empty section would have to be read as "no branches
  // exist" and as "the branch machinery never ran", which are different claims.
  const report = new RunReport("hello-world", "easy");
  assert.equal("branches" in report.toJSON(), false);
});

test("every enumerated branch appears in the report — run, or skipped with a reason", () => {
  const report = new RunReport("hello-world", "easy");
  report.recordBranches("all", "branch/bolt", [
    {
      branch: "branch/hold",
      ran: false,
      passed: false,
      reason: "selected by this tier; a branch run needs a fresh world",
      chronicle: "branch-chronicle-hold.md",
      entryCommands: [],
      endings: ["ending/held"],
    },
    {
      branch: "branch/bolt",
      ran: true,
      passed: true,
      pathFile: "branch-path-bolt.json",
      chronicle: "branch-chronicle-bolt.md",
      entryCommands: ["/trigger dw.dlg_keeper set 3"],
      endings: ["ending/abandoned"],
    },
  ]);
  const json = report.toJSON() as {
    branches: { tier: string; driven: string; outcomes: Record<string, unknown>[] };
  };
  assert.equal(json.branches.tier, "all");
  assert.equal(json.branches.driven, "branch/bolt");
  assert.equal(json.branches.outcomes.length, 2);
  const hold = json.branches.outcomes[0]!;
  assert.equal(hold["ran"], false);
  assert.equal(hold["passed"], false);
  assert.match(String(hold["reason"]), /fresh world/);
  const bolt = json.branches.outcomes[1]!;
  assert.equal(bolt["ran"], true);
  assert.equal(bolt["passed"], true);
  assert.equal(bolt["reason"], null);
  assert.deepEqual(bolt["entry_commands"], ["/trigger dw.dlg_keeper set 3"]);
  assert.deepEqual(bolt["endings"], ["ending/abandoned"]);
  assert.equal(bolt["chronicle"], "branch-chronicle-bolt.md");
});









test("a branch recorded as run-but-failed never reads as passed", () => {
  const report = new RunReport("hello-world", "easy");
  report.recordBranches("branch/bolt", "branch/bolt", [
    {
      branch: "branch/bolt",
      ran: true,
      passed: false,
      pathFile: "branch-path-bolt.json",
      chronicle: "branch-chronicle-bolt.md",
      entryCommands: ["/trigger dw.dlg_keeper set 3"],
      endings: ["ending/abandoned"],
    },
  ]);
  const json = report.toJSON() as { branches: { outcomes: Record<string, unknown>[] } };
  assert.equal(json.branches.outcomes[0]!["passed"], false);
});


test("a skipped die-retry stage is recorded as skipped, never as passed", () => {
  // The failure mode this guards: reading a green run and assuming the retry loop
  // was proven when the stage never ran.
  const report = new RunReport("hollow-vigil", "easy");
  report.stage({
    stage: "die-retry",
    ran: false,
    passed: false,
    findings: ["skipped via DELVEWRIGHT_DIE_RETRY=0"],
    failures: [],
  });
  const json = report.toJSON() as { stages: Record<string, unknown>[] };
  assert.equal(json["stages"][0]!["ran"], false);
  assert.equal(json["stages"][0]!["passed"], false);
});

test("stages appear in ladder order regardless of the order they were recorded", () => {
  const report = new RunReport("x", "normal");
  report.stage({ stage: "die-retry", ran: true, passed: true, findings: [], failures: [] });
  report.stage({ stage: "critical-path", ran: true, passed: true, findings: [], failures: [] });
  const json = report.toJSON() as { stages: { stage: string }[] };
  assert.deepEqual(
    json["stages"].map((s) => s.stage),
    ["critical-path", "die-retry"],
  );
});

test("death trials and muster findings reach the artifact", () => {
  const report = new RunReport("the-drowned-bell", "normal");
  report.recordTrials([TRIAL]);
  report.recordMusterFinding(
    "wave/bellkeeper: `max_health` is declared 30 and the body's own attribute reads 20",
  );
  const json = report.toJSON() as {
    die_retry: Record<string, unknown>[];
    muster_findings: string[];
  };
  assert.equal(json["die_retry"][0]!["phase"], "mid-fight");
  assert.equal(json["die_retry"][0]!["at_checkpoint"], true);
  assert.deepEqual(json["die_retry"][0]!["respawn_pos"], [97, 71, -96]);
  assert.equal(json["muster_findings"].length, 1);
  assert.equal(report.findings().length, 1);
});

test("the report is written only when the environment names a path", () => {
  assert.equal(reportPathFromEnv({}), undefined);
  assert.equal(reportPathFromEnv({ DELVEWRIGHT_RUN_REPORT: "" }), undefined);
  assert.equal(reportPathFromEnv({ DELVEWRIGHT_RUN_REPORT: "/out/run.json" }), "/out/run.json");
});



// `phase_reached: cleared` says the step ended; it never said who ended it. A
// gallery encounter standing beside a lethal volume reads `cleared` over a cohort
// the bot barely touched, which is what this field exists to separate.
test("an encounter row states who felled its bodies, and says so when nobody can tell", () => {
  const report = new RunReport("gallery", "normal");
  report.recordEncounters([
    {
      encounter: ENC.objective,
      wave: ENC.wave,
      tier: ENC.tier,
      phaseReached: "cleared",
      declaredFacts: 2,
      attribution: waveAttribution(3, 0, 1),
    },
    {
      encounter: "obj/second",
      wave: "wave/second",
      tier: ENC.tier,
      phaseReached: "cleared",
      declaredFacts: 0,
      attribution: { kind: "unattributed", reason: "no wave census answered" },
    },
  ]);
  const json = report.toJSON() as { encounters: Record<string, unknown>[] };
  assert.deepEqual(json["encounters"][0]!["attribution"], {
    bodies: 3,
    standing: 0,
    credited: 1,
    uncredited: 2,
  });
  assert.deepEqual(json["encounters"][1]!["attribution"], {
    unattributed: "no wave census answered",
  });
});

test("a die-retry entry says whether its loop ever reached a verdict", () => {
  const report = new RunReport("the-drowned-bell", "normal");
  report.recordTrials([
    { ...TRIAL, completed: false, abortedWith: "the run ended at the scripted death" },
  ]);
  const json = report.toJSON() as { die_retry: Record<string, unknown>[] };
  assert.equal(json["die_retry"].length, 1, "a death that happened is in the artifact");
  assert.equal(json["die_retry"][0]!["completed"], false);
  assert.equal(json["die_retry"][0]!["aborted_with"], "the run ended at the scripted death");
  assert.equal(json["die_retry"][0]!["cause"], "delve-bot died");
});

test("a die-retry entry states what was waiting at the end of the loop", () => {
  // `re_engaged: false` alone cannot distinguish a won fight from a soft lock;
  // `outcome` does, and it is the field a reader (and the ladder) judges on.
  const report = new RunReport("keep-trial", "easy");
  report.recordTrials([
    { ...TRIAL, outcome: "cleared-before-retry", reEngaged: false, objectiveComplete: true },
  ]);
  const json = report.toJSON() as { die_retry: Record<string, unknown>[] };
  assert.equal(json["die_retry"][0]!["outcome"], "cleared-before-retry");
  assert.equal(json["die_retry"][0]!["re_engaged"], false);
  assert.equal(json["die_retry"][0]!["objective_complete"], true);
});

test("a die-retry entry publishes what the settled re-engage probe saw", () => {
  // `present: 0` is only readable next to `settle_ms`: a probe that answered
  // instantly saw an empty room, one that spent its budget waited for a room that
  // never filled (the island-r14 false negative was the former).
  const report = new RunReport("nobodys-cave-island", "normal");
  report.recordTrials([
    {
      ...TRIAL,
      reseats: true,
      reseat: {
        present: 3,
        declared: 3,
        carriedOver: 0,
        healthReadable: 3,
        damaged: 0,
        credited: 0,
        nearest: 1,
        farthest: 2,
        settleMs: 120,
      },
      reengage: {
        present: 3,
        declared: 3,
        carriedOver: 0,
        healthReadable: 3,
        damaged: 0,
        credited: 2,
        nearest: 12.5,
        farthest: 61.25,
        settleMs: 750,
      },
    },
  ]);
  const json = report.toJSON() as { die_retry: Record<string, unknown>[] };
  const re = json["die_retry"][0]!["reengage"] as Record<string, unknown>;
  assert.equal(json["die_retry"][0]!["reseats_on_rest"], true);
  assert.equal(re["present"], 3);
  assert.equal(re["carried_over"], 0);
  assert.equal(re["credited"], 2, "the correction the count half was judged against");
  assert.equal(re["farthest_blocks"], 61.25, "how far a feral mob strayed is evidence");
  assert.equal(re["settle_ms"], 750);
  const at = json["die_retry"][0]!["reseat"] as Record<string, unknown>;
  assert.equal(at["present"], 3, "the reading fidelity is judged on is published beside it");
  assert.equal(at["settle_ms"], 120);
});

test("a wave that does not re-seat publishes no re-seat reading", () => {
  const report = new RunReport("nobodys-cave-island", "normal");
  report.recordTrials([TRIAL]);
  const json = report.toJSON() as { die_retry: Record<string, unknown>[] };
  assert.equal(json["die_retry"][0]!["reseat"], null);
});

test("named-entity deaths carry their scripted_teardown/combat classification, never dropped", () => {
  // The island run's report surfaced five named-entity deaths with no way to tell
  // which two were the compiler's despawn-actor vanishes and which three were
  // real losses. `kind` is the fix: reclassified, both sides always present.
  const report = new RunReport("nobodys-cave-island", "normal");
  report.recordNamedEntityDeaths([
    { name: "Hollow Gate-Warder", entityId: 1, position: [10, 63, -4], kind: "combat" },
    { name: "island-herdsman", entityId: 4, position: [10, -128, 9], kind: "scripted_teardown" },
  ]);
  const json = report.toJSON() as { named_entity_deaths: Record<string, unknown>[] };
  assert.equal(json["named_entity_deaths"].length, 2);
  assert.equal(json["named_entity_deaths"][0]!["name"], "Hollow Gate-Warder");
  assert.equal(json["named_entity_deaths"][0]!["kind"], "combat");
  assert.equal(json["named_entity_deaths"][1]!["name"], "island-herdsman");
  assert.equal(json["named_entity_deaths"][1]!["kind"], "scripted_teardown");
  assert.deepEqual(json["named_entity_deaths"][1]!["position"], [10, -128, 9]);
});

test("a report with no named-entity deaths still carries an empty (not absent) array", () => {
  // Unlike `branches`, this section is always present — its absence would have
  // to be read as "the harness cannot see any deaths", not "there were none".
  const report = new RunReport("hello-world", "easy");
  const json = report.toJSON() as { named_entity_deaths: unknown[] };
  assert.deepEqual(json["named_entity_deaths"], []);
});

test("spec-0029: the name-preference binding is always reported, zero included", () => {
  // i18n v2 emits an authored custom name as a translate component, weakening
  // (never breaking) the same-type preference heuristic. The spec requires the
  // weakening be MEASURED: how many candidate-preference decisions the run made
  // and how many had a usable name. A run that made none is `unbound: true` — a
  // finding, not a pass.
  const empty = new RunReport("hello-world", "easy").toJSON()["name_preference"] as Record<string, unknown>;
  assert.deepEqual(empty, {
    decisions: 0,
    with_usable_name: 0,
    candidates: 0,
    named_candidates: 0,
    unbound: true,
  });

  const r = new RunReport("hello-world", "easy");
  r.recordNamePreference({
    decisions: 3,
    withUsableName: 3,
    candidates: 7,
    namedCandidates: 4,
  });
  assert.deepEqual(r.toJSON()["name_preference"], {
    decisions: 3,
    with_usable_name: 3,
    candidates: 7,
    named_candidates: 4,
    unbound: false,
  });
});
