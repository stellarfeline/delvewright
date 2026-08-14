// Step sequencer: enforces the critical-path ordering invariants and drives an
// executor through the steps. The sequencer is transport-agnostic — it talks to a
// StepExecutor interface, so it is fully unit-testable with a fake executor and no
// running server.

import type {
  AssertCompleteStep,
  CollectStep,
  CriticalPath,
  InteractStep,
  KillStep,
  ReachStep,
  RestStep,
  SelectClassStep,
  Step,
  TalkToStep,
} from "./critical-path.ts";
import { BotDeathError } from "./death.ts";

/**
 * One method per critical-path action. Implementations perform the actual
 * interaction (mineflayer against a live server, or a fake in tests). A rejected
 * promise means the step failed and aborts the run.
 */
export interface StepExecutor {
  selectClass(step: SelectClassStep): Promise<void>;
  talkTo(step: TalkToStep): Promise<void>;
  reach(step: ReachStep): Promise<void>;
  kill(step: KillStep): Promise<void>;
  collect(step: CollectStep): Promise<void>;
  interact(step: InteractStep): Promise<void>;
  /** Rest at a bonfire: click the affordance, then run the button's
   * command. Proves no objective — it performs the loop later steps are proven
   * under. Optional so existing fakes keep compiling; a path carrying a `rest`
   * step against an executor without it is a hard failure, never a silent skip. */
  rest?(step: RestStep): Promise<void>;
  assertComplete(step: AssertCompleteStep): Promise<void>;
  /**
   * Optional (gap 8): after a step whose completion teleports the player to
   * another area (its `transport` marker), wait for the position discontinuity to
   * settle before the next step's pathfinding begins. Navigation plumbing only —
   * no game logic. Executors without cross-area transport (and test fakes) may
   * omit it; the sequencer skips the wait when it is absent.
   */
  awaitTransport?(dest: readonly [number, number, number]): Promise<void>;
  /**
   * Optional (spec-0008 gap-7): after a step marked `cutscene_seconds`, the bot may
   * be forced into spectator and flown for ~n seconds. Pause until control returns
   * (gamemode back to adventure AND position stabilised), bounded. Navigation
   * plumbing only — no assertions about the cutscene itself.
   */
  awaitCutscene?(seconds: number): Promise<void>;
  /**
   * Optional (spec-0008 gap-7, retry path): after a {@link BotDeathError}, ready the
   * bot to resume — wait for respawn and clear the death latch. Called by the
   * sequencer only when `retryOnDeath` is enabled.
   */
  recoverFromDeath?(): Promise<void>;
  /**
   * Optional: put the kit back on after a respawn, WITHOUT moving the
   * bot. Paired with {@link recoverFromDeath}.
   *
   * The sequencer used to re-run `select-class` here, on the premise that a respawn
   * drops class state. It does not — the delve seals `gamerule keep_inventory true`
   * and class state lives in scoreboard values and tags — while the class trigger's
   * `class_apply_<class>` ends in `teleport @s <campaign entry point>`. So the
   * "re-arm" silently teleported the bot to the start of the delve after every
   * death, and whatever the retried step then measured, it was not measuring the
   * respawn the player got.
   */
  rearmAfterRespawn?(): Promise<unknown>;
  /**
   * Optional (AUDIT-P0): the run is about to execute step `index`. Attribution only
   * — the executor uses it to record which step a completion marker arrived during.
   */
  beginStep?(index: number): void;
  /**
   * Optional (AUDIT-P0): assert the campaign has NOT completed yet. Called after
   * every step that still has an objective step ahead of it. Campaign completion
   * belongs to the last objective step; arriving earlier means the remaining steps
   * are hollow, so the executor throws and the run fails at the step that revealed
   * it. Executors without a completion channel (test fakes) may omit it.
   */
  assertEndgameNotReached?(stepIndex: number, finalObjectiveIndex: number): void;
}

/** Options controlling how {@link runSequence} handles failures. */
export interface RunOptions {
  /**
   * Opt-in single retry after a bot death (spec-0008): recover the bot, put its
   * kit back on where it stands, and retry the failed step once.
   * Default (fail-fast) surfaces the death diagnostic immediately. Intended for
   * future lethal-delve validation; the safe-route ladder stays fail-fast.
   */
  readonly retryOnDeath?: boolean;
}

/** Raised when the step sequence violates a structural ordering invariant. */
export class StepOrderError extends Error {
  override readonly name = "StepOrderError";
}

/** Raised when a step's executor call fails; wraps the underlying cause. */
export class StepExecutionError extends Error {
  override readonly name = "StepExecutionError";
  /** 0-based index of the failing step in `steps`. */
  readonly index: number;

  constructor(index: number, action: string, cause: unknown) {
    const detail = cause instanceof Error ? cause.message : String(cause);
    super(`step ${index} (${action}) failed: ${detail}`);
    this.index = index;
    this.cause = cause;
  }
}

/**
 * Validate the critical-path ordering invariants (spec-0002: one step per
 * objective in a valid topological order):
 *   - exactly one `select-class`, and it is the first step;
 *   - exactly one `assert-complete`, and it is the last step.
 * `talk-to` / `reach` steps occupy the middle. Throws {@link StepOrderError} on
 * the first violation.
 */
