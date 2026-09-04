// The one owner of the bot's pathfinder goal.
//
// ## Why this exists
//
// `mineflayer-pathfinder`'s `goto(goal)` registers four listeners on the bot and
// resolves (or rejects) exactly once, when one of them fires. One of them is
// `goal_updated`: ANY later `setGoal` — a fresh `goto`, or the `setGoal(null)` the
// executor uses to clear the pathfinder — rejects every goto still in flight with
// `GoalChanged`, on a `setTimeout(..., 0)`, from outside the caller's stack.
//
// That makes an in-flight goto a promise that can reject long after whoever asked
// for it has stopped listening. A caller that raced the walk against a death or a
// timeout has moved on; the rejection lands with no handler; Node's default
// `--unhandled-rejections=throw` turns it into a fatal error and the process dies
// mid-run. Measured on a 24-place delve whose boss killed the bot during the
// die-retry trade: the re-approach's `goto` set a goal while an abandoned one was
// still pending, and the run died with `bot-1 exited with code 1` — a harness crash
// reported as a red stage, with no run report at all.
//
// So the pathfinder gets ONE owner, and two rules that hold for every trip:
//
//   1. **The owner observes every promise it creates, for that promise's whole
//      life.** A rejection arriving after the caller stopped caring is delivered to
//      a handler that was attached the moment the trip started, so `GoalChanged` is
//      an EXPECTED outcome of an abandoned trip and can never be an unhandled one.
//   2. **Goals are serialised: cancel, wait for the cancelled trip to settle, then
//      set.** No `setGoal` is ever issued while another trip is pending, so the
//      out-of-band rejection this class is made of does not arise in the first
//      place. Rule 1 is what makes the residue safe; rule 2 is what stops producing
//      it.
//
// This is navigation ownership, not game logic: the owner decides nothing about
// where the bot goes, only that one goal is in flight at a time and that every
// outcome is read.

/**
 * The `bot.pathfinder` surface this owner drives — the three members it touches,
 * so a test can supply a fake (or the real `mineflayer-pathfinder` goto helper)
 * without standing up a server.
 */
export interface NavigationSurface {
  goto(goal: unknown): Promise<void>;
  stop(): void;
  setGoal(goal: unknown | null): void;
}

/**
 * How long the owner waits for a cancelled trip to settle before issuing the next
 * goal. `goto`'s own cleanup rejects on a `setTimeout(0)`, so a settle is one
 * event-loop turn in practice; the cap only bounds a pathfinder that has stopped
 * answering. Exceeding it is SAFE — the owner keeps observing the abandoned trip
 * either way — so this is a latency bound, never a correctness one.
 */
const SETTLE_MS = 2_000;

/** One trip's collected outcome: the rejection reason, or `undefined` on arrival. */
interface Trip {
  readonly outcome: Promise<unknown>;
}

export class NavigationOwner {
  readonly #surface: () => NavigationSurface | undefined;
  readonly #settleMs: number;
  /** The trip whose goal the pathfinder is currently holding, if any. */
  #trip: Trip | undefined;
  /** How many trips the owner collected on an abandoning caller's behalf. */
  #collected = 0;
  /** How many trips it created at all — the denominator of the count above. */
  #issued = 0;

  /**
   * @param surface resolves the live pathfinder. A function rather than a value
   *   because the executor attaches its bot after construction, and a disconnect
   *   takes it away again.
   */
  constructor(surface: () => NavigationSurface | undefined, settleMs = SETTLE_MS) {
    this.#surface = surface;
    this.#settleMs = settleMs;
  }

  /**
   * Walk to `goal`, resolving on arrival and rejecting with whatever the pathfinder
   * rejected with.
   *
   * The caller may stop awaiting this at any time (a death, a timeout, a stalker
   * winning a race). Doing so is safe: the underlying trip is still observed here,
   * and the next `goto` clears it before setting its own goal.
   */
  async goto(goal: unknown): Promise<void> {
    const surface = this.#surface();
    if (surface === undefined) {
      throw new Error("navigation: no bot is attached");
    }
    // Serialise. Nothing may hold the goal when the new one is set.
    await this.release();
    // The ONE place a pathfinder promise is created — and it is observed on the
    // very next expression, before any `await` can hand control back to a caller
    // that might walk away from it.
    const trip: Trip = {
      outcome: surface.goto(goal).then(
        () => undefined,
        (err: unknown) => err ?? new Error("the pathfinder rejected with no reason"),
      ),
    };
    this.#trip = trip;
    this.#issued += 1;
    const err = await trip.outcome;
    if (this.#trip === trip) this.#trip = undefined;
    if (err !== undefined) throw err;
  }

  /**
   * Abandon whatever the pathfinder is doing, leaving it in a state the next `goto`
   * can actually use. Synchronous, because the death handler is.
   *
   * `stop()` only raises `mineflayer-pathfinder`'s internal `stopPathing` flag; the
   * flag is cleared when the walking bot next reaches a path node, or by a
   * `setGoal`/`setMovements` reset — so calling `stop()` on a bot that is NOT
   * mid-path leaves it raised. The next `goto` then sets its goal, the reset sees
   * the raised flag, fires `path_stop` synchronously, and the brand-new goto rejects
   * instantly with "Path was stopped before it could be completed!" — while the
   * caller's failure handler stops the pathfinder again, re-arming the flag. The
   * result is a self-sustaining loop where every later hop fails without the bot
   * ever attempting to walk (observed on the-drowned-bell: after the bot stopped to
   * fight an ambusher, every subsequent hop and every recovery re-path failed that
   * way, and the run died on a leg it had walked fine the run before).
   *
   * `setGoal(null)` immediately after performs that reset ourselves: the flag is
   * consumed here, once, instead of poisoning the next caller.
   *
   * The trip stays registered afterwards, deliberately: it is cancelled, not
   * settled, and the next `goto` must still wait for its rejection to arrive.
   */
  stopNow(): void {
    const surface = this.#surface();
    if (surface === undefined) return;
    try {
      surface.stop();
      surface.setGoal(null);
    } catch {
      // best effort — clearing the pathfinder must never mask the reason we stopped
    }
  }

  /**
   * Cancel the in-flight trip and wait for it to settle. Never rejects: the point of
   * this method is that the cancelled trip's `GoalChanged` (or `PathStopped`) is
   * consumed here rather than escaping.
   *
   * A no-op when nothing is in flight — issuing `stop()`/`setGoal(null)` on an idle
   * pathfinder churns its state and is exactly what the note on {@link stopNow}
   * describes; the executor's unstick path depends on it not happening.
   */
  async release(): Promise<void> {
    const trip = this.#trip;
    if (trip === undefined) return;
    this.#trip = undefined;
    this.#collected += 1;
    this.stopNow();
    await Promise.race([trip.outcome, delay(this.#settleMs)]);
  }

  /** Whether a goal is currently held. */
  inFlight(): boolean {
    return this.#trip !== undefined;
  }

  /**
   * What this owner did, for a report or a test: trips issued, and how many of them
   * a caller walked away from and the owner collected instead. A non-zero
   * `collected` is the ordinary state of a run with deaths in it — it is the count
   * of rejections that WOULD have been unhandled.
   */
  binding(): { issued: number; collected: number } {
    return { issued: this.#issued, collected: this.#collected };
  }
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
