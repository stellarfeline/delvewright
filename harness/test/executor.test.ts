import { test } from "node:test";
import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import type { Bot } from "mineflayer";
import { MineflayerExecutor, completionWindowMs, type BotConfig } from "../src/executor.ts";
import { BotDeathError } from "../src/death.ts";
import type { AssertCompleteStep } from "../src/critical-path.ts";
import { lethalTrialFailures, parseDeathPlan } from "../src/death-loop.ts";

/** The `dsl_version` an emitted plan carries.
 *
 * The harness never compares it to anything: `death-loop.ts` and `combat.ts`
 * require a non-empty string and pass it through, and nothing downstream reads
 * it. So these fixtures name a number that is deliberately NOT the engine's —
 * a bump moves one file, and this one is not it, and a fixture that started
 * agreeing with the engine would be claiming a coupling the harness does not
 * have.
 */
const PLAN_VERSION = "0.0.0-fixture";

// Minimal Vec3 stand-in with the methods the executor reads off bot.entity.position.
/**
 * The attack speed the fake server sends: 20 swings a second, so the bot's
 * full-charge wait (`fullChargeMs`) is two ticks and a test of the fight's LOGIC
 * does not sit through a sword's real cooldown. The cadence itself is
 * `melee.test.ts`'s subject.
 */
const FAKE_WEAPON = { "generic.attack_speed": { value: 20, modifiers: [] } };

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
  entity = { position: new FakeVec3(0, 64, 0), onGround: true, attributes: FAKE_WEAPON };
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
  executor.useNonCombatants(new Set(["mannequin", "villager"]));
  executor.attachBot(bot as unknown as Bot);
  return executor;
}

test("a death event records position + likely cause and stops the pathfinder", () => {
  const bot = new FakeBot();
  // Deliberately a position where FLOOR and ROUND disagree on both horizontal
  // axes. The block an entity is in is the floor of its position, and this is
  // the position a live run measured: a body killed by the west pit died at
  // x = 12.59, one cell east of a box whose face is at 12.0. Rounding put it in
  // cell 13 — two cells out — and the death-loop credit rule reads this position.
  bot.entity.position = new FakeVec3(12.4, 65, -3.6);
  const executor = attach(bot);
  // The death message arrives in chat, then the death event fires.
  bot.emit("messagestr", "delve-bot was slain by Zombie");
  bot.emit("death");

  const diag = executor.deathDiagnostic();
  assert.ok(diag instanceof BotDeathError);
  // EXACT. Rounding it produced a triple that is neither the position nor the
  // cell the body was in, and the death-loop stage read it as a cell.
  assert.deepEqual(diag.position, [12.4, 65, -3.6]);
  assert.equal(diag.likelyCause, "delve-bot was slain by Zombie");
  assert.equal(bot.pathfinderStops, 1); // in-flight pathfinding aborted
  // …and the stop is ALWAYS paired with a goal reset. mineflayer-pathfinder's `stop()`
  // only raises an internal flag that a later `goto` consumes by rejecting instantly
  // ("Path was stopped before it could be completed!") — leaving it raised poisons every
  // subsequent hop, which is how a the-drowned-bell run failed a leg it had walked
  // fine the run before. The reset consumes the flag here, once.
  assert.deepEqual(bot.pathfinderCalls, ["stop", "setGoal(null)"]);
});

