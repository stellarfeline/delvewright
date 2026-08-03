import { test } from "node:test";
import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import type { Bot } from "mineflayer";
import { MineflayerExecutor, type BotConfig } from "../src/executor.ts";
import { BotDeathError } from "../src/death.ts";
import type { AssertCompleteStep } from "../src/critical-path.ts";

// Minimal Vec3 stand-in with the methods the executor reads off bot.entity.position.
class FakeVec3 {
  readonly x: number;
  readonly y: number;
  readonly z: number;
  constructor(x: number, y: number, z: number) {
    this.x = x;
    this.y = y;
    this.z = z;
  }
  clone(): FakeVec3 {
    return new FakeVec3(this.x, this.y, this.z);
  }
  distanceTo(o: { x: number; y: number; z: number }): number {
    return Math.hypot(this.x - o.x, this.y - o.y, this.z - o.z);
  }
  offset(dx: number, dy: number, dz: number): FakeVec3 {
    return new FakeVec3(this.x + dx, this.y + dy, this.z + dz);
  }
}

// A fake mineflayer Bot: an EventEmitter with just the surface the executor touches
// (entity.position, game.gameMode, username, pathfinder.stop). Cast to Bot at the
// attach seam — tests may use structural fakes the full type can't express.
class FakeBot extends EventEmitter {
  username = "delve-bot";
  entity = { position: new FakeVec3(0, 64, 0), onGround: true };
  game = { gameMode: "adventure" as "adventure" | "spectator" };
  pathfinderStops = 0;
  /** Every pathfinder call, in order — pins that a stop is always followed by the
   * goal reset that consumes mineflayer-pathfinder's internal `stopPathing` flag. */
  pathfinderCalls: string[] = [];
  pathfinder = {
    stop: (): void => {
      this.pathfinderStops += 1;
      this.pathfinderCalls.push("stop");
    },
    setGoal: (goal: unknown): void => {
      this.pathfinderCalls.push(goal === null ? "setGoal(null)" : "setGoal");
    },
  };
  /** Whether `blockAt` returns a (loaded) block; false models an unloaded chunk. */
  chunkLoaded = true;
  loadPlugin(): void {}
  /** Mirrors mineflayer's `blockAt`: a stub block when loaded, `null` when not. */
  blockAt(): { name: string } | null {
    return this.chunkLoaded ? { name: "stone" } : null;
  }
}

const CONFIG: BotConfig = {
  host: "127.0.0.1",
  port: 25565,
  username: "delve-bot",
  version: "1.21.11",
  auth: "offline",
};

function attach(bot: FakeBot, env: Record<string, string | undefined> = {}): MineflayerExecutor {
  const executor = new MineflayerExecutor(CONFIG, env);
  executor.attachBot(bot as unknown as Bot);
  return executor;
}

test("a death event records position + likely cause and stops the pathfinder", () => {
  const bot = new FakeBot();
  bot.entity.position = new FakeVec3(12.4, 65, -3.6);
  const executor = attach(bot);
  // The death message arrives in chat, then the death event fires.
  bot.emit("messagestr", "delve-bot was slain by Zombie");
  bot.emit("death");

  const diag = executor.deathDiagnostic();
  assert.ok(diag instanceof BotDeathError);
  assert.deepEqual(diag.position, [12, 65, -4]); // rounded to whole blocks
  assert.equal(diag.likelyCause, "delve-bot was slain by Zombie");
  assert.equal(bot.pathfinderStops, 1); // in-flight pathfinding aborted
  // …and the stop is ALWAYS paired with a goal reset. mineflayer-pathfinder's `stop()`
  // only raises an internal flag that a later `goto` consumes by rejecting instantly
  // ("Path was stopped before it could be completed!") — leaving it raised poisons every
  // subsequent hop, which is how a the-drowned-bell run failed a leg it had walked
  // fine the run before. The reset consumes the flag here, once.
  assert.deepEqual(bot.pathfinderCalls, ["stop", "setGoal(null)"]);
});

test("a death fails an in-flight assert-complete fast with the death diagnostic", async () => {
  const bot = new FakeBot();
  const executor = attach(bot);
  const step: AssertCompleteStep = {
    action: "assert-complete",
    objective: "dw.campaign",
    value: 1,
  };
  // The objective never completes; without the death check this would poll for the
  // full settle window. The death makes it reject promptly instead.
  bot.emit("messagestr", "delve-bot fell from a high place");
  bot.emit("death");
  await assert.rejects(
    () => executor.assertComplete(step),
    (err: unknown) => err instanceof BotDeathError && /high place/.test(err.message),
  );
});

test("a respawn that lands before the wait is armed is still observed", async () => {
  // mineflayer auto-respawns within a few dozen ms; the caller only reaches
  // recoverFromDeath after polling the death latch. The old `once("spawn")` was
  // therefore armed AFTER the event it waited for and burned the full 15s
  // timeout — free before spec-0023, 15s per scripted death once the die-retry
  // stage runs (task #102, observed live on the keep-trial fixture).
  const bot = new FakeBot();
  const executor = attach(bot);
  bot.emit("death");
  bot.emit("spawn"); // the server respawned it before anyone was listening
  const started = Date.now();
  await executor.recoverFromDeath();
  assert.ok(
    Date.now() - started < 1_000,
    "the spawn counter cannot miss an event that already fired",
  );
  assert.equal(executor.deathDiagnostic(), undefined, "the death latch is cleared");
});

test("awaitCutscene waits out the cutscene and returns once control is restored", async () => {
  const bot = new FakeBot();
  bot.game.gameMode = "spectator";
  bot.entity.position = new FakeVec3(0, 100, 0); // flying camera
  const executor = attach(bot);
  // Restore adventure control shortly after the wait begins.
  setTimeout(() => {
    bot.game.gameMode = "adventure";
    bot.entity.position = new FakeVec3(8, 65, 8); // teleported back, then still
  }, 150);
  const startedInSpectator = bot.game.gameMode;
  await executor.awaitCutscene(0);
  assert.equal(startedInSpectator, "spectator");
  assert.equal(bot.game.gameMode, "adventure"); // control confirmed returned
});

test("awaitCutscene is bounded: it continues (does not hang) if control never returns", async () => {
  const bot = new FakeBot();
  bot.game.gameMode = "spectator"; // never restored
  // Small grace so the bounded give-up path resolves quickly.
  const executor = attach(bot, { DELVEWRIGHT_CUTSCENE_GRACE_MS: "150" });
  await executor.awaitCutscene(0); // resolves (logs + continues), never throws/hangs
  assert.equal(bot.game.gameMode, "spectator");
});

test("awaitCutscene aborts fast if the bot dies during the cutscene", async () => {
  const bot = new FakeBot();
  const executor = attach(bot);
  setTimeout(() => {
    bot.emit("messagestr", "delve-bot was slain by Warden");
    bot.emit("death");
  }, 50);
  await assert.rejects(
    () => executor.awaitCutscene(5), // would otherwise sleep ~5s
    (err: unknown) => err instanceof BotDeathError && /Warden/.test(err.message),
  );
});

// --- gap 8 (task #32): cross-area transport hardening ------------------------

test("awaitTransport waits for the position jump before returning", async () => {
  const bot = new FakeBot();
  bot.entity.position = new FakeVec3(6.5, 66, 4.3); // still in the old area
  const executor = attach(bot);
  let resolved = false;
  const done = executor.awaitTransport([260, 65, 4]).then(() => {
    resolved = true;
  });
  // Before the teleport lands, awaitTransport must NOT have returned.
  await new Promise((r) => setTimeout(r, 120));
  assert.equal(resolved, false);
  // The server teleport lands: mineflayer sets the position, then emits forcedMove.
  bot.entity.position = new FakeVec3(260.5, 65, 4.5);
  bot.emit("forcedMove");
  await done;
  assert.equal(resolved, true);
});

test("awaitTransport holds until the destination chunk is loaded (footing)", async () => {
  const bot = new FakeBot();
  // The jump has already landed near the destination, but its chunk is not yet
  // loaded — pathfinding now would fail instantly with "No path to the goal!".
  bot.entity.position = new FakeVec3(260.5, 65, 4.5);
  bot.entity.onGround = true;
  bot.chunkLoaded = false;
  const executor = attach(bot);
  let resolved = false;
  const done = executor.awaitTransport([260, 65, 4]).then(() => {
    resolved = true;
  });
  await new Promise((r) => setTimeout(r, 150));
  assert.equal(resolved, false); // still waiting for the chunk to load
  bot.chunkLoaded = true; // chunk finishes loading
  await done;
  assert.equal(resolved, true);
});

test("awaitTransport resets the pathfinder as the jump lands", async () => {
  const bot = new FakeBot();
  bot.entity.position = new FakeVec3(260.5, 65, 4.5); // already arrived
  const executor = attach(bot);
  await executor.awaitTransport([260, 65, 4]);
  assert.ok(bot.pathfinderStops >= 1); // stale cross-area path dropped
});

test("awaitTransport aborts fast if the bot dies mid-transport", async () => {
  const bot = new FakeBot();
  bot.entity.position = new FakeVec3(6.5, 66, 4.3); // never reaches the destination
  const executor = attach(bot);
  setTimeout(() => {
    bot.emit("messagestr", "delve-bot fell out of the world");
    bot.emit("death");
  }, 50);
  await assert.rejects(
    () => executor.awaitTransport([260, 65, 4]), // would otherwise wait ~15s
    (err: unknown) => err instanceof BotDeathError && /out of the world/.test(err.message),
  );
});

test("forcedMove resets the pathfinder only on a large cross-area jump", () => {
  const bot = new FakeBot();
  const executor = attach(bot);
  void executor;
  // First forced move (spawn): no previous reference, so no reset.
  bot.entity.position = new FakeVec3(5, 65, 2);
  bot.emit("forcedMove");
  assert.equal(bot.pathfinderStops, 0);
  // A small in-area nudge must not reset the path.
  bot.entity.position = new FakeVec3(7, 65, 3);
  bot.emit("forcedMove");
  assert.equal(bot.pathfinderStops, 0);
  // A ~256-block cross-area teleport resets the path exactly once.
  bot.entity.position = new FakeVec3(260, 65, 4);
  bot.emit("forcedMove");
  assert.equal(bot.pathfinderStops, 1);
});

// --- stall-recovery (task #45) ---------------------------------------------

import { isWaveMob, replayLegWithRecovery, type Unstick } from "../src/executor.ts";
import type { GoalSpec } from "../src/waypoints.ts";

