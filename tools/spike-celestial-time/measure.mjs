#!/usr/bin/env node
// SPIKE TOOLING (celestial time) — NOT part of the shipped pipeline, not wired
// into CI. Run by `tools/spike-celestial-time/run.sh`.
//
// Measures what `/time set` does on the pinned 1.21.11 server and what a
// connected client is told, for spec-0081. Every command goes through the shared
// rejection rule (`tools/lib/rcon.mjs`): a command whose response nobody reads
// cannot fail. The bot is mineflayer at the harness pin; the only thing read from
// it is the raw `update_time` packet, which is the server's own statement of the
// clock, never mineflayer's derived fields (those are listed beside it, labelled
// as derived, so the two can be compared).

import { createRequire } from "node:module";
import { writeFileSync } from "node:fs";

import { rconChannel } from "../lib/rcon.mjs";

const require = createRequire(new URL("../../harness/package.json", import.meta.url));
const mineflayer = require("mineflayer");

const CONTAINER = process.env.SPIKE_CONTAINER ?? "dw-spike-celestial-time";
const PORT = Number(process.env.SPIKE_PORT ?? 25599);
const OUT = process.env.SPIKE_OUT ?? new URL("./observations.json", import.meta.url).pathname;
const BOT = "dw_celestial";

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const rcon = rconChannel(CONTAINER);

/** `time query …` answers "The time is N"; anything else is a malformed probe. */
function theTimeIs(reply) {
  const m = String(reply).match(/^The time is (-?\d+)$/);
  if (!m) throw new Error(`not a time query answer: ${JSON.stringify(reply)}`);
  return Number(m[1]);
}
async function clock() {
  return {
    day: theTimeIs(await rcon.run("time query day")),
    daytime: theTimeIs(await rcon.run("time query daytime")),
    gametime: theTimeIs(await rcon.run("time query gametime")),
  };
}
// A conditional `execute` with no `run` answers an EMPTY reply on 1.21.11, which
// cannot be told from a refusal; so every boolean probe runs a `time query` and
// reads "The time is" as true, "" as false, anything else as a broken probe.
async function predicate(id) {
  const r = await rcon.run(`execute if predicate dwspike:${id} run time query gametime`);
  if (r.startsWith("The time is")) return true;
  if (r === "") return false;
  throw new Error(`predicate probe did not evaluate: ${JSON.stringify(r)}`);
}

// ------------------------------------------------------------------- the bot
const T0 = Date.now();
const packets = [];
const bot = mineflayer.createBot({
  host: "127.0.0.1",
  port: PORT,
  username: BOT,
  version: process.env.DELVEWRIGHT_MC_VERSION ?? "1.21.11",
  auth: "offline",
});
const longToBigInt = (v) =>
  Array.isArray(v) ? BigInt.asIntN(64, BigInt(v[0]) << 32n) | BigInt(v[1] >>> 0) : BigInt(v);
bot._client.on("update_time", (p) => {
  packets.push({
    ms: Date.now() - T0,
    gametime: Number(longToBigInt(p.age)),
    daytime: Number(longToBigInt(p.time)),
    tickDayTime: p.tickDayTime,
  });
});
bot.on("kicked", (why) => {
  console.error("[spike] bot kicked:", why);
  process.exit(1);
});
bot.on("error", (e) => {
  console.error("[spike] bot error:", e);
  process.exit(1);
});
await new Promise((resolve) => bot.once("spawn", resolve));
await sleep(1500);

/** Wait for the first `update_time` after `sinceMs` whose daytime equals `want`. */
async function awaitPacket(want, sinceMs, timeoutMs = 4000) {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    const hit = packets.find((p) => p.ms > sinceMs && p.daytime === want);
    if (hit) return hit;
    if (Date.now() > deadline) return null;
    await sleep(20);
  }
}

const obs = {
  instrument: {
    server:
      "itzg/minecraft-server@sha256:3e7db256… (versions.toml [images.base]), vanilla 1.21.11",
    bot: `mineflayer ${require("mineflayer/package.json").version} (harness pin)`,
    note: "every reply read through tools/lib/rcon.mjs; packets are the raw update_time (ClientboundSetTimePacket: gameTime, dayTime, tickDayTime)",
  },
  replies: {},
  steps: [],
};

// 1. The engine's own seal: the day cycle frozen.
obs.replies.freeze = await rcon.run("gamerule advance_time false");
obs.replies.freeze_readback = await rcon.run("gamerule advance_time");
await sleep(1200);
obs.steps.push({ step: "baseline (frozen cycle)", clock: await clock() });