export function validateStepOrder(steps: readonly Step[]): void {
  if (steps.length === 0) {
    throw new StepOrderError("critical path has no steps");
  }

  const selectClassCount = steps.filter((s) => s.action === "select-class").length;
  if (selectClassCount !== 1) {
    throw new StepOrderError(
      `expected exactly one select-class step, found ${selectClassCount}`,
    );
  }

  const assertCompleteCount = steps.filter(
    (s) => s.action === "assert-complete",
  ).length;
  if (assertCompleteCount !== 1) {
    throw new StepOrderError(
      `expected exactly one assert-complete step, found ${assertCompleteCount}`,
    );
  }

  const first = steps[0]!;
  if (first.action !== "select-class") {
    throw new StepOrderError(
      `first step must be select-class, found ${first.action}`,
    );
  }

  const last = steps[steps.length - 1]!;
  if (last.action !== "assert-complete") {
    throw new StepOrderError(
      `last step must be assert-complete, found ${last.action}`,
    );
  }
}

async function dispatch(executor: StepExecutor, step: Step): Promise<void> {
  switch (step.action) {
    case "select-class":
      return executor.selectClass(step);
    case "talk-to":
      return executor.talkTo(step);
    case "reach":
      return executor.reach(step);
    case "kill":
      return executor.kill(step);
    case "collect":
      return executor.collect(step);
    case "interact":
      return executor.interact(step);
    case "rest":
      if (!executor.rest) {
        throw new Error(
          `critical path carries a rest step at bonfire ${step.bonfire} but this executor ` +
            `cannot rest — the checkpoint would never move and every later proof would ` +
            `run against the wrong respawn point`,
        );
      }
      return executor.rest(step);
    case "assert-complete":
      return executor.assertComplete(step);
  }
}

/**
 * Validate ordering, then execute each step in order against `executor`. Resolves
 * when the terminal assert-complete step succeeds. Rejects with
 * {@link StepOrderError} for a bad sequence or {@link StepExecutionError} for a
 * failing step (naming the failed step, per the spec-0003 diagnostic requirement).
 */
export async function runSequence(
  path: CriticalPath,
  executor: StepExecutor,
  options: RunOptions = {},
): Promise<void> {
  validateStepOrder(path.steps);
  let deathRetryUsed = false;
  // The last step that stands for an objective — `assert-complete` is terminal and
  // proves nothing itself, so it is the step before it (validateStepOrder has
  // already guaranteed exactly one assert-complete, last). Campaign completion is
  // due at this step and nowhere earlier.
  // …and a `rest` step stands for no objective at all, so a fire
  // rested at just before the finale must not be mistaken for the beat the campaign
  // marker is due at.
  const finalObjectiveIndex = (() => {
    for (let i = path.steps.length - 2; i >= 0; i--) {
      if (path.steps[i]!.action !== "rest") return i;
    }
    return path.steps.length - 2;
  })();

  for (let i = 0; i < path.steps.length; i++) {
    const step = path.steps[i]!;
    executor.beginStep?.(i);
    // Retry loop: at most one re-attempt, and only after a bot death when opted in.
    for (;;) {
      try {
        await dispatch(executor, step);
        // Endgame discipline (AUDIT-P0): the campaign must not already be complete
        // while objective steps remain. Checked before the transport/cutscene waits
        // so an incoherent path fails at the step that revealed it, not later.
        if (i < finalObjectiveIndex) {
          executor.assertEndgameNotReached?.(i, finalObjectiveIndex);
        }
        // gap 8: if completing this step teleports the player across areas, wait for
        // the jump to land before the next step starts pathfinding. Attributed to
        // this step so a failed settle surfaces here, not as a next-step path error.
        const transport = "transport" in step ? step.transport : undefined;
        if (transport && executor.awaitTransport) {
          await executor.awaitTransport(transport);
        }
        // gap 7: if this step triggers a cutscene, wait for control to return before
        // the next step runs (else the next path starts while the bot is in spectator).
        const cutscene = "cutsceneSeconds" in step ? step.cutsceneSeconds : undefined;
        if (cutscene !== undefined && executor.awaitCutscene) {
          await executor.awaitCutscene(cutscene);
        }
        break;
      } catch (cause) {
        if (
          options.retryOnDeath &&
          !deathRetryUsed &&
          cause instanceof BotDeathError &&
          executor.recoverFromDeath
        ) {
          deathRetryUsed = true;
          try {
            await executor.recoverFromDeath();
            // Re-arm WHERE THE BOT STANDS. Re-running `select-class` here would
            // teleport it to the campaign entry point, so the retried
            // step would be walked from the start of the delve rather than from
            // the respawn the player actually got.
            await executor.rearmAfterRespawn?.();
          } catch (recoverCause) {
            throw new StepExecutionError(i, step.action, recoverCause);
          }
          continue; // retry the same step once
        }
        throw new StepExecutionError(i, step.action, cause);
      }
    }
  }
}
