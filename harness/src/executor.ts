// mineflayer-backed StepExecutor. Connects a headless bot to a pinned 1.21.11
// server and drives the critical path against the amended bot-interaction
// contract (spec-0002, 2026-07-30).
//
// Interaction channel (settled): Minecraft 1.21.6+ routes NPC dialogue / class
// selection through the server-driven dialog system. mineflayer 4.37.x exposes no
// high-level dialog API and cannot reliably emit a dialog button click, so the
// compiler emits every dialog button as a `run_command` firing a `/trigger`, and
// the bot drives the same outcome by chatting that exact command (`bot.chat`).
// select-class / talk-to therefore just send `step.command`; talk-to walks to the
// NPC first (realism + reach mechanics that some dialogs gate on).

import { createBot, type Bot } from "mineflayer";
// mineflayer-pathfinder is CommonJS; import the default and destructure (the only
// harness dependency added for v0.3 — replaces the naive "face + hold forward"
// walk so turns/branches in jigsaw layouts are walkable).
import pathfinderPkg from "mineflayer-pathfinder";
const { pathfinder, Movements, goals } = pathfinderPkg;
import type {
  AssertCompleteStep,
  CollectStep,
  InteractStep,
  KillStep,
  ReachStep,
  SelectClassStep,
  TalkToStep,
  Vec3Tuple,
} from "./critical-path.ts";
import type { StepExecutor } from "./sequencer.ts";
import { BotDeathError, likelyDeathCause } from "./death.ts";
import { allowNonCollidingEntities, configureLeg } from "./movement.ts";
import { nextLegWaypoints, walkGoals, type GoalSpec, type Waypoints } from "./waypoints.ts";

/** Bounded number of physics-unstick bursts before a wedged hop fails loudly. */
const UNSTICK_ATTEMPTS = 3;

/**
 * A raw, pathfinder-free nudge toward `target` to dislodge a physically wedged bot
 * (a concave corner beside a wall the A* pathfinder cannot escape). Navigation
 * robustness, NOT game logic. Provided by the executor; injected so the recovery
 * control flow stays unit-testable.
 */
export type Unstick = (target: GoalSpec) => Promise<void>;

/**
 * Replay a leg's ordered goals with **stall-recovery** (task #45). Each `goto`
 * performs one verified hop (rejecting on stall / death). Extracted from `walkTo`
 * as a pure control-flow function — injecting `goto` (and an optional physics
 * `unstick`) — so the recovery logic is unit-testable without a live pathfinder.
 *
 * A leg replays compiler-proven cells at `WAYPOINT_RANGE = 1`. That range-1
 * tolerance lets the bot satisfy the PREVIOUS hop at an off-route cell — a corner
 * pocket beside a wall — from which the next hop wedges: the bot oscillates and
 * times out (the nobodys-cave perimedes approach). Recovery escalates:
 *   1. re-path to the exact last proven cell (`range 0`, back onto the proven
 *      polyline); if that succeeds, retry the hop;
 *   2. if the recovery pathfind ITSELF stalls (the wedge defeats the pathfinder
 *      too), fall back to a bounded {@link Unstick} — a raw look+forward(+jump)
 *      burst toward the proven cell that bypasses the pathfinder — and after each
 *      burst retry the **actual next hop at its own range** (never the proven cell
 *      at range 0: a freed bot overshoots the strict target and oscillates; the
 *      hop's normal range 1 is forgiving enough to land).
 * Range 0 is used only for the level-1 re-centre, so a legitimate slab/stair
 * fractional-height floor on the happy path is unaffected, and the per-hop `goto`
 * timeout is untouched. A first-hop stall (nothing proven yet) is not this class and
 * is rethrown; a hop still unwalkable after recovery + unstick fails loudly.
 */
export async function replayLegWithRecovery(
  goalsList: readonly GoalSpec[],
  label: string,
  goto: (spec: GoalSpec, label: string) => Promise<void>,
  unstick?: Unstick,
): Promise<void> {
  let lastProven: GoalSpec | undefined;
  for (let g = 0; g < goalsList.length; g++) {
    const spec = goalsList[g]!;
    const last = g === goalsList.length - 1;
    const glabel = last ? label : `${label} waypoint ${g + 1}/${goalsList.length}`;
    try {
      await goto(spec, glabel);
    } catch (err) {
      if (err instanceof BotDeathError) throw err;
      if (!lastProven) throw err; // nothing proven yet — not the pocket-wedge class
      await recoverAndRetry(spec, glabel, lastProven, goto, unstick);
    }
    lastProven = spec;
  }
}

/** Try a `goto`, returning whether it arrived; a bot death still propagates. */
async function reached(fn: () => Promise<void>): Promise<boolean> {
  try {
    await fn();
    return true;
  } catch (err) {
    if (err instanceof BotDeathError) throw err;
    return false;
  }
}

/**
 * Recover from a stalled hop and retry it (task #45). Level 1: re-path to the last
 * proven cell (range 0) to re-centre on the polyline, then retry the hop. Level 2
 * (the wedge defeats the pathfinder too): a bounded physics {@link Unstick} toward
 * the proven cell to break the bot free, retrying the ACTUAL hop at its own range
 * after each burst. A hop still unwalkable after the budget fails loudly.
 */
