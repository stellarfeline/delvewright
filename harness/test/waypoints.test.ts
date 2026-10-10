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
  subdivideStraightRuns,
  walkGoals,
  WaypointsParseError,
  WAYPOINT_RANGE,
  type Waypoints,
  nearestIndex,
} from "../src/waypoints.ts";
import type { Vec3Tuple } from "../src/critical-path.ts";
import { distSqToRoute } from "../src/sculk.ts";

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

test("an empty version is rejected", () => {
  assert.throws(
    () => parseWaypoints({ ...VALID, version: "" }),
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

test("a straight run longer than the bound is split at its own proven cells", () => {
  // the-stranding r6, the Run's bank: one thinned hop of 268 blocks.
  const sub = subdivideStraightRuns([
    [210, 64, 848],
    [210, 64, 580],
  ]);
  assert.deepEqual(sub.cells, [
    [210, 64, 848],
    [210, 64, 784],
    [210, 64, 720],
    [210, 64, 656],
    [210, 64, 592],
    [210, 64, 580],
  ]);
  assert.equal(sub.split, 1);
  assert.equal(sub.inserted, 4);
  assert.deepEqual(sub.unsplittable, []);
});

test("a straight stair run splits on its own diagonal, every inserted cell on the run", () => {
  const sub = subdivideStraightRuns(
    [
      [0, 60, 0],
      [10, 70, 0],
    ],
    4,
  );
  assert.deepEqual(sub.cells, [
    [0, 60, 0],
    [4, 64, 0],
    [8, 68, 0],
    [10, 70, 0],
  ]);
});

test("a hop within the bound, or of a shape that is not one straight run, is left whole", () => {
  const short: Vec3Tuple[] = [
    [0, 64, 0],
    [64, 64, 0],
  ];
  assert.deepEqual(subdivideStraightRuns(short).cells, short, "exactly the bound is not split");
  const bent: Vec3Tuple[] = [
    [0, 64, 0],
    [100, 64, 3],
  ];
  const sub = subdivideStraightRuns(bent);
  assert.deepEqual(sub.cells, bent, "never a point the compiler did not walk");
  assert.equal(sub.split, 0);
  assert.deepEqual(sub.unsplittable, [[[0, 64, 0], [100, 64, 3]]], "and it is reported");
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

test("nearestIndex finds where along a leg a crossing lies, ties to the earlier cell", () => {
  const cells: [number, number, number][] = [
    [10, 64, 0],
    [8, 64, 0],
    [6, 64, 0],
    [4, 64, 0],
  ];
  assert.equal(nearestIndex(cells, [4, 64, 1]), 3);
  assert.equal(nearestIndex(cells, [7, 64, 0]), 1, "equidistant from 8 and 6: the earlier");
  assert.equal(nearestIndex([], [0, 0, 0]), 0);
});

// spec-0099: the gallery's cabin-ladder legs, as the compiler exports them — up
// the ladder onto the roof, and back down.
const CLIMBING = {
  version: "0.4.0",
  campaign_id: "gallery",
  legs: [
    {
      from: [11, 67, 25],
      to: [2, 71, 28],
      waypoints: [
        [11, 67, 25],
        [5, 67, 28],
        [4, 71, 28],
        [2, 71, 28],
      ],
      climbs: [
        {
          block: "minecraft:ladder[facing=east]",
          bottom: [5, 67, 28],
          facing: "east",
          from: [5, 67, 28],
          to: [4, 71, 28],
          top: [5, 70, 28],
        },
      ],
    },
    {
      from: [2, 71, 28],
      to: [11, 67, 25],
      waypoints: [
        [2, 71, 28],
        [4, 71, 28],
        [5, 67, 27],
        [11, 67, 25],
      ],
      climbs: [
        {
          block: "minecraft:ladder[facing=east]",
          bottom: [5, 68, 28],
          facing: "east",
          from: [4, 71, 28],
          to: [5, 67, 27],
          top: [5, 70, 28],
        },
      ],
    },
  ],
};

test("a leg's climbs parse, and the hop between a climb's ends carries it (spec-0099)", async () => {
  const { climbDirection, supportCell } = await import("../src/waypoints.ts");
  const wp = parseWaypoints(CLIMBING);
  assert.equal(wp.legs[0]!.climbs.length, 1);
  const up = wp.legs[0]!.climbs[0]!;
  assert.equal(climbDirection(up), "up");
  assert.equal(climbDirection(wp.legs[1]!.climbs[0]!), "down");
  // The ladder faces east, so the block it hangs on — what the bot pushes — is west.
  assert.deepEqual(supportCell(up, 69), [4, 69, 28]);
  const goals = walkGoals(wp.legs[0]!.waypoints, [2, 71, 28], 1, wp.legs[0]!.climbs);
  const driven = goals.filter((g) => g.climb !== undefined);
  assert.equal(driven.length, 1, "exactly the from→to hop is the climb");
  assert.deepEqual([driven[0]!.x, driven[0]!.y, driven[0]!.z], [4, 71, 28]);
  // The match hands the climbs over with the waypoints, and an artifact without
  // them parses to none.
  const m = nextLegWaypoints(wp.legs, 0, [2, 71, 28]);
  assert.equal(m.climbs.length, 1);
  assert.equal(parseWaypoints(VALID).legs[0]!.climbs.length, 0);
  // A climb with nothing in its column, or ends that are not the leg's waypoints,
  // is a structural fault, never a climb walked by the pathfinder in silence.
  const bad = structuredClone(CLIMBING);
  bad.legs[0]!.climbs[0]!.to = [9, 99, 9];
  assert.throws(() => parseWaypoints(bad), WaypointsParseError);
  const skew = structuredClone(CLIMBING);
  skew.legs[0]!.climbs[0]!.top = [6, 70, 28];
  assert.throws(() => parseWaypoints(skew), WaypointsParseError);
});

test("the climb executor refuses what the bot's physics cannot climb (spec-0099)", async () => {
  const { clientClimbs, climbBudgetMs } = await import("../src/executor/climb.ts");
  assert.ok(clientClimbs("minecraft:ladder[facing=east]"));
  assert.ok(clientClimbs("minecraft:vine[west=true]"));
  assert.ok(!clientClimbs("minecraft:weeping_vines_plant"));
  assert.ok(!clientClimbs("minecraft:twisting_vines"));
  const c = parseWaypoints(CLIMBING).legs[0]!.climbs[0]!;
  assert.ok(climbBudgetMs(c) >= 6_000 + 4 * 1_000);
});

// The gallery's listening floor (spec-0100 §6), as the compiler exports it: the
// leg off the loft stair to the ferry deck carries the plain sensor set into
// the floor and the shrieker that answers it — and no other leg of the
// critical path carries a shrieker, so the counts below are the gallery's.
const LISTENING = {
  version: "0.0.0-fixture",
  campaign_id: "gallery",
  legs: [
    {
      from: [19, 70, 18],
      to: [9, 67, 21],
      waypoints: [
        [19, 70, 18],
        [19, 70, 20],
        [19, 69, 21],
        [19, 68, 22],
        [18, 67, 22],
        [17, 67, 22],
        [9, 67, 22],
        [9, 67, 21],
      ],
      vibrations: [{ sensor: [8, 66, 23], shriekers: [[8, 67, 27]] }],
    },
    { from: [9, 67, 21], to: [10, 67, 21], waypoints: [[9, 67, 21], [10, 67, 21]] },
  ],
};

test("a leg's vibrations parse, and the match hands them over (spec-0100 §4.6)", () => {
  const wp = parseWaypoints(LISTENING);
  assert.equal(wp.legs[0]!.vibrations.length, 1, "exactly the gallery's one predicted sensor");
  assert.deepEqual(wp.legs[0]!.vibrations[0]!.sensor, [8, 66, 23]);
  assert.equal(wp.legs[0]!.vibrations[0]!.shriekers.length, 1, "and its one shrieker");
  assert.deepEqual(wp.legs[0]!.vibrations[0]!.shriekers[0], [8, 67, 27]);
  assert.equal(wp.legs[1]!.vibrations.length, 0, "absent parses to none");
  const m = nextLegWaypoints(wp.legs, 0, [9, 67, 21]);
  assert.equal(m.vibrations.length, 1);
  assert.equal(nextLegWaypoints(wp.legs, 1, [0, 0, 0]).vibrations.length, 0, "no match, none");
  assert.equal(parseWaypoints(VALID).legs[0]!.vibrations.length, 0);
});

test("a sensor out of earshot of its leg, or a shrieker out of its sensor's, is refused", () => {
  // A sensor more than 8 blocks from every waypoint of its leg.
  const far = structuredClone(LISTENING);
  far.legs[0]!.vibrations![0]!.sensor = [8, 66, 40];
  assert.throws(() => parseWaypoints(far), (e: unknown) =>
    e instanceof WaypointsParseError && /within 8 blocks of its leg's route/.test(e.message),
  );
  // A shrieker more than 8 blocks from its sensor.
  const deaf = structuredClone(LISTENING);
  deaf.legs[0]!.vibrations![0]!.shriekers = [[8, 67, 32]];
  assert.throws(() => parseWaypoints(deaf), (e: unknown) =>
    e instanceof WaypointsParseError && /within 8 blocks of its sensor/.test(e.message),
  );
  // A malformed list is a pointer, not a silent skip.
  const bad = structuredClone(LISTENING) as unknown as { legs: { vibrations: unknown }[] };
  bad.legs[0]!.vibrations = { sensor: [8, 66, 23] };
  assert.throws(() => parseWaypoints(bad), WaypointsParseError);
});

test("a sensor beside the middle of a straight two-waypoint leg is in earshot of its route", () => {
  // The compiler predicts from the dense route cells and the export thins a
  // straight run to its two ends: a demo level's sensor sat at distSqr 66 to
  // the nearer of its leg's two waypoints, inside earshot of the route cells
  // between them, and the parse refused it before the bot walked.
  const straight = {
    version: "0.0.0-fixture",
    campaign_id: "straight",
    legs: [
      {
        from: [0, 64, 0],
        to: [30, 64, 0],
        waypoints: [
          [0, 64, 0],
          [30, 64, 0],
        ],
        vibrations: [{ sensor: [15, 63, 7], shriekers: [] }],
      },
    ],
  };
  const wp = parseWaypoints(straight);
  assert.deepEqual(wp.legs[0]!.vibrations[0]!.sensor, [15, 63, 7]);
  // Measured along the route, not to its vertices: the nearest route cell
  // [15, 64, 0] is at distSqr 50; the nearest waypoint at 275.
  assert.equal(distSqToRoute([15, 63, 7], [[0, 64, 0], [30, 64, 0]]), 50);
  // Beyond 8 blocks of every point of the route it is still refused.
  const off = structuredClone(straight);
  off.legs[0]!.vibrations[0]!.sensor = [15, 63, 9];
  assert.throws(() => parseWaypoints(off), (e: unknown) =>
    e instanceof WaypointsParseError && /within 8 blocks of its leg's route/.test(e.message),
  );
  // Past the leg's end the distance is to the end, not to the line's extension.
  assert.equal(distSqToRoute([40, 64, 0], [[0, 64, 0], [30, 64, 0]]), 100);
});
