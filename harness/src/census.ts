// spec-0134 §1: a census the bot performs by command — the pure half: the
// commands that put a set's bodies in a box, and the verdict on the count the
// game reads back.

import type { CensusRow } from "./critical-path.ts";
import type { GateTerm } from "./death-loop.ts";

/** What one census did, for the run log and the run report. */
export interface CensusResult {
  readonly state: string;
  readonly placed: number;
  readonly passed: boolean;
  readonly detail: string;
}

/**
 * The box's centre, where the bodies are put: the middle of the inclusive
 * cell box, on its floor course.
 */
export function censusCentre(row: CensusRow): [number, number, number] {
  const [lo, hi] = row.region;
  return [(lo[0] + hi[0] + 1) / 2, lo[1], (lo[2] + hi[2] + 1) / 2];
}

/**
 * The command that teleports `row.count` bodies of the set into the box —
 * the nearest ones, by the set's own selector terms. A count of zero teleports
 * nothing and asks only that the box hold none.
 */
export function censusCommands(row: CensusRow): string[] {
  if (row.count === 0) return [];
  const [x, y, z] = censusCentre(row);
  return [`/tp @e[${row.select},limit=${row.count},sort=nearest] ${x} ${y} ${z}`];
}

/** The term the game's count must meet: exactly the bodies placed. */
export function censusTerm(row: CensusRow): GateTerm {
  return { objective: row.objective, holder: "#party", min: row.count, max: row.count, negate: false };
}

/**
 * The verdict: the game counts the bodies the bot placed, no fewer and no
 * more. `undefined` (the server answered neither way) establishes nothing and
 * fails the step by name.
 */
export function censusVerdict(row: CensusRow, read: boolean | undefined): CensusResult {
  const base = { state: row.state, placed: row.count };
  if (read === undefined) {
    return { ...base, passed: false, detail: `the server never said what \`${row.objective}\` holds` };
  }
  if (!read) {
    return {
      ...base,
      passed: false,
      detail:
        `the game's census of \`${row.state}\` disagrees with the ${row.count} body(ies) the bot ` +
        `put in the box — it does not read ${row.count}`,
    };
  }
  return { ...base, passed: true, detail: `\`${row.state}\` reads ${row.count}, the bodies placed` };
}