async function recoverAndRetry(
  spec: GoalSpec,
  glabel: string,
  proven: GoalSpec,
  goto: (spec: GoalSpec, label: string) => Promise<void>,
  unstick?: Unstick,
): Promise<void> {
  const provenGoal: GoalSpec = { x: proven.x, y: proven.y, z: proven.z, range: 0 };
  process.stderr.write(
    `[recover] re-centering on proven cell [${proven.x}, ${proven.y}, ${proven.z}] ` +
      `(range 0), then retrying ${glabel}\n`,
  );
  // Level 1: pathfinder re-centre, then retry the hop.
  if (await reached(() => goto(provenGoal, `${glabel} recovery to last proven cell`))) {
    await goto(spec, glabel); // retry; rethrows if still stuck
    return;
  }
  // Level 2: the recovery pathfind stalled too — the bot is physically wedged. Break
  // it free with a bounded raw-movement burst toward the proven cell, then retry the
  // ACTUAL hop at its own (forgiving) range. Escalate the burst to a jump only when a
  // gentle burst made no progress.
  if (unstick) {
    for (let a = 0; a < UNSTICK_ATTEMPTS; a++) {
      process.stderr.write(`[recover] physics-unstick burst ${a + 1}/${UNSTICK_ATTEMPTS}\n`);
      await unstick(provenGoal);
      if (await reached(() => goto(spec, `${glabel} retry after unstick ${a + 1}`))) {
        return;
      }
    }
  }
  await goto(spec, glabel); // budget exhausted — surface the failure loudly
}

/** Connection + identity for the bot. Sourced from the environment (see below). */
export interface BotConfig {
  readonly host: string;
  readonly port: number;
  readonly username: string;
  /** Pinned per ADR-0009; mineflayer's max supported version is 1.21.11. */
  readonly version: string;
  /** `offline` for local/CI (offline-mode server); `microsoft` for real accounts. */
  readonly auth: "offline" | "microsoft";
}

/** The pinned Minecraft version (ADR-0009). Single source of truth for the harness. */
export const PINNED_MC_VERSION = "1.21.11";

/**
 * Build a {@link BotConfig} from environment variables, with local-testing
 * defaults:
 *   DELVEWRIGHT_MC_HOST      (default `127.0.0.1`)
 *   DELVEWRIGHT_MC_PORT      (default `25565`)
 *   DELVEWRIGHT_BOT_USERNAME (default `delve-bot`)
 *   DELVEWRIGHT_MC_VERSION   (default `1.21.11`, the ADR-0009 pin)
 *   DELVEWRIGHT_MC_AUTH      (`offline` | `microsoft`, default `offline`)
 */
export function botConfigFromEnv(
  env: Record<string, string | undefined> = process.env,
): BotConfig {
  const portRaw = env["DELVEWRIGHT_MC_PORT"] ?? "25565";
  const port = Number.parseInt(portRaw, 10);
  if (!Number.isInteger(port) || port <= 0 || port > 65535) {
    throw new Error(
      `DELVEWRIGHT_MC_PORT must be a valid TCP port, got ${JSON.stringify(portRaw)}`,
    );
  }
  const authRaw = env["DELVEWRIGHT_MC_AUTH"] ?? "offline";
  if (authRaw !== "offline" && authRaw !== "microsoft") {
    throw new Error(
      `DELVEWRIGHT_MC_AUTH must be 'offline' or 'microsoft', got ${JSON.stringify(authRaw)}`,
    );
  }
  return {
    host: env["DELVEWRIGHT_MC_HOST"] ?? "127.0.0.1",
    port,
    username: env["DELVEWRIGHT_BOT_USERNAME"] ?? "delve-bot",
    version: env["DELVEWRIGHT_MC_VERSION"] ?? PINNED_MC_VERSION,
    auth: authRaw,
  };
}

/** How long (ms) a movement step may run before it is declared failed. */
const REACH_TIMEOUT_MS = 60_000;
/** Polling interval (ms) while walking toward a target. */
const REACH_POLL_MS = 250;
/** How long (ms) a `kill` step may run before it is declared failed. */
const KILL_TIMEOUT_MS = 90_000;
/** Attack cadence (ms) — roughly the vanilla sword cooldown. */
const ATTACK_INTERVAL_MS = 400;
/**
 * How long (ms) to wait for a scoreboard value to reach its target after a chat
 * command. The datapack acts on the trigger on the next server tick(s); give it a
 * generous window so slow CI servers don't flake.
 */
const SCORE_SETTLE_MS = 15_000;
const SCORE_POLL_MS = 250;
/** Settle time (ms) after class selection (teleport + kit give) before moving on. */
const CLASS_SETTLE_MS = 3_000;
/**
 * Physics-unstick (task #45): a SHORT forward tap per burst (~a cell, not a launch —
 * a long burst overshoots a tight 2-wide corridor and oscillates wall-to-wall), a
 * jump only when a gentle burst moved less than `UNSTICK_MIN_PROGRESS` blocks (truly
 * wedged against a lip), and a brief settle before re-pathing.
 */
const UNSTICK_BURST_MS = 250;
const UNSTICK_MIN_PROGRESS = 0.5;
const UNSTICK_SETTLE_MS = 300;
/**
 * gap 8: how long (ms) to wait for a cross-area teleport to land after a step whose
 * completion relocates the player, how close (blocks, horizontal) counts as
 * "arrived at the destination", and how long to settle once it has.
 */
const TRANSPORT_TIMEOUT_MS = 15_000;
const TRANSPORT_NEAR = 4;
const TRANSPORT_SETTLE_MS = 1_500;
/**
 * gap 8 (task #32): a server-forced position jump of at least this many blocks
 * (horizontal) is treated as a cross-area teleport rather than knockback or a
 * within-area nudge. Areas sit ~256 blocks apart across void (an unambiguous jump);
 * in-area relocations (spawn, class teleport) stay well under this, so the threshold
 * cleanly separates a transport from ordinary forced moves. When one is observed the
 * pathfinder is reset, so a path computed in the OLD area cannot survive the jump and
 * strand the next step with a spurious "No path to the goal!".
 */
const TRANSPORT_JUMP_BLOCKS = 64;
/**
 * gap 8 (task #32): after the jump lands, how long (ms) to wait for the destination
 * chunk to load and the bot to come to rest on solid ground before the next step
 * starts pathfinding, and the poll cadence. The pathfinder's A* fails immediately
 * ("No path to the goal!") if it starts while the block under the bot is still
 * unloaded (`blockAt` → null) — the race this closes. Bounded: on timeout the wait
 * settles and lets the next step surface its own diagnostic.
 */
