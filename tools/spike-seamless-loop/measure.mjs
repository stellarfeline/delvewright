#!/usr/bin/env node
// SPIKE TOOLING (seamless relative-teleport loop) — NOT part of the shipped
// pipeline, not wired into CI. Driven by `run.sh` against a throwaway pinned
// 1.21.11 server.
//
// What a SERVER plus two protocol-level bots can say about a loop that moves a
// body by a whole-block offset when it crosses a one-cell slab:
//   * what the server does to the body in the tick it moves it — position,
//     motion and rotation read back before and after the `tp`, in the same tick;
//   * what the mover's CLIENT is told — the position packet's relative flags, its
//     deltas, and whether any chunk is sent or unloaded around the move;
//   * what the mover's client keeps across the move — its velocity, yaw and pitch
//     one tick before against the first tick after;
//   * whether a one-tick poll over a one-cell slab catches every crossing at the
//     fastest horizontal locomotion a body has (sprint-jumping), and whether it
//     catches a body falling at terminal speed, against a three-cell slab;
//   * what a WITNESS standing in the corridor is told about the mover;
//   * that the release score stands the loop down: a crossing after it moves nobody.
// What a client RENDERS between two frames is not measurable here.
//
// Every rcon reply is read: setup goes through `run` (throws on refusal), and a
// boolean probe is spelled `execute <cond> run time query gametime` so "false"
// (empty reply) is distinguishable from a malformed probe.

import { createRequire } from "node:module";
import { writeFileSync } from "node:fs";

import { rconChannel } from "../lib/rcon.mjs";

const require = createRequire(new URL("../../harness/package.json", import.meta.url));
const mineflayer = require("mineflayer");

const CONTAINER = process.env.SPIKE_CONTAINER ?? "dw-spike-loop";
const PORT = Number(process.env.SPIKE_PORT);
const OUT = process.env.SPIKE_OUT ?? new URL("./observations.json", import.meta.url).pathname;
const MOVER = "dw_loop_mover";
const WITNESS = "dw_loop_witness";
const OBJ = "dwl.s";
const OFFSET = { x: 0, y: 0, z: -12 };
const SLAB_Z = 4218;
const DROPS = 10;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const r = rconChannel(CONTAINER);
const obs = {
  instrument: {
    server: "itzg/minecraft-server@sha256:3e7db256… (versions.toml [images.base]), vanilla 1.21.11, view-distance 8",
    bots: "mineflayer 4.37.1 (harness pin); the mover's physics is prismarine-physics, the harness's own",
    rig: "tools/spike-seamless-loop/gen.py → dw-loop-spike (slab z=4218 one cell thick, offset 0 0 -12)",
  },
  replies: {},
};

