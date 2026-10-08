// MineflayerExecutor: a timed gate on a leg: its window, waiting for it, dashing through.

import type { Vec3Tuple } from "../critical-path.ts";
import { BotDeathError } from "../death.ts";
import { type GoalSpec, type TimedGate } from "../waypoints.ts";
import {
  GATE_MIN_ATTEMPTS,
  GATE_POLL_MS,
  crossingEstimateMs,
  describeGates,
  gateRegionCells,
  gateRetryBudgetMs,
  gateWindowWaitMs,
  insideGate,
  nearCell,
  needsStandoff,
  openMs,
} from "../timed-gate.ts";
import { delay, fmt } from "./connection.ts";
import {
  type Unstick,
  type LegSettled,
  legSettledReason,
  reached,
  recoverAndRetry,
} from "./walk.ts";
import type { MineflayerExecutor } from "../executor.ts";

/**
 * How far (squared horizontal blocks) the bot may sit from its gate staging cell
 * before the station-keep drives it back. 0.4 blocks: tight enough
 * that a window always opens with the bot ON the compiler-pinned mouth, loose
 * enough that the per-poll correction is not a permanent jitter.
 */
const GATE_HOLD_SLACK_SQ = 0.4 * 0.4;

/** Re-aim cadence of the raw crush-gate crossing dash. Faster than
 * the physics would meaningfully change; slow enough not to spam look packets. */
export const GATE_DASH_TICK_MS = 50;

/** Squared horizontal arrival radius of the dash at the far mouth cell. */
const GATE_DASH_ARRIVE_SQ = 0.75 * 0.75;

/** Bound on the dash's emergency raw retreat out of the fill (with the corridor
 * current behind it, clearing one or two cells takes a fraction of this). */
const GATE_DASH_RETREAT_MS = 1_500;

/**
 * The bot-facing half of a timed-gate crossing, injected by the executor so the
 * control flow above stays unit-testable without a live server.
 */
export interface GateAssist {
  /** The gates the compiler proved this leg's route crosses (empty ⇒ no assist). */
  readonly gates: readonly TimedGate[];
  /**
   * Block (bounded) until `gates` are observed to go from closed to OPEN, so the
   * crossing begins at the top of a window rather than its tail. Returns `true`
   * when the fresh edge was actually OBSERVED and `false` when the wait gave up
   * (region blocks unreadable). On `false` a non-crush caller may still try the
   * hop; a crush-gate caller must NOT — blind entry is the lethal defect.
   *
   * `hold`, when given, is a feet cell the bot must actively KEEP during the wait
   * (a passive idle is not a stance: the tide-mill gate corridor is flowing water,
   * and the current carried the idle bot 8 blocks off the mouth while it watched
   * for the edge — a human holds a movement key against the tide).
   *
   * `press`, when given (crush staging), is the crossing target to LEAN toward
   * whenever the gates provably read CLOSED: the shut gate is a solid wall, so
   * driving at it parks the bot pressed against the plane with forward momentum —
   * and the open edge releases it THROUGH, contact-started. Measured live: from a
   * standing start at the mouth the tide-mill pour is a wall (the wade covered
   * less than 1.5 blocks in the whole 1.8 s window); the press is how a player
   * rides a portcullis. Only applied while the gates read closed — leaning into an
   * OPEN gate outside a fresh window is the blind entry this file forbids.
   */
  readonly waitForWindow: (
    gates: readonly TimedGate[],
    hold?: Vec3Tuple,
    press?: Vec3Tuple,
  ) => Promise<boolean>;
  /** The bot's current feet cell, or `undefined` when it cannot be read. */
  readonly feetCell: () => Vec3Tuple | undefined;
  /**
   * Raw-control crossing burst for a `crush: true` entry: drive
   * straight from the near mouth `from` toward the far mouth `to`, through
   * `through`'s regions, within `budgetMs`; if the budget runs out while still
   * inside a region, retreat raw toward `from` rather than lingering. Returns
   * `true` iff the bot cleared the region on the far side. The pathfinder is the
   * wrong tool for this span (measured live: its start latency plus mid-water
   * replans lost a 1.8 s window and the closing edge killed the bot mid-crossing);
   * a raw look-and-walk is the same mechanism the unstick path and the
   * drowned-bell crossing burst already trust.
   */
  readonly dash?: (
    through: readonly TimedGate[],
    from: Vec3Tuple,
    to: Vec3Tuple,
    budgetMs: number,
  ) => Promise<boolean>;
  /** Injectable clock (tests). */
  readonly now?: () => number;
}

