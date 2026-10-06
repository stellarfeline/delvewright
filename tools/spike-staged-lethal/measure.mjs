#!/usr/bin/env node
// SPIKE TOOLING (a lethal volume that goes live on a party score) — NOT part of
// the shipped pipeline, not wired into CI. Driven by `run.sh` against a
// throwaway pinned 1.21.11 server.
//
// The question: a body is already standing where a lethal volume is declared,
// and the volume's gate flips open. What happens to it, and when?
//
// The rig is the emitted shape with one score standing in for `dw.f_<flag>`:
// a tick line `execute if score #party dwsl.f matches 1 run function
// dwsl:lethal`, and a volume function that `damage`s every player whose hitbox
// the box selector matches. `dwsl:arm` stamps the server tick and flips the
// score in ONE function, so the two land in the same tick; the volume function
// stamps the tick it first finds a body. The difference is the number this rig
// exists to measure. Five standing points, varying only where the body stands:
//
//   inside       feet in the box                                    (volume A)
//   ring-near    feet one cell outside the box, 0.25 from its face  (volume A)
//   ring-far     the same cell, 0.75 from the face                  (volume A)
//   outside      two cells out                                      (volume A)
//   under        feet one course UNDER a box, body reaching up      (volume B)
//
// plus a control in which the gate stays shut while the body stands inside.
//
// Every rcon reply is read: setup goes through `run` (throws on refusal);
// readings are `data get` / `scoreboard players get`, parsed or refused.

import { createRequire } from "node:module";
import { writeFileSync } from "node:fs";

import { rconChannel } from "../lib/rcon.mjs";

const require = createRequire(new URL("../../harness/package.json", import.meta.url));
const mineflayer = require("mineflayer");

const CONTAINER = process.env.SPIKE_CONTAINER ?? "dw-spike-staged-lethal";
const PORT = Number(process.env.SPIKE_PORT);
const OUT = process.env.SPIKE_OUT ?? new URL("./observations.json", import.meta.url).pathname;
const BOT = "dw_spike";
const REPS = Number(process.env.SPIKE_REPS ?? 3);
// Vanilla refuses damage to a player for 60 ticks after a respawn; wait it out
// and then some before every trial so the reading is about the volume.
const RESPAWN_IMMUNITY_MS = 4000;
const WATCH_MS = 3000;

// Volume A, inclusive cells; its continuous extent is [lo, hi + 1).
const A = { lo: [100, 1, 100], hi: [102, 2, 102] };
// Volume B: A shifted +10 in x and one course up.
const B = { lo: [110, 2, 100], hi: [112, 3, 102] };
const HALF_WIDTH = 0.3;
const HEIGHT = 1.8;

const POINTS = [
  { name: "inside", volume: "A", box: A, pos: [101.5, 1, 101.5] },
  { name: "ring-near", volume: "A", box: A, pos: [103.25, 1, 101.5] },
  { name: "ring-far", volume: "A", box: A, pos: [103.75, 1, 101.5] },
  { name: "outside", volume: "A", box: A, pos: [104.5, 1, 101.5] },
  { name: "under", volume: "B", box: B, pos: [111.5, 1, 101.5] },
];

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const r = rconChannel(CONTAINER);

/** Whether a player AABB at `pos` intersects the continuous box of `v`. */
function bodyMeets(pos, v) {
  const [x, y, z] = pos;
  const bx = [x - HALF_WIDTH, x + HALF_WIDTH];
  const by = [y, y + HEIGHT];
  const bz = [z - HALF_WIDTH, z + HALF_WIDTH];
  const lo = v.lo;
  const hi = v.hi.map((c) => c + 1);
  const meets = (a, l, h) => a[0] < h && a[1] > l;
  return meets(bx, lo[0], hi[0]) && meets(by, lo[1], hi[1]) && meets(bz, lo[2], hi[2]);
}

async function score(holder, obj) {
  const reply = await r.run(`scoreboard players get ${holder} ${obj}`);
  const m = reply.match(/ has (-?\d+) \[/);
  if (!m) throw new Error(`no score for ${holder} ${obj}: ${reply}`);
  return Number(m[1]);
}
async function health() {
  const reply = await r.run(`data get entity ${BOT} Health`);
  const m = reply.match(/entity data: (-?\d+(?:\.\d+)?)f/);
  if (!m) throw new Error(`no Health: ${reply}`);
  return Number(m[1]);
}
async function pos() {
  const reply = await r.run(`data get entity ${BOT} Pos`);
  const m = reply.match(/-?\d+(?:\.\d+)?(?:[eE]-?\d+)?/g);
  if (!m || m.length < 3) throw new Error(`no Pos: ${reply}`);
  return m.slice(0, 3).map(Number);
}
async function gametime() {
  const reply = await r.run("time query gametime");
  const m = reply.match(/(-?\d+)/);
  if (!m) throw new Error(`no gametime: ${reply}`);
  return Number(m[1]);
}
async function until(fn, ms, what) {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    if (await fn()) return true;
    await sleep(100);
  }
  throw new Error(`timed out waiting for ${what}`);
}

