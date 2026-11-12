// spec-0135 §6: the ride executor's pure parts and its verdicts on a mock bot.

import { test } from "node:test";
import assert from "node:assert/strict";
import { arrivedAt, drivePoints, vehicleTag } from "../src/executor/ride.ts";
import { methods } from "../src/executor/ride.ts";
import type { RideStep, Vec3Tuple } from "../src/critical-path.ts";

test("the tag is the compiler's", () => {
  assert.equal(vehicleTag("vehicle/race-pig"), "dw_vehicle_race_pig");
});

test("the drive reports one position a tick along the medium at the rate", () => {
  const medium: Vec3Tuple[] = [[0, 1, 0], [1, 1, 0], [2, 1, 0], [2, 1, 1]];
  const pts = drivePoints(medium, 5, () => 1.5);
  // 3 blocks at 5 blocks a second is 12 ticks; each run rounds up.
  assert.equal(pts.length, 12);
  assert.deepEqual(pts[pts.length - 1]!.at, [2.5, 1.5, 1.5]);
  for (let i = 1; i < pts.length; i++) {
    const a = pts[i - 1]!.at;
    const b = pts[i]!.at;
    assert.ok(Math.hypot(a[0] - b[0], a[2] - b[2]) <= 0.25 + 1e-9);
  }
  // Yaw along the tangent: east is -90, south is 0.
  assert.equal(Math.round(pts[0]!.yaw) + 0, -90);
  assert.equal(Math.round(pts[pts.length - 1]!.yaw) + 0, 0);
});

test("arrival is the landing tolerance around the last cell", () => {
  assert.ok(arrivedAt([10.4, 67.5, 3.6], [10, 67, 3]));
  assert.ok(!arrivedAt([20, 67, 3], [10, 67, 3]));
});

/** A mock executor: a bot that mounts and reports, and a server read the test scripts. */
/** What the mock server answers a census read with; a test sets it. */
const census: { read: boolean | undefined } = { read: true };

function mock(serverReads: { riding: boolean; pos: Vec3Tuple }[], aboardAt?: { vehicle: string; y: number; at: Vec3Tuple }) {
  const sent: { name: string; data: unknown }[] = [];
  const walked: Vec3Tuple[] = [];
  const reads = [...serverReads];
  let mounted = serverReads.length > 0 && aboardAt !== undefined;
  const self = {
    rides: [] as unknown[],
    aboard: aboardAt as { vehicle: string; y: number; at: Vec3Tuple } | undefined,
    requireBot: () => ({
      inventory: { items: () => [{ name: "carrot_on_a_stick" }] },
      equip: async () => {},
      nearestEntity: () => ({ type: "mob", name: "pig", position: { x: 8.5, y: 67, z: 3.5, offset: () => ({}) }, height: 0.9 }),
      lookAt: async () => {},
      on: () => {},
      removeListener: () => {},
      entity: {
        position: {
          x: 0,
          y: 0,
          z: 0,
          set(x: number, y: number, z: number) {
            Object.assign(this, { x, y, z });
          },
        },
      },
      mount: () => {
        mounted = true;
      },
      _client: { write: (name: string, data: unknown) => sent.push({ name, data }) },
    }),
    walkTo: async (to: Vec3Tuple) => {
      walked.push(to);
    },
    resyncBody: async () => true,
    censusResults: [] as { state: string; passed: boolean; detail: string }[],
    askTerm: async () => census.read,
    readRide: async () => {
      assert.ok(mounted, "the server is read only after the click");
      return reads.length > 1 ? reads.shift()! : reads[0]!;
    },
  };
  return { self, sent, walked };
}

const step: RideStep = {
  action: "ride",
  vehicle: "vehicle/race-pig",
  kind: "mount",
  board: [6, 67, 3],
  pos: [6, 67, 3],
  medium: [[8, 67, 3], [9, 67, 3], [10, 67, 3]],
    body: [[8, 67, 3], [8, 68, 3]],
  rate: 10,
  leave: [11, 67, 4],
  boards: true,
  stays: false,
  steeringItem: "minecraft:carrot_on_a_stick",
};

test("a mount is driven by reported moves, left by the sneak bit, read on the server", async () => {
  const { self, sent, walked } = mock([
    { riding: true, pos: [8.5, 67.3, 3.5] },
    { riding: true, pos: [10.5, 67.3, 3.5] },
    { riding: false, pos: [11.5, 67, 4.5] },
  ]);
  await methods.ride.call(self as never, step);
  const moves = sent.filter((s) => s.name === "vehicle_move");
  assert.equal(moves.length, 4, "2 blocks at 10 a second is 4 ticks");
  const inputs = sent.filter((s) => s.name === "player_input").map((s) => s.data);
  assert.deepEqual(inputs, [{ inputs: { shift: true } }, { inputs: { shift: false } }]);
  assert.deepEqual(walked, [step.board, step.leave]);
  const rec = self.rides[0] as { passed: boolean; serverSeen: number };
  assert.ok(rec.passed);
  assert.ok(Math.abs(rec.serverSeen - 2) < 1e-9);
});

test("the vehicle the server left behind is named", async () => {
  const { self } = mock([
    { riding: true, pos: [8.5, 67.3, 3.5] },
    { riding: true, pos: [8.5, 67.3, 3.5] },
  ]);
  const far = { ...step, medium: [[8, 67, 3], [12, 67, 3], [16, 67, 3]] as Vec3Tuple[] };
  await assert.rejects(() => methods.ride.call(self as never, far), /did not take the ride/);
  assert.equal((self.rides[0] as { passed: boolean }).passed, false);
});