const FOOTING_TIMEOUT_MS = 10_000;
const FOOTING_POLL_MS = 100;
/**
 * gap 7 (cutscene): how long (ms) the bot's position must hold steady, once it is
 * back in adventure mode, before control counts as restored; how far (blocks) a
 * position may drift and still count as "steady"; and the grace added on top of the
 * declared cutscene length before the wait gives up and continues (bounded so a
 * cutscene glitch cannot hang the run). Grace is env-tunable for tests.
 */
const CUTSCENE_SETTLE_MS = 500;
const CUTSCENE_STEADY_EPS = 0.05;
const CUTSCENE_POLL_MS = 250;
/** gap 7 (retry): how long (ms) to wait for the bot to respawn before resuming. */
const RESPAWN_TIMEOUT_MS = 15_000;
/** Recent chat lines retained for death-cause diagnosis. */
const CHAT_BUFFER = 16;

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/** Reject with a labelled error if `promise` does not settle within `ms`. */
function withTimeout<T>(promise: Promise<T>, ms: number, what: string): Promise<T> {
  let timer: ReturnType<typeof setTimeout>;
  const guard = new Promise<never>((_, reject) => {
    timer = setTimeout(() => reject(new Error(`${what}: timed out after ${ms}ms`)), ms);
  });
  return Promise.race([promise, guard]).finally(() => clearTimeout(timer));
}

/**
 * A mineflayer-backed executor. Construct, `await connect()`, then hand it to
 * `runSequence`. `close()` disconnects the bot. Not reusable across servers.
 */
/**
 * Stable completion marker the compiler broadcasts on campaign completion:
 * `[Delvewright] complete <objective> <value>`. The bot observes THIS (chat is
 * reliably parsed by mineflayer) rather than the sidebar score, because mineflayer
 * 4.37.x cannot decode 1.21.11 scoreboard score packets. The datapack still
 * displays the objective in the sidebar (amended contract, spec-0002) — this is
 * only the harness's observation channel, and it can switch to the live sidebar
 * read once mineflayer gains 1.21.11 score support.
 */
const COMPLETION_MARKER = /\[Delvewright\] complete (\S+) (-?\d+)/;

export class MineflayerExecutor implements StepExecutor {
  private readonly config: BotConfig;
  private bot: Bot | undefined;
  /** Latest value seen per objective from broadcast completion markers. */
  private readonly markerScores = new Map<string, number>();
  /**
   * gap 7 (death): set once when the bot dies; long waits race against it so a death
   * fails FAST with a diagnostic instead of respawning and pathfinding across the void.
   */
  private death: BotDeathError | undefined;
  /** One-shot callbacks armed by {@link raceDeath}, fired on death. */
  private readonly deathWaiters = new Set<(err: BotDeathError) => void>();
  /** Ring buffer of recent chat lines, mined for the death-cause message. */
  private readonly recentChat: string[] = [];
  /**
   * gap 8 (task #32): the bot position captured at the previous server-forced move
   * (`forcedMove`). A forced move whose horizontal delta from this reaches
   * {@link TRANSPORT_JUMP_BLOCKS} is a cross-area teleport; used to reset the
   * pathfinder so a path computed in the old area cannot survive the jump.
   */
  private lastForcedPos: { x: number; y: number; z: number } | undefined;
  /** Grace (ms) added onto a cutscene's declared length before giving up. */
  private readonly cutsceneGraceMs: number;
  /**
   * task #38: the compiler's proven per-leg critical-path waypoints (keyed by
   * destination anchor). When a walked step's target has a leg here, `walkTo`
   * replays it as successive nearby goals so each mineflayer A* solve is trivial —
   * instead of one distant goal that strands the bot on a large open winding cave.
   * Absent → the original single distant-goal behavior (fallback). Compiler-proven
   * navigation data, not a route the harness computes.
   */
  private waypoints: Waypoints | undefined;
  /**
   * task #38: how many walked legs have been consumed. Legs are matched in lockstep
   * path order (not by destination coordinate), so an anchor visited more than once
   * — e.g. the cave entry the player returns to — never grabs the wrong leg's route.
   */
  private legCursor = 0;

  constructor(config: BotConfig, env: Record<string, string | undefined> = process.env) {
    this.config = config;
    const raw = env["DELVEWRIGHT_CUTSCENE_GRACE_MS"];
    const parsed = raw === undefined ? NaN : Number.parseInt(raw, 10);
    this.cutsceneGraceMs = Number.isInteger(parsed) && parsed >= 0 ? parsed : 10_000;
  }

  /**
   * task #38: supply the compiler's proven critical-path waypoints. Optional —
   * without it, `walkTo` uses the original single distant-goal behavior. Called by
   * the entrypoint when the `validation/critical-path-waypoints.json` artifact
   * accompanies the critical path.
   */
  useWaypoints(waypoints: Waypoints): void {
    this.waypoints = waypoints;
  }

  /** Connect and resolve once the bot has spawned into the world. */
  async connect(): Promise<void> {
    const bot = createBot({
      host: this.config.host,
      port: this.config.port,
      username: this.config.username,
      version: this.config.version,
      auth: this.config.auth,
    });
    this.bot = bot;
    bot.loadPlugin(pathfinder);
    this.installHandlers(bot);

    await new Promise<void>((resolve, reject) => {
      const onSpawn = (): void => {
        cleanup();
        resolve();
      };
      const onError = (err: Error): void => {
        cleanup();
        reject(err);
      };
      const onEnd = (reason: string): void => {
        cleanup();
        reject(new Error(`bot disconnected before spawn: ${reason}`));
      };
      const onKicked = (reason: string): void => {
        cleanup();
        reject(new Error(`bot kicked before spawn: ${reason}`));
      };
      const cleanup = (): void => {
        bot.removeListener("spawn", onSpawn);
        bot.removeListener("error", onError);
        bot.removeListener("end", onEnd);
        bot.removeListener("kicked", onKicked);
      };
      bot.once("spawn", onSpawn);
      bot.once("error", onError);
      bot.once("end", onEnd);
      bot.once("kicked", onKicked);
    });
  }

