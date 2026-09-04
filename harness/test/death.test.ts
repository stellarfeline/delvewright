import { test } from "node:test";
import assert from "node:assert/strict";
import { BotDeathError, likelyDeathCause } from "../src/death.ts";

test("likelyDeathCause picks the most recent line starting with the username", () => {
  const chat = [
    "delve-bot joined the game",
    "[dw:complete hello-world obj/greet]",
    "delve-bot was slain by Zombie",
  ];
  assert.equal(likelyDeathCause(chat, "delve-bot"), "delve-bot was slain by Zombie");
});

test("likelyDeathCause returns the latest death line when several match", () => {
  const chat = [
    "delve-bot fell from a high place",
    "some other chatter",
    "delve-bot was blown up by Creeper",
  ];
  assert.equal(likelyDeathCause(chat, "delve-bot"), "delve-bot was blown up by Creeper");
});

test("likelyDeathCause returns undefined when nothing matches", () => {
  assert.equal(likelyDeathCause(["a villager mutters"], "delve-bot"), undefined);
});

test("likelyDeathCause returns undefined for an empty username", () => {
  assert.equal(likelyDeathCause(["delve-bot drowned"], ""), undefined);
});

test("BotDeathError formats position and cause into the message", () => {
  // The position is the body's own, never a block cell: it is carried EXACTLY
  // and only the sentence rounds it, to two decimals.
  const err = new BotDeathError([12.4, 65, -3.6], "delve-bot was slain by Zombie");
  assert.equal(err.name, "BotDeathError");
  assert.deepEqual(err.position, [12.4, 65, -3.6]);
  assert.equal(err.likelyCause, "delve-bot was slain by Zombie");
  assert.match(err.message, /\[12\.40, 65\.00, -3\.60\]/);
  assert.match(err.message, /delve-bot was slain by Zombie/);
});

test("BotDeathError degrades gracefully with no position or cause", () => {
  const err = new BotDeathError(undefined, undefined);
  assert.equal(err.position, undefined);
  assert.equal(err.likelyCause, undefined);
  assert.match(err.message, /unknown position/);
  assert.match(err.message, /cause not found/);
});