// 2. An absolute tick count above one day: day 4 + the `night` keyword's 13000.
//    moon.json puts `new_moon` at 96000..119999 of its 192000 period, so this is
//    a new moon with the sun just set.
{
  const before = await clock();
  const sent = Date.now() - T0;
  const reply = await rcon.run("time set 109000");
  const after = await clock();
  const pkt = await awaitPacket(109000, sent - 1);
  await sleep(1500);
  obs.steps.push({
    step: "time set 109000",
    reply,
    before,
    after,
    gametime_moved_by: after.gametime - before.gametime,
    client_packet: pkt,
    client_latency_ms: pkt ? pkt.ms - sent : null,
    client_packets_in_window: packets.filter((p) => p.ms > sent).length,
    server_predicate_new_moon: await predicate("new_moon"),
    server_predicate_full_moon: await predicate("full_moon"),
    mineflayer_derived: {
      day: bot.time.day,
      timeOfDay: bot.time.timeOfDay,
      moonPhase: bot.time.moonPhase,
      doDaylightCycle: bot.time.doDaylightCycle,
    },
  });
}

// 3. Does the set state HOLD under the frozen cycle?
{
  const a = await clock();
  await sleep(2500);
  const b = await clock();
  obs.steps.push({ step: "hold 2.5 s under advance_time false", first: a, second: b, held: a.day === b.day && a.daytime === b.daytime });
}

// 4. A keyword after a day offset: is `time set night` absolute (day resets to 0)?
{
  const before = await clock();
  const sent = Date.now() - T0;
  const reply = await rcon.run("time set night");
  const after = await clock();
  const pkt = await awaitPacket(13000, sent - 1);
  await sleep(1200);
  obs.steps.push({
    step: "time set night (keyword) after day 4",
    reply,
    before,
    after,
    day_reset_to_zero: after.day === 0,
    client_packet: pkt,
    server_predicate_new_moon: await predicate("new_moon"),
    server_predicate_full_moon: await predicate("full_moon"),
    mineflayer_derived: { day: bot.time.day, moonPhase: bot.time.moonPhase },
  });
}

// 5. One whole moon cycle: day 8 is phase 0 again (192000 = 8 × 24000).
{
  const sent = Date.now() - T0;
  const reply = await rcon.run("time set 192000");
  const after = await clock();
  const pkt = await awaitPacket(192000, sent - 1);
  await sleep(1200);
  obs.steps.push({
    step: "time set 192000",
    reply,
    after,
    client_packet: pkt,
    server_predicate_full_moon: await predicate("full_moon"),
    server_predicate_new_moon: await predicate("new_moon"),
    mineflayer_derived: { day: bot.time.day, moonPhase: bot.time.moonPhase },
  });
}

// 6. `time add` is relative to the current dayTime (so the day count survives).
{
  await rcon.run("time set 96000");
  const before = await clock();
  const reply = await rcon.run("time add 13000");
  const after = await clock();
  obs.steps.push({ step: "time set 96000; time add 13000", reply, before, after });
}

// 7. The argument's floor: the command tree says `minecraft:time` min 0.
{
  const reply = await rcon.probe("time set -1");
  obs.steps.push({ step: "time set -1 (expect a parse refusal)", reply });
}

// 8. Suffixed forms the argument accepts: `d` days, `s` seconds, `t` ticks.
{
  const r1 = await rcon.run("time set 4d");
  const c1 = await clock();
  const r2 = await rcon.run("time set 1s");
  const c2 = await clock();
  obs.steps.push({ step: "time set 4d", reply: r1, after: c1 });
  obs.steps.push({ step: "time set 1s", reply: r2, after: c2 });
}

obs.all_packets = packets;
// Canonical at the writer (`delvec fmt`'s form: keys sorted, two-space indent),
// so the committed observations pass the canonical-form sweep as written.
const canon = (v) =>
  Array.isArray(v)
    ? v.map(canon)
    : v && typeof v === "object"
      ? Object.fromEntries(Object.keys(v).sort().map((k) => [k, canon(v[k])]))
      : v;
writeFileSync(OUT, JSON.stringify(canon(obs), null, 2) + "\n");
console.log(`[spike] ${obs.steps.length} steps, ${packets.length} update_time packets -> ${OUT}`);
bot.quit();
process.exit(0);
