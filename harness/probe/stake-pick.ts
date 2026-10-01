// Which recovery-stake place does a right-click collect: the box clicked, or the
// box nearest the player who clicked?
//
//   node harness/probe/stake-pick.ts <host> <port> <p1x,p1y,p1z> <p2x,p2y,p2z> <objective>
//
// Two players on a delve whose `on_death` drops a stake collected by anyone
// (`delve-bot`, opped, and `delve-bot2`). Each is given 2 of `<objective>` and
// killed where it stands — delve-bot at P1, delve-bot2 at P2 — so each death
// leaves its place. delve-bot2 then stands one block from P1 (nearer P1's box
// than P2's), looks at P2's box and right-clicks IT. The probe prints both
// players' ledgers and live slots, and which boxes still stand. The setup
// (`/tp`, `/kill`, `/scoreboard`) is the probe's; the click is a client's.
import { createHarnessBot } from "../src/client-loaded.ts";

const [host, portText, p1Text, p2Text, objective] = process.argv.slice(2);
if (!host || !portText || !p1Text || !p2Text || !objective) {
  process.stderr.write("usage: stake-pick.ts <host> <port> <x,y,z> <x,y,z> <objective>\n");
  process.exit(2);
}
const cell = (t: string) => t.split(",").map(Number) as [number, number, number];
const P1 = cell(p1Text);
const P2 = cell(p2Text);
const port = Number(portText);
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const join = (username: string) =>
  createHarnessBot({ host, port, username, version: "1.21.11", auth: "offline" }).bot;

const a = join("delve-bot");
const b = join("delve-bot2");
const said: string[] = [];
a.on("messagestr", (m: string) => said.push(m));
await Promise.all([a, b].map((bot) => new Promise((r) => bot.once("spawn", r))));
await sleep(4000);

async function ask(lines: string[]): Promise<string[]> {
  const from = said.length;
  for (const l of lines) a.chat(l);
  await sleep(1500);
  return said.slice(from).filter((m) => m.startsWith("PROBE "));
}
const ledger = () =>
  ask(
    ["delve-bot", "delve-bot2"].flatMap((p) => [
      `/tellraw delve-bot ["PROBE ${p} ${objective}=",{"score":{"name":"${p}","objective":"${objective}"}}]`,
      `/tellraw delve-bot ["PROBE ${p} live0=",{"score":{"name":"${p}","objective":"dw.kl0_${objective.replace(/^dw\.s_/, "")}"}}]`,
    ]),
  );
const boxes = () =>
  Object.values(a.entities)
    .filter((e) => e.name === "interaction")
    .map((e) => `[${e.position.x.toFixed(2)}, ${e.position.y.toFixed(2)}, ${e.position.z.toFixed(2)}]`);

a.chat(`/scoreboard players set delve-bot ${objective} 2`);
a.chat(`/scoreboard players set delve-bot2 ${objective} 2`);
a.chat(`/tp delve-bot2 ${P2[0] + 0.5} ${P2[1]} ${P2[2] + 0.5}`);
a.chat(`/tp @s ${P1[0] + 0.5} ${P1[1]} ${P1[2] + 0.5}`);
await sleep(2000);
a.chat("/kill delve-bot2");
await sleep(1500);
a.chat("/kill @s");
await sleep(3000);
for (const bot of [a, b]) if (bot.health <= 0 || !bot.entity?.isValid) bot.respawn();
await sleep(3000);
process.stdout.write(`before: boxes ${boxes().join(" ")}\n`);
for (const l of await ledger()) process.stdout.write(`before: ${l}\n`);

// delve-bot2 stands one block from P1's box, two or more from P2's, and clicks P2's.
a.chat(`/tp delve-bot2 ${P1[0] + 1.5} ${P1[1]} ${P1[2] + 0.5}`);
await sleep(2500);
const target = Object.values(b.entities)
  .filter((e) => e.name === "interaction")
  .sort(
    (x, y) =>
      Math.hypot(x.position.x - (P2[0] + 0.5), x.position.z - (P2[2] + 0.5)) -
      Math.hypot(y.position.x - (P2[0] + 0.5), y.position.z - (P2[2] + 0.5)),
  )[0];
if (!target) {
  process.stdout.write("no box at P2 to click\n");
  process.exit(1);
}
const me = b.entity.position;
process.stdout.write(
  `delve-bot2 at [${me.x.toFixed(2)}, ${me.y.toFixed(2)}, ${me.z.toFixed(2)}] clicks the box at ` +
    `[${target.position.x.toFixed(2)}, ${target.position.y.toFixed(2)}, ${target.position.z.toFixed(2)}]\n`,
);
await b.lookAt(target.position.offset(0, 1, 0), true);
await b.activateEntity(target);
await sleep(2500);
process.stdout.write(`after: boxes ${boxes().join(" ")}\n`);
for (const l of await ledger()) process.stdout.write(`after: ${l}\n`);
a.quit();
b.quit();
