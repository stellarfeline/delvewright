// Prove, with two real bodies, what a death and a rest do to a party at a
// bonfire (spec-0016 §1, multiplayer addendum).
//
//   node harness/probe/party-wipe.ts <host> <port> <container> <ns> <bonfire> <wave>
//
// `<container>` is the server's container id (rcon goes through
// `tools/lib/rcon.mjs`, the shared rejection rule). `<bonfire>` is the bonfire
// index whose `bonfire_save_<i>` / `bonfire_open_<i>` the probe drives, and
// `<wave>` is the safe id of a wave the fire re-seats (`spawn_<wave>`).
//
// Two mineflayer clients join with auto-respawn OFF, so a dead body stays on its
// death screen until the probe says otherwise. The re-seat is observed by
// IDENTITY, never by count: every body of the wave is tagged `pw_old`, and a
// re-seat is the old bodies gone and the same number of untagged ones standing.
// A defect that left the wave alone cannot produce fresh bodies, and one that
// re-seated cannot keep the tagged ones.
//
//   1. One death, one respawn, the other player alive: nothing re-seats.
//   2. Both dead (a wipe), then each respawns: the first respawn re-seats once,
//      the second does not re-seat again.
//   3. Both hurt, one rests: the wave re-seats and BOTH are back at full health.
//
// Exit 0 only when every assertion holds; every assertion prints its reading.
import type { Bot } from "mineflayer";
// @ts-expect-error — a plain ES module shared with the shell half, no types.
import { rconChannel } from "../../tools/lib/rcon.mjs";
import { createHarnessBot } from "../src/client-loaded.ts";

const [host, portText, container, ns, bonfireText, wave] = process.argv.slice(2);
if (!host || !portText || !container || !ns || !bonfireText || !wave) {
  process.stderr.write("usage: party-wipe.ts <host> <port> <container> <ns> <bonfire> <wave>\n");
  process.exit(2);
}
const rcon = rconChannel(container) as {
  run(cmd: string): Promise<string>;
  probe(cmd: string): Promise<string>;
};
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const failures: string[] = [];
function check(label: string, ok: boolean, reading: string): void {
  process.stdout.write(`${ok ? "PASS" : "FAIL"} ${label} — ${reading}\n`);
  if (!ok) failures.push(label);
}

function join(username: string): Promise<Bot> {
  return new Promise((resolve, reject) => {
    // `createHarnessBot` reports `player_loaded` after the join and every respawn,
    // as the vanilla client does; without it the server holds each body
    // unhurtable for 60 ticks and `kill` answers `Killed <name>` with nothing dead.
    const { bot } = createHarnessBot({
      host,
      port: Number(portText),
      username,
      version: "1.21.11",
      auth: "offline",
      respawn: false,
    });
    bot.once("spawn", () => resolve(bot));
    bot.once("error", reject);
    bot.once("kicked", (r) => reject(new Error(`${username} kicked: ${JSON.stringify(r)}`)));
  });
}

/** `execute if entity` answers `Test passed, count: N` or `Test failed`. */
async function count(selector: string): Promise<number> {
  const reply = await rcon.probe(`execute if entity ${selector}`);
  const m = /count: (\d+)/i.exec(reply);
  if (m) return Number(m[1]);
  if (/Test failed/.test(reply)) return 0;
  throw new Error(`unreadable count for ${selector}: ${reply}`);
}

/** `scoreboard players get` answers `<holder> has N [obj]`, or that it has none. */
async function score(holder: string, objective: string): Promise<number | undefined> {
  const reply = await rcon.probe(`scoreboard players get ${holder} ${objective}`);
  const m = /has (-?\d+)/.exec(reply);
  return m ? Number(m[1]) : undefined;
}

async function health(name: string): Promise<number> {
  const reply = await rcon.run(`data get entity ${name} Health`);
  const m = /(-?[\d.]+)f$/.exec(reply);
  if (!m) throw new Error(`unreadable health for ${name}: ${reply}`);
  return Number(m[1]);
}

const OLD = `@e[tag=dw_wave_${wave},tag=pw_old]`;
const FRESH = `@e[tag=dw_wave_${wave},tag=!pw_old,tag=!dw_unseen]`;

async function markWave(): Promise<number> {
  await rcon.run(`effect give @e[tag=dw_wave_${wave}] minecraft:fire_resistance infinite 0 true`);
  await rcon.run(`tag @e[tag=dw_wave_${wave}] add pw_old`);
  return count(OLD);
}

async function waveState(): Promise<{ old: number; fresh: number }> {
  return { old: await count(OLD), fresh: await count(FRESH) };
}

