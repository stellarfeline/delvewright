// STEP 2 — the Unremembered Guard, alone, under the assist spec-0023 specifies.
//
// The rig boots the SHIPPED delve image built from campaign/vesperhold at content
// 512608c (Dockerfile.delve, pinned base digest), so the geometry, the world
// settings (`difficulty=normal`) and the wave are the campaign's own bytes. The
// bot is the harness's own `MineflayerExecutor` running its own `fightWave` under
// its own `withAssist` — no combat code is replaced, nothing about the encounter
// is changed.
//
// Per repetition: the bot is re-dressed from the datapack's own
// `class_apply_sellsword`, stood at the great hall's entry anchor, the wave is
// re-seated by the datapack's own `spawn_unremembered_guard`, and the fight is
// run. Every number comes from the SERVER: the player's damage statistics and the
// wave census, never the bot's client-side tally.
import { readFile, writeFile } from "node:fs/promises";
import { botConfigFromEnv, MineflayerExecutor } from "../../harness/src/executor.ts";
import { loadCombatPlanForCriticalPath } from "../../harness/src/combat.ts";
import { parseCriticalPathJson, type KillStep } from "../../harness/src/critical-path.ts";
import { loadWaypointsForCriticalPath } from "../../harness/src/waypoints.ts";
import { rig, num, score, sleep } from "./rig.mjs";

const CONTAINER = process.env["SPIKE_CONTAINER"] ?? "dw-guard-rig";
const CP = process.env["SPIKE_CP"] ?? "validation/delve-output/critical-path.json";
const OUT = process.env["SPIKE_OUT"] ?? new URL("./guard-observations.json", import.meta.url).pathname;
const REPS = Number(process.env["SPIKE_REPS"] ?? 10);
const BOT = process.env["DELVEWRIGHT_BOT_USERNAME"] ?? "delve-bot";
/** Where the bot stands when the fight opens: the great hall's own reach anchor. */
const ENTRY = [83, 80, 86] as const;

const r = rig(CONTAINER);

const OBJ: Record<string, string> = {
  taken: "minecraft.custom:minecraft.damage_taken",
  blocked: "minecraft.custom:minecraft.damage_blocked_by_shield",
  resisted: "minecraft.custom:minecraft.damage_resisted",
  dealt: "minecraft.custom:minecraft.damage_dealt",
};

async function objectives(): Promise<void> {
  for (const [k, crit] of Object.entries(OBJ)) await r.probe(`scoreboard objectives add dw.${k} ${crit}`);
  await r.probe("scoreboard objectives add dw.hp dummy");
  const list = await r.run("scoreboard objectives list");
  for (const k of Object.keys(OBJ)) {
    if (!list.includes(`dw.${k}`)) throw new Error(`objective dw.${k} is not in the server's list: ${list}`);
  }
  return;
}

async function stats(): Promise<Record<string, number>> {
  const out = await r.batch([
    `scoreboard players get ${BOT} dw.taken`,
    `scoreboard players get ${BOT} dw.blocked`,
    `scoreboard players get ${BOT} dw.resisted`,
    `scoreboard players get ${BOT} dw.dealt`,
    `data get entity ${BOT} Health`,
  ]);
  // Vanilla stores each of these statistics as round(damage * 10).
  return {
    taken: score(out[0]!) ?? 0,
    blocked: score(out[1]!) ?? 0,
    resisted: score(out[2]!) ?? 0,
    dealt: score(out[3]!) ?? 0,
    hp: num(out[4]!) ?? -1,
  };
}

/** The wave's remaining bodies and their total health, read from the SERVER. */
async function standing(): Promise<{ count: number; health: number }> {
  await r.probe(
    "execute store result score #rigN dw.sys if entity @e[tag=dw_wave_unremembered_guard]",
  );
  const count = score(await r.probe("scoreboard players get #rigN dw.sys")) ?? 0;
  // Summed on the server rather than one `data get` per body: one reply, so no
  // answer can be cut at the rcon packet ceiling.
  await r.probe("scoreboard players set #rigHP dw.sys 0");
  await r.probe(
    "execute as @e[tag=dw_wave_unremembered_guard] store result score @s dw.hp run data get entity @s Health 1",
  );
  await r.probe(
    "execute as @e[tag=dw_wave_unremembered_guard] run scoreboard players operation #rigHP dw.sys += @s dw.hp",
  );
  const health = score(await r.probe("scoreboard players get #rigHP dw.sys")) ?? 0;
  return { count, health };
}

// A death that arrives after the fight loop has already returned rejects a waiter
// nobody is holding. It is RECORDED here rather than ending the run, because it
// is a fact about the rig's teardown and not about the fight just measured.
const strayDeaths: string[] = [];
const stray = (e: unknown): void => {
  const m = e instanceof Error ? e.message : String(e);
  if (!m.startsWith("bot died")) {
    process.stderr.write(`[rig] FATAL: ${m}\n`);
    process.exit(1);
  }
  strayDeaths.push(m);
  process.stderr.write(`[rig] stray death after the fight: ${m}\n`);
};
process.on("unhandledRejection", stray);
process.on("uncaughtException", stray);

