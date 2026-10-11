import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, mkdir, writeFile, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import {
  PulsePlanParseError,
  judgeListening,
  judgeSilent,
  loadPulsePlanForCriticalPath,
  parsePulsePlan,
  pulseBindingLine,
  pulseVerdicts,
  standTicks,
  stationsBefore,
  type HeardSound,
} from "../src/pulse.ts";

// spec-0102 §5.3: a pulse is heard at its listening station — at least two
// beats naming its sound, at the source, at the derived volume and the
// declared pitch — and not at its silent one. Each failure mode is planted
// here and must red.

const raw = {
  declared: 2,
  pulses: [
    {
      id: "pulse/the-heart",
      sound: "minecraft:entity.warden.heartbeat",
      source: [5.5, 66.5, 9.5],
      every: 30,
      volume: 3.390099965749945,
      pitch: 1.0,
      stations: {
        listening: { cell: [23, 67, 12], step: 4, before: "trigger/strike-the-sentinel" },
        silent: { cell: [517, 65, 68], step: 13, before: "obj/reach-the-hall-end" },
      },
    },
    {
      id: "pulse/the-drip",
      sound: "minecraft:block.pointed_dripstone.drip_water",
      source: [259.5, 67.5, 10.5],
      every: 40,
      volume: 1.0,
      pitch: 1.0,
      stations: { listening: null, silent: null },
      not_heard: "no station",
    },
  ],
};
const plan = parsePulsePlan(raw);
const heart = plan.pulses[0]!;

const beat = (over: Partial<HeardSound> = {}): HeardSound => ({
  // mineflayer names a registry sound without its namespace.
  name: "entity.warden.heartbeat",
  pos: [5.5, 66.5, 9.5],
  volume: 3.3900999,
  pitch: 1.0,
  ...over,
});

test("a station stands two intervals and ten ticks", () => {
  assert.equal(standTicks(heart), 70);
});

test("two beats at the source, volume and pitch are heard as the compiler wrote them", () => {
  assert.equal(judgeListening(heart, [beat(), beat()]), undefined);
});

test("one beat is not enough", () => {
  assert.match(judgeListening(heart, [beat()]) ?? "", /heard 1 beat/);
});

test("a beat at the wrong volume, place or pitch is not the beat", () => {
  for (const wrong of [beat({ volume: 1.0 }), beat({ pos: [7, 66.5, 9.5] }), beat({ pitch: 1.2 })]) {
    const f = judgeListening(heart, [wrong, wrong, wrong]);
    assert.match(f ?? "", /heard 0 beat\(s\).*instead/);
  }
});

test("other sounds neither count nor red", () => {
  const other = beat({ name: "block.bell.use" });
  assert.match(judgeListening(heart, [other, other]) ?? "", /heard 0 beat/);
  assert.equal(judgeSilent(heart, [other]), undefined);
});

test("any beat at the silent station reds it", () => {
  assert.equal(judgeSilent(heart, []), undefined);
  assert.match(judgeSilent(heart, [beat({ volume: 0.2 })]) ?? "", /silent station/);
});

test("stations are due before their step's token, once", () => {
  const due = stationsBefore(plan, "trigger/strike-the-sentinel", new Set());
  assert.deepEqual(
    due.map((d) => d.key),
    ["pulse/the-heart#listening"],
  );
  assert.equal(stationsBefore(plan, "trigger/strike-the-sentinel", new Set([due[0]!.key])).length, 0);
  assert.equal(stationsBefore(plan, "obj/nowhere", new Set()).length, 0);
});

test("a pulse with no station is reported, never passed; an unreached station is named", () => {
  const recorded = new Map<string, string | undefined>([["pulse/the-heart#listening", undefined]]);
  const v = pulseVerdicts(plan, recorded);
  assert.equal(v[0]!.listening, "heard");
  assert.equal(v[0]!.silent, "not reached");
  assert.equal(v[0]!.unreached.length, 1);
  assert.equal(v[1]!.listening, "no station");
  assert.deepEqual(v[1]!.failures, []);
  assert.match(pulseBindingLine(plan, v), /1 of 2 pulse\(s\) heard.*not_heard: no station — pulse\/the-drip/);
});

test("a malformed export is a hard failure", () => {
  assert.throws(() => parsePulsePlan({ declared: 3, pulses: raw.pulses }), PulsePlanParseError);
  assert.throws(
    () => parsePulsePlan({ declared: 1, pulses: [{ ...raw.pulses[0], volume: "loud" }] }),
    PulsePlanParseError,
  );
});

test("the export is read beside the critical path, and absence is no plan", async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), "pulse-plan-"));
  try {
    const cp = path.join(dir, "critical-path.json");
    assert.equal(await loadPulsePlanForCriticalPath(cp), undefined);
    await mkdir(path.join(dir, "validation"));
    await writeFile(path.join(dir, "validation", "pulses.json"), JSON.stringify(raw));
    assert.equal((await loadPulsePlanForCriticalPath(cp))?.pulses.length, 2);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});
