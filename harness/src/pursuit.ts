// The pursuit leg's verdict (spec-0132 §5): a pursued step — one the compiler
// says is walked while an `outrun` body is loose — passes only when the bot
// arrives alive, nothing was staged away and nothing restored, every pursuer was
// read standing at every reading, and at least one reading was taken. Whether the
// body is free to act is the compiler's output, proven by its Rust tests; whether
// it then moves is vanilla's, and is stated as displacement, never judged.
//
// The readings are the compiler's census (`actor_census_<id>`, the anchored
// `[dw:actor …]` line), called once a second on the server's clock while the
// step is walked and once more after its completion lands. The bot never infers
// which bodies are pursuers: the exported path names them (`pursuers`, format 6).
//
// This file is pure: the readings are taken by `executor/pursuit.ts`.

import type { PursuerRow, Vec3Tuple } from "./critical-path.ts";

/** One census of one pursuer, beside where the bot stood and its health. */
export interface PursuitReading {
  readonly actor: string;
  /** The server's game tick the reading answered at. */
  readonly tick: number;
  /** Whether the free body stood. */
  readonly alive: boolean;
  /** The body's feet, when it stood. */
  readonly feet?: Vec3Tuple;
  /** The bot's feet at the reading. */
  readonly botFeet: Vec3Tuple;
  /** The bot's health at the reading. */
  readonly botHealth: number;
}

/** What one pursued step recorded. */
export interface PursuitStepRecord {
  /** The 0-based step index. */
  readonly step: number;
  /** The step, in words. */
  readonly label: string;
  readonly pursuers: readonly PursuerRow[];
  readonly readings: readonly PursuitReading[];
  /** Set when the bot died during the step: who the server named, or a stated other. */
  readonly died?: { readonly attacker: string };
  /** Bodies the harness staged away during the step (asserted zero). */
  readonly staged: number;
  /** Health the harness restored or refunded during the step (asserted zero). */
  readonly restored: number;
  /** The staging and restoring acts the pursuit rule refused during the step. */
  readonly refusals: readonly string[];
}

/** Where and when a pursuer was first read standing, across the run. */
export interface Arrival {
  readonly cell: Vec3Tuple;
  readonly tick: number;
}

/** The measurement over one pursued step — stated, never a verdict. */
export interface PursuitMeasurement {
  /** The least bot–pursuer distance read, or `undefined` when none stood. */
  readonly closestApproach: number | undefined;
  /** How far the pursuers' feet moved, reading to reading, summed. */
  readonly displacement: number;
  /** The bot's lowest health read. */
  readonly lowestHealth: number | undefined;
  readonly readings: number;
}

/** The verdict on one pursued step. */
export interface PursuitVerdict {
  readonly failures: readonly string[];
  readonly measurement: PursuitMeasurement;
}

function distance(a: Vec3Tuple, b: Vec3Tuple): number {
  return Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);
}

function fmt(n: number): string {
  return n.toFixed(2);
}

/**
 * **Judge one pursued step.** `arrivals` is the run's record of where each
 * pursuer was first read standing (updated here: the first standing reading of
 * a body sets its arrival cell and tick; an undated pursuer not yet arrived is not lost).
 *
 * The refusals, each a `pursuit` failure naming the step and the pursuer:
 * `died`, `staged`, `restored`, `pursuer_lost` (absent or dead at a reading),
 * `unread` (no reading). Displacement is a measurement, never a refusal.
 */
export function judgePursuedStep(
  rec: PursuitStepRecord,
  arrivals: Map<string, Arrival>,
): PursuitVerdict {
  const failures: string[] = [];
  const at = `step ${rec.step} (${rec.label})`;
  if (rec.died !== undefined) {
    failures.push(`${at}: died — the bot was killed on a pursued step by ${rec.died.attacker}`);
  }
  if (rec.staged > 0) {
    failures.push(
      `${at}: staged — ${rec.staged} body(ies) were staged away on a pursued step, where the ` +
        `pursuit rule turns staging off`,
    );
  }
  if (rec.restored > 0) {
    failures.push(
      `${at}: restored — health was restored or refunded ${rec.restored} time(s) on a pursued ` +
        `step, where the pursuit rule turns restoring off`,
    );
  }
  let closest: number | undefined;
  let displacement = 0;
  let lowest: number | undefined;
  for (const r of rec.readings) {
    lowest = lowest === undefined ? r.botHealth : Math.min(lowest, r.botHealth);
  }
  for (const p of rec.pursuers) {
    const mine = rec.readings.filter((r) => r.actor === p.actor);
    if (mine.length === 0) {
      failures.push(`${at}: unread — no census of \`${p.actor}\` was read while the step was walked`);
      continue;
    }
    let lost: PursuitReading | undefined;
    let last: Vec3Tuple | undefined;
    for (const r of mine) {
      if (!r.alive || r.feet === undefined) {
        // An undated pursuer may not have arrived yet; one that has, or a dated
        // one, is lost the moment it is not read standing.
        if (p.undated && !arrivals.has(p.actor)) continue;
        lost ??= r;
        continue;
      }
      if (!arrivals.has(p.actor)) arrivals.set(p.actor, { cell: r.feet, tick: r.tick });
      const d = distance(r.botFeet, r.feet);
      closest = closest === undefined ? d : Math.min(closest, d);
      if (last !== undefined) displacement += distance(last, r.feet);
      last = r.feet;
    }
    if (lost !== undefined) {
      failures.push(
        `${at}: pursuer_lost — \`${p.actor}\` was not read standing at game tick ${lost.tick}`,
      );
    }
    if (!arrivals.has(p.actor)) {
      failures.push(`${at}: unread — \`${p.actor}\` was never read standing while the step was walked`);
      continue;
    }
  }
  return {
    failures,
    measurement: {
      closestApproach: closest,
      displacement,
      lowestHealth: lowest,
      readings: rec.readings.length,
    },
  };
}

