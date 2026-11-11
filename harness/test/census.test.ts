import { test } from "node:test";
import assert from "node:assert/strict";

import type { CensusRow, ReachStep } from "../src/critical-path.ts";
import { parseCensus } from "../src/critical-path.ts";
import { censusCentre, censusCommands, censusTerm, censusVerdict } from "../src/census.ts";
import { runSequence } from "../src/sequencer.ts";

const row: CensusRow = {
  state: "state/goats-home",
  objective: "dw.s_goats_home",
  select: "tag=dw_wave_goats",
  region: [
    [10, 64, 20],
    [12, 65, 22],
  ],
  count: 3,
};

test("a census puts the needed bodies of the set in the box's centre, nearest first", () => {
  assert.deepEqual(censusCentre(row), [11.5, 64, 21.5]);
  assert.deepEqual(censusCommands(row), [
    "/tp @e[tag=dw_wave_goats,limit=3,sort=nearest] 11.5 64 21.5",
  ]);
  assert.deepEqual(censusCommands({ ...row, count: 0 }), []);
});

test("the count read back is the party's, and exactly the bodies placed", () => {
  assert.deepEqual(censusTerm(row), {
    objective: "dw.s_goats_home",
    holder: "#party",
    min: 3,
    max: 3,
    negate: false,
  });
});

test("a count that disagrees with the bodies placed refuses the step by name", () => {
  assert.equal(censusVerdict(row, true).passed, true);
  const no = censusVerdict(row, false);
  assert.equal(no.passed, false);
  assert.match(no.detail, /state\/goats-home.*disagrees with the 3 body/);
  const unread = censusVerdict(row, undefined);
  assert.equal(unread.passed, false);
  assert.match(unread.detail, /never said/);
});

test("a step's census parses whole and refuses an unknown key", () => {
  const wire = [{ ...row, region: [[10, 64, 20], [12, 65, 22]] }];
  assert.deepEqual(parseCensus(wire, "/steps/1/census"), [row]);
  assert.deepEqual(parseCensus(undefined, "/steps/1/census"), []);
  assert.throws(() => parseCensus([{ ...wire[0], extra: 1 }], "/steps/1/census"), /extra/);
});

test("a step gated on a census against an executor that cannot perform one fails by name", async () => {
  const reach: ReachStep = {
    action: "reach",
    objective: "obj/pen",
    anchor: "anchor/pen",
    pos: [11, 64, 21],
    radius: 2,
    census: [row],
  } as never;
  const calls: string[] = [];
  const executor = new Proxy(
    {},
    {
      get: (_t, prop) => {
        if (prop === "performCensus" || prop === "pursue" || prop === "then") return undefined;
        return async () => {
          calls.push(String(prop));
        };
      },
    },
  );
  await assert.rejects(
    runSequence(
      {
        version: "0.0.0",
        formatVersion: 8,
        campaignId: "c",
        nonCombatants: { kinds: [], examined: 0, ambiguous: [], unbound: false },
        steps: [
          { action: "select-class", class: "class/a", command: "/trigger dw.class set 1" },
          reach,
          { action: "assert-complete", scoreboard: { objective: "dw.campaign", value: 1 } },
        ],
      } as never,
      executor as never,
    ),
    (e: unknown) => /cannot perform a census/.test(String((e as { cause?: unknown }).cause ?? e)),
  );
  assert.ok(!calls.includes("reach"), "the step is not performed without its census");
});
