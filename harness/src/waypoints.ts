// Parser + lookup for the compiler's critical-path waypoints validation artifact
// (`validation/critical-path-waypoints.json`, emitted by the compiler's `waypoints`
// module — task #38).
//
// The artifact is COMPILER-PROVEN navigation data: for each walked critical-path
// leg, the ordered waypoint polyline the compile-time DW0311 A* guard proved
// connects it over the assembled geometry. The harness replays these as successive
// nearby pathfinder goals so each solve is trivial, instead of one distant goal
// that explodes mineflayer's A* budget on a large open winding cave (the
// "No path to the goal!" strand on geometry that is provably connected).
//
// Assertions + navigation only (the harness contract): the harness never COMPUTES a
// route — it only replays a route the compiler already proved. Absence of the
// artifact is not an error; the caller falls back to single-goal navigation.

import { readFile } from "node:fs/promises";
import path from "node:path";
import { SUPPORTED_DSL_VERSIONS, type Vec3Tuple } from "./critical-path.ts";

/** The sub-path of the waypoints artifact relative to `critical-path.json`'s dir. */
const WAYPOINTS_SUBPATH = ["validation", "critical-path-waypoints.json"] as const;

/** How close (blocks) the bot must get to each intermediate waypoint. Small — the
 * waypoints are standable floor cells and the hops between them are short. */
export const WAYPOINT_RANGE = 1;

/** One walked critical-path leg: the ordered waypoint polyline connecting `from` to
 * `to`, both raw visited anchor cells (matching `critical-path.json` step `pos`). */
export interface WaypointLeg {
  readonly from: Vec3Tuple;
  readonly to: Vec3Tuple;
  readonly waypoints: readonly Vec3Tuple[];
}

/** The parsed waypoints artifact. Legs are in critical-path order — the compiler
 * emits exactly one leg per WALKED critical position, and the harness walks those
 * positions in the same order, so legs are consumed in lockstep (see
 * {@link nextLegWaypoints}). Keying by destination alone is ambiguous when an anchor
 * is visited more than once, so order — not coordinate — is authoritative. */
export interface Waypoints {
  readonly version: string;
  readonly campaignId: string;
  readonly legs: readonly WaypointLeg[];
}

/** A single pathfinder goal: get within `range` blocks of `(x, y, z)`. */
export interface GoalSpec {
  readonly x: number;
  readonly y: number;
  readonly z: number;
  readonly range: number;
}

/**
 * Raised when the waypoints artifact is present but structurally invalid. A missing
 * artifact is NOT an error (the caller falls back) — only malformed data throws.
 */
