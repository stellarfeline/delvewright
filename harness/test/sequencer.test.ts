import { test } from "node:test";
import assert from "node:assert/strict";
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
} from "../src/critical-path.ts";
import {
  runSequence,
  StepExecutionError,
  StepOrderError,
  type StepExecutor,
  validateStepOrder,
} from "../src/sequencer.ts";
import { BotDeathError } from "../src/death.ts";

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
  kill(_step: KillStep): Promise<void> {
    return this.record("kill");
  }
  collect(_step: CollectStep): Promise<void> {
    return this.record("collect");
  }
  interact(_step: InteractStep): Promise<void> {
    return this.record("interact");
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
  objective: "obj/greet",
  npc: "npc/keeper",
  pos: [8, 65, 12],
  command: "/trigger dw.dlg_keeper set 2",
};
const reach: ReachStep = {
  action: "reach",
  objective: "obj/exit",
  anchor: "anchor/exit",
  pos: [8, 65, 24],
  radius: 2,
};
const kill: KillStep = {
  action: "kill",
  objective: "obj/guards",
  wave: "wave/guards",
  pos: [22, 65, 12],
  tag: "dw_wave_guards",
  count: 2,
};
const collect: CollectStep = {
  action: "collect",
  objective: "obj/hook",
  item: "minecraft:tripwire_hook",
  count: 1,
  pos: [44, 65, 20],
};
const interact: InteractStep = {
  action: "interact",
  objective: "obj/door",
  anchor: "anchor/door",
  pos: [2, 65, 12],
  command: "/trigger dw.i_door set 1",
  requiresItem: "minecraft:tripwire_hook",
};
const assertComplete: AssertCompleteStep = {
  action: "assert-complete",
  objective: "dw.campaign",
  value: 1,
};

