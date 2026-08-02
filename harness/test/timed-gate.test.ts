// spec-0016 §4 timed-gate timing/geometry helpers (task #81).

import test from "node:test";
import assert from "node:assert/strict";

import {
  GATE_RETRY_MARGIN_MS,
  TICKS_PER_SECOND,
  cycleMs,
  cycleTicks,
  describeGates,
  gateRegionCells,
  gateRetryBudgetMs,
  gateWindowWaitMs,
  insideGate,
  maxCycleMs,
  needsStandoff,
} from "../src/timed-gate.ts";
import type { TimedGate } from "../src/waypoints.ts";

/** The-drowned-bell's portcullis: a 5×3×1 iron-bars wall on a 100/100 clock. */
const portcullis: TimedGate = {
  id: "timed-gate/portcullis",
  min: [22, 63, -10],
  max: [26, 65, -10],
  block: "minecraft:iron_bars",
  openTicks: 100,
  closedTicks: 100,
  phase: 0,
};

test("a gate's cycle is its two halves, in ticks and milliseconds", () => {
  assert.equal(cycleTicks(portcullis), 200);
  assert.equal(cycleMs(portcullis), (200 / TICKS_PER_SECOND) * 1_000);
  assert.equal(cycleMs(portcullis), 10_000);
});

test("the retry budget covers strictly more than two full cycles", () => {
  const budget = gateRetryBudgetMs([portcullis]);
  assert.equal(budget, 2 * 10_000 + GATE_RETRY_MARGIN_MS);
  assert.ok(
    budget > 2 * cycleMs(portcullis),
    "two cycles guarantee one attempt that began at the TOP of a window",
  );
  // The slowest gate governs when a leg crosses several.
  const slow: TimedGate = { ...portcullis, id: "timed-gate/slow", openTicks: 200, closedTicks: 400 };
  assert.equal(maxCycleMs([portcullis, slow]), 30_000);
  assert.equal(gateRetryBudgetMs([portcullis, slow]), 60_000 + GATE_RETRY_MARGIN_MS);
  // No gate ⇒ no budget: an unmarked leg is never granted retries.
  assert.equal(gateRetryBudgetMs([]), 0);
});

test("the per-attempt window wait is bounded by one full cycle plus margin", () => {
  // One cycle is enough to observe the closed→open edge; the cap only exists so an
  // unreadable region can never hang the run.
  assert.equal(gateWindowWaitMs([portcullis]), 10_000 + GATE_RETRY_MARGIN_MS);
  assert.equal(gateWindowWaitMs([]), 0);
});

test("the region enumerates every declared cell, inclusive of both corners", () => {
  const cells = gateRegionCells(portcullis);
  assert.equal(cells.length, 5 * 3 * 1);
  assert.deepEqual(cells[0], [22, 63, -10]);
  assert.deepEqual(cells[cells.length - 1], [26, 65, -10]);
  // Deterministic order (x, then y, then z) — the same list every run.
  assert.deepEqual(cells, gateRegionCells(portcullis));
});

test("a standoff is required only when the bot stands IN the fill", () => {
  assert.ok(insideGate([24, 63, -10], portcullis), "feet inside the region");
  // A gate whose fill is head-height only still catches a walker standing under it.
  const overhead: TimedGate = { ...portcullis, min: [22, 64, -10], max: [26, 65, -10] };
  assert.ok(insideGate([24, 63, -10], overhead), "head cell inside the region");
  assert.ok(!insideGate([24, 62, -10], overhead), "two blocks of clearance under it");
  // One block clear of the fill: the fill is atomic and lands nowhere outside the
  // region, so this is the IDEAL place to wait — retreating from it would only add
  // blocks that must be re-walked inside the open window.
  assert.ok(!insideGate([24, 63, -9], portcullis));
  assert.ok(!needsStandoff([24, 63, -9], [portcullis]));
  assert.ok(needsStandoff([24, 63, -10], [portcullis]));
  // The observed failure position from the live ladder run: bot at [24.5, 63, -8.4]
  // (feet cell [24, 63, -9]) — already clear, so it waits where it stands rather than
  // walking 18 blocks back to the previous waypoint.
  assert.ok(!needsStandoff([24, 63, -9], [portcullis]));
});

test("an unknown position is conservatively treated as unsafe", () => {
  assert.ok(needsStandoff(undefined, [portcullis]));
  // …but with no gate there is nothing to stand off from.
  assert.ok(!needsStandoff(undefined, []));
});

test("the failure description names the gate and its cycle", () => {
  const text = describeGates([portcullis]);
  assert.match(text, /timed-gate\/portcullis/);
  assert.match(text, /100t open \/ 100t closed/);
  assert.match(text, /200t cycle/);
});
