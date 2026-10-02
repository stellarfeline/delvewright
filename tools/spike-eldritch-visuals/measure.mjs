#!/usr/bin/env node
// SPIKE TOOLING (eldritch visuals lab) — NOT part of the shipped pipeline, not
// wired into CI. Driven by `run.sh` against a throwaway pinned 1.21.11 server.
//
// What a SERVER plus a protocol-level bot can say about the lab: that the pack
// loads, every station builds, every trigger fires, every effect/particle/sound
// packet is SENT to the client, and — the question another concept needs —
// whether a `/fillbiome` reaches an already-connected client without a chunk
// reload (a `chunk_biomes` packet for exactly the changed chunks, and no
// `map_chunk`/`unload_chunk` for them). What a client RENDERS is not measurable
// here; README.md separates the two.
//
// Every rcon reply is read: setup goes through `run` (throws on refusal), and a
// boolean probe is spelled `execute <cond> run time query gametime` so "false"
// (empty reply) is distinguishable from a malformed probe.

import { createRequire } from "node:module";
import { writeFileSync } from "node:fs";

import { rconChannel } from "../lib/rcon.mjs";

const require = createRequire(new URL("../../harness/package.json", import.meta.url));
const mineflayer = require("mineflayer");

const CONTAINER = process.env.SPIKE_CONTAINER ?? "dw-spike-eldritch";
const PORT = Number(process.env.SPIKE_PORT);
const OUT = process.env.SPIKE_OUT ?? new URL("./observations.json", import.meta.url).pathname;
const BOT = "dw_eldritch";

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const r = rconChannel(CONTAINER);
const obs = { instrument: { server: "itzg/minecraft-server@sha256:3e7db256… (versions.toml [images.base]), vanilla 1.21.11", bot: "mineflayer 4.37.1 (harness pin)" }, replies: {} };

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
  const reply = await r.probe(`scoreboard players get ${holder} dwe.s`);
  const m = reply.match(/ has (-?\d+) \[/);
  if (!m) throw new Error(`no score for ${holder}: ${reply}`);
  return Number(m[1]);
}
async function count(sel) {
  await run(`execute store result score #n dwe.s if entity ${sel}`);
  return score("#n");
}
async function until(fn, ms, what) {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    if (await fn()) return true;
    await sleep(200);
  }
  throw new Error(`timed out waiting for ${what}`);
}

// ------------------------------------------------------------------ the bot
const bot = mineflayer.createBot({ host: "127.0.0.1", port: PORT, username: BOT, version: "1.21.11", auth: "offline" });
const packets = [];
const WATCH = new Set(["chunk_biomes", "map_chunk", "unload_chunk", "world_particles", "sound_effect", "position"]);
bot._client.on("packet", (data, meta) => {
  if (!WATCH.has(meta.name)) return;
  const e = { t: Date.now(), name: meta.name };
  if (meta.name === "chunk_biomes") e.chunks = data.biomes.map((b) => b.position);
  else if (meta.name === "map_chunk" || meta.name === "unload_chunk") e.chunk = { x: data.x ?? data.chunkX, z: data.z ?? data.chunkZ };
  else if (meta.name === "world_particles") e.particle = data.particle?.type ?? data.particle;
  else if (meta.name === "sound_effect") e.sound = data.sound;
  else if (meta.name === "position") e.pos = { x: data.x, y: data.y, z: data.z, dx: data.dx, dz: data.dz, flags: data.flags };
  packets.push(e);
});
await new Promise((res, rej) => { bot.once("spawn", res); bot.once("error", rej); bot.once("kicked", rej); });
await sleep(1500);
const since = (t0) => packets.filter((p) => p.t >= t0);
const P = bot.registry;

