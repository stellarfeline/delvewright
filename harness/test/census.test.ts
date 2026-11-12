import { test } from "node:test";
import assert from "node:assert/strict";

import type { CensusRow, ReachStep } from "../src/critical-path.ts";
import { parseCensus } from "../src/critical-path.ts";
import { censusCentre, censusCommands, censusReleaseCommands, censusTerm, censusVerdict } from "../src/census.ts";
import { methods as censusMethods } from "../src/executor/census.ts";
import { methods as walkMethods } from "../src/executor/walk.ts";
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

test("a census puts the needed bodies of the set in the box's centre, nearest first, and holds them", () => {
  assert.deepEqual(censusCentre(row), [11.5, 64, 21.5]);
  assert.deepEqual(censusCommands(row), [
    "/tag @e[tag=dw_wave_goats,limit=3,sort=nearest] add dw_census_placed",
    "/tp @e[tag=dw_census_placed] 11.5 64 21.5",
    "/execute as @e[tag=dw_census_placed,nbt=!{NoAI:1b}] run tag @s add dw_census_held",
    "/execute as @e[tag=dw_census_held] run data merge entity @s {NoAI:1b}",
    "/tag @e[tag=dw_census_placed] remove dw_census_placed",
  ]);
  assert.deepEqual(censusCommands({ ...row, count: 0 }), []);
  // Only the bodies the census stilled are woken: a puppet placed already
  // without its AI never carried the held tag.
  assert.deepEqual(censusReleaseCommands(), [
    "/execute as @e[tag=dw_census_held] run data merge entity @s {NoAI:0b}",
    "/execute as @e[tag=dw_census_held] run tag @s remove dw_census_held",
  ]);
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

/** A path of one census-gated reach between the frame steps. */
function pathOf(step: unknown) {
  return {
    version: "0.0.0",
    formatVersion: 8,
    campaignId: "c",
    nonCombatants: { kinds: [], examined: 0, ambiguous: [], unbound: false },
    steps: [
      { action: "select-class", class: "class/a", command: "/trigger dw.class set 1" },
      step,
      { action: "assert-complete", scoreboard: { objective: "dw.campaign", value: 1 } },
    ],
  } as never;
}

const penReach = {
  action: "reach",
  objective: "obj/pen",
  anchor: "anchor/pen",
  pos: [11, 64, 21],
  radius: 2,
  completion: { kind: "sphere", pos: [11, 64, 21], radius: 2 },
  census: [row, { ...row, state: "state/hens-home", objective: "dw.s_hens_home", select: "tag=dw_wave_hens", count: 2 }],
};

/**
 * An executor whose census is the real one, over a server that answers every
 * command and counts what was placed; `reach` walks, reaches its stance, and
 * acts. Every event lands in `log` in the order it happened.
 */
function censusExecutor(reach: (self: { atStance?: () => Promise<void> }, log: string[]) => Promise<void>) {
  const log: string[] = [];
  const self = {
    ...censusMethods,
    stanceDue: undefined as (() => Promise<void>) | undefined,
    censusResults: [] as unknown[],
    async bracketed(commands: readonly string[]) {
      log.push(commands.some((c) => c.includes("{NoAI:0b}")) ? "release" : `place ${commands.length}`);
      return { answered: true, lines: [] as string[] };
    },
    async askTerm() {
      log.push("read");
      return true;
    },
    async selectClass() {},
    async assertComplete() {},
    async reach(this: { atStance?: () => Promise<void> }) {
      await reach(this, log);
    },
  };
  return { self, log };
}

test("a census is placed at the act's stance — after the walk, every body at once — and let go after the act", async () => {
  const { self, log } = censusExecutor(async (me, log) => {
    log.push("walk");
    await me.atStance?.();
    log.push("act");
  });
  await runSequence(pathOf(penReach), self as never);
  // Before the fix the census ran before the step was dispatched, so the
  // bodies were placed while the bot still stood a walk away: place, read,
  // place, read, walk, act.
  assert.deepEqual(log, ["walk", "place 10", "read", "read", "act", "release"]);
});

test("a census-gated act that ends without reaching its stance is refused by name", async () => {
  const { self, log } = censusExecutor(async (_me, log) => {
    log.push("act");
  });
  await assert.rejects(
    runSequence(pathOf(penReach), self as never),
    (e: unknown) => /never placed — the step's act ended without reaching its stance/.test(String((e as { cause?: unknown }).cause ?? e)),
  );
  assert.deepEqual(log, ["act"], "nothing was placed, so nothing is let go");
});

test("the held bodies are let go when the act fails after its stance", async () => {
  const { self, log } = censusExecutor(async (me, log) => {
    await me.atStance?.();
    log.push("act");
    throw new Error("the latch never answered");
  });
  await assert.rejects(runSequence(pathOf(penReach), self as never), /latch never answered/);
  assert.deepEqual(log, ["place 10", "read", "read", "act", "release"]);
});

test("the executor's reach places its census after its walk and before its completion is read", async () => {
  const log: string[] = [];
  const self = {
    async walkTo() {
      log.push("walk");
    },
    async atStance() {
      log.push("stance");
    },
    async requireObjective() {
      log.push("read completion");
    },
  };
  await walkMethods.reach.call(self as never, penReach as never);
  assert.deepEqual(log, ["walk", "stance", "read completion"]);
});
