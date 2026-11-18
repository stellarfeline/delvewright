import { test } from "node:test";
import assert from "node:assert/strict";
import { DamageTypes, STARVE } from "../src/damage-type.ts";

// The registry's head in 1.21.11 registry order is irrelevant here: the index is
// whatever position the server sent the key at, and only that is read.
const packet = {
  id: "minecraft:damage_type",
  entries: [
    { key: "minecraft:arrow" },
    { key: "minecraft:fall" },
    { key: STARVE, value: undefined },
  ],
};

test("a damage type is named by its place in the registry the server sent", () => {
  const t = new DamageTypes();
  assert.equal(t.size, 0);
  assert.equal(t.name(2), undefined, "nothing is named before the registry arrives");
  t.observe(packet);
  assert.equal(t.size, 3);
  assert.equal(t.name(2), STARVE);
  assert.equal(t.name(1), "minecraft:fall");
});

test("another registry, a missing index and a non-integer name nothing", () => {
  const t = new DamageTypes();
  t.observe({ id: "minecraft:dimension_type", entries: [{ key: "minecraft:overworld" }] });
  assert.equal(t.size, 0, "a registry other than damage types is not read");
  t.observe(packet);
  assert.equal(t.name(3), undefined);
  assert.equal(t.name(-1), undefined);
  assert.equal(t.name(1.5), undefined);
  assert.equal(t.name("2"), undefined);
});
