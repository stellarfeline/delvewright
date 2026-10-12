// spec-0101 §5.4: the bot's side of a watching body — the record it reads,
// the bearing it asks the server about, and the binding it states.

import { test } from "node:test";
import assert from "node:assert/strict";
import { CriticalPathParseError, parseCriticalPath } from "../src/critical-path.ts";
import {
  WatchLedger,
  bearing,
  WATCH_REACH_MARGIN,
  feetDistance,
  mayReach,
  parsePosReply,
  parseYawReply,
  wrapDegrees,
  yRotationRange,
  watchStops,
  type Watcher,
} from "../src/watch.ts";

function pathWith(watchers: unknown): Record<string, unknown> {
  return {
    version: "fixture",
    format_version: 4,
    campaign_id: "gallery",
    non_combatants: { kinds: ["villager"], ambiguous: [], examined: 1, unbound: false },
    steps: [
      { action: "select-class", class: "class/warder", command: "/trigger dw.class set 2" },
      { action: "assert-complete", scoreboard: { objective: "dw.campaign", value: 1 } },
    ],
    ...(watchers === undefined ? {} : { watchers }),
  };
}

function row(over: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    id: "npc/warden",
    class: "npc",
    tag: "dw_npc_warden",
    selector: "tag=dw_npc_warden,tag=dw_npc",
    feet: [20.5, 67, 5.5],
    stands: [
      [20.5, 67, 5.5],
      [27.5, 67, 3.5],
    ],
    home_yaw: -180,
    who: "nearest",
    class_tag: null,
    within: 8,
    drawable_cells: 138,
    test_cell: [20, 67, 6],
    test_yaw: 0,
    ...over,
  };
}

test("the game's bearing: south is 0, west 90, north -180, east -90", () => {
  const o: [number, number, number] = [0.5, 64, 0.5];
  // The game multiplies by the float constant 57.2957763671875, not 180/π, so
  // its quarter turns land a few millionths of a degree off.
  const near = (a: number, b: number): void => assert.ok(Math.abs(wrapDegrees(a - b)) < 1e-4, `${a} vs ${b}`);
  near(bearing(o, [0.5, 64, 5.5]), 0);
  near(bearing(o, [-4.5, 64, 0.5]), 90);
  near(bearing(o, [0.5, 64, -4.5]), -180);
  near(bearing(o, [5.5, 64, 0.5]), -90);
  assert.equal(wrapDegrees(270), -90);
  assert.equal(Object.is(wrapDegrees(-0), 0), true);
});

test("a y_rotation range across the seam is written wrapped, as the selector reads it", () => {
  assert.equal(yRotationRange(179.5, 2), "177.50..-178.50");
  assert.equal(yRotationRange(0, 2), "-2.00..2.00");
});

test("a position and a yaw are read off the server's NBT renderings", () => {
  assert.deepEqual(parsePosReply("[dw:watch-pos npc/warden [20.5d, 67.0d, 5.5d]]"), [20.5, 67, 5.5]);
  // The pinned server prints a list with no space after the comma.
  assert.deepEqual(parsePosReply("[dw:watch-pos @s [19.3d,67.0d,12.68d]]"), [19.3, 67, 12.68]);
  assert.equal(parsePosReply("[dw:watch-pos npc/warden ]"), undefined);
  assert.deepEqual(parsePosReply("x [1.0d, 2.0d, 3.0d] y [-4.5d, 6.0d, -1.25E-4d]"), [-4.5, 6, -0.000125]);
  assert.equal(parseYawReply("-90.0f]"), -90);
  assert.equal(parseYawReply("]"), undefined);
});

test("a waypoint asks the server only where some stand of the body is within reach", () => {
  const w: Watcher = {
    id: "npc/warden",
    class: "npc",
    tag: "dw_npc_warden",
    selector: "tag=dw_npc_warden,tag=dw_npc",
    feet: [20.5, 67, 5.5],
    stands: [
      [20.5, 67, 5.5],
      [40.5, 67, 5.5],
    ],
    homeYaw: -180,
    who: "nearest",
    within: 8,
  };
  assert.equal(mayReach(w, [25, 67, 5.5]), true);
  assert.equal(mayReach(w, [30.5, 67, 5.5]), false);
  assert.equal(mayReach(w, [35, 67, 5.5]), true, "a walk's end is a stand");
});

