#!/usr/bin/env node
// STEP 1 — does the harness's shield block, server-side?
//
// Instrument: pinned vanilla 1.21.11 (versions.toml [images.base] mirror_of),
// a mineflayer bot from harness/node_modules driven by exactly the call the
// harness makes (`bot.activateItem(true)` / `bot.deactivateItem()`), and the
// server's OWN state read over a sync-fenced rcon channel. Two arms per blow
// that share no configuration: the statistic
// `minecraft.custom:minecraft.damage_blocked_by_shield` (the player's stat file)
// and the bot's `Health` delta (entity NBT). Only the shield varies.
//
// Every blow asserts its own preconditions — full health, armour on, shield in
// the off hand, the server's Rotation equal to the one asked for — because the
// first version of this rig let the bot die on its fourth blow and then measured
// eighteen more rows on a naked bot with no shield, every one of them a zero.
import { createRequire } from "node:module";
import { writeFileSync } from "node:fs";
import { rig, nbt, num, score, sleep } from "./rig.mjs";

const require = createRequire(new URL("../../harness/package.json", import.meta.url));
const mineflayer = require("mineflayer");

const CONTAINER = process.env.SPIKE_CONTAINER ?? "dw-spike-shield";
const PORT = Number(process.env.SPIKE_PORT ?? 32768);
const OUT = process.env.SPIKE_OUT ?? new URL("./shield-observations.json", import.meta.url).pathname;
const BOT = "dw_shield";
const REPS = Number(process.env.SPIKE_REPS ?? 8);

const r = rig(CONTAINER);
const obs = { instrument: {}, probes: {} };

const OBJ = {
  blocked: "minecraft.custom:minecraft.damage_blocked_by_shield",
  taken: "minecraft.custom:minecraft.damage_taken",
  resisted: "minecraft.custom:minecraft.damage_resisted",
  dealt: "minecraft.custom:minecraft.damage_dealt",
};

async function setup() {
  const created = {};
  for (const [k, crit] of Object.entries(OBJ)) {
    await r.probe(`scoreboard objectives add dw.${k} ${crit}`); // idempotent across re-runs
    created[k] = crit;
  }
  // An objective the server REFUSED reads as a permanent 0 and would make the
  // shield look broken by construction, so presence is verified, not assumed.
  const list = await r.run("scoreboard objectives list");
  obs.instrument.objectives_reply = list;
  for (const k of Object.keys(OBJ)) {
    if (!list.includes(`dw.${k}`)) throw new Error(`objective dw.${k} is not in the server's list: ${list}`);
  }
  obs.instrument.objectives = created;
  // vesperhold's world.json declares difficulty "normal"; Player.hurt scales
  // incoming damage by 1.5 on hard, so the rig runs at the content's difficulty.
  await r.runAll([
    "difficulty normal",
    "gamerule spawn_mobs false",
    "gamerule advance_time false",
    "gamerule natural_health_regeneration false",
    "gamerule immediate_respawn true",
    "gamerule send_command_feedback true",
    "time set midnight",
    "weather clear",
    "setworldspawn 0 -60 0",
    "forceload add -20 -20 20 20",
  ]);
  await r.probe("fill -20 -61 -20 20 -61 20 minecraft:stone");
  for (const [x, z] of [[-20, -20], [0, 0], [20, 20], [2, 0]]) {
    const at = await r.probe(`execute if block ${x} -61 ${z} minecraft:stone run time query gametime`);
    if (!at.startsWith("The time is")) throw new Error(`floor is not stone at ${x},-61,${z}: ${JSON.stringify(at)}`);
  }
}

function connect() {
  return new Promise((resolve, reject) => {
    const bot = mineflayer.createBot({
      host: "127.0.0.1", port: PORT, username: BOT, auth: "offline", version: "1.21.11",
    });
    bot.once("spawn", () => resolve(bot));
    bot.once("error", reject);
    bot.once("kicked", (m) => reject(new Error(`kicked: ${JSON.stringify(m)}`)));
  });
}

/** The Sellsword kit's wearables, verbatim from campaigns/vesperhold/classes.json. */
const KIT = [
  "minecraft:iron_sword", "minecraft:shield", "minecraft:iron_helmet",
  "minecraft:iron_chestplate", "minecraft:chainmail_leggings", "minecraft:iron_boots",
];

async function dress(bot) {
  await r.runAll([
    `gamemode survival ${BOT}`,
    `effect clear ${BOT}`,
    `clear ${BOT}`,
    ...KIT.map((i) => `give ${BOT} ${i}`),
    `tp ${BOT} 0 -60 0`,
  ]);
  await sleep(900);
  // Exactly what the harness's `equipLoadout` does.
  for (const [key, dest] of [["sword", "hand"], ["helmet", "head"], ["chestplate", "torso"], ["leggings", "legs"], ["boots", "feet"]]) {
    const item = bot.inventory.items().find((i) => i.name.includes(key));
    if (item) await bot.equip(item, dest);
  }
  const shield = bot.inventory.items().find((i) => i.name === "shield");
  if (shield) await bot.equip(shield, "off-hand");
  await sleep(400);
}

/** The server's view of the kit — not the client's. */
async function dressed() {
  const out = await r.batch([
    `execute as ${BOT} if items entity @s weapon.offhand minecraft:shield run time query gametime`,
    `execute as ${BOT} if items entity @s armor.chest minecraft:iron_chestplate run time query gametime`,
    `execute as ${BOT} if items entity @s weapon.mainhand minecraft:iron_sword run time query gametime`,
  ]);
  return out.every((l) => l.startsWith("The time is"));
}

