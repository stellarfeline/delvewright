import { test } from "node:test";
import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import { createRequire } from "node:module";
import { NavigationOwner, type NavigationSurface } from "../src/navigation.ts";

// The REAL `mineflayer-pathfinder` goto helper, not a model of it. The whole defect
// this module exists for is a behaviour of that file — which listeners it registers,
// when it rejects, and on whose stack — so a hand-written stand-in could agree with
// the harness and disagree with the library, which is the one failure a test like
// this must not have. It is CommonJS and untyped; `createRequire` is how a `.ts`
// module under NodeNext reaches it.
const requireCjs = createRequire(import.meta.url);
const gotoUtil = requireCjs("mineflayer-pathfinder/lib/goto.js") as (
  bot: unknown,
  goal: unknown,
) => Promise<void>;

/**
 * A bot with just the pathfinder surface `goto.js` drives: an EventEmitter, and a
 * `setGoal` that emits `goal_updated` exactly as mineflayer-pathfinder's does. A
 * trip resolves only when the test says the bot arrived.
 */
class PathfinderBot extends EventEmitter {
  readonly calls: string[] = [];
  readonly pathfinder = {
    stop: (): void => {
      this.calls.push("stop");
    },
    setGoal: (goal: unknown): void => {
      this.calls.push(goal === null ? "setGoal(null)" : "setGoal");
      // index.js:145 — every `setGoal` announces itself, and every goto still
      // listening for a goal that is not this one rejects with GoalChanged.
      this.emit("goal_updated", goal, false);
    },
    goto: (goal: unknown): Promise<void> => gotoUtil(this, goal),
  } satisfies NavigationSurface;

  /** The bot reached whatever it was walking to. */
  arrive(): void {
    this.emit("goal_reached");
  }
}

/** Let every already-queued microtask AND `setTimeout(0)` run — goto.js settles its
 * promise from a zero timer, so a microtask drain alone would miss it. */
async function drain(turns = 4): Promise<void> {
  for (let i = 0; i < turns; i++) await new Promise((resolve) => setTimeout(resolve, 0));
}

/** Capture unhandled rejections for the duration of `body` instead of dying on them. */
async function withUnhandledCapture(body: () => Promise<void>): Promise<unknown[]> {
  const seen: unknown[] = [];
  const capture = (err: unknown): void => void seen.push(err);
  process.on("unhandledRejection", capture);
  try {
    await body();
    await drain();
  } finally {
    process.off("unhandledRejection", capture);
  }
  return seen;
}

test("a trip the caller walked away from is collected, never left unhandled", async () => {
  // The shipped crash, at its smallest. A walk is abandoned (live: the death latch
  // fired and the caller stopped awaiting it), a later goal is set, and
  // mineflayer-pathfinder rejects the abandoned trip with GoalChanged from a zero
  // timer. With nobody listening that is a fatal unhandled rejection: the run dies
  // with `bot-1 exited with code 1` and writes no report at all.
  const bot = new PathfinderBot();
  const nav = new NavigationOwner(() => bot.pathfinder);

  const unhandled = await withUnhandledCapture(async () => {
    // The caller issues a hop and then stops caring about it — exactly what
    // racing a walk against the death latch does.
    void nav.goto("goal/first").catch(() => {
      // The caller's own await is gone; this stands in for it.
    });
    await drain(1);
    assert.equal(nav.inFlight(), true, "the first trip holds the goal");
    // A second hop. Its `setGoal` is what rejects the abandoned one.
    const second = nav.goto("goal/second");
    await drain(2);
    bot.arrive();
    await second;
  });

  assert.deepEqual(unhandled, [], "no rejection escaped the owner");
  assert.deepEqual(
    nav.binding(),
    { issued: 2, collected: 1 },
    "two trips issued; the abandoned one was collected by the owner",
  );
});

test("goals are serialised: the cancelled trip settles BEFORE the next goal is set", async () => {
  // Rule 2. The out-of-band rejection only exists because a goal can be replaced
  // while a trip is pending, so the owner cancels first and waits — the ordering is
  // the assertion, and `stop`+`setGoal(null)` is one act (the stop flag is consumed
  // by the reset, or it poisons the next hop; see navigation.ts).
  const bot = new PathfinderBot();
  const nav = new NavigationOwner(() => bot.pathfinder);

  const unhandled = await withUnhandledCapture(async () => {
    void nav.goto("goal/first").catch(() => {});
    await drain(1);
    const second = nav.goto("goal/second");
    await drain(2);
    bot.arrive();
    await second;
  });

  assert.deepEqual(unhandled, []);
  assert.deepEqual(
    bot.calls,
    ["setGoal", "stop", "setGoal(null)", "setGoal"],
    "the second goal is set only after the first was stopped and cleared",
  );
});

test("an arriving trip resolves, and leaves no goal held", async () => {
  const bot = new PathfinderBot();
  const nav = new NavigationOwner(() => bot.pathfinder);
  const trip = nav.goto("goal/only");
  await drain(1);
  bot.arrive();
  await trip;
  assert.equal(nav.inFlight(), false);
  assert.deepEqual(nav.binding(), { issued: 1, collected: 0 }, "nothing had to be collected");
});

test("a rejecting trip rejects the caller with the pathfinder's own error", async () => {
  // The owner absorbs rejections the CALLER has abandoned; a caller that is still
  // waiting must still be told, or a stuck hop would read as an arrival.
  const bot = new PathfinderBot();
  const nav = new NavigationOwner(() => bot.pathfinder);
  const trip = nav.goto("goal/only");
  await drain(1);
  bot.emit("path_update", { status: "noPath", path: [1] });
  await assert.rejects(trip, (err: unknown) => err instanceof Error && err.name === "NoPath");
  assert.equal(nav.inFlight(), false);
});

test("navigating with no bot attached fails loudly rather than silently", async () => {
  const nav = new NavigationOwner(() => undefined);
  await assert.rejects(nav.goto("goal/anywhere"), /no bot is attached/);
});
