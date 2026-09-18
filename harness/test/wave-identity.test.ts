import { test } from "node:test";
import assert from "node:assert/strict";
import { CENSUS_MATCH_RADIUS, isWaveBody, pickWaveBody } from "../src/wave.ts";

const WATCHMAN: readonly [number, number, number] = [90.7, 80, 166.8];

test("a body is the wave's only where the census last stood one", () => {
  const census = [{ pos: WATCHMAN }];
  assert.equal(isWaveBody({ pos: [91.2, 80, 167.5], census }), true);
  // The stable's horse puppet, thirty blocks off: a living body, not the wave.
  assert.equal(isWaveBody({ pos: [31.5, 80, 133.5], census }), false);
  assert.equal(
    isWaveBody({ pos: [WATCHMAN[0] + CENSUS_MATCH_RADIUS + 0.1, 80, WATCHMAN[2]], census }),
    false,
  );
  // A census that answered with nobody standing matches nothing.
  assert.equal(isWaveBody({ pos: WATCHMAN, census: [] }), false);
  // No census yet: position does not decide.
  assert.equal(isWaveBody({ pos: [0, 0, 0], census: undefined }), true);
});

test("of the census bodies, the nearest of a kind the wave seats is hunted first", () => {
  const kinds = new Set(["skeleton"]);
  const horse = { kind: "skeleton_horse", distance: 1 };
  const watchman = { kind: "skeleton", distance: 3 };
  assert.equal(pickWaveBody([horse, watchman], kinds), watchman);
  // A wave body that changed kind (a zombie that drowned) is still the census's.
  assert.equal(pickWaveBody([{ kind: "drowned", distance: 2 }], new Set(["zombie"]))?.kind, "drowned");
  assert.equal(pickWaveBody([], kinds), undefined);
});