try {
  // ---------------------------------------------------------------- load
  await run("reload", "reload");
  const packs = await run("datapack list enabled", "datapack_list");
  if (!packs.includes("dw-eldritch-spike")) throw new Error(`pack not enabled: ${packs}`);
  await run(`execute as ${BOT} run function dwe:start`, "start");
  await until(async () => (await r.probe("scoreboard players get #on dwe.s")).includes(" has 1 "), 90000, "build to finish");
  obs.build = {
    block_displays: await count("@e[type=minecraft:block_display,tag=dwe]"),
    block_displays_main: await count("@e[type=minecraft:block_display,tag=dwe_main]"),
    endermen: await count("@e[type=minecraft:enderman,tag=dwe_watcher]"),
    glow_squids: await count("@e[type=minecraft:glow_squid,tag=dwe]"),
    viewer_tagged: await count(`@a[tag=dwe_viewer]`),
    floor_at_start: await flag("if block 4096 63 4090 minecraft:polished_deepslate"),
    pit_open: await flag("if block 4096 63 4116 minecraft:air"),
    lab_biome_at_zone: await flag("if biome 4096 70 4140 dwe:lab"),
    corridor_lamp: await flag("if block 4096 66 4196 minecraft:soul_lantern[hanging=true]"),
    shrieker: await flag("if block 4096 64 4298 minecraft:sculk_shrieker"),
    sign_text: await r.probe("data get block 4092 64 4100 front_text.messages"),
    bot_pos_after_start: { ...bot.entity.position },
  };
  obs.tick_before = await run("tick query", "tick_query_before");

  // ---------------------------------------------------------------- 1 tentacle
  await run(`tp ${BOT} 4096.5 64 4104.5 0 0`);
  await until(async () => (await score("#tframe")) >= 1, 5000, "tentacle wake");
  const tip = [];
  for (let i = 0; i < 6; i++) {
    tip.push({ s: i * 2, frame: await score("#tframe"), translation: await r.probe("data get entity @e[tag=dwe_main,tag=dwe_seg_33,limit=1] transformation.translation"), brightness: await r.probe("data get entity @e[tag=dwe_main,tag=dwe_seg_33,limit=1] brightness") });
    await sleep(2000);
  }
  obs.tentacle = { tip_samples: tip, interpolation_duration: await r.probe("data get entity @e[tag=dwe_main,tag=dwe_seg_0,limit=1] interpolation_duration") };
  obs.tick_during = await run("tick query", "tick_query_during_tentacle");

  // ---------------------------------------------------------------- 2 zone / fillbiome
  const t2 = Date.now();
  await run(`tp ${BOT} 4096.5 64 4130.5 0 0`);
  await until(async () => (await score("#zone")) === 1, 5000, "zone trigger");
  await sleep(2500);
  const win = since(t2);
  const zoneChunks = new Set();
  for (let x = 4080 >> 4; x <= 4111 >> 4; x++) for (let z = 4128 >> 4; z <= 4167 >> 4; z++) zoneChunks.add(`${x},${z}`);
  const biomePk = win.filter((p) => p.name === "chunk_biomes");
  const got = new Set(biomePk.flatMap((p) => p.chunks.map((c) => `${c.x},${c.z}`)));
  obs.fillbiome = {
    server_inside_is_wrong_place: await flag("if biome 4096 70 4140 dwe:wrong_place"),
    server_outside_still_lab: await flag("if biome 4096 70 4120 dwe:lab"),
    expected_chunks: [...zoneChunks],
    chunk_biomes_packets: biomePk.length,
    chunk_biomes_chunks: [...got],
    all_expected_received: [...zoneChunks].every((k) => got.has(k)),
    map_chunk_for_zone: win.filter((p) => p.name === "map_chunk" && zoneChunks.has(`${p.chunk.x},${p.chunk.z}`)).length,
    unload_chunk_for_zone: win.filter((p) => p.name === "unload_chunk" && zoneChunks.has(`${p.chunk.x},${p.chunk.z}`)).length,
    raw_first: biomePk[0] ?? null,
  };

  // ---------------------------------------------------------------- 3 perception
  const t3 = Date.now();
  await run(`tp ${BOT} 4096.5 64 4176.5 0 0`);
  await until(async () => (await score("#p3")) === 1, 5000, "plate trigger");
  await sleep(500);
  const effects1 = await r.probe(`data get entity ${BOT} active_effects`);
  await sleep(3000);
  const effects2 = await r.probe(`data get entity ${BOT} active_effects`);
  const w3 = since(t3);
  const curseId = P.soundsByName["entity.elder_guardian.curse"].id;
  obs.perception = {
    effects_at_0_5s: effects1,
    effects_at_3_5s: effects2,
    elder_guardian_particle_id: P.particlesByName.elder_guardian.id,
    particles_received: w3.filter((p) => p.name === "world_particles").map((p) => p.particle),
    sounds_received: w3.filter((p) => p.name === "sound_effect").map((p) => p.sound),
    curse_sound_registry_id: curseId,
  };
  await run(`tp ${BOT} 4096.5 64 4170.5 0 0`);
  await sleep(500);

  // ---------------------------------------------------------------- 4 endless hall
  const t4 = Date.now();
  await run(`tp ${BOT} 4096.5 64 4206.5 0 0`);
  await sleep(1500);
  const loops = [];
  bot.on("forcedMove", () => loops.push({ t: Date.now(), z: bot.entity.position.z }));
  bot.setControlState("forward", true);
  await until(async () => (await score("#loops")) >= 6, 60000, "six loops");
  obs.hall = {
    loops_scored: await score("#loops"),
    walls_cracked_after_loop_2: await flag("if block 4094 65 4200 minecraft:cracked_stone_bricks"),
    lamp_4196_removed_after_loop_4: await flag("if block 4096 66 4196 minecraft:air"),
    lamp_4190_kept: await flag("if block 4096 66 4190 minecraft:soul_lantern[hanging=true]"),
    forced_moves: loops,
    position_packets: since(t4).filter((p) => p.name === "position").map((p) => p.pos),
  };

  // walk on until the room
  await until(async () => bot.entity.position.z > 4281, 60000, "reach the room");
  bot.setControlState("forward", false);
  obs.hall.released_bot_z = bot.entity.position.z;

  // ---------------------------------------------------------------- 5 living room
  const t5 = Date.now();
  await run(`tp ${BOT} 4092.5 64 4282.5 0 0`);
  await sleep(1000);
  const yawRead = await r.probe("data get entity @e[tag=dwe_watcher,limit=1] Rotation");
  const watcherPos0 = await r.probe("data get entity @e[tag=dwe_watcher,limit=1] Pos");
  let sensorActive = false;
  let shrieked = false;
  bot.setControlState("forward", true);
  const endWalk = Date.now() + 4000;
  while (Date.now() < endWalk) {
    if (!sensorActive) sensorActive = await flag("if block 4088 64 4286 minecraft:sculk_sensor[sculk_sensor_phase=active]") || await flag("if block 4088 64 4290 minecraft:sculk_sensor[sculk_sensor_phase=active]");
    if (!shrieked) shrieked = await flag("if block 4096 64 4298 minecraft:sculk_shrieker[shrieking=true]");
    await sleep(150);
  }
  bot.setControlState("forward", false);
  await sleep(1500);
  const w5 = since(t5);
  const hb = P.soundsByName["entity.warden.heartbeat"].id;
  obs.living = {
    watcher_rotation: yawRead,
    watcher_pos_before: watcherPos0,
    watcher_pos_after: await r.probe("data get entity @e[tag=dwe_watcher,limit=1] Pos"),
    watcher_wpos: await score("#wpos"),
    bot_pos: { ...bot.entity.position },
    sculk_sensor_went_active: sensorActive,
    shrieker_shrieked: shrieked,
    heartbeat_registry_id: hb,
    sounds_received: w5.filter((p) => p.name === "sound_effect").map((p) => p.sound),
  };

  // ---------------------------------------------------------------- stop
  await run(`execute as ${BOT} run function dwe:stop`, "stop");
  await sleep(500);
  obs.after_stop = { dwe_entities: await count("@e[tag=dwe]") };
} finally {
  writeFileSync(OUT, JSON.stringify(obs, null, 2) + "\n");
  bot.quit();
}
console.log(JSON.stringify(obs, null, 2));
