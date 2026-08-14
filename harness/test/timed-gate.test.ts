// spec-0016 §4 timed-gate timing/geometry helpers.

import test from "node:test";
import assert from "node:assert/strict";

import {
  GATE_ENTRY_LATENCY_MS,
  GATE_RETRY_MARGIN_MS,
  TICKS_PER_SECOND,
  WALK_MS_PER_BLOCK,
  crossingEstimateMs,
  cycleMs,
  cycleTicks,
  describeGates,
  gateRegionCells,
  gateRetryBudgetMs,
  gateWindowWaitMs,
  gatesCrossedByHop,
  hopCrossesGate,
  insideGate,
  maxCycleMs,
  nearCell,
  needsStandoff,
  openMs,
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
  crush: false,
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

// --- crossing detection + window-margin arithmetic ----------------------------
//
// The tide-mill `timed-gate/tide` (36t open / 84t closed, phase 55, crush) killed
// the bot because the harness only engaged its gate machinery AFTER a hop failed —
// blind first entry, which a crushing close makes lethal. Staging needs two pure
// facts: does THIS hop's straight mouth-to-mouth segment cross the gate, and does
// the crossing fit an open window with margin.

/** The tide-mill crusher, as the live run exported it (short window, phase offset). */
const tide: TimedGate = {
  id: "timed-gate/tide",
  min: [258, 61, 13],
  max: [262, 63, 14],
  block: "minecraft:polished_deepslate",
  openTicks: 36,
  closedTicks: 84,
  phase: 55,
  crush: true,
};

test("a straight mouth-to-mouth hop through the region IS the crossing", () => {
  // Flanking mouth cells (compiler-pinned): one before the region, one after.
  assert.ok(hopCrossesGate([260, 61, 12], [260, 61, 15], tide));
  // The whole approach leg toward the far anchor also pierces the region.
  assert.ok(hopCrossesGate([261, 61, 4], [260, 61, 24], tide));
  // A hop that stays on one side never crosses — no staging licence.
  assert.ok(!hopCrossesGate([260, 61, 4], [260, 61, 12], tide));
  assert.ok(!hopCrossesGate([260, 61, 15], [260, 61, 24], tide));
  // Body occupancy counts: walking UNDER a head-height fill is a crossing.
  const overhead: TimedGate = { ...tide, min: [258, 62, 13], max: [262, 63, 14] };
  assert.ok(hopCrossesGate([260, 61, 12], [260, 61, 15], overhead));
  // …but two blocks of clearance above the walker is not.
  const high: TimedGate = { ...tide, min: [258, 63, 13], max: [262, 64, 14] };
  assert.ok(!hopCrossesGate([260, 61, 12], [260, 61, 15], high));
});

test("gatesCrossedByHop filters to the gates the segment pierces, in order", () => {
  assert.deepEqual(gatesCrossedByHop([260, 61, 12], [260, 61, 15], [portcullis, tide]), [tide]);
  assert.deepEqual(gatesCrossedByHop([260, 61, 4], [260, 61, 12], [portcullis, tide]), []);
  // An unknown origin grants no staging licence (the reactive path still covers it).
  assert.deepEqual(gatesCrossedByHop(undefined, [260, 61, 15], [tide]), []);
});

test("nearCell is the range-1 arrival tolerance — a drifted bot is off-station", () => {
  // At the mouth, and the legal range-1 arrivals around it.
  assert.ok(nearCell([260, 61, 12], [260, 61, 12]));
  assert.ok(nearCell([261, 61, 11], [260, 61, 12]));
  assert.ok(nearCell([260, 62, 12], [260, 61, 12]));
  // The live tide-mill drift: the current carried the idle bot to the pool.
  assert.ok(!nearCell([260, 61, 4], [260, 61, 12]));
  // An unknown position is not a station.
  assert.ok(!nearCell(undefined, [260, 61, 12]));
});

test("the crossing estimate is entry latency plus distance at a conservative walk", () => {
  assert.equal(
    crossingEstimateMs([260, 61, 12], [260, 61, 15]),
    Math.ceil(GATE_ENTRY_LATENCY_MS + 3 * WALK_MS_PER_BLOCK),
  );
  // The tide gate's designed crossing (mouth-to-mouth, ~3 blocks) fits its 1.8s
  // window with margin — the harness must PASS the margin check the design earns.
  assert.equal(openMs(tide), 1_800);
  assert.ok(
    crossingEstimateMs([260, 61, 12], [260, 61, 15]) < openMs(tide),
    "the designed 36t window admits the staged crossing",
  );
});
