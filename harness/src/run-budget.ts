// The run's wall-clock budget, derived from the build it walks.
//
// A run that exceeds its budget fails red, so a hung bot cannot hang CI. The
// budget is the sum of what each part of the run may take by the harness's own
// bounds — the world's placement, the connect, every step of the path being
// walked (its walk at a floor speed, the wait for its completion, its declared
// waits, its fights), every scripted death the stages take — times a stated
// margin. A bigger campaign gets a bigger budget; nothing is hand-set per
// campaign. `DELVEWRIGHT_RUN_TIMEOUT_MS` is an explicit override, printed as one.

import { CLIENT_WAIT_TIMEOUT_MS } from "./client-loaded.ts";
import { DIE_RETRY_DEATHS } from "./combat.ts";
import type { Step, Vec3Tuple } from "./critical-path.ts";
import { RESPAWN_TIMEOUT_MS } from "./executor/death.ts";
import { RESPAWN_PROTECTION_TIMEOUT_MS, REENGAGE_SETTLE_MS } from "./executor/die-retry.ts";
import { LETHAL_DEATH_TIMEOUT_MS } from "./executor/lethal.ts";
import { OBJECTIVE_TIMEOUT_MS } from "./executor/objective.ts";
import { DRIVE_HELD_BUDGET_MS } from "./executor/ride.ts";
import { KILL_TIMEOUT_MS } from "./executor/wave.ts";
import type { WaypointLeg } from "./waypoints.ts";

/** Blocks a second the budget allows a walk: half the vanilla walking speed
 * (4.317 blocks/s), for the pathfinder's turns, stops and replans. */
export const WALK_FLOOR_BLOCKS_PER_S = 2;

/** What the budget multiplies the summed bounds by. */
export const BUDGET_MARGIN = 1.5;

/** A walk with no waypoint artifact is charged this many times its straight line. */
export const DETOUR_FACTOR = 3;

const TICK_MS = 50;

/** What one scripted death costs: the respawn, its protection, the re-engage
 * probe, and a walk back bounded like a step's completion wait. */
export const DEATH_MS =
  RESPAWN_TIMEOUT_MS + RESPAWN_PROTECTION_TIMEOUT_MS + REENGAGE_SETTLE_MS + OBJECTIVE_TIMEOUT_MS;

/** What the budget is derived from. */
export interface BudgetInputs {
  readonly steps: readonly Step[];
  /** The proven waypoint legs of the path walked, when the build exports them. */
  readonly legs?: readonly Pick<WaypointLeg, "waypoints">[];
  /** The world's placement budget (`placement.ts`). */
  readonly placementMs: number;
  /** Mandatory encounters the die-retry stage dies at; 0 when it is off. */
  readonly dieRetryEncounters: number;
  /** Lethal volumes the death-loop stage walks into; 0 when it is off. */
  readonly lethalVolumes: number;
  /** Crush gates the crush stage dies under; 0 when it is off. */
  readonly crushGates: number;
}

/** One summed part of the budget. */
export interface BudgetTerm {
  readonly what: string;
  readonly ms: number;
}

/** The derived budget and the terms it sums, before the margin. */
export interface RunBudget {
  readonly ms: number;
  readonly terms: readonly BudgetTerm[];
}

function dist(a: Vec3Tuple, b: Vec3Tuple): number {
  return Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);
}

function polyline(points: readonly Vec3Tuple[]): number {
  let n = 0;
  for (let i = 1; i < points.length; i++) n += dist(points[i - 1]!, points[i]!);
  return n;
}

/** The waits a step declares, in ms, beyond its walk and its completion. */
function stepWaitsMs(step: Step): number {
  const s = step as Step & {
    cutsceneSeconds?: number;
    enRouteCutsceneSeconds?: number;
    windowTicks?: number;
    waitTicks?: number;
    raceWaitTicks?: number;
    endingTailTicks?: number;
    times?: number;
    medium?: readonly Vec3Tuple[];
    rate?: number;
  };
  let ms = 0;
  ms += (s.cutsceneSeconds ?? 0) * 1000;
  ms += (s.enRouteCutsceneSeconds ?? 0) * 1000;
  ms += (s.windowTicks ?? 0) * TICK_MS;
  ms += (s.waitTicks ?? 0) * TICK_MS;
  ms += (s.raceWaitTicks ?? 0) * TICK_MS;
  ms += (s.endingTailTicks ?? 0) * TICK_MS;
  if (step.action === "kill") ms += KILL_TIMEOUT_MS;
  if (step.action === "loop") ms += (s.times ?? 1) * OBJECTIVE_TIMEOUT_MS;
  if (step.action === "ride" && s.medium !== undefined) {
    const rate = s.rate !== undefined && s.rate > 0 ? s.rate : WALK_FLOOR_BLOCKS_PER_S;
    ms += DRIVE_HELD_BUDGET_MS + (polyline(s.medium) / Math.min(rate, WALK_FLOOR_BLOCKS_PER_S)) * 1000;
  }
  return ms;
}