/**
 * Retry a hop that a `timed-gate` clock can interrupt (spec-0016 §4).
 *
 * mineflayer-pathfinder has no concept of a window: when the gate region fills
 * mid-approach it aborts the path ("Path was stopped before it could be completed!")
 * and, before this, the leg failed as though the geometry were broken. The compiler
 * already proves the crossing is READABLE (DW0378: ≥20% of every cycle admits it);
 * this teaches the runtime rung the same verb.
 *
 * Each attempt is: **stand off** — only when the bot is standing IN the fill (see
 * {@link needsStandoff}); every retreated block has to be re-walked inside the open
 * window, and DW0378's proof covers the gate SPAN, not an arbitrary run-up to it, so
 * a bot already clear waits exactly where it stands — then **wait** for the
 * closed→open edge, then re-run the hop, escalating to the ordinary stall
 * recovery inside the same window (the pathfinder loses a path whose blocks
 * are rewritten under it; a walk does not). The loop is bounded by
 * {@link gateRetryBudgetMs} (two full cycles + margin) and {@link GATE_MIN_ATTEMPTS};
 * once the budget is spent it makes one final full physical recovery (in case the
 * hop was never about the clock) and then fails loudly, naming the gate and its
 * cycle.
 *
 * This is bounded patience for legs the compiler MARKED, never a blanket retry: an
 * unmarked leg is untouched, and a marked leg that is genuinely unwalkable still
 * fails — the check is not weakened, only told what a gate is.
 *
 * Every retry decision consults the {@link LegSettled} oracle first. A
 * step can complete AT the gate crossing (the tide-mill wheelpit: the objective's
 * distance check fires as the bot lands the crossing, and its emission transports
 * the bot to the next area) — the interrupted hop then looks blocked forever from
 * a cell the one-way transport made unreachable, while the objective the leg
 * exists to reach is already complete. Objective complete ⇒ the crossing
 * SUCCEEDED; no standoff, window wait, or recovery may path the bot back.
 * Returns `"crossed"` when the hop was physically completed and `"settled"` when
 * the oracle ended it — the caller must then stop replaying the WHOLE leg, since
 * its remaining hops belong to the area the transport carried the bot out of.
 *
 * `crush: true` (the portcullis judgement): a crush gate's closing edge
 * KILLS a player caught inside the region, so the discipline above gains three hard
 * rules whenever any staged gate crushes:
 *   1. **no blind entry** — an attempt is made only after the closed→open edge was
 *      actually OBSERVED (`waitForWindow` returning `true`); "crossing anyway" when
 *      the region cannot be read is fine for a gate that merely blocks, and lethal
 *      for one that crushes;
 *   2. **full margin** — the crossing estimate (entry latency + mouth-to-mouth walk,
 *      {@link crossingEstimateMs}) must fit inside the freshly opened window, and
 *      the within-window escalation re-checks the REMAINING window before running;
 *   3. **no post-budget blind recovery** — the final "maybe it was never the clock"
 *      recovery paths straight through the gate at an arbitrary clock position, so
 *      a crush-staged crossing fails loudly instead.
 * The caller (`replayLegWithRecovery`) invokes this PROACTIVELY for a hop whose
 * straight mouth-to-mouth segment crosses a crush gate (`crossing`), staging the
 * entry before the first attempt — the reactive path only ever sees a crush gate
 * after something already went wrong, and with an instant kill there is no
 * afterwards. Checks are not weakened anywhere: refusal to enter still burns the
 * same bounded budget and still ends in the same loud failure.
 */
