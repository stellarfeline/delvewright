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
): CueEvent => ({ kind: "sound", name, pos, volume, pitch: 1, bot });

test("cues at the bot pass, and count as judged beyond the range", () => {
  const j = judgeCues(ledger, [
    sound("minecraft:block.note_block.pling", far),
    marker("obj/a"),
    sound("minecraft:entity.experience_orb.pickup", far),
    sound("minecraft:ui.toast.challenge_complete", far),
    marker("campaign"),
  ]);
  assert.deepEqual(j.failures, []);
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
