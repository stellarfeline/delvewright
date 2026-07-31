import { test } from "node:test";
import assert from "node:assert/strict";
import {
  parseWaypoints,
  parseWaypointsJson,
  walkGoals,
  WaypointsParseError,
  WAYPOINT_RANGE,
  type Waypoints,
} from "../src/waypoints.ts";

const VALID = {
  version: "0.4.0",
  campaign_id: "nobodys-cave",
  legs: [
    {
      from: [263, 65, 0],
      to: [197, 69, -20],
      waypoints: [
        [263, 65, 0],
        [253, 65, -4],
        [243, 66, -9],
        [220, 68, -14],
        [200, 69, -19],
        [197, 69, -20],
      ],
    },
    {
      from: [197, 69, -20],
      to: [180, 69, -30],
      waypoints: [
        [197, 69, -20],
        [180, 69, -30],
      ],
    },
  ],
};

test("parseWaypoints accepts a well-formed artifact and indexes legs by destination", () => {
  const wp = parseWaypoints(VALID);
  assert.equal(wp.version, "0.4.0");
  assert.equal(wp.campaignId, "nobodys-cave");
  assert.equal(wp.legs.length, 2);
  // Lookup by the leg's destination anchor (matches a critical-path step `pos`).
  const leg = wp.legTo([197, 69, -20]);
  assert.ok(leg);
  assert.equal(leg.length, 6);
  assert.deepEqual(leg[0], [263, 65, 0]);
  assert.deepEqual(leg[leg.length - 1], [197, 69, -20]);
  // A position with no leg → undefined (caller falls back to single-goal nav).
  assert.equal(wp.legTo([1, 2, 3]), undefined);
});

test("parseWaypointsJson round-trips the on-disk shape", () => {
  const wp = parseWaypointsJson(JSON.stringify(VALID));
  assert.equal(wp.legs.length, 2);
});

test("an unknown version is rejected", () => {
  assert.throws(
    () => parseWaypoints({ ...VALID, version: "9.9.9" }),
    (err: unknown) => err instanceof WaypointsParseError && /version/.test(err.message),
  );
});

test("a malformed waypoint coordinate is rejected with a pointer", () => {
  const bad = {
    ...VALID,
    legs: [{ from: [0, 0, 0], to: [1, 1, 1], waypoints: [[0, 0, "x"]] }],
  };
  assert.throws(
    () => parseWaypoints(bad),
    (err: unknown) =>
      err instanceof WaypointsParseError && err.pointer === "/legs/0/waypoints/0/2",
  );
});

test("an empty waypoints list is rejected (a walked leg has at least its endpoints)", () => {
  const bad = { ...VALID, legs: [{ from: [0, 0, 0], to: [1, 1, 1], waypoints: [] }] };
  assert.throws(
    () => parseWaypoints(bad),
    (err: unknown) => err instanceof WaypointsParseError && /waypoints/.test(err.message),
  );
});

test("walkGoals replays the proven hops then the true destination", () => {
  const wp: Waypoints = parseWaypoints(VALID);
  const goals = walkGoals(wp, [197, 69, -20], 2);
  // 6 waypoint hops (each at WAYPOINT_RANGE) + 1 final destination goal (range 2).
  assert.equal(goals.length, 7);
  for (let i = 0; i < 6; i++) {
    assert.equal(goals[i]!.range, WAYPOINT_RANGE);
  }
  const final = goals[6]!;
  assert.deepEqual([final.x, final.y, final.z], [197, 69, -20]);
  assert.equal(final.range, 2);
});

test("walkGoals falls back to a single destination goal when no leg is known", () => {
  const wp: Waypoints = parseWaypoints(VALID);
  const goals = walkGoals(wp, [999, 64, 999], 3);
  assert.equal(goals.length, 1);
  assert.deepEqual([goals[0]!.x, goals[0]!.y, goals[0]!.z, goals[0]!.range], [999, 64, 999, 3]);
});

test("walkGoals with no waypoints artifact yields the single destination goal", () => {
  const goals = walkGoals(undefined, [10, 65, 20], 2);
  assert.equal(goals.length, 1);
  assert.deepEqual([goals[0]!.x, goals[0]!.y, goals[0]!.z, goals[0]!.range], [10, 65, 20, 2]);
});