async function readState() {
  const out = await r.batch([
    `scoreboard players get ${BOT} dw.blocked`,
    `scoreboard players get ${BOT} dw.taken`,
    `scoreboard players get ${BOT} dw.resisted`,
    `data get entity ${BOT} Health`,
    `data get entity ${BOT} Rotation`,
  ]);
  // Vanilla stores these statistics as round(damage * 10).
  return {
    blocked: score(out[0]) ?? 0,
    taken: score(out[1]) ?? 0,
    resisted: score(out[2]) ?? 0,
    hp: num(out[3]),
    rot: nbt(out[4]),
  };
}

// mineflayer's own convention (`toNotchianYaw(y) = degrees(PI - y)`), inverted.
const MC_TO_MF = (mcYaw) => Math.PI - (mcYaw * Math.PI) / 180;
/** The husk sits due EAST of the bot, so facing it is MC yaw -90. */
const FACING = -90;

async function main() {
  await setup();
  const bot = await connect();
  await dress(bot);

  obs.probes.offhand = {
    equipment: nbt(await r.probe(`data get entity ${BOT} equipment`)),
    items_check: await r.probe(`execute as ${BOT} if items entity @s weapon.offhand minecraft:shield run time query gametime`),
    mineflayer_slot45: bot.inventory.slots[45]?.name ?? null,
  };

  await r.probe("kill @e[type=minecraft:husk]"); // may match nobody on a fresh rig
  await r.run("summon minecraft:husk 2 -60 0 {NoAI:1b,PersistenceRequired:1b,Silent:1b}");
  await sleep(300);
  obs.probes.attacker_uuid = nbt(await r.probe("data get entity @e[type=minecraft:husk,limit=1] UUID"));
  const HUSK = "@e[type=minecraft:husk,limit=1]";

  let aborted = 0;
  /** One blow. `warmTicks` < 0 means the shield is never raised (the control). */
  async function blow({ warmTicks, yawOffset }) {
    const yaw = FACING + yawOffset;
    for (let attempt = 0; attempt < 4; attempt++) {
      await r.runAll([
        `effect clear ${BOT}`,
        `effect give ${BOT} minecraft:instant_health 1 10 true`,
        `tp ${BOT} 0 -60 0 ${yaw} 0`,
      ]);
      bot.look(MC_TO_MF(yaw), 0, true);
      await sleep(450);
      if (!(await dressed())) {
        await dress(bot);
        aborted += 1;
        continue;
      }
      const before = await readState();
      if (before.hp !== 20) { aborted += 1; await sleep(400); continue; }
      const gotYaw = Number.parseFloat(String(before.rot).replace(/[[\]f]/g, "").split(",")[0]);
      if (Math.abs(((gotYaw - yaw + 540) % 360) - 180) > 1.5) {
        throw new Error(`the server's yaw is ${gotYaw}, asked for ${yaw} — the look never landed`);
      }
      if (warmTicks >= 0) {
        bot.activateItem(true);
        await sleep(warmTicks * 50);
      }
      const reply = await r.run(`damage ${BOT} 6.0 minecraft:mob_attack by ${HUSK}`);
      await sleep(300);
      const after = await readState();
      if (warmTicks >= 0) bot.deactivateItem();
      await sleep(150);
      const d = {
        warmTicks, yawOffset, yaw: gotYaw, reply,
        dBlocked: after.blocked - before.blocked,
        dTaken: after.taken - before.taken,
        dResisted: after.resisted - before.resisted,
        dHp: Number(((before.hp ?? 0) - (after.hp ?? 0)).toFixed(3)),
      };
      // A blow that landed on NOTHING is not a reading about the shield.
      if (d.dBlocked === 0 && d.dTaken === 0) { aborted += 1; continue; }
      return d;
    }
    throw new Error(`could not take a clean reading at warm=${warmTicks} yaw=${yawOffset}`);
  }

  const warmRows = [];
  for (const w of [-1, 0, 2, 3, 4, 5, 6, 8, 12, 20]) {
    const rows = [];
    for (let i = 0; i < REPS; i++) rows.push(await blow({ warmTicks: w, yawOffset: 0 }));
    const blockedN = rows.filter((x) => x.dBlocked > 0).length;
    warmRows.push({
      warmTicks: w, n: rows.length, blockedN,
      dBlocked: rows.map((x) => x.dBlocked),
      dTaken: rows.map((x) => x.dTaken),
      dHp: rows.map((x) => x.dHp),
    });
    console.log(`warm ${w} ticks: blocked ${blockedN}/${rows.length}; hp lost ${rows.map((x) => x.dHp).join(",")}; blocked*10 ${rows.map((x) => x.dBlocked).join(",")}`);
  }
  obs.probes.warmup = warmRows;

  const arcRows = [];
  for (const a of [0, 45, 80, 85, 88, 90, 92, 95, 110, 180]) {
    const rows = [];
    for (let i = 0; i < Math.max(4, Math.floor(REPS / 2)); i++) rows.push(await blow({ warmTicks: 20, yawOffset: a }));
    const blockedN = rows.filter((x) => x.dBlocked > 0).length;
    arcRows.push({ yawOffset: a, n: rows.length, blockedN, dHp: rows.map((x) => x.dHp), yaw: rows[0].yaw });
    console.log(`yaw +${a}deg: blocked ${blockedN}/${rows.length}; hp lost ${rows.map((x) => x.dHp).join(",")}`);
  }
  obs.probes.arc = arcRows;
  obs.instrument.aborted_readings = aborted;

  writeFileSync(OUT, JSON.stringify(obs, null, 2));
  console.log(`aborted readings: ${aborted}\n-> ${OUT}`);
  bot.quit();
    process.exit(0);
}

main().catch((e) => { console.error(e); process.exit(1); });