// Record every goto the replay issues, and script per-hop outcomes.
function recorder(failFirstAt: (spec: GoalSpec, label: string) => boolean) {
  const calls: Array<{ spec: GoalSpec; label: string }> = [];
  const failedOnce = new Set<string>();
  const goto = async (spec: GoalSpec, label: string): Promise<void> => {
    calls.push({ spec, label });
    // A hop that should stall the first time it's attempted (and not a recovery).
    if (failFirstAt(spec, label) && !label.includes("recovery") && !failedOnce.has(label)) {
      failedOnce.add(label);
      throw new Error(`stall at ${label}`);
    }
  };
  return { calls, goto };
}

const G = (x: number, y: number, z: number, range = 1): GoalSpec => ({ x, y, z, range });

test("replayLegWithRecovery walks a clean leg with no recovery", async () => {
  const { calls, goto } = recorder(() => false);
  await replayLegWithRecovery([G(0, 65, 0), G(0, 65, 3), G(0, 65, 6, 3)], "npc x", goto);
  assert.equal(calls.length, 3); // one goto per goal, no recovery
  assert.ok(!calls.some((c) => c.label.includes("recovery")));
});

test("replayLegWithRecovery re-centers on the last proven cell then retries a stalled hop", async () => {
  // Stall on the SECOND hop (the wp8->wp9 pocket-wedge shape). Recovery must go to
  // the FIRST hop's exact cell at range 0, then the retry succeeds.
  const { calls, goto } = recorder((_spec, label) => label.includes("waypoint 2/3"));
  await replayLegWithRecovery([G(1, 65, -3), G(1, 65, 0), G(2, 66, 1, 3)], "npc perimedes", goto);
  // Sequence: hop1 ok, hop2 stall, recovery→[1,65,-3] range 0, hop2 retry ok, hop3 ok.
  const labels = calls.map((c) => c.label);
  assert.deepEqual(labels, [
    "npc perimedes waypoint 1/3",
    "npc perimedes waypoint 2/3",
    "npc perimedes waypoint 2/3 recovery to last proven cell",
    "npc perimedes waypoint 2/3",
    "npc perimedes",
  ]);
  const recovery = calls.find((c) => c.label.includes("recovery"))!;
  assert.deepEqual([recovery.spec.x, recovery.spec.y, recovery.spec.z], [1, 65, -3]);
  assert.equal(recovery.spec.range, 0, "recovery snaps to the exact proven cell");
});

test("replayLegWithRecovery rethrows a first-hop stall (nothing proven yet)", async () => {
  const { calls, goto } = recorder((_spec, label) => label.includes("waypoint 1/2"));
  await assert.rejects(
    () => replayLegWithRecovery([G(0, 65, 0), G(0, 65, 3, 3)], "npc x", goto),
    /stall at/,
  );
  // No recovery attempted for a first-hop stall.
  assert.ok(!calls.some((c) => c.label.includes("recovery")));
});

test("replayLegWithRecovery rethrows if the hop is still unwalkable after recovery", async () => {
  // A non-terminal hop that fails every real attempt (even after recovery) must
  // surface loudly — recovery is a best-effort nudge, never a way to pass a genuinely
  // unwalkable hop. Recovery gotos (range 0) succeed; the real hop keeps failing.
  const alwaysFail = async (_spec: GoalSpec, label: string): Promise<void> => {
    if (label.includes("waypoint 2/3") && !label.includes("recovery")) {
      throw new Error(`persistent stall at ${label}`);
    }
  };
  await assert.rejects(
    () => replayLegWithRecovery([G(0, 65, 0), G(3, 65, 0), G(6, 65, 0, 3)], "npc x", alwaysFail),
    /persistent stall/,
  );
});

test("replayLegWithRecovery escalates to a physics unstick when the recovery pathfind also stalls", async () => {
  // The wp9 pocket-wedge shape where the RECOVERY pathfind stalls too: both the hop
  // and the range-0 re-path fail until a physics unstick burst frees the bot.
  const unstickTargets: GoalSpec[] = [];
  let unstuck = false;
  const calls: string[] = [];
  const goto = async (_spec: GoalSpec, label: string): Promise<void> => {
    calls.push(label);
    const isHop2 = label.includes("waypoint 2/3");
    if (isHop2 && label.includes("recovery to last proven cell") && !unstuck) {
      throw new Error("recovery pathfind wedged too");
    }
    if (isHop2 && !label.includes("recovery") && !unstuck) {
      throw new Error("hop wedged");
    }
    // recovery-after-unstick and the retried hop succeed once unstuck
  };
  const unstick: Unstick = async (target) => {
    unstickTargets.push(target);
    unstuck = true; // one burst frees the bot
    return 1; // moved a block
  };
  await replayLegWithRecovery([G(1, 65, -3), G(1, 65, 0), G(2, 66, 1, 3)], "npc x", goto, unstick);
  assert.equal(unstickTargets.length, 1, "one physics-unstick burst was enough");
  const t = unstickTargets[0]!;
  // First burst aims at the GOAL (forward progress); the hop's goal is waypoint 2.
  assert.deepEqual([t.x, t.y, t.z], [1, 65, 0], "the first unstick burst aims at the goal");
  assert.ok(
    calls.some((l) => l.includes("retry after unstick")),
    "retries the actual next hop after the burst (not the strict proven cell)",
  );
});

test("replayLegWithRecovery unstick falls back to the proven cell after a zero-progress burst", async () => {
  // Goal-direction bursts are wall-blocked (0 progress, the concave pocket); the
  // adaptive aim must switch the NEXT burst toward the proven cell, which frees the
  // bot. Proves both directions are tried within the budget.
  const aims: GoalSpec[] = [];
  let freed = false;
  const goto = async (_spec: GoalSpec, label: string): Promise<void> => {
    const isHop2 = label.includes("waypoint 2/3");
    if (isHop2 && label.includes("recovery to last proven cell") && !freed) throw new Error("re-path wedged");
    if (isHop2 && !label.includes("recovery") && !freed) throw new Error("hop wedged");
  };
  const unstick: Unstick = async (target) => {
    aims.push(target);
    const towardGoal = target.z === 0; // goal is [1,65,0]; proven is [1,65,-3]
    if (!towardGoal) freed = true; // a proven-direction burst escapes the pocket
    return towardGoal ? 0 : 1; // goal-direction is wall-blocked → 0 progress
  };
  await replayLegWithRecovery([G(1, 65, -3), G(1, 65, 0), G(2, 66, 1, 3)], "npc x", goto, unstick);
  assert.equal(aims[0]!.z, 0, "burst 1 aims at the goal");
  assert.equal(aims[1]!.z, -3, "burst 2 falls back to the proven cell after zero progress");
});

test("replayLegWithRecovery bounds the physics unstick then fails loudly", async () => {
  // A permanently wedged hop: recovery pathfind + every post-unstick re-path fail, so
  // after UNSTICK_ATTEMPTS bursts the hop surfaces loudly (never a silent pass).
  let bursts = 0;
  const goto = async (_spec: GoalSpec, label: string): Promise<void> => {
    if (label.includes("waypoint 2/3")) throw new Error(`permanently wedged at ${label}`);
  };
  const unstick: Unstick = async () => {
    bursts += 1;
    return 0;
  };
  await assert.rejects(
    () =>
      replayLegWithRecovery([G(1, 65, -3), G(1, 65, 0), G(2, 66, 1, 3)], "npc x", goto, unstick),
    /permanently wedged/,
  );
  assert.equal(bursts, 3, "bounded to UNSTICK_ATTEMPTS bursts before failing");
});

// Wave-mob classification (kill-step target selection). Regression: an Invulnerable
// `minecraft:mannequin` NPC — mineflayer resolves its name to "mannequin", height
// 1.8 — must NOT be a wave-mob target. When a wave anchor sits beside a class-post
// mannequin (nobodys-cave surf wave), misclassifying it fixated the bot on an
// unkillable puppet at d<3 and timed the kill step out with drowned still alive.
const SELF = { name: "delve-bot", height: 1.8, position: { x: 0, y: 64, z: 0 } };
function ent(name: string | undefined, height = 1.8): unknown {
  return { name, height, position: { x: 1, y: 64, z: 0 } };
}

test("isWaveMob classifies a living hostile as a wave mob", () => {
  assert.equal(isWaveMob(ent("drowned", 1.95), SELF), true);
  assert.equal(isWaveMob(ent("zombie"), SELF), true);
});

test("isWaveMob never targets an Invulnerable mannequin NPC (regression)", () => {
  assert.equal(isWaveMob(ent("mannequin", 1.8), SELF), false);
});

test("isWaveMob excludes NPCs, displays, drops, and the bot itself", () => {
  for (const name of ["villager", "armor_stand", "interaction", "item", "text_display"]) {
    assert.equal(isWaveMob(ent(name), SELF), false, `${name} must not be a wave mob`);
  }
  assert.equal(isWaveMob(SELF, SELF), false, "the bot is not its own target");
  assert.equal(isWaveMob(ent(undefined), SELF), false, "an unnamed entity is not a target");
  assert.equal(isWaveMob(ent("item", 0.25), SELF), false, "a short dropped entity is excluded");
});

// --- completion oracle (AUDIT-P0) ---------------------------------------------

const COMPLETE: AssertCompleteStep = {
  action: "assert-complete",
  objective: "dw.campaign",
  value: 1,
};

test("assert-complete passes only on the anchored campaign marker for THIS campaign", async () => {
  const bot = new FakeBot();
  const executor = attach(bot);
  executor.useCampaign("hello-world");
  bot.emit("messagestr", "[dw:complete hello-world campaign]");
  await executor.assertComplete(COMPLETE); // resolves
});

test("assert-complete ignores a lookalike line and another campaign's marker", async () => {
  const bot = new FakeBot();
  const executor = attach(bot, {});
  executor.useCampaign("hello-world");
  // Everything a hollow run used to accept.
  bot.emit("messagestr", "[Delvewright] complete dw.campaign 1");
  bot.emit("messagestr", "<player> [dw:complete hello-world campaign]");
  bot.emit("messagestr", "[dw:complete other-delve campaign]");
  bot.emit("messagestr", "[dw:complete hello-world obj/greet]");
  // The bot dies so the (otherwise 15s) settle wait ends promptly; the point is that
  // none of the lines above satisfied the assertion.
  bot.emit("messagestr", "delve-bot fell out of the world");
  bot.emit("death");
  await assert.rejects(
    () => executor.assertComplete(COMPLETE),
    (err: unknown) => err instanceof BotDeathError,
  );
});

test("assertEndgameNotReached passes while the campaign is unfinished", () => {
  const bot = new FakeBot();
  const executor = attach(bot);
  executor.useCampaign("hello-world");
  executor.beginStep(3);
  bot.emit("messagestr", "[dw:complete hello-world obj/greet]");
  assert.doesNotThrow(() => executor.assertEndgameNotReached(3, 9));
});

