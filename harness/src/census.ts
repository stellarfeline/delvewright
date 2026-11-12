// spec-0134 §1: a census the bot performs by command — the pure half: the
// commands that put a set's bodies in a box and hold them there, the commands
// that let them go, and the verdict on the count the game reads back.

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

/** The tag a body carries between its selection and its teleport. */
export const CENSUS_PLACED_TAG = "dw_census_placed";
/** The tag a body carries while the census holds it still. */
export const CENSUS_HELD_TAG = "dw_census_held";

/**
 * The commands that put `row.count` bodies of the set in the box and hold them
 * there — the nearest ones to the bot, by the set's own selector terms. The
 * bodies are chosen once (tagged), teleported, and each that moves by its own
 * AI is stilled (`NoAI`) until {@link censusReleaseCommands}: a goat moved into
 * a pen leaves it again by its own wandering, and the herding is the
 * playtest's, not the bot's. A body already without its AI (a puppet) is not
 * held, so the release never wakes it. A count of zero teleports nothing and
 * asks only that the box hold none.
 */
export function censusCommands(row: CensusRow): string[] {
  // A vehicle's riders are placed by boarding (the `ride` step), never here.
  if (row.count === 0 || row.select === undefined) return [];
  const [x, y, z] = censusCentre(row);
  return [
    `/tag @e[${row.select},limit=${row.count},sort=nearest] add ${CENSUS_PLACED_TAG}`,
    `/tp @e[tag=${CENSUS_PLACED_TAG}] ${x} ${y} ${z}`,
    `/execute as @e[tag=${CENSUS_PLACED_TAG},nbt=!{NoAI:1b}] run tag @s add ${CENSUS_HELD_TAG}`,
    `/execute as @e[tag=${CENSUS_HELD_TAG}] run data merge entity @s {NoAI:1b}`,
    `/tag @e[tag=${CENSUS_PLACED_TAG}] remove ${CENSUS_PLACED_TAG}`,
  ];
}

/** The commands that give every held body its AI back, once the act gated on
 * the census has ended. */
export function censusReleaseCommands(): string[] {
  return [
    `/execute as @e[tag=${CENSUS_HELD_TAG}] run data merge entity @s {NoAI:0b}`,
    `/execute as @e[tag=${CENSUS_HELD_TAG}] run tag @s remove ${CENSUS_HELD_TAG}`,
  ];
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
