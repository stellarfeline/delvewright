// MineflayerExecutor: walking a pursued step (spec-0132 §5) — the readings of
// every pursuer while the step runs, the staging and restoring the pursuit rule
// turns off, and the verdict written before a death ends the run.

import type { Step, Vec3Tuple } from "../critical-path.ts";
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

  /** A body's width by kind, from the pinned registry the client was built for. */
  bodyWidth(this: MineflayerExecutor, kind: string): number {
    const name = kind.replace(/^minecraft:/, "");
    const registry = (this.bot as unknown as {
      registry?: { entitiesByName?: Record<string, { width?: number }> };
    })?.registry;
    return registry?.entitiesByName?.[name]?.width ?? 1;
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
    const verdict = judgePursuedStep(record, this.pursuitArrivals, (kind) => this.bodyWidth(kind));
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
