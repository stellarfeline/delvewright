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
import type {
  AssertCompleteStep,
  ReachStep,
  SelectClassStep,
  TalkToStep,
} from "./critical-path.ts";
import type { StepExecutor } from "./sequencer.ts";

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
/**
 * How long (ms) to wait for a scoreboard value to reach its target after a chat
 * command. The datapack acts on the trigger on the next server tick(s); give it a
 * generous window so slow CI servers don't flake.
 */
const SCORE_SETTLE_MS = 15_000;
const SCORE_POLL_MS = 250;
/** Settle time (ms) after class selection (teleport + kit give) before moving on. */
const CLASS_SETTLE_MS = 3_000;

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
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

  constructor(config: BotConfig) {
    this.config = config;
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

    // Capture completion markers from the moment we connect: the marker is
    // broadcast when the campaign completes, which happens DURING the final reach
    // step (before assertComplete runs), so it must be buffered as it arrives.
    bot.on("messagestr", (message: string) => {
      const match = COMPLETION_MARKER.exec(message);
      if (match) {
        this.markerScores.set(match[1]!, Number.parseInt(match[2]!, 10));
      }
    });

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
  }

  async talkTo(step: TalkToStep): Promise<void> {
    const bot = this.requireBot();
    // Walk to the NPC first (realism; some dialog effects are reach-gated), then
    // chat the dialog-option `/trigger` command the button would have run.
    await this.walkTo(step.pos, 3, `npc ${step.npc}`);
    bot.chat(step.command);
    // Give the datapack time to apply the dialog effect (e.g. open the gate).
    await delay(2_000);
  }

  /**
   * Walk to within `radius` blocks (horizontal distance) of the absolute target.
   * Simple movement: face the target and hold forward, polling until arrival or
   * timeout. Deliberately not a full pathfinder (mineflayer only, no plugin) —
   * adequate for M1's flat single-prefab layouts; jigsaw/complex terrain in M2 may
   * warrant mineflayer-pathfinder (flagged as a future dependency).
   */
  async reach(step: ReachStep): Promise<void> {
    await this.walkTo(step.pos, step.radius, `anchor ${step.anchor}`);
  }

  private async walkTo(
    pos: readonly [number, number, number],
    radius: number,
    label: string,
  ): Promise<void> {
    const bot = this.requireBot();
    const [x, y, z] = pos;
    const here = bot.entity.position;
    const target = here.offset(x + 0.5 - here.x, y - here.y, z + 0.5 - here.z);

    const deadline = Date.now() + REACH_TIMEOUT_MS;
    try {
      while (bot.entity.position.xzDistanceTo(target) > radius) {
        if (Date.now() > deadline) {
          const dist = bot.entity.position.xzDistanceTo(target).toFixed(2);
          throw new Error(
            `timed out after ${REACH_TIMEOUT_MS}ms reaching ${label} ` +
              `at [${x}, ${y}, ${z}] (radius ${radius}); still ${dist} blocks away ` +
              `(bot at ${fmt(bot.entity.position)})`,
          );
        }
        await bot.lookAt(target, true);
        bot.setControlState("forward", true);
        // A short jump helps clear the 1-block sill some prefab doorways have.
        bot.setControlState("jump", bot.entity.onGround);
        await delay(REACH_POLL_MS);
      }
    } finally {
      bot.setControlState("forward", false);
      bot.setControlState("jump", false);
    }
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
