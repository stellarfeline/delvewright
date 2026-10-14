// spec-0110 §7, the live half: the observer's bookkeeping, over the record
// shape the build writes (`validation/shot-subjects.json`).

import { test } from "node:test";
import assert from "node:assert/strict";

import {
  ShotSubjectObserver,
  parseShotSubjectPlan,
  shotSubjectBindingLine,
  shotSubjectFailures,
  type SeenEntity,
} from "../src/shot-subjects.ts";

/** A build's ledger: one cutscene, a mark shot then an assembly shot. */
const LEDGER = {
  cutscenes: [
    {
      path: "/content/quests/0/on_complete/0",
      cutscene_digest: "bca46816",
      start_lens: [17.5, 70.5, 9.5],
      shots: [
        {
          index: 0,
          start_tick: 0,
          ticks: 4,
          kind: "anchor",
          subjects: [{ field: "subject", kind: "anchor", subject: "a:anchor/x@0,0,0", entity: null }],
        },
        {
          index: 1,
          start_tick: 5,
          ticks: 4,
          kind: "assembly",
          subjects: [
            {
              field: "subject",
              kind: "assembly",
              subject: "s:assembly/sentinel",
              entity: "minecraft:item_display",
              body_at: [23.5, 67, 12.5],
            },
          ],
        },
      ],
    },
    // A cutscene that frames no body is not watched.
    {
      path: "/content/triggers/9/effects/0",
      cutscene_digest: "9284e8c7",
      start_lens: [0, 80, 0],
      shots: [{ index: 0, start_tick: 0, ticks: 4, kind: "landscape", subjects: [] }],
    },
  ],
};

const ROOT: SeenEntity = { id: 7, name: "item_display", pos: [23.5, 67, 12.5] };
const LENS = [17.5, 70.5, 9.5] as const;
const PATH = "/content/quests/0/on_complete/0";
/** The game time of driver tick 0. */
const G0 = 1000;

interface Play {
  /** The entity table as the server left it at the end of driver tick `d`
   * (`d = -1` is the start function's tick). */
  table?: (d: number) => readonly SeenEntity[];
  /** Driver ticks the server plays before the end function (the ledger plans 10). */
  total?: number;
  /** Driver ticks whose gap between server ticks no reading lands in. */
  missed?: readonly number[];
  /** Driver ticks in which the client is told a body-typed entity spawned or left. */
  churn?: readonly number[];
  /** Game ticks between the end and the reading after it (1 places it). */
  afterGap?: number;
  /** Where the first lens stands. */
  lens?: readonly [number, number, number];
}

/**
 * One cutscene as the server sends it: the start function's gamemode change
 * in tick `G0 - 1`; each driver tick `d` at game time `G0 + d` (lens packets
 * at the first two, entity churn, then a reading in the gap after it); the
 * end function in the same game tick as the last driver tick, after which one
 * reading answers.
 */
function play(o: ShotSubjectObserver, p: Play = {}) {
  const table = p.table ?? (() => [ROOT]);
  const total = p.total ?? 10;
  const missed = new Set(p.missed ?? []);
  const churn = new Set(p.churn ?? []);
  const last = total - 1;
  o.gameModeChanged("spectator", []);
  if (churn.has(-1)) o.entityChanged("item_display");
  o.clock(G0 - 1, table(-1));
  for (let d = 0; d < last; d++) {
    if (d < 2) o.lensTick("spectator", p.lens ?? LENS);
    if (churn.has(d)) o.entityChanged("minecraft:item_display");
    if (!missed.has(d)) o.clock(G0 + d, table(d));
  }
  o.gameModeChanged("adventure", table(last - 1));
  o.clock(G0 + last + (p.afterGap ?? 1) - 1, []);
}

function run(p: Play = {}) {
  const plan = parseShotSubjectPlan(LEDGER);
  const o = new ShotSubjectObserver(plan);
  play(o, p);
  return { plan, records: o.records, spans: o.spans, watching: o.watching };
}

test("only cutscenes that frame a body are watched", () => {
  const plan = parseShotSubjectPlan(LEDGER);
  assert.equal(plan.cutscenes.length, 1);
  assert.equal(plan.cutscenes[0]!.shots.length, 1);
  assert.equal(plan.cutscenes[0]!.driverTicks, 10);
});

