import { test } from "node:test";
import assert from "node:assert/strict";
import { BotDeathError, likelyDeathCause } from "../src/death.ts";

test("likelyDeathCause picks the most recent line starting with the username", () => {
  const chat = [
    "delve-bot joined the game",
    "[Delvewright] complete dw.campaign 0",
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
  const err = new BotDeathError([12, 65, -4], "delve-bot was slain by Zombie");
  assert.equal(err.name, "BotDeathError");
  assert.deepEqual(err.position, [12, 65, -4]);
  assert.equal(err.likelyCause, "delve-bot was slain by Zombie");
  assert.match(err.message, /\[12, 65, -4\]/);
  assert.match(err.message, /delve-bot was slain by Zombie/);
});

test("BotDeathError degrades gracefully with no position or cause", () => {
  const err = new BotDeathError(undefined, undefined);
  assert.equal(err.position, undefined);
  assert.equal(err.likelyCause, undefined);
  assert.match(err.message, /unknown position/);
  assert.match(err.message, /cause not found/);
});
