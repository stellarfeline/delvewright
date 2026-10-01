// Measure how an IDLE player body moves in still water on a live server.
//
//   node harness/probe/water-sink.ts <host> <port> <x> <z> <trials>
//
// The bot is placed (by `/tp`, which is the probe's setup and never the stage's
// mechanism) with its feet in the top water block of the column at (x, z), every
// control released, and then left alone. Two observers record the descent:
//
//   * the CLIENT: `bot.entity.position` on every physics tick;
//   * the SERVER: the entity's own `Pos[1]` read back by `/tellraw` (a delve turns
//     command feedback off, so `/data get` answers nothing), the position the server holds and
//     that a volume's selector is adjudicated against.
//
// It prints one line per sample and a summary per trial: the water column read
// from the world, the terminal descent rate each observer saw, and the time from
// release to death (when the column ends in something that kills) or to rest.
import mineflayer from "mineflayer";

const [host, portText, xText, zText, trialsText] = process.argv.slice(2);
if (!host || !portText || !xText || !zText) {
  process.stderr.write("usage: water-sink.ts <host> <port> <x> <z> [trials]\n");
  process.exit(2);
}
const X = Number(xText);
const Z = Number(zText);
const TRIALS = Number(trialsText ?? "3");
const bot = mineflayer.createBot({
  host,
  port: Number(portText),
  username: process.env.DELVEWRIGHT_BOT_USERNAME ?? "delve-bot",
  version: "1.21.11",
  auth: "offline",
});
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
let dead = false;
bot.on("death", () => {
  dead = true;
});
const serverPos: { t: number; y: number }[] = [];
bot.on("messagestr", (msg: string) => {
  const m = /^SINKPOS (-?[\d.]+)d$/.exec(msg);
  if (m) serverPos.push({ t: Date.now(), y: Number(m[1]) });
  else process.stdout.write(`chat: ${msg}\n`);
});

function column(): { top: number; bottom: number; still: boolean } | undefined {
  let top: number | undefined;
  let bottom: number | undefined;
  let still = true;
  for (let y = 120; y > -60; y--) {
    const p = bot.entity.position;
    const b = bot.blockAt(p.offset(X - p.x, y - p.y, Z - p.z));
    if (!b) continue;
    if (b.name === "water") {
      if (top === undefined) top = y;
      bottom = y;
      const level = (b.getProperties() as { level?: number | string }).level;
      if (level !== undefined && Number(level) !== 0) still = false;
    } else if (top !== undefined) break;
  }
  return top === undefined || bottom === undefined ? undefined : { top, bottom, still };
}

function terminal(samples: { t: number; y: number }[], perSecond: boolean): number | undefined {
  // Rate over the last half of the descent, where the body has reached terminal speed.
  if (samples.length < 4) return undefined;
  const a = samples[Math.floor(samples.length / 2)]!;
  const b = samples[samples.length - 1]!;
  const dt = perSecond ? (b.t - a.t) / 1000 : b.t - a.t;
  return dt > 0 ? (a.y - b.y) / dt : undefined;
}

bot.once("spawn", async () => {
  await sleep(4000);
  for (let trial = 1; trial <= TRIALS; trial++) {
    if (dead) {
      bot.respawn();
      dead = false;
      await sleep(3000);
    }
    bot.chat(`/tp @s ${X} 100 ${Z}`);
    await sleep(2500);
    const col = column();
    if (!col) {
      process.stdout.write(`trial ${trial}: no water column at (${X}, ${Z})\n`);
      break;
    }
    bot.chat(`/tp @s ${X} ${col.top} ${Z}`);
    await sleep(150);
    bot.clearControlStates();
    serverPos.length = 0;
    const client: { t: number; y: number; tick: number; inWater: boolean; vy: number }[] = [];
    let tick = 0;
    const onTick = () => {
      tick++;
      const p = bot.entity.position;
      client.push({ t: Date.now(), y: p.y, tick, inWater: bot.entity.isInWater, vy: bot.entity.velocity.y });
    };
    bot.on("physicsTick", onTick);
    const start = Date.now();
    const poll = setInterval(() => bot.chat('/tellraw @s ["SINKPOS ",{"nbt":"Pos[1]","entity":"@s"}]'), 250);
    let restTicks = 0;
    let lastY = bot.entity.position.y;
    while (!dead && Date.now() - start < 60_000) {
      await sleep(50);
      const y = bot.entity.position.y;
      restTicks = Math.abs(y - lastY) < 1e-4 ? restTicks + 1 : 0;
      lastY = y;
      if (restTicks > 40) break;
    }
    clearInterval(poll);
    bot.off("physicsTick", onTick);
    const elapsed = Date.now() - start;
    for (const s of client.filter((_, i) => i % 5 === 0)) {
      process.stdout.write(
        `trial ${trial} client tick ${s.tick} y=${s.y.toFixed(3)} vy=${s.vy.toFixed(4)} inWater=${s.inWater}\n`,
      );
    }
    for (const s of serverPos) {
      process.stdout.write(`trial ${trial} server +${s.t - start}ms y=${s.y.toFixed(3)}\n`);
    }
    const perTick = client.length >= 4
      ? (client[Math.floor(client.length / 2)]!.y - client[client.length - 1]!.y) /
        (client[client.length - 1]!.tick - client[Math.floor(client.length / 2)]!.tick)
      : undefined;
    process.stdout.write(
      `SUMMARY trial ${trial}: column top=${col.top} bottom=${col.bottom} still=${col.still}; ` +
        `start y=${client[0]?.y.toFixed(3)} end y=${client[client.length - 1]?.y.toFixed(3)}; ` +
        `client terminal ${perTick?.toFixed(4)} blocks/tick, ${terminal(client, true)?.toFixed(3)} blocks/s; ` +
        `server terminal ${terminal(serverPos, true)?.toFixed(3)} blocks/s over ${serverPos.length} reads; ` +
        `${dead ? "DIED" : "came to rest"} after ${elapsed}ms (${client.length} ticks)\n`,
    );
  }
  bot.quit();
});
bot.on("kicked", (r) => process.stdout.write(`kicked: ${JSON.stringify(r)}\n`));
bot.on("error", (e) => process.stdout.write(`error: ${e}\n`));
