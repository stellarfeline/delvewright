import { test } from "node:test";
import assert from "node:assert/strict";
import {
  CueLedgerParseError,
  cueBinding,
  cueBindingLine,
  judgeCues,
  parseCueLedger,
  type CueEvent,
} from "../src/cues.ts";
import { ServerView } from "../src/sound-heard.ts";

// The engine's quest cues arrive where the bot stands. A cue played at the
// command source instead lands at the world spawn, or not at all beyond its
// range; each failure mode is planted here and must red.

const raw = {
  world_spawn: [0, 64, 0],
  volume: 1,
  pitch: 1,
  range: 16,
  cues: {
    announce: "minecraft:block.note_block.pling",
    complete: "minecraft:entity.experience_orb.pickup",
    campaign: "minecraft:ui.toast.challenge_complete",
  },
  objectives: ["obj/a"],
};
const ledger = parseCueLedger(raw);

const far: [number, number, number] = [100.5, 64, 0.5];
const marker = (token: string, bot = far): CueEvent => ({ kind: "marker", token, bot });
const sound = (
  name: string,
  pos: [number, number, number],
  bot = far,
  volume = 1,
): CueEvent => ({ kind: "sound", name, pos, volume, pitch: 1, bot, held: [[...bot]] });

test("cues at the bot pass, and count as judged beyond the range", () => {
  const j = judgeCues(ledger, [
    sound("minecraft:block.note_block.pling", far),
    marker("obj/a"),
    sound("minecraft:entity.experience_orb.pickup", far),
    sound("minecraft:ui.toast.challenge_complete", far),
    marker("campaign"),
  ]);
  assert.deepEqual(j.failures, []);
  assert.doesNotMatch(j.owed[0]!.detail, /held by the server/, "no note when held where the packet found it");
  const b = cueBinding(ledger, j);
  assert.equal(b.owed, 2);
  assert.equal(b.arrived, 2);
  assert.equal(b.beyondRange, 2);
  assert.equal(b.unclaimed, 1);
  assert.equal(b.unclaimedAtBot, 1);
  assert.match(cueBindingLine(ledger, b), /^cue binding: 2 of 2 owed cue\(s\) arrived at the bot/);
});

test("a cue at the world spawn reds, owed or not", () => {
  const near: [number, number, number] = [8.5, 64, 0.5];
  const j = judgeCues(ledger, [
    sound("minecraft:block.note_block.pling", [0, 64, 0], near),
    marker("obj/a", near),
    sound("minecraft:entity.experience_orb.pickup", [0, 64, 0], near),
  ]);
  assert.equal(j.failures.length, 2);
  assert.ok(j.failures.every((f) => f.includes("NOT at the bot")), j.failures.join("\n"));
});

test("an owed cue that never arrives reds", () => {
  const j = judgeCues(ledger, [marker("obj/a"), marker("campaign")]);
  assert.equal(j.failures.length, 2);
  assert.ok(j.failures.every((f) => f.includes("did NOT arrive")), j.failures.join("\n"));
  assert.match(cueBindingLine(ledger, cueBinding(ledger, j)), /0 of 2 owed/);
});

test("a completion cue is the one after its own marker, never the previous objective's", () => {
  const two = parseCueLedger({ ...raw, objectives: ["obj/a", "obj/b"] });
  const j = judgeCues(two, [
    marker("obj/a"),
    sound("minecraft:entity.experience_orb.pickup", far),
    marker("obj/b"),
  ]);
  assert.equal(j.owed.length, 2);
  assert.equal(j.owed[0]!.ok, true);
  assert.equal(j.owed[1]!.ok, false);
});

test("an objective the ledger does not name owes nothing; a quiet sound is not a cue", () => {
  const j = judgeCues(ledger, [
    marker("obj/other"),
    sound("minecraft:entity.experience_orb.pickup", [0, 64, 0], far, 0.1),
  ]);
  assert.deepEqual(j.failures, []);
  assert.equal(cueBinding(ledger, j).owed, 0);
  assert.match(cueBindingLine(ledger, cueBinding(ledger, j)), /UNBOUND/);
});

test("the campaign cue may sound before its marker", () => {
  const j = judgeCues(ledger, [
    marker("obj/other"),
    sound("minecraft:ui.toast.challenge_complete", far),
    marker("campaign"),
  ]);
  assert.deepEqual(j.failures, []);
  assert.equal(j.owed.length, 1);
});

test("a malformed ledger is refused", () => {
  assert.throws(() => parseCueLedger({ ...raw, cues: { announce: "x" } }), CueLedgerParseError);
  assert.throws(() => parseCueLedger({ ...raw, world_spawn: [0.5, 64, 0] }), CueLedgerParseError);
  assert.throws(() => parseCueLedger({ ...raw, objectives: [1] }), CueLedgerParseError);
});

