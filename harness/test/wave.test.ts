import { test } from "node:test";
import assert from "node:assert/strict";
import {
  WAVE_ENGAGE_NEAR,
  WAVE_KILL_NEAR,
  beginWave,
  creditsWaveKill,
  waveEngagementCleared,
} from "../src/wave.ts";

test("a mob the bot engaged, dying near the anchor, is a wave kill", () => {
  const w = beginWave("wave/grave-echoes", [24, 71, -103]);
  w.engaged.add(43);
  assert.equal(creditsWaveKill(w, 43, { x: 24.5, y: 71, z: -103.5 }), true);
});

test("a wave mob killed on the APPROACH counts — the run-13 deadlock", () => {
  // Self-defense killed husk#43 during `wave wave/grave-echoes waypoint 11/12`, i.e.
  // before the kill loop ever ran. The engagement is armed for the whole step, so the
  // defense path records it and the proximity rule credits it; crediting only
  // kill-loop targets leaves `killed` unable to reach `count`, and the step deadlocks.
  const w = beginWave("wave/grave-echoes", [24, 71, -103]);
  w.engaged.add(43); // recorded by defendAgainst, not by the kill loop
  assert.equal(creditsWaveKill(w, 43, { x: 22, y: 71, z: -99 }), true);
});

test("a mob the bot never touched is not a wave kill", () => {
  const w = beginWave("wave/gate-assault", [12, 71, -85]);
  assert.equal(creditsWaveKill(w, 99, { x: 12.5, y: 71, z: -85.5 }), false);
});

test("a mob that winks out far from the anchor is not a confirmed kill", () => {
  // Chunk unload or a far despawn, not a kill the bot can claim.
  const w = beginWave("wave/gate-assault", [12, 71, -85]);
  w.engaged.add(30);
  assert.equal(
    creditsWaveKill(w, 30, { x: 12.5, y: 71, z: -85.5 - (WAVE_KILL_NEAR + 1) }),
    false,
  );
  // …and an entity whose last position is unknown likewise proves nothing.
  assert.equal(creditsWaveKill(w, 30, undefined), false);
});

test("a re-fired entityGone cannot double-count a kill", () => {
  const w = beginWave("wave/gate-assault", [12, 71, -85]);
  w.engaged.add(30);
  const pos = { x: 12.5, y: 71, z: -85.5 };
  assert.equal(creditsWaveKill(w, 30, pos), true);
  w.credited.add(30); // what the executor does on the first credit
  assert.equal(creditsWaveKill(w, 30, pos), false);
});

test("the live-mob exit fires only once everything engaged is down AND nothing is near", () => {
  const down = new Set([1, 2]);
  const isDown = (id: number): boolean => down.has(id);
  // One engaged mob still alive → not cleared, however quiet it looks.
  assert.equal(
    waveEngagementCleared({
      engagedIds: [1, 2, 3],
      isDown,
      nearestEligibleDistance: undefined,
    }),
    false,
  );
  // All engaged down, nothing hostile visible → cleared.
  assert.equal(
    waveEngagementCleared({ engagedIds: [1, 2], isDown, nearestEligibleDistance: undefined }),
    true,
  );
  // All engaged down but something hostile is still in the fight → NOT cleared.
  assert.equal(
    waveEngagementCleared({
      engagedIds: [1, 2],
      isDown,
      nearestEligibleDistance: WAVE_ENGAGE_NEAR - 1,
    }),
    false,
  );
  // A hostile beyond the engagement band is another area's actor, not this fight.
  assert.equal(
    waveEngagementCleared({
      engagedIds: [1, 2],
      isDown,
      nearestEligibleDistance: WAVE_ENGAGE_NEAR + 1,
    }),
    true,
  );
});

test("the live-mob exit can never fire before the bot has fought something", () => {
  assert.equal(
    waveEngagementCleared({
      engagedIds: [],
      isDown: () => true,
      nearestEligibleDistance: undefined,
    }),
    false,
  );
});
