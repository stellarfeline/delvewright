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

/** The tag a body carries between its selection and its teleport. */
export const CENSUS_PLACED_TAG = "dw_census_placed";
/** The tag a body carries while the census holds it still. */
export const CENSUS_HELD_TAG = "dw_census_held";
/** The tag every body the census placed carries until it is let go, so no
 * second cell takes it. */
export const CENSUS_TAKEN_TAG = "dw_census_taken";
/** The tag on a body the census summoned for a census of a kind. */
export const CENSUS_STAND_IN_TAG = "dw_census_standin";

/** A cell's feet position: the centre of its floor. */
function feet(c: readonly [number, number, number]): string {
  return `${c[0] + 0.5} ${c[1]} ${c[2] + 0.5}`;
}

/**
 * The commands that put `row.count` bodies of the set in the box and hold them
 * there, each on a cell of its own (`row.cells`, the box's standable cells the
 * compiler handed over — never the box's centre, which can be inside a block,
 * and never one cell for two bodies). A census of a declared body (a wave's,
 * an actor's, an NPC's) takes the nearest bodies of the set, one per cell, by
 * the set's own selector terms; a census of a kind (`row.standIn`) takes no
 * body some other object owns — a raider of a fight still to come, its named
 * elite included — and summons its own for the act. Each body that moves by
 * its own AI is stilled (`NoAI`) until {@link censusReleaseCommands}: a goat
 * moved into a pen leaves it again by its own wandering, and the herding is
 * the playtest's, not the bot's. A body already without its AI (a puppet) is
 * not held, so the release never wakes it. A count of zero places nothing and
 * asks only that the box hold none. A row whose box holds fewer cells than it
 * places is refused by name.
 */
export function censusCommands(row: CensusRow): string[] {
  // A vehicle's riders are placed by boarding (the `ride` step), never here.
  if (row.count === 0 || row.select === undefined) return [];
  const cells = row.cells ?? [];
  if (cells.length < row.count) {
    throw new Error(
      `census ${row.state}: the box holds ${cells.length} cell(s) a body of the set stands on, ` +
        `and the step places ${row.count} — no body is put inside a block or onto another`,
    );
  }
  const out: string[] = [];
  for (const cell of cells.slice(0, row.count)) {
    if (row.standIn !== undefined) {
      out.push(
        `/summon ${row.standIn} ${feet(cell)} ` +
          `{Tags:["${CENSUS_STAND_IN_TAG}","${CENSUS_TAKEN_TAG}"],NoAI:1b,PersistenceRequired:1b}`,
      );
      continue;
    }
    out.push(
      `/tag @e[${row.select},tag=!${CENSUS_TAKEN_TAG},limit=1,sort=nearest] add ${CENSUS_PLACED_TAG}`,
      `/tp @e[tag=${CENSUS_PLACED_TAG}] ${feet(cell)}`,
      `/execute as @e[tag=${CENSUS_PLACED_TAG},nbt=!{NoAI:1b}] run tag @s add ${CENSUS_HELD_TAG}`,
      `/tag @e[tag=${CENSUS_PLACED_TAG}] add ${CENSUS_TAKEN_TAG}`,
      `/tag @e[tag=${CENSUS_PLACED_TAG}] remove ${CENSUS_PLACED_TAG}`,
    );
  }
  if (row.standIn === undefined) {
    out.push(`/execute as @e[tag=${CENSUS_HELD_TAG}] run data merge entity @s {NoAI:1b}`);
  }
  return out;
}

/** The commands that give every held body its AI back, remove every body the
 * census summoned, and free every body it took, once the act gated on the
 * census has ended. Each is run as its bodies, so a census that summoned or
 * took none answers nothing rather than a refusal. */
export function censusReleaseCommands(): string[] {
  return [
    `/execute as @e[tag=${CENSUS_HELD_TAG}] run data merge entity @s {NoAI:0b}`,
    `/execute as @e[tag=${CENSUS_HELD_TAG}] run tag @s remove ${CENSUS_HELD_TAG}`,
    `/execute as @e[tag=${CENSUS_STAND_IN_TAG}] run kill @s`,
    `/execute as @e[tag=${CENSUS_TAKEN_TAG}] run tag @s remove ${CENSUS_TAKEN_TAG}`,
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
