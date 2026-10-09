// MineflayerExecutor: objective markers: observing them, awaiting them, the campaign's completion.

import type { AssertCompleteStep } from "../critical-path.ts";
import { CAMPAIGN_TOKEN, markerLine, parseCompletionMarker } from "../markers.ts";
import { delay, fmt } from "./connection.ts";
import { SCORE_POLL_MS } from "./score.ts";
import { swallowedTriggerVerdict } from "./trigger.ts";
import type { MineflayerExecutor } from "../executor.ts";

// (how long the bot may swing at one body before giving up on it is no longer a
// constant here: it is `bodies[].give_up_swings` in the combat plan, per kind,
// out of the encounter's own arithmetic. See `giveUpBudgetFor`.)
// (the wave-kill proximity rule and its constant now live in wave.ts, shared with the
// self-defense path — see the import above.)
/**
 * How long (ms) to wait for a scoreboard value to reach its target after a chat
 * command. The datapack acts on the trigger on the next server tick(s); give it a
 * generous window so slow CI servers don't flake.
 */
const SCORE_SETTLE_MS = 15_000;

/**
 * Margin (ms) added on top of a path's exported `ending_tail_ticks` when sizing
 * the completion window: the compiler schedules the ending's finale
 * — the-wake fires `campaign-complete` 250t into its closing `sequence` — and
 * exports that tail on the terminal step; the window must outlive the tail plus
 * server slack. See {@link completionWindowMs}.
 */
const ENDING_TAIL_MARGIN_MS = 10_000;

/** Minecraft server ticks per second (the tick → wall-clock conversion). */
const TICKS_PER_SECOND = 20;

/**
 * How long (ms) `assertComplete` may wait for the completion marker: the default
 * settle window, widened — never narrowed — by the path's exported
 * scheduled-ending tail (`ending_tail_ticks` + margin). Exported for its unit
 * test; pure arithmetic, no bot state.
 */
export function completionWindowMs(endingTailTicks: number | undefined): number {
  const tailMs = ((endingTailTicks ?? 0) * 1000) / TICKS_PER_SECOND;
  return Math.max(SCORE_SETTLE_MS, tailMs + ENDING_TAIL_MARGIN_MS);
}

/**
 * How long (ms) to wait for a step's OWN objective-completion marker after the bot
 * has done the thing the step asks for (AUDIT-P0). The datapack completes an
 * objective on the tick its condition holds and broadcasts the marker in the same
 * function, so the honest wait is a tick or two; the window is wide enough that a
 * loaded CI server, a lagging advancement or a wave countdown settling can never
 * flake, and short enough that a genuinely uncompletable objective fails the run
 * well inside the wall-clock budget. NOT a tolerance: on expiry the step FAILS.
 */
export const OBJECTIVE_TIMEOUT_MS = 30_000;

/**
 * Settle (ms) after an objective's marker before the next step runs, so effects the
 * objective fires (open a gate, give an item, move an NPC) have landed. The marker
 * is broadcast as the score flips — deliberately, so completion timing is exact —
 * which means the effects that follow it in the same function may not have applied
 * yet.
 */