  private requireBot(): Bot {
    if (!this.bot) {
      throw new Error("executor is not connected; call connect() first");
    }
    return this.bot;
  }

  /**
   * Wire the always-on listeners: completion-marker + chat-ring capture, and the
   * death handler. Called from {@link connect} and from {@link attachBot} (tests).
   */
  private installHandlers(bot: Bot): void {
    // Capture completion markers from the moment we connect: the marker is
    // broadcast when the campaign completes, which happens DURING the final reach
    // step (before assertComplete runs), so it must be buffered as it arrives. The
    // same stream feeds the recent-chat ring the death diagnostic mines for a cause.
    bot.on("messagestr", (message: string) => {
      const match = COMPLETION_MARKER.exec(message);
      if (match) {
        this.markerScores.set(match[1]!, Number.parseInt(match[2]!, 10));
      }
      this.recentChat.push(message);
      if (this.recentChat.length > CHAT_BUFFER) {
        this.recentChat.shift();
      }
    });
    bot.on("death", () => this.onDeath());
    // gap 8 (task #32): mineflayer applies a server position packet to
    // `bot.entity.position` and THEN emits `forcedMove` (lib/plugins/physics.js).
    // A large horizontal jump is the compiler's cross-area `teleport` landing; stop
    // the pathfinder so any path/goal computed in the old area is dropped rather than
    // fought or resumed across the void (the "No path to the goal!" / "Path was
    // stopped" race documented in the nobodys-cave gap-8 field notes).
    bot.on("forcedMove", () => this.onForcedMove());
  }

  /**
   * Handle a server-forced position update. Records the new position and, when the
   * jump is large enough to be a cross-area teleport, resets the pathfinder. Reading
   * the position after the event is correct: mineflayer sets it before emitting.
   */
  private onForcedMove(): void {
    const bot = this.bot;
    const p = bot?.entity?.position;
    if (!p) return;
    const now = { x: p.x, y: p.y, z: p.z };
    const prev = this.lastForcedPos;
    this.lastForcedPos = now;
    if (prev && Math.hypot(now.x - prev.x, now.z - prev.z) >= TRANSPORT_JUMP_BLOCKS) {
      try {
        bot.pathfinder.stop();
      } catch {
        // best effort — resetting the path must never mask the relocation
      }
    }
  }

  /**
   * Test/advanced seam: adopt an already-created (or fake) bot and install the same
   * handlers `connect()` wires, without the network path. Unit tests use this to
   * drive death/cutscene behaviour against a mocked bot.
   */
  attachBot(bot: Bot): void {
    this.bot = bot;
    this.installHandlers(bot);
  }

  /**
   * Death handler: record where and (best-effort) why the bot died, stop any
   * in-flight pathfinding, and fire the death waiters so the current long-running
   * step rejects promptly with a {@link BotDeathError} instead of hanging.
   */
  private onDeath(): void {
    if (this.death) return; // already recorded this death
    const bot = this.bot;
    let position: readonly [number, number, number] | undefined;
    const p = bot?.entity?.position;
    if (p) {
      position = [Math.round(p.x), Math.round(p.y), Math.round(p.z)];
    }
    const cause = likelyDeathCause(this.recentChat, bot?.username ?? "");
    const err = new BotDeathError(position, cause);
    this.death = err;
    try {
      bot?.pathfinder.stop();
    } catch {
      // best effort — stopping the pathfinder must never mask the death
    }
    for (const waiter of this.deathWaiters) {
      waiter(err);
    }
    this.deathWaiters.clear();
  }

  /**
   * Race `op` against the bot dying: resolves/rejects with `op`, but rejects with the
   * {@link BotDeathError} the instant a death is observed (the underlying op keeps
   * running but the pathfinder is already stopped in {@link onDeath}). Used to abort
   * the ~60s `pathfinder.goto` wait the moment the bot dies.
   */
  private raceDeath<T>(op: Promise<T>): Promise<T> {
    if (this.death) return Promise.reject(this.death);
    return new Promise<T>((resolve, reject) => {
      let settled = false;
      const onDeath = (err: BotDeathError): void => {
        if (settled) return;
        settled = true;
        reject(err);
      };
      this.deathWaiters.add(onDeath);
      op.then(
        (value) => {
          if (settled) return;
          settled = true;
          this.deathWaiters.delete(onDeath);
          resolve(value);
        },
        (err: unknown) => {
          if (settled) return;
          settled = true;
          this.deathWaiters.delete(onDeath);
          reject(err);
        },
      );
    });
  }

  /**
   * The death recorded so far, if any. Diagnostic accessor (also lets tests assert the
   * captured position/cause after a simulated death).
   */
  deathDiagnostic(): BotDeathError | undefined {
    return this.death;
  }

  /**
   * gap 7 (retry path): ready the bot to resume after a death — wait for it to respawn,
   * then clear the death latch so subsequent steps run against the live bot again. The
   * sequencer re-runs `select-class` afterwards (respawn drops class state).
   */
  async recoverFromDeath(): Promise<void> {
    const bot = this.requireBot();
    try {
      await withTimeout(
        new Promise<void>((resolve) => {
          const onSpawn = (): void => {
            bot.removeListener("spawn", onSpawn);
            resolve();
          };
          bot.once("spawn", onSpawn);
        }),
        RESPAWN_TIMEOUT_MS,
        "waiting for respawn",
      );
    } catch (err) {
      // Best effort: if we miss the respawn spawn event, proceed anyway — the
      // re-select-class teleport re-establishes a known position regardless.
      const detail = err instanceof Error ? err.message : String(err);
      process.stderr.write(`[death] ${detail}; resuming anyway\n`);
    }
    this.death = undefined;
  }

