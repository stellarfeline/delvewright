import { test } from "node:test";
import assert from "node:assert/strict";
import { BotDeathError, likelyDeathCause, linesSince } from "../src/death.ts";

// --- the chat window: a bounded ring, read without losing the answer ---------

test("a window is read from the STREAM's index, not the ring's length", () => {
  // The defect this replaced: mark `ring.length` on a full ring and `slice(mark)`
  // is empty for the rest of the run, so a command's reply reads as silence
  // however loudly the server answered. Here the ring holds the last 3 of 5 lines.
  const ring = ["c", "d", "e"];
  assert.deepEqual(linesSince(ring, 5, 5), { lines: [], lost: 0 });
  assert.deepEqual(linesSince(ring, 5, 4), { lines: ["e"], lost: 0 });
  assert.deepEqual(linesSince(ring, 5, 2), { lines: ["c", "d", "e"], lost: 0 });
});

test("a window that lost lines to eviction SAYS so — it did not observe nothing", () => {
  assert.deepEqual(linesSince(["c", "d", "e"], 5, 0), { lines: ["c", "d", "e"], lost: 2 });
});

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
