// MineflayerExecutor: walking a pursued step (spec-0132 §5) — the readings of
// every pursuer while the step runs, the staging and restoring the pursuit rule
// turns off, and the verdict written before a death ends the run.

import type { PursuerRow, Step, Vec3Tuple } from "../critical-path.ts";
import { pursuersOf } from "../critical-path.ts";
import { BotDeathError } from "../death.ts";
import {
  judgePursuedStep,
  parseActorCensus,
  type PursuitReading,
  type PursuitStepRecord,
} from "../pursuit.ts";
import { delay } from "./connection.ts";
import type { MineflayerExecutor } from "../executor.ts";

/** Game ticks between two readings: once a second on the server's clock. */
export const PURSUIT_READING_TICKS = 20;

/** Wall time (ms) between two looks at the server's clock while waiting for the next reading. */
const PURSUIT_CLOCK_POLL_MS = 250;

/** How long (ms) a named hit stays the explanation of a death that follows it. */
const DEATH_ATTRIBUTION_MS = 3_000;

/**
 * How long (ms, wall time) a wait for quiet goes without a new pursuit reading
 * before it fails: ten of the once-a-second readings missed in a row.
 */
const QUIET_READING_STALE_MS = 10_000;

/** One pursuer that hears and is not yet out of earshot of a move. */
export interface InEarshot {
  readonly actor: string;
  /** Its distance to the move, or `undefined` while it has no reading. */
  readonly distance: number | undefined;
  readonly reach: number;
}

/** The least distance from `p` to the segment `a`–`b`, all three axes. */
export function distanceToMove(p: Vec3Tuple, a: Vec3Tuple, b: Vec3Tuple): number {
  const d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]] as const;
  const len2 = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
  const t =
    len2 === 0
      ? 0
      : Math.max(0, Math.min(1, ((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1] + (p[2] - a[2]) * d[2]) / len2));
  return Math.hypot(p[0] - (a[0] + t * d[0]), p[1] - (a[1] + t * d[1]), p[2] - (a[2] + t * d[2]));
}

/**
 * **Which pursuers would hear a move made standing** (spec-0132 §5.2): every
 * row whose kind hears (`hearing`, from the compiler's `senses` registry) and
 * whose latest reading has the body standing within its reach of the move's
 * segment — or has no reading yet, which is not out of earshot. A body read
 * absent is not live and hears nothing. Pure.
 */
export function inEarshot(
  rows: readonly PursuerRow[],
  latest: ReadonlyMap<string, PursuitReading>,
  from: Vec3Tuple,
  to: Vec3Tuple,
): InEarshot[] {
  const out: InEarshot[] = [];
  for (const row of rows) {
    if (row.hearing === undefined) continue;
    const r = latest.get(row.actor);
    if (r === undefined) {
      out.push({ actor: row.actor, distance: undefined, reach: row.hearing.reach });
      continue;
    }
    if (!r.alive || r.feet === undefined) continue;
    const distance = distanceToMove(r.feet, from, to);
    if (distance <= row.hearing.reach) out.push({ actor: row.actor, distance, reach: row.hearing.reach });
  }
  return out;
}