async function run(cmd, key) {
  const reply = await r.run(cmd);
  if (key) obs.replies[key] = { cmd, reply };
  return reply;
}
async function flag(cond) {
  const reply = await r.probe(`execute ${cond} run time query gametime`);
  if (reply.startsWith("The time is")) return true;
  if (reply === "") return false;
  throw new Error(`probe did not evaluate: ${cond} -> ${JSON.stringify(reply)}`);
}
async function score(holder) {
  const reply = await r.probe(`scoreboard players get ${holder} ${OBJ}`);
  const m = reply.match(/ has (-?\d+) \[/);
  if (!m) throw new Error(`no score for ${holder}: ${reply}`);
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
const v3 = (p) => ({ x: p.x, y: p.y, z: p.z });
const sub = (a, b) => ({ x: a.x - b.x, y: a.y - b.y, z: a.z - b.z });

// ------------------------------------------------------------------ the bots
function makeBot(username) {
  return mineflayer.createBot({ host: "127.0.0.1", port: PORT, username, version: "1.21.11", auth: "offline" });
}
const mover = makeBot(MOVER);
const witness = makeBot(WITNESS);
const spawned = (b) => new Promise((res, rej) => { b.once("spawn", res); b.once("error", rej); b.once("kicked", rej); });

// Every packet the mover's client receives that bears on a move: its own
// position packets and chunk traffic.
const moverPackets = [];
const MOVER_WATCH = new Set(["position", "map_chunk", "unload_chunk", "set_chunk_cache_center"]);
mover._client.on("packet", (data, meta) => {
  if (!MOVER_WATCH.has(meta.name)) return;
  const e = { t: Date.now(), name: meta.name };
  if (meta.name === "position") e.pos = { x: data.x, y: data.y, z: data.z, dx: data.dx, dy: data.dy, dz: data.dz, yaw: data.yaw, pitch: data.pitch, flags: data.flags };
  else if (meta.name === "map_chunk" || meta.name === "unload_chunk") e.chunk = { x: data.x ?? data.chunkX, z: data.z ?? data.chunkZ };
  else e.center = { x: data.chunkX, z: data.chunkZ };
  moverPackets.push(e);
});
// Every packet the witness receives about the mover's entity.
const witnessPackets = [];
let moverEntityId = null;
witness._client.on("packet", (data, meta) => {
  if (moverEntityId === null || data?.entityId !== moverEntityId) return;
  witnessPackets.push({ t: Date.now(), name: meta.name });
});

// One snapshot of the mover per physics tick, so "the tick before the move" exists.
const ticks = [];
mover.on("physicsTick", () => {
  ticks.push({ t: Date.now(), pos: v3(mover.entity.position), vel: v3(mover.entity.velocity), yaw: mover.entity.yaw, pitch: mover.entity.pitch, onGround: mover.entity.onGround });
  if (ticks.length > 4000) ticks.shift();
});
const forced = [];
mover.on("forcedMove", () => {
  const before = ticks[ticks.length - 1];
  forced.push({ t: Date.now(), before, after: { pos: v3(mover.entity.position), vel: v3(mover.entity.velocity), yaw: mover.entity.yaw, pitch: mover.entity.pitch } });
});

await Promise.all([spawned(mover), spawned(witness)]);
await sleep(1500);
const since = (arr, t0, t1 = Infinity) => arr.filter((p) => p.t >= t0 && p.t <= t1);

async function serverReadback() {
  const g = async (k) => score(`#${k}`);
  const o = {};
  for (const suf of ["0", "1"]) {
    o[suf] = {
      pos: { x: (await g(`x${suf}`)) / 1000, y: (await g(`y${suf}`)) / 1000, z: (await g(`z${suf}`)) / 1000 },
      motion: { x: (await g(`mx${suf}`)) / 1000, y: (await g(`my${suf}`)) / 1000, z: (await g(`mz${suf}`)) / 1000 },
      yaw: (await g(`yaw${suf}`)) / 100,
      pitch: (await g(`pitch${suf}`)) / 100,
    };
  }
  return { before: o["0"], after: o["1"], delta_pos: sub(o["1"].pos, o["0"].pos), delta_motion: sub(o["1"].motion, o["0"].motion), delta_yaw: o["1"].yaw - o["0"].yaw, delta_pitch: o["1"].pitch - o["0"].pitch };
}


// The fastest tick of a phase, the loop's own move excluded by its size (no
// locomotion covers two blocks in a tick; the move covers twelve).
function fastestTick(phase) {
  let maxStep = 0, maxStepZ = 0;
  for (let i = 1; i < phase.length; i++) {
    const d = sub(phase[i].pos, phase[i - 1].pos);
    if (Math.abs(d.z) > 2) continue;
    const h = Math.hypot(d.x, d.z);
    if (h > maxStep) maxStep = h;
    if (Math.abs(d.z) > maxStepZ) maxStepZ = Math.abs(d.z);
  }
  return { maxStep, maxStepZ };
}

function moveRecord(f, t0) {
  const bot_delta = sub(f.after.pos, f.before.pos);
  // The client applied the offset to the position it held the tick before plus
  // at most one tick of its own motion, so the whole-block part is the offset.
  return {
    bot_before: f.before,
    bot_after: f.after,
    bot_delta_pos: bot_delta,
    bot_delta_vel: sub(f.after.vel, f.before.vel),
    bot_delta_yaw: f.after.yaw - f.before.yaw,
    bot_delta_pitch: f.after.pitch - f.before.pitch,
    position_packets: since(moverPackets, f.t - 300, f.t + 50).filter((p) => p.name === "position").map((p) => p.pos),
    chunk_packets_in_window: since(moverPackets, f.t - 300, f.t + 1500).filter((p) => p.name !== "position").map((p) => ({ name: p.name, ...(p.chunk ?? p.center) })),
    witness_packets_in_window: since(witnessPackets, f.t - 300, f.t + 600).reduce((acc, p) => ((acc[p.name] = (acc[p.name] ?? 0) + 1), acc), {}),
  };
}

try {
  // ---------------------------------------------------------------- load
  await run("reload", "reload");
  const packs = await run("datapack list enabled", "datapack_list");
  if (!packs.includes("dw-loop-spike")) throw new Error(`pack not enabled: ${packs}`);
  await run("function dwl:start", "start");
  await until(async () => (await score("#on")) === 1, 30000, "start");
  obs.build = {
    floor: await flag("if block 4096 63 4218 minecraft:dark_oak_planks"),
    wall: await flag("if block 4094 65 4218 minecraft:stone_bricks"),
    interior_air: await flag("if block 4096 65 4218 minecraft:air"),
    lamp: await flag("if block 4096 66 4196 minecraft:soul_lantern[hanging=true]"),
  };
  await run(`gamemode adventure ${MOVER}`);
  await run(`gamemode adventure ${WITNESS}`);
  // The witness stands in the corridor behind the slab, looking down it.
  await run(`tp ${WITNESS} 4096.5 64 4196.5 0 0`);
  await sleep(800);

  // ---------------------------------------------------------------- 1 one walked crossing
  await run(`tp ${MOVER} 4096.5 64 4206.5 0 0`);
  await sleep(1500);
  moverEntityId = witness.players[MOVER]?.entity?.id ?? Object.values(witness.entities).find((e) => e.username === MOVER)?.id ?? null;
  obs.witness_sees_mover = moverEntityId !== null;
  const t1 = Date.now();
  const base1 = forced.length;
  const tick1 = ticks.length;
  mover.setControlState("forward", true);
  await until(async () => (await score("#moves")) >= 1, 30000, "first move");
  mover.setControlState("forward", false);
  await sleep(600);
  obs.walk = { server: await serverReadback(), client: moveRecord(forced[base1], t1), forced_moves_in_phase: forced.length - base1, moves: await score("#moves"), fastest_tick: fastestTick(ticks.slice(tick1)), witness_packets_in_phase: since(witnessPackets, t1).reduce((acc, p) => ((acc[p.name] = (acc[p.name] ?? 0) + 1), acc), {}) };

  // ---------------------------------------------------------------- 2 a sprinting crossing
  await run(`tp ${MOVER} 4096.5 64 4206.5 0 0`);
  await sleep(1200);
  const t2 = Date.now();
  const base2 = forced.length;
  const tick2 = ticks.length;
  mover.setControlState("sprint", true);
  mover.setControlState("forward", true);
  await until(async () => (await score("#moves")) >= 2, 30000, "sprint move");
  mover.setControlState("forward", false);
  mover.setControlState("sprint", false);
  await sleep(600);
  obs.sprint = { server: await serverReadback(), client: moveRecord(forced[base2], t2), forced_moves_in_phase: forced.length - base2, moves: await score("#moves"), fastest_tick: fastestTick(ticks.slice(tick2)) };

  // ---------------------------------------------------------------- 3 sprint-jumping, many crossings
  await run(`tp ${MOVER} 4096.5 64 4206.5 0 0`);
  await sleep(1200);
  const t3 = Date.now();
  const tickStart = ticks.length;
  const base3 = forced.length;
  mover.setControlState("sprint", true);
  mover.setControlState("forward", true);
  mover.setControlState("jump", true);
  const target = 2 + 8;
  const slipped = async () => mover.entity.position.z > SLAB_Z + 3; // past the slab and not returned
  let slip = false;
  await until(async () => { if (await slipped()) { slip = true; return true; } return (await score("#moves")) >= target; }, 90000, "eight sprint-jump moves");
  mover.setControlState("jump", false);
  mover.setControlState("forward", false);
  mover.setControlState("sprint", false);
  await sleep(600);
  const { maxStep, maxStepZ } = fastestTick(ticks.slice(tickStart));
  obs.sprint_jump = {
    crossings_caught: (await score("#moves")) - 2,
    slipped_past_the_slab: slip,
    max_horizontal_blocks_per_tick: maxStep,
    max_z_blocks_per_tick: maxStepZ,
    per_move_bot_delta: forced.slice(base3).map((f) => sub(f.after.pos, f.before.pos)),
    per_move_vel_kept: forced.slice(base3).map((f) => ({ before: f.before.vel, after: f.after.vel })),
    last_server_readback: await serverReadback(),
  };

  // ---------------------------------------------------------------- 4 release, then a crossing that moves nobody
  await run(`tp ${MOVER} 4096.5 64 4206.5 0 0`);
  await sleep(1200);
  await run(`scoreboard players set #released ${OBJ} 1`, "release");
  const movesBefore = await score("#moves");
  const forcedBefore = forced.length;
  mover.setControlState("forward", true);
  await until(async () => mover.entity.position.z > SLAB_Z + 8, 30000, "walk through the released slab");
  mover.setControlState("forward", false);
  obs.release = {
    moves_before: movesBefore,
    moves_after: await score("#moves"),
    forced_moves_during: forced.length - forcedBefore,
    mover_z: mover.entity.position.z,
  };

  // ---------------------------------------------------------------- 5 a falling body against a one-cell and a three-cell slab
  // The per-tick fall of one drop, and the law it obeys: successive speeds fit
  // v' = k·(v + g) by least squares over the pairs, whose fixed point k·g/(1−k)
  // is the speed a longer fall would approach. The slab rule takes that limit,
  // not the fastest tick a 184-block fall happened to reach.
  function fallLaw(dys) {
    const pairs = [];
    for (let i = 1; i < dys.length; i++) if (dys[i - 1] > 0.5 && dys[i] > 0.5) pairs.push([dys[i - 1], dys[i]]);
    const n = pairs.length;
    const sx = pairs.reduce((a, [x]) => a + x, 0), sy = pairs.reduce((a, [, y]) => a + y, 0);
    const sxx = pairs.reduce((a, [x]) => a + x * x, 0), sxy = pairs.reduce((a, [x, y]) => a + x * y, 0);
    const k = (n * sxy - sx * sy) / (n * sxx - sx * sx);
    const b = (sy - k * sx) / n; // v' = k·v + b, so g = b / k
    return { pairs: n, k, g: b / k, limit_blocks_per_tick: b / (1 - k), samples: dys.map((v) => Number(v.toFixed(4))) };
  }
  async function drops(thick) {
    await run(`scoreboard players set #thick ${OBJ} ${thick ? 1 : 0}`);
    let caught = 0;
    let maxFall = 0;
    let atSlab = 0;
    let law = null;
    for (let i = 0; i < DROPS; i++) {
      await run(`tag ${MOVER} remove dwl_caught`);
      await run(`tp ${MOVER} 4096.5 310 4302.5 0 0`);
      const start = ticks.length;
      await until(async () => mover.entity.position.y < 130, 30000, "the fall");
      await sleep(300);
      const dys = [];
      for (let k = Math.max(1, start); k < ticks.length; k++) {
        const dy = ticks[k - 1].pos.y - ticks[k].pos.y;
        if (dy > 0 && dy < 50) {
          dys.push(dy);
          if (dy > maxFall) maxFall = dy;
          // the speed on the tick the body passed the slab's height
          if (ticks[k - 1].pos.y >= 150 && ticks[k].pos.y < 151 && dy > atSlab) atSlab = dy;
        }
      }
      if (!law) law = fallLaw(dys);
      if (await flag(`if entity @a[name=${MOVER},tag=dwl_caught]`)) caught += 1;
      await run(`tp ${MOVER} 4096.5 64 4196.5 0 0`);
      await sleep(400);
    }
    return { drops: DROPS, caught, max_fall_blocks_per_tick: maxFall, fastest_tick_at_the_slab: atSlab, law };
  }
  obs.fall_thin_slab = await drops(false);
  obs.fall_thick_slab = await drops(true);

  // ---------------------------------------------------------------- stop
  await run("function dwl:stop", "stop");
} finally {
  writeFileSync(OUT, JSON.stringify(obs, null, 2) + "\n");
  mover.quit();
  witness.quit();
}