  /** Disconnect the bot, if connected. Safe to call more than once. */
  close(): void {
    if (this.bot) {
      this.bot.end();
      this.bot = undefined;
    }
  }

  async selectClass(step: SelectClassStep): Promise<void> {
    const bot = this.requireBot();
    // The class-selection dialog button runs `step.command` (a `/trigger`); the
    // bot fires the same command directly. The per-tick handler then applies the
    // kit and teleports the player to the campaign spawn.
    bot.chat(step.command);
    // Give the datapack a tick to reset the trigger, give the kit and teleport.
    await delay(CLASS_SETTLE_MS);
    // Equip the kit (sword + armor) so the bot can fight v0.3 combat waves. A
    // no-op for kits without those items.
    await this.equipLoadout();
  }

  /**
   * Equip the best weapon and each armour piece from the current inventory. Item
   * names are matched by substring (`sword`, `helmet`, …). Best-effort per slot.
   */
  private async equipLoadout(): Promise<void> {
    const bot = this.requireBot();
    const slots: ReadonlyArray<[string, "hand" | "head" | "torso" | "legs" | "feet"]> = [
      ["sword", "hand"],
      ["helmet", "head"],
      ["chestplate", "torso"],
      ["leggings", "legs"],
      ["boots", "feet"],
    ];
    for (const [key, dest] of slots) {
      const item = bot.inventory.items().find((i) => i.name.includes(key));
      if (item) {
        try {
          await bot.equip(item, dest);
        } catch {
          // best effort — a missing slot is not a failure
        }
      }
    }
  }

  async talkTo(step: TalkToStep): Promise<void> {
    const bot = this.requireBot();
    // Walk to the NPC first (realism; some dialog effects are reach-gated), then
    // chat the dialog-option `/trigger` command the button would have run.
    await this.walkTo(step.pos, 3, `npc ${step.npc}`, step.sneak);
    bot.chat(step.command);
    // Give the datapack time to apply the dialog effect (e.g. open the gate).
    await delay(2_000);
  }

  /**
   * Walk to the anchor using the pathfinder. The datapack completes the objective
   * with a `distance=..radius` check on the exact anchor point, so the bot targets
   * a goal one block *tighter* than `radius` — landing well inside the check rather
   * than on its boundary (where the pathfinder's block-granular goal and the
   * server's precise-position check can disagree).
   */
  async reach(step: ReachStep): Promise<void> {
    await this.walkTo(step.pos, Math.max(1, step.radius - 1), `anchor ${step.anchor}`, step.sneak);
  }

  /**
   * Pathfind to within `range` blocks of the absolute target (mineflayer-pathfinder
   * `GoalNear`). Replaces the pre-v0.3 "face + hold forward" walk, so turns and
   * branches in jigsaw layouts are walkable. Digging is disabled (adventure mode).
   * A `sneak` leg (gap 7) walks crouched with sprinting disabled; the crouch is
   * restored to off afterwards so a later plain leg is not left sneaking. The long
   * `goto` wait races the death latch so a death aborts it fast, not after ~60s.
   */
  private async walkTo(
    pos: readonly [number, number, number],
    range: number,
    label: string,
    sneak = false,
  ): Promise<void> {
    const bot = this.requireBot();
    const r = Math.max(1, Math.floor(range));
    const movements = new Movements(bot);
    const restoreControls = configureLeg(bot, movements, sneak);
    // task #38: no cave-specific Movements override is needed — the compiler-proven
    // waypoints keep the bot on clear standable ground (the DW0311 A* treats water
    // and fences as impassable, so the route never crosses them, and gravity blocks
    // and stairs are ordinary floor). Entity detection is left ON (the pathfinder
    // default) so the bot routes AROUND a transient mob on a hop rather than ramming
    // it — disabling it made the bot wedge against a leaked mob and time out.
    // task #45: but the pathfinder's default treats EVERY non-passable entity as an
    // obstacle, including non-colliding display/interaction/marker entities that
    // block nothing in-world. Those (a completed interact objective's leaked
    // `interaction` hitbox, an NPC's co-located hitbox, floating item/text displays)
    // congested the terminal approach to an NPC and timed the leg out. Mark them
    // passable so the bot paths through them — physics-honest, and solid entities
    // (mobs, the mannequin NPC itself) are still avoided.
    allowNonCollidingEntities(movements);
    bot.pathfinder.setMovements(movements);
    // Long multi-level layouts (e.g. a 5-storey keep, ~90 blocks + 4 staircases)
    // sit at the edge of the default A* budget and fail nondeterministically
    // with "No path to the goal!" — give the search real headroom. With leg-by-leg
    // waypoints each solve is tiny, so this budget is only a safety margin.
    bot.pathfinder.thinkTimeout = 30_000;
    try {
      // task #38: when the compiler proved a waypoint polyline for this leg, replay
      // it as short hops so each A* solve is trivial (avoids the single giant solve
      // that strands the bot on a large open winding cave); the final goal is always
      // the true destination. Legs are matched in lockstep path order and consumed
      // as walked; a non-matching walk (a sub-walk, or a post-transport step) does
      // not consume and falls back to the single destination goal.
      let legWaypoints: readonly Vec3Tuple[] | undefined;
      if (this.waypoints) {
        const match = nextLegWaypoints(this.waypoints.legs, this.legCursor, [
          pos[0],
          pos[1],
          pos[2],
        ]);
        legWaypoints = match.waypoints;
        this.legCursor = match.cursor;
      }
      const goalsList = walkGoals(legWaypoints, [pos[0], pos[1], pos[2]], r);
      await replayLegWithRecovery(
        goalsList,
        label,
        (spec, glabel) => this.runGoto(spec, glabel),
        (target) => this.unstickToward(target),
      );
    } finally {
      restoreControls();
    }
  }

