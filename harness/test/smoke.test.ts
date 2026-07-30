import { test } from "node:test";
import assert from "node:assert/strict";

// Trivial smoke test so the harness has a runnable `test` target on the empty
// stub. Real bot navigation/assertions arrive with spec-0003.
test("harness smoke", () => {
  const sum: number = 1 + 1;
  assert.equal(sum, 2);
});
