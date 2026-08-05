import { test } from "node:test";
import assert from "node:assert/strict";
import { hasSettled } from "../src/entity-settle.ts";

test("an empty history is never settled", () => {
  assert.equal(hasSettled([]), false);
});

test("fewer polls than the stability window is never settled, however high the count", () => {
  assert.equal(hasSettled([2, 2]), false); // default stablePolls = 3
});

test("an all-zero history never settles — the island bug: an empty tracker is not stable", () => {
  assert.equal(hasSettled([0, 0, 0, 0]), false);
});

test("three consecutive equal non-zero polls settle", () => {
  assert.equal(hasSettled([0, 1, 2, 2, 2]), true);
});

test("still climbing (late-arriving packets) does not settle", () => {
  // Each poll sees one more entity than the last — packets are still landing.
  assert.equal(hasSettled([0, 1, 2, 3]), false);
});

test("a custom policy's stability window is honoured", () => {
  assert.equal(hasSettled([5], { stablePolls: 1 }), true);
  assert.equal(hasSettled([5, 5], { stablePolls: 3 }), false);
});

test("only the trailing window matters — an early wobble does not block a later settle", () => {
  assert.equal(hasSettled([0, 1, 3, 3, 3]), true);
});