test("assertEndgameNotReached throws, naming the step the campaign completed at", () => {
  const bot = new FakeBot();
  const executor = attach(bot);
  executor.useCampaign("nobodys-cave-island");
  // The real island shape: `complete_o_board_flee` (step index 11 of a 22-step path)
  // calls `campaign_complete`, while the path's last objective step is 20 — so nine
  // steps ran after the delve was already over.
  executor.beginStep(11);
  bot.emit("messagestr", "[dw:complete nobodys-cave-island campaign]");
  assert.throws(
    () => executor.assertEndgameNotReached(11, 20),
    (err: unknown) =>
      err instanceof Error &&
      /campaign completed at step 11/.test(err.message) &&
      /through step 20/.test(err.message),
  );
});

test("markers arriving before useCampaign or for another campaign are not counted", () => {
  const bot = new FakeBot();
  const executor = attach(bot);
  // No campaign adopted yet: an early marker cannot be attributed and is dropped.
  bot.emit("messagestr", "[dw:complete hello-world campaign]");
  executor.useCampaign("hello-world");
  bot.emit("messagestr", "[dw:complete other-delve campaign]");
  executor.beginStep(2);
  assert.doesNotThrow(() => executor.assertEndgameNotReached(2, 5));
});

test("requireObjective resolves on that objective's own marker, not another's", async () => {
  const bot = new FakeBot();
  const executor = attach(bot);
  executor.useCampaign("hello-world");
  executor.beginStep(2);
  setTimeout(() => {
    // Noise first: a neighbouring objective, another campaign, a human-readable line
    // that talks about the very same objective.
    bot.emit("messagestr", "[dw:complete hello-world obj/greet]");
    bot.emit("messagestr", "[dw:complete other-delve obj/exit]");
    bot.emit("messagestr", "Objective complete: Leave the hall");
    bot.emit("messagestr", "[dw:complete hello-world obj/exit]");
  }, 50);
  await executor.requireObjective("obj/exit", "reach anchor/exit");
});

test("requireObjective fails the step when its objective never completes", async () => {
  const bot = new FakeBot();
  const executor = attach(bot);
  executor.useCampaign("hello-world");
  executor.beginStep(2);
  // Arriving at the anchor is not completing: only the marker is. A death ends the
  // (otherwise 30s) wait promptly and proves the wait was still running.
  bot.emit("messagestr", "delve-bot fell out of the world");
  bot.emit("death");
  await assert.rejects(
    () => executor.requireObjective("obj/exit", "reach anchor/exit"),
    (err: unknown) => err instanceof BotDeathError,
  );
});

test("requireObjective accepts a marker that arrived before the step started", async () => {
  const bot = new FakeBot();
  const executor = attach(bot);
  executor.useCampaign("hello-world");
  // An overlapping trigger zone can complete a later objective early. The objective
  // DID complete, so the step passes — the incoherence that matters (the campaign
  // finishing early) is caught by assertEndgameNotReached, not here.
  executor.beginStep(1);
  bot.emit("messagestr", "[dw:complete hello-world obj/exit]");
  executor.beginStep(2);
  await executor.requireObjective("obj/exit", "reach anchor/exit");
});

// --- timed-gate crossings (spec-0016 §4, task #81) --------------------------

import type { GateAssist } from "../src/executor.ts";
import type { TimedGate } from "../src/waypoints.ts";
import { GATE_MIN_ATTEMPTS, gateRetryBudgetMs } from "../src/timed-gate.ts";

/** The-drowned-bell portcullis: 5×3×1 at z = -10, 100 open / 100 closed. */
const PORTCULLIS: TimedGate = {
  id: "timed-gate/portcullis",
  min: [22, 63, -10],
  max: [26, 65, -10],
  block: "minecraft:iron_bars",
  openTicks: 100,
  closedTicks: 100,
  phase: 0,
};

/** A GateAssist with a virtual clock, so the bounded wait costs no wall time. */
function fakeGate(
  opts: { feet?: () => [number, number, number] | undefined } = {},
): GateAssist & { waits: number; clock: { t: number } } {
  const clock = { t: 0 };
  const state = { waits: 0 };
  return {
    gates: [PORTCULLIS],
    // Each wait advances the virtual clock by one full cycle.
    waitForWindow: async () => {
      state.waits++;
      clock.t += 10_000;
    },
    feetCell: opts.feet ?? (() => [24, 63, -9]),
    now: () => clock.t,
    get waits() {
      return state.waits;
    },
    clock,
  };
}

test("a gate-crossing hop waits for the window and retries instead of failing", async () => {
  // The observed ladder failure: the portcullis fills mid-approach and the pathfinder
  // aborts. The hop must succeed once the window is waited out — not fail the leg.
  const gate = fakeGate();
  const labels: string[] = [];
  let shut = true;
  const goto = async (_spec: GoalSpec, label: string): Promise<void> => {
    labels.push(label);
    if (label.includes("waypoint 1/2") && shut) {
      shut = false; // the very next attempt lands inside an open window
      throw new Error("Path was stopped before it could be completed!");
    }
  };
  await replayLegWithRecovery(
    [G(24, 63, -14), G(24, 63, -14, 3)],
    "anchor anchor/l1a-ward",
    goto,
    undefined,
    gate,
  );
  assert.equal(gate.waits, 1, "waited for exactly one window");
  assert.ok(
    labels.some((l) => l.includes("gate attempt 1")),
    `the retry is labelled as a gate attempt: ${labels.join(" | ")}`,
  );
});

test("a gate-crossing hop does not retreat when the bot is already clear of the fill", async () => {
  // Retreating costs blocks that must be re-walked inside the open window, so the bot
  // only stands off when it is caught IN the fill.
  const gate = fakeGate({ feet: () => [24, 63, -9] }); // one block clear
  const labels: string[] = [];
  let first = true;
  const goto = async (_spec: GoalSpec, label: string): Promise<void> => {
    labels.push(label);
    if (label.includes("waypoint 2/3") && first) {
      first = false;
      throw new Error("Path was stopped before it could be completed!");
    }
  };
  await replayLegWithRecovery(
    [G(24, 63, 4), G(24, 63, -14), G(24, 63, -14, 3)],
    "anchor anchor/l1a-ward",
    goto,
    undefined,
    gate,
  );
  assert.ok(!labels.some((l) => l.includes("standoff")), labels.join(" | "));
});

test("a gate-crossing hop retreats to the last proven cell when caught inside the fill", async () => {
  const gate = fakeGate({ feet: () => [24, 63, -10] }); // standing IN the region
  const specs: Array<{ spec: GoalSpec; label: string }> = [];
  let first = true;
  const goto = async (spec: GoalSpec, label: string): Promise<void> => {
    specs.push({ spec, label });
    if (label.includes("waypoint 2/3") && first) {
      first = false;
      throw new Error("Path was stopped before it could be completed!");
    }
  };
  await replayLegWithRecovery(
    [G(24, 63, 4), G(24, 63, -14), G(24, 63, -14, 3)],
    "anchor anchor/l1a-ward",
    goto,
    undefined,
    gate,
  );
  const standoff = specs.find((s) => s.label.includes("standoff"));
  assert.ok(standoff, "stood off out of the fill");
  assert.deepEqual(
    [standoff!.spec.x, standoff!.spec.y, standoff!.spec.z],
    [24, 63, 4],
    "the standoff is the last proven waypoint",
  );
});

test("a genuinely unwalkable gate leg still fails, naming the gate and its cycle", async () => {
  // The bound is strict: patience is not a pass. Past two full cycles the leg fails.
  const gate = fakeGate();
  const goto = async (_spec: GoalSpec, label: string): Promise<void> => {
    if (label.includes("waypoint 1/2") || label.includes("gate attempt")) {
      throw new Error("No path to the goal!");
    }
  };
  await assert.rejects(
    () =>
      replayLegWithRecovery(
        [G(24, 63, -14), G(24, 63, -14, 3)],
        "anchor anchor/l1a-ward",
        goto,
        undefined,
        gate,
      ),
    (err: unknown) => {
      assert.ok(err instanceof Error);
      assert.match(err.message, /timed-gate\/portcullis/);
      assert.match(err.message, /100t open \/ 100t closed/);
      assert.match(err.message, /real \s*navigation failure|No path to the goal/);
      return true;
    },
  );
  assert.ok(gate.waits >= GATE_MIN_ATTEMPTS, `at least ${GATE_MIN_ATTEMPTS} attempts`);
  assert.ok(
    gate.clock.t > 2 * (gateRetryBudgetMs([PORTCULLIS]) / 3),
    "the budget spans more than two full cycles before giving up",
  );
});

test("an UNMARKED leg gets no gate retries — a real navigation regression still fails fast", async () => {
  // The licence to retry comes from the compiler's crossing mark, never from the
  // harness. A leg with no gate keeps the pre-task-#81 behaviour exactly.
  let attempts = 0;
  const goto = async (_spec: GoalSpec, label: string): Promise<void> => {
    if (label.includes("waypoint 2/3") && !label.includes("recovery")) {
      attempts++;
      throw new Error("Path was stopped before it could be completed!");
    }
  };
  await assert.rejects(
    () =>
      replayLegWithRecovery(
        [G(0, 65, 0), G(0, 65, 3), G(0, 65, 6, 3)],
        "anchor anchor/plain",
        goto,
      ),
    /Path was stopped/,
  );
  // One initial try plus the single stall-recovery retry — no window loop.
  assert.equal(attempts, 2, "no blanket retry on an unmarked leg");
});

test("a gate crossing the pathfinder cannot hold is finished by walking, inside the window", async () => {
  // The-drowned-bell portcullis, observed: the pathfinder walks the bot to the gate's
  // mouth and aborts every window ("Path was stopped…"), while a raw physics burst
  // crosses the same span at once — a path whose blocks are rewritten under it twice
  // per cycle is not something A* holds on to. The attempt must escalate to the
  // ordinary task-#45 recovery WITHIN the window, not only after the budget is spent.
  const gate = fakeGate();
  let freed = false;
  const bursts: GoalSpec[] = [];
  const goto = async (_spec: GoalSpec, label: string): Promise<void> => {
    const isHop = label.includes("waypoint 2/3");
    if (isHop && !freed) throw new Error("Path was stopped before it could be completed!");
  };
  const unstick: Unstick = async (target) => {
    bursts.push(target);
    freed = true; // one burst walks the span
    return 1;
  };
  await replayLegWithRecovery(
    [G(24, 63, -9), G(24, 63, -11), G(24, 63, -14, 3)],
    "anchor anchor/l1a-ward",
    goto,
    unstick,
    gate,
  );
  assert.equal(bursts.length, 1, "one physical crossing burst was enough");
  assert.equal(gate.waits, 1, "and it happened inside the FIRST window, not after the budget");
});

