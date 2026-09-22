import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { REJECTION, isRejection } from "../src/rejection.ts";

const HERE = path.dirname(fileURLToPath(import.meta.url));
const RCON_MJS = path.resolve(HERE, "../../tools/lib/rcon.mjs");

/**
 * The harness's copy of the rejection rule and the repo's own must be the same
 * rule, character for character.
 *
 * The harness ships as a container carrying `harness/` and nothing else, so it
 * cannot import `tools/lib/rcon.mjs`; the copy is the price of that, and this is
 * what keeps the price at zero. A shape enters the rule on evidence, in
 * `rcon.mjs`, and this test reds until the harness follows.
 */
test("the harness's rejection rule is `tools/lib/rcon.mjs`'s, exactly", () => {
  const mjs = readFileSync(RCON_MJS, "utf8");
  const start = mjs.indexOf("export const REJECTION = new RegExp(");
  assert.notEqual(start, -1, "rcon.mjs still defines REJECTION");
  const end = mjs.indexOf(");", start);
  // rcon.mjs's own comments sit BETWEEN the pieces and quote server replies, so
  // they are stripped before the literals are read — the comparison is the
  // PATTERN, never the prose around it.
  const body = mjs
    .slice(start, end)
    .split("\n")
    .filter((line) => !line.trim().startsWith("//"))
    .join("\n");
  const pieces = [...body.matchAll(/"(?:[^"\\]|\\.)*"/g)].map(
    (m) => JSON.parse(m[0]) as string,
  );
  assert.ok(pieces.length > 0, "the pattern is assembled from string literals");
  const theirs = new RegExp(pieces.join(""));
  assert.equal(
    theirs.source,
    REJECTION.source,
    "harness/src/rejection.ts and tools/lib/rcon.mjs disagree about what a refusal looks like",
  );
});

test("the shapes the pinned server refuses with are recognised", () => {
  assert.equal(isRejection("Incorrect argument for commandgamerule fallDamage<--[HERE]"), true);
  assert.equal(isRejection("No entity was found"), true);
  assert.equal(isRejection("That position is not loaded"), true);
  assert.equal(isRejection("Unable to modify player data"), true);
  // A command's ordinary answer is not a refusal, however alarming it reads.
  assert.equal(isRejection("Applied 100000 damage to Unremembered Guard"), false);
  assert.equal(isRejection("#wmus_n has 5 [dw.sys]"), false);
});