/** The pursuit rule in force for the step being walked. */
export interface PursuedStep {
  readonly index: number;
  readonly label: string;
  /** The staging and restoring acts refused during the step. */
  readonly refusals: string[];
  /** The step's pursuers, as the compiler named them. */
  readonly rows: readonly PursuerRow[];
  /** Each pursuer's latest reading, kept by the step's reading loop. */
  readonly latest: Map<string, PursuitReading>;
}

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  /**
   * **Refuse a staging or restoring act on a pursued step, and record it.**
   * Returns `true` when the pursuit rule is in force (the caller returns
   * without acting); the refusal is written into the step's record, so the
   * run artifact shows what the rule held back.
   */
  pursuitRefuses(this: MineflayerExecutor, act: string): boolean {
    const p = this.pursuedStep;
    if (p === undefined) return false;
    p.refusals.push(act);
    process.stderr.write(`[pursuit] step ${p.index} (${p.label}): refused — ${act}\n`);
    return true;
  },

  /**
   * **Wait for quiet before a move made standing** (spec-0132 §5.2). On a
   * pursued step, a move the step cannot make crouched (a `stand_ups` hop:
   * a climb down, a step down, a jump) waits, the body crouched and still,
   * until every live pursuer that hears stands beyond its kind's reach of the
   * move — the way a player watches from above before going down. The wait is
   * judged on the step's own once-a-second readings and bounded by the
   * longest `forget_ticks` of the pursuers that hear: past it no anger a sound
   * earned remains, so a body still in earshot is not leaving, and waiting on
   * buys nothing. A spent bound fails the step, naming the move and every
   * pursuer still in earshot. Off a pursued step it returns at once.
   */
  async quietBeforeStanding(
    this: MineflayerExecutor,
    from: Vec3Tuple,
    to: Vec3Tuple,
    label: string,
  ): Promise<void> {
    const p = this.pursuedStep;
    if (p === undefined) return;
    const hearers = p.rows.filter((r) => r.hearing !== undefined);
    if (hearers.length === 0) return;
    const bound = Math.max(...hearers.map((r) => r.hearing!.forgetTicks));
    const tickOf = (): number | undefined => {
      let t: number | undefined;
      for (const r of p.latest.values()) t = t === undefined ? r.tick : Math.max(t, r.tick);
      return t;
    };
    const move = `[${from.map((v) => v.toFixed(1)).join(", ")}]→[${to.join(", ")}]`;
    let start: number | undefined;
    let lastTick: number | undefined;
    let lastNewAt = Date.now();
    for (;;) {
      const now = tickOf();
      if (now !== lastTick) {
        lastTick = now;
        lastNewAt = Date.now();
      }
      if (start === undefined && now !== undefined) start = now;
      const near = inEarshot(hearers, p.latest, from, to);
      const waited = start === undefined || now === undefined ? 0 : now - start;
      if (near.length === 0) {
        if (waited > 0) {
          process.stderr.write(
            `[pursuit] step ${p.index} (${p.label}): waited ${waited} tick(s) before the move ${move} ` +
              `made standing, until every pursuer that hears was out of earshot\n`,
          );
        }
        return;
      }
      const said = near
        .map((n) => `${n.actor} ${n.distance === undefined ? "unread" : `${n.distance.toFixed(1)} block(s)`} (reach ${n.reach})`)
        .join(", ");
      if (waited >= bound) {
        throw new Error(
          `${label}: the move ${move} is made standing and waited ${waited} tick(s), its bound — the ` +
            `longest a pursuer here takes to forget (${bound}) — with ${said} still in earshot`,
        );
      }
      if (Date.now() - lastNewAt > QUIET_READING_STALE_MS) {
        throw new Error(
          `${label}: waiting for quiet before the move ${move}, no pursuit reading arrived for ` +
            `${QUIET_READING_STALE_MS} ms (${said})`,
        );
      }
      await delay(PURSUIT_CLOCK_POLL_MS);
    }
  },

  /** One census of every pursuer, in one bracketed turn on the server's clock. */
  async readPursuers(this: MineflayerExecutor, step: Step): Promise<PursuitReading[]> {
    const rows = pursuersOf(step);
    const reading = await this.readGameTime("a pursuit reading", () =>
      rows.map((r) => `/function ${r.census}`),
    );
    const bot = this.bot;
    const p = bot?.entity?.position;
    const botFeet: Vec3Tuple = p ? [p.x, p.y, p.z] : [Number.NaN, Number.NaN, Number.NaN];
    const botHealth = bot?.health ?? 0;
    const out: PursuitReading[] = [];
    for (const row of rows) {
      const census = reading.lines
        .map((line) => {
          // The anchored line may be prefixed by the chat stream's own framing.
          const at = line.indexOf("[dw:actor ");
          return at < 0 ? undefined : parseActorCensus(line.slice(at));
        })
        .find((c) => c !== undefined && c.actor === row.actor && c.campaignId === this.campaignId);
      if (census === undefined) continue; // not answered: no reading, never a zero
      out.push({
        actor: row.actor,
        tick: reading.tick,
        alive: census.alive,
        ...(census.feet === undefined ? {} : { feet: census.feet }),
        botFeet,
        botHealth,
      });
    }
    return out;
  },

  /**
   * **Walk a pursued step** (spec-0132 §5.2): `act` is the step exactly as
   * every other is performed. While it runs every pursuer is read once a
   * second on the server's clock, staging and restoring are refused, and
   * after its completion one more reading is taken. The verdict is recorded
   * before a death ends the run, and the death is rethrown as on any walk.
   */
  async pursue(
    this: MineflayerExecutor,
    index: number,
    step: Step,
    act: () => Promise<void>,
  ): Promise<void> {
    const label = "objective" in step ? `${step.action} ${step.objective}` : step.action;
    const latest = new Map<string, PursuitReading>();
    this.pursuedStep = { index, label, refusals: [], rows: pursuersOf(step), latest };
    const stagedBefore = this.stagedRemovals.length;
    const readings: PursuitReading[] = [];
    let stop = false;
    const loop = (async () => {
      let last: number | undefined;
      while (!stop && this.death === undefined) {
        if (last !== undefined) {
          const now = await this.readGameTime("a pursuit reading's wait");
          if (now.tick < last + PURSUIT_READING_TICKS) {
            await delay(PURSUIT_CLOCK_POLL_MS);
            continue;
          }
        }
        if (stop) break;
        const got = await this.readPursuers(step);
        readings.push(...got);
        for (const r of got) latest.set(r.actor, r);
        last = got[0]?.tick ?? (await this.readGameTime("a pursuit reading's tick")).tick;
      }
    })().catch((err: unknown) => {
      process.stderr.write(
        `[pursuit] step ${index}: a reading failed — ${err instanceof Error ? err.message : String(err)}\n`,
      );
    });
    let cause: unknown;
    try {
      await act();
    } catch (err) {
      cause = err;
    }
    stop = true;
    await loop;
    // One more reading after the completion landed.
    if (cause === undefined && this.death === undefined) {
      try {
        readings.push(...(await this.readPursuers(step)));
      } catch (err) {
        process.stderr.write(
          `[pursuit] step ${index}: the reading after the completion failed — ` +
            `${err instanceof Error ? err.message : String(err)}\n`,
        );
      }
    }
    const added = this.stagedRemovals.slice(stagedBefore);
    const died = cause instanceof BotDeathError || this.death !== undefined;
    const hit = this.lastNamedHit;
    const attacker =
      hit !== undefined && Date.now() - hit.at < DEATH_ATTRIBUTION_MS
        ? `${this.bot?.entities[hit.id]?.name ?? "an unnamed body"}#${hit.id} (the server named it)`
        : `no body the server named${
            cause instanceof BotDeathError && cause.likelyCause ? ` — ${cause.likelyCause}` : ""
          }`;
    const record: PursuitStepRecord = {
      step: index,
      label,
      pursuers: pursuersOf(step),
      readings,
      ...(died ? { died: { attacker } } : {}),
      staged: added.filter((r) => r.kind !== "player").length,
      restored: added.filter((r) => r.kind === "player").length,
      refusals: [...this.pursuedStep.refusals],
    };
    this.pursuedStep = undefined;
    const verdict = judgePursuedStep(record, this.pursuitArrivals);
    this.pursuitRecords.push({ record, verdict });
    const m = verdict.measurement;
    process.stderr.write(
      `[pursuit] step ${index} (${label}): ${record.pursuers.map((p) => p.actor).join(", ")}; ` +
        `${m.readings} reading(s), closest approach ` +
        `${m.closestApproach === undefined ? "none" : m.closestApproach.toFixed(2)}, ` +
        `displacement ${m.displacement.toFixed(2)}, lowest health ` +
        `${m.lowestHealth === undefined ? "unread" : m.lowestHealth.toFixed(1)}; ` +
        `${verdict.failures.length === 0 ? "passed" : `RED: ${verdict.failures.join("; ")}`}\n`,
    );
    if (cause !== undefined) throw cause;
  },
};
