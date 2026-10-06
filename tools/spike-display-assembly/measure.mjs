#!/usr/bin/env node
// SPIKE TOOLING (display assembly) — NOT part of the shipped pipeline, not wired
// into CI. Driven by `run.sh` against a throwaway pinned 1.21.11 server. Reads
// every coordinate from `site.json` (written by `gen.py`), never retypes one.
//
// The questions, each answered by the server's own state and the bot's packets:
//   A  a melee attack on the `interaction` hitbox writes `attack`, the poll
//      counts it, and the fifth hit plays `retract`;
//   B  an arrow (shot from a bow by the bot, and summoned with and without an
//      owner) does or does not write `attack`, and where it comes to rest;
//   C  how many passengers one entity holds, and what a `tp` of the root, a
//      `rotate` of the root and a passenger's own in-place `tp` do to the set;
//   D  the strike pattern: windup, hold, strike, damage at the landing frame to a
//      body in the strike box and to nobody else; idle when nobody is in the
//      trigger box;
//   E  tick cost at 1, 4 and 10 assemblies (34 parts each) at 5-tick and 1-tick
//      keyframes;
//   F  the distance at which the server stops tracking the parts for a client;
//   G  `stop` leaves nothing behind.
//
// Every rcon reply is read: setup goes through `run` (throws on refusal); a
// boolean probe is spelled `execute <cond> run time query gametime` so "false"
// (empty reply) is distinguishable from a malformed probe.

import { createRequire } from "node:module";
import { readFileSync, writeFileSync } from "node:fs";
import { execFile } from "node:child_process";
import { promisify } from "node:util";

import { rconChannel } from "../lib/rcon.mjs";

const require = createRequire(new URL("../../harness/package.json", import.meta.url));
const mineflayer = require("mineflayer");
const { Vec3 } = require("vec3");
const execFileP = promisify(execFile);

const CONTAINER = process.env.SPIKE_CONTAINER ?? "dw-spike-assembly";
const PORT = Number(process.env.SPIKE_PORT);
const OUT = process.env.SPIKE_OUT ?? new URL("./observations.json", import.meta.url).pathname;
const SITE = JSON.parse(readFileSync(new URL("./site.json", import.meta.url), "utf8"));
const BOT = "dw_asm";

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const r = rconChannel(CONTAINER);
const obs = {
  instrument: {
    server: "itzg/minecraft-server@sha256:3e7db256… (versions.toml [images.base]), vanilla 1.21.11, flat world, adventure",
    bot: "mineflayer 4.37.1 (harness pin), offline, protocol 1.21.11",
    rig: "tools/spike-display-assembly/gen.py -> spikepack/ (site.json is the coordinate authority)",
    host: `${process.platform} ${process.arch}, node ${process.version}`,
  },
  site: SITE,
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
  const reply = await r.probe(`scoreboard players get ${holder} dwa.s`);
  const m = reply.match(/ has (-?\d+) \[/);
  if (!m) throw new Error(`no score for ${holder}: ${reply}`);
  return Number(m[1]);
}
async function count(sel) {
  await run(`execute store result score #n dwa.s if entity ${sel}`);
  return score("#n");
}
async function passengersOf(sel) {
  await run("scoreboard players set #n dwa.s 0");
  await r.probe(`execute as ${sel} on passengers run scoreboard players add #n dwa.s 1`);
  return score("#n");
}
async function until(fn, ms, what) {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    if (await fn()) return true;
    await sleep(150);
  }
  throw new Error(`timed out waiting for ${what}`);
}
async function gametime() {
  const reply = await run("time query gametime");
  const m = reply.match(/The time is (\d+)/);
  return Number(m[1]);
}
function parseTick(reply) {
  const avg = reply.match(/Average time per tick: ([\d.]+)ms/);
  const p50 = reply.match(/P50: ([\d.]+)ms/);
  const p95 = reply.match(/P95: ([\d.]+)ms/);
  const p99 = reply.match(/P99: ([\d.]+)ms/);
  const n = reply.match(/Sample: (\d+)/);
  return { avg_ms: Number(avg?.[1]), p50_ms: Number(p50?.[1]), p95_ms: Number(p95?.[1]), p99_ms: Number(p99?.[1]), sample: Number(n?.[1]), raw: reply };
}
async function tickQuery(label) {
  // `tick query` samples the last 100 ticks: wait a full window first.
  await sleep(6000);
  const t = parseTick(await run("tick query"));
  t.label = label;
  t.entities = { parts: await count("@e[tag=dwa_part]"), roots: await count("@e[tag=dwa_root]"), hitboxes: await count("@e[tag=dwa_hitbox]") };
  return t;
}
async function health() {
  const reply = await run(`data get entity ${BOT} Health`);
  const m = reply.match(/: ([\d.]+)f/);
  return Number(m[1]);
}
async function pos(sel) {
  const reply = await r.probe(`data get entity ${sel} Pos`);
  const m = reply.match(/\[(-?[\d.]+)d, (-?[\d.]+)d, (-?[\d.]+)d\]/);
  return m ? [Number(m[1]), Number(m[2]), Number(m[3])] : reply;
}
async function rotation(sel) {
  const reply = await r.probe(`data get entity ${sel} Rotation`);
  const m = reply.match(/\[(-?[\d.]+)f, (-?[\d.]+)f\]/);
  return m ? [Number(m[1]), Number(m[2])] : reply;
}
function uuidIntArray(uuid) {
  const hex = uuid.replace(/-/g, "");
  const out = [];
  for (let i = 0; i < 4; i++) out.push(Number(BigInt.asIntN(32, BigInt("0x" + hex.slice(i * 8, i * 8 + 8)))));
  return `[I;${out.join(",")}]`;
}
const S = SITE;
const P = S.positions;
const tp = (p, yaw = 0) => run(`tp ${BOT} ${p[0]} ${p[1]} ${p[2]} ${yaw} 0`);
const botEntities = (name) => Object.values(bot.entities).filter((e) => e.name === name);