// The town ladder's load shape (branch world 14, its server behind the clock):
// the bot walks +x into `obj/corner-the-fox`'s completion cube (lo x 242) toward
// its point (x 246); the server played the cue in the tick it first held the bot
// inside, on the cube's face, at [242.5,164,36.375]; the packet reached the bot
// 0.65 s later at [245.375,164,36.5]. Every one of the eleven red worlds had this
// shape: the cue on the cube's entry face, the bot deeper in along its path.
const STEP = 4.317 / 20; // blocks per physics tick, walking
const walk = (view: ServerView, from: number, x0: number, until: number): [number, number, number] => {
  let x = x0;
  for (let t = from; t <= until; t += 50) {
    view.tell(t, [x, 164, 36.5]);
    x += STEP;
  }
  return [x - STEP, 164, 36.5];
};
const town = parseCueLedger({ ...raw, world_spawn: [-10, 70, 200], objectives: ["obj/corner-the-fox"] });
const faceCue: [number, number, number] = [242.5, 164, 36.375];

function loadShape(): { view: ServerView; arrival: [number, number, number] } {
  const view = new ServerView();
  view.clockRead(0, 1000);
  view.clockRead(1000, 1020);
  // 1100 ms: the bot tells the server it stands at x ~242.4, inside the cube's
  // face; the server stalls, the bot walks on, and the cue's tick's packets,
  // the next clock reading among them, arrive together at 1750 ms.
  const arrival = walk(view, 0, 237.82, 1750);
  view.clockRead(1750, 1040);
  return { view, arrival };
}

test("load shape: a cue played where the server held the walking bot passes", () => {
  const { view, arrival } = loadShape();
  assert.ok(Math.abs(arrival[0] - 245.375) < 0.2, `arrival ${arrival[0]}`);
  const held = view.heldAt(arrival);
  const events: CueEvent[] = [
    { kind: "marker", token: "obj/corner-the-fox", bot: arrival },
    { kind: "sound", name: "minecraft:entity.experience_orb.pickup", pos: faceCue, volume: 1, pitch: 1, bot: arrival, held },
  ];
  const j = judgeCues(town, events);
  assert.deepEqual(j.failures, []);
  assert.match(j.owed[0]!.detail, /held by the server at \[242\.\d+,164,36\.5\] when it played it/);
  // The rule this replaces judged the bot where the packet found it: the observed red.
  const old = judgeCues(town, [events[0]!, { ...(events[1] as CueEvent & { kind: "sound" }), held: [arrival] }]);
  assert.equal(old.failures.length, 1);
  assert.match(old.failures[0]!, /NOT at the bot \(\[245\.375,164,36\.5\]/);
});

test("load shape: a cue at the world spawn still reds", () => {
  const { view, arrival } = loadShape();
  const j = judgeCues(town, [
    marker("obj/corner-the-fox", arrival),
    { kind: "sound", name: "minecraft:entity.experience_orb.pickup", pos: [-10, 70, 200], volume: 1, pitch: 1, bot: arrival, held: view.heldAt(arrival) },
  ]);
  assert.equal(j.failures.length, 1);
  assert.match(j.failures[0]!, /NOT at the bot .*nor at any of the \d+ position\(s\) the server may have held/);
});

test("a cue where the bot stood before the server's clock window reds", () => {
  const view = new ServerView();
  view.clockRead(0, 1000);
  view.tell(100, [0, 64, 0]); // the bot stood at the spawn, long before
  walk(view, 200, 230, 950);
  view.clockRead(1000, 1020);
  const arrival = walk(view, 1000, 234, 2950);
  view.clockRead(2000, 1040); // the window opens at 1000 ms
  const held = view.heldAt(arrival);
  assert.ok(!held.some((p) => p[0] === 0), "a position told before the window is not held");
  assert.ok(held.some((p) => p[0] < 234), "the one told last before the window stood at its start");
  const j = judgeCues(ledger, [
    marker("obj/a", arrival),
    { kind: "sound", name: "minecraft:entity.experience_orb.pickup", pos: [0, 64, 0], volume: 1, pitch: 1, bot: arrival, held },
  ]);
  assert.equal(j.failures.length, 1);
});

test("before two clock readings, the server holds the bot only where the packet found it", () => {
  const view = new ServerView();
  view.clockRead(0, 1000);
  const arrival = walk(view, 0, 237.6, 1750);
  assert.deepEqual(view.heldAt(arrival), [arrival]);
  // a repeated age is the same tick, not a second reading
  view.clockRead(900, 1000);
  assert.deepEqual(view.heldAt(arrival), [arrival]);
  const j = judgeCues(town, [
    marker("obj/corner-the-fox", arrival),
    { kind: "sound", name: "minecraft:entity.experience_orb.pickup", pos: faceCue, volume: 1, pitch: 1, bot: arrival, held: view.heldAt(arrival) },
  ]);
  assert.equal(j.failures.length, 1);
});