// spec-0135 §5: a ride whose objective waits for its own rider.
const aboard = {
  state: "state/aboard",
  objective: "dw.s_aboard",
  riders: "dw_vehicle_race_pig",
  region: [[0, 60, 0], [20, 70, 20]] as [Vec3Tuple, Vec3Tuple],
  count: 1,
};
const rideReads = (): { riding: boolean; pos: Vec3Tuple }[] => [
  { riding: true, pos: [8.5, 67.3, 3.5] },
  { riding: true, pos: [10.5, 67.3, 3.5] },
  { riding: false, pos: [11.5, 67, 4.5] },
];

test("a riders census is read at the ride's end, with the bot aboard", async () => {
  census.read = true;
  const { self } = mock(rideReads());
  await methods.ride.call(self as never, { ...step, census: [aboard] });
  assert.equal(self.censusResults.length, 1);
  assert.ok(self.censusResults[0]!.passed, self.censusResults[0]!.detail);
  assert.ok((self.rides[0] as { passed: boolean }).passed);
});

test("a riders census the game does not count fails the ride by name", async () => {
  census.read = false;
  const { self } = mock(rideReads());
  await assert.rejects(
    () => methods.ride.call(self as never, { ...step, census: [aboard] }),
    /census state\/aboard: .*does not read 1 with the bot aboard/,
  );
  assert.equal((self.rides[0] as { passed: boolean }).passed, false);
  census.read = undefined;
  const silent = mock(rideReads());
  await assert.rejects(() => methods.ride.call(silent.self as never, { ...step, census: [aboard] }), /never said/);
  census.read = true;
});

test("a riders census that needs two, or names another vehicle, fails by name", async () => {
  const two = mock(rideReads());
  await assert.rejects(
    () => methods.ride.call(two.self as never, { ...step, census: [{ ...aboard, count: 2 }] }),
    /one bot is one rider/,
  );
  const other = mock(rideReads());
  await assert.rejects(
    () => methods.ride.call(other.self as never, { ...step, census: [{ ...aboard, riders: "dw_vehicle_gull" }] }),
    /not of the vehicle ridden/,
  );
});

test("a click the server does not seat fails by name", async () => {
  const { self } = mock([{ riding: false, pos: [6.5, 67, 3.5] }]);
  await assert.rejects(() => methods.ride.call(self as never, step), /did not seat the bot/);
});

test("a cart is waited for, never driven", async () => {
  const { self, sent } = mock([
    { riding: true, pos: [8.5, 67, 3.5] },
    { riding: true, pos: [14.5, 67, 3.5] },
    { riding: true, pos: [20.5, 67, 3.5] },
    { riding: false, pos: [21.5, 67, 4.5] },
  ]);
  const medium = Array.from({ length: 13 }, (_, i) => [8 + i, 67, 3] as Vec3Tuple);
  const cart = { ...step, kind: "minecart" as const, rate: 10.4, medium };
  delete (cart as { steeringItem?: string }).steeringItem;
  await methods.ride.call(self as never, cart);
  assert.equal(sent.filter((s) => s.name === "vehicle_move").length, 0);
});

// spec-0135 §5: an act done from the seat.
const boatOut: RideStep = {
  action: "ride",
  vehicle: "vehicle/gull",
  kind: "boat",
  board: [3, 4, 3],
  pos: [3, 4, 3],
  medium: [[4, 3, 3], [5, 3, 3], [6, 3, 3]],
  body: [[4, 3, 3], [4, 4, 3]],
  rate: 5,
  leave: [6, 3, 3],
  boards: true,
  stays: true,
};

test("a ride that stays leaves the bot aboard, unseated by nothing", async () => {
  const { self, sent, walked } = mock([
    { riding: true, pos: [4.5, 3.11, 3.5] },
    { riding: true, pos: [6.5, 3.11, 3.5] },
  ]);
  await methods.ride.call(self as never, boatOut);
  assert.deepEqual(walked, [boatOut.board], "walks to the boarding cell and nowhere after");
  assert.equal(sent.filter((s) => s.name === "player_input").length, 0, "no sneak: the bot stays seated");
  assert.equal(self.aboard?.vehicle, "vehicle/gull");
  const rec = self.rides[0] as { passed: boolean; left: boolean };
  assert.ok(rec.passed && !rec.left);
});

test("a ride on from aboard boards nothing and needs the bot seated", async () => {
  const on: RideStep = { ...boatOut, board: [6, 3, 3], pos: [6, 3, 3], medium: [[6, 3, 3], [7, 3, 3]], leave: [9, 4, 3], boards: false, stays: false };
  const seated = mock(
    [
      { riding: true, pos: [6.5, 3.11, 3.5] },
      { riding: true, pos: [7.5, 3.11, 3.5] },
      { riding: false, pos: [8.5, 4, 3.5] },
    ],
    { vehicle: "vehicle/gull", y: 3.52, at: [6.5, 3.11, 3.5] },
  );
  await methods.ride.call(seated.self as never, on);
  assert.deepEqual(seated.walked, [on.leave], "no walk to a boarding cell; the walk off is to the shore");
  const moves = seated.sent.filter((s) => s.name === "vehicle_move").map((s) => s.data as { y: number });
  assert.ok(moves.length > 0 && moves.every((m) => m.y === 3.52), "driven at the height the first ride reported");
  assert.equal(seated.self.aboard, undefined);
  const elsewhere = mock([{ riding: true, pos: [6.5, 3.11, 3.5] }], { vehicle: "vehicle/tern", y: 3.52, at: [6.5, 3.11, 3.5] });
  await assert.rejects(() => methods.ride.call(elsewhere.self as never, on), /sits in vehicle\/tern/);
});