// ------------------------------------------------------------------ the bot
const bot = mineflayer.createBot({ host: "127.0.0.1", port: PORT, username: BOT, version: "1.21.11", auth: "offline" });
const packets = [];
const WATCH = new Set(["spawn_entity", "entity_destroy", "remove_entities", "damage_event", "set_passengers", "entity_teleport", "sync_entity_position"]);
bot._client.on("packet", (data, meta) => {
  if (!WATCH.has(meta.name)) return;
  packets.push({ t: Date.now(), name: meta.name, data: meta.name === "spawn_entity" ? { type: data.type, id: data.entityId } : meta.name === "damage_event" ? { id: data.entityId, sourceType: data.sourceTypeId } : undefined });
});
await new Promise((res, rej) => { bot.once("spawn", res); bot.once("error", rej); bot.once("kicked", rej); });
await sleep(1500);
const since = (t0) => packets.filter((p) => p.t >= t0);

try {
  // ---------------------------------------------------------------- server properties
  const { stdout: props } = await execFileP("docker", ["exec", CONTAINER, "sh", "-c", "grep -E '^(view-distance|simulation-distance|entity-broadcast-range-percentage|gamemode|difficulty)=' /data/server.properties"]);
  obs.server_properties = props.trim().split("\n");

  // ---------------------------------------------------------------- load
  await run("reload", "reload");
  const packs = await run("datapack list enabled", "datapack_list");
  if (!packs.includes("dw-spike-assembly")) throw new Error(`pack not enabled: ${packs}`);
  await run(`gamemode adventure ${BOT}`);
  await run(`function dwa:start`, "start");
  await until(async () => (await score("#on")) !== 0, 90000, "build to finish");
  if ((await score("#on")) !== 1) throw new Error("the site never finished loading");
  await tp(P.outside);
  await sleep(2000);
  obs.build = {
    parts: await count("@e[tag=dwa_part]"),
    roots: await count("@e[tag=dwa_root]"),
    hitboxes: await count("@e[tag=dwa_hitbox]"),
    root_passengers: await passengersOf("@e[tag=dwa_root_0,limit=1]"),
    part0_pos: await pos("@e[tag=dwa_asm_0,tag=dwa_p_0,limit=1]"),
    hitbox_pos: await pos("@e[tag=dwa_hit_0,limit=1]"),
    hitbox_nbt: await run("data get entity @e[tag=dwa_hit_0,limit=1] width"),
    part0_interpolation_duration: await run("data get entity @e[tag=dwa_asm_0,tag=dwa_p_0,limit=1] interpolation_duration"),
    clip_playing: await score("#clip_0"),
    frame: await score("#f_0"),
    bot_sees: { block_display: botEntities("block_display").length, item_display: botEntities("item_display").length, interaction: botEntities("interaction").length },
    floor_marked_strike: await flag(`if block ${S.strike_box.x} ${S.fy} ${S.strike_box.z} minecraft:red_concrete`),
  };
  await sleep(1500);
  obs.build.frame_advances = (await score("#f_0")) !== obs.build.frame;

  // ---------------------------------------------------------------- C riding
  // how many passengers does one entity take? asked directly, so every refusal is read
  const a = [S.ax + 0.5, S.wy, S.az + 10.5];
  await run(`summon minecraft:item_display ${a[0]} ${a[1]} ${a[2]} {Tags:["dwa","dwa_cap_root"]}`);
  const capacity = [];
  for (let i = 0; i < 3; i++) {
    await run(`summon minecraft:block_display ${a[0]} ${a[1]} ${a[2]} {Tags:["dwa","dwa_cap","dwa_cap_${i}"],block_state:{Name:"minecraft:stone"}}`);
    capacity.push({ passenger: i + 1, reply: await r.probe(`ride @e[tag=dwa_cap_${i},limit=1] mount @e[tag=dwa_cap_root,limit=1]`) });
  }
  const capCount = await passengersOf("@e[tag=dwa_cap_root,limit=1]");
  const stillRiding = [];
  for (let i = 0; i < 3; i++) {
    await run("scoreboard players set #n dwa.s 0");
    await r.probe(`execute store result score #n dwa.s as @e[tag=dwa_cap_${i},limit=1] on vehicle if entity @s`);
    stillRiding.push({ passenger: i + 1, riding: await score("#n") });
  }
  await run(`summon minecraft:zombie ${a[0]} ${a[1]} ${a[2]} {NoAI:1b,Silent:1b,Tags:["dwa","dwa_cap_mob"]}`);
  const mobRides = [];
  for (let i = 0; i < 2; i++) {
    await run(`summon minecraft:block_display ${a[0]} ${a[1]} ${a[2]} {Tags:["dwa","dwa_cap","dwa_capm_${i}"],block_state:{Name:"minecraft:stone"}}`);
    mobRides.push({ passenger: i + 1, reply: await r.probe(`ride @e[tag=dwa_capm_${i},limit=1] mount @e[tag=dwa_cap_mob,limit=1]`) });
  }
  const mobCapCount = await passengersOf("@e[tag=dwa_cap_mob,limit=1]");
  await run("kill @e[tag=dwa_cap_root]"); await run("kill @e[tag=dwa_cap_mob]"); await run("kill @e[tag=dwa_cap]");
  obs.passenger_capacity = { item_display_root: { rides: capacity, passengers_after: capCount, each_still_riding: stillRiding }, zombie: { rides: mobRides, passengers_after: mobCapCount } };

  // the chain: p_0 rides the root, p_i rides p_{i-1}
  const vehicleOf = async (i) => { await run("scoreboard players set #n dwa.s 0"); await r.probe(`execute store result score #n dwa.s as @e[tag=dwa_asm_0,tag=dwa_p_${i},limit=1] on vehicle if entity @s`); return score("#n"); };
  const chainState = async () => {
    const out = [];
    for (const i of [0, 1, 17, 33]) out.push({ part: i, has_vehicle: await vehicleOf(i), pos: await pos(`@e[tag=dwa_asm_0,tag=dwa_p_${i},limit=1]`), rotation: await rotation(`@e[tag=dwa_asm_0,tag=dwa_p_${i},limit=1]`) });
    return { root_pos: await pos("@e[tag=dwa_root_0,limit=1]"), root_rotation: await rotation("@e[tag=dwa_root_0,limit=1]"), parts: out, bot_min_part_x: botEntities("block_display").map((e) => e.position.x).sort()[0], bot_sees_parts: botEntities("block_display").length };
  };
  obs.chain = { mount: "star: every part rides the root", root_direct_passengers: obs.build.root_passengers, at_rest: await chainState() };
  // `/tp <target> ~x` is relative to the COMMAND SOURCE (rcon = world spawn), never the target.
  const t1 = Date.now();
  obs.chain.root_moved_plus2x = { reply: await r.probe("execute as @e[tag=dwa_root_0,limit=1] at @s run tp @s ~2 ~ ~") };
  await sleep(600);
  Object.assign(obs.chain.root_moved_plus2x, await chainState(), { packets_to_bot: since(t1).map((p) => p.name) });
  obs.chain.root_turned_90_in_place = { reply: await r.probe("execute as @e[tag=dwa_root_0,limit=1] at @s run tp @s ~-2 ~ ~ 90 0") };
  await sleep(400);
  Object.assign(obs.chain.root_turned_90_in_place, await chainState());
  obs.chain.root_rotated_by_rotate_command = { reply: await r.probe("rotate @e[tag=dwa_root_0,limit=1] 45 0") };
  await sleep(400);
  Object.assign(obs.chain.root_rotated_by_rotate_command, await chainState());
  // a passenger teleported in place: does it stay mounted? (the first run: no)
  obs.chain.one_passenger_teleported_in_place = { reply: await r.probe("execute as @e[tag=dwa_asm_0,tag=dwa_p_17,limit=1] at @s run tp @s ~ ~ ~ ~ ~") };
  await sleep(400);
  Object.assign(obs.chain.one_passenger_teleported_in_place, await chainState(), { root_passengers_now: await passengersOf("@e[tag=dwa_root_0,limit=1]") });
  await run("ride @e[tag=dwa_asm_0,tag=dwa_p_17,limit=1] mount @e[tag=dwa_root_0,limit=1]").catch(() => {});
  obs.chain.reset = { reply: await r.probe("execute as @e[tag=dwa_root_0,limit=1] at @s run tp @s ~ ~ ~ 0 0") };
  await sleep(400);
  Object.assign(obs.chain.reset, await chainState(), { root_passengers_now: await passengersOf("@e[tag=dwa_root_0,limit=1]") });
  obs.chain.data_merge_on_passenger = await run("execute as @e[tag=dwa_asm_0,tag=dwa_p_33,limit=1] run data merge entity @s {start_interpolation:0}", "merge_on_passenger");

  // ---------------------------------------------------------------- A melee
  await tp(P.attack, 180);
  await sleep(800);
  await bot.lookAt(new Vec3(S.ax + 0.5, S.wy + 2, S.az + 0.5), true);
  const hb = botEntities("interaction")[0];
  if (!hb) throw new Error("the bot does not see the interaction hitbox");
  await run("scoreboard players set #poll dwa.s 0");
  const rawBefore = await r.probe("data get entity @e[tag=dwa_hit_0,limit=1] attack");
  bot.attack(hb);
  await sleep(400);
  const rawAfter = await r.probe("data get entity @e[tag=dwa_hit_0,limit=1] attack");
  await run("data remove entity @e[tag=dwa_hit_0,limit=1] attack");
  await run("scoreboard players set #hits dwa.s 0");
  await run("scoreboard players set #poll dwa.s 1");
  const clipBefore = await score("#clip_0");
  const hitTimes = [];
  for (let i = 0; i < 7; i++) {
    bot.attack(hb);
    await sleep(600);
    hitTimes.push({ swing: i + 1, hits: await score("#hits"), clip: await score("#clip_0") });
  }
  obs.melee = {
    bot_pos: { ...bot.entity.position },
    hitbox_entity_seen_by_bot: { id: hb.id, name: hb.name, pos: { ...hb.position } },
    attack_record_before: rawBefore,
    attack_record_after_one_attack: rawAfter,
    attack_record_names_player: /player: \[I;/.test(rawAfter),
    clip_before: clipBefore,
    per_swing: hitTimes,
    hits_counted: await score("#hits"),
    clip_after: await score("#clip_0"),
    retract_index: S.clips.retract.index,
    retract_played_at_fifth_hit: hitTimes[4]?.clip === S.clips.retract.index && hitTimes[3]?.clip !== S.clips.retract.index,
  };
  // the same hitbox riding the chain's last part: does an attack still register?
  await run("ride @e[tag=dwa_hit_0,limit=1] mount @e[tag=dwa_asm_0,tag=dwa_p_33,limit=1]", "mount_hitbox");
  await sleep(300);
  await run("scoreboard players set #hits dwa.s 0");
  bot.attack(hb);
  await sleep(600);
  obs.melee.hitbox_riding_a_part = { pos: await pos("@e[tag=dwa_hit_0,limit=1]"), hits_after_one_attack: await score("#hits") };
  await run("ride @e[tag=dwa_hit_0,limit=1] dismount");
  // back to idle so the strike rows start from a known clip
  await run(`function dwa:play/0_${S.clips.idle.index}`);
  await run("scoreboard players set #hits dwa.s 0");

  // ---------------------------------------------------------------- B arrows
  await run("scoreboard players set #poll dwa.s 0");
  await run("kill @e[type=minecraft:arrow]").catch(() => {});
  await tp(P.bow, 180);
  await run(`give ${BOT} minecraft:bow 1`);
  await run(`give ${BOT} minecraft:arrow 8`);
  await until(() => bot.inventory.items().some((i) => i.name === "bow") && bot.inventory.items().some((i) => i.name === "arrow"), 5000, "bow and arrows in inventory");
  await bot.equip(bot.inventory.items().find((i) => i.name === "bow"), "hand");
  await bot.lookAt(new Vec3(...P.bow_aim), true);
  await sleep(300);
  const arrowsBefore = await count("@e[type=minecraft:arrow]");
  const tShot = Date.now();
  bot.activateItem();
  await sleep(1300);
  bot.deactivateItem();
  await sleep(2000);
  const arrowsAfter = await count("@e[type=minecraft:arrow]");
  const shot = {
    arrows_before: arrowsBefore,
    arrows_after_shot: arrowsAfter,
    arrow_pos: arrowsAfter > 0 ? await pos("@e[type=minecraft:arrow,limit=1,sort=nearest]") : null,
    arrow_in_ground: arrowsAfter > 0 ? await run("data get entity @e[type=minecraft:arrow,limit=1,sort=nearest] inGround") : null,
    attack_record: await r.probe("data get entity @e[tag=dwa_hit_0,limit=1] attack"),
    damage_events_seen_by_bot: since(tShot).filter((p) => p.name === "damage_event").length,
  };
  const box = S.hitbox.aabb;
  const classify = (p) => (!Array.isArray(p) ? "no arrow" : p[2] < box[2] ? "passed through (rests north of the box)" : p[2] > box[5] ? "stopped before the box (bounced or fell short)" : "inside the box's z-span");
  shot.verdict = classify(shot.arrow_pos);
  await run("kill @e[type=minecraft:arrow]").catch(() => {});
  await run("data remove entity @e[tag=dwa_hit_0,limit=1] attack").catch(() => {});
  // summoned arrows: with the bot as owner, and with none
  const owner = uuidIntArray(bot.entity.uuid ?? bot._client.uuid);
  const summoned = [];
  for (const [label, extra] of [["owned by the bot", `Owner:${owner},`], ["no owner", ""]]) {
    const a = P.arrow_summon;
    await run(`summon minecraft:arrow ${a[0]} ${a[1]} ${a[2]} {${extra}Motion:[0.0d,0.0d,-1.6d],NoGravity:1b,Tags:["dwa_arrow"]}`);
    await sleep(1500);
    const n = await count("@e[tag=dwa_arrow]");
    const p = n > 0 ? await pos("@e[tag=dwa_arrow,limit=1]") : null;
    summoned.push({
      label,
      arrows: n,
      arrow_pos_after_1500ms: p,
      verdict: classify(p),
      attack_record: await r.probe("data get entity @e[tag=dwa_hit_0,limit=1] attack"),
    });
    await run("kill @e[tag=dwa_arrow]").catch(() => {});
    await run("data remove entity @e[tag=dwa_hit_0,limit=1] attack").catch(() => {});
  }
  obs.arrows = { hitbox_aabb: box, bow_shot: shot, summoned };
  await run("scoreboard players set #poll dwa.s 1");
  await run(`clear ${BOT}`);

  // ---------------------------------------------------------------- D strikes
  await run("gamerule natural_health_regeneration false", "no_regen");
  await run(`effect give ${BOT} minecraft:instant_health 1 10 true`).catch(() => {});
  await sleep(300);
  const h0 = await health();
  const lands0 = await score("#lands");
  await tp(P.in_strike, 180);
  await until(async () => (await score("#sm")) >= 1, 5000, "the pattern to begin");
  const windAt = await score("#wind_at");
  await until(async () => (await score("#lands")) > lands0, 20000, "the first landing");
  const landAt = await score("#land_at");
  await sleep(300);
  const h1 = await health();
  const timeline = { wind_at: windAt, hold_at: await score("#hold_at"), swing_at: await score("#swing_at"), land_at: landAt };
  // in the trigger box, out of the strike box
  await tp(P.in_trigger_only, 180);
  await run(`effect give ${BOT} minecraft:instant_health 1 10 true`).catch(() => {});
  await sleep(300);
  const h2 = await health();
  const lands1 = await score("#lands");
  await until(async () => (await score("#lands")) > lands1, 20000, "the second landing");
  await sleep(300);
  const h3 = await health();
  // out of the trigger box
  await tp(P.outside, 0);
  await until(async () => (await score("#sm")) === 0, 15000, "the pattern to finish its cycle");
  const lands2 = await score("#lands");
  await sleep(5000);
  obs.strikes = {
    hold_ticks_declared: S.hold_ticks,
    windup_frames: S.clips.windup.frames,
    strike_frames: S.clips.strike.frames,
    frame_ticks: await score("#ft"),
    timeline_gametime: timeline,
    windup_to_hold_ticks: timeline.hold_at - timeline.wind_at,
    hold_to_swing_ticks: timeline.swing_at - timeline.hold_at,
    swing_to_land_ticks: timeline.land_at - timeline.swing_at,
    in_strike_box: { health_before: h0, health_after_landing: h1, damage_taken: h0 - h1, declared: S.strike_damage },
    in_trigger_only: { health_before: h2, health_after_landing: h3, damage_taken: h2 - h3 },
    outside: { lands_when_left: lands2, lands_after_5s: await score("#lands"), state_machine: await score("#sm"), idle_clip: (await score("#clip_0")) === S.clips.idle.index },
  };

  // ---------------------------------------------------------------- E tick cost
  const cost = [];
  cost.push(await tickQuery("1 assembly, keyframe every 5 ticks"));
  await run("function dwa:interp/1");
  cost.push(await tickQuery("1 assembly, keyframe every tick"));
  for (let k = 1; k < 4; k++) await run(`function dwa:spawn/${k}`);
  await run("function dwa:interp/1");
  cost.push(await tickQuery("4 assemblies, keyframe every tick"));
  await run("function dwa:interp/5");
  cost.push(await tickQuery("4 assemblies, keyframe every 5 ticks"));
  for (let k = 4; k < S.n_asm; k++) await run(`function dwa:spawn/${k}`);
  cost.push(await tickQuery("10 assemblies, keyframe every 5 ticks"));
  await run("function dwa:interp/1");
  cost.push(await tickQuery("10 assemblies, keyframe every tick"));
  await run("scoreboard players set #ft dwa.s 100000");
  cost.push(await tickQuery("10 assemblies, animation stopped (control)"));
  await run("function dwa:interp/5");
  obs.tick_cost = cost;

  // ---------------------------------------------------------------- F tracking
  for (let k = 1; k < S.n_asm; k++) await run(`function dwa:despawn/${k}`);
  await sleep(1000);
  const lane = [];
  for (const x of S.lane) {
    await tp([x + 0.5, S.wy, S.az + 0.5], 90);
    await sleep(3000);
    const parts = botEntities("block_display");
    lane.push({
      distance_blocks: x - S.ax,
      block_displays_seen: parts.length,
      hitbox_seen: botEntities("interaction").length,
      root_seen: botEntities("item_display").length,
    });
  }
  obs.tracking = { view_distance_chunks: Number(obs.server_properties.find((l) => l.startsWith("view-distance="))?.split("=")[1]), lane };

  // ---------------------------------------------------------------- G stop
  const tStop = Date.now();
  await run("function dwa:stop", "stop");
  await sleep(1500);
  obs.after_stop = {
    dwa_entities: await count("@e[tag=dwa]"),
    bot_sees_block_displays: botEntities("block_display").length,
    remove_packets: since(tStop).filter((p) => p.name === "remove_entities" || p.name === "entity_destroy").length,
  };
} finally {
  writeFileSync(OUT, JSON.stringify(obs, null, 2) + "\n");
  bot.quit();
}
console.log(JSON.stringify(obs, null, 2));