export const EFFECT_SETTLE_MS = 1_000;

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  /**
   * Record an anchored completion marker (AUDIT-P0). Exact whole-line parse, scoped
   * to this run's campaign; everything else on the chat stream is ignored, including
   * lines that merely mention completion. First arrival wins — a re-broadcast must
   * not relabel when an objective actually completed.
   */
  observeMarker(this: MineflayerExecutor, message: string): void {
    const marker = parseCompletionMarker(message);
    if (!marker || marker.campaignId !== this.campaignId) return;
    if (this.repaintWatch) {
      // Where the bot stood decides which chunks of a repaint it was owed.
      const p = this.bot?.entity?.position;
      this.repaintWatch.marker(marker.token, Date.now(), p ? [p.x, p.y, p.z] : undefined);
    }
    if (marker.token === CAMPAIGN_TOKEN) {
      this.campaignCompleteAtStep ??= this.currentStep;
      return;
    }
    this.markerArrivals.set(marker.token, (this.markerArrivals.get(marker.token) ?? 0) + 1);
    if (!this.completedObjectives.has(marker.token)) {
      this.completedObjectives.set(marker.token, this.currentStep);
    }
  },

  /**
   * Wait until `objectiveId`'s own anchored completion marker has arrived — the
   * ONLY evidence that a step's objective completed. Arriving somewhere, opening a
   * dialogue or emptying a chest are means, never proof; a step whose marker never
   * comes fails loudly with the bot's position and what the delve did broadcast.
   * Death-aware and bounded.
   *
   * Public so it is directly unit-testable: it is the executor's whole success
   * criterion for a step, and testing it through `reach`/`collect` would need a live
   * pathfinder and a real chest.
   */
  async requireObjective(this: MineflayerExecutor, objectiveId: string, label: string): Promise<void> {
    await this.awaitObjectiveMarker(objectiveId, label);
  },

  /** The completion wait itself — see {@link requireObjective}. */
  async awaitObjectiveMarker(this: MineflayerExecutor, objectiveId: string, label: string): Promise<void> {
    const alreadyDone = this.completedObjectives.get(objectiveId);
    if (alreadyDone !== undefined && alreadyDone < this.currentStep) {
      // Not a failure — the objective did complete — but the path claims THIS step
      // proves it, so the ordering is worth surfacing in the run log.
      process.stderr.write(
        `[oracle] ${objectiveId} completed during step ${alreadyDone}, before its own ` +
          `step ${this.currentStep} (${label})\n`,
      );
      return;
    }
    const arrived = await this.waitFor(
      () => this.completedObjectives.has(objectiveId),
      OBJECTIVE_TIMEOUT_MS,
      SCORE_POLL_MS,
    );
    if (arrived) return;
    const seen = [...this.completedObjectives.keys()];
    throw new Error(
      `${label}: objective ${objectiveId} did not complete within ` +
        `${OBJECTIVE_TIMEOUT_MS}ms — no \`${markerLine(this.campaignId ?? "?", objectiveId)}\` ` +
        `marker arrived; bot at ${fmt(this.requireBot().entity.position)}; objectives ` +
        `completed so far: ${seen.join(", ") || "none"}${swallowedTriggerVerdict(this.trigger)}`,
    );
  },

  /**
   * Endgame discipline (AUDIT-P0). Called by the sequencer after every step that
   * still has an objective step ahead of it: campaign completion belongs to the LAST
   * objective step, so its marker arriving any earlier proves the path is incoherent
   * — the remaining steps cannot be doing anything the campaign still needs. Fail
   * here, at the step that revealed it, rather than reporting a green run whose tail
   * was hollow.
   */
  assertEndgameNotReached(this: MineflayerExecutor, stepIndex: number, finalObjectiveIndex: number): void {
    if (this.campaignCompleteAtStep === undefined) return;
    throw new Error(
      `campaign completed at step ${this.campaignCompleteAtStep}, but the critical path ` +
        `runs objective steps through step ${finalObjectiveIndex} (detected after step ` +
        `${stepIndex}) — every later step is hollow. The path and the delve's completion ` +
        `condition disagree; fix the campaign or the path, never the check`,
    );
  },

  async assertComplete(this: MineflayerExecutor, step: AssertCompleteStep): Promise<void> {
    const bot = this.requireBot();
    // Completion is observed two ways, whichever surfaces first:
    //   1. The anchored campaign-completion marker (the working path on 1.21.11 —
    //      see markers.ts), buffered since connect.
    //   2. The sidebar score via mineflayer (future-proof: works if/when mineflayer
    //      gains 1.21.11 score-packet support; currently always unset).
    // The campaign completes during the LAST objective step; the sequencer has
    // already failed the run if the marker arrived any earlier than that
    // (assertEndgameNotReached), so reaching here means it is either due now or —
    // when the path exports a scheduled-ending tail (`ending_tail_ticks`: the-wake
    // fires `campaign-complete` 250t into its closing `sequence`) — due within
    // that tail. The window covers whichever is longer.
    const windowMs = completionWindowMs(step.endingTailTicks);
    const deadline = Date.now() + windowMs;
    while (Date.now() < deadline) {
      if (this.death) throw this.death;
      if (this.campaignCompleteAtStep !== undefined) {
        return;
      }
      const board = bot.scoreboards[step.objective];
      if (board?.itemsMap[bot.username]?.value === step.value) {
        return;
      }
      await delay(SCORE_POLL_MS);
    }
    const board = bot.scoreboards[step.objective];
    const sidebar = board?.itemsMap[bot.username]?.value ?? "unset";
    const done = [...this.completedObjectives.keys()];
    throw new Error(
      `campaign not complete after ${windowMs}ms: no ` +
        `\`${markerLine(this.campaignId ?? "?", CAMPAIGN_TOKEN)}\` marker arrived ` +
        `(objective ${step.objective} expected ${step.value}; sidebar: ${sidebar}); ` +
        `objectives completed: ${done.join(", ") || "none"}`,
    );
  },
};
