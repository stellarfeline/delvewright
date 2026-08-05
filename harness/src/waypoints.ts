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

/**
 * A compiler-exported `timed-gate` (spec-0016 §4): a world region a datapack clock
 * fills and clears on a fixed cycle, so passage is a timing read. The harness gets
 * it as pure navigation metadata — WHERE the fill lands and HOW LONG each half of
 * the cycle runs — which is exactly what it needs to wait for a window instead of
 * declaring a leg unwalkable the moment the gate shuts on it. `region` corners are
 * inclusive and canonical (`min` ≤ `max` componentwise).
 */
export interface TimedGate {
  readonly id: string;
  readonly min: Vec3Tuple;
  readonly max: Vec3Tuple;
  /** The block the region is filled with while closed (informational). */
  readonly block: string;
  readonly openTicks: number;
  readonly closedTicks: number;
  /** Ticks after world init before the first open window. */
  readonly phase: number;
  /**
   * Whether the closing edge KILLS a player caught inside the region (spec-0016 §4
   * addendum — the portcullis judgement, unsurvivable by gearing). A crush gate must
   * never be entered blind: the harness stages at the edge and enters only on an
   * observed fresh window with full margin. `false` for every pre-crush artifact.
   */
  readonly crush: boolean;
}

/** One walked critical-path leg: the ordered waypoint polyline connecting `from` to
 * `to`, both raw visited anchor cells (matching `critical-path.json` step `pos`).
 * `timedGates` are the gates the compiler proved this leg's route walks THROUGH —
 * empty for a leg no gate clock can interrupt. */
export interface WaypointLeg {
  readonly from: Vec3Tuple;
  readonly to: Vec3Tuple;
  readonly waypoints: readonly Vec3Tuple[];
  readonly timedGates: readonly TimedGate[];
}

/** The parsed waypoints artifact. Legs are in critical-path order — the compiler
 * emits exactly one leg per WALKED critical position, and the harness walks those
 * positions in the same order, so legs are consumed in lockstep (see
 * {@link nextLegWaypoints}). Keying by destination alone is ambiguous when an anchor
 * is visited more than once, so order — not coordinate — is authoritative.
 * `timedGates` is the campaign's whole gate table; each leg carries the (resolved)
 * subset its route crosses. */
export interface Waypoints {
  readonly version: string;
  readonly campaignId: string;
  readonly timedGates: readonly TimedGate[];
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

/** A non-negative integer field (a tick count). */
function requireTicks(obj: Record<string, unknown>, key: string, pointer: string): number {
  const value = obj[key];
  if (typeof value !== "number" || !Number.isInteger(value) || value < 0) {
    fail(`${pointer}/${key}`, `must be a non-negative integer, got ${describe(value)}`);
  }
  return value;
}

/** Parse the optional top-level `timed_gates` table (spec-0016 §4). Absent → `[]`
 * (a campaign with no gate clock, and every pre-task-#81 artifact). */
function parseTimedGates(raw: Record<string, unknown>): TimedGate[] {
  const value = raw["timed_gates"];
  if (value === undefined) return [];
  if (!Array.isArray(value)) {
    fail("/timed_gates", `must be an array, got ${describe(value)}`);
  }
  return value.map((entry, i) => {
    const pointer = `/timed_gates/${i}`;
    if (!isRecord(entry)) {
      fail(pointer, `must be an object, got ${describe(entry)}`);
    }
    const region = entry["region"];
    if (!isRecord(region)) {
      fail(`${pointer}/region`, `must be an object, got ${describe(region)}`);
    }
    const min = requireVec3(region["min"], `${pointer}/region/min`);
    const max = requireVec3(region["max"], `${pointer}/region/max`);
    for (let axis = 0; axis < 3; axis++) {
      if (min[axis]! > max[axis]!) {
        fail(`${pointer}/region`, `min must not exceed max on axis ${axis}`);
      }
    }
    const openTicks = requireTicks(entry, "open_ticks", pointer);
    const closedTicks = requireTicks(entry, "closed_ticks", pointer);
    if (openTicks === 0 || closedTicks === 0) {
      // A half-cycle of 0 is DW0377 at compile time; if one ever reached the harness
      // the wait below would have no window to wait for, so refuse it here.
      fail(pointer, "open_ticks and closed_ticks must both be positive (a clock, not a static gate)");
    }
    // `crush` is optional for compatibility with pre-crush artifacts (absent →
    // false, a gate whose closing edge merely blocks). Present-but-non-boolean is a
    // structural fault: silently coercing it could blind-enter a lethal gate.
    const crushValue = entry["crush"];
    if (crushValue !== undefined && typeof crushValue !== "boolean") {
      fail(`${pointer}/crush`, `must be a boolean, got ${describe(crushValue)}`);
    }
    return {
      id: requireString(entry, "id", pointer),
      min,
      max,
      block: requireString(entry, "block", pointer),
      openTicks,
      closedTicks,
      phase: requireTicks(entry, "phase", pointer),
      crush: crushValue ?? false,
    };
  });
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
  const timedGates = parseTimedGates(raw);
  const gatesById = new Map(timedGates.map((g) => [g.id, g]));

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
    // Gate ids are resolved against the table at PARSE time, so a leg naming a gate
    // the artifact does not declare is a hard structural fault (the run would
    // otherwise silently lose the wait the compiler said this leg needs).
    const gatesValue = entry["timed_gates"];
    let legGates: readonly TimedGate[] = [];
    if (gatesValue !== undefined) {
      if (!Array.isArray(gatesValue)) {
        fail(`${pointer}/timed_gates`, `must be an array, got ${describe(gatesValue)}`);
      }
      legGates = gatesValue.map((id, j) => {
        if (typeof id !== "string") {
          fail(`${pointer}/timed_gates/${j}`, `must be a string, got ${describe(id)}`);
        }
        const gate = gatesById.get(id);
        if (!gate) {
          fail(
            `${pointer}/timed_gates/${j}`,
            `names ${JSON.stringify(id)}, which is not declared in the top-level timed_gates table`,
          );
        }
        return gate;
      });
    }
    return { from, to, waypoints, timedGates: legGates };
  });

  return { version, campaignId, timedGates, legs };
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

