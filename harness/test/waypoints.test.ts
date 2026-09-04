import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import {
  branchWaypointsFileFor,
  loadWaypointsForBranchPath,
  parseWaypoints,
  parseWaypointsJson,
  nextLegWaypoints,
  retainStandableWaypoints,
  walkGoals,
  WaypointsParseError,
  WAYPOINT_RANGE,
  type Waypoints,
} from "../src/waypoints.ts";
import type { Vec3Tuple } from "../src/critical-path.ts";

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

test("retainStandableWaypoints drops a fence-top proven cell, keeps the rest in order", () => {
  // The ram-pen leg (nobodys-cave-island): the compiler routed the player OVER the
  // oak_fence at [17, 78, -63] because its full-solid model treats a fence as a
  // stand-on-able 1×1×1 cube. Vanilla physics cannot climb a 1.5-tall fence, so this
  // cell is un-standable and must be dropped; the level cells on either side stay.
  const leg: Vec3Tuple[] = [
    [17, 77, -62],
    [17, 78, -63], // stands on the oak_fence → un-standable, dropped
    [17, 77, -64],
    [17, 77, -65],
    [17, 77, -66],
    [18, 77, -66],
  ];
  // Support of the fence-top cell ([17, 77, -63]) is the fence; every other cell
  // stands on stone.
  const fenceTopped = new Set(["17,77,-63"]);
  const kept = retainStandableWaypoints(
    leg,
    (c) => !fenceTopped.has(`${c[0]},${c[1] - 1},${c[2]}`),
  );
  assert.deepEqual(kept, [
    [17, 77, -62],
    [17, 77, -64],
    [17, 77, -65],
    [17, 77, -66],
    [18, 77, -66],
  ]);
});

test("retainStandableWaypoints is identity when every cell is standable", () => {
  const leg: Vec3Tuple[] = [
    [0, 64, 0],
    [1, 64, 0],
    [2, 65, 0],
  ];
  const kept = retainStandableWaypoints(leg, () => true);
  assert.deepEqual(kept, leg);
});

// --- spec-0016 §4 timed gates ------------------------------------

/** The-drowned-bell shape: a straight leg whose proven route runs through a
 * portcullis on a 100/100 clock, plus a gate-free leg beside it. */
const GATED = {
  version: "0.6.0",
  campaign_id: "the-drowned-bell",
  timed_gates: [
    {
      id: "timed-gate/portcullis",
      region: { min: [22, 63, -10], max: [26, 65, -10] },
      block: "minecraft:iron_bars",
      open_ticks: 100,
      closed_ticks: 100,
      phase: 0,
    },
  ],
  legs: [
    {
      from: [24, 63, 4],
      to: [24, 63, -14],
      waypoints: [
        [24, 63, 4],
        [24, 63, -14],
      ],
      timed_gates: ["timed-gate/portcullis"],
    },
    {
      from: [24, 63, -14],
      to: [24, 71, -37],
      waypoints: [
        [24, 63, -14],
        [24, 71, -37],
      ],
    },
  ],
};

test("a leg's timed gates are resolved against the declared table", () => {
  const wp = parseWaypoints(GATED);
  assert.equal(wp.timedGates.length, 1);
  const gate = wp.timedGates[0]!;
  assert.equal(gate.id, "timed-gate/portcullis");
  assert.deepEqual(gate.min, [22, 63, -10]);
  assert.deepEqual(gate.max, [26, 65, -10]);
  assert.equal(gate.openTicks, 100);
  assert.equal(gate.closedTicks, 100);
  assert.equal(gate.phase, 0);
  // `crush` absent (every artifact that predates the field) means the gate merely
  // blocks — the staged-entry discipline is reserved for gates that KILL.
  assert.equal(gate.crush, false);
  // The crossing leg carries the resolved gate; the leg beside it carries none, so
  // it can never claim the gate's licence to retry.
  assert.deepEqual(wp.legs[0]!.timedGates, [gate]);
  assert.deepEqual(wp.legs[1]!.timedGates, []);
});

test("a gate exporting crush: true parses as lethal", () => {
  const crushing = {
    ...GATED,
    timed_gates: [{ ...GATED.timed_gates[0], crush: true }],
  };
  const wp = parseWaypoints(crushing);
  assert.equal(wp.timedGates[0]!.crush, true);
});

test("a non-boolean crush is a structural fault, never coerced", () => {
  // Silently coercing (e.g. the string "false" → truthy) could blind-enter a
  // lethal gate — refuse the artifact instead.
  const bad = {
    ...GATED,
    timed_gates: [{ ...GATED.timed_gates[0], crush: "yes" }],
  };
  assert.throws(
    () => parseWaypoints(bad),
    (err: unknown) =>
      err instanceof WaypointsParseError && err.pointer === "/timed_gates/0/crush",
  );
});