export async function crossTimedGate(
  spec: GoalSpec,
  glabel: string,
  proven: GoalSpec | undefined,
  gate: GateAssist,
  goto: (spec: GoalSpec, label: string) => Promise<void>,
  unstick: Unstick | undefined,
  firstErr: unknown,
  settled?: LegSettled,
  crossing?: readonly TimedGate[],
): Promise<"crossed" | "settled"> {
  const gates = gate.gates;
  // The gates this hop physically crosses (staged wait/standoff target); the whole
  // leg's table when the caller could not tell (reactive path).
  const staged = crossing && crossing.length > 0 ? crossing : gates;
  const lethal = staged.some((g) => g.crush);
  const now = gate.now ?? (() => Date.now());
  const budget = gateRetryBudgetMs(staged);
  const start = now();
  let lastErr = firstErr;
  let attempt = 0;
  const done = (): boolean => legSettledReason(settled, glabel) !== undefined;
  process.stderr.write(
    firstErr === undefined
      ? `[timed-gate] ${glabel}: staged crossing of ${describeGates(staged)} (budget ` +
          `${(budget / 1_000).toFixed(1)}s, min ${GATE_MIN_ATTEMPTS} attempts)\n`
      : `[timed-gate] ${glabel} was interrupted by ${describeGates(staged)}; waiting for a ` +
          `window (budget ${(budget / 1_000).toFixed(1)}s, min ${GATE_MIN_ATTEMPTS} attempts)\n`,
  );
  // The staging cell for a lethal crossing: the last proven waypoint, which for a
  // marked crossing hop is the compiler-pinned gate MOUTH (waypoints.rs
  // `gate_mouth_cells`) — one cell outside the fill, flanking the span DW0378
  // charges. Entry margin is judged from here, so the bot must actually BE here.
  const staging: Vec3Tuple | undefined = proven ? [proven.x, proven.y, proven.z] : undefined;
  while (attempt < GATE_MIN_ATTEMPTS || now() - start < budget) {
    if (done()) return "settled";
    attempt++;
    if (proven && needsStandoff(gate.feetCell(), staged)) {
      process.stderr.write(
        `[timed-gate] standing off to [${proven.x}, ${proven.y}, ${proven.z}] — the bot ` +
          `is inside the gate's fill\n`,
      );
      await reached(() => goto({ ...proven, range: 1 }, `${glabel} gate standoff`));
    }
    // Re-stage a lethal crossing whose bot is off-station (tide-mill: the corridor
    // current carried the idle bot 8 blocks back to the pool between attempts).
    // The mouth is where the margin proof lives; entering from anywhere else is
    // exactly the unproven dash the margin check below refuses.
    if (lethal && proven && staging && !nearCell(gate.feetCell(), staging)) {
      process.stderr.write(
        `[timed-gate] re-staging at the gate mouth [${staging.join(", ")}] — the bot ` +
          `drifted off it\n`,
      );
      await reached(() => goto({ ...proven, range: 1 }, `${glabel} gate re-stage`));
    }
    // Hold the pre-wait stance (post-standoff/re-stage) through the wait: waiting
    // where you stand is the rule (never retreat needlessly), and in a
    // current "where you stand" requires actively standing there. Never hold a
    // cell inside the fill (a range-1 arrival can land one cell into the region;
    // holding there through a crush close is the death itself) — fall back to the
    // proven mouth.
    const feetNow = gate.feetCell();
    const holdCell =
      feetNow && !staged.some((g) => insideGate(feetNow, g)) ? feetNow : staging;
    const observed = await gate.waitForWindow(
      staged,
      holdCell,
      lethal ? [spec.x, spec.y, spec.z] : undefined,
    );
    const openedAt = now();
    // The window wait is long (up to a full cycle) — the settle signal may have
    // landed during it, in which case there is no crossing left to attempt.
    if (done()) return "settled";
    if (lethal && !observed) {
      // Rule 1: a crush gate is never entered on faith. The refusal consumes a
      // bounded wait, so the loop still terminates on the same budget and the
      // failure below still names the gate.
      lastErr = new Error(
        `the gate's closed→open edge could not be observed and ` +
          `${describeGates(staged.filter((g) => g.crush))} crushes — refusing blind entry`,
      );
      process.stderr.write(`[timed-gate] attempt ${attempt}: ${(lastErr as Error).message}\n`);
      continue;
    }
    let crushWindowMs: number | undefined;
    if (lethal) {
      // Rule 2: the crossing must fit the fresh window with margin. The crossing
      // is judged from where the bot actually stands (fallback: the proven mouth),
      // against the SHORTEST open half among the crushing gates.
      const from = gate.feetCell() ?? staging;
      const windowMs = Math.min(...staged.filter((g) => g.crush).map((g) => openMs(g)));
      crushWindowMs = windowMs;
      const estimate = from ? crossingEstimateMs(from, [spec.x, spec.y, spec.z]) : undefined;
      if (estimate !== undefined && estimate >= windowMs) {
        // A failed margin from OFF the mouth is a stance problem, not a design
        // problem: the hold/re-stage above lost to whatever moved the bot (the
        // tide-mill current) — take another attempt, which re-stages first. Only
        // a failed margin from ON the compiler-pinned mouth is terminal: DW0378
        // proves every shipped window admits its designed crossing, so that
        // firing means the artifact disagrees with the world, and entering would
        // gamble the bot's life on a proof that no longer applies.
        if (staging && !nearCell(gate.feetCell(), staging)) {
          lastErr = new Error(
            `drifted off the staging mouth [${staging.join(", ")}] to ` +
              `[${from!.join(", ")}] during the window wait — re-staging`,
          );
          process.stderr.write(
            `[timed-gate] attempt ${attempt}: ${(lastErr as Error).message}\n`,
          );
          continue;
        }
        throw new Error(
          `${glabel}: crossing estimate ${(estimate / 1_000).toFixed(1)}s from ` +
            `[${from!.join(", ")}] does not fit the ${(windowMs / 1_000).toFixed(1)}s open ` +
            `window of crushing ${describeGates(staged.filter((g) => g.crush))} — refusing ` +
            `to enter a crush gate without full margin (DW0378 proves the designed ` +
            `crossing fits from the pinned mouth, where the bot is staged)`,
        );
      }
    }
    const alabel = `${glabel} gate attempt ${attempt}`;
    try {
      // A crush entry crosses RAW first (zero pathfinder latency — the flood
      // through the freshly opened plane outruns a pathfinder start), then the
      // ordinary goto merely verifies/finishes the arrival from outside the fill.
      if (lethal && gate.dash && staging && crushWindowMs !== undefined) {
        const dashBudget = crushWindowMs - (now() - openedAt);
        const cleared = await gate.dash(staged, staging, [spec.x, spec.y, spec.z], dashBudget);
        if (!cleared) {
          throw new Error(
            `the raw crossing dash did not clear the gate within its ` +
              `${(Math.max(0, dashBudget) / 1_000).toFixed(1)}s window`,
          );
        }
      }
      await goto(spec, alabel);
      return "crossed";
    } catch (err) {
      if (err instanceof BotDeathError) throw err;
      if (done()) return "settled";
      lastErr = err;
      // Every attempt's own reason is logged, not just the last one: a run where the
      // bot never moved and a run where it crossed and was cut off look identical in
      // a single terminal message, and telling them apart is the whole diagnosis.
      process.stderr.write(
        `[timed-gate] attempt ${attempt} pathfind failed: ` +
          `${err instanceof Error ? err.message : String(err)}\n`,
      );
    }
    // Escalate INSIDE the window (observed on the-drowned-bell's portcullis): the
    // pathfinder walks the bot to the cell at the gate's mouth and then aborts with
    // "Path was stopped before it could be completed!", every window, while the raw
    // physics burst walks the very same span in a fraction of a second. A path whose
    // blocks are rewritten under it twice per cycle is not something A* can hold on
    // to; the last blocks of a clocked span have to be crossed by walking. That is
    // the ordinary stall escalation (pathfind → look-and-walk burst →
    // re-path), reused verbatim — plain movement a human player makes, still
    // bounded, and it still has to physically get through an open gate.
    //
    // Crush addendum (rule 2): the escalation is only run while the REMAINING
    // window still fits the crossing — bursting into a crush gate as it closes is
    // exactly the death this function exists to prevent. Out of margin ⇒ take the
    // next fresh window instead (the loop's next iteration).
    if (proven) {
      if (lethal) {
        const from = gate.feetCell() ?? ([proven.x, proven.y, proven.z] as Vec3Tuple);
        const windowMs = Math.min(...staged.filter((g) => g.crush).map((g) => openMs(g)));
        const remaining = windowMs - (now() - openedAt);
        if (crossingEstimateMs(from, [spec.x, spec.y, spec.z]) >= remaining) {
          process.stderr.write(
            `[timed-gate] attempt ${attempt}: remaining window ` +
              `${(Math.max(0, remaining) / 1_000).toFixed(1)}s is too short to escalate into a ` +
              `crush gate — waiting for the next fresh window\n`,
          );
          continue;
        }
      }
      try {
        await recoverAndRetry(spec, alabel, proven, goto, unstick);
        return "crossed";
      } catch (err) {
        if (err instanceof BotDeathError) throw err;
        if (done()) return "settled";
        lastErr = err;
        process.stderr.write(
          `[timed-gate] attempt ${attempt} physical crossing failed: ` +
            `${err instanceof Error ? err.message : String(err)}\n`,
        );
      }
    }
  }
  // Budget spent. Before calling it a failure, give the hop the ordinary physical
  // recovery — the gate mark says a clock CAN interrupt this leg, not that every
  // failure on it is the clock's doing. NOT for a crush-staged crossing (rule 3):
  // this recovery paths straight through the gate at an arbitrary clock position,
  // which on a crushing gate is the lethal blind entry itself.
  if (done()) return "settled";
  if (!lethal) {
    try {
      if (proven) {
        await recoverAndRetry(spec, glabel, proven, goto, unstick);
      } else {
        await goto(spec, glabel);
      }
      return "crossed";
    } catch (err) {
      if (err instanceof BotDeathError) throw err;
      if (done()) return "settled";
      lastErr = err;
    }
  }
  const detail = lastErr instanceof Error ? lastErr.message : String(lastErr);
  throw new Error(
    `${glabel}: still blocked after ${attempt} timed-gate crossing attempt(s) over ` +
      `${((now() - start) / 1_000).toFixed(1)}s — more than two full cycles of ` +
      `${describeGates(staged)}. The window is not the problem; this is a real ` +
      `navigation failure: ${detail}`,
  );
}

