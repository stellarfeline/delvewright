// Prove, with two real bodies, what a declared respawn wait does to a party
// (spec-0077 §6).
//
//   node harness/probe/respawn-wait-party.ts <host> <port> <container> <ns> \
//     <bonfire> <wave> <trigger> <x,y,z> <seconds> [alone]
//
// `<container>` is the server's container id (rcon goes through
// `tools/lib/rcon.mjs`, the shared rejection rule). `<bonfire>` is the bonfire
// index whose `bonfire_save_<i>` makes it the active checkpoint, `<wave>` the safe
// id of a wave that fire re-seats (`spawn_<wave>`), `<trigger>` the safe id of an
// `approach` trigger (`#trig_<trigger>` is its once-latch) and `<x,y,z>` the cell
// it watches. `<seconds>` is the campaign's `world.respawn_wait.seconds`.
//
// Two mineflayer clients join with auto-respawn OFF, so a dead body stays on its
// death screen until the probe clicks *Respawn* for it. A re-seat is observed by
// IDENTITY, never by count: every body of the wave is tagged `pw_old`, and a
// re-seat is the old bodies gone and the same number of untagged ones standing.
//
//   1. One of two dies and respawns: a spectator, tagged, clocked, watching the
//      other; `#wipe` stays 0 and nothing re-seats.
//   2. The waiting body, holding sneak, is put inside the trigger's range: the
//      trigger does not fire.
//   3. It relogs mid-wait: it comes back waiting, and its clock did not run while
//      it was away.
//   4. After `seconds`: adventure, on the checkpoint cell, untagged, flask full.
//   8. The bonfire during a wait (spec-0078's tooltips meet the wait): a player
//      in play clicks the fire and is shown its dialog with both tooltips; the
//      waiting player, beside the same fire, clicks it and is shown nothing,
//      and its own `/trigger dw.rest` rests nothing; the player in play rests
//      during the wait: the wave re-seats once, the waiter keeps waiting, and
//      the release seats it on the fire.
//   5. One waits and the other dies: both are released, the wave re-seats once,
//      and the second one back neither waits nor re-seats it again.
//   6. Alone (`alone: false`), a death does not wait.
//   7. Control: a player in play at the same trigger cell fires it.
//
// With the literal `alone` as the last argument it proves a build declaring
// `alone: true` instead, with ONE body: a death waits, the watcher is held on the
// checkpoint cell (nobody to watch), and the wait ends after `seconds`.
//
// Exit 0 only when every assertion holds; every assertion prints its reading.
import type { Bot } from "mineflayer";
// @ts-expect-error — a plain ES module shared with the shell half, no types.
import { rconChannel } from "../../tools/lib/rcon.mjs";
import { createHarnessBot } from "../src/client-loaded.ts";

const [host, portText, container, ns, bonfireText, wave, trigger, cellText, secondsText, aloneArg] =
  process.argv.slice(2);
if (!host || !portText || !container || !ns || !bonfireText || !wave || !trigger || !cellText || !secondsText) {
  process.stderr.write(
    "usage: respawn-wait-party.ts <host> <port> <container> <ns> <bonfire> <wave> <trigger> <x,y,z> <seconds>\n",
  );
  process.exit(2);
}
const cell = cellText.split(",").map(Number);
const seconds = Number(secondsText);
const rcon = rconChannel(container) as {
  run(cmd: string): Promise<string>;
  probe(cmd: string): Promise<string>;
};
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const failures: string[] = [];
let checks = 0;
function check(label: string, ok: boolean, reading: string): void {
  checks += 1;
  process.stdout.write(`${ok ? "PASS" : "FAIL"} ${label} — ${reading}\n`);
  if (!ok) failures.push(label);
}

type RegistryData = { id: string; entries: { key: string; value?: unknown }[] };
const dialogRegistry = new Map<string, RegistryData["entries"]>();

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
    // The dialog registry the server syncs at configuration: a `show_dialog`
    // names a datapack dialog by its index here, never inline.
    (bot as unknown as { _client: { on(n: string, l: (p: RegistryData) => void): void } })._client.on(
      "registry_data",
      (packet) => {
        if (packet.id === "minecraft:dialog") dialogRegistry.set(username, packet.entries);
      },
    );
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

/** The server's own `playerGameType`: 2 adventure, 3 spectator. */
async function mode(name: string): Promise<number> {
  const reply = await rcon.run(`data get entity ${name} playerGameType`);
  const m = /(-?\d+)$/.exec(reply.trim());
  if (!m) throw new Error(`unreadable game mode for ${name}: ${reply}`);
  return Number(m[1]);
}

