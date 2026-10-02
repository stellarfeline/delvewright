import { test } from "node:test";
import assert from "node:assert/strict";

import { echoCommand, ReplyBrackets } from "../src/command-reply.ts";

test("each bracket holds its own command's reply, whatever arrived around it", async () => {
  const brackets = new ReplyBrackets();
  const strike = brackets.begin();
  const blow = brackets.begin();
  // Wire order: the strike's bracket, then the blow's — and a stray line from a
  // command sent outside any bracket between them.
  for (const line of [
    strike.open,
    "Running function vesperhold:wave_strike_walk_ambush",
    strike.close,
    "Target is invulnerable to the given damage type",
    blow.open,
    "Target is invulnerable to the given damage type",
    blow.close,
  ]) {
    brackets.observe(line);
  }
  assert.deepEqual(await brackets.reply(strike, 1_000), {
    lines: ["Running function vesperhold:wave_strike_walk_ambush"],
    answered: true,
  });
  assert.deepEqual(await brackets.reply(blow, 1_000), {
    lines: ["Target is invulnerable to the given damage type"],
    answered: true,
  });
});

test("a marker is protocol: observe says so, and a reply waited on before it closes resolves", async () => {
  const brackets = new ReplyBrackets();
  const b = brackets.begin();
  const pending = brackets.reply(b, 1_000);
  assert.equal(brackets.observe(b.open), true);
  assert.equal(brackets.observe("Applied 100000.0 damage to Hired Knife"), false);
  assert.equal(brackets.observe(b.close), true);
  assert.deepEqual(await pending, { lines: ["Applied 100000.0 damage to Hired Knife"], answered: true });
});

test("a bracket whose closing marker never arrives is unanswered, never an empty reply", async () => {
  const brackets = new ReplyBrackets();
  const b = brackets.begin();
  brackets.observe(b.open);
  assert.deepEqual(await brackets.reply(b, 30), { lines: [], answered: false });
});

test("the echo command is a tellraw of the marker text alone", () => {
  const b = new ReplyBrackets().begin();
  assert.equal(echoCommand(b.open), `/tellraw @s {"text":"[dw:cmd 1 open]"}`);
});