/** The executor's methods this file holds; `executor.ts` installs them on its prototype. */
export const methods = {
  /**
   * Whether every cell of `gate`'s compiler-declared region currently reads as empty
   * space — i.e. the clock has the gate OPEN. `undefined` when the region's blocks
   * cannot be read (chunk not loaded), so the caller can distinguish "shut" from
   * "cannot see". The emptiness test is the block's own collision shape
   * (`boundingBox === "empty"`), not a block-name comparison, so it stays correct for
   * whatever block a campaign fills its gate with.
   */
  gateOpen(this: MineflayerExecutor, gate: TimedGate): boolean | undefined {
    const bot = this.requireBot();
    const p = bot.entity.position;
    for (const [x, y, z] of gateRegionCells(gate)) {
      const block = bot.blockAt(p.offset(x - p.x, y - p.y, z - p.z));
      if (!block) return undefined; // not loaded — state unknown
      if (block.boundingBox !== "empty") return false;
    }
    return true;
  },

  /**
   * Wait for `gates` to swing from closed to OPEN, so a crossing starts at the top of
   * the window rather than its tail (spec-0016 §4).
   *
   * Two bounded phases: first watch until the gates read CLOSED (so an already-open
   * window whose remaining ticks are unknown is not mistaken for a fresh one), then
   * watch until they read OPEN. Each phase is capped at {@link gateWindowWaitMs} —
   * one full cycle plus margin, within which the clock is guaranteed to produce the
   * edge — so an unreadable region (chunk not loaded) can never hang the run; the
   * wait simply gives up and the caller tries the hop anyway.
   *
   * Returns `true` iff the fresh closed→open edge was actually OBSERVED (both
   * phases succeeded). `false` means the caller knows nothing about the clock —
   * safe to try on a gate that merely blocks, forbidden on one that crushes.
   *
   * `hold`: a feet cell to actively KEEP while watching. The tide-mill
   * gate corridor is flowing water; a bot that merely idles is carried off the
   * staging mouth by the current (observed: 8 blocks back to the pool during one
   * 4-second wait), and every drifted block must be re-walked inside the open
   * window. A dynamic pathfinder goal pinned to the cell is the bot's equivalent
   * of a player holding a movement key against the tide. Cleared before returning,
   * so the crossing `goto` starts from clean pathfinder state.
   *
   * This is navigation, not game logic: the harness reads the world only to TIME a
   * movement the compiler already proved possible. It asserts nothing about the gate.
   */
  async waitForGateWindow(
    this: MineflayerExecutor,
    gates: readonly TimedGate[],
    hold?: Vec3Tuple,
    press?: Vec3Tuple,
  ): Promise<boolean> {
    const bot = this.requireBot();
    const cap = gateWindowWaitMs(gates);
    const allOpen = (): boolean | undefined => {
      let known = true;
      for (const g of gates) {
        const open = this.gateOpen(g);
        if (open === undefined) known = false;
        else if (!open) return false;
      }
      return known ? true : undefined;
    };
    // Station-keeping with RAW controls, once per poll. A dynamic pathfinder goal
    // was tried first and lost ~4 blocks per wait: the pathfinder "arrives", clears
    // its controls, the current takes the idle bot, and the re-solve lags the
    // drift. Raw look-and-walk is the mechanism this file already trusts against
    // clocked geometry (the unstick path, the-drowned-bell's crossing burst):
    // face the station, hold forward while off it, release on it. Drift downstream
    // is corrected against the current the same way the proven approach hops walk
    // it; an overshoot past the mouth is carried back by the very current that
    // caused it (and the shut gate is a solid wall — the region cannot be entered
    // while it matters).
    //
    // With `press` (crush staging), a gate that provably reads CLOSED upgrades the
    // stance: lean INTO the shut plane toward the crossing target, so the open
    // edge releases a bot that is already in contact and already moving. Solid
    // blocks make the lean safe; the moment the state is open (or unknown), the
    // stance falls back to holding the mouth.
    const keepStation = async (): Promise<void> => {
      if (press && allOpen() === false) {
        const p = bot.entity.position;
        try {
          await bot.lookAt(p.offset(press[0]! + 0.5 - p.x, 0, press[2]! + 0.5 - p.z), true);
        } catch {
          // best effort — a look failure must not abort the watch
        }
        bot.setControlState("sprint", true);
        bot.setControlState("forward", true);
        return;
      }
      if (!hold) return;
      const p = bot.entity.position;
      const dx = hold[0]! + 0.5 - p.x;
      const dz = hold[2]! + 0.5 - p.z;
      if (dx * dx + dz * dz > GATE_HOLD_SLACK_SQ) {
        try {
          await bot.lookAt(p.offset(dx, 0, dz), true);
        } catch {
          // best effort — a look failure must not abort the watch
        }
        bot.setControlState("forward", true);
      } else {
        bot.setControlState("forward", false);
        bot.setControlState("sprint", false);
      }
    };
    const watch = async (want: boolean, phase: string): Promise<boolean> => {
      const deadline = Date.now() + cap;
      while (Date.now() < deadline) {
        if (allOpen() === want) return true;
        await keepStation();
        await delay(GATE_POLL_MS);
      }
      process.stderr.write(
        `[timed-gate] gave up waiting for the gate to read ${phase} after ` +
          `${(cap / 1_000).toFixed(1)}s — crossing on the next attempt regardless\n`,
      );
      return false;
    };
    try {
      if (!(await watch(false, "closed"))) return false;
      process.stderr.write(`[timed-gate] gate is shut; waiting for it to open\n`);
      if (await watch(true, "open")) {
        process.stderr.write(`[timed-gate] window open — crossing now\n`);
        return true;
      }
      return false;
    } finally {
      if (hold || press) {
        bot.clearControlStates();
      }
    }
  },

  /**
   * Raw-control crossing dash for a `crush: true` gate entry (see
   * {@link GateAssist.dash}). Face the far mouth and drive forward (sprinting)
   * from the near mouth until the far mouth is reached, re-aiming every
   * {@link GATE_DASH_TICK_MS}. No pathfinder anywhere: the tide-mill corridor
   * floods through the freshly opened plane, and a pathfinder start (plus its
   * mid-water replans) measured slower than the 1.8 s window — while raw forward
   * drive is exactly how the proven approach hops already beat the same current.
   *
   * Bounded by `budgetMs`: if it expires with the bot still inside a gate region,
   * the dash REVERSES raw toward `from` (the current helps — it points out the
   * near side) so the closing edge finds the bot outside the fill, and reports
   * failure for the caller to take the next window. Never a check weakened: a
   * dash that cannot clear still fails its attempt loudly.
   */
  async dashThroughGate(
    this: MineflayerExecutor,
    through: readonly TimedGate[],
    from: Vec3Tuple,
    to: Vec3Tuple,
    budgetMs: number,
  ): Promise<boolean> {
    const bot = this.requireBot();
    const inside = (): boolean => {
      const feet = this.feetCell();
      return feet !== undefined && through.some((g) => insideGate(feet, g));
    };
    const driveToward = async (cell: Vec3Tuple): Promise<void> => {
      const p = bot.entity.position;
      try {
        await bot.lookAt(p.offset(cell[0]! + 0.5 - p.x, 0, cell[2]! + 0.5 - p.z), true);
      } catch {
        // best effort — a look failure must not abort the dash
      }
      bot.setControlState("sprint", true);
      bot.setControlState("forward", true);
      // Deliberately NO jump: measured live in the tide-mill race (1–2 deep
      // flowing water), holding jump turns the drive into an upward swim that
      // lifted the bot into the fill plane at head height — slower AND lethal.
      // The press stance (see waitForGateWindow) is what buys the crossing time.
    };
    try {
      bot.clearControlStates();
      const deadline = Date.now() + Math.max(GATE_DASH_TICK_MS * 4, budgetMs);
      while (Date.now() < deadline) {
        const p = bot.entity.position;
        const dx = to[0]! + 0.5 - p.x;
        const dz = to[2]! + 0.5 - p.z;
        // Arrived on the far mouth, out of the fill: crossed.
        if (dx * dx + dz * dz <= GATE_DASH_ARRIVE_SQ && !inside()) {
          return true;
        }
        await driveToward(to);
        await delay(GATE_DASH_TICK_MS);
      }
      if (!inside()) {
        // Out of budget but already clear of every region — let the caller's goto
        // finish (or fail) the hop; nothing here is in the fill's path.
        process.stderr.write(
          `[timed-gate] dash out of window at ${fmt(bot.entity.position)} (clear of the ` +
            `fill; aiming for [${to.join(", ")}])\n`,
        );
        return false;
      }
      // Emergency: still inside the fill with the window spent. Reverse OUT the
      // near side — with the corridor current, retreat is downhill.
      process.stderr.write(
        `[timed-gate] dash out of window while inside the gate — retreating raw to ` +
          `[${from.join(", ")}]\n`,
      );
      const retreatDeadline = Date.now() + GATE_DASH_RETREAT_MS;
      while (Date.now() < retreatDeadline && inside()) {
        await driveToward(from);
        await delay(GATE_DASH_TICK_MS);
      }
      return false;
    } finally {
      bot.clearControlStates();
    }
  },
};