/** The server's own position of `name`. */
async function pos(name: string): Promise<[number, number, number]> {
  const reply = await rcon.run(`data get entity ${name} Pos`);
  const m = /\[(-?[\d.E-]+)d, (-?[\d.E-]+)d, (-?[\d.E-]+)d\]/.exec(reply);
  if (!m) throw new Error(`unreadable position for ${name}: ${reply}`);
  return [Number(m[1]), Number(m[2]), Number(m[3])];
}

const dist = (a: number[], b: number[]) => Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);
const fmt = (p: number[]) => p.map((v) => v.toFixed(2)).join(" ");
const tagged = (name: string) => count(`@a[name=${name},tag=dw_cutscene]`);
const clock = (name: string) => score(name, "dw.rwait");

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
  await sleep(1500);
}
/** Kill `name` and confirm the death landed, by its own death counter. */
async function kill(name: string): Promise<void> {
  const before = await score(name, "dw.deaths");
  await rcon.run(`kill ${name}`);
  await sleep(300);
  const after = await score(name, "dw.deaths");
  if (after !== (before ?? 0) + 1) throw new Error(`${name} did not die: dw.deaths ${before} -> ${after}`);
}
const guard = (name: string) => rcon.run(`effect give ${name} minecraft:resistance infinite 255 true`);
const FLASK = `minecraft:potion[potion_contents={potion:"minecraft:healing"}]`;
async function flasks(name: string): Promise<number> {
  const reply = await rcon.probe(`execute if items entity ${name} container.* ${FLASK}`);
  const m = /count: (\d+)/i.exec(reply);
  return m ? Number(m[1]) : 0;
}
const latch = () => score(`#trig_${trigger}`, "dw.sys");

if (aloneArg === "alone") {
  const solo = await join("rw-solo");
  await sleep(3000);
  await rcon.run(`function ${ns}:bonfire_save_${bonfireText}`);
  const reply = await rcon.run("data get storage dw:cp pos");
  const mm = /\[(-?\d+), (-?\d+), (-?\d+)\]/.exec(reply);
  if (!mm) throw new Error(`unreadable checkpoint mirror: ${reply}`);
  const seat = [Number(mm[1]) + 0.5, Number(mm[2]), Number(mm[3]) + 0.5];
  await kill("rw-solo");
  await respawn(solo);
  check("alone: true — a death alone waits", (await mode("rw-solo")) === 3 && ((await clock("rw-solo")) ?? 0) > 0, `playerGameType=${await mode("rw-solo")} dw.rwait=${await clock("rw-solo")}`);
  await rcon.run(`tp rw-solo ${cell[0] + 0.5} ${cell[1]} ${cell[2] + 0.5}`);
  await sleep(1000);
  const ps = await pos("rw-solo");
  check("alone: true — with nobody to watch, the watcher is held at the checkpoint", dist(ps, seat) < 0.5, `rw-solo at ${fmt(ps)}, cell ${fmt(seat)}`);
  check("alone: true — the waiting body does not fire the trigger", (await latch()) !== 1, `#trig_${trigger}=${await latch()}`);
  const left = Math.max(0, seconds * 20 - ((await clock("rw-solo")) ?? 0));
  await sleep((left / 20) * 1000 + 1500);
  check("alone: true — the wait ends after `seconds`", (await mode("rw-solo")) === 2 && (await clock("rw-solo")) === undefined, `playerGameType=${await mode("rw-solo")} dw.rwait=${await clock("rw-solo")}`);
  solo.quit();
  await sleep(500);
  process.stdout.write(
    failures.length === 0
      ? `respawn-wait-party (alone): every assertion held (${checks} of ${checks})\n`
      : `respawn-wait-party (alone): ${failures.length} of ${checks} failed: ${failures.join("; ")}\n`,
  );
  process.exit(failures.length === 0 ? 0 : 1);
}

let a = await join("rw-a");
let b = await join("rw-b");
await sleep(3000);
await guard("rw-a");
await guard("rw-b");
// rw-b carries a class whose kit holds a flask, so the release's refill is visible.
await rcon.run("tag rw-b add dw_class_warder");
await rcon.run(`function ${ns}:bonfire_save_${bonfireText}`);
// The checkpoint mirror the save just wrote; the seat is the cell's centre.
const seatCell = await (async () => {
  const reply = await rcon.run("data get storage dw:cp pos");
  const m = /\[(-?\d+), (-?\d+), (-?\d+)\]/.exec(reply);
  if (!m) throw new Error(`unreadable checkpoint mirror: ${reply}`);
  return [Number(m[1]) + 0.5, Number(m[2]), Number(m[3]) + 0.5];
})();
await rcon.run(`function ${ns}:spawn_${wave}`);
await sleep(1000);
let seated = await markWave();
check("the wave is seated and marked", seated > 0, `${seated} body(ies) tagged pw_old`);
check("the trigger has not fired before the probe", (await latch()) !== 1, `#trig_${trigger}=${await latch()}`);