// --- interact: the mainhand contract (compiler PR #205) ----------------------

import type { InteractStep } from "../src/critical-path.ts";
import registryFor from "prismarine-registry";

/**
 * A FakeBot that can be driven through a whole `interact` step. Two additions over
 * the base fake: a real pinned registry (mineflayer-pathfinder's `Movements`
 * constructor reads the block table) and an inventory/equip/chat recorder.
 *
 * The bot is placed AT the interact anchor, so `runGoto` short-circuits on
 * `withinGoal` and the leg costs no wall time — the walk is not what is under test.
 */
class InteractFakeBot extends FakeBot {
  registry = registryFor("1.21.11");
  health = 20;
  food = 20;
  entities: Record<number, unknown> = {};
  /** Everything the step made the bot DO, in order. */
  calls: string[] = [];
  carried: Array<{ name: string; type: number }> = [];
  inventory = { items: (): Array<{ name: string; type: number }> => this.carried };
  override pathfinder = {
    stop: (): void => {
      this.pathfinderStops += 1;
      this.pathfinderCalls.push("stop");
    },
    setGoal: (goal: unknown): void => {
      this.pathfinderCalls.push(goal === null ? "setGoal(null)" : "setGoal");
    },
    setMovements: (): void => {},
    thinkTimeout: 0,
    goto: async (): Promise<void> => {
      this.calls.push("goto");
    },
  };
  setControlState(): void {}
  async equip(item: { name: string }, destination: string): Promise<void> {
    this.calls.push(`equip(${item.name},${destination})`);
  }
  chat(message: string): void {
    this.calls.push(`chat(${message})`);
  }
}

function interactStep(requiresItem: string | null): InteractStep {
  return {
    action: "interact",
    objective: "obj/unbar",
    anchor: "anchor/gate",
    pos: [0, 64, 0],
    command: "/trigger dw.i.unbar",
    requiresItem,
    sneak: false,
  };
}

test("interact equips the required item BEFORE chatting the trigger", async () => {
  // The PR #205 regression: `requires_item` became MAINHAND-held, and the bot — which
  // only ever carried the item — had every trigger swallowed by the datapack guard,
  // then died on its own objective timeout. Order is the whole assertion: the guard
  // reads the hand on the tick it consumes the trigger.
  const bot = new InteractFakeBot();
  bot.carried = [
    { name: "stone_sword", type: 1 },
    { name: "trial_key", type: 2 },
  ];
  const executor = attach(bot);
  executor.useCampaign("keep-trial");
  executor.beginStep(3);
  setTimeout(() => bot.emit("messagestr", "[dw:complete keep-trial obj/unbar]"), 20);
  await executor.interact(interactStep("minecraft:trial_key"));
  assert.deepEqual(bot.calls, ["equip(trial_key,hand)", "chat(/trigger dw.i.unbar)"]);
});

test("interact leaves the hand alone when the step requires no item", async () => {
  // The loadout put a sword there; a step that asked for nothing must not disarm the
  // bot on its way to the next fight.
  const bot = new InteractFakeBot();
  bot.carried = [
    { name: "stone_sword", type: 1 },
    { name: "trial_key", type: 2 },
  ];
  const executor = attach(bot);
  executor.useCampaign("keep-trial");
  executor.beginStep(3);
  setTimeout(() => bot.emit("messagestr", "[dw:complete keep-trial obj/unbar]"), 20);
  await executor.interact(interactStep(null));
  assert.deepEqual(bot.calls, ["chat(/trigger dw.i.unbar)"]);
});

// --- the die-retry stage: the run artifact must never lose a death (task #102) ---

import type { KillStep, SelectClassStep } from "../src/critical-path.ts";
import {
  dieRetryCoverageFailures,
  dieRetryFindings,
  trialVerdict,
  type CombatPlan,
} from "../src/combat.ts";

/** How a test asks the fake server to re-seat the wave. */
interface ReseatSpec {
  count?: number;
  /** Health every freshly summoned mob arrives with (default: full). */
  health?: number;
  /** Entity ids the re-seat failed to clear — the survivors. */
  keepIds?: number[];
  /** Health those survivors kept from the last life. */
  survivorHealth?: number;
  /** Blocks from the encounter anchor. */
  distance?: number;
}

/** One wave mob as the fake server publishes it to a client. */
interface FakeMob {
  id: number;
  name: string;
  height: number;
  position: FakeVec3;
  metadata: Record<number, unknown>;
  attributes: Record<string, { value: number }>;
  /** Stands in for the compiler's `dw_wave_<id>` tag: only a mob the wave itself
   * summoned carries it, so the census can never count a bonfire affordance, an
   * ambush actor or a neighbouring wave (task #123). */
  waveTagged: true;
}

/** Where the pinned registry puts `health` in a zombie's metadata. Resolved by
 * NAME, exactly as the harness does — neither side hardcodes the index. */
const ZOMBIE_HEALTH_IDX = (
  registryFor("1.21.11") as unknown as {
    entitiesByName: Record<string, { metadataKeys: string[] }>;
  }
).entitiesByName["zombie"]!.metadataKeys.indexOf("health");

const FULL_HEALTH = 20;

/**
 * A FakeBot that can be driven through a whole `kill` step with the die-retry
 * stage on.
 *
 * The fake server does what a real one does: it kills the bot when the harness
 * `/damage`s itself, respawns it, and re-seats the wave on that respawn — with
 * whatever fidelity the test asks for. Wave mobs live in `entities`, because the
 * re-seat check reads the whole tracked SET, not a nearest hit.
 */
class CombatFakeBot extends InteractFakeBot {
  /** Whether `/damage @s` actually kills the bot (the bot is opped for it). */
  scriptedDeathsLand = true;
  /** Injected fault, armed only AFTER a death so it lands on the re-engage probe:
   * stands in for ANY unexpected fault once the death has been taken. The shipped
   * one was the death-aware wait throwing on the very death it waited for. */
  failReEngageProbe = false;
  /** Where the server puts the bot on respawn. `undefined` = it never moves — the
   * default, which keeps every other test's geometry exactly as it was. */
  respawnAt: [number, number, number] | undefined;
  /** The route back from the respawn is not walkable — the bell run-five symptom. */
  failReturnLeg = false;
  /** The server did NOT keep the inventory across the death (a broken
   * `gamerule keep_inventory true` seal). */
  loseKitOnDeath = false;
  /** The wave kills the bot the first time it swings, mid-trade — before the
   * harness gets to script the death this trial asked for (task #121). */
  killBotOnTrade = false;
  /** How the respawn re-seats the wave. `undefined` = do not re-seat at all. */
  reSeat: ReseatSpec | undefined = { count: 1 };
  /** Delay before the re-seated wave becomes visible to the client — entity
   * tracking lags arrival, which is the island-r14 false negative. */
  reSeatVisibleAfterMs = 0;
  private died = false;
  private nextId = 100;
  /** Server-side census state: which mobs wear the brand, and how many censuses
   * have been answered (the sequence the harness tells fresh from stale by). */
  private readonly branded = new Set<number>();
  private censusSeq = 0;

  constructor() {
    super();
    this.seat(1);
  }

  override pathfinder = {
    stop: (): void => {
      this.pathfinderStops += 1;
      this.pathfinderCalls.push("stop");
    },
    setGoal: (goal: unknown): void => {
      this.pathfinderCalls.push(goal === null ? "setGoal(null)" : "setGoal");
    },
    setMovements: (): void => {},
    thinkTimeout: 0,
    goto: async (): Promise<void> => {
      this.calls.push("goto");
      // A route walkable on the way in and not on the way back: exactly what a
      // respawn dumped somewhere unreachable looks like to the bot.
      if (this.failReturnLeg && this.died) {
        throw new Error("no path to the encounter from here");
      }
    },
  };

  /** Replace the tracked wave with `count` fresh mobs. */
  seat(count: number, opts: ReseatSpec | Omit<ReseatSpec, "count"> = {}): void {
    this.entities = {};
    for (const id of opts.keepIds ?? []) this.entities[id] = this.makeMob(id, opts);
    const fresh = count - (opts.keepIds?.length ?? 0);
    for (let i = 0; i < fresh; i++) {
      this.entities[this.nextId] = this.makeMob(this.nextId, opts);
      this.nextId += 1;
    }
  }

  private makeMob(id: number, opts: Omit<ReseatSpec, "count">): FakeMob {
    const d = opts.distance ?? 1;
    // A survivor the re-seat failed to clear keeps the damage the last life dealt
    // it; a freshly summoned mob is whole unless the test says otherwise.
    const survivor = (opts.keepIds ?? []).includes(id);
    const health = survivor
      ? (opts.survivorHealth ?? opts.health ?? FULL_HEALTH)
      : (opts.health ?? FULL_HEALTH);
    const self = this;
    return {
      id,
      name: "zombie",
      height: 2,
      waveTagged: true,
      metadata: { [ZOMBIE_HEALTH_IDX]: health },
      get attributes(): Record<string, { value: number }> {
        return { "minecraft:max_health": { value: FULL_HEALTH } };
      },
      get position(): FakeVec3 {
        return new FakeVec3(d, 64, 0);
      },
    };
  }

  override chat(message: string): void {
    this.calls.push(`chat(${message})`);
    // The fake server answers the census the way a real one does: by TAG (task
    // #123). Only mobs in `entities` carry the wave tag here, so anything a test
    // parks beside the encounter is invisible to it — which is the whole point.
    if (message.startsWith("/function ")) {
      const fn = message.slice("/function ".length);
      if (fn.includes(":wave_brand_")) {
        for (const m of this.waveMobs()) this.branded.add(m.id);
        return;
      }
      if (fn.includes(":wave_unbrand_")) {
        this.branded.clear();
        return;
      }
      if (fn.includes(":wave_census_")) {
        if (this.failReEngageProbe && this.died) return; // the probe never answers
        this.censusSeq += 1;
        const mobs = this.waveMobs();
        for (const m of mobs) {
          const p = m.position;
          const h = Math.round(this.healthOf(m) * 100);
          this.emit(
            "messagestr",
            `[dw:censusmob the-drowned-bell wave/gate-assault ${this.censusSeq} ` +
              `${Math.round(p.x * 100)} ${Math.round(p.y * 100)} ${Math.round(p.z * 100)} ` +
              `${h} ${FULL_HEALTH * 100}]`,
          );
        }
        const branded = mobs.filter((m) => this.branded.has(m.id)).length;
        const damaged = mobs.filter((m) => this.healthOf(m) < FULL_HEALTH).length;
        this.emit(
          "messagestr",
          `[dw:census the-drowned-bell wave/gate-assault ${this.censusSeq} ` +
            `${mobs.length} ${branded} ${damaged}]`,
        );
        return;
      }
      return;
    }
    if (!message.startsWith("/damage") || !this.scriptedDeathsLand) return;
    setTimeout(() => {
      this.died = true;
      this.emit("messagestr", "delve-bot was slain by Vindicator");
      this.emit("death");
      // The respawn lands FAST, as a real server's does — faster than the harness
      // can poll the death latch and arm a wait, which is the race the spawn
      // counter exists for.
      setTimeout(() => {
        this.entities = {};
        // `gamerule keep_inventory true` is what a delve seals; a server without
        // it hands the player back an empty bag.
        if (this.loseKitOnDeath) this.carried = [];
        // A respawn puts the player at their spawn point, which is somewhere else.
        if (this.respawnAt) {
          this.entity.position = new FakeVec3(...this.respawnAt);
        }
        const reseat = this.reSeat;
        if (reseat) {
          const apply = (): void => this.seat(reseat.count ?? 0, reseat);
          if (this.reSeatVisibleAfterMs > 0) setTimeout(apply, this.reSeatVisibleAfterMs);
          else apply();
        }
        this.emit("spawn");
      }, 10);
    }, 5);
  }

