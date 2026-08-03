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
  respawnPos: [97, 71, -96],
  atCheckpoint: true,
  returned: true,
  reEngaged: true,
  objectivesIntact: true,
  lostObjectives: [],
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
