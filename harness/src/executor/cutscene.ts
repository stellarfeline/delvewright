// MineflayerExecutor: control of the body: a cutscene or scripted hold taking it, and waiting it out.

import { CONTROLLED_GAMEMODE } from "../combat.ts";
import { delay, fmt } from "./connection.ts";
import type { MineflayerExecutor } from "../executor.ts";

/**
 * gap 7 (cutscene): how long (ms) the bot's position must hold steady, once it is
 * back in adventure mode, before control counts as restored; how far (blocks) a
 * position may drift and still count as "steady"; and the grace added on top of the
 * declared cutscene length before the wait gives up and continues (bounded so a
 * cutscene glitch cannot hang the run). Grace is env-tunable for tests.
 */
const CUTSCENE_SETTLE_MS = 500;

const CUTSCENE_STEADY_EPS = 0.05;

export const CUTSCENE_POLL_MS = 250;

/** A walk abandoned because a cutscene took the bot's body (gamemode left adventure). */
export class ControlTakenError extends Error {
  readonly mode: string | undefined;
  constructor(mode: string | undefined) {
    super(`control taken (gamemode \`${mode ?? "?"}\`)`);
    this.mode = mode;
  }
}

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  /**
   * Pathfind to a single {@link GoalSpec} (get within `spec.range` blocks of it),
   * with the death-aware, timed, one-retry behavior the critical path depends on: a
   * bot death rethrows the {@link BotDeathError} immediately (never retried across
   * the void); a transient failure is retried once after a settle (an `open-gate`
   * fill may land after the first path computation started); a persistent failure
   * throws a diagnostic naming the goal and the bot's position. The caller sets the
   * Movements and think budget once for the whole leg.
   *
   * Arrival is VERIFIED, not trusted: mineflayer's `goto` can resolve on a
   * best-effort partial path without the bot actually reaching the goal (observed on
   * an unwalkable waypoint hop — it "succeeds" while the bot sits blocks away). A
   * resolve that leaves the bot outside the goal range is treated as a failure, so a
   * stuck hop fails the step loudly instead of silently marching the walk forward.
   */
  /** Whether a cutscene holds the bot's body: it is in a gamemode it does not walk in. */
  controlTaken(this: MineflayerExecutor): boolean {
    const mode = this.gameModeNow();
    return mode !== undefined && mode !== CONTROLLED_GAMEMODE;
  },

  /**
   * `p`, abandoned the moment a cutscene takes the body. Polls the gamemode at
   * {@link CUTSCENE_POLL_MS}; `p`'s own settlement after an abandonment lands on
   * the handler attached here.
   */
  raceControl<T>(this: MineflayerExecutor, p: Promise<T>): Promise<T> {
    return new Promise<T>((resolve, reject) => {
      const timer = setInterval(() => {
        if (this.controlTaken()) {
          clearInterval(timer);
          reject(new ControlTakenError(this.gameModeNow()));
        }
      }, CUTSCENE_POLL_MS);
      p.then(
        (v) => {
          clearInterval(timer);
          resolve(v);
        },
        (e: unknown) => {
          clearInterval(timer);
          reject(e);
        },
      );
    });
  },

  /**
   * Hold, mid-walk, until a cutscene gives the body back — bounded by the
   * step's own declaration, never by a number invented here: the larger of its
   * `cutscene_seconds` and `en_route_cutscene_seconds`, plus the grace
   * {@link awaitCutscene} uses. A step that declares neither is refused: the
   * plan did not account for a cutscene the route fired, and that is the
   * finding.
   */
  async awaitControlEnRoute(this: MineflayerExecutor, label: string): Promise<void> {
    const bot = this.requireBot();
    const mode = this.gameModeNow();
    const declared = this.stepCutsceneAllowanceS;
    if (declared === undefined) {
      throw new Error(
        `${label}: a cutscene took control (gamemode \`${mode ?? "?"}\`) at ` +
          `${fmt(bot.entity.position)} during step \`${this.stepLabel}\`, which declares no ` +
          `\`cutscene_seconds\` and no \`en_route_cutscene_seconds\` — the critical path ` +
          `did not account for a cutscene this step fires`,
      );
    }
    const budget = declared * 1000 + this.cutsceneGraceMs;
    const started = Date.now();
    process.stderr.write(
      `[cutscene] ${label}: control taken (gamemode \`${mode ?? "?"}\`) at ` +
        `${fmt(bot.entity.position)}; waiting up to ${declared}s declared + ` +
        `${this.cutsceneGraceMs}ms grace\n`,
    );
    while (Date.now() - started < budget) {
      if (this.death) throw this.death;
      if (!this.controlTaken()) {
        process.stderr.write(
          `[cutscene] ${label}: control returned after ${Date.now() - started}ms at ` +
            `${fmt(bot.entity.position)}; resuming the walk\n`,
        );
        await delay(CUTSCENE_SETTLE_MS);
        return;
      }
      await delay(CUTSCENE_POLL_MS);
    }
    throw new Error(
      `${label}: still \`${this.gameModeNow() ?? "?"}\` after ${budget}ms — longer than the ` +
        `${declared}s step \`${this.stepLabel}\` declares`,
    );
  },

  /**
   * A walk that starts with the body already held: wait for control up to the
   * step's declared allowance plus grace, then walk whatever the gamemode —
   * bounded, never a refusal, the convention {@link awaitCutscene} keeps. A step
   * that declares no cutscene does not wait; the hold is logged and the walk
   * made as before.
   */
  async awaitHeldAtStart(this: MineflayerExecutor, label: string): Promise<void> {
    const declared = this.stepCutsceneAllowanceS;
    const mode = this.gameModeNow();
    if (declared === undefined) {
      process.stderr.write(
        `[cutscene] ${label}: the walk starts with the body held (gamemode \`${mode ?? "?"}\`) ` +
          `during step \`${this.stepLabel}\`, which declares no cutscene — walking as before\n`,
      );
      return;
    }
    const budget = declared * 1000 + this.cutsceneGraceMs;
    const started = Date.now();
    process.stderr.write(
      `[cutscene] ${label}: the walk starts with the body held (gamemode \`${mode ?? "?"}\`); ` +
        `waiting up to ${declared}s declared + ${this.cutsceneGraceMs}ms grace\n`,
    );
    while (Date.now() - started < budget) {
      if (this.death) throw this.death;
      if (!this.controlTaken()) {
        process.stderr.write(
          `[cutscene] ${label}: control returned after ${Date.now() - started}ms; walking\n`,
        );
        await delay(CUTSCENE_SETTLE_MS);
        return;
      }
      await delay(CUTSCENE_POLL_MS);
    }
    process.stderr.write(
      `[cutscene] ${label}: still \`${this.gameModeNow() ?? "?"}\` after ${budget}ms — walking anyway\n`,
    );
  },

  /**
   * The gamemode the client currently believes it is in, as a plain string.
   *
   * Widened deliberately: mineflayer's own type for `bot.game.gameMode` lists
   * `survival | creative | spectator` and omits `adventure`, which is the mode every
   * delve actually runs in. Comparing against the mode a delve uses is not an
   * unintentional comparison; the library's enumeration is short.
   */
  gameModeNow(this: MineflayerExecutor): string | undefined {
    return this.bot?.game?.gameMode;
  },

  /**
   * gap 7 (cutscene): after a step marked `cutscene_seconds`, the compiler may force
   * the bot into spectator and dolly a camera for ~n seconds, then restore gamemode
   * and position. The harness makes no assertions about the cutscene — it only waits
   * for control to return so the next step does not start pathfinding mid-spectator.
   *
   * Two phases, both death-aware and bounded (deadline = n + grace, so a cutscene
   * glitch cannot hang the run):
   *   1. Sleep through the declared duration — the bot is out of our control anyway
   *      (this also covers a cutscene brief enough that we would otherwise miss the
   *      spectator window entirely).
   *   2. Extend the awaitTransport discontinuity pattern: wait for the gamemode to be
   *      back to adventure AND the position to hold steady for a short settle window.
   */
  async awaitCutscene(this: MineflayerExecutor, seconds: number): Promise<void> {
    const bot = this.requireBot();
    const start = Date.now();
    const minEnd = start + seconds * 1000;
    const deadline = minEnd + this.cutsceneGraceMs;

    // Phase 1: wait out the declared cutscene length (control is not ours meanwhile).
    while (Date.now() < minEnd) {
      if (this.death) throw this.death;
      await delay(CUTSCENE_POLL_MS);
    }

    // Phase 2: confirm control returned — adventure mode AND a settled position.
    let steadySince: number | undefined;
    let last = bot.entity.position.clone();
    while (Date.now() < deadline) {
      if (this.death) throw this.death;
      const here = bot.entity.position;
      const moved = here.distanceTo(last) > CUTSCENE_STEADY_EPS;
      last = here.clone();
      if (bot.game.gameMode === "adventure" && !moved) {
        steadySince ??= Date.now();
        if (Date.now() - steadySince >= CUTSCENE_SETTLE_MS) return;
      } else {
        steadySince = undefined;
      }
      await delay(CUTSCENE_POLL_MS);
    }
    process.stderr.write(
      `[cutscene] control not confirmed restored within ${seconds}s + grace; ` +
        `gamemode ${bot.game.gameMode}, bot at ${fmt(bot.entity.position)} — continuing\n`,
    );
  },
};