const obs = {
  instrument: {
    server: "itzg/minecraft-server@sha256:3e7db256… (versions.toml [images.base]), vanilla 1.21.11, flat (stone top y 0), hard, view-distance 8",
    bot: "mineflayer 4.37.1 (harness pin), one body, standing still (no controls held)",
    rig: "tools/spike-staged-lethal/spikepack: tick `execute if score #party dwsl.f matches 1 run function dwsl:lethal`; volume A cells x100..102 y1..2 z100..102, volume B x110..112 y2..3 z100..102; `dwsl:arm` stamps `time query gametime` and sets the score in one function; `dwsl:lethal` stamps the tick it first matches a body and `damage @s 1000 minecraft:generic`s it",
    body: { half_width: HALF_WIDTH, height: HEIGHT },
    reps_per_point: REPS,
    watch_ms: WATCH_MS,
  },
  control: null,
  trials: [],
};

const bot = mineflayer.createBot({ host: "127.0.0.1", port: PORT, username: BOT, version: "1.21.11", auth: "offline" });
let deaths = [];
let spawns = 0;
bot.on("death", () => deaths.push(Date.now()));
bot.on("spawn", () => { spawns += 1; });
bot.on("kicked", (why) => { console.error("[spike] bot kicked:", why); process.exit(1); });
bot.on("error", (e) => { console.error("[spike] bot error:", e); process.exit(1); });
const spawnOnce = () => new Promise((res) => bot.once("spawn", res));

await spawnOnce();
if (spawns < 1) throw new Error("spawn counted nothing");
console.log("[spike] bot spawned");
await r.run(`gamemode survival ${BOT}`);
await sleep(500);

/** Put the body down at `p`, standing, and read back where the server has it. */
async function place(p) {
  await r.run(`tp ${BOT} ${p[0]} ${p[1]} ${p[2]}`);
  await until(async () => {
    const q = await pos();
    return Math.abs(q[0] - p[0]) < 0.01 && Math.abs(q[2] - p[2]) < 0.01 && Math.abs(q[1] - p[1]) < 0.01;
  }, 5000, `the body to stand at ${p}`);
  await sleep(600);
  return pos();
}

async function trial(point, rep) {
  const t = { point: point.name, volume: point.volume, rep, requested: point.pos };
  t.pos = await place(point.pos);
  t.feet_cell = t.pos.map(Math.floor);
  t.body_meets_box = bodyMeets(t.pos, point.box);
  t.health_before = await health();
  if (t.health_before !== 20) throw new Error(`trial opened at health ${t.health_before}`);
  if ((await score("#party", "dwsl.f")) !== 0) throw new Error("the gate was open before the trial");
  deaths = [];
  const spawnsBefore = spawns;
  const armedAt = Date.now();
  await r.run("function dwsl:arm");
  t.set_tick = await score("#settick", "dwsl.s");
  // Watch for the kill stamp or the window to run out.
  let killTick = -1;
  const end = Date.now() + WATCH_MS;
  while (Date.now() < end) {
    killTick = await score("#killtick", "dwsl.s");
    if (killTick >= 0) break;
    await sleep(50);
  }
  t.kill_tick = killTick >= 0 ? killTick : null;
  t.ticks_from_arm_to_kill = killTick >= 0 ? killTick - t.set_tick : null;
  t.gametime_at_end_of_watch = await gametime();
  t.kills_counted = await score("#kills", "dwsl.s");
  t.health_after = await health();
  t.client_death_events = deaths.length;
  t.client_death_ms_after_arm = deaths.length ? deaths[0] - armedAt : null;
  t.died = t.health_after === 0 || deaths.length > 0;
  await r.run("function dwsl:disarm");
  if (t.died) {
    // mineflayer respawns on its own, often before this line runs; wait for the
    // spawn COUNT to move rather than for the next event, then for the immunity.
    await until(async () => spawns > spawnsBefore, 10000, "the body to respawn");
    await sleep(RESPAWN_IMMUNITY_MS);
  }
  console.log(`[spike] ${point.name} #${rep}: meets=${t.body_meets_box} died=${t.died} ticks=${t.ticks_from_arm_to_kill} hp=${t.health_after}`);
  return t;
}

// The control: the gate stays shut; a body inside the box is not touched.
{
  const p = POINTS[0];
  const c = { point: p.name, gate: "shut" };
  c.pos = await place(p.pos);
  c.body_meets_box = bodyMeets(c.pos, p.box);
  c.health_before = await health();
  deaths = [];
  const t0 = await gametime();
  await sleep(WATCH_MS);
  c.ticks_watched = (await gametime()) - t0;
  c.kill_tick = await score("#killtick", "dwsl.s");
  c.kills_counted = await score("#kills", "dwsl.s");
  c.health_after = await health();
  c.client_death_events = deaths.length;
  obs.control = c;
  console.log(`[spike] control: watched ${c.ticks_watched} ticks, kill_tick=${c.kill_tick} hp=${c.health_after}`);
}

for (const point of POINTS) {
  for (let rep = 1; rep <= REPS; rep++) {
    obs.trials.push(await trial(point, rep));
  }
}

// The summary a reader checks the trials against.
obs.summary = {};
for (const point of POINTS) {
  const ts = obs.trials.filter((t) => t.point === point.name);
  obs.summary[point.name] = {
    trials: ts.length,
    body_meets_box: [...new Set(ts.map((t) => t.body_meets_box))],
    died: ts.filter((t) => t.died).length,
    ticks_from_arm_to_kill: [...new Set(ts.map((t) => t.ticks_from_arm_to_kill))],
    kills_counted: [...new Set(ts.map((t) => t.kills_counted))],
  };
}
writeFileSync(OUT, JSON.stringify(obs, null, 2) + "\n");
console.log("[spike] summary", JSON.stringify(obs.summary));
bot.quit();
process.exit(0);
