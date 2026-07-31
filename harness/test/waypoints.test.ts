import { test } from "node:test";
import assert from "node:assert/strict";
import {
  parseWaypoints,
  parseWaypointsJson,
  nextLegWaypoints,
  walkGoals,
  WaypointsParseError,
  WAYPOINT_RANGE,
  type Waypoints,
} from "../src/waypoints.ts";

// A cave that VISITS [197, 69, -20] twice and RETURNS to the entry [262, 66, 1] —
// exactly the nobodys-cave shape whose duplicate destinations broke a by-coordinate
// lookup. Legs are in critical-path order; order (not coordinate) is authoritative.
const VALID = {
  version: "0.4.0",
  campaign_id: "nobodys-cave",
  legs: [
    {
      from: [262, 66, 1],
      to: [197, 69, -20],
      waypoints: [
        [262, 66, 1],
        [243, 66, -9],
        [200, 69, -19],
        [197, 69, -20],
      ],
    },
    {
      from: [197, 69, -20],
      to: [197, 69, 2],
      waypoints: [
        [197, 69, -20],
        [197, 69, 2],
      ],
    },
    // A SECOND leg ending at [197, 69, -20] (revisited), from a different origin.
    {
      from: [197, 69, 2],
      to: [197, 69, -20],
      waypoints: [
        [197, 69, 2],
        [197, 69, -20],
      ],
    },
    // The RETURN leg to the entry [262, 66, 1] — the one a by-`to` map wrongly
    // grabbed for the early post-transport step.
    {
      from: [244, 65, -5],
      to: [262, 66, 1],
      waypoints: [
        [244, 65, -5],
        [255, 66, -1],
        [262, 66, 1],
      ],
    },
  ],
};

test("parseWaypoints accepts a well-formed artifact and preserves leg order", () => {
  const wp = parseWaypoints(VALID);
  assert.equal(wp.version, "0.4.0");
  assert.equal(wp.campaignId, "nobodys-cave");
  assert.equal(wp.legs.length, 4);
  assert.deepEqual(wp.legs[0]!.from, [262, 66, 1]);
  assert.deepEqual(wp.legs[0]!.to, [197, 69, -20]);
  assert.deepEqual(wp.legs[3]!.to, [262, 66, 1]);
});

test("parseWaypointsJson round-trips the on-disk shape", () => {
  const wp = parseWaypointsJson(JSON.stringify(VALID));
  assert.equal(wp.legs.length, 4);
});

test("nextLegWaypoints consumes legs in lockstep, immune to duplicate destinations", () => {
  const wp = parseWaypoints(VALID);
  let cursor = 0;

  // A post-transport step whose target is the entry [262, 66, 1] must NOT grab the
  // return leg (legs[3]); the cursor is at legs[0] (to [197,69,-20]) so it does not
  // match → fallback, cursor unchanged. (This was the real stranding bug.)
  let m = nextLegWaypoints(wp.legs, cursor, [262, 66, 1]);
  assert.equal(m.waypoints, undefined);
  assert.equal(m.cursor, 0);
  cursor = m.cursor;

  // Reaching the cavern [197,69,-20] consumes legs[0] (the entry→cavern route).
  m = nextLegWaypoints(wp.legs, cursor, [197, 69, -20]);
  assert.ok(m.waypoints);
  assert.deepEqual(m.waypoints[0], [262, 66, 1]);
  assert.equal(m.cursor, 1);
  cursor = m.cursor;

  // Next legs consume in order.
  m = nextLegWaypoints(wp.legs, cursor, [197, 69, 2]);
  assert.ok(m.waypoints);
  assert.equal(m.cursor, 2);
  cursor = m.cursor;

  // The SECOND visit to [197,69,-20] consumes legs[2] (from [197,69,2]), not legs[0].
  m = nextLegWaypoints(wp.legs, cursor, [197, 69, -20]);
  assert.ok(m.waypoints);
  assert.deepEqual(m.waypoints[0], [197, 69, 2]);
  assert.equal(m.cursor, 3);
  cursor = m.cursor;

  // Finally the RETURN to the entry consumes legs[3].
  m = nextLegWaypoints(wp.legs, cursor, [262, 66, 1]);
  assert.ok(m.waypoints);
  assert.deepEqual(m.waypoints[0], [244, 65, -5]);
  assert.equal(m.cursor, 4);
});

test("nextLegWaypoints does not consume on a sub-walk to an unrelated position", () => {
  const wp = parseWaypoints(VALID);
  // A mob-chase sub-walk (not the next leg's destination) leaves the cursor put.
  const m = nextLegWaypoints(wp.legs, 1, [123, 64, 45]);
  assert.equal(m.waypoints, undefined);
  assert.equal(m.cursor, 1);
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
  const leg = wp.legs[0]!.waypoints; // 4 hops → 4 waypoint goals + 1 final goal
  const goals = walkGoals(leg, [197, 69, -20], 2);
  assert.equal(goals.length, 5);
  for (let i = 0; i < 4; i++) {
    assert.equal(goals[i]!.range, WAYPOINT_RANGE);
  }
  const final = goals[4]!;
  assert.deepEqual([final.x, final.y, final.z], [197, 69, -20]);
  assert.equal(final.range, 2);
});

test("walkGoals falls back to a single destination goal when no leg matched", () => {
  const goals = walkGoals(undefined, [999, 64, 999], 3);
  assert.equal(goals.length, 1);
  assert.deepEqual([goals[0]!.x, goals[0]!.y, goals[0]!.z, goals[0]!.range], [999, 64, 999, 3]);
});
