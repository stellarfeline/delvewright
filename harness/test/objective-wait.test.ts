import { test } from "node:test";
import assert from "node:assert/strict";
import { OBJECTIVE_TIMEOUT_MS, objectiveWait } from "../src/executor/objective.ts";

// Format 18: a step whose objective runs a race's course waits for the race to
// be over beyond its ordinary completion wait — the town's 600-tick grace is
// 30 s more, so a race that ends by its grace is not a race against the wait.
test("a step's completion wait grows by the race it may wait on", () => {
  assert.equal(objectiveWait(0), OBJECTIVE_TIMEOUT_MS);
  assert.equal(objectiveWait((600 * 1000) / 20), OBJECTIVE_TIMEOUT_MS + 30_000);
  assert.equal(objectiveWait(-1), OBJECTIVE_TIMEOUT_MS);
});
