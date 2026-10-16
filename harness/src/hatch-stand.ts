// The hatch stand: a body standing on every shut floor gate stays on it.
//
// A floor gate — a hatch over a shaft or a stair's head, the cover over a
// ladder's hole — is a floor until the story opens it, and a floor holds the
// body that stands on it. The compiler's nav model judges that from the blocks'
// collision footprints; this stage judges it on a running server, with a
// client's own physics, which is the observer a player is.
//
// What it reads is `validation/gate-seal.json`: every gate the world-load seal
// measured, and for each one that is a floor, `stand` — the feet height a body
// on it has and the points over it to stand at (every cell centre, every
// midpoint and corner its cells share). Those are measured from the bytes,
// never from the model's verdict, so the stage tests the model instead of
// repeating it.
//
// It runs on a fresh world, before any beat (`validation/bot-run.sh` gives it a
// session of its own), so every gate is as the world was built: shut. The bot
// is put at each point by `/tp` — the probe's setup, never a mechanism the
// stage credits — left still for 30 of the server's game ticks (a game-tick
// wait, judged by the game clock and never by the wall clock), and read twice:
// its own position, and the server's (`Pos[1]`, by `tellraw`, run in the tick
// of a game-time reading so its reply is the reading's). A body below the
// gate's course on either reading fell through it.
//
// Everything above `runHatchStand` is pure, and tested.

import { readFile } from "node:fs/promises";
import path from "node:path";

/** Where the world-load seal ledger sits relative to `critical-path.json`. */
const GATE_SEAL_SUBPATH = ["validation", "gate-seal.json"] as const;

/** One shut floor gate and where to stand on it. */
export interface FloorGate {
  readonly anchor: string;
  /** The gate's course: a body below it fell through. */
  readonly courseY: number;
  readonly feetY: number;
  readonly points: readonly (readonly [number, number])[];
}

/** The floor gates the ledger names, and how many gates it examined. */
export interface HatchPlan {
  readonly examined: number;
  readonly gates: readonly FloorGate[];
}

function num(v: unknown, what: string): number {
  if (typeof v !== "number" || !Number.isFinite(v)) throw new Error(`gate-seal.json: ${what} is not a number`);
  return v;
}

/** Parse the ledger: every SEALED gate with a `stand` is a floor gate. */
export function parseHatchPlan(raw: unknown): HatchPlan {
  if (typeof raw !== "object" || raw === null) throw new Error("gate-seal.json is not an object");
  const o = raw as Record<string, unknown>;
  const gates = o["gates"];
  if (!Array.isArray(gates)) throw new Error("gate-seal.json has no gates[]");
  const out: FloorGate[] = [];
  for (const g of gates as Record<string, unknown>[]) {
    const stand = g["stand"];
    if (g["sealed"] !== true || stand === null || stand === undefined) continue;
    const s = stand as Record<string, unknown>;
    const from = g["from"] as unknown[];
    const pts = s["points"];
    if (!Array.isArray(pts) || pts.length === 0) {
      throw new Error(`gate-seal.json: ${String(g["anchor"])} has a stand with no point`);
    }
    out.push({
      anchor: String(g["anchor"]),
      courseY: num(from[1], "from[1]"),
      feetY: num(s["feet_y"], "stand.feet_y"),
      points: pts.map((p) => {
        const a = p as unknown[];
        return [num(a[0], "point x"), num(a[1], "point z")] as const;
      }),
    });
  }
  return { examined: gates.length, gates: out };
}

/** The ledger beside `critical-path.json`, or `undefined` when the build has none. */
export async function loadHatchPlanForCriticalPath(criticalPathFile: string): Promise<HatchPlan | undefined> {
  const file = path.join(path.dirname(criticalPathFile), ...GATE_SEAL_SUBPATH);
  let text: string;
  try {
    text = await readFile(file, "utf8");
  } catch (e) {
    if ((e as NodeJS.ErrnoException).code === "ENOENT") return undefined;
    throw e;
  }
  return parseHatchPlan(JSON.parse(text) as unknown);
}

/** How far a reading may sit from the gate's feet height and still be on it. */
export const STAND_TOLERANCE = 0.05;

/** How many of the server's game ticks the body is left still before it is read. */
export const SETTLE_TICKS = 30;

/** The `tellraw` that reports the body's feet height, as the server holds it. */
export const HATCH_POS_COMMAND = '/tellraw @s ["HATCHPOS ",{"nbt":"Pos[1]","entity":"@s"}]';

/** The feet height a {@link HATCH_POS_COMMAND} reply carries, or `undefined` when no line does. */
export function parseHatchPos(lines: readonly string[]): number | undefined {
  for (const line of lines) {
    const m = /^HATCHPOS (-?[\d.]+)d$/.exec(line);
    if (m) return Number(m[1]);
  }
  return undefined;
}

