// MineflayerExecutor: walking a pursued step (spec-0132 §5) — the readings of
// every pursuer while the step runs, the staging and restoring the pursuit rule
// turns off, and the verdict written before a death ends the run.

import type { Step, Vec3Tuple } from "../critical-path.ts";
import { nearestIndex } from "../waypoints.ts";
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


/** The pursuit rule in force for the step being walked. */
export interface PursuedStep {
  readonly index: number;
  readonly label: string;
  /** The staging and restoring acts refused during the step. */
  readonly refusals: string[];
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

  /** Note that the current step walks the proven leg `leg`. */
  noteStepLeg(this: MineflayerExecutor, leg: number): void {
    this.stepLegs.add(leg);
  },

  /**
   * **Walk a pursued step again after a death on it** (spec-0132 §5.2), the
   * respawn already taken where the game put the bot. The step is walked from
   * there: the leg cursor goes back to where the step began, and its first leg
   * resumes from the proven waypoint nearest the bot. The bound guards
   * termination, never survival: a step may take at most as many deaths as its
   * proven legs have waypoints, and the death past that is a death loop that
   * fails the step by name. Whether a life got further than the last is not
   * asked — that is the vanilla will of the pursuer, the designer's call.
   */
  resumePursuedStep(this: MineflayerExecutor, index: number, label: string): void {
    const deaths = (this.pursuedDeaths.get(index) ?? 0) + 1;
    this.pursuedDeaths.set(index, deaths);
    const legs = [...this.stepLegs].sort((a, b) => a - b);
    const bound = legs.reduce((n, l) => n + (this.waypoints?.legs[l]?.waypoints.length ?? 0), 0);
    if (deaths > bound) {
      throw new Error(
        `step ${index} (${label}): death loop — death ${deaths} on this pursued step, past its bound ` +
          `of ${bound}: the proven waypoints of the leg(s) it walks (${legs.length === 0 ? "none" : legs.join(", ")})`,
      );
    }
    this.legCursor = this.stepStartCursor;
    const leg = this.waypoints?.legs[this.legCursor];
    const feet = this.feetCell();
    if (leg !== undefined && feet !== undefined) {
      this.legResume = { leg: this.legCursor, from: nearestIndex(leg.waypoints, feet) };
    }
    process.stderr.write(
      `[pursuit] step ${index} (${label}): death ${deaths} of at most ${bound}; respawned at ` +
        `[${feet?.join(", ") ?? "?"}], walking the step again` +
        (leg === undefined ? "" : ` from proven waypoint ${this.legResume?.from}`) +
        `\n`,
    );
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
      // spec-0132 §5.7: a warden's darkness is judged on the server's
      // positions — this reading — never on the client's entity table.
      if (row.kind.replace(/^minecraft:/, "") === "warden") {
        const f = census.feet;
        this.sculk.onWardenReading(
          row.actor,
          census.alive && f !== undefined && Number.isFinite(botFeet[0])
            ? Math.hypot(f[0] - botFeet[0], f[1] - botFeet[1], f[2] - botFeet[2])
            : undefined,
        );
      }
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
    this.pursuedStep = { index, label, refusals: [] };
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
