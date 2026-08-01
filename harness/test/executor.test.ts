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
}

// A fake mineflayer Bot: an EventEmitter with just the surface the executor touches
// (entity.position, game.gameMode, username, pathfinder.stop). Cast to Bot at the
// attach seam — tests may use structural fakes the full type can't express.
class FakeBot extends EventEmitter {
  username = "delve-bot";
  entity = { position: new FakeVec3(0, 64, 0), onGround: true };
  game = { gameMode: "adventure" as "adventure" | "spectator" };
  pathfinderStops = 0;
  pathfinder = { stop: (): void => void (this.pathfinderStops += 1) };
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