test("nextLegWaypoints surfaces the matched leg's gates, and none when unmatched", () => {
  const wp = parseWaypoints(GATED);
  const hit = nextLegWaypoints(wp.legs, 0, [24, 63, -14]);
  assert.equal(hit.timedGates.length, 1);
  assert.equal(hit.cursor, 1);
  assert.equal(hit.matched, true);
  // A sub-walk that matches no leg surfaces no LEG gates — there is no proven route
  // to read them off. Which gates then bind that walk is `gatesBindingWalk`'s
  // question, and `matched` is what lets it tell this from a proven route that
  // crosses nothing.
  const miss = nextLegWaypoints(wp.legs, 0, [99, 63, 0]);
  assert.equal(miss.matched, false);
  assert.deepEqual(miss.timedGates, []);
  assert.equal(miss.cursor, 0);
});

test("an artifact with no timed_gates table parses with empty gate sets", () => {
  const wp = parseWaypoints(VALID);
  assert.deepEqual(wp.timedGates, []);
  for (const leg of wp.legs) assert.deepEqual(leg.timedGates, []);
});

test("a leg naming an undeclared gate is rejected with a pointer", () => {
  const bad = {
    ...GATED,
    legs: [{ ...GATED.legs[0], timed_gates: ["timed-gate/ghost"] }, GATED.legs[1]],
  };
  assert.throws(
    () => parseWaypoints(bad),
    (err: unknown) =>
      err instanceof WaypointsParseError && err.pointer === "/legs/0/timed_gates/0",
  );
});

test("a gate with a zero half-cycle is rejected (a clock, not a static gate)", () => {
  const bad = {
    ...GATED,
    timed_gates: [{ ...GATED.timed_gates[0], open_ticks: 0 }],
  };
  assert.throws(
    () => parseWaypoints(bad),
    (err: unknown) =>
      err instanceof WaypointsParseError && err.pointer === "/timed_gates/0",
  );
});

test("a gate region whose min exceeds its max is rejected", () => {
  const bad = {
    ...GATED,
    timed_gates: [
      { ...GATED.timed_gates[0], region: { min: [26, 63, -10], max: [22, 65, -10] } },
    ],
  };
  assert.throws(
    () => parseWaypoints(bad),
    (err: unknown) =>
      err instanceof WaypointsParseError && err.pointer === "/timed_gates/0/region",
  );
});

// ---------------------------------------------------------------------------
// Per-branch waypoints
// ---------------------------------------------------------------------------

test("the per-branch waypoints file is derived from the branch path file", () => {
  assert.equal(
    branchWaypointsFileFor("/delve/validation/branch-path-flee.json"),
    "/delve/validation/branch-waypoints-flee.json",
  );
  // A multi-point product slug survives the derivation untouched.
  assert.equal(
    branchWaypointsFileFor("/delve/validation/branch-path-wait+boast.json"),
    "/delve/validation/branch-waypoints-wait+boast.json",
  );
});

test("a path file outside the branch-path contract is a hard fault, not a fallback", () => {
  // A wrong name here means the branch PLAN is broken (the two files are one
  // contract) — silently deriving nothing would demote that to a quiet
  // un-waypointed walk, the exact failure mode the loud fallback exists to end.
  for (const bad of ["/delve/critical-path.json", "/delve/validation/branch-flee.json"]) {
    assert.throws(
      () => branchWaypointsFileFor(bad),
      (err: unknown) => err instanceof WaypointsParseError,
    );
  }
});

test("an absent per-branch artifact loads as undefined; a present one parses", async () => {
  const dir = await mkdtemp(path.join(tmpdir(), "dw-branch-wp-"));
  try {
    const pathFile = path.join(dir, "branch-path-flee.json");
    // Absent → undefined (the CALLER owns the loud fallback report).
    assert.equal(await loadWaypointsForBranchPath(pathFile), undefined);
    // Present → parsed under the same structural rules as the critical-path
    // artifact (same parser, same hard failure on malformed data).
    await writeFile(path.join(dir, "branch-waypoints-flee.json"), JSON.stringify(VALID));
    const wp = await loadWaypointsForBranchPath(pathFile);
    assert.equal(wp?.campaignId, "nobodys-cave");
    assert.equal(wp?.legs.length, VALID.legs.length);
    // Malformed → throws, never a silent fallback.
    await writeFile(path.join(dir, "branch-waypoints-flee.json"), "{not json");
    await assert.rejects(
      loadWaypointsForBranchPath(pathFile),
      (err: unknown) => err instanceof WaypointsParseError,
    );
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});
