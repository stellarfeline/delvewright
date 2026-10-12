// The one route rule: a proven leg read as the polyline through its corners.

import { test } from "node:test";
import assert from "node:assert/strict";
import { nearestInnerCell, nearestOnRoute, routeIndexAfter, straightRunStep } from "../src/route.ts";

const leg: [number, number, number][] = [
  [0, 64, 0],
  [20, 64, 0],
  [20, 64, 10],
];

test("the nearest point lies on the segment, not at its nearest corner", () => {
  const at = nearestOnRoute([6, 64, 3], leg)!;
  assert.equal(at.segment, 0);
  assert.equal(at.t, 0.3);
  assert.deepEqual(at.point, [6, 64, 0]);
  assert.equal(at.distSq, 9);
  assert.equal(nearestOnRoute([0, 0, 0], []), undefined);
});

test("a station beside a straight run resumes from the corner after it, never the one behind", () => {
  // Nearer the run's start than its end: the nearest corner is behind it.
  assert.equal(routeIndexAfter([6, 64, 1], leg), 1);
  assert.equal(routeIndexAfter([20, 64, 4], leg), 2);
  // On a corner: that corner.
  assert.equal(routeIndexAfter([0, 64, 0], leg), 0);
  assert.equal(routeIndexAfter([1, 1, 1], []), 0);
});

test("only a straight run of equal steps has known inner cells", () => {
  assert.deepEqual(straightRunStep([0, 64, 0], [5, 64, 5]), [1, 0, 1]);
  assert.deepEqual(straightRunStep([0, 64, 0], [3, 61, 0]), [1, -1, 0]);
  assert.equal(straightRunStep([0, 64, 0], [5, 64, 2]), undefined);
  assert.equal(straightRunStep([0, 64, 0], [0, 64, 0]), undefined);
  assert.deepEqual(nearestInnerCell([10, 64, 4], leg), { cell: [10, 64, 0], index: 1, distSq: 16 });
  assert.equal(nearestInnerCell([10, 64, 4], [[0, 64, 0], [20, 64, 3]]), undefined, "not one straight run");
  assert.equal(nearestInnerCell([0, 64, 1], leg), undefined, "a corner is nearest");
});
