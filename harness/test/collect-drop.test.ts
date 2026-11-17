import { test } from "node:test";
import assert from "node:assert/strict";
import { nearestDropOf, type SeenDrop } from "../src/executor/collect.ts";
import { WAVE_ENGAGE_NEAR } from "../src/wave.ts";

const ANCHOR: [number, number, number] = [517, 65, 140];

function drop(id: number, x: number, y: number, z: number, item = "feather"): SeenDrop {
  return { id, position: { x, y, z }, item };
}

test("a drop beside the anchor is found from the anchor", () => {
  const found = nearestDropOf([drop(1, 519, 65, 148)], "feather", ANCHOR, [], WAVE_ENGAGE_NEAR);
  assert.equal(found?.id, 1);
  assert.equal(found?.centre, 0);
});

test("a body that fled past the anchor's reach is found where the server last read it standing", () => {
  // The gallery's hen, chipped in a die-retry trial and left alone through the
  // walk back, was struck 38.5 blocks from its anchor: its feather lay outside
  // every radius drawn around the anchor.
  const fled: [number, number, number] = [517.3, 65, 178.4];
  const feather = drop(7, 517.6, 65.2, 178.9);
  assert.equal(
    nearestDropOf([feather], "feather", ANCHOR, [], WAVE_ENGAGE_NEAR),
    undefined,
    "the anchor alone cannot see a drop the body carried away",
  );
  const found = nearestDropOf([feather], "feather", ANCHOR, [fled], WAVE_ENGAGE_NEAR);
  assert.equal(found?.id, 7);
  assert.equal(found?.centre, 1);
  assert.ok(found!.from < 1, `measured from where the body stood: ${found!.from}`);
});

test("another item, or the same item far from every centre, is not the fight's drop", () => {
  const found = nearestDropOf(
    [drop(1, 519, 65, 148, "string"), drop(2, 600, 65, 300)],
    "feather",
    ANCHOR,
    [[517, 65, 178]],
    WAVE_ENGAGE_NEAR,
  );
  assert.equal(found, undefined);
});

test("of two drops the nearer to any centre is chosen", () => {
  const found = nearestDropOf(
    [drop(1, 517, 65, 160), drop(2, 517.5, 65, 178.5)],
    "feather",
    ANCHOR,
    [[517.5, 65, 178.5]],
    WAVE_ENGAGE_NEAR,
  );
  assert.equal(found?.id, 2);
});