async function main(): Promise<void> {
  await objectives();
  const cpText = await readFile(CP, "utf8");
  const cp = parseCriticalPathJson(cpText);
  const step = cp.steps.find(
    (s): s is KillStep => s.action === "kill" && s.wave === "wave/unremembered-guard",
  );
  if (!step) throw new Error("the critical path holds no kill step for wave/unremembered-guard");
  const combatPlan = await loadCombatPlanForCriticalPath(CP);
  if (!combatPlan) throw new Error("no combat plan beside the critical path");
  const waypoints = await loadWaypointsForCriticalPath(CP);

  // ONE bot for the whole run, recovered between repetitions the way the harness
  // recovers on the ladder. A fresh connection per repetition logged the new body
  // in while the old one was still a corpse, and the run died on a death that had
  // already been measured.
  const exec = new MineflayerExecutor(botConfigFromEnv());
  exec.useCampaign(cp.campaignId);
  if (waypoints) exec.useWaypoints(waypoints);
  exec.usePathObjectives(new Set(cp.steps.flatMap((s) => ("objective" in s ? [s.objective] : []))));
  exec.useNonCombatants(cp.nonCombatants.kinds);
  exec.useCombatPlan(combatPlan, false, false);
  await exec.connect();
  await sleep(1_500);

  const rows: unknown[] = [];
  for (let rep = 0; rep < REPS; rep++) {
    if (exec.deathDiagnostic()) await exec.recoverFromDeath();
    await r.runAll([`op ${BOT}`, `gamemode adventure ${BOT}`]);
    // A fresh body each repetition, dressed by the datapack's own kit function.
    await r.runAll([
      `clear ${BOT}`,
      `effect clear ${BOT}`,
      `execute as ${BOT} run function vesperhold:class_apply_sellsword`,
      `tp ${BOT} ${ENTRY[0]}.5 ${ENTRY[1]} ${ENTRY[2]}.5 180 0`,
      `effect give ${BOT} minecraft:instant_health 1 10 true`,
    ]);
    await r.probe(`kill @e[tag=dw_wave_unremembered_guard]`);
    await sleep(500);
    await r.run("function vesperhold:spawn_unremembered_guard");
    await sleep(1_000);
    const seated = await standing();
    if (seated.count !== 5) throw new Error(`the wave seated ${seated.count} bodies, not 5`);
    await sleep(1_500);

    const before = await stats();
    if (before.hp! < 20) throw new Error(`the bot opened on ${before.hp} health`);
    const t0 = Date.now();
    let result = "won";
    let err: string | undefined;
    // The encounter, through the harness's own assist and its own fight loop.
    const enc = combatPlan.encounters.find((e) => e.wave === step.wave);
    if (!enc) throw new Error("the combat plan holds no encounter for the wave");
    try {
      await (exec as unknown as {
        withAssist: <T>(e: unknown, why: string, body: () => Promise<T>) => Promise<T>;
        fightWave: (s: KillStep) => Promise<void>;
      }).withAssist(enc, "rig: the assisted attempt, measured", () =>
        (exec as unknown as { fightWave: (s: KillStep) => Promise<void> }).fightWave(step),
      );
    } catch (e) {
      result = exec.deathDiagnostic() ? "died" : "lost";
      err = e instanceof Error ? e.message : String(e);
    }
    const seconds = (Date.now() - t0) / 1000;
    const after = await stats();
    const left = await standing();
    const row = {
      rep,
      result,
      seconds: Number(seconds.toFixed(2)),
      // Statistics are round(damage * 10); divided here so every number below is
      // in half-hearts of damage.
      damage_taken: (after.taken! - before.taken!) / 10,
      damage_blocked: (after.blocked! - before.blocked!) / 10,
      damage_resisted: (after.resisted! - before.resisted!) / 10,
      damage_dealt: (after.dealt! - before.dealt!) / 10,
      bodies_left: left.count,
      wave_health_left: left.health,
      wave_health_dealt: 5 * 34 - left.health,
      hp_left: after.hp,
      error: err,
    };
    rows.push(row);
    process.stderr.write(`[rig] rep ${rep}: ${JSON.stringify(row)}\n`);
    // The wave goes down BEFORE the corpse is released, so a bot that is
    // respawning cannot be killed again by last repetition's bodies.
    await r.probe("kill @e[tag=dw_wave_unremembered_guard]");
    await sleep(2_000);
  }
  exec.close();
  await writeFile(OUT, JSON.stringify({ reps: REPS, rows, strayDeaths }, null, 2));
  process.stderr.write(`-> ${OUT}\n`);
    process.exit(0);
}

main().catch((e) => {
  console.error(e);
    process.exit(1);
});