test("the binding line states the record's count, the reached and the asserted, and names the unreached", () => {
  const ledger = new WatchLedger(
    parseCriticalPath(
      pathWith([row(), row({ id: "actor/standard-bearer", class: "actor", who: "class/warder", class_tag: "dw_class_warder" })]),
    ).watchers,
  );
  ledger.reach("npc/warden");
  ledger.pass("npc/warden");
  ledger.pass("npc/warden");
  assert.equal(
    ledger.line(),
    "[watch] 2 watcher(s) in the record, 1 within reach on the proven path, 1 asserted (2 judgement(s)); " +
      "never within reach: actor/standard-bearer",
  );
  assert.deepEqual(ledger.failures(), []);
  const empty = new WatchLedger(parseCriticalPath(pathWith(undefined)).watchers);
  assert.equal(empty.line(), "[watch] 0 watcher(s) in the record, 0 within reach on the proven path, 0 asserted (0 judgement(s))");
});

test("the record's watchers are parsed whole, and a malformed row is refused by pointer", () => {
  const ws = parseCriticalPath(pathWith([row()])).watchers;
  assert.equal(ws.length, 1);
  assert.equal(ws[0]!.within, 8);
  assert.equal(ws[0]!.classTag, undefined);
  assert.equal(ws[0]!.stands.length, 2);
  const refuses = (watchers: unknown, pointer: string): void => {
    assert.throws(
      () => parseCriticalPath(pathWith(watchers)),
      (e: unknown) => e instanceof CriticalPathParseError && e.pointer === pointer,
    );
  };
  refuses([], "/watchers");
  refuses([row({ within: 0 })], "/watchers/0/within");
  refuses([row({ stands: [] })], "/watchers/0/stands");
  refuses([row({ extra: 1 })], "/watchers/0/extra");
  refuses([row({ class_tag: "dw_class_warder" })], "/watchers/0");
  refuses([row({ who: "class/warder" })], "/watchers/0");
});

test("a watcher beside the middle of a straight two-waypoint leg is judged from the middle", () => {
  // The compiler draws the body from the dense route cells; the export keeps a
  // straight leg's two ends. The body stands 4 blocks off the leg's middle,
  // reach 7, and more than 10 from either end.
  const [w] = parseCriticalPath(
    pathWith([row({ id: "actor/the-tailor", feet: [10.5, 64, 4.5], stands: [[10.5, 64, 4.5]], within: 7 })]),
  ).watchers as [Watcher];
  const leg: [number, number, number][] = [
    [0, 64, 0],
    [20, 64, 0],
  ];
  const centre = (c: readonly number[]): [number, number, number] => [c[0]! + 0.5, c[1]!, c[2]! + 0.5];
  assert.ok(!leg.some((v) => mayReach(w, centre(v))), "neither end is within reach");
  const stops = watchStops(leg, [w]);
  assert.deepEqual(stops.cells, [
    [0, 64, 0],
    [10, 64, 0],
    [20, 64, 0],
  ]);
  assert.equal(stops.inserted, 1);
  assert.deepEqual(stops.reachable, ["actor/the-tailor"]);
  assert.ok(feetDistance(w.feet, centre([10, 64, 0])) <= w.within - WATCH_REACH_MARGIN);
  // A hop that must stay whole (a climb, a gate crossing) is never split.
  assert.equal(watchStops(leg, [w], () => true).inserted, 0);
  // A body already in reach of a corner adds no stop.
  const [near] = parseCriticalPath(
    pathWith([row({ id: "npc/near", feet: [1.5, 64, 2.5], stands: [[1.5, 64, 2.5]], within: 7 })]),
  ).watchers as [Watcher];
  const corner = watchStops(leg, [near]);
  assert.equal(corner.inserted, 0);
  assert.deepEqual(corner.reachable, ["npc/near"]);
  // A body out of reach of the whole route stays unreached.
  const [far] = parseCriticalPath(
    pathWith([row({ id: "npc/far", feet: [10.5, 64, 12.5], stands: [[10.5, 64, 12.5]], within: 7 })]),
  ).watchers as [Watcher];
  assert.deepEqual(watchStops(leg, [far]), { cells: leg, inserted: 0, reachable: [] });
});
