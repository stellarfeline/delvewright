import { test } from "node:test";
import assert from "node:assert/strict";
import { gameTimeCommands, parseGameTime } from "../src/game-time.ts";
import {
  legStartCommands,
  parseLegStartBusy,
  parseUndeclaredWardens,
  SculkEar,
  undeclaredWardenCommands,
} from "../src/sculk.ts";

test("a game-time reading stores the time, then speaks it under its serial", () => {
  const cmds = gameTimeCommands(7);
  assert.equal(cmds[0], "/execute store result storage dw_harness:clock t int 1 run time query gametime");
  assert.equal(
    cmds[1],
    '/tellraw @s ["[dw:clock 7 ",{"storage":"dw_harness:clock","nbt":"t"},"]"]',
  );
  assert.equal(parseGameTime(7, ["The time is 41230", "[dw:clock 7 41230]"]), 41230);
  assert.equal(parseGameTime(7, ["[dw:clock 6 41230]"]), undefined, "another reading's line is not this one's");
  assert.equal(parseGameTime(7, ["[dw:clock 7 ]"]), undefined);
});

test("a leg's start asks each predicted device once whether it is busy, and reads which are", () => {
  const cmds = legStartCommands(3, [
    { sensor: [163, 64, 622], shriekers: [[160, 65, 622]] },
    { sensor: [163, 64, 622], shriekers: [] },
  ]);
  assert.deepEqual(cmds, [
    '/execute if loaded 163 64 622 if block 163 64 622 minecraft:sculk_sensor[sculk_sensor_phase=active] run tellraw @s {"text":"[dw:sculk 3 busy 163 64 622]"}',
    '/execute if loaded 163 64 622 if block 163 64 622 minecraft:sculk_sensor[sculk_sensor_phase=cooldown] run tellraw @s {"text":"[dw:sculk 3 busy 163 64 622]"}',
    '/execute if loaded 163 64 622 if block 163 64 622 minecraft:calibrated_sculk_sensor[sculk_sensor_phase=active] run tellraw @s {"text":"[dw:sculk 3 busy 163 64 622]"}',
    '/execute if loaded 163 64 622 if block 163 64 622 minecraft:calibrated_sculk_sensor[sculk_sensor_phase=cooldown] run tellraw @s {"text":"[dw:sculk 3 busy 163 64 622]"}',
    '/execute if loaded 160 65 622 if block 160 65 622 minecraft:sculk_shrieker[shrieking=true] run tellraw @s {"text":"[dw:sculk 3 busy 160 65 622]"}',
  ]);
  const busy = parseLegStartBusy(3, ["[dw:sculk 3 busy 163 64 622]", "[dw:sculk 2 busy 160 65 622]"]);
  assert.deepEqual([...busy], ["163 64 622"]);
});

test("an event before the leg answers for it only when its device was busy at the start, and arrived before the reading", () => {
  const ear = new SculkEar();
  const v = [{ sensor: [1, 2, 3] as [number, number, number], shriekers: [] }];
  ear.onBlockUpdate([1, 2, 3], { sculk_sensor_phase: "active" }, 100);
  const idle = { at: 500, tick: 40, readAt: 520, busy: new Set<string>() };
  assert.equal(ear.hear(v, idle, 900).sensorsHeard, 0, "not busy at the start: the leg owes its own click");
  const busy = { at: 500, tick: 40, readAt: 520, busy: new Set(["1 2 3"]) };
  assert.equal(ear.hear(v, busy, 900).sensorsHeard, 1, "busy at the start: the earlier click is the leg's");
  const late = new SculkEar();
  late.onBlockUpdate([1, 2, 3], { sculk_sensor_phase: "active" }, 1_000);
  assert.equal(late.hear(v, busy, 900).sensorsHeard, 0, "a click after `to` is not heard");
});

// spec-0132 §5.7: a warden the campaign declares is not a shrieker's.

test("a declared warden no longer fails the run; an undeclared one still does", () => {
  const ear = new SculkEar();
  ear.onSpawn("warden");
  ear.onSpawn("warden");
  const one = ear.runFailure(1);
  assert.ok(one !== undefined, "one undeclared warden standing fails the run");
  assert.match(one, /warden spawns heard 2, undeclared standing 1/);
  assert.equal(ear.runFailure(0), undefined, "two heard, none undeclared: the campaign's own");
  // Unread is never a zero: a run whose server did not answer fails on what it heard.
  assert.match(ear.runFailure() ?? "", /undeclared standing unread/);
});

test("a darkness a warden in reach explains is counted apart and fails nothing", () => {
  const ear = new SculkEar();
  ear.onSelfEffect("darkness", true);
  assert.equal(ear.runFailure(0), undefined);
  assert.match(ear.runLine(0), /darkness 0 \(1 more with a warden in reach\)/);
  ear.onSelfEffect("darkness", false);
  assert.match(ear.runFailure(0) ?? "", /darkness 1 \(1 more with a warden in reach\)/);
});

test("the undeclared-warden reading asks for wardens wearing no dw_actor tag, and parses its count", () => {
  const cmds = undeclaredWardenCommands(4);
  assert.match(cmds[0]!, /if entity @e\[type=minecraft:warden,tag=!dw_actor\]$/);
  assert.equal(parseUndeclaredWardens(4, ["[dw:sculk 4 undeclared 2]"]), 2);
  assert.equal(parseUndeclaredWardens(4, ["[dw:sculk 3 undeclared 2]"]), undefined);
});

test("an unpredicted shriek beside a declared warden fails the run, and its darkness is not excused", () => {
  const ear = new SculkEar();
  // The run predicted one shrieker, at [4, 64, 4]; a declared warden stands in reach.
  ear.predictShriekers([[4, 64, 4]]);
  ear.onSpawn("warden");
  // A shrieker nobody predicted shrieks at [30, 64, 30], and its darkness lands
  // on the bot in the same moment, with the warden within 20 blocks.
  ear.onWorldEvent(3007, [30, 64, 30], 1_000);
  ear.onSelfEffect("darkness", true, 1_050);
  const failure = ear.runFailure(0);
  assert.ok(failure !== undefined, "an unpredicted shriek is never excused by a warden in reach");
  assert.match(failure, /unpredicted shrieks 1 \(at \[30, 64, 30\]\)/);
  assert.match(failure, /darkness 1 \(0 more with a warden in reach\)/);
});

test("a predicted shriek still un-excuses the darkness that arrives with it", () => {
  const ear = new SculkEar();
  ear.predictShriekers([[4, 64, 4]]);
  ear.onWorldEvent(3007, [4, 64, 4], 1_000);
  ear.onSelfEffect("darkness", true, 1_020);
  assert.match(ear.runFailure(0) ?? "", /darkness 1 \(0 more with a warden in reach\), unpredicted shrieks 0/);
  // A warden's pulse with no shriek around it is the warden's.
  const quiet = new SculkEar();
  quiet.onWorldEvent(3007, [4, 64, 4], 1_000);
  quiet.predictShriekers([[4, 64, 4]]);
  quiet.onSelfEffect("darkness", true, 9_000);
  assert.equal(quiet.runFailure(0), undefined);
});
