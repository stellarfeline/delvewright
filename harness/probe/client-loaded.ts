// Measure, on a live pinned server, how long a player cannot be hurt after it
// joins and after it respawns — for a client that never reports `player_loaded`
// and for one that reports it the way the vanilla client does.
//
//   node harness/probe/client-loaded.ts <host> <port> <container> [respawns]
//
// `<container>` is the server's container id (rcon goes through
// `tools/lib/rcon.mjs`). Two bots join one after the other:
//
//   * `cl-silent` — a bare mineflayer bot, which never sends the packet. This
//     file is the one place in the harness allowed to make one
//     (`test/client-loaded.test.ts`), because measuring it is its purpose.
//   * `cl-loaded` — made by `createHarnessBot`, the harness's only bot factory.
//
// For each, after the join and after each of `[respawns]` (default 2) respawns,
// the probe asks the server `/damage <name> 1` until it is applied. Each reply is
// either `Applied 1.0 damage to …` or the refusal `Target is invulnerable to the
// given damage type`; the window is the server's game time at the first applied
// hit minus the game time read when the window opened (the `login`/`respawn`
// packet's arrival), so it is an UPPER bound by one rcon round trip. Deaths are
// driven by `/kill`, re-sent until the body's `death` event arrives, which
// measures the same window from the other side: `kill` answers `Killed <name>`
// even when nothing died.
import mineflayer, { type Bot } from "mineflayer";
// @ts-expect-error — a plain ES module shared with the shell half, no types.
import { rconChannel } from "../../tools/lib/rcon.mjs";
import { createHarnessBot } from "../src/client-loaded.ts";

const [host, portText, container, respawnsText] = process.argv.slice(2);
if (!host || !portText || !container) {
  process.stderr.write("usage: client-loaded.ts <host> <port> <container> [respawns]\n");
  process.exit(2);
}
const RESPAWNS = Number(respawnsText ?? "2");
const rcon = rconChannel(container) as {
  run(cmd: string): Promise<string>;
  probe(cmd: string): Promise<string>;
};
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

async function gametime(): Promise<number> {
  const reply = await rcon.run("time query gametime");
  const m = /(\d+)\s*$/.exec(reply.trim());
  if (!m) throw new Error(`unreadable game time: ${reply}`);
  return Number(m[1]);
}

/** Poll `/damage` until it lands; the ticks and ms from `openedTick`/`openedMs`. */
async function hurtableAfter(
  name: string,
  openedTick: number,
  openedMs: number,
): Promise<{ ticks: number; ms: number; refusals: number }> {
  let refusals = 0;
  const deadline = Date.now() + 40_000;
  while (Date.now() < deadline) {
    const reply = (await rcon.probe(`damage ${name} 1 minecraft:generic`)).trim();
    if (/^Applied /.test(reply)) {
      const t = await gametime();
      await rcon.run(`effect give ${name} minecraft:instant_health 1 5 true`);
      return { ticks: t - openedTick, ms: Date.now() - openedMs, refusals };
    }
    if (!/invulnerable/i.test(reply)) throw new Error(`unexpected reply to /damage: ${reply}`);
    refusals += 1;
  }
  throw new Error(`${name} was never hurtable within 40 s (${refusals} refusals)`);
}

async function join(name: string, loaded: boolean): Promise<{ bot: Bot; tick: number; ms: number }> {
  const options = { host: host!, port: Number(portText), username: name, version: "1.21.11", auth: "offline" as const, respawn: false };
  const bot = loaded ? createHarnessBot(options).bot : mineflayer.createBot(options);
  const opened = new Promise<{ tick: number; ms: number }>((resolve) => {
    bot._client.once("login", () => {
      const ms = Date.now();
      void gametime().then((tick) => resolve({ tick, ms }));
    });
  });
  await new Promise<void>((resolve, reject) => {
    bot.once("spawn", () => resolve());
    bot.once("error", reject);
    bot.once("kicked", (r) => reject(new Error(`${name} kicked: ${JSON.stringify(r)}`)));
  });
  return { bot, ...(await opened) };
}

async function dieAndRespawn(bot: Bot): Promise<{ tick: number; ms: number; killsSent: number }> {
  const name = bot.username;
  let dead = false;
  bot.once("death", () => {
    dead = true;
  });
  let killsSent = 0;
  while (!dead && killsSent < 400) {
    await rcon.run(`kill ${name}`);
    killsSent += 1;
    await sleep(100);
  }
  if (!dead) throw new Error(`${name} never died under ${killsSent} kills`);
  await sleep(1000);
  const opened = new Promise<{ tick: number; ms: number }>((resolve) => {
    bot._client.once("respawn", () => {
      const ms = Date.now();
      void gametime().then((tick) => resolve({ tick, ms }));
    });
  });
  bot.respawn();
  return { ...(await opened), killsSent };
}

const rows: string[] = [];
for (const [name, loaded] of [
  ["cl-silent", false],
  ["cl-loaded", true],
] as const) {
  const j = await join(name, loaded);
  const first = await hurtableAfter(name, j.tick, j.ms);
  rows.push(`${name} join: hurtable after ${first.ticks} tick(s) / ${first.ms} ms (${first.refusals} refusal(s))`);
  for (let i = 1; i <= RESPAWNS; i++) {
    const r = await dieAndRespawn(j.bot);
    const h = await hurtableAfter(name, r.tick, r.ms);
    rows.push(
      `${name} respawn ${i}: hurtable after ${h.ticks} tick(s) / ${h.ms} ms (${h.refusals} refusal(s)); ` +
        `the death before it took ${r.killsSent} /kill(s)`,
    );
  }
  j.bot.quit();
  await sleep(1500);
}
for (const row of rows) process.stdout.write(`${row}\n`);
process.exit(0);