/**
 * The per-branch waypoint artifact that accompanies a branch's executable path
 * (task #117): `branch-path-<slug>.json` → `branch-waypoints-<slug>.json`, same
 * directory. A derivation, not a search — the compiler emits both names from one
 * slug, so the two files are one contract. Throws on a path file that is not in
 * the `branch-path-<slug>.json` shape (that would be a branch-plan contract
 * break, not a missing artifact).
 */
export function branchWaypointsFileFor(branchPathFile: string): string {
  const base = path.basename(branchPathFile);
  if (!base.startsWith("branch-path-") || !base.endsWith(".json")) {
    throw new WaypointsParseError(
      "",
      `cannot derive a per-branch waypoints file from ${JSON.stringify(branchPathFile)} — ` +
        `expected a branch-path-<slug>.json (the branch-plan contract)`,
    );
  }
  return path.join(
    path.dirname(branchPathFile),
    `branch-waypoints-${base.slice("branch-path-".length)}`,
  );
}

/**
 * Load the per-branch waypoint artifact beside a branch's executable path
 * (task #117). Returns `undefined` when absent — the CALLER must then fall back
 * LOUDLY (stderr + a run-report finding), never silently: an un-waypointed branch
 * walk is terrain-flaky where the waypointed one is deterministic, and a reader
 * comparing runs needs to know which kind this was. Present-but-malformed throws
 * {@link WaypointsParseError}, same stance as the critical-path artifact.
 */
export async function loadWaypointsForBranchPath(
  branchPathFile: string,
): Promise<Waypoints | undefined> {
  const wpPath = branchWaypointsFileFor(branchPathFile);
  let text: string;
  try {
    text = await readFile(wpPath, "utf8");
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code === "ENOENT") {
      return undefined; // caller reports the loud fallback
    }
    throw err;
  }
  return parseWaypointsJson(text);
}

/** The result of an ordered leg match: the proven waypoint polyline to replay (or
 * `undefined` for single-goal fallback) and the advanced cursor. */
export interface LegMatch {
  readonly waypoints: readonly Vec3Tuple[] | undefined;
  /** The timed gates the matched leg's proven route crosses (empty when none, and
   * when no leg matched — an unmatched walk gets no gate licence to retry). */
  readonly timedGates: readonly TimedGate[];
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
    return { waypoints: leg.waypoints, timedGates: leg.timedGates, cursor: cursor + 1 };
  }
  return { waypoints: undefined, timedGates: [], cursor };
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
