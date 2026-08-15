#!/usr/bin/env node
// SPIKE TOOLING (fluid plane / dynamic water level) — NOT part of the shipped
// pipeline, not wired into CI. Same shape as `tools/spike-death-teleport/`.
//
// Every mutation's response is checked (`ok()`); censuses that legitimately
// count zero go through `raw()` and parse the reply instead: a command whose
// response nobody reads cannot fail.
//
// World facts (must match crates/compiler/src/horizon.rs): water top y=62,
// water layers y=55..62, sea floor (stone) top y=54.

import { spawn } from "node:child_process";
import { writeFileSync } from "node:fs";
import readline from "node:readline";
import { createRequire } from "node:module";

import { REJECTION, assertAccepted } from "../lib/rcon.mjs";

const require = createRequire(new URL("../../harness/package.json", import.meta.url));
const mineflayer = require("mineflayer");

const CONTAINER = process.env.SPIKE_CONTAINER ?? "dw-spike-fluid-plane";
const PORT = Number(process.env.SPIKE_PORT ?? 25599);
const OUT = process.env.SPIKE_OUT ?? new URL("./observations.json", import.meta.url).pathname;
const BOT = "dw_fluid";

const SEA = 62; // water top layer y (horizon::SEA_LEVEL)

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// ---------------------------------------------------------------- rcon channel
// One long-lived `rcon-cli` on stdin; commands PIPELINED, every batch fenced by
// a #sync scoreboard write/read (the death-spike pattern), plus two hardenings
// that pattern turned out to need:
//   - a dead channel REJECTS the pending read instead of leaving an empty event
//     loop (which node exits 0 on — the first run of this spike "succeeded"
//     with no observations at all exactly that way);
//   - channel start is a retried handshake, because the server's readiness
//     probe passing and the next rcon connection being accepted are two events
//     with a measurable race between them.
let proc = null;
let inbox = [];
let notify = null;
let channelDown = null;
function startChannel() {
  inbox = [];
  channelDown = null;
  proc = spawn("docker", ["exec", "-i", CONTAINER, "rcon-cli"], {
    stdio: ["pipe", "pipe", "inherit"],
  });
  const rl = readline.createInterface({ input: proc.stdout });
  rl.on("line", (line) => {
    inbox.push(line.replace(/^>\s?/, ""));
    if (notify) {
      const n = notify;
      notify = null;
      n.resolve();
    }
  });
  proc.on("close", (code) => {
    channelDown = new Error(`rcon channel closed (exit ${code})`);
    if (notify) {
      const n = notify;
      notify = null;
      n.reject(channelDown);
    }
  });
  proc.stdin.on("error", () => {}); // EPIPE surfaces via 'close' above
}
async function readLines(n) {
  while (inbox.length < n) {
    if (channelDown) throw channelDown;
    await new Promise((resolve, reject) => (notify = { resolve, reject }));
  }
  return inbox.splice(0, n);
}
/** Start the channel and prove it answers, retrying across the boot race. */
async function connectChannel() {
  for (let attempt = 1; attempt <= 24; attempt++) {
    startChannel();
    try {
      proc.stdin.write("list\n");
      await Promise.race([
        readLines(1),
        new Promise((_, rej) => setTimeout(() => rej(new Error("handshake timeout")), 10000)),
      ]);
      inbox = []; // discard the handshake reply
      return;
    } catch (e) {
      console.error(`[measure] rcon handshake attempt ${attempt}: ${e.message ?? e}`);
      try {
        proc.kill();
      } catch {}
      await sleep(5000);
    }
  }
  throw new Error("rcon channel never came up");
}
let syncN = 0;
async function batch(cmds) {
  const n = ++syncN;
  const all = [
    `scoreboard players set #sync dw.sync ${n}`,
    ...cmds,
    "scoreboard players get #sync dw.sync",
  ];
  proc.stdin.write(all.join("\n") + "\n");
  const out = await readLines(all.length);
  if (!out[out.length - 1].includes(`#sync has ${n} `)) {
    throw new Error(`rcon desync at ${n}; responses were ${JSON.stringify(out)}`);
  }
  return out.slice(1, -1);
}
const raw = async (cmd) => (await batch([cmd]))[0];
async function ok(cmd) {
  return assertAccepted(cmd, await raw(cmd));
}
/**
 * For commands whose PAYLOAD spans multiple lines (`tick query`): positional
 * alignment is impossible, so read until the sync fence and join what came
 * between. Only ever call this for one command at a time.
 */