function path(steps: Step[]): CriticalPath {
  return { version: "0.2.0", formatVersion: 2, campaignId: "hello-world", steps };
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

test("runSequence dispatches the v0.3 kill/collect/interact steps", async () => {
  const executor = new RecordingExecutor();
  await runSequence(
    path([selectClass, talkTo, kill, collect, interact, reach, assertComplete]),
    executor,
  );
  assert.deepEqual(executor.calls, [
    "select-class",
    "talk-to",
    "kill",
    "collect",
    "interact",
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

test("runSequence awaits transport after a transport-marked step (gap 8)", async () => {
  // A reach step carrying a transport marker; the sequencer must call
  // awaitTransport with its destination after dispatching the step.
  const reachWithTransport: ReachStep = {
    action: "reach",
    objective: "obj/exit",
    anchor: "anchor/exit",
    pos: [8, 65, 24],
    radius: 2,
    transport: [261, 65, 4],
  };
  const transports: Array<readonly [number, number, number]> = [];
  const executor = new (class extends RecordingExecutor {
    awaitTransport(dest: readonly [number, number, number]): Promise<void> {
      transports.push(dest);
      return Promise.resolve();
    }
  })();
  await runSequence(
    path([selectClass, reachWithTransport, assertComplete]),
    executor,
  );
  assert.deepEqual(transports, [[261, 65, 4]]);
});

test("runSequence does not await transport for a plain step", async () => {
  let awaited = 0;
  const executor = new (class extends RecordingExecutor {
    awaitTransport(): Promise<void> {
      awaited += 1;
      return Promise.resolve();
    }
  })();
  await runSequence(path([selectClass, reach, assertComplete]), executor);
  assert.equal(awaited, 0);
});

test("runSequence awaits a cutscene after a cutscene-marked step (gap 7)", async () => {
  const reachWithCutscene: ReachStep = {
    action: "reach",
    objective: "obj/altar",
    anchor: "anchor/altar",
    pos: [8, 65, 24],
    radius: 2,
    cutsceneSeconds: 6,
  };
  const durations: number[] = [];
  const executor = new (class extends RecordingExecutor {
    awaitCutscene(seconds: number): Promise<void> {
      durations.push(seconds);
      return Promise.resolve();
    }
  })();
  await runSequence(path([selectClass, reachWithCutscene, assertComplete]), executor);
  assert.deepEqual(durations, [6]);
});

test("runSequence does not await a cutscene for a plain step", async () => {
  let awaited = 0;
  const executor = new (class extends RecordingExecutor {
    awaitCutscene(): Promise<void> {
      awaited += 1;
      return Promise.resolve();
    }
  })();
  await runSequence(path([selectClass, reach, assertComplete]), executor);
  assert.equal(awaited, 0);
});

// A fake executor that dies (BotDeathError) on the first N attempts of `reach`, then
// succeeds. `recoverFromDeath` and the base `selectClass`/`reach` all append to the
// single shared `calls` timeline so the retry sequence is asserted end-to-end.
class DyingExecutor extends RecordingExecutor {
  private deathsLeft: number;

  constructor(deaths = 1) {
    super();
    this.deathsLeft = deaths;
  }

  override reach(step: ReachStep): Promise<void> {
    if (this.deathsLeft > 0) {
      this.deathsLeft -= 1;
      this.calls.push("reach");
      return Promise.reject(new BotDeathError([1, 2, 3], "delve-bot was slain by Zombie"));
    }
    return super.reach(step);
  }

  recoverFromDeath(): Promise<void> {
    this.calls.push("recover");
    return Promise.resolve();
  }

  rearmAfterRespawn(): Promise<boolean> {
    this.calls.push("rearm");
    return Promise.resolve(true);
  }
}

test("retryOnDeath recovers, re-arms where the bot stands, and retries the dead step once", async () => {
  const executor = new DyingExecutor(1);
  await runSequence(
    path([selectClass, reach, assertComplete]),
    executor,
    { retryOnDeath: true },
  );
  // reach dies → recover → re-arm → reach passes → assert-complete.
  assert.deepEqual(executor.calls, [
    "select-class",
    "reach", // died here
    "recover",
    "rearm", // kit back on, bot NOT moved
    "reach", // retried and passed
    "assert-complete",
  ]);
});

test("the death retry never re-selects the class — that would teleport the bot away", async () => {
  // task #120. `class_apply_<class>` ends in `teleport @s <campaign entry point>`
  // and the `dw.class` trigger is re-enabled for every player on every tick, so a
  // second `/trigger dw.class` after a death silently warps the bot back to the
  // start of the delve. The whole retry path — and every die-retry measurement
  // taken through it — is about where the player RESPAWNS, so nothing on it may
  // move the bot.
  const executor = new DyingExecutor(1);
  await runSequence(path([selectClass, reach, assertComplete]), executor, {
    retryOnDeath: true,
  });
  assert.equal(
    executor.calls.filter((c) => c === "select-class").length,
    1,
    "the class is selected once, at the start of the run, and never again",
  );
});

test("default (fail-fast) surfaces a death as a StepExecutionError without retrying", async () => {
  const executor = new DyingExecutor(1);
  await assert.rejects(
    () => runSequence(path([selectClass, reach, assertComplete]), executor),
    (err: unknown) =>
      err instanceof StepExecutionError &&
      err.index === 1 &&
      err.cause instanceof BotDeathError,
  );
  // No recovery attempted; the run aborted at the death.
  assert.deepEqual(executor.calls, ["select-class", "reach"]);
});

test("retryOnDeath retries at most once — a second death fails the run", async () => {
  const executor = new DyingExecutor(2); // dies on both attempts
  await assert.rejects(
    () => runSequence(path([selectClass, reach, assertComplete]), executor, {
      retryOnDeath: true,
    }),
    (err: unknown) =>
      err instanceof StepExecutionError && err.cause instanceof BotDeathError,
  );
  // Recovery ran exactly once (the single allowed retry), then the second death failed.
  assert.equal(executor.calls.filter((c) => c === "recover").length, 1);
});

// --- endgame discipline (AUDIT-P0) --------------------------------------------

test("runSequence checks the endgame after every step that still has objectives ahead", async () => {
  // The check must not fire on the last objective step (campaign completion is DUE
  // there) nor on assert-complete, but must fire on every earlier step.
  const checked: Array<[number, number]> = [];
  const executor = new (class extends RecordingExecutor {
    assertEndgameNotReached(stepIndex: number, finalObjectiveIndex: number): void {
      checked.push([stepIndex, finalObjectiveIndex]);
    }
  })();
  // steps: 0 select-class, 1 talk-to, 2 kill, 3 reach (last objective), 4 assert
  await runSequence(
    path([selectClass, talkTo, kill, reach, assertComplete]),
    executor,
  );
  assert.deepEqual(checked, [
    [0, 3],
    [1, 3],
    [2, 3],
  ]);
});

test("an endgame violation fails the run at the step that revealed it", async () => {
  const executor = new (class extends RecordingExecutor {
    assertEndgameNotReached(stepIndex: number): void {
      if (stepIndex === 1) {
        throw new Error("campaign completed at step 1, but objective steps remain");
      }
    }
  })();
  await assert.rejects(
    () => runSequence(path([selectClass, talkTo, kill, reach, assertComplete]), executor),
    (err: unknown) =>
      err instanceof StepExecutionError &&
      err.index === 1 &&
      /campaign completed at step 1/.test(err.message),
  );
  // The run stops there: the hollow tail is never executed.
  assert.deepEqual(executor.calls, ["select-class", "talk-to"]);
});

test("runSequence announces each step index to the executor before dispatching it", async () => {
  const begun: number[] = [];
  const executor = new (class extends RecordingExecutor {
    beginStep(index: number): void {
      begun.push(index);
    }
  })();
  await runSequence(path([selectClass, talkTo, reach, assertComplete]), executor);
  assert.deepEqual(begun, [0, 1, 2, 3]);
});

// --- rest steps (compiler #220) ---------------------------------------------

const rest: RestStep = {
  action: "rest",
  bonfire: 1,
  anchor: "anchor/beach-fire",
  pos: [4, 64, 4],
  command: "/trigger dw.rest set 2",
};

test("a rest step is dispatched to the executor like any other", async () => {
  const executor = new (class extends RecordingExecutor {
    rest(_step: RestStep): Promise<void> {
      this.calls.push("rest");
      return Promise.resolve();
    }
  })();
  await runSequence(path([selectClass, rest, kill, assertComplete]), executor);
  assert.deepEqual(executor.calls, ["select-class", "rest", "kill", "assert-complete"]);
});

test("a path with rest steps against an executor that cannot rest fails loudly", async () => {
  // Never a silent skip: an unperformed rest leaves the checkpoint at world spawn
  // and every later proof runs against the wrong respawn point.
  const executor = new RecordingExecutor();
  await assert.rejects(
    () => runSequence(path([selectClass, rest, kill, assertComplete]), executor),
    (err: unknown) => err instanceof StepExecutionError && /cannot rest/.test(err.message),
  );
});

test("a rest step is never mistaken for the beat the campaign marker is due at", async () => {
  // A fire rested at just before the finale would otherwise become the
  // `finalObjectiveIndex`, and the real last objective's marker would trip the
  // endgame check on the step that legitimately completes the campaign.
  const checked: Array<[number, number]> = [];
  const executor = new (class extends RecordingExecutor {
    rest(_step: RestStep): Promise<void> {
      this.calls.push("rest");
      return Promise.resolve();
    }
    assertEndgameNotReached(stepIndex: number, finalObjectiveIndex: number): void {
      checked.push([stepIndex, finalObjectiveIndex]);
    }
  })();
  // steps: 0 select-class, 1 talk-to, 2 kill (last OBJECTIVE), 3 rest, 4 assert
  await runSequence(path([selectClass, talkTo, kill, rest, assertComplete]), executor);
  assert.deepEqual(
    checked,
    [
      [0, 2],
      [1, 2],
    ],
    "the last objective is the kill at index 2, not the rest at index 3",
  );
});