  /** Park a mob-shaped entity that is NOT part of the wave: an ambush actor, a
   * neighbouring wave's straggler. Visible to `nearestEntity`, invisible to the
   * census — exactly the drowned bell's belfry (task #124). */
  addBystander(id: number, distance = 1): void {
    const self = this;
    this.entities[id] = {
      id,
      name: "husk",
      height: 2,
      metadata: { [ZOMBIE_HEALTH_IDX]: FULL_HEALTH },
      get attributes(): Record<string, { value: number }> {
        return { "minecraft:max_health": { value: FULL_HEALTH } };
      },
      get position(): FakeVec3 {
        return new FakeVec3(distance, 64, 0);
      },
    } as unknown as FakeMob;
    void self;
  }

  /** Everything wearing the wave tag — the census's whole universe. */
  private waveMobs(): FakeMob[] {
    return (Object.values(this.entities) as FakeMob[]).filter((e) => e?.waveTagged === true);
  }

  private healthOf(m: FakeMob): number {
    const raw = m.metadata?.[ZOMBIE_HEALTH_IDX];
    return typeof raw === "number" ? raw : FULL_HEALTH;
  }

  nearestEntity(match: (e: unknown) => boolean = () => true): unknown {
    let best: FakeMob | undefined;
    for (const e of Object.values(this.entities) as FakeMob[]) {
      if (!match(e)) continue;
      if (!best || e.position.distanceTo(this.entity.position) < best.position.distanceTo(this.entity.position)) {
        best = e;
      }
    }
    return best;
  }

  /** Swings a wave mob takes before it drops. 1 (one swing) unless a test wants
   * the fight to outlast something else dying beside it. */
  waveHitsToKill = 1;
  private readonly hitsTaken = new Map<number, number>();

  attack(mob: { id: number }): void {
    this.calls.push("attack");
    // The wave wins the exchange: the bot dies mid-trade, before it ever reaches
    // the line that scripts its own death (task #121). Armed once.
    if (this.killBotOnTrade) {
      this.killBotOnTrade = false;
      this.killBot();
      return;
    }
    const tagged = (this.entities[mob.id] as { waveTagged?: boolean } | undefined)?.waveTagged;
    const need = tagged ? this.waveHitsToKill : 1;
    const taken = (this.hitsTaken.get(mob.id) ?? 0) + 1;
    this.hitsTaken.set(mob.id, taken);
    if (taken < need) return;
    const ent = this.entities[mob.id];
    delete this.entities[mob.id]; // one swing is enough in the fake world
    // A real server announces the removal, and that announcement is what credits
    // a confirmed kill (`entityGone` → `creditsWaveKill`). Without it the fake
    // world could never reproduce the drowned bell's belfry, where a husk's death
    // was credited to the Bellkeeper's wave.
    if (ent) this.emit("entityGone", ent);
  }

  /** A death the harness did NOT script, delivered the way a server delivers it:
   * the death, then a fast auto-respawn. */
  private killBot(): void {
    this.died = true;
    this.emit("messagestr", "delve-bot was slain by Vindicator");
    this.emit("death");
    setTimeout(() => this.emit("spawn"), 10);
  }

  async lookAt(): Promise<void> {}
}

const KILL_STEP: KillStep = {
  action: "kill",
  objective: "obj/hold-the-gate",
  wave: "wave/gate-assault",
  pos: [0, 64, 0],
  tag: "dw.wave.gate_assault",
  count: 1,
  sneak: false,
};

const ENCOUNTER = {
  wave: "wave/gate-assault",
  objective: "obj/hold-the-gate",
  step: 11,
  tier: "ordinary" as const,
  pos: [0, 64, 0] as [number, number, number],
  count: 1,
  respawns_on_rest: true,
  checkpoint: [0, 64, 0] as [number, number, number],
  census: {
    census: "the-drowned-bell:wave_census_gate_assault",
    brand: "the-drowned-bell:wave_brand_gate_assault",
    unbrand: "the-drowned-bell:wave_unbrand_gate_assault",
  },
};

function combatPlan(count = 1, respawnsOnRest = true): CombatPlan {
  return {
    version: "0.6.0",
    campaignId: "the-drowned-bell",
    difficulty: "normal",
    encounters: [
      {
        wave: ENCOUNTER.wave,
        objective: ENCOUNTER.objective,
        step: ENCOUNTER.step,
        tier: ENCOUNTER.tier,
        pos: ENCOUNTER.pos,
        count,
        respawnsOnRest,
        census: ENCOUNTER.census,
        checkpoint: ENCOUNTER.checkpoint,
      },
    ],
    // No tiered actor and an empty (but PRESENT) ledger: this fixture's campaign
    // bills nothing the wave gate does not already cover.
    actors: [],
    floorGate: { present: true, covered: [], notCovered: [] },
  };
}

const ASSIST_ON = "chat(/effect give @s minecraft:resistance 60 2 true)";
const ASSIST_OFF = "chat(/effect clear @s minecraft:resistance)";
const SCRIPTED_DEATH = "chat(/damage @s 1000 minecraft:generic)";

test("the die-retry stage is assisted into melee range, and takes its death bare", async () => {
  // the-drowned-bell run six (task #121). The die-retry stage walked to within 3
  // blocks of a LIVE encounter with nothing on, so two vindicators killed the bot
  // before it could script death 1: `dieRetryAt` threw, `kill` never reached its
  // own assisted phase, and the artifact showed 0/2 trials beside
  // `assist_windows: []`. Bot fencing skill was deciding whether the stage could
  // run at all — the exact thing spec-0023 downgraded from gate to telemetry.
  //
  // The ordering below IS the fix, and each half of it matters:
  //   * assist ON before the bot is ever in melee range, and again before the walk
  //     back — the segments where it must SURVIVE to make a measurement;
  //   * assist OFF before every scripted death — so `/damage @s 1000` needs no
  //     argument about resistance arithmetic to be lethal.
  const bot = new CombatFakeBot();
  const executor = attach(bot);
  executor.useCampaign("the-drowned-bell");
  executor.useCombatPlan(combatPlan(), true);
  await executor.kill(KILL_STEP);

  const combat = bot.calls.filter(
    (c) => c === ASSIST_ON || c === ASSIST_OFF || c === SCRIPTED_DEATH,
  );
  const deaths = combat.filter((c) => c === SCRIPTED_DEATH).length;
  assert.equal(deaths, 2, "two scripted deaths were taken");
  for (const [i, call] of combat.entries()) {
    if (call !== SCRIPTED_DEATH) continue;
    assert.equal(
      combat[i - 1],
      ASSIST_OFF,
      `the scripted death is taken with no assist in force: ${combat.join(" | ")}`,
    );
    assert.equal(
      combat[i + 1],
      ASSIST_ON,
      `and the walk back is assisted again immediately after: ${combat.join(" | ")}`,
    );
  }
  assert.equal(
    combat[0],
    ASSIST_ON,
    `the very first combat act of the stage is arming the assist — before the ` +
      `approach walks into melee range: ${combat.join(" | ")}`,
  );
});

test("die-retry assist windows are named in the ledger, not taken silently", async () => {
  // spec-0023 §3 asks for disclosure, not for a single window. Each segment the
  // stage protects is opened, logged and closed on its own, so `assist_windows`
  // says exactly when the bot was helped.
  const bot = new CombatFakeBot();
  const executor = attach(bot);
  executor.useCampaign("the-drowned-bell");
  executor.useCombatPlan(combatPlan(), true);
  await executor.kill(KILL_STEP);

  const windows = executor.assistWindows();
  const dieRetry = windows.filter((w) => w.reason.startsWith("die-retry:"));
  assert.ok(
    dieRetry.length >= 4,
    `each approach and each walk back is its own named window: ${windows.map((w) => w.reason).join(" | ")}`,
  );
  assert.ok(
    dieRetry.every((w) => w.encounter === "obj/hold-the-gate" && w.closedAtMs !== undefined),
    "every die-retry window names its encounter and is closed",
  );
  assert.deepEqual(executor.leakedAssists(), [], "and none of them leaked");
});

test("a wave that kills the bot mid-trade does not get credited as the scripted death", async () => {
  // The first-contact/mid-fight race (task #121). `tradeBlows` deliberately stands
  // in melee; if the wave wins that exchange the bot is already dead when the
  // harness reads `deathSeq` and chats `/damage`, so the trial would wait for a
  // death that has to happen a SECOND time and credit its loop to a life the
  // harness never opened. The accidental death is now recovered from first.
  const bot = new CombatFakeBot();
  bot.killBotOnTrade = true;
  const executor = attach(bot);
  executor.useCampaign("the-drowned-bell");
  executor.useCombatPlan(combatPlan(), true);
  await executor.kill(KILL_STEP);

  const trials = executor.deathTrials();
  assert.equal(trials.length, 2, "still exactly the two scripted deaths spec-0023 asks for");
  assert.ok(
    trials.every((t) => t.completed),
    "and both loops still reached a verdict",
  );
  assert.equal(
    bot.calls.filter((c) => c === SCRIPTED_DEATH).length,
    2,
    "one scripted death per trial — the accidental one did not stand in for either",
  );
  assert.deepEqual(dieRetryFindings(trials), []);
});