  /**
   * Raw, pathfinder-free nudge toward `target` to dislodge a physically wedged bot
   * (task #45). When the stall-recovery pathfind itself can't escape a concave corner
   * beside a wall, this bypasses the A* pathfinder: clear controls, face the proven
   * cell, and drive forward for a SHORT burst — a gentle tap, not a launch, so on a
   * tight 2-wide corridor the bot edges toward the corridor axis instead of
   * overshooting to the far wall. Only if that gentle burst makes no progress (the
   * bot is truly stuck against a lip) does it add a jump. It deliberately does NOT
   * call `pathfinder.stop()`: the previous hop already returned, and stopping here
   * churns pathfinder state and interrupts the caller's very next `goto` ("Path was
   * stopped"). Navigation robustness, NOT game logic; the caller re-paths afterwards
   * and still fails loudly if the hop stays unwalkable.
   */
  private async unstickToward(target: GoalSpec): Promise<void> {
    const bot = this.requireBot();
    bot.clearControlStates();
    // Face the block-centre of the proven cell so the forward drive heads toward it.
    const p0 = bot.entity.position;
    try {
      await bot.lookAt(p0.offset(target.x + 0.5 - p0.x, 0, target.z + 0.5 - p0.z), true);
    } catch {
      // best effort — an unforced look failure must not abort the unstick
    }
    const before = bot.entity.position.clone();
    bot.setControlState("forward", true);
    await delay(UNSTICK_BURST_MS);
    // Jump only when the gentle forward burst got nowhere (wedged against a lip).
    if (bot.entity.position.distanceTo(before) < UNSTICK_MIN_PROGRESS) {
      bot.setControlState("jump", true);
      await delay(UNSTICK_BURST_MS);
      bot.setControlState("jump", false);
    }
    bot.setControlState("forward", false);
    bot.clearControlStates();
    await delay(UNSTICK_SETTLE_MS);
  }

  /**
   * Pathfind to a single {@link GoalSpec} (get within `spec.range` blocks of it),
   * with the death-aware, timed, one-retry behavior the critical path depends on: a
   * bot death rethrows the {@link BotDeathError} immediately (never retried across
   * the void); a transient failure is retried once after a settle (an `open-gate`
   * fill may land after the first path computation started); a persistent failure
   * throws a diagnostic naming the goal and the bot's position. The caller sets the
   * Movements and think budget once for the whole leg.
   *
   * Arrival is VERIFIED, not trusted: mineflayer's `goto` can resolve on a
   * best-effort partial path without the bot actually reaching the goal (observed on
   * an unwalkable waypoint hop — it "succeeds" while the bot sits blocks away). A
   * resolve that leaves the bot outside the goal range is treated as a failure, so a
   * stuck hop fails the step loudly instead of silently marching the walk forward.
   */
  private async runGoto(spec: GoalSpec, label: string): Promise<void> {
    const bot = this.requireBot();
    const { x, y, z, range } = spec;
    let lastErr: unknown;
    for (let attempt = 0; attempt < 2; attempt++) {
      if (attempt > 0) {
        await delay(1_500);
      }
      try {
        await this.raceDeath(
          withTimeout(
            bot.pathfinder.goto(new goals.GoalNear(x, y, z, range)),
            REACH_TIMEOUT_MS,
            `reaching ${label}`,
          ),
        );
        if (this.withinGoal(spec)) {
          return;
        }
        throw new Error(
          `pathfinder resolved but the bot is at ${fmt(bot.entity.position)}, ` +
            `not within ${range} of the goal`,
        );
      } catch (err) {
        // A death is terminal for this run — never retry a path across the void.
        if (err instanceof BotDeathError) throw err;
        lastErr = err;
        try {
          bot.pathfinder.stop();
        } catch {
          // ignore
        }
      }
    }
    const detail = lastErr instanceof Error ? lastErr.message : String(lastErr);
    const near = Object.values(bot.entities)
      .filter((e) => e && e !== bot.entity && bot.entity.position.distanceTo(e.position) < 12)
      .map((e) => `${e.name ?? "?"}@${e.position.distanceTo(bot.entity.position).toFixed(1)}`);
    process.stderr.write(`[stuck] near ${fmt(bot.entity.position)}: ${near.join(", ") || "none"}\n`);
    throw new Error(
      `failed ${label} at [${x}, ${y}, ${z}] (range ${range}); bot at ` +
        `${fmt(bot.entity.position)}: ${detail}`,
    );
  }

  /**
   * Whether the bot's block position is within `spec.range` blocks of the goal cell
   * — the same block-distance metric mineflayer-pathfinder's `GoalNear` uses to
   * decide it arrived. The `y` axis is given one extra block of slack so standing on
   * a stair/slab (a fractional-height floor) still counts as arrived.
   */
  private withinGoal(spec: GoalSpec): boolean {
    const p = this.requireBot().entity.position;
    const dx = Math.floor(p.x) - spec.x;
    const dz = Math.floor(p.z) - spec.z;
    const dy = Math.floor(p.y) - spec.y;
    const yTol = spec.range + 1;
    return dx * dx + dz * dz <= spec.range * spec.range && Math.abs(dy) <= yTol;
  }

