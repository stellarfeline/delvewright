// Measure what the pinned server offers a respawn wait (spec-0077 §2).
//
//   node harness/probe/respawn-wait.ts <host> <port> <container>
//
// Two `createHarnessBot` clients, auto-respawn off, `player_loaded` sent after
// the join and every respawn (without it the server holds a player invulnerable
// and nothing dies).
// Each reading is printed with the command that produced it; nothing is judged,
// because the subject is what vanilla does, not what the engine emits.
//
//   M1  the server respawns a player the moment its client asks;
//   M2  `immediate_respawn` is an instruction to the client — a client that does
//       not act on it stays dead;
//   M3  `minecraft.custom:minecraft.time_since_death` is 0 on the death screen and
//       counts from the respawn, spectator included;
//   M4  `spectate` binds a spectator's body to its target, and sneaking releases it;
//   M5  a waiting spectator who disconnects: the stat does not count offline, and
//       on a delve server (`force-gamemode=true`) the player rejoins in adventure.
import type { Bot } from "mineflayer";
// @ts-expect-error — a plain ES module shared with the shell half, no types.
import { rconChannel } from "../../tools/lib/rcon.mjs";
import { createHarnessBot } from "../src/client-loaded.ts";

const [host, portText, container] = process.argv.slice(2);
if (!host || !portText || !container) {
  process.stderr.write("usage: respawn-wait.ts <host> <port> <container>\n");
  process.exit(2);
}
const rcon = rconChannel(container) as { probe(cmd: string): Promise<string> };
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
// `createHarnessBot` reports `player_loaded` after the join and every respawn, as
// the vanilla client does.
const join = (username: string) =>
  new Promise<Bot>((resolve) => {
    const { bot } = createHarnessBot({
      host,
      port: Number(portText),
      username,
      version: "1.21.11",
      auth: "offline",
      respawn: false,
    });
    bot.once("spawn", () => resolve(bot));
  });
const q = async (cmd: string) => {
  const reply = await rcon.probe(cmd);
  process.stdout.write(`> ${cmd}\n  ${reply}\n`);
  return reply;
};

const a = await join("rd-a");
let b = await join("rd-b");
await sleep(2000);
await q("scoreboard objectives add rd.since minecraft.custom:minecraft.time_since_death");

process.stdout.write("== M1\n");
await q("kill rd-b");
await sleep(100);
await q("data get entity rd-b Health");
b.respawn();
await sleep(150);
await q("data get entity rd-b Health");

process.stdout.write("== M2\n");
await q("gamerule immediate_respawn true");
await q("kill rd-b");
await sleep(3000);
await q("data get entity rd-b Health");
b.respawn();
await sleep(500);
await q("gamerule immediate_respawn false");

process.stdout.write("== M3\n");
await q("kill rd-b");
await sleep(100);
await q("scoreboard players get rd-b rd.since");
await sleep(2000);
await q("scoreboard players get rd-b rd.since");
b.respawn();
await sleep(200);
await q("gamemode spectator rd-b");
await q("scoreboard players get rd-b rd.since");
await sleep(2000);
await q("scoreboard players get rd-b rd.since");

process.stdout.write("== M4\n");
await q("spectate rd-a rd-b");
await sleep(300);
await q("tp rd-a 15 67 2");
await sleep(1000);
await q("data get entity rd-b Pos");
b.setControlState("sneak", true);
await sleep(500);
b.setControlState("sneak", false);
await sleep(300);
await q("tp rd-a 15 67 9");
await sleep(1000);
await q("data get entity rd-a Pos");
await q("data get entity rd-b Pos");

process.stdout.write("== M5\n");
await q("scoreboard players get rd-b rd.since");
b.quit();
await sleep(3000);
await q("scoreboard players get rd-b rd.since");
b = await join("rd-b");
await sleep(1000);
await q("data get entity rd-b playerGameType");
await q("scoreboard players get rd-b rd.since");

a.quit();
b.quit();
await sleep(300);
process.exit(0);
