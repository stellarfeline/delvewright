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

test("the ladder has exactly the two labelled stages spec-0023 names", () => {
  assert.deepEqual([...STAGES], ["critical-path", "die-retry"]);
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