test("the completion window covers an exported scheduled-ending tail", () => {
  // No tail (synchronous ending): the historical 15s settle window.
  assert.equal(completionWindowMs(undefined), 15_000);
  // A short tail stays inside the default window — never narrowed.
  assert.equal(completionWindowMs(20), 15_000); // 1s + 10s margin < 15s
  // the-wake's 250t sequence tail: 12.5s + 10s margin — the old flat 15s
  // window could expire while the ending was still legitimately scheduled.
  assert.equal(completionWindowMs(250), 22_500);
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
  // stage runs (observed live on the keep-trial fixture).
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

// --- entity-tracker settle race (2026-08-06 island triage) -------------------

interface FakeEntity {
  id: number;
  type: string;
  name: string;
  position: FakeVec3;
  customName?: string;
}

class EntityTrackingFakeBot extends FakeBot {
  entities: Record<number, FakeEntity> = {};
}

function fakeEntity(
  id: number,
  opts: { type?: string; name?: string; customName?: string; position?: FakeVec3 } = {},
): FakeEntity {
  return {
    id,
    type: opts.type ?? "mob",
    name: opts.name ?? "warden",
    customName: opts.customName,
    position: opts.position ?? new FakeVec3(0, 64, 0),
  };
}

test("awaitEntitySettle resolves once the non-player entity count holds steady", async () => {
  const bot = new EntityTrackingFakeBot();
  const executor = attach(bot);
  bot.entities[100] = fakeEntity(100);
  bot.entities[101] = fakeEntity(101);
  const started = Date.now();
  await executor.awaitEntitySettle();
  assert.ok(
    Date.now() - started < 2_000,
    "settles well inside the default poll budget once the count stops changing",
  );
});

test("awaitEntitySettle does not settle on a count that is still growing from late packets", async () => {
  const bot = new EntityTrackingFakeBot();
  const executor = attach(bot);
  // Packets land one at a time, the way the island's world-persisted actors did —
  // a settle read taken too early would see a non-empty tracker and stop waiting.
  bot.entities[200] = fakeEntity(200);
  setTimeout(() => {
    bot.entities[201] = fakeEntity(201);
  }, 150);
  setTimeout(() => {
    bot.entities[202] = fakeEntity(202);
  }, 350);
  const started = Date.now();
  await executor.awaitEntitySettle();
  const elapsed = Date.now() - started;
  assert.equal(Object.keys(bot.entities).length, 3, "waited for every packet, not just the first");
  assert.ok(elapsed > 350, `resolved at ${elapsed}ms — before the last packet even landed`);
  assert.ok(elapsed < 3_000, `took ${elapsed}ms to settle after the last packet landed`);
});

test("awaitEntitySettle gives up after its bounded timeout when nothing ever populates", async () => {
  const bot = new EntityTrackingFakeBot(); // entities stays empty — a legitimately quiet spawn
  const executor = attach(bot, { DELVEWRIGHT_ENTITY_SETTLE_TIMEOUT_MS: "250" });
  const started = Date.now();
  await executor.awaitEntitySettle(); // must not hang the run
  const elapsed = Date.now() - started;
  assert.ok(elapsed >= 250 && elapsed < 1_500, `gave up near the 250ms bound (took ${elapsed}ms)`);
});

// --- scripted-teardown death classification (2026-08-06 island triage) -------

test("a named entity's death is recorded with its last known position", () => {
  const bot = new EntityTrackingFakeBot();
  const executor = attach(bot);
  bot.emit(
    "entityDead",
    fakeEntity(7, { customName: "island-herdsman", position: new FakeVec3(10, -128, 9) }),
  );
  assert.deepEqual(executor.namedEntityDeaths(), [
    { name: "island-herdsman", entityId: 7, position: [10, -128, 9] },
  ]);
});

test("an unnamed mob's death is not recorded — this ledger is about actors, not every mob", () => {
  const bot = new EntityTrackingFakeBot();
  const executor = attach(bot);
  bot.emit("entityDead", fakeEntity(8, { position: new FakeVec3(5, 64, 5) })); // no customName
  assert.deepEqual(executor.namedEntityDeaths(), []);
});

test("the bot's own death is not recorded here — onDeath owns that diagnostic", () => {
  const bot = new EntityTrackingFakeBot();
  (bot.entity as unknown as { id: number }).id = 1;
  const executor = attach(bot);
  bot.emit(
    "entityDead",
    fakeEntity(1, { type: "player", customName: "delve-bot", position: new FakeVec3(0, 64, 0) }),
  );
  assert.deepEqual(executor.namedEntityDeaths(), []);
});

test("multiple named-entity deaths accumulate in order, undeduplicated", () => {
  const bot = new EntityTrackingFakeBot();
  const executor = attach(bot);
  bot.emit(
    "entityDead",
    fakeEntity(1, { customName: "Hollow Gate-Warder", position: new FakeVec3(10, 63, -4) }),
  );
  bot.emit(
    "entityDead",
    fakeEntity(4, { customName: "island-herdsman", position: new FakeVec3(10, -128, 9) }),
  );
  const deaths = executor.namedEntityDeaths();
  assert.equal(deaths.length, 2);
  assert.equal(deaths[0]!.name, "Hollow Gate-Warder");
  assert.equal(deaths[1]!.name, "island-herdsman");
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

// --- gap 8: cross-area transport hardening ------------------------

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

// --- stall-recovery ---------------------------------------------

import {
  disconnectReason,
  isLivingBody,
  isWaveMob,
  replayLegWithRecovery,
  type Unstick,
} from "../src/executor.ts";
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
/** A fake entity. `type` is the pinned registry's category, which mineflayer
 * copies onto every entity it tracks; the default is the one a wave mob has. */
function ent(name: string | undefined, height = 1.8, type = "hostile"): unknown {
  return { name, height, type, position: { x: 1, y: 64, z: 0 } };
}
/** What the delve says is not a fight. In a real run this comes off
 * `critical-path.json`; here it is written out so each case says which half of
 * the rule it is exercising. */
const CAST = new Set(["mannequin", "villager"]);
const NO_CAST: ReadonlySet<string> = new Set<string>();

test("isWaveMob classifies a living hostile as a wave mob", () => {
  assert.equal(isWaveMob(ent("drowned", 1.95), SELF, CAST), true);
  assert.equal(isWaveMob(ent("zombie"), SELF, CAST), true);
});

test("isWaveMob never targets a body the delve says is not a fight", () => {
  // Both of these used to be literals in the harness. They are the compiler's
  // knowledge, and they arrive as data now — so the same names are targets when
  // the delve does not claim them.
  assert.equal(isWaveMob(ent("mannequin", 1.8), SELF, CAST), false);
  assert.equal(isWaveMob(ent("villager"), SELF, CAST), false);
  assert.equal(
    isWaveMob(ent("mannequin", 1.8), SELF, NO_CAST),
    true,
    "a delve that does not stage mannequins as NPCs has no reason to spare one",
  );
});

test("isWaveMob excludes vanilla non-bodies and the bot itself, cast or no cast", () => {
  // These need no campaign to say so: no delve can make a dropped item a fight.
  for (const name of ["player", "armor_stand", "interaction", "item", "text_display"]) {
    assert.equal(isWaveMob(ent(name), SELF, NO_CAST), false, `${name} must not be a wave mob`);
  }
  assert.equal(isWaveMob(SELF, SELF, CAST), false, "the bot is not its own target");
  assert.equal(isWaveMob(ent(undefined), SELF, CAST), false, "an unnamed entity is not a target");
  assert.equal(
    isWaveMob(ent("item", 0.25, "other"), SELF, CAST),
    false,
    "a short dropped entity is excluded",
  );
});

// Vesperhold's choir: a Drowned Chorister threw its trident at the bot, the
// trident lay beside it, and the mid-fight trade swung at it. A thrown trident is
// an `AbstractArrow`, and vanilla DISCONNECTS a player who attacks one ("Attempting
// to attack an invalid entity") — the bot was kicked, and the scripted death that
// followed "never landed". It was half a block tall, so the height rule passed it.
test("isWaveMob never targets a projectile or any other non-living entity", () => {
  for (const [name, height, type] of [
    ["trident", 0.5, "projectile"],
    ["arrow", 0.5, "projectile"],
    ["fireball", 1, "projectile"],
    ["falling_block", 0.98, "other"],
    ["tnt", 0.98, "other"],
    ["evoker_fangs", 0.8, "other"],
    ["oak_boat", 0.5625, "other"],
    ["experience_orb", 0.5, "orb"],
  ] as const) {
    assert.equal(isWaveMob(ent(name, height, type), SELF, NO_CAST), false, `${name} is not a body`);
    assert.equal(isLivingBody(ent(name, height, type)), false, `${name} is never swung at`);
  }
});

test("isWaveMob targets a living mob of any size, by category rather than height", () => {
  // The height proxy spared a silverfish or an endermite wave for no reason.
  assert.equal(isWaveMob(ent("silverfish", 0.3, "hostile"), SELF, NO_CAST), true);
  assert.equal(isWaveMob(ent("magma_cube", 0.52, "mob"), SELF, NO_CAST), true);
  assert.equal(isLivingBody(ent("zombie")), true);
  assert.equal(isLivingBody(undefined), false);
});

test("a disconnect reason is read out of whatever shape the packet carried", () => {
  assert.equal(
    disconnectReason({
      type: "compound",
      value: { translate: { type: "string", value: "multiplayer.disconnect.invalid_entity_attacked" } },
    }),
    "multiplayer.disconnect.invalid_entity_attacked",
  );
  assert.equal(
    disconnectReason('{"translate":"multiplayer.disconnect.kicked"}'),
    "multiplayer.disconnect.kicked",
  );
  assert.equal(disconnectReason("socketClosed"), "socketClosed");
  assert.equal(disconnectReason({ odd: 1 }), '{"odd":1}');
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

// --- timed-gate crossings (spec-0016 §4) --------------------------

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
  crush: false,
};

/** A GateAssist with a virtual clock, so the bounded wait costs no wall time. */
function fakeGate(
  opts: {
    feet?: () => [number, number, number] | undefined;
    gates?: readonly TimedGate[];
    /** Whether the closed→open edge is OBSERVED by each wait (default yes). */
    observed?: () => boolean;
    /** Event hook so a test can assert wait-vs-goto ordering. */
    onWait?: () => void;
    /** Optional raw-dash stub (crush entries use it when present). */
    dash?: GateAssist["dash"];
  } = {},
): GateAssist & {
  waits: number;
  clock: { t: number };
  holds: Array<readonly number[] | undefined>;
  presses: Array<readonly number[] | undefined>;
} {
  const clock = { t: 0 };
  const state = { waits: 0 };
  const holds: Array<readonly number[] | undefined> = [];
  const presses: Array<readonly number[] | undefined> = [];
  return {
    gates: opts.gates ?? [PORTCULLIS],
    // Each wait advances the virtual clock by one full cycle.
    waitForWindow: async (_gates, hold, press) => {
      state.waits++;
      clock.t += 10_000;
      holds.push(hold);
      presses.push(press);
      opts.onWait?.();
      return opts.observed?.() ?? true;
    },
    feetCell: opts.feet ?? (() => [24, 63, -9]),
    dash: opts.dash,
    now: () => clock.t,
    get waits() {
      return state.waits;
    },
    clock,
    holds,
    presses,
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

test("a walk with no gates bound gets no gate retries — a real regression still fails fast", async () => {
  // Patience is licensed by a DECLARED gate, never by the harness: a campaign that
  // declares none (or a leg the compiler proved crosses none) binds no gate, gets no
  // assist, and a stall is a stall. `gatesBindingWalk` decides that set; here it has
  // already answered "nothing", which is what an absent assist means.
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
  // ordinary stall recovery WITHIN the window, not only after the budget is spent.
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

// --- `crush: true` gates are staged, never entered blind ----------------------
//
// The tide-mill death: `timed-gate/tide` (36t open / 84t closed, phase 55, crush)
// killed the bot on the FIRST live crush-gate encounter, at [261, 62, 13] inside
// the gate corridor. Root cause: the gate machinery above is reactive — it waits
// for a window only AFTER a hop fails. A non-crush gate's worst case is a path
// abort (information); a crush gate's closing edge is an instant, gear-independent
// kill, so the first "failure" is the bot's death and no retry ever runs. A hop
// whose straight mouth-to-mouth segment crosses a crush gate must therefore be
// STAGED: hold at the compiler-pinned mouth cell, observe a fresh closed→open
// edge, check the crossing fits the window with margin, and only then enter.

/** The tide-mill crusher, as the live artifact exported it (short window, phase
 * offset — the phase is metadata; entry timing is OBSERVED, never computed). */
const TIDE: TimedGate = {
  id: "timed-gate/tide",
  min: [258, 61, 13],
  max: [262, 63, 14],
  block: "minecraft:polished_deepslate",
  openTicks: 36,
  closedTicks: 84,
  phase: 55,
  crush: true,
};

/** The tide leg: mouth cell before the region, mouth cell after, then the anchor. */
const TIDE_GOALS = [G(260, 61, 12), G(260, 61, 15), G(260, 61, 24, 3)];

test("a crush-gate crossing is staged: fresh window observed BEFORE any entry", async () => {
  const events: string[] = [];
  const gate = fakeGate({
    gates: [TIDE],
    feet: () => [260, 61, 12], // staged at the near mouth, fully outside the fill
    onWait: () => events.push("wait"),
  });
  const goto = async (_spec: GoalSpec, label: string): Promise<void> => {
    events.push(label);
  };
  await replayLegWithRecovery(TIDE_GOALS, "interact anchor/objective", goto, undefined, gate);
  assert.equal(gate.waits, 1, "one fresh window for the one crossing hop");
  const wait = events.indexOf("wait");
  const entry = events.findIndex((e) => e.includes("waypoint 2/3"));
  assert.ok(entry >= 0, `the crossing hop ran: ${events.join(" | ")}`);
  assert.ok(wait >= 0 && wait < entry, `window precedes entry: ${events.join(" | ")}`);
  assert.ok(
    events[entry]!.includes("gate attempt"),
    `the crossing entry is a staged gate attempt, never a plain hop: ${events.join(" | ")}`,
  );
  // The approach and the post-gate hop are ordinary hops — staging is scoped to
  // the crossing, not smeared over the leg.
  assert.equal(events[0], "interact anchor/objective waypoint 1/3");
  assert.ok(!events[0]!.includes("gate attempt"));
  assert.equal(events[events.length - 1], "interact anchor/objective");
  // The wait HOLDS the staging stance (the live tide-mill lesson: the corridor
  // current carried an idle bot 8 blocks off the mouth during one 4 s wait).
  assert.deepEqual(gate.holds, [[260, 61, 12]], "the wait pins the bot to the mouth cell");
  // …and PRESSES into the shut plane toward the crossing target, so the open edge
  // releases a contact-started bot (from a standing start the pour is a wall).
  assert.deepEqual(gate.presses, [[260, 61, 15]], "the wait leans into the closed gate");
});

test("a crush entry crosses RAW: the dash runs mouth-to-mouth before any pathfinder hop", async () => {
  // The live lesson, round 3: even a fresh-edge pathfinder entry lost the 1.8 s
  // window (start latency + mid-water replans against the flood through the
  // opened plane) and the closing edge killed the bot mid-crossing. The crossing
  // span itself is raw physics; the pathfinder only finishes the arrival.
  const events: string[] = [];
  const gate = fakeGate({
    gates: [TIDE],
    feet: () => [260, 61, 12],
    dash: async (through, from, to) => {
      events.push(`dash [${from.join(",")}] -> [${to.join(",")}] through ${through[0]!.id}`);
      return true;
    },
  });
  const goto = async (_spec: GoalSpec, label: string): Promise<void> => {
    events.push(label);
  };
  await replayLegWithRecovery(TIDE_GOALS, "interact anchor/objective", goto, undefined, gate);
  const dash = events.findIndex((e) => e.startsWith("dash "));
  const entry = events.findIndex((e) => e.includes("gate attempt"));
  assert.ok(dash >= 0, `the raw dash ran: ${events.join(" | ")}`);
  assert.ok(dash < entry, `dash precedes the pathfinder arrival hop: ${events.join(" | ")}`);
  assert.equal(
    events[dash],
    "dash [260,61,12] -> [260,61,15] through timed-gate/tide",
    "the dash is the staged mouth-to-mouth crossing",
  );
});

test("a dash that cannot clear fails its attempt and takes the NEXT window — never lingers", async () => {
  const gate = fakeGate({
    gates: [TIDE],
    feet: () => [260, 61, 12],
    dash: undefined, // set below, needs the clock
  });
  let dashes = 0;
  (gate as { dash?: GateAssist["dash"] }).dash = async () => {
    dashes++;
    gate.clock.t += 3_000; // the failed dash consumed the whole window
    return dashes > 1; // second window's dash clears
  };
  const labels: string[] = [];
  const goto = async (_spec: GoalSpec, label: string): Promise<void> => {
    labels.push(label);
  };
  await replayLegWithRecovery(TIDE_GOALS, "interact anchor/objective", goto, undefined, gate);
  assert.equal(dashes, 2, "one failed dash, one clean one");
  assert.equal(gate.waits, 2, "the retry waited for its own fresh window");
  assert.ok(
    !labels.some((l) => l.includes("recovery")),
    `no pathfinder escalation into the spent window: ${labels.join(" | ")}`,
  );
});

test("a bot the current carried off the mouth is re-staged, never margin-failed from the drift", async () => {
  // The live tide-mill failure mode after staging landed: the corridor is flowing
  // water, the idle bot drifted from the mouth [260,61,12] back to the pool at
  // [260,61,4] during the window wait, and the margin check — honestly — refused
  // an 8-block dash through a 1.8 s window. A drifted margin read is a stance
  // problem: walk back to the mouth and take the next window. The hard margin
  // failure is reserved for a bot verifiably ON the pinned mouth.
  let feet: [number, number, number] = [260, 61, 12];
  let drifted = false;
  const gate = fakeGate({
    gates: [TIDE],
    feet: () => feet,
    onWait: () => {
      if (!drifted) {
        drifted = true;
        feet = [260, 61, 4]; // the tide won the first wait
      }
    },
  });
  const labels: string[] = [];
  const goto = async (_spec: GoalSpec, label: string): Promise<void> => {
    labels.push(label);
    if (label.includes("re-stage")) feet = [260, 61, 12]; // walked back to the mouth
  };
  await replayLegWithRecovery(TIDE_GOALS, "interact anchor/objective", goto, undefined, gate);
  assert.ok(
    labels.some((l) => l.includes("gate re-stage")),
    `the drifted bot was walked back to the mouth: ${labels.join(" | ")}`,
  );
  assert.equal(gate.waits, 2, "the re-staged attempt waited for its own fresh window");
  assert.ok(
    labels.some((l) => l.includes("gate attempt 2")),
    `the crossing then ran from the mouth: ${labels.join(" | ")}`,
  );
});

test("a crush gate whose window edge cannot be observed is never entered blind", async () => {
  // The lethal defect inverted: when the harness cannot SEE a fresh window it must
  // refuse the crossing and fail loudly — "crossing anyway" is only survivable on
  // gates that merely block.
  const attempts: string[] = [];
  const gate = fakeGate({
    gates: [TIDE],
    feet: () => [260, 61, 12],
    observed: () => false,
  });
  const goto = async (_spec: GoalSpec, label: string): Promise<void> => {
    if (label.includes("waypoint 2/3")) attempts.push(label);
  };
  await assert.rejects(
    () => replayLegWithRecovery(TIDE_GOALS, "interact anchor/objective", goto, undefined, gate),
    (err: unknown) => {
      assert.ok(err instanceof Error);
      assert.match(err.message, /timed-gate\/tide/);
      assert.match(err.message, /refusing blind entry/);
      return true;
    },
  );
  assert.equal(attempts.length, 0, `no entry was ever attempted: ${attempts.join(" | ")}`);
  assert.ok(gate.waits >= GATE_MIN_ATTEMPTS, "the refusal still burned the bounded budget");
});

test("a bot caught inside a crush gate's cells stands off BEFORE waiting — no dwell in the fill", async () => {
  const events: string[] = [];
  // The bot reaches the mouth, then drifts INTO the region (range-1 tolerance) —
  // the one place the closing fill kills. The staged crossing must pull it out
  // before any waiting or entering happens.
  let feet: [number, number, number] = [260, 61, 12];
  const gate = fakeGate({
    gates: [TIDE],
    feet: () => feet,
    onWait: () => events.push("wait"),
  });
  const goto = async (_spec: GoalSpec, label: string): Promise<void> => {
    events.push(label);
    if (label.includes("waypoint 1/3")) feet = [260, 61, 13]; // drifted into the fill
    if (label.includes("standoff")) feet = [260, 61, 12]; // the standoff pulls it out
  };
  await replayLegWithRecovery(TIDE_GOALS, "interact anchor/objective", goto, undefined, gate);
  const standoff = events.findIndex((e) => e.includes("standoff"));
  const wait = events.indexOf("wait");
  const entry = events.findIndex((e) => e.includes("gate attempt"));
  assert.ok(standoff >= 0, `stood off out of the fill: ${events.join(" | ")}`);
  assert.ok(standoff < wait && wait < entry, `standoff → wait → enter: ${events.join(" | ")}`);
});

test("a fresh window too short for the crossing is refused loudly, not gambled", async () => {
  // DW0378 proves every shipped window admits its crossing, so this can only fire
  // when the bot is staged off the proven mouth or the artifact disagrees with the
  // world — entering would gamble the bot's life on a proof that no longer applies.
  const sliver: TimedGate = { ...TIDE, id: "timed-gate/sliver", openTicks: 2 };
  const entries: string[] = [];
  const gate = fakeGate({ gates: [sliver], feet: () => [260, 61, 12] });
  const goto = async (_spec: GoalSpec, label: string): Promise<void> => {
    if (label.includes("waypoint 2/3")) entries.push(label);
  };
  await assert.rejects(
    () => replayLegWithRecovery(TIDE_GOALS, "interact anchor/objective", goto, undefined, gate),
    (err: unknown) => {
      assert.ok(err instanceof Error);
      assert.match(err.message, /timed-gate\/sliver/);
      assert.match(err.message, /full margin/);
      return true;
    },
  );
  assert.equal(entries.length, 0, "the too-short window was never entered");
});

test("a failed crush entry does not escalate into a stale window — it takes the next fresh one", async () => {
  // The within-window walking escalation (the-drowned-bell lesson) stays available,
  // but never into a crush gate whose remaining window no longer fits the crossing:
  // bursting into a closing crusher is exactly the death this staging prevents.
  const gate = fakeGate({ gates: [TIDE], feet: () => [260, 61, 12] });
  const labels: string[] = [];
  let failedOnce = false;
  const goto = async (_spec: GoalSpec, label: string): Promise<void> => {
    labels.push(label);
    if (label.includes("gate attempt 1") && !failedOnce) {
      failedOnce = true;
      // The failed attempt consumed more than the 1.8s window.
      gate.clock.t += 3_000;
      throw new Error("Path was stopped before it could be completed!");
    }
  };
  await replayLegWithRecovery(TIDE_GOALS, "interact anchor/objective", goto, undefined, gate);
  assert.equal(gate.waits, 2, "the retry waited for the NEXT fresh window");
  assert.ok(
    !labels.some((l) => l.includes("recovery")),
    `no recovery re-path into the stale window: ${labels.join(" | ")}`,
  );
  assert.ok(labels.some((l) => l.includes("gate attempt 2")), labels.join(" | "));
});

// --- completion signals outrank position --------------------------------------
//
// The tide-mill wheelpit defect: `obj/wheelpit` sits right past a timed-gate
// crossing and its completion emission teleports the player to the next area (a
// physically one-way transport). The bot crossed the sluice, the objective
// completed — and the leg's remaining hops then failed on the position
// discontinuity, which the harness read as the gate blocking a leg it had already
// walked: three gate "attempts", then a re-center toward a cell the one-way
// transport makes unreachable. Objective complete ⇒ the leg SUCCEEDED.

test("a gate-leg hop failure is SUCCESS when the step settled mid-crossing (tide-mill wheelpit)", async () => {
  const gate = fakeGate();
  const calls: string[] = [];
  const goto = async (_spec: GoalSpec, label: string): Promise<void> => {
    calls.push(label);
    // The completion teleport landed and stopped the pathfinder mid-hop.
    if (label.includes("waypoint 2/3")) {
      throw new Error("Path was stopped before it could be completed!");
    }
  };
  await replayLegWithRecovery(
    [G(24, 63, 4), G(24, 63, -11), G(24, 63, -14, 3)],
    "anchor anchor/wheelpit",
    goto,
    undefined,
    gate,
    // The oracle already reports the step settled (marker arrived / transport
    // landed) by the time the failure is judged.
    () => "objective obj/wheelpit is complete (its marker arrived)",
  );
  assert.equal(gate.waits, 0, "no window wait: there is no crossing left to make");
  assert.ok(!calls.some((l) => l.includes("gate attempt")), calls.join(" | "));
  assert.ok(!calls.some((l) => l.includes("standoff")), calls.join(" | "));
  assert.ok(!calls.some((l) => l.includes("recovery")), calls.join(" | "));
  // The replay ends with the leg: the hops after the one-way transport — which
  // belong to the area the bot was carried out of — are never pathed.
  assert.ok(
    calls.every((l) => l.includes("waypoint")),
    `the old area's final goal was never pathed: ${calls.join(" | ")}`,
  );
});

test("a settle signal landing during the window wait ends the crossing before re-pathing", async () => {
  // The marker is a chat packet racing the position jump — it can arrive while the
  // gate loop is already waiting for a window. The next decision after the wait
  // must be "settled", not another crossing attempt toward the old area.
  const inner = fakeGate();
  let settledNow = false;
  const gate: GateAssist = {
    gates: inner.gates,
    waitForWindow: async (gates) => {
      const observed = await inner.waitForWindow(gates);
      settledNow = true;
      return observed;
    },
    feetCell: inner.feetCell,
    now: inner.now,
  };
  const calls: string[] = [];
  const goto = async (_spec: GoalSpec, label: string): Promise<void> => {
    calls.push(label);
    if (label.includes("waypoint 2/3") || label.includes("gate attempt")) {
      throw new Error("Path was stopped before it could be completed!");
    }
  };
  await replayLegWithRecovery(
    [G(24, 63, 4), G(24, 63, -11), G(24, 63, -14, 3)],
    "anchor anchor/wheelpit",
    goto,
    undefined,
    gate,
    () => (settledNow ? "objective obj/wheelpit is complete (its marker arrived)" : undefined),
  );
  assert.equal(inner.waits, 1, "one window wait, then the settle signal ended the crossing");
  assert.ok(!calls.some((l) => l.includes("gate attempt")), calls.join(" | "));
  assert.ok(
    calls.every((l) => l.includes("waypoint")),
    `no goal beyond the settled leg was pathed: ${calls.join(" | ")}`,
  );
});

test("a non-gate leg hop failure is SUCCESS when the completion transport already landed", async () => {
  const calls: string[] = [];
  const goto = async (_spec: GoalSpec, label: string): Promise<void> => {
    calls.push(label);
    if (label.includes("waypoint 2/3")) {
      throw new Error("No path to the goal!");
    }
  };
  await replayLegWithRecovery(
    [G(0, 65, 0), G(0, 65, 3), G(0, 65, 6, 3)],
    "anchor anchor/next-area",
    goto,
    undefined,
    undefined,
    () => "the step's completion transport landed the bot at its exported destination [260, 61, 4]",
  );
  // No re-center toward the unreachable old-area cell, and no goal beyond the leg.
  assert.ok(!calls.some((l) => l.includes("recovery")), calls.join(" | "));
  assert.ok(
    calls.every((l) => l.includes("waypoint")),
    `the old area's final goal was never pathed: ${calls.join(" | ")}`,
  );
});

test("a settle oracle that never fires leaves the gate failure verdict untouched", async () => {
  // The oracle is not a tolerance: a genuinely blocked leg with an unsettled step
  // fails exactly as before, naming the gate and its cycle.
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
        () => undefined,
      ),
    /timed-gate\/portcullis/,
  );
  assert.ok(gate.waits >= GATE_MIN_ATTEMPTS, "the full retry discipline still ran");
});

// --- interact: the mainhand contract -----------------------------------------

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
  inventory = {
    items: (): Array<{ name: string; type: number }> => this.carried,
    slots: [] as Array<{ name: string } | undefined>,
  };
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
  // `requires_item` is MAINHAND-held: a bot that only carries the item has every
  // trigger swallowed by the datapack guard, and then dies on its own objective
  // timeout. Order is the whole assertion: the guard reads the hand on the tick it
  // consumes the trigger.
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

// --- executor tier: reach + timed gate + completion transport -----------------

import type { ReachStep } from "../src/critical-path.ts";
import { nextLegWaypoints, parseWaypoints } from "../src/waypoints.ts";
import { gatesBindingWalk } from "../src/timed-gate.ts";

/**
 * A bot whose every pathfind ends the tide-mill way: the objective's distance
 * check fires as the bot lands the crossing, the datapack broadcasts the marker
 * and teleports it to the next area, and the forced move stops the pathfinder —
 * so the in-flight `goto` rejects with the exact live-run message.
 */
class TransportReachBot extends FakeBot {
  registry = registryFor("1.21.11");
  health = 20;
  food = 20;
  entities: Record<number, unknown> = {};
  inventory = {
    items: (): Array<{ name: string; type: number }> => [],
    slots: [] as Array<{ name: string } | undefined>,
  };
  gotoCalls = 0;
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
      this.gotoCalls++;
      this.entity.position = new FakeVec3(260.5, 61.0, 4.5);
      this.emit("forcedMove");
      this.emit("messagestr", "[dw:complete the-tide-mill obj/wheelpit]");
      throw new Error("Path was stopped before it could be completed!");
    },
  };
  setControlState(): void {}
}

test("reach: a completion transport landing mid-gate-leg is step success, not a gate failure", async () => {
  // Six live tide-mill runs failed here: `dw.o_wheelpit = 1` on the server while the
  // harness looped "still blocked after 3 timed-gate crossing attempt(s)" and tried
  // to re-center across a one-way transport. The whole reach step must now pass on
  // the authoritative signals, without a single gate retry.
  const bot = new TransportReachBot();
  bot.entity.position = new FakeVec3(4.5, 63.0, 0.5);
  const executor = attach(bot);
  executor.useCampaign("the-tide-mill");
  executor.useWaypoints(
    parseWaypoints({
      version: "0.6.0",
      campaign_id: "the-tide-mill",
      timed_gates: [
        {
          id: "timed-gate/sluice",
          region: { min: [18, 62, -30], max: [22, 64, -30] },
          block: "minecraft:oak_fence",
          open_ticks: 100,
          closed_ticks: 100,
          phase: 0,
        },
      ],
      legs: [
        {
          from: [4, 63, 0],
          to: [20, 63, -40],
          waypoints: [
            [10, 63, -10],
            [20, 63, -30],
          ],
          timed_gates: ["timed-gate/sluice"],
        },
      ],
    }),
  );
  executor.beginStep(4);
  const step: ReachStep = {
    action: "reach",
    objective: "obj/wheelpit",
    anchor: "anchor/wheelpit",
    pos: [20, 63, -40],
    radius: 3,
    completion: { kind: "cube" as const, lo: [17, 60, -43], hi: [23, 66, -37] },
    transport: [260, 61, 4],
  };
  const started = Date.now();
  await executor.reach(step); // resolves — before the fix this looped gate retries and threw
  // One hop, retried once by runGoto's own transient-retry — never the gate loop's
  // window waits (each up to a full cycle + 15s margin) or its re-center recovery.
  assert.ok(bot.gotoCalls <= 2, `no gate-loop retries: ${bot.gotoCalls} pathfinds`);
  assert.ok(
    Date.now() - started < 10_000,
    "the step settled on the completion signals, not on a spent gate budget",
  );
});

// --- the die-retry stage: the run artifact must never lose a death ---

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
  /** Whether the seated bodies wear the wave's tag (default `true`). `false`
   * stands in for a hostile that belongs to no wave — an ambusher the census
   * cannot see and the objective does not require dead. */
  waveTagged?: boolean;
  /** Swings these bodies take, whatever their tagging. Without it a tagged body
   * takes `waveHitsToKill` and an untagged bystander takes one. */
  hitsToKill?: number;
  /**
   * How many of the freshly seated bodies the WORLD kills the instant they land,
   * with nobody credited.
   *
   * The gallery seats `wave/muster` within a stride of `lethal/east-pit` and a
   * drop, and on the ladder run that measured this, one of the re-seated cohort
   * withered two seconds after it appeared and another fell one second later.
   */
  worldKills?: number;
  /** The same, but after the re-seat has been read once rather than the instant
   * the cohort lands: vesperhold's choir, whose re-seated drowned swam into the lethal well
   * beside their seat 14–19 s after the re-seat, before the bot got there. */
  worldKillsOnReturn?: number;
}

/** One wave mob as the fake server publishes it to a client. */
interface FakeMob {
  id: number;
  name: string;
  /** The registry category mineflayer copies onto a tracked entity. */
  type: string;
  height: number;
  position: FakeVec3;
  metadata: Record<number, unknown>;
  attributes: Record<string, { value: number }>;
  /** Stands in for the compiler's `dw_wave_<id>` tag: only a mob the wave itself
   * summoned carries it, so the census can never count a bonfire affordance, an
   * ambush actor or a neighbouring wave. */
  waveTagged: boolean;
  /** Swings this particular body takes, overriding the bot-wide rule. */
  hitsToKill?: number;
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
  /** Bodies of the current cohort the world kills once the re-seat has been read. */
  killOnReturn: number[] = [];
  censusSinceSeat = 0;
  /** The server did NOT keep the inventory across the death (a broken
   * `gamerule keep_inventory true` seal). */
  loseKitOnDeath = false;
  /** The wave kills the bot the first time it swings, mid-trade — before the
   * harness gets to script the death this trial asked for. */
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
  /** Ids parked by {@link addBystander}, and the arguments they were parked with. */
  private readonly bystanders = new Set<number>();
  private readonly bystanderArgs = new Map<number, [number, number]>();
  private censusSeq = 0;
  private musterSeq = 0;
  /** The server's credited-kill ledger for the seating in force: `k_reward_<wave>`
   * adds one per `player_killed_entity`, and `spawn_<wave>` zeroes it. A body the
   * WORLD kills moves nothing here, which is the whole distinction. */
  private credited = 0;

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
    // `spawn_<wave>` zeroes the credited-kill ledger: a fresh seating starts a
    // fresh attribution, so the sweep that cleared the last cohort is not counted
    // against this one.
    this.credited = 0;
    this.entities = {};
    for (const id of opts.keepIds ?? []) this.entities[id] = this.makeMob(id, opts);
    const fresh = count - (opts.keepIds?.length ?? 0);
    const seated: number[] = [];
    for (let i = 0; i < fresh; i++) {
      this.entities[this.nextId] = this.makeMob(this.nextId, opts);
      seated.push(this.nextId);
      this.nextId += 1;
    }
    for (const id of seated.slice(0, opts.worldKills ?? 0)) this.worldKill(id);
    this.censusSinceSeat = 0;
    this.killOnReturn = seated.slice(opts.worldKills ?? 0).slice(0, opts.worldKillsOnReturn ?? 0);
    // Bystanders are not of the wave, so the re-seat's tag sweep never touched
    // them: they are still standing where they were.
    for (const id of [...this.bystanders]) {
      const [d, h] = this.bystanderArgs.get(id) ?? [1, 1];
      this.addBystander(id, d, h);
    }
  }

  /** The ids of every body the last seating placed, tagged or not. */
  waveIdsAll(): number[] {
    return (Object.values(this.entities) as FakeMob[])
      .filter((m) => !this.bystanders.has(m.id))
      .map((m) => m.id);
  }

  /** The ids of everything currently wearing the wave tag. */
  waveIds(): number[] {
    return this.waveMobs().map((m) => m.id);
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
      type: "hostile",
      height: 2,
      waveTagged: opts.waveTagged ?? true,
      ...(opts.hitsToKill !== undefined ? { hitsToKill: opts.hitsToKill } : {}),
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
    // The fake server answers the census the way a real one does: by TAG. Only
    // mobs in `entities` carry the wave tag here, so anything a test parks beside
    // the encounter is invisible to it — which is the whole point.
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
      if (fn.includes(":wave_muster_")) {
        // The muster reads the SAME bodies the census counts, and states each
        // one's attributes. The fixture's cohort is exactly what the plan
        // declares, so a clean run reports no finding — a test that wants one
        // moves a body's health.
        this.musterSeq += 1;
        const bodies = this.waveMobs();
        for (const _ of bodies) {
          this.emit(
            "messagestr",
            `[dw:musterbody the-drowned-bell wave/gate-assault ${this.musterSeq} ` +
              `0 1 ${FULL_HEALTH * 1000} 0 0 250 -1 -1 -1]`,
          );
        }
        this.emit(
          "messagestr",
          `[dw:muster the-drowned-bell wave/gate-assault ${this.musterSeq} ` +
            `${bodies.length} ${bodies.length}]`,
        );
        return;
      }
      if (fn.includes(":wave_strike_")) {
        // One attributed blow, one body — and the party is credited, which is the
        // whole reason the staged clear is a `player_attack` and not a `kill`.
        const [first] = this.waveMobs();
        if (first) {
          delete this.entities[first.id];
          this.credited += 1;
        }
        return;
      }
      if (fn.includes(":wave_chip_")) {
        for (const m of this.waveMobs()) {
          m.metadata ??= [];
          m.metadata[ZOMBIE_HEALTH_IDX] = Math.max(1, this.healthOf(m) - 1);
        }
        return;
      }
      if (fn.includes(":wave_census_")) {
        if (this.failReEngageProbe && this.died) return; // the probe never answers
        // The world thins the cohort AFTER the re-seat has been read once: every
        // later answer sees it gone, exactly as the census at the encounter did.
        if (this.censusSinceSeat >= 1 && this.killOnReturn.length > 0) {
          for (const id of this.killOnReturn) this.worldKill(id);
          this.killOnReturn = [];
        }
        this.censusSinceSeat += 1;
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
            `${mobs.length} ${branded} ${damaged} ${this.credited}]`,
        );
        return;
      }
      return;
    }
    if (!message.startsWith("/damage") || !this.scriptedDeathsLand) return;
    // A spectator is invulnerable, so a real server does NOTHING with this and
    // says so. The gallery's muster completion starts two cutscenes, and a
    // cutscene's first act is `gamemode spectator @a`.
    if (this.game.gameMode === "spectator") {
      this.emit("messagestr", "This entity cannot be damaged");
      return;
    }
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
   * census — exactly the drowned bell's belfry. */
  addBystander(id: number, distance = 1, hitsToKill = 1): void {
    const self = this;
    // A re-seat clears the WAVE (`kill @e[tag=dw_wave_<id>]`) and nothing else, so
    // a body belonging to no wave outlives it — which is the whole reason one is
    // parked here. `seat` re-installs these for the same reason.
    this.bystanders.add(id);
    this.bystanderArgs.set(id, [distance, hitsToKill]);
    this.entities[id] = {
      id,
      name: "husk",
      type: "hostile",
      height: 2,
      hitsToKill,
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
  /** Swings that landed on body `id`. */
  hitsOn(id: number): number {
    return this.hitsTaken.get(id) ?? 0;
  }

  attack(mob: { id: number }): void {
    this.calls.push("attack");
    // The wave wins the exchange: the bot dies mid-trade, before it ever reaches
    // the line that scripts its own death. Armed once.
    if (this.killBotOnTrade) {
      this.killBotOnTrade = false;
      this.killBot();
      return;
    }
    const body = this.entities[mob.id] as
      | { waveTagged?: boolean; hitsToKill?: number }
      | undefined;
    const need = body?.hitsToKill ?? (body?.waveTagged ? this.waveHitsToKill : 1);
    const taken = (this.hitsTaken.get(mob.id) ?? 0) + 1;
    this.hitsTaken.set(mob.id, taken);
    if (taken < need) return;
    const ent = this.entities[mob.id] as FakeMob | undefined;
    delete this.entities[mob.id]; // one swing is enough in the fake world
    // A real server announces the removal, and that announcement is what credits
    // a confirmed kill (`entityGone` → `creditsWaveKill`). Without it the fake
    // world could never reproduce the drowned bell's belfry, where a husk's death
    // was credited to the Bellkeeper's wave.
    if (ent) this.emit("entityGone", ent);
    // The server's own credit: vanilla grants `player_killed_entity` and
    // `k_reward_<wave>` records it. Only a PLAYER kill reaches here.
    if (ent?.waveTagged === true) this.credited += 1;
  }

  /**
   * A wave body the WORLD kills — a lethal volume, a fall, a trap, another mob.
   * Gone from the world and gone from the census, with nobody credited: vanilla
   * has no trigger for "this entity died", so no advancement fires and the
   * credited ledger does not move.
   *
   * This is the gallery's `wave/muster`, whose three bodies stand within a stride
   * of `lethal/east-pit` and a drop.
   */
  worldKill(id: number): void {
    const ent = this.entities[id] as FakeMob | undefined;
    delete this.entities[id];
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

function combatPlan(
  count = 1,
  respawnsOnRest = true,
  tier: "ordinary" | "elite" | "boss" = ENCOUNTER.tier,
): CombatPlan {
  return {
    version: "0.6.0",
    campaignId: "the-drowned-bell",
    difficulty: "normal",
    encounters: [
      {
        wave: ENCOUNTER.wave,
        objective: ENCOUNTER.objective,
        step: ENCOUNTER.step,
        tier,
        pos: ENCOUNTER.pos,
        count,
        respawnsOnRest,
        census: ENCOUNTER.census,
        muster: {
          probe: "the-drowned-bell:wave_muster_gate_assault",
          strike: "the-drowned-bell:wave_strike_gate_assault",
          chip: "the-drowned-bell:wave_chip_gate_assault",
          scale: 1000,
          unread: -1,
          bodies: count,
          checked: count + 1,
          types: [
            {
              entity: "minecraft:drowned",
              facts: ["name=Gate Drowned"],
              droppedFacts: [],
              readsAttackDamage: false,
              readsFollowRange: false,
            },
          ],
          profiles: [
            {
              typeIndex: 0,
              count,
              mask: 1,
              label: `${count} × minecraft:drowned \`Gate Drowned\``,
              maxHealth: FULL_HEALTH,
              armorAtLeast: 0,
              armorToughnessAtLeast: 0,
            },
          ],
        },
        checkpoint: ENCOUNTER.checkpoint,
      },
    ],
    runBacks: [],
  };
}

const SCRIPTED_DEATH = "chat(/damage @s 1000 minecraft:generic)";



test("a cutscene's spectator window cannot eat a scripted death", async () => {
  // The gallery, measured: `complete_o_clear_the_muster` fires two cutscenes, and
  // each one opens with `tag @a add dw_cutscene` + `gamemode spectator @a`. A
  // mid-fight trade that finishes the wave therefore completes the objective, the
  // cutscene starts, and the next `/damage @s 1000` hits an invulnerable body. The
  // stage waited its full respawn timeout and then blamed the op seed.
  //
  // The window is the campaign's own number: the `kill` step carries
  // `cutscene_seconds`, exactly as a walking step does, and the stage now waits it
  // out before scripting a death.
  const bot = new CombatFakeBot();
  const executor = attach(bot, { DELVEWRIGHT_CUTSCENE_GRACE_MS: "2000" });
  executor.useCampaign("the-drowned-bell");
  executor.useCombatPlan(combatPlan(), true);

  // The cutscene the fight started: spectator now, control back in a moment.
  bot.game.gameMode = "spectator";
  setTimeout(() => {
    bot.game.gameMode = "adventure";
  }, 400);

  await executor.kill({ ...KILL_STEP, cutsceneSeconds: 1 });

  const trials = executor.deathTrials();
  assert.equal(trials.length, 2, "both scripted deaths were taken");
  assert.deepEqual(
    trials.map((t) => t.abortedWith),
    [undefined, undefined],
    "and neither was abandoned",
  );
  assert.ok(
    trials.every((t) => t.completed),
    "both loops reached a verdict",
  );
});

test("a death that never lands says what it SAW, not what it assumed", async () => {
  // The window outlasts everything the build declared: the stage scripts the death
  // anyway rather than swallowing the finding in its own wait, and the refusal
  // names the gamemode it read and the answer the server gave.
  const bot = new CombatFakeBot();
  const executor = attach(bot, { DELVEWRIGHT_CUTSCENE_GRACE_MS: "200" });
  executor.useCampaign("the-drowned-bell");
  executor.useCombatPlan(combatPlan(), true);
  bot.game.gameMode = "spectator"; // and it never comes back

  await assert.rejects(() => executor.kill({ ...KILL_STEP, cutsceneSeconds: 1 }));

  const t = executor.deathTrials()[0]!;
  assert.match(t.abortedWith ?? "", /spectator/);
  assert.match(t.abortedWith ?? "", /This entity cannot be damaged/);
  assert.doesNotMatch(t.abortedWith ?? "", /is the bot opped\?/);
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
  // The-drowned-bell run five. A re-arm that replays `select-class` is destructive:
  // the `dw.class` trigger is re-enabled for every player on every tick and
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
  // The fault is now a census that never answers — the shape a refused
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

// --- what was waiting at the end of the loop ----------------------------------

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

// --- re-seat fidelity + the wandered-mob false negative ----------

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

test("a cohort the world thins AFTER a whole re-seat is the encounter's finding, not a short re-seat", async () => {
  const bot = new CombatFakeBot();
  bot.seat(4);
  bot.reSeat = { count: 4, worldKillsOnReturn: 2 };
  const executor = await dieRetryAgainst(bot, 4);

  const t = executor.deathTrials()[0]!;
  assert.equal(t.reseat!.present, 4, "the re-seat was read whole the moment it landed");
  assert.equal(t.reengage!.present, 2);
  const [finding] = dieRetryFindings(executor.deathTrials());
  assert.match(String(finding), /re-seat brought back 4 of 4/);
  assert.match(String(finding), /kills its own wave/);
  assert.doesNotMatch(String(finding), /came back SHORT/);
});

test("a server kick is named as the cause, and no scripted death is chatted to a dead socket", async () => {
  const bot = new CombatFakeBot();
  bot.seat(2);
  bot.reSeat = { count: 2 };
  const executor = attach(bot);
  executor.useCampaign("the-drowned-bell");
  executor.useCombatPlan(combatPlan(2, true), true);
  bot.emit("kicked", {
    type: "compound",
    value: { translate: { type: "string", value: "multiplayer.disconnect.invalid_entity_attacked" } },
  });
  await assert.rejects(
    executor.kill({ ...KILL_STEP, count: 2 }),
    /disconnected the bot \(kicked: multiplayer\.disconnect\.invalid_entity_attacked\)/,
  );
  assert.equal(
    bot.calls.filter((c) => c.startsWith("chat(/damage")).length,
    0,
    "nothing is sent after the server dropped the connection",
  );
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
  // The fake's census answers from the same table the delay gates, so the first
  // reading to meet the late cohort is the one taken at the re-seat: that is the
  // probe that must wait rather than guess, and the return probe then finds it.
  assert.ok(trials[0]!.reseat!.settleMs >= 500, "the probe waited rather than guessed");
  assert.equal(trials[0]!.reseat!.present, 2);
  assert.equal(trials[0]!.reengage!.present, 2);
});

// --- bonfire rest steps + the die-retry precondition -------------------------

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
   * Re-published after every re-seat: a bonfire is rested at, never used up.
   *
   * The dimensions are the real ones — the compiler summons every affordance as
   * `minecraft:interaction` with `width:1.0f,height:2.0f` — because the crosshair
   * acquisition that now guards `rest` is ray-vs-hitbox: a stub with no box is a
   * body the ray cannot meet, and a fake that cannot be aimed at proves nothing
   * about a fire that can. */
  armBonfire(id: number, pos: FakeVec3): void {
    if (!this.affordance) return;
    this.bonfires ??= new Map();
    this.bonfires.set(id, { id, name: "interaction", width: 1, height: 2, position: pos });
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

import { displayNameOf } from "../src/executor.ts";

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

test("i18n v2: a translate-component name is read through its fallback", () => {
  // spec-0029: an authored custom name ships as
  // `{"translate": "<l10n key>", "fallback": "<English source>"}`. `fallback` is
  // by construction the English string the plan's `actors[].name` carries, so the
  // same-type preference heuristic keeps matching — and it must NOT depend on
  // whether the installed prismarine-chat resolves an unknown key to its fallback
  // or renders the raw key, which is exactly what the `toString` branch would.
  const component = { translate: "actor.polyphemus.name", fallback: "Polyphemus" };
  assert.equal(displayNameOf({ customName: component }), "Polyphemus");
  assert.equal(displayNameOf({ displayName: component }), "Polyphemus");
  // A component whose translate key resolved to the raw key string must still
  // prefer the fallback, not the key.
  assert.equal(
    displayNameOf({
      customName: { ...component, toString: () => "actor.polyphemus.name" },
    }),
    "Polyphemus",
  );
  // An empty fallback is not a name.
  assert.equal(displayNameOf({ customName: { translate: "x", fallback: "" } }), undefined);
});

test("an encounter with NO governing checkpoint skips the death as an ADVISORY, not a red", async () => {
  // With `fire_step < i`, souls-bonfire's encounter truthfully reports no
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

// --- the kill loop ends on the CENSUS, never on a lookalike ------


test("a cohort the world finished is cleared by the SERVER's answer, not the bot's tally", async () => {
  // The gallery ladder, reduced. `wave/muster` seats three bodies within a stride
  // of `lethal/east-pit` and a drop: two died to the world before the bot swung,
  // it felled the third, and its own tally read `1/3` — a number that could never
  // reach 3 for the rest of the step, because vanilla credits nobody for a mob a
  // volume kills.
  //
  // The bystander is what makes this bind to the CENSUS and to nothing else. It is
  // mob-shaped, in reach, of no wave, and the encounter states no budget for its
  // kind, so every client-side test is pinned open: the tally cannot reach the
  // declared count, `waveEngagementCleared` cannot fire while something hostile is
  // within 32 blocks, and "no eligible mob remains" is never true. Before the
  // census became the terminal condition this step burned its whole 90s budget on
  // a wave that was already down.
  const bot = new CombatFakeBot();
  bot.seat(3);
  bot.reSeat = undefined;
  const [a, b] = bot.waveIds();
  bot.worldKill(a!); // withered in the pit
  bot.worldKill(b!); // hit the ground too hard
  bot.addBystander(900, 2, 10_000); // in reach, and not going anywhere

  const executor = attach(bot);
  executor.useCampaign("the-drowned-bell");
  executor.useCombatPlan(combatPlan(3, false), false);

  const started = Date.now();
  await executor.kill({ ...KILL_STEP, count: 3 });
  assert.ok(
    Date.now() - started < 30_000,
    "the step ends when the server says the wave is down, not when the budget runs out",
  );
  assert.equal(bot.waveIds().length, 0, "the wave really is down");
  assert.ok(bot.entities[900], "…and the bystander is still standing, unkilled");
});


test("a re-seat resets what the SERVER says, and the step follows the server", async () => {
  // Finding 1 in its own shape. A scripted die-retry death re-seats a
  // `respawns_on_rest` wave; the harness's confirmed-kill count starts over on the
  // new cohort, and two of that cohort land in the pit. The tally therefore reads
  // `1/3` for the rest of the step, exactly as it did on the gallery.
  const bot = new CombatFakeBot();
  bot.seat(3);
  bot.reSeat = { count: 3, worldKills: 2 };
  bot.addBystander(900, 2, 10_000);

  const executor = attach(bot);
  executor.useCampaign("the-drowned-bell");
  executor.useCombatPlan(combatPlan(3, true, "elite"), true);

  const started = Date.now();
  await executor.kill({ ...KILL_STEP, count: 3 });
  assert.ok(
    Date.now() - started < 60_000,
    "the re-seated cohort is judged by the census, not by a counter that restarted",
  );
  assert.equal(executor.encounterPhase(KILL_STEP.wave), "cleared");
  assert.equal(bot.waveIds().length, 0);
  // Two of the three fell to the WORLD, so nothing that pays on a player's kill
  // fired for them — the attribution is what says so, and it is the only place
  // that can.
  const attribution = executor.waveAttribution(KILL_STEP.wave);
  assert.equal(attribution.kind, "measured");
  assert.ok(attribution.kind === "measured" && attribution.uncredited > 0);
});

// --- talk-to: the walk-then-trigger contract ----------------------

import type { TalkToStep } from "../src/critical-path.ts";

/**
 * A bot that can be driven through a whole `talk-to` step. Unlike
 * {@link InteractFakeBot} (parked AT its anchor so the walk costs no wall time),
 * this one starts AWAY from the NPC and its `goto` actually moves it — the walk is
 * exactly what is under test here.
 *
 * `gotoFailures` makes the first N pathfinds reject the way a live transient does,
 * so a leg that needed recovery can be told from one that walked clean.
 */
class TalkToFakeBot extends InteractFakeBot {
  /** Every pathfind goal the executor asked for, in order. */
  goals: Array<[number, number, number]> = [];
  /** How many of the next pathfinds reject before one is allowed to land. */
  gotoFailures = 0;
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
    // Optional-parameter shape so this stays assignable to the base fake's
    // zero-argument `goto` (the executor always passes a GoalNear).
    goto: async (goal?: { x: number; y: number; z: number }): Promise<void> => {
      if (!goal) throw new Error("the executor must pass a goal");
      this.goals.push([goal.x, goal.y, goal.z]);
      this.calls.push(`goto(${goal.x},${goal.y},${goal.z})`);
      if (this.gotoFailures > 0) {
        this.gotoFailures -= 1;
        throw new Error("Path was stopped before it could be completed!");
      }
      this.entity.position = new FakeVec3(goal.x + 0.5, goal.y, goal.z + 0.5);
    },
  };
}

/** The island's own step 1: hear Eurylochus out at the beach camp. */
function talkToStep(extra: Partial<TalkToStep> = {}): TalkToStep {
  return {
    action: "talk-to",
    objective: "obj/muster",
    npc: "npc/eurylochus",
    pos: [7, 63, 9],
    command: "/trigger dw.dlg_eurylochus set 4",
    sneak: false,
    ...extra,
  };
}

test("talk-to walks to the NPC, THEN chats the dialog trigger", async () => {
  // The whole step in order: a dialog option is the NPC's, so the bot stands with
  // the NPC before it fires the `/trigger` the button would have run. A talk-to that
  // chatted from wherever it happened to be would pass every campaign whose dialog
  // is reach-free and silently mis-drive every campaign whose dialog is not.
  const bot = new TalkToFakeBot();
  bot.entity.position = new FakeVec3(24.5, 63.0, 30.5); // well outside the NPC's range
  const executor = attach(bot);
  executor.useCampaign("nobodys-cave-island");
  executor.beginStep(1);
  setTimeout(
    () => bot.emit("messagestr", "[dw:complete nobodys-cave-island obj/muster]"),
    20,
  );

  await executor.talkTo(talkToStep());

  assert.deepEqual(bot.calls, [
    "goto(7,63,9)",
    "chat(/trigger dw.dlg_eurylochus set 4)",
  ]);
  // …and the bot really is standing at the NPC when it speaks.
  assert.ok(
    Math.hypot(bot.entity.position.x - 7.5, bot.entity.position.z - 9.5) <= 3,
    `bot spoke from ${bot.entity.position.x}, ${bot.entity.position.z}`,
  );
});

test("a talk-to fires its dialog trigger however the walk ended", async () => {
  // The completion oracle (LegSettled) ends a walk as SUCCEEDED on a failure
  // path when the step's exported transport has already landed the bot. That must
  // never cost the step its dialog trigger: the walk is the means, the `/trigger` IS
  // the step. The island's obj/muster read as "no marker arrived" for a whole batch
  // round, and "the leg was shortcut, so the trigger was never sent" was the first
  // theory — this pins that the trigger is unconditional, so that theory can never
  // become true.
  const bot = new TalkToFakeBot();
  // Standing AT the step's exported transport destination, with a hop that fails:
  // the oracle's transport branch reads settled and ends the leg early.
  bot.entity.position = new FakeVec3(260.5, 61.0, 4.5);
  bot.gotoFailures = 99; // every pathfind rejects — the leg can only end via the oracle
  const executor = attach(bot);
  executor.useCampaign("nobodys-cave-island");
  executor.useWaypoints(
    parseWaypoints({
      version: "0.6.0",
      campaign_id: "nobodys-cave-island",
      legs: [
        {
          from: [260, 61, 4],
          to: [7, 63, 9],
          waypoints: [
            [200, 61, 4],
            [100, 62, 6],
          ],
        },
      ],
    }),
  );
  executor.beginStep(1);
  // The marker arrives ONLY once the trigger has been chatted — exactly as the live
  // datapack behaves — so the leg cannot end on the oracle's marker branch. The only
  // way this walk ends is the transport branch, and the step still has to speak.
  const chat = bot.chat.bind(bot);
  bot.chat = (message: string): void => {
    chat(message);
    if (message.includes("dlg_")) {
      setTimeout(() => bot.emit("messagestr", "[dw:complete nobodys-cave-island obj/muster]"), 20);
    }
  };

  await executor.talkTo(talkToStep({ transport: [260, 61, 4] }));

  assert.ok(
    bot.calls.includes("chat(/trigger dw.dlg_eurylochus set 4)"),
    `the trigger must still be sent; calls were ${bot.calls.join(" | ")}`,
  );
});

// --- a swallowed trigger names itself -----------------------------

import { answersTrigger, swallowedTriggerVerdict, triggerObjective } from "../src/executor.ts";

test("triggerObjective names the scoreboard objective a trigger command drives", () => {
  assert.equal(triggerObjective("/trigger dw.dlg_eurylochus set 4"), "dw.dlg_eurylochus");
  assert.equal(triggerObjective("  /trigger dw.i_brake set 1"), "dw.i_brake");
  // Anything that is not a trigger command has no objective — and therefore gets no
  // verdict clause, rather than a guessed one.
  assert.equal(triggerObjective("/damage @s 1000 minecraft:generic"), undefined);
  assert.equal(triggerObjective("hello"), undefined);
});

test("both of vanilla's answers to a trigger are recognised", () => {
  // Success names the objective; the refusals do not, but every one of them says
  // "trigger". Missing the refusal shape is the expensive direction: it would report
  // a REACHED trigger as unreachable.
  assert.equal(
    answersTrigger("Triggered [dw.dlg_eurylochus] (set value to 4)", "dw.dlg_eurylochus"),
    true,
  );
  assert.equal(answersTrigger("You can't trigger this objective yet", "dw.dlg_eurylochus"), true);
  assert.equal(answersTrigger("This objective is not a trigger", "dw.dlg_eurylochus"), true);
  // Ordinary delve narration is not an answer.
  assert.equal(answersTrigger("The surf gives up its dead.", "dw.dlg_eurylochus"), false);
});

test("a talk-to that times out says whether its trigger reached the delve", async () => {
  // The island's obj/muster symptom, at the executor tier. The trigger IS answered by
  // the server and the objective still does not complete — the delve's own guard
  // consumed it (what a re-used world's already-set score does). Before this, both
  // this and an undelivered command produced the same bare timeout, and telling them
  // apart cost a round of misattributed red runs (round 13, then this one).
  const bot = new TalkToFakeBot();
  bot.entity.position = new FakeVec3(7.5, 63.0, 9.5); // at the NPC: no walk under test
  const executor = attach(bot);
  executor.useCampaign("nobodys-cave-island");
  executor.beginStep(1);
  const chat = bot.chat.bind(bot);
  bot.chat = (message: string): void => {
    chat(message);
    // The server answers the trigger — and nothing else happens.
    setTimeout(() => bot.emit("messagestr", "Triggered [dw.dlg_eurylochus] (set value to 4)"), 10);
  };

  await assert.rejects(
    () => executor.talkTo(talkToStep()),
    (err: Error) => {
      assert.match(err.message, /objective obj\/muster did not complete/);
      assert.match(err.message, /the server ANSWERED/);
      assert.match(err.message, /Triggered \[dw\.dlg_eurylochus\]/);
      assert.match(err.message, /fresh-volumes\.sh --project/);
      return true;
    },
  );
});

test("the verdict tells a swallowed trigger from an undelivered one", () => {
  // The fork itself, in isolation: the wiring above proves the answer is captured,
  // and this proves what the harness concludes from it. Both texts must name the
  // side to look at — the whole point is that a bare timeout named neither.
  const answered = swallowedTriggerVerdict({
    command: "/trigger dw.dlg_eurylochus set 4",
    objective: "dw.dlg_eurylochus",
    lines: ["Triggered [dw.dlg_eurylochus] (set value to 4)"],
  });
  assert.match(answered, /the server ANSWERED/);
  assert.match(answered, /its own guard consumed it/);
  assert.match(answered, /re-used world/);
  assert.match(answered, /fresh-volumes\.sh --project/);

  const silent = swallowedTriggerVerdict({
    command: "/trigger dw.dlg_eurylochus set 4",
    objective: "dw.dlg_eurylochus",
    lines: [],
  });
  assert.match(silent, /the server never answered/);
  assert.match(silent, /harness\/infrastructure failure/);

  // A step that sent no trigger at all (reach, collect, kill) gets no clause —
  // never a guessed one.
  assert.equal(swallowedTriggerVerdict(undefined), "");
});

test("a trigger echo never leaks into the next step's failure", async () => {
  // The echo belongs to the step that sent it. A `reach` step that times out two
  // steps later must not quote the last talk-to's `/trigger` as if it were its own:
  // a diagnostic pointing at the wrong command is worse than no diagnostic.
  const bot = new TalkToFakeBot();
  bot.entity.position = new FakeVec3(7.5, 63.0, 9.5);
  const executor = attach(bot);
  executor.useCampaign("nobodys-cave-island");
  executor.beginStep(1);
  const chat = bot.chat.bind(bot);
  bot.chat = (message: string): void => {
    chat(message);
    setTimeout(() => bot.emit("messagestr", "Triggered [dw.dlg_eurylochus] (set value to 4)"), 10);
    setTimeout(
      () => bot.emit("messagestr", "[dw:complete nobodys-cave-island obj/muster]"),
      20,
    );
  };
  await executor.talkTo(talkToStep());

  // The next step sends no trigger of its own — so its message carries no verdict.
  executor.beginStep(2);
  await assert.rejects(
    () => executor.requireObjective("obj/surf", "reach anchor/surf"),
    (err: Error) => {
      assert.match(err.message, /objective obj\/surf did not complete/);
      assert.doesNotMatch(err.message, /dlg_eurylochus/);
      assert.doesNotMatch(err.message, /the server (ANSWERED|never answered)/);
      return true;
    },
  );
});

// --- die-retry: the re-approach must not take the process down ----------------

import { createRequire } from "node:module";

// The REAL mineflayer-pathfinder goto helper. Every other fake in this file stubs
// `goto` as a promise that resolves, which is why 414 green tests sat beside a live
// run that died: the whole defect is a behaviour of goto.js — the four listeners it
// leaves on the bot, and the fact that ANY later `setGoal` rejects a trip still
// holding them, from a zero timer, outside the caller's stack. A model of that could
// agree with the harness and disagree with the library.
const requireCjs = createRequire(import.meta.url);
const gotoUtil = requireCjs("mineflayer-pathfinder/lib/goto.js") as (
  bot: unknown,
  goal: unknown,
) => Promise<void>;

/**
 * The greyhithe-saltworks rehearsal, in a test.
 *
 * A boss encounter whose wave stands off the anchor, so the mid-fight trade has to
 * WALK into melee; that walk finds no path first time, and the wave kills the bot
 * while the harness is waiting out its retry settle. The bot comes back at the
 * checkpoint — somewhere else — so the re-approach really does have to pathfind,
 * and it is that `setGoal` which rejects whatever trip the death left behind.
 *
 * Live, that rejection had no handler and Node killed the process: `bot-1 exited
 * with code 1`, no run report, a red stage that read as a content verdict on a
 * delve the bot had just walked end to end in two and a quarter minutes.
 */
class RealGotoCombatBot extends CombatFakeBot {
  /** Where the checkpoint puts the bot back — deliberately not the anchor. */
  checkpointAt: [number, number, number] = [20, 64, 0];
  /** The one hop that finds no path and gets the bot killed while it settles. */
  strandAt: [number, number, number] | undefined;
  private goalNow: unknown;

  override pathfinder = {
    stop: (): void => {
      this.pathfinderStops += 1;
      this.pathfinderCalls.push("stop");
    },
    setGoal: (goal: unknown): void => {
      this.pathfinderCalls.push(goal === null ? "setGoal(null)" : "setGoal");
      this.goalNow = goal;
      // index.js:145. This is the line the whole defect hangs on.
      this.emit("goal_updated", goal, false);
      if (goal === null) return;
      const g = goal as { x: number; y: number; z: number };
      setTimeout(() => {
        if (this.goalNow !== goal) return; // a superseded goal never arrives
        this.entity.position = new FakeVec3(g.x, g.y, g.z);
        this.emit("goal_reached");
      }, 5);
    },
    setMovements: (): void => {},
    thinkTimeout: 0,
    // Optional, to stay assignable to the stub it overrides; the executor always
    // passes a goal, and a call without one is a fault in the test, not in the bot.
    goto: (goal?: unknown): Promise<void> => {
      this.calls.push("goto");
      if (goal === undefined) throw new Error("the fake pathfinder was given no goal");
      const g = goal as { x: number; y: number; z: number };
      const strand = this.strandAt;
      if (strand && g.x === strand[0] && g.y === strand[1] && g.z === strand[2]) {
        this.strandAt = undefined;
        // The wave lands the killing blow while the harness sits in its settle
        // between the failed attempt and the retry.
        setTimeout(() => this.dieAtTheCheckpoint(), 200);
        return Promise.reject(new Error("No path to the goal!"));
      }
      return gotoUtil(this, goal);
    },
  };

  /** An unscripted death, delivered as a server delivers one: the message, the
   * death, then a fast respawn AT THE CHECKPOINT. */
  private dieAtTheCheckpoint(): void {
    this.emit("messagestr", "delve-bot was slain by The Court Watch");
    this.emit("death");
    setTimeout(() => {
      this.entity.position = new FakeVec3(...this.checkpointAt);
      this.emit("spawn");
    }, 10);
  }
}


// --- when to stop swinging at one body (the census round) ----------------------



// --- the death loop: a trial that opens over a corpse -----------------------

/** A body that can be driven: the walk in uses raw controls, never the pathfinder. */
class DrivableFakeBot extends FakeBot {
  async lookAt(): Promise<void> {}
  setControlState(): void {}
  clearControlStates(): void {}
  /** Open air, so the declared box has cells a body could be in. */
  override blockAt(): { name: string; boundingBox: string } | null {
    return this.chunkLoaded ? { name: "air", boundingBox: "empty" } : null;
  }
}

/** The smallest plan the stage will walk: one volume, one `on_death`, no stake. */
function oneVolumePlan(): ReturnType<typeof parseDeathPlan> {
  return parseDeathPlan({
    format_version: 2,
    version: PLAN_VERSION,
    campaign_id: "probe",
    lethal_volumes: [
      {
        id: "lethal/the-pit",
        region: { lo: [4, 65, 8], hi: [6, 65, 10] },
        // The volume widened by a player's own half-width, as the compiler
        // computes it: one cell out horizontally, `ceil(height)` down.
        keep_out: { lo: [3, 64, 7], hi: [7, 65, 11] },
        message: "The floor gives way.",
        message_key: null,
        damage_type: "minecraft:fall",
      },
    ],
    on_death: { effects: 1, drops_stake: [] },
    stakes: [],
    placement: { seats: [], regions: [], rows: [] },
    binding: {
      lethal_volumes: 1,
      on_death_effects: 1,
      stakes: 0,
      respawn_seats: 0,
      placement_rows: 0,
      unbound: false,
      reason: null,
    },
  });
}

/** The gallery's west pit, as `delvec` emits it — the volume this rule was measured on. */
function westPitPlan(): ReturnType<typeof parseDeathPlan> {
  return parseDeathPlan({
    format_version: 2,
    version: PLAN_VERSION,
    campaign_id: "gallery",
    lethal_volumes: [
      {
        id: "lethal/west-pit",
        region: { lo: [1, 63, 2], hi: [3, 67, 4] },
        // The compiler's own answer for this box, carried as `delvec` emits it:
        // the volume widened by a player's half-width, one cell out
        // horizontally and `ceil(height)` down.
        keep_out: { lo: [0, 62, 1], hi: [4, 67, 5] },
        message: "The floor in the west corner is not a floor.",
        message_key: "lethal.west-pit.message",
        damage_type: "minecraft:fall",
      },
    ],
    on_death: { effects: 1, drops_stake: [] },
    stakes: [],
    placement: { seats: [], regions: [], rows: [] },
    binding: {
      lethal_volumes: 1,
      on_death_effects: 1,
      stakes: 0,
      respawn_seats: 0,
      placement_rows: 0,
      unbound: false,
      reason: null,
    },
  });
}

test("a lethal trial that opens over an unrecovered death is not credited to the volume", async () => {
  // The previous trial's walk back killed the bot and nothing recovered from it.
  // `stepInto` rethrows the death latch on its first line, so the bot never takes
  // a step; the wait for a NEW death then runs out, and `deathPos` is read off the
  // OLD death — which is how a volume that kills a real player at every cell of
  // its box (measured live, 75 of 75) was reported as one the bot stood in and
  // survived. The repair recovers first, so the trial is a trial.
  const bot = new DrivableFakeBot();
  bot.entity.position = new FakeVec3(0.5, 65, 0.5); // nowhere near the volume
  const executor = attach(bot);
  executor.useDeathPlan(oneVolumePlan());
  bot.emit("death"); // the corpse the previous trial left behind
  bot.emit("spawn"); // …and the respawn nobody consumed

  // The death this trial is actually about, delivered while the walk in is
  // driving. Recorded only if the latch was cleared first — `recordDeath` returns
  // early while a death is already held.
  setTimeout(() => {
    bot.entity.position = new FakeVec3(5.4, 65, 9.4);
    bot.emit("death");
    setTimeout(() => bot.emit("spawn"), 100);
  }, 300);

  await executor.runDeathLoop();

  const trials = executor.deathLoopTrials();
  assert.equal(trials.length, 1);
  const t = trials[0]!;
  assert.equal(t.died, true, "the death inside the volume is this volume's kill");
  assert.deepEqual(
    t.deathPos,
    [5.4, 65, 9.4],
    "and the position is THIS death's, not the last one's",
  );
  assert.equal(t.enteredVolume, true);
  assert.equal(t.abandoned, undefined);
});

test("a volume whose every cell is filled by a block is a finding, not a ten-second drive", async () => {
  // `FakeBot.blockAt` answers with a solid block everywhere, which is the shape of
  // the gallery's east-pit corner: the cell nearest the approach, and one no body
  // can be in. Before, the walk drove at it until its deadline and the trial then
  // said the bot had stood there.
  const bot = new DrivableFakeBot();
  bot.entity.position = new FakeVec3(0.5, 65, 0.5);
  const executor = attach(bot);
  bot.blockAt = () => ({ name: "stone", boundingBox: "block" });
  executor.useDeathPlan(oneVolumePlan());

  await executor.runDeathLoop();

  const t = executor.deathLoopTrials()[0]!;
  assert.equal(t.enteredVolume, false);
  assert.equal(t.died, false);
  assert.match(t.abandoned ?? "", /can hold a body/);
});
test("a kill the volume's own selector made is credited, whatever cell the body's feet were in", async () => {
  // The gallery's west pit, measured on the ladder: the volume killed the bot and
  // its own promised line reached that player, and the stage reported the death as
  // OUTSIDE the volume and refused to credit it. Two readings were wrong and this
  // is both of them at once. `onDeath` rounded the position, so a body at z = 4.6
  // (cell 4, inside the box) was recorded at 5; and the credit asked `inBox`, a
  // CELL question, where the server had asked whether a 0.6-wide hitbox intersects
  // [lo, hi + 1] — which reaches a third of a block past the face either way.
  //
  // Here the body dies at z = 5.1: cell 5, outside the declared box, inside the
  // reach of the selector that killed it. The volume's kill is the volume's.
  const bot = new DrivableFakeBot();
  bot.entity.position = new FakeVec3(0.5, 65, 0.5);
  const executor = attach(bot);
  executor.useDeathPlan(westPitPlan());

  setTimeout(() => {
    bot.entity.position = new FakeVec3(3.5, 65, 5.1);
    bot.emit("death");
    setTimeout(() => bot.emit("spawn"), 100);
  }, 200);

  await executor.runDeathLoop();

  const t = executor.deathLoopTrials()[0]!;
  assert.equal(t.abandoned, undefined, "a kill by this volume is not an abandoned trial");
  assert.equal(t.died, true);
  assert.deepEqual(t.deathPos, [3.5, 65, 5.1], "recorded exactly, never rounded to a cell");
  assert.equal(t.enteredVolume, true);
  assert.deepEqual(lethalTrialFailures(t).filter((f) => /did NOT die|never OBSERVED/.test(f)), []);
});

test("a death beyond the volume's reach is still refused, and the refusal says reach", async () => {
  // The other direction, and it is what keeps the widened rule from crediting any
  // lethal accident anywhere: a third of a block further out is a body the
  // selector cannot match.
  const bot = new DrivableFakeBot();
  bot.entity.position = new FakeVec3(0.5, 65, 0.5);
  const executor = attach(bot);
  executor.useDeathPlan(westPitPlan());

  setTimeout(() => {
    bot.entity.position = new FakeVec3(3.5, 65, 5.4);
    bot.emit("death");
    setTimeout(() => bot.emit("spawn"), 100);
  }, 200);

  await executor.runDeathLoop();

  const t = executor.deathLoopTrials()[0]!;
  assert.equal(t.died, false);
  assert.match(t.abandoned ?? "", /OUTSIDE the reach of the declared volume/);
  assert.match(t.abandoned ?? "", /\[3\.50, 65\.00, 5\.40\]/);
});

// --- a walk with no proven leg crosses a timed gate by the same rule -----------

/**
 * A fake world whose only route to the near lip runs through a declared timed door.
 *
 * The door's clock is driven by the world, not by a wall-clock timer: it fills the
 * region until the harness has actually LOOKED at it once, then clears — so the
 * closed→open edge the crossing rule waits for really happens, exactly once, with no
 * race for a test to lose. While it is filled every pathfind answers `No path to the
 * goal!` (vanilla A* over a filled region, the ladder's own message) and its cells
 * read as a solid block; once cleared a pathfind walks the bot to its goal.
 */
class GatedApproachBot extends DrivableFakeBot {
  registry = registryFor("1.21.11");
  health = 20;
  food = 20;
  entities: Record<number, unknown> = {};
  inventory = {
    items: (): Array<{ name: string; type: number }> => [],
    slots: [] as Array<{ name: string } | undefined>,
  };
  /** The door's state at each pathfind, in order. */
  gotoStates: string[] = [];
  /** The gallery's mid door: a 1×3×1 plug at [5, 65..67, 9]. */
  private readonly door = { x: 5, y0: 65, y1: 67, z: 9 };
  private looks = 0;
  private get shut(): boolean {
    return this.looks === 0;
  }
  override blockAt(pos?: { x: number; y: number; z: number }): {
    name: string;
    boundingBox: string;
  } | null {
    if (!this.chunkLoaded) return null;
    if (
      pos !== undefined &&
      pos.x === this.door.x &&
      pos.z === this.door.z &&
      pos.y >= this.door.y0 &&
      pos.y <= this.door.y1
    ) {
      if (this.shut) {
        this.looks++;
        return { name: "polished_deepslate", boundingBox: "block" };
      }
    }
    return { name: "air", boundingBox: "empty" };
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
    goto: async (goal: { x: number; y: number; z: number }): Promise<void> => {
      this.gotoStates.push(this.shut ? "shut" : "open");
      if (this.shut) throw new Error("No path to the goal!");
      this.entity.position = new FakeVec3(goal.x + 0.5, goal.y, goal.z + 0.5);
    },
  };
  /** The step INTO the volume is raw controls, so the drive lands the body there
   * and the pit does what a pit does. Only reached once the approach succeeded. */
  private stepped = false;
  override setControlState(): void {
    if (this.stepped) return;
    this.stepped = true;
    setTimeout(() => {
      this.entity.position = new FakeVec3(2.5, 65, 3.5); // inside the west pit
      this.emit("death");
      setTimeout(() => this.emit("spawn"), 50);
    }, 0);
  }
}

/** The west pit with a placement row, so the stage actually walks to a near lip. */
function westPitPlanWithLip(): ReturnType<typeof parseDeathPlan> {
  return parseDeathPlan({
    format_version: 2,
    version: PLAN_VERSION,
    campaign_id: "gallery",
    lethal_volumes: [
      {
        id: "lethal/west-pit",
        region: { lo: [1, 63, 2], hi: [3, 67, 4] },
        // The same box `westPitPlan` carries, and the same answer: the volume
        // widened by a player's half-width, one cell out horizontally and
        // `ceil(height)` down. Derived from the region rather than copied —
        // `metrics::keep_out_box` on [1,63,2]..=[3,67,4] with a 0.6 x 1.8 body.
        keep_out: { lo: [0, 62, 1], hi: [4, 67, 5] },
        message: "The floor in the west corner is not a floor.",
        message_key: "lethal.west-pit.message",
        damage_type: "minecraft:fall",
      },
    ],
    on_death: { effects: 1, drops_stake: [] },
    stakes: [],
    placement: {
      seats: [{ cp: 0, label: "spawn", cell: [0, 65, 0] }],
      regions: [
        { label: "west-pit", lethal: true, volume: "lethal/west-pit", region: { lo: [1, 63, 2], hi: [3, 67, 4] } },
      ],
      // The near lip, as `stake::choose_anchor` now picks one: the nearest cell
      // to the region that lies OUTSIDE the keep-out. This row used to name
      // [1, 65, 5], which is one step past the pit's south face and inside the
      // keep-out — a cell the volume kills from, and so a goal the navigator
      // itself refuses. Its `box_dist2` to the region is 4, the same as the
      // cell it replaces had before the keep-out existed: the lip moved one
      // cell out because a body has a width, not because the scenario changed.
      rows: [{ seat: 0, region: 0, anchor: [1, 65, 6] }],
    },
    binding: {
      lethal_volumes: 1,
      on_death_effects: 1,
      stakes: 0,
      respawn_seats: 1,
      placement_rows: 1,
      unbound: false,
      reason: null,
    },
  });
}

/** The gallery's three doors, as `delvec` exports them, on a leg that goes elsewhere. */
const GALLERY_DOORS = {
  version: PLAN_VERSION,
  campaign_id: "gallery",
  timed_gates: [
    {
      id: "timed-gate/side-door",
      region: { min: [9, 65, 9], max: [9, 67, 9] },
      block: "minecraft:polished_deepslate",
      open_ticks: 40,
      closed_ticks: 60,
      phase: 10,
    },
    {
      id: "timed-gate/mid-door",
      region: { min: [5, 65, 9], max: [5, 67, 9] },
      block: "minecraft:polished_deepslate",
      open_ticks: 40,
      closed_ticks: 60,
      phase: 20,
    },
    {
      id: "timed-gate/inner-door",
      region: { min: [7, 65, 9], max: [7, 67, 9] },
      block: "minecraft:polished_deepslate",
      open_ticks: 40,
      closed_ticks: 60,
      phase: 30,
      crush: true,
    },
  ],
  legs: [
    {
      from: [0, 65, 0],
      to: [40, 65, 40],
      waypoints: [[20, 65, 20]],
      timed_gates: ["timed-gate/side-door"],
    },
  ],
};

test("a walk with NO proven leg waits out a shut declared gate instead of failing", async () => {
  // The ladder's line, from the death-loop stage on the gallery: `lethal/west-pit:
  // … the near lip [1, 65, 5] could not be reached: failed death-loop approach …
  // No path to the goal!` — the bot standing at the mouth of `anchor/timed-door-mid`
  // while all three doors were shut. The approach to a placement-table lip is no
  // critical-path leg, so the compiler marks no gate on it; before this the walk got
  // no gate assist at all and read a shut clock as broken geometry.
  const bot = new GatedApproachBot();
  bot.entity.position = new FakeVec3(9.5, 65.0, 17.5);
  const executor = attach(bot);
  executor.useDeathPlan(westPitPlanWithLip());
  executor.useWaypoints(parseWaypoints(GALLERY_DOORS));

  await executor.runDeathLoop();

  const t = executor.deathLoopTrials()[0]!;
  assert.ok(
    !(t.abandoned ?? "").includes("could not be reached"),
    `the approach crossed the door: ${t.abandoned ?? "(no fault)"}`,
  );
  assert.equal(t.enteredVolume, true, "and the trial got to make its measurement");
  // The proof it was the GATE rule that carried it: the walk failed while the door
  // was filled and was retried only after the region read clear.
  assert.ok(bot.gotoStates.includes("shut"), bot.gotoStates.join(","));
  assert.ok(bot.gotoStates.includes("open"), bot.gotoStates.join(","));
});

test("a matched leg still binds only the gates the compiler proved it crosses", async () => {
  // The narrowing survives: the declared table is the authority only where there is
  // no proven route. A leg the compiler marked keeps its own subset, so a walk is
  // never charged patience for a door on the other side of the map.
  const wp = parseWaypoints(GALLERY_DOORS);
  const hit = nextLegWaypoints(wp.legs, 0, [40, 65, 40]);
  assert.equal(hit.matched, true);
  assert.deepEqual(
    gatesBindingWalk(hit.matched, hit.timedGates, wp.timedGates).gates.map((g) => g.id),
    ["timed-gate/side-door"],
  );
  // …and the same walk with no leg behind it takes the declared table, less the
  // crush door, which has no compiler-pinned mouth to stage from.
  const miss = nextLegWaypoints(wp.legs, 0, [1, 65, 5]);
  assert.equal(miss.matched, false);
  const binding = gatesBindingWalk(miss.matched, miss.timedGates, wp.timedGates);
  assert.deepEqual(
    binding.gates.map((g) => g.id),
    ["timed-gate/side-door", "timed-gate/mid-door"],
  );
  assert.deepEqual(
    binding.withheld.map((g) => g.id),
    ["timed-gate/inner-door"],
  );
});

// --- a drop-gated collect walks onto the drop, wherever the body fell ---------

import type { CollectStep } from "../src/critical-path.ts";

/**
 * A world with one dropped item lying where a wave body fell — seven blocks
 * from the wave's anchor, as vesperhold's Porter left his key. The pathfinder
 * moves the bot to each goal it is given; vanilla pickup is modelled as "the
 * bot's cell is within a block of the item", which completes the objective.
 */
class DropFakeBot extends TransportReachBot {
  goals: Array<[number, number, number]> = [];
  dropAt = new FakeVec3(94.7, 80, 161.5);
  constructor() {
    super();
    this.entity.position = new FakeVec3(87.5, 80, 158.5);
    const self = this;
    this.entities[77] = {
      id: 77,
      name: "item",
      get position(): FakeVec3 {
        return self.dropAt;
      },
      getDroppedItem: () => ({ name: "tripwire_hook" }),
    };
    (this.pathfinder as { goto: (goal: unknown) => Promise<void> }).goto = async (
      goal: unknown,
    ): Promise<void> => {
      const g = goal as { x: number; y: number; z: number };
      this.goals.push([g.x, g.y, g.z]);
      this.entity.position = new FakeVec3(g.x + 0.5, g.y, g.z + 0.5);
      const d = Math.hypot(g.x + 0.5 - this.dropAt.x, g.z + 0.5 - this.dropAt.z);
      if (d <= 1.5 && this.entities[77]) {
        delete this.entities[77];
        this.emit("messagestr", "[dw:complete vesperhold obj/take-the-postern-key]");
      }
    };
  }
  async lookAt(): Promise<void> {}
}

const DROP_STEP: CollectStep = {
  action: "collect",
  objective: "obj/take-the-postern-key",
  item: "minecraft:tripwire_hook",
  count: 1,
  droppedBy: "wave/porter",
  pos: [87, 80, 158],
  sneak: false,
} as unknown as CollectStep;

test("a drop-gated collect walks onto the drop where the body fell, not to the anchor", async () => {
  const bot = new DropFakeBot();
  const executor = attach(bot);
  executor.useCampaign("vesperhold");
  executor.beginStep(5);
  await executor.collect(DROP_STEP);
  // The last goal is the drop's own cell; before the fix the walk ended at the
  // anchor and the step timed out beside a key seven blocks away.
  assert.deepEqual(bot.goals.at(-1), [94, 80, 161]);
  assert.equal(bot.entities[77], undefined, "the key was picked up");
});

test("a drop beyond the fight's radius is not this fight's drop", async () => {
  const bot = new DropFakeBot();
  bot.dropAt = new FakeVec3(87.5 + 40, 80, 158.5);
  const executor = attach(bot);
  executor.useCampaign("vesperhold");
  executor.beginStep(5);
  // Nothing within reach of the fight: the step waits on its objective and says so.
  setTimeout(() => bot.emit("messagestr", "delve-bot fell out of the world"), 300);
  setTimeout(() => bot.emit("death"), 310);
  await assert.rejects(() => executor.collect(DROP_STEP));
  assert.ok(
    bot.goals.every((g) => Math.abs(g[0] - 127) > 2),
    `never walked to the far item: ${JSON.stringify(bot.goals)}`,
  );
});

// --- trigger steps: the bot does to the target what a player does ---------------

import type { TriggerStep } from "../src/critical-path.ts";

/**
 * A fake server with one environment trigger's `interaction` hitbox at the
 * target cell. It fires the trigger the way the datapack does — only on the
 * client act the trigger watches (an attack for a strike, a right-click for a
 * use) — and answers with the trigger's fired marker. Nothing else fires it:
 * a chatted command, in particular, does nothing.
 */
class TriggerFakeBot extends InteractFakeBot {
  /** Which client act the emitted tick line reads (`attack` or `interaction`). */
  watches: "attack" | "interaction" = "attack";
  constructor() {
    super();
    // The real summon: `minecraft:interaction` 1.0 x 2.0 on the anchor cell.
    this.entities[77] = {
      id: 77,
      name: "interaction",
      width: 1,
      height: 2,
      position: new FakeVec3(1.5, 64, 0.5),
    };
  }
  async lookAt(): Promise<void> {
    this.calls.push("lookAt");
  }
  private fire(act: "attack" | "interaction", id: number): void {
    if (act === this.watches && id === 77) {
      setTimeout(
        () => this.emit("messagestr", "[dw:complete vesperhold trigger/psalter-wall]"),
        10,
      );
    }
  }
  attack(e: { id: number }): void {
    this.calls.push(`attack(${e.id})`);
    this.fire("attack", e.id);
  }
  async activateEntity(e: { id: number }): Promise<void> {
    this.calls.push(`activateEntity(${e.id})`);
    this.fire("interaction", e.id);
  }
}

const STRIKE_STEP: TriggerStep = {
  action: "trigger",
  trigger: "trigger/psalter-wall",
  on: "strike",
  anchor: "anchor/psalter-face",
  pos: [1, 64, 0],
};

test("a strike trigger step ATTACKS the target's hitbox and passes on the fired marker", async () => {
  // The vesperhold ladder stopped in front of a wall only a strike opens. The step
  // is a real left-click on the hitbox the compiler summoned — never a command —
  // and it passes on the trigger's own marker, never on the swing landing.
  const bot = new TriggerFakeBot();
  const executor = attach(bot);
  executor.useCampaign("vesperhold");
  executor.beginStep(16);
  await executor.fireTrigger(STRIKE_STEP);
  assert.deepEqual(
    bot.calls.filter((c) => c !== "goto"),
    ["lookAt", "attack(77)"],
    "looked at the hitbox, then hit it — and chatted nothing",
  );
});

test("a use trigger step RIGHT-CLICKS the hitbox instead of hitting it", async () => {
  const bot = new TriggerFakeBot();
  bot.watches = "interaction";
  const executor = attach(bot);
  executor.useCampaign("vesperhold");
  await executor.fireTrigger({ ...STRIKE_STEP, on: "use" });
  assert.deepEqual(
    bot.calls.filter((c) => c !== "goto"),
    ["lookAt", "activateEntity(77)"],
  );
});

test("a strike-npc trigger step hits the NPC's own hitbox at its station", async () => {
  const bot = new TriggerFakeBot();
  const executor = attach(bot);
  executor.useCampaign("vesperhold");
  const { anchor: _anchor, ...rest } = STRIKE_STEP;
  await executor.fireTrigger({ ...rest, on: "strike-npc", npc: "npc/giant" });
  assert.ok(bot.calls.includes("attack(77)"), bot.calls.join(", "));
});

test("an approach trigger step walks into range and clicks nothing", async () => {
  const bot = new TriggerFakeBot();
  const executor = attach(bot);
  executor.useCampaign("vesperhold");
  // The fake server fires an approach on proximity; the bot already stands there.
  setTimeout(() => bot.emit("messagestr", "[dw:complete vesperhold trigger/psalter-wall]"), 10);
  const { anchor, ...rest } = STRIKE_STEP;
  await executor.fireTrigger({ ...rest, anchor, on: "approach", range: 6 });
  assert.deepEqual(
    bot.calls.filter((c) => c !== "goto"),
    [],
    "an approach is a walk: no look, no swing, no click, no command",
  );
});

test("a strike with no hitbox at the target fails the step loudly, before any wait", async () => {
  const bot = new TriggerFakeBot();
  delete bot.entities[77];
  const executor = attach(bot);
  executor.useCampaign("vesperhold");
  await assert.rejects(() => executor.fireTrigger(STRIKE_STEP), /nothing to hit/);
  assert.ok(!bot.calls.some((c) => c.startsWith("attack")), bot.calls.join(", "));
});

test("the kill step hunts only what the census calls the wave, never a bystander", async () => {
  // vesperhold, third ladder: with the Cliff Watchmen down the bot swung 94 times
  // at the stable's Invulnerable horse puppets — living bodies, never of the wave
  // — while the last watchman stood elsewhere. A body the census does not stand
  // is not hunted; the wave's own body is.
  const bot = new CombatFakeBot();
  bot.seat(1, { distance: 3 });
  bot.addBystander(77, 1, 10_000); // nearer than the wave body, and unkillable
  const executor = attach(bot);
  executor.useCampaign("the-drowned-bell");
  executor.useCombatPlan(combatPlan(1, true), false);

  await executor.kill(KILL_STEP);

  assert.equal(bot.hitsOn(77), 0, "no swing at the bystander");
  assert.deepEqual(bot.waveIds(), [], "the wave body was");
});

// --- run-backs: a re-seated fight beside a later leg is fought, assisted --------

import type { RunBack } from "../src/combat.ts";

const RUN_BACK: RunBack = {
  wave: "wave/gate-assault",
  objective: "obj/hold-the-gate",
  bonfire: 1,
  before: "obj/great-hall",
  tier: "ordinary",
  pos: [0, 64, 0],
  count: 1,
  radius: 16,
  crossing: [4, 64, 0],
  distance: 4,
  paths: ["critical-path"],
};

const GREAT_HALL: ReachStep = {
  action: "reach",
  objective: "obj/great-hall",
  anchor: "anchor/great-hall",
  pos: [0, 64, 0],
  radius: 2,
  completion: { kind: "cube", lo: [-1, 63, -1], hi: [1, 65, 1] },
};

test("a run-back the rest re-seated is fought under a named assist before the leg", async () => {
  // vesperhold, ladder run: the Tower Fire rest put wave/walk-ambush back on the
  // buttress walk, and the leg to the great hall walked through it unassisted and
  // died in one exchange. The leg is an encounter; the ladder fights it as one.
  const bot = new BonfireFakeBot();
  bot.armBonfire(55, new FakeVec3(0.5, 64, 0.5));
  bot.reSeat = undefined;
  const executor = attach(bot);
  executor.useCampaign("the-drowned-bell");
  executor.useCombatPlan({ ...combatPlan(1, true), runBacks: [RUN_BACK] }, false);

  executor.beginStep(9);
  await executor.kill(KILL_STEP); // cleared once…
  executor.beginStep(12);
  await executor.rest(REST_STEP); // …the rest puts it back…
  bot.seat(1);
  executor.beginStep(14);
  const strikesBefore = bot.calls.filter((c) => c.includes(":wave_strike_")).length;
  await executor.beforeStep(GREAT_HALL); // …so the leg to the hall meets it

  assert.ok(
    bot.calls.filter((c) => c.includes(":wave_strike_")).length > strikesBefore,
    "the re-seated wave was read and cleared before the leg",
  );
  assert.ok(
    executor.stagedBodies().some((r) => r.why.startsWith("staged clear:")),
    "and the artifact says the harness cleared it, not the delve",
  );
  assert.deepEqual(executor.runBacks().map((r) => r.wave), ["wave/gate-assault"]);
});

test("no run-back is fought before the rest that re-seats it, nor twice after one", async () => {
  const bot = new BonfireFakeBot();
  bot.armBonfire(55, new FakeVec3(0.5, 64, 0.5));
  bot.reSeat = undefined;
  const executor = attach(bot);
  executor.useCampaign("the-drowned-bell");
  executor.useCombatPlan({ ...combatPlan(1, true), runBacks: [RUN_BACK] }, false);

  executor.beginStep(9);
  await executor.kill(KILL_STEP);
  executor.beginStep(10);
  await executor.beforeStep(GREAT_HALL); // cleared, not rested: the leg is empty
  assert.equal(executor.runBacks().length, 0);

  executor.beginStep(12);
  await executor.rest(REST_STEP);
  bot.seat(1);
  executor.beginStep(14);
  await executor.beforeStep(GREAT_HALL);
  executor.beginStep(15);
  await executor.beforeStep(GREAT_HALL); // fought once; down until the next rest
  assert.equal(executor.runBacks().length, 1);
});

test("a wave two rests put back beside one leg is fought once", async () => {
  const bot = new BonfireFakeBot();
  bot.armBonfire(55, new FakeVec3(0.5, 64, 0.5));
  bot.reSeat = undefined;
  const executor = attach(bot);
  executor.useCampaign("the-drowned-bell");
  executor.useCombatPlan(
    { ...combatPlan(1, true), runBacks: [RUN_BACK, { ...RUN_BACK, bonfire: 2 }] },
    false,
  );
  executor.beginStep(9);
  await executor.kill(KILL_STEP);
  executor.beginStep(12);
  await executor.rest(REST_STEP);
  executor.beginStep(13);
  await executor.rest({ ...REST_STEP, bonfire: 2 });
  bot.seat(1);
  executor.beginStep(14);
  await executor.beforeStep(GREAT_HALL);
  assert.equal(executor.runBacks().length, 1);
});

/** A fake whose pathfinder actually arrives: `goto` puts the bot on the goal. */
class WalkingBonfireBot extends BonfireFakeBot {
  constructor() {
    super();
    const base = this.pathfinder;
    this.pathfinder = {
      ...base,
      // mineflayer hands `goto` its goal; the base fake's signature ignores it.
      goto: (async (goal: unknown): Promise<void> => {
        this.calls.push("goto");
        const g = goal as { x?: number; y?: number; z?: number };
        if (typeof g?.x === "number" && typeof g.y === "number" && typeof g.z === "number") {
          this.entity.position = new FakeVec3(g.x + 0.5, g.y, g.z + 0.5);
        }
      }) as unknown as () => Promise<void>,
    };
  }
}

/** The hall step, twenty blocks off the fight, so its leg is its own. */
const HALL_FAR: ReachStep = {
  ...GREAT_HALL,
  pos: [0, 64, 20],
  completion: { kind: "cube", lo: [-1, 63, 19], hi: [1, 65, 21] },
};

test("a run-back is met along the leg's own proven cells, and the leg resumes from the fight", async () => {
  // The first ladder run with run-backs walked from the belfry to the wave's
  // anchor across the map with no proven route and stranded. The fight is met
  // where the compiler measured the crossing: the bot walks the leg's cells up to
  // it, fights, and the step's walk goes on from there — never back to the start.
  const bot = new WalkingBonfireBot();
  bot.armBonfire(55, new FakeVec3(0.5, 64, 0.5));
  bot.reSeat = undefined;
  const executor = attach(bot);
  executor.useCampaign("the-drowned-bell");
  executor.useCombatPlan(
    { ...combatPlan(1, true), runBacks: [{ ...RUN_BACK, crossing: [4, 64, 20] }] },
    false,
  );
  executor.beginStep(9);
  // The kill's own walk would consume the leg in lockstep; this test is about the
  // leg the NEXT step walks, so the kill is fought without waypoints first.
  executor.useWaypoints(parseWaypoints({ version: "0.6.0", campaign_id: "x", legs: [] }));
  await executor.kill(KILL_STEP);
  executor.useWaypoints(
    parseWaypoints({
      version: "0.6.0",
      campaign_id: "the-drowned-bell",
      legs: [
        {
          from: [12, 64, 20],
          to: [0, 64, 20],
          waypoints: [
            [10, 64, 20],
            [8, 64, 20],
            [6, 64, 20],
            [4, 64, 20],
            [2, 64, 20],
          ],
        },
      ],
    }),
  );
  executor.beginStep(12);
  await executor.rest(REST_STEP);
  bot.seat(1);
  executor.beginStep(14);
  const gotos = (): number => bot.calls.filter((c) => c === "goto").length;
  const before = gotos();
  await executor.beforeStep(HALL_FAR);
  assert.equal(executor.runBacks().length, 1);
  const afterFight = gotos();
  assert.ok(afterFight - before >= 4, "walked the leg's cells up to the crossing");
  setTimeout(() => bot.emit("messagestr", "[dw:complete the-drowned-bell obj/great-hall]"), 20);
  await executor.reach(HALL_FAR);
  assert.equal(
    gotos() - afterFight,
    3,
    "the step resumed at the crossing: two remaining cells and the goal, not all six",
  );
});

