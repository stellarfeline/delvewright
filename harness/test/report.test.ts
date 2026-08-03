import { test } from "node:test";
import assert from "node:assert/strict";
import { RunReport, STAGES, reportPathFromEnv } from "../src/report.ts";
import { AssistLedger, type DeathTrial, type Encounter } from "../src/combat.ts";

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
  reengage: undefined,
  objectivesIntact: true,
  lostObjectives: [],
  completed: true,
  abortedWith: undefined,
};

test("the ladder's labelled stages: spec-0023's two, framed by spec-0025's branch run", () => {
  // `branch-run` comes first because it says WHICH storyline the two stages below
  // it are about — a passed critical path means something different on each branch.
  assert.deepEqual([...STAGES], ["branch-run", "critical-path", "die-retry"]);
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

test("the run report prints the compiler's floor-gate ledger, both sides, verbatim", () => {
  // #114: before this, an unmeasurable elite surfaced only as a build-time
  // DW0477 — so a reader holding a green run report had no way to learn that its
  // empty findings list covered a fight nobody ever had.
  const report = new RunReport("souls-bonfire", "normal");
  report.recordCombatCoverage(
    {
      present: true,
      covered: [{ kind: "wave", id: "wave/bellkeeper", tier: "boss" }],
      notCovered: [
        {
          kind: "actor",
          id: "actor/barrow-warden",
          tier: "elite",
          reason: "it is staged but never unleashed, and it is not `vulnerable`",
        },
      ],
    },
    [],
  );
  const json = report.toJSON() as {
    floor_gate: { present: boolean; covered: unknown[]; not_covered: Record<string, unknown>[] };
  };
  assert.equal(json.floor_gate.present, true);
  assert.deepEqual(json.floor_gate.covered, [
    { kind: "wave", id: "wave/bellkeeper", tier: "boss" },
  ]);
  assert.equal(json.floor_gate.not_covered[0]!["id"], "actor/barrow-warden");
  assert.match(String(json.floor_gate.not_covered[0]!["reason"]), /never unleashed/);
});

test("a build with no ledger reports it ABSENT, not as an empty ledger", () => {
  // "This campaign bills nothing hard" and "this build cannot tell you" are
  // different facts, and only one of them is reassuring.
  const report = new RunReport("hello-world", "peaceful");
  const json = report.toJSON() as { floor_gate: { present: boolean } };
  assert.equal(json.floor_gate.present, false);
});

test("every tiered actor gets a row — fought with its outcome, or skipped with a reason", () => {
  const report = new RunReport("souls-bonfire", "normal");
  report.recordCombatCoverage({ present: true, covered: [], notCovered: [] }, [
    {
      actor: "actor/barrow-warden",
      tier: "elite",
      entity: "minecraft:wither_skeleton",
      anchor: "anchor/wave",
      covered: true,
      exercised: true,
      trial: {
        actor: "actor/barrow-warden",
        tier: "elite",
        afterObjective: "obj/open-the-door",
        outcome: "lost",
        swings: 7,
        elapsedMs: 12_000,
        detail: "bot died",
      },
    },
    {
      actor: "actor/graveward",
      tier: "boss",
      entity: "minecraft:warden",
      anchor: "anchor/graves",
      covered: true,
      exercised: false,
      reason: "unleashed only by an ambient `strike` trigger",
    },
  ]);
  const json = report.toJSON() as { actors: Record<string, unknown>[] };
  assert.equal(json.actors.length, 2);
  assert.equal(json.actors[0]!["exercised"], true);
  assert.equal(json.actors[0]!["outcome"], "lost");
  assert.equal(json.actors[0]!["swings"], 7);
  assert.equal(json.actors[0]!["reason"], null);
  // The skipped one is present, named, and has no outcome that could be misread.
  assert.equal(json.actors[1]!["exercised"], false);
  assert.equal(json.actors[1]!["outcome"], null);
  assert.match(String(json.actors[1]!["reason"]), /ambient `strike` trigger/);
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

test("the report names every assist window with its encounter id and ticks", () => {
  const report = new RunReport("the-drowned-bell", "easy");
  const ledger = new AssistLedger();
  const w = ledger.open(ENC, "after an unassisted attempt failed", 1_000);
  ledger.close(w, 61_000);
  report.recordAssists(ledger.windows());
  const json = report.toJSON() as { assist_windows: Record<string, unknown>[] };
  assert.equal(json["assist_windows"].length, 1);
  assert.equal(json["assist_windows"][0]!["encounter"], "obj/the-keeper");
  assert.equal(json["assist_windows"][0]!["ticks"], 1_200);
  assert.equal(json["assist_windows"][0]!["reason"], "after an unassisted attempt failed");
  assert.equal(json["assist_windows"][0]!["closed_at_ms"], 61_000);
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

test("death trials and floor findings reach the artifact", () => {
  const report = new RunReport("the-drowned-bell", "normal");
  report.recordTrials([TRIAL]);
  report.recordFloorFinding("wave/bellkeeper is billed `boss` and the bot beat it cold");
  const json = report.toJSON() as {
    die_retry: Record<string, unknown>[];
    floor_findings: string[];
  };
  assert.equal(json["die_retry"][0]!["phase"], "mid-fight");
  assert.equal(json["die_retry"][0]!["at_checkpoint"], true);
  assert.deepEqual(json["die_retry"][0]!["respawn_pos"], [97, 71, -96]);
  assert.equal(json["floor_findings"].length, 1);
  assert.equal(report.findings().length, 1);
});

test("the report is written only when the environment names a path", () => {
  assert.equal(reportPathFromEnv({}), undefined);
  assert.equal(reportPathFromEnv({ DELVEWRIGHT_RUN_REPORT: "" }), undefined);
  assert.equal(reportPathFromEnv({ DELVEWRIGHT_RUN_REPORT: "/out/run.json" }), "/out/run.json");
});

// --- reading an EMPTY assist ledger (task #102) ------------------------------

test("the report states each encounter's assist policy and how far the run got", () => {
  // The-drowned-bell round 3 shipped `assist_windows: []` on a run in which the
  // bot demonstrably died. Empty was the CORRECT reading — spec-0023 takes no
  // assist while the die-retry stage is deliberately dying, and the run never got
  // past that stage — but the artifact could not say so, leaving "per policy"
  // indistinguishable from "never wired". This is what makes it readable.
  const report = new RunReport("the-drowned-bell", "normal");
  report.recordEncounters([
    {
      encounter: ENC.objective,
      wave: ENC.wave,
      tier: ENC.tier,
      assistPolicy: "unassisted-first",
      phaseReached: "die-retry",
      assistWindows: 0,
    },
  ]);
  const json = report.toJSON() as { encounters: Record<string, unknown>[] };
  assert.equal(json["encounters"].length, 1);
  assert.equal(json["encounters"][0]!["assist_policy"], "unassisted-first");
  assert.equal(json["encounters"][0]!["phase_reached"], "die-retry");
  assert.equal(json["encounters"][0]!["assist_windows"], 0);
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
      reengage: {
        present: 3,
        declared: 3,
        carriedOver: 0,
        healthReadable: 3,
        damaged: 0,
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
  assert.equal(re["farthest_blocks"], 61.25, "how far a feral mob strayed is evidence");
  assert.equal(re["settle_ms"], 750);
});
