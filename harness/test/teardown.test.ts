import { test } from "node:test";
import assert from "node:assert/strict";
import {
  classifyDeathDepth,
  classifyNamedEntityDeaths,
  scriptedTeardownThreshold,
  type NamedEntityDeath,
} from "../src/teardown.ts";

test("with no world min_y, the fallback threshold is -100", () => {
  assert.equal(scriptedTeardownThreshold(), -100);
});

test("with a world min_y, the threshold derives as minY - 64", () => {
  assert.equal(scriptedTeardownThreshold(0), -64);
  assert.equal(scriptedTeardownThreshold(-64), -128); // vanilla overworld default
});

test("a death at the island's observed vanish depth (-128) classifies as scripted_teardown", () => {
  assert.equal(classifyDeathDepth(-128), "scripted_teardown");
});

test("a death well within the playable map classifies as combat", () => {
  assert.equal(classifyDeathDepth(64), "combat");
  assert.equal(classifyDeathDepth(-55), "combat"); // the island herdsman's own Y
});

test("the threshold is inclusive: exactly -100 is a scripted teardown, one above is not", () => {
  assert.equal(classifyDeathDepth(-100), "scripted_teardown");
  assert.equal(classifyDeathDepth(-99), "combat");
});

test("classifyNamedEntityDeaths reclassifies a batch without dropping any entry — the island's five", () => {
  const deaths: NamedEntityDeath[] = [
    { name: "Hollow Gate-Warder", entityId: 1, position: [10, 63, -4] },
    { name: "Hollow Wall-Warder", entityId: 2, position: [12, 61, -6] },
    { name: "The Bellkeeper", entityId: 3, position: [101, 93, -99] },
    { name: "island-herdsman", entityId: 4, position: [10, -128, 9] }, // scripted vanish
    { name: "island-crew", entityId: 5, position: [-3, -128, 12] }, // scripted vanish
  ];
  const classified = classifyNamedEntityDeaths(deaths);
  assert.equal(classified.length, 5, "no entry is dropped — reclassify, never suppress");
  assert.deepEqual(
    classified.map((d) => d.kind),
    ["combat", "combat", "combat", "scripted_teardown", "scripted_teardown"],
  );
  // Every other field carries through untouched.
  assert.equal(classified[0]!.name, "Hollow Gate-Warder");
  assert.deepEqual(classified[3]!.position, [10, -128, 9]);
});