async function rawMulti(cmd) {
  const n = ++syncN;
  proc.stdin.write(
    `scoreboard players set #sync dw.sync ${n}\n${cmd}\nscoreboard players get #sync dw.sync\n`,
  );
  const lines = [];
  for (;;) {
    const [l] = await readLines(1);
    lines.push(l);
    if (l.includes(`#sync has ${n} `)) break;
  }
  return lines.slice(1, -1).join(" ");
}

// ------------------------------------------------------------------- parsing
const PROBE = "run time query gametime"; // conditional-execute boolean probe
function flag(line) {
  if (line.startsWith("The time is")) return true;
  if (line === "") return false;
  throw new Error(`boolean probe did not evaluate: ${JSON.stringify(line)}`);
}
/** "Successfully filled N block(s)" -> N; "No blocks were filled" -> 0. */
function filled(line) {
  const m = line.match(/Successfully filled (\d+) block/);
  if (m) return Number(m[1]);
  if (line.startsWith("No blocks were filled")) return 0;
  throw new Error(`not a fill reply: ${JSON.stringify(line)}`);
}
/** /tick query -> average mspt. Multi-line payload => rawMulti. */
async function mspt() {
  const r = await rawMulti("tick query");
  const m = r.match(/Average time per tick: ([\d.]+)ms/);
  if (!m) throw new Error(`tick query unparsed: ${JSON.stringify(r)}`);
  return Number(m[1]);
}
const DATA = / has the following entity data: /;
function nbt(line) {
  const i = line.search(DATA);
  return i < 0 ? null : line.slice(i).replace(DATA, "");
}
const num = (line) => {
  const v = nbt(line);
  return v === null ? null : Number.parseFloat(v);
};

// --------------------------------------------------------- census primitives
/**
 * Destructive flowing-water census over a box: replaces water[level=1..15]
 * with glass, level by level, counting. `xw`/`zw` are inclusive corners;
 * strips of <=32768 cells per command. Returns {flowing, byLevel}.
 */
async function flowingCensus(x0, y, x1, z0, z1, y1 = null) {
  const yy1 = y1 ?? y;
  const byLevel = {};
  let flowing = 0;
  for (let lvl = 1; lvl <= 15; lvl++) {
    let count = 0;
    for (const [a0, a1, b0, b1] of strips(x0, x1, z0, z1, yy1 - y + 1)) {
      const r = await raw(
        `fill ${a0} ${y} ${b0} ${a1} ${yy1} ${b1} minecraft:glass replace minecraft:water[level=${lvl}]`,
      );
      count += filled(r);
    }
    if (count > 0) byLevel[lvl] = count;
    flowing += count;
  }
  return { flowing, byLevel };
}
/** Destructive source census (water[level=0] -> blue glass). */
async function sourceCensus(x0, y, x1, z0, z1, y1 = null) {
  const yy1 = y1 ?? y;
  let n = 0;
  for (const [a0, a1, b0, b1] of strips(x0, x1, z0, z1, yy1 - y + 1)) {
    const r = await raw(
      `fill ${a0} ${y} ${b0} ${a1} ${yy1} ${b1} minecraft:blue_stained_glass replace minecraft:water[level=0]`,
    );
    n += filled(r);
  }
  return n;
}
/** Split [x0..x1]x[z0..z1] (times `layers`) into strips of <=32768 cells. */
function strips(x0, x1, z0, z1, layers = 1) {
  const xw = x1 - x0 + 1;
  const maxZ = Math.max(1, Math.floor(32768 / (xw * layers)));
  const out = [];
  for (let z = z0; z <= z1; z += maxZ) {
    out.push([x0, x1, z, Math.min(z1, z + maxZ - 1)]);
  }
  return out;
}
/**
 * NON-destructive transect: sample a row of cells at fixed y,z across
 * [x0..x1], each cell classified source / flowing / other(air,solid).
 */
