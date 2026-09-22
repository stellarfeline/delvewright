#!/usr/bin/env node
// STEP 2, the defensive CEILING — one body in the Guard's hall that never swings,
// holds the shield up for the whole fight and always faces the nearest Guard.
//
// Nothing a real fencer does can beat this defensively: swinging costs the
// shield's 250 ms warm-up (measured in `shield.mjs`) every time, and a body that
// blocks perfectly deals nothing. So the survival time this probe measures is an
// upper bound on how long ANY single body lives in this fight, and it is read
// from the server's own statistics, sharing no configuration with the arithmetic
// it cross-checks.
import { createRequire } from "node:module";
import { writeFileSync } from "node:fs";
import { rig, num, score, sleep } from "./rig.mjs";

const require = createRequire(new URL("../../harness/package.json", import.meta.url));
const mineflayer = require("mineflayer");

const CONTAINER = process.env.SPIKE_CONTAINER ?? "dw-guard-rig";
const PORT = Number(process.env.SPIKE_PORT ?? 32769);
const OUT = process.env.SPIKE_OUT ?? new URL("./hold-observations.json", import.meta.url).pathname;
const BOT = "hold_bot";
const REPS = Number(process.env.SPIKE_REPS ?? 4);
const LIMIT_MS = Number(process.env.SPIKE_LIMIT_MS ?? 30_000);
const ENTRY = [83.5, 80, 86.5];

const r = rig(CONTAINER);

async function stats() {
  const out = await r.batch([
    `scoreboard players get ${BOT} dw.taken`,
    `scoreboard players get ${BOT} dw.blocked`,
    `scoreboard players get ${BOT} dw.resisted`,
    `data get entity ${BOT} Health`,
  ]);
  return {
    taken: score(out[0]) ?? 0,
    blocked: score(out[1]) ?? 0,
    resisted: score(out[2]) ?? 0,
    hp: num(out[3]) ?? -1,
  };
}

function connect() {
  return new Promise((resolve, reject) => {
    const bot = mineflayer.createBot({
      host: "127.0.0.1", port: PORT, username: BOT, auth: "offline", version: "1.21.11",
    });
    bot.once("spawn", () => resolve(bot));
    bot.once("error", reject);
  });
}

async function main() {
  const bot = await connect();
  await sleep(1_500);
  await r.runAll([`op ${BOT}`, `gamemode adventure ${BOT}`]);
  const rows = [];
  for (let rep = 0; rep < REPS; rep++) {
    await r.probe(`kill @e[tag=dw_wave_unremembered_guard]`);
    await r.runAll([
      `clear ${BOT}`,
      `effect clear ${BOT}`,
      `execute as ${BOT} run function vesperhold:class_apply_sellsword`,
      `tp ${BOT} ${ENTRY[0]} ${ENTRY[1]} ${ENTRY[2]} 180 0`,
      `effect give ${BOT} minecraft:instant_health 1 10 true`,
    ]);
    await sleep(1_200);
    // The harness's own `equipLoadout`, so the armour and the off-hand shield are
    // where a fighting body carries them.
    for (const [key, dest] of [["sword", "hand"], ["helmet", "head"], ["chestplate", "torso"], ["leggings", "legs"], ["boots", "feet"]]) {
      const item = bot.inventory.items().find((i) => i.name.includes(key));
      if (item) await bot.equip(item, dest);
    }
    const shield = bot.inventory.items().find((i) => i.name === "shield");
    if (shield) await bot.equip(shield, "off-hand");
    await sleep(600);
    const dressed = await r.probe(
      `execute as ${BOT} if items entity @s weapon.offhand minecraft:shield run time query gametime`,
    );
    if (!dressed.startsWith("The time is")) throw new Error(`the shield is not in the off hand: ${dressed}`);
    await r.run("function vesperhold:spawn_unremembered_guard");
    // spec-0023's assist, the exact command `assistCommand()` emits.
    await r.run(`effect give ${BOT} minecraft:resistance 60 2 true`);
    await sleep(800);

    const before = await stats();
    const t0 = Date.now();
    bot.activateItem(true);
    let raisedAt = Date.now();
    let dead = false;
    const onDeath = () => { dead = true; };
    bot.once("death", onDeath);
    let guardMs = 0;
    while (!dead && Date.now() - t0 < LIMIT_MS) {
      // Always face the nearest Guard: the 180-degree blocking arc, measured in
      // `shield.mjs`, points where the most blows come from.
      const near = Object.values(bot.entities)
        .filter((e) => e && e.name === "zombie" && e.position)
        .sort((a, b) => bot.entity.position.distanceTo(a.position) - bot.entity.position.distanceTo(b.position))[0];
      if (near) bot.lookAt(near.position.offset(0, 1.4, 0), true);
      // mineflayer's look packets do not drop the item use, but a re-raise costs
      // nothing if it is already up and covers a use the server dropped.
      if (Date.now() - raisedAt > 2_000) { bot.activateItem(true); raisedAt = Date.now(); }
      guardMs += 100;
      await sleep(100);
    }
    bot.removeListener("death", onDeath);
    const seconds = (Date.now() - t0) / 1000;
    const after = await stats();
    const row = {
      rep,
      died: dead,
      seconds: Number(seconds.toFixed(2)),
      damage_taken: (after.taken - before.taken) / 10,
      damage_blocked: (after.blocked - before.blocked) / 10,
      damage_resisted: (after.resisted - before.resisted) / 10,
      hp_left: after.hp,
      shield_uptime_ms: guardMs,
    };
    rows.push(row);
    console.log(`[hold] ${JSON.stringify(row)}`);
    bot.deactivateItem();
    await r.probe(`kill @e[tag=dw_wave_unremembered_guard]`);
    await sleep(2_500);
  }
  writeFileSync(OUT, JSON.stringify({ reps: REPS, limitMs: LIMIT_MS, rows }, null, 2));
  console.log(`-> ${OUT}`);
  bot.quit();
    process.exit(0);
}

main().catch((e) => { console.error(e); process.exit(1); });