/** The pursuit binding over a run: what the leg judged, and over what. */
export interface PursuitBinding {
  /** Pursued steps walked. */
  readonly pursued: number;
  /** Exported steps walked — the denominator. */
  readonly walked: number;
  /** Distinct pursuers over the pursued steps. */
  readonly pursuers: number;
  /** Readings over the pursued steps. */
  readonly readings: number;
  /** The least bot–pursuer distance read over the run. */
  readonly closestApproach: number | undefined;
}

/** The binding over every pursued step's record and measurement. */
export function pursuitBinding(
  records: readonly PursuitStepRecord[],
  measurements: readonly PursuitMeasurement[],
  walked: number,
): PursuitBinding {
  const actors = new Set<string>();
  for (const r of records) for (const p of r.pursuers) actors.add(p.actor);
  let closest: number | undefined;
  for (const m of measurements) {
    if (m.closestApproach === undefined) continue;
    closest = closest === undefined ? m.closestApproach : Math.min(closest, m.closestApproach);
  }
  return {
    pursued: records.length,
    walked,
    pursuers: actors.size,
    readings: records.reduce((n, r) => n + r.readings.length, 0),
    closestApproach: closest,
  };
}

/**
 * The run's closing line: `[pursuit] <n> pursued step(s) of <N>, <m>
 * pursuer(s), <k> reading(s); closest approach <d>` — `<N>` the exported steps
 * walked, so a zero states its denominator.
 */
export function pursuitLine(b: PursuitBinding): string {
  return (
    `[pursuit] ${b.pursued} pursued step(s) of ${b.walked}, ${b.pursuers} pursuer(s), ` +
    `${b.readings} reading(s); closest approach ` +
    `${b.closestApproach === undefined ? "none" : `${fmt(b.closestApproach)} block(s)`}`
  );
}

// ---------------------------------------------------------------------------
// The actor census channel
// ---------------------------------------------------------------------------
//
//     [dw:actor <campaign-id> <actor-id> <seq> 1 <x> <y> <z>]
//     [dw:actor <campaign-id> <actor-id> <seq> 0]
//
// The compiler's `actor_census_<id>` speaks one line per call: the free body
// (wearing `dw_actor_<id>`, not the puppet's marker) standing with its feet ×100,
// or not standing. Same sigil, same anchored whole-line match as the wave census.

const ACTOR_CENSUS_RE =
  /^\[dw:actor ([a-z0-9]+(?:-[a-z0-9]+)*) (actor\/[a-z0-9]+(?:-[a-z0-9]+)*) (-?[0-9]+) (?:1 (-?[0-9]+) (-?[0-9]+) (-?[0-9]+)|(0))\]$/;

/** One actor census line, parsed. */
export interface ActorCensus {
  readonly campaignId: string;
  readonly actor: string;
  readonly seq: number;
  readonly alive: boolean;
  readonly feet?: Vec3Tuple;
}

/** Parse one chat line as an actor census line, or `undefined`. Whole-line, strict. */
export function parseActorCensus(line: string): ActorCensus | undefined {
  const m = ACTOR_CENSUS_RE.exec(line);
  if (!m) return undefined;
  if (m[7] === "0") {
    return { campaignId: m[1]!, actor: m[2]!, seq: Number(m[3]), alive: false };
  }
  return {
    campaignId: m[1]!,
    actor: m[2]!,
    seq: Number(m[3]),
    alive: true,
    feet: [Number(m[4]) / 100, Number(m[5]) / 100, Number(m[6]) / 100],
  };
}
