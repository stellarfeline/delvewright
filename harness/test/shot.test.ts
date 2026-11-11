import { test } from "node:test";
import assert from "node:assert/strict";

import { SHOT_ELEVATIONS_AT_27, shotAim, shotSpent } from "../src/shot.ts";

test("the aim rises over the centre by the record's elevation, scaled with distance", () => {
  const eye: [number, number, number] = [0.5, 65.62, 0.5];
  const at27 = shotAim(eye, [27, 65, 0], 0)!;
  assert.equal(at27[0], 27.5);
  assert.ok(Math.abs(at27[1] - (65.5 + 2.0)) < 1e-9, `${at27[1]}`);
  const at13 = shotAim(eye, [13, 65, 0], 0)!;
  assert.ok(Math.abs(at13[1] - (65.5 + (2.0 * 13) / 27)) < 1e-9, "less distance, less rise");
});

test("a miss re-aims from the table, and the step fails by name once it is spent", () => {
  const eye: [number, number, number] = [0.5, 65.62, 0.5];
  const ys = SHOT_ELEVATIONS_AT_27.map((_, k) => shotAim(eye, [27, 65, 0], k)![1]);
  assert.deepEqual(ys.map((y) => Math.round((y - 65.5) * 10) / 10), [2.0, 1.5, 2.5]);
  assert.equal(shotAim(eye, [27, 65, 0], SHOT_ELEVATIONS_AT_27.length), undefined);
  assert.match(shotSpent("trigger trigger/t (shot)", 3), /no arrow of 3 landed.*table.*is spent/);
});