// --- 1. one of two dies and respawns ---
await rcon.run(`clear rw-b ${FLASK}`);
const bars: string[] = [];
// Read off the raw `action_bar` packet: mineflayer 4.37.1 receives the 1.21.11
// NBT-form component and emits no `actionBar` event for it.
(b as unknown as { _client: { on(n: string, l: (p: unknown) => void): void } })._client.on(
  "action_bar",
  (packet) => bars.push(JSON.stringify(packet)),
);
await kill("rw-b");
await respawn(b);
{
  const countdown = bars.find((j) => j.includes("delvewright.ui.respawn.wait"));
  check("the waiting player sees the countdown", countdown !== undefined, countdown ?? `${bars.length} action bar(s), none the countdown`);
  const m = await mode("rw-b");
  check("the fallen player waits as a spectator", m === 3, `playerGameType=${m}`);
  check("the waiting player carries the observation tag", (await tagged("rw-b")) === 1, `${await tagged("rw-b")} tagged`);
  const c = await clock("rw-b");
  check("the waiting player's clock runs", c !== undefined && c > 1, `dw.rwait=${c}`);
  check("one waiting is not a wipe", (await score("#wipe", "dw.sys")) === 0, `#wipe=${await score("#wipe", "dw.sys")}`);
  const w = await waveState();
  check("a wait re-seats nothing", w.old === seated && w.fresh === 0, `old=${w.old} fresh=${w.fresh} (seated ${seated})`);
  const pa = await pos("rw-a");
  const pb = await pos("rw-b");
  check("the waiting player watches the teammate in play", dist(pa, pb) < 0.5, `rw-a at ${fmt(pa)}, rw-b at ${fmt(pb)}`);
}

// --- 2. the waiting body, freed by sneak, stands in the trigger ---
b.setControlState("sneak", true);
await sleep(300);
await rcon.run(`tp rw-b ${cell[0] + 0.5} ${cell[1]} ${cell[2] + 0.5}`);
await sleep(1500);
{
  const pb = await pos("rw-b");
  const held = dist(pb, [cell[0] + 0.5, cell[1], cell[2] + 0.5]);
  check("the waiting body stood inside the trigger's range", held < 3, `rw-b at ${fmt(pb)}, ${held.toFixed(2)} from the cell`);
  check("a waiting body does not fire the trigger", (await latch()) !== 1, `#trig_${trigger}=${await latch()}`);
}
b.setControlState("sneak", false);
await sleep(1000);
{
  const pa = await pos("rw-a");
  const pb = await pos("rw-b");
  check("letting go of sneak binds the view again", dist(pa, pb) < 0.5, `rw-a at ${fmt(pa)}, rw-b at ${fmt(pb)}`);
}

// --- 3. it relogs mid-wait ---
const before = (await clock("rw-b")) ?? -1;
b.quit();
await sleep(3000);
b = await join("rw-b");
await sleep(500);
{
  const after = (await clock("rw-b")) ?? -1;
  check("the clock does not run while its player is away", before > 0 && after >= before && after - before < 40, `dw.rwait ${before} at quit, ${after} after 3 s away`);
  const m = await mode("rw-b");
  check("a waiting player who relogs comes back waiting", m === 3 && (await tagged("rw-b")) === 1, `playerGameType=${m}, tagged=${await tagged("rw-b")}`);
}

// --- 4. the wait ends ---
{
  const left = Math.max(0, seconds * 20 - ((await clock("rw-b")) ?? 0));
  await sleep((left / 20) * 1000 + 1500);
  const m = await mode("rw-b");
  check("after the wait the player is back in adventure", m === 2, `playerGameType=${m}`);
  check("after the wait the player is untagged and unclocked", (await tagged("rw-b")) === 0 && (await clock("rw-b")) === undefined, `tagged=${await tagged("rw-b")} dw.rwait=${await clock("rw-b")}`);
  const pb = await pos("rw-b");
  check("after the wait the player stands on the checkpoint cell", dist(pb, seatCell) < 0.5, `rw-b at ${fmt(pb)}, cell ${fmt(seatCell)}`);
  const f = await flasks("rw-b");
  check("after the wait the flask is refilled", f > 0, `${f} flask(s)`);
  const w = await waveState();
  check("the release of a wait re-seats nothing", w.old === seated && w.fresh === 0, `old=${w.old} fresh=${w.fresh}`);
}
await guard("rw-b");

