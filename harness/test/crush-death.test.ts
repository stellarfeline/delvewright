import { test } from "node:test";
import assert from "node:assert/strict";

import {
  atSeat,
  crushBinding,
  crushDueAt,
  crushStage,
  crushTrialFailures,
  governingSeats,
  openCrushTrial,
  type CrushTrial,
} from "../src/crush-death.ts";
import type { CrushGate, DeathPlan, Seat } from "../src/death-loop.ts";

/** The Tide Mill's shape: an entry spawn and a checkpoint past each paddle. */
const SEATS: Seat[] = [
  { cp: -1, label: "entry", cell: [0, 64, 40], inForceAfter: -1 },
  { cp: 0, label: "pound-1", cell: [0, 64, 29], inForceAfter: 1 },
  { cp: 1, label: "pound-2", cell: [0, 64, 20], inForceAfter: 2 },
];

/** A gate whose death the build names the boundary after step `trialAt` for. */
function gate(id: string, trialAt: number | undefined): CrushGate {
  return {
    id,
    region: { lo: [0, 64, 33], hi: [2, 66, 33] },
    openTicks: 80,
    closedTicks: 40,
    phase: 0,
    approaches: [{ mouth: [0, 64, 32], stand: [0, 64, 33] }],
    crossedAt: undefined,
    trialAt,
    trialMouth: trialAt === undefined ? undefined : [0, 64, 32],
    gate: { terms: [] },
  };
}

function plan(gates: CrushGate[]): DeathPlan {
  return {
    version: "fixture",
    formatVersion: 6,
    campaignId: "t",
    volumes: [],
    onDeathEffects: 0,
    dropsStake: [],
    stakes: [],
    seats: SEATS,
    regions: [],
    rows: [],
    crushGates: gates,
    binding: {
      volumes: 0,
      onDeathEffects: 0,
      stakes: 0,
      seats: SEATS.length,
      rows: 0,
      crushGates: gates.length,
      unbound: true,
      reason: "no volume",
    },
  };
}

function walked(g: CrushGate, boundary: number, respawnedAtGoverning: boolean): CrushTrial {
  const t = openCrushTrial(g, boundary, `after step ${boundary}`);
  t.enteredRegion = true;
  t.died = true;
  t.credited = true;
  t.respawnedAtGoverning = respawnedAtGoverning;
  return t;
}

test("the governing seat is the latest one in force at the boundary, never a later one", () => {
  assert.deepEqual(governingSeats(SEATS, 0), [0], "before the first paddle: the entry spawn");
  assert.deepEqual(governingSeats(SEATS, 1), [1], "past the first paddle: pound-1");
  assert.deepEqual(governingSeats(SEATS, 9), [2]);
});

test("a trigger-set seat may govern anywhere, beside the latest stepped one", () => {
  const seats: Seat[] = [...SEATS, { cp: 2, label: "struck", cell: [9, 64, 9], inForceAfter: null }];
  assert.deepEqual(governingSeats(seats, 1), [1, 3]);
});

test("a respawn is at a seat within vanilla's offset, and not a block away", () => {
  assert.equal(atSeat([0.5, 64.1, 29.5], SEATS[1]!), true);
  assert.equal(atSeat([0.5, 64, 31.5], SEATS[1]!), false);
});

test("a gate is due at exactly the boundary the build names, and at no other", () => {
  const g = gate("timed-gate/second", 2);
  assert.equal(crushDueAt(g, 1), false, "never before the boundary the build names");
  assert.equal(crushDueAt(g, 2), true);
  assert.equal(crushDueAt(g, 3), false, "never after it either: the seat judged moves with it");
  assert.equal(
    crushDueAt(gate("timed-gate/off-route", undefined), 0),
    false,
    "a gate the build names no boundary for is walked after the path, never at a step",
  );
});

test("a trial walked at a boundary other than the one the build names is red", () => {
  const t = walked(gate("g", 18), 17, true);
  const f = crushTrialFailures(t);
  assert.equal(f.length, 1, JSON.stringify(f));
  assert.match(f[0]!, /boundary 17\), and the build names the boundary after step 18/);
  assert.deepEqual(crushTrialFailures(walked(gate("g", 18), 18, true)), []);
  assert.deepEqual(
    crushTrialFailures(walked(gate("g", undefined), 30, true)),
    [],
    "a gate the build names no boundary for is judged wherever the after-path walk took it",
  );
});

test("a crush that did not kill a body standing in it is red", () => {
  const t = openCrushTrial(gate("g", 0), 0, "after step 0");
  t.enteredRegion = true;
  assert.match(crushTrialFailures(t)[0]!, /did NOT die/);
  const never = openCrushTrial(gate("g", 0), 0, "after step 0");
  assert.match(crushTrialFailures(never)[0]!, /never got its feet/);
});

test("a respawn away from the governing seat is red, and names the seat", () => {
  const t = walked(gate("g", 0), 0, false);
  t.expectedSeats = ["entry [0, 64, 40]"];
  assert.match(crushTrialFailures(t)[0]!, /not at the seat that governs.*entry/);
  assert.deepEqual(crushTrialFailures(walked(gate("g", 0), 0, true)), []);
});

test("the stage counts crush deaths walked over the declared gates", () => {
  const gs = [gate("a", 0), gate("b", 1)];
  const trials = [walked(gs[0]!, 0, true), walked(gs[1]!, 1, true)];
  assert.deepEqual(crushBinding(plan(gs), trials), {
    declared: 2,
    entered: 2,
    deathsWalked: 2,
    respawnsAtGoverning: 2,
    notLive: 0,
  });
  const row = crushStage({
    enabled: true,
    disabledReason: "",
    pathProven: true,
    interruption: undefined,
    plan: plan(gs),
    trials,
  });
  assert.equal(row.passed, true, row.failures.join("; "));
  assert.match(row.findings[0]!, /walked 2 of 2/);
});

test("a declared crush gate no trial walked is red, and so is an empty walk", () => {
  const gs = [gate("a", 1), gate("b", 2)];
  const row = crushStage({
    enabled: true,
    disabledReason: "",
    pathProven: true,
    interruption: undefined,
    plan: plan(gs),
    trials: [],
  });
  assert.equal(row.passed, false);
  assert.ok(row.failures.some((f) => f.includes("a is a declared crush gate")));
  assert.ok(row.failures.some((f) => f.includes("ZERO of 2")));
});

test("a crush trial taken before the path failed is still judged", () => {
  const gs = [gate("a", 0), gate("b", 1)];
  const spared = openCrushTrial(gs[0]!, 0, "after step 0");
  spared.enteredRegion = true;
  const row = crushStage({
    enabled: true,
    disabledReason: "",
    pathProven: false,
    interruption: undefined,
    plan: plan(gs),
    trials: [spared],
  });
  assert.equal(row.ran, true);
  assert.equal(row.passed, false);
  assert.match(row.failures[0]!, /did NOT die/);
  assert.ok(!row.failures.some((f) => f.includes("b is a declared crush gate")));
});