  /**
   * Slay a wave: go to the wave anchor, then attack the nearest hostile mob in a
   * loop until none remain (the world is sealed, so the only mobs are the wave)
   * or the budget runs out. The datapack's kill advancement + countdown complete
   * the objective when the last mob dies.
   */
  async kill(step: KillStep): Promise<void> {
    const bot = this.requireBot();
    await this.equipLoadout();
    await this.walkTo(step.pos, 3, `wave ${step.wave}`, step.sneak);
    // Give AI-enabled mobs a moment to path toward the bot after we arrive.
    await delay(1_000);
    // Diagnostic: what does the bot see near the wave anchor?
    const near = Object.values(bot.entities)
      .filter((e) => e && e !== bot.entity && bot.entity.position.distanceTo(e.position) < 48)
      .map((e) => `${e.name ?? "?"}(t=${e.type},k=${(e as { kind?: string }).kind ?? "?"},h=${e.height ?? "?"})`);
    process.stderr.write(`[kill ${step.wave}] nearby(${near.length}): ${near.join(", ") || "none"}\n`);

    const deadline = Date.now() + KILL_TIMEOUT_MS;
    let emptyStreak = 0;
    while (Date.now() < deadline) {
      // Fail fast if a mob killed the bot mid-fight (gap 7) rather than looping.
      if (this.death) throw this.death;
      const mob = bot.nearestEntity((e) => isWaveMob(e, bot.entity));
      if (!mob) {
        // A sustained absence of wave mobs (world is sealed) → wave cleared.
        if (++emptyStreak >= 8) return;
        await delay(REACH_POLL_MS);
        continue;
      }
      emptyStreak = 0;
      const dist = bot.entity.position.distanceTo(mob.position);
      if (dist > 3) {
        await this.walkTo(
          [Math.floor(mob.position.x), Math.floor(mob.position.y), Math.floor(mob.position.z)],
          2,
          `mob ${mob.name ?? "?"}`,
          step.sneak,
        );
      } else {
        await bot.lookAt(mob.position.offset(0, (mob.height ?? 1) * 0.5, 0), true);
        bot.attack(mob);
        await delay(ATTACK_INTERVAL_MS);
      }
    }
    throw new Error(
      `kill timed out after ${KILL_TIMEOUT_MS}ms: wave ${step.wave} ` +
        `(${step.count} mobs) not cleared`,
    );
  }

  /** Collect items from the chest at the anchor: go there, open it, withdraw all. */
  async collect(step: CollectStep): Promise<void> {
    const bot = this.requireBot();
    await this.walkTo(step.pos, 2, `chest ${step.item}`, step.sneak);
    const here = bot.entity.position;
    const target = here.offset(
      step.pos[0] + 0.5 - here.x,
      step.pos[1] + 0.5 - here.y,
      step.pos[2] + 0.5 - here.z,
    );
    const block = bot.blockAt(target);
    if (!block) {
      throw new Error(`no block at collect anchor [${step.pos.join(", ")}]`);
    }
    const chest = await bot.openContainer(block);
    try {
      for (const item of chest.containerItems()) {
        await chest.withdraw(item.type, null, item.count);
      }
    } finally {
      chest.close();
    }
    // Let the inventory_changed advancement fire the completion.
    await delay(SCORE_POLL_MS * 4);
  }

  /** Interact at the anchor: go there, then chat the emitted `/trigger` command. */
  async interact(step: InteractStep): Promise<void> {
    const bot = this.requireBot();
    await this.walkTo(step.pos, 3, `interact ${step.anchor}`, step.sneak);
    // The interaction advancement and this chat command both feed the same
    // per-tick handler; the datapack applies the requires_item + flag guards.
    bot.chat(step.command);
    await delay(2_000);
  }

  /**
   * gap 8: after a step whose completion teleports the player to another area, hold
   * the next step's pathfinding until the relocation has fully landed. Areas sit
   * ~256 blocks apart across void, so the destination is far from the pre-teleport
   * position and the arrival is unambiguous. Navigation plumbing only — no game logic.
   *
   * Three deterministic phases (task #32), each bounded and death-aware so nothing
   * can hang the run and a mid-transport death still fails fast:
   *   1. Wait for the position to jump to near `dest` — the teleport landing. The
   *      `forcedMove` handler resets the pathfinder as the jump arrives, so a path
   *      computed in the old area cannot survive it.
   *   2. Reset the pathfinder again here (belt-and-braces): the next `walkTo` must
   *      start from a clean state at the new position.
   *   3. Wait for the destination chunk to load and the bot to rest on solid footing.
   *      A `walkTo` that starts while the block under the bot is still unloaded
   *      (`blockAt` → null) makes the pathfinder's A* fail instantly with "No path to
   *      the goal!"; this closes that race at the boundary rather than racing ahead.
   * If the jump is not observed within the budget, settle briefly and let the next
   * step surface its own diagnostic.
   */
  async awaitTransport(dest: readonly [number, number, number]): Promise<void> {
    const bot = this.requireBot();
    const [x, y, z] = dest;
    const arrived = await this.waitFor(
      () => {
        const p = bot.entity.position;
        return (
          Math.abs(p.x - (x + 0.5)) < TRANSPORT_NEAR &&
          Math.abs(p.z - (z + 0.5)) < TRANSPORT_NEAR &&
          Math.abs(p.y - y) < 4
        );
      },
      TRANSPORT_TIMEOUT_MS,
      REACH_POLL_MS,
    );
    // Drop any path/goal still referencing the old area (the forcedMove handler has
    // usually done this already; idempotent).
    try {
      bot.pathfinder.stop();
    } catch {
      // best effort
    }
    if (!arrived) {
      process.stderr.write(
        `[transport] did not observe the jump to [${x}, ${y}, ${z}] within ` +
          `${TRANSPORT_TIMEOUT_MS}ms; bot at ${fmt(bot.entity.position)} — continuing\n`,
      );
      await delay(TRANSPORT_SETTLE_MS);
      return;
    }
    // The jump landed: wait for the destination chunk to load and the bot to settle
    // onto solid ground before the next step pathfinds from here.
    const footed = await this.waitFor(
      () => bot.entity.onGround === true && bot.blockAt(bot.entity.position) != null,
      FOOTING_TIMEOUT_MS,
      FOOTING_POLL_MS,
    );
    if (!footed) {
      process.stderr.write(
        `[transport] landed near [${x}, ${y}, ${z}] but footing/chunk not confirmed ` +
          `within ${FOOTING_TIMEOUT_MS}ms; bot at ${fmt(bot.entity.position)} — continuing\n`,
      );
    }
    await delay(TRANSPORT_SETTLE_MS);
  }