/** The walk the path takes, in blocks: the proven legs when there are any,
 * else the straight line between successive step positions times the detour. */
function walkBlocks(steps: readonly Step[], legs: BudgetInputs["legs"]): number {
  if (legs !== undefined && legs.length > 0) {
    return legs.reduce((n, l) => n + polyline(l.waypoints), 0);
  }
  let n = 0;
  let last: Vec3Tuple | undefined;
  for (const s of steps) {
    const pos = (s as { pos?: Vec3Tuple }).pos;
    if (pos === undefined) continue;
    if (last !== undefined) n += dist(last, pos) * DETOUR_FACTOR;
    last = pos;
  }
  return n;
}

/** The run's budget for this build: every term, summed, times {@link BUDGET_MARGIN}. */
export function runBudget(i: BudgetInputs): RunBudget {
  const blocks = walkBlocks(i.steps, i.legs);
  const terms: BudgetTerm[] = [
    { what: "placement", ms: i.placementMs },
    { what: "connect", ms: CLIENT_WAIT_TIMEOUT_MS },
    {
      what: `${i.steps.length} step completion wait(s)`,
      ms: i.steps.length * OBJECTIVE_TIMEOUT_MS,
    },
    {
      what: `${Math.round(blocks)} block(s) walked at ${WALK_FLOOR_BLOCKS_PER_S} blocks/s`,
      ms: (blocks / WALK_FLOOR_BLOCKS_PER_S) * 1000,
    },
    {
      what: "declared waits, fights, loops and rides",
      ms: i.steps.reduce((n, s) => n + stepWaitsMs(s), 0),
    },
    {
      what: `${i.dieRetryEncounters * DIE_RETRY_DEATHS} die-retry death(s)`,
      ms: i.dieRetryEncounters * DIE_RETRY_DEATHS * (DEATH_MS + KILL_TIMEOUT_MS),
    },
    {
      what: `${i.lethalVolumes} death-loop trial(s)`,
      ms: i.lethalVolumes * (DEATH_MS + LETHAL_DEATH_TIMEOUT_MS),
    },
    { what: `${i.crushGates} crush death(s)`, ms: i.crushGates * (DEATH_MS + LETHAL_DEATH_TIMEOUT_MS) },
  ];
  const sum = terms.reduce((n, t) => n + t.ms, 0);
  return { ms: Math.ceil(sum * BUDGET_MARGIN), terms };
}

/** The budget a run uses: the environment's explicit override, or the derived one. */
export function budgetFromEnv(
  derived: RunBudget,
  env: Record<string, string | undefined> = process.env,
): { ms: number; overridden: boolean } {
  const raw = env["DELVEWRIGHT_RUN_TIMEOUT_MS"];
  if (raw === undefined || raw.length === 0) return { ms: derived.ms, overridden: false };
  const ms = Number.parseInt(raw, 10);
  if (!Number.isInteger(ms) || ms <= 0 || String(ms) !== raw.trim()) {
    throw new Error(`DELVEWRIGHT_RUN_TIMEOUT_MS must be a positive integer, got ${JSON.stringify(raw)}`);
  }
  return { ms, overridden: true };
}

/** The line a run prints for its budget. */
export function describeBudget(b: RunBudget, used: { ms: number; overridden: boolean }): string {
  const parts = b.terms.filter((t) => t.ms > 0).map((t) => `${t.what} ${Math.round(t.ms / 1000)}s`);
  const derived = `derived ${b.ms}ms (${parts.join(" + ")}, x${BUDGET_MARGIN})`;
  return used.overridden
    ? `run budget: ${used.ms}ms OVERRIDDEN by DELVEWRIGHT_RUN_TIMEOUT_MS; the build's ${derived}`
    : `run budget: ${derived}`;
}
