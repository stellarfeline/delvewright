// Step sequencer: enforces the critical-path ordering invariants and drives an
// executor through the steps. The sequencer is transport-agnostic — it talks to a
// StepExecutor interface, so it is fully unit-testable with a fake executor and no
// running server.

import type {
  AssertCompleteStep,
  CriticalPath,
  ReachStep,
  SelectClassStep,
  Step,
  TalkToStep,
} from "./critical-path.ts";

/**
 * One method per critical-path action. Implementations perform the actual
 * interaction (mineflayer against a live server, or a fake in tests). A rejected
 * promise means the step failed and aborts the run.
 */
export interface StepExecutor {
  selectClass(step: SelectClassStep): Promise<void>;
  talkTo(step: TalkToStep): Promise<void>;
  reach(step: ReachStep): Promise<void>;
  assertComplete(step: AssertCompleteStep): Promise<void>;
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
): Promise<void> {
  validateStepOrder(path.steps);

  for (let i = 0; i < path.steps.length; i++) {
    const step = path.steps[i]!;
    try {
      await dispatch(executor, step);
    } catch (cause) {
      throw new StepExecutionError(i, step.action, cause);
    }
  }
}