test("the die-retry stage survives its OWN scripted death and records both trials", async () => {
  // The shipped defect (the-drowned-bell round 3): the stage waited for its
  // scripted death with the harness's death-AWARE poll, which throws the recorded
  // BotDeathError the instant one exists. So the wait threw on the very death it
  // was asked for, `kill` aborted, the run blamed the content ("bot died … likely
  // cause: Hollow Gate-Warder") and the artifact shipped `die_retry: []` with
  // `passed: true`. No die-retry trial could EVER complete.
  const bot = new CombatFakeBot();
  const executor = attach(bot);
  executor.useCampaign("the-drowned-bell");
  executor.useCombatPlan(combatPlan(), true);
  await executor.kill(KILL_STEP);

  const trials = executor.deathTrials();
  assert.equal(trials.length, 2, "spec-0023 takes two scripted deaths per encounter");
  assert.deepEqual(
    trials.map((t) => t.phase),
    ["first-contact", "mid-fight"],
  );
  assert.ok(
    trials.every((t) => t.completed),
    "both loops reached a verdict",
  );
  assert.deepEqual(dieRetryFindings(trials), [], "and every verdict was clean");
  assert.deepEqual(
    trials.map((t) => t.outcome),
    ["re-engaged", "re-engaged"],
    "hostiles were standing there again both times",
  );
  assert.equal(trials[0]!.cause, "delve-bot was slain by Vindicator");
  // The bot really did chat the death command, twice — not a bookkeeping-only pass.
  assert.equal(bot.calls.filter((c) => c === "chat(/damage @s 1000 minecraft:generic)").length, 2);
});

/** The class step the die-retry stage used to replay after every death. */
const SELECT_CLASS_STEP: SelectClassStep = {
  action: "select-class",
  class: "class/warden",
  command: "/trigger dw.class set 1",
};

test("a scripted death re-arms the bot WITHOUT re-selecting the class", async () => {
  // task #120, the-drowned-bell run five. The re-arm used to replay `select-class`.
  // The `dw.class` trigger is re-enabled for every player on every tick and
  // `class_apply_<class>` ENDS IN `teleport @s <campaign entry point>`, so every
  // post-death re-arm silently warped the bot from the checkpoint it had just
  // respawned on back to the start of the delve — 150 blocks and eight levels away
  // on the bell. `respawn_pos` was measured correctly at the bonfire and made a lie
  // one second later, and the walk back then measured a route no dying player walks.
  const bot = new CombatFakeBot();
  bot.carried = [{ name: "iron_sword", type: 1 }];
  const executor = attach(bot);
  executor.useCampaign("the-drowned-bell");
  executor.useCombatPlan(combatPlan(), true);
  await executor.selectClass(SELECT_CLASS_STEP);
  await executor.kill(KILL_STEP);

  assert.equal(
    bot.calls.filter((c) => c === `chat(${SELECT_CLASS_STEP.command})`).length,
    1,
    "the class trigger is chatted once, at the start of the run, and never after a death",
  );
  // What a respawn DOES need: the kept kit back on. `keep_inventory` is sealed by
  // the compiler, so re-equipping is the whole of a legitimate re-arm.
  assert.ok(
    bot.calls.filter((c) => c === "equip(iron_sword,hand)").length >= 3,
    `the kit goes back on after each of the two deaths: ${bot.calls.join(",")}`,
  );
  assert.ok(
    executor.deathTrials().every((t) => t.kitKept),
    "and the kit survived every death",
  );
});

test("a kit lost across a death reds the trial — keep_inventory is the seal", async () => {
  const bot = new CombatFakeBot();
  bot.carried = [{ name: "iron_sword", type: 1 }];
  bot.loseKitOnDeath = true;
  const executor = attach(bot);
  executor.useCampaign("the-drowned-bell");
  executor.useCombatPlan(combatPlan(), true);
  await executor.selectClass(SELECT_CLASS_STEP);
  await executor.kill(KILL_STEP);

  const trials = executor.deathTrials();
  assert.ok(trials.length > 0);
  assert.equal(trials[0]!.kitKept, false);
  assert.match(String(trialVerdict(trials[0]!)), /EMPTY-HANDED/);
});

test("a trial that never walked back reports NO re-engagement observation", async () => {
  // The run-five artifact carried, in ONE trial: `returned: false` ("the route from
  // the respawn back to the encounter is not walkable"), `re_engaged: true` and
  // `completed: true`. The probe reads the entities the CLIENT tracks, so a bot
  // stuck 150 blocks away was reporting on wherever it stood, not on the fight.
  // "Did not look" and "looked and found nothing" are different facts and neither
  // is a pass.
  // The delve's own shape: the checkpoint (a bonfire) is a long way from the fight,
  // the respawn lands ON it — the loop's premise holds — and the route back is
  // broken. That is a real content failure and it must read as exactly one.
  const bot = new CombatFakeBot();
  bot.respawnAt = [60, 64, 0];
  bot.failReturnLeg = true;
  const executor = attach(bot);
  executor.useCampaign("the-drowned-bell");
  const plan = combatPlan();
  const encounter = { ...plan.encounters[0]!, checkpoint: [60, 64, 0] as [number, number, number] };
  executor.useCombatPlan({ ...plan, encounters: [encounter] }, true);
  await assert.rejects(() => executor.kill(KILL_STEP));

  const trials = executor.deathTrials();
  assert.ok(trials.length > 0);
  for (const t of trials) {
    assert.deepEqual(t.respawnPos, [60, 64, 0], "the respawn point is MEASURED, not assumed");
    assert.equal(t.atCheckpoint, true);
    assert.equal(t.returned, false);
    assert.equal(t.reEngaged, false, "no re-engagement is claimed from a fight never reached");
    assert.equal(t.reengage, undefined, "and no observation is fabricated for the artifact");
    assert.equal(t.outcome, "unproven");
  }
  assert.match(String(trialVerdict(trials[0]!)), /not walkable/);
});

test("a loop abandoned after the death still carries the death in the artifact", async () => {
  // The integrity rule: a scripted death that HAPPENED is in the report the moment
  // it happens, however the run ends. Before this, the trial was appended only
  // after the whole loop succeeded, so an abort discarded it — and the stage, with
  // nothing recorded and therefore no findings, read `passed: true`.
  //
  // The fault is now a census that never answers (task #123) — the shape a refused
  // `/function` takes on an unopped bot. It must abort the trial, never return an
  // empty count: a silent zero would read as `stranded` and blame the delve for
  // the harness's own broken probe.
  const bot = new CombatFakeBot();
  bot.failReEngageProbe = true;
  const executor = attach(bot);
  executor.useCampaign("the-drowned-bell");
  executor.useCombatPlan(combatPlan(), true);

  await assert.rejects(() => executor.kill(KILL_STEP), /census .* never answered/);

  const trials = executor.deathTrials();
  assert.equal(trials.length, 1, "the death that happened is recorded");
  assert.equal(trials[0]!.completed, false);
  assert.match(String(trials[0]!.abortedWith), /census .* never answered/);
  // …and it reads RED, not silent: an unfinished trial is never a passed one.
  assert.match(String(trialVerdict(trials[0]!)), /ABANDONED/);
  const failures = [
    ...dieRetryFindings(trials),
    ...dieRetryCoverageFailures(
      combatPlan().encounters,
      executor.dieRetryEngagements(),
      trials,
    ),
  ];
  assert.ok(failures.length > 0, "the stage cannot report a pass");
  assert.ok(failures.some((f) => /ENGAGED this encounter but proved only 0\/2/.test(f)));
});

test("an encounter the stage entered but never died at is engaged, not silent", async () => {
  // The approach leg fails, so no death is ever taken. Nothing to record — which
  // is exactly the silence that used to read as a pass. The engagement is booked
  // before the walk, so coverage still fails the stage.
  const bot = new CombatFakeBot();
  bot.entity.position = new FakeVec3(0, 64, 0);
  const executor = attach(bot);
  executor.useCampaign("the-drowned-bell");
  const plan = combatPlan();
  executor.useCombatPlan(plan, true);
  const far: KillStep = { ...KILL_STEP, pos: [400, 64, 400] };

  await assert.rejects(() => executor.kill(far));

  assert.equal(executor.deathTrials().length, 0);
  assert.ok(executor.dieRetryEngagements().has("wave/gate-assault"));
  const failures = dieRetryCoverageFailures(
    plan.encounters,
    executor.dieRetryEngagements(),
    executor.deathTrials(),
  );
  assert.equal(failures.length, 1);
  assert.match(failures[0]!, /ENGAGED this encounter but proved only 0\/2/);
});

// --- what was waiting at the end of the loop (planner ruling 2026-08-03) ------

test("a wave already beaten before the death records cleared-before-retry, and passes", async () => {
  // `respawns_on_rest: false` is a legitimate design — a won fight stays won —
  // so the wave is simply gone when the bot walks back. With the encounter's
  // objective COMPLETE, the party that died here can still finish the delve:
  // the loop worked. Before this, the same fixture went red or green depending
  // on whether the bot's timed melee happened to finish the wave first.
  const bot = new CombatFakeBot();
  bot.seat(0); // the fight was won before the scripted death
  bot.reSeat = undefined; // `respawns_on_rest: false` — a won fight stays won
  const executor = attach(bot);
  executor.useCampaign("the-drowned-bell");
  executor.useCombatPlan(combatPlan(1, false), true);
  bot.emit("messagestr", "[dw:complete the-drowned-bell obj/hold-the-gate]");

  await executor.kill(KILL_STEP);

  const trials = executor.deathTrials();
  assert.equal(trials.length, 2);
  assert.deepEqual(
    trials.map((t) => t.outcome),
    ["cleared-before-retry", "cleared-before-retry"],
  );
  assert.ok(trials.every((t) => t.objectiveComplete && !t.reEngaged));
  assert.deepEqual(dieRetryFindings(trials), [], "a won fight staying won is not a finding");
  assert.deepEqual(
    dieRetryCoverageFailures(
      combatPlan(1, false).encounters,
      executor.dieRetryEngagements(),
      trials,
    ),
    [],
    "and it counts as full coverage — these are proved trials, not skipped ones",
  );
});

test("a wave that vanishes with its objective UNFINISHED is a soft lock, loudly", async () => {
  // The failure the stage exists to catch, and the one the old uniform
  // "did not re-engage" red could not tell apart from a won fight: the party can
  // neither finish the encounter nor fight it again.
  const bot = new CombatFakeBot();
  bot.seat(0);
  bot.reSeat = undefined;
  const executor = attach(bot);
  executor.useCampaign("the-drowned-bell");
  executor.useCombatPlan(combatPlan(1, false), true);
  // …and no completion marker for obj/hold-the-gate ever arrives.

  await executor.kill(KILL_STEP);

  const trials = executor.deathTrials();
  assert.deepEqual(
    trials.map((t) => t.outcome),
    ["stranded", "stranded"],
  );
  assert.ok(trials.every((t) => !t.objectiveComplete && !t.reEngaged));
  const findings = dieRetryFindings(trials);
  assert.equal(findings.length, 2, "every stranded trial is a red finding");
  assert.match(findings[0]!, /STRANDED/);
  assert.match(findings[0]!, /obj\/hold-the-gate/);
});

