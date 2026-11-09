import { test } from "node:test";
import assert from "node:assert/strict";
import type { PursuerRow, Vec3Tuple } from "../src/critical-path.ts";
import {
  judgePursuedStep,
  parseActorCensus,
  pursuitBinding,
  pursuitLine,
  type Arrival,
  type PursuitReading,
  type PursuitStepRecord,
} from "../src/pursuit.ts";

// spec-0132 §5: the pursuit leg's verdict over synthetic readings.

const WARDEN: PursuerRow = {
  actor: "actor/the-sleeper",
  kind: "minecraft:warden",
  census: "gallery:actor_census_the_sleeper",
  entranceTicks: 133,
  undated: false,
};

const width = (kind: string): number => (kind === "minecraft:warden" ? 0.9 : 0.6);

/** A reading of the warden at `tick`, its feet at `feet` (absent when `undefined`). */
function at(tick: number, feet: Vec3Tuple | undefined, over: Partial<PursuitReading> = {}): PursuitReading {
  return {
    actor: WARDEN.actor,
    tick,
    alive: feet !== undefined,
    ...(feet === undefined ? {} : { feet }),
    botFeet: [10, 64, 30],
    botHealth: 20,
    ...over,
  };
}

function step(readings: PursuitReading[], over: Partial<PursuitStepRecord> = {}): PursuitStepRecord {
  return {
    step: 16,
    label: "loop loop/the-hall-that-runs-on",
    pursuers: [WARDEN],
    readings,
    staged: 0,
    restored: 0,
    refusals: [],
    ...over,
  };
}

/** The warden digs out at [5,64,2] at tick 1000, stands up 133 ticks later and walks. */
function walked(): PursuitReading[] {
  return [
    at(1000, [5, 64, 2]),
    at(1020, [5, 64, 2]),
    at(1140, [5, 64, 2]),
    at(1160, [6.5, 64, 4]),
    at(1180, [8, 64, 7]),
  ];
}

const judge = (rec: PursuitStepRecord, arrivals = new Map<string, Arrival>()) =>
  judgePursuedStep(rec, arrivals, width);

test("a pursuer that moves after its window and stands at the end passes, with the closest approach stated", () => {
  const v = judge(step(walked()));
  assert.deepEqual(v.failures, []);
  assert.equal(v.measurement.readings, 5);
  assert.ok(v.measurement.closestApproach !== undefined);
  assert.equal(v.measurement.closestApproach!.toFixed(2), "23.09");
  assert.ok(v.measurement.displacement > 5);
  assert.equal(v.measurement.lowestHealth, 20);
});

test("a death reds `died`, naming the attacker", () => {
  const v = judge(step(walked(), { died: { attacker: "warden#12 (the server named it)" } }));
  assert.equal(v.failures.length, 1);
  assert.match(v.failures[0]!, /died — .*warden#12/);
});

test("a missing pursuer reds `pursuer_lost`", () => {
  const v = judge(step([...walked(), at(1200, undefined)]));
  assert.equal(v.failures.length, 1);
  assert.match(v.failures[0]!, /pursuer_lost — `actor\/the-sleeper` was not read standing at game tick 1200/);
});

test("a pursuer that never leaves its cell after its window reds `pursuer_inert`", () => {
  const still = [at(1000, [5, 64, 2]), at(1100, [5, 64, 2]), at(1200, [5.3, 64, 2.2]), at(1220, [5, 64, 2])];
  const v = judge(step(still));
  assert.equal(v.failures.length, 1);
  assert.match(v.failures[0]!, /pursuer_inert/);
});

test("a pursuer that leaves its cell during the window only reds `pursuer_inert` too", () => {
  // Moved 3 blocks at tick 1060 — inside the 133-tick window — and then stood.
  const early = [at(1000, [5, 64, 2]), at(1060, [8, 64, 2]), at(1140, [5, 64, 2]), at(1180, [5.2, 64, 2])];
  const v = judge(step(early));
  assert.equal(v.failures.length, 1);
  assert.match(v.failures[0]!, /pursuer_inert/);
});

test("a step with no readings reds `unread`", () => {
  const v = judge(step([]));
  assert.equal(v.failures.length, 1);
  assert.match(v.failures[0]!, /unread — no census of `actor\/the-sleeper`/);
});

test("a body staged away or a restore on a pursued step reds `staged` / `restored`", () => {
  const v = judge(step(walked(), { staged: 1, restored: 2 }));
  assert.equal(v.failures.length, 2);
  assert.match(v.failures[0]!, /staged — 1 body/);
  assert.match(v.failures[1]!, /restored — health was restored or refunded 2 time/);
});

test("the window counts from the first reading of the run, across steps", () => {
  const arrivals = new Map<string, Arrival>();
  // The first step reads the arrival and the window; the second sees it move.
  judge(step([at(1000, [5, 64, 2]), at(1100, [5, 64, 2])]), arrivals);
  assert.deepEqual(arrivals.get(WARDEN.actor), { cell: [5, 64, 2], tick: 1000 });
  const second = judge(step([at(1150, [5, 64, 4])]), arrivals);
  assert.deepEqual(second.failures, []);
});

test("an undated pursuer not yet arrived is not lost; one never seen is unread", () => {
  const undated: PursuerRow = { ...WARDEN, undated: true };
  const before = judge(step([at(1000, undefined)], { pursuers: [undated] }));
  assert.equal(before.failures.length, 1);
  assert.match(before.failures[0]!, /unread — `actor\/the-sleeper` was never read standing/);
});

test("the `[pursuit]` line states its binding with its denominator", () => {
  const rec = step(walked());
  const v = judge(rec);
  const line = pursuitLine(pursuitBinding([rec], [v.measurement], 34));
  assert.equal(
    line,
    "[pursuit] 1 pursued step(s) of 34, 1 pursuer(s), 5 reading(s); closest approach 23.09 block(s)",
  );
  assert.equal(
    pursuitLine(pursuitBinding([], [], 34)),
    "[pursuit] 0 pursued step(s) of 34, 0 pursuer(s), 0 reading(s); closest approach none",
  );
});

test("the actor census line is parsed whole, alive with feet ×100 or not standing", () => {
  assert.deepEqual(parseActorCensus("[dw:actor gallery actor/the-sleeper 7 1 51750 6500 450]"), {
    campaignId: "gallery",
    actor: "actor/the-sleeper",
    seq: 7,
    alive: true,
    feet: [517.5, 65, 4.5],
  });
  assert.deepEqual(parseActorCensus("[dw:actor gallery actor/the-sleeper 8 0]"), {
    campaignId: "gallery",
    actor: "actor/the-sleeper",
    seq: 8,
    alive: false,
  });
  assert.equal(parseActorCensus("[dw:actor gallery actor/the-sleeper 8 1]"), undefined);
  assert.equal(parseActorCensus("<bot> [dw:actor gallery actor/the-sleeper 8 0]"), undefined);
});