export class WaypointsParseError extends Error {
  override readonly name = "WaypointsParseError";
  readonly pointer: string;
  constructor(pointer: string, detail: string) {
    super(`waypoints${pointer} ${detail}`);
    this.pointer = pointer;
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function fail(pointer: string, detail: string): never {
  throw new WaypointsParseError(pointer, detail);
}

function requireString(obj: Record<string, unknown>, key: string, pointer: string): string {
  const value = obj[key];
  if (typeof value !== "string" || value.length === 0) {
    fail(`${pointer}/${key}`, `must be a non-empty string, got ${describe(value)}`);
  }
  return value;
}

/** An absolute `[x, y, z]` integer block position. */
function requireVec3(value: unknown, pointer: string): Vec3Tuple {
  if (!Array.isArray(value)) {
    fail(pointer, `must be an array, got ${describe(value)}`);
  }
  if (value.length !== 3) {
    fail(pointer, `must have exactly 3 elements, got ${value.length}`);
  }
  const coords = value.map((entry, i) => {
    if (typeof entry !== "number" || !Number.isFinite(entry)) {
      fail(`${pointer}/${i}`, `must be a finite number, got ${describe(entry)}`);
    }
    return entry;
  });
  return [coords[0]!, coords[1]!, coords[2]!];
}

/** Whether two integer block positions are identical. */
function samePos(a: Vec3Tuple, b: Vec3Tuple): boolean {
  return a[0] === b[0] && a[1] === b[1] && a[2] === b[2];
}

/** Validate and normalize a parsed JSON value into a {@link Waypoints}. */
export function parseWaypoints(raw: unknown): Waypoints {
  if (!isRecord(raw)) {
    fail("", `must be an object, got ${describe(raw)}`);
  }
  const version = requireString(raw, "version", "");
  if (!(SUPPORTED_DSL_VERSIONS as readonly string[]).includes(version)) {
    fail(
      "/version",
      `unsupported version ${JSON.stringify(version)}; harness supports ${SUPPORTED_DSL_VERSIONS.join(", ")}`,
    );
  }
  const campaignId = requireString(raw, "campaign_id", "");

  const legsValue = raw["legs"];
  if (!Array.isArray(legsValue)) {
    fail("/legs", `must be an array, got ${describe(legsValue)}`);
  }
  const legs: WaypointLeg[] = legsValue.map((entry, i) => {
    const pointer = `/legs/${i}`;
    if (!isRecord(entry)) {
      fail(pointer, `must be an object, got ${describe(entry)}`);
    }
    const from = requireVec3(entry["from"], `${pointer}/from`);
    const to = requireVec3(entry["to"], `${pointer}/to`);
    const wpsValue = entry["waypoints"];
    if (!Array.isArray(wpsValue)) {
      fail(`${pointer}/waypoints`, `must be an array, got ${describe(wpsValue)}`);
    }
    if (wpsValue.length === 0) {
      fail(`${pointer}/waypoints`, "must contain at least one waypoint");
    }
    const waypoints = wpsValue.map((w, j) => requireVec3(w, `${pointer}/waypoints/${j}`));
    return { from, to, waypoints };
  });

  return { version, campaignId, legs };
}

/** Parse waypoints JSON text: JSON.parse then structural validation. */
export function parseWaypointsJson(text: string): Waypoints {
  let raw: unknown;
  try {
    raw = JSON.parse(text);
  } catch (cause) {
    const detail = cause instanceof Error ? cause.message : String(cause);
    throw new WaypointsParseError("", `is not valid JSON: ${detail}`);
  }
  return parseWaypoints(raw);
}

/**
 * Load the waypoints artifact that sits alongside `criticalPathPath`
 * (`<dir>/validation/critical-path-waypoints.json`). Returns `undefined` when the
 * artifact is absent — the caller then falls back to single-goal navigation. A
 * present-but-malformed artifact throws {@link WaypointsParseError}.
 */
export async function loadWaypointsForCriticalPath(
  criticalPathPath: string,
): Promise<Waypoints | undefined> {
  const wpPath = path.join(path.dirname(criticalPathPath), ...WAYPOINTS_SUBPATH);
  let text: string;
  try {
    text = await readFile(wpPath, "utf8");
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") {
      return undefined; // no artifact → fall back to single-goal navigation
    }
    throw err;
  }
  return parseWaypointsJson(text);
}

/** The result of an ordered leg match: the proven waypoint polyline to replay (or
 * `undefined` for single-goal fallback) and the advanced cursor. */
export interface LegMatch {
  readonly waypoints: readonly Vec3Tuple[] | undefined;
  readonly cursor: number;
}

/**
 * Lockstep leg matcher (pure). If the leg at `cursor` targets `pos`, return its
 * waypoints and `cursor + 1` (consume it); otherwise return `undefined` waypoints
 * and the UNCHANGED cursor (the caller falls back to single-goal navigation, and a
 * later walked step can still match this leg).
 *
 * Order — not destination coordinate — is authoritative: the compiler emits exactly
 * one leg per walked critical position in path order, and the harness walks those
 * positions in the same order. A sub-walk (e.g. chasing a wave mob) or a
 * post-transport step whose target is not the next leg's destination simply does
 * not consume, so an anchor visited more than once never grabs the wrong leg's
 * route.
 */
export function nextLegWaypoints(
  legs: readonly WaypointLeg[],
  cursor: number,
  pos: Vec3Tuple,
): LegMatch {
  const leg = legs[cursor];
  if (leg && samePos(leg.to, pos)) {
    return { waypoints: leg.waypoints, cursor: cursor + 1 };
  }
  return { waypoints: undefined, cursor };
}

/**
 * Drop proven waypoints the bot's own physical model cannot stand on. A waypoint is
 * a FEET cell the compiler proved standable under its full-solid occupancy model —
 * every non-air block is a 1×1×1 cube, so the compiler can prove a leg by standing
 * the player ON TOP of a fence (a legal +1 step in that model). Vanilla physics
 * makes a fence 1.5 blocks tall, and mineflayer-pathfinder marks any block whose
 * collision shape is taller than 1 (fence/wall/closed fence-gate) NON-physical
 * (`Movements.fences`): it will never solve a subgoal standing atop one, so replaying
 * that waypoint as a hard hop wedges the leg. `supportStandable(cell)` reports
 * whether the block directly below a feet cell is one the bot can actually stand on;
 * a waypoint that fails it is dropped. Endpoints are not special-cased — the leg's
 * TRUE destination is appended by {@link walkGoals} regardless — so the compiler's
 * actual proof (end-to-end connectivity) is preserved; the pathfinder simply bridges
 * the neighbouring proven cells with a real-shape route (through the adjacent gate,
 * opening it as a player must). Pure (predicate injected) so it is unit-testable
 * without a bot.
 */
export function retainStandableWaypoints(
  waypoints: readonly Vec3Tuple[],
  supportStandable: (cell: Vec3Tuple) => boolean,
): readonly Vec3Tuple[] {
  return waypoints.filter((w) => supportStandable(w));
}

/**
 * The ordered pathfinder goals for a walk to `pos` at `finalRange`: the given proven
 * waypoint hops (each at {@link WAYPOINT_RANGE}) followed by the final destination
 * goal. When `legWaypoints` is `undefined` (no leg matched), just the single
 * destination goal — the original single-goal behavior (fallback). Pure (no bot) so
 * the leg-by-leg plan is unit-testable.
 */
export function walkGoals(
  legWaypoints: readonly Vec3Tuple[] | undefined,
  pos: Vec3Tuple,
  finalRange: number,
): readonly GoalSpec[] {
  const out: GoalSpec[] = [];
  if (legWaypoints) {
    for (const [x, y, z] of legWaypoints) {
      out.push({ x, y, z, range: WAYPOINT_RANGE });
    }
  }
  out.push({ x: pos[0], y: pos[1], z: pos[2], range: finalRange });
  return out;
}

function describe(value: unknown): string {
  if (value === null) return "null";
  if (Array.isArray(value)) return "array";
  return typeof value;
}
