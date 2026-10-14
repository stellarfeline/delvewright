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

/**
 * One cutscene as the server plays it: driver tick `t` sends a lens packet,
 * and the end function runs in the same server tick as the last driver tick
 * (`total - 1`), its gamemode change arriving after that tick's lens packet.
 * `perTick(t)` is the entity table as the server left it at the end of tick
 * `t`; `total` is the driver ticks played (the ledger plans 10).
 */
function play(o: ShotSubjectObserver, perTick: (t: number) => readonly SeenEntity[], total: number) {
  o.gameModeChanged("spectator", []);
  for (let t = 0; t < total; t++) o.lensTick("spectator", LENS, t === 0 ? [] : perTick(t - 1));
  o.gameModeChanged("adventure", perTick(total - 1));
}

function run(perTick: (t: number) => readonly SeenEntity[], total = 10) {
  const plan = parseShotSubjectPlan(LEDGER);
  const o = new ShotSubjectObserver(plan);
  play(o, perTick, total);
  return { plan, records: o.records, spans: o.spans };
}

test("only cutscenes that frame a body are watched", () => {
  const plan = parseShotSubjectPlan(LEDGER);
  assert.equal(plan.cutscenes.length, 1);
  assert.equal(plan.cutscenes[0]!.shots.length, 1);
});

test("a body spawned before its shot and kept through it reads true", () => {
  const { plan, records } = run(() => [ROOT]);
  assert.equal(records.length, 1);
  assert.deepEqual(
    [records[0]!.spawnedBefore, records[0]!.spawnedThrough],
    [true, true],
  );
  assert.deepEqual(shotSubjectFailures(records), []);
  assert.match(shotSubjectBindingLine(plan, records), /1 of 1 framed body row/);
});

test("a body that is not there at the shot's first tick reads false", () => {
  const { records } = run((t) => (t >= 7 ? [ROOT] : []));
  assert.equal(records[0]!.spawnedBefore, false);
  assert.equal(shotSubjectFailures(records).length, 1);
});

test("a body that leaves mid-shot reads spawned before, not through", () => {
  const { records } = run((t) => (t === 7 ? [] : [ROOT]));
  assert.deepEqual(
    [records[0]!.spawnedBefore, records[0]!.spawnedThrough],
    [true, false],
  );
});

test("an entity of another type or far from the mark is not the body", () => {
  const { records } = run(() => [
    { id: 8, name: "block_display", pos: [23.5, 67, 12.5] },
    { id: 9, name: "item_display", pos: [40, 67, 12.5] },
  ]);
  assert.equal(records[0]!.spawnedBefore, false);
});

test("a cutscene cut short records its unfinished shot as not through", () => {
  const { records, spans } = run(() => [ROOT], 7);
  assert.equal(records.length, 1);
  assert.equal(records[0]!.spawnedThrough, false);
  assert.deepEqual(spans, [{ cutscene: "/content/quests/0/on_complete/0", played: 7, planned: 10 }]);
  assert.match(shotSubjectFailures(records, spans).join("\n"), /played 7 driver tick\(s\) of the 10/);
});

// The defect this clock removes: a shot that ends its cutscene. Its last
// driver tick and the end function share one server tick, so a client never
// sees that tick in spectator; counting the client's own physics ticks
// recorded the shot unfinished, as a body that left. On the server's clock
// the end closes the last tick, and the row reads true.
test("a shot that ends its cutscene is closed by the end, not recorded as a body that left", () => {
  const { plan, records, spans } = run(() => [ROOT]);
  assert.deepEqual(
    records.map((r) => [r.shot, r.spawnedBefore, r.spawnedThrough]),
    [[1, true, true]],
  );
  assert.equal(plan.cutscenes[0]!.driverTicks, 10);
  assert.deepEqual(spans, [{ cutscene: "/content/quests/0/on_complete/0", played: 10, planned: 10 }]);
  assert.deepEqual(shotSubjectFailures(records, spans), []);
  assert.match(shotSubjectBindingLine(plan, records, spans), /10 of 10 planned driver tick\(s\) played/);
});

test("a body that leaves on the cutscene's last tick reads not through", () => {
  const { records } = run((t) => (t === 9 ? [] : [ROOT]));
  assert.deepEqual([records[0]!.spawnedBefore, records[0]!.spawnedThrough], [true, false]);
});

test("a body spawned to the client as the lens arrives counts as there at tick 0's end", () => {
  // The client is teleported to the lens in tick 0; the server tracks what
  // stands near the lens at the end of that tick.
  const plan = parseShotSubjectPlan({
    cutscenes: [{ ...LEDGER.cutscenes[0]!, shots: [{ ...LEDGER.cutscenes[0]!.shots[1]!, start_tick: 0, ticks: 9 }] }],
  });
  const o = new ShotSubjectObserver(plan);
  play(o, () => [ROOT], 10);
  assert.deepEqual([o.records[0]!.spawnedBefore, o.records[0]!.spawnedThrough], [true, true]);
});

test("a cutscene whose first lens is no watched cutscene's is not recorded", () => {
  const plan = parseShotSubjectPlan(LEDGER);
  const o = new ShotSubjectObserver(plan);
  o.gameModeChanged("spectator", []);
  for (let t = 0; t < 10; t++) o.lensTick("spectator", [0, 80, 0], [ROOT]);
  o.gameModeChanged("adventure", [ROOT]);
  assert.deepEqual([o.records, o.spans], [[], []]);
});

test("a lens packet outside spectator is not a driver tick", () => {
  const plan = parseShotSubjectPlan(LEDGER);
  const o = new ShotSubjectObserver(plan);
  o.lensTick("adventure", LENS, [ROOT]);
  o.gameModeChanged("adventure", [ROOT]);
  assert.deepEqual([o.records, o.spans], [[], []]);
});
