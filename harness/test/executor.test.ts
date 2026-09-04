import { test } from "node:test";
import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import type { Bot } from "mineflayer";
import { MineflayerExecutor, completionWindowMs, type BotConfig } from "../src/executor.ts";
import { BotDeathError } from "../src/death.ts";
import type { AssertCompleteStep } from "../src/critical-path.ts";
import { parseDeathPlan } from "../src/death-loop.ts";

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
  executor.useNonCombatants(new Set(["mannequin", "villager"]));
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
    isWaveMob(ent("item", 0.25), SELF, CAST),
    false,
    "a short dropped entity is excluded",
  );
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

test("an UNMARKED leg gets no gate retries — a real navigation regression still fails fast", async () => {
  // The licence to retry comes from the compiler's crossing mark, never from the
  // harness. A leg with no gate gets no gate handling at all.
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
import { parseWaypoints } from "../src/waypoints.ts";

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
  inventory = { items: (): Array<{ name: string; type: number }> => [] };
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
  type EncounterBody,
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
   * census — exactly the drowned bell's belfry. */
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

function combatPlan(
  count = 1,
  respawnsOnRest = true,
  bodies: readonly EncounterBody[] = [{ kind: "drowned", count, giveUpSwings: 24 }],
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
        tier: ENCOUNTER.tier,
        pos: ENCOUNTER.pos,
        count,
        respawnsOnRest,
        census: ENCOUNTER.census,
        bodies,
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
  // the-drowned-bell run six. The die-retry stage walked to within 3
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
  // The first-contact/mid-fight race. `tradeBlows` deliberately stands
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

// ---------------------------------------------------------------------------
// The actor floor gate: the other shape an elite takes
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

test("a wave that kills the bot mid-walk does not crash the harness on the re-approach", async () => {
  // Reproduces the greyhithe-saltworks rehearsal crash and asserts the two things
  // that separate a harness fault from a content verdict:
  //   * NOTHING rejects into the void — a `GoalChanged` on an abandoned trip is an
  //     expected outcome the navigation owner collects, not a fatal error;
  //   * the run CONTINUES: both trials still take their scripted death and reach a
  //     verdict, which is what the stage exists to measure.
  const bot = new RealGotoCombatBot();
  // The wave stands off the anchor, so the trade has to walk into melee.
  bot.seat(1, { distance: 6 });
  bot.reSeat = { count: 1, distance: 6 };
  // The bot starts away from the anchor, so the approach is a real hop.
  bot.entity.position = new FakeVec3(20, 64, 0);
  // …and the walk into melee is the hop that strands and gets it killed.
  bot.strandAt = [6, 64, 0];
  const executor = attach(bot);
  executor.useCampaign("the-drowned-bell");
  executor.useCombatPlan(combatPlan(), true);

  const unhandled: unknown[] = [];
  const capture = (err: unknown): void => void unhandled.push(err);
  process.on("unhandledRejection", capture);
  try {
    await executor.kill(KILL_STEP);
    // The rejection lands from a zero timer, so give the loop turns to deliver it.
    for (let i = 0; i < 6; i++) await new Promise((r) => setTimeout(r, 5));
  } finally {
    process.off("unhandledRejection", capture);
  }

  assert.deepEqual(
    unhandled.map((e) => (e instanceof Error ? `${e.name}: ${e.message}` : String(e))),
    [],
    "no pathfinder rejection escaped: an unhandled one here kills the whole run",
  );
  // The bot really did walk its hops — otherwise the test proves nothing about the
  // path it is named for.
  assert.ok(
    bot.calls.filter((c) => c === "goto").length >= 3,
    `the stage walked its hops: ${bot.calls.filter((c) => c === "goto").length}`,
  );
  const trials = executor.deathTrials();
  assert.equal(trials.length, 2, "both scripted deaths were still taken");
  assert.ok(
    trials.every((t) => t.completed),
    "and both loops reached a verdict rather than dying with the process",
  );
  assert.deepEqual(dieRetryFindings(trials), []);
});

// --- when to stop swinging at one body (the census round) ----------------------

test("a body that outlives its encounter's melee budget is a FINDING, not a silent drop", async () => {
  // What replaced `WAVE_UNKILLABLE_MS`. The old rule meleed anything for a flat
  // six seconds and then blacklisted it in silence — too long for a rat, too
  // short for an elite, and it reported nothing either way. The budget is now
  // the encounter's own arithmetic, and crossing it is a content defect the run
  // report names: either nothing in the party's kit can damage this body, or the
  // encounter's numbers are wrong.
  const bot = new CombatFakeBot();
  // A hostile the wave census cannot see — an ambusher belonging to no wave, so
  // the fight can still end and this test is about the finding rather than about
  // the 90s timeout an unkillable TAGGED body correctly earns.
  bot.seat(1, { waveTagged: false, hitsToKill: 10_000 }); // nothing the bot does fells it
  const executor = attach(bot);
  executor.useCampaign("the-drowned-bell");
  executor.useCombatPlan(
    combatPlan(1, true, [{ kind: "zombie", count: 1, giveUpSwings: 3 }]),
    false,
  );

  await executor.kill(KILL_STEP);

  const findings = executor.unkillableFindings();
  assert.equal(findings.length, 1, findings.join(" | "));
  assert.match(findings[0]!, /wave\/gate-assault/);
  assert.match(findings[0]!, /`zombie`/);
  assert.match(findings[0]!, /3 swing/);
  // And it did stop: the budget is 3, so the bot cannot have gone on swinging.
  const swings = bot.calls.filter((c) => c === "attack").length;
  assert.equal(swings, 3, `stopped at the budget, not at a timer: ${swings} swing(s)`);
});

test("an encounter that states no budget for a kind gives up on nothing", async () => {
  // `give_up_swings: null` — an undeclared `max_health`, which Mojang publishes
  // no default for. There is then no basis for calling a body unkillable, so the
  // bot keeps swinging: this one takes six hits, twice what any budget in the
  // sibling case allowed, and still falls. No finding, and no constant of the
  // harness's own deciding it was scenery at swing four.
  const bot = new CombatFakeBot();
  bot.seat(1, { waveTagged: false, hitsToKill: 6 });
  const executor = attach(bot);
  executor.useCampaign("the-drowned-bell");
  executor.useCombatPlan(
    combatPlan(1, true, [
      { kind: "zombie", count: 1, giveUpSwings: undefined, reason: "declares no max_health" },
    ]),
    false,
  );

  await executor.kill(KILL_STEP);

  assert.deepEqual(executor.unkillableFindings(), []);
  assert.equal(bot.calls.filter((c) => c === "attack").length, 6);
});

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
    format_version: 1,
    version: "0.19.0",
    campaign_id: "probe",
    lethal_volumes: [
      {
        id: "lethal/the-pit",
        region: { lo: [4, 65, 8], hi: [6, 65, 10] },
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
  assert.deepEqual(t.deathPos, [5, 65, 9], "and the position is THIS death's, not the last one's");
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