async function transect(x0, x1, y, z) {
  const cmds = [];
  for (let x = x0; x <= x1; x++) {
    cmds.push(`execute if block ${x} ${y} ${z} minecraft:water[level=0] ${PROBE}`);
    cmds.push(`execute if block ${x} ${y} ${z} minecraft:water ${PROBE}`);
  }
  const r = await batch(cmds);
  let source = 0,
    flowing = 0,
    other = 0;
  for (let i = 0; i < r.length; i += 2) {
    const isSource = flag(r[i]);
    const isWater = flag(r[i + 1]);
    if (isSource) source++;
    else if (isWater) flowing++;
    else other++;
  }
  return { source, flowing, other };
}
/** Timed chunked fill of one horizontal layer band. Returns ms + blocks. */
async function timedFill(x0, x1, z0, z1, y0, y1, block, replace) {
  const layers = y1 - y0 + 1;
  const t0 = Date.now();
  let blocks = 0;
  let cmdCount = 0;
  for (const [a0, a1, b0, b1] of strips(x0, x1, z0, z1, layers)) {
    const suffix = replace ? ` replace ${replace}` : "";
    const r = await raw(`fill ${a0} ${y0} ${b0} ${a1} ${y1} ${b1} ${block}${suffix}`);
    blocks += filled(r);
    cmdCount++;
  }
  return { ms: Date.now() - t0, blocks, commands: cmdCount };
}
/** Forceload a block-coordinate rectangle in <=16x16-chunk tiles. */
async function forceload(verb, x0, x1, z0, z1) {
  for (let x = x0; x <= x1; x += 256) {
    for (let z = z0; z <= z1; z += 256) {
      await ok(`forceload ${verb} ${x} ${z} ${Math.min(x1, x + 255)} ${Math.min(z1, z + 255)}`);
    }
  }
}

const obs = {
  meta: {
    date: new Date().toISOString(),
    mc: "1.21.11",
    world: "ocean superflat (delve emission literal), water top y=62",
    view_distance: 8,
    simulation_distance: 8,
  },
};

