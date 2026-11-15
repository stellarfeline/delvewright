// The run's wall-clock budget is derived from the build it walks: a bigger
// campaign gets a bigger budget, and an override is an explicit, printed act.
import { test } from "node:test";
import assert from "node:assert/strict";
import type { Step, Vec3Tuple } from "../src/critical-path.ts";
import { OBJECTIVE_TIMEOUT_MS } from "../src/executor/objective.ts";
import { KILL_TIMEOUT_MS } from "../src/executor/wave.ts";
import {
  BUDGET_MARGIN,
  DEATH_MS,
  WALK_FLOOR_BLOCKS_PER_S,
  budgetFromEnv,
  describeBudget,
  runBudget,
  type BudgetInputs,
} from "../src/run-budget.ts";
import { DIE_RETRY_DEATHS } from "../src/combat.ts";

/** `n` reach steps 40 blocks apart along x, with one proven leg per step. */
function path(n: number): Pick<BudgetInputs, "steps" | "legs"> {
  const steps: Step[] = [];
  const legs: { waypoints: Vec3Tuple[] }[] = [];
  for (let i = 0; i < n; i++) {
    const pos: Vec3Tuple = [i * 40, 64, 0];
    steps.push({ action: "reach", objective: `obj/o${i}`, pos, pursuers: [] } as unknown as Step);
    if (i > 0) legs.push({ waypoints: [[(i - 1) * 40, 64, 0], [i * 40 - 20, 64, 10], pos] });
  }
  return { steps, legs };
}

const none = { placementMs: 300_000, dieRetryEncounters: 0, lethalVolumes: 0, crushGates: 0 };

test("a 46-step path gets more than the 20 minutes a fixed budget gave it", () => {
  const b = runBudget({ ...path(46), ...none });
  assert.ok(b.ms > 20 * 60 * 1000, `derived ${b.ms}ms`);
  // At least every step's completion wait, times the margin.
  assert.ok(b.ms >= 46 * OBJECTIVE_TIMEOUT_MS * BUDGET_MARGIN);
});

test("the budget grows with the campaign: steps, walk, fights and deaths", () => {
  const small = runBudget({ ...path(10), ...none }).ms;
  const big = runBudget({ ...path(46), ...none }).ms;
  assert.ok(big > small, `${big} vs ${small}`);
  const walked = path(2);
  const far = {
    ...walked,
    legs: [{ waypoints: [[0, 64, 0], [400, 64, 0]] as Vec3Tuple[] }],
  };
  const extra = runBudget({ ...far, ...none }).ms - runBudget({ ...walked, ...none }).ms;
  assert.ok(extra > 0, "a longer proven leg costs more");
  const deaths = runBudget({ ...path(10), ...none, dieRetryEncounters: 3 }).ms - small;
  assert.ok(
    Math.abs(deaths - Math.ceil(3 * DIE_RETRY_DEATHS * (DEATH_MS + KILL_TIMEOUT_MS) * BUDGET_MARGIN)) <= 1,
    `three encounters' scripted deaths: ${deaths}`,
  );
});

test("a path with no waypoint artifact is charged its detoured straight line", () => {
  const { steps } = path(5);
  const b = runBudget({ steps, ...none });
  const walk = b.terms.find((t) => t.what.includes("walked"))!;
  // 4 gaps of 40 blocks, three times over, at the floor speed.
  assert.equal(walk.ms, ((4 * 40 * 3) / WALK_FLOOR_BLOCKS_PER_S) * 1000);
});

test("an explicit override is honoured and said so; a malformed one refuses", () => {
  const b = runBudget({ ...path(5), ...none });
  assert.deepEqual(budgetFromEnv(b, {}), { ms: b.ms, overridden: false });
  assert.deepEqual(budgetFromEnv(b, { DELVEWRIGHT_RUN_TIMEOUT_MS: "" }), { ms: b.ms, overridden: false });
  const o = budgetFromEnv(b, { DELVEWRIGHT_RUN_TIMEOUT_MS: "1234" });
  assert.deepEqual(o, { ms: 1234, overridden: true });
  assert.match(describeBudget(b, o), /OVERRIDDEN/);
  assert.doesNotMatch(describeBudget(b, budgetFromEnv(b, {})), /OVERRIDDEN/);
  assert.throws(() => budgetFromEnv(b, { DELVEWRIGHT_RUN_TIMEOUT_MS: "20m" }));
});
