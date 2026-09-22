// The Sellsword kit as the SERVER sees it: the numbers the fight arithmetic uses.
import { createRequire } from "node:module";
import { writeFileSync } from "node:fs";
import { rig, nbt, sleep } from "./rig.mjs";
const require = createRequire(new URL("../../harness/package.json", import.meta.url));
const mineflayer = require("mineflayer");
const r = rig(process.env.SPIKE_CONTAINER ?? "dw-guard-rig");
const BOT = "kit_bot";
const bot = await new Promise((res, rej) => {
  const b = mineflayer.createBot({ host: "127.0.0.1", port: Number(process.env.SPIKE_PORT ?? 32769), username: BOT, auth: "offline", version: "1.21.11" });
  b.once("spawn", () => res(b)); b.once("error", rej);
});
await sleep(1500);
await r.runAll([`op ${BOT}`, `clear ${BOT}`, `execute as ${BOT} run function vesperhold:class_apply_sellsword`]);
await sleep(1200);
for (const [key, dest] of [["sword", "hand"], ["helmet", "head"], ["chestplate", "torso"], ["leggings", "legs"], ["boots", "feet"]]) {
  const i = bot.inventory.items().find((x) => x.name.includes(key));
  if (i) await bot.equip(i, dest);
}
const sh = bot.inventory.items().find((x) => x.name === "shield");
if (sh) await bot.equip(sh, "off-hand");
await sleep(600);
const out = {};
for (const a of ["attack_speed", "attack_damage", "armor", "armor_toughness", "max_health"]) {
  out[a] = await r.run(`attribute ${BOT} minecraft:${a} get`);
}
out.equipment = nbt(await r.probe(`data get entity ${BOT} equipment`));
out.inventory = nbt(await r.probe(`data get entity ${BOT} Inventory`));
console.log(JSON.stringify(out, null, 1));
writeFileSync(process.env.SPIKE_OUT ?? new URL("./kit-observations.json", import.meta.url).pathname, JSON.stringify(out, null, 2));
bot.quit(); process.exit(0);