// --- 8. the bonfire during a wait (spec-0077 meets spec-0078) ---
//
// The fire's hitbox is armed where the campaign's own `bonfire` effect arms it:
// on the checkpoint cell `bonfire_save` made active, tagged `dw_bonfire_<i>`,
// which is what the `bf_<i>` advancement matches. Every dialog either client is
// shown is read off the raw `show_dialog` packet.
{
  const [sx, sy, sz] = seatCell as [number, number, number];
  await rcon.run(
    `execute unless entity @e[tag=dw_bonfire_${bonfireText}] run summon minecraft:interaction ${sx} ${sy} ${sz} {width:1.0f,height:2.0f,response:1b,Tags:["dw_bonfire_${bonfireText}"]}`,
  );
  const dialogsA: string[] = [];
  const dialogsB: string[] = [];
  const onDialog = (bot: Bot, into: string[]) =>
    (bot as unknown as { _client: { on(n: string, l: (p: unknown) => void): void } })._client.on(
      "show_dialog",
      (packet) => into.push(JSON.stringify(packet)),
    );
  onDialog(a, dialogsA);
  onDialog(b, dialogsB);
  const fireOf = (bot: Bot) =>
    Object.values(bot.entities).find(
      (e) => e.name === "interaction" && Math.hypot(e.position.x - sx, e.position.y - sy, e.position.z - sz) < 0.5,
    );

  // Control: a player in play clicks the fire and is shown its dialog, both
  // buttons carrying their tooltips.
  await rcon.run(`tp rw-a ${sx} ${sy} ${sz + 1.5} facing ${sx} ${sy + 1} ${sz}`);
  await sleep(1500);
  const fireA = fireOf(a);
  check("control: the player in play sees the fire's hitbox", fireA !== undefined, `${Object.values(a.entities).filter((e) => e.name === "interaction").length} interaction(s) known to rw-a`);
  if (fireA) await a.activateEntity(fireA);
  await sleep(1000);
  {
    // `show_dialog` carries `{dialog: <registry index>}`; the entry it names is
    // read from the registry the server synced to rw-a at configuration.
    const shown = dialogsA.map((j) => (JSON.parse(j) as { dialog?: { dialog?: unknown } }).dialog?.dialog);
    const entries = dialogRegistry.get("rw-a") ?? [];
    const named = shown.map((i) => (typeof i === "number" ? entries[i] : undefined));
    const fire = named.find((e) => e?.key === `${ns}:bonfire_${bonfireText}`);
    const body = fire ? JSON.stringify(fire.value) : "";
    check(
      "control: a click by a player in play opens the fire's dialog, tooltips on both buttons",
      fire !== undefined && body.includes("rest_tooltip") && body.includes("save_tooltip"),
      `${dialogsA.length} dialog(s) shown, naming ${named.map((e) => e?.key ?? "?").join(", ")} of ${entries.length} synced; rest_tooltip ${body.includes("rest_tooltip")}, save_tooltip ${body.includes("save_tooltip")}`,
    );
    const at = await score("rw-a", "dw.rest_at");
    check("control: the opener ran for the player in play", at === Number(bonfireText), `dw.rest_at=${at}`);
  }

  // The waiting player watches rw-a, so it stands beside the same fire, and
  // clicks it: nothing opens.
  await rcon.run("scoreboard players reset rw-b dw.rest_at");
  await kill("rw-b");
  await respawn(b);
  await sleep(1000);
  const waitingAtFire = (await mode("rw-b")) === 3 && ((await clock("rw-b")) ?? 0) > 0;
  check("a fall beside the fire waits", waitingAtFire, `playerGameType=${await mode("rw-b")} dw.rwait=${await clock("rw-b")}`);
  const fireB = fireOf(b);
  {
    const pb = await pos("rw-b");
    check("the waiting player stands within reach of the fire", fireB !== undefined && dist(pb, seatCell) < 3, `rw-b at ${fmt(pb)}, fire at ${fmt(seatCell)}; hitbox known to rw-b: ${fireB !== undefined}`);
  }
  if (fireB) await b.activateEntity(fireB);
  await sleep(1000);
  {
    const at = await score("rw-b", "dw.rest_at");
    check("a waiting player's click on the fire runs no opener", at === undefined, `dw.rest_at=${at}`);
    check("a waiting player is shown no dialog", dialogsB.length === 0, `${dialogsB.length} dialog(s) shown to rw-b`);
  }
  b.chat("/trigger dw.rest set 2");
  await sleep(1000);
  {
    const w = await waveState();
    check("a waiting player's own /trigger of the rest rests nothing", w.old === seated && w.fresh === 0, `old=${w.old} fresh=${w.fresh} (seated ${seated})`);
  }

  // rw-a rests while rw-b waits: the party-wide rest runs, and the wait goes on.
  a.chat("/trigger dw.rest set 2");
  await sleep(1500);
  {
    const w = await waveState();
    check("a rest during a wait re-seats the wave once", w.old === 0 && w.fresh === seated, `old=${w.old} fresh=${w.fresh} (seated ${seated})`);
    const m = await mode("rw-b");
    const c = await clock("rw-b");
    check("a rest during a wait does not release the waiter", m === 3 && c !== undefined && c > 0 && (await tagged("rw-b")) === 1, `playerGameType=${m} dw.rwait=${c} tagged=${await tagged("rw-b")}`);
    check("a rest during a wait is not a wipe", (await score("#wipe", "dw.sys")) === 0, `#wipe=${await score("#wipe", "dw.sys")}`);
  }
  seated = await markWave();
  {
    const left = Math.max(0, seconds * 20 - ((await clock("rw-b")) ?? 0));
    await sleep((left / 20) * 1000 + 1500);
    const m = await mode("rw-b");
    const pb = await pos("rw-b");
    check("after a rest during the wait, the release seats the waiter on the fire", m === 2 && (await clock("rw-b")) === undefined && dist(pb, seatCell) < 0.5, `playerGameType=${m} dw.rwait=${await clock("rw-b")} rw-b at ${fmt(pb)}, cell ${fmt(seatCell)}`);
  }
}
await guard("rw-b");

