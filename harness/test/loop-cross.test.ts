import { test } from "node:test";
import assert from "node:assert/strict";
import {
  LOOP_BARREN_TRIPS,
  LOOP_MOVE_TICKS,
  LoopCrossWatch,
  LoopNotMovedError,
  LoopWalkBarrenError,
  pastFace,
} from "../src/loop-cross.ts";
import type { Vec3Tuple } from "../src/critical-path.ts";

// The gallery's long hall: the slab at z = 74, crossed toward +z (offset -6).
const CROSS: Vec3Tuple = [517, 65, 74];
const watch = (): LoopCrossWatch => new LoopCrossWatch("loop loop/hall", CROSS, 2, 1, 0, 2);

test("the near face is the slab's low face walking +axis and its high face walking -axis", () => {
  assert.equal(pastFace([517.5, 65, 73.5], CROSS, 2, 1), -0.5);
  assert.equal(pastFace([517.5, 65, 74.25], CROSS, 2, 1), 0.25);
  assert.equal(pastFace([517.5, 65, 75.5], CROSS, 2, -1), -0.5);
  assert.equal(pastFace([517.5, 65, 74.75], CROSS, 2, -1), 0.25);
});

test("a body in the slab is judged not moved after the allowance in game ticks, and not before", () => {
  const w = watch();
  for (let t = 1_000; t < 1_000 + LOOP_MOVE_TICKS; t++) w.reading(t, [517.5, 65, 74.3]);
  assert.throws(() => w.reading(1_000 + LOOP_MOVE_TICKS, [517.5, 65, 74.3]), LoopNotMovedError);
});

test("however many readings answer at one tick, and however late, the stand is not judged on them", () => {
  // A slow server answers many readings within one tick; a stalled host answers
  // none for a long while. Neither is a game tick, and the watch reads no clock.
  const w = watch();
  for (let i = 0; i < 10_000; i++) w.reading(1_000, [517.5, 65, 74.3]);
  w.reading(1_000 + LOOP_MOVE_TICKS - 1, [517.5, 65, 74.3]);
  assert.throws(() => w.reading(1_000 + LOOP_MOVE_TICKS, [517.5, 65, 74.3]), LoopNotMovedError);
});

test("a body short of the slab is never judged by the loop, and a stand restarts when it steps back", () => {
  const w = watch();
  for (let t = 0; t < 10 * LOOP_MOVE_TICKS; t++) w.reading(t, [517.5, 65, 70.5]);
  for (let t = 1_000; t < 1_000 + LOOP_MOVE_TICKS; t++) w.reading(t, [517.5, 65, 74.3]);
  w.reading(1_000 + LOOP_MOVE_TICKS, [517.5, 65, 73.9]);
  w.reading(1_001 + LOOP_MOVE_TICKS, [517.5, 65, 74.3]);
});

test("a trip ended short is walked again; one ended at the slab is not", () => {
  const w = watch();
  w.tripBegins([517.5, 65, 68.5]);
  assert.equal(w.tripEnded([517.5, 65, 69.4], "the pathfinder resolved"), true);
  w.tripBegins([517.5, 65, 69.4]);
  assert.equal(w.tripEnded([517.5, 65, 74.1], "the pathfinder resolved"), false);
});

test("trips that end short without advancing fail after their count; one that advances resets it", () => {
  const w = watch();
  for (let i = 1; i < LOOP_BARREN_TRIPS; i++) {
    w.tripBegins([517.5, 65, 68.5]);
    assert.equal(w.tripEnded([517.5, 65, 68.5], "No path to the goal!"), true);
  }
  // An advancing trip: the count starts again.
  w.tripBegins([517.5, 65, 68.5]);
  assert.equal(w.tripEnded([517.5, 65, 70.5], "the pathfinder resolved"), true);
  for (let i = 1; i < LOOP_BARREN_TRIPS; i++) {
    w.tripBegins([517.5, 65, 70.5]);
    assert.equal(w.tripEnded([517.5, 65, 70.5], "No path to the goal!"), true);
  }
  w.tripBegins([517.5, 65, 70.5]);
  assert.throws(() => w.tripEnded([517.5, 65, 70.5], "No path to the goal!"), LoopWalkBarrenError);
});
