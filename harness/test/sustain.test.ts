import { test } from "node:test";
import assert from "node:assert/strict";
import {
  EAT_HEALTH_FRACTION,
  EAT_SAFE_RANGE,
  eatDecision,
  pickFood,
} from "../src/sustain.ts";

const BASE = {
  maxHealth: 20,
  food: 14,
  maxFood: 20,
  nearestHostileDistance: undefined,
  hasFood: true,
} as const;

test("a healthy bot does not stop to eat", () => {
  assert.equal(
    eatDecision({ ...BASE, health: 20 * EAT_HEALTH_FRACTION + 0.1 }),
    "healthy",
  );
});

test("a hurt bot with food and no mob in reach eats", () => {
  assert.equal(eatDecision({ ...BASE, health: 8 }), "eat");
  // Exactly at the threshold counts as hurt (the boundary is inclusive).
  assert.equal(eatDecision({ ...BASE, health: 20 * EAT_HEALTH_FRACTION }), "eat");
});

test("a hurt bot with a hostile in reach keeps fighting instead of eating", () => {
  assert.equal(
    eatDecision({ ...BASE, health: 6, nearestHostileDistance: EAT_SAFE_RANGE }),
    "hostile-near",
  );
  // Just outside reach → eating is safe.
  assert.equal(
    eatDecision({ ...BASE, health: 6, nearestHostileDistance: EAT_SAFE_RANGE + 0.1 }),
    "eat",
  );
});

test("full hunger reports itself rather than pretending to eat", () => {
  // Vanilla forbids eating ordinary food at 20/20 (mineflayer throws "Food is full").
  // The distinct outcome is what tells a log reader "hurt but regenerating".
  assert.equal(eatDecision({ ...BASE, health: 6, food: 20 }), "hunger-full");
});

test("an empty pack reports no-food, and is not confused with hunger", () => {
  assert.equal(eatDecision({ ...BASE, health: 6, hasFood: false }), "no-food");
});

test("pickFood takes the most nourishing item, ties broken by name", () => {
  assert.equal(
    pickFood([
      { name: "bread", foodPoints: 5 },
      { name: "rabbit_stew", foodPoints: 10 },
      { name: "apple", foodPoints: 4 },
    ])?.name,
    "rabbit_stew",
  );
  assert.equal(
    pickFood([
      { name: "cooked_cod", foodPoints: 5 },
      { name: "bread", foodPoints: 5 },
    ])?.name,
    "bread",
    "deterministic tie-break, so the same inventory always eats the same thing",
  );
  assert.equal(pickFood([]), undefined);
});
