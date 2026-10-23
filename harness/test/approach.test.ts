// An `approach` trigger is fired by where the walk leaves the bot.
//
// The tick tests `positioned <anchor> if entity @a[distance=..range]`, measured
// from the middle of the anchor cell's floor. The walk's goal accepts any cell
// within the goal range of the anchor cell, and the body stands anywhere inside
// it; every such stop must be inside the trigger's range, for every legal range.

import { test } from "node:test";
import assert from "node:assert/strict";

import { approachGoalRange } from "../src/executor/trigger.ts";

/** The farthest a body standing in a goal cell can be from the server's point. */
function worstStop(k: number): number {
  let worst = 0;
  for (let dx = -k; dx <= k; dx++) {
    for (let dy = -k; dy <= k; dy++) {
      for (let dz = -k; dz <= k; dz++) {
        if (dx * dx + dy * dy + dz * dz > k * k) continue; // outside GoalNear(k)
        // The cell spans [d, d + 1) on x and z; the point is at 0.5 on both.
        for (const fx of [dx, dx + 1]) {
          for (const fz of [dz, dz + 1]) {
            worst = Math.max(worst, Math.hypot(fx - 0.5, dy, fz - 0.5));
          }
        }
      }
    }
  }
  return worst;
}

test("every legal range: no cell the walk accepts leaves the bot outside the trigger", () => {
  let ranges = 0;
  for (let range = 1; range <= 32; range++) {
    const k = approachGoalRange(range);
    assert.ok(Number.isInteger(k) && k >= 0, `range ${range}: goal ${k}`);
    assert.ok(worstStop(k) <= range, `range ${range}: goal ${k} lets the bot stop ${worstStop(k).toFixed(2)} out`);
    ranges++;
  }
  assert.equal(ranges, 32);
});

test("range 1 is a block goal on the anchor cell", () => {
  // The demo's door: the walk to one block out stopped 1.1 from the point.
  assert.equal(approachGoalRange(1), 0);
  assert.ok(worstStop(1) > 1, "one block out is outside a range-1 trigger");
});

test("the goal is the widest that still fits", () => {
  for (let range = 1; range <= 32; range++) {
    const k = approachGoalRange(range);
    assert.ok(worstStop(k + 1) > range, `range ${range}: goal ${k + 1} would also fit`);
  }
});
