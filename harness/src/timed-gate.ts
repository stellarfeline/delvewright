// Timing arithmetic and geometry for spec-0016 §4 `timed-gate` crossings.
//
// A timed gate fills and clears a world region on a fixed datapack clock, so a leg
// that walks through it is only walkable during the open half of the cycle. The
// compile-time proof (DW0378) already guarantees the window is READABLE — at least
// 20% of every cycle admits a crossing. What the runtime harness lacked is the same
// verb: mineflayer-pathfinder has no concept of "wait for the window", so the gate
// filling mid-approach aborted the path ("Path was stopped before it could be
// completed!") and the leg failed as if the geometry were broken.
//
// Everything here is pure arithmetic/geometry over the compiler-exported gate table
// (`validation/critical-path-waypoints.json`) — no bot, no game logic, no campaign
// knowledge. The executor supplies the bot-facing pieces (block reads, pathfinding).

import type { TimedGate } from "./waypoints.ts";
import type { Vec3Tuple } from "./critical-path.ts";

/** Minecraft server tick rate — the unit the compiler states gate clocks in. */
export const TICKS_PER_SECOND = 20;

/**
 * Wall-clock slack added on top of the pure cycle arithmetic, covering everything
 * the tick clock does not: pathfinder think time, the walk itself, chunk load, and
 * server tick jitter under CI load.
 */
export const GATE_RETRY_MARGIN_MS = 15_000;

/**
 * The minimum number of crossing attempts a gate-marked hop gets, regardless of the
 * wall-clock budget. A single attempt can burn most of the budget on pathfinder
 * timeouts, and a run that never got a second try has not actually tested the
 * window — so the budget is a floor on TIME, this is a floor on TRIES.
 */
export const GATE_MIN_ATTEMPTS = 3;

/**
 * The player's standing occupancy in blocks — feet cell plus the cell above it. The
 * same model the compiler marks a crossing with (`compiler::waypoints`), so both
 * rungs agree on what "the gate caught the player" means.
 */
export const PLAYER_OCCUPANCY = 2;

/** Poll interval when watching a gate region's blocks for an open/closed edge. */
export const GATE_POLL_MS = 100;

/** One full open+closed cycle of `gate`, in ticks. */
export function cycleTicks(gate: TimedGate): number {
  return gate.openTicks + gate.closedTicks;
}

/** One full open+closed cycle of `gate`, in milliseconds. */
export function cycleMs(gate: TimedGate): number {
  return (cycleTicks(gate) / TICKS_PER_SECOND) * 1_000;
}

/** The longest full cycle among `gates`, in milliseconds (0 for an empty set). */
export function maxCycleMs(gates: readonly TimedGate[]): number {
  return gates.reduce((acc, g) => Math.max(acc, cycleMs(g)), 0);
}

/**
 * The wall-clock budget a gate-crossing hop may spend retrying before it is a real
 * failure: **two** full cycles of the slowest gate plus {@link GATE_RETRY_MARGIN_MS}.
 *
 * Two cycles, not one, is the point — one cycle only proves the bot saw a window,
 * and a window it entered a tick late looks identical to a wall. Two guarantee at
 * least one crossing attempt that began at the TOP of an open window. The bound is
 * strict: past it the leg fails, naming the gate, so a genuinely unwalkable leg is
 * never retried into a green.
 */
export function gateRetryBudgetMs(gates: readonly TimedGate[]): number {
  if (gates.length === 0) return 0;
  return 2 * maxCycleMs(gates) + GATE_RETRY_MARGIN_MS;
}

/**
 * How long to watch for a closed→open edge before crossing anyway: one full cycle
 * (the edge is guaranteed within it) plus margin. Bounded so a gate whose blocks the
 * bot cannot see (chunk not loaded) never hangs the run — the wait gives up and the
 * caller simply tries the hop.
 */
export function gateWindowWaitMs(gates: readonly TimedGate[]): number {
  if (gates.length === 0) return 0;
  return maxCycleMs(gates) + GATE_RETRY_MARGIN_MS;
}

/** Every block cell of a gate's (inclusive) region, in deterministic order. */
export function gateRegionCells(gate: TimedGate): Vec3Tuple[] {
  const cells: Vec3Tuple[] = [];
  for (let x = gate.min[0]; x <= gate.max[0]; x++) {
    for (let y = gate.min[1]; y <= gate.max[1]; y++) {
      for (let z = gate.min[2]; z <= gate.max[2]; z++) {
        cells.push([x, y, z]);
      }
    }
  }
  return cells;
}

/**
 * Whether a bot whose feet are at `cell` is standing IN `gate`'s fill — its feet or
 * head cell inside the region ({@link PLAYER_OCCUPANCY}). That, and only that, is
 * where the closing fill can catch it: the fill is atomic and lands nowhere outside
 * the region, so a bot even one block clear is safe where it stands.
 */
export function insideGate(cell: Vec3Tuple, gate: TimedGate): boolean {
  return (
    cell[0]! >= gate.min[0]! &&
    cell[0]! <= gate.max[0]! &&
    cell[2]! >= gate.min[2]! &&
    cell[2]! <= gate.max[2]! &&
    cell[1]! <= gate.max[1]! &&
    cell[1]! + (PLAYER_OCCUPANCY - 1) >= gate.min[1]!
  );
}

/**
 * Whether a failed crossing must RETREAT to a standoff before waiting, rather than
 * simply waiting where it stands. Only a bot caught inside the fill must move: every
 * retreated block has to be re-walked inside the open window, so a needless retreat
 * makes the next crossing strictly harder (the compiler's DW0378 window proof covers
 * the gate SPAN, not an arbitrary run-up to it).
 */
export function needsStandoff(
  cell: Vec3Tuple | undefined,
  gates: readonly TimedGate[],
): boolean {
  // An unknown position is treated as unsafe: retreating to a proven cell is the
  // conservative move when we cannot prove the bot is already clear.
  if (!cell) return gates.length > 0;
  return gates.some((g) => insideGate(cell, g));
}

/** Human-readable gate summary for a failure message: id and cycle, per gate. */
export function describeGates(gates: readonly TimedGate[]): string {
  return gates
    .map(
      (g) =>
        `\`${g.id}\` (${g.openTicks}t open / ${g.closedTicks}t closed, ` +
        `${cycleTicks(g)}t cycle ≈ ${(cycleMs(g) / 1_000).toFixed(1)}s)`,
    )
    .join(", ");
}