  /**
   * Poll `predicate` every `pollMs` until it holds (→ true) or `timeoutMs` elapses
   * (→ false). Death-aware: throws the recorded {@link BotDeathError} the moment the
   * bot dies, so a transport/footing wait never outlives a death. A pure timing helper
   * — no game logic.
   */
  private async waitFor(
    predicate: () => boolean,
    timeoutMs: number,
    pollMs: number,
  ): Promise<boolean> {
    const deadline = Date.now() + timeoutMs;
    for (;;) {
      if (this.death) throw this.death;
      if (predicate()) return true;
      if (Date.now() >= deadline) return false;
      await delay(pollMs);
    }
  }

  /**
   * gap 7 (cutscene): after a step marked `cutscene_seconds`, the compiler may force
   * the bot into spectator and dolly a camera for ~n seconds, then restore gamemode
   * and position. The harness makes no assertions about the cutscene — it only waits
   * for control to return so the next step does not start pathfinding mid-spectator.
   *
   * Two phases, both death-aware and bounded (deadline = n + grace, so a cutscene
   * glitch cannot hang the run):
   *   1. Sleep through the declared duration — the bot is out of our control anyway
   *      (this also covers a cutscene brief enough that we would otherwise miss the
   *      spectator window entirely).
   *   2. Extend the awaitTransport discontinuity pattern: wait for the gamemode to be
   *      back to adventure AND the position to hold steady for a short settle window.
   */
  async awaitCutscene(seconds: number): Promise<void> {
    const bot = this.requireBot();
    const start = Date.now();
    const minEnd = start + seconds * 1000;
    const deadline = minEnd + this.cutsceneGraceMs;

    // Phase 1: wait out the declared cutscene length (control is not ours meanwhile).
    while (Date.now() < minEnd) {
      if (this.death) throw this.death;
      await delay(CUTSCENE_POLL_MS);
    }

    // Phase 2: confirm control returned — adventure mode AND a settled position.
    let steadySince: number | undefined;
    let last = bot.entity.position.clone();
    while (Date.now() < deadline) {
      if (this.death) throw this.death;
      const here = bot.entity.position;
      const moved = here.distanceTo(last) > CUTSCENE_STEADY_EPS;
      last = here.clone();
      if (bot.game.gameMode === "adventure" && !moved) {
        steadySince ??= Date.now();
        if (Date.now() - steadySince >= CUTSCENE_SETTLE_MS) return;
      } else {
        steadySince = undefined;
      }
      await delay(CUTSCENE_POLL_MS);
    }
    process.stderr.write(
      `[cutscene] control not confirmed restored within ${seconds}s + grace; ` +
        `gamemode ${bot.game.gameMode}, bot at ${fmt(bot.entity.position)} — continuing\n`,
    );
  }

  async assertComplete(step: AssertCompleteStep): Promise<void> {
    const bot = this.requireBot();
    // Completion is observed two ways, whichever surfaces first:
    //   1. The broadcast completion marker (the working path on 1.21.11 — see
    //      COMPLETION_MARKER), buffered since connect.
    //   2. The sidebar score via mineflayer (future-proof: works if/when mineflayer
    //      gains 1.21.11 score-packet support; currently always unset).
    // The score is set on a server tick after the final reach, so poll until it
    // settles or the budget runs out.
    const deadline = Date.now() + SCORE_SETTLE_MS;
    while (Date.now() < deadline) {
      if (this.death) throw this.death;
      if (this.markerScores.get(step.objective) === step.value) {
        return;
      }
      const board = bot.scoreboards[step.objective];
      if (board?.itemsMap[bot.username]?.value === step.value) {
        return;
      }
      await delay(SCORE_POLL_MS);
    }
    const marker = this.markerScores.has(step.objective)
      ? `${this.markerScores.get(step.objective)}`
      : "no marker received";
    const board = bot.scoreboards[step.objective];
    const sidebar = board?.itemsMap[bot.username]?.value ?? "unset";
    throw new Error(
      `campaign not complete after ${SCORE_SETTLE_MS}ms: objective ${step.objective} ` +
        `expected ${step.value}; completion marker: ${marker}; sidebar: ${sidebar}`,
    );
  }
}

function fmt(p: { x: number; y: number; z: number }): string {
  return `[${p.x.toFixed(1)}, ${p.y.toFixed(1)}, ${p.z.toFixed(1)}]`;
}

/**
 * Entity names that are never a combat target. The delve world is sealed
 * (`spawn_mobs false`), so the only living mobs are compiler-summoned wave mobs;
 * everything else near the bot is an NPC, a display, or a dropped object.
 */
const NON_WAVE_ENTITIES = new Set<string>([
  "player",
  "villager",
  "interaction",
  "item",
  "experience_orb",
  "arrow",
  "spectral_arrow",
  "armor_stand",
  "marker",
  "text_display",
  "block_display",
  "item_display",
  "area_effect_cloud",
  "item_frame",
  "glow_item_frame",
  "painting",
  "leash_knot",
  "fishing_bobber",
]);

/**
 * True if `e` is a slayable wave mob: not the bot, not a player/NPC/display/
 * dropped object, and tall enough to be a living mob (excludes small dropped
 * entities). Classified by name (reliable across mineflayer versions) rather than
 * `type`/`kind`, which vary.
 */
function isWaveMob(e: unknown, self: unknown): boolean {
  if (!e || e === self) return false;
  const ent = e as { name?: string; height?: number };
  const name = ent.name ?? "";
  if (name === "" || NON_WAVE_ENTITIES.has(name)) return false;
  return (ent.height ?? 0) >= 0.5;
}