// --- re-seat fidelity + the wandered-mob false negative (task #108) ----------

async function dieRetryAgainst(bot: CombatFakeBot, count: number): Promise<MineflayerExecutor> {
  const executor = attach(bot);
  executor.useCampaign("the-drowned-bell");
  executor.useCombatPlan(combatPlan(count, true), true);
  await executor.kill({ ...KILL_STEP, count });
  return executor;
}

test("a wave that re-seats whole — fresh entities, full health, authored count — passes", async () => {
  const bot = new CombatFakeBot();
  bot.seat(3);
  bot.reSeat = { count: 3 };
  const executor = await dieRetryAgainst(bot, 3);

  const trials = executor.deathTrials();
  assert.deepEqual(
    trials.map((t) => t.outcome),
    ["re-engaged", "re-engaged"],
  );
  assert.deepEqual(dieRetryFindings(trials), [], "a faithful re-seat is silent");
  for (const t of trials) {
    assert.equal(t.reengage!.present, 3);
    assert.equal(t.reengage!.carriedOver, 0, "every mob is a NEW entity");
    assert.equal(t.reengage!.damaged, 0);
    assert.equal(t.reengage!.healthReadable, 3, "health was readable via the pinned registry");
  }
});

test("a re-seat that comes back SHORT is red", async () => {
  const bot = new CombatFakeBot();
  bot.seat(3);
  bot.reSeat = { count: 2 }; // one mob never came back
  const executor = await dieRetryAgainst(bot, 3);

  const findings = dieRetryFindings(executor.deathTrials());
  assert.equal(findings.length, 2);
  assert.match(findings[0]!, /came back SHORT — 2 mob\(s\) standing, 3 declared/);
  assert.equal(executor.deathTrials()[0]!.reengage!.present, 2);
});

test("a damaged survivor carried across a life is red — the owner's grind rule", async () => {
  // 打一半的怪要移除重新生成一模一样的: a half-fought mob is REMOVED and regenerated.
  // Here the re-seat tops the wave up AROUND the survivor the last life chipped,
  // which is exactly how a party grinds a boss down one swing per death.
  const bot = new CombatFakeBot();
  bot.seat(3);
  const survivor = Object.values(bot.entities as Record<number, { id: number }>)[0]!.id;
  bot.reSeat = { count: 3, keepIds: [survivor], survivorHealth: 6 };
  const executor = await dieRetryAgainst(bot, 3);

  const trials = executor.deathTrials();
  const findings = dieRetryFindings(trials);
  assert.equal(findings.length, 2);
  assert.match(findings[0]!, /the bot already\s+fought in a previous life/);
  assert.match(findings[0]!, /never topped up around its survivors/);
  assert.equal(trials[0]!.reengage!.carriedOver, 1);
  assert.equal(trials[0]!.reengage!.damaged, 1, "exactly the survivor is the wounded one");
});

test("a wave that comes back whole but WOUNDED is red", async () => {
  // No carried-over entity — the re-seat did replace them — but they arrived
  // below full health. The player respawns whole; so must the wave.
  const bot = new CombatFakeBot();
  bot.seat(3);
  bot.reSeat = { count: 3, health: 11 };
  const executor = await dieRetryAgainst(bot, 3);

  const findings = dieRetryFindings(executor.deathTrials());
  assert.equal(findings.length, 2);
  assert.match(findings[0]!, /came back BELOW full/);
  assert.equal(executor.deathTrials()[0]!.reengage!.damaged, 3);
});

test("wave mobs that WANDERED off the anchor are re-engaged, never stranded", async () => {
  // The island-r14 false negative: three feral drowned (follow_range 48) wander
  // off the anchor after killing the bot. They are alive and will come — the bot
  // cleared 3/3 moments later — but both trials reported "no hostile was there to
  // fight" and went red. There is no distance filter: anything the client tracks
  // is inside vanilla's 128-block monster range, well beyond any follow_range.
  const bot = new CombatFakeBot();
  bot.seat(3, { distance: 60 });
  bot.reSeat = { count: 3, distance: 60 };
  const executor = await dieRetryAgainst(bot, 3);

  const trials = executor.deathTrials();
  assert.deepEqual(
    trials.map((t) => t.outcome),
    ["re-engaged", "re-engaged"],
    "alive-but-wandered is the fight still existing",
  );
  assert.deepEqual(dieRetryFindings(trials), []);
  assert.ok(trials[0]!.reengage!.farthest! > 48, "and how far they had strayed is recorded");
});

test("the re-engage probe SETTLES instead of sampling the instant it arrives", async () => {
  // The other half of r14: a client learns about an entity when the server sends
  // it, which takes ticks after arrival. One instantaneous sample read an empty
  // room; the probe now waits for the room to fill.
  const bot = new CombatFakeBot();
  bot.seat(2);
  bot.reSeat = { count: 2 };
  bot.reSeatVisibleAfterMs = 900; // tracking catches up well after the walk back
  const executor = await dieRetryAgainst(bot, 2);

  const trials = executor.deathTrials();
  assert.deepEqual(
    trials.map((t) => t.outcome),
    ["re-engaged", "re-engaged"],
  );
  assert.ok(trials[0]!.reengage!.settleMs >= 500, "the probe waited rather than guessed");
  assert.equal(trials[0]!.reengage!.present, 2);
});

// --- bonfire rest steps + the die-retry precondition (compiler #220) ---------

import type { RestStep } from "../src/critical-path.ts";

/** A CombatFakeBot that also publishes a bonfire's `interaction` affordance. */
class BonfireFakeBot extends CombatFakeBot {
  /** Whether the compiler's affordance is actually there to click. */
  affordance = true;
  activated: number[] = [];

  constructor() {
    super();
    this.seat(0);
    this.reSeat = undefined;
  }

  /** The affordance is an entity like any other, so it lives in `entities`.
   * Re-published after every re-seat: a bonfire is rested at, never used up. */
  armBonfire(id: number, pos: FakeVec3): void {
    if (!this.affordance) return;
    this.bonfires ??= new Map();
    this.bonfires.set(id, { id, name: "interaction", height: 0, position: pos });
    this.entities[id] = this.bonfires.get(id)!;
  }
  // Declared without an initialiser on purpose: the BASE constructor calls `seat`,
  // which runs this override before a subclass field initialiser would have run.
  private bonfires?: Map<number, unknown>;

  override seat(count: number, opts: Parameters<CombatFakeBot["seat"]>[1] = {}): void {
    super.seat(count, opts);
    for (const [id, e] of this.bonfires ?? []) this.entities[id] = e;
  }

  async activateEntity(e: { id: number }): Promise<void> {
    this.activated.push(e.id);
    this.calls.push(`activateEntity(${e.id})`);
  }
}

const REST_STEP: RestStep = {
  action: "rest",
  bonfire: 1,
  anchor: "anchor/beach-fire",
  pos: [0, 64, 0],
  command: "/trigger dw.rest set 2",
};

test("a rest CLICKS the bonfire affordance before it chats the trigger", async () => {
  // Order is the whole step. The click fires the `player_interacted_with_entity`
  // advancement whose reward ENABLES `dw.rest`; until then the trigger is disabled
  // and the chat line is a silent no-op — which is how bell round 3 walked past
  // every fire and still respawned on the beach.
  const bot = new BonfireFakeBot();
  bot.armBonfire(55, new FakeVec3(0.5, 64, 0.5));
  const executor = attach(bot);
  executor.useCampaign("the-drowned-bell");

  await executor.rest(REST_STEP);

  assert.deepEqual(bot.calls.filter((c) => c.startsWith("activateEntity") || c.startsWith("chat")), [
    "activateEntity(55)",
    "chat(/trigger dw.rest set 2)",
  ]);
  assert.deepEqual(bot.activated, [55], "the affordance was right-clicked, not just chatted at");
});

test("a bonfire with no affordance to click fails the step loudly", async () => {
  // Nothing to right-click means the rest can never be performed — and a silently
  // skipped rest is exactly the failure this step exists to prevent.
  const bot = new BonfireFakeBot();
  bot.affordance = false;
  const executor = attach(bot);
  executor.useCampaign("the-drowned-bell");

  await assert.rejects(() => executor.rest(REST_STEP), /nothing to right-click/);
  assert.deepEqual(bot.activated, []);
});

test("the die-retry precondition proceeds once the governing bonfire has been rested", async () => {
  const bot = new BonfireFakeBot();
  bot.seat(1); // `seat` re-publishes the tracked set, so the fire is armed after it
  bot.armBonfire(55, new FakeVec3(0.5, 64, 0.5));
  bot.reSeat = { count: 1 };
  const executor = attach(bot);
  executor.useCampaign("the-drowned-bell");
  executor.useRestSteps([{ bonfire: 1, anchor: "anchor/beach-fire", pos: [0, 64, 0], step: 2 }]);
  const plan = combatPlan(1, true);
  const withCp: CombatPlan = {
    ...plan,
    encounters: [{ ...plan.encounters[0]!, checkpoint: [0, 64, 0] }],
  };
  executor.useCombatPlan(withCp, true);

  executor.beginStep(3);
  await executor.rest(REST_STEP); // the party rests at the fire…
  executor.beginStep(9);
  await executor.kill(KILL_STEP); // …so the death loop measures something real

  assert.equal(executor.deathTrials().length, 2, "both scripted deaths were taken");
  assert.deepEqual(executor.dieRetryPreconditionFindings(), []);
  assert.equal(executor.performedRests().length, 1);
});

test("an unrested bonfire skips the scripted death and reports the gap", async () => {
  // Bell round 3: every fire walked past, both trials respawned at world spawn on
  // the far beach, and a 60s walk-back budget judged the CAMPAIGN for a proof that
  // never performed the player loop. No death is taken now — the run still goes
  // red, but on the harness's gap rather than the delve's difficulty.
  const bot = new BonfireFakeBot();
  bot.seat(1);
  bot.reSeat = { count: 1 };
  const executor = attach(bot);
  executor.useCampaign("the-drowned-bell");
  executor.useRestSteps([{ bonfire: 1, anchor: "anchor/beach-fire", pos: [0, 64, 0], step: 2 }]);
  const plan = combatPlan(1, true);
  executor.useCombatPlan(
    { ...plan, encounters: [{ ...plan.encounters[0]!, checkpoint: [0, 64, 0] }] },
    true,
  );

  executor.beginStep(9); // …and the rest step at index 2 was never performed
  await executor.kill(KILL_STEP);

  assert.equal(executor.deathTrials().length, 0, "no death was scripted");
  assert.equal(
    bot.calls.filter((c) => c === "chat(/damage @s 1000 minecraft:generic)").length,
    0,
  );
  const findings = executor.dieRetryPreconditionFindings();
  assert.equal(findings.length, 1);
  assert.match(findings[0]!, /no checkpoint armed/);
  assert.match(findings[0]!, /passed bonfire 1 \(anchor\/beach-fire\) without resting/);
});