async function respawn(bot: Bot): Promise<void> {
  bot.respawn();
  await sleep(2000);
}

/** Kill `name` and confirm the death landed, by its own death counter. */
async function kill(name: string): Promise<void> {
  const before = await score(name, "dw.deaths");
  await rcon.run(`kill ${name}`);
  await sleep(300);
  const after = await score(name, "dw.deaths");
  if (after !== (before ?? 0) + 1) throw new Error(`${name} did not die: dw.deaths ${before} -> ${after}`);
}

const a = await join("pw-a");
const b = await join("pw-b");
await sleep(3000);
const guard = (name: string) =>
  rcon.run(`effect give ${name} minecraft:resistance infinite 255 true`);
await guard("pw-a");
await guard("pw-b");
// The party has rested here once: this fire is the active checkpoint.
await rcon.run(`function ${ns}:bonfire_save_${bonfireText}`);
await rcon.run(`function ${ns}:spawn_${wave}`);
await sleep(1000);
const seated = await markWave();
check("the wave is seated and marked", seated > 0, `${seated} body(ies) tagged pw_old`);

// --- 1. one death in a party that is still fighting ---
await kill("pw-b");
await sleep(2000);
check("one death is not a wipe", (await score("#wipe", "dw.sys")) === 0, `#wipe=${await score("#wipe", "dw.sys")}`);
await respawn(b);
await guard("pw-b");
{
  const w = await waveState();
  check("one death re-seats nothing", w.old === seated && w.fresh === 0, `old=${w.old} fresh=${w.fresh} (seated ${seated})`);
}

// --- 2. a party wipe ---
await kill("pw-a");
await sleep(500);
await kill("pw-b");
await sleep(2000);
check("both dead latches the wipe", (await score("#wipe", "dw.sys")) === 1, `#wipe=${await score("#wipe", "dw.sys")}`);
check("both dead bodies carry dw_wiped", (await count("@a[tag=dw_wiped]")) === 2, `${await count("@a[tag=dw_wiped]")} tagged`);
{
  const w = await waveState();
  check("nothing re-seats while everyone is still dead", w.old === seated && w.fresh === 0, `old=${w.old} fresh=${w.fresh}`);
}
await respawn(a);
await guard("pw-a");
{
  const w = await waveState();
  check("the first respawn after a wipe re-seats the wave", w.old === 0 && w.fresh === seated, `old=${w.old} fresh=${w.fresh} (seated ${seated})`);
  check("the first respawn spends the wipe", (await score("#wipe", "dw.sys")) === 0, `#wipe=${await score("#wipe", "dw.sys")}`);
}
const reseated = await markWave();
await respawn(b);
await guard("pw-b");
{
  const w = await waveState();
  check("the second respawn does not re-seat again", w.old === reseated && w.fresh === 0, `old=${w.old} fresh=${w.fresh} (re-seated ${reseated})`);
  check("no body is left tagged dw_wiped", (await count("@a[tag=dw_wiped]")) === 0, `${await count("@a[tag=dw_wiped]")} tagged`);
}

// --- 3. one rests, both are restored, the wave re-seats ---
for (const n of ["pw-a", "pw-b"]) {
  await rcon.run(`effect clear ${n} minecraft:resistance`);
  await rcon.run(`damage ${n} 12 minecraft:generic`);
}
const hurtA = await health("pw-a");
const hurtB = await health("pw-b");
check("both bodies are hurt before the rest", hurtA < 20 && hurtB < 20, `pw-a=${hurtA} pw-b=${hurtB}`);
// What the bonfire's right-click advancement runs as the clicking player; then
// the player's own answer, through the dialog button's `/trigger` channel.
await rcon.run(`execute as pw-a run function ${ns}:bonfire_open_${bonfireText}`);
a.chat("/trigger dw.rest set 2");
await sleep(1500);
const restA = await health("pw-a");
const restB = await health("pw-b");
check("the player who rested is restored", restA === 20, `pw-a=${restA}`);
check("the player who did not rest is restored too", restB === 20, `pw-b=${restB}`);
{
  const w = await waveState();
  check("the rest re-seats the wave", w.old === 0 && w.fresh === reseated, `old=${w.old} fresh=${w.fresh} (re-seated ${reseated})`);
}

a.quit();
b.quit();
await sleep(500);
process.stdout.write(failures.length === 0 ? "party-wipe: every assertion held\n" : `party-wipe: ${failures.length} failed: ${failures.join("; ")}\n`);
process.exit(failures.length === 0 ? 0 : 1);
