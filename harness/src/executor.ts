// mineflayer-backed StepExecutor skeleton. Connects a headless bot to a pinned
// 1.21.11 server and drives the critical path. Only `reach` and `assert-complete`
// are implemented for M1; `select-class` and `talk-to` are TODO stubs pending the
// dialog-vs-tellraw planning decision (see the investigation in the m1-harness
// report / spec-0001 dialogue routing).
//
// Interaction-channel caveat (planning-gating, do NOT design around here):
// Minecraft 1.21.6+ routes NPC dialogue / class selection through the server-driven
// dialog system. mineflayer 4.37.1 receives `show_dialog` packets at the raw
// protocol level but exposes NO high-level dialog API, and emitting the
// `custom_click_action` response for a button click is unsupported/fragile. The
// likely fallback is chat-based `tellraw` click_events (`run_command`) + `/trigger`,
// which a bot drives with `bot.chat('/…')`. That choice changes compiler emission,
// so select-class / talk-to stay stubbed until it is decided.

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

/** How long (ms) a `reach` step may run before it is declared failed. */
const REACH_TIMEOUT_MS = 30_000;
/** Polling interval (ms) while walking toward a reach target. */
const REACH_POLL_MS = 250;

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * A mineflayer-backed executor. Construct, `await connect()`, then hand it to
 * `runSequence`. `close()` disconnects the bot. Not reusable across servers.
 */
export class MineflayerExecutor implements StepExecutor {
  private readonly config: BotConfig;
  private bot: Bot | undefined;

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
    // TODO(m1-planning): implement once the class-selection interaction channel
    // is decided (server-driven dialog vs tellraw+/trigger). See file header.
    throw new Error(
      `select-class not implemented (class=${step.class}, option_path=[${step.optionPath.join(",")}]): ` +
        "dialog-vs-tellraw interaction channel is an open planning decision",
    );
  }

  async talkTo(step: TalkToStep): Promise<void> {
    // TODO(m1-planning): implement once the NPC-dialogue interaction channel is
    // decided (server-driven dialog vs tellraw+/trigger). See file header.
    throw new Error(
      `talk-to not implemented (npc=${step.npc}, option_path=[${step.optionPath.join(",")}]): ` +
        "dialog-vs-tellraw interaction channel is an open planning decision",
    );
  }

  /**
   * Walk to within `radius` blocks (horizontal distance) of the absolute target.
   * Simple movement: face the target and hold forward, polling until arrival or
   * timeout. Deliberately not a full pathfinder (mineflayer only, no plugin) —
   * adequate for M1's flat single-prefab layouts; jigsaw/complex terrain in M2 may
   * warrant mineflayer-pathfinder (flagged as a future dependency).
   */
  async reach(step: ReachStep): Promise<void> {
    const bot = this.requireBot();
    const [x, y, z] = step.pos;
    // Aim for the block centre. Build an absolute Vec3 by offsetting the current
    // position (avoids importing the transitive `vec3` package directly).
    const here = bot.entity.position;
    const target = here.offset(x + 0.5 - here.x, y - here.y, z + 0.5 - here.z);

    const deadline = Date.now() + REACH_TIMEOUT_MS;
    try {
      while (bot.entity.position.xzDistanceTo(target) > step.radius) {
        if (Date.now() > deadline) {
          const dist = bot.entity.position.xzDistanceTo(target).toFixed(2);
          throw new Error(
            `timed out after ${REACH_TIMEOUT_MS}ms reaching anchor ${step.anchor} ` +
              `at [${x}, ${y}, ${z}] (radius ${step.radius}); still ${dist} blocks away`,
          );
        }
        await bot.lookAt(target, true);
        bot.setControlState("forward", true);
        await delay(REACH_POLL_MS);
      }
    } finally {
      bot.setControlState("forward", false);
    }
  }

  async assertComplete(step: AssertCompleteStep): Promise<void> {
    const bot = this.requireBot();
    // The bot can only observe a scoreboard objective that the datapack makes
    // client-visible (a display slot). The compiler must display the completion
    // objective for the bot tier to read it; otherwise `scoreboards` is empty.
    const board = bot.scoreboards[step.objectiveScoreboard];
    if (!board) {
      throw new Error(
        `scoreboard objective ${step.objectiveScoreboard} not visible to the bot ` +
          "(is it assigned to a display slot by the datapack?)",
      );
    }
    const item = board.itemsMap[bot.username];
    const actual = item?.value;
    if (actual !== step.value) {
      throw new Error(
        `campaign not complete: ${step.objectiveScoreboard}[${bot.username}] = ` +
          `${actual ?? "unset"}, expected ${step.value}`,
      );
    }
  }
}