// ================================================================= scenarios
async function main() {
  await connectChannel();
  // `dw.sync` is the batch fence, so it has to exist BEFORE the first fenced
  // batch; write it unfenced and swallow the one reply.
  proc.stdin.write("scoreboard objectives add dw.sync dummy\n");
  await readLines(1);
  await ok("gamerule spawn_mobs false");
  await ok("gamerule advance_time false");
  await ok("gamerule advance_weather false");
  await ok("time set noon");
  await ok("gamerule respawn_radius 0");

  // Site A is forceloaded first: every chunk ticks — the WORST case for
  // stillness, and the honest one (a delve's site chunks tick, the party is
  // standing in them).
  await forceload("add", -256, 255, -256, 255);
  await sleep(4000); // let chunk generation settle

  // ---- A. the /fill ceiling and the gamerule that moves it ------------------
  // STONE, deliberately: the measurand is block-modification throughput for one
  // giant command vs the chunked path, and stone has no fluid ticks, so the
  // probe cannot contaminate the water measurements below. (A giant WATER fill
  // at altitude would rain 262k falling columns onto the measurement site.)
  {
    const over = await raw("fill -256 90 -256 255 90 255 minecraft:stone"); // 262144 > 32768
    const rule = await raw("gamerule max_block_modifications");
    await ok("gamerule max_block_modifications 1048576");
    const t0 = Date.now();
    const big = await raw("fill -256 90 -256 255 90 255 minecraft:stone");
    const bigMs = Date.now() - t0;
    const t1 = Date.now();
    const undo = await raw("fill -256 90 -256 255 90 255 minecraft:air replace minecraft:stone");
    const undoMs = Date.now() - t1;
    await ok("gamerule max_block_modifications 32768");
    obs.fill_ceiling = {
      oversize_refusal: over,
      gamerule_default: rule,
      single_command_512x512_stone: { reply: big.slice(0, 80), ms: bigMs, placed: filled(big) },
      single_command_undo: { ms: undoMs, cleared: filled(undo) },
    };
  }

  // ---- site A: raise the plane by two layers over the 512x512 extent --------
  {
    const msptBefore = await mspt();

    // The bot's pad: stone at y=62 replacing the water top, bot feet at y=63.
    await ok("fill -2 62 -2 2 62 2 minecraft:stone replace minecraft:water");
    await ok("setworldspawn 0 63 0");

    // Bot joins before the raise, so the raise happens AROUND a player.
    const bot = mineflayer.createBot({
      host: "127.0.0.1",
      port: PORT,
      username: BOT,
      auth: "offline",
    });
    await new Promise((res, rej) => {
      bot.once("spawn", res);
      bot.once("kicked", (r) => rej(new Error(`kicked: ${JSON.stringify(r)}`)));
      bot.once("error", rej);
      setTimeout(() => rej(new Error("bot did not spawn in 60s")), 60000);
    });
    await sleep(2000);
    const posBefore = nbt(await raw(`data get entity ${BOT} Pos`));
    const airBefore = num(await raw(`data get entity ${BOT} Air`));

    // Raise: y=63 then y=64, replace air, whole forceloaded extent.
    const l1 = await timedFill(-256, 255, -256, 255, 63, 63, "minecraft:water", "minecraft:air");
    const msptMid = await mspt();
    const l2 = await timedFill(-256, 255, -256, 255, 64, 64, "minecraft:water", "minecraft:air");
    const msptAfterFill = await mspt();

    // The player who was standing there: displacement + drowning clock.
    // 10 samples x 2s: air (300 ticks = 15s) runs out at ~15s, then ~2 hp/s of
    // drowning — 20s stays comfortably clear of a death.
    const samples = [];
    for (let i = 0; i < 10; i++) {
      const [p, a, h] = await batch([
        `data get entity ${BOT} Pos`,
        `data get entity ${BOT} Air`,
        `data get entity ${BOT} Health`,
      ]);
      samples.push({ t_s: i * 2, pos: nbt(p), air: num(a), hp: num(h) });
      await sleep(2000);
    }
    obs.player_in_rising_column = {
      before: { pos: posBefore, air: airBefore },
      after: samples,
      note: "pad top y=62, feet y=63: a 2-layer raise (63,64) fully submerges the head",
    };
    // Rescue: teleport the bot above the new surface onto a fresh pad.
    await ok("fill 6 64 6 10 64 10 minecraft:stone");
    await ok(`tp ${BOT} 8 65 8`);
    await ok(`effect give ${BOT} minecraft:instant_health 1 10 true`);

    await sleep(15000); // several fluid-tick generations
    const msptSettled = await mspt();

    // Interior stillness: everything 16 blocks in from the fill rim.
    const interior = {};
    for (const y of [63, 64]) {
      interior[`y${y}`] = await flowingCensus(-240, y, 239, -240, 239);
    }
    // Rim band (outer 16 of the fill) — the edge SAT in ticking chunks, so this
    // is where flow appears if it appears at all.
    const rim = {};
    for (const y of [63, 64]) {
      const bands = [
        [-256, -241, -256, 255],
        [240, 255, -256, 255],
        [-240, 239, -256, -241],
        [-240, 239, 240, 255],
      ];
      let flowing = 0;
      for (const [bx0, bx1, bz0, bz1] of bands) {
        const c = await flowingCensus(bx0, y, bx1, bz0, bz1);
        flowing += c.flowing;
      }
      rim[`y${y}`] = flowing;
    }
    // Spill BEYOND the fill rim (border chunks): did edge sources flow outward?
    const beyond = {};
    for (const y of [63, 64]) {
      let total = 0;
      const bands = [
        [-288, -257, -288, 287],
        [256, 287, -288, 287],
        [-256, 255, -288, -257],
        [-256, 255, 256, 287],
      ];
      for (const [bx0, bx1, bz0, bz1] of bands) {
        const cmds = [];
        // Transect sampling (border chunks may or may not accept /fill).
        for (let i = 0; i < 48; i++) {
          const x = bx0 + Math.floor(((bx1 - bx0) * i) / 47);
          const z = bz0 + Math.floor(((bz1 - bz0) * i) / 47);
          cmds.push(`execute if block ${x} ${y} ${z} minecraft:water ${PROBE}`);
        }
        const r = await batch(cmds);
        total += r.filter((l) => flag(l)).length;
      }
      beyond[`y${y}_sampled192`] = total;
    }
    obs.plane_raise_512 = {
      mspt: { before: msptBefore, mid: msptMid, after_fill: msptAfterFill, settled_15s: msptSettled },
      layer_63: l1,
      layer_64: l2,
      interior_flowing_after_15s: interior,
      rim16_flowing_after_15s: rim,
      spill_beyond_rim_samples: beyond,
    };
  }

  // ---- site B: lower the plane with a TICKING edge — the healing number -----
  {
    await forceload("add", 3968, 4223, -128, 127);
    await sleep(4000);
    const clear = await timedFill(4000, 4191, -96, 95, SEA, SEA, "minecraft:air", "minecraft:water");
    // Non-destructive centerline transects over time: how fast does the ambient
    // sea creep back across the cleared layer?
    const heal = [];
    const t0 = Date.now();
    for (const at of [0, 5, 10, 20, 40, 60, 90]) {
      const wait = t0 + at * 1000 - Date.now();
      if (wait > 0) await sleep(wait);
      heal.push({ t_s: at, ...(await transect(4000, 4191, SEA, 0)) });
    }
    // End state: full census of the cleared box.
    const endFlow = await flowingCensus(4000, SEA, 4191, -96, 95);
    const endSrc = await sourceCensus(4000, SEA, 4191, -96, 95);
    obs.plane_lower_ticking_edge = {
      cleared: clear,
      note: "192x192 y=62 layer cleared; edges 32 blocks inside a ticking (forceloaded) extent; ambient y=62 sea all around; y=61 sources below",
      centerline_transect_over_time: heal,
      after_90s_full_census: { flowing: endFlow, sources_regenerated: endSrc },
    };
  }

  // ---- site C: the transition window — forceload add, fill, remove ----------
  // The emission strategy for a level change over a mostly-unloaded extent:
  // forceload the ring, fill, forceload remove, all inside one batch. Fluid
  // scheduled ticks have a 5-tick delay; the window is shorter, so the rim's
  // pending ticks should FREEZE with the chunks and fire only if the rim ever
  // ticks again.
  {
    // C1: /fill into genuinely unloaded chunks — the exact refusal.
    const unloaded = await raw("fill 8320 63 0 8330 63 10 minecraft:water");
    // C2: the window. Chunk generation is async, so the load is its own step
    // with a settle sleep; the measured window is fill -> forceload remove.
    await ok("forceload add 8064 -128 8319 127");
    await sleep(5000);
    const cmds = [];
    for (const [a0, a1, b0, b1] of strips(8064, 8319, -128, 127)) {
      cmds.push(`fill ${a0} 63 ${b0} ${a1} 63 ${b1} minecraft:water replace minecraft:air`);
    }
    cmds.push("forceload remove 8064 -128 8319 127");
    const t0 = Date.now();
    const rWindow = await batch(cmds);
    const windowMs = Date.now() - t0;
    await sleep(20000);
    // C3: reload only the WEST RIM chunk column and watch whether flow begins
    // (pending ticks firing) — sampled without loading anything else.
    await ok("forceload add 8048 -128 8079 127"); // rim column + its west neighbor
    await sleep(1000);
    const rimT0 = await transect(8064, 8079, 63, 0);
    await sleep(15000);
    const rimT15 = await transect(8064, 8079, 63, 0);
    const rimBeyondCmds = [];
    for (let z = -40; z <= 40; z += 4) {
      rimBeyondCmds.push(`execute if block 8063 63 ${z} minecraft:water ${PROBE}`);
      rimBeyondCmds.push(`execute if block 8060 63 ${z} minecraft:water ${PROBE}`);
    }
    const rimBeyond = (await batch(rimBeyondCmds)).filter((l) => flag(l)).length;
    obs.transition_window = {
      unloaded_fill_refusal: unloaded,
      window: {
        ms: windowMs,
        replies: rWindow.map((x) => x.slice(0, 60)),
      },
      rim_after_reload_t0: rimT0,
      rim_after_reload_t15s: rimT15,
      water_beyond_rim_sampled42: rimBeyond,
      note: "rim y=63 water borders untouched y=62 ocean to the west; if pending ticks fire on reload, flowing appears at/beyond x=8063",
    };
    await ok("forceload remove all");
    // Site A stays relevant below; re-add it.
    await forceload("add", -256, 255, -256, 255);
  }

  // ---- site D: basins — saturation vs settle vs interior gap ----------------
  {
    await forceload("add", 16336, 16463, -64, 63);
    await sleep(3000);
    // Platform + three identical basins, interiors 12x12x3 (y 70..72), rims
    // y=73, floors y=69, walls stone.
    const basins = [
      { id: "saturated", x: 16350 },
      { id: "settle_one_source", x: 16380 },
      { id: "interior_gap", x: 16410 },
      { id: "waterlogged", x: 16440 },
    ];
    for (const b of basins) {
      await ok(`fill ${b.x} 69 -10 ${b.x + 13} 73 3 minecraft:stone`);
      await ok(`fill ${b.x + 1} 70 -9 ${b.x + 12} 73 2 minecraft:air`);
    }
    // D1 saturated: every interior cell y70..72 becomes a source in one fill.
    {
      const b = basins[0];
      await ok(`fill ${b.x + 1} 70 -9 ${b.x + 12} 72 2 minecraft:water`);
      await sleep(10000);
      const c = await flowingCensus(b.x + 1, 70, b.x + 12, -9, 2, 72);
      obs.basin_saturated = { flowing_after_10s: c.flowing, byLevel: c.byLevel };
    }
    // D2 flow-and-settle: one source in a corner, 90s to settle.
    {
      const b = basins[1];
      await ok(`setblock ${b.x + 1} 72 -9 minecraft:water`);
      await sleep(90000);
      const c = await flowingCensus(b.x + 1, 70, b.x + 12, -9, 2, 72);
      const s = await sourceCensus(b.x + 1, 70, b.x + 12, -9, 2, 72);
      obs.basin_settle = {
        flowing_after_90s: c.flowing,
        byLevel: c.byLevel,
        sources_after_90s: s,
        interior_cells: 12 * 12 * 3,
      };
    }
    // D3 interior gap: saturate but leave ONE mid-depth cell air; how does the
    // defect present, and does it self-heal?
    {
      const b = basins[2];
      await ok(`fill ${b.x + 1} 70 -9 ${b.x + 12} 72 2 minecraft:water`);
      await ok(`setblock ${b.x + 6} 71 -3 minecraft:air`);
      const t = [];
      for (const at of [1, 3, 5, 10, 20]) {
        await sleep(at === 1 ? 1000 : (at - t[t.length - 1].t_s) * 1000);
        const gapIsSource = flag(
          await raw(`execute if block ${b.x + 6} 71 -3 minecraft:water[level=0] ${PROBE}`),
        );
        const gapIsWater = flag(
          await raw(`execute if block ${b.x + 6} 71 -3 minecraft:water ${PROBE}`),
        );
        t.push({ t_s: at, gapIsSource, gapIsWater });
      }
      await sleep(10000);
      const c = await flowingCensus(b.x + 1, 70, b.x + 12, -9, 2, 72);
      obs.basin_interior_gap = { gap_cell_over_time: t, flowing_after_30s: c.flowing, byLevel: c.byLevel };
    }
    // D4 waterloggables: stairs inside a body under a rising fill, and a
    // waterlogged block left behind by a lowering clear.
    {
      const b = basins[3];
      await ok(`setblock ${b.x + 3} 70 -3 minecraft:oak_stairs[facing=north]`);
      await ok(`fill ${b.x + 1} 70 -9 ${b.x + 12} 72 2 minecraft:water replace minecraft:air`);
      await sleep(5000);
      const stairsWaterlogged = flag(
        await raw(
          `execute if block ${b.x + 3} 70 -3 minecraft:oak_stairs[waterlogged=true] ${PROBE}`,
        ),
      );
      const stairsDry = flag(
        await raw(
          `execute if block ${b.x + 3} 70 -3 minecraft:oak_stairs[waterlogged=false] ${PROBE}`,
        ),
      );
      // Now the lowering half: clear the top layer with `replace water` — the
      // waterlogged block (if any) is NOT minecraft:water and survives; place
      // one explicitly to be sure, then clear around it and watch it leak.
      await ok(`setblock ${b.x + 8} 72 -3 minecraft:oak_stairs[facing=north,waterlogged=true]`);
      await ok(
        `fill ${b.x + 1} 72 -9 ${b.x + 12} 72 2 minecraft:air replace minecraft:water`,
      );
      await sleep(8000);
      const leakedFlow = await flowingCensus(b.x + 1, 72, b.x + 12, -9, 2);
      const stillWaterlogged = flag(
        await raw(
          `execute if block ${b.x + 8} 72 -3 minecraft:oak_stairs[waterlogged=true] ${PROBE}`,
        ),
      );
      obs.waterloggables = {
        fill_replace_air_waterlogs_stairs: stairsWaterlogged,
        stairs_left_dry_inside_body: stairsDry,
        clear_replace_water_leaves_waterlogged: stillWaterlogged,
        waterlogged_leaks_into_cleared_layer_flowing_cells: leakedFlow.flowing,
      };
    }
  }

  // ---- H. a SOLID runtime fill over a standing player (entombment) ----------
  {
    await ok("fill 16340 80 -10 16350 80 0 minecraft:stone");
    await ok(`tp ${BOT} 16345 81 -5`);
    await sleep(1500);
    await ok(`effect give ${BOT} minecraft:instant_health 1 10 true`);
    await sleep(500);
    const hp0 = num(await raw(`data get entity ${BOT} Health`));
    await ok("fill 16344 81 -6 16346 83 -4 minecraft:stone");
    const tomb = [];
    for (let i = 0; i < 8; i++) {
      await sleep(1000);
      tomb.push({ t_s: i + 1, hp: num(await raw(`data get entity ${BOT} Health`)) });
    }
    await ok("fill 16344 81 -6 16346 83 -4 minecraft:air");
    obs.solid_fill_entombment = { hp_before: hp0, hp_over_time: tomb };
  }

  // ---- site A again: lower BOTH raised layers around the (rescued) player ---
  {
    const t0 = Date.now();
    const l64 = await timedFill(-256, 255, -256, 255, 64, 64, "minecraft:air", "minecraft:water");
    const l63 = await timedFill(-256, 255, -256, 255, 63, 63, "minecraft:air", "minecraft:water");
    const ms = Date.now() - t0;
    await sleep(10000);
    const interior = await flowingCensus(-240, 63, 239, -240, 239, 64);
    obs.plane_lower_512_full = {
      ms,
      layer_64: l64,
      layer_63: l63,
      interior_flowing_after_10s: interior,
      note: "lowering back to the generator level: below is y=62 ambient water everywhere, so cleared cells sit above a still surface",
    };
  }

  writeFileSync(OUT, JSON.stringify(obs, null, 2) + "\n");
  console.log(`[measure] wrote ${OUT}`);
  process.exit(0);
}

main().catch((e) => {
  console.error(e);
  try {
    writeFileSync(OUT, JSON.stringify({ error: String(e), partial: obs }, null, 2) + "\n");
  } catch {}
  process.exit(1);
});
