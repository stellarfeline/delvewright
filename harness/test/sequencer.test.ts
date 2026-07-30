import { test } from "node:test";
import assert from "node:assert/strict";
import type {
  AssertCompleteStep,
  CriticalPath,
  ReachStep,
  SelectClassStep,
  Step,
  TalkToStep,
} from "../src/critical-path.ts";
import {
  runSequence,
  StepExecutionError,
  StepOrderError,
  type StepExecutor,
  validateStepOrder,
} from "../src/sequencer.ts";

// A fake executor that records the actions it was asked to perform, and can be
// primed to fail on a specific action. No server involved.
class RecordingExecutor implements StepExecutor {
  readonly calls: string[] = [];
  readonly failOn: Step["action"] | undefined;

  constructor(failOn?: Step["action"]) {
    this.failOn = failOn;
  }

  private async record(action: Step["action"]): Promise<void> {
    this.calls.push(action);
    if (this.failOn === action) {
      throw new Error(`boom on ${action}`);
    }
  }

  selectClass(_step: SelectClassStep): Promise<void> {
    return this.record("select-class");
  }
  talkTo(_step: TalkToStep): Promise<void> {
    return this.record("talk-to");
  }
  reach(_step: ReachStep): Promise<void> {
    return this.record("reach");
  }
  assertComplete(_step: AssertCompleteStep): Promise<void> {
    return this.record("assert-complete");
  }
}

const selectClass: SelectClassStep = {
  action: "select-class",
  class: "class/wanderer",
  command: "/trigger dw.class set 1",
};
const talkTo: TalkToStep = {
  action: "talk-to",
  npc: "npc/keeper",
  pos: [8, 65, 12],
  command: "/trigger dw.dlg_keeper set 2",
};
const reach: ReachStep = {
  action: "reach",
  anchor: "anchor/exit",
  pos: [8, 65, 24],
  radius: 2,
};
const assertComplete: AssertCompleteStep = {
  action: "assert-complete",
  objective: "dw.campaign",
  value: 1,
};

function path(steps: Step[]): CriticalPath {
  return { version: "0.2.0", campaignId: "hello-world", steps };
}

test("validateStepOrder accepts the canonical order", () => {
  assert.doesNotThrow(() =>
    validateStepOrder([selectClass, talkTo, reach, assertComplete]),
  );
});

test("validateStepOrder accepts the minimal select-then-assert path", () => {
  assert.doesNotThrow(() => validateStepOrder([selectClass, assertComplete]));
});

test("validateStepOrder rejects an empty sequence", () => {
  assert.throws(
    () => validateStepOrder([]),
    (err: unknown) => err instanceof StepOrderError && /no steps/.test(err.message),
  );
});

test("validateStepOrder requires select-class first", () => {
  assert.throws(
    () => validateStepOrder([reach, selectClass, assertComplete]),
    (err: unknown) =>
      err instanceof StepOrderError && /first step must be select-class/.test(err.message),
  );
});

test("validateStepOrder requires assert-complete last", () => {
  assert.throws(
    () => validateStepOrder([selectClass, reach, assertComplete, talkTo]),
    (err: unknown) =>
      err instanceof StepOrderError &&
      /last step must be assert-complete, found talk-to/.test(err.message),
  );
});

test("validateStepOrder rejects two select-class steps", () => {
  assert.throws(
    () => validateStepOrder([selectClass, selectClass, assertComplete]),
    (err: unknown) =>
      err instanceof StepOrderError &&
      /expected exactly one select-class step, found 2/.test(err.message),
  );
});

test("validateStepOrder rejects a missing assert-complete", () => {
  assert.throws(
    () => validateStepOrder([selectClass, reach]),
    (err: unknown) =>
      err instanceof StepOrderError &&
      /expected exactly one assert-complete step, found 0/.test(err.message),
  );
});

test("runSequence dispatches each step in order", async () => {
  const executor = new RecordingExecutor();
  await runSequence(path([selectClass, talkTo, reach, assertComplete]), executor);
  assert.deepEqual(executor.calls, [
    "select-class",
    "talk-to",
    "reach",
    "assert-complete",
  ]);
});

test("runSequence wraps a failing step in StepExecutionError naming the step", async () => {
  const executor = new RecordingExecutor("reach");
  await assert.rejects(
    () => runSequence(path([selectClass, talkTo, reach, assertComplete]), executor),
    (err: unknown) =>
      err instanceof StepExecutionError &&
      err.index === 2 &&
      /step 2 \(reach\) failed: boom on reach/.test(err.message),
  );
  // It aborts on failure — assert-complete is never attempted.
  assert.deepEqual(executor.calls, ["select-class", "talk-to", "reach"]);
});

test("runSequence surfaces an ordering error before executing anything", async () => {
  const executor = new RecordingExecutor();
  await assert.rejects(
    () => runSequence(path([talkTo, selectClass, assertComplete]), executor),
    (err: unknown) => err instanceof StepOrderError,
  );
  assert.deepEqual(executor.calls, []);
});