// --- 5. one waits, the other dies: a wipe ---
await kill("rw-b");
await respawn(b);
const waitingAtWipe = (await mode("rw-b")) === 3 && ((await clock("rw-b")) ?? 0) > 0;
check("the second fall waits again", waitingAtWipe, `playerGameType=${await mode("rw-b")} dw.rwait=${await clock("rw-b")}`);
await kill("rw-a");
await sleep(1500);
{
  const m = await mode("rw-b");
  check("a wipe releases the waiting player at once", waitingAtWipe && m === 2 && (await clock("rw-b")) === undefined, `waiting before the wipe: ${waitingAtWipe}; after it playerGameType=${m} dw.rwait=${await clock("rw-b")}`);
  const w = await waveState();
  check("the wipe re-seats the wave once", w.old === 0 && w.fresh === seated, `old=${w.old} fresh=${w.fresh} (seated ${seated})`);
  check("the release spends the wipe", (await score("#wipe", "dw.sys")) === 0, `#wipe=${await score("#wipe", "dw.sys")}`);
}
const reseated = await markWave();
await respawn(a);
await guard("rw-a");
{
  const m = await mode("rw-a");
  check("the second one back after a wipe does not wait", m === 2 && (await clock("rw-a")) === undefined, `playerGameType=${m} dw.rwait=${await clock("rw-a")}`);
  const w = await waveState();
  check("the second one back does not re-seat again", w.old === reseated && w.fresh === 0, `old=${w.old} fresh=${w.fresh} (re-seated ${reseated})`);
  check("no body is left tagged dw_wiped", (await count("@a[tag=dw_wiped]")) === 0, `${await count("@a[tag=dw_wiped]")} tagged`);
}
await guard("rw-b");

// --- 7. control: a player in play at the trigger cell fires it ---
await rcon.run(`tp rw-a ${cell[0] + 0.5} ${cell[1]} ${cell[2] + 0.5}`);
await sleep(1000);
check("control: a player in play at the cell fires the trigger", (await latch()) === 1, `#trig_${trigger}=${await latch()}`);

// --- 6. alone, a death does not wait ---
a.quit();
await sleep(1000);
await kill("rw-b");
await respawn(b);
{
  const m = await mode("rw-b");
  check("alone, a death does not wait", m === 2 && (await clock("rw-b")) === undefined && (await tagged("rw-b")) === 0, `playerGameType=${m} dw.rwait=${await clock("rw-b")} tagged=${await tagged("rw-b")}`);
}

b.quit();
await sleep(500);
process.stdout.write(
  failures.length === 0
    ? `respawn-wait-party: every assertion held (${checks} of ${checks})\n`
    : `respawn-wait-party: ${failures.length} of ${checks} failed: ${failures.join("; ")}\n`,
);
process.exit(failures.length === 0 ? 0 : 1);
