import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, mkdir, writeFile, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import {
  ContainerLedgerParseError,
  judgeNotice,
  loadContainerLedgerForCriticalPath,
  parseContainerLedger,
} from "../src/containers.ts";

// spec-0105 §5.3: the arrival line the census ledger names reaches the bot in
// the window between its class command and the warp in. Each failure mode is
// planted here and must red.

const TEXT = "The long hall was emptied long ago, and the chest in its end room holds nothing.";

const chatLedger = { cells: 6, decoration: 1, notice: { text: TEXT, style: "chat", sound: null }, rows: [] };

/** The bot after the warp, and a window holding only what a test puts in it. */
const ARRIVAL = [8195.5, 64, 8202.5] as const;
const quiet = { sounds: [], arrival: ARRIVAL };

test("a declared chat notice that arrives passes", () => {
  const v = judgeNotice(parseContainerLedger(chatLedger), {
    chat: ["New objective: Take the emeralds", TEXT],
    titles: [],
    lost: 0,
    ...quiet,
  });
  assert.equal(v.ok, true, v.detail);
  assert.equal(v.declared, true);
});

test("a declared notice that never arrives reds, naming the text", () => {
  const v = judgeNotice(parseContainerLedger(chatLedger), {
    chat: ["New objective: Take the emeralds"],
    titles: [],
    lost: 0,
    ...quiet,
  });
  assert.equal(v.ok, false);
  assert.match(v.detail, /did NOT arrive/);
  assert.match(v.detail, /emptied long ago/);
});

test("a chat notice is not found among titles, nor a title among chat", () => {
  const chat = judgeNotice(parseContainerLedger(chatLedger), { chat: [], titles: [TEXT], lost: 0, ...quiet });
  assert.equal(chat.ok, false);
  const titled = parseContainerLedger({ ...chatLedger, notice: { text: TEXT, style: "title", sound: null } });
  assert.equal(judgeNotice(titled, { chat: [TEXT], titles: [], lost: 0, ...quiet }).ok, false);
  assert.equal(judgeNotice(titled, { chat: [], titles: [JSON.stringify({ fallback: TEXT })], lost: 0, ...quiet }).ok, true);
});

test("a window the ring dropped lines from says it was not fully observed", () => {
  const v = judgeNotice(parseContainerLedger(chatLedger), { chat: [], titles: [], lost: 3, ...quiet });
  assert.equal(v.ok, false);
  assert.match(v.detail, /3 chat line\(s\) fell out of the ring/);
});

test("with no notice declared the verdict states what the window held", () => {
  const v = judgeNotice(parseContainerLedger({ ...chatLedger, decoration: 0, notice: null }), {
    chat: ["a", "b"],
    titles: [],
    lost: 0,
    ...quiet,
  });
  assert.equal(v.ok, true);
  assert.equal(v.declared, false);
  assert.match(v.detail, /no notice declared \(0 of 6/);
  assert.match(v.detail, /2 line\(s\) told/);
});

test("a malformed ledger is a hard failure", () => {
  assert.throws(() => parseContainerLedger({ cells: 1, decoration: 0 }), ContainerLedgerParseError);
  assert.throws(() => parseContainerLedger({ cells: 1, decoration: 2, notice: null }), ContainerLedgerParseError);
  assert.throws(
    () => parseContainerLedger({ cells: 1, decoration: 1, notice: { text: "x", style: "shout", sound: null } }),
    ContainerLedgerParseError,
  );
  assert.throws(
    () => parseContainerLedger({ cells: 1, decoration: 1, notice: { text: "", style: "chat", sound: null } }),
    ContainerLedgerParseError,
  );
});

const SOUND = "minecraft:block.barrel.close";
const soundLedger = { ...chatLedger, notice: { text: TEXT, style: "chat", sound: SOUND } };

test("a notice's sound that arrives at the bot after the warp passes, and is recorded", () => {
  const v = judgeNotice(parseContainerLedger(soundLedger), {
    chat: [TEXT],
    titles: [],
    lost: 0,
    sounds: [{ name: "block.barrel.close", pos: [8195.5, 64, 8202.5] }],
    arrival: ARRIVAL,
  });
  assert.equal(v.ok, true, v.detail);
  assert.deepEqual(v.sound?.pos, [8195.5, 64, 8202.5]);
  assert.match(v.detail, /arrived at \[8195\.5,64,8202\.5\]/);
});

test("a notice's sound at the command source, not the bot, reds naming both points", () => {
  // The probe's shape: the world spawn is 20 blocks from where the warp put the bot.
  const v = judgeNotice(parseContainerLedger(soundLedger), {
    chat: [TEXT],
    titles: [],
    lost: 0,
    sounds: [{ name: "block.barrel.close", pos: [8215, 64, 8202] }],
    arrival: ARRIVAL,
  });
  assert.equal(v.ok, false);
  assert.match(v.detail, /NOT at the bot/);
  assert.match(v.detail, /8215/);
});

test("a notice's sound that never arrives reds, even when the text did", () => {
  const v = judgeNotice(parseContainerLedger(soundLedger), {
    chat: [TEXT],
    titles: [],
    lost: 0,
    sounds: [{ name: "entity.player.levelup", pos: [8195.5, 64, 8202.5] }],
    arrival: ARRIVAL,
  });
  assert.equal(v.ok, false);
  assert.match(v.detail, /did NOT arrive: 1 sound packet/);
  assert.equal(v.sound, null);
});

test("a ledger notice without a `sound` field is a hard failure", () => {
  assert.throws(
    () => parseContainerLedger({ cells: 1, decoration: 1, notice: { text: "x", style: "chat" } }),
    ContainerLedgerParseError,
  );
});

test("the ledger loads from beside the critical path, and its absence is undefined", async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), "dw-containers-"));
  try {
    const cp = path.join(dir, "critical-path.json");
    assert.equal(await loadContainerLedgerForCriticalPath(cp), undefined);
    await mkdir(path.join(dir, "validation"));
    await writeFile(path.join(dir, "validation", "containers.json"), JSON.stringify(chatLedger));
    const l = await loadContainerLedgerForCriticalPath(cp);
    assert.equal(l?.notice?.text, TEXT);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});
