import { test } from "node:test";
import assert from "node:assert/strict";
import {
  ATTRIBUTION_RANGE,
  attributeBotDamage,
  type ThreatCandidate,
} from "../src/threat.ts";

test("damage is attributed to the server-named source when it is visible", () => {
  const candidates: ThreatCandidate[] = [
    { id: 20, distance: 9 }, // the shooter, far away
    { id: 21, distance: 1 }, // an innocent bystander in melee range
  ];
  assert.equal(attributeBotDamage(20, candidates), 20);
});

test("damage with no named source falls back to the nearest hostile in reach", () => {
  const candidates: ThreatCandidate[] = [
    { id: 30, distance: 3.5 },
    { id: 31, distance: 1.2 },
  ];
  assert.equal(attributeBotDamage(undefined, candidates), 31);
  // A source id the client cannot see is not usable; the fallback still applies.
  assert.equal(attributeBotDamage(999, candidates), 31);
});

test("a hazard with no hostile in reach blames nobody", () => {
  // Fall damage / a trap / drowning: the nearest mob is well out of melee reach, so
  // NOTHING is recorded — the bot must never swing at a bystander for a hazard.
  const candidates: ThreatCandidate[] = [{ id: 40, distance: ATTRIBUTION_RANGE + 0.5 }];
  assert.equal(attributeBotDamage(undefined, candidates), undefined);
  assert.equal(attributeBotDamage(undefined, []), undefined);
});