test("a body spawned before its shot and kept through it reads true", () => {
  const { plan, records, spans, watching } = run();
  assert.equal(watching, false);
  assert.equal(records.length, 1);
  assert.deepEqual([records[0]!.spawnedBefore, records[0]!.spawnedThrough], [true, true]);
  assert.deepEqual(spans, [{ cutscene: PATH, planned: 10, placed: true, read: 10, unplaced: 0, overrun: false }]);
  assert.deepEqual(shotSubjectFailures(records, spans), []);
  assert.match(
    shotSubjectBindingLine(plan, records, spans),
    /1 of 1 framed body row.*1 spawned before and through.*1 of 1 cutscene run\(s\) placed on the server's clock, 10 of 10 driver tick\(s\) read, 0 unplaced/,
  );
});

// The defect this clock removes: a shot that ends its cutscene. Its last
// driver tick and the end function share one server tick, so no client sees
// that tick in spectator; counting the client's own physics ticks recorded
// the shot unfinished, as a body that left. On the server's clock the end
// is the last driver tick, and the row reads true.
test("a shot that ends its cutscene is closed by the end, not recorded as a body that left", () => {
  const { records } = run();
  assert.deepEqual(
    records.map((r) => [r.shot, r.spawnedBefore, r.spawnedThrough]),
    [[1, true, true]],
  );
});

test("a body that is not there at the shot's first tick reads false", () => {
  const { records } = run({ table: (d) => (d >= 7 ? [ROOT] : []), churn: [7] });
  assert.equal(records[0]!.spawnedBefore, false);
  assert.equal(shotSubjectFailures(records).length, 1);
});

test("a body that leaves mid-shot reads spawned before, not through", () => {
  const { records } = run({ table: (d) => (d === 7 ? [] : [ROOT]), churn: [7, 8] });
  assert.deepEqual([records[0]!.spawnedBefore, records[0]!.spawnedThrough], [true, false]);
});

test("a body gone at the end reads not through", () => {
  const { records } = run({ table: (d) => (d === 8 ? [] : [ROOT]), churn: [8] });
  assert.deepEqual([records[0]!.spawnedBefore, records[0]!.spawnedThrough], [true, false]);
});

test("an entity of another type or far from the mark is not the body", () => {
  const { records } = run({
    table: () => [
      { id: 8, name: "block_display", pos: [23.5, 67, 12.5] },
      { id: 9, name: "item_display", pos: [40, 67, 12.5] },
    ],
  });
  assert.equal(records[0]!.spawnedBefore, false);
});

test("a tick no reading landed on takes its neighbours' table when nothing churned", () => {
  const { records, spans } = run({ missed: [5, 6] });
  assert.deepEqual([records[0]!.spawnedBefore, records[0]!.spawnedThrough], [true, true]);
  assert.deepEqual([spans[0]!.read, spans[0]!.unplaced], [8, 0]);
});

test("a tick no reading landed on, with churn around it, is unplaced and counts as absent", () => {
  const { records, spans } = run({ missed: [6], churn: [6] });
  assert.deepEqual([records[0]!.spawnedBefore, records[0]!.spawnedThrough], [true, false]);
  assert.deepEqual([spans[0]!.read, spans[0]!.unplaced], [9, 1]);
  assert.match(shotSubjectFailures(records, spans)[0]!, /1 tick\(s\) of the shot no reading placed/);
});

test("an end the readings do not place judges nothing and fails", () => {
  const { records, spans } = run({ missed: [8], afterGap: 1 });
  // The last reading before the end is at driver tick 7; the one after is at
  // the end's tick: two game ticks apart, so the end could be either.
  assert.deepEqual(records, []);
  assert.equal(spans[0]!.placed, false);
  assert.match(shotSubjectFailures(records, spans)[0]!, /not one game tick apart/);
});

test("a cutscene that plays longer than its plan fails", () => {
  const { spans, records } = run({ total: 12 });
  assert.equal(spans[0]!.overrun, true);
  assert.match(shotSubjectFailures(records, spans).join("\n"), /longer from its start to its end than its 10 planned/);
});

test("a cutscene whose first lens is no watched cutscene's is not recorded", () => {
  const { records, spans, watching } = run({ lens: [0, 80, 0] });
  assert.deepEqual([records, spans, watching], [[], [], false]);
});

test("a lens packet outside spectator names nothing", () => {
  const plan = parseShotSubjectPlan(LEDGER);
  const o = new ShotSubjectObserver(plan);
  o.lensTick("adventure", LENS);
  o.clock(G0, [ROOT]);
  o.gameModeChanged("adventure", [ROOT]);
  assert.deepEqual([o.records, o.spans, o.watching], [[], [], false]);
});