/** One stand's two readings. */
export interface StandReading {
  readonly anchor: string;
  readonly at: readonly [number, number];
  readonly feetY: number;
  readonly clientY: number;
  readonly serverY: number | undefined;
  readonly died: boolean;
}

/** Why a stand failed, or `undefined` when the body stayed on the gate. */
export function judgeStand(r: StandReading): string | undefined {
  const where = `${r.anchor} at (${r.at[0]}, ${r.at[1]})`;
  if (r.died) return `${where}: the body died standing on the shut gate`;
  if (r.serverY === undefined) return `${where}: the server never answered where the body is`;
  for (const [who, y] of [
    ["client", r.clientY],
    ["server", r.serverY],
  ] as const) {
    if (Math.abs(y - r.feetY) > STAND_TOLERANCE) {
      return (
        `${where}: the ${who} holds the body's feet at y=${y.toFixed(3)}, not on the shut gate ` +
        `at y=${r.feetY.toFixed(3)}${y < r.feetY ? " — it fell through" : ""}`
      );
    }
  }
  return undefined;
}

/** The stage's binding line: what it stood on, out of what the ledger examined. */
export function hatchBindingLine(plan: HatchPlan, stood: number, failed: number): string {
  const points = plan.gates.reduce((n, g) => n + g.points.length, 0);
  return (
    `hatch-stand binding: ${plan.gates.length} shut floor gate(s) of ${plan.examined} gate(s) ` +
    `examined, ${points} stand point(s), ${stood} stood, ${failed} fell or failed`
  );
}

/**
 * The `/tp` that puts the body's feet at `(x, feetY, z)`. Every coordinate is
 * written with a decimal point: the game reads a bare integer `x` or `z` as the
 * block's centre (`21` is 21.5), and a point between two cells — where a
 * lattice's gap is — would be moved onto a cell centre, where its post is.
 */
export function tpCommand(x: number, feetY: number, z: number): string {
  return `/tp @s ${x.toFixed(4)} ${feetY.toFixed(4)} ${z.toFixed(4)}`;
}

/** Put a bot on every stand point of every floor gate and judge each. */
export async function runHatchStand(criticalPathFile: string): Promise<number> {
  const plan = await loadHatchPlanForCriticalPath(criticalPathFile);
  if (plan === undefined || plan.gates.length === 0) {
    process.stderr.write(
      `hatch-stand: SKIPPED — the build ships ${plan === undefined ? "no gate-seal ledger" : "no shut floor gate"}; ` +
        `nothing to stand on\n`,
    );
    return 0;
  }
  const { botConfigFromEnv, MineflayerExecutor } = await import("./executor.ts");
  const { parseCriticalPathJson } = await import("./critical-path.ts");
  const criticalPath = parseCriticalPathJson(await readFile(criticalPathFile, "utf8"));
  const executor = new MineflayerExecutor(botConfigFromEnv());
  executor.useCampaign(criticalPath.campaignId);
  // A body that falls through a gate takes fall damage, and the executor's
  // damage handler asks which kinds are never a combat target: the delve's own
  // statement, handed over as the critical-path run hands it.
  executor.useNonCombatants(criticalPath.nonCombatants.kinds);
  try {
    await executor.connect();
    const bot = executor.requireBot();
    let died = false;
    bot.on("death", () => {
      died = true;
    });
    const failures: string[] = [];
    let stood = 0;
    for (const g of plan.gates) {
      for (const at of g.points) {
        died = false;
        const where = `hatch stand on ${g.anchor} at (${at[0]}, ${at[1]})`;
        bot.chat(tpCommand(at[0], g.feetY, at[1]));
        bot.clearControlStates();
        await executor.waitForGameTicks(SETTLE_TICKS, where);
        const reading = await executor.readGameTime(`${where}: the server's position`, () => [HATCH_POS_COMMAND]);
        const serverY = parseHatchPos(reading.lines);
        const r: StandReading = {
          anchor: g.anchor,
          at,
          feetY: g.feetY,
          clientY: bot.entity.position.y,
          serverY,
          died,
        };
        const why = judgeStand(r);
        process.stderr.write(
          `[hatch-stand] ${g.anchor} (${at[0]}, ${at[1]}): client y=${r.clientY.toFixed(3)} ` +
            `server y=${r.serverY?.toFixed(3) ?? "?"} gate feet y=${g.feetY.toFixed(3)} — ` +
            `${why === undefined ? "HELD" : "FELL"}\n`,
        );
        if (why === undefined) stood++;
        else failures.push(why);
        if (died) {
          bot.respawn();
          await executor.waitForGameTicks(40, `${where}: the respawn`);
        }
      }
    }
    process.stderr.write(`${hatchBindingLine(plan, stood, failures.length)}\n`);
    for (const f of failures) process.stderr.write(`hatch-stand FAILED: ${f}\n`);
    return failures.length === 0 ? 0 : 1;
  } finally {
    executor.close();
  }
}
