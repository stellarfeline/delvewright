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

function run(perTick: (t: number) => readonly SeenEntity[], ticks = 12) {
  const plan = parseShotSubjectPlan(LEDGER);
  const o = new ShotSubjectObserver(plan);
  o.tick("adventure", [0, 0, 0], []);
  for (let t = 0; t < ticks; t++) o.tick("spectator", [17.5, 70.5, 9.5], perTick(t));
  o.tick("adventure", [0, 0, 0], []);
  return { plan, records: o.records };
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
  const { records } = run(() => [ROOT], 7);
  assert.equal(records.length, 1);
  assert.equal(records[0]!.spawnedThrough, false);
});