// ---------------------------------------------------------------------------
// The actor floor gate (#114): the other shape an elite takes
// ---------------------------------------------------------------------------

import type { ActorEncounter, CombatPlan as CP } from "../src/combat.ts";
import { displayNameOf } from "../src/executor.ts";

/** The body an `unleash-actor` beat leaves standing: a real-AI twin of the actor's
 * entity type, at the actor's anchor cell, wearing its custom name. */
class ActorFakeBot extends InteractFakeBot {
  /** Swings needed before the body goes down — the fight's whole difficulty here. */
  hitsToKill = 1;
  private hits = 0;

  seatBody(id = 500, pos: [number, number, number] = [1, 64, 0]): void {
    this.entities[id] = {
      id,
      name: "wither_skeleton",
      height: 2.4,
      customName: "Barrow Warden",
      position: new FakeVec3(pos[0], pos[1], pos[2]),
    };
  }

  attack(mob: { id: number }): void {
    this.calls.push("attack");
    if (++this.hits >= this.hitsToKill) delete this.entities[mob.id];
  }

  async lookAt(): Promise<void> {}
  nearestEntity(): unknown {
    return undefined;
  }
}

const BARROW_WARDEN: ActorEncounter = {
  actor: "actor/barrow-warden",
  entity: "minecraft:wither_skeleton",
  name: "Barrow Warden",
  tier: "elite",
  anchor: "anchor/wave",
  pos: [0, 64, 0],
  tag: "dw_actor_barrow_warden",
  vulnerable: false,
  spawnedBy: [],
  unleashedBy: [
    {
      site: "objective",
      owner: "quest/the-barrow",
      objective: "obj/hold-the-gate",
      path: "/content/quests/0/on_objective_complete/obj~1hold-the-gate/1",
    },
  ],
  floorGate: { covered: true },
  maxHealth: 60,
};

function actorPlan(actors: ActorEncounter[] = [BARROW_WARDEN]): CP {
  return {
    version: "0.8.0",
    campaignId: "souls-bonfire",
    difficulty: "normal",
    encounters: [],
    actors,
    floorGate: { present: true, covered: [], notCovered: [] },
  };
}

/** Drive an objective to completion with the actor gate armed. */
async function completeObjective(
  bot: ActorFakeBot,
  env: Record<string, string | undefined> = {},
  actors: ActorEncounter[] = [BARROW_WARDEN],
): Promise<MineflayerExecutor> {
  const executor = attach(bot, env);
  executor.useCampaign("souls-bonfire");
  executor.usePathObjectives(["obj/hold-the-gate"]);
  executor.useCombatPlan(actorPlan(actors), false, env["DELVEWRIGHT_ACTOR_FLOOR"] !== "0");
  executor.beginStep(3);
  bot.emit("messagestr", "[dw:complete souls-bonfire obj/hold-the-gate]");
  await executor.requireObjective("obj/hold-the-gate", "test");
  return executor;
}

test("the objective that unleashes a billed actor starts one unassisted fight", async () => {
  const bot = new ActorFakeBot();
  bot.seatBody();
  const executor = await completeObjective(bot);

  const trials = executor.actorFightTrials();
  assert.equal(trials.length, 1);
  assert.equal(trials[0]!.actor, "actor/barrow-warden");
  assert.equal(trials[0]!.afterObjective, "obj/hold-the-gate");
  assert.equal(trials[0]!.outcome, "won-first-try");
  assert.ok(trials[0]!.swings >= 1);
  // …and beating a billed fight cold is the advisory the inverted gate exists for.
  const findings = executor.floorGateFindings();
  assert.equal(findings.length, 1);
  assert.match(findings[0]!, /billed `elite`/);
  // NO assist window was opened: nothing downstream waits on an actor fight, so
  // there is no obligation to win it and nothing to unblock (spec-0023's assist
  // exists for fights the run must finish).
  assert.deepEqual([...executor.assistWindows()], []);
});

test("a body that never appears is `body-not-found`, never a win", async () => {
  // The silence this closes: an unleash beat that did not fire looks exactly like
  // a fight won instantly if the outcome is inferred from an empty room.
  const bot = new ActorFakeBot(); // no body seated
  const executor = await completeObjective(bot);
  const trial = executor.actorFightTrials()[0]!;
  assert.equal(trial.outcome, "body-not-found");
  assert.equal(trial.swings, 0);
  assert.match(trial.detail!, /no live `minecraft:wither_skeleton`/);
  assert.deepEqual([...executor.floorGateFindings()], []);
});

test("the actor gate fires once per actor, however often the marker is re-broadcast", async () => {
  const bot = new ActorFakeBot();
  bot.seatBody();
  const executor = await completeObjective(bot);
  bot.seatBody(501);
  await executor.requireObjective("obj/hold-the-gate", "test again");
  assert.equal(executor.actorFightTrials().length, 1);
});

test("DELVEWRIGHT_ACTOR_FLOOR=0 skips the fight and records no measurement", async () => {
  const bot = new ActorFakeBot();
  bot.seatBody();
  const executor = await completeObjective(bot, { DELVEWRIGHT_ACTOR_FLOOR: "0" });
  assert.deepEqual([...executor.actorFightTrials()], []);
  assert.deepEqual([...executor.floorGateFindings()], []);
  assert.equal(bot.calls.includes("attack"), false);
});

test("an actor no on-path objective unleashes is never engaged", async () => {
  const bot = new ActorFakeBot();
  bot.seatBody();
  const ambient: ActorEncounter = {
    ...BARROW_WARDEN,
    unleashedBy: [
      {
        site: "trigger",
        owner: "trigger/warden-answers",
        path: "/content/triggers/0/effects/1",
        on: "strike-npc",
        npc: "npc/keeper",
      },
    ],
  };
  const executor = await completeObjective(bot, {}, [ambient]);
  assert.deepEqual([...executor.actorFightTrials()], []);
  assert.equal(bot.calls.includes("attack"), false);
});

test("a custom name is read from every shape mineflayer hands it back in", () => {
  assert.equal(displayNameOf({ customName: "Barrow Warden" }), "Barrow Warden");
  assert.equal(displayNameOf({ displayName: { text: "Barrow Warden" } }), "Barrow Warden");
  assert.equal(
    displayNameOf({ displayName: { toString: () => "Barrow Warden" } }),
    "Barrow Warden",
  );
  assert.equal(displayNameOf({}), undefined);
  assert.equal(displayNameOf({ displayName: {} }), undefined);
});

test("an encounter with NO governing checkpoint skips the death as an ADVISORY, not a red", async () => {
  // Post-#223 (`fire_step < i`) souls-bonfire's encounter truthfully reports no
  // governing checkpoint: the only fire is armed by the very kill this encounter
  // IS, so nothing is armed when a mid-fight death would land. A death here
  // respawns at world spawn and the retry loop is a full restart of the delve.
  //
  // Three things must all hold, and the third is the one worth pinning: the death
  // is NOT taken (it would measure the delve against world spawn), the gap is
  // NAMED (an unproven loop must never be silent), and it lands in the ADVISORY
  // channel — where the campaign puts its rest points is a content staging
  // judgement the compiler's DW0379/DW0315 rules own, not this stage's.
  const bot = new CombatFakeBot();
  bot.seat(1);
  const executor = attach(bot);
  executor.useCampaign("souls-bonfire");
  const plan = combatPlan(1, false);
  executor.useCombatPlan(
    { ...plan, encounters: [{ ...plan.encounters[0]!, checkpoint: undefined }] },
    true,
  );

  executor.beginStep(9);
  await executor.kill(KILL_STEP);

  assert.equal(executor.deathTrials().length, 0, "no death was scripted");
  assert.equal(
    bot.calls.filter((c) => c === "chat(/damage @s 1000 minecraft:generic)").length,
    0,
  );
  // Advisory, not a failure: nothing here reds the stage.
  assert.deepEqual([...executor.dieRetryPreconditionFindings()], []);
  const advisories = executor.dieRetryPreconditionAdvisories();
  assert.equal(advisories.length, 1);
  assert.match(advisories[0]!, /no governing checkpoint/);
  assert.match(advisories[0]!, /die-retry cannot prove safe death here/);
  // …and coverage stays silent about it, exactly as for the unarmed case: the
  // advisory already says why the loop is unproven, and "never reached this
  // encounter" would be plainly untrue.
  assert.equal(executor.dieRetryPreconditionWaves().has(KILL_STEP.wave), true);
  // The fight itself still happened — only the scripted death was skipped.
  assert.equal(executor.encounterPhase(KILL_STEP.wave), "cleared");
});

// --- the kill loop ends on the CENSUS, never on a lookalike (task #124) ------

test("killing a bystander beside the fight does not clear the wave", async () => {
  // The drowned bell's belfry, reduced: `ambush/the-rafters` puts two husks where
  // the Bellkeeper stands, and the kill loop counted one of them as the wave —
  // `confirmed kill: husk#232 (1/1)` — then walked away from a wither skeleton
  // still very much alive. The objective never completed, so the quest never
  // completed, so the NEXT quest was never armed, so the next step's `interact`
  // click was adjudicated against an unarmed quest and spent. The click was the
  // symptom; this is the cause.
  const bot = new CombatFakeBot();
  bot.seat(1); // the real wave mob, in reach
  bot.waveHitsToKill = 3; // …and it outlives the bystander, as the Bellkeeper did
  bot.addBystander(900, 0.5); // an ambush husk, NEARER — the bot swings at it first
  const executor = attach(bot);
  executor.useCampaign("the-drowned-bell");
  executor.useCombatPlan(combatPlan(1, false), false);

  await executor.kill({ ...KILL_STEP, count: 1 });

  // The step only returned once the WAVE was down; the bystander being killed
  // first bought nothing.
  const left = Object.values(bot.entities).filter(
    (e) => (e as { waveTagged?: boolean }).waveTagged === true,
  );
  assert.equal(left.length, 0, "the wave itself is what has to die");
  assert.equal(bot.entities[900], undefined, "the bystander died on the way, which is fine");
});
